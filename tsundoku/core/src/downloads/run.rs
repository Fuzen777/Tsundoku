//! The queue's current run: the downloads started since the queue last went from idle to busy, until each has ended.
//! Its overall progress fills Tsundoku's taskbar button, and the tray icon's tooltip sums it up.

use std::collections::BTreeMap;

use super::{DownloadProgress, DownloadStage, QueueItem, QueueState};

/// yt-dlp reports a fraction for each file it downloads. A video downloaded as two files (its video, then its audio)
/// counts its video file as this much of the whole, and its audio as the rest.
const VIDEO_SHARE: f64 = 0.9;

/// The downloads of the current run. Follows the queue's items and progress; empty while there's no run.
#[derive(Debug, Default)]
pub struct QueueRun {
    members: BTreeMap<i64, Member>,
}

#[derive(Debug)]
struct Member {
    state: QueueState,
    /// Its size in bytes, when known as a whole: a file host's link from the start, a single file once it downloads,
    /// and any download once done.
    size: Option<u64>,
    /// How much of it is downloaded, from 0 to 1, once known.
    fraction: Option<f64>,
    /// Its video came as a file of its own, so its audio file is the last part of it.
    split: bool,
}

impl Member {
    fn new(item: &QueueItem) -> Self {
        Self {
            state: item.state,
            size: item.video.files.map(|files| files.bytes),
            fraction: None,
            split: false,
        }
    }

    fn follow(&mut self, item: &QueueItem) {
        self.state = item.state;
        if item.state == QueueState::Done {
            self.fraction = Some(1.0);
            self.size = item
                .result
                .as_ref()
                .and_then(|result| result.file_size)
                .and_then(|size| u64::try_from(size).ok())
                .or(self.size);
        }
    }

    fn progress(&mut self, progress: &DownloadProgress) {
        let Some(fraction) = progress.fraction else {
            return;
        };
        match progress.stage {
            DownloadStage::DownloadingVideo => {
                self.split = true;
                self.size = None;
                self.fraction = Some(fraction * VIDEO_SHARE);
            }
            DownloadStage::DownloadingAudio if self.split => {
                self.fraction = Some(VIDEO_SHARE + fraction * (1.0 - VIDEO_SHARE));
            }
            DownloadStage::DownloadingAudio | DownloadStage::Downloading => {
                self.fraction = Some(fraction);
                self.size = self.size.or(progress.total_bytes);
            }
            // Starting, and what follows a download (merging, converting, a file host checking a file), keep the
            // fraction it reached.
            _ => {}
        }
    }

    /// Ended: done, or failed and not retried yet.
    fn has_ended(&self) -> bool {
        matches!(self.state, QueueState::Done | QueueState::Failed)
    }

    /// How much of it is over, from 0 to 1, and whether that's known: an ended download is all over, one waiting or
    /// starting hasn't begun.
    fn share_done(&self) -> (f64, bool) {
        if self.has_ended() {
            (1.0, true)
        } else {
            (self.fraction.unwrap_or(0.0), self.fraction.is_some())
        }
    }
}

impl QueueRun {
    /// Follows the queue's items. A download waiting or running joins the run (a new one if there was none), so do
    /// those added or resumed meanwhile. One removed from the queue leaves it, unless it was done (Clear Finished).
    /// The run ends once none of its downloads is waiting, running or stopped.
    pub fn update(&mut self, items: &[QueueItem]) {
        self.members.retain(|id, member| {
            member.state == QueueState::Done || items.iter().any(|item| item.id == *id)
        });
        for item in items {
            match self.members.get_mut(&item.id) {
                Some(member) => member.follow(item),
                None if matches!(item.state, QueueState::Waiting | QueueState::Running) => {
                    let mut member = Member::new(item);
                    if let Some(progress) = &item.progress {
                        member.progress(progress);
                    }
                    self.members.insert(item.id, member);
                }
                None => {}
            }
        }
        if self.members.values().all(Member::has_ended) {
            self.members.clear();
        }
    }

    /// A running download's progress.
    pub fn progress(&mut self, id: i64, progress: &DownloadProgress) {
        if let Some(member) = self.members.get_mut(&id) {
            member.progress(progress);
        }
    }

    pub fn summary(&self) -> RunSummary {
        let count = |state: QueueState| {
            self.members
                .values()
                .filter(|member| member.state == state)
                .count()
        };
        RunSummary {
            downloading: count(QueueState::Running),
            waiting: count(QueueState::Waiting),
            stopped: count(QueueState::Stopped),
            failed: count(QueueState::Failed),
            done: count(QueueState::Done),
            percent: self.overall().map(|done| (done * 100.0).floor() as u64),
        }
    }

