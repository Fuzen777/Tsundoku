//! The download queue: videos, posts of images and file hosts' files waiting, running (a few at a time), stopped, done or failed, kept in `tsundoku.db` so
//! they come back after a restart. A finished download is recorded in the history. See
//! `DOCUMENTATION/Docs/Download Pipeline.md`.

mod file_runner;
mod holds;
mod post_runner;
mod progress;
mod room_check;
mod run;
mod runner;
mod store;
mod unpacking;
mod window_offer;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub(crate) use self::file_runner::free_name;
pub use self::holds::{Hold, Recovery};
use self::holds::{asked_to_slow_down, watch_holds};
use self::room_check::Reserved;
pub use self::run::{Notice, QueueRun, RunEvent, RunSummary, TaskbarProgress, notices};
pub use self::runner::{JobOutcome, JobRunner, JobSpec, PostJob, ToolRunner};
use self::store::QueueStore;
pub use self::unpacking::{DownloadedFiles, UnpackJob};
pub use self::window_offer::{JobOffer, OFFER_WAIT, WindowAnswer, WindowOffer, WindowReply};
use crate::browsers;
use crate::cancel::CancelToken;
use crate::database::Database;
use crate::history::{History, NewDownload, SavedFile};
use crate::metadata::{PreviewError, VideoInfo};
use crate::passwords::Password;
use crate::post_names::{self, PostNameError};
use crate::presets::{Preset, PresetError, PresetManager};
use crate::room::{DiskSpace, SystemDisks};
use crate::rules::{self, Destination, RuleSubject};
use crate::settings::{DownloadSettings, SettingsStore, UnpackSettings};
use crate::site_limits::{SiteLimits, now_ms};
use crate::sites::{self, PacedSite};
use crate::sync::lock;
use crate::tools::{ToolId, ToolManager};
use crate::unpack::UnpackProblem;

/// Logs older than this are deleted at startup.
const LOG_LIFETIME: Duration = Duration::from_secs(30 * 24 * 60 * 60);
/// The most downloads that can run at once.
pub const MAX_AT_ONCE: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum QueueState {
    Waiting,
    Running,
    /// Stopped by the user, or running when the app closed. Its partial files are kept, so Resume continues.
    Stopped,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QueueItem {
    #[ts(type = "number")]
    pub id: i64,
    pub video: VideoInfo,
    /// The preset as it was when the video was added, so editing the preset doesn't change the download. `None`
    /// for a post of images, whose files are kept as the site gives them.
    pub preset: Option<crate::presets::Preset>,
    /// Where the file goes, or a post's files: the site's folder in the posts folder.
    pub folder: String,
    /// The folder was picked for this download (`set_folder`, or picked before it was added) rather than given by the
    /// rules.
    #[serde(default)]
    pub folder_chosen: bool,
    /// The rule picked for this download in Pending, by its ID, which decides everything it sets before any other
    /// (`rules::destination`): the folder it was given when added, and again when its folder goes back to the rules'.
    /// `None` when none was.
    #[serde(default)]
    pub rule_id: Option<String>,
    /// How a post's files, or a file host's one file, are named by a rule when it was added (`rules::file_name`), or
    /// `None` for Settings' name or the file's own, and for a video. The UI doesn't need it.
    #[serde(skip)]
    #[ts(skip)]
    pub file_name: Option<String>,
    pub state: QueueState,
    /// Download Again with the same preset: replace the file made before.
    pub overwrite: bool,
    /// In seconds since 1970.
    #[ts(type = "number")]
    pub added_at: i64,
    /// Once done.
    pub result: Option<DownloadResult>,
    /// Once failed.
    pub error: Option<DownloadError>,
    /// The latest progress, while running.
    pub progress: Option<DownloadProgress>,
    /// Why it waits instead of starting, and until when: its site asked to slow down, or the hourly limit is reached.
    /// Worked out again whenever the queue schedules, so it isn't saved.
    #[serde(default)]
    pub hold: Option<Hold>,
    /// What its automatic retries have done so far: kept in memory only, so the UI never sees it.
    #[serde(skip)]
    #[ts(skip)]
    pub recovery: Recovery,
    /// A file host's download addresses caught in the check window or pasted (`hosts::check`), which its next runs
    /// download from while they still lead to the files. In memory only.
    #[serde(skip)]
    #[ts(skip)]
    pub caught_links: Vec<String>,
    /// Its files when its archives weren't all unpacked (stopped while unpacking, or one failed), so Resume or Retry
    /// only unpacks them. In memory only: after a restart, a run finds its files already there.
    #[serde(skip)]
    #[ts(skip)]
    pub unpack_files: Option<DownloadedFiles>,
    /// The password that opens its link, when it needs one: typed by the person or kept by the history. Saved with the
    /// item, never sent to the UI.
    #[serde(skip)]
    #[ts(skip)]
    pub password: Option<Password>,
}

/// How much of a log the Queue's log viewer shows: its end.
const LOG_VIEW_BYTES: u64 = 256 * 1024;

/// The end of a download's log, as the Queue's log viewer shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DownloadLog {
    pub text: String,
    /// The log is longer: only its end is here.
    pub cut: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DownloadResult {
    pub file_path: String,
    #[ts(type = "number | null")]
    pub file_size: Option<i64>,
    /// The download in the history, to open the file through it.
    #[ts(type = "number")]
    pub download_id: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DownloadProgress {
    pub stage: DownloadStage,
    /// From 0 to 1, within the current stage, when known.
    pub fraction: Option<f64>,
    #[ts(type = "number | null")]
    pub downloaded_bytes: Option<u64>,
    #[ts(type = "number | null")]
    pub total_bytes: Option<u64>,
    /// In bytes per second.
    pub speed: Option<f64>,
    /// Seconds left.
    #[ts(type = "number | null")]
    pub eta: Option<u64>,
}

impl DownloadProgress {
    pub fn at(stage: DownloadStage) -> Self {
        Self {
            stage,
            fraction: None,
            downloaded_bytes: None,
            total_bytes: None,
            speed: None,
            eta: None,
        }
    }
}

/// Sent to the UI while a download runs.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DownloadProgressEvent {
    #[ts(type = "number")]
    pub id: i64,
    pub progress: DownloadProgress,
}

/// One download for `DownloadQueue::add_many`, such as an item picked from a channel.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QueuePick {
    pub video: VideoInfo,
    /// The preset for a video; `None` for a post or a file host's link, whose files are kept as they are.
    pub preset_id: Option<String>,
    /// Download Again with the same preset: replace the file made before.
    pub overwrite: bool,
    /// A folder picked for it instead of the one the rules give.
    #[serde(default)]
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DownloadStage {
    /// yt-dlp is reading the video's page.
    Starting,
    DownloadingVideo,
    DownloadingAudio,
    /// A single file with both, or a format whose kind isn't known.
    Downloading,
    Merging,
    /// Extracting or converting audio, or remuxing video into the preset's container.
    Converting,
    Embedding,
    /// Anything else yt-dlp does after downloading, such as SponsorBlock.
    Processing,
    /// Moving the file into its folder.
    Finishing,
    /// A file host's countdown before a free download (1fichier's), with the seconds left as `eta`.
    Countdown,
    /// Unpacking an archive the download brought (`unpacking`), with the bytes written of what it holds.
    Unpacking,
}

/// Why a download, or an action on the queue, failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum DownloadError {
    /// yt-dlp, gallery-dl for a post, or a file host failed, or the tool couldn't run; its error, sorted as a
    /// preview's is.
    #[error("download failed: {error}")]
    YtDlp { error: PreviewError },
    /// The destination or temporary folder couldn't be made.
    #[error("couldn't make {path}: {detail}")]
    Folder { path: String, detail: String },
    /// yt-dlp finished without saying which file it made.
    #[error("yt-dlp didn't report a file")]
    NoFile,
    #[error("preset: {error}")]
    Preset { error: PresetError },
    /// The video is already in the queue and not done.
    #[error("already in the queue")]
    AlreadyQueued,
    #[error("no such item in the queue")]
    NotFound,
    /// A file host's file didn't match its checksum (MEGA's MAC, MediaFire's SHA-256). It was deleted, so Retry
    /// downloads it again.
    #[error("{file} arrived damaged")]
    Damaged { file: String },
    /// The file the check window was saving (a host whose download runs in the window) didn't arrive: the window was
    /// closed, the download stopped, or the site ended it.
    #[error("the download in {site}'s check window stopped")]
    WindowDownloadStopped { site: String },
    /// The action doesn't fit the item's state, such as Retry on one that didn't fail.
    #[error("not possible in this state")]
    WrongState,
    /// The queue or the history couldn't be saved.
    #[error("couldn't save: {detail}")]
    Storage { detail: String },
    /// A speed limit yt-dlp wouldn't understand.
    #[error("{value} isn't a speed limit")]
    InvalidRateLimit { value: String },
    /// A name for posts' files (Settings → Posts of Images) that could lose files, or that Windows refuses.
    #[error("post file name: {error}")]
    PostFileName { error: PostNameError },
    /// A post or a shared folder with every file unticked.
    #[error("no file is picked")]
    NothingPicked,
    /// A shared folder none of whose picked files is in it any more: they were moved, renamed or deleted since the
    /// link was read. The download doesn't fall back to the whole folder.
    #[error("the files picked are no longer in the shared folder")]
    PickedFilesGone,
    /// The clipboard shortcut picked in Settings → Adding Links is taken by another app, so Windows won't give it to
    /// Tsundoku.
    #[error("{keys} is taken by another app")]
    ShortcutTaken { keys: String },
    /// An archive the download brought couldn't be unpacked (Settings → Archives); the files are kept, and Retry
    /// unpacks again without downloading.
    #[error("couldn't unpack {archive}: {problem}")]
    Unpack {
        archive: String,
        problem: UnpackProblem,
    },
    #[error("unexpected error: {detail}")]
    Unexpected { detail: String },
}

