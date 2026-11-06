//! The pending links, read by a fake reader the tests answer for, and started into a queue whose downloads fail at
//! once, so what was added stays in it to look at.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::data_folder::DataPaths;
use crate::downloads::{
    DownloadProgress, JobOutcome, JobRunner, JobSpec, QueueItem, QueueListener, QueueServices,
};
use crate::history::NewDownload;
use crate::metadata::{FileContents, PostContents};
use crate::tools::ToolManager;

/// Answers what each link is: a YouTube video, an Instagram post or a MEGA folder named after the link's end, unless
/// a test gave another answer. A held link isn't answered until it's let go, or its read is cancelled.
#[derive(Default)]
struct FakeReader {
    reads: Mutex<Vec<(String, Instant)>>,
    answers: Mutex<HashMap<String, Result<Fetched, PreviewError>>>,
    held: Mutex<HashSet<String>>,
}

impl FakeReader {
    fn answer(&self, link: &str, answer: Result<Fetched, PreviewError>) {
        lock(&self.answers).insert(link.to_owned(), answer);
    }

    fn hold(&self, link: &str) {
        lock(&self.held).insert(link.to_owned());
    }

    fn release(&self, link: &str) {
        lock(&self.held).remove(link);
    }

    fn reads(&self) -> Vec<String> {
        lock(&self.reads)
            .iter()
            .map(|(link, _)| link.clone())
            .collect()
    }

    fn read_at(&self, link: &str) -> Instant {
        lock(&self.reads)
            .iter()
            .find(|(read, _)| read == link)
            .map(|(_, at)| *at)
            .unwrap()
    }
}

impl LinkReader for FakeReader {
    fn read(&self, link: &str, cancel: &CancelToken) -> Result<Fetched, PreviewError> {
        lock(&self.reads).push((link.to_owned(), Instant::now()));
        while lock(&self.held).contains(link) {
            if cancel.is_cancelled() {
                return Err(PreviewError::Cancelled);
            }
            thread::sleep(Duration::from_millis(5));
        }
        lock(&self.answers)
            .get(link)
            .cloned()
            .unwrap_or_else(|| Ok(fetched(video_at(link))))
    }
}

/// Every download fails at once, so the queue keeps what was added.
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

impl PendingListener for CountingListener {
    fn changed(&self, _links: &[PendingLink]) {
        self.changes.fetch_add(1, Ordering::SeqCst);
    }
}

struct Test {
    _folder: tempfile::TempDir,
    database: Arc<Database>,
    history: Arc<History>,
    queue: Arc<DownloadQueue>,
    presets: Arc<PresetManager>,
    settings: Arc<SettingsStore>,
    reader: Arc<FakeReader>,
    listener: Arc<CountingListener>,
    links: Arc<PendingLinks>,
}

impl Test {
    fn new() -> Self {
        Self::with_pause(PauseRange { min: 0, max: 0 })
    }

