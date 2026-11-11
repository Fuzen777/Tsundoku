//! Why a waiting download doesn't start yet: it lost its connection or found its site's download limit used up, and
//! tries again by itself later (`Recovery`), its site answered "too many requests" (`site_limits`), or the hourly limit
//! on a paced site's files is reached. The queue works each waiting item's hold out whenever it schedules, and a
//! thread schedules again once one ends.

use std::sync::Weak;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{DownloadError, DownloadQueue, QueueItem, QueueState};
use crate::metadata::PreviewError;
use crate::settings::DownloadSettings;
use crate::site_limits::{self, SiteWait, hourly_limit_end, now_ms};
use crate::sites;
use crate::sync::lock;

/// How often the queue looks for a hold that ended.
const TICK: Duration = Duration::from_secs(1);

/// How long a download that lost its connection waits before each automatic retry, longer each time. Once they're
/// used up, it fails.
pub const CONNECTION_RETRY_WAITS: [Duration; 3] = [
    Duration::from_secs(30),
    Duration::from_secs(2 * 60),
    Duration::from_secs(8 * 60),
];
/// The wait after a site's download limit first runs out (MEGA's transfer quota, a Google Drive, Dropbox or OneDrive
/// file's).
pub const FIRST_LIMIT_WAIT: Duration = Duration::from_secs(30 * 60);
/// The longest wait for a download limit: each time it's still used up, the wait doubles, up to this. A download never
/// fails for it.
pub const LONGEST_LIMIT_WAIT: Duration = Duration::from_secs(4 * 60 * 60);

/// What a download's automatic retries have done so far, in memory only: put back to its start by a run that
/// finishes, and by Resume and Retry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recovery {
    /// Runs in a row that lost the connection.
    connection_losses: u32,
    /// Runs in a row that found the site's download limit used up.
    limits_reached: u32,
    /// Its own wait before the next run, after either: a `Hold::Retry` or `Hold::LimitReached`.
    wait: Option<Hold>,
}

impl Recovery {
    /// Whether a run that failed with `error` at `now` is tried again by itself, and when: a lost connection after
    /// `CONNECTION_RETRY_WAITS`, a used-up download limit after a wait that doubles from `FIRST_LIMIT_WAIT` up to
    /// `LONGEST_LIMIT_WAIT`. `None` means it fails.
    pub(super) fn after_failure(&mut self, error: &DownloadError, now: i64) -> Option<Hold> {
        let DownloadError::YtDlp { error } = error else {
            return None;
        };
        let hold = match error {
            PreviewError::Network { .. } => {
                let wait = CONNECTION_RETRY_WAITS.get(self.connection_losses as usize)?;
                self.connection_losses += 1;
                Hold::Retry {
                    attempt: self.connection_losses,
                    attempts: CONNECTION_RETRY_WAITS.len() as u32,
                    until: now + millis(*wait),
                }
            }
            PreviewError::LimitReached { site, .. } => {
                self.limits_reached = self.limits_reached.saturating_add(1);
                Hold::LimitReached {
                    site: site.clone(),
                    until: now + millis(limit_wait(self.limits_reached)),
                }
            }
            _ => return None,
        };
        self.wait = Some(hold.clone());
        Some(hold)
    }
}

/// The wait after a site's download limit was found used up `times` times in a row: `FIRST_LIMIT_WAIT`, doubled for
/// each time after the first, up to `LONGEST_LIMIT_WAIT`.
pub fn limit_wait(times: u32) -> Duration {
    let doublings = times.saturating_sub(1).min(16);
    FIRST_LIMIT_WAIT
        .saturating_mul(1 << doublings)
        .min(LONGEST_LIMIT_WAIT)
}

fn millis(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

/// Why a waiting download doesn't start yet, and until when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum Hold {
    /// The site answered "too many requests": none of its downloads start before `until`.
    SlowDown {
        site: String,
        /// In milliseconds since 1970.
        #[ts(type = "number")]
        until: i64,
    },
    /// The past hour holds `limit` files from the site already (Settings → Instagram).
    HourlyLimit {
        site: String,
        limit: u32,
        /// In milliseconds since 1970.
        #[ts(type = "number")]
        until: i64,
    },
    /// The connection dropped: it's tried again at `until`, the `attempt`th of `attempts` automatic retries.
    Retry {
        attempt: u32,
        attempts: u32,
        /// In milliseconds since 1970.
        #[ts(type = "number")]
        until: i64,
    },
    /// The site's download limit is used up for now (MEGA's transfer quota, a Google Drive or Dropbox file's): it's
    /// tried again at `until`.
    LimitReached {
        site: String,
        /// In milliseconds since 1970.
        #[ts(type = "number")]
        until: i64,
    },
    /// Its drive has `free` bytes, and it needs `needed` (with a margin) besides the `others` that the downloads
    /// running there still need (`room`). Checked again at `until`, and whenever a download ends or is removed.
    NotEnoughRoom {
        /// "D:", or "\\server\share".
        drive: String,
        #[ts(type = "number")]
        free: u64,
        #[ts(type = "number")]
        needed: u64,
        #[ts(type = "number")]
        others: u64,
        /// In milliseconds since 1970.
        #[ts(type = "number")]
        until: i64,
    },
}

