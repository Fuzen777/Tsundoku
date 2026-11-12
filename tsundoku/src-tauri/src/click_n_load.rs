//! The Click'n'Load receiver (`tsundoku_core::click_n_load`) while Settings → Adding Links has it on: started at launch,
//! started or stopped when the setting changes, its links put in Pending with a notification, and its state sent to
//! the UI as `click-n-load-changed`. It runs as long as the app does, in the tray too.

use std::sync::{Arc, Mutex, PoisonError};

use tauri::{AppHandle, Emitter, Manager, State};
use tsundoku_core::click_n_load::{
    ClickNLoadListener, ClickNLoadStatus, PORT, Receiver, ReceiverState, Senders, StartError,
};
use tsundoku_core::pending::PendingLinks;

use crate::background;

const CHANGED_EVENT: &str = "click-n-load-changed";

/// The receiver while it runs, and its state.
pub struct ClickNLoad {
    running: Mutex<Running>,
}

struct Running {
    receiver: Option<Receiver>,
    state: ReceiverState,
}

impl Default for ClickNLoad {
    fn default() -> Self {
        Self {
            running: Mutex::new(Running {
                receiver: None,
                state: ReceiverState::Off,
            }),
        }
    }
}

impl ClickNLoad {
    fn status(&self) -> ClickNLoadStatus {
        status_of(
            self.running
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .state
                .clone(),
        )
    }
}

fn status_of(state: ReceiverState) -> ClickNLoadStatus {
    ClickNLoadStatus {
        state,
        port: port(),
        senders: Senders::names(),
    }
}

/// Settings → Adding Links' Click'n'Load line.
#[tauri::command]
pub fn click_n_load_status(click_n_load: State<'_, ClickNLoad>) -> ClickNLoadStatus {
    click_n_load.status()
}

/// Listens when `on` and it doesn't yet (again after the port was taken), stops when not, and tells the UI what
/// changed. Called at launch and whenever Settings → Adding Links is saved.
pub fn apply(app: &AppHandle, on: bool) {
    let click_n_load = app.state::<ClickNLoad>();
    let mut running = click_n_load
        .running
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let before = running.state.clone();
    if !on {
        // Dropping the receiver frees the port.
        running.receiver = None;
        running.state = ReceiverState::Off;
    } else if running.receiver.is_none() {
        let listener = Arc::new(Listener { app: app.clone() });
        running.state = match Receiver::start(port(), senders(), listener) {
            Ok(receiver) => {
                running.receiver = Some(receiver);
                ReceiverState::Listening
            }
            Err(StartError::PortTaken) => ReceiverState::PortTaken,
            Err(StartError::Failed(detail)) => ReceiverState::Failed { detail },
        };
    }
    if running.state != before {
        let _ = app.emit(CHANGED_EVENT, status_of(running.state.clone()));
    }
}

/// The port listened on: `PORT`, or in a development build `TSUNDOKU_CNL_TEST_PORT`, so a test copy can run beside a
/// JDownloader that has the port, and a test page never sends it anything. A release build never reads it.
fn port() -> u16 {
    if cfg!(debug_assertions)
        && let Some(port) = std::env::var("TSUNDOKU_CNL_TEST_PORT")
            .ok()
            .and_then(|port| port.trim().parse().ok())
    {
        port
    } else {
        PORT
    }
}

/// The sites taken from: the tested list, and in a development build the test page `TSUNDOKU_CNL_TEST_SENDER` names
/// (a host, such as 127.0.0.1), for a developer's own Click'n'Load page. A release build never reads it.
fn senders() -> Senders {
    if cfg!(debug_assertions)
        && let Ok(host) = std::env::var("TSUNDOKU_CNL_TEST_SENDER")
    {
        Senders::with_test_host(&host)
    } else {
        Senders::listed()
    }
}

/// Puts a sender's links in Pending and says so; notes a refused page in the app's output.
struct Listener {
    app: AppHandle,
}

impl ClickNLoadListener for Listener {
    fn received(&self, site: &str, links: &[String]) -> bool {
        match self.app.state::<Arc<PendingLinks>>().add_from_page(links) {
            Ok(added) => {
                background::page_sent(&self.app, site, added.added);
                true
            }
            Err(_) => false,
        }
    }

    fn refused(&self, host: Option<&str>) {
        eprintln!(
            "Click'n'Load: refused links from {}",
            host.unwrap_or("a page that didn't say where it was")
        );
    }
}
