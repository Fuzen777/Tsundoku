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
use crate::settings::UnpackPlace;
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

/// Fails each run at once, noting where it would have unpacked its archives (`None` for left packed).
#[derive(Default)]
struct FailingRunner {
    unpacked_to: Mutex<Vec<Option<UnpackPlace>>>,
}

impl FailingRunner {
    fn unpacked_to(&self) -> Vec<Option<UnpackPlace>> {
        lock(&self.unpacked_to).clone()
    }
}

impl JobRunner for FailingRunner {
    fn run(
        &self,
        job: &JobSpec,
        _cancel: &CancelToken,
        _on_progress: &mut dyn FnMut(DownloadProgress),
    ) -> JobOutcome {
        lock(&self.unpacked_to).push(job.unpack.as_ref().map(|unpack| unpack.place));
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
    database: Arc<Database>,
    history: Arc<History>,
    queue: Arc<DownloadQueue>,
    runner: Arc<FailingRunner>,
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
        let runner = Arc::new(FailingRunner::default());
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
            runner: runner.clone() as Arc<dyn JobRunner>,
            listener: Arc::new(NoQueueEvents),
            source_choices: Arc::new(WatchedChoices::new(Arc::clone(&database))),
        })
        .unwrap();
        let reader = Arc::new(FakeReader::default());
        let sources = WatchedSources::open(WatchedServices {
            database: Arc::clone(&database),
            history: Arc::clone(&history),
            queue: Arc::clone(&queue),
            presets,
            settings,
            reader: reader.clone() as Arc<dyn ListingReader>,
            listener: Arc::new(CountingListener::default()),
            pictures_folder: folder.path().join("pictures"),
        });
        Self {
            folder,
            database,
            history,
            queue,
            runner,
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

    /// Adds the channel with these items newest first, all of them new, as checks would have found them.
    fn add_new(&self, ids: &[&str]) -> i64 {
        self.add_new_at(CHANNEL, ids)
    }

    /// Adds the listing at `url` with these items newest first, all of them new, as checks would have found them.
    fn add_new_at(&self, url: &str, ids: &[&str]) -> i64 {
        let id = self.add_at(url, ids, FirstItems::FromNow);
        let keys: Vec<ItemKey> = ids.iter().map(|id| key(id)).collect();
        self.sources
            .store
            .set_state(id, &keys, ItemState::New)
            .unwrap();
        id
    }

    fn queued_ids(&self) -> Vec<String> {
        self.queue
            .items()
            .into_iter()
            .map(|item| item.video.video_id)
            .collect()
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
        tags: Vec::new(),
        pixiv: None,
        fanbox: None,
        links: Vec::new(),
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
fn adding_downloads_what_was_asked_and_knows_the_rest() {
    // Nothing now: a check lists only what came after.
    let test = Test::new();
    let id = test.add(&["a", "b", "c"], FirstItems::FromNow);
    assert!(test.queued_ids().is_empty());
    assert!(test.new_ids(id).is_empty());
    test.reader.lists(&["n", "a", "b", "c"]);
    test.check(id);
    assert_eq!(test.new_ids(id), ["n"]);

    // The latest two listed, but the downloaded one.
    let test = Test::new();
    test.downloaded("b");
    let id = test.add(&["a", "b", "c", "d", "e"], FirstItems::Latest { count: 2 });
    assert_eq!(
        test.queued_ids(),
        ["a"],
        "the downloaded one isn't downloaded again"
    );
    assert!(test.new_ids(id).is_empty());
    test.reader.lists(&["a", "b", "c", "d", "e"]);
    test.check(id);
    assert!(
        test.new_ids(id).is_empty(),
        "what it didn't download is known"
    );

    // Everything listed, but what the history has, each once.
    let test = Test::new();
    test.downloaded("b");
    let id = test.add(&["a", "b", "c", "a"], FirstItems::All);
    assert_eq!(test.queued_ids(), ["a", "c"]);
    let source = test.source(id);
    assert!(source.new_items.is_empty() && source.skipped_items.is_empty());
    assert_eq!(
        test.queue.items()[0]
            .source
            .as_ref()
            .map(|source| source.url.as_str()),
        Some(CHANNEL),
        "the Downloads view shows them under their source"
    );
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
    assert_eq!(
        test.queued_ids().len(),
        2,
        "nothing queued for a refused one"
    );
}

#[test]
fn adding_downloads_the_items_ticked_with_the_sources_choices() {
    let test = Test::new();
    test.downloaded("b");
    test.queue
        .add_many(
            vec![QueuePick {
                video: video("d"),
                preset_id: Some(crate::presets::DEFAULT_PRESET_ID.to_owned()),
                overwrite: false,
                folder: None,
            }],
            None,
        )
        .unwrap();
    let id = test
        .sources
        .add(NewWatchedSource {
            url: CHANNEL.to_owned(),
            kind: ListingKind::Channel,
            name: "Someone".to_owned(),
            site_name: "youtube.com".to_owned(),
            thumbnail_url: None,
            items: ["a", "b", "c", "d"]
                .iter()
                .map(|id| ListedVideo {
                    video: video(id),
                    approximate_date: None,
                })
                .collect(),
            // "x" isn't listed, and "d" is in the queue already.
            first: FirstItems::Picked {
                items: vec![key("b"), key("c"), key("d"), key("x")],
            },
            folder: Some("Channels\\{uploader}".to_owned()),
            preset_id: None,
        })
        .unwrap();
    let queued: Vec<(String, bool, bool)> = test
        .queue
        .items()
        .into_iter()
        .map(|item| (item.video.video_id, item.overwrite, item.folder_chosen))
        .collect();
    assert_eq!(
        queued,
        [
            ("d".to_owned(), false, false),
            ("b".to_owned(), true, true),
            ("c".to_owned(), false, true),
        ],
        "the downloaded one again with the preset it had, replacing its file, both in the source's folder"
    );
    assert!(
        test.queue.items()[1]
            .folder
            .ends_with(&format!("Channels{}Someone", std::path::MAIN_SEPARATOR))
    );
    let source = test.source(id);
    assert!(source.new_items.is_empty() && source.skipped_items.is_empty());
    test.reader.lists(&["a", "b", "c", "d"]);
    test.check(id);
    assert!(
        test.new_ids(id).is_empty(),
        "the one left unticked is known"
    );
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
    let id = test.add_new(&["b", "c", "d"]);
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
    let id = test.add_new(&["b", "c", "old"]);
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
    let id = test.add_new(&["a", "b", "c"]);
    test.sources
        .edit(
            id,
            Some(" Channels\\{uploader} ".to_owned()),
            None,
            None,
            None,
        )
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
    let id = test.add_new(&["a"]);
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
    let first = test.add_new(&["a", "shared", "b"]);
    let second = test.add_new_at(
        "https://www.youtube.com/@other/videos",
        &["shared", "c", "d"],
    );
    test.sources
        .edit(second, Some("Other".to_owned()), None, None, None)
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
    let id = test.add_new(&["a", "b"]);
    test.sources.download_new(&[key("b")], &[]).unwrap();
    assert_eq!(test.new_ids(id), ["a"]);
    assert!(test.source(id).skipped_items.is_empty());
}

#[test]
fn skip_all_of_every_source_skips_what_it_shows() {
    let test = Test::new();
    let first = test.add_new(&["a", "shared", "b"]);
    let second = test.add_new_at(
        "https://www.youtube.com/@other/videos",
        &["shared", "c", "d"],
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
    let id = test.add_new(&["a", "b", "c", "d"]);
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
    let first = test.add_new(&["a", "b"]);
    let second = test.add_new_at("https://www.youtube.com/@other/videos", &["c", "d"]);
    test.sources.download_new(&[key("a")], &[key("c")]).unwrap();
    assert_eq!(test.new_ids(first), ["b"]);
    assert_eq!(ids(&test.source(second).skipped_items), ["c"]);
    assert_eq!(test.new_ids(second), ["d"]);
}

/// A JPEG's first bytes, which is all `picture::PictureKind` looks at, then `tag`, so two pictures differ.
fn jpeg(tag: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0];
    bytes.extend_from_slice(tag.as_bytes());
    bytes
}

/// Reads a picture as the site would answer it: a JPEG for any link but `not-a-picture`, a page.
fn read_picture(url: &str) -> Result<Vec<u8>, WatchedError> {
    if url.ends_with("not-a-picture") {
        Ok(b"<!doctype html><title>Not Found</title>".to_vec())
    } else {
        Ok(jpeg(url))
    }
}

const WHOLE: PictureCrop = PictureCrop {
    x: 0.0,
    y: 0.0,
    width: 1.0,
    height: 1.0,
};

impl Test {
    /// Adds an account with its own picture, whose items "a" (with a picture), "b" (a post of two files, each with
    /// its picture) and "c" (none) are listed newest first.
    fn add_with_pictures(&self) -> i64 {
        let post = VideoInfo {
            thumbnail_url: Some(picture_of("b")),
            post_files: (1..=2)
                .map(|number| crate::metadata::PostFile {
                    number,
                    video: false,
                    thumbnail_url: Some(picture_of(&format!("b{number}"))),
                    picked: true,
                    name: None,
                    size: None,
                })
                .collect(),
            ..video("b")
        };
        let items = [
            VideoInfo {
                thumbnail_url: Some(picture_of("a")),
                ..video("a")
            },
            post,
            video("c"),
        ];
        self.sources
            .add(NewWatchedSource {
                url: CHANNEL.to_owned(),
                kind: ListingKind::Account,
                name: "Someone".to_owned(),
                site_name: "youtube.com".to_owned(),
                thumbnail_url: Some(picture_of("avatar")),
                items: items
                    .into_iter()
                    .map(|video| ListedVideo {
                        video,
                        approximate_date: None,
                    })
                    .collect(),
                first: FirstItems::All,
                folder: None,
                preset_id: None,
            })
            .unwrap()
    }

    fn picture(&self, id: i64) -> Option<SourcePicture> {
        self.source(id).picture
    }

    fn pictures_on_disk(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.folder.path().join("pictures"))
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }
}

#[test]
fn a_sources_pictures_are_its_listings_then_its_items_newest_first() {
    let test = Test::new();
    let id = test.add_with_pictures();
    assert_eq!(
        test.sources.picture_choices(id).unwrap(),
        [
            picture_of("avatar"),
            picture_of("a"),
            // A post's files one by one, rather than its cover, which is the first of them.
            picture_of("b1"),
            picture_of("b2"),
        ]
    );
    assert_eq!(
        test.sources.picture_choices(id + 1),
        Err(WatchedError::NotFound)
    );
}

#[test]
fn a_picked_picture_is_kept_with_its_area_until_the_automatic_one_is_back() {
    let test = Test::new();
    let id = test.add_with_pictures();
    assert_eq!(test.picture(id), None);

    let face = PictureCrop {
        x: 0.25,
        y: 0.1,
        width: 0.5,
        height: 0.4,
    };
    test.sources
        .pick_picture(id, &picture_of("b2"), face, &read_picture)
        .unwrap();
    let picked = test.picture(id).unwrap();
    assert_eq!(picked.crop, face);
    assert_eq!(
        picked.file,
        picture::file_name(id, &jpeg(&picture_of("b2")), picture::PictureKind::Jpeg)
    );
    assert_eq!(test.pictures_on_disk(), std::slice::from_ref(&picked.file));
    assert_eq!(
        test.source(id).thumbnail_url,
        Some(picture_of("avatar")),
        "the listing's picture is still known, for Automatic"
    );

    // Another area of it: the same file.
    test.sources.crop_picture(id, WHOLE).unwrap();
    assert_eq!(
        test.picture(id),
        Some(SourcePicture {
            file: picked.file.clone(),
            crop: WHOLE,
        })
    );

    // Another picture replaces it on the disk too.
    test.sources
        .pick_picture(id, &picture_of("a"), face, &read_picture)
        .unwrap();
    let replaced = test.picture(id).unwrap();
    assert_ne!(replaced.file, picked.file);
    assert_eq!(test.pictures_on_disk(), [replaced.file]);

    // Automatic.
    test.sources.reset_picture(id).unwrap();
    assert_eq!(test.picture(id), None);
    assert!(test.pictures_on_disk().is_empty());
    assert_eq!(
        test.sources.crop_picture(id, face),
        Err(WatchedError::UnknownPicture),
        "there's no picture of its own to crop"
    );
}

#[test]
fn only_a_sources_own_pictures_can_be_picked() {
    let test = Test::new();
    let id = test.add_with_pictures();
    let read_anything =
        |_: &str| -> Result<Vec<u8>, WatchedError> { panic!("only its pictures are read") };
    assert_eq!(
        test.sources
            .pick_picture(id, "https://elsewhere.example/a.jpg", WHOLE, &read_anything),
        Err(WatchedError::UnknownPicture)
    );
    assert_eq!(test.picture(id), None);
}

#[test]
fn a_page_instead_of_a_picture_is_refused_and_leaves_the_picture_as_it_was() {
    let test = Test::new();
    let id = test.add_with_pictures();
    test.sources
        .pick_picture(id, &picture_of("a"), WHOLE, &read_picture)
        .unwrap();
    let kept = test.picture(id);
    let page = |_: &str| read_picture("not-a-picture");
    assert!(matches!(
        test.sources
            .pick_picture(id, &picture_of("b1"), WHOLE, &page),
        Err(WatchedError::PictureUnavailable { .. })
    ));
    let failing = |_: &str| -> Result<Vec<u8>, WatchedError> {
        Err(WatchedError::PictureUnavailable {
            detail: "the site answered 404 Not Found".to_owned(),
        })
    };
    assert!(
        test.sources
            .pick_picture(id, &picture_of("b1"), WHOLE, &failing)
            .is_err()
    );
    assert_eq!(test.picture(id), kept);
    assert_eq!(test.pictures_on_disk().len(), 1);
}

#[test]
fn a_check_keeps_the_picked_picture() {
    let test = Test::new();
    let id = test.add_with_pictures();
    test.sources
        .pick_picture(id, &picture_of("a"), WHOLE, &read_picture)
        .unwrap();
    let picked = test.picture(id);
    *lock(&test.reader.listing) = vec!["d".to_owned(), "a".to_owned()];
    test.check(id);
    assert_eq!(test.picture(id), picked);
}

#[test]
fn stopping_watching_deletes_its_picture_and_opening_deletes_what_no_source_shows() {
    let test = Test::new();
    let id = test.add_with_pictures();
    test.sources
        .pick_picture(id, &picture_of("a"), WHOLE, &read_picture)
        .unwrap();
    let shown = test.picture(id).unwrap().file;
    test.sources.remove(id).unwrap();
    assert!(test.pictures_on_disk().is_empty());

    // Left behind: a picture no source shows, one half written, and a file that isn't one of its pictures.
    let folder = test.folder.path().join("pictures");
    fs::write(folder.join(&shown), b"gone").unwrap();
    fs::write(folder.join("watched-7-0123456789abcdef.png.part"), b"half").unwrap();
    fs::write(folder.join("notes.txt"), b"someone's").unwrap();
    WatchedSources::open(WatchedServices {
        database: Arc::clone(&test.database),
        history: Arc::clone(&test.history),
        queue: Arc::clone(&test.queue),
        presets: Arc::clone(&test.sources.presets),
        settings: Arc::clone(&test.sources.settings),
        reader: test.reader.clone() as Arc<dyn ListingReader>,
        listener: Arc::new(CountingListener::default()),
        pictures_folder: folder,
    });
    assert_eq!(test.pictures_on_disk(), ["notes.txt"]);
}

#[test]
fn an_area_is_kept_inside_its_picture() {
    let checked = |x: f64, y: f64, width: f64, height: f64| {
        PictureCrop {
            x,
            y,
            width,
            height,
        }
        .checked()
    };
    assert_eq!(checked(0.2, 0.3, 0.5, 0.5), checked(0.2, 0.3, 0.5, 0.5));
    assert_eq!(
        checked(0.8, -0.5, 0.5, 2.0),
        PictureCrop {
            x: 0.5,
            y: 0.0,
            width: 0.5,
            height: 1.0,
        }
    );
    assert_eq!(checked(f64::NAN, 0.0, f64::INFINITY, 0.0001).width, 1.0);
    assert_eq!(checked(f64::NAN, 0.0, 1.0, 0.0001).height, 0.01);
    assert_eq!(checked(f64::NAN, 0.0, 1.0, 1.0).x, 0.0);
}

#[test]
fn only_the_names_it_gives_are_pictures() {
    let name = picture::file_name(12, b"picture", picture::PictureKind::Webp);
    assert!(name.starts_with("watched-12-") && name.ends_with(".webp"));
    assert_eq!(
        picture::kind_of_file(&name),
        Some(picture::PictureKind::Webp)
    );
    for refused in [
        "watched-12-0123456789abcdef.exe",
        "watched--0123456789abcdef.jpg",
        "watched-12-0123456789ABCDEF.jpg",
        "watched-12-0123.jpg",
        "../watched-12-0123456789abcdef.jpg",
        "watched-12-0123456789abcdef.jpg/x",
        "youtube-0123456789abcdef.jpg",
    ] {
        assert_eq!(picture::kind_of_file(refused), None, "{refused}");
    }
    assert_eq!(
        picture::PictureKind::of(b"\x89PNG\r\n\x1a\n...."),
        Some(picture::PictureKind::Png)
    );
    assert_eq!(
        picture::PictureKind::of(b"RIFF\0\0\0\0WEBPVP8 "),
        Some(picture::PictureKind::Webp)
    );
    assert_eq!(
        picture::PictureKind::of(b"GIF89a"),
        Some(picture::PictureKind::Gif)
    );
    assert_eq!(picture::PictureKind::of(b"<html>"), None);
}

#[test]
fn a_source_takes_settings_new_items_start_until_it_has_its_own() {
    let test = Test::new();
    let id = test.add_new(&["a"]);
    assert_eq!(test.source(id).new_items_start, None);

    test.sources
        .edit(
            id,
            Some("Folder".to_owned()),
            None,
            Some(NewItemsStart::Ticked),
            None,
        )
        .unwrap();
    let source = test.source(id);
    assert_eq!(source.new_items_start, Some(NewItemsStart::Ticked));
    assert_eq!(source.folder.as_deref(), Some("Folder"));

    test.sources
        .edit(id, None, None, Some(NewItemsStart::Unticked), None)
        .unwrap();
    assert_eq!(
        test.source(id).new_items_start,
        Some(NewItemsStart::Unticked)
    );
    test.sources.edit(id, None, None, None, None).unwrap();
    assert_eq!(test.source(id).new_items_start, None);

    // A choice from a newer version is taken as none: Settings' applies.
    test.database
        .lock()
        .execute(
            "UPDATE watched_sources SET new_items_start = 'askEachTime' WHERE id = ?1",
            [id],
        )
        .unwrap();
    assert_eq!(test.source(id).new_items_start, None);
}

#[test]
fn a_sources_unpack_archives_is_read_when_its_downloads_run() {
    let test = Test::new();
    let id = test.add_new(&["a"]);
    assert_eq!(test.source(id).unpack, None);
    let ran = |times: usize| {
        wait_until("the download ran", || {
            test.runner.unpacked_to().len() == times
                && test
                    .queue
                    .items()
                    .iter()
                    .all(|item| item.state == crate::downloads::QueueState::Failed)
        });
        test.runner.unpacked_to()[times - 1]
    };

    test.sources
        .edit(id, None, None, None, Some(RuleUnpacking::InPlace))
        .unwrap();
    assert_eq!(test.source(id).unpack, Some(RuleUnpacking::InPlace));
    test.sources.download(id, &[key("a")], &[]).unwrap();
    assert_eq!(
        ran(1),
        Some(UnpackPlace::InPlace),
        "the source's own choice"
    );
    let item = test.queue.items()[0].id;

    test.sources
        .edit(id, None, None, None, Some(RuleUnpacking::Off))
        .unwrap();
    test.queue.retry(item).unwrap();
    assert_eq!(
        ran(2),
        None,
        "a change reaches the download waiting: it's asked when it runs"
    );

    test.sources.edit(id, None, None, None, None).unwrap();
    assert_eq!(test.source(id).unpack, None);
    test.queue.retry(item).unwrap();
    assert_eq!(ran(3), Some(UnpackPlace::Smart), "Default: Settings' place");

    // A choice from a newer version is taken as none: the rules and Settings decide.
    test.database
        .lock()
        .execute(
            "UPDATE watched_sources SET unpack = 'askEachTime' WHERE id = ?1",
            [id],
        )
        .unwrap();
    assert_eq!(test.source(id).unpack, None);

    test.sources
        .edit(id, None, None, None, Some(RuleUnpacking::ArchiveFolder))
        .unwrap();
    test.sources.remove(id).unwrap();
    test.queue.retry(item).unwrap();
    assert_eq!(
        ran(4),
        Some(UnpackPlace::Smart),
        "once it's no longer watched, its downloads follow Settings again"
    );
}
