//! The check window: a site's own page in a small browser window of Tsundoku's, for a check only a person can pass
//! (`tsundoku_core::hosts::check`): Cloudflare's in front of any site Tsundoku reads, or a known window site's
//! (`hosts::window_sites`). A download that meets one waits in the queue until the person opens it with Open Check;
//! nothing opens by itself.
//!
//! The window can't reach Tsundoku's commands: its pages are on the web, which Tauri keeps from the app's commands,
//! no capability names it, and it never navigates to a local page. It keeps its own lasting profile in the data
//! folder's `browser` folder, so a check passed once is remembered as a browser remembers it, and nothing of it (its
//! cookies, its sign-ins) leaves the window. Tsundoku never touches the check, and never reads a page's layout: the
//! person does what the page asks, then starts the download there.
//!
//! The window takes the first download a page starts from the link's site (its own address on the site), from the
//! page, one of its frames, or a window it opened. It holds that download (WebView2's deferral) and offers it to the
//! queue (`DownloadQueue::offer_window_download`), whose run asks the address itself, as Tsundoku: when the answer is
//! strictly the file, the window stops its own download and the row downloads it with its progress; otherwise the
//! window saves the file into the download's temporary folder, hides, and the row shows that download's progress
//! until the download moves the file into place. Closing the window, or stopping or removing the download, stops it
//! and leaves nothing behind. With several files behind the check, it shows each one's page in turn.
//!
//! The page stays on the site: an advert can't take the window elsewhere. A window the page opens is a hidden one
//! that never loads anything: an address on the site it's sent to is opened in the check window itself (MediaFire's
//! Download button may send the file there), and one elsewhere closes it, so a download it would start never counts.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use tauri::webview::{NewWindowFeatures, NewWindowResponse, PermissionKind, PermissionResponse};
use tauri::{AppHandle, Manager, State, Url, WebviewUrl, WebviewWindowBuilder, WindowEvent, Wry};
use tsundoku_core::data_folder::DataPaths;
use tsundoku_core::downloads::{DownloadError, DownloadQueue, WindowAnswer, WindowOffer};
use tsundoku_core::hosts::check::{BrowserCheck, CheckPage};

use crate::DataFolder;

#[cfg(windows)]
mod watch;

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

/// The pages of one download's check.
struct Visit {
    /// The download it's for, or `None` for a development build's own (`open_from_environment`).
    item: Option<i64>,
    site: String,
    /// The first page, for its site, which the window doesn't leave and whose downloads alone count.
    home: CheckPage,
    /// The pages still to get past, the one shown first.
    left: Vec<CheckPage>,
    /// How many pages it had.
    total: usize,
    /// The page whose download the window holds while the queue decides.
    offering: Option<CheckPage>,
    /// The page whose file the window is saving itself, and where.
    saving: Option<(CheckPage, PathBuf)>,
}

impl Visit {
    fn new(item: Option<i64>, check: BrowserCheck) -> Option<Self> {
        Some(Self {
            item,
            total: check.pages.len(),
            site: check.site,
            home: check.pages.first()?.clone(),
            left: check.pages,
            offering: None,
            saving: None,
        })
    }

    /// The page shown now, and the window's title for it.
    fn shown(&self) -> Option<(Url, String)> {
        let url = Url::parse(&self.left.first()?.url).ok()?;
        let number = self.total - self.left.len() + 1;
        Some((url, text::title(&self.site, number, self.total)))
    }

    /// Whether a download from `address` is the link's site's own: a download from anywhere else (an advert's) never
    /// counts.
    fn counts(&self, address: &str) -> bool {
        Url::parse(address).is_ok_and(|url| {
            matches!(url.scheme(), "https" | "http")
                && (development::is_test_page(&url)
                    || url.host_str().is_some_and(|host| self.home.on_site(host)))
        })
    }
}

