//! The preview and history commands: `tsundoku_core`'s preview service and history exposed to the UI.

use std::sync::Arc;

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tsundoku_core::history::{History, HistoryError};
use tsundoku_core::metadata::PreviewError;
use tsundoku_core::preview::{PreviewService, VideoPreview};
use tsundoku_core::tools::ToolManager;

#[tauri::command]
pub async fn fetch_preview(
    tools: State<'_, Arc<ToolManager>>,
    history: State<'_, Arc<History>>,
    previews: State<'_, Arc<PreviewService>>,
    link: String,
) -> Result<VideoPreview, PreviewError> {
    let (tools, history, previews) = (
        Arc::clone(&tools),
        Arc::clone(&history),
        Arc::clone(&previews),
    );
    // yt-dlp takes seconds to answer: off the async runtime's threads, which must never block.
    tauri::async_runtime::spawn_blocking(move || previews.fetch(&tools, &history, &link))
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

fn open_failed(error: tauri_plugin_opener::Error) -> HistoryError {
    HistoryError::OpenFailed {
        detail: error.to_string(),
    }
}