/// Told about every change, to pass it on to the UI.
pub trait QueueListener: Send + Sync {
    /// The items changed: added, removed, or in a new state.
    fn changed(&self, items: &[QueueItem]);
    /// A running item's progress. Frequent, so separate from `changed`.
    fn progress(&self, id: i64, progress: &DownloadProgress);
}

/// What the tray's one item for the whole queue does: Pause All while anything runs or waits, else Resume All while
/// something is stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WholeQueueAction {
    PauseAll,
    ResumeAll,
}

impl WholeQueueAction {
    /// `None` when there's nothing to pause or resume.
    pub fn for_items(items: &[QueueItem]) -> Option<Self> {
        let any_in = |states: &[QueueState]| items.iter().any(|item| states.contains(&item.state));
        if any_in(&[QueueState::Running, QueueState::Waiting]) {
            Some(Self::PauseAll)
        } else if any_in(&[QueueState::Stopped]) {
            Some(Self::ResumeAll)
        } else {
            None
        }
    }
}

/// What ends a running download early, set before its process is ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EndRequest {
    Stop,
    Cancel,
}

struct Running {
    cancel: CancelToken,
    end_request: Option<EndRequest>,
}

#[derive(Default)]
struct QueueContents {
    items: Vec<QueueItem>,
    running: HashMap<i64, Running>,
    /// What the last Clear Finished took, in the queue's order, for Undo to put back.
    cleared: Vec<Cleared>,
}

/// A download Clear Finished took, kept for Undo.
struct Cleared {
    /// Where it was in the queue's items.
    index: usize,
    /// Where it was in the saved queue's order (`QueueStore::take`).
    position: i64,
    item: QueueItem,
}

/// The services the queue uses.
pub struct QueueServices {
    pub database: Arc<Database>,
    pub history: Arc<History>,
    pub tools: Arc<ToolManager>,
    pub presets: Arc<PresetManager>,
    pub settings: Arc<SettingsStore>,
    pub logs_folder: PathBuf,
    pub runner: Arc<dyn JobRunner>,
    pub listener: Arc<dyn QueueListener>,
}

pub struct DownloadQueue {
    state: Mutex<QueueContents>,
    store: QueueStore,
    history: Arc<History>,
    tools: Arc<ToolManager>,
    presets: Arc<PresetManager>,
    settings: Arc<SettingsStore>,
    logs_folder: PathBuf,
    /// What each drive has free, to check a download has room before it starts (`room_check`).
    disks: Mutex<Arc<dyn DiskSpace>>,
    runner: Arc<dyn JobRunner>,
    listener: Arc<dyn QueueListener>,
    /// Each site's wait after it answered "too many requests", which the pending links and the preview card follow too.
    limits: SiteLimits,
    /// The queue's own `Arc`, for the threads it starts.
    myself: Weak<DownloadQueue>,
}

