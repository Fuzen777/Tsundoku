//! Settings → Appearance: whether Tsundoku is light, dark, or follows Windows. The window takes the theme here, which
//! sets its title bar, the Mica backdrop's tint and WebView2's preferred color scheme; the UI's colors follow the
//! choice on their own (`data-theme` in src/theme.ts), so they never wait on the WebView.

use std::sync::Arc;

use tauri::{AppHandle, Manager, State, WebviewWindow};
use tsundoku_core::downloads::DownloadError;
use tsundoku_core::settings::{AppearanceSettings, SettingsStore, Theme};
#[cfg(windows)]
use tsundoku_core::settings::{color_of_color_ref, color_ref_of};

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

/// Opens Windows' color picker over the window, on `initial`, and answers the color chosen as `#rrggbb`, or `None` when
/// it was cancelled. WebView2 doesn't open a picker for a color input, so the custom accent's swatch asks here.
#[tauri::command]
pub async fn pick_color(app: AppHandle, initial: String) -> Option<String> {
    #[cfg(windows)]
    {
        // A window handle isn't `Send`; the picker only needs its value to sit over the window.
        let owner = app
            .get_webview_window("main")
            .and_then(|window| window.hwnd().ok())
            .map(|hwnd| hwnd.0 as isize)
            .unwrap_or_default();
        tauri::async_runtime::spawn_blocking(move || windows_color_picker::pick(owner, &initial))
            .await
            .ok()
            .flatten()
    }
    #[cfg(not(windows))]
    {
        let _ = (app, initial);
        None
    }
}

#[cfg(windows)]
mod windows_color_picker {
    use std::sync::Mutex;

    use windows_sys::Win32::UI::Controls::Dialogs::{
        CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW, ChooseColorW,
    };

    use super::{color_of_color_ref, color_ref_of};

    /// The picker's 16 custom colors, kept while the app runs, white to start with.
    static CUSTOM_COLORS: Mutex<[u32; 16]> = Mutex::new([0x00ff_ffff; 16]);

    /// Shows the picker, modal to `owner`, and blocks until it closes.
    pub fn pick(owner: isize, initial: &str) -> Option<String> {
        let mut custom_colors = CUSTOM_COLORS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut dialog = CHOOSECOLORW {
            lStructSize: size_of::<CHOOSECOLORW>() as u32,
            hwndOwner: owner as _,
            rgbResult: color_ref_of(initial).unwrap_or_default(),
            lpCustColors: custom_colors.as_mut_ptr(),
            Flags: CC_RGBINIT | CC_FULLOPEN,
            ..Default::default()
        };
        // SAFETY: `dialog` is filled in as ChooseColorW expects, and `lpCustColors` points at 16 colors that stay
        // locked until it returns.
        let chosen = unsafe { ChooseColorW(&mut dialog) } != 0;
        chosen.then(|| color_of_color_ref(dialog.rgbResult))
    }
}
