//! The Tauri app: the window, the plugins, and the commands that expose `tsundoku_core` to the UI.

use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_window_state::StateFlags;
use tsundoku_core::app_info::AppInfo;

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
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .expect("tauri.conf.json defines the main window");
            // Mica needs Windows 11; on Windows 10 this fails and the UI paints its own background.
            let has_mica = window_vibrancy::apply_mica(&window, None).is_ok();
            app.manage(Backdrop { has_mica });
            show_main_window_if_still_hidden(window);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
