use std::cmp::Ordering;

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
    /// A portable copy (`data_folder::PORTABLE_MARKER` beside its exe), which keeps its data beside itself and doesn't
    /// update itself yet.
    pub portable: bool,
}

/// Compares two of Tsundoku's versions ("0.1.35", or "v0.1.35" as tags name them) part by part, as numbers. A
/// missing part, or one that isn't a number, counts as 0; anything after a `-` (a pre-release) is ignored.
pub fn compare_versions(left: &str, right: &str) -> Ordering {
    let (left, right) = (version_parts(left), version_parts(right));
    (0..left.len().max(right.len()))
        .map(|index| {
            let part = |parts: &[u64]| parts.get(index).copied().unwrap_or(0);
            part(&left).cmp(&part(&right))
        })
        .find(|ordering| ordering.is_ne())
        .unwrap_or(Ordering::Equal)
}

fn version_parts(version: &str) -> Vec<u64> {
    let version = version.trim().trim_start_matches(['v', 'V']);
    let release = version.split('-').next().unwrap_or_default();
    release
        .split('.')
        .map(|part| part.trim().parse().unwrap_or(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_as_numbers() {
        assert_eq!(compare_versions("0.1.10", "0.1.9"), Ordering::Greater);
        assert_eq!(compare_versions("0.1.35", "0.2.0"), Ordering::Less);
        assert_eq!(compare_versions("v0.1.35", "0.1.35"), Ordering::Equal);
        assert_eq!(compare_versions("1.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("0.2.0-beta.1", "0.2.0"), Ordering::Equal);
        assert_eq!(compare_versions("", "0.0.1"), Ordering::Less);
    }
}
