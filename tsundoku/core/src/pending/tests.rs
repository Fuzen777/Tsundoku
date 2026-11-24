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
use crate::passwords::Password;
use crate::rules::DownloadKind;
use crate::settings::{ClipboardShortcut, PauseRange, SitePace, SitePacing};
use crate::tools::ToolManager;

/// Answers what each link is: a YouTube video, an Instagram post or a MEGA folder named after the link's end, unless
/// a test gave another answer. A held link isn't answered until it's let go, or its read is cancelled.
#[derive(Default)]
struct FakeReader {
    /// Each link read, when, and whether yt-dlp could look at any page.
    reads: Mutex<Vec<(String, Instant, bool)>>,
    answers: Mutex<HashMap<String, Result<Fetched, PreviewError>>>,
    held: Mutex<HashSet<String>>,
    /// Links that only open with a password, and the password.
    locked: Mutex<HashMap<String, String>>,
    /// The password each read was given, in order.
    passwords: Mutex<Vec<(String, Option<String>)>>,
}

impl FakeReader {
    fn answer(&self, link: &str, answer: Result<Fetched, PreviewError>) {
        lock(&self.answers).insert(link.to_owned(), answer);
    }

    fn hold(&self, link: &str) {
        lock(&self.held).insert(link.to_owned());
    }

    /// The link needs `password`, as a file host's or a video's that takes one.
    fn lock_with(&self, link: &str, password: &str) {
        lock(&self.locked).insert(link.to_owned(), password.to_owned());
    }

    fn passwords_given(&self, link: &str) -> Vec<Option<String>> {
        lock(&self.passwords)
            .iter()
            .filter(|(read, _)| read == link)
            .map(|(_, password)| password.clone())
            .collect()
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
        password: Option<&Password>,
        cancel: &CancelToken,
    ) -> Result<Fetched, PreviewError> {
        lock(&self.reads).push((link.to_owned(), Instant::now(), any_page));
        lock(&self.passwords).push((
            link.to_owned(),
            password.map(|password| password.reveal().to_owned()),
        ));
        while lock(&self.held).contains(link) {
            if cancel.is_cancelled() {
                return Err(PreviewError::Cancelled);
            }
            thread::sleep(Duration::from_millis(5));
        }
        if let Some(needed) = lock(&self.locked).get(link) {
            match password {
                Some(given) if given.reveal() == needed => {}
                given => {
                    return Err(PreviewError::password_needed(given.is_some()));
                }
            }
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
    /// The copied links announced as needing picking, and what their address says they are.
    to_pick: Mutex<Vec<(String, Option<ListingKind>)>>,
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

    fn copied_link_to_pick(&self, link: &str, listing: Option<ListingKind>) {
        lock(&self.to_pick).push((link.to_owned(), listing));
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
                let pace = SitePace { at_once: 1, pause };
                settings.downloads.pacing = SitePacing {
                    instagram: pace,
                    x: pace,
                    pixiv: pace,
                    tumblr: pace,
                    fanbox: pace,
                };
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
            source_choices: Arc::new(crate::watched::WatchedChoices::new(Arc::clone(&database))),
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
                    share_paths: Vec::new(),
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
                Some(PendingState::Ready | PendingState::Failed | PendingState::NeedsPicking)
            )
        });
    }

    /// Waits until the link was read `reads` times and needs a password, `wrong` when the one tried didn't open it.
    fn wait_until_locked(&self, link: &str, reads: usize, wrong: bool) {
        wait_until(&format!("{link} needs a password"), || {
            self.reader
                .reads()
                .iter()
                .filter(|read| *read == link)
                .count()
                == reads
                && self.link(link).is_some_and(|pending| {
                    pending.state == PendingState::NeedsPassword
                        && pending.error == Some(PreviewError::password_needed(wrong))
                })
        });
    }

    fn unlock(&self, link: &str, password: &str) -> Result<(), DownloadError> {
        let id = self.link(link).unwrap().id;
        self.links.unlock(id, Password::new(password).unwrap())
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
            others: 0,
        }),
        files: shared.then_some(FileContents {
            count: 3,
            bytes: 1000,
        }),
        post_files: Vec::new(),
        shared_files: Vec::new(),
        playlist: None,
        tags: Vec::new(),
        pixiv: None,
        fanbox: None,
        links: Vec::new(),
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

