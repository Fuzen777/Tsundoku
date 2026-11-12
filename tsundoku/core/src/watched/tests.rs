//! The watched sources, checked by a fake reader the tests answer for, queueing into a queue whose downloads fail at
//! once, so what was added stays in it to look at.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::*;
use crate::data_folder::DataPaths;
use crate::downloads::{
    DownloadProgress, JobOutcome, JobRunner, JobSpec, QueueItem, QueueListener, QueueServices,
};
use crate::history::NewDownload;
use crate::listing::ListedItem;
use crate::rules::DownloadKind;
use crate::tools::ToolManager;

/// Answers each check with the listing a test set, newest first.
#[derive(Default)]
struct FakeReader {
    listing: Mutex<Vec<String>>,
    failure: Mutex<Option<PreviewError>>,
    reads: AtomicUsize,
}

impl FakeReader {
    fn lists(&self, ids: &[&str]) {
        *lock(&self.listing) = ids.iter().map(|id| (*id).to_owned()).collect();
    }
}

impl ListingReader for FakeReader {
    fn read(&self, url: &str, _cancel: &CancelToken) -> Result<Listing, PreviewError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = lock(&self.failure).clone() {
            return Err(error);
        }
        Ok(Listing {
            kind: ListingKind::Channel,
            title: "Renamed Channel".to_owned(),
            uploader: None,
            site_name: "youtube.com".to_owned(),
            url: url.to_owned(),
            thumbnail_url: None,
            total: None,
            newest_only: true,
            items: lock(&self.listing)
                .iter()
                .map(|id| ListedItem {
                    video: video(id),
                    approximate_date: None,
                    last_download: None,
                    in_queue: false,
                })
                .collect(),
        })
    }
}

struct FailingRunner;

impl JobRunner for FailingRunner {
    fn run(
        &self,
        _job: &JobSpec,
        _cancel: &CancelToken,
        _on_progress: &mut dyn FnMut(DownloadProgress),
    ) -> JobOutcome {
        JobOutcome::Failed(DownloadError::NoFile)
    }
}

struct NoQueueEvents;

impl QueueListener for NoQueueEvents {
    fn changed(&self, _items: &[QueueItem]) {}
    fn progress(&self, _id: i64, _progress: &DownloadProgress) {}
}

#[derive(Default)]
struct CountingListener {
    changes: AtomicUsize,
}

impl WatchedListener for CountingListener {
    fn changed(&self, _sources: &[WatchedSource]) {
        self.changes.fetch_add(1, Ordering::SeqCst);
    }
}

struct Test {
    folder: tempfile::TempDir,
    history: Arc<History>,
    queue: Arc<DownloadQueue>,
    reader: Arc<FakeReader>,
    sources: Arc<WatchedSources>,
}

const CHANNEL: &str = "https://www.youtube.com/@someone/videos";

impl Test {
    fn new() -> Self {
        let folder = tempfile::tempdir().unwrap();
        let database = Arc::new(Database::open_in_memory().unwrap());
        let history = Arc::new(
            History::new(Arc::clone(&database), folder.path().join("archive.txt")).unwrap(),
        );
        let settings = Arc::new(SettingsStore::open(folder.path().join("settings.json")).unwrap());
        let downloads = folder.path().join("downloads").display().to_string();
        settings
            .update(|settings| {
                settings.downloads.cookies_browser = None;
                settings.downloads.folder = Some(downloads.clone());
                // Nothing runs, so queued items stay waiting to be looked at.
                settings.downloads.at_once = 1;
            })
            .unwrap();
        let presets = Arc::new(PresetManager::new(Arc::clone(&settings)));
        let queue = DownloadQueue::open(QueueServices {
            database: Arc::clone(&database),
            history: Arc::clone(&history),
            tools: Arc::new(ToolManager::new(
                DataPaths::new(folder.path().to_path_buf()),
                Arc::clone(&settings),
                "0.0.0".to_owned(),
            )),
            presets: Arc::clone(&presets),
            settings: Arc::clone(&settings),
            logs_folder: folder.path().join("logs"),
            runner: Arc::new(FailingRunner),
            listener: Arc::new(NoQueueEvents),
        })
        .unwrap();
        let reader = Arc::new(FakeReader::default());
        let sources = WatchedSources::open(WatchedServices {
            database,
            history: Arc::clone(&history),
            queue: Arc::clone(&queue),
            presets,
            settings,
            reader: reader.clone() as Arc<dyn ListingReader>,
            listener: Arc::new(CountingListener::default()),
        });
        Self {
            folder,
            history,
            queue,
            reader,
            sources,
        }
    }

