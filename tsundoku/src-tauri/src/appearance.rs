//! Settings → Appearance: whether Tsundoku is light, dark, or follows Windows. The window takes the theme here, which
//! sets its title bar, the Mica backdrop's tint and WebView2's preferred color scheme; the UI's colors follow the
//! choice on their own (`data-theme` in src/theme.ts), so they never wait on the WebView.

use std::sync::Arc;

use tauri::{AppHandle, Manager, State, WebviewWindow};
use tsundoku_core::downloads::DownloadError;
use tsundoku_core::settings::{AppearanceSettings, SettingsStore, Theme};

/// Gives the window the theme: Light or Dark, or `None` for Windows' own, which it then follows as it changes.
pub fn apply_theme(window: &WebviewWindow, theme: Theme) -> tauri::Result<()> {
    window.set_theme(match theme {
        Theme::Auto => None,
        Theme::Light => Some(tauri::Theme::Light),
        Theme::Dark => Some(tauri::Theme::Dark),
    })
}

/// Settings → Appearance.
#[tauri::command]
pub fn appearance_options(settings: State<'_, Arc<SettingsStore>>) -> AppearanceSettings {
    settings.get().appearance
}

/// Saves Settings → Appearance and gives the window the theme right away.
#[tauri::command]
pub fn save_appearance_options(
    app: AppHandle,
    settings: State<'_, Arc<SettingsStore>>,
    options: AppearanceSettings,
) -> Result<AppearanceSettings, DownloadError> {
    let options = options.checked();
    settings
        .update(|settings| settings.appearance = options.clone())
        .map_err(|error| DownloadError::Storage {
            detail: error.to_string(),
        })?;
    if let Some(window) = app.get_webview_window("main") {
        apply_theme(&window, options.theme).map_err(|error| DownloadError::Unexpected {
            detail: error.to_string(),
        })?;
    }
    Ok(options)
}
