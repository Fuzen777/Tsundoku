//! The check window: a site's own page in a small browser window of Tsundoku's, for a check only a person can pass
//! (`tsundoku_core::hosts::check`), such as Cloudflare's in front of some MediaFire files. A download that meets one
//! waits in the queue until the person opens it with Open Check; nothing opens by itself.
//!
//! The window can't reach Tsundoku's commands: its pages are on the web, which Tauri keeps from the app's commands,
//! no capability names it, and it never navigates to a local page. It keeps its own lasting profile in the data
//! folder's `browser` folder, so a check passed once is remembered as a browser remembers it. Tsundoku never touches
//! the check: the person does what it asks, then starts the download on the site's page. The window catches that
//! download's address as the page goes to it, so the WebView never fetches the file (a download it starts anyway is
//! stopped before a byte is saved), and gives the address to the download, which then downloads the file from it.
//! With several files behind the check, it shows each one's page in turn.
//!
//! The page stays on the site: an advert can't take the window elsewhere. A window the page opens (MediaFire's
//! Download button may open the file in one, sending the page to an advert) is a hidden one that never loads
//! anything: the file's address is caught from it the same way.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use tauri::webview::{
    DownloadEvent, NewWindowFeatures, NewWindowResponse, PermissionKind, PermissionResponse,
};
use tauri::{AppHandle, Manager, State, Url, WebviewUrl, WebviewWindowBuilder, WindowEvent, Wry};
use tsundoku_core::data_folder::DataPaths;
use tsundoku_core::downloads::{DownloadError, DownloadQueue};
use tsundoku_core::hosts::check::{BrowserCheck, CheckPage};

use crate::DataFolder;

/// The window's label: there's one at a time.
const LABEL: &str = "check";
/// The windows its pages open, numbered.
static OPENED_WINDOWS: AtomicUsize = AtomicUsize::new(0);
const WIDTH: f64 = 1000.0;
const HEIGHT: f64 = 760.0;
const MIN_WIDTH: f64 = 480.0;
const MIN_HEIGHT: f64 = 400.0;

/// What the check window shows, while it's open.
#[derive(Default)]
pub struct CheckWindow(Mutex<Option<Visit>>);

impl CheckWindow {
    fn lock(&self) -> MutexGuard<'_, Option<Visit>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The pages of one download's check, and the addresses caught on them so far.
struct Visit {
    /// The download it's for, or `None` for a development build's own (`open_from_environment`).
    item: Option<i64>,
    site: String,
    /// The first page, for its site, which the window doesn't leave.
    home: CheckPage,
    /// The pages still to get past, the one shown first.
    left: Vec<CheckPage>,
    /// How many pages it had.
    total: usize,
    caught: Vec<String>,
}

impl Visit {
    fn new(item: Option<i64>, check: BrowserCheck) -> Option<Self> {
        Some(Self {
            item,
            total: check.pages.len(),
            site: check.site,
            home: check.pages.first()?.clone(),
            left: check.pages,
            caught: Vec::new(),
        })
    }

    /// The page shown now, and the window's title for it.
    fn shown(&self) -> Option<(Url, String)> {
        let url = Url::parse(&self.left.first()?.url).ok()?;
        let number = self.total - self.left.len() + 1;
        Some((url, text::title(&self.site, number, self.total)))
    }
}

/// Open Check: shows the download's check in the check window, or brings the window forward if it shows it already.
/// A window showing another download's check moves on to this one, giving that one what it caught. Async, since a
/// window made from a command that runs on the main thread would wait for it forever.
#[tauri::command]
pub async fn open_check_window(
    app: AppHandle,
    queue: State<'_, Arc<DownloadQueue>>,
    id: i64,
) -> Result<(), DownloadError> {
    let check = queue.check_for(id)?;
    open(&app, Some(id), check).map_err(|detail| DownloadError::Unexpected { detail })
}

fn open(app: &AppHandle, item: Option<i64>, check: BrowserCheck) -> Result<(), String> {
    let window = app.get_webview_window(LABEL);
    let state = app.state::<CheckWindow>();
    let mut current = state.lock();
    if let Some(window) = &window
        && current.as_ref().is_some_and(|visit| visit.item == item)
    {
        drop(current);
        bring_forward(window);
        return Ok(());
    }
    let no_page = || "the check has no page to open".to_owned();
    let visit = Visit::new(item, check).ok_or_else(no_page)?;
    let (url, title) = visit.shown().ok_or_else(no_page)?;
    let previous = current.replace(visit);
    drop(current);
    if let Some(previous) = previous {
        hand_over(app, previous);
    }
    match window {
        Some(window) => {
            window.navigate(url).map_err(|error| error.to_string())?;
            let _ = window.set_title(&title);
            bring_forward(&window);
            Ok(())
        }
        None => build(app, url, &title).map_err(|error| {
            state.lock().take();
            error.to_string()
        }),
    }
}

fn build(app: &AppHandle, url: Url, title: &str) -> tauri::Result<()> {
    let profile = DataPaths::new(app.state::<DataFolder>().path.clone()).browser_folder();
    let guard = app.clone();
    let catcher = app.clone();
    let opener = app.clone();
    let builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External(url))
        .title(title)
        .inner_size(WIDTH, HEIGHT)
        .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
        .center()
        .data_directory(profile)
        // The page's file is caught as the page goes to it, so it's never fetched.
        .on_navigation(move |url| !take(&guard, url.as_str()) && stays_on_site(&guard, url))
        // A download that starts anyway stops before a byte is saved, and is caught if it's the page's file.
        .on_download(move |_, event| {
            if let DownloadEvent::Requested { url, .. } = event
                && !take(&catcher, url.as_str())
            {
                development::log(&format!("stopped a download: {url}"));
            }
            false
        })
        .on_permission_request(|_, kind| permission(kind))
        .on_new_window(move |_, features| open_catcher(&opener, features));
    let window = development::with_debugging(builder).build()?;
    let closer = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            closed(&closer);
        }
    });
    Ok(())
}

