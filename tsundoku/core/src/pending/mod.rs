//! Links waiting to be started: pasted or dropped several at once, or copied in another app while the clipboard
//! watcher is on (`watcher`), read one at a time in the background so each shows what it is, then started into the
//! download queue one by one or all at once. Kept in `tsundoku.db` (`pending_links`), so they come back after a
//! restart.
//!
//! A copied link is held to a stricter standard than one added by hand, since nobody chose it for Tsundoku: yt-dlp
//! reads only sites it knows, and a link nothing can download, or that's downloaded or in the queue already, leaves
//! without a word. With Download Right Away on, the rest start once read.

pub mod files;
mod store;
pub mod watcher;

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, Hasher};
use std::sync::{Arc, Condvar, Mutex, PoisonError, Weak};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use self::store::PendingStore;
use crate::cancel::CancelToken;
use crate::database::Database;
use crate::downloads::{DownloadError, DownloadQueue};
use crate::history::History;
use crate::metadata::{self, Fetched, PreviewError, VideoInfo};
use crate::passwords::Password;
use crate::presets::{Preset, PresetError, PresetManager};
use crate::preview::VideoPreview;
use crate::rules::{self, RuleSubject};
use crate::settings::{AddingSettings, PauseRange, SettingsStore};
use crate::site_limits::SiteWait;
use crate::sites::{self, PacedSite};
use crate::sync::lock;

/// How long the reading thread waits when there's nothing to read before looking again. Adding a link wakes it at
/// once.
const IDLE_WAIT: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PendingState {
    /// Waiting its turn to be read.
    Waiting,
    /// Being read now.
    Reading,
    /// Read: `preview` says what it is.
    Ready,
    /// Couldn't be read: `error` says why.
    Failed,
    /// Its link needs a password, which the person enters on its row (`PendingLinks::unlock`); `error` says whether one
    /// was tried. It isn't failed: it never leaves by itself, and it starts once a password opens it if it was started.
    NeedsPassword,
}

/// A link waiting to be started, as the Pending view shows it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PendingLink {
    #[ts(type = "number")]
    pub id: i64,
    pub link: String,
    pub state: PendingState,
    /// Once read: what the link is, whether it was downloaded before and whether it's in the queue, as the preview
    /// card shows them.
    pub preview: Option<VideoPreview>,
    pub error: Option<PreviewError>,
    /// The preset picked for this link, or `None` for the one picked for them all. A post has none.
    pub preset_id: Option<String>,
    /// A folder picked for this link instead of the one the rules give, as a full path.
    pub folder: Option<String>,
    /// A rule picked for this link, by its ID, which decides everything it sets (its folder, preset and file name)
    /// before any other rule, whatever its conditions (`rules::destination`). `None` for the rules as usual.
    pub rule_id: Option<String>,
    /// Started before it was read: it joins the queue once it's read, unless it was downloaded before.
    pub start_when_read: bool,
    /// In seconds since 1970.
    #[ts(type = "number")]
    pub added_at: i64,
    /// Copied in another app and caught by the clipboard watcher. Until it's read, the UI only counts it, since it may
    /// turn out to be nothing to download and leave.
    pub caught: bool,
    /// Read from a file dropped on the window (`files`). Until it's read, the UI only counts it, as a copied link.
    pub from_file: bool,
    /// Sent by a page's Click'n'Load button (`click_n_load`). Until it's read, the UI only counts it, as a copied
    /// link.
    pub from_page: bool,
    /// While it waits to be read, the wait of its site, which answered "too many requests".
    pub hold: Option<SiteWait>,
}

/// What adding links did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AddedLinks {
    pub added: u32,
    /// Links that were already waiting, left as they were.
    pub already_pending: u32,
}

/// Every link in `text`, in order and each once: words that are http or https links, or look like one without
/// "https://" ("youtu.be/…"), with brackets, quotes and a sentence's trailing punctuation around them left out. Other
/// words are skipped, so a pasted paragraph gives only its links.
pub fn links_in(text: &str) -> Vec<String> {
    let mut links: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        let word = word
            .trim_start_matches(['<', '(', '[', '{', '"', '\''])
            .trim_end_matches(['>', ')', ']', '}', '"', '\'', ',', ';', '!', '?']);
        if let Some(link) = metadata::normalize_link(word)
            && !links.contains(&link)
        {
            links.push(link);
        }
    }
    links
}

/// The scheme of the links that send a page to Tsundoku: the browser bookmark in Settings → Adding Links opens
/// `tsundoku://add?url=…` with the page it's on, and Windows starts Tsundoku with it, or hands it to the running one.
pub const SEND_SCHEME: &str = "tsundoku";

/// The links a `tsundoku://add?url=…` link sends, each `url` once (`links_in`). Any other link sends none.
pub fn links_in_sent_link(link: &str) -> Vec<String> {
    let Ok(parsed) = reqwest::Url::parse(link) else {
        return Vec::new();
    };
    if parsed.scheme() != SEND_SCHEME || parsed.host_str() != Some("add") {
        return Vec::new();
    }
    let sent: Vec<String> = parsed
        .query_pairs()
        .filter(|(key, _)| key == "url")
        .map(|(_, value)| value.into_owned())
        .collect();
    links_in(&sent.join("\n"))
}