    /// How much of the run is over, from 0 to 1: by bytes when every download's size is known, otherwise the average
    /// of each one's share. `None` until something is known.
    fn overall(&self) -> Option<f64> {
        let shares: Vec<(f64, bool, Option<u64>)> = self
            .members
            .values()
            .map(|member| {
                let (share, known) = member.share_done();
                (share, known, member.size)
            })
            .collect();
        if !shares.iter().any(|(_, known, _)| *known) {
            return None;
        }
        let sizes: Option<Vec<u64>> = shares.iter().map(|(_, _, size)| *size).collect();
        let total_size: u64 = sizes.iter().flatten().sum();
        match sizes {
            Some(sizes) if total_size > 0 => Some(
                shares
                    .iter()
                    .zip(sizes)
                    .map(|((share, _, _), size)| share * size as f64)
                    .sum::<f64>()
                    / total_size as f64,
            ),
            _ => Some(shares.iter().map(|(share, _, _)| share).sum::<f64>() / shares.len() as f64),
        }
    }
}

/// Where the run stands: how many of its downloads are in each state, and how much of it is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RunSummary {
    pub downloading: usize,
    pub waiting: usize,
    pub stopped: usize,
    pub failed: usize,
    pub done: usize,
    /// From 0 to 100; `None` while nothing is known, such as while every download is still starting.
    pub percent: Option<u64>,
}

/// What Tsundoku's taskbar button shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskbarProgress {
    /// No run: no bar.
    Hidden,
    /// A bar moving across, while nothing's size is known yet.
    Indeterminate,
    Normal(u64),
    /// Yellow: everything left in the run is stopped.
    Paused(u64),
    /// Red: a download failed.
    Failed(u64),
}

impl RunSummary {
    fn is_empty(&self) -> bool {
        self.downloading + self.waiting + self.stopped + self.failed + self.done == 0
    }

    pub fn taskbar(&self) -> TaskbarProgress {
        let percent = self.percent.unwrap_or(0);
        if self.is_empty() {
            TaskbarProgress::Hidden
        } else if self.failed > 0 {
            TaskbarProgress::Failed(percent)
        } else if self.downloading + self.waiting == 0 {
            TaskbarProgress::Paused(percent)
        } else if self.percent.is_none() {
            TaskbarProgress::Indeterminate
        } else {
            TaskbarProgress::Normal(percent)
        }
    }

