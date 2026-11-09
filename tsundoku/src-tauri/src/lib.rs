//! The Tauri app: the window, the plugins, and the commands that expose `tsundoku_core` to the UI.

mod appearance;
mod background;
mod backup;
mod downloads;
mod history;
mod pending;
mod queue_status;
mod report;
mod tools;
mod whats_new;

use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::utils::config::AppDirectoriesOverride;
use tauri::{App, AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_window_state::StateFlags;
use tsundoku_core::app_info::AppInfo;
use tsundoku_core::data_folder::{self, BuildKind, DataPaths};
use tsundoku_core::database::Database;
use tsundoku_core::downloads::{DownloadQueue, QueueServices, ToolRunner};
use tsundoku_core::history::History;
use tsundoku_core::pending::{PendingLinks, PendingServices};
use tsundoku_core::presets::PresetManager;
use tsundoku_core::preview::{PreviewService, ToolReader};
use tsundoku_core::rules::RuleManager;
use tsundoku_core::settings::SettingsStore;
use tsundoku_core::thumbnails::{FfmpegMaker, Thumbnails};
use tsundoku_core::tools::ToolManager;

/// How long the hidden main window waits for the UI to show it before showing itself, so a UI that fails to start
/// still leaves a window on screen.
const SHOW_WINDOW_FALLBACK_DELAY: Duration = Duration::from_secs(3);

/// Whether the main window got Mica, decided once at startup.
struct Backdrop {
    has_mica: bool,
}

/// Where this build keeps its data, resolved once at startup.
pub(crate) struct DataFolder {
    pub(crate) path: PathBuf,
}

#[tauri::command]
fn app_info(
    app: AppHandle,
    backdrop: State<'_, Backdrop>,
    data_folder: State<'_, DataFolder>,
) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        has_mica_backdrop: backdrop.has_mica,
        data_folder: data_folder.path.display().to_string(),
        started_in_tray: app
            .try_state::<background::StartedInTray>()
            .is_some_and(|started| started.0),
        portable: data_folder::portable_folder().is_some(),
    }
}

/// Opens the data folder in File Explorer. Fails only if Windows can't open it, with its reason.
#[tauri::command]
fn open_data_folder(app: AppHandle, data_folder: State<'_, DataFolder>) -> Result<(), String> {
    app.opener()
        .open_path(data_folder.path.display().to_string(), None::<&str>)
        .map_err(|error| error.to_string())
}

/// Brings the main window to the front: for a second launch of the app, the tray icon, or a click on a notification.
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

