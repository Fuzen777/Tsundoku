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
use crate::metadata::{FileContents, PostContents, SignInCookies};
use crate::rules::DownloadKind;
use crate::settings::ClipboardShortcut;
use crate::tools::ToolManager;

/// Answers what each link is: a YouTube video, an Instagram post or a MEGA folder named after the link's end, unless
/// a test gave another answer. A held link isn't answered until it's let go, or its read is cancelled.
#[derive(Default)]
struct FakeReader {
    /// Each link read, when, and whether yt-dlp could look at any page.
    reads: Mutex<Vec<(String, Instant, bool)>>,
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
            .map(|(link, _, _)| link.clone())
            .collect()
    }

    fn read_at(&self, link: &str) -> Instant {
        lock(&self.reads)
            .iter()
            .find(|(read, _, _)| read == link)
            .map(|(_, at, _)| *at)
            .unwrap()
    }

    fn read_any_page(&self, link: &str) -> bool {
        lock(&self.reads)
            .iter()
            .find(|(read, _, _)| read == link)
            .map(|(_, _, any_page)| *any_page)
            .unwrap()
    }
}

impl LinkReader for FakeReader {
    fn read(
        &self,
        link: &str,
        any_page: bool,
        cancel: &CancelToken,
    ) -> Result<Fetched, PreviewError> {
        lock(&self.reads).push((link.to_owned(), Instant::now(), any_page));
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
    /// How many links the last change listed.
    last_count: AtomicUsize,
    /// The copied links announced as read: the video's ID, and whether it started.
    copied: Mutex<Vec<(String, bool)>>,
    /// The options as each change saved them.
    options: Mutex<Vec<AddingSettings>>,
}

impl PendingListener for CountingListener {
    fn changed(&self, links: &[PendingLink]) {
        self.changes.fetch_add(1, Ordering::SeqCst);
        self.last_count.store(links.len(), Ordering::SeqCst);
    }

    fn copied_link_read(&self, video: &VideoInfo, started: bool) {
        lock(&self.copied).push((video.video_id.clone(), started));
    }

