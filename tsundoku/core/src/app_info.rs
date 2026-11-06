use serde::Serialize;
use ts_rs::TS;

/// What the UI needs to know about the running app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    /// The app's version, from package.json.
    pub version: String,
    /// Whether the window has Windows 11's Mica backdrop. Without it (Windows 10), the UI paints its own background.
    pub has_mica_backdrop: bool,
    /// The data folder (settings, tools, history, logs), shown in Settings → About with Open Folder.
    pub data_folder: String,
    /// Windows started Tsundoku at sign-in and it keeps running in the tray, so the window stays hidden until it's
    /// opened from there.
    pub started_in_tray: bool,
}