impl DownloadQueue {
    /// Loads the saved queue and starts what's waiting. Items that were running when the app closed come back
    /// stopped. Deletes logs older than 30 days.
    pub fn open(services: QueueServices) -> Result<Arc<Self>, DownloadError> {
        let store = QueueStore::new(services.database);
        let mut items = store.load()?;
        for item in items
            .iter_mut()
            .filter(|item| item.state == QueueState::Running)
        {
            item.state = QueueState::Stopped;
            store.update(item)?;
        }
        prune_logs(&services.logs_folder, LOG_LIFETIME);
        let queue = Arc::new_cyclic(|myself| Self {
            state: Mutex::new(QueueContents {
                items,
                running: HashMap::new(),
                cleared: Vec::new(),
            }),
            store,
            history: services.history,
            tools: services.tools,
            presets: services.presets,
            settings: services.settings,
            logs_folder: services.logs_folder,
            disks: Mutex::new(Arc::new(SystemDisks)),
            runner: services.runner,
            listener: services.listener,
            limits: SiteLimits::new(),
            myself: myself.clone(),
        });
        queue.schedule();
        let watching = Arc::downgrade(&queue);
        thread::spawn(move || watch_holds(&watching));
        Ok(queue)
    }

    pub fn items(&self) -> Vec<QueueItem> {
        lock(&self.state).items.clone()
    }

    /// Whether the video is in the queue and not done yet.
    pub fn contains(&self, site: &str, video_id: &str) -> bool {
        lock(&self.state).items.iter().any(|item| {
            item.state != QueueState::Done
                && item.video.site == site
                && item.video.video_id == video_id
        })
    }

    /// Whether the video is in the queue and not done yet, unless it waits for a check that `video`, the Download
    /// button's own link, gets past (`pass_check_with`): what the preview card and Pending call "in the queue".
    pub fn contains_video(&self, video: &VideoInfo) -> bool {
        self.contains(&video.site, &video.video_id) && !self.check_takes(&video.url)
    }

    /// Whether a download not done yet has exactly this address (its page's, as the tool that read it gave it).
    pub fn has_link(&self, url: &str) -> bool {
        lock(&self.state)
            .items
            .iter()
            .any(|item| item.state != QueueState::Done && item.video.url == url)
    }

    pub fn running_count(&self) -> usize {
        lock(&self.state).running.len()
    }

    /// Adds a video with a preset, to download when there's room.
    pub fn add(
        &self,
        video: VideoInfo,
        preset_id: &str,
        overwrite: bool,
    ) -> Result<QueueItem, DownloadError> {
        self.add_in(video, preset_id, overwrite, None, None, None)
    }

    /// `add`, into `folder` when one was picked for it instead of the one the rules give, with the rule picked for it
    /// (`rule_id`) deciding what it sets before any other, and the `password` that opened its link.
    pub fn add_in(
        &self,
        video: VideoInfo,
        preset_id: &str,
        overwrite: bool,
        folder: Option<String>,
        rule_id: Option<String>,
        password: Option<Password>,
    ) -> Result<QueueItem, DownloadError> {
        let item = self.insert_video(video, preset_id, overwrite, folder, rule_id, password)?;
        self.schedule();
        Ok(item)
    }

    /// Saves a video with a preset in the queue, waiting, for `add` or `add_many` to start.
    fn insert_video(
        &self,
        video: VideoInfo,
        preset_id: &str,
        overwrite: bool,
        chosen: Option<String>,
        rule_id: Option<String>,
        password: Option<Password>,
    ) -> Result<QueueItem, DownloadError> {
        let preset = self.presets.get(preset_id).ok_or(DownloadError::Preset {
            error: PresetError::NotFound,
        })?;
        preset
            .validate()
            .map_err(|error| DownloadError::Preset { error })?;
        if self.contains(&video.site, &video.video_id) {
            return Err(DownloadError::AlreadyQueued);
        }
        let chosen = self.chosen_folder(chosen);
        let folder_chosen = chosen.is_some();
        let folder = chosen.unwrap_or_else(|| {
            self.destination(&video, Some(&preset), rule_id.as_deref())
                .folder
        });
        let file_name = None;
        let password = self.password_for(&video, password);
        let mut item = QueueItem {
            id: 0,
            folder,
            folder_chosen,
            rule_id,
            file_name,
            password,
            video,
            preset: Some(preset),
            state: QueueState::Waiting,
            overwrite,
            added_at: seconds_since_epoch(),
            result: None,
            error: None,
            progress: None,
            hold: None,
            recovery: Recovery::default(),
            caught_links: Vec::new(),
            unpack_files: None,
        };
        item.id = self.store.insert(&item)?;
        lock(&self.state).items.push(item.clone());
        Ok(item)
    }

    /// Adds a post of images (a video whose `post` is set, from gallery-dl) or a file host's link (`files` set), to
    /// download all its files into the folder the rules give it (a post's by default the site's folder in the
    /// download folder). Files already there are skipped, so adding it again only fetches what's missing.
    pub fn add_post(&self, video: VideoInfo) -> Result<QueueItem, DownloadError> {
        self.add_post_in(video, None, None, None)
    }

    /// `add_post`, into `folder` when one was picked for it instead of the one the rules give, with the rule picked for
    /// it (`rule_id`) deciding what it sets before any other, and the `password` that opened its link.
    pub fn add_post_in(
        &self,
        video: VideoInfo,
        folder: Option<String>,
        rule_id: Option<String>,
        password: Option<Password>,
    ) -> Result<QueueItem, DownloadError> {
        let item = self.insert_post(video, folder, rule_id, password)?;
        self.schedule();
        Ok(item)
    }

