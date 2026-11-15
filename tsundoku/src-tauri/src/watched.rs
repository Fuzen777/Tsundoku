//! The watched sources' commands: `tsundoku_core`'s watched channels, playlists and accounts exposed to the UI, with
//! their changes sent as an event.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tsundoku_core::watched::{
    ItemKey, NewWatchedSource, WatchedError, WatchedListener, WatchedSource, WatchedSources,
};

/// Sent with every watched source when they change.
const WATCHED_CHANGED_EVENT: &str = "watched-changed";

type Watched<'a> = State<'a, Arc<WatchedSources>>;

/// Passes the watched sources' changes on to the UI as events.
pub struct EventListener {
    pub app: AppHandle,
}

impl WatchedListener for EventListener {
    fn changed(&self, sources: &[WatchedSource]) {
        let _ = self.app.emit(WATCHED_CHANGED_EVENT, sources);
    }
}

#[tauri::command]
pub fn watched_sources(watched: Watched<'_>) -> Result<Vec<WatchedSource>, WatchedError> {
    watched.list()
}

/// Watch, from the preview card's picker. The new source's ID.
#[tauri::command]
pub fn add_watched_source(
    watched: Watched<'_>,
    source: NewWatchedSource,
) -> Result<i64, WatchedError> {
    watched.add(source)
}

/// A source's Save In and Videos Start As.
#[tauri::command]
pub fn edit_watched_source(
    watched: Watched<'_>,
    id: i64,
    folder: Option<String>,
    preset_id: Option<String>,
) -> Result<(), WatchedError> {
    watched.edit(id, folder, preset_id)
}

#[tauri::command]
pub fn remove_watched_source(watched: Watched<'_>, id: i64) -> Result<(), WatchedError> {
    watched.remove(id)
}

/// Check Now: the check runs in the background, and `watched-changed` tells when it ends.
#[tauri::command]
pub fn check_watched_source(watched: Watched<'_>, id: i64) -> Result<(), WatchedError> {
    watched.check(id)
}

/// Check All: each of these sources is checked in the background, one after the other in this order.
#[tauri::command]
pub fn check_watched_sources(watched: Watched<'_>, ids: Vec<i64>) -> Result<(), WatchedError> {
    watched.check_many(&ids)
}

/// Download Selected: the picked items are queued and the new ones of `skipped` (the picker's other items shown) are
/// skipped; or Download on skipped items, with none skipped. How many were queued.
#[tauri::command]
pub fn download_watched_items(
    watched: Watched<'_>,
    id: i64,
    items: Vec<ItemKey>,
    skipped: Vec<ItemKey>,
) -> Result<usize, WatchedError> {
    watched.download(id, &items, &skipped)
}

/// Skip All: these items of the source, those the picker shows, are skipped.
#[tauri::command]
pub fn skip_watched_items(
    watched: Watched<'_>,
    id: i64,
    items: Vec<ItemKey>,
) -> Result<(), WatchedError> {
    watched.skip(id, &items)
}

/// All New Items' Download Selected: the picked new items of every source, each with its source's folder and preset,
/// and the new ones of `skipped` skipped. How many were queued.
#[tauri::command]
pub fn download_new_watched_items(
    watched: Watched<'_>,
    items: Vec<ItemKey>,
    skipped: Vec<ItemKey>,
) -> Result<usize, WatchedError> {
    watched.download_new(&items, &skipped)
}

/// All New Items' Skip All: these items, those it shows, are skipped in every source where they're new.
#[tauri::command]
pub fn skip_new_watched_items(
    watched: Watched<'_>,
    items: Vec<ItemKey>,
) -> Result<(), WatchedError> {
    watched.skip_new(&items)
}
