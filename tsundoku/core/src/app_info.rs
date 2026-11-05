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
}
