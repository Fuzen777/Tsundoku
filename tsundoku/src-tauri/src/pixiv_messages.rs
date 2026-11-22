//! Pixiv's warnings (`tsundoku_core::pixiv_warning`): the reader that asks Pixiv for the account's latest message
//! threads, and what the app does when one is a warning. Pixiv's messages need its website's sign-in, which the
//! sign-in window's profile (`DataPaths::accounts_browser_folder`) holds since the person signed in there. The reader
//! opens a hidden window (label `pixiv-messages`) on that profile at `READER_PAGE`, a small page of Pixiv's own, asks
//! for `THREADS_ADDRESS` from it as the website's own script does (WebView2's DevTools call `Runtime.evaluate`,
//! awaiting the answer), then closes it. The window has no capability, stays on www.pixiv.net, opens nothing else,
//! downloads nothing and is never shown.
//!
//! When a check finds a warning, the Pixiv downloads running are stopped (and resumed once the person goes on), the
//! UI is told (`pixiv-warning-changed`), and a notification says so while the window isn't in front.

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tauri::webview::{NewWindowResponse, PageLoadEvent, PermissionResponse};
use tauri::{AppHandle, Emitter, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tsundoku_core::downloads::{DownloadQueue, QueueState};
use tsundoku_core::pixiv_warning::{
    self, MessageReader, MessagesAnswer, PixivWarning, PixivWarningStatus, READER_PAGE,
    THREADS_ADDRESS, WarningListener,
};

use crate::background;

const LABEL: &str = "pixiv-messages";
const CHANGED_EVENT: &str = "pixiv-warning-changed";
/// Pixiv's own site, the only one the window goes to.
const PIXIV_HOST: &str = "www.pixiv.net";
/// How long the page may take to load, and the threads to come.
const LOAD_WAIT: Duration = Duration::from_secs(30);
const ANSWER_WAIT: Duration = Duration::from_secs(30);
/// How long the script waits for Pixiv's answer, under `ANSWER_WAIT`.
const FETCH_TIMEOUT_MS: u32 = 20_000;

/// Starts the warning check, once the queue is in Tauri's state: `accounts` is the data folder's accounts folder,
/// `profile` the sign-in window's.
pub fn start(app: &AppHandle, accounts: PathBuf, profile: PathBuf) {
    pixiv_warning::start(
        &accounts,
        Box::new(WindowReader {
            app: app.clone(),
            profile,
        }),
        Box::new(Listener {
            app: app.clone(),
            stopped: Mutex::new(Vec::new()),
        }),
    );
}

/// The warning check's status.
#[tauri::command]
pub fn pixiv_warning_status() -> PixivWarningStatus {
    pixiv_warning::status()
}

/// Reads Pixiv's messages now (Settings → Sign-Ins' Check Now).
#[tauri::command]
pub async fn check_pixiv_messages() -> PixivWarningStatus {
    tauri::async_runtime::spawn_blocking(pixiv_warning::check)
        .await
        .unwrap_or_else(|_| pixiv_warning::status())
}

/// The person goes on past Pixiv's warning: its downloads and reads go on.
#[tauri::command]
pub async fn dismiss_pixiv_warning() -> PixivWarningStatus {
    tauri::async_runtime::spawn_blocking(pixiv_warning::dismiss)
        .await
        .unwrap_or_else(|_| pixiv_warning::status())
}

struct WindowReader {
    app: AppHandle,
    profile: PathBuf,
}

impl MessageReader for WindowReader {
    fn read(&self) -> Result<MessagesAnswer, String> {
        // Development builds can stand a file in for Pixiv's answer, to try what a warning does.
        #[cfg(debug_assertions)]
        if let Ok(path) = std::env::var("TSUNDOKU_PIXIV_MESSAGES") {
            return std::fs::read_to_string(path)
                .map(|body| MessagesAnswer { status: 200, body })
                .map_err(|error| error.to_string());
        }
        if let Some(window) = self.app.get_webview_window(LABEL) {
            let _ = window.destroy();
        }
        let page = Url::parse(READER_PAGE).map_err(|error| error.to_string())?;
        let (loaded, on_load) = mpsc::channel::<Url>();
        let loaded = Mutex::new(loaded);
        let window = WebviewWindowBuilder::new(&self.app, LABEL, WebviewUrl::External(page))
            .title("Pixiv Messages")
            .visible(false)
            .focused(false)
            .skip_taskbar(true)
            .data_directory(self.profile.clone())
            .on_navigation(|url| url.scheme() == "https" && url.host_str() == Some(PIXIV_HOST))
            .on_new_window(|_, _| NewWindowResponse::Deny)
            .on_permission_request(|_, _| PermissionResponse::Deny)
            .on_download(|_, _| false)
            .on_page_load(move |_, payload| {
                if payload.event() == PageLoadEvent::Finished {
                    let _ = loaded
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .send(payload.url().clone());
                }
            })
            .build()
            .map_err(|error| error.to_string())?;
        let answer = on_load
            .recv_timeout(LOAD_WAIT)
            .map_err(|_| "Pixiv's page didn't load".to_owned())
            .and_then(|url| {
                if url.host_str() == Some(PIXIV_HOST) {
                    ask_threads(&window)
                } else {
                    Err(format!("Pixiv's page went to {url}"))
                }
            });
        let _ = window.destroy();
        answer
    }
}

/// Asks the page for the threads, as the website's own script does, and gives Pixiv's answer.
fn ask_threads(window: &WebviewWindow) -> Result<MessagesAnswer, String> {
    let (answered, answer) = mpsc::channel::<Result<String, String>>();
    window
        .with_webview(move |webview| {
            // SAFETY: WebView2's calls, made on the window's thread as `with_webview` runs it.
            if let Err(error) = unsafe { evaluate(&webview.controller(), answered.clone()) } {
                let _ = answered.send(Err(error.to_string()));
            }
        })
        .map_err(|error| error.to_string())?;
    let result = answer
        .recv_timeout(ANSWER_WAIT)
        .map_err(|_| "Pixiv didn't answer".to_owned())??;
    answer_in(&result)
}

/// The script that asks for the threads: Pixiv's status and body, as JSON.
fn script() -> String {
    format!(
        "(async () => {{ const answer = await fetch('{THREADS_ADDRESS}', {{ credentials: 'same-origin', signal: \
         AbortSignal.timeout({FETCH_TIMEOUT_MS}) }}); return JSON.stringify({{ status: answer.status, body: await \
         answer.text() }}); }})()"
    )
}

unsafe fn evaluate(
    controller: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller,
    answered: Sender<Result<String, String>>,
) -> windows::core::Result<()> {
    use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
    use windows::core::HSTRING;
    let parameters = serde_json::json!({
        "expression": script(),
        "awaitPromise": true,
        "returnByValue": true,
    })
    .to_string();
    // SAFETY: the caller is on the window's thread.
    unsafe {
        let webview = controller.CoreWebView2()?;
        webview.CallDevToolsProtocolMethod(
            &HSTRING::from("Runtime.evaluate"),
            &HSTRING::from(parameters),
            &CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |result, json| {
                let _ = answered.send(result.map(|()| json).map_err(|error| error.to_string()));
                Ok(())
            })),
        )
    }
}

