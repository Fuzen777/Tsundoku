//! The preset and queue commands: `tsundoku_core`'s presets and download queue exposed to the UI, with the queue's
//! changes and progress sent as events.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;
use tsundoku_core::downloads::{
    DownloadError, DownloadProgress, DownloadProgressEvent, DownloadQueue, QueueItem, QueueListener,
};
use tsundoku_core::metadata::VideoInfo;
use tsundoku_core::presets::{Preset, PresetError, PresetManager, PresetsOverview};
use tsundoku_core::settings::DownloadSettings;

/// Sent with every item when the queue changes.
const QUEUE_CHANGED_EVENT: &str = "queue-changed";
/// Sent with a `DownloadProgressEvent` while a download runs.
const DOWNLOAD_PROGRESS_EVENT: &str = "download-progress";

type Presets<'a> = State<'a, Arc<PresetManager>>;
type Queue<'a> = State<'a, Arc<DownloadQueue>>;

/// Passes the queue's changes on to the UI as events.
pub struct EventListener {
    pub app: AppHandle,
}

impl QueueListener for EventListener {
    fn changed(&self, items: &[QueueItem]) {
        let _ = self.app.emit(QUEUE_CHANGED_EVENT, items);
    }

    fn progress(&self, id: i64, progress: &DownloadProgress) {
        let _ = self.app.emit(
            DOWNLOAD_PROGRESS_EVENT,
            DownloadProgressEvent {
                id,
                progress: progress.clone(),
            },
        );
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
) -> Result<QueueItem, DownloadError> {
    queue.add(video, &preset_id, overwrite)
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

#[tauri::command]
pub fn cancel_download(queue: Queue<'_>, id: i64) -> Result<(), DownloadError> {
    queue.cancel(id)
}

#[tauri::command]
pub fn clear_finished_downloads(queue: Queue<'_>) -> Result<(), DownloadError> {
    queue.clear_finished()
}

/// How downloads run: how many at once, the speed limit and the cookies browser.
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

/// Opens the download's log in the default text editor.
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
