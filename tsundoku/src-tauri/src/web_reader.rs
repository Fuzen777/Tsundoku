//! Reading a site's own API as its website does, in a hidden window on the sign-in window's profile
//! (`DataPaths::accounts_browser_folder`), whose sessions the person signed in there: Pixiv's messages
//! (`pixiv_messages.rs`) and Fanbox's posts. The window (never shown, no capability, only the site's own host, nothing
//! opened or downloaded) loads a small page of the site's own, then asks for each address with a `fetch` from that
//! page, sending its cookies as the site's own script does (WebView2's DevTools call `Runtime.evaluate`, awaiting the
//! answer), and is destroyed once they're answered. Tsundoku never presses anything there and never reads a page's
//! layout: only the answers to the addresses it asks.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use tauri::webview::{NewWindowResponse, PageLoadEvent, PermissionResponse};
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// How long the page may take to load, and each answer to come.
const LOAD_WAIT: Duration = Duration::from_secs(30);
const ANSWER_WAIT: Duration = Duration::from_secs(30);
/// How long the page's `fetch` waits for the site, under `ANSWER_WAIT`.
const FETCH_TIMEOUT_MS: u32 = 20_000;

/// A site's answer to one address: its HTTP status and body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageAnswer {
    pub status: u16,
    pub body: String,
}

/// A hidden window that reads a site's API from one of its pages.
pub struct PageReader {
    pub app: AppHandle,
    /// The sign-in window's profile.
    pub profile: PathBuf,
    /// The window's label: one reader of each kind at a time.
    pub label: &'static str,
    /// The site's page the addresses are asked from.
    pub page: &'static str,
}

impl PageReader {
    /// Asks for each address in turn from the reader's page, waiting `pause` between them, and gives each answer, or
    /// why the page couldn't be opened. One that can't be asked gives its reason in its place. The window is closed
    /// once they're answered.
    pub fn read(
        &self,
        addresses: &[String],
        pause: Duration,
    ) -> Result<Vec<Result<PageAnswer, String>>, String> {
        let window = self.open()?;
        let answers = addresses
            .iter()
            .enumerate()
            .map(|(index, address)| {
                if index > 0 {
                    std::thread::sleep(pause);
                }
                ask(&window, address)
            })
            .collect();
        let _ = window.destroy();
        Ok(answers)
    }

    /// Opens the reader's window on its page, replacing one left open, and waits for the page to load.
    pub fn open(&self) -> Result<WebviewWindow, String> {
        if let Some(window) = self.app.get_webview_window(self.label) {
            let _ = window.destroy();
        }
        let page = Url::parse(self.page).map_err(|error| error.to_string())?;
        let host = page.host_str().unwrap_or_default().to_owned();
        let (loaded, on_load) = mpsc::channel::<Url>();
        let loaded = Mutex::new(loaded);
        let window = WebviewWindowBuilder::new(&self.app, self.label, WebviewUrl::External(page))
            .title("Tsundoku")
            .visible(false)
            .focused(false)
            .skip_taskbar(true)
            .data_directory(self.profile.clone())
            .on_navigation(move |url| {
                url.scheme() == "https" && url.host_str() == Some(host.as_str())
            })
            .on_new_window(|_, _| NewWindowResponse::Deny)
            .on_permission_request(|_, _| PermissionResponse::Deny)
            .on_download(|_, _| false)
            .on_page_load(move |_, payload| {
                if payload.event() == PageLoadEvent::Finished {
                    let _ = loaded
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .send(payload.url().clone());
                }
            })
            .build()
            .map_err(|error| error.to_string())?;
        let loaded = on_load
            .recv_timeout(LOAD_WAIT)
            .map_err(|_| "the site's page didn't load".to_owned())
            .and_then(|url| {
                let expected = Url::parse(self.page).map_err(|error| error.to_string())?;
                if url.host_str() == expected.host_str() {
                    Ok(())
                } else {
                    Err(format!("the site's page went to {url}"))
                }
            });
        match loaded {
            Ok(()) => Ok(window),
            Err(error) => {
                let _ = window.destroy();
                Err(error)
            }
        }
    }
}

/// Asks the page of a reader's window (`PageReader::open`) for one address and gives the site's answer.
pub fn ask(window: &WebviewWindow, address: &str) -> Result<PageAnswer, String> {
    let (answered, answer) = mpsc::channel::<Result<String, String>>();
    let expression = script(address);
    window
        .with_webview(move |webview| {
            // SAFETY: WebView2's calls, made on the window's thread as `with_webview` runs it.
            if let Err(error) =
                unsafe { evaluate(&webview.controller(), &expression, answered.clone()) }
            {
                let _ = answered.send(Err(error.to_string()));
            }
        })
        .map_err(|error| error.to_string())?;
    let result = answer
        .recv_timeout(ANSWER_WAIT)
        .map_err(|_| "the site didn't answer".to_owned())??;
    answer_in(&result)
}

