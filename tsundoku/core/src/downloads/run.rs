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
    /// Its video's title, for the notification when it fails.
    title: String,
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
            title: item.video.title.clone(),
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
    /// The run ends once none of its downloads is waiting, running or stopped. Answers what happened, for the
    /// notifications: downloads that failed, and the run ending.
    pub fn update(&mut self, items: &[QueueItem]) -> Vec<RunEvent> {
        let mut events = Vec::new();
        self.members.retain(|id, member| {
            member.state == QueueState::Done || items.iter().any(|item| item.id == *id)
        });
        for item in items {
            match self.members.get_mut(&item.id) {
                Some(member) => {
                    if item.state == QueueState::Failed && member.state != QueueState::Failed {
                        events.push(RunEvent::Failed {
                            title: member.title.clone(),
                        });
                    }
                    member.follow(item);
                }
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
        if !self.members.is_empty() && self.members.values().all(Member::has_ended) {
            let summary = self.summary();
            events.push(RunEvent::Ended {
                finished: summary.done,
                failed: summary.failed,
            });
            self.members.clear();
        }
        events
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

/// What happened to the run, for the notifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEvent {
    /// One of its downloads failed.
    Failed { title: String },
    /// Every one of its downloads has ended: how many finished and how many failed. Cancelled ones aren't counted.
    Ended { finished: usize, failed: usize },
}

/// A Windows notification's title and text. The UI's texts are in strings.ts; these show outside it, in the same voice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub title: String,
    pub body: String,
}

/// The notifications for events that came close together, as Settings → Adding Links asks: one for all the failures
/// among them (`failures`), then one for a run that ended (`finished`). A run where nothing finished adds nothing to
/// the failures already told.
pub fn notices(events: &[RunEvent], failures: bool, finished: bool) -> Vec<Notice> {
    let mut notices = Vec::new();
    let failed: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            RunEvent::Failed { title } => Some(title.as_str()),
            RunEvent::Ended { .. } => None,
        })
        .collect();
    if failures && !failed.is_empty() {
        notices.push(failure_notice(&failed));
    }
    for event in events {
        if let RunEvent::Ended {
            finished: done,
            failed,
        } = *event
            && finished
            && !(done == 0 && failures)
        {
            notices.push(ended_notice(done, failed));
        }
    }
    notices
}

/// "Couldn't Download" and the title, or for several, how many and the first titles.
fn failure_notice(titles: &[&str]) -> Notice {
    match titles {
        [title] => Notice {
            title: "Couldn't Download".to_owned(),
            body: (*title).to_owned(),
        },
        [first, second] => Notice {
            title: "Couldn't Download 2 Links".to_owned(),
            body: format!("{first} and {second}"),
        },
        [first, second, rest @ ..] => Notice {
            title: format!("Couldn't Download {} Links", titles.len()),
            body: format!("{first}, {second} and {} more", rest.len()),
        },
        [] => Notice {
            title: "Couldn't Download".to_owned(),
            body: String::new(),
        },
    }
}

