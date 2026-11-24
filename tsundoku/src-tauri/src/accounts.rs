//! Settings → Sign-Ins: Tsundoku's own sign-in to Pixiv (`tsundoku_core::pixiv_account`). Sign In opens Pixiv's own
//! login page in a window of Tsundoku's (label `pixiv-sign-in`), with a lasting browser profile of its own
//! (`DataPaths::accounts_browser_folder`), no capability, and only Pixiv's pages and the sign-in services its login
//! offers. The person types and clicks there; Tsundoku never does. When the login ends, the window's navigation to
//! Pixiv's callback carries a one-time code (`pixiv_account::code_in`), which is taken, never loaded, and turned into
//! the token on a thread of its own; the window then closes and `pixiv-account-changed` tells the UI, or
//! `pixiv-sign-in-failed` says why it didn't work.

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use tauri::webview::{NewWindowResponse, PermissionKind, PermissionResponse};
use tauri::{AppHandle, Emitter, Manager, State, Url, WebviewUrl, WebviewWindowBuilder};
use tsundoku_core::data_folder::DataPaths;
use tsundoku_core::fanbox_account::{self, FanboxSignIn};
use tsundoku_core::pixiv_account::{self, AccountError, PixivAccount, PixivLogin};
use tsundoku_core::site_limits::now_ms;

use crate::DataFolder;
use crate::web_reader::{KeptReader, PageReader};

const LABEL: &str = "pixiv-sign-in";
const WIDTH: f64 = 520.0;
const HEIGHT: f64 = 760.0;
/// Pixiv's own domains, which its login moves between.
const PIXIV_DOMAINS: [&str; 2] = ["pixiv.net", "pixiv.co.jp"];
/// The sign-in services Pixiv's login offers besides its own (Google, Apple, X, Facebook), with the pages their
/// sign-ins go through.
const SIGN_IN_SERVICES: [&str; 9] = [
    "accounts.google.com",
    "accounts.youtube.com",
    "appleid.apple.com",
    "idmsa.apple.com",
    "x.com",
    "twitter.com",
    "api.twitter.com",
    "facebook.com",
    "m.facebook.com",
];
const ACCOUNT_CHANGED_EVENT: &str = "pixiv-account-changed";
const SIGN_IN_FAILED_EVENT: &str = "pixiv-sign-in-failed";

/// The window's title.
const TITLE: &str = "Sign In to Pixiv";

const FANBOX_LABEL: &str = "fanbox-sign-in";
const FANBOX_TITLE: &str = "Sign In to Fanbox";
/// Fanbox's sign-in, which goes through Pixiv's and comes back to Fanbox's home page.
const FANBOX_LOGIN: &str = "https://www.fanbox.cc/login?return_to=https%3A%2F%2Fwww.fanbox.cc%2F";
const FANBOX_DOMAIN: &str = "fanbox.cc";
const FANBOX_HOME: &str = "www.fanbox.cc";
/// A page of Fanbox's own to ask its API from, and the request that tells whether the session is signed in (401
/// signed out, the creators supported otherwise).
const FANBOX_PAGE: &str = "https://www.fanbox.cc/robots.txt";
const FANBOX_SIGNED_IN_CHECK: &str = "https://api.fanbox.cc/post.listSupporting?limit=1";
const FANBOX_CHECK_LABEL: &str = "fanbox-sign-in-check";
const FANBOX_READER_LABEL: &str = "fanbox-reader";
/// Fanbox's file server, whose session cookie its supported posts' files need.
const FANBOX_FILES: &str = "https://downloads.fanbox.cc/";
/// The pause between two of Fanbox's requests, at random within it: Fanbox, like Pixiv, flags requests made with no
/// pause between them.
const FANBOX_PAUSE_MS: (u64, u64) = (1000, 2000);
const FANBOX_CHANGED_EVENT: &str = "fanbox-sign-in-changed";
const FANBOX_FAILED_EVENT: &str = "fanbox-sign-in-failed";

/// The pixiv account Tsundoku is signed in to, if any.
#[tauri::command]
pub fn pixiv_account() -> Option<PixivAccount> {
    pixiv_account::account()
}

