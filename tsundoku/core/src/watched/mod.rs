//! Watched sources: channels, playlists and accounts pinned to check for what's new (the Watched view). Each keeps
//! every item a check or its adding has met (`watched_items`), so what was skipped or downloaded never comes back as
//! new. A check reads the source's listing (`ListingReader`), newest first for a channel or an account, and stops past
//! what it knows (`listing::before_known_ground`), which the reader is told so it can stop reading there too (an
//! account, read its site's first answer first: `listing::read_for_check`); a playlist is compared
//! whole. New items wait on the source's card
//! until they're downloaded, with the source's own folder and preset, or skipped.
//!
//! Checks run by hand for now (Check Now, Check All), on a thread per site (`sites::site_of`, the key the queue's
//! waits use): one at a time on each site, in the order asked, and side by side across sites, so an X account doesn't
//! wait for the Instagram accounts asked before it. They keep to a site's "too many requests" wait as previews do.
//!
//! A source shows its listing's picture, which each check keeps current, unless one of its own pictures was picked for
//! it by hand, with the area of it to show (`picture`).
//!
//! What a check finds starts ticked or unticked on the source's card (`NewItemsStart`): the source's own choice, else
//! Settings → Watched's (`WatchedSettings`). Only the UI reads it: the core never downloads an item nobody picked.
//!
//! A source can say whether the archives its downloads bring are unpacked, and where (`WatchedSource::unpack`), over
//! every rule and Settings → Archives. The queue asks when a download runs (`WatchedChoices`), by the page it was picked
//! from, so a change reaches the downloads already waiting, and a pick from the source's own list in the preview card
//! follows it too.

pub mod picture;
mod store;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, Weak};
use std::thread;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::cancel::CancelToken;
use crate::database::Database;
use crate::downloads::{DownloadError, DownloadQueue, QueuePick, QueueSource, SourceChoices};
use crate::history::{History, PastDownload};
use crate::listing::{self, Listing, ListingKind};
use crate::metadata::{PreviewError, VideoInfo};
use crate::presets::PresetManager;
use crate::rules::{self, RuleSubject, RuleUnpacking};
use crate::settings::SettingsStore;
use crate::sites;
use crate::sync::lock;
use picture::{PictureCrop, SourcePicture};
use store::{ItemState, StoredItem, StoredSource, WatchedStore};

/// The most pictures offered to pick from: the listing's, then the newest items'.
const PICTURE_CHOICES_AT_MOST: usize = 500;

/// Reads a watched source's listing, as the preview card reads a channel, playlist or account.
pub trait ListingReader: Send + Sync {
    /// `reached` says whether the items read so far reach what the source saw before (`listing::before_known_ground`),
    /// so a reader that pays for each page, such as Instagram's, can stop there.
    fn read(
        &self,
        url: &str,
        cancel: &CancelToken,
        reached: &dyn Fn(&Listing) -> bool,
    ) -> Result<Listing, PreviewError>;
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
    /// Where the pictures picked by hand are kept: the data folder's `pictures\`.
    pub pictures_folder: PathBuf,
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
    /// A picture picked for the source couldn't be read from its site, or isn't a picture.
    #[error("couldn't get the picture: {detail}")]
    PictureUnavailable { detail: String },
    /// The picture isn't one of the source's, or there's no picture picked to crop.
    #[error("this picture isn't one of the source's")]
    UnknownPicture,
}

/// Whether what a check finds starts ticked on the source's card, to untick what isn't wanted (as before 0.2.22), or
/// unticked, to tick what is. An item downloaded before starts unticked either way.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NewItemsStart {
    #[default]
    Unticked,
    Ticked,
}