/// Pixiv's answer in DevTools' answer to `Runtime.evaluate`: the script's JSON string as its value, or the error it
/// threw.
fn answer_in(evaluated: &str) -> Result<MessagesAnswer, String> {
    let evaluated: serde_json::Value =
        serde_json::from_str(evaluated).map_err(|error| error.to_string())?;
    if let Some(exception) = evaluated.get("exceptionDetails") {
        let description = exception
            .pointer("/exception/description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("the request failed");
        return Err(description.lines().next().unwrap_or_default().to_owned());
    }
    let text = evaluated
        .pointer("/result/value")
        .and_then(serde_json::Value::as_str)
        .ok_or("Pixiv's answer is missing")?;
    let fetched: serde_json::Value =
        serde_json::from_str(text).map_err(|error| error.to_string())?;
    Ok(MessagesAnswer {
        status: fetched
            .get("status")
            .and_then(serde_json::Value::as_u64)
            .and_then(|status| u16::try_from(status).ok())
            .unwrap_or_default(),
        body: fetched
            .get("body")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    })
}

struct Listener {
    app: AppHandle,
    /// The Pixiv downloads a warning stopped, resumed once the person goes on.
    stopped: Mutex<Vec<i64>>,
}

impl WarningListener for Listener {
    fn changed(&self, status: &PixivWarningStatus) {
        let _ = self.app.emit(CHANGED_EVENT, status);
    }

    fn warned(&self, _warning: PixivWarning) {
        if let Some(queue) = self.app.try_state::<Arc<DownloadQueue>>() {
            let running: Vec<i64> = queue
                .items()
                .into_iter()
                .filter(|item| {
                    item.state == QueueState::Running && pixiv_warning::holds(&item.video.url)
                })
                .map(|item| item.id)
                .collect();
            if !running.is_empty() && queue.stop_these(&running).is_ok() {
                self.stopped
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .extend(running);
            }
        }
        background::pixiv_warned(&self.app);
    }

    fn dismissed(&self) {
        let stopped =
            std::mem::take(&mut *self.stopped.lock().unwrap_or_else(PoisonError::into_inner));
        if let (false, Some(queue)) = (
            stopped.is_empty(),
            self.app.try_state::<Arc<DownloadQueue>>(),
        ) {
            let _ = queue.resume_these(&stopped);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_pixivs_answer_from_devtools() {
        let evaluated = serde_json::json!({
            "result": {"type": "string", "value": r#"{"status":403,"body":"{\"error\":true}"}"#}
        })
        .to_string();
        assert_eq!(
            answer_in(&evaluated),
            Ok(MessagesAnswer {
                status: 403,
                body: r#"{"error":true}"#.to_owned()
            })
        );
        let thrown = serde_json::json!({
            "result": {"type": "object"},
            "exceptionDetails": {"exception": {"description": "TypeError: Failed to fetch\n    at <anonymous>:1:20"}}
        })
        .to_string();
        assert_eq!(
            answer_in(&thrown),
            Err("TypeError: Failed to fetch".to_owned())
        );
    }

    #[test]
    fn asks_pixivs_own_address() {
        assert!(script().contains("fetch('/rpc/index.php?mode=latest_message_threads2"));
    }
}