/// Opens the sign-in window on Pixiv's login page, or brings it forward when it's open. `async`, as making a window
/// from a command that runs on the main thread deadlocks on Windows: the window stays on a blank page.
#[tauri::command]
pub async fn sign_in_to_pixiv(
    app: AppHandle,
    data_folder: State<'_, DataFolder>,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.unminimize();
        let _ = window.set_focus();
        return Ok(());
    }
    let login = PixivLogin::start().map_err(|error| error.to_string())?;
    let url = Url::parse(&login.url()).map_err(|error| error.to_string())?;
    let profile = DataPaths::new(data_folder.path.clone()).accounts_browser_folder();
    let login = Arc::new(login);
    // The login's end is taken once, even if Pixiv sends the page on to its app's own link too.
    let taken = Arc::new(AtomicBool::new(false));
    let finisher = app.clone();
    let builder = WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::External(url))
        .title(TITLE)
        .inner_size(WIDTH, HEIGHT)
        .center()
        .data_directory(profile)
        .on_navigation(move |url| {
            if let Some(code) = pixiv_account::code_in(url.as_str()) {
                if !taken.swap(true, Ordering::SeqCst) {
                    finish(finisher.clone(), Arc::clone(&login), code);
                }
                return false;
            }
            is_login_page(url)
        })
        .on_permission_request(|_, kind| permission(kind))
        // Pixiv's login opens nothing in a window of its own; a page that tries is refused.
        .on_new_window(|_, _| NewWindowResponse::Deny);
    builder.build().map_err(|error| error.to_string())?;
    Ok(())
}

/// Forgets the sign-in, and the window's own (its browser profile), so signing in again shows Pixiv's login page.
#[tauri::command]
pub async fn sign_out_of_pixiv(
    app: AppHandle,
    data_folder: State<'_, DataFolder>,
) -> Result<(), AccountError> {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.destroy();
    }
    if let Some(window) = app.get_webview_window(FANBOX_LABEL) {
        let _ = window.destroy();
    }
    pixiv_account::forget()?;
    // Fanbox's sign-in lives in the same profile.
    fanbox_account::forget();
    // Best effort: a profile still in use (a window just closed) stays, and Pixiv then signs in again at once.
    let _ = fs::remove_dir_all(DataPaths::new(data_folder.path.clone()).accounts_browser_folder());
    let _ = app.emit(ACCOUNT_CHANGED_EVENT, None::<PixivAccount>);
    let _ = app.emit(FANBOX_CHANGED_EVENT, None::<FanboxSignIn>);
    Ok(())
}

/// Whether the person signed in to Fanbox in Tsundoku's window.
#[tauri::command]
pub fn fanbox_sign_in() -> Option<FanboxSignIn> {
    fanbox_account::signed_in()
}

/// Opens Fanbox's sign-in in Tsundoku's window, on the profile Pixiv's sign-in uses: Fanbox signs in with the pixiv
/// account, and the person continues there once. When the window comes back to Fanbox's home page, Fanbox is asked
/// whether the session is signed in, and the window closes. `async` for the reason `sign_in_to_pixiv` is.
#[tauri::command]
pub async fn sign_in_to_fanbox(
    app: AppHandle,
    data_folder: State<'_, DataFolder>,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(FANBOX_LABEL) {
        let _ = window.unminimize();
        let _ = window.set_focus();
        return Ok(());
    }
    let url = Url::parse(FANBOX_LOGIN).map_err(|error| error.to_string())?;
    let profile = DataPaths::new(data_folder.path.clone()).accounts_browser_folder();
    let taken = Arc::new(AtomicBool::new(false));
    let finisher = app.clone();
    let reader_profile = profile.clone();
    WebviewWindowBuilder::new(&app, FANBOX_LABEL, WebviewUrl::External(url))
        .title(FANBOX_TITLE)
        .inner_size(WIDTH, HEIGHT)
        .center()
        .data_directory(profile)
        .on_navigation(move |url| {
            if is_fanbox_home(url) && !taken.swap(true, Ordering::SeqCst) {
                finish_fanbox(finisher.clone(), reader_profile.clone());
            }
            is_login_page(url) || on_fanbox(url)
        })
        .on_permission_request(|_, kind| permission(kind))
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .build()
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Asks Fanbox, from the window's session, whether it's signed in, keeps the answer, closes the window and tells the
/// UI, on a thread of its own.
fn finish_fanbox(app: AppHandle, profile: std::path::PathBuf) {
    thread::spawn(move || {
        if let Some(window) = app.get_webview_window(FANBOX_LABEL) {
            let _ = window.close();
        }
        let reader = PageReader {
            app: app.clone(),
            profile,
            label: FANBOX_CHECK_LABEL,
            page: FANBOX_PAGE,
        };
        let answer = reader
            .read(
                &[FANBOX_SIGNED_IN_CHECK.to_owned()],
                std::time::Duration::ZERO,
            )
            .and_then(|mut answers| {
                answers
                    .pop()
                    .ok_or_else(|| "Fanbox didn't answer".to_owned())
            })
            .and_then(|answer| answer);
        let outcome = match answer.map(|answer| fanbox_account::answer_signed_in(answer.status)) {
            Ok(Some(true)) => fanbox_account::keep(now_ms()),
            Ok(Some(false)) => Err("Fanbox didn't count the window as signed in".to_owned()),
            Ok(None) => Err("Fanbox didn't say whether it's signed in".to_owned()),
            Err(error) => Err(error),
        };
        let _ = match outcome {
            Ok(sign_in) => app.emit(FANBOX_CHANGED_EVENT, Some(sign_in)),
            Err(error) => app.emit(FANBOX_FAILED_EVENT, error),
        };
    });
}

