//! The check window's own downloads, through WebView2's download operation rather than Tauri's download hook, which
//! tells neither the name the site gives a file nor its progress: which download the page started (its address, the
//! name in its Content-Disposition and its size), where the window saves it, its progress, its end, and stopping it.
//! WebView2 wants its objects touched on the window's thread, so the download being saved is kept there (`SAVING`),
//! and stopping it is sent there (`cancel`).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use tauri::{AppHandle, WebviewWindow};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_DOWNLOAD_STATE, COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED,
    COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS, ICoreWebView2_4, ICoreWebView2DownloadOperation,
};
use webview2_com::{
    BytesReceivedChangedEventHandler, DownloadStartingEventHandler, StateChangedEventHandler,
    take_pwstr,
};
use windows::core::{HSTRING, Interface, PWSTR};

thread_local! {
    /// The download the window is saving, kept on the window's thread.
    static SAVING: RefCell<Option<ICoreWebView2DownloadOperation>> = const { RefCell::new(None) };
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
                    Decision::Save(path) => {
                        args.SetResultFilePath(&HSTRING::from(path.as_os_str()))?;
                        // No download bar of WebView2's own.
                        args.SetHandled(true)?;
                        follow(&operation, Rc::clone(&callbacks))?;
                        SAVING.with(|saving| *saving.borrow_mut() = Some(operation));
                    }
                    Decision::Refuse => args.SetCancel(true)?,
                }
                Ok(())
            })),
            &mut token,
        )
    }
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
