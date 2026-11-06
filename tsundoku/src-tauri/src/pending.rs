//! The pending links' commands: `tsundoku_core`'s pending links exposed to the UI, with their changes sent as an
//! event.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tsundoku_core::downloads::DownloadError;
use tsundoku_core::pending::{self, AddedLinks, PendingLink, PendingLinks, PendingListener};
use tsundoku_core::settings::AddingSettings;

/// Sent with every pending link when they change.
const PENDING_CHANGED_EVENT: &str = "pending-changed";

type Pending<'a> = State<'a, Arc<PendingLinks>>;

/// Passes the pending links' changes on to the UI as events.
pub struct EventListener {
    pub app: AppHandle,
}

impl PendingListener for EventListener {
    fn changed(&self, links: &[PendingLink]) {
        let _ = self.app.emit(PENDING_CHANGED_EVENT, links);
    }
}

#[tauri::command]
pub fn pending_links(pending: Pending<'_>) -> Vec<PendingLink> {
    pending.list()
}

/// The links in pasted or dropped text, so the add bar can tell one link from several.
#[tauri::command]
pub fn links_in_text(text: String) -> Vec<String> {
    pending::links_in(&text)
}

#[tauri::command]
pub fn add_pending_links(
    pending: Pending<'_>,
    links: Vec<String>,
) -> Result<AddedLinks, DownloadError> {
    pending.add(&links)
}

#[tauri::command]
pub fn remove_pending_links(pending: Pending<'_>, ids: Vec<i64>) -> Result<(), DownloadError> {
    pending.remove(&ids)
}

#[tauri::command]
pub fn read_pending_link_again(pending: Pending<'_>, id: i64) -> Result<(), DownloadError> {
    pending.read_again(id)
}

/// Picks one link's preset, or `None` for the one picked for them all.
#[tauri::command]
pub fn set_pending_preset(
    pending: Pending<'_>,
    id: i64,
    preset_id: Option<String>,
) -> Result<(), DownloadError> {
    pending.set_preset(id, preset_id)
}

/// Starts links: those read join the queue, the others once they're read.
#[tauri::command]
pub fn start_pending_links(
    pending: Pending<'_>,
    ids: Vec<i64>,
    preset_id: String,
) -> Result<(), DownloadError> {
    pending.start(&ids, &preset_id)
}

/// How links get added besides pasting them: the clipboard watcher and Download Right Away.
#[tauri::command]
pub fn adding_options(pending: Pending<'_>) -> AddingSettings {
    pending.options()
}

#[tauri::command]
pub fn save_adding_options(
    pending: Pending<'_>,
    options: AddingSettings,
) -> Result<AddingSettings, DownloadError> {
    pending.set_options(options)
}