    /// With this pause between reads of a paced site's links.
    fn with_pause(pause: PauseRange) -> Self {
        let folder = tempfile::tempdir().unwrap();
        let database = Arc::new(Database::open_in_memory().unwrap());
        let history = Arc::new(
            History::new(Arc::clone(&database), folder.path().join("archive.txt")).unwrap(),
        );
        let settings = Arc::new(SettingsStore::open(folder.path().join("settings.json")).unwrap());
        let downloads = folder.path().join("downloads").display().to_string();
        settings
            .update(|settings| {
                // No cookies, so the tests never look at this PC's browsers.
                settings.downloads.cookies_browser = None;
                settings.downloads.folder = Some(downloads.clone());
                settings.downloads.post_pause = pause;
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
        let listener = Arc::new(CountingListener::default());
        let links = Self::open_links(
            &database, &history, &queue, &presets, &settings, &reader, &listener,
        );
        Self {
            _folder: folder,
            database,
            history,
            queue,
            presets,
            settings,
            reader,
            listener,
            links,
        }
    }

    fn open_links(
        database: &Arc<Database>,
        history: &Arc<History>,
        queue: &Arc<DownloadQueue>,
        presets: &Arc<PresetManager>,
        settings: &Arc<SettingsStore>,
        reader: &Arc<FakeReader>,
        listener: &Arc<CountingListener>,
    ) -> Arc<PendingLinks> {
        PendingLinks::open(PendingServices {
            database: Arc::clone(database),
            history: Arc::clone(history),
            queue: Arc::clone(queue),
            presets: Arc::clone(presets),
            settings: Arc::clone(settings),
            reader: reader.clone() as Arc<dyn LinkReader>,
            listener: listener.clone() as Arc<dyn PendingListener>,
        })
        .unwrap()
    }

    fn reopen(&self) -> Arc<PendingLinks> {
        Self::open_links(
            &self.database,
            &self.history,
            &self.queue,
            &self.presets,
            &self.settings,
            &self.reader,
            &self.listener,
        )
    }

    fn add(&self, links: &[&str]) -> AddedLinks {
        let links: Vec<String> = links.iter().map(|link| (*link).to_owned()).collect();
        self.links.add(&links).unwrap()
    }

    fn link(&self, link: &str) -> Option<PendingLink> {
        self.links
            .list()
            .into_iter()
            .find(|pending| pending.link == link)
    }

    fn state(&self, link: &str) -> Option<PendingState> {
        self.link(link).map(|pending| pending.state)
    }

    fn wait_until_read(&self, link: &str) {
        wait_until(&format!("{link} is read"), || {
            matches!(
                self.state(link),
                Some(PendingState::Ready | PendingState::Failed)
            )
        });
    }

    fn queued(&self, video_id: &str) -> Option<QueueItem> {
        self.queue
            .items()
            .into_iter()
            .find(|item| item.video.video_id == video_id)
    }
}

const FIRST: &str = "https://www.youtube.com/watch?v=first";
const SECOND: &str = "https://www.youtube.com/watch?v=second";
const THIRD: &str = "https://www.youtube.com/watch?v=third";

/// A YouTube video, an Instagram post for an Instagram link or a shared folder for a MEGA link, whose ID is what ends
/// the link.
fn video_at(link: &str) -> VideoInfo {
    let id = link
        .trim_end_matches('/')
        .rsplit(['=', '/'])
        .next()
        .unwrap()
        .to_owned();
    let post = link.contains("instagram.com");
    let shared = link.contains("mega.nz");
    let (site, site_name) = match (post, shared) {
        (true, _) => ("instagram", "instagram.com"),
        (_, true) => ("mega", "mega.nz"),
        _ => ("youtube", "youtube.com"),
    };
    VideoInfo {
        site: site.to_owned(),
        site_name: site_name.to_owned(),
        video_id: id.clone(),
        url: link.to_owned(),
        title: format!("Title of {id}"),
        uploader: None,
        duration_seconds: (!post && !shared).then_some(10.0),
        upload_date: None,
        thumbnail_url: None,
        best_height: (!post && !shared).then_some(1080),
        post: post.then_some(PostContents {
            images: 2,
            videos: 0,
        }),
        files: shared.then_some(FileContents {
            count: 3,
            bytes: 1000,
        }),
    }
}

fn fetched(video: VideoInfo) -> Fetched {
    Fetched {
        video,
        missing_js_runtime: false,
    }
}

/// Waits for something the reading thread does.
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
fn finds_the_links_in_pasted_text() {
    let text = "Look: https://www.youtube.com/watch?v=a, and (youtu.be/b)\n\
        <https://x.com/someone/status/1> twice https://www.youtube.com/watch?v=a\n\
        not links: hello, someone@example.com, ftp://example.com/file, C:\\Videos\\clip.mp4";
    assert_eq!(
        links_in(text),
        [
            "https://www.youtube.com/watch?v=a",
            "https://youtu.be/b",
            "https://x.com/someone/status/1",
        ]
    );
    assert_eq!(links_in("   "), Vec::<String>::new());
}

#[test]
fn reads_each_link_once_in_the_order_added() {
    let test = Test::new();
    let added = test.add(&[FIRST, SECOND, "not a link", THIRD]);
    assert_eq!(
        added,
        AddedLinks {
            added: 3,
            already_pending: 0
        }
    );
    for link in [FIRST, SECOND, THIRD] {
        test.wait_until_read(link);
    }
    assert_eq!(test.reader.reads(), [FIRST, SECOND, THIRD]);

    let first = test.link(FIRST).unwrap();
    let preview = first.preview.unwrap();
    assert_eq!(preview.video.video_id, "first");
    assert_eq!(preview.last_download, None);
    assert!(!preview.in_queue);
    assert!(test.listener.changes.load(Ordering::SeqCst) > 0);

    assert_eq!(
        test.add(&[SECOND, "https://www.youtube.com/watch?v=fourth"]),
        AddedLinks {
            added: 1,
            already_pending: 1
        },
        "a link already here is left as it is"
    );
}

#[test]
fn starting_a_read_link_puts_it_in_the_queue() {
    let test = Test::new();
    let post = "https://www.instagram.com/p/Post1/";
    let shared = "https://mega.nz/folder/Folder1";
    test.add(&[FIRST, post, shared]);
    for link in [FIRST, post, shared] {
        test.wait_until_read(link);
    }
    let ids: Vec<i64> = test.links.list().iter().map(|link| link.id).collect();

    test.links.start(&ids, "audio-mp3").unwrap();
    assert!(test.links.list().is_empty(), "started links leave");
    let video = test.queued("first").unwrap();
    assert_eq!(video.preset.unwrap().id, "audio-mp3");
    let post = test.queued("Post1").unwrap();
    assert_eq!(post.preset, None, "a post has no preset");
    let shared = test.queued("Folder1").unwrap();
    assert_eq!(shared.preset, None, "nor has a shared folder");
}

#[test]
fn a_links_own_preset_wins() {
    let test = Test::new();
    test.add(&[FIRST]);
    test.wait_until_read(FIRST);
    let id = test.link(FIRST).unwrap().id;
    test.links
        .set_preset(id, Some("best-audio".to_owned()))
        .unwrap();

    test.links.start(&[id], "best-video").unwrap();
    assert_eq!(
        test.queued("first").unwrap().preset.unwrap().id,
        "best-audio"
    );
}

#[test]
fn a_link_started_before_it_is_read_joins_the_queue_once_read() {
    let test = Test::new();
    test.reader.hold(FIRST);
    test.add(&[FIRST]);
    wait_until("it's being read", || {
        test.state(FIRST) == Some(PendingState::Reading)
    });
    let id = test.link(FIRST).unwrap().id;

    test.links.start(&[id], "audio-mp3").unwrap();
    let pending = test.link(FIRST).unwrap();
    assert!(pending.start_when_read);
    assert_eq!(pending.preset_id.as_deref(), Some("audio-mp3"));

    test.reader.release(FIRST);
    wait_until("it's in the queue", || test.queued("first").is_some());
    assert_eq!(
        test.queued("first").unwrap().preset.unwrap().id,
        "audio-mp3"
    );
    wait_until("it leaves", || test.link(FIRST).is_none());
}

#[test]
fn a_link_downloaded_before_waits_for_the_user_even_when_started() {
    let test = Test::new();
    test.history
        .record(
            &video_at(FIRST),
            &NewDownload {
                preset: "Best Video".to_owned(),
                file_path: "first.mkv".to_owned(),
                files: Vec::new(),
                file_size: None,
                format: None,
                downloaded_at: 1,
            },
        )
        .unwrap();
    test.reader.hold(FIRST);
    test.add(&[FIRST]);
    wait_until("it's being read", || {
        test.state(FIRST) == Some(PendingState::Reading)
    });
    let id = test.link(FIRST).unwrap().id;
    test.links.start(&[id], "best-video").unwrap();

    test.reader.release(FIRST);
    test.wait_until_read(FIRST);
    let pending = test.link(FIRST).unwrap();
    assert!(!pending.start_when_read);
    assert!(pending.preview.unwrap().last_download.is_some());
    assert!(test.queued("first").is_none());

    test.links.start(&[id], "best-video").unwrap();
    let queued = test.queued("first").unwrap();
    assert!(
        queued.overwrite,
        "started again with the same preset, it replaces the file"
    );
}

#[test]
fn a_link_that_couldnt_be_read_can_be_read_again() {
    let test = Test::new();
    let error = PreviewError::Network {
        detail: "offline".to_owned(),
    };
    test.reader.answer(FIRST, Err(error.clone()));
    test.add(&[FIRST]);
    test.wait_until_read(FIRST);
    let failed = test.link(FIRST).unwrap();
    assert_eq!(failed.state, PendingState::Failed);
    assert_eq!(failed.error, Some(error));

    test.links.start(&[failed.id], "best-video").unwrap();
    assert!(test.queued("first").is_none(), "it isn't started");

    test.reader.answer(FIRST, Ok(fetched(video_at(FIRST))));
    test.links.read_again(failed.id).unwrap();
    wait_until("it's read", || {
        test.state(FIRST) == Some(PendingState::Ready)
    });
    assert_eq!(test.link(FIRST).unwrap().error, None);
    assert_eq!(
        test.links.read_again(failed.id),
        Err(DownloadError::WrongState)
    );
}

#[test]
fn links_come_back_after_a_restart() {
    let test = Test::new();
    test.add(&[FIRST]);
    test.wait_until_read(FIRST);
    test.reader.hold(SECOND);
    test.add(&[SECOND]);
    wait_until("the second is being read", || {
        test.state(SECOND) == Some(PendingState::Reading)
    });

    test.reader.release(SECOND);
    let reopened = test.reopen();
    let links = reopened.list();
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].link, FIRST);
    assert_eq!(links[0].state, PendingState::Ready);
    assert_eq!(links[0].preview.as_ref().unwrap().video.video_id, "first");
    wait_until("the second is read again", || {
        reopened
            .list()
            .iter()
            .any(|link| link.link == SECOND && link.state == PendingState::Ready)
    });
}