/// A MEGA folder of three files, all ticked, at `link`.
fn shared_folder_at(link: &str) -> VideoInfo {
    VideoInfo {
        shared_files: ["Pack/a.zip", "Pack/Sub/b.zip", "Pack/c.txt"]
            .iter()
            .map(|path| crate::metadata::SharedFile {
                path: (*path).to_owned(),
                size: 100,
                picked: true,
            })
            .collect(),
        ..video_at(link)
    }
}

fn picked_paths(link: &PendingLink) -> Vec<String> {
    link.preview
        .as_ref()
        .unwrap()
        .video
        .shared_files
        .iter()
        .filter(|file| file.picked)
        .map(|file| file.path.clone())
        .collect()
}

#[test]
fn keeps_the_files_of_a_shared_folder_picked() {
    let test = Test::new();
    let folder = "https://mega.nz/folder/Pack";
    test.reader
        .answer(folder, Ok(fetched(shared_folder_at(folder))));
    test.add(&[folder, FIRST]);
    test.wait_until_read(folder);
    test.wait_until_read(FIRST);
    let id = test.link(folder).unwrap().id;
    assert_eq!(
        picked_paths(&test.link(folder).unwrap()).len(),
        3,
        "every file to start with"
    );

    test.links.set_shared_files(id, &[0, 2, 9]).unwrap();
    assert_eq!(
        picked_paths(&test.link(folder).unwrap()),
        ["Pack/a.zip", "Pack/c.txt"],
        "a place the folder doesn't have is ignored"
    );
    let reopened = test.reopen();
    let kept = reopened
        .list()
        .into_iter()
        .find(|link| link.id == id)
        .unwrap();
    assert_eq!(
        picked_paths(&kept),
        ["Pack/a.zip", "Pack/c.txt"],
        "kept through a restart"
    );
    assert_eq!(
        test.links
            .set_shared_files(test.link(FIRST).unwrap().id, &[0]),
        Err(DownloadError::WrongState),
        "a video lists no files"
    );

    test.links.start(&[id], "best-video").unwrap();
    let queued = test.queued("Pack").expect("started");
    assert_eq!(
        queued.video.picked_shared_files(),
        Some(vec!["Pack/a.zip".to_owned(), "Pack/c.txt".to_owned()])
    );
}

#[test]
fn a_shared_folder_with_nothing_picked_doesnt_start() {
    let test = Test::new();
    let folder = "https://mega.nz/folder/Empty";
    test.reader
        .answer(folder, Ok(fetched(shared_folder_at(folder))));
    test.add(&[folder]);
    test.wait_until_read(folder);
    let id = test.link(folder).unwrap().id;
    test.links.set_shared_files(id, &[]).unwrap();
    assert_eq!(
        test.links.start(&[id], "best-video"),
        Err(DownloadError::NothingPicked)
    );
    assert!(test.queued("Empty").is_none());
}

