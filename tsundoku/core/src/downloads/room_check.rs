//! Room on its drive for a download about to start (`room`): one that won't fit waits with `Hold::NotEnoughRoom`
//! instead of starting, while waiting downloads that fit go ahead of it. It's checked again whenever the queue
//! schedules (a download ends, is removed, retried or resumed) and every 30 seconds, and starts by itself once there's
//! room. A download whose size isn't known starts as before.

use std::collections::HashMap;
use std::path::Path;
#[cfg(test)]
use std::sync::Arc;

use super::{DownloadQueue, Hold, QueueItem, QueueState};
use crate::room;
#[cfg(test)]
use crate::room::DiskSpace;
use crate::sync::lock;

/// How long a download without room waits before its drive is looked at again, besides each time the queue schedules.
const RECHECK_MS: i64 = 30_000;

/// What the downloads started on each drive still need, by the drive's name in lowercase, set aside while the queue
/// decides what else can start.
pub(super) struct Reserved(HashMap<String, u64>);

impl Reserved {
    /// What each running download is expected to take, less what it has downloaded so far.
    pub(super) fn of_running(items: &[QueueItem]) -> Self {
        let mut reserved = HashMap::new();
        for item in items
            .iter()
            .filter(|item| item.state == QueueState::Running)
        {
            let Some(bytes) = room::expected_bytes(&item.video, item.preset.as_ref()) else {
                continue;
            };
            let done = item
                .progress
                .as_ref()
                .and_then(|progress| progress.downloaded_bytes)
                .unwrap_or(0);
            *reserved.entry(drive_key(&item.folder)).or_insert(0) += bytes.saturating_sub(done);
        }
        Self(reserved)
    }
}

/// The drive of a download's folder, to add up what each drive's downloads need.
fn drive_key(folder: &str) -> String {
    room::drive_name(Path::new(folder)).to_lowercase()
}

impl DownloadQueue {
    /// Why `item`, about to start at `now`, can't: its drive hasn't room for it besides what the downloads started
    /// there still need. When it has, what it needs is set aside for the next one to start.
    pub(super) fn room_hold(
        &self,
        item: &QueueItem,
        reserved: &mut Reserved,
        now: i64,
    ) -> Option<Hold> {
        let bytes = room::expected_bytes(&item.video, item.preset.as_ref())?;
        let space = lock(&self.disks).free(Path::new(&item.folder))?;
        let key = drive_key(&item.folder);
        let others = reserved.0.get(&key).copied().unwrap_or(0);
        if let Some(short) = room::shortfall(&space, bytes, others) {
            return Some(Hold::NotEnoughRoom {
                drive: short.drive,
                free: short.free,
                needed: short.needed,
                others: short.others,
                until: now + RECHECK_MS,
            });
        }
        *reserved.0.entry(key).or_insert(0) += bytes;
        None
    }

    /// Answers how much room drives have with `disks` instead of Windows.
    #[cfg(test)]
    pub(crate) fn use_disk_space(&self, disks: Arc<dyn DiskSpace>) {
        *lock(&self.disks) = disks;
    }

    /// Looks at the waiting downloads' room again now, as the queue does every 30 seconds.
    #[cfg(test)]
    pub(crate) fn check_room_again(&self) {
        self.schedule();
    }
}