    /// The tray icon's tooltip, such as "3 downloading · 45% · 1 failed", leaving out what's zero. `None` with no run,
    /// for the app's name.
    pub fn tooltip(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let counted = |count: usize, what: &str| (count > 0).then(|| format!("{count} {what}"));
        let parts: Vec<String> = [
            counted(self.downloading, "downloading"),
            counted(self.waiting, "waiting"),
            counted(self.stopped, "stopped"),
            self.percent
                .filter(|percent| *percent > 0)
                .map(|percent| format!("{percent}%")),
            counted(self.failed, "failed"),
        ]
        .into_iter()
        .flatten()
        .collect();
        Some(parts.join(" · "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloads::DownloadResult;
    use crate::downloads::tests::item_in;
    use crate::metadata::FileContents;

    fn item(id: i64, state: QueueState) -> QueueItem {
        QueueItem {
            id,
            ..item_in(state)
        }
    }

    fn done(id: i64, size: i64) -> QueueItem {
        QueueItem {
            result: Some(DownloadResult {
                file_path: String::new(),
                file_size: Some(size),
                download_id: id,
            }),
            ..item(id, QueueState::Done)
        }
    }

    fn at(stage: DownloadStage, fraction: f64, total_bytes: Option<u64>) -> DownloadProgress {
        DownloadProgress {
            fraction: Some(fraction),
            total_bytes,
            ..DownloadProgress::at(stage)
        }
    }

    #[test]
    fn a_run_starts_when_the_queue_gets_busy_and_ends_once_everything_has_ended() {
        let mut run = QueueRun::default();
        run.update(&[item(1, QueueState::Done), item(2, QueueState::Stopped)]);
        assert_eq!(run.summary(), RunSummary::default(), "nothing started yet");
        assert_eq!(run.summary().taskbar(), TaskbarProgress::Hidden);
        assert_eq!(run.summary().tooltip(), None);

        run.update(&[
            item(1, QueueState::Done),
            item(2, QueueState::Running),
            item(3, QueueState::Waiting),
        ]);
        let summary = run.summary();
        assert_eq!(
            (summary.downloading, summary.waiting, summary.done),
            (1, 1, 0),
            "an earlier download isn't in it"
        );
        assert_eq!(
            summary.taskbar(),
            TaskbarProgress::Indeterminate,
            "nothing's size is known yet"
        );

        run.update(&[
            item(2, QueueState::Running),
            item(3, QueueState::Running),
            item(4, QueueState::Waiting),
        ]);
        assert_eq!(run.summary().waiting, 1, "one added meanwhile joins");

        run.update(&[done(2, 10), item(3, QueueState::Failed), done(4, 10)]);
        assert_eq!(run.summary(), RunSummary::default(), "it ended");
    }

    #[test]
    fn a_cleared_download_still_counts_and_a_cancelled_one_leaves() {
        let mut run = QueueRun::default();
        run.update(&[
            item(1, QueueState::Running),
            item(2, QueueState::Running),
            item(3, QueueState::Waiting),
        ]);
        run.update(&[
            done(1, 10),
            item(2, QueueState::Running),
            item(3, QueueState::Waiting),
        ]);
        // Clear Finished removes the first; Cancel the third.
        run.update(&[item(2, QueueState::Running)]);
        let summary = run.summary();
        assert_eq!(
            (summary.done, summary.downloading, summary.waiting),
            (1, 1, 0)
        );
        assert_eq!(summary.percent, Some(50));
    }

    #[test]
    fn overall_progress_averages_each_downloads_share_while_a_size_is_unknown() {
        let mut run = QueueRun::default();
        run.update(&[
            item(1, QueueState::Running),
            item(2, QueueState::Running),
            item(3, QueueState::Waiting),
        ]);
        run.progress(1, &at(DownloadStage::Downloading, 0.5, Some(1000)));
        run.progress(2, &at(DownloadStage::Downloading, 0.25, None));
        assert_eq!(run.summary().percent, Some(25), "(50% + 25% + 0%) / 3");
        assert_eq!(run.summary().taskbar(), TaskbarProgress::Normal(25));
    }

    #[test]
    fn overall_progress_goes_by_bytes_when_every_size_is_known() {
        let mut run = QueueRun::default();
        let mut share = item(2, QueueState::Waiting);
        share.video.files = Some(FileContents {
            count: 3,
            bytes: 3000,
        });
        run.update(&[item(1, QueueState::Running), share]);
        run.progress(1, &at(DownloadStage::Downloading, 0.5, Some(1000)));
        assert_eq!(run.summary().percent, Some(12), "500 of 4000 bytes");
    }

    #[test]
    fn a_videos_audio_comes_after_its_video() {
        let mut run = QueueRun::default();
        run.update(&[item(1, QueueState::Running), item(2, QueueState::Running)]);
        run.progress(1, &at(DownloadStage::DownloadingVideo, 1.0, Some(9000)));
        run.progress(1, &at(DownloadStage::DownloadingAudio, 0.5, Some(1000)));
        run.progress(2, &at(DownloadStage::DownloadingAudio, 0.5, Some(1000)));
        assert_eq!(
            run.summary().percent,
            Some(72),
            "(95% + 50%, audio alone) / 2"
        );
        run.progress(1, &DownloadProgress::at(DownloadStage::Merging));
        assert_eq!(
            run.summary().percent,
            Some(72),
            "merging keeps what it reached"
        );
    }

    #[test]
    fn the_taskbar_is_yellow_while_stopped_and_red_once_one_failed() {
        let mut run = QueueRun::default();
        run.update(&[item(1, QueueState::Running), item(2, QueueState::Waiting)]);
        run.progress(1, &at(DownloadStage::Downloading, 0.5, None));
        run.update(&[item(1, QueueState::Stopped), item(2, QueueState::Stopped)]);
        assert_eq!(run.summary().taskbar(), TaskbarProgress::Paused(25));
        assert_eq!(run.summary().tooltip().as_deref(), Some("2 stopped · 25%"));

        run.update(&[item(1, QueueState::Running), item(2, QueueState::Failed)]);
        assert_eq!(
            run.summary().taskbar(),
            TaskbarProgress::Failed(75),
            "a failed one is over"
        );
        assert_eq!(
            run.summary().tooltip().as_deref(),
            Some("1 downloading · 75% · 1 failed")
        );
    }

    #[test]
    fn the_tooltip_leaves_out_what_is_zero() {
        let summary = RunSummary {
            downloading: 3,
            failed: 1,
            percent: Some(45),
            ..RunSummary::default()
        };
        assert_eq!(
            summary.tooltip().as_deref(),
            Some("3 downloading · 45% · 1 failed")
        );
        let starting = RunSummary {
            waiting: 2,
            percent: Some(0),
            ..RunSummary::default()
        };
        assert_eq!(starting.tooltip().as_deref(), Some("2 waiting"));
    }
}
