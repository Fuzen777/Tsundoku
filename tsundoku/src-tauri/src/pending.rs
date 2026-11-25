//! The pending links' commands: `tsundoku_core`'s pending links exposed to the UI, with their changes sent as an
//! event.

use std::path::Path;
use std::sync::Arc;

use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Emitter, State};
use tsundoku_core::downloads::DownloadError;
use tsundoku_core::listing::ListingKind;
use tsundoku_core::metadata::VideoInfo;
use tsundoku_core::passwords::Password;
use tsundoku_core::pending::files::{self, LinkFileError};
use tsundoku_core::pending::{self, AddedLinks, PendingLink, PendingLinks, PendingListener};
use tsundoku_core::settings::AddingSettings;

use crate::{background, click_n_load};

/// Sent with every pending link when they change.
const PENDING_CHANGED_EVENT: &str = "pending-changed";
/// Sent with the options when how links get added changes, such as from the tray's Watch Clipboard.
const ADDING_OPTIONS_CHANGED_EVENT: &str = "adding-options-changed";

type Pending<'a> = State<'a, Arc<PendingLinks>>;

/// Passes the pending links' changes on to the UI as events, and to the tray and notifications.
pub struct EventListener {
    pub app: AppHandle,
}

impl PendingListener for EventListener {
    fn changed(&self, links: &[PendingLink]) {
        let _ = self.app.emit(PENDING_CHANGED_EVENT, links);
        background::pending_changed(&self.app, links);
    }

    fn copied_link_read(&self, video: &VideoInfo, started: bool) {
        background::copied_link_read(&self.app, &video.title, started);
    }

    fn copied_link_to_pick(&self, link: &str, listing: Option<ListingKind>) {
        background::copied_link_to_pick(&self.app, link, listing);
    }

    fn options_changed(&self, options: &AddingSettings) {
        let _ = self.app.emit(ADDING_OPTIONS_CHANGED_EVENT, options);
        background::watching_changed(&self.app, options.watch_clipboard);
    }
}

#[tauri::command]
pub fn pending_links(pending: Pending<'_>) -> Vec<PendingLink> {
    pending.list()
}

/// The links in typed or dropped text, so the app can tell one link from several.
#[tauri::command]
pub fn links_in_text(text: String) -> Vec<String> {
    pending::links_in(&text)
}

/// The links in text that came off the clipboard without being read first: Paste Link's, and Ctrl+V's outside a
/// text field.
#[tauri::command]
pub fn links_in_copy(text: String) -> Vec<String> {
    pending::links_in_copy(&text)
}

/// The header that names a dropped file for `links_in_link_file`: the name as a JSON string with every character
/// past ASCII escaped, since a header value is ASCII.
const LINK_FILE_NAME_HEADER: &str = "tsundoku-file-name";

/// Whether a dropped file can be read for links, from its name and size, before the UI reads it.
#[tauri::command]
pub fn check_link_file(name: String, size: u64) -> Result<(), LinkFileError> {
    files::check(&name, size).map(|_| ())
}

/// The links in a dropped file: its bytes as the request's body, its name in `LINK_FILE_NAME_HEADER`.
#[tauri::command]
pub fn links_in_link_file(request: Request<'_>) -> Result<Vec<String>, LinkFileError> {
    let name = request
        .headers()
        .get(LINK_FILE_NAME_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| serde_json::from_str::<String>(value).ok())
        .unwrap_or_default();
    let bytes = match request.body() {
        InvokeBody::Raw(bytes) => bytes.as_slice(),
        InvokeBody::Json(_) => &[],
    };
    files::links_in_file(&name, bytes)
}

#[tauri::command]
pub fn add_pending_links(
    pending: Pending<'_>,
    links: Vec<String>,
) -> Result<AddedLinks, DownloadError> {
    pending.add(&links)
}

/// Removes links, and answers how many: Undo puts them back (`restore_removed_pending_links`).
/// The links in a file chosen on this PC, read off the main thread since it can be several megabytes.
#[tauri::command]
pub async fn links_in_file_at(path: String) -> Result<Vec<String>, LinkFileError> {
    let name = path.clone();
    tauri::async_runtime::spawn_blocking(move || files::links_in_path(Path::new(&path)))
        .await
        .unwrap_or_else(|error| {
            Err(LinkFileError::Unreadable {
                name,
                detail: error.to_string(),
            })
        })
}