#[test]
fn a_paced_sites_links_wait_while_others_are_read() {
    let test = Test::with_pause(PauseRange { min: 1, max: 1 });
    let first_post = "https://www.instagram.com/p/One/";
    let second_post = "https://www.instagram.com/p/Two/";
    test.add(&[first_post, second_post, FIRST]);
    for link in [first_post, second_post, FIRST] {
        test.wait_until_read(link);
    }
    assert_eq!(test.reader.reads(), [first_post, FIRST, second_post]);
    let gap = test.reader.read_at(second_post) - test.reader.read_at(first_post);
    assert!(gap >= Duration::from_secs(1), "waited {gap:?}");
}

#[test]
fn the_same_video_from_two_links_is_kept_once() {
    let test = Test::new();
    let short = "https://youtu.be/first";
    test.add(&[FIRST, short]);
    test.wait_until_read(FIRST);
    wait_until("the second link is read", || test.reader.reads().len() == 2);
    wait_until("it's dropped", || test.links.list().len() == 1);
    assert_eq!(test.links.list()[0].link, FIRST);
}

#[test]
fn removing_a_link_being_read_stops_reading_it() {
    let test = Test::new();
    test.reader.hold(FIRST);
    test.add(&[FIRST, SECOND]);
    wait_until("it's being read", || {
        test.state(FIRST) == Some(PendingState::Reading)
    });
    let id = test.link(FIRST).unwrap().id;

    test.links.remove(&[id]).unwrap();
    assert!(test.link(FIRST).is_none());
    test.wait_until_read(SECOND);
    assert!(test.link(FIRST).is_none(), "it doesn't come back once read");
    assert_eq!(test.reopen().list().len(), 1, "nor after a restart");
}
