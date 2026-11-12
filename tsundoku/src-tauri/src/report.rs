//! Copy Details on a failed download: `tsundoku_core::bug_report` put together from the queue, the tools and Windows.

use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tsundoku_core::bug_report::{self, DownloadReport, LOG_LINES, ShownError, UserNames};
use tsundoku_core::downloads::{DownloadError, DownloadQueue};
use tsundoku_core::system;
use tsundoku_core::tools::ToolManager;

/// What a bug report about the download `id` needs, as plain text with the user's name blanked out: Tsundoku's, the
/// tools' and Windows' versions, the link, `shown_error` as the UI shows it, and the end of the download's log. Takes a
/// moment, since each tool is run for its version.
#[tauri::command]
pub async fn download_details(
    app: AppHandle,
    id: i64,
    shown_error: ShownError,
) -> Result<String, DownloadError> {
    tauri::async_runtime::spawn_blocking(move || {
        let queue = app.state::<Arc<DownloadQueue>>();
        let item = queue
            .items()
            .into_iter()
            .find(|item| item.id == id)
            .ok_or(DownloadError::NotFound)?;
        let report = DownloadReport {
            app_version: app.package_info().version.to_string(),
            windows: system::windows_version(),
            tools: bug_report::tool_versions(&app.state::<Arc<ToolManager>>().overview()),
            link: item.video.url,
            error: shown_error,
            log: bug_report::log_tail(&queue.log_path(id), LOG_LINES),
            password: item.password,
        };
        Ok(report.text(&UserNames::current()))
    })
    .await
    .unwrap_or_else(|error| {
        Err(DownloadError::Unexpected {
            detail: error.to_string(),
        })
    })
}