impl NewItemsStart {
    /// As `watched_sources.new_items_start` holds it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Unticked => "unticked",
            Self::Ticked => "ticked",
        }
    }

    /// `None` for a name this version doesn't know (from a newer one): Settings' choice applies.
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name {
            "unticked" => Some(Self::Unticked),
            "ticked" => Some(Self::Ticked),
            _ => None,
        }
    }
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
    /// The picture picked for it by hand (Edit → Picture), shown instead of `thumbnail_url`.
    pub picture: Option<SourcePicture>,
    /// Save In: where its downloads go, beating every rule, with the rules' placeholders. `None` lets the rules decide.
    pub folder: Option<String>,
    /// Videos Start As. `None` for the preset a rule picks, else the default.
    pub preset_id: Option<String>,
    /// New Items Start: whether what a check finds starts ticked. `None` for Settings → Watched's choice.
    pub new_items_start: Option<NewItemsStart>,
    /// Unpack Archives: whether the archives its downloads bring are unpacked, and where, over every rule and Settings
    /// → Archives. `None` leaves it to them.
    pub unpack: Option<RuleUnpacking>,
    /// Seconds since 1970.
    #[ts(type = "number")]
    pub added_at: i64,
    #[ts(type = "number | null")]
    pub last_checked_at: Option<i64>,
    /// Why the last check failed, until one works.
    pub last_error: Option<PreviewError>,
    /// A check of it is running or waiting to.
    pub checking: bool,
    /// How many of its items the history has: what the history lists for it (`HistoryQuery::watched_source`).
    pub downloaded_count: u32,
    /// The day its newest item was posted (YYYY-MM-DD, roughly for YouTube), when any of them says.
    pub latest_post: Option<String>,
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

/// What's downloaded of the listing when a source is added: asked each time. Every item listed is known from then on,
/// so a check lists only what comes after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum FirstItems {
    /// Nothing: only what's posted after it's added, which a check lists as new.
    FromNow,
    /// Its first `count` items as listed (the newest, for a channel or an account), but those the history or the
    /// queue has.
    Latest { count: u32 },
    /// The items ticked in the picker, but those the queue has. One the history has is downloaded again, replacing its
    /// file when the preset it starts with is the one it was downloaded with, as the picker's Download Selected does.
    Picked { items: Vec<ItemKey> },
    /// Everything listed, but what the history or the queue has.
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
    pictures_folder: PathBuf,
    /// Sources whose check is running or waiting.
    checking: Mutex<HashSet<i64>>,
    /// Each site's checks' thread, by the site's key (`check_site`), started with its first check.
    to_check: Mutex<HashMap<String, Sender<i64>>>,
    /// Itself, for the checks' threads, which end once it's gone.
    this: Weak<WatchedSources>,
}

impl WatchedSources {
    pub fn open(services: WatchedServices) -> Arc<Self> {
        let sources = Arc::new_cyclic(|this| Self {
            store: WatchedStore::new(services.database),
            history: services.history,
            queue: services.queue,
            presets: services.presets,
            settings: services.settings,
            reader: services.reader,
            listener: services.listener,
            pictures_folder: services.pictures_folder,
            checking: Mutex::default(),
            to_check: Mutex::default(),
            this: this.clone(),
        });
        sources.settle_pictures();
        sources
    }

    /// Every source, in the order added, with its new and skipped items.
    pub fn list(&self) -> Result<Vec<WatchedSource>, WatchedError> {
        let checking = lock(&self.checking).clone();
        let sources = self.store.sources()?;
        let downloaded = self.store.downloaded_counts()?;
        sources
            .into_iter()
            .map(|source| {
                let items = self.store.items(source.id)?;
                let is_checking = checking.contains(&source.id);
                let count = downloaded.get(&source.id).copied().unwrap_or(0);
                Ok(self.present(source, items, is_checking, count))
            })
            .collect()
    }

