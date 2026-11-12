//! The check window's own downloads, through WebView2's download operation rather than Tauri's download hook, which
//! tells neither the name the site gives a file nor its progress, and can't hold a download: which download the page
//! started (its address, the name in its Content-Disposition and its size), holding it while the queue decides what
//! becomes of it (WebView2's deferral), where the window saves it, its progress, its end, and stopping it. WebView2
//! wants its objects touched on the window's thread, so the downloads held and the one being saved are kept there
//! (`HELD`, `SAVING`), and answers and stopping are sent there (`answer`, `cancel`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use tauri::{AppHandle, WebviewWindow};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_DOWNLOAD_STATE, COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED,
    COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS, ICoreWebView2_4, ICoreWebView2Deferral,
    ICoreWebView2DownloadOperation, ICoreWebView2DownloadStartingEventArgs,
};
use webview2_com::{
    BytesReceivedChangedEventHandler, DownloadStartingEventHandler, StateChangedEventHandler,
    take_pwstr,
};
use windows::core::{HSTRING, Interface, PWSTR};

/// The downloads held, numbered: a download is held from its start until the queue answers.
static NEXT_HELD: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// The downloads held while the queue decides, kept on the window's thread.
    static HELD: RefCell<HashMap<u64, Held>> = RefCell::new(HashMap::new());
    /// The download the window is saving, kept on the window's thread.
    static SAVING: RefCell<Option<ICoreWebView2DownloadOperation>> = const { RefCell::new(None) };
}

/// A download held: the event's arguments, which still take what's decided, and its deferral.
struct Held {
    args: ICoreWebView2DownloadStartingEventArgs,
    deferral: ICoreWebView2Deferral,
    operation: ICoreWebView2DownloadOperation,
    callbacks: Rc<Callbacks>,
}

/// A download the window's page started.
pub struct Started {
    pub address: String,
    /// From its Content-Disposition, as the site gives it.
    pub name: Option<String>,
    /// From its Content-Length, when the site gives it.
    pub total: Option<u64>,
}

/// What the window does with a download its page started.
pub enum Decision {
    /// Saves it there, then tells its progress and its end.
    Save(PathBuf),
    /// Stops it before a byte is saved.
    Refuse,
    /// Holds it, then calls this with the number to answer it by (`answer`).
    Hold(Box<dyn FnOnce(u64)>),
}

/// The answer for a download held.
pub enum Answer {
    Save(PathBuf),
    Stop,
}

/// What the window asks about its downloads, all on the window's thread.
pub struct Callbacks {
    pub decide: Box<dyn Fn(&Started) -> Decision + Send>,
    /// Bytes received, and the total when known.
    pub progress: Box<dyn Fn(u64, Option<u64>) + Send>,
    /// Whether it arrived whole.
    pub finished: Box<dyn Fn(bool) + Send>,
}

/// Watches the downloads the window's page starts. The window has no download hook of Tauri's, so every download goes
/// through `decide`.
pub fn watch(window: &WebviewWindow, callbacks: Callbacks) -> tauri::Result<()> {
    window.with_webview(move |webview| {
        let callbacks = Rc::new(callbacks);
        // SAFETY: WebView2's calls, made on the window's thread as `with_webview` runs it.
        if let Err(error) = unsafe { install(&webview.controller(), callbacks) } {
            super::development::log(&format!("couldn't watch the window's downloads: {error}"));
        }
    })
}

unsafe fn install(
    controller: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller,
    callbacks: Rc<Callbacks>,
) -> windows::core::Result<()> {
    // SAFETY: the caller is on the window's thread.
    unsafe {
        let webview: ICoreWebView2_4 = controller.CoreWebView2()?.cast()?;
        let mut token = 0;
        webview.add_DownloadStarting(
            &DownloadStartingEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let operation = args.DownloadOperation()?;
                let started = started(&operation)?;
                match (callbacks.decide)(&started) {
                    Decision::Save(path) => save(&args, &operation, &path, Rc::clone(&callbacks))?,
                    Decision::Refuse => args.SetCancel(true)?,
                    Decision::Hold(then) => {
                        let deferral = args.GetDeferral()?;
                        let number = NEXT_HELD.fetch_add(1, Ordering::Relaxed);
                        HELD.with(|held| {
                            held.borrow_mut().insert(
                                number,
                                Held {
                                    args: args.clone(),
                                    deferral,
                                    operation: operation.clone(),
                                    callbacks: Rc::clone(&callbacks),
                                },
                            )
                        });
                        then(number);
                    }
                }
                Ok(())
            })),
            &mut token,
        )
    }
}

