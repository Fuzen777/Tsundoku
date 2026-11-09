//! The preset, folder rule and queue commands: `tsundoku_core`'s presets, rules and download queue exposed to the
//! UI, with the queue's changes and progress sent as events. Also how the Pending and Queue views arrange their lists.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;
use tsundoku_core::downloads::{
    DownloadError, DownloadLog, DownloadProgress, DownloadProgressEvent, DownloadQueue, QueueItem,
    QueueListener, QueuePick,
};
use tsundoku_core::metadata::VideoInfo;
use tsundoku_core::presets::{Preset, PresetError, PresetManager, PresetsOverview};
use tsundoku_core::rules::{
    self, Destination, FolderRule, RightAwayRule, RuleError, RuleManager, RulePreset, RuleSubject,
};
use tsundoku_core::settings::{DownloadSettings, ListArrangements, SettingsStore};

use crate::queue_status::QueueStatus;

/// Sent with every item when the queue changes.
const QUEUE_CHANGED_EVENT: &str = "queue-changed";
/// Sent with a `DownloadProgressEvent` while a download runs.
const DOWNLOAD_PROGRESS_EVENT: &str = "download-progress";

type Presets<'a> = State<'a, Arc<PresetManager>>;
type Queue<'a> = State<'a, Arc<DownloadQueue>>;
type Rules<'a> = State<'a, Arc<RuleManager>>;

/// Passes the queue's changes on to the UI as events, and to the taskbar and the tray (`QueueStatus`).
pub struct EventListener {
    pub app: AppHandle,
    pub status: Arc<QueueStatus>,
}

impl QueueListener for EventListener {
    fn changed(&self, items: &[QueueItem]) {
        let _ = self.app.emit(QUEUE_CHANGED_EVENT, items);
        crate::background::queue_changed(&self.app, items);
        self.status.changed(&self.app, items);
    }

    fn progress(&self, id: i64, progress: &DownloadProgress) {
        let _ = self.app.emit(
            DOWNLOAD_PROGRESS_EVENT,
            DownloadProgressEvent {
                id,
                progress: progress.clone(),
            },
        );
        self.status.progress(&self.app, id, progress);
    }
}

#[tauri::command]
pub fn presets_overview(presets: Presets<'_>) -> PresetsOverview {
    presets.overview()
}

#[tauri::command]
pub fn save_preset(presets: Presets<'_>, preset: Preset) -> Result<PresetsOverview, PresetError> {
    presets.save(preset)
}

#[tauri::command]
pub fn duplicate_preset(
    presets: Presets<'_>,
    id: String,
    name: String,
) -> Result<Preset, PresetError> {
    presets.duplicate(&id, name)
}

#[tauri::command]
pub fn add_preset(presets: Presets<'_>, name: String) -> Result<Preset, PresetError> {
    presets.add(name)
}

#[tauri::command]
pub fn move_preset(
    presets: Presets<'_>,
    id: String,
    index: usize,
) -> Result<PresetsOverview, PresetError> {
    presets.move_to(&id, index)
}

#[tauri::command]
pub fn delete_preset(presets: Presets<'_>, id: String) -> Result<PresetsOverview, PresetError> {
    presets.delete(&id)
}

#[tauri::command]
pub fn reset_preset(presets: Presets<'_>, id: String) -> Result<PresetsOverview, PresetError> {
    presets.reset(&id)
}

#[tauri::command]
pub fn set_default_preset(
    presets: Presets<'_>,
    id: String,
) -> Result<PresetsOverview, PresetError> {
    presets.set_default(&id)
}

/// The yt-dlp arguments a preset gives, shown under the preset editor.
#[tauri::command]
pub fn preset_arguments(preset: Preset) -> Result<Vec<String>, PresetError> {
    preset.to_args()
}

/// The folder rules, in the order they're tried.
#[tauri::command]
pub fn folder_rules(manager: Rules<'_>) -> Vec<FolderRule> {
    manager.list()
}

/// Replaces every folder rule, and answers with them as saved: trimmed, new ones with their ID.
#[tauri::command]
pub fn save_folder_rules(
    manager: Rules<'_>,
    rules: Vec<FolderRule>,
) -> Result<Vec<FolderRule>, RuleError> {
    manager.save(rules)
}

/// Where a download would go and what decided it, for the preview card: with a preset for a video, none for a post.
#[tauri::command]
pub fn download_destination(
    queue: Queue<'_>,
    presets: Presets<'_>,
    video: VideoInfo,
    preset_id: Option<String>,
) -> Destination {
    let preset = preset_id.and_then(|id| presets.get(&id));
    queue.destination(&video, preset.as_ref())
}

/// The preset a rule picks for a video, and which rule, for the preview card and Pending: `None` when no rule picks
/// one, and for a post or a file host's files.
#[tauri::command]
pub fn rule_preset(
    manager: Rules<'_>,
    presets: Presets<'_>,
    video: VideoInfo,
) -> Option<RulePreset> {
    manager.start_preset(&video, |id| presets.get(id).is_some())
}

/// Whether a rule makes copied and sent links of a video start once read, and which rule, for Rules' Test a Link:
/// `None` when Settings' Download Right Away decides.
#[tauri::command]
pub fn rule_download_right_away(
    manager: Rules<'_>,
    presets: Presets<'_>,
    video: VideoInfo,
    preset_id: Option<String>,
) -> Option<RightAwayRule> {
    let preset = preset_id.and_then(|id| presets.get(&id));
    let subject = RuleSubject {
        video: &video,
        preset: preset.as_ref(),
    };
    rules::right_away_rule(&manager.list(), &subject)
}