    /// Pins a listing read by the preview card, and queues what `first` says with the source's choices, as Download
    /// Selected on its card would. Every item listed is known from then on: those queued as queued, the rest as seen.
    /// The items are queued before the source is kept, so one that can't be queued leaves nothing watched.
    pub fn add(&self, new: NewWatchedSource) -> Result<i64, WatchedError> {
        if self.store.find_by_url(&new.url)?.is_some() {
            return Err(WatchedError::AlreadyWatched);
        }
        let now = now_seconds();
        let picked: HashSet<&ItemKey> = match &new.first {
            FirstItems::Picked { items } => items.iter().collect(),
            _ => HashSet::new(),
        };
        let mut seen = HashSet::new();
        let mut items = Vec::new();
        for (position, listed) in new.items.into_iter().enumerate() {
            let key = ItemKey::of(&listed.video);
            if !seen.insert(key.clone()) {
                continue;
            }
            let queued = match &new.first {
                FirstItems::FromNow => false,
                FirstItems::Latest { count } => {
                    seen.len() <= *count as usize && !self.downloaded_or_queued(&listed.video)
                }
                FirstItems::Picked { .. } => {
                    picked.contains(&key)
                        && !self
                            .queue
                            .contains(&listed.video.site, &listed.video.video_id)
                }
                FirstItems::All => !self.downloaded_or_queued(&listed.video),
            };
            items.push(StoredItem {
                video: listed.video,
                approximate_date: listed.approximate_date,
                state: if queued {
                    ItemState::Queued
                } else {
                    ItemState::Seen
                },
                first_seen_at: now,
                position: i64::try_from(position).unwrap_or(i64::MAX),
            });
        }
        let source = StoredSource {
            id: 0,
            url: new.url,
            kind: new.kind,
            name: new.name,
            site_name: new.site_name,
            thumbnail_url: new.thumbnail_url,
            picture: None,
            folder: blank_to_none(new.folder),
            preset_id: blank_to_none(new.preset_id),
            new_items_start: None,
            unpack: None,
            added_at: now,
            last_checked_at: Some(now),
            last_error: None,
        };
        let download_folder = self.settings.get().downloads.resolved_folder();
        let picks: Vec<QueuePick> = items
            .iter()
            .filter(|item| item.state == ItemState::Queued)
            .map(|item| {
                let pick = self.pick_for(&source, &item.video, &download_folder);
                QueuePick {
                    overwrite: self.replaces_its_download(&pick),
                    ..pick
                }
            })
            .collect();
        if !picks.is_empty() {
            self.queue
                .add_many(picks, Some(queue_source(&source)))
                .map_err(|error| WatchedError::Queue { error })?;
        }
        let id = self.store.insert_source(&source, &items)?;
        self.notify();
        Ok(id)
    }

    /// Changes a source's Save In, Videos Start As, New Items Start (`None` for Settings') and Unpack Archives (`None`
    /// for the rules', else Settings').
    pub fn edit(
        &self,
        id: i64,
        folder: Option<String>,
        preset_id: Option<String>,
        new_items_start: Option<NewItemsStart>,
        unpack: Option<RuleUnpacking>,
    ) -> Result<(), WatchedError> {
        self.store.set_choices(
            id,
            blank_to_none(folder),
            blank_to_none(preset_id),
            new_items_start,
            unpack,
        )?;
        self.notify();
        Ok(())
    }

    /// Stops watching a source, forgetting its items and its picture. What was queued from it stays in the queue.
    pub fn remove(&self, id: i64) -> Result<(), WatchedError> {
        let picture = self.store.source(id)?.and_then(|source| source.picture);
        self.store.delete_source(id)?;
        if let Some(picture) = picture {
            picture::delete(&self.pictures_folder, &picture.file);
        }
        self.notify();
        Ok(())
    }