/// Reads what a link is, as the preview card does (`preview::ToolReader`). Tests use a fake.
pub trait LinkReader: Send + Sync {
    /// Without `any_page`, yt-dlp reads only sites it knows (`FetchTools::any_page`), turning others down at once.
    /// `password` opens a link that needs one.
    fn read(
        &self,
        link: &str,
        any_page: bool,
        password: Option<&Password>,
        cancel: &CancelToken,
    ) -> Result<Fetched, PreviewError>;
}

/// Told when the pending links change, to pass them on to the UI.
pub trait PendingListener: Send + Sync {
    fn changed(&self, links: &[PendingLink]);

    /// A link copied in another app was read and stays: waiting in the pending links, or `started` into the queue.
    /// The app notifies the user while its window is hidden.
    fn copied_link_read(&self, _video: &VideoInfo, _started: bool) {}

    /// How links get added changed, from Settings or the tray's Watch Clipboard.
    fn options_changed(&self, _options: &AddingSettings) {}
}

/// What the pending links use.
pub struct PendingServices {
    pub database: Arc<Database>,
    pub history: Arc<History>,
    pub queue: Arc<DownloadQueue>,
    pub presets: Arc<PresetManager>,
    pub settings: Arc<SettingsStore>,
    pub reader: Arc<dyn LinkReader>,
    pub listener: Arc<dyn PendingListener>,
}

/// A link as it's kept: what was read rather than the preview, whose duplicate warning changes as downloads finish.
#[derive(Debug, Clone, PartialEq)]
struct Entry {
    id: i64,
    link: String,
    state: PendingState,
    fetched: Option<Fetched>,
    error: Option<PreviewError>,
    /// The preset picked for this link. For one started before it was read, the one picked for them all then, which
    /// a rule's comes before.
    preset_id: Option<String>,
    /// A folder picked for this link instead of the one the rules give.
    folder: Option<String>,
    /// A rule picked for this link, which decides what it sets before any other.
    rule_id: Option<String>,
    start_when_read: StartWhenRead,
    added_at: i64,
    origin: LinkOrigin,
    /// The password that opens the link: typed on its row, or the history's for the link once it opened it.
    password: Option<Password>,
}

/// Where a link came from, which decides how it's read and whether it stays once read (`leaves_once_read`). Kept in
/// `pending_links.caught` as 0 to 3, so a version from before 0.1.54 reads a link from a file as copied, and one from
/// before 0.1.129 a link a page sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkOrigin {
    /// Pasted, dropped as text, or sent by the clipboard shortcut or a browser.
    Added,
    /// Copied in another app and caught by the clipboard watcher.
    Copied,
    /// Read from a file dropped on the window (`files`), such as a bookmarks export.
    File,
    /// Sent by a page's Click'n'Load button (`click_n_load`): kept as a link from a file is.
    Page,
}

/// How long the pending-changed notices wait after one before sending the next, so a burst of changes (thousands of
/// links added or read) sends a few, each with the list as it is then. Longer for a long list, which takes longer to
/// put together and to show.
fn change_gap(links: usize) -> Duration {
    if links > 500 {
        Duration::from_secs(1)
    } else {
        Duration::from_millis(250)
    }
}

/// When the last pending-changed notice went out, and whether one is due once the gap is up.
#[derive(Default)]
struct ChangeNotices {
    last_sent: Option<Instant>,
    due: bool,
}

/// Whether a link not read yet starts once it's read. Kept in `pending_links.start_when_read` as 0, 1 and 2, so a
/// version from before 0.1.35 reads a sent link as started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartWhenRead {
    /// It waits to be started.
    No,
    /// Started before it was read.
    Yes,
    /// Sent from outside the window (the clipboard shortcut, a browser): it starts once read if Download Right Away
    /// says so for what it turns out to be, by a rule or Settings, as a copied link does.
    IfRightAway,
}

#[derive(Default)]
struct Contents {
    entries: Vec<Entry>,
    /// The link being read, and what stops it.
    reading: Option<(i64, CancelToken)>,
    /// When the next link on each site that limits how fast it's read (Instagram, and X, Pixiv or Tumblr with a pause
    /// set) may be read.
    paced_until: HashMap<PacedSite, Instant>,
    /// What the last Remove took, oldest first, for Undo to put back.
    removed: Vec<Entry>,
}

pub struct PendingLinks {
    state: Mutex<Contents>,
    /// Wakes the reading thread when there's a link to read.
    wake: Condvar,
    store: PendingStore,
    history: Arc<History>,
    queue: Arc<DownloadQueue>,
    presets: Arc<PresetManager>,
    settings: Arc<SettingsStore>,
    reader: Arc<dyn LinkReader>,
    listener: Arc<dyn PendingListener>,
    notices: Mutex<ChangeNotices>,
    /// Wakes the thread that sends a notice once its gap is up.
    notices_due: Condvar,
}