#[tauri::command]
pub fn queue_items(queue: Queue<'_>) -> Vec<QueueItem> {
    queue.items()
}

#[tauri::command]
pub fn add_to_queue(
    queue: Queue<'_>,
    video: VideoInfo,
    preset_id: String,
    overwrite: bool,
    folder: Option<String>,
) -> Result<QueueItem, DownloadError> {
    queue.add_in(video, &preset_id, overwrite, folder)
}

/// Adds a post of images, whose files all go into the site's folder in the posts folder.
#[tauri::command]
pub fn add_post_to_queue(
    queue: Queue<'_>,
    video: VideoInfo,
    folder: Option<String>,
) -> Result<QueueItem, DownloadError> {
    queue.add_post_in(video, folder)
}

/// Saves a download that isn't running in a folder picked for it, or back in the rules' (`None`).
#[tauri::command]
pub fn set_queue_folder(
    queue: Queue<'_>,
    id: i64,
    folder: Option<String>,
) -> Result<QueueItem, DownloadError> {
    queue.set_folder(id, folder)
}

/// Adds the items picked from a channel, playlist or account, each as its own download. How many were added.
#[tauri::command]
pub fn add_many_to_queue(queue: Queue<'_>, picks: Vec<QueuePick>) -> Result<usize, DownloadError> {
    queue.add_many(picks)
}

/// Where downloads go when no folder is chosen: a Tsundoku folder in Downloads.
#[tauri::command]
pub fn default_download_folder() -> String {
    tsundoku_core::settings::default_download_folder()
        .display()
        .to_string()
}

#[tauri::command]
pub fn stop_download(queue: Queue<'_>, id: i64) -> Result<(), DownloadError> {
    queue.stop(id)
}

#[tauri::command]
pub fn resume_download(queue: Queue<'_>, id: i64) -> Result<(), DownloadError> {
    queue.resume(id)
}

#[tauri::command]
pub fn retry_download(queue: Queue<'_>, id: i64) -> Result<(), DownloadError> {
    queue.retry(id)
}

/// Try Now: starts a download that waits to try again, or for its site, without waiting any longer.
#[tauri::command]
pub fn try_download_now(queue: Queue<'_>, id: i64) -> Result<(), DownloadError> {
    queue.try_now(id)
}

#[tauri::command]
pub fn cancel_download(queue: Queue<'_>, id: i64) -> Result<(), DownloadError> {
    queue.cancel(id)
}

/// Pause All: stops every running download and holds every waiting one.
#[tauri::command]
pub fn stop_all_downloads(queue: Queue<'_>) -> Result<(), DownloadError> {
    queue.stop_all()
}

#[tauri::command]
pub fn resume_all_downloads(queue: Queue<'_>) -> Result<(), DownloadError> {
    queue.resume_all()
}

#[tauri::command]
pub fn retry_failed_downloads(queue: Queue<'_>) -> Result<(), DownloadError> {
    queue.retry_failed()
}

/// Moves a waiting download to the front of the line.
#[tauri::command]
pub fn download_next(queue: Queue<'_>, id: i64) -> Result<(), DownloadError> {
    queue.download_next(id)
}

/// Removes the finished downloads, and answers how many: Undo puts them back (`restore_cleared_downloads`).
#[tauri::command]
pub fn clear_finished_downloads(queue: Queue<'_>) -> Result<usize, DownloadError> {
    queue.clear_finished()
}

/// Removes one finished download; Undo puts it back (`restore_cleared_downloads`).
#[tauri::command]
pub fn remove_finished_download(queue: Queue<'_>, id: i64) -> Result<(), DownloadError> {
    queue.remove_finished(id)
}

/// Undo for the last Clear Finished, or the last finished download removed.
#[tauri::command]
pub fn restore_cleared_downloads(queue: Queue<'_>) -> Result<(), DownloadError> {
    queue.restore_cleared().map(|_| ())
}

/// How downloads run: how many at once, the speed limit, the cookies browser, and where posts go and how fast.
#[tauri::command]
pub fn download_options(queue: Queue<'_>) -> DownloadSettings {
    queue.options()
}

#[tauri::command]
pub fn save_download_options(
    queue: Queue<'_>,
    options: DownloadSettings,
) -> Result<DownloadSettings, DownloadError> {
    queue.set_options(options)
}

/// How the Pending and Queue views group and sort their lists.
#[tauri::command]
pub fn list_arrangements(settings: State<'_, Arc<SettingsStore>>) -> ListArrangements {
    settings.get().lists
}

#[tauri::command]
pub fn save_list_arrangements(
    settings: State<'_, Arc<SettingsStore>>,
    arrangements: ListArrangements,
) -> Result<ListArrangements, DownloadError> {
    settings
        .update(|settings| settings.lists = arrangements)
        .map_err(|error| DownloadError::Storage {
            detail: error.to_string(),
        })?;
    Ok(arrangements)
}

/// Opens the download's log in the default text editor.
/// The end of a download's log, for the Queue's log viewer.
#[tauri::command]
pub fn download_log(queue: Queue<'_>, id: i64) -> Result<DownloadLog, DownloadError> {
    queue.log(id)
}

#[tauri::command]
pub fn open_download_log(app: AppHandle, queue: Queue<'_>, id: i64) -> Result<(), DownloadError> {
    let path = queue.log_path(id);
    if !path.is_file() {
        return Err(DownloadError::NotFound);
    }
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|error| DownloadError::Unexpected {
            detail: error.to_string(),
        })
}
