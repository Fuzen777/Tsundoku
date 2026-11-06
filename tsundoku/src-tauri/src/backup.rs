//! Settings → About's Back Up and Restore: `tsundoku_core::backup` exposed to the UI, which restarts the app after a
//! restore so everything reads what was restored.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tsundoku_core::backup::{self, BackupData, BackupError};
use tsundoku_core::data_folder::{self, DataPaths};
use tsundoku_core::database::Database;
use tsundoku_core::settings::SettingsStore;

use crate::DataFolder;

/// Runs work on a backup off the async runtime's threads, since copying the database and zipping take a moment.
async fn with_backup_data(
    app: AppHandle,
    work: impl FnOnce(&BackupData) -> Result<(), BackupError> + Send + 'static,
) -> Result<(), BackupError> {
    tauri::async_runtime::spawn_blocking(move || {
        let paths = DataPaths::new(app.state::<DataFolder>().path.clone());
        let app_version = app.package_info().version.to_string();
        work(&BackupData {
            paths: &paths,
            settings: &app.state::<Arc<SettingsStore>>(),
            database: &app.state::<Arc<Database>>(),
            app_version: &app_version,
            portable: data_folder::portable_folder().is_some(),
        })
    })
    .await
    .unwrap_or_else(|error| {
        Err(BackupError::Storage {
            detail: error.to_string(),
        })
    })
}

/// Saves a backup of the settings and the history to `path`, chosen in the UI's save dialog.
#[tauri::command]
pub async fn save_backup(app: AppHandle, path: PathBuf) -> Result<(), BackupError> {
    with_backup_data(app, move |data| backup::create(data, &path)).await
}

/// Replaces the settings and the history with the backup at `path`, after saving the current ones as an automatic
/// backup in the data folder's `backups\`.
#[tauri::command]
pub async fn restore_backup(app: AppHandle, path: PathBuf) -> Result<(), BackupError> {
    with_backup_data(app, move |data| backup::restore(data, &path).map(|_| ())).await
}
