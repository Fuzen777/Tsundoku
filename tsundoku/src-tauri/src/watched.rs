//! The watched sources' commands: `tsundoku_core`'s watched channels, playlists and accounts exposed to the UI, with
//! their changes sent as an event.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use tauri::http::{Request, Response, StatusCode, header};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, UriSchemeContext, UriSchemeResponder};
use tsundoku_core::downloads::DownloadError;
use tsundoku_core::rules::RuleUnpacking;
use tsundoku_core::settings::{SettingsStore, WatchedSettings};
use tsundoku_core::watched::picture::{self, PictureCrop};
use tsundoku_core::watched::{
    ItemKey, NewItemsStart, NewWatchedSource, WatchedError, WatchedListener, WatchedSource,
    WatchedSources,
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

/// A source's Save In, Videos Start As, New Items Start (`None` for Settings → Watched's) and Unpack Archives (`None`
/// for the rules', else Settings → Archives').
#[tauri::command]
pub fn edit_watched_source(
    watched: Watched<'_>,
    id: i64,
    folder: Option<String>,
    preset_id: Option<String>,
    new_items_start: Option<NewItemsStart>,
    unpack: Option<RuleUnpacking>,
) -> Result<(), WatchedError> {
    watched.edit(id, folder, preset_id, new_items_start, unpack)
}

/// Settings → Watched: what every source does unless it has its own choice.
#[tauri::command]
pub fn watched_options(settings: State<'_, Arc<SettingsStore>>) -> WatchedSettings {
    settings.get().watched
}

#[tauri::command]
pub fn save_watched_options(
    settings: State<'_, Arc<SettingsStore>>,
    options: WatchedSettings,
) -> Result<WatchedSettings, DownloadError> {
    settings
        .update(|settings| settings.watched = options)
        .map_err(|error| DownloadError::Storage {
            detail: error.to_string(),
        })?;
    Ok(options)
}

#[tauri::command]
pub fn remove_watched_source(watched: Watched<'_>, id: i64) -> Result<(), WatchedError> {
    watched.remove(id)
}

/// The pictures a source can show instead of its listing's: the listing's, then its items', newest first.
#[tauri::command]
pub fn watched_picture_choices(watched: Watched<'_>, id: i64) -> Result<Vec<String>, WatchedError> {
    watched.picture_choices(id)
}

/// Custom: one of the source's pictures, read from its site and kept, with the area of it to show.
#[tauri::command]
pub async fn pick_watched_picture(
    watched: Watched<'_>,
    id: i64,
    url: String,
    crop: PictureCrop,
) -> Result<(), WatchedError> {
    let watched = Arc::clone(&watched);
    tauri::async_runtime::spawn_blocking(move || {
        watched.pick_picture(id, &url, crop, &picture::download)
    })
    .await
    .unwrap_or_else(|error| {
        Err(WatchedError::Storage {
            detail: error.to_string(),
        })
    })
}

/// Another area of the picture picked for a source.
#[tauri::command]
pub fn crop_watched_picture(
    watched: Watched<'_>,
    id: i64,
    crop: PictureCrop,
) -> Result<(), WatchedError> {
    watched.crop_picture(id, crop)
}

/// Automatic: the source shows its listing's picture again.
#[tauri::command]
pub fn reset_watched_picture(watched: Watched<'_>, id: i64) -> Result<(), WatchedError> {
    watched.reset_picture(id)
}

/// The data folder's `pictures\`, for `serve_picture`.
pub struct PicturesFolder(pub PathBuf);

/// Serves a picture picked for a watched source to the window, at `picture://localhost/<name>`
/// (`http://picture.localhost/<name>` in WebView2), read off the async runtime's threads. Only names
/// `picture::file_name` gives are served, from the pictures folder alone, so the window can't read any other file.
pub fn serve_picture<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let folder = context
        .app_handle()
        .try_state::<PicturesFolder>()
        .map(|folder| folder.0.clone());
    let name = request.uri().path().trim_start_matches('/').to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let picture = folder
            .zip(picture::kind_of_file(&name))
            .and_then(|(folder, kind)| {
                fs::read(folder.join(&name)).ok().map(|bytes| (kind, bytes))
            });
        let response = match picture {
            // A picture's name changes with the picture, so it never goes stale.
            Some((kind, bytes)) => Response::builder()
                .header(header::CONTENT_TYPE, kind.content_type())
                .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
                .body(bytes),
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new()),
        };
        if let Ok(response) = response {
            responder.respond(response);
        }
    });
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