impl PendingLinks {
    /// Loads the saved links and starts reading those not read yet, one at a time. A link that was being read when
    /// the app closed is read again.
    pub fn open(services: PendingServices) -> Result<Arc<Self>, DownloadError> {
        let store = PendingStore::new(services.database);
        let entries = store.load()?;
        let links = Arc::new(Self {
            state: Mutex::new(Contents {
                entries,
                ..Contents::default()
            }),
            wake: Condvar::new(),
            store,
            history: services.history,
            queue: services.queue,
            presets: services.presets,
            settings: services.settings,
            reader: services.reader,
            listener: services.listener,
            notices: Mutex::default(),
            notices_due: Condvar::new(),
        });
        let reading = Arc::downgrade(&links);
        thread::spawn(move || read_links(&reading));
        let notifying = Arc::downgrade(&links);
        thread::spawn(move || send_due_notices(&notifying));
        Ok(links)
    }

    /// Every link, oldest first, with what the history and the queue say about it now.
    pub fn list(&self) -> Vec<PendingLink> {
        let entries = lock(&self.state).entries.clone();
        entries
            .into_iter()
            .map(|entry| self.present(entry))
            .collect()
    }

    /// Adds links to read, skipping those already here and anything that isn't a link.
    pub fn add(&self, links: &[String]) -> Result<AddedLinks, DownloadError> {
        self.insert(links, LinkOrigin::Added, StartWhenRead::No)
    }

    /// Adds the links read from files dropped on the window (`files`), checked as copied links are: those with
    /// nothing to download leave once read, without a word (`leaves_once_read`).
    pub fn add_from_files(&self, links: &[String]) -> Result<AddedLinks, DownloadError> {
        self.insert(links, LinkOrigin::File, StartWhenRead::No)
    }

    /// Adds the links a page's Click'n'Load button sent (`click_n_load`), kept as links from a dropped file are: those
    /// with nothing to download leave once read, without a word, and none starts by itself, whatever Download Right
    /// Away says, since the page sending them can't always be told for sure.
    pub fn add_from_page(&self, links: &[String]) -> Result<AddedLinks, DownloadError> {
        self.insert(links, LinkOrigin::Page, StartWhenRead::No)
    }

    /// Adds links copied in another app (`watcher`), skipping those already here and those the queue or the history
    /// has by the same address. Reading them tells whether anything can download them, and whether it was
    /// downloaded before by another address.
    pub fn catch(&self, links: &[String]) -> Result<AddedLinks, DownloadError> {
        let new: Vec<String> = links
            .iter()
            .filter(|link| {
                !self.queue.has_link(link) && !self.history.has_link(link).unwrap_or(false)
            })
            .cloned()
            .collect();
        self.insert(&new, LinkOrigin::Copied, StartWhenRead::No)
    }

    /// Adds links sent from outside the window: the clipboard shortcut's, and the page a browser sends
    /// (`links_in_sent_link`). As if pasted, so each shows even when nothing can download it, but started once read
    /// when Download Right Away is on for it (by a rule or Settings), unless it was downloaded before.
    pub fn add_sent(&self, links: &[String]) -> Result<AddedLinks, DownloadError> {
        self.insert(links, LinkOrigin::Added, StartWhenRead::IfRightAway)
    }

    /// Starts every link Start All would (`starts_with_all`), each video with its own preset or `preset_id`. How many
    /// were started.
    pub fn start_all(&self, preset_id: &str) -> Result<usize, DownloadError> {
        let ids: Vec<i64> = self
            .list()
            .iter()
            .filter(|link| starts_with_all(link))
            .map(|link| link.id)
            .collect();
        self.start(&ids, preset_id)?;
        Ok(ids.len())
    }

    /// How links get added besides pasting them: the clipboard watcher and Download Right Away.
    pub fn options(&self) -> AddingSettings {
        self.settings.get().adding
    }

    /// Saves how links get added. Turning the watcher on leaves alone what's already copied; it catches what's
    /// copied next.
    pub fn set_options(&self, options: AddingSettings) -> Result<AddingSettings, DownloadError> {
        self.settings
            .update(|settings| settings.adding = options)
            .map_err(|error| DownloadError::Storage {
                detail: error.to_string(),
            })?;
        self.listener.options_changed(&options);
        Ok(options)
    }

    /// Adds links, saved together in one go, since a file can hold thousands.
    fn insert(
        &self,
        links: &[String],
        origin: LinkOrigin,
        start_when_read: StartWhenRead,
    ) -> Result<AddedLinks, DownloadError> {
        let mut outcome = AddedLinks {
            added: 0,
            already_pending: 0,
        };
        {
            let mut state = lock(&self.state);
            let mut here: HashSet<String> = state
                .entries
                .iter()
                .map(|entry| entry.link.clone())
                .collect();
            let added_at = seconds_since_epoch();
            let mut new = Vec::new();
            for text in links {
                let Some(link) = metadata::normalize_link(text) else {
                    continue;
                };
                if !here.insert(link.clone()) {
                    outcome.already_pending += 1;
                    continue;
                }
                new.push(Entry {
                    id: 0,
                    link,
                    state: PendingState::Waiting,
                    fetched: None,
                    error: None,
                    preset_id: None,
                    folder: None,
                    rule_id: None,
                    start_when_read,
                    added_at,
                    origin,
                    password: None,
                });
            }
            self.store.insert_all(&mut new)?;
            outcome.added = new.len() as u32;
            state.entries.append(&mut new);
        }
        if outcome.added > 0 {
            self.wake.notify_all();
            self.notify();
        }
        Ok(outcome)
    }

