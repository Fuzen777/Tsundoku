//! Watched sources: channels, playlists and accounts pinned to check for what's new (the Watched view). Each keeps
//! every item a check or its adding has met (`watched_items`), so what was skipped or downloaded never comes back as
//! new. A check reads the source's listing (`ListingReader`), newest first for a channel or an account, and stops past
//! what it knows (`listing::before_known_ground`); a playlist is compared whole. New items wait on the source's card
//! until they're downloaded, with the source's own folder and preset, or skipped.
//!
//! Checks run one at a time on a thread of their own, by hand for now (Check Now). They keep to a site's
//! "too many requests" wait as previews do.

mod store;
#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, Weak};
use std::thread;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::cancel::CancelToken;
use crate::database::Database;
use crate::downloads::{DownloadError, DownloadQueue, QueuePick};
use crate::history::{History, PastDownload};
use crate::listing::{self, Listing, ListingKind};
use crate::metadata::{PreviewError, VideoInfo};
use crate::presets::PresetManager;
use crate::rules::{self, RuleSubject};
use crate::settings::SettingsStore;
use crate::sync::lock;
use store::{ItemState, StoredItem, StoredSource, WatchedStore};

/// Reads a watched source's listing, as the preview card reads a channel, playlist or account.
pub trait ListingReader: Send + Sync {
    fn read(&self, url: &str, cancel: &CancelToken) -> Result<Listing, PreviewError>;
}

/// Told when the watched sources change, to pass them on to the UI.
pub trait WatchedListener: Send + Sync {
    fn changed(&self, sources: &[WatchedSource]);
}

/// What the watched sources use.
pub struct WatchedServices {
    pub database: Arc<Database>,
    pub history: Arc<History>,
    pub queue: Arc<DownloadQueue>,
    pub presets: Arc<PresetManager>,
    pub settings: Arc<SettingsStore>,
    pub reader: Arc<dyn ListingReader>,
    pub listener: Arc<dyn WatchedListener>,
}

/// Why an action on the watched sources failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum WatchedError {
    /// The source was removed meanwhile.
    #[error("this source isn't watched anymore")]
    NotFound,
    /// The same listing is watched already.
    #[error("this source is watched already")]
    AlreadyWatched,
    /// Queueing its items failed.
    #[error("{error}")]
    Queue { error: DownloadError },
    /// The database couldn't be read or written.
    #[error("couldn't save the watched sources: {detail}")]
    Storage { detail: String },
}

/// A pinned channel, playlist or account, as the Watched view shows it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WatchedSource {
    #[ts(type = "number")]
    pub id: i64,
    /// The listing's own page, which each check reads.
    pub url: String,
    pub kind: ListingKind,
    pub name: String,
    /// The site as people know it ("youtube.com").
    pub site_name: String,
    pub thumbnail_url: Option<String>,
    /// Save In: where its downloads go, beating every rule, with the rules' placeholders. `None` lets the rules decide.
    pub folder: Option<String>,
    /// Videos Start As. `None` for the preset a rule picks, else the default.
    pub preset_id: Option<String>,
    /// Seconds since 1970.
    #[ts(type = "number")]
    pub added_at: i64,
    #[ts(type = "number | null")]
    pub last_checked_at: Option<i64>,
    /// Why the last check failed, until one works.
    pub last_error: Option<PreviewError>,
    /// A check of it is running or waiting to.
    pub checking: bool,
    /// What checks found that's neither downloaded nor skipped, newest first.
    pub new_items: Vec<WatchedItem>,
    /// What was left out of a download, newest first: never new again, and still downloadable.
    pub skipped_items: Vec<WatchedItem>,
}

/// An item of a watched source, with what the history and the queue say about it now.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WatchedItem {
    pub video: VideoInfo,
    /// The day it was posted, when the site only says roughly (YouTube).
    pub approximate_date: Option<String>,
    /// When a check or the adding first met it, in seconds since 1970.
    #[ts(type = "number")]
    pub first_seen_at: i64,
    /// Its latest download, when the history has it.
    pub last_download: Option<PastDownload>,
    /// It's in the download queue and not done yet.
    pub in_queue: bool,
}

/// An item, by its site and ID as the history keys it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ItemKey {
    pub site: String,
    pub video_id: String,
}

impl ItemKey {
    fn of(video: &VideoInfo) -> Self {
        Self {
            site: video.site.clone(),
            video_id: video.video_id.clone(),
        }
    }
}

