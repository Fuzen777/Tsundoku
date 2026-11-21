//! The watched sources, checked by a fake reader the tests answer for, queueing into a queue whose downloads fail at
//! once, so what was added stays in it to look at.

use std::sync::Condvar;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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

/// Answers each check with the listing a test set, newest first, noting whether the check said the items reached what
/// it had seen (`reached`), which links it started reading (`started`), and holding a read of a link a test holds until
/// it's released.
#[derive(Default)]
struct FakeReader {
    listing: Mutex<Vec<String>>,
    failure: Mutex<Option<PreviewError>>,
    reads: AtomicUsize,
    reached: Mutex<Option<bool>>,
    started: Mutex<Vec<String>>,
    held: Mutex<HashSet<String>>,
    released: Condvar,
    /// Whether the listing gives each item a picture (`picture_of`).
    pictures: AtomicBool,
    /// What the read met past its items (`Listing::also_read`).
    also_read: Mutex<Vec<String>>,
}

impl FakeReader {
    fn lists(&self, ids: &[&str]) {
        *lock(&self.listing) = ids.iter().map(|id| (*id).to_owned()).collect();
    }

    /// The links whose reads started, in that order.
    fn started(&self) -> Vec<String> {
        lock(&self.started).clone()
    }

    /// Reads of `url` wait until it's released.
    fn hold(&self, url: &str) {
        lock(&self.held).insert(url.to_owned());
    }

    fn release(&self, url: &str) {
        lock(&self.held).remove(url);
        self.released.notify_all();
    }
}

impl ListingReader for FakeReader {
    fn read(
        &self,
        url: &str,
        _cancel: &CancelToken,
        reached: &dyn Fn(&Listing) -> bool,
    ) -> Result<Listing, PreviewError> {
        lock(&self.started).push(url.to_owned());
        // A test that never releases fails on its own wait rather than hanging here.
        drop(
            self.released
                .wait_timeout_while(lock(&self.held), Duration::from_secs(10), |held| {
                    held.contains(url)
                })
                .unwrap(),
        );
        self.reads.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = lock(&self.failure).clone() {
            return Err(error);
        }
        let read = |id: &String| VideoInfo {
            thumbnail_url: self.pictures.load(Ordering::SeqCst).then(|| picture_of(id)),
            ..video(id)
        };
        let listing = Listing {
            kind: ListingKind::Channel,
            title: "Renamed Channel".to_owned(),
            uploader: None,
            site_name: "youtube.com".to_owned(),
            url: url.to_owned(),
            thumbnail_url: None,
            total: None,
            media_total: None,
            pace: None,
            newest_only: true,
            items: lock(&self.listing)
                .iter()
                .map(|id| ListedItem {
                    video: read(id),
                    approximate_date: None,
                    last_download: None,
                    in_queue: false,
                })
                .collect(),
            more: None,
            also_read: lock(&self.also_read).iter().map(read).collect(),
        };
        *lock(&self.reached) = Some(reached(&listing));
        Ok(listing)
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
        self.add_at(CHANNEL, ids, first)
    }

