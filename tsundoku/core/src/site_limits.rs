//! Keeping to sites' limits. A site that answers "too many requests" (HTTP 429) gets none of its downloads started
//! and none of its links read until a time, which grows with each such answer in a row and is shared by the queue, the
//! pending links' reader and the preview card. And a paced site's files (Instagram's) can be held to a number an hour.
//! The queue owns the waits (`DownloadQueue::site_wait`); see `DOCUMENTATION/Docs/Download Pipeline.md`.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::sites::Site;
use crate::sync::lock;

/// The wait after a site's first "too many requests". Neither yt-dlp nor gallery-dl passes on the site's Retry-After,
/// so it's Tsundoku's own.
pub const FIRST_WAIT: Duration = Duration::from_secs(5 * 60);
/// The longest wait: each "too many requests" in a row doubles the one before (5, 10, 20, 40 minutes), up to this.
pub const LONGEST_WAIT: Duration = Duration::from_secs(60 * 60);
/// A wait that ended this long ago without another "too many requests" is forgotten: the next one waits
/// `FIRST_WAIT` again, as after the site worked.
const FORGET_AFTER: Duration = LONGEST_WAIT;
/// The window of the hourly limit, in milliseconds.
pub const HOUR_MS: i64 = 60 * 60 * 1000;

/// A site's downloads and reads wait until `until`, after it answered "too many requests".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SiteWait {
    /// The site's name, as the user knows it ("Instagram").
    pub site: String,
    /// In milliseconds since 1970.
    #[ts(type = "number")]
    pub until: i64,
}

/// Each site's wait, by `Site::key`.
#[derive(Debug, Default)]
pub struct SiteLimits {
    sites: Mutex<HashMap<String, Cooldown>>,
}

#[derive(Debug)]
struct Cooldown {
    name: String,
    /// In milliseconds since 1970.
    until: i64,
    /// "Too many requests" answers since the site last worked.
    strikes: u32,
}

impl SiteLimits {
    pub fn new() -> Self {
        Self::default()
    }

    /// The site answered "too many requests" at `now`: it waits `FIRST_WAIT`, doubled for each answer since it last
    /// worked, up to `LONGEST_WAIT`. An answer while it already waits (a download started before the wait) leaves the
    /// wait as it is.
    pub fn slow_down(&self, site: &Site, now: i64) -> SiteWait {
        let mut sites = lock(&self.sites);
        let cooldown = sites.entry(site.key.clone()).or_insert_with(|| Cooldown {
            name: site.name.clone(),
            until: now,
            strikes: 0,
        });
        if cooldown.until <= now {
            if now - cooldown.until > millis(FORGET_AFTER) {
                cooldown.strikes = 0;
            }
            cooldown.strikes = cooldown.strikes.saturating_add(1);
            cooldown.until = now + millis(wait_after(cooldown.strikes));
        }
        SiteWait {
            site: cooldown.name.clone(),
            until: cooldown.until,
        }
    }

    /// The site worked: a download from it finished or a link on it was read. Its next "too many requests" waits
    /// `FIRST_WAIT` again. A wait already running goes on: a download that started before it can still finish.
    pub fn worked(&self, key: &str) {
        if let Some(cooldown) = lock(&self.sites).get_mut(key) {
            cooldown.strikes = 0;
        }
    }

    /// Until when the site's downloads and reads wait, if they still do at `now`.
    pub fn wait(&self, key: &str, now: i64) -> Option<SiteWait> {
        lock(&self.sites)
            .get(key)
            .filter(|cooldown| cooldown.until > now)
            .map(|cooldown| SiteWait {
                site: cooldown.name.clone(),
                until: cooldown.until,
            })
    }

    /// Ends every wait, as if its time had come.
    #[cfg(test)]
    pub(crate) fn end_all(&self) {
        for cooldown in lock(&self.sites).values_mut() {
            cooldown.until = 0;
        }
    }
}

/// The wait after the `strikes`th "too many requests" in a row: `FIRST_WAIT`, doubled for each one after the first,
/// up to `LONGEST_WAIT`.
pub fn wait_after(strikes: u32) -> Duration {
    let doublings = strikes.saturating_sub(1).min(16);
    FIRST_WAIT.saturating_mul(1 << doublings).min(LONGEST_WAIT)
}