/// What counts as new when a source is added: asked each time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum FirstItems {
    /// Nothing yet: only what's posted after it's added.
    FromNow,
    /// Its first `count` items as listed (the newest, for a channel or an account).
    Latest { count: u32 },
    /// Everything the listing showed.
    All,
}

/// One item of the listing a source is added from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListedVideo {
    pub video: VideoInfo,
    pub approximate_date: Option<String>,
}

/// Watch, from the preview card's picker: the listing as it was read, and what to do with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NewWatchedSource {
    pub url: String,
    pub kind: ListingKind,
    pub name: String,
    pub site_name: String,
    pub thumbnail_url: Option<String>,
    pub items: Vec<ListedVideo>,
    pub first: FirstItems,
    pub folder: Option<String>,
    pub preset_id: Option<String>,
}

pub struct WatchedSources {
    store: WatchedStore,
    history: Arc<History>,
    queue: Arc<DownloadQueue>,
    presets: Arc<PresetManager>,
    settings: Arc<SettingsStore>,
    reader: Arc<dyn ListingReader>,
    listener: Arc<dyn WatchedListener>,
    /// Sources whose check is running or waiting.
    checking: Mutex<HashSet<i64>>,
    to_check: Mutex<Sender<i64>>,
}

impl WatchedSources {
    pub fn open(services: WatchedServices) -> Arc<Self> {
        let (sender, receiver) = mpsc::channel();
        let sources = Arc::new(Self {
            store: WatchedStore::new(services.database),
            history: services.history,
            queue: services.queue,
            presets: services.presets,
            settings: services.settings,
            reader: services.reader,
            listener: services.listener,
            checking: Mutex::default(),
            to_check: Mutex::new(sender),
        });
        let checking = Arc::downgrade(&sources);
        thread::spawn(move || run_checks(&checking, &receiver));
        sources
    }

    /// Every source, in the order added, with its new and skipped items.
    pub fn list(&self) -> Result<Vec<WatchedSource>, WatchedError> {
        let checking = lock(&self.checking).clone();
        let sources = self.store.sources()?;
        sources
            .into_iter()
            .map(|source| {
                let items = self.store.items(source.id)?;
                let is_checking = checking.contains(&source.id);
                Ok(self.present(source, items, is_checking))
            })
            .collect()
    }

    /// Pins a listing read by the preview card. Items the history or the queue has are known from the start; of the
    /// rest, `first` says which are new.
    pub fn add(&self, new: NewWatchedSource) -> Result<i64, WatchedError> {
        if self.store.find_by_url(&new.url)?.is_some() {
            return Err(WatchedError::AlreadyWatched);
        }
        let now = now_seconds();
        let mut new_count = match new.first {
            FirstItems::FromNow => 0,
            FirstItems::Latest { count } => count as usize,
            FirstItems::All => usize::MAX,
        };
        let mut seen = HashSet::new();
        let mut items = Vec::new();
        for (position, listed) in new.items.into_iter().enumerate() {
            if !seen.insert(ItemKey::of(&listed.video)) {
                continue;
            }
            let known = self.downloaded_or_queued(&listed.video);
            let state = if !known && new_count > 0 {
                new_count -= 1;
                ItemState::New
            } else {
                ItemState::Seen
            };
            items.push(StoredItem {
                video: listed.video,
                approximate_date: listed.approximate_date,
                state,
                first_seen_at: now,
                position: i64::try_from(position).unwrap_or(i64::MAX),
            });
        }
        let id = self.store.insert_source(
            &StoredSource {
                id: 0,
                url: new.url,
                kind: new.kind,
                name: new.name,
                site_name: new.site_name,
                thumbnail_url: new.thumbnail_url,
                folder: blank_to_none(new.folder),
                preset_id: blank_to_none(new.preset_id),
                added_at: now,
                last_checked_at: Some(now),
                last_error: None,
            },
            &items,
        )?;
        self.notify();
        Ok(id)
    }

    /// Changes a source's Save In and Videos Start As.
    pub fn edit(
        &self,
        id: i64,
        folder: Option<String>,
        preset_id: Option<String>,
    ) -> Result<(), WatchedError> {
        self.store
            .set_choices(id, blank_to_none(folder), blank_to_none(preset_id))?;
        self.notify();
        Ok(())
    }

