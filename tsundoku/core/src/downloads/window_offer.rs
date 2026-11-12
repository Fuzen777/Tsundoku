//! A download the person started in the check window, offered to the queue before the window saves anything. The
//! queue decides per download, never per site: the download's run makes its own first request for the address, as
//! Tsundoku and with nothing else changed (`file_runner`). When the answer is strictly the file, the run keeps it and
//! the window stops its own download (`WindowAnswer::Fetching`); otherwise, or when no run takes the offer within
//! `OFFER_WAIT` (the queue is full), the window saves the file itself into the download's temporary folder
//! (`WindowAnswer::SaveIn`), the row shows its progress, and the download moves it into place once it arrives. A
//! download stopped or removed meanwhile refuses the offer, and the window stops its download.

use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use std::time::Duration;

use reqwest::Url;

use super::{DownloadError, DownloadQueue, QueueItem, QueueState};
use crate::hosts::{self, windowed};
use crate::rules::safe_folder_name;
use crate::sync::lock;

/// How long the window's download waits for the queue's answer before the window saves the file itself (shorter
/// for the queue's tests).
pub const OFFER_WAIT: Duration = if cfg!(test) {
    Duration::from_millis(300)
} else {
    Duration::from_secs(20)
};

/// A download the check window's page started, as the window saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowOffer {
    pub address: String,
    /// From its Content-Disposition, as the site gives it.
    pub name: Option<String>,
    /// From its Content-Length, when the site gives it.
    pub total: Option<u64>,
}

/// What the window does with a download it offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowAnswer {
    /// Tsundoku downloads the file itself: the window stops its own download.
    Fetching,
    /// The window saves the file at this path, telling its progress to the download's row.
    SaveIn(PathBuf),
    /// Nothing: the download was stopped or removed, or doesn't wait for this file. The window stops its download.
    Refuse,
}

/// How the window hears the queue's answer, once.
pub type WindowReply = Box<dyn FnOnce(WindowAnswer) + Send>;

/// The window's reply until someone answers: the run, the wait running out, or the download stopping.
struct Slot(Mutex<Option<WindowReply>>);

impl Slot {
    fn take(&self) -> Option<WindowReply> {
        lock(&self.0).take()
    }
}

/// An offer a download holds until its run takes it.
#[derive(Clone)]
pub(crate) struct Offered {
    /// The file it's for (`CheckPage::file_id`).
    pub file_id: String,
    pub offer: WindowOffer,
    slot: Arc<Slot>,
}

impl fmt::Debug for Offered {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Offered")
            .field("file_id", &self.file_id)
            .field("offer", &self.offer)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Offered {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.slot, &other.slot)
    }
}

impl Eq for Offered {}

impl Offered {
    /// Stops the window's download: the download no longer wants it.
    pub(crate) fn refuse(&self) {
        if let Some(reply) = self.slot.take() {
            reply(WindowAnswer::Refuse);
        }
    }
}

/// The offer for one of a run's files.
#[derive(Clone)]
pub struct JobOffer {
    pub file_id: String,
    pub offer: WindowOffer,
    id: i64,
    queue: Weak<DownloadQueue>,
    slot: Arc<Slot>,
}

impl fmt::Debug for JobOffer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JobOffer")
            .field("file_id", &self.file_id)
            .field("offer", &self.offer)
            .finish_non_exhaustive()
    }
}

impl JobOffer {
    /// The run's own first request answered with the file: the window stops its download, and the address is kept for
    /// the download's next runs. Whether the run may go on with it: not once the window's download carried on.
    pub fn fetch(&self) -> bool {
        let Some(reply) = self.slot.take() else {
            return false;
        };
        if let Some(queue) = self.queue.upgrade() {
            queue.fetching_offer(self.id, &self.file_id, &self.offer.address);
        }
        reply(WindowAnswer::Fetching);
        true
    }

    /// The run's first request didn't answer with the file: the window saves it.
    pub fn leave_to_window(&self) {
        leave_to_window(&self.queue, self.id, &self.offer, &self.slot);
    }

    /// The run ended without answering: the window saves the file, unless the download was stopped or removed.
    pub(super) fn settle(&self, stopped: bool) {
        if stopped {
            if let Some(reply) = self.slot.take() {
                reply(WindowAnswer::Refuse);
            }
        } else {
            self.leave_to_window();
        }
    }
}

/// The window saves the file, if nobody answered yet: its path in the download's temporary folder, or nothing once
/// the download no longer waits for it.
fn leave_to_window(queue: &Weak<DownloadQueue>, id: i64, offer: &WindowOffer, slot: &Slot) {
    let Some(reply) = slot.take() else {
        return;
    };
    let answer = queue
        .upgrade()
        .and_then(|queue| queue.save_in_window(id, offer).ok())
        .map_or(WindowAnswer::Refuse, WindowAnswer::SaveIn);
    reply(answer);
}

impl DownloadQueue {
    /// What a run knows of the offer it takes.
    pub(super) fn job_offer(&self, id: i64, offered: Offered) -> JobOffer {
        JobOffer {
            file_id: offered.file_id,
            offer: offered.offer,
            id,
            queue: self.myself.clone(),
            slot: offered.slot,
        }
    }