/// When a download of `files` files may start under a limit of `limit` files an hour, given the downloads that
/// finished in the past hour (`recent`: when each finished, in milliseconds since 1970, and how many files it made):
/// `None` when it may start now, else the moment enough of them are an hour old. One bigger than the limit starts once
/// the past hour holds none.
pub fn hourly_limit_end(recent: &[(i64, u32)], files: u32, limit: u32, now: i64) -> Option<i64> {
    let mut recent: Vec<(i64, u32)> = recent
        .iter()
        .copied()
        .filter(|(finished, _)| *finished > now - HOUR_MS)
        .collect();
    recent.sort_unstable_by_key(|(finished, _)| *finished);
    let needed = files.clamp(1, limit.max(1));
    let fits = |used: u32| used.saturating_add(needed) <= limit;
    let mut used: u32 = recent.iter().map(|(_, count)| *count).sum();
    if fits(used) {
        return None;
    }
    for (finished, count) in recent {
        used = used.saturating_sub(count);
        if fits(used) {
            return Some(finished + HOUR_MS);
        }
    }
    None
}

/// Now, in milliseconds since 1970.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

fn millis(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: i64 = 60 * 1000;

    fn instagram() -> Site {
        Site {
            key: "instagram.com".to_owned(),
            name: "Instagram".to_owned(),
        }
    }

    #[test]
    fn waits_grow_with_each_answer_in_a_row_up_to_an_hour() {
        let minutes: Vec<u64> = (1..=7)
            .map(|strikes| wait_after(strikes).as_secs() / 60)
            .collect();
        assert_eq!(minutes, [5, 10, 20, 40, 60, 60, 60]);
        assert_eq!(wait_after(u32::MAX), LONGEST_WAIT);
    }

    #[test]
    fn a_site_waits_then_waits_longer_until_it_works() {
        let limits = SiteLimits::new();
        let site = instagram();
        let first = limits.slow_down(&site, 0);
        assert_eq!(first.site, "Instagram");
        assert_eq!(first.until, 5 * MINUTE);
        assert_eq!(
            limits.wait("instagram.com", 4 * MINUTE),
            Some(first.clone())
        );
        assert_eq!(limits.wait("instagram.com", 5 * MINUTE), None, "over");
        assert_eq!(limits.wait("x.com", 0), None, "other sites go on");

        // Another answer while it waits (a download started before) doesn't make it longer.
        assert_eq!(limits.slow_down(&site, MINUTE), first);
        // The next one after it, without the site working in between, waits twice as long.
        assert_eq!(limits.slow_down(&site, 6 * MINUTE).until, 16 * MINUTE);

        limits.worked("instagram.com");
        assert_eq!(
            limits
                .wait("instagram.com", 7 * MINUTE)
                .map(|wait| wait.until),
            Some(16 * MINUTE),
            "a download that started before the wait and worked doesn't end it"
        );
        assert_eq!(
            limits.slow_down(&site, 20 * MINUTE).until,
            25 * MINUTE,
            "back to the first wait once it worked"
        );
    }

    #[test]
    fn a_long_quiet_spell_forgets_the_answers_before() {
        let limits = SiteLimits::new();
        let site = instagram();
        limits.slow_down(&site, 0);
        let later = 5 * MINUTE + millis(FORGET_AFTER) + 1;
        assert_eq!(limits.slow_down(&site, later).until, later + 5 * MINUTE);
    }

    #[test]
    fn the_hourly_limit_waits_for_old_downloads_to_drop_out() {
        let now = 2 * HOUR_MS;
        // 10 files 50 minutes ago, 20 files 10 minutes ago, and one download from before the past hour.
        let recent = [
            (now - 70 * MINUTE, 50),
            (now - 50 * MINUTE, 10),
            (now - 10 * MINUTE, 20),
        ];
        assert_eq!(
            hourly_limit_end(&recent, 5, 40, now),
            None,
            "30 + 5 fit in 40"
        );
        assert_eq!(
            hourly_limit_end(&recent, 15, 40, now),
            Some(now + 10 * MINUTE),
            "once the 10 files are an hour old"
        );
        assert_eq!(
            hourly_limit_end(&recent, 25, 40, now),
            Some(now + 50 * MINUTE),
            "once both are"
        );
        assert_eq!(
            hourly_limit_end(&recent, 100, 40, now),
            Some(now + 50 * MINUTE),
            "a post bigger than the limit goes once the hour is empty"
        );
        assert_eq!(hourly_limit_end(&[], 100, 40, now), None);
        assert_eq!(
            hourly_limit_end(&recent, 0, 30, now),
            Some(now + 10 * MINUTE)
        );
    }
}