    /// Adds the channel as the picker listed it, with these items newest first.
    fn add(&self, ids: &[&str], first: FirstItems) -> i64 {
        self.sources
            .add(NewWatchedSource {
                url: CHANNEL.to_owned(),
                kind: ListingKind::Channel,
                name: "Someone".to_owned(),
                site_name: "youtube.com".to_owned(),
                thumbnail_url: None,
                items: ids
                    .iter()
                    .map(|id| ListedVideo {
                        video: video(id),
                        approximate_date: None,
                    })
                    .collect(),
                first,
                folder: None,
                preset_id: None,
            })
            .unwrap()
    }

    fn source(&self, id: i64) -> WatchedSource {
        self.sources
            .list()
            .unwrap()
            .into_iter()
            .find(|source| source.id == id)
            .unwrap()
    }

    fn new_ids(&self, id: i64) -> Vec<String> {
        ids(&self.source(id).new_items)
    }

    fn check(&self, id: i64) {
        let reads = self.reader.reads.load(Ordering::SeqCst);
        self.sources.check(id).unwrap();
        wait_until("the check ends", || {
            self.reader.reads.load(Ordering::SeqCst) > reads && !self.source(id).checking
        });
    }

    fn downloaded(&self, id: &str) {
        self.history
            .record(
                &video(id),
                &NewDownload {
                    preset: "Best Video".to_owned(),
                    file_path: self
                        .folder
                        .path()
                        .join(format!("{id}.mkv"))
                        .display()
                        .to_string(),
                    files: Vec::new(),
                    file_size: None,
                    format: None,
                    downloaded_at: 1,
                    kind: DownloadKind::Video,
                    share_paths: Vec::new(),
                },
            )
            .unwrap();
    }
}

fn video(id: &str) -> VideoInfo {
    VideoInfo {
        site: "youtube".to_owned(),
        site_name: "youtube.com".to_owned(),
        video_id: id.to_owned(),
        url: format!("https://www.youtube.com/watch?v={id}"),
        title: format!("Title of {id}"),
        uploader: Some("Someone".to_owned()),
        account: None,
        duration_seconds: Some(10.0),
        upload_date: None,
        thumbnail_url: None,
        best_height: Some(1080),
        post: None,
        files: None,
        post_files: Vec::new(),
        shared_files: Vec::new(),
        playlist: None,
    }
}

fn key(id: &str) -> ItemKey {
    ItemKey::of(&video(id))
}

fn ids(items: &[WatchedItem]) -> Vec<String> {
    items
        .iter()
        .map(|item| item.video.video_id.clone())
        .collect()
}