    /// Adds several downloads at once, such as those picked from a channel, each as `add` (with a preset) or `add_post`
    /// (without) would, then starts what can start: the UI hears of the change once rather than once each. One already
    /// in the queue is skipped. How many were added; the first error is returned once every other one is in.
    pub fn add_many(&self, picks: Vec<QueuePick>) -> Result<usize, DownloadError> {
        let mut added = 0;
        let mut first_error = None;
        for QueuePick {
            video,
            preset_id,
            overwrite,
            folder,
        } in picks
        {
            let inserted = match preset_id {
                Some(preset_id) => {
                    self.insert_video(video, &preset_id, overwrite, folder, None, None)
                }
                None => self.insert_post(video, folder, None, None),
            };
            match inserted {
                Ok(_) => added += 1,
                Err(DownloadError::AlreadyQueued) => {}
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        if added > 0 {
            self.schedule();
        }
        first_error.map_or(Ok(added), Err)
    }

    /// Saves a post or a file host's link in the queue, waiting, for `add_post` or `add_many` to start.
    fn insert_post(
        &self,
        video: VideoInfo,
        chosen: Option<String>,
        rule_id: Option<String>,
        password: Option<Password>,
    ) -> Result<QueueItem, DownloadError> {
        if video.post.is_none() && video.files.is_none() {
            return Err(DownloadError::WrongState);
        }
        // A post's or a shared folder's files can be unticked (`PostFile::picked`, `SharedFile::picked`), but not all
        // of them.
        if (!video.post_files.is_empty() && !video.post_files.iter().any(|file| file.picked))
            || (!video.shared_files.is_empty()
                && !video.shared_files.iter().any(|file| file.picked))
        {
            return Err(DownloadError::NothingPicked);
        }
        // The Download button's own link, pasted or copied, for a download that waits for its check.
        if let Some(item) = self.pass_check_with(&video.url) {
            return Ok(item);
        }
        if self.contains(&video.site, &video.video_id) {
            return Err(DownloadError::AlreadyQueued);
        }
        let chosen = self.chosen_folder(chosen);
        let folder_chosen = chosen.is_some();
        let folder =
            chosen.unwrap_or_else(|| self.destination(&video, None, rule_id.as_deref()).folder);
        let file_name = self.rule_file_name(&video, rule_id.as_deref());
        let password = self.password_for(&video, password);
        let mut item = QueueItem {
            id: 0,
            folder,
            folder_chosen,
            rule_id,
            file_name,
            password,
            video,
            preset: None,
            state: QueueState::Waiting,
            overwrite: false,
            added_at: seconds_since_epoch(),
            result: None,
            error: None,
            progress: None,
            hold: None,
            recovery: Recovery::default(),
            caught_links: Vec::new(),
            unpack_files: None,
        };
        item.id = self.store.insert(&item)?;
        lock(&self.state).items.push(item.clone());
        Ok(item)
    }

    /// Stops a running download, keeping its partial files, or holds a waiting one.
    pub fn stop(&self, id: i64) -> Result<(), DownloadError> {
        let mut state = lock(&self.state);
        if let Some(running) = state.running.get_mut(&id) {
            running.end_request = Some(EndRequest::Stop);
            running.cancel.cancel();
            return Ok(());
        }
        let item = find(&mut state.items, id)?;
        if item.state != QueueState::Waiting {
            return Err(DownloadError::WrongState);
        }
        item.state = QueueState::Stopped;
        let offered = item.recovery.offer.take();
        self.store.update(item)?;
        drop(state);
        // The check window stops the download it offered.
        if let Some(offered) = offered {
            offered.refuse();
        }
        self.notify();
        Ok(())
    }

    /// Pause All: Stop for every download running or waiting. The running ones end keeping their partial files, the
    /// waiting ones are held. One being cancelled is left to finish cancelling.
    pub fn stop_all(&self) -> Result<(), DownloadError> {
        let mut state = lock(&self.state);
        let contents = &mut *state;
        let mut saved = Ok(());
        let mut offers = Vec::new();
        // The waiting ones first: a running one that ends meanwhile waits for this lock, then finds nothing to start.
        for item in contents
            .items
            .iter_mut()
            .filter(|item| item.state == QueueState::Waiting)
        {
            item.state = QueueState::Stopped;
            offers.extend(item.recovery.offer.take());
            saved = saved.and(self.store.update(item));
        }
        for running in contents
            .running
            .values_mut()
            .filter(|running| running.end_request.is_none())
        {
            running.end_request = Some(EndRequest::Stop);
            running.cancel.cancel();
        }
        drop(state);
        for offered in offers {
            offered.refuse();
        }
        self.notify();
        saved
    }

    /// Puts a stopped download back in line.
    pub fn resume(&self, id: i64) -> Result<(), DownloadError> {
        self.requeue(id, QueueState::Stopped)
    }

    /// Resume All: Resume for every stopped download, which start in their order.
    pub fn resume_all(&self) -> Result<(), DownloadError> {
        self.requeue_all(QueueState::Stopped)
    }

    /// Puts a failed download back in line. Its partial files are kept, so it continues where it failed.
    pub fn retry(&self, id: i64) -> Result<(), DownloadError> {
        self.requeue(id, QueueState::Failed)
    }

    /// Retry Failed: Retry for every failed download.
    pub fn retry_failed(&self) -> Result<(), DownloadError> {
        self.requeue_all(QueueState::Failed)
    }

    fn requeue(&self, id: i64, from: QueueState) -> Result<(), DownloadError> {
        {
            let mut state = lock(&self.state);
            let item = find(&mut state.items, id)?;
            if item.state != from {
                return Err(DownloadError::WrongState);
            }
            self.put_back_in_line(item)?;
        }
        self.schedule();
        Ok(())
    }

    /// Puts every download in the state `from` back in line. One that can't be saved still goes; the first error is
    /// the answer.
    fn requeue_all(&self, from: QueueState) -> Result<(), DownloadError> {
        let mut saved = Ok(());
        {
            let mut state = lock(&self.state);
            for item in state.items.iter_mut().filter(|item| item.state == from) {
                saved = saved.and(self.put_back_in_line(item));
            }
        }
        self.schedule();
        saved
    }

    /// Resume and Retry, for one download or all of them.
    fn put_back_in_line(&self, item: &mut QueueItem) -> Result<(), DownloadError> {
        item.state = QueueState::Waiting;
        item.error = None;
        // A fresh start: the automatic retries begin again, and a wait they set is dropped.
        item.recovery = Recovery::default();
        self.store.update(item)
    }

    /// Download Next: moves a waiting download to the front of the line, so it starts before every other waiting one.
    /// The order is saved, so it holds through a restart.
    pub fn download_next(&self, id: i64) -> Result<(), DownloadError> {
        let mut state = lock(&self.state);
        let index = state
            .items
            .iter()
            .position(|item| item.id == id)
            .ok_or(DownloadError::NotFound)?;
        if state.items[index].state != QueueState::Waiting {
            return Err(DownloadError::WrongState);
        }
        self.store.move_to_front(id)?;
        let item = state.items.remove(index);
        state.items.insert(0, item);
        drop(state);
        self.notify();
        Ok(())
    }

    /// Saves a download in another folder, picked for it, or back in the one the rules give it (`None`). Only while it
    /// isn't running or done; its partial files, which sit in its folder, are deleted, so a stopped one starts over.
    pub fn set_folder(&self, id: i64, folder: Option<String>) -> Result<QueueItem, DownloadError> {
        let chosen = self.chosen_folder(folder);
        let item = {
            let mut state = lock(&self.state);
            let item = state
                .items
                .iter_mut()
                .find(|item| item.id == id)
                .ok_or(DownloadError::NotFound)?;
            if !matches!(
                item.state,
                QueueState::Waiting | QueueState::Stopped | QueueState::Failed
            ) {
                return Err(DownloadError::WrongState);
            }
            let folder_chosen = chosen.is_some();
            let folder = match chosen {
                Some(folder) => folder,
                None => {
                    self.destination(&item.video, item.preset.as_ref(), item.rule_id.as_deref())
                        .folder
                }
            };
            if folder != item.folder {
                remove_folder(&temp_folder(item));
                // It starts over there, archives and all.
                item.unpack_files = None;
            }
            item.folder = folder;
            item.folder_chosen = folder_chosen;
            self.store.set_folder(item)?;
            item.clone()
        };
        self.notify();
        self.schedule();
        Ok(item)
    }

    /// A folder picked for a download, as a full path: one that isn't is inside the download folder. Blank means none.
    fn chosen_folder(&self, folder: Option<String>) -> Option<String> {
        let folder = folder?.trim().to_owned();
        if folder.is_empty() {
            return None;
        }
        let path = Path::new(&folder);
        Some(if path.is_absolute() {
            folder
        } else {
            self.settings
                .get()
                .downloads
                .resolved_folder()
                .join(path)
                .display()
                .to_string()
        })
    }

    /// What the tray's Pause All item would do now.
    pub fn whole_queue_action(&self) -> Option<WholeQueueAction> {
        WholeQueueAction::for_items(&lock(&self.state).items)
    }

    /// Removes a download that isn't done, deleting its partial files. A running one is ended first; its thread
    /// removes it once the process is gone and has let go of the files.
    pub fn cancel(&self, id: i64) -> Result<(), DownloadError> {
        let mut state = lock(&self.state);
        if let Some(running) = state.running.get_mut(&id) {
            running.end_request = Some(EndRequest::Cancel);
            running.cancel.cancel();
            return Ok(());
        }
        let item = find(&mut state.items, id)?;
        if item.state == QueueState::Done {
            return Err(DownloadError::WrongState);
        }
        let offered = item.recovery.offer.take();
        // The files go first, so once the item is gone, they are too.
        remove_folder(&temp_folder(item));
        state.items.retain(|item| item.id != id);
        self.store.delete(id)?;
        drop(state);
        if let Some(offered) = offered {
            offered.refuse();
        }
        // Its partial files are gone: a download waiting for room may fit now.
        self.schedule();
        Ok(())
    }

    /// Removes the finished downloads from the queue. They stay in the history, and in the queue's memory until the
    /// next Clear Finished, so Undo can put them back (`restore_cleared`). How many were removed.
    pub fn clear_finished(&self) -> Result<usize, DownloadError> {
        self.clear_done(|_| true)
    }

    /// Removes one finished download from the queue, as Clear Finished does for them all: it stays in the history,
    /// and Undo (`restore_cleared`) puts it back.
    pub fn remove_finished(&self, id: i64) -> Result<(), DownloadError> {
        {
            let mut state = lock(&self.state);
            if find(&mut state.items, id)?.state != QueueState::Done {
                return Err(DownloadError::WrongState);
            }
        }
        self.clear_done(|item| item.id == id).map(|_| ())
    }

    /// Removes the finished downloads `pick` takes, keeping them for Undo in place of the last ones removed.
    fn clear_done(&self, pick: impl Fn(&QueueItem) -> bool) -> Result<usize, DownloadError> {
        let mut state = lock(&self.state);
        let mut cleared = Vec::new();
        let mut kept = Vec::new();
        let mut first_error = None;
        for (index, item) in std::mem::take(&mut state.items).into_iter().enumerate() {
            if item.state != QueueState::Done || !pick(&item) {
                kept.push(item);
                continue;
            }
            match self.store.take(item.id) {
                Ok(position) => cleared.push(Cleared {
                    index,
                    position,
                    item,
                }),
                // Still saved, so it stays.
                Err(error) => {
                    first_error.get_or_insert(error);
                    kept.push(item);
                }
            }
        }
        state.items = kept;
        let count = cleared.len();
        state.cleared = cleared;
        drop(state);
        self.notify();
        first_error.map_or(Ok(count), Err)
    }

    /// Undo for the last Clear Finished: puts its downloads back where they were, with their IDs (so their logs
    /// follow) and their places in the queue's order, saved as before. One whose video is in the queue again by now
    /// stays out. How many came back; the first error is returned once every other download is back.
    pub fn restore_cleared(&self) -> Result<usize, DownloadError> {
        let mut restored = 0;
        let mut first_error = None;
        let mut state = lock(&self.state);
        // Each goes back to where it was, less the ones before it that stayed out.
        let mut left_out = 0;
        for Cleared {
            index,
            position,
            item,
        } in std::mem::take(&mut state.cleared)
        {
            let here_again = state.items.iter().any(|other| {
                other.video.site == item.video.site && other.video.video_id == item.video.video_id
            });
            if here_again {
                left_out += 1;
                continue;
            }
            if let Err(error) = self.store.restore(&item, position) {
                first_error.get_or_insert(error);
                left_out += 1;
                continue;
            }
            let place = (index - left_out).min(state.items.len());
            state.items.insert(place, item);
            restored += 1;
        }
        drop(state);
        if restored > 0 {
            self.notify();
        }
        first_error.map_or(Ok(restored), Err)
    }

    /// The password a new download's link opens with: the one given, else the one the history keeps for the video, so
    /// downloading it again never asks twice.
    fn password_for(&self, video: &VideoInfo, given: Option<Password>) -> Option<Password> {
        given.or_else(|| {
            self.history
                .password_for(&video.site, &video.video_id)
                .ok()
                .flatten()
        })
    }

    /// Enter Password, for a download that waits for one (`Hold::NeedsPassword`): it's kept with the download, saved,
    /// and the download starts with it. A wrong one has it wait for a password again.
    pub fn enter_password(&self, id: i64, password: Password) -> Result<(), DownloadError> {
        {
            let mut state = lock(&self.state);
            let item = find(&mut state.items, id)?;
            if item.state != QueueState::Waiting || item.recovery.needs_password.is_none() {
                return Err(DownloadError::WrongState);
            }
            item.password = Some(password);
            self.store.set_password(item)?;
            item.recovery.needs_password = None;
        }
        self.schedule();
        Ok(())
    }

    /// How a post's files, or a file host's one file, are named by the rules, the one `picked` for it first
    /// (`rules::file_name`).
    fn rule_file_name(&self, video: &VideoInfo, picked: Option<&str>) -> Option<String> {
        rules::file_name(
            &self.settings.get().rules.list,
            &RuleSubject {
                video,
                preset: None,
            },
            picked,
        )
    }

    /// Where a download of `video` would go, with `preset` (`None` for a post): the folder of the rule `picked` for it
    /// (by ID) when it picks one, else of the last rule that matches, else the preset's own folder, else the download
    /// folder. Adding uses it, and the preview card and Pending show it.
    pub fn destination(
        &self,
        video: &VideoInfo,
        preset: Option<&Preset>,
        picked: Option<&str>,
    ) -> Destination {
        let settings = self.settings.get();
        rules::destination(
            &settings.rules.list,
            &RuleSubject { video, preset },
            picked,
            &settings.downloads.resolved_folder(),
        )
    }

    /// How downloads run: where they go, how many at once, the speed limit, the cookies browser, and how fast posts
    /// are read.
    pub fn options(&self) -> DownloadSettings {
        self.settings.get().downloads
    }

    /// Saves how downloads run, and starts more right away if more can run at once. The speed limit is checked and
    /// written as yt-dlp writes it ("2M"); an empty one means no limit. A blank folder means the default one, and so
    /// does a blank name for posts' files; a name that could lose files is refused (`post_names::check`).
    pub fn set_options(
        &self,
        options: DownloadSettings,
    ) -> Result<DownloadSettings, DownloadError> {
        let options = DownloadSettings {
            folder: options
                .folder
                .map(|folder| folder.trim().to_owned())
                .filter(|folder| !folder.is_empty()),
            at_once: options.at_once.clamp(1, MAX_AT_ONCE),
            rate_limit: normalize_rate_limit(options.rate_limit.as_deref())?,
            cookies_browser: options.cookies_browser,
            post_file_name: post_names::check(&options.post_file_name)
                .map_err(|error| DownloadError::PostFileName { error })?,
            post_pause: options.post_pause.normalized(),
            site_pauses: options.site_pauses.normalized(),
            hourly_file_limit: DownloadSettings::normalized_hourly_limit(options.hourly_file_limit),
            unpack: UnpackSettings {
                passwords: UnpackSettings::normalized_passwords(options.unpack.passwords),
                ..options.unpack
            },
        };
        self.settings
            .update(|settings| settings.downloads = options.clone())
            .map_err(|error| DownloadError::Storage {
                detail: error.to_string(),
            })?;
        self.schedule();
        Ok(options)
    }

    /// The item's log: yt-dlp's output for each of its runs.
    pub fn log_path(&self, id: i64) -> PathBuf {
        self.logs_folder.join(format!("{id}.log"))
    }

    /// The end of a download's log, for the Queue's log viewer: at most `LOG_VIEW_BYTES`, so a long one stays light.
    pub fn log(&self, id: i64) -> Result<DownloadLog, DownloadError> {
        crate::bug_report::read_end(&self.log_path(id), LOG_VIEW_BYTES)
            .map(|(text, cut)| DownloadLog { text, cut })
            .ok_or(DownloadError::NotFound)
    }

    /// Starts waiting items while there's room, in the queue's order: the order they were added, apart from those
    /// Download Next moved to the front. A post on a site that limits how fast it's read (Instagram, and X, Pixiv or
    /// Tumblr with a pause set) waits while another of that site's posts runs, since each pauses between its files on its own and two at once would read twice as
    /// fast; downloads from other sites go ahead of it meanwhile. One that's held (`Hold`: its site asked to slow down,
    /// or the hourly limit is reached) waits too, until a thread schedules again once its hold ends (`watch_holds`).
    fn schedule(&self) {
        let options = self.settings.get().downloads;
        let at_once = options.at_once.max(1) as usize;
        let mut started = Vec::new();
        {
            let mut state = lock(&self.state);
            // Through a plain reference, so the items and the running map can be borrowed at once.
            let contents = &mut *state;
            self.settle_holds(&mut contents.items, now_ms());
            // What the downloads already running need on their drives, and those started below once each starts.
            let mut reserved = Reserved::of_running(&contents.items);
            while contents.running.len() < at_once {
                let pacing: Vec<PacedSite> = contents
                    .items
                    .iter()
                    .filter(|item| item.state == QueueState::Running)
                    .filter_map(|item| paced_site(item, &options))
                    .collect();
                let may_start = |item: &QueueItem| {
                    item.state == QueueState::Waiting
                        && item.hold.is_none()
                        && paced_site(item, &options).is_none_or(|site| !pacing.contains(&site))
                };
                // A download the person just started in the check window goes first: the window holds it meanwhile.
                let Some(index) = contents
                    .items
                    .iter()
                    .position(|item| item.recovery.offer.is_some() && may_start(item))
                    .or_else(|| contents.items.iter().position(may_start))
                else {
                    break;
                };
                let item = &mut contents.items[index];
                // A download that won't fit waits, held, and the next one is tried.
                if let Some(hold) = self.room_hold(item, &mut reserved, now_ms()) {
                    item.hold = Some(hold);
                    continue;
                }
                item.state = QueueState::Running;
                item.progress = Some(DownloadProgress::at(DownloadStage::Starting));
                if let Err(error) = self.store.update(item) {
                    item.state = QueueState::Failed;
                    item.error = Some(error);
                    continue;
                }
                let mut job = self.job_for(item);
                job.window_offer = item
                    .recovery
                    .offer
                    .take()
                    .map(|offered| self.job_offer(item.id, offered));
                let cancel = CancelToken::new();
                contents.running.insert(
                    item.id,
                    Running {
                        cancel: cancel.clone(),
                        end_request: None,
                    },
                );
                started.push((job, cancel));
            }
        }
        for (job, cancel) in started {
            self.start(job, cancel);
        }
        self.notify();
    }

    fn job_for(&self, item: &QueueItem) -> JobSpec {
        let options = self.options();
        let pause = options.pause_for(&item.video.url);
        JobSpec {
            id: item.id,
            url: item.video.url.clone(),
            preset: item.preset.clone(),
            folder: PathBuf::from(&item.folder),
            temp_folder: temp_folder(item),
            log_path: self.log_path(item.id),
            overwrite: item.overwrite,
            yt_dlp: self.tools.executable(ToolId::YtDlp),
            ffmpeg: self.tools.executable(ToolId::Ffmpeg),
            deno: self.tools.executable(ToolId::Deno),
            caches: self.tools.cache_folder(),
            rate_limit: options.rate_limit,
            cookies: browsers::cookies_for(options.cookies_browser, &item.video.url),
            post: item.video.post.map(|contents| PostJob {
                gallery_dl: self.tools.executable(ToolId::GalleryDl),
                pause,
                files: item
                    .video
                    .post_file_count()
                    .unwrap_or(contents.images + contents.videos + contents.others),
                range: item.video.picked_post_files(),
                file_name: post_names::gallery_format(
                    item.file_name.as_deref().unwrap_or(&options.post_file_name),
                    &item.video,
                ),
            }),
            files: item.video.files,
            picked_files: item.video.picked_shared_files(),
            shared_file_name: item
                .video
                .files
                .is_some()
                .then(|| item.file_name.clone())
                .flatten(),
            caught_links: item.caught_links.clone(),
            window_offer: None,
            unpack: options.unpack.enabled.then(|| UnpackJob {
                after: options.unpack.after,
                passwords: options.unpack.passwords.clone(),
            }),
            // Downloaded before: unpacked again while the archives are all still there, else downloaded again.
            unpack_only: item
                .unpack_files
                .clone()
                .filter(|files| files.archives.iter().all(|archive| archive.exists())),
            password: item.password.clone(),
        }
    }

    fn start(&self, job: JobSpec, cancel: CancelToken) {
        let Some(queue) = self.myself.upgrade() else {
            return;
        };
        thread::spawn(move || {
            let id = job.id;
            let outcome = queue.runner.run(&job, &cancel, &mut |progress| {
                queue.report_progress(id, progress)
            });
            // Instagram's "rate-limit reached or login required" waits like a slow-down while signed in; the
            // browsers are read here, outside the queue's lock.
            let outcome = match outcome {
                JobOutcome::Failed(DownloadError::YtDlp { error }) if error.may_be_slow_down() => {
                    let choice = queue.options().cookies_browser;
                    JobOutcome::Failed(DownloadError::YtDlp {
                        error: browsers::slow_down_if_signed_in(error, choice, &job.url),
                    })
                }
                other => other,
            };
            queue.finish(&job, outcome);
        });
    }

    fn report_progress(&self, id: i64, progress: DownloadProgress) {
        if let Some(item) = lock(&self.state)
            .items
            .iter_mut()
            .find(|item| item.id == id)
        {
            item.progress = Some(progress.clone());
        }
        self.listener.progress(id, &progress);
    }

    /// Settles a run: done (and recorded in the history), failed, stopped, or removed if it was cancelled.
    fn finish(&self, job: &JobSpec, outcome: JobOutcome) {
        let mut state = lock(&self.state);
        let end_request = state
            .running
            .remove(&job.id)
            .and_then(|running| running.end_request);
        let stopped = end_request.is_some() || outcome == JobOutcome::Cancelled;
        if end_request == Some(EndRequest::Cancel) {
            // The files go first, so once the item is gone, they are too.
            remove_folder(&job.temp_folder);
            state.items.retain(|item| item.id != job.id);
            let _ = self.store.delete(job.id);
            drop(state);
        } else if let Ok(item) = find(&mut state.items, job.id) {
            item.progress = None;
            // A lost connection or a used-up download limit is tried again by itself, unless the user ended the run.
            let retry = match (&end_request, &outcome) {
                (None, JobOutcome::Failed(error)) => item.recovery.after_failure(error, now_ms()),
                _ => None,
            };
            match (end_request, outcome) {
                // Downloaded: Resume or Retry only unpacks.
                (end_request, JobOutcome::NotUnpacked { files, error }) => {
                    item.unpack_files = Some(files);
                    match error {
                        Some(error) if end_request.is_none() => {
                            item.state = QueueState::Failed;
                            item.error = Some(error);
                        }
                        _ => item.state = QueueState::Stopped,
                    }
                }
                (Some(EndRequest::Stop), _) | (_, JobOutcome::Cancelled) => {
                    item.state = QueueState::Stopped
                }
                // The site answered "too many requests": its downloads wait, this one with them, keeping its partial
                // files.
                (_, JobOutcome::Failed(error))
                    if asked_to_slow_down(&error) && sites::site_of(&item.video.url).is_some() =>
                {
                    self.note_slow_down(&item.video.url);
                    item.state = QueueState::Waiting;
                }
                // It waits with its partial files (`Recovery::after_failure`), then runs again.
                (_, JobOutcome::Failed(_)) if retry.is_some() => {
                    item.state = QueueState::Waiting;
                }
                // Its link needs a password it doesn't have, or the one it had no longer opens it: it waits for the
                // person to enter one, without retrying by itself. One that failed is dropped, so it's asked again.
                (
                    _,
                    JobOutcome::Failed(DownloadError::YtDlp {
                        error: PreviewError::PasswordNeeded { .. },
                    }),
                ) => {
                    item.state = QueueState::Waiting;
                    item.recovery.needs_password = Some(item.password.is_some());
                    if item.password.take().is_some() {
                        let _ = self.store.set_password(item);
                    }
                }
                // It waits, with the files it could download saved, until a person opens the check. A file the window
                // saved meanwhile no longer waits for it.
                (_, JobOutcome::NeedsCheck(mut check)) => {
                    item.state = QueueState::Waiting;
                    check.pages.retain(|page| {
                        !item.caught_links.iter().any(|link| {
                            page.takes(link) && crate::hosts::windowed::is_saved_link(link)
                        })
                    });
                    item.recovery.check = (!check.pages.is_empty()).then_some(check);
                }
                (_, JobOutcome::Failed(error)) => {
                    item.state = QueueState::Failed;
                    item.error = Some(error);
                }
                (
                    _,
                    JobOutcome::Finished {
                        files,
                        format,
                        shared,
                    },
                ) => {
                    name_after_its_file(&mut item.video, &files);
                    match self.record(item, &files, &shared, format) {
                        Ok(result) => {
                            item.state = QueueState::Done;
                            item.result = Some(result);
                            remove_folder(&job.temp_folder);
                            self.site_worked(&item.video.url);
                            item.recovery = Recovery::default();
                            item.caught_links.clear();
                            item.unpack_files = None;
                        }
                        Err(error) => {
                            item.state = QueueState::Failed;
                            item.error = Some(error);
                        }
                    }
                }
            }
            let _ = self.store.update(item);
            drop(state);
        }
        // A file offered from the check window that the run never answered for: the window saves it, unless the
        // download was stopped or removed.
        if let Some(offer) = &job.window_offer {
            offer.settle(stopped);
        }
        self.schedule();
    }

    /// Records a finished download in the history: its file, or every file of a post or a file host's link with the
    /// size of all of them, its preset's name (empty for a post or a file host's link) and kind, the password that
    /// opened its link, and for a file host's link the paths in the share of the files it took (`shared`).
    fn record(
        &self,
        item: &QueueItem,
        files: &[PathBuf],
        shared: &[String],
        format: Option<String>,
    ) -> Result<DownloadResult, DownloadError> {
        let video = &item.video;
        let kind = RuleSubject {
            video,
            preset: item.preset.as_ref(),
        }
        .kind();
        let file_path = files.first().ok_or(DownloadError::NoFile)?;
        let saved: Vec<SavedFile> = files
            .iter()
            .map(|file| SavedFile {
                path: file.display().to_string(),
                size: fs::metadata(file)
                    .ok()
                    .map(|metadata| metadata.len() as i64),
            })
            .collect();
        // Two files of one path (some hosts allow it) are one path taken.
        let mut seen = HashSet::new();
        let share_paths: Vec<String> = shared
            .iter()
            .filter(|path| seen.insert(path.as_str()))
            .cloned()
            .collect();
        let sizes: Vec<i64> = saved.iter().filter_map(|file| file.size).collect();
        let file_size = (!sizes.is_empty()).then(|| sizes.iter().sum());
        let download = NewDownload {
            preset: item
                .preset
                .as_ref()
                .map(|preset| preset.name.clone())
                .unwrap_or_default(),
            file_path: file_path.display().to_string(),
            files: if video.post.is_some() || video.files.is_some() {
                saved
            } else {
                Vec::new()
            },
            file_size,
            format,
            downloaded_at: seconds_since_epoch(),
            kind,
            share_paths,
        };
        let download_id = self
            .history
            .record_opened_with(video, &download, item.password.as_ref())
            .map_err(|error| DownloadError::Storage {
                detail: error.to_string(),
            })?;
        Ok(DownloadResult {
            file_path: download.file_path,
            file_size,
            download_id,
        })
    }

    fn notify(&self) {
        let items = self.items();
        self.listener.changed(&items);
    }
}

/// A speed limit as yt-dlp's `--limit-rate` takes it: bytes per second, with an optional K, M or G ("500K", "4.2M").
/// Blank means no limit.
fn normalize_rate_limit(text: Option<&str>) -> Result<Option<String>, DownloadError> {
    let text = text.unwrap_or_default().trim();
    if text.is_empty() {
        return Ok(None);
    }
    let (number, unit) = match text.char_indices().last() {
        Some((index, unit)) if unit.is_ascii_alphabetic() => {
            (&text[..index], Some(unit.to_ascii_uppercase()))
        }
        _ => (text, None),
    };
    let is_number = number.parse::<f64>().is_ok_and(|value| value > 0.0)
        && number
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.');
    match unit {
        None | Some('K' | 'M' | 'G') if is_number => Ok(Some(format!(
            "{number}{}",
            unit.map(String::from).unwrap_or_default()
        ))),
        _ => Err(DownloadError::InvalidRateLimit {
            value: text.to_owned(),
        }),
    }
}

/// The site of a post whose files are downloaded with a pause between them (`DownloadSettings::pause_for`): one such
/// post runs at a time on each site.
fn paced_site(item: &QueueItem, options: &DownloadSettings) -> Option<PacedSite> {
    item.video.post.as_ref()?;
    options.pause_for(&item.video.url)?;
    sites::paced_site(&item.video.url)
}

fn find(items: &mut [QueueItem], id: i64) -> Result<&mut QueueItem, DownloadError> {
    items
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or(DownloadError::NotFound)
}

/// Where an item's partial files go: a folder of its own in the destination folder, so moving the finished file is a
/// rename on the same drive. The check window saves a file there too, for a host whose download runs in it.
fn temp_folder(item: &QueueItem) -> PathBuf {
    Path::new(&item.folder).join(format!(".tsundoku-{}", item.id))
}

/// Best effort: a folder that can't be deleted (a file still open in another app) only wastes space.
fn remove_folder(folder: &Path) {
    let _ = fs::remove_dir_all(folder);
}

fn prune_logs(logs_folder: &Path, lifetime: Duration) {
    let Ok(entries) = fs::read_dir(logs_folder) else {
        return;
    };
    for entry in entries.flatten() {
        let is_old = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > lifetime);
        if is_old
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "log")
        {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn seconds_since_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

/// A file host's link read through the check window is named after the link until its file is known: once it's
/// downloaded, it takes its file's name (`hosts::windowed`).
fn name_after_its_file(video: &mut VideoInfo, files: &[PathBuf]) {
    if video.files.is_some()
        && video.title == video.video_id
        && let [file] = files
        && let Some(name) = file.file_name().and_then(|name| name.to_str())
    {
        video.title = name.to_owned();
        if let Some(contents) = &mut video.files
            && contents.bytes == 0
        {
            contents.bytes = fs::metadata(file).map_or(0, |metadata| metadata.len());
        }
    }
}

#[cfg(test)]
mod tests;