impl Hold {
    /// When it ends, in milliseconds since 1970.
    pub fn until(&self) -> i64 {
        match self {
            Self::SlowDown { until, .. }
            | Self::HourlyLimit { until, .. }
            | Self::Retry { until, .. }
            | Self::LimitReached { until, .. } => *until,
            Self::NotEnoughRoom { until, .. } => *until,
        }
    }
}

impl DownloadQueue {
    /// Until when the site of `link` waits after answering "too many requests", if it does now. Its downloads don't
    /// start, and the pending links' reader and the preview card don't read its links, until then.
    pub fn site_wait(&self, link: &str) -> Option<SiteWait> {
        let site = sites::site_of(link)?;
        self.limits.wait(&site.key, now_ms())
    }

    /// The site of `link` answered "too many requests" to a read: its downloads and reads wait
    /// (`SiteLimits::slow_down`). The wait, unless the link has no site.
    pub fn slow_down(&self, link: &str) -> Option<SiteWait> {
        let wait = self.note_slow_down(link);
        self.schedule();
        wait
    }

    /// The site of `link` worked: a link on it was read, or a download from it finished.
    pub fn site_worked(&self, link: &str) {
        if let Some(site) = sites::site_of(link) {
            self.limits.worked(&site.key);
        }
    }

    /// Starts the site's wait without scheduling, for a run that ends while the queue is locked.
    pub(super) fn note_slow_down(&self, link: &str) -> Option<SiteWait> {
        let site = sites::site_of(link)?;
        Some(self.limits.slow_down(&site, now_ms()))
    }

    /// Works out each waiting item's hold at `now`, and clears every other item's. An item's own wait that ended is
    /// dropped.
    pub(super) fn settle_holds(&self, items: &mut [QueueItem], now: i64) {
        let options = self.settings.get().downloads;
        for item in items {
            if item
                .recovery
                .wait
                .as_ref()
                .is_some_and(|wait| wait.until() <= now)
            {
                item.recovery.wait = None;
            }
            item.hold = if item.state == QueueState::Waiting {
                self.hold_for(item, &options, now)
            } else {
                None
            };
        }
    }

    /// Try Now: starts a held download as soon as there's room, without waiting for its hold to end. A retry's or a
    /// download limit's wait is dropped; a site's "too many requests" wait is ended for all of the site's downloads and
    /// reads, though the next one waits longer if the site still refuses. The hourly limit is the user's own, so it's
    /// changed in Settings instead.
    pub fn try_now(&self, id: i64) -> Result<(), DownloadError> {
        {
            let mut state = lock(&self.state);
            let item = super::find(&mut state.items, id)?;
            match (item.state, &item.hold) {
                (QueueState::Waiting, Some(Hold::Retry { .. } | Hold::LimitReached { .. })) => {
                    item.recovery.wait = None;
                }
                (QueueState::Waiting, Some(Hold::SlowDown { .. })) => {
                    if let Some(site) = sites::site_of(&item.video.url) {
                        self.limits.end(&site.key, now_ms());
                    }
                    item.recovery.wait = None;
                }
                _ => return Err(DownloadError::WrongState),
            }
        }
        self.schedule();
        Ok(())
    }

    /// Why `item` can't start at `now`: its own wait after losing its connection or finding the site's download limit
    /// used up, its site's wait after a "too many requests", or the hourly limit on a paced site's files leaving no
    /// room for its files (a post's, or one for a video).
    fn hold_for(&self, item: &QueueItem, options: &DownloadSettings, now: i64) -> Option<Hold> {
        if let Some(wait) = &item.recovery.wait {
            return Some(wait.clone());
        }
        let site = sites::site_of(&item.video.url)?;
        if let Some(wait) = self.limits.wait(&site.key, now) {
            return Some(Hold::SlowDown {
                site: wait.site,
                until: wait.until,
            });
        }
        let limit = DownloadSettings::normalized_hourly_limit(options.hourly_file_limit)
            .filter(|_| sites::paced_site(&item.video.url) == Some(sites::PacedSite::Instagram))?;
        let since = (now - site_limits::HOUR_MS) / 1000;
        let recent: Vec<(i64, u32)> = self
            .history
            .recent_downloads(&item.video.site, since)
            .ok()?
            .into_iter()
            .map(|(finished, files)| (finished * 1000, files))
            .collect();
        let files = item
            .video
            .post
            .map_or(1, |post| post.images.saturating_add(post.videos));
        hourly_limit_end(&recent, files, limit, now).map(|until| Hold::HourlyLimit {
            site: site.name,
            limit,
            until,
        })
    }