fn bring_forward(window: &tauri::WebviewWindow) {
    // Best effort: a window that can't be restored or focused is still open.
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

/// Whether the check window may go to `url`: a page on the check's site (`is_web_page`), not an advert's.
fn stays_on_site(app: &AppHandle, url: &Url) -> bool {
    let on_site = url.scheme() == "about"
        || url.host_str().is_some_and(|host| {
            app.state::<CheckWindow>()
                .lock()
                .as_ref()
                .is_none_or(|visit| visit.home.on_site(host))
        });
    if !on_site {
        development::log(&format!("stayed on the site instead of going to {url}"));
    }
    on_site && is_web_page(url)
}

/// A window a page opens: hidden, and never loading anything. Where the page sends it is caught as a download would
/// be (MediaFire's Download button sends the file there), and it closes.
fn open_catcher(app: &AppHandle, features: NewWindowFeatures) -> NewWindowResponse<Wry> {
    let label = format!(
        "{LABEL}-opened-{}",
        OPENED_WINDOWS.fetch_add(1, Ordering::Relaxed)
    );
    let catcher = app.clone();
    let closer = label.clone();
    let built = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(blank_page()))
        .window_features(features)
        .visible(false)
        .on_navigation(move |url| {
            if url.scheme() == "about" {
                return true;
            }
            if !take(&catcher, url.as_str()) {
                development::log(&format!("closed a window opened on {url}"));
            }
            let app = catcher.clone();
            let label = closer.clone();
            thread::spawn(move || {
                if let Some(window) = app.get_webview_window(&label) {
                    let _ = window.destroy();
                }
            });
            false
        })
        .on_download(|_, _| false)
        .on_permission_request(|_, kind| permission(kind))
        .build();
    match built {
        Ok(window) => NewWindowResponse::Create { window },
        Err(error) => {
            development::log(&format!("couldn't make the opened window: {error}"));
            NewWindowResponse::Deny
        }
    }
}

/// What the window's pages may do beyond showing themselves: nothing (no camera, microphone, location, notifications
/// or clipboard), apart from starting downloads one after another, which all stop before a byte is saved.
fn permission(kind: PermissionKind) -> PermissionResponse {
    match kind {
        PermissionKind::AutomaticDownloads => PermissionResponse::Allow,
        _ => PermissionResponse::Deny,
    }
}

fn blank_page() -> Url {
    Url::parse("about:blank").expect("about:blank is an address")
}

/// Whether the window may go to `url`: web pages only, never a local one (the app's own pages, or a server on this
/// PC), so nothing it shows can reach Tsundoku.
fn is_web_page(url: &Url) -> bool {
    match url.scheme() {
        "about" => true,
        "https" | "http" => url.host_str().is_some_and(|host| {
            let host = host.trim_end_matches('.').to_ascii_lowercase();
            !(host == "localhost"
                || host.ends_with(".localhost")
                || host.starts_with("127.")
                || host == "[::1]"
                || host == "0.0.0.0")
        }),
        _ => false,
    }
}

/// Catches `address` if it's the download of a file the check is for (`CheckPage::takes`): the window moves on to the
/// next page, or closes once none is left, giving the download what it caught. Whether it was one; anything else (a
/// page, another file, an advert's download) is left to the caller.
fn take(app: &AppHandle, address: &str) -> bool {
    let state = app.state::<CheckWindow>();
    let mut current = state.lock();
    let Some(visit) = current.as_mut() else {
        return false;
    };
    let Some(index) = visit.left.iter().position(|page| page.takes(address)) else {
        return false;
    };
    development::log(&format!("caught: {address}"));
    visit.left.remove(index);
    visit.caught.push(address.to_owned());
    let next = visit.shown();
    let finished = if next.is_none() { current.take() } else { None };
    drop(current);
    let app = app.clone();
    // Out of the WebView's own event, which mustn't navigate or close its window.
    thread::spawn(move || {
        if let Some(window) = app.get_webview_window(LABEL) {
            match next {
                Some((url, title)) => {
                    let _ = window.navigate(url);
                    let _ = window.set_title(&title);
                }
                None => {
                    let _ = window.close();
                }
            }
        }
        if let Some(visit) = finished {
            hand_over(&app, visit);
        }
    });
    true
}