    /// The pictures a source can show instead of its listing's: the listing's own first, then each item's, newest
    /// first, a post's files one by one, each once, `PICTURE_CHOICES_AT_MOST` at most.
    pub fn picture_choices(&self, id: i64) -> Result<Vec<String>, WatchedError> {
        let source = self.store.source(id)?.ok_or(WatchedError::NotFound)?;
        let items = self.store.items(id)?;
        let mut seen = HashSet::new();
        let pictures = source
            .thumbnail_url
            .into_iter()
            .chain(items.into_iter().flat_map(|item| {
                let video = item.video;
                let files: Vec<String> = video
                    .post_files
                    .into_iter()
                    .filter_map(|file| file.thumbnail_url)
                    .collect();
                if files.is_empty() {
                    video.thumbnail_url.into_iter().collect()
                } else {
                    files
                }
            }));
        Ok(pictures
            .filter(|url| seen.insert(url.clone()))
            .take(PICTURE_CHOICES_AT_MOST)
            .collect())
    }

    /// Custom: shows one of the source's pictures (`picture_choices`) instead of its listing's, the `crop` of it. The
    /// picture is read with `read` and kept in the data folder, since sites' links expire; the one it replaces is
    /// deleted.
    pub fn pick_picture(
        &self,
        id: i64,
        url: &str,
        crop: PictureCrop,
        read: &dyn Fn(&str) -> Result<Vec<u8>, WatchedError>,
    ) -> Result<(), WatchedError> {
        if !self.picture_choices(id)?.iter().any(|choice| choice == url) {
            return Err(WatchedError::UnknownPicture);
        }
        let bytes = read(url)?;
        let file = picture::save(&self.pictures_folder, id, &bytes)?;
        let before = self.store.source(id)?.and_then(|source| source.picture);
        let picked = SourcePicture {
            file: file.clone(),
            crop: crop.checked(),
        };
        if let Err(error) = self.store.set_picture(id, Some(&picked)) {
            if before.as_ref().is_none_or(|before| before.file != file) {
                picture::delete(&self.pictures_folder, &file);
            }
            return Err(error);
        }
        if let Some(before) = before.filter(|before| before.file != file) {
            picture::delete(&self.pictures_folder, &before.file);
        }
        self.notify();
        Ok(())
    }

    /// Shows another area of the picture picked for the source.
    pub fn crop_picture(&self, id: i64, crop: PictureCrop) -> Result<(), WatchedError> {
        let source = self.store.source(id)?.ok_or(WatchedError::NotFound)?;
        let picture = source.picture.ok_or(WatchedError::UnknownPicture)?;
        self.store.set_picture(
            id,
            Some(&SourcePicture {
                file: picture.file,
                crop: crop.checked(),
            }),
        )?;
        self.notify();
        Ok(())
    }

    /// Automatic: the source shows its listing's picture again, and the one picked for it is deleted.
    pub fn reset_picture(&self, id: i64) -> Result<(), WatchedError> {
        let source = self.store.source(id)?.ok_or(WatchedError::NotFound)?;
        self.store.set_picture(id, None)?;
        if let Some(picture) = source.picture {
            picture::delete(&self.pictures_folder, &picture.file);
        }
        self.notify();
        Ok(())
    }