    /// Removes links, stopping the one being read if it's among them, and keeps them until the next removal so Undo
    /// can put them back (`restore_removed`). How many were removed.
    pub fn remove(&self, ids: &[i64]) -> Result<usize, DownloadError> {
        let count;
        {
            let mut state = lock(&self.state);
            if let Some((id, cancel)) = &state.reading
                && ids.contains(id)
            {
                cancel.cancel();
            }
            for id in ids {
                self.store.delete(*id)?;
            }
            let (removed, kept) = std::mem::take(&mut state.entries)
                .into_iter()
                .partition(|entry| ids.contains(&entry.id));
            state.entries = kept;
            state.removed = removed;
            count = state.removed.len();
        }
        self.notify();
        Ok(count)
    }

    /// Undo for the last removal: puts its links back as they were, with their IDs, so they take their places again
    /// (the list is in the order of IDs), saved as before. A link being read when it was removed is read again. One
    /// whose address, or whose video once read, is here again by now stays out. How many came back; the first error
    /// is returned once every other link is back.
    pub fn restore_removed(&self) -> Result<usize, DownloadError> {
        let mut restored = 0;
        let mut first_error = None;
        {
            let mut state = lock(&self.state);
            for mut entry in std::mem::take(&mut state.removed) {
                let here_again = state.entries.iter().any(|other| {
                    other.link == entry.link
                        || other
                            .fetched
                            .as_ref()
                            .zip(entry.fetched.as_ref())
                            .is_some_and(|(known, restoring)| {
                                same_video(&known.video, &restoring.video)
                            })
                });
                if here_again {
                    continue;
                }
                if entry.state == PendingState::Reading {
                    entry.state = PendingState::Waiting;
                }
                if let Err(error) = self.store.insert(&entry) {
                    first_error.get_or_insert(error);
                    continue;
                }
                let index = state.entries.partition_point(|other| other.id < entry.id);
                state.entries.insert(index, entry);
                restored += 1;
            }
        }
        if restored > 0 {
            self.wake.notify_all();
            self.notify();
        }
        first_error.map_or(Ok(restored), Err)
    }

    /// Unlock, for a link that needs a password: it's read again with `password`, before the other links waiting to be
    /// read. A wrong one has it need a password again, saying so.
    pub fn unlock(&self, id: i64, password: Password) -> Result<(), DownloadError> {
        {
            let mut state = lock(&self.state);
            let entry = find(&mut state.entries, id)?;
            if entry.state != PendingState::NeedsPassword {
                return Err(DownloadError::WrongState);
            }
            entry.state = PendingState::Waiting;
            entry.error = None;
            entry.password = Some(password);
            self.store.update(entry)?;
        }
        self.wake.notify_all();
        self.notify();
        Ok(())
    }

    /// Reads a link that couldn't be read again, after a fix such as signing in.
    pub fn read_again(&self, id: i64) -> Result<(), DownloadError> {
        {
            let mut state = lock(&self.state);
            let entry = find(&mut state.entries, id)?;
            if entry.state != PendingState::Failed {
                return Err(DownloadError::WrongState);
            }
            entry.state = PendingState::Waiting;
            entry.error = None;
            self.store.update(entry)?;
        }
        self.wake.notify_all();
        self.notify();
        Ok(())
    }

    /// Picks which files of a read post the link downloads, by their number (`PostFile::picked`). Any number the post
    /// doesn't have is ignored.
    pub fn set_post_files(&self, id: i64, picked: &[u32]) -> Result<(), DownloadError> {
        {
            let mut state = lock(&self.state);
            let entry = find(&mut state.entries, id)?;
            let video = entry
                .fetched
                .as_mut()
                .map(|fetched| &mut fetched.video)
                .filter(|video| !video.post_files.is_empty())
                .ok_or(DownloadError::WrongState)?;
            for file in &mut video.post_files {
                file.picked = picked.contains(&file.number);
            }
            self.store.update(entry)?;
        }
        self.notify();
        Ok(())
    }

    /// Picks which files of a read shared folder the link downloads, by their place in its list
    /// (`VideoInfo::shared_files`, from 0). Any place the list doesn't have is ignored.
    pub fn set_shared_files(&self, id: i64, picked: &[u32]) -> Result<(), DownloadError> {
        {
            let mut state = lock(&self.state);
            let entry = find(&mut state.entries, id)?;
            let video = entry
                .fetched
                .as_mut()
                .map(|fetched| &mut fetched.video)
                .filter(|video| !video.shared_files.is_empty())
                .ok_or(DownloadError::WrongState)?;
            let picked: HashSet<u32> = picked.iter().copied().collect();
            for (index, file) in video.shared_files.iter_mut().enumerate() {
                file.picked = u32::try_from(index).is_ok_and(|index| picked.contains(&index));
            }
            self.store.update(entry)?;
        }
        self.notify();
        Ok(())
    }

    /// What was read, with a post or a shared folder downloaded before ticking only the files left out then
    /// (`History::left_out_files`, `left_out_shared_files`), as the preview card ticks them.
    fn with_left_out_ticked(&self, mut fetched: Fetched) -> Fetched {
        if let Ok(Some(left_out)) = self.history.left_out_files(&fetched.video) {
            for file in &mut fetched.video.post_files {
                file.picked = left_out.contains(&file.number);
            }
        }
        if let Ok(Some(left_out)) = self.history.left_out_shared_files(&fetched.video) {
            let left_out: HashSet<u32> = left_out.into_iter().collect();
            for (index, file) in fetched.video.shared_files.iter_mut().enumerate() {
                file.picked = u32::try_from(index).is_ok_and(|index| left_out.contains(&index));
            }
        }
        fetched
    }