/// Saves the download at `path`, with no download bar of WebView2's own, and follows it.
unsafe fn save(
    args: &ICoreWebView2DownloadStartingEventArgs,
    operation: &ICoreWebView2DownloadOperation,
    path: &Path,
    callbacks: Rc<Callbacks>,
) -> windows::core::Result<()> {
    // SAFETY: the caller is on the window's thread.
    unsafe {
        args.SetResultFilePath(&HSTRING::from(path.as_os_str()))?;
        args.SetHandled(true)?;
        follow(operation, callbacks)?;
    }
    SAVING.with(|saving| *saving.borrow_mut() = Some(operation.clone()));
    Ok(())
}

/// Answers download `number`, held since it started: the window saves it, or stops it. A download whose window is
/// gone meanwhile went with it.
pub fn answer(app: &AppHandle, number: u64, answer: Answer) {
    let _ = app.run_on_main_thread(move || {
        let Some(held) = HELD.with(|held| held.borrow_mut().remove(&number)) else {
            return;
        };
        // SAFETY: on the window's thread, where the download was kept.
        let answered = unsafe {
            match answer {
                Answer::Save(path) => save(&held.args, &held.operation, &path, held.callbacks),
                Answer::Stop => held.args.SetCancel(true),
            }
            .and_then(|()| held.deferral.Complete())
        };
        if let Err(error) = answered {
            super::development::log(&format!("couldn't answer a download: {error}"));
        }
    });
}

/// The download's address, name and size.
unsafe fn started(operation: &ICoreWebView2DownloadOperation) -> windows::core::Result<Started> {
    // SAFETY: the caller is on the window's thread.
    unsafe {
        let mut address = PWSTR::null();
        operation.Uri(&mut address)?;
        let mut disposition = PWSTR::null();
        operation.ContentDisposition(&mut disposition)?;
        let mut total = 0;
        operation.TotalBytesToReceive(&mut total)?;
        Ok(Started {
            address: take_pwstr(address),
            name: tsundoku_core::hosts::attachment_name(&take_pwstr(disposition)),
            total: u64::try_from(total).ok().filter(|total| *total > 0),
        })
    }
}

/// Tells the saved download's progress and its end.
unsafe fn follow(
    operation: &ICoreWebView2DownloadOperation,
    callbacks: Rc<Callbacks>,
) -> windows::core::Result<()> {
    // SAFETY: the caller is on the window's thread.
    unsafe {
        let mut token = 0;
        let told = Rc::clone(&callbacks);
        operation.add_BytesReceivedChanged(
            &BytesReceivedChangedEventHandler::create(Box::new(move |operation, _| {
                if let Some(operation) = operation {
                    let mut received = 0;
                    operation.BytesReceived(&mut received)?;
                    let mut total = 0;
                    operation.TotalBytesToReceive(&mut total)?;
                    (told.progress)(
                        u64::try_from(received).unwrap_or(0),
                        u64::try_from(total).ok().filter(|total| *total > 0),
                    );
                }
                Ok(())
            })),
            &mut token,
        )?;
        operation.add_StateChanged(
            &StateChangedEventHandler::create(Box::new(move |operation, _| {
                if let Some(operation) = operation {
                    let mut state = COREWEBVIEW2_DOWNLOAD_STATE::default();
                    operation.State(&mut state)?;
                    if state != COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS {
                        SAVING.with(|saving| saving.borrow_mut().take());
                        (callbacks.finished)(state == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED);
                    }
                }
                Ok(())
            })),
            &mut token,
        )
    }
}

/// Stops the download the window is saving, if any: it ends as not arrived, and what was saved is deleted.
pub fn cancel(app: &AppHandle) {
    let _ = app.run_on_main_thread(|| {
        SAVING.with(|saving| {
            if let Some(operation) = saving.borrow().as_ref() {
                // SAFETY: on the window's thread, where the operation was kept.
                let _ = unsafe { operation.Cancel() };
            }
        });
    });
}