fn wait_until(what: &str, condition: impl Fn() -> bool) {
    let started = Instant::now();
    while !condition() {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "timed out waiting until {what}"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_first_items_are_what_was_asked_when_adding() {
    let test = Test::new();
    test.downloaded("b");
    let latest = test.add(&["a", "b", "c", "d", "e"], FirstItems::Latest { count: 2 });
    assert_eq!(
        test.new_ids(latest),
        ["a", "c"],
        "the downloaded one isn't new"
    );
    test.sources.remove(latest).unwrap();

    let all = test.add(&["a", "b", "c"], FirstItems::All);
    assert_eq!(test.new_ids(all), ["a", "c"]);
    test.sources.remove(all).unwrap();

    let from_now = test.add(&["a", "b", "c"], FirstItems::FromNow);
    assert!(test.new_ids(from_now).is_empty());
    assert!(matches!(
        test.sources.add(NewWatchedSource {
            url: CHANNEL.to_owned(),
            kind: ListingKind::Channel,
            name: String::new(),
            site_name: String::new(),
            thumbnail_url: None,
            items: Vec::new(),
            first: FirstItems::All,
            folder: None,
            preset_id: None,
        }),
        Err(WatchedError::AlreadyWatched)
    ));
}

#[test]
fn a_check_keeps_only_what_it_doesnt_know() {
    let test = Test::new();
    let id = test.add(&["x", "c", "d", "e", "f"], FirstItems::FromNow);
    test.downloaded("b");
    // "x" is pinned above the newest: the check goes on past it, and past "b", downloaded, to three known in a row.
    test.reader
        .lists(&["x", "a", "b", "c", "d", "e", "f", "old"]);
    test.check(id);
    let source = test.source(id);
    assert_eq!(ids(&source.new_items), ["a"]);
    assert_eq!(source.name, "Renamed Channel");
    assert!(source.last_error.is_none());

    test.reader.lists(&["n", "x", "a", "b", "c", "d"]);
    test.check(id);
    assert_eq!(
        test.new_ids(id),
        ["n", "a"],
        "the latest find first, each found once"
    );
}

#[test]
fn downloading_queues_the_picked_and_skips_the_rest() {
    let test = Test::new();
    let id = test.add(&["a", "b", "c"], FirstItems::All);
    test.sources
        .edit(id, Some(" Channels\\{uploader} ".to_owned()), None)
        .unwrap();
    assert_eq!(test.sources.download(id, &[key("a")], true).unwrap(), 1);
    let source = test.source(id);
    assert!(source.new_items.is_empty());
    assert_eq!(ids(&source.skipped_items), ["b", "c"]);
    let queued = test.queue.items();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].video.video_id, "a");
    assert_eq!(
        Path::new(&queued[0].folder),
        test.folder
            .path()
            .join("downloads")
            .join("Channels")
            .join("Someone"),
        "its own folder, inside the download folder"
    );
    assert!(queued[0].folder_chosen, "beats every rule");
    assert!(queued[0].preset.is_some());

    // A skipped item can still be downloaded, and a check never finds it again.
    test.sources.download(id, &[key("c")], false).unwrap();
    assert_eq!(ids(&test.source(id).skipped_items), ["b"]);
    test.reader.lists(&["c", "b", "a"]);
    test.check(id);
    assert!(test.new_ids(id).is_empty());
}

#[test]
fn a_failed_check_keeps_its_error_until_one_works() {
    let test = Test::new();
    let id = test.add(&["a"], FirstItems::FromNow);
    *lock(&test.reader.failure) = Some(PreviewError::sign_in("login required".to_owned()));
    test.check(id);
    assert!(matches!(
        test.source(id).last_error,
        Some(PreviewError::SignInRequired { .. })
    ));
    *lock(&test.reader.failure) = None;
    test.reader.lists(&["b", "a"]);
    test.check(id);
    let source = test.source(id);
    assert!(source.last_error.is_none());
    assert_eq!(ids(&source.new_items), ["b"]);
}

#[test]
fn removing_forgets_the_source() {
    let test = Test::new();
    let id = test.add(&["a"], FirstItems::All);
    test.sources.remove(id).unwrap();
    assert!(test.sources.list().unwrap().is_empty());
    assert!(matches!(
        test.sources.check(id),
        Err(WatchedError::NotFound)
    ));
    assert!(matches!(
        test.sources.remove(id),
        Err(WatchedError::NotFound)
    ));
}