/// Opens what lives in the data folder (the settings with the presets and rules, the tools they set up, the history,
/// the download queue and the pending links) and gives each to Tauri's state for the commands. Development builds
/// keep their own data folder (AGENTS.md → Data and Testing).
fn open_data(app: &App) -> Result<Arc<ToolManager>, Box<dyn Error>> {
    let build = if cfg!(debug_assertions) {
        BuildKind::Development
    } else {
        BuildKind::Release
    };
    let root = data_folder::resolve(build, app.path().app_local_data_dir()?)
        .ok_or("Windows has no local app data folder")?;
    let paths = DataPaths::new(root.clone());
    fs::create_dir_all(paths.root())?;
    app.manage(DataFolder { path: root });

    let settings = Arc::new(SettingsStore::open(paths.settings_file())?);
    let database = Arc::new(Database::open(&paths.database_file())?);
    // For Back Up and Restore, which copy the history out of it and back in.
    app.manage(Arc::clone(&database));
    let thumbnails_folder = paths.thumbnails_folder();
    let history = Arc::new(
        History::new(Arc::clone(&database), paths.archive_file())?
            .with_thumbnails_folder(thumbnails_folder.clone()),
    );
    let presets = Arc::new(PresetManager::new(Arc::clone(&settings)));
    let rules = Arc::new(RuleManager::new(Arc::clone(&settings)));
    let logs_folder = paths.logs_folder();
    let tools = Arc::new(ToolManager::new(
        paths,
        Arc::clone(&settings),
        app.package_info().version.to_string(),
    ));
    // The history's thumbnails, made in the background and kept in the data folder.
    Thumbnails::new(
        thumbnails_folder.clone(),
        Arc::clone(&history),
        Arc::new(FfmpegMaker {
            tools: Arc::clone(&tools),
            work_folder: thumbnails_folder.clone(),
        }),
    )
    .start();
    app.manage(history::ThumbnailsFolder(thumbnails_folder));
    // The taskbar's progress and the tray's tooltip, from the queue's changes.
    let queue_status = Arc::new(queue_status::QueueStatus::default());
    app.manage(Arc::clone(&queue_status));
    let queue = DownloadQueue::open(QueueServices {
        database: Arc::clone(&database),
        history: Arc::clone(&history),
        tools: Arc::clone(&tools),
        presets: Arc::clone(&presets),
        settings: Arc::clone(&settings),
        logs_folder,
        runner: Arc::new(ToolRunner),
        listener: Arc::new(downloads::EventListener {
            app: app.handle().clone(),
            status: queue_status,
        }),
    })?;
    let pending = PendingLinks::open(PendingServices {
        database,
        history: Arc::clone(&history),
        queue: Arc::clone(&queue),
        presets: Arc::clone(&presets),
        settings: Arc::clone(&settings),
        reader: Arc::new(ToolReader {
            tools: Arc::clone(&tools),
            settings: Arc::clone(&settings),
        }),
        listener: Arc::new(pending::EventListener {
            app: app.handle().clone(),
        }),
    })?;
    // Links copied in other apps, while Settings → Adding Links has the watcher on.
    #[cfg(windows)]
    tsundoku_core::pending::watcher::watch_clipboard(
        tsundoku_core::clipboard::SystemClipboard::new(),
        Arc::downgrade(&pending),
    );

    app.manage(Arc::clone(&tools));
    app.manage(history);
    app.manage(presets);
    app.manage(rules);
    app.manage(queue);
    app.manage(pending);
    app.manage(Arc::new(PreviewService::new()));
    background::start(app.handle(), &settings);
    app.manage(settings);
    Ok(tools)
}

