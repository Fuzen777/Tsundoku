//! The Tauri app: the window, the plugins, and the commands that expose `tsundoku_core` to the UI.

mod downloads;
mod history;
mod tools;

use std::error::Error;
use std::fs;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::{App, AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_window_state::StateFlags;
use tsundoku_core::app_info::AppInfo;
use tsundoku_core::data_folder::{self, BuildKind, DataPaths};
use tsundoku_core::database::Database;
use tsundoku_core::downloads::{DownloadQueue, QueueServices, YtDlpRunner};
use tsundoku_core::history::History;
use tsundoku_core::presets::PresetManager;
use tsundoku_core::preview::PreviewService;
use tsundoku_core::settings::SettingsStore;
use tsundoku_core::tools::ToolManager;

/// How long the hidden main window waits for the UI to show it before showing itself, so a UI that fails to start
/// still leaves a window on screen.
const SHOW_WINDOW_FALLBACK_DELAY: Duration = Duration::from_secs(3);

/// Whether the main window got Mica, decided once at startup.
struct Backdrop {
    has_mica: bool,
}

#[tauri::command]
fn app_info(app: AppHandle, backdrop: State<'_, Backdrop>) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        has_mica_backdrop: backdrop.has_mica,
    }
}

/// Brings the main window to the front, for a second launch of the app.
fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        // Best effort: a window that can't be restored or focused is still the one already open.
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// The window starts hidden (tauri.conf.json) and the UI shows it after its first paint, so it never flashes white.
/// This shows it anyway if the UI hasn't after a few seconds.
fn show_main_window_if_still_hidden(window: WebviewWindow) {
    thread::spawn(move || {
        thread::sleep(SHOW_WINDOW_FALLBACK_DELAY);
        if !window.is_visible().unwrap_or(true) {
            let _ = window.show();
        }
    });
}

/// Opens what lives in the data folder (the settings, the tools they set up, the history and the download queue) and
/// gives each to Tauri's state for the commands. Development builds keep their own data folder (AGENTS.md → Data and
/// Testing).
fn open_data(app: &App) -> Result<Arc<ToolManager>, Box<dyn Error>> {
    let build = if cfg!(debug_assertions) {
        BuildKind::Development
    } else {
        BuildKind::Release
    };
    let root = data_folder::resolve(build, app.path().app_local_data_dir()?)
        .ok_or("Windows has no local app data folder")?;
    let paths = DataPaths::new(root);
    fs::create_dir_all(paths.root())?;

    let settings = Arc::new(SettingsStore::open(paths.settings_file())?);
    let database = Arc::new(Database::open(&paths.database_file())?);
    let history = Arc::new(History::new(Arc::clone(&database), paths.archive_file())?);
    let presets = Arc::new(PresetManager::new(Arc::clone(&settings)));
    let logs_folder = paths.logs_folder();
    let tools = Arc::new(ToolManager::new(
        paths,
        Arc::clone(&settings),
        app.package_info().version.to_string(),
    ));
    let queue = DownloadQueue::open(QueueServices {
        database,
        history: Arc::clone(&history),
        tools: Arc::clone(&tools),
        presets: Arc::clone(&presets),
        settings,
        logs_folder,
        runner: Arc::new(YtDlpRunner),
        listener: Arc::new(downloads::EventListener {
            app: app.handle().clone(),
        }),
    })?;

    app.manage(Arc::clone(&tools));
    app.manage(history);
    app.manage(presets);
    app.manage(queue);
    app.manage(Arc::new(PreviewService::new()));
    Ok(tools)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // First, as the plugin requires: a second launch focuses the running window instead of opening another copy,
        // so two copies never share the data folder.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main_window(app)
        }))
        // Visibility is left out: the window stays hidden until the UI has painted.
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // Updates from the GitHub releases' latest.json, signed with the key whose public half is in tauri.conf.json.
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Restarts the app once an update is installed.
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .expect("tauri.conf.json defines the main window");
            // Mica needs Windows 11; on Windows 10 this fails and the UI paints its own background.
            let has_mica = window_vibrancy::apply_mica(&window, None).is_ok();
            app.manage(Backdrop { has_mica });

            let tool_manager = open_data(app)?;
            tools::start_daily_yt_dlp_update(app.handle().clone(), tool_manager);

            show_main_window_if_still_hidden(window);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            tools::tools_need_setup,
            tools::tools_overview,
            tools::detect_tools,
            tools::tool_downloads,
            tools::install_tool,
            tools::use_tool_copy,
            tools::cancel_tool_operation,
            tools::update_yt_dlp,
            tools::check_tool_update,
            tools::set_yt_dlp_channel,
            history::fetch_preview,
            history::cancel_preview,
            history::open_download,
            history::show_download_in_folder,
            history::search_history,
            history::history_filters,
            history::video_downloads,
            history::forget_video,
            downloads::presets_overview,
            downloads::save_preset,
            downloads::duplicate_preset,
            downloads::delete_preset,
            downloads::reset_preset,
            downloads::set_default_preset,
            downloads::preset_arguments,
            downloads::queue_items,
            downloads::add_to_queue,
            downloads::stop_download,
            downloads::resume_download,
            downloads::retry_download,
            downloads::cancel_download,
            downloads::clear_finished_downloads,
            downloads::open_download_log,
            downloads::download_options,
            downloads::save_download_options,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
