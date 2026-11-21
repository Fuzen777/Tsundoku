//! What's New, after an update or opened by hand: `tsundoku_core::changelog` over the CHANGELOG.md this build was
//! made with.

use std::sync::Arc;

use tauri::{AppHandle, State};
use tsundoku_core::changelog::{self, ChangelogSection};
use tsundoku_core::downloads::DownloadError;
use tsundoku_core::settings::SettingsStore;

/// The changelog as it was when this build was made, so a copy tells its own changes without the network.
const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

/// At startup: a first install records its version at once, so it has nothing new to tell once it's set up.
pub fn note_start(app: &AppHandle, settings: &SettingsStore, first_install: bool) {
    // Best effort: if it can't be saved, the first start after the setup tells this version's changes.
    let _ = changelog::note_start(
        settings,
        &app.package_info().version.to_string(),
        first_install,
    );
}

/// What changed since the version that ran last, newest first, for the What's New card; empty when there's nothing
/// to tell.
#[tauri::command]
pub fn whats_new(app: AppHandle, settings: State<'_, Arc<SettingsStore>>) -> Vec<ChangelogSection> {
    changelog::whats_new(
        &settings,
        CHANGELOG,
        &app.package_info().version.to_string(),
    )
}

/// Every version's changes up to the running one, newest first, for the What's New card opened by hand (Settings →
/// About), which browses them.
#[tauri::command]
pub fn release_notes(app: AppHandle) -> Vec<ChangelogSection> {
    changelog::up_to(CHANGELOG, &app.package_info().version.to_string())
}

/// The What's New card was closed: it doesn't show again until a newer version runs.
#[tauri::command]
pub fn dismiss_whats_new(
    app: AppHandle,
    settings: State<'_, Arc<SettingsStore>>,
) -> Result<(), DownloadError> {
    changelog::dismiss(&settings, &app.package_info().version.to_string()).map_err(|error| {
        DownloadError::Storage {
            detail: error.to_string(),
        }
    })
}