/// Gives the core the reader Fanbox's API is asked through: the sign-in window's session, in a hidden window kept open
/// between reads.
pub fn start_fanbox(app: &AppHandle, profile: std::path::PathBuf) {
    let kept = KeptReader::start(
        PageReader {
            app: app.clone(),
            profile,
            label: FANBOX_READER_LABEL,
            page: FANBOX_PAGE,
        },
        FANBOX_PAUSE_MS,
    );
    tsundoku_core::hosts::fanbox::start(Box::new(FanboxReader(kept)));
}

/// Fanbox's API, asked one read at a time from a window kept open between reads (`KeptReader`).
struct FanboxReader(Arc<KeptReader>);

impl tsundoku_core::hosts::fanbox::FanboxApi for FanboxReader {
    fn get(
        &self,
        addresses: &[String],
    ) -> Result<Vec<Result<tsundoku_core::hosts::fanbox::ApiAnswer, String>>, String> {
        Ok(self
            .0
            .get(addresses)?
            .into_iter()
            .map(|answer| answer.map(crate::web_reader::api_answer))
            .collect())
    }

    /// The session's cookie for Fanbox's file server.
    fn session(&self) -> Result<Option<String>, String> {
        self.0
            .cookie(FANBOX_FILES, tsundoku_core::hosts::fanbox::SESSION_COOKIE)
    }
}

/// Fanbox's home page, where its sign-in ends: www.fanbox.cc, but not its login or the sign-in's own pages.
fn is_fanbox_home(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some(FANBOX_HOME)
        && !url.path().starts_with("/login")
        && !url.path().starts_with("/auth")
}

fn on_fanbox(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host_str().is_some_and(|host| {
            host == FANBOX_DOMAIN || host.ends_with(&format!(".{FANBOX_DOMAIN}"))
        })
}

/// Turns the code into the token on a thread of its own, keeps it, closes the window and tells the UI.
fn finish(app: AppHandle, login: Arc<PixivLogin>, code: String) {
    thread::spawn(move || {
        let signed_in = login.exchange(&code).and_then(pixiv_account::keep);
        if let Some(window) = app.get_webview_window(LABEL) {
            let _ = window.close();
        }
        let _ = match signed_in {
            Ok(account) => app.emit(ACCOUNT_CHANGED_EVENT, Some(account)),
            Err(error) => app.emit(SIGN_IN_FAILED_EVENT, error),
        };
    });
}

/// Whether the window may go to a page: one of Pixiv's, or of a sign-in service its login offers, over https.
fn is_login_page(url: &Url) -> bool {
    if url.as_str() == "about:blank" {
        return true;
    }
    let Some(host) = url.host_str().filter(|_| url.scheme() == "https") else {
        return false;
    };
    let on = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    PIXIV_DOMAINS.into_iter().any(on) || SIGN_IN_SERVICES.into_iter().any(on)
}

/// Nothing beyond showing the page: no camera, microphone, location, notifications or clipboard.
fn permission(_kind: PermissionKind) -> PermissionResponse {
    PermissionResponse::Deny
}

/// Reads the sign-in kept in the data folder, and deletes config files a run left when the app stopped mid-run.
pub fn open(paths: &DataPaths) {
    let folder = paths.accounts_folder();
    pixiv_account::remove_left_configs(&folder);
    pixiv_account::open(&folder);
    fanbox_account::open(&folder);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_on_pixiv_and_the_sign_in_services() {
        let page = |address: &str| is_login_page(&Url::parse(address).unwrap());
        assert!(page("https://accounts.pixiv.net/login?return_to=x"));
        assert!(page(
            "https://app-api.pixiv.net/web/v1/login?code_challenge=x"
        ));
        assert!(page("https://accounts.google.com/o/oauth2/auth"));
        assert!(!page("http://accounts.pixiv.net/login"), "only https");
        assert!(!page("https://pixiv.net.example.com/"));
        assert!(!page("https://example.com/"));
    }

    #[test]
    fn ends_fanboxs_sign_in_on_its_home_page() {
        let home = |address: &str| is_fanbox_home(&Url::parse(address).unwrap());
        assert!(home("https://www.fanbox.cc/"));
        assert!(home("https://www.fanbox.cc/home"));
        assert!(!home("https://www.fanbox.cc/login?return_to=x"));
        assert!(!home("https://www.fanbox.cc/auth/start?code=x"));
        assert!(!home("https://accounts.pixiv.net/login"));
        let fanbox = |address: &str| on_fanbox(&Url::parse(address).unwrap());
        assert!(fanbox("https://api.fanbox.cc/x") && fanbox("https://someone.fanbox.cc/"));
        assert!(!fanbox("https://fanbox.cc.example.com/"));
    }
}