    /// The person started the download of the file `file_id` of download `id` in the check window, which holds it
    /// until `reply` says what to do (`WindowAnswer`). The download starts at once if there's room, ahead of the other
    /// waiting ones, and its run decides; without an answer within `OFFER_WAIT`, the window saves the file.
    pub fn offer_window_download(
        &self,
        id: i64,
        file_id: &str,
        offer: WindowOffer,
        reply: WindowReply,
    ) {
        let slot = Arc::new(Slot(Mutex::new(Some(reply))));
        let offered = Offered {
            file_id: file_id.to_owned(),
            offer: offer.clone(),
            slot: Arc::clone(&slot),
        };
        let taken = {
            let mut state = lock(&self.state);
            match super::find(&mut state.items, id) {
                Ok(item) if takes_offer(item, file_id) => {
                    item.recovery.offer = Some(offered.clone());
                    true
                }
                _ => false,
            }
        };
        if !taken {
            offered.refuse();
            return;
        }
        let queue = self.myself.clone();
        thread::spawn(move || {
            thread::sleep(OFFER_WAIT);
            leave_to_window(&queue, id, &offer, &slot);
        });
        self.schedule();
    }

    /// The run fetches the offered file itself: the download keeps the address for its next runs, and stops waiting
    /// for the file's page.
    fn fetching_offer(&self, id: i64, file_id: &str, address: &str) {
        let mut state = lock(&self.state);
        let Ok(item) = super::find(&mut state.items, id) else {
            return;
        };
        let link = windowed::caught_link(file_id, address);
        if let Some(check) = &mut item.recovery.check {
            check.passed(&link);
            if check.pages.is_empty() {
                item.recovery.check = None;
            }
        }
        item.caught_links.push(link);
    }

    /// Where the window saves an offered file: the download's temporary folder, made if it isn't there, under the
    /// name the site gives it. The download waits for the window meanwhile (`Hold::WindowDownload`), and a link read
    /// through the window takes the file's name and size at once, when the site gives them. `WrongState` once the
    /// download no longer waits for the file (stopped, removed, failed).
    pub(crate) fn save_in_window(
        &self,
        id: i64,
        offer: &WindowOffer,
    ) -> Result<PathBuf, DownloadError> {
        let name = file_name(offer);
        let folder = {
            let mut state = lock(&self.state);
            let ending = state
                .running
                .get(&id)
                .is_some_and(|running| running.end_request.is_some());
            let item = super::find(&mut state.items, id)?;
            if item
                .recovery
                .offer
                .as_ref()
                .is_some_and(|offered| offered.offer == *offer)
            {
                item.recovery.offer = None;
            }
            let waits = match item.state {
                QueueState::Waiting => true,
                QueueState::Running => !ending,
                _ => false,
            };
            let Some(site) = item.recovery.check.as_ref().map(|check| check.site.clone()) else {
                return Err(DownloadError::WrongState);
            };
            if !waits {
                return Err(DownloadError::WrongState);
            }
            item.recovery.window_download = Some(site);
            if item.video.files.is_some() && item.video.title == item.video.video_id {
                item.video.title = name.clone();
                if let (Some(contents), Some(size)) = (&mut item.video.files, offer.total)
                    && contents.bytes == 0
                {
                    contents.bytes = size;
                }
                let _ = self.store.update(item);
            }
            super::temp_folder(item)
        };
        fs::create_dir_all(&folder).map_err(|error| DownloadError::Folder {
            path: folder.display().to_string(),
            detail: error.to_string(),
        })?;
        let path = folder.join(name);
        let _ = fs::remove_file(&path);
        self.schedule();
        Ok(path)
    }

    /// Whether download `id` still waits for the file the check window saves for it: the window stops its download
    /// once it doesn't (stopped, removed).
    pub fn waits_for_window(&self, id: i64) -> bool {
        let mut state = lock(&self.state);
        super::find(&mut state.items, id).is_ok_and(|item| {
            item.recovery.window_download.is_some()
                && matches!(item.state, QueueState::Waiting | QueueState::Running)
        })
    }

    /// The check window closed before it began saving a file it was told to save: the download waits for its check
    /// again, as if the person hadn't started it.
    pub fn window_download_withdrawn(&self, id: i64) {
        {
            let mut state = lock(&self.state);
            let Ok(item) = super::find(&mut state.items, id) else {
                return;
            };
            item.recovery.window_download = None;
            item.recovery.window_meter = None;
            item.progress = None;
        }
        self.schedule();
    }

    /// The check window closed while it held a download it offered: the download no longer gets it.
    pub fn withdraw_window_offer(&self, id: i64) {
        let offered = {
            let mut state = lock(&self.state);
            super::find(&mut state.items, id)
                .ok()
                .and_then(|item| item.recovery.offer.take())
        };
        if let Some(offered) = offered {
            offered.refuse();
        }
        self.schedule();
    }
}

/// Whether `item` takes an offer for its file `file_id`: it waits for that file's page, and nothing else is offered
/// or saved for it meanwhile.
fn takes_offer(item: &QueueItem, file_id: &str) -> bool {
    matches!(item.state, QueueState::Waiting | QueueState::Running)
        && item.recovery.offer.is_none()
        && item.recovery.window_download.is_none()
        && item
            .recovery
            .check
            .as_ref()
            .is_some_and(|check| check.pages.iter().any(|page| page.file_id() == file_id))
}

/// The name the window saves an offered file under: the one the site gives it, made safe, else the last part of its
/// address, never one the browser numbered after what a Downloads folder holds.
fn file_name(offer: &WindowOffer) -> String {
    let name = offer
        .name
        .clone()
        .or_else(|| {
            Url::parse(&offer.address)
                .ok()?
                .path_segments()?
                .next_back()
                .filter(|segment| !segment.is_empty())
                .map(hosts::percent_decode)
        })
        .unwrap_or_else(|| "download".to_owned());
    safe_folder_name(&name, "download")
}
