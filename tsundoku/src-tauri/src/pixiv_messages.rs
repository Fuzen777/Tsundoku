//! Pixiv's warnings (`tsundoku_core::pixiv_warning`): the reader that asks Pixiv for the account's latest message
//! threads, and what the app does when one is a warning. Pixiv's messages need its website's sign-in, which the
//! sign-in window's profile (`DataPaths::accounts_browser_folder`) holds since the person signed in there: the reader
//! asks for `THREADS_ADDRESS` from `READER_PAGE`, a small page of Pixiv's own, in a hidden window (`web_reader`, label
//! `pixiv-messages`).
//!
//! When a check finds a warning, the Pixiv downloads running are stopped (and resumed once the person goes on), the
//! UI is told (`pixiv-warning-changed`), and a notification says so while the window isn't in front.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tsundoku_core::downloads::{DownloadQueue, QueueState};
use tsundoku_core::pixiv_warning::{
    self, MessageReader, MessagesAnswer, PixivWarning, PixivWarningStatus, READER_PAGE,
    THREADS_ADDRESS, WarningListener,
};

use crate::background;
use crate::web_reader::PageReader;

const LABEL: &str = "pixiv-messages";
const CHANGED_EVENT: &str = "pixiv-warning-changed";

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
        let reader = PageReader {
            app: self.app.clone(),
            profile: self.profile.clone(),
            label: LABEL,
            page: READER_PAGE,
        };
        let answer = reader
            .read(&[THREADS_ADDRESS.to_owned()], Duration::ZERO)?
            .pop()
            .ok_or("Pixiv didn't answer")??;
        Ok(MessagesAnswer {
            status: answer.status,
            body: answer.body,
        })
    }
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