/// The text on the clipboard, for Paste Link and the empty views' Paste, which add the links in it.
#[tauri::command]
pub fn clipboard_text() -> Option<String> {
    tsundoku_core::clipboard::copied_text()
}

/// Adds the links read from files dropped on the window: they're checked as copied links are, and those with
/// nothing to download leave once read.
#[tauri::command]
pub fn add_pending_links_from_files(
    pending: Pending<'_>,
    links: Vec<String>,
) -> Result<AddedLinks, DownloadError> {
    pending.add_from_files(&links)
}

#[tauri::command]
pub fn remove_pending_links(pending: Pending<'_>, ids: Vec<i64>) -> Result<usize, DownloadError> {
    pending.remove(&ids)
}

/// Undo for the last removal.
#[tauri::command]
pub fn restore_removed_pending_links(pending: Pending<'_>) -> Result<(), DownloadError> {
    pending.restore_removed().map(|_| ())
}

#[tauri::command]
pub fn read_pending_link_again(pending: Pending<'_>, id: i64) -> Result<(), DownloadError> {
    pending.read_again(id)
}

/// Unlock, for a link that needs a password: it's read again with it.
#[tauri::command]
pub fn unlock_pending_link(
    pending: Pending<'_>,
    id: i64,
    password: String,
) -> Result<(), DownloadError> {
    let password = Password::new(password).ok_or(DownloadError::WrongState)?;
    pending.unlock(id, password)
}

/// A channel, playlist or account pasted or dropped in the window: it waits in Pending's Needs Attention to be picked
/// from, and the answer is its ID there, for the picker.
#[tauri::command]
pub fn add_link_to_pick(pending: Pending<'_>, link: String) -> Result<i64, DownloadError> {
    pending.add_to_pick(&link)
}

/// A link that needs picking was dealt with in the preview card's picker (items picked from it joined the queue, or
/// it's watched now): it leaves Pending.
#[tauri::command]
pub fn pending_link_picked(pending: Pending<'_>, id: i64) -> Result<(), DownloadError> {
    pending.picked(id)
}

/// Picks which files of a read post one link downloads, by their number in the post.
#[tauri::command]
pub fn set_pending_post_files(
    pending: Pending<'_>,
    id: i64,
    picked: Vec<u32>,
) -> Result<(), DownloadError> {
    pending.set_post_files(id, &picked)
}

/// Picks which files of a read shared folder one link downloads, by their place in its list, from 0.
#[tauri::command]
pub fn set_pending_shared_files(
    pending: Pending<'_>,
    id: i64,
    picked: Vec<u32>,
) -> Result<(), DownloadError> {
    pending.set_shared_files(id, &picked)
}

/// Picks one link's folder, or `None` for the one the rules give.
#[tauri::command]
pub fn set_pending_folder(
    pending: Pending<'_>,
    id: i64,
    folder: Option<String>,
) -> Result<(), DownloadError> {
    pending.set_folder(id, folder)
}

/// Picks the rule that decides first what it sets for one link, by its ID, or `None` for the rules as usual.
#[tauri::command]
pub fn set_pending_rule(
    pending: Pending<'_>,
    id: i64,
    rule_id: Option<String>,
) -> Result<(), DownloadError> {
    pending.set_rule(id, rule_id)
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

/// How links get added besides pasting them: the clipboard watcher, the clipboard shortcut, Download Right Away and
/// notifications.
#[tauri::command]
pub fn adding_options(pending: Pending<'_>) -> AddingSettings {
    pending.options()
}

/// Saves how links get added, first asking Windows for a newly picked clipboard shortcut: when another app has it,
/// nothing is saved and the error says so.
#[tauri::command]
pub fn save_adding_options(
    app: AppHandle,
    pending: Pending<'_>,
    options: AddingSettings,
) -> Result<AddingSettings, DownloadError> {
    let previous = pending.options().clipboard_shortcut;
    background::change_clipboard_shortcut(&app, previous, options.clipboard_shortcut)?;
    let saved = pending.set_options(options)?;
    // Listens or stops as the setting says, and listens again after the port was taken (Settings' Try Again).
    click_n_load::apply(&app, saved.click_n_load);
    Ok(saved)
}