/// Open Check: shows the download's check in the check window, or brings the window forward if it shows it already.
/// A window showing another download's check moves on to this one. Async, since a window made from a command that
/// runs on the main thread would wait for it forever.
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
    if current
        .as_ref()
        .is_some_and(|visit| visit.saving.is_some() || visit.offering.is_some())
    {
        return Err("the check window is still busy with another download".to_owned());
    }
    let no_page = || "the check has no page to open".to_owned();
    let visit = Visit::new(item, check).ok_or_else(no_page)?;
    let (url, title) = visit.shown().ok_or_else(no_page)?;
    current.replace(visit);
    drop(current);
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
        .on_navigation(move |url| {
            note_address(&guard, "went to", url.as_str());
            stays_on_site(&guard, url)
        })
        // Its downloads go through WebView2's own download operation (`watch`), not a hook of Tauri's.
        .on_permission_request(|_, kind| permission(kind))
        .on_new_window(move |_, features| open_catcher(&opener, features));
    let window = development::with_debugging(builder).build()?;
    #[cfg(windows)]
    watch::watch(&window, downloads(&catcher))?;
    #[cfg(not(windows))]
    let _ = catcher;
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

/// What the window does with a download its page starts: one from the link's site, while nothing else is offered or
/// saved, is held and offered to its download (or saved, for a development build's own page); anything else is stopped
/// before a byte is saved.
#[cfg(windows)]
fn downloads(app: &AppHandle) -> watch::Callbacks {
    let decider = app.clone();
    let teller = app.clone();
    let ender = app.clone();
    watch::Callbacks {
        decide: Box::new(move |started| {
            note_address(&decider, "started a download from", &started.address);
            decide(&decider, started)
        }),
        progress: Box::new(move |received, total| {
            window_download_progress(&teller, received, total)
        }),
        finished: Box::new(move |success| window_download_finished(&ender, success)),
    }
}

#[cfg(windows)]
fn decide(app: &AppHandle, started: &watch::Started) -> watch::Decision {
    let state = app.state::<CheckWindow>();
    let mut current = state.lock();
    let Some(visit) = current.as_mut() else {
        return watch::Decision::Refuse;
    };
    if !visit.counts(&started.address) {
        development::log(&format!(
            "stopped a download from another site: {}",
            started.address
        ));
        return watch::Decision::Refuse;
    }
    if visit.offering.is_some() || visit.saving.is_some() {
        development::log("stopped a second download");
        return watch::Decision::Refuse;
    }
    let Some(page) = visit.left.first().cloned() else {
        return watch::Decision::Refuse;
    };
    let offer = WindowOffer {
        address: started.address.clone(),
        name: started.name.clone(),
        total: started.total,
    };
    match visit.item {
        Some(id) => {
            visit.offering = Some(page.clone());
            drop(current);
            let app = app.clone();
            watch::Decision::Hold(Box::new(move |held| {
                let replier = app.clone();
                let file_id = page.file_id().to_owned();
                app.state::<Arc<DownloadQueue>>().offer_window_download(
                    id,
                    &file_id,
                    offer,
                    Box::new(move |answer| answered(&replier, id, &page, held, answer)),
                );
            }))
        }
        // A development build's own page: saved in its test folder.
        None => match development::test_folder() {
            Some(folder) => {
                let path = folder.join(tsundoku_core::rules::safe_folder_name(
                    started.name.as_deref().unwrap_or("download"),
                    "download",
                ));
                let _ = std::fs::remove_file(&path);
                visit.saving = Some((page, path.clone()));
                drop(current);
                hide_window(app);
                watch::Decision::Save(path)
            }
            None => watch::Decision::Refuse,
        },
    }
}

/// The queue answered for a download the window holds (`WindowAnswer`): the window stops its own and moves on, saves
/// the file and hides, or stops it.
#[cfg(windows)]
fn answered(app: &AppHandle, id: i64, page: &CheckPage, held: u64, answer: WindowAnswer) {
    let state = app.state::<CheckWindow>();
    let mut current = state.lock();
    let visit = current.as_mut().filter(|visit| visit.item == Some(id));
    match answer {
        WindowAnswer::Fetching => {
            development::log("Tsundoku fetches the file itself");
            watch::answer(app, held, watch::Answer::Stop);
            if let Some(visit) = visit {
                visit.offering = None;
                visit.left.retain(|left| left.file_id() != page.file_id());
                move_on(app, current);
            }
        }
        WindowAnswer::SaveIn(path) => match visit {
            Some(visit) => {
                development::log(&format!("saving to {}", path.display()));
                visit.offering = None;
                visit.saving = Some((page.clone(), path.clone()));
                drop(current);
                watch::answer(app, held, watch::Answer::Save(path));
                hide_window(app);
                let app = app.clone();
                thread::spawn(move || watch_window_download(&app, id));
            }
            // The window closed meanwhile, and its download with it: the download waits for its check again.
            None => {
                drop(current);
                watch::answer(app, held, watch::Answer::Stop);
                app.state::<Arc<DownloadQueue>>()
                    .window_download_withdrawn(id);
            }
        },
        WindowAnswer::Refuse => {
            development::log("the download no longer wants it: stopped");
            watch::answer(app, held, watch::Answer::Stop);
            if let Some(visit) = visit {
                visit.offering = None;
            }
            drop(current);
            // Stopped or removed meanwhile: its check has nothing more to do.
            if app.state::<Arc<DownloadQueue>>().check_for(id).is_err() {
                let app = app.clone();
                thread::spawn(move || {
                    if let Some(window) = app.get_webview_window(LABEL) {
                        let _ = window.close();
                    }
                });
            }
        }
    }
}