/// A release build with `.portable.txt` beside its exe keeps everything in a `data` folder there: Tauri's app folders
/// follow, so the settings, tools, history, WebView data and window position all do. Development builds keep their
/// own data folder (AGENTS.md → Data and Testing).
fn make_portable_if_marked(context: &mut tauri::Context) {
    if cfg!(debug_assertions) {
        return;
    }
    let Some(folder) = std::env::current_exe()
        .ok()
        .and_then(|exe| data_folder::portable_folder_of(&exe))
    else {
        return;
    };
    context.config_mut().app.app_directories_override = Some(AppDirectoriesOverride::Root(
        folder.join(data_folder::PORTABLE_DATA_FOLDER),
    ));
    data_folder::set_portable_folder(folder);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut context = tauri::generate_context!();
    make_portable_if_marked(&mut context);
    tauri::Builder::default()
        // First, as the plugin requires: a second launch focuses the running window instead of opening another copy,
        // so two copies never share the data folder. A launch for a page a browser sent hands it to the deep link
        // plugin instead (background::start), leaving the browser in front.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !background::is_sent_link(&args) {
                focus_main_window(app);
            }
        }))
        // tsundoku://add?url=… links, which the browser bookmark in Settings → Adding Links opens.
        .plugin(tauri_plugin_deep_link::init())
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
        // Start with Windows: an entry among Windows' startup apps, which starts Tsundoku in the tray.
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name(background::startup_name())
                .args([background::IN_TRAY_ARG])
                .build(),
        )
        // The clipboard shortcut, registered once the settings are read (background::start).
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(background::on_shortcut)
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .register_asynchronous_uri_scheme_protocol("thumbnail", history::serve_thumbnail)
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .expect("tauri.conf.json defines the main window");
            // Mica needs Windows 11; on Windows 10 this fails and the UI paints its own background.
            let has_mica = window_vibrancy::apply_mica(&window, None).is_ok();
            app.manage(Backdrop { has_mica });

            let tool_manager = open_data(app)?;
            whats_new::note_start(
                app.handle(),
                &app.state::<Arc<SettingsStore>>(),
                tool_manager.needs_setup(),
            );
            tools::start_daily_updates(app.handle().clone(), tool_manager);
            // The saved theme, while the window is still hidden, so its title bar, Mica and the WebView match from the
            // first frame. A window left in Windows' theme is still usable, so a failure doesn't stop the app.
            let theme = app.state::<Arc<SettingsStore>>().get().appearance.theme;
            let _ = appearance::apply_theme(&window, theme);

            // Started by Windows with the tray on, the window waits there until it's opened.
            if !app.state::<background::StartedInTray>().0 {
                show_main_window_if_still_hidden(window);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            open_data_folder,
            backup::save_backup,
            backup::restore_backup,
            tools::tools_need_setup,
            tools::first_run_options,
            tools::save_first_run_choices,
            tools::set_tool_caches,
            tools::tools_overview,
            tools::detect_tools,
            tools::tool_downloads,
            tools::install_tool,
            tools::use_tool_copy,
            tools::cancel_tool_operation,
            tools::update_yt_dlp,
            tools::update_gallery_dl,
            tools::check_tool_update,
            tools::update_all_tools,
            tools::set_yt_dlp_channel,
            tools::supported_sites,
            history::fetch_preview,
            history::cancel_preview,
            history::left_out_files,
            history::listing_kind,
            history::open_download,
            history::show_download_in_folder,
            history::history_page,
            history::history_filters,
            history::video_downloads,
            history::forget_video,
            history::history_ids,
            history::forget_videos,
            history::history_folder_count,
            history::show_videos_in_folders,
            history::download_files,
            history::open_download_file,
            history::show_download_file_in_folder,
            downloads::presets_overview,
            downloads::save_preset,
            downloads::duplicate_preset,
            downloads::delete_preset,
            downloads::reset_preset,
            downloads::set_default_preset,
            downloads::preset_arguments,
            downloads::folder_rules,
            downloads::save_folder_rules,
            downloads::download_destination,
            downloads::rule_preset,
            downloads::queue_items,
            downloads::add_to_queue,
            downloads::add_post_to_queue,
            downloads::add_many_to_queue,
            downloads::default_download_folder,
            downloads::stop_download,
            downloads::resume_download,
            downloads::retry_download,
            downloads::try_download_now,
            downloads::cancel_download,
            downloads::stop_all_downloads,
            downloads::resume_all_downloads,
            downloads::retry_failed_downloads,
            downloads::download_next,
            downloads::clear_finished_downloads,
            downloads::restore_cleared_downloads,
            downloads::open_download_log,
            report::download_details,
            whats_new::whats_new,
            whats_new::dismiss_whats_new,
            downloads::list_arrangements,
            downloads::save_list_arrangements,
            pending::pending_links,
            pending::links_in_text,
            pending::check_link_file,
            pending::links_in_link_file,
            pending::links_in_file_at,
            pending::clipboard_text,
            pending::add_pending_links,
            pending::add_pending_links_from_files,
            pending::remove_pending_links,
            pending::restore_removed_pending_links,
            pending::read_pending_link_again,
            pending::set_pending_preset,
            pending::set_pending_post_files,
            pending::start_pending_links,
            pending::adding_options,
            pending::save_adding_options,
            background::background_options,
            background::save_background_options,
            appearance::appearance_options,
            appearance::save_appearance_options,
            downloads::download_options,
            downloads::save_download_options,
        ])
        .run(context)
        .expect("error while running tauri application");
}