    /// Picks the preset for one link, or `None` for the one picked for them all.
    pub fn set_preset(&self, id: i64, preset_id: Option<String>) -> Result<(), DownloadError> {
        {
            let mut state = lock(&self.state);
            let entry = find(&mut state.entries, id)?;
            entry.preset_id = preset_id;
            self.store.update(entry)?;
        }
        self.notify();
        Ok(())
    }

    /// Picks one link's folder, or `None` for the one the rules give. One that isn't a full path is inside the download
    /// folder.
    pub fn set_folder(&self, id: i64, folder: Option<String>) -> Result<(), DownloadError> {
        let folder = folder
            .map(|folder| folder.trim().to_owned())
            .filter(|folder| !folder.is_empty())
            .map(|folder| {
                let path = std::path::Path::new(&folder);
                if path.is_absolute() {
                    folder
                } else {
                    let downloads = self.settings.get().downloads.resolved_folder();
                    downloads.join(path).display().to_string()
                }
            });
        {
            let mut state = lock(&self.state);
            let entry = find(&mut state.entries, id)?;
            entry.folder = folder;
            self.store.update(entry)?;
        }
        self.notify();
        Ok(())
    }

    /// Picks the rule that decides, before any other, everything it sets for one link (its folder, preset and file
    /// name), by its ID, or `None` for the rules as usual.
    pub fn set_rule(&self, id: i64, rule_id: Option<String>) -> Result<(), DownloadError> {
        let rule_id = rule_id
            .map(|rule_id| rule_id.trim().to_owned())
            .filter(|rule_id| !rule_id.is_empty());
        {
            let mut state = lock(&self.state);
            let entry = find(&mut state.entries, id)?;
            entry.rule_id = rule_id;
            self.store.update(entry)?;
        }
        self.notify();
        Ok(())
    }

    /// Starts links. One that was read joins the download queue now, with its own preset, else the one a rule picks,
    /// else `preset_id`, and leaves the pending links; one not read yet joins once it's read, unless it turns out to be
    /// downloaded already, with the preset a rule picks then, else `preset_id`. One that couldn't be read stays. A
    /// video downloaded before with the same preset replaces its file, as Download Again does once the user confirms
    /// it (the UI asks first). The first error is returned once every other link has been started.
    pub fn start(&self, ids: &[i64], preset_id: &str) -> Result<(), DownloadError> {
        let mut first_error = None;
        {
            let mut state = lock(&self.state);
            let mut started = Vec::new();
            for entry in state
                .entries
                .iter_mut()
                .filter(|entry| ids.contains(&entry.id))
            {
                match entry.state {
                    PendingState::Ready => {
                        match self.enqueue(entry, entry.preset_id.as_deref(), Some(preset_id)) {
                            Ok(()) => started.push(entry.id),
                            Err(error) => {
                                first_error.get_or_insert(error);
                            }
                        }
                    }
                    PendingState::Waiting | PendingState::Reading => {
                        entry.start_when_read = StartWhenRead::Yes;
                        entry.preset_id.get_or_insert_with(|| preset_id.to_owned());
                        if let Err(error) = self.store.update(entry) {
                            first_error.get_or_insert(error);
                        }
                    }
                    PendingState::Failed | PendingState::NeedsPassword => {}
                }
            }
            for id in &started {
                if let Err(error) = self.store.delete(*id) {
                    first_error.get_or_insert(error);
                }
            }
            state.entries.retain(|entry| !started.contains(&entry.id));
        }
        self.notify();
        first_error.map_or(Ok(()), Err)
    }