/// Hides the window while it saves a file: the download's row shows the progress.
fn hide_window(app: &AppHandle) {
    let app = app.clone();
    // Out of the WebView's own event, which mustn't hide its window.
    thread::spawn(move || {
        if let Some(window) = app.get_webview_window(LABEL) {
            let _ = window.hide();
        }
    });
}

/// Tells the download's row how far the window's download has got.
fn window_download_progress(app: &AppHandle, received: u64, total: Option<u64>) {
    let item = app
        .state::<CheckWindow>()
        .lock()
        .as_ref()
        .filter(|visit| visit.saving.is_some())
        .map(|visit| visit.item);
    match item {
        Some(Some(id)) => app
            .state::<Arc<DownloadQueue>>()
            .window_download_progress(id, received, total),
        Some(None) => development::log(&format!("received {received} of {total:?} bytes")),
        None => {}
    }
}

/// Whether the check window may go to `url`: a page on the check's site (`is_web_page`), not an advert's.
fn stays_on_site(app: &AppHandle, url: &Url) -> bool {
    if development::is_test_page(url) {
        return true;
    }
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

/// A window a page opens: hidden, and never loading anything. An address on the site it's sent to opens in the check
/// window itself, where a download it starts is taken as any other (MediaFire's Download button may send the file
/// there); anywhere else, and it closes. Either way, it closes, and no download of its own ever counts.
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
            note_address(&catcher, "a window it opened went to", url.as_str());
            let on_site = catcher
                .state::<CheckWindow>()
                .lock()
                .as_ref()
                .is_some_and(|visit| visit.counts(url.as_str()));
            let app = catcher.clone();
            let label = closer.clone();
            let url = url.clone();
            // Out of the WebView's own event, which mustn't navigate or close a window.
            thread::spawn(move || {
                if let Some(window) = app.get_webview_window(&label) {
                    let _ = window.destroy();
                }
                if on_site && let Some(window) = app.get_webview_window(LABEL) {
                    development::log(&format!("opened {url} in the check window"));
                    let _ = window.navigate(url);
                } else {
                    development::log(&format!("closed a window opened on {url}"));
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
/// or clipboard), apart from starting downloads one after another, each of which the window decides on.
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

/// After a page's file was fetched or saved: the window shows the next page, or closes once none is left.
fn move_on(app: &AppHandle, mut current: MutexGuard<'_, Option<Visit>>) {
    let Some(visit) = current.as_mut() else {
        return;
    };
    let next = visit.shown();
    drop(current);
    let app = app.clone();
    // Out of the WebView's own event, which mustn't navigate or close its window.
    thread::spawn(move || {
        if let Some(window) = app.get_webview_window(LABEL) {
            match next {
                Some((url, title)) => {
                    let _ = window.navigate(url);
                    let _ = window.set_title(&title);
                    bring_forward(&window);
                }
                None => {
                    let _ = window.close();
                }
            }
        }
    });
}

/// While the window saves a download's file: closes the window, which stops the download, once the download no longer
/// waits for it (stopped, removed).
fn watch_window_download(app: &AppHandle, id: i64) {
    let saving = || {
        app.state::<CheckWindow>()
            .lock()
            .as_ref()
            .is_some_and(|visit| visit.item == Some(id) && visit.saving.is_some())
    };
    loop {
        thread::sleep(Duration::from_secs(1));
        if !saving() {
            return;
        }
        // Its end hands the file over first, so a download no longer waiting with the window still saving was stopped.
        if !app.state::<Arc<DownloadQueue>>().waits_for_window(id) && saving() {
            development::log("the download was stopped: stopping the window's");
            #[cfg(windows)]
            watch::cancel(app);
            // Its end closes the window; this is in case it never comes.
            thread::sleep(Duration::from_secs(2));
            if let Some(window) = app.get_webview_window(LABEL) {
                let _ = window.destroy();
            }
            return;
        }
    }
}

/// The window's own download ended: the download gets the file at once, or fails and loses what was left.
fn window_download_finished(app: &AppHandle, success: bool) {
    let state = app.state::<CheckWindow>();
    let mut current = state.lock();
    let Some(visit) = current.as_mut() else {
        return;
    };
    let Some((page, saved)) = visit.saving.take() else {
        return;
    };
    let item = visit.item;
    if success && saved.is_file() {
        development::log(&format!("saved {}", saved.display()));
        let link = page.saved_link(&saved);
        visit.left.retain(|left| !left.takes(&link));
        move_on(app, current);
        if let Some(id) = item {
            // A download removed meanwhile has nothing to give it to.
            let _ = app
                .state::<Arc<DownloadQueue>>()
                .passed_check(id, vec![link]);
        }
        return;
    }
    development::log("the window's download stopped");
    current.take();
    drop(current);
    match item {
        Some(id) => app
            .state::<Arc<DownloadQueue>>()
            .window_download_stopped(id),
        None => {
            let _ = std::fs::remove_file(&saved);
        }
    }
    let app = app.clone();
    thread::spawn(move || {
        if let Some(window) = app.get_webview_window(LABEL) {
            let _ = window.destroy();
        }
    });
}

/// The window closed, by the person or once done: a download it held or was saving stops with it.
fn closed(app: &AppHandle) {
    let visit = app.state::<CheckWindow>().lock().take();
    let Some(visit) = visit else {
        return;
    };
    let Some(id) = visit.item else {
        return;
    };
    let queue = app.state::<Arc<DownloadQueue>>();
    if visit.offering.is_some() {
        queue.withdraw_window_offer(id);
    }
    if visit.saving.is_some() {
        queue.window_download_stopped(id);
    }
}

/// Writes an address the window or a window its page opened went to (or started a download from) in the log of the
/// download the window is for, so a check shows where the page went once the person clicks Download. Addresses only,
/// never what a page holds.
fn note_address(app: &AppHandle, what: &str, address: &str) {
    development::log(&format!("{what} {address}"));
    let item = app
        .state::<CheckWindow>()
        .lock()
        .as_ref()
        .and_then(|visit| visit.item);
    if let Some(id) = item {
        let path = app.state::<Arc<DownloadQueue>>().log_path(id);
        // Best effort: a log that can't be written loses the line.
        if let Ok(mut log) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(log, "check window: {what} {address}");
        }
    }
}

/// The window's title, shown outside the UI, so in Rust: "MediaFire Check", or "MediaFire Check, File 2 of 3", with
/// a hint, since a site's first click on Download may go to an advert, which the window closes.
mod text {
    pub fn title(site: &str, number: usize, total: usize) -> String {
        let title = if total > 1 {
            format!("{site} Check, File {number} of {total}")
        } else {
            format!("{site} Check")
        };
        format!("{title} · If Nothing Happens, Click Download Again")
    }
}

/// Development builds only: `TSUNDOKU_CHECK_LINK=<link>` opens the check window on every file of a link at startup,
/// writing what happens to the console; `TSUNDOKU_CHECK_LINK=<queue ID>` opens it for that download once it waits
/// for a check, as Open Check does; and `TSUNDOKU_CHECK_DEBUG_PORT=<port>` lets DevTools connect to its pages.
/// `TSUNDOKU_CHECK_TEST_PAGE=<address>` opens it on a page of the developer's own (on this PC, which the window
/// otherwise refuses): with `TSUNDOKU_CHECK_TEST_ITEM=<queue ID>`, as that waiting download's page, whose downloads
/// are offered to the download as a site's would be; without, saving whatever download the page starts in
/// `TSUNDOKU_CHECK_TEST_FOLDER` and writing its progress to the console.
#[cfg(debug_assertions)]
mod development {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use tauri::{AppHandle, Manager, Runtime, Url, WebviewWindowBuilder};
    use tsundoku_core::cancel::CancelToken;
    use tsundoku_core::downloads::DownloadQueue;
    use tsundoku_core::hosts::check::{self, BrowserCheck, CheckPage};

    pub fn open_from_environment(app: &AppHandle) {
        if let Ok(page) = std::env::var("TSUNDOKU_CHECK_TEST_PAGE") {
            let item = std::env::var("TSUNDOKU_CHECK_TEST_ITEM")
                .ok()
                .and_then(|id| id.parse::<i64>().ok());
            let app = app.clone();
            std::thread::spawn(move || {
                let opened = match item {
                    // A waiting download's own check, its page moved to the test page.
                    Some(id) => wait_for_check(&app, id).and_then(|check| {
                        let moved = BrowserCheck {
                            site: check.site,
                            pages: check
                                .pages
                                .iter()
                                .map(|first| first.moved_to(&page))
                                .collect(),
                        };
                        super::open(&app, Some(id), moved)
                    }),
                    None => CheckPage::of_window_link("https://workupload.com/file/testpage")
                        .ok_or_else(|| "no test page".to_owned())
                        .and_then(|first| {
                            let moved = BrowserCheck {
                                site: "Test".to_owned(),
                                pages: vec![first.moved_to(&page)],
                            };
                            super::open(&app, None, moved)
                        }),
                };
                if let Err(error) = opened {
                    log(&error);
                }
            });
            return;
        }
        let Ok(link) = std::env::var("TSUNDOKU_CHECK_LINK") else {
            return;
        };
        let app = app.clone();
        std::thread::spawn(move || {
            let opened = match link.parse::<i64>() {
                Ok(id) => {
                    wait_for_check(&app, id).and_then(|check| super::open(&app, Some(id), check))
                }
                Err(_) => check::for_link(&link, &CancelToken::new())
                    .map_err(|error| error.to_string())
                    .and_then(|check| super::open(&app, None, check)),
            };
            if let Err(error) = opened {
                log(&error);
            }
        });
    }

    /// The check download `id` waits for, once it does, within a minute.
    fn wait_for_check(app: &AppHandle, id: i64) -> Result<BrowserCheck, String> {
        (0..60)
            .find_map(|_| {
                let check = app.state::<Arc<DownloadQueue>>().check_for(id).ok();
                if check.is_none() {
                    std::thread::sleep(Duration::from_secs(1));
                }
                check
            })
            .ok_or_else(|| format!("download {id} doesn't wait for a check"))
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

    /// Whether `url` is on the host of the developer's own test page (`TSUNDOKU_CHECK_TEST_PAGE`).
    pub fn is_test_page(url: &Url) -> bool {
        std::env::var("TSUNDOKU_CHECK_TEST_PAGE")
            .ok()
            .and_then(|page| Url::parse(&page).ok())
            .is_some_and(|page| {
                page.host_str().is_some()
                    && page.host_str() == url.host_str()
                    && page.port_or_known_default() == url.port_or_known_default()
            })
    }

    /// Where the window saves the test page's download (`TSUNDOKU_CHECK_TEST_FOLDER`).
    pub fn test_folder() -> Option<PathBuf> {
        let folder = PathBuf::from(std::env::var("TSUNDOKU_CHECK_TEST_FOLDER").ok()?);
        std::fs::create_dir_all(&folder).ok()?;
        Some(folder)
    }
}

#[cfg(not(debug_assertions))]
mod development {
    use std::path::PathBuf;

    use tauri::{AppHandle, Runtime, Url, WebviewWindowBuilder};

    pub fn open_from_environment(_app: &AppHandle) {}

    pub fn is_test_page(_url: &Url) -> bool {
        false
    }

    pub fn test_folder() -> Option<PathBuf> {
        None
    }

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
        assert_eq!(
            text::title("MediaFire", 1, 1),
            "MediaFire Check · If Nothing Happens, Click Download Again"
        );
        assert_eq!(
            text::title("MediaFire", 2, 3),
            "MediaFire Check, File 2 of 3 · If Nothing Happens, Click Download Again"
        );
    }
}