    fn options_changed(&self, options: &AddingSettings) {
        lock(&self.options).push(*options);
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

    /// Links copied in another app, as the clipboard watcher hands them over.
    fn copy(&self, links: &[&str]) -> AddedLinks {
        let links: Vec<String> = links.iter().map(|link| (*link).to_owned()).collect();
        self.links.catch(&links).unwrap()
    }

    fn download_right_away(&self, on: bool) {
        let options = AddingSettings {
            watch_clipboard: true,
            download_right_away: on,
            ..AddingSettings::default()
        };
        assert_eq!(self.links.set_options(options).unwrap(), options);
    }

    /// Waits until a link was read and then left.
    fn wait_until_gone(&self, link: &str) {
        wait_until(&format!("{link} is read and gone"), || {
            self.reader.reads().iter().any(|read| read == link) && self.link(link).is_none()
        });
    }

    fn record_download(&self, video: &VideoInfo) {
        self.history
            .record(
                video,
                &NewDownload {
                    preset: "Best Video".to_owned(),
                    file_path: format!("{}.mkv", video.video_id),
                    files: Vec::new(),
                    file_size: None,
                    format: None,
                    downloaded_at: 1,
                    kind: DownloadKind::Video,
                },
            )
            .unwrap();
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
        account: None,
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
        post_files: Vec::new(),
        playlist: None,
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
fn finds_the_page_a_browser_sends() {
    let video = "https://www.youtube.com/watch?v=jNQXAC9IVRw";
    let sent = "tsundoku://add?url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3DjNQXAC9IVRw";
    assert_eq!(links_in_sent_link(sent), [video]);
    assert_eq!(
        links_in_sent_link(&sent.replace("add?", "add/?")),
        [video],
        "a browser may add a slash"
    );
    assert_eq!(
        links_in_sent_link(&format!(
            "{sent}&url=https%3A%2F%2Fx.com%2Fsomeone%2Fstatus%2F1&url={}",
            "https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3DjNQXAC9IVRw"
        )),
        [video, "https://x.com/someone/status/1"],
        "each link once"
    );
    for other in [
        "tsundoku://open?url=https%3A%2F%2Fwww.youtube.com%2F",
        "https://add?url=https%3A%2F%2Fwww.youtube.com%2F",
        "tsundoku://add?link=https%3A%2F%2Fwww.youtube.com%2F",
        "tsundoku://add?url=about%3Ablank",
        "--in-tray",
    ] {
        assert!(links_in_sent_link(other).is_empty(), "{other}");
    }
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
fn a_links_own_folder_goes_with_it_to_the_queue_and_through_a_restart() {
    let test = Test::new();
    let post = "https://www.instagram.com/p/Post1/";
    test.add(&[FIRST, post]);
    for link in [FIRST, post] {
        test.wait_until_read(link);
    }
    let picked = r"D:\Picked".to_owned();
    for link in test.links.list() {
        test.links
            .set_folder(link.id, Some(picked.clone()))
            .unwrap();
    }
    let reopened = test.reopen();
    assert!(
        reopened
            .list()
            .iter()
            .all(|link| link.folder.as_deref() == Some(picked.as_str())),
        "kept through a restart"
    );

    let ids: Vec<i64> = test.links.list().iter().map(|link| link.id).collect();
    test.links.start(&ids, "best-video").unwrap();
    for video_id in ["first", "Post1"] {
        let item = test.queued(video_id).unwrap();
        assert_eq!(
            (item.folder.as_str(), item.folder_chosen),
            (picked.as_str(), true)
        );
    }
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

/// A rule below the catch-all that matches titles containing `title` and does `actions` (a preset, Download Right
/// Away) without picking a folder.
fn add_rule(test: &Test, title: &str, actions: rules::FolderRule) {
    test.settings
        .update(|settings| {
            let at = settings.rules.list.len() - 1;
            settings.rules.list.insert(
                at,
                rules::FolderRule {
                    conditions: vec![rules::RuleCondition {
                        field: rules::RuleField::Title,
                        operator: rules::RuleOperator::Contains,
                        value: title.to_owned(),
                        case_sensitive: false,
                    }],
                    ..actions
                },
            );
        })
        .unwrap();
}

/// A rule's actions: none, for a test to set the ones it wants.
fn no_actions() -> rules::FolderRule {
    rules::FolderRule {
        id: String::new(),
        enabled: true,
        conditions: Vec::new(),
        folder: String::new(),
        preset_id: None,
        download_right_away: None,
        join: rules::ConditionJoin::All,
    }
}

#[test]
fn a_rule_picks_the_preset_unless_the_link_has_its_own() {
    let test = Test::new();
    add_rule(
        &test,
        "Title of",
        rules::FolderRule {
            preset_id: Some("audio-mp3".to_owned()),
            ..no_actions()
        },
    );
    test.add(&[FIRST, SECOND]);
    test.wait_until_read(FIRST);
    test.wait_until_read(SECOND);
    let first = test.link(FIRST).unwrap().id;
    let second = test.link(SECOND).unwrap().id;
    test.links
        .set_preset(second, Some("best-audio".to_owned()))
        .unwrap();

    test.reader.hold(THIRD);
    test.add(&[THIRD]);
    wait_until("the third is being read", || {
        test.state(THIRD) == Some(PendingState::Reading)
    });
    let third = test.link(THIRD).unwrap().id;
    test.links
        .start(&[first, second, third], "1080p-mp4")
        .unwrap();
    test.reader.release(THIRD);
    wait_until("the third is in the queue", || {
        test.queued("third").is_some()
    });

    let preset_of = |id: &str| test.queued(id).unwrap().preset.unwrap().id;
    assert_eq!(
        preset_of("first"),
        "audio-mp3",
        "the rule's, over Start All's"
    );
    assert_eq!(preset_of("second"), "best-audio", "the link's own");
    assert_eq!(
        preset_of("third"),
        "audio-mp3",
        "started before it was read, the rule's"
    );
}

#[test]
fn a_rules_deleted_preset_is_passed_over() {
    let test = Test::new();
    add_rule(
        &test,
        "Title of",
        rules::FolderRule {
            preset_id: Some("custom-9".to_owned()),
            ..no_actions()
        },
    );
    test.add(&[FIRST]);
    test.wait_until_read(FIRST);
    let id = test.link(FIRST).unwrap().id;
    test.links.start(&[id], "best-audio").unwrap();
    assert_eq!(
        test.queued("first").unwrap().preset.unwrap().id,
        "best-audio"
    );
}

#[test]
fn a_rule_turns_download_right_away_on_or_off() {
    let test = Test::new();
    test.download_right_away(false);
    add_rule(
        &test,
        "first",
        rules::FolderRule {
            download_right_away: Some(true),
            preset_id: Some("best-audio".to_owned()),
            ..no_actions()
        },
    );
    add_rule(
        &test,
        "second",
        rules::FolderRule {
            download_right_away: Some(false),
            ..no_actions()
        },
    );

    test.copy(&[FIRST]);
    wait_until("the copied first is in the queue", || {
        test.queued("first").is_some()
    });
    assert_eq!(
        test.queued("first").unwrap().preset.unwrap().id,
        "best-audio",
        "with the rule's preset"
    );
    test.links.add_sent(&[THIRD.to_owned()]).unwrap();
    test.wait_until_read(THIRD);
    assert!(test.queued("third").is_none(), "no rule: Settings, off");
    let pasted = "https://www.youtube.com/watch?v=first-pasted";
    test.add(&[pasted]);
    test.wait_until_read(pasted);
    assert!(
        test.queued("first-pasted").is_none(),
        "a link added by hand still waits"
    );

    test.download_right_away(true);
    test.links.add_sent(&[SECOND.to_owned()]).unwrap();
    test.wait_until_read(SECOND);
    assert!(test.queued("second").is_none(), "the rule turns it off");
    let post = "https://www.instagram.com/p/Post1/";
    test.links.add_sent(&[post.to_owned()]).unwrap();
    wait_until("the sent post is in the queue", || {
        test.queued("Post1").is_some()
    });
}

#[test]
fn a_sent_link_still_follows_download_right_away_after_a_restart() {
    let test = Test::new();
    test.download_right_away(true);
    test.reader.hold(FIRST);
    test.links.add_sent(&[FIRST.to_owned()]).unwrap();
    wait_until("it's being read", || {
        test.state(FIRST) == Some(PendingState::Reading)
    });
    assert!(test.link(FIRST).unwrap().start_when_read);
    let reopened = test.reopen();
    assert!(reopened.list()[0].start_when_read);
    test.download_right_away(false);
    assert!(
        !reopened.list()[0].start_when_read,
        "it goes by Download Right Away, not a start"
    );
    test.reader.release(FIRST);
}

#[test]
fn a_link_downloaded_before_waits_for_the_user_even_when_started() {
    let test = Test::new();
    test.record_download(&video_at(FIRST));
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
fn a_site_that_asks_to_slow_down_is_read_again_once_its_wait_ends() {
    let test = Test::new();
    let busy = "https://www.instagram.com/p/Busy/";
    let next = "https://www.instagram.com/p/Next/";
    test.reader.answer(
        busy,
        Err(PreviewError::too_many_requests(
            "HttpError: '429 Too Many Requests'".to_owned(),
        )),
    );
    test.add(&[busy, next, FIRST]);
    test.wait_until_read(FIRST);
    let pending = test.link(busy).unwrap();
    assert_eq!(pending.state, PendingState::Waiting, "it didn't fail");
    assert_eq!(pending.error, None);
    assert_eq!(
        pending.hold.map(|wait| wait.site),
        Some("Instagram".to_owned())
    );
    assert!(
        test.link(next).unwrap().hold.is_some(),
        "the site's other links wait too"
    );
    assert_eq!(
        test.reader.reads(),
        [busy, FIRST],
        "other sites' links are read meanwhile"
    );
    assert!(test.link(FIRST).unwrap().hold.is_none());
    assert!(
        test.queue.site_wait(next).is_some(),
        "the site's downloads wait too"
    );

    lock(&test.reader.answers).remove(busy);
    test.queue.end_site_waits();
    test.wait_until_read(busy);
    test.wait_until_read(next);
    assert_eq!(test.state(busy), Some(PendingState::Ready));
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

#[test]
fn undo_puts_removed_links_back_as_they_were() {
    let test = Test::new();
    test.reader.answer(THIRD, Err(PreviewError::Playlist));
    test.add(&[FIRST, SECOND, THIRD]);
    for link in [FIRST, SECOND, THIRD] {
        test.wait_until_read(link);
    }
    let first = test.link(FIRST).unwrap().id;
    let third = test.link(THIRD).unwrap().id;
    test.links
        .set_preset(first, Some("best-audio".to_owned()))
        .unwrap();
    let before = test.links.list();

    assert_eq!(test.links.remove(&[first, third]).unwrap(), 2);
    assert_eq!(test.links.list().len(), 1);
    assert_eq!(test.links.restore_removed().unwrap(), 2);
    assert_eq!(
        test.links.list(),
        before,
        "the same IDs, places, preset and what was read"
    );
    assert_eq!(test.reader.reads().len(), 3, "nothing is read again");
    assert_eq!(test.reopen().list(), before, "saved as before");
    assert_eq!(test.links.restore_removed().unwrap(), 0, "only once");
}

#[test]
fn undo_leaves_out_links_added_again_meanwhile() {
    let test = Test::new();
    test.add(&[FIRST, SECOND, THIRD]);
    for link in [FIRST, SECOND, THIRD] {
        test.wait_until_read(link);
    }
    let ids: Vec<i64> = test.links.list().iter().map(|link| link.id).collect();
    assert_eq!(test.links.remove(&ids).unwrap(), 3);
    // The first by the same address, the second's video by another.
    let short = "https://youtu.be/second";
    test.add(&[FIRST, short]);
    test.wait_until_read(FIRST);
    test.wait_until_read(short);

    assert_eq!(test.links.restore_removed().unwrap(), 1);
    let links: Vec<String> = test
        .links
        .list()
        .into_iter()
        .map(|link| link.link)
        .collect();
    assert_eq!(links, [THIRD, FIRST, short]);
}

#[test]
fn undo_reads_again_a_link_removed_while_it_was_read() {
    let test = Test::new();
    test.reader.hold(FIRST);
    test.add(&[FIRST]);
    wait_until("it's being read", || {
        test.state(FIRST) == Some(PendingState::Reading)
    });
    let id = test.link(FIRST).unwrap().id;

    test.links.remove(&[id]).unwrap();
    assert_eq!(test.links.restore_removed().unwrap(), 1);
    assert_eq!(test.link(FIRST).unwrap().id, id);
    test.reader.release(FIRST);
    wait_until("it's read again", || {
        test.state(FIRST) == Some(PendingState::Ready)
    });
    assert_eq!(test.reader.reads(), [FIRST, FIRST]);
}

#[test]
fn a_copied_link_is_read_only_by_sites_the_tools_know() {
    let test = Test::new();
    test.copy(&[FIRST]);
    test.add(&[SECOND]);
    test.wait_until_read(FIRST);
    test.wait_until_read(SECOND);
    assert!(!test.reader.read_any_page(FIRST));
    assert!(
        test.reader.read_any_page(SECOND),
        "a link added by hand may be any page"
    );
    assert!(test.link(FIRST).unwrap().caught);
    assert!(!test.link(SECOND).unwrap().caught);
}

#[test]
fn a_copied_link_with_nothing_to_download_leaves_quietly() {
    let test = Test::new();
    let unsupported = "https://github.com/Fuzen777/Tsundoku";
    let article = "https://www.bbc.com/news/articles/an-article";
    let channel = "https://www.youtube.com/@someone";
    test.reader
        .answer(unsupported, Err(PreviewError::UnsupportedSite));
    test.reader.answer(
        article,
        Err(PreviewError::NoVideo {
            detail: "No video formats found".to_owned(),
        }),
    );
    test.reader.answer(channel, Err(PreviewError::Playlist));
    let account = "https://x.com/NASA";
    test.reader.answer(
        account,
        Err(PreviewError::NotListable {
            site: "X".to_owned(),
        }),
    );
    test.copy(&[unsupported, article, channel, account, FIRST]);
    for link in [unsupported, article, channel, account] {
        test.wait_until_gone(link);
    }
    test.wait_until_read(FIRST);
    assert_eq!(test.links.list().len(), 1);
    assert_eq!(test.reopen().list().len(), 1, "they're gone for good");

    test.reader
        .answer(SECOND, Err(PreviewError::UnsupportedSite));
    test.add(&[SECOND]);
    test.wait_until_read(SECOND);
    assert_eq!(
        test.state(SECOND),
        Some(PendingState::Failed),
        "a link added by hand stays to say why"
    );
}

#[test]
fn a_copied_link_that_needs_a_fix_stays_to_say_so() {
    let test = Test::new();
    let post = "https://www.instagram.com/p/Private/";
    let error = PreviewError::SignInRequired {
        detail: "login required".to_owned(),
        site: Some("Instagram".to_owned()),
        cookies: SignInCookies::NotSignedIn,
    };
    test.reader.answer(post, Err(error.clone()));
    test.copy(&[post]);
    test.wait_until_read(post);
    let pending = test.link(post).unwrap();
    assert_eq!(pending.state, PendingState::Failed);
    assert_eq!(pending.error, Some(error));
}

#[test]
fn a_copied_file_host_link_tsundoku_cant_read_stays_to_say_so() {
    let test = Test::new();
    let link = "https://www.mediafire.com/some/new/form/of/link";
    let error = PreviewError::LinkNotRecognized {
        site: "MediaFire".to_owned(),
    };
    test.reader.answer(link, Err(error.clone()));
    test.copy(&[link]);
    test.wait_until_read(link);
    let pending = test.link(link).unwrap();
    assert_eq!(pending.state, PendingState::Failed);
    assert_eq!(pending.error, Some(error));
}

#[test]
fn a_copied_link_downloaded_or_queued_already_leaves_quietly() {
    let test = Test::new();
    // The history and the queue have these by the same address, so they're never read.
    test.record_download(&video_at(FIRST));
    test.queue
        .add(video_at(THIRD), "best-video", false)
        .unwrap();
    assert_eq!(test.copy(&[FIRST, THIRD]).added, 0);

    // This one is read to find it's the second video, downloaded before by another address.
    test.record_download(&video_at(SECOND));
    let short = "https://youtu.be/second";
    test.copy(&[short]);
    test.wait_until_gone(short);
    assert!(test.links.list().is_empty());
    assert_eq!(test.reader.reads(), [short]);
}

#[test]
fn download_right_away_starts_copied_links_once_read() {
    let test = Test::new();
    test.download_right_away(true);
    let post = "https://www.instagram.com/p/Post1/";
    test.copy(&[FIRST, post]);
    wait_until("both are in the queue", || {
        test.queued("first").is_some() && test.queued("Post1").is_some()
    });
    assert!(test.links.list().is_empty());
    let default_id = test.presets.overview().default_id;
    assert_eq!(test.queued("first").unwrap().preset.unwrap().id, default_id);

    test.add(&[SECOND]);
    test.wait_until_read(SECOND);
    assert_eq!(
        test.state(SECOND),
        Some(PendingState::Ready),
        "a link added by hand waits to be started"
    );

    test.download_right_away(false);
    test.copy(&[THIRD]);
    test.wait_until_read(THIRD);
    assert!(
        test.queued("third").is_none(),
        "without it, a copied link waits too"
    );
}

#[test]
fn a_copied_link_is_still_copied_after_a_restart() {
    let test = Test::new();
    test.reader.hold(FIRST);
    test.copy(&[FIRST]);
    wait_until("it's being read", || {
        test.state(FIRST) == Some(PendingState::Reading)
    });
    test.reader.release(FIRST);
    let reopened = test.reopen();
    assert!(reopened.list()[0].caught);
}

#[test]
fn links_sent_from_the_shortcut_or_a_browser_are_added_as_if_pasted() {
    let test = Test::new();
    test.download_right_away(true);
    let unsupported = "https://github.com/Fuzen777/Tsundoku";
    test.reader
        .answer(unsupported, Err(PreviewError::UnsupportedSite));
    test.record_download(&video_at(SECOND));
    let links: Vec<String> = [FIRST, SECOND, unsupported]
        .iter()
        .map(|link| (*link).to_owned())
        .collect();
    assert_eq!(test.links.add_sent(&links).unwrap().added, 3);

    wait_until("the first is in the queue", || {
        test.queued("first").is_some()
    });
    test.wait_until_read(SECOND);
    test.wait_until_read(unsupported);
    assert_eq!(
        test.state(SECOND),
        Some(PendingState::Ready),
        "downloaded before, it waits"
    );
    assert_eq!(
        test.state(unsupported),
        Some(PendingState::Failed),
        "it shows why, as a pasted link does"
    );
    assert!(test.reader.read_any_page(unsupported));

    test.download_right_away(false);
    test.links.add_sent(&[THIRD.to_owned()]).unwrap();
    test.wait_until_read(THIRD);
    assert!(test.queued("third").is_none());
}

#[test]
fn start_all_takes_what_the_pending_view_offers() {
    let test = Test::new();
    test.record_download(&video_at(SECOND));
    test.reader.answer(
        THIRD,
        Err(PreviewError::Network {
            detail: "offline".to_owned(),
        }),
    );
    test.add(&[FIRST, SECOND, THIRD]);
    for link in [FIRST, SECOND, THIRD] {
        test.wait_until_read(link);
    }
    let copied = "https://www.youtube.com/watch?v=copied";
    test.reader.hold(copied);
    test.copy(&[copied]);
    wait_until("the copy is being checked", || {
        test.state(copied) == Some(PendingState::Reading)
    });

    assert_eq!(test.links.start_all("best-video").unwrap(), 1);
    assert!(test.queued("first").is_some());
    assert!(test.queued("second").is_none());
    assert!(test.link(copied).is_some_and(|link| !link.start_when_read));
    test.reader.release(copied);
}

#[test]
fn copied_links_are_announced_once_read() {
    let test = Test::new();
    test.copy(&[FIRST]);
    test.wait_until_read(FIRST);
    test.download_right_away(true);
    test.copy(&[SECOND]);
    wait_until("the second is in the queue", || {
        test.queued("second").is_some()
    });
    test.add(&[THIRD]);
    test.wait_until_read(THIRD);
    assert_eq!(
        *lock(&test.listener.copied),
        [("first".to_owned(), false), ("second".to_owned(), true)],
        "a pasted link isn't announced"
    );
}

#[test]
fn saved_options_are_announced_and_kept() {
    let test = Test::new();
    assert_eq!(
        test.links.options(),
        AddingSettings {
            watch_clipboard: false,
            download_right_away: false,
            clipboard_shortcut: None,
            notify: true,
            notify_failures: true,
            notify_finished: true,
        },
        "everything starts off but notifications"
    );
    let options = AddingSettings {
        watch_clipboard: true,
        clipboard_shortcut: Some(ClipboardShortcut::CtrlAltD),
        ..AddingSettings::default()
    };
    test.links.set_options(options).unwrap();
    assert_eq!(*lock(&test.listener.options), [options]);
    assert_eq!(test.settings.get().adding, options);
}

/// Links read from files dropped on the window, as the window hands them over.
fn from_files(test: &Test, links: &[&str]) -> AddedLinks {
    let links: Vec<String> = links.iter().map(|link| (*link).to_owned()).collect();
    test.links.add_from_files(&links).unwrap()
}

#[test]
fn links_from_a_file_are_checked_as_copied_links_are() {
    let test = Test::new();
    let unsupported = "https://github.com/Fuzen777/Tsundoku";
    test.reader
        .answer(unsupported, Err(PreviewError::UnsupportedSite));
    test.reader.hold(FIRST);
    test.record_download(&video_at(SECOND));
    from_files(&test, &[unsupported, FIRST, SECOND]);
    let first = test.link(FIRST).unwrap();
    assert!(first.from_file && !first.caught);
    assert!(
        !starts_with_all(&first),
        "counted as being checked until it's read"
    );
    test.reader.release(FIRST);

    test.wait_until_gone(unsupported);
    test.wait_until_read(FIRST);
    test.wait_until_read(SECOND);
    assert!(
        !test.reader.read_any_page(FIRST),
        "read only by sites the tools know"
    );
    let second = test.link(SECOND).unwrap();
    assert!(
        second.preview.unwrap().last_download.is_some(),
        "one downloaded before stays, marked, as one added by hand does"
    );
    assert!(
        lock(&test.listener.copied).is_empty(),
        "nothing is announced"
    );
    assert!(
        test.reopen().list().iter().all(|link| link.from_file),
        "still from a file after a restart"
    );
}

#[test]
fn a_link_from_a_file_that_needs_a_fix_stays_to_say_so() {
    let test = Test::new();
    let post = "https://www.instagram.com/p/Private/";
    let error = PreviewError::SignInRequired {
        detail: "login required".to_owned(),
        site: Some("Instagram".to_owned()),
        cookies: SignInCookies::NotSignedIn,
    };
    test.reader.answer(post, Err(error.clone()));
    from_files(&test, &[post]);
    test.wait_until_read(post);
    assert_eq!(test.link(post).unwrap().error, Some(error));
}

#[test]
fn many_links_are_saved_together_each_once() {
    let test = Test::new();
    let links: Vec<String> = (0..1500)
        .map(|number| format!("https://www.youtube.com/watch?v=v{}", number % 1000))
        .collect();
    let started = Instant::now();
    let added = test.links.add_from_files(&links).unwrap();
    assert_eq!(
        added,
        AddedLinks {
            added: 1000,
            already_pending: 500
        }
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "saved in one go, not one by one: {:?}",
        started.elapsed()
    );
    assert_eq!(test.reopen().list().len(), 1000);
}

#[test]
fn a_burst_of_changes_sends_a_few_notices_and_then_the_last() {
    let test = Test::new();
    let before = test.listener.changes.load(Ordering::SeqCst);
    for number in 0..100 {
        test.add(&[&format!("https://www.youtube.com/watch?v=burst{number}")]);
    }
    wait_until("every link is read", || {
        test.links
            .list()
            .iter()
            .all(|link| link.state == PendingState::Ready)
    });
    // Adding and reading 100 links changed the list 300 times.
    let sent = test.listener.changes.load(Ordering::SeqCst) - before;
    assert!(sent < 60, "{sent} notices");
    wait_until("the last notice lists every link", || {
        test.listener.last_count.load(Ordering::SeqCst) == 100 && test.links.list().len() == 100
    });
}
