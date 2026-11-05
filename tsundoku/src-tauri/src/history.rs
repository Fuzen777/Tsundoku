//! The preview and history commands: `tsundoku_core`'s preview service and history exposed to the UI.

use std::sync::Arc;

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tsundoku_core::downloads::DownloadQueue;
use tsundoku_core::history::{
    History, HistoryError, HistoryFilters, HistoryQuery, HistoryVideo, PastDownload,
};
use tsundoku_core::metadata::PreviewError;
use tsundoku_core::preview::{PreviewService, VideoPreview};
use tsundoku_core::tools::ToolManager;

#[tauri::command]
pub async fn fetch_preview(
    tools: State<'_, Arc<ToolManager>>,
    history: State<'_, Arc<History>>,
    previews: State<'_, Arc<PreviewService>>,
    queue: State<'_, Arc<DownloadQueue>>,
    link: String,
) -> Result<VideoPreview, PreviewError> {
    let (tools, history, previews, queue) = (
        Arc::clone(&tools),
        Arc::clone(&history),
        Arc::clone(&previews),
        Arc::clone(&queue),
    );
    // yt-dlp takes seconds to answer: off the async runtime's threads, which must never block.
    tauri::async_runtime::spawn_blocking(move || previews.fetch(&tools, &history, &queue, &link))
        .await
        .unwrap_or_else(|error| {
            Err(PreviewError::Failed {
                detail: error.to_string(),
            })
        })
}

#[tauri::command]
pub fn cancel_preview(previews: State<'_, Arc<PreviewService>>) {
    previews.cancel();
}

/// Opens a downloaded file in its default app. The UI names the download, never a path, so it can't open anything
/// else.
#[tauri::command]
pub fn open_download(
    app: AppHandle,
    history: State<'_, Arc<History>>,
    download_id: i64,
) -> Result<(), HistoryError> {
    let path = history.download_path(download_id)?;
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(open_failed)
}

/// Opens the download's folder in File Explorer, with the file selected.
#[tauri::command]
pub fn show_download_in_folder(
    app: AppHandle,
    history: State<'_, Arc<History>>,
    download_id: i64,
) -> Result<(), HistoryError> {
    let path = history.download_path(download_id)?;
    app.opener().reveal_item_in_dir(path).map_err(open_failed)
}

/// The history matching the view's search, filters and sort. Off the async runtime's threads, since it checks each
/// file still exists.
#[tauri::command]
pub async fn search_history(
    history: State<'_, Arc<History>>,
    query: HistoryQuery,
) -> Result<Vec<HistoryVideo>, HistoryError> {
    let history = Arc::clone(&history);
    tauri::async_runtime::spawn_blocking(move || history.search(&query))
        .await
        .unwrap_or_else(|error| {
            Err(HistoryError::Database {
                detail: error.to_string(),
            })
        })
}

#[tauri::command]
pub fn history_filters(history: State<'_, Arc<History>>) -> Result<HistoryFilters, HistoryError> {
    history.filters()
}

/// Every download of a video, newest first, for a history row that's expanded.
#[tauri::command]
pub fn video_downloads(
    history: State<'_, Arc<History>>,
    video_id: i64,
) -> Result<Vec<PastDownload>, HistoryError> {
    history.downloads_of(video_id)
}

/// Removes a video from the history and `archive.txt`, so it can be downloaded again. Its files stay.
#[tauri::command]
pub fn forget_video(history: State<'_, Arc<History>>, video_id: i64) -> Result<(), HistoryError> {
    history.forget(video_id)
}

fn open_failed(error: tauri_plugin_opener::Error) -> HistoryError {
    HistoryError::OpenFailed {
        detail: error.to_string(),
    }
}