    /// Deletes what the pictures folder holds that no source shows: a source's removed while the app closed, a
    /// picture half written as it quit. Best effort: a folder that can't be read is left alone.
    fn settle_pictures(&self) {
        let Ok(shown) = self.store.picture_files() else {
            return;
        };
        let Ok(entries) = fs::read_dir(&self.pictures_folder) else {
            return;
        };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let half_written = name
                .strip_suffix(".part")
                .is_some_and(|name| picture::kind_of_file(name).is_some());
            if half_written {
                let _ = fs::remove_file(entry.path());
            } else if !shown.contains(&name) {
                picture::delete(&self.pictures_folder, &name);
            }
        }
    }

    /// Check Now: reads the source again, on its site's checks' thread, after the checks on that site before it.
    pub fn check(&self, id: i64) -> Result<(), WatchedError> {
        let source = self.store.source(id)?.ok_or(WatchedError::NotFound)?;
        if self.ask_check(&source) {
            self.notify();
        }
        Ok(())
    }

    /// Check All: reads each of these sources again, those on one site one after the other in the order given, after
    /// the checks on that site before them, and those on different sites side by side. A source removed meanwhile, or
    /// already waiting for its check, is left out.
    pub fn check_many(&self, ids: &[i64]) -> Result<(), WatchedError> {
        let watched: HashMap<i64, StoredSource> = self
            .store
            .sources()?
            .into_iter()
            .map(|source| (source.id, source))
            .collect();
        let mut asked = false;
        for source in ids.iter().filter_map(|id| watched.get(id)) {
            asked |= self.ask_check(source);
        }
        if asked {
            self.notify();
        }
        Ok(())
    }

    /// Puts a source's check on its site's checks' thread, starting that thread for the site's first check, unless the
    /// check waits there already. Whether it was put there.
    fn ask_check(&self, source: &StoredSource) -> bool {
        if !lock(&self.checking).insert(source.id) {
            return false;
        }
        let mut threads = lock(&self.to_check);
        let sender = threads.entry(check_site(&source.url)).or_insert_with(|| {
            let (sender, receiver) = mpsc::channel();
            let sources = self.this.clone();
            thread::spawn(move || run_checks(&sources, &receiver));
            sender
        });
        let _ = sender.send(source.id);
        true
    }

    /// Download Selected: queues the picked items with the source's folder and preset, and skips the new ones of
    /// `skipped` (the picker's other items, among those it shows). How many were queued.
    pub fn download(
        &self,
        id: i64,
        picked: &[ItemKey],
        skipped: &[ItemKey],
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
            .add_many(picks, Some(queue_source(&source)))
            .map_err(|error| WatchedError::Queue { error })?;
        let queued: Vec<ItemKey> = picked.iter().map(|&key| key.clone()).collect();
        self.store.set_state(id, &queued, ItemState::Queued)?;
        let skipped: HashSet<&ItemKey> =
            skipped.iter().filter(|key| !picked.contains(key)).collect();
        self.store
            .set_state(id, &new_among(&items, &skipped), ItemState::Skipped)?;
        self.notify();
        Ok(added)
    }

    /// All New Items' Download Selected: queues the picked items of every source's new ones, each with its own
    /// source's folder and preset, and skips the new ones of `skipped`. An item two sources found is queued once, from
    /// the first source added, and leaves both; one skipped is skipped in every source that has it. How many were
    /// queued.
    pub fn download_new(
        &self,
        picked: &[ItemKey],
        skipped: &[ItemKey],
    ) -> Result<usize, WatchedError> {
        let picked: HashSet<&ItemKey> = picked.iter().collect();
        let skipped: HashSet<&ItemKey> =
            skipped.iter().filter(|key| !picked.contains(key)).collect();
        let settings = self.settings.get();
        let download_folder = settings.downloads.resolved_folder();
        let mut queued_here = HashSet::new();
        let mut added = 0;
        for source in self.store.sources()? {
            let items = self.store.items(source.id)?;
            let mut picks = Vec::new();
            let mut taken = Vec::new();
            for item in items.iter().filter(|item| item.state == ItemState::New) {
                let key = ItemKey::of(&item.video);
                if !picked.contains(&key) {
                    continue;
                }
                if queued_here.insert(key.clone()) {
                    picks.push(self.pick_for(&source, &item.video, &download_folder));
                }
                taken.push(key);
            }
            if !picks.is_empty() {
                added += self
                    .queue
                    .add_many(picks, Some(queue_source(&source)))
                    .map_err(|error| WatchedError::Queue { error })?;
            }
            self.store.set_state(source.id, &taken, ItemState::Queued)?;
            self.store
                .set_state(source.id, &new_among(&items, &skipped), ItemState::Skipped)?;
        }
        self.notify();
        Ok(added)
    }

    /// Skip All: these items of the source (what its picker shows) become skipped, those still new.
    pub fn skip(&self, id: i64, keys: &[ItemKey]) -> Result<(), WatchedError> {
        let items = self.store.items(id)?;
        let keys: HashSet<&ItemKey> = keys.iter().collect();
        self.store
            .set_state(id, &new_among(&items, &keys), ItemState::Skipped)?;
        self.notify();
        Ok(())
    }

    /// The items the source watching the listing at `page` keeps as skipped, for that listing's picker in Pending,
    /// which starts them unticked as its own Skipped Before does. None when no source watches it.
    pub fn skipped_at(&self, page: &str) -> Result<Vec<ItemKey>, WatchedError> {
        let Some(id) = self.store.find_by_url(page)? else {
            return Ok(Vec::new());
        };
        Ok(self
            .store
            .items(id)?
            .iter()
            .filter(|item| item.state == ItemState::Skipped)
            .map(|item| ItemKey::of(&item.video))
            .collect())
    }

    /// The listing at `page` was downloaded from in Pending's picker, these items left unticked: those still new
    /// become skipped in the source watching it, as Download in its own picker does. Nothing when no source watches it.
    pub fn skip_at(&self, page: &str, keys: &[ItemKey]) -> Result<(), WatchedError> {
        match self.store.find_by_url(page)? {
            Some(id) => self.skip(id, keys),
            None => Ok(()),
        }
    }

    /// All New Items' Skip All: these items (what it shows) become skipped in every source where they're still new.
    pub fn skip_new(&self, keys: &[ItemKey]) -> Result<(), WatchedError> {
        let keys: HashSet<&ItemKey> = keys.iter().collect();
        for source in self.store.sources()? {
            let items = self.store.items(source.id)?;
            self.store
                .set_state(source.id, &new_among(&items, &keys), ItemState::Skipped)?;
        }
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
                    rules::start_preset(&rules, video, None, |id| self.presets.get(id).is_some())
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

    /// Whether the pick downloads its video again with the preset of its latest download, so its file replaces the one
    /// made then.
    fn replaces_its_download(&self, pick: &QueuePick) -> bool {
        let Some(preset) = pick
            .preset_id
            .as_deref()
            .and_then(|id| self.presets.get(id))
        else {
            return false;
        };
        self.history
            .last_download(&pick.video.site, &pick.video.video_id)
            .ok()
            .flatten()
            .is_some_and(|last| last.preset == preset.name)
    }

    /// Reads a source and keeps what's new. Runs on its site's checks' thread.
    fn run_check(&self, id: i64) {
        let Ok(Some(source)) = self.store.source(id) else {
            return;
        };
        let outcome = self.known_items(id).and_then(|known| {
            let is_known = |item: &listing::ListedItem| self.is_known(&known, item);
            // Where the check can stop: past what it knows, as `keep_new` takes it. A playlist is read whole.
            let reached = |found: &Listing| {
                found.kind != ListingKind::Playlist
                    && listing::before_known_ground(found.items.iter(), |item| is_known(item))
                        .reached_known
            };
            let read = match self.queue.site_wait(&source.url) {
                Some(wait) => Err(PreviewError::TooManyRequests {
                    detail: String::new(),
                    site: Some(wait.site),
                    until: Some(wait.until),
                }),
                None => self.reader.read(&source.url, &CancelToken::new(), &reached),
            };
            match read {
                Ok(found) => {
                    self.queue.site_worked(&source.url);
                    self.keep_new(&source, &found, |item| is_known(item))
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
            }
        });
        let _ = self
            .store
            .set_checked(id, now_seconds(), outcome.err().as_ref());
    }

    /// The items a source has met, by key.
    fn known_items(&self, id: i64) -> Result<HashSet<ItemKey>, PreviewError> {
        Ok(self
            .store
            .items(id)
            .map_err(storage_failure)?
            .iter()
            .map(|item| ItemKey::of(&item.video))
            .collect())
    }

    /// Whether a check counts an item as seen before: the source met it, or the history or the queue has it.
    fn is_known(&self, known: &HashSet<ItemKey>, item: &listing::ListedItem) -> bool {
        known.contains(&ItemKey::of(&item.video)) || self.downloaded_or_queued(&item.video)
    }

    /// Stores the items of `found` that aren't known (`is_known`), as new, and the source's name and picture as the
    /// listing has them now. The items it has without a picture take the one read now, from the listing's items or
    /// from what it read past them (`Listing::also_read`).
    fn keep_new(
        &self,
        source: &StoredSource,
        found: &Listing,
        is_known: impl Fn(&listing::ListedItem) -> bool,
    ) -> Result<(), PreviewError> {
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
        let pictured: Vec<&VideoInfo> = found
            .items
            .iter()
            .map(|item| &item.video)
            .chain(&found.also_read)
            .filter(|video| video.thumbnail_url.is_some())
            .collect();
        self.store
            .add_pictures(source.id, &pictured)
            .map_err(storage_failure)?;
        self.store
            .insert_items(
                source.id,
                &items,
                &found.title,
                found.thumbnail_url.as_deref(),
            )
            .map_err(storage_failure)
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
        downloaded_count: u32,
    ) -> WatchedSource {
        let latest_post = items
            .iter()
            .filter_map(|item| {
                item.video
                    .upload_date
                    .as_ref()
                    .or(item.approximate_date.as_ref())
            })
            .max()
            .cloned();
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
            picture: source.picture,
            folder: source.folder,
            preset_id: source.preset_id,
            new_items_start: source.new_items_start,
            unpack: source.unpack,
            added_at: source.added_at,
            last_checked_at: source.last_checked_at,
            last_error: source.last_error,
            checking,
            downloaded_count,
            latest_post,
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

/// The watched sources' own choices that the queue asks when a download runs (`SourceChoices`), read from the
/// database alone, so the queue can have them before the watched sources open.
pub struct WatchedChoices {
    store: WatchedStore,
}

impl WatchedChoices {
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            store: WatchedStore::new(database),
        }
    }
}

impl SourceChoices for WatchedChoices {
    /// The Unpack Archives of the watched source whose page the download was picked from: from its card, from All New
    /// Items, or from its list in the preview card. `None` once it's no longer watched, or when it can't be read.
    fn unpacking(&self, source: &QueueSource) -> Option<RuleUnpacking> {
        self.store.unpacking_of(&source.url).ok().flatten()
    }
}

/// A check that couldn't read or keep the source's items, as the check's failure.
fn storage_failure(error: WatchedError) -> PreviewError {
    PreviewError::Failed {
        detail: error.to_string(),
    }
}

/// A site's checks' thread: one of its sources at a time, in the order asked, while the sources exist.
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

/// The site whose checks' thread checks the source at `url`: the key the queue's waits use (`sites::site_of`), so a
/// site's pacing and "too many requests" wait hold across its sources (x.com and twitter.com are one site). A link
/// without a site goes on the thread of those without one.
fn check_site(url: &str) -> String {
    sites::site_of(url).map(|site| site.key).unwrap_or_default()
}

/// The source a download from a watched source is queued with, so the Queue shows it with the others picked from it.
fn queue_source(source: &StoredSource) -> QueueSource {
    QueueSource {
        url: source.url.clone(),
        kind: source.kind,
        name: source.name.clone(),
        site_name: source.site_name.clone(),
        thumbnail_url: source.thumbnail_url.clone(),
    }
}

/// The keys of `items` that are still new and among `keys`, so a skip never touches one queued or seen meanwhile.
fn new_among(items: &[StoredItem], keys: &HashSet<&ItemKey>) -> Vec<ItemKey> {
    items
        .iter()
        .filter(|item| item.state == ItemState::New)
        .map(|item| ItemKey::of(&item.video))
        .filter(|key| keys.contains(key))
        .collect()
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn now_seconds() -> i64 {
    crate::site_limits::now_ms() / 1000
}
