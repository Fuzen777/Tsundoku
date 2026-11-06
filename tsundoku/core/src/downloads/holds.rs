//! Why a waiting download doesn't start yet: its site answered "too many requests" (`site_limits`), or the hourly limit
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
}

impl Hold {
    /// When it ends, in milliseconds since 1970.
    pub fn until(&self) -> i64 {
        match self {
            Self::SlowDown { until, .. } | Self::HourlyLimit { until, .. } => *until,
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

    /// Works out each waiting item's hold at `now`, and clears every other item's.
    pub(super) fn settle_holds(&self, items: &mut [QueueItem], now: i64) {
        let options = self.settings.get().downloads;
        for item in items {
            item.hold = if item.state == QueueState::Waiting {
                self.hold_for(item, &options, now)
            } else {
                None
            };
        }
    }

    /// Why `item` can't start at `now`: its site waits after a "too many requests", or the hourly limit on a paced
    /// site's files leaves no room for its files (a post's, or one for a video).
    fn hold_for(&self, item: &QueueItem, options: &DownloadSettings, now: i64) -> Option<Hold> {
        let site = sites::site_of(&item.video.url)?;
        if let Some(wait) = self.limits.wait(&site.key, now) {
            return Some(Hold::SlowDown {
                site: wait.site,
                until: wait.until,
            });
        }
        let limit = DownloadSettings::normalized_hourly_limit(options.hourly_file_limit)
            .filter(|_| sites::paces_downloads(&item.video.url))?;
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