    /// Stops watching a source, forgetting its items. What was queued from it stays in the queue.
    pub fn remove(&self, id: i64) -> Result<(), WatchedError> {
        self.store.delete_source(id)?;
        self.notify();
        Ok(())
    }

    /// Check Now: reads the source again, on the checks' thread, after the checks before it.
    pub fn check(&self, id: i64) -> Result<(), WatchedError> {
        self.store.source(id)?.ok_or(WatchedError::NotFound)?;
        if !lock(&self.checking).insert(id) {
            return Ok(());
        }
        let _ = lock(&self.to_check).send(id);
        self.notify();
        Ok(())
    }

    /// Download Selected: queues these items with the source's folder and preset. With `skip_rest`, the new items not
    /// picked become skipped. How many were queued.
    pub fn download(
        &self,
        id: i64,
        picked: &[ItemKey],
        skip_rest: bool,
    ) -> Result<usize, WatchedError> {
        let source = self.store.source(id)?.ok_or(WatchedError::NotFound)?;
        let items = self.store.items(id)?;
        let picked: HashSet<&ItemKey> = picked.iter().collect();
        let settings = self.settings.get();
        let download_folder = settings.downloads.resolved_folder();
        let picks: Vec<QueuePick> = items
            .iter()
            .filter(|item| picked.contains(&ItemKey::of(&item.video)))
            .map(|item| self.pick_for(&source, &item.video, &download_folder))
            .collect();
        let added = self
            .queue
            .add_many(picks)
            .map_err(|error| WatchedError::Queue { error })?;
        let queued: Vec<ItemKey> = picked.into_iter().cloned().collect();
        self.store.set_state(id, &queued, ItemState::Queued)?;
        if skip_rest {
            let rest: Vec<ItemKey> = items
                .iter()
                .filter(|item| item.state == ItemState::New)
                .map(|item| ItemKey::of(&item.video))
                .filter(|key| !queued.contains(key))
                .collect();
            self.store.set_state(id, &rest, ItemState::Skipped)?;
        }
        self.notify();
        Ok(added)
    }

    /// Skip All: every new item of the source becomes skipped.
    pub fn skip_new(&self, id: i64) -> Result<(), WatchedError> {
        let items = self.store.items(id)?;
        let new: Vec<ItemKey> = items
            .iter()
            .filter(|item| item.state == ItemState::New)
            .map(|item| ItemKey::of(&item.video))
            .collect();
        self.store.set_state(id, &new, ItemState::Skipped)?;
        self.notify();
        Ok(())
    }

    /// How many new items wait across every source, for the sidebar's count.
    pub fn new_count(&self) -> usize {
        self.list().map_or(0, |sources| {
            sources.iter().map(|source| source.new_items.len()).sum()
        })
    }

    /// A download of `video` from `source`: its folder, beating the rules, with their placeholders filled, and its
    /// preset, else the one a rule picks, else the default. A post or a file host's files have no preset.
    fn pick_for(
        &self,
        source: &StoredSource,
        video: &VideoInfo,
        download_folder: &Path,
    ) -> QueuePick {
        let preset_id = (video.post.is_none() && video.files.is_none()).then(|| {
            source
                .preset_id
                .clone()
                .filter(|id| self.presets.get(id).is_some())
                .or_else(|| {
                    let rules = self.settings.get().rules.list;
                    rules::start_preset(&rules, video, |id| self.presets.get(id).is_some())
                        .map(|picked| picked.preset_id)
                })
                .unwrap_or_else(|| self.presets.overview().default_id)
        });
        let folder = source.folder.as_deref().map(|template| {
            let preset = preset_id.as_deref().and_then(|id| self.presets.get(id));
            let subject = RuleSubject {
                video,
                preset: preset.as_ref(),
            };
            rules::filled_folder(template, &subject, download_folder)
                .display()
                .to_string()
        });
        QueuePick {
            video: video.clone(),
            preset_id,
            overwrite: false,
            folder,
        }
    }