#[test]
fn a_shared_folder_downloaded_before_ticks_the_files_it_didnt_take() {
    let test = Test::new();
    let folder = "https://mega.nz/folder/Again";
    let video = shared_folder_at(folder);
    test.history
        .record(
            &video,
            &NewDownload {
                preset: String::new(),
                file_path: r"D:\Pack.zip".to_owned(),
                files: vec![crate::history::SavedFile {
                    path: r"D:\Pack.zip".to_owned(),
                    size: Some(100),
                }],
                file_size: Some(100),
                format: None,
                downloaded_at: 1,
                kind: DownloadKind::File,
                share_paths: vec!["Pack/a.zip".to_owned()],
            },
        )
        .unwrap();
    test.reader.answer(folder, Ok(fetched(video)));
    test.add(&[folder]);
    test.wait_until_read(folder);
    assert_eq!(
        picked_paths(&test.link(folder).unwrap()),
        ["Pack/Sub/b.zip", "Pack/c.txt"]
    );
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
fn takes_only_links_written_out_from_a_copy_of_several_words() {
    // A copy of one word is a link with or without "https://".
    assert_eq!(links_in_copy(" youtu.be/b\r\n"), ["https://youtu.be/b"]);
    assert_eq!(
        links_in_copy("<https://youtu.be/b>"),
        ["https://youtu.be/b"]
    );
    // Among several words, only those written with it: nobody read the text before it was added.
    let text = "Saved as notes.txt. See (https://youtu.be/b), youtu.be/c and e.g.\n\
        HTTPS://x.com/someone/status/1 twice https://youtu.be/b";
    assert_eq!(
        links_in_copy(text),
        ["https://youtu.be/b", "HTTPS://x.com/someone/status/1"]
    );
    assert_eq!(
        links_in_copy("https://youtu.be/b\nhttps://youtu.be/c"),
        ["https://youtu.be/b", "https://youtu.be/c"]
    );
    let none = Vec::<String>::new();
    assert_eq!(links_in_copy("A sentence. Nothing else."), none);
    assert_eq!(links_in_copy("hello"), none);
    assert_eq!(links_in_copy("C:\\Videos\\clip.mp4"), none);
    assert_eq!(links_in_copy("  "), none);
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
                        join: rules::ConditionJoin::All,
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
        name: String::new(),
        enabled: true,
        manual: false,
        conditions: Vec::new(),
        folder: String::new(),
        file_name: String::new(),
        missing_values: rules::MissingValues::StandIn,
        preset_id: None,
        download_right_away: None,
        unpack: None,
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
fn a_rule_picked_for_a_link_decides_what_it_sets_through_a_restart_and_into_the_queue() {
    let test = Test::new();
    add_rule(
        &test,
        "Title of",
        rules::FolderRule {
            preset_id: Some("audio-mp3".to_owned()),
            ..no_actions()
        },
    );
    add_rule(
        &test,
        "matches nothing",
        rules::FolderRule {
            id: "rule-8".to_owned(),
            folder: r"D:\Picked".to_owned(),
            preset_id: Some("best-audio".to_owned()),
            ..no_actions()
        },
    );
    test.add(&[FIRST, SECOND]);
    test.wait_until_read(FIRST);
    test.wait_until_read(SECOND);
    let first = test.link(FIRST).unwrap().id;
    let second = test.link(SECOND).unwrap().id;
    test.links
        .set_rule(first, Some("rule-8".to_owned()))
        .unwrap();
    test.links
        .set_rule(second, Some("rule-8".to_owned()))
        .unwrap();
    test.links.set_rule(second, Some("  ".to_owned())).unwrap();
    let reopened = test.reopen();
    let rule_of = |id: i64| {
        reopened
            .list()
            .into_iter()
            .find(|link| link.id == id)
            .and_then(|link| link.rule_id)
    };
    assert_eq!(
        rule_of(first).as_deref(),
        Some("rule-8"),
        "kept through a restart"
    );
    assert_eq!(rule_of(second), None, "a blank one is none");

    test.links.start(&[first, second], "1080p-mp4").unwrap();
    let picked = test.queued("first").unwrap();
    assert_eq!(
        (
            picked.folder.as_str(),
            picked.folder_chosen,
            picked.preset.unwrap().id.as_str(),
            picked.rule_id.as_deref()
        ),
        (r"D:\Picked", false, "best-audio", Some("rule-8")),
        "the picked rule's folder and preset, over the matching rule's"
    );
    let usual = test.queued("second").unwrap();
    assert_eq!(usual.preset.unwrap().id, "audio-mp3");
    assert_ne!(usual.folder, r"D:\Picked");
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
    test.reader
        .answer(unsupported, Err(PreviewError::UnsupportedSite));
    test.reader.answer(
        article,
        Err(PreviewError::NoVideo {
            detail: "No video formats found".to_owned(),
        }),
    );
    let short_link = "https://bit.ly/an-article";
    test.reader.answer(
        short_link,
        Err(PreviewError::ShortLinkToUnsupportedSite {
            site: "Bitly".to_owned(),
            target: "example.com".to_owned(),
        }),
    );
    let account = "https://x.com/NASA";
    test.reader.answer(
        account,
        Err(PreviewError::NotListable {
            site: "X".to_owned(),
        }),
    );
    test.copy(&[unsupported, article, short_link, account, FIRST]);
    for link in [unsupported, article, short_link, account] {
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
            click_n_load: false,
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
fn links_a_page_sent_are_read_and_never_start_by_themselves() {
    let test = Test::new();
    test.download_right_away(true);
    let unsupported = "https://github.com/Fuzen777/Tsundoku";
    test.reader
        .answer(unsupported, Err(PreviewError::UnsupportedSite));
    test.reader.hold(FIRST);
    test.record_download(&video_at(SECOND));
    let links: Vec<String> = [unsupported, FIRST, SECOND]
        .iter()
        .map(|link| (*link).to_owned())
        .collect();
    assert_eq!(test.links.add_from_page(&links).unwrap().added, 3);
    let first = test.link(FIRST).unwrap();
    assert!(first.from_page && !first.from_file && !first.caught);
    assert!(!first.start_when_read);
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
    assert_eq!(test.state(FIRST), Some(PendingState::Ready));
    assert!(
        test.queued("first").is_none(),
        "it waits to be started, though Download Right Away is on"
    );
    assert!(
        test.link(SECOND)
            .unwrap()
            .preview
            .unwrap()
            .last_download
            .is_some(),
        "one downloaded before stays, marked"
    );
    assert!(
        lock(&test.listener.copied).is_empty(),
        "nothing is announced"
    );
    assert!(
        test.reopen().list().iter().all(|link| link.from_page),
        "still sent by a page after a restart"
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

#[test]
fn a_link_that_needs_a_password_waits_for_one_and_opens_with_it() {
    let test = Test::new();
    test.reader.lock_with(FIRST, "pw");
    test.reader.lock_with(SECOND, "pw");
    test.add(&[FIRST]);
    test.download_right_away(true);
    test.copy(&[SECOND]);
    test.wait_until_locked(FIRST, 1, false);
    test.wait_until_locked(SECOND, 1, false);
    assert!(
        test.link(SECOND).unwrap().caught,
        "a copied link that needs one stays, though Download Right Away is on"
    );
    assert!(!starts_with_all(&test.link(FIRST).unwrap()));
    assert_eq!(test.links.start_all("best-video").unwrap(), 0);
    assert!(test.queue.items().is_empty(), "Start All leaves it");

    test.unlock(FIRST, "nope").unwrap();
    test.wait_until_locked(FIRST, 2, true);
    assert_eq!(
        test.reader.passwords_given(FIRST),
        [None, Some("nope".to_owned())]
    );
    // It needs a password through a restart, without the wrong one.
    let reopened = test.reopen();
    let after_restart = reopened
        .list()
        .into_iter()
        .find(|pending| pending.link == FIRST)
        .unwrap();
    assert_eq!(after_restart.state, PendingState::NeedsPassword);
    drop(reopened);

    test.unlock(FIRST, "pw").unwrap();
    test.wait_until_read(FIRST);
    assert_eq!(test.state(FIRST), Some(PendingState::Ready));
    assert_eq!(
        test.unlock(FIRST, "pw"),
        Err(DownloadError::WrongState),
        "it's open"
    );
    let id = test.link(FIRST).unwrap().id;
    test.links.start(&[id], "best-video").unwrap();
    wait_until("it's in the queue", || test.queued("first").is_some());
    assert_eq!(
        test.queued("first")
            .unwrap()
            .password
            .as_ref()
            .map(Password::reveal),
        Some("pw"),
        "the password goes with it"
    );

    // The copied one, once opened, downloads right away as Download Right Away says.
    test.unlock(SECOND, "pw").unwrap();
    wait_until("it's in the queue", || test.queued("second").is_some());
    assert_eq!(
        test.queued("second")
            .unwrap()
            .password
            .as_ref()
            .map(Password::reveal),
        Some("pw")
    );
}

#[test]
fn a_started_link_that_needs_a_password_starts_once_one_opens_it() {
    let test = Test::new();
    test.reader.lock_with(FIRST, "pw");
    test.reader.hold(FIRST);
    test.add(&[FIRST]);
    wait_until("it's being read", || {
        test.state(FIRST) == Some(PendingState::Reading)
    });
    let id = test.link(FIRST).unwrap().id;
    test.links.start(&[id], "audio-mp3").unwrap();
    test.reader.release(FIRST);
    test.wait_until_locked(FIRST, 1, false);
    assert!(test.link(FIRST).unwrap().start_when_read, "still started");
    assert!(test.queued("first").is_none());

    test.unlock(FIRST, "pw").unwrap();
    wait_until("it's in the queue", || test.queued("first").is_some());
    let queued = test.queued("first").unwrap();
    assert_eq!(queued.preset.unwrap().id, "audio-mp3");
    assert_eq!(queued.password.as_ref().map(Password::reveal), Some("pw"));
    wait_until("it leaves", || test.link(FIRST).is_none());
}

#[test]
fn a_password_typed_for_a_link_is_read_before_the_links_waiting() {
    let test = Test::new();
    test.reader.lock_with(FIRST, "pw");
    test.add(&[FIRST]);
    test.wait_until_locked(FIRST, 1, false);
    test.reader.hold(SECOND);
    test.add(&[SECOND, THIRD]);
    wait_until("the second is being read", || {
        test.state(SECOND) == Some(PendingState::Reading)
    });
    test.unlock(FIRST, "pw").unwrap();
    test.reader.release(SECOND);
    test.wait_until_read(THIRD);
    assert_eq!(test.reader.reads(), [FIRST, SECOND, FIRST, THIRD]);
}

#[test]
fn a_link_downloaded_with_a_password_opens_with_it_again() {
    let test = Test::new();
    test.reader.lock_with(FIRST, "pw");
    test.history
        .record_opened_with(
            &video_at(FIRST),
            &NewDownload {
                preset: "Best Video".to_owned(),
                file_path: "first.mkv".to_owned(),
                files: Vec::new(),
                file_size: None,
                format: None,
                downloaded_at: 1,
                kind: DownloadKind::Video,
                share_paths: Vec::new(),
            },
            Password::new("pw").as_ref(),
        )
        .unwrap();
    test.add(&[FIRST]);
    test.wait_until_read(FIRST);
    assert_eq!(
        test.state(FIRST),
        Some(PendingState::Ready),
        "without asking"
    );
    assert_eq!(test.reader.passwords_given(FIRST), [Some("pw".to_owned())]);
}

#[test]
fn a_link_that_needs_a_password_is_tried_with_the_known_ones_before_asking() {
    let test = Test::new();
    test.settings
        .update(|settings| {
            settings.downloads.unpack.passwords = vec!["nope".to_owned(), "pw".to_owned()];
        })
        .unwrap();
    test.reader.lock_with(FIRST, "pw");
    test.reader.lock_with(SECOND, "another");
    test.add(&[FIRST, SECOND]);
    test.wait_until_read(FIRST);
    assert_eq!(
        test.state(FIRST),
        Some(PendingState::Ready),
        "without asking"
    );
    assert_eq!(
        test.reader.passwords_given(FIRST),
        [None, Some("nope".to_owned()), Some("pw".to_owned())],
        "the list's, in order, until one opens it"
    );
    // None opens the second: it asks, saying how many were tried.
    wait_until("the second asks", || {
        test.link(SECOND).is_some_and(|pending| {
            pending.state == PendingState::NeedsPassword
                && pending.error
                    == Some(PreviewError::PasswordNeeded {
                        wrong: false,
                        tried: 2,
                    })
        })
    });
    let id = test.link(FIRST).unwrap().id;
    test.links.start(&[id], "best-video").unwrap();
    wait_until("it's in the queue", || test.queued("first").is_some());
    assert_eq!(
        test.queued("first")
            .unwrap()
            .password
            .as_ref()
            .map(Password::reveal),
        Some("pw"),
        "the one that opened it goes with it"
    );

    // A typed one that's wrong says so, without the list being tried again.
    test.unlock(SECOND, "typo").unwrap();
    test.wait_until_locked(SECOND, 4, true);
    assert_eq!(
        test.queue.passwords().past_count().unwrap(),
        0,
        "nothing is kept unless Save Past Passwords is on"
    );
}

#[test]
fn a_typed_password_that_opens_a_link_is_tried_on_its_sites_next_ones() {
    let test = Test::new();
    test.settings
        .update(|settings| settings.downloads.save_past_passwords = true)
        .unwrap();
    test.reader.lock_with(FIRST, "pw");
    test.reader.lock_with(SECOND, "pw");
    test.add(&[FIRST]);
    test.wait_until_locked(FIRST, 1, false);
    test.unlock(FIRST, "pw").unwrap();
    test.wait_until_read(FIRST);
    assert_eq!(test.queue.passwords().past_count().unwrap(), 1);

    test.add(&[SECOND]);
    test.wait_until_read(SECOND);
    assert_eq!(
        test.state(SECOND),
        Some(PendingState::Ready),
        "without asking"
    );
    assert_eq!(
        test.reader.passwords_given(SECOND),
        [None, Some("pw".to_owned())]
    );
}

const CHANNEL: &str = "https://www.youtube.com/@someone";
const PLAYLIST: &str = "https://www.youtube.com/playlist?list=PLsomething";
const ACCOUNT: &str = "https://x.com/NASA";
/// Another site's playlist, which only reading tells.
const ALBUM: &str = "https://soundcloud.com/someone/sets/an-album";

/// Each listing link answers as the reader refuses one.
fn answer_listings(test: &Test) {
    for link in [CHANNEL, PLAYLIST, ACCOUNT, ALBUM] {
        test.reader.answer(link, Err(PreviewError::Playlist));
    }
}

#[test]
fn a_channel_playlist_or_account_waits_to_be_picked_from_whatever_its_origin() {
    let test = Test::new();
    answer_listings(&test);
    test.download_right_away(true);
    test.copy(&[CHANNEL, ALBUM]);
    test.add(&[PLAYLIST]);
    from_files(&test, &[ACCOUNT]);
    for link in [CHANNEL, PLAYLIST, ACCOUNT, ALBUM] {
        test.wait_until_read(link);
        let pending = test.link(link).unwrap();
        assert_eq!(pending.state, PendingState::NeedsPicking, "{link}");
        assert_eq!(pending.error, None);
        assert!(!pending.start_when_read);
        assert!(!starts_with_all(&pending), "Start All leaves {link}");
    }
    assert_eq!(
        test.link(CHANNEL).unwrap().listing,
        Some(ListingKind::Channel)
    );
    assert_eq!(
        test.link(PLAYLIST).unwrap().listing,
        Some(ListingKind::Playlist)
    );
    assert_eq!(
        test.link(ACCOUNT).unwrap().listing,
        Some(ListingKind::Account)
    );
    assert_eq!(
        test.link(ALBUM).unwrap().listing,
        None,
        "only reading said so"
    );
    assert!(
        test.queue.items().is_empty(),
        "Download Right Away starts none"
    );
    assert_eq!(
        *lock(&test.listener.to_pick),
        [
            (CHANNEL.to_owned(), Some(ListingKind::Channel)),
            (ALBUM.to_owned(), None)
        ],
        "only copied links are announced"
    );
    assert!(lock(&test.listener.copied).is_empty());

    let ids: Vec<i64> = test.links.list().iter().map(|link| link.id).collect();
    assert_eq!(test.links.start_all("best-video").unwrap(), 0);
    test.links.start(&ids, "best-video").unwrap();
    assert_eq!(test.links.list().len(), 4, "Start leaves them waiting too");
    assert!(test.queue.items().is_empty());

    let reopened = test.reopen();
    assert_eq!(reopened.list(), test.links.list(), "kept through a restart");
    assert_eq!(test.reader.reads().len(), 4, "without being read again");
}

#[test]
fn a_listing_started_before_it_was_read_waits_to_be_picked_from() {
    let test = Test::new();
    answer_listings(&test);
    test.reader.hold(CHANNEL);
    test.add(&[CHANNEL]);
    let id = test.link(CHANNEL).unwrap().id;
    test.links.start(&[id], "best-video").unwrap();
    assert!(test.link(CHANNEL).unwrap().start_when_read);
    test.reader.release(CHANNEL);
    test.wait_until_read(CHANNEL);
    let pending = test.link(CHANNEL).unwrap();
    assert_eq!(pending.state, PendingState::NeedsPicking);
    assert!(!pending.start_when_read);
    assert!(test.queue.items().is_empty());
}

#[test]
fn a_second_link_to_a_listing_here_already_leaves() {
    let test = Test::new();
    answer_listings(&test);
    let shorts = "https://www.youtube.com/@someone/shorts";
    let media = "https://twitter.com/NASA/media";
    test.reader.answer(shorts, Err(PreviewError::Playlist));
    test.reader.answer(media, Err(PreviewError::Playlist));
    test.copy(&[CHANNEL, ACCOUNT]);
    test.wait_until_read(CHANNEL);
    test.wait_until_read(ACCOUNT);
    test.copy(&[shorts]);
    test.add(&[media]);
    test.wait_until_gone(shorts);
    test.wait_until_gone(media);
    let links: Vec<String> = test
        .links
        .list()
        .into_iter()
        .map(|link| link.link)
        .collect();
    assert_eq!(links, [CHANNEL, ACCOUNT]);
}

#[test]
fn picking_from_a_listing_takes_it_out_of_pending() {
    let test = Test::new();
    answer_listings(&test);
    test.add(&[CHANNEL, FIRST, SECOND]);
    for link in [CHANNEL, FIRST, SECOND] {
        test.wait_until_read(link);
    }
    let channel = test.link(CHANNEL).unwrap().id;
    let first = test.link(FIRST).unwrap().id;
    let second = test.link(SECOND).unwrap().id;
    test.links.remove(&[second]).unwrap();

    test.links.picked(first).unwrap();
    assert!(
        test.link(FIRST).is_some(),
        "a link that doesn't need picking stays"
    );
    test.links.picked(channel).unwrap();
    assert!(test.link(CHANNEL).is_none());
    assert!(
        test.reopen().list().iter().all(|link| link.id != channel),
        "for good"
    );
    test.links.picked(channel).unwrap();

    assert_eq!(
        test.links.restore_removed().unwrap(),
        1,
        "Undo still puts back what was removed, not the listing"
    );
    let links: Vec<String> = test
        .links
        .list()
        .into_iter()
        .map(|link| link.link)
        .collect();
    assert_eq!(links, [FIRST, SECOND]);
}
