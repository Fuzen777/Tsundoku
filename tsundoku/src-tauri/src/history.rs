//! The preview and history commands: `tsundoku_core`'s preview service and history exposed to the UI, and the
//! `thumbnail` protocol that shows it the history's kept thumbnails.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use tauri::http::{Request, Response, StatusCode, header};
use tauri::ipc::Channel;
use tauri::{Manager, Runtime, State, UriSchemeContext, UriSchemeResponder};
use tsundoku_core::data_folder::{DataPaths, HistoryData};
use tsundoku_core::downloads::DownloadQueue;
use tsundoku_core::history::{
    History, HistoryError, HistoryFile, HistoryFilters, HistoryPage, HistoryQuery, HistorySize,
    PastDownload, folder_count,
};
use tsundoku_core::listing::{self, ListingKind, ListingMore, MoreItems, MoreRead};
use tsundoku_core::metadata::PreviewError;
use tsundoku_core::passwords::Password;
use tsundoku_core::preview::{LinkPreview, PreviewService};
use tsundoku_core::relocate::MovedFiles;
use tsundoku_core::thumbnails;
use tsundoku_core::tools::ToolManager;

use crate::{DataFolder, shell};

/// What a link is, for the preview card: a video or post, or a channel's, playlist's or account's items.
#[tauri::command]
pub async fn fetch_preview(
    tools: State<'_, Arc<ToolManager>>,
    history: State<'_, Arc<History>>,
    previews: State<'_, Arc<PreviewService>>,
    queue: State<'_, Arc<DownloadQueue>>,
    link: String,
    password: Option<String>,
) -> Result<LinkPreview, PreviewError> {
    let (tools, history, previews, queue) = (
        Arc::clone(&tools),
        Arc::clone(&history),
        Arc::clone(&previews),
        Arc::clone(&queue),
    );
    // yt-dlp takes seconds to answer: off the async runtime's threads, which must never block.
    let password = password.and_then(Password::new);
    tauri::async_runtime::spawn_blocking(move || {
        previews.fetch(&tools, &history, &queue, &link, password)
    })
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

/// The files of a shared folder its downloads didn't take, by their place in its list, or `None` when there were
/// none (or no download).
#[tauri::command]
pub fn left_out_shared_files(
    history: State<'_, Arc<History>>,
    video: tsundoku_core::metadata::VideoInfo,
) -> Result<Option<Vec<u32>>, HistoryError> {
    history.left_out_shared_files(&video)
}

#[tauri::command]
pub fn cancel_preview(previews: State<'_, Arc<PreviewService>>) {
    previews.cancel();
}

/// The items after those a listing on the preview card has: the next ones (Load More), on until enough new ones (Load
/// New) or all the rest (Load All), as `read` says, sent over `on_items` a batch at a time as they're read. Ends once
/// they're read, or with why it stopped.
#[tauri::command]
pub async fn load_more_of_listing(
    tools: State<'_, Arc<ToolManager>>,
    history: State<'_, Arc<History>>,
    previews: State<'_, Arc<PreviewService>>,
    queue: State<'_, Arc<DownloadQueue>>,
    more: ListingMore,
    read: MoreRead,
    on_items: Channel<MoreItems>,
) -> Result<(), PreviewError> {
    let (tools, history, previews, queue) = (
        Arc::clone(&tools),
        Arc::clone(&history),
        Arc::clone(&previews),
        Arc::clone(&queue),
    );
    // Reading takes seconds to hours: off the async runtime's threads, which must never block.
    tauri::async_runtime::spawn_blocking(move || {
        previews.fetch_more(&tools, &history, &queue, &more, read, &mut |batch| {
            // Only fails once the window is gone, which ends the app.
            let _ = on_items.send(batch);
        })
    })
    .await
    .unwrap_or_else(|error| {
        Err(PreviewError::Failed {
            detail: error.to_string(),
        })
    })
}

/// How many items a listing on the preview card holds in all, when its first read didn't say and another read can (a
/// YouTube channel's videos), for Load All to say how many it would list; `None` when no read can.
#[tauri::command]
pub async fn count_listing(
    tools: State<'_, Arc<ToolManager>>,
    previews: State<'_, Arc<PreviewService>>,
    queue: State<'_, Arc<DownloadQueue>>,
    more: ListingMore,
) -> Result<Option<u32>, PreviewError> {
    let (tools, previews, queue) = (
        Arc::clone(&tools),
        Arc::clone(&previews),
        Arc::clone(&queue),
    );
    // yt-dlp takes seconds: off the async runtime's threads, which must never block.
    tauri::async_runtime::spawn_blocking(move || previews.count_more(&tools, &queue, &more))
        .await
        .unwrap_or_else(|error| {
            Err(PreviewError::Failed {
                detail: error.to_string(),
            })
        })
}

/// Stops Load More, Load New or Load All, keeping what was read.
#[tauri::command]
pub fn cancel_loading_more(previews: State<'_, Arc<PreviewService>>) {
    previews.cancel_more();
}

/// Whether a link is a channel, playlist or account, from its address alone, for the preview card to say what it's
/// reading while it does.
#[tauri::command]
pub fn listing_kind(link: String) -> Option<ListingKind> {
    listing::kind_of(&link)
}

/// Opens a downloaded file in its default app. The UI names the download, never a path, so it can't open anything
/// else. Off the window's thread, like every command that opens something (`shell`).
#[tauri::command]
pub async fn open_download(
    history: State<'_, Arc<History>>,
    download_id: i64,
) -> Result<(), HistoryError> {
    let history = Arc::clone(&history);
    blocking(move || {
        let path = history.download_path(download_id)?;
        shell::open_file(&path).map_err(open_failed)
    })
    .await
}

/// Opens the download's folder in the default folder app, with the file selected (`shell::show_in_folders`).
#[tauri::command]
pub async fn show_download_in_folder(
    history: State<'_, Arc<History>>,
    download_id: i64,
) -> Result<(), HistoryError> {
    let history = Arc::clone(&history);
    blocking(move || {
        let path = history.download_path(download_id)?;
        shell::show_in_folders(&[path]).map_err(open_failed)
    })
    .await
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
pub async fn open_download_file(
    history: State<'_, Arc<History>>,
    download_id: i64,
    position: u32,
) -> Result<(), HistoryError> {
    let history = Arc::clone(&history);
    blocking(move || {
        let path = history.file_path(download_id, position)?;
        shell::open_file(&path).map_err(open_failed)
    })
    .await
}

/// Opens one file's folder in the default folder app, with the file selected.
#[tauri::command]
pub async fn show_download_file_in_folder(
    history: State<'_, Arc<History>>,
    download_id: i64,
    position: u32,
) -> Result<(), HistoryError> {
    let history = Arc::clone(&history);
    blocking(move || {
        let path = history.file_path(download_id, position)?;
        shell::show_in_folders(&[path]).map_err(open_failed)
    })
    .await
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

/// What the files of the videos the history view's query matches take on the disk: every download's files that are
/// still there. Off the async runtime's threads, since it asks the disk about each folder.
#[tauri::command]
pub async fn history_size(
    history: State<'_, Arc<History>>,
    query: HistoryQuery,
) -> Result<HistorySize, HistoryError> {
    let history = Arc::clone(&history);
    blocking(move || history.size(&query)).await
}

/// What the data folder holds for the history: the database, the kept thumbnails, the downloads' logs and yt-dlp's
/// archive. Off the async runtime's threads, since it lists the thumbnails' folder.
#[tauri::command]
pub async fn history_data_size(
    data_folder: State<'_, DataFolder>,
) -> Result<HistoryData, HistoryError> {
    let paths = DataPaths::new(data_folder.path.clone());
    blocking(move || Ok(paths.history_data())).await
}

/// Forgets several videos at once, as `forget_video` does each, and answers how many were in the history.
#[tauri::command]
pub fn forget_videos(
    history: State<'_, Arc<History>>,
    video_ids: Vec<i64>,
) -> Result<u32, HistoryError> {
    history.forget_all(&video_ids)
}

/// History's Move Files…: moves the videos' files into `folder`, off the async runtime's threads, since a move to
/// another drive copies.
#[tauri::command]
pub async fn move_history_files(
    history: State<'_, Arc<History>>,
    video_ids: Vec<i64>,
    folder: PathBuf,
) -> Result<MovedFiles, HistoryError> {
    let history = Arc::clone(&history);
    blocking(move || history.move_files(&video_ids, &folder)).await
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

/// Opens each folder holding the videos' files once, with those files selected (only the first when it falls back to
/// File Explorer: `shell::show_in_folders`).
#[tauri::command]
pub async fn show_videos_in_folders(
    history: State<'_, Arc<History>>,
    video_ids: Vec<i64>,
) -> Result<(), HistoryError> {
    let history = Arc::clone(&history);
    blocking(move || {
        let files = history.latest_files(&video_ids)?;
        shell::show_in_folders(&files).map_err(open_failed)
    })
    .await
}

/// Where the history's thumbnails are kept, for the `thumbnail` protocol.
pub struct ThumbnailsFolder(pub PathBuf);

/// Serves a kept thumbnail to the window, at `thumbnail://localhost/<name>` (`http://thumbnail.localhost/<name>` in
/// WebView2), read off the async runtime's threads. Only names `thumbnails::file_name` gives are served, from the
/// thumbnails folder alone, so the window can't read any other file.
pub fn serve_thumbnail<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let folder = context
        .app_handle()
        .try_state::<ThumbnailsFolder>()
        .map(|folder| folder.0.clone());
    let name = request.uri().path().trim_start_matches('/').to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = folder
            .filter(|_| thumbnails::is_file_name(&name))
            .and_then(|folder| fs::read(folder.join(&name)).ok());
        let response = match bytes {
            Some(bytes) => Response::builder()
                .header(header::CONTENT_TYPE, "image/jpeg")
                .header(header::CACHE_CONTROL, "max-age=3600")
                .body(bytes),
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new()),
        };
        if let Ok(response) = response {
            responder.respond(response);
        }
    });
}

/// Runs `work` off the async runtime's threads, which must never wait on the disk or Windows' shell.
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

fn open_failed(error: std::io::Error) -> HistoryError {
    HistoryError::OpenFailed {
        detail: error.to_string(),
    }
}