    /// Reads a source and keeps what's new. Runs on the checks' thread.
    fn run_check(&self, id: i64) {
        let Ok(Some(source)) = self.store.source(id) else {
            return;
        };
        let read = match self.queue.site_wait(&source.url) {
            Some(wait) => Err(PreviewError::TooManyRequests {
                detail: String::new(),
                site: Some(wait.site),
                until: Some(wait.until),
            }),
            None => self.reader.read(&source.url, &CancelToken::new()),
        };
        let outcome = match read {
            Ok(found) => {
                self.queue.site_worked(&source.url);
                self.keep_new(&source, found)
            }
            Err(PreviewError::TooManyRequests { detail, .. }) => {
                let wait = self.queue.slow_down(&source.url);
                Err(PreviewError::TooManyRequests {
                    detail,
                    site: wait.as_ref().map(|wait| wait.site.clone()),
                    until: wait.map(|wait| wait.until),
                })
            }
            Err(error) => Err(error),
        };
        let _ = self
            .store
            .set_checked(id, now_seconds(), outcome.err().as_ref());
    }

    /// Stores the items of `found` that the source, the history and the queue don't know, as new, and the source's
    /// name and picture as the listing has them now.
    fn keep_new(&self, source: &StoredSource, found: Listing) -> Result<(), PreviewError> {
        let storage = |error: WatchedError| PreviewError::Failed {
            detail: error.to_string(),
        };
        let known: HashSet<ItemKey> = self
            .store
            .items(source.id)
            .map_err(storage)?
            .iter()
            .map(|item| ItemKey::of(&item.video))
            .collect();
        let is_known = |item: &listing::ListedItem| {
            known.contains(&ItemKey::of(&item.video)) || self.downloaded_or_queued(&item.video)
        };
        let new = if found.kind == ListingKind::Playlist {
            // A playlist is often oldest first, so it's compared whole.
            found.items.iter().filter(|item| !is_known(item)).collect()
        } else {
            listing::before_known_ground(found.items.iter(), |item| is_known(item)).new
        };
        let now = now_seconds();
        let mut seen = HashSet::new();
        let items: Vec<StoredItem> = new
            .into_iter()
            .filter(|item| seen.insert(ItemKey::of(&item.video)))
            .enumerate()
            .map(|(position, item)| StoredItem {
                video: item.video.clone(),
                approximate_date: item.approximate_date.clone(),
                state: ItemState::New,
                first_seen_at: now,
                position: i64::try_from(position).unwrap_or(i64::MAX),
            })
            .collect();
        self.store
            .insert_items(
                source.id,
                &items,
                &found.title,
                found.thumbnail_url.as_deref(),
            )
            .map_err(storage)
    }

    fn downloaded_or_queued(&self, video: &VideoInfo) -> bool {
        self.queue.contains(&video.site, &video.video_id)
            || self
                .history
                .last_download(&video.site, &video.video_id)
                .ok()
                .flatten()
                .is_some()
    }

    fn present(
        &self,
        source: StoredSource,
        items: Vec<StoredItem>,
        checking: bool,
    ) -> WatchedSource {
        let mut new_items = Vec::new();
        let mut skipped_items = Vec::new();
        for item in items {
            let list = match item.state {
                ItemState::New => &mut new_items,
                ItemState::Skipped => &mut skipped_items,
                ItemState::Queued | ItemState::Seen => continue,
            };
            list.push(WatchedItem {
                last_download: self
                    .history
                    .last_download(&item.video.site, &item.video.video_id)
                    .ok()
                    .flatten(),
                in_queue: self.queue.contains(&item.video.site, &item.video.video_id),
                video: item.video,
                approximate_date: item.approximate_date,
                first_seen_at: item.first_seen_at,
            });
        }
        WatchedSource {
            id: source.id,
            url: source.url,
            kind: source.kind,
            name: source.name,
            site_name: source.site_name,
            thumbnail_url: source.thumbnail_url,
            folder: source.folder,
            preset_id: source.preset_id,
            added_at: source.added_at,
            last_checked_at: source.last_checked_at,
            last_error: source.last_error,
            checking,
            new_items,
            skipped_items,
        }
    }

    fn notify(&self) {
        if let Ok(sources) = self.list() {
            self.listener.changed(&sources);
        }
    }
}

/// The checks' thread: one source at a time, in the order asked, while the sources exist.
fn run_checks(sources: &Weak<WatchedSources>, receiver: &Receiver<i64>) {
    while let Ok(id) = receiver.recv() {
        let Some(sources) = sources.upgrade() else {
            return;
        };
        sources.run_check(id);
        lock(&sources.checking).remove(&id);
        sources.notify();
    }
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn now_seconds() -> i64 {
    crate::site_limits::now_ms() / 1000
}
