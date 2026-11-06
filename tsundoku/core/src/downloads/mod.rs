//! The download queue: videos, posts of images and file hosts' files waiting, running (a few at a time), stopped, done or failed, kept in `tsundoku.db` so
//! they come back after a restart. A finished download is recorded in the history. See
//! `DOCUMENTATION/Docs/Download Pipeline.md`.

mod file_runner;
mod post_runner;
mod progress;
mod run;
mod runner;
mod store;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use self::run::{Notice, QueueRun, RunEvent, RunSummary, TaskbarProgress, notices};
pub use self::runner::{JobOutcome, JobRunner, JobSpec, PostJob, ToolRunner};
use self::store::QueueStore;
use crate::browsers;
use crate::cancel::CancelToken;
use crate::database::Database;
use crate::history::{History, NewDownload, SavedFile};
use crate::metadata::{PreviewError, VideoInfo};
use crate::post_names::{self, PostNameError};
use crate::presets::{Preset, PresetError, PresetManager};
use crate::rules::{self, Destination, RuleSubject};
use crate::settings::{DownloadSettings, SettingsStore};
use crate::sites;
use crate::sync::lock;
use crate::tools::{ToolId, ToolManager};

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
    /// The clipboard shortcut picked in Settings → Adding Links is taken by another app, so Windows won't give it to
    /// Tsundoku.
    #[error("{keys} is taken by another app")]
    ShortcutTaken { keys: String },
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
    runner: Arc<dyn JobRunner>,
    listener: Arc<dyn QueueListener>,
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
            runner: services.runner,
            listener: services.listener,
            myself: myself.clone(),
        });
        queue.schedule();
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
        let preset = self.presets.get(preset_id).ok_or(DownloadError::Preset {
            error: PresetError::NotFound,
        })?;
        preset
            .validate()
            .map_err(|error| DownloadError::Preset { error })?;
        if self.contains(&video.site, &video.video_id) {
            return Err(DownloadError::AlreadyQueued);
        }
        let folder = self.destination(&video, Some(&preset)).folder;
        let mut item = QueueItem {
            id: 0,
            folder,
            video,
            preset: Some(preset),
            state: QueueState::Waiting,
            overwrite,
            added_at: seconds_since_epoch(),
            result: None,
            error: None,
            progress: None,
        };
        item.id = self.store.insert(&item)?;
        lock(&self.state).items.push(item.clone());
        self.schedule();
        Ok(item)
    }

    /// Adds a post of images (a video whose `post` is set, from gallery-dl) or a file host's link (`files` set), to
    /// download all its files into the folder the rules give it (a post's by default the site's folder in the
    /// download folder). Files already there are skipped, so adding it again only fetches what's missing.
    pub fn add_post(&self, video: VideoInfo) -> Result<QueueItem, DownloadError> {
        if video.post.is_none() && video.files.is_none() {
            return Err(DownloadError::WrongState);
        }
        if self.contains(&video.site, &video.video_id) {
            return Err(DownloadError::AlreadyQueued);
        }
        let folder = self.destination(&video, None).folder;
        let mut item = QueueItem {
            id: 0,
            folder,
            video,
            preset: None,
            state: QueueState::Waiting,
            overwrite: false,
            added_at: seconds_since_epoch(),
            result: None,
            error: None,
            progress: None,
        };
        item.id = self.store.insert(&item)?;
        lock(&self.state).items.push(item.clone());
        self.schedule();
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
        self.store.update(item)?;
        drop(state);
        self.notify();
        Ok(())
    }

    /// Pause All: Stop for every download running or waiting. The running ones end keeping their partial files, the
    /// waiting ones are held. One being cancelled is left to finish cancelling.
    pub fn stop_all(&self) -> Result<(), DownloadError> {
        let mut state = lock(&self.state);
        let contents = &mut *state;
        let mut saved = Ok(());
        // The waiting ones first: a running one that ends meanwhile waits for this lock, then finds nothing to start.
        for item in contents
            .items
            .iter_mut()
            .filter(|item| item.state == QueueState::Waiting)
        {
            item.state = QueueState::Stopped;
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
        // The files go first, so once the item is gone, they are too.
        remove_folder(&temp_folder(item));
        state.items.retain(|item| item.id != id);
        self.store.delete(id)?;
        drop(state);
        self.notify();
        Ok(())
    }

    /// Removes the finished downloads from the queue. They stay in the history, and in the queue's memory until the
    /// next Clear Finished, so Undo can put them back (`restore_cleared`). How many were removed.
    pub fn clear_finished(&self) -> Result<usize, DownloadError> {
        let mut state = lock(&self.state);
        let mut cleared = Vec::new();
        let mut kept = Vec::new();
        let mut first_error = None;
        for (index, item) in std::mem::take(&mut state.items).into_iter().enumerate() {
            if item.state != QueueState::Done {
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

    /// Where a download of `video` would go, with `preset` (`None` for a post): the folder of the first rule that
    /// matches, else the preset's own folder, else the download folder. Adding uses it, and the preview card shows it.
    pub fn destination(&self, video: &VideoInfo, preset: Option<&Preset>) -> Destination {
        let settings = self.settings.get();
        rules::destination(
            &settings.rules.list,
            &RuleSubject { video, preset },
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

    /// Starts waiting items while there's room, in the queue's order: the order they were added, apart from those
    /// Download Next moved to the front. A post on a site that limits how fast it's read (Instagram) waits while another
    /// of that site's posts runs, since each pauses between its files on its own and two at once would read twice as
    /// fast; downloads from other sites go ahead of it meanwhile.
    fn schedule(&self) {
        let at_once = self.settings.get().downloads.at_once.max(1) as usize;
        let mut started = Vec::new();
        {
            let mut state = lock(&self.state);
            // Through a plain reference, so the items and the running map can be borrowed at once.
            let contents = &mut *state;
            while contents.running.len() < at_once {
                let pacing = contents
                    .items
                    .iter()
                    .any(|item| item.state == QueueState::Running && is_paced(item));
                let Some(item) = contents
                    .items
                    .iter_mut()
                    .find(|item| item.state == QueueState::Waiting && !(pacing && is_paced(item)))
                else {
                    break;
                };
                item.state = QueueState::Running;
                item.progress = Some(DownloadProgress::at(DownloadStage::Starting));
                if let Err(error) = self.store.update(item) {
                    item.state = QueueState::Failed;
                    item.error = Some(error);
                    continue;
                }
                let job = self.job_for(item);
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
                pause: sites::paces_downloads(&item.video.url).then_some(options.post_pause),
                files: contents.images + contents.videos,
                file_name: post_names::gallery_format(&options.post_file_name, &item.video),
            }),
            files: item.video.files,
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
        if end_request == Some(EndRequest::Cancel) {
            // The files go first, so once the item is gone, they are too.
            remove_folder(&job.temp_folder);
            state.items.retain(|item| item.id != job.id);
            let _ = self.store.delete(job.id);
            drop(state);
        } else if let Ok(item) = find(&mut state.items, job.id) {
            item.progress = None;
            match (end_request, outcome) {
                (Some(EndRequest::Stop), _) | (_, JobOutcome::Cancelled) => {
                    item.state = QueueState::Stopped
                }
                (_, JobOutcome::Failed(error)) => {
                    item.state = QueueState::Failed;
                    item.error = Some(error);
                }
                (_, JobOutcome::Finished { files, format }) => {
                    let preset = item
                        .preset
                        .as_ref()
                        .map(|preset| preset.name.clone())
                        .unwrap_or_default();
                    match self.record(&item.video, &preset, &files, format) {
                        Ok(result) => {
                            item.state = QueueState::Done;
                            item.result = Some(result);
                            remove_folder(&job.temp_folder);
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
        self.schedule();
    }

    /// Records a finished download in the history: its file, or every file of a post or a file host's link with the
    /// size of all of them. Their preset name is empty.
    fn record(
        &self,
        video: &VideoInfo,
        preset_name: &str,
        files: &[PathBuf],
        format: Option<String>,
    ) -> Result<DownloadResult, DownloadError> {
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
        let sizes: Vec<i64> = saved.iter().filter_map(|file| file.size).collect();
        let file_size = (!sizes.is_empty()).then(|| sizes.iter().sum());
        let download = NewDownload {
            preset: preset_name.to_owned(),
            file_path: file_path.display().to_string(),
            files: if video.post.is_some() || video.files.is_some() {
                saved
            } else {
                Vec::new()
            },
            file_size,
            format,
            downloaded_at: seconds_since_epoch(),
        };
        let download_id =
            self.history
                .record(video, &download)
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

/// A post on a site that limits how fast it's read, whose files are downloaded with a pause between them.
fn is_paced(item: &QueueItem) -> bool {
    item.video.post.is_some() && sites::paces_downloads(&item.video.url)
}

fn find(items: &mut [QueueItem], id: i64) -> Result<&mut QueueItem, DownloadError> {
    items
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or(DownloadError::NotFound)
}

/// Where an item's partial files go: a folder of its own in the destination folder, so moving the finished file is a
/// rename on the same drive.
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

#[cfg(test)]
mod tests;