    /// Ends every site's wait, as if its time had come.
    #[cfg(test)]
    pub(crate) fn end_site_waits(&self) {
        self.limits.end_all();
        self.schedule();
    }

    /// Whether a waiting item's hold has ended, so it may start.
    fn hold_ended(&self) -> bool {
        let now = now_ms();
        lock(&self.state).items.iter().any(|item| {
            item.state == QueueState::Waiting
                && item.hold.as_ref().is_some_and(|hold| hold.until() <= now)
        })
    }
}

/// Whether a run failed because its site answered "too many requests". It then waits instead of failing.
pub(super) fn asked_to_slow_down(error: &DownloadError) -> bool {
    matches!(
        error,
        DownloadError::YtDlp {
            error: PreviewError::TooManyRequests { .. }
        }
    )
}

/// Starts what was held once its hold ends, while the queue exists.
pub(super) fn watch_holds(queue: &Weak<DownloadQueue>) {
    loop {
        thread::sleep(TICK);
        let Some(queue) = queue.upgrade() else {
            return;
        };
        if queue.hold_ended() {
            queue.schedule();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(error: PreviewError) -> DownloadError {
        DownloadError::YtDlp { error }
    }

    fn lost_connection() -> DownloadError {
        failure(PreviewError::Network {
            detail: "timed out".to_owned(),
        })
    }

    #[test]
    fn instagram_signed_out_isnt_tried_again_but_a_dropped_connection_is() {
        let download_failure =
            |line: &str| failure(crate::gallery::classify_download_failure(Some(4), &[line]));
        let mut recovery = Recovery::default();
        let signed_out = download_failure(
            "[instagram][error] KeyError: 'xdt_api__v1__feed__user_timeline_graphql_connection'",
        );
        assert_eq!(recovery.after_failure(&signed_out, 0), None);
        let dropped = download_failure(
            "[instagram][error] HttpError: Connection aborted. ('Connection reset by peer')",
        );
        assert!(matches!(
            recovery.after_failure(&dropped, 0),
            Some(Hold::Retry { attempt: 1, .. })
        ));
    }

    #[test]
    fn a_used_up_limit_waits_longer_each_time_up_to_four_hours() {
        let minutes: Vec<u64> = (1..=6)
            .map(|times| limit_wait(times).as_secs() / 60)
            .collect();
        assert_eq!(minutes, [30, 60, 120, 240, 240, 240]);
        assert_eq!(limit_wait(u32::MAX), LONGEST_LIMIT_WAIT);
    }

    #[test]
    fn only_a_lost_connection_or_a_used_up_limit_is_tried_again() {
        let mut recovery = Recovery::default();
        for error in [
            DownloadError::NoFile,
            failure(PreviewError::sign_in("login required".to_owned())),
            failure(PreviewError::too_many_requests("HTTP Error 429".to_owned())),
        ] {
            assert_eq!(recovery.after_failure(&error, 0), None, "{error:?}");
        }
        assert_eq!(recovery, Recovery::default());

        let waits: Vec<Option<Hold>> = (0..4)
            .map(|_| recovery.after_failure(&lost_connection(), 0))
            .collect();
        assert_eq!(
            waits,
            [
                Some(Hold::Retry {
                    attempt: 1,
                    attempts: 3,
                    until: 30_000
                }),
                Some(Hold::Retry {
                    attempt: 2,
                    attempts: 3,
                    until: 120_000
                }),
                Some(Hold::Retry {
                    attempt: 3,
                    attempts: 3,
                    until: 480_000
                }),
                None,
            ],
            "three tries, then it fails"
        );

        let limit = failure(PreviewError::LimitReached {
            site: "Google Drive".to_owned(),
            detail: "Too many users have viewed or downloaded this file recently".to_owned(),
        });
        assert_eq!(
            recovery.after_failure(&limit, 1_000),
            Some(Hold::LimitReached {
                site: "Google Drive".to_owned(),
                until: 1_000 + 30 * 60 * 1000
            }),
            "a used-up limit counts apart from lost connections"
        );
    }
}
