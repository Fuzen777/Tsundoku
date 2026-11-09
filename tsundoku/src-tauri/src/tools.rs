//! The tool commands: `tsundoku_core::tools::ToolManager` exposed to the UI, with its progress sent as events.

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};
use tsundoku_core::data_folder::{self, DataPaths};
use tsundoku_core::downloads::DownloadError;
use tsundoku_core::first_run::{self, FirstRunChoices, FirstRunOptions};
use tsundoku_core::settings::{SettingsStore, ToolCaches, UpdateChannel};
use tsundoku_core::supported_sites::{self, SupportedSites};
use tsundoku_core::tools::{
    DetectedTool, ToolDownload, ToolError, ToolId, ToolManager, ToolProgress, ToolStatus,
    ToolUpdateCheck, ToolUpdateOutcome, ToolsOverview,
};

/// Sent with a `ToolProgress` while a tool is installed or updated.
const TOOL_PROGRESS_EVENT: &str = "tool-progress";
/// Sent when a tool changed without the UI asking, such as yt-dlp's or gallery-dl's daily update.
const TOOLS_CHANGED_EVENT: &str = "tools-changed";
/// How often the daily updates look whether one is due: a look only reads the settings, and a short wait means an
/// update put off while its tool ran comes soon after.
const DAILY_UPDATES_LOOK_AGAIN: Duration = Duration::from_secs(10 * 60);

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

/// The first-run setup's choices (where the tools go, their caches, the download folder), asked before the tools.
#[tauri::command]
pub fn first_run_options(
    app: AppHandle,
    settings: State<'_, Arc<SettingsStore>>,
    data_folder: State<'_, crate::DataFolder>,
) -> FirstRunOptions {
    // An installed copy's data folder in AppData: Tauri's own base folder, which a portable copy's override leaves as
    // it is, and the identifier.
    let installed_data_folder = app
        .path()
        .local_data_dir()
        .ok()
        .map(|folder| folder.join(&app.config().identifier));
    first_run::options(
        &settings.get(),
        &DataPaths::new(data_folder.path.clone()),
        data_folder::portable_folder(),
        installed_data_folder.as_deref(),
    )
}

#[tauri::command]
pub fn save_first_run_choices(
    settings: State<'_, Arc<SettingsStore>>,
    choices: FirstRunChoices,
) -> Result<(), DownloadError> {
    first_run::save(&settings, choices, data_folder::portable_folder()).map_err(|error| {
        DownloadError::Storage {
            detail: error.to_string(),
        }
    })
}

/// Settings → Tools: where the tools keep their caches. Fails only if the settings can't be saved.
#[tauri::command]
pub fn set_tool_caches(tools: Tools<'_>, caches: ToolCaches) -> Result<(), DownloadError> {
    tools
        .set_caches(caches)
        .map_err(|error| DownloadError::Storage {
            detail: error.to_string(),
        })
}

#[tauri::command]
pub async fn tools_overview(tools: Tools<'_>) -> Result<ToolsOverview, ToolError> {
    let tools = Arc::clone(&tools);
    run_blocking(move || Ok(tools.overview())).await
}

/// Every site yt-dlp or gallery-dl can read, with the browser profiles signed in to the sites that need an account.
#[tauri::command]
pub async fn supported_sites(tools: Tools<'_>) -> Result<SupportedSites, ToolError> {
    let tools = Arc::clone(&tools);
    run_blocking(move || {
        let yt_dlp = tools.executable(ToolId::YtDlp);
        let gallery_dl = tools.executable(ToolId::GalleryDl);
        Ok(supported_sites::supported_sites(
            yt_dlp.as_deref(),
            gallery_dl.as_deref(),
        ))
    })
    .await
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
pub async fn update_all_tools(
    app: AppHandle,
    tools: Tools<'_>,
) -> Result<Vec<ToolUpdateOutcome>, ToolError> {
    let tools = Arc::clone(&tools);
    run_blocking(move || Ok(tools.update_all(&progress_reporter(app)))).await
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

#[tauri::command]
pub async fn update_gallery_dl(app: AppHandle, tools: Tools<'_>) -> Result<ToolStatus, ToolError> {
    let tools = Arc::clone(&tools);
    run_blocking(move || tools.update_gallery_dl(&progress_reporter(app))).await
}

/// Updates the app's copies of yt-dlp and gallery-dl in the background, each at most once a day: at startup, then
/// whenever one is due again while Tsundoku keeps running (in the tray for days, say). Sites change often, and an
/// outdated tool is the most common reason a download fails. A tool that's running when its update is due is updated
/// at a later look, once it's free.
pub fn start_daily_updates(app: AppHandle, tools: Arc<ToolManager>) {
    thread::spawn(move || {
        loop {
            if tools.run_due_updates(&progress_reporter(app.clone())) {
                let _ = app.emit(TOOLS_CHANGED_EVENT, ());
            }
            thread::sleep(DAILY_UPDATES_LOOK_AGAIN);
        }
    });
}