/// "All 12 downloads finished", or with failures, "11 of 12 downloads finished · 1 failed".
fn ended_notice(finished: usize, failed: usize) -> Notice {
    let total = finished + failed;
    let body = match (finished, failed) {
        (1, 0) => "Your download finished".to_owned(),
        (_, 0) => format!("All {total} downloads finished"),
        (0, 1) => "Your download failed".to_owned(),
        (0, _) => format!("All {total} downloads failed"),
        _ => format!("{finished} of {total} downloads finished · {failed} failed"),
    };
    Notice {
        title: "Downloads Finished".to_owned(),
        body,
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

    fn titled(id: i64, state: QueueState) -> QueueItem {
        let mut item = item(id, state);
        item.video.title = format!("Video {id}");
        item
    }

    fn failed(title: &str) -> RunEvent {
        RunEvent::Failed {
            title: title.to_owned(),
        }
    }

    #[test]
    fn tells_each_failure_once_and_the_end_with_what_finished() {
        let mut run = QueueRun::default();
        let none: Vec<RunEvent> = Vec::new();
        assert_eq!(
            run.update(&[
                titled(1, QueueState::Running),
                titled(2, QueueState::Running),
                titled(3, QueueState::Waiting),
                titled(4, QueueState::Waiting),
            ]),
            none
        );
        let events = run.update(&[
            titled(1, QueueState::Failed),
            titled(2, QueueState::Running),
            titled(3, QueueState::Running),
            titled(4, QueueState::Waiting),
        ]);
        assert_eq!(events, [failed("Video 1")]);
        let events = run.update(&[
            titled(1, QueueState::Failed),
            done(2, 10),
            titled(3, QueueState::Running),
            titled(4, QueueState::Waiting),
        ]);
        assert_eq!(events, none, "a failure is told once");

        // The fourth is cancelled, which isn't a failure, and the third finishes the run.
        let events = run.update(&[titled(1, QueueState::Failed), done(2, 10), done(3, 10)]);
        assert_eq!(
            events,
            [RunEvent::Ended {
                finished: 2,
                failed: 1
            }]
        );
        assert_eq!(
            run.update(&[titled(1, QueueState::Failed), done(2, 10), done(3, 10)]),
            none,
            "it ends once"
        );
    }

    #[test]
    fn a_retried_download_that_fails_again_is_told_again() {
        let mut run = QueueRun::default();
        run.update(&[
            titled(1, QueueState::Running),
            titled(2, QueueState::Running),
        ]);
        assert_eq!(
            run.update(&[
                titled(1, QueueState::Failed),
                titled(2, QueueState::Running)
            ]),
            [failed("Video 1")]
        );
        run.update(&[
            titled(1, QueueState::Waiting),
            titled(2, QueueState::Running),
        ]);
        assert_eq!(
            run.update(&[
                titled(1, QueueState::Failed),
                titled(2, QueueState::Running)
            ]),
            [failed("Video 1")]
        );
    }

    #[test]
    fn failures_that_come_together_are_one_notice() {
        let notice = |title: &str, body: &str| Notice {
            title: title.to_owned(),
            body: body.to_owned(),
        };
        assert_eq!(
            notices(&[failed("Zoo")], true, true),
            [notice("Couldn't Download", "Zoo")]
        );
        assert_eq!(
            notices(&[failed("Zoo"), failed("Cats")], true, true),
            [notice("Couldn't Download 2 Links", "Zoo and Cats")]
        );
        assert_eq!(
            notices(&[failed("Zoo"), failed("Cats"), failed("Tour")], true, true),
            [notice("Couldn't Download 3 Links", "Zoo, Cats and 1 more")]
        );
        assert_eq!(notices(&[failed("Zoo")], false, true), []);
    }

    #[test]
    fn the_end_says_how_many_finished() {
        let ended = |finished, failed| RunEvent::Ended { finished, failed };
        let body = |events: &[RunEvent], failures| {
            notices(events, failures, true)
                .into_iter()
                .map(|notice| notice.body)
                .collect::<Vec<_>>()
        };
        assert_eq!(body(&[ended(12, 0)], true), ["All 12 downloads finished"]);
        assert_eq!(body(&[ended(1, 0)], true), ["Your download finished"]);
        assert_eq!(
            body(&[ended(11, 1)], true),
            ["11 of 12 downloads finished · 1 failed"]
        );
        assert_eq!(
            body(&[failed("Zoo"), ended(0, 1)], true),
            ["Zoo"],
            "nothing finished: the failure says it all"
        );
        assert_eq!(
            body(&[ended(0, 3)], false),
            ["All 3 downloads failed"],
            "unless failures aren't told"
        );
        assert_eq!(notices(&[ended(12, 0)], true, false), []);
        assert_eq!(
            notices(&[failed("Zoo"), ended(1, 1)], true, true)
                .iter()
                .map(|notice| notice.title.as_str())
                .collect::<Vec<_>>(),
            ["Couldn't Download", "Downloads Finished"],
            "the failure first"
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
