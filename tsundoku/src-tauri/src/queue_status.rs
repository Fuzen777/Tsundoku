//! The queue's run outside the window: Tsundoku's taskbar button fills with its overall progress (yellow while
//! stopped, red once a download failed), and the tray icon's tooltip sums it up ("3 downloading · 45% · 1 failed").
//! Both follow the queue's changes at once, and its progress at most four times a second. Its failures and its end go
//! on to the notifications (`background::queue_events`).

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use tauri::window::{ProgressBarState, ProgressBarStatus};
use tauri::{AppHandle, Manager};
use tsundoku_core::downloads::{DownloadProgress, QueueItem, QueueRun, TaskbarProgress};

use crate::background;

/// How often progress alone may update the taskbar and the tooltip.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

/// Follows the queue's run, for the taskbar and the tray.
#[derive(Default)]
pub struct QueueStatus {
    state: Mutex<StatusState>,
}

#[derive(Default)]
struct StatusState {
    run: QueueRun,
    /// What should show, as worked out last.
    wanted: Shown,
    /// What shows; only the main thread changes it.
    shown: Shown,
    worked_out_at: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Shown {
    taskbar: TaskbarProgress,
    /// `None` for the app's name.
    tooltip: Option<String>,
}

impl Default for Shown {
    fn default() -> Self {
        Self {
            taskbar: TaskbarProgress::Hidden,
            tooltip: None,
        }
    }
}

impl QueueStatus {
    /// The queue's items changed. Failures and the run's end go on to the notifications.
    pub fn changed(&self, app: &AppHandle, items: &[QueueItem]) {
        let mut state = self.lock();
        let events = state.run.update(items);
        self.work_out(app, state);
        if !events.is_empty() {
            background::queue_events(app, events);
        }
    }

    /// A running download's progress.
    pub fn progress(&self, app: &AppHandle, id: i64, progress: &DownloadProgress) {
        let mut state = self.lock();
        state.run.progress(id, progress);
        if state
            .worked_out_at
            .is_some_and(|at| at.elapsed() < PROGRESS_INTERVAL)
        {
            return;
        }
        self.work_out(app, state);
    }

    /// The tray icon's tooltip as it should read now, for a tray icon being made.
    pub fn tooltip(&self) -> Option<String> {
        self.lock().wanted.tooltip.clone()
    }

    fn lock(&self) -> MutexGuard<'_, StatusState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Works out what should show and, when it changed, has the main thread show it. Showing happens there, never
    /// under this lock: the tray waits for the main thread, which may be waiting for this lock (a queue command).
    fn work_out(&self, app: &AppHandle, mut state: MutexGuard<'_, StatusState>) {
        let summary = state.run.summary();
        let wanted = Shown {
            taskbar: summary.taskbar(),
            tooltip: summary.tooltip(),
        };
        state.worked_out_at = Some(Instant::now());
        if wanted == state.wanted {
            return;
        }
        state.wanted = wanted;
        drop(state);
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(status) = handle.try_state::<Arc<QueueStatus>>() {
                status.show(&handle);
            }
        });
    }

    /// On the main thread: shows what's wanted now, so an older request that arrives late changes nothing.
    fn show(&self, app: &AppHandle) {
        let (wanted, shown) = {
            let state = self.lock();
            (state.wanted.clone(), state.shown.clone())
        };
        if wanted.taskbar != shown.taskbar
            && let Some(window) = app.get_webview_window("main")
        {
            let _ = window.set_progress_bar(progress_bar(wanted.taskbar));
        }
        if wanted.tooltip != shown.tooltip {
            background::set_tray_tooltip(app, wanted.tooltip.as_deref());
        }
        self.lock().shown = wanted;
    }
}

fn progress_bar(taskbar: TaskbarProgress) -> ProgressBarState {
    let (status, progress) = match taskbar {
        TaskbarProgress::Hidden => (ProgressBarStatus::None, None),
        TaskbarProgress::Indeterminate => (ProgressBarStatus::Indeterminate, None),
        TaskbarProgress::Normal(percent) => (ProgressBarStatus::Normal, Some(percent)),
        TaskbarProgress::Paused(percent) => (ProgressBarStatus::Paused, Some(percent)),
        TaskbarProgress::Failed(percent) => (ProgressBarStatus::Error, Some(percent)),
    };
    ProgressBarState {
        status: Some(status),
        progress,
    }
}