/// The value of the cookie `name` the window's session sends to `address`, if it has one. Read for the site's own
/// servers only (Fanbox's file server, which asks for the session's cookie).
pub fn cookie(window: &WebviewWindow, address: &str, name: &str) -> Result<Option<String>, String> {
    let (answered, answer) = mpsc::channel::<Result<Option<String>, String>>();
    let (address, name) = (address.to_owned(), name.to_owned());
    window
        .with_webview(move |webview| {
            // SAFETY: WebView2's calls, made on the window's thread as `with_webview` runs it.
            if let Err(error) =
                unsafe { cookie_of(&webview.controller(), &address, name, answered.clone()) }
            {
                let _ = answered.send(Err(error.to_string()));
            }
        })
        .map_err(|error| error.to_string())?;
    answer
        .recv_timeout(ANSWER_WAIT)
        .map_err(|_| "the window didn't answer".to_owned())?
}

unsafe fn cookie_of(
    controller: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller,
    address: &str,
    name: String,
    answered: Sender<Result<Option<String>, String>>,
) -> windows::core::Result<()> {
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2_2, ICoreWebView2CookieList};
    use webview2_com::{GetCookiesCompletedHandler, take_pwstr};
    use windows::core::{HSTRING, Interface, PWSTR};
    let find =
        move |list: Option<ICoreWebView2CookieList>| -> windows::core::Result<Option<String>> {
            let Some(list) = list else {
                return Ok(None);
            };
            // SAFETY: WebView2's calls, on the window's thread, where the completed handler runs.
            unsafe {
                let mut count = 0;
                list.Count(&mut count)?;
                for index in 0..count {
                    let cookie = list.GetValueAtIndex(index)?;
                    let mut cookie_name = PWSTR::null();
                    cookie.Name(&mut cookie_name)?;
                    if take_pwstr(cookie_name) == name {
                        let mut value = PWSTR::null();
                        cookie.Value(&mut value)?;
                        return Ok(Some(take_pwstr(value)));
                    }
                }
            }
            Ok(None)
        };
    // SAFETY: the caller is on the window's thread.
    unsafe {
        let webview: ICoreWebView2_2 = controller.CoreWebView2()?.cast()?;
        webview.CookieManager()?.GetCookies(
            &HSTRING::from(address),
            &GetCookiesCompletedHandler::create(Box::new(move |result, list| {
                let _ = answered.send(
                    result
                        .and_then(|()| find(list))
                        .map_err(|error| error.to_string()),
                );
                Ok(())
            })),
        )
    }
}

/// The script that asks for `address` with the page's cookies: the site's status and body, as JSON.
fn script(address: &str) -> String {
    let address = serde_json::to_string(address).unwrap_or_default();
    format!(
        "(async () => {{ const answer = await fetch({address}, {{ credentials: 'include', signal: \
         AbortSignal.timeout({FETCH_TIMEOUT_MS}) }}); return JSON.stringify({{ status: answer.status, body: await \
         answer.text() }}); }})()"
    )
}

unsafe fn evaluate(
    controller: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller,
    expression: &str,
    answered: Sender<Result<String, String>>,
) -> windows::core::Result<()> {
    use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
    use windows::core::HSTRING;
    let parameters = serde_json::json!({
        "expression": expression,
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

/// The site's answer in DevTools' answer to `Runtime.evaluate`: the script's JSON string as its value, or the error
/// it threw.
fn answer_in(evaluated: &str) -> Result<PageAnswer, String> {
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
        .ok_or("the site's answer is missing")?;
    let fetched: serde_json::Value =
        serde_json::from_str(text).map_err(|error| error.to_string())?;
    Ok(PageAnswer {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_sites_answer_from_devtools() {
        let evaluated = serde_json::json!({
            "result": {"type": "string", "value": r#"{"status":403,"body":"{\"error\":true}"}"#}
        })
        .to_string();
        assert_eq!(
            answer_in(&evaluated),
            Ok(PageAnswer {
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
    fn quotes_the_address_it_asks() {
        let asked = script("/rpc/index.php?mode=latest_message_threads2&num=5");
        assert!(asked.contains(r#"fetch("/rpc/index.php?mode=latest_message_threads2&num=5", "#));
        assert!(script("x'); alert(1); ('").contains(r#"fetch("x'); alert(1); ('", "#));
    }
}