    /// Adds the listing at `url` as the picker listed it, with these items newest first.
    fn add_at(&self, url: &str, ids: &[&str], first: FirstItems) -> i64 {
        self.sources
            .add(NewWatchedSource {
                url: url.to_owned(),
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

fn picture_of(id: &str) -> String {
    format!("https://pictures.example/{id}.jpg")
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
fn a_check_gives_the_items_it_has_the_pictures_it_reads_for_them() {
    let test = Test::new();
    // Listed without pictures, as an X account's videos were before their covers were read.
    let id = test.add(&["b", "c", "d"], FirstItems::All);
    let pictures = |test: &Test| -> Vec<(String, Option<String>)> {
        test.source(id)
            .new_items
            .iter()
            .map(|item| {
                (
                    item.video.video_id.clone(),
                    item.video.thumbnail_url.clone(),
                )
            })
            .collect()
    };
    assert!(pictures(&test).iter().all(|(_, picture)| picture.is_none()));

    test.sources.skip(id, &[key("d")]).unwrap();
    test.reader.pictures.store(true, Ordering::SeqCst);
    test.reader.lists(&["a", "b", "c", "d"]);
    test.check(id);
    assert_eq!(
        pictures(&test),
        [
            ("a".to_owned(), Some(picture_of("a"))),
            ("b".to_owned(), Some(picture_of("b"))),
            ("c".to_owned(), Some(picture_of("c"))),
        ],
        "the new one and the ones it had, which stay new in their places"
    );
    let source = test.source(id);
    assert_eq!(ids(&source.skipped_items), ["d"], "still skipped");
    assert_eq!(
        source.skipped_items[0].video.thumbnail_url,
        Some(picture_of("d"))
    );

    // A picture it has is kept, whatever a later read gives or lacks.
    test.reader.pictures.store(false, Ordering::SeqCst);
    test.check(id);
    assert_eq!(pictures(&test)[1].1, Some(picture_of("b")));
}

#[test]
fn a_check_takes_pictures_from_what_it_read_past_its_items_too() {
    let test = Test::new();
    let id = test.add(&["b", "c", "old"], FirstItems::All);
    // As an X account's check reads: "old" came with the videos, past where the photos stopped, so it isn't listed.
    test.reader.pictures.store(true, Ordering::SeqCst);
    test.reader.lists(&["a", "b", "c"]);
    *lock(&test.reader.also_read) = vec!["old".to_owned(), "older".to_owned()];
    test.check(id);
    let source = test.source(id);
    assert_eq!(
        ids(&source.new_items),
        ["a", "b", "c", "old"],
        "one the source never listed isn't taken from there"
    );
    assert!(
        source
            .new_items
            .iter()
            .all(|item| item.video.thumbnail_url == Some(picture_of(&item.video.video_id)))
    );
}

#[test]
fn a_check_tells_the_reader_once_it_reaches_what_it_knows() {
    let test = Test::new();
    let id = test.add(&["x", "c", "d", "e", "f"], FirstItems::FromNow);
    test.downloaded("b");
    let reached = |listing: &[&str]| {
        test.reader.lists(listing);
        test.check(id);
        lock(&test.reader.reached).take().unwrap()
    };
    // Past "x", pinned, more than three known in a row ("b" downloaded): an Instagram read can stop there.
    assert!(reached(&["n", "x", "b", "c", "d"]));
    // Three known in a row could all be pinned above what's new, so the read goes on.
    assert!(!reached(&["n2", "x", "c", "d", "m"]));
    assert!(!reached(&["n3", "n4"]));
}

#[test]
fn downloading_queues_the_picked_and_skips_the_rest() {
    let test = Test::new();
    let id = test.add(&["a", "b", "c"], FirstItems::All);
    test.sources
        .edit(id, Some(" Channels\\{uploader} ".to_owned()), None)
        .unwrap();
    assert_eq!(
        test.sources
            .download(id, &[key("a")], &[key("b"), key("c")])
            .unwrap(),
        1
    );
    let source = test.source(id);
    assert!(source.new_items.is_empty());
    assert_eq!(ids(&source.skipped_items), ["b", "c"]);
    let queued = test.queue.items();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].video.video_id, "a");
    assert_eq!(
        queued[0].source,
        Some(QueueSource {
            url: CHANNEL.to_owned(),
            kind: ListingKind::Channel,
            name: "Someone".to_owned(),
            site_name: "youtube.com".to_owned(),
            thumbnail_url: None,
        }),
        "the Queue shows it under the source"
    );
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
    test.sources.download(id, &[key("c")], &[]).unwrap();
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

#[test]
fn a_source_says_how_many_of_its_items_were_downloaded_and_when_it_last_posted() {
    let test = Test::new();
    test.downloaded("b");
    let dated = |id: &str, exact: Option<&str>, rough: Option<&str>| ListedVideo {
        video: VideoInfo {
            upload_date: exact.map(str::to_owned),
            ..video(id)
        },
        approximate_date: rough.map(str::to_owned),
    };
    let id = test
        .sources
        .add(NewWatchedSource {
            url: CHANNEL.to_owned(),
            kind: ListingKind::Channel,
            name: "Someone".to_owned(),
            site_name: "youtube.com".to_owned(),
            thumbnail_url: None,
            items: vec![
                dated("a", None, Some("2026-11-02")),
                dated("b", Some("2026-11-05"), None),
                dated("c", None, None),
            ],
            first: FirstItems::FromNow,
            folder: None,
            preset_id: None,
        })
        .unwrap();
    let source = test.source(id);
    assert_eq!(source.downloaded_count, 1);
    assert_eq!(source.latest_post.as_deref(), Some("2026-11-05"));

    test.downloaded("c");
    test.downloaded("not-its-own");
    assert_eq!(test.source(id).downloaded_count, 2);

    let empty = test.add_at(
        "https://www.youtube.com/@empty/videos",
        &[],
        FirstItems::All,
    );
    let source = test.source(empty);
    assert_eq!(source.downloaded_count, 0);
    assert_eq!(source.latest_post, None);
}

#[test]
fn check_all_checks_each_source_once_in_the_order_given() {
    let test = Test::new();
    let first = test.add(&["a"], FirstItems::FromNow);
    let second = test.add_at(
        "https://www.youtube.com/@other/videos",
        &["a"],
        FirstItems::FromNow,
    );
    test.reader.lists(&["n", "a"]);
    test.sources
        .check_many(&[second, 999, first, second])
        .unwrap();
    wait_until("both checks end", || {
        test.reader.reads.load(Ordering::SeqCst) == 2
            && test
                .sources
                .list()
                .unwrap()
                .iter()
                .all(|source| !source.checking)
    });
    assert_eq!(test.new_ids(first), ["n"]);
    assert_eq!(test.new_ids(second), ["n"]);
    assert_eq!(
        test.reader.reads.load(Ordering::SeqCst),
        2,
        "a source asked twice is read once, and one not watched isn't read"
    );
    assert_eq!(
        test.reader.started(),
        ["https://www.youtube.com/@other/videos", CHANNEL],
        "both are on YouTube, so they're read in the order given"
    );
}

#[test]
fn a_check_on_one_site_doesnt_wait_for_one_on_another() {
    const INSTAGRAM: &str = "https://www.instagram.com/someone/posts/";
    let test = Test::new();
    let instagram = test.add_at(INSTAGRAM, &["a"], FirstItems::FromNow);
    let x = test.add_at("https://x.com/someone/media", &["a"], FirstItems::FromNow);
    test.reader.lists(&["n", "a"]);
    test.reader.hold(INSTAGRAM);
    test.sources.check_many(&[instagram, x]).unwrap();
    wait_until("the X account's check ends", || !test.source(x).checking);
    assert_eq!(test.new_ids(x), ["n"]);
    assert!(
        test.source(instagram).checking,
        "the Instagram account's check still runs"
    );
    test.reader.release(INSTAGRAM);
    wait_until("the Instagram account's check ends", || {
        !test.source(instagram).checking
    });
    assert_eq!(test.new_ids(instagram), ["n"]);
}

#[test]
fn checks_on_one_site_run_one_after_the_other() {
    const FIRST: &str = "https://x.com/first/media";
    // twitter.com is X too, so it waits its turn behind x.com.
    const SECOND: &str = "https://twitter.com/second/media";
    let test = Test::new();
    let first = test.add_at(FIRST, &["a"], FirstItems::FromNow);
    let second = test.add_at(SECOND, &["a"], FirstItems::FromNow);
    test.reader.lists(&["n", "a"]);
    test.reader.hold(FIRST);
    test.sources.check_many(&[first, second]).unwrap();
    wait_until("the first check starts", || {
        test.reader.started() == [FIRST]
    });
    thread::sleep(Duration::from_millis(100));
    assert_eq!(
        test.reader.started(),
        [FIRST],
        "the second waits for the first"
    );
    assert!(test.source(second).checking);
    test.reader.release(FIRST);
    wait_until("both checks end", || {
        !test.source(first).checking && !test.source(second).checking
    });
    assert_eq!(test.reader.started(), [FIRST, SECOND]);
    assert_eq!(test.new_ids(second), ["n"]);
}

#[test]
fn downloading_from_every_source_queues_each_with_its_own_choices() {
    let test = Test::new();
    let first = test.add(&["a", "shared", "b"], FirstItems::All);
    let second = test.add_at(
        "https://www.youtube.com/@other/videos",
        &["shared", "c", "d"],
        FirstItems::All,
    );
    test.sources
        .edit(second, Some("Other".to_owned()), None)
        .unwrap();
    let added = test
        .sources
        .download_new(&[key("a"), key("shared"), key("c")], &[key("b"), key("d")])
        .unwrap();
    assert_eq!(added, 3, "the item both found is queued once");
    let queued: Vec<(String, bool)> = test
        .queue
        .items()
        .into_iter()
        .map(|item| (item.video.video_id, item.folder_chosen))
        .collect();
    assert_eq!(queued.len(), 3);
    assert!(
        queued.contains(&("c".to_owned(), true)),
        "with its source's folder"
    );
    assert!(queued.contains(&("a".to_owned(), false)));
    assert!(
        queued.contains(&("shared".to_owned(), false)),
        "from the source added first"
    );
    let source_of = |id: &str| {
        test.queue
            .items()
            .into_iter()
            .find(|item| item.video.video_id == id)
            .and_then(|item| item.source)
            .map(|source| source.url)
    };
    assert_eq!(
        source_of("a").as_deref(),
        Some(CHANNEL),
        "the Queue shows each under its own source"
    );
    assert_eq!(source_of("shared").as_deref(), Some(CHANNEL));
    assert_eq!(
        source_of("c").as_deref(),
        Some("https://www.youtube.com/@other/videos")
    );

    let first = test.source(first);
    assert!(first.new_items.is_empty());
    assert_eq!(ids(&first.skipped_items), ["b"]);
    let second = test.source(second);
    assert!(second.new_items.is_empty(), "the shared item left both");
    assert_eq!(ids(&second.skipped_items), ["d"]);
}

#[test]
fn downloading_from_every_source_can_leave_the_rest_new() {
    let test = Test::new();
    let id = test.add(&["a", "b"], FirstItems::All);
    test.sources.download_new(&[key("b")], &[]).unwrap();
    assert_eq!(test.new_ids(id), ["a"]);
    assert!(test.source(id).skipped_items.is_empty());
}

#[test]
fn skip_all_of_every_source_skips_what_it_shows() {
    let test = Test::new();
    let first = test.add(&["a", "shared", "b"], FirstItems::All);
    let second = test.add_at(
        "https://www.youtube.com/@other/videos",
        &["shared", "c", "d"],
        FirstItems::All,
    );
    test.sources
        .skip_new(&[key("a"), key("shared"), key("c")])
        .unwrap();
    assert_eq!(ids(&test.source(first).skipped_items), ["a", "shared"]);
    assert_eq!(test.new_ids(first), ["b"], "what it didn't show stays new");
    assert_eq!(ids(&test.source(second).skipped_items), ["shared", "c"]);
    assert_eq!(test.new_ids(second), ["d"]);
}

#[test]
fn downloading_skips_only_the_items_the_picker_showed() {
    let test = Test::new();
    let id = test.add(&["a", "b", "c", "d"], FirstItems::All);
    // The picker showed a, b and c (d is another type): a is picked, the rest shown is skipped, d stays new.
    test.sources
        .download(id, &[key("a")], &[key("a"), key("b"), key("c")])
        .unwrap();
    let source = test.source(id);
    assert_eq!(ids(&source.new_items), ["d"]);
    assert_eq!(
        ids(&source.skipped_items),
        ["b", "c"],
        "a picked item is never skipped"
    );
    assert_eq!(test.queue.items().len(), 1);

    // Skip All skips what it's given, only while it's new: the queued item stays queued.
    test.sources.skip(id, &[key("a"), key("d")]).unwrap();
    let source = test.source(id);
    assert!(source.new_items.is_empty());
    assert_eq!(ids(&source.skipped_items), ["b", "c", "d"]);
}

#[test]
fn downloading_from_every_source_skips_only_what_it_showed() {
    let test = Test::new();
    let first = test.add(&["a", "b"], FirstItems::All);
    let second = test.add_at(
        "https://www.youtube.com/@other/videos",
        &["c", "d"],
        FirstItems::All,
    );
    test.sources.download_new(&[key("a")], &[key("c")]).unwrap();
    assert_eq!(test.new_ids(first), ["b"]);
    assert_eq!(ids(&test.source(second).skipped_items), ["c"]);
    assert_eq!(test.new_ids(second), ["d"]);
}