/// The window closed, by the person or once done: the download gets what was caught, so it downloads those files and
/// waits again for the rest.
fn closed(app: &AppHandle) {
    let visit = app.state::<CheckWindow>().lock().take();
    if let Some(visit) = visit {
        hand_over(app, visit);
    }
}

/// Gives the download the addresses caught for it, if any.
fn hand_over(app: &AppHandle, visit: Visit) {
    if visit.caught.is_empty() {
        return;
    }
    match visit.item {
        // A download removed meanwhile has nothing to give them to.
        Some(id) => {
            let _ = app
                .state::<Arc<DownloadQueue>>()
                .passed_check(id, visit.caught);
        }
        None => development::log(&format!("caught {} addresses", visit.caught.len())),
    }
}

/// The window's title, shown outside the UI, so in Rust: "MediaFire Check", or "MediaFire Check, File 2 of 3".
mod text {
    pub fn title(site: &str, number: usize, total: usize) -> String {
        if total > 1 {
            format!("{site} Check, File {number} of {total}")
        } else {
            format!("{site} Check")
        }
    }
}

/// Development builds only: `TSUNDOKU_CHECK_LINK=<link>` opens the check window on every file of a link at startup,
/// writing what it catches to the console instead of giving it to a download; `TSUNDOKU_CHECK_LINK=<queue ID>` opens
/// it for that download once it waits for a check, as Open Check does; and `TSUNDOKU_CHECK_DEBUG_PORT=<port>` lets
/// DevTools connect to its pages.
#[cfg(debug_assertions)]
mod development {
    use std::sync::Arc;
    use std::time::Duration;

    use tauri::{AppHandle, Manager, Runtime, WebviewWindowBuilder};
    use tsundoku_core::cancel::CancelToken;
    use tsundoku_core::downloads::DownloadQueue;
    use tsundoku_core::hosts::check;

    pub fn open_from_environment(app: &AppHandle) {
        let Ok(link) = std::env::var("TSUNDOKU_CHECK_LINK") else {
            return;
        };
        let app = app.clone();
        std::thread::spawn(move || {
            let opened = match link.parse::<i64>() {
                Ok(id) => (0..60)
                    .find_map(|_| {
                        let check = app.state::<Arc<DownloadQueue>>().check_for(id).ok();
                        if check.is_none() {
                            std::thread::sleep(Duration::from_secs(1));
                        }
                        check
                    })
                    .ok_or_else(|| format!("download {id} doesn't wait for a check"))
                    .and_then(|check| super::open(&app, Some(id), check)),
                Err(_) => check::for_link(&link, &CancelToken::new())
                    .map_err(|error| error.to_string())
                    .and_then(|check| super::open(&app, None, check)),
            };
            if let Err(error) = opened {
                log(&error);
            }
        });
    }

    pub fn with_debugging<'a, R: Runtime, M: tauri::Manager<R>>(
        builder: WebviewWindowBuilder<'a, R, M>,
    ) -> WebviewWindowBuilder<'a, R, M> {
        match std::env::var("TSUNDOKU_CHECK_DEBUG_PORT") {
            // wry's own arguments, which these replace, then the port.
            Ok(port) => builder.additional_browser_args(&format!(
                "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port={port}"
            )),
            Err(_) => builder,
        }
    }

    pub fn log(message: &str) {
        eprintln!("check window: {message}");
    }
}

#[cfg(not(debug_assertions))]
mod development {
    use tauri::{AppHandle, Runtime, WebviewWindowBuilder};

    pub fn open_from_environment(_app: &AppHandle) {}

    pub fn with_debugging<'a, R: Runtime, M: tauri::Manager<R>>(
        builder: WebviewWindowBuilder<'a, R, M>,
    ) -> WebviewWindowBuilder<'a, R, M> {
        builder
    }

    pub fn log(_message: &str) {}
}

pub use development::open_from_environment;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goes_to_web_pages_only() {
        for (url, allowed) in [
            ("https://www.mediafire.com/file/abc123", true),
            ("http://example.com/", true),
            ("about:blank", true),
            ("http://localhost:1420/", false),
            ("http://tauri.localhost/", false),
            ("http://ipc.localhost/open_check_window", false),
            ("http://127.0.0.1:8080/", false),
            ("http://[::1]/", false),
            ("tauri://localhost/", false),
            ("file:///C:/Windows/", false),
            ("javascript:alert(1)", false),
        ] {
            assert_eq!(is_web_page(&Url::parse(url).unwrap()), allowed, "{url}");
        }
    }

    #[test]
    fn titles_say_which_file() {
        assert_eq!(text::title("MediaFire", 1, 1), "MediaFire Check");
        assert_eq!(
            text::title("MediaFire", 2, 3),
            "MediaFire Check, File 2 of 3"
        );
    }
}