    /// Adds a link that was read to the download queue, with its folder and rule: a post or a file host's link as it
    /// is, a video with the preset `start_preset` gives. A video already in the queue counts as started.
    fn enqueue(
        &self,
        entry: &Entry,
        chosen: Option<&str>,
        otherwise: Option<&str>,
    ) -> Result<(), DownloadError> {
        let video = entry
            .fetched
            .as_ref()
            .map(|fetched| fetched.video.clone())
            .ok_or(DownloadError::WrongState)?;
        let folder = entry.folder.clone();
        let rule_id = entry.rule_id.clone();
        let password = entry.password.clone();
        let added = if video.post.is_some() || video.files.is_some() {
            self.queue.add_post_in(video, folder, rule_id, password)
        } else {
            let preset = self
                .start_preset(&video, chosen, rule_id.as_deref(), otherwise)
                .ok_or(DownloadError::Preset {
                    error: PresetError::NotFound,
                })?;
            let overwrite = self
                .history
                .last_download(&video.site, &video.video_id)
                .ok()
                .flatten()
                .is_some_and(|last| last.preset == preset.name);
            self.queue
                .add_in(video, &preset.id, overwrite, folder, rule_id, password)
        };
        match added {
            Ok(_) | Err(DownloadError::AlreadyQueued) => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// The preset a video starts with: `chosen` for it, else the one the rule `picked` for it or another rule picks,
    /// else `otherwise`, else the default one, each only if it still exists.
    fn start_preset(
        &self,
        video: &VideoInfo,
        chosen: Option<&str>,
        picked: Option<&str>,
        otherwise: Option<&str>,
    ) -> Option<Preset> {
        let rules = self.settings.get().rules.list;
        let by_rule =
            rules::start_preset(&rules, video, picked, |id| self.presets.get(id).is_some());
        [
            chosen,
            by_rule.as_ref().map(|pick| pick.preset_id.as_str()),
            otherwise,
        ]
        .into_iter()
        .flatten()
        .find_map(|id| self.presets.get(id))
        .or_else(|| self.presets.get(&self.presets.overview().default_id))
    }

    /// Whether a copied or sent link starts once read: what the last rule that says so for it gives, else Settings.
    /// A video counts with the preset it would start with.
    fn downloads_right_away(&self, entry: &Entry, video: &VideoInfo) -> bool {
        let settings = self.settings.get();
        let preset = (video.post.is_none() && video.files.is_none())
            .then(|| {
                self.start_preset(
                    video,
                    None,
                    entry.rule_id.as_deref(),
                    entry.preset_id.as_deref(),
                )
            })
            .flatten();
        let subject = RuleSubject {
            video,
            preset: preset.as_ref(),
        };
        rules::download_right_away(&settings.rules.list, &subject)
            .unwrap_or(settings.adding.download_right_away)
    }

    /// Whether the UI shows a link as starting once it's read. A sent link does while Settings' Download Right Away
    /// is on, as no rule can tell until it's read.
    fn shows_starting_once_read(&self, start_when_read: StartWhenRead) -> bool {
        match start_when_read {
            StartWhenRead::No => false,
            StartWhenRead::Yes => true,
            StartWhenRead::IfRightAway => self.options().download_right_away,
        }
    }

    /// The first link to read: the oldest waiting one, skipping those on a paced site while its pause runs. It's
    /// marked as being read.
    fn next_to_read(&self) -> Option<Turn> {
        let mut state = lock(&self.state);
        let index = readable(&state, Instant::now(), |link| self.site_waits(link))?;
        let entry = &mut state.entries[index];
        entry.state = PendingState::Reading;
        let typed = entry.password.is_some();
        let password = entry
            .password
            .clone()
            .or_else(|| self.history.password_for_link(&entry.link).ok().flatten());
        let turn = Turn {
            id: entry.id,
            link: entry.link.clone(),
            origin: entry.origin,
            password,
            typed,
            cancel: CancelToken::new(),
        };
        state.reading = Some((turn.id, turn.cancel.clone()));
        drop(state);
        self.notify();
        Some(turn)
    }

    /// Waits until there may be a link to read: one is added, a paced site's pause ends, or a second goes by.
    fn wait_for_work(&self) {
        let state = lock(&self.state);
        let now = Instant::now();
        if readable(&state, now, |link| self.site_waits(link)).is_some() {
            return;
        }
        let wait = state
            .paced_until
            .values()
            .filter(|until| **until > now)
            .map(|until| *until - now)
            .fold(IDLE_WAIT, Duration::min);
        drop(
            self.wake
                .wait_timeout(state, wait)
                .unwrap_or_else(PoisonError::into_inner),
        );
    }

    /// Reads a link and keeps what was found. A link on a paced site makes the next one wait. A link to a video
    /// that another link here already found is dropped, and one started before it was read goes to the queue. A
    /// copied link also leaves when there's nothing to download or the history or the queue has it, and starts once
    /// read with Download Right Away on. A site that answers "too many requests" waits (`DownloadQueue::slow_down`),
    /// and its links are read again once the wait ends.
    fn read(&self, turn: &Turn) {
        let outcome = self
            .reader
            .read(
                &turn.link,
                turn.origin == LinkOrigin::Added,
                turn.password.as_ref(),
                &turn.cancel,
            )
            .map_err(|error| error.after_password(turn.typed));
        match &outcome {
            Err(PreviewError::TooManyRequests { .. }) => {
                self.queue.slow_down(&turn.link);
            }
            Ok(_) => self.queue.site_worked(&turn.link),
            Err(_) => {}
        }
        let settings = self.settings.get();
        let mut announce = None;
        {
            let mut state = lock(&self.state);
            state.reading = None;
            if let (Some(site), Some(pause)) = (
                sites::paced_site(&turn.link),
                settings.downloads.pause_for(&turn.link),
            ) {
                state
                    .paced_until
                    .insert(site, Instant::now() + random_pause(pause));
            }
            // Removed while it was read.
            if turn.cancel.is_cancelled() {
                return;
            }
            let Some(index) = state.entries.iter().position(|entry| entry.id == turn.id) else {
                return;
            };
            let leaves = match &outcome {
                Ok(fetched) => {
                    let duplicate = state.entries.iter().any(|other| {
                        other.id != turn.id
                            && other
                                .fetched
                                .as_ref()
                                .is_some_and(|known| same_video(&known.video, &fetched.video))
                    });
                    duplicate || self.leaves_once_read(turn.origin, &outcome, &turn.link)
                }
                Err(_) => self.leaves_once_read(turn.origin, &outcome, &turn.link),
            };
            if leaves {
                state.entries.remove(index);
                let _ = self.store.delete(turn.id);
                drop(state);
                self.notify();
                return;
            }
            match outcome {
                Ok(fetched) => {
                    let video = fetched.video.clone();
                    let entry = &mut state.entries[index];
                    entry.state = PendingState::Ready;
                    entry.fetched = Some(self.with_left_out_ticked(fetched));
                    entry.error = None;
                    // The password that opened it goes with it to the queue.
                    entry.password.clone_from(&turn.password);
                    let starts = match entry.start_when_read {
                        StartWhenRead::Yes => true,
                        StartWhenRead::IfRightAway => self.downloads_right_away(entry, &video),
                        StartWhenRead::No => {
                            turn.origin == LinkOrigin::Copied
                                && self.downloads_right_away(entry, &video)
                        }
                    };
                    let started = starts && self.start_once_read(entry);
                    entry.start_when_read = StartWhenRead::No;
                    if started {
                        state.entries.remove(index);
                        let _ = self.store.delete(turn.id);
                    } else {
                        let _ = self.store.update(&state.entries[index]);
                    }
                    if turn.origin == LinkOrigin::Copied {
                        announce = Some((video, started));
                    }
                }
                Err(error) => {
                    let entry = &mut state.entries[index];
                    // Only removing a link cancels its read, so this is the app closing: read it next time. A site
                    // that asked to slow down is read again once its wait ends.
                    if matches!(
                        error,
                        PreviewError::Cancelled | PreviewError::TooManyRequests { .. }
                    ) {
                        entry.state = PendingState::Waiting;
                    } else if matches!(error, PreviewError::PasswordNeeded { .. }) {
                        // It waits for a password; one that didn't open it isn't kept. Started, it still starts once
                        // a password opens it.
                        entry.state = PendingState::NeedsPassword;
                        entry.error = Some(error);
                        entry.password = None;
                    } else {
                        entry.state = PendingState::Failed;
                        entry.error = Some(error);
                        entry.start_when_read = StartWhenRead::No;
                    }
                    let _ = self.store.update(entry);
                }
            }
        }
        self.notify();
        if let Some((video, started)) = announce {
            self.listener.copied_link_read(&video, started);
        }
    }

    /// Whether the queue or the history has the video already.
    fn has_video(&self, video: &VideoInfo) -> bool {
        self.queue.contains_video(video)
            || self
                .history
                .last_download(&video.site, &video.video_id)
                .ok()
                .flatten()
                .is_some()
    }

    /// Adds a link started before it was read, or that downloads right away, to the queue, unless the history has it:
    /// then it stays, marked, for the user to decide. A video starts with the preset a rule picks, else the one picked
    /// when it was started, else the default one. Whether it went to the queue.
    fn start_once_read(&self, entry: &mut Entry) -> bool {
        entry.start_when_read = StartWhenRead::No;
        let Some(fetched) = &entry.fetched else {
            return false;
        };
        let downloaded_before = self
            .history
            .last_download(&fetched.video.site, &fetched.video.video_id)
            .ok()
            .flatten()
            .is_some();
        if downloaded_before {
            return false;
        }
        self.enqueue(entry, None, entry.preset_id.as_deref())
            .is_ok()
    }

    fn present(&self, entry: Entry) -> PendingLink {
        let hold = (entry.state == PendingState::Waiting)
            .then(|| self.queue.site_wait(&entry.link))
            .flatten();
        let preview = entry.fetched.map(|fetched| VideoPreview {
            last_download: self
                .history
                .last_download(&fetched.video.site, &fetched.video.video_id)
                .ok()
                .flatten(),
            in_queue: self
                .queue
                .contains(&fetched.video.site, &fetched.video.video_id),
            missing_js_runtime: fetched.missing_js_runtime,
            video: fetched.video,
        });
        PendingLink {
            id: entry.id,
            link: entry.link,
            state: entry.state,
            preview,
            error: entry.error,
            preset_id: entry.preset_id,
            folder: entry.folder,
            rule_id: entry.rule_id,
            start_when_read: self.shows_starting_once_read(entry.start_when_read),
            added_at: entry.added_at,
            caught: entry.origin == LinkOrigin::Copied,
            from_file: entry.origin == LinkOrigin::File,
            from_page: entry.origin == LinkOrigin::Page,
            hold,
        }
    }

    /// Whether the link's site waits after answering "too many requests".
    fn site_waits(&self, link: &str) -> bool {
        self.queue.site_wait(link).is_some()
    }

    /// Tells the listener the links changed: at once when the last notice went out a while ago (`change_gap`), else
    /// once that's up, with the list as it is then (`send_due_notices`).
    fn notify(&self) {
        let count = lock(&self.state).entries.len();
        {
            let mut notices = lock(&self.notices);
            let now = Instant::now();
            if notices
                .last_sent
                .is_some_and(|last| now.duration_since(last) < change_gap(count))
            {
                if !notices.due {
                    notices.due = true;
                    self.notices_due.notify_all();
                }
                return;
            }
            notices.last_sent = Some(now);
            notices.due = false;
        }
        self.send_changes();
    }

    fn send_changes(&self) {
        let links = self.list();
        self.listener.changed(&links);
    }

    /// Whether a link leaves once read, without a word: the rule for each origin, in one place. A link added by hand
    /// always stays, to say what it is or why it couldn't be read. A copied link leaves when there's nothing to
    /// download (`nothing_to_download`) or the queue or the history has its video, since nobody picked it. A link from
    /// a file, or one a page sent, leaves when there's nothing to download, and otherwise stays as one added by hand
    /// does: marked when it was downloaded before. (A second link to a video already here leaves whatever its origin.)
    fn leaves_once_read(
        &self,
        origin: LinkOrigin,
        outcome: &Result<Fetched, PreviewError>,
        link: &str,
    ) -> bool {
        match (origin, outcome) {
            (LinkOrigin::Added, _) => false,
            (LinkOrigin::Copied | LinkOrigin::File | LinkOrigin::Page, Err(error)) => {
                nothing_to_download(error, link)
            }
            (LinkOrigin::Copied, Ok(fetched)) => self.has_video(&fetched.video),
            (LinkOrigin::File | LinkOrigin::Page, Ok(_)) => false,
        }
    }
}

/// Sends the pending-changed notice that `notify` left for later, once its gap is up, while the pending links exist.
fn send_due_notices(links: &Weak<PendingLinks>) {
    while let Some(links) = links.upgrade() {
        let count = lock(&links.state).entries.len();
        let mut notices = lock(&links.notices);
        if !notices.due {
            drop(
                links
                    .notices_due
                    .wait_timeout(notices, IDLE_WAIT)
                    .unwrap_or_else(PoisonError::into_inner),
            );
            continue;
        }
        let gap = change_gap(count);
        let since = notices.last_sent.map_or(gap, |last| last.elapsed());
        if since < gap {
            drop(notices);
            drop(links);
            thread::sleep(gap - since);
            continue;
        }
        notices.due = false;
        notices.last_sent = Some(Instant::now());
        drop(notices);
        links.send_changes();
    }
}

/// Reads waiting links one at a time while the pending links exist.
fn read_links(links: &Weak<PendingLinks>) {
    while let Some(links) = links.upgrade() {
        match links.next_to_read() {
            Some(turn) => links.read(&turn),
            None => links.wait_for_work(),
        }
    }
}

/// Whether Start All takes a link: not one that couldn't be read, is in the queue, was downloaded before or starts
/// once read already, nor a copied link still being checked. The Pending view's own Start All matches it.
pub fn starts_with_all(link: &PendingLink) -> bool {
    let checking = (link.caught || link.from_file || link.from_page)
        && matches!(link.state, PendingState::Waiting | PendingState::Reading);
    !matches!(
        link.state,
        PendingState::Failed | PendingState::NeedsPassword
    ) && !checking
        && !link.start_when_read
        && link
            .preview
            .as_ref()
            .is_none_or(|preview| !preview.in_queue && preview.last_download.is_none())
}

/// A link's turn to be read.
struct Turn {
    id: i64,
    link: String,
    origin: LinkOrigin,
    /// The password to read it with: the one typed on its row, else the history's for the link.
    password: Option<Password>,
    /// `password` was typed on its row, so it's wrong if the link still needs one.
    typed: bool,
    cancel: CancelToken,
}

/// Whether a copied link's error means there's nothing there to download, rather than something to fix such as
/// signing in: then it leaves without a word.
fn nothing_to_download(error: &PreviewError, link: &str) -> bool {
    match error {
        PreviewError::InvalidLink
        | PreviewError::UnsupportedSite
        // A short link to such a site, as the site's own link would be.
        | PreviewError::ShortLinkToUnsupportedSite { .. }
        | PreviewError::NoVideo { .. }
        | PreviewError::Playlist
        | PreviewError::NotListable { .. } => true,
        // Without gallery-dl, a page with no video may be images; on a site that isn't about images, it's more
        // likely just a page.
        PreviewError::ImagesNotSetUp => !sites::prefers_images(link),
        _ => false,
    }
}

/// Where the next link to read is: one the person just typed a password for, else the oldest waiting one, leaving
/// paced sites for later while their pause runs, and sites that asked to slow down while they wait (`site_waits`).
fn readable(state: &Contents, now: Instant, site_waits: impl Fn(&str) -> bool) -> Option<usize> {
    let paced_ready = |link: &str| {
        sites::paced_site(link)
            .and_then(|site| state.paced_until.get(&site))
            .is_none_or(|until| *until <= now)
    };
    let ready = |entry: &Entry| {
        entry.state == PendingState::Waiting && paced_ready(&entry.link) && !site_waits(&entry.link)
    };
    state
        .entries
        .iter()
        .position(|entry| ready(entry) && entry.password.is_some())
        .or_else(|| state.entries.iter().position(ready))
}

fn same_video(first: &VideoInfo, second: &VideoInfo) -> bool {
    first.site == second.site && first.video_id == second.video_id
}

/// A random time within the range, as gallery-dl's `--sleep` picks one before each file.
fn random_pause(range: PauseRange) -> Duration {
    let range = range.normalized();
    let spread = u64::from(range.max - range.min) + 1;
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos()),
    );
    Duration::from_secs(u64::from(range.min) + hasher.finish() % spread)
}

fn find(entries: &mut [Entry], id: i64) -> Result<&mut Entry, DownloadError> {
    entries
        .iter_mut()
        .find(|entry| entry.id == id)
        .ok_or(DownloadError::NotFound)
}

fn seconds_since_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

#[cfg(test)]
mod tests;
