//! The tool commands: `tsundoku_core::tools::ToolManager` exposed to the UI, with its progress sent as events.

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use tauri::{AppHandle, Emitter, State};
use tsundoku_core::settings::UpdateChannel;
use tsundoku_core::tools::{
    DetectedTool, ToolDownload, ToolError, ToolId, ToolManager, ToolProgress, ToolStatus,
    ToolUpdateCheck, ToolsOverview,
};

/// Sent with a `ToolProgress` while a tool is installed or updated.
const TOOL_PROGRESS_EVENT: &str = "tool-progress";
/// Sent when a tool changed without the UI asking, such as yt-dlp's daily update.
const TOOLS_CHANGED_EVENT: &str = "tools-changed";

type Tools<'a> = State<'a, Arc<ToolManager>>;

/// Runs blocking work (programs, downloads) off the async runtime's threads, which must never block.
async fn run_blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ToolError> + Send + 'static,
) -> Result<T, ToolError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .unwrap_or_else(|error| {
            Err(ToolError::Unexpected {
                detail: error.to_string(),
            })
        })
}

fn progress_reporter(app: AppHandle) -> impl Fn(ToolProgress) {
    move |progress| {
        // Nothing to do if the window is gone.
        let _ = app.emit(TOOL_PROGRESS_EVENT, progress);
    }
}

#[tauri::command]
pub fn tools_need_setup(tools: Tools<'_>) -> bool {
    tools.needs_setup()
}

#[tauri::command]
pub async fn tools_overview(tools: Tools<'_>) -> Result<ToolsOverview, ToolError> {
    let tools = Arc::clone(&tools);
    run_blocking(move || Ok(tools.overview())).await
}

#[tauri::command]
pub async fn detect_tools(tools: Tools<'_>) -> Result<Vec<DetectedTool>, ToolError> {
    let tools = Arc::clone(&tools);
    run_blocking(move || Ok(tools.detect())).await
}

#[tauri::command]
pub async fn tool_downloads(tools: Tools<'_>) -> Result<Vec<ToolDownload>, ToolError> {
    let tools = Arc::clone(&tools);
    run_blocking(move || Ok(tools.downloads())).await
}

#[tauri::command]
pub async fn install_tool(
    app: AppHandle,
    tools: Tools<'_>,
    id: ToolId,
) -> Result<ToolStatus, ToolError> {
    let tools = Arc::clone(&tools);
    run_blocking(move || tools.install(id, &progress_reporter(app))).await
}

#[tauri::command]
pub async fn use_tool_copy(
    tools: Tools<'_>,
    id: ToolId,
    path: PathBuf,
) -> Result<ToolStatus, ToolError> {
    let tools = Arc::clone(&tools);
    run_blocking(move || tools.use_copy(id, path)).await
}

#[tauri::command]
pub fn cancel_tool_operation(tools: Tools<'_>, id: ToolId) {
    tools.cancel(id);
}

#[tauri::command]
pub async fn update_yt_dlp(app: AppHandle, tools: Tools<'_>) -> Result<ToolStatus, ToolError> {
    let tools = Arc::clone(&tools);
    run_blocking(move || tools.update_yt_dlp(&progress_reporter(app))).await
}

#[tauri::command]
pub async fn check_tool_update(tools: Tools<'_>, id: ToolId) -> Result<ToolUpdateCheck, ToolError> {
    let tools = Arc::clone(&tools);
    run_blocking(move || tools.check_for_update(id)).await
}

#[tauri::command]
pub fn set_yt_dlp_channel(tools: Tools<'_>, channel: UpdateChannel) -> Result<(), ToolError> {
    tools.set_yt_dlp_channel(channel)
}

/// Updates the app's copy of yt-dlp in the background if it hasn't been checked today. Sites change often, and an
/// outdated yt-dlp is the most common reason a download fails.
pub fn start_daily_yt_dlp_update(app: AppHandle, tools: Arc<ToolManager>) {
    if !tools.yt_dlp_update_due() {
        return;
    }
    thread::spawn(move || {
        if tools.update_yt_dlp(&progress_reporter(app.clone())).is_ok() {
            let _ = app.emit(TOOLS_CHANGED_EVENT, ());
        }
    });
}
