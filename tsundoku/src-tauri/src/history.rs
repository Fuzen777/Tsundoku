//! The preview and history commands: `tsundoku_core`'s preview service and history exposed to the UI.

use std::sync::Arc;

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tsundoku_core::downloads::DownloadQueue;
use tsundoku_core::history::{
    History, HistoryError, HistoryFile, HistoryFilters, HistoryPage, HistoryQuery, PastDownload,
    folder_count,
};
use tsundoku_core::listing::{self, ListingKind};
use tsundoku_core::metadata::PreviewError;
use tsundoku_core::preview::{LinkPreview, PreviewService};
use tsundoku_core::tools::ToolManager;

/// What a link is, for the preview card: a video or post, or a channel's, playlist's or account's items.
#[tauri::command]
pub async fn fetch_preview(
    tools: State<'_, Arc<ToolManager>>,
    history: State<'_, Arc<History>>,
    previews: State<'_, Arc<PreviewService>>,
    queue: State<'_, Arc<DownloadQueue>>,
    link: String,
) -> Result<LinkPreview, PreviewError> {
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

/// The files of a post its downloads left out, by number, or `None` when there were none (or no download).
#[tauri::command]
pub fn left_out_files(
    history: State<'_, Arc<History>>,
    video: tsundoku_core::metadata::VideoInfo,
) -> Result<Option<Vec<u32>>, HistoryError> {
    history.left_out_files(&video)
}

#[tauri::command]
pub fn cancel_preview(previews: State<'_, Arc<PreviewService>>) {
    previews.cancel();
}

/// Whether a link is a channel, playlist or account, from its address alone, for the preview card to say what it's
/// reading while it does.
#[tauri::command]
pub fn listing_kind(link: String) -> Option<ListingKind> {
    listing::kind_of(&link)
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

/// Every file of a download, for a post's list of files. Off the async runtime's threads, since it checks each
/// file still exists.
#[tauri::command]
pub async fn download_files(
    history: State<'_, Arc<History>>,
    download_id: i64,
) -> Result<Vec<HistoryFile>, HistoryError> {
    let history = Arc::clone(&history);
    tauri::async_runtime::spawn_blocking(move || history.files_of(download_id))
        .await
        .unwrap_or_else(|error| {
            Err(HistoryError::Database {
                detail: error.to_string(),
            })
        })
}

/// Opens one file of a download, by its place in `download_files`, in its default app.
#[tauri::command]
pub fn open_download_file(
    app: AppHandle,
    history: State<'_, Arc<History>>,
    download_id: i64,
    position: u32,
) -> Result<(), HistoryError> {
    let path = history.file_path(download_id, position)?;
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(open_failed)
}

/// Opens one file's folder in File Explorer, with the file selected.
#[tauri::command]
pub fn show_download_file_in_folder(
    app: AppHandle,
    history: State<'_, Arc<History>>,
    download_id: i64,
    position: u32,
) -> Result<(), HistoryError> {
    let path = history.file_path(download_id, position)?;
    app.opener().reveal_item_in_dir(path).map_err(open_failed)
}

/// One page of the history matching the view's search, filters and sort, from the `offset`th video on, with how many
/// match in all. Off the async runtime's threads, since it checks each file on the page still exists.
#[tauri::command]
pub async fn history_page(
    history: State<'_, Arc<History>>,
    query: HistoryQuery,
    offset: u32,
    limit: u32,
) -> Result<HistoryPage, HistoryError> {
    let history = Arc::clone(&history);
    tauri::async_runtime::spawn_blocking(move || history.page(&query, offset, limit))
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

/// The IDs of every video the history view's query matches, for ticking them all. Off the async runtime's threads,
/// since Only Missing Files checks each file.
#[tauri::command]
pub async fn history_ids(
    history: State<'_, Arc<History>>,
    query: HistoryQuery,
) -> Result<Vec<i64>, HistoryError> {
    let history = Arc::clone(&history);
    blocking(move || history.ids(&query)).await
}

/// Forgets several videos at once, as `forget_video` does each, and answers how many were in the history.
#[tauri::command]
pub fn forget_videos(
    history: State<'_, Arc<History>>,
    video_ids: Vec<i64>,
) -> Result<u32, HistoryError> {
    history.forget_all(&video_ids)
}

/// How many folders hold the videos' files that are still there, to ask before opening many.
#[tauri::command]
pub async fn history_folder_count(
    history: State<'_, Arc<History>>,
    video_ids: Vec<i64>,
) -> Result<u32, HistoryError> {
    let history = Arc::clone(&history);
    blocking(move || {
        let files = history.latest_files(&video_ids)?;
        Ok(folder_count(&files) as u32)
    })
    .await
}

/// Opens each folder holding the videos' files in File Explorer once, with those files selected.
#[tauri::command]
pub async fn show_videos_in_folders(
    app: AppHandle,
    history: State<'_, Arc<History>>,
    video_ids: Vec<i64>,
) -> Result<(), HistoryError> {
    let history = Arc::clone(&history);
    let files = blocking(move || history.latest_files(&video_ids)).await?;
    app.opener().reveal_items_in_dir(files).map_err(open_failed)
}

/// Runs `work` off the async runtime's threads, which must never wait on the disk.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, HistoryError> + Send + 'static,
) -> Result<T, HistoryError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .unwrap_or_else(|error| {
            Err(HistoryError::Database {
                detail: error.to_string(),
            })
        })
}

fn open_failed(error: tauri_plugin_opener::Error) -> HistoryError {
    HistoryError::OpenFailed {
        detail: error.to_string(),
    }
}
