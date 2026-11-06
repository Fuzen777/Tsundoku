//! The queue's state machine, run against a fake runner the tests finish, fail or let be cancelled.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use super::*;
use crate::data_folder::DataPaths;
use crate::presets::built_in_presets;

/// A runner that waits until the test gives it an outcome, or until it's cancelled.
#[derive(Default)]
struct FakeRunner {
    started: Mutex<Vec<i64>>,
    jobs: Mutex<Vec<JobSpec>>,
    outcomes: Mutex<HashMap<i64, JobOutcome>>,
}

impl FakeRunner {
    fn complete(&self, id: i64, outcome: JobOutcome) {
        lock(&self.outcomes).insert(id, outcome);
    }

    fn started(&self) -> Vec<i64> {
        lock(&self.started).clone()
    }

    fn job(&self, id: i64) -> JobSpec {
        lock(&self.jobs)
            .iter()
            .find(|job| job.id == id)
            .cloned()
            .unwrap()
    }
}

impl JobRunner for FakeRunner {
    fn run(
        &self,
        job: &JobSpec,
        cancel: &CancelToken,
        on_progress: &mut dyn FnMut(DownloadProgress),
    ) -> JobOutcome {
        lock(&self.started).push(job.id);
        lock(&self.jobs).push(job.clone());
        fs::create_dir_all(&job.temp_folder).unwrap();
        on_progress(DownloadProgress::at(DownloadStage::DownloadingVideo));
        loop {
            if cancel.is_cancelled() {
                return JobOutcome::Cancelled;
            }
            if let Some(outcome) = lock(&self.outcomes).remove(&job.id) {
                return outcome;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}

#[derive(Default)]
struct CountingListener {
    changes: AtomicUsize,
    progress: AtomicUsize,
}

impl QueueListener for CountingListener {
    fn changed(&self, _items: &[QueueItem]) {
        self.changes.fetch_add(1, Ordering::SeqCst);
    }

    fn progress(&self, _id: i64, _progress: &DownloadProgress) {
        self.progress.fetch_add(1, Ordering::SeqCst);
    }
}

struct Test {
    folder: tempfile::TempDir,
    database: Arc<Database>,
    history: Arc<History>,
    runner: Arc<FakeRunner>,
    listener: Arc<CountingListener>,
    queue: Arc<DownloadQueue>,
    settings: Arc<SettingsStore>,
}

impl Test {
    fn new() -> Self {
        let folder = tempfile::tempdir().unwrap();
        let database = Arc::new(Database::open_in_memory().unwrap());
        let history = Arc::new(
            History::new(Arc::clone(&database), folder.path().join("archive.txt")).unwrap(),
        );
        let settings = Arc::new(SettingsStore::open(folder.path().join("settings.json")).unwrap());
        // Every preset saves into the test's folder.
        let downloads = folder.path().join("downloads").display().to_string();
        settings
            .update(|settings| {
                for preset in &mut settings.presets.list {
                    preset.folder = Some(downloads.clone());
                }
                // No cookies, so the tests never look at this PC's browsers.
                settings.downloads.cookies_browser = None;
                settings.downloads.folder = Some(downloads.clone());
                // Downloads no preset folder takes go in a folder for their site in by-site.
                settings.rules.list = vec![crate::rules::FolderRule {
                    folder: folder
                        .path()
                        .join("by-site")
                        .join("{site}")
                        .display()
                        .to_string(),
                    ..crate::rules::catch_all_rule()
                }];
            })
            .unwrap();
        let runner = Arc::new(FakeRunner::default());
        let listener = Arc::new(CountingListener::default());
        let queue = Self::open_queue(&folder, &database, &history, &settings, &runner, &listener);
        Self {
            folder,
            database,
            history,
            runner,
            listener,
            queue,
            settings,
        }
    }

    fn open_queue(
        folder: &tempfile::TempDir,
        database: &Arc<Database>,
        history: &Arc<History>,
        settings: &Arc<SettingsStore>,
        runner: &Arc<FakeRunner>,
        listener: &Arc<CountingListener>,
    ) -> Arc<DownloadQueue> {
        let paths = DataPaths::new(folder.path().to_path_buf());
        DownloadQueue::open(QueueServices {
            database: Arc::clone(database),
            history: Arc::clone(history),
            tools: Arc::new(ToolManager::new(
                paths,
                Arc::clone(settings),
                "0.0.0".to_owned(),
            )),
            presets: Arc::new(PresetManager::new(Arc::clone(settings))),
            settings: Arc::clone(settings),
            logs_folder: folder.path().join("logs"),
            runner: runner.clone() as Arc<dyn JobRunner>,
            listener: listener.clone() as Arc<dyn QueueListener>,
        })
        .unwrap()
    }

    fn reopen(&self) -> Arc<DownloadQueue> {
        Self::open_queue(
            &self.folder,
            &self.database,
            &self.history,
            &self.settings,
            &self.runner,
            &self.listener,
        )
    }

    fn add(&self, video_id: &str) -> i64 {
        self.queue
            .add(video(video_id), "best-video", false)
            .unwrap()
            .id
    }

    fn state(&self, id: i64) -> Option<QueueState> {
        self.queue
            .items()
            .into_iter()
            .find(|item| item.id == id)
            .map(|item| item.state)
    }

    /// A file the fake download "made".
    fn finished_file(&self, name: &str) -> JobOutcome {
        let path = self.folder.path().join("downloads").join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"video").unwrap();
        JobOutcome::Finished {
            files: vec![path],
            format: Some("1080p · avc1 + mp4a".to_owned()),
        }
    }

    fn temp_folder(&self, id: i64) -> PathBuf {
        self.folder
            .path()
            .join("downloads")
            .join(format!(".tsundoku-{id}"))
    }
}

fn video(video_id: &str) -> VideoInfo {
    VideoInfo {
        site: "youtube".to_owned(),
        site_name: "youtube.com".to_owned(),
        video_id: video_id.to_owned(),
        url: format!("https://www.youtube.com/watch?v={video_id}"),
        title: format!("Video {video_id}"),
        uploader: None,
        duration_seconds: Some(10.0),
        upload_date: None,
        thumbnail_url: None,
        best_height: Some(1080),
        post: None,
        files: None,
    }
}

/// Waits for something the queue's threads do.
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
fn runs_two_at_a_time_in_order() {
    let test = Test::new();
    let [first, second, third] = ["a", "b", "c"].map(|id| test.add(id));
    wait_until("two run", || test.runner.started().len() == 2);
    // The two start on threads of their own at once, so which records itself first varies.
    let mut running = test.runner.started();
    running.sort_unstable();
    assert_eq!(running, [first, second]);
    assert_eq!(test.state(third), Some(QueueState::Waiting));

    test.runner.complete(first, test.finished_file("a.mkv"));
    wait_until("the third starts", || test.runner.started().len() == 3);
    assert_eq!(test.state(first), Some(QueueState::Done));
    assert_eq!(test.runner.started()[2], third);
    assert!(test.listener.progress.load(Ordering::SeqCst) > 0);
}

#[test]
fn records_a_finished_download_in_the_history() {
    let test = Test::new();
    let id = test.add("jNQXAC9IVRw");
    wait_until("it runs", || test.state(id) == Some(QueueState::Running));
    test.runner.complete(id, test.finished_file("zoo.mkv"));
    wait_until("it's done", || test.state(id) == Some(QueueState::Done));

    let item = test
        .queue
        .items()
        .into_iter()
        .find(|item| item.id == id)
        .unwrap();
    let result = item.result.unwrap();
    assert_eq!(result.file_size, Some(5));
    let last = test
        .history
        .last_download("youtube", "jNQXAC9IVRw")
        .unwrap()
        .unwrap();
    assert_eq!(last.id, result.download_id);
    assert_eq!(last.preset, "Best Video");
    assert_eq!(last.format.as_deref(), Some("1080p · avc1 + mp4a"));
    assert_eq!(
        fs::read_to_string(test.folder.path().join("archive.txt")).unwrap(),
        "youtube jNQXAC9IVRw\n"
    );
    assert!(
        !test.temp_folder(id).exists(),
        "the temporary folder is cleaned up"
    );
    assert!(
        !test.queue.contains("youtube", "jNQXAC9IVRw"),
        "a done video can be added again"
    );
}

#[test]
fn stopping_keeps_the_partial_files_and_resume_continues() {
    let test = Test::new();
    let id = test.add("a");
    wait_until("it runs", || test.state(id) == Some(QueueState::Running));

    test.queue.stop(id).unwrap();
    wait_until("it stops", || test.state(id) == Some(QueueState::Stopped));
    assert!(test.temp_folder(id).exists());

    test.queue.resume(id).unwrap();
    wait_until("it runs again", || test.runner.started() == [id, id]);
    assert_eq!(test.queue.resume(id), Err(DownloadError::WrongState));
}

#[test]
fn stopping_a_waiting_download_holds_it() {
    let test = Test::new();
    test.settings
        .update(|settings| settings.downloads.at_once = 1)
        .unwrap();
    let [first, second] = ["a", "b"].map(|id| test.add(id));
    test.queue.stop(second).unwrap();
    assert_eq!(test.state(second), Some(QueueState::Stopped));

    test.runner.complete(first, test.finished_file("a.mkv"));
    wait_until("the first is done", || {
        test.state(first) == Some(QueueState::Done)
    });
    assert_eq!(test.runner.started(), [first], "the held one doesn't start");
}

#[test]
fn cancelling_removes_the_download_and_its_files() {
    let test = Test::new();
    let id = test.add("a");
    wait_until("it runs", || test.state(id) == Some(QueueState::Running));

    test.queue.cancel(id).unwrap();
    wait_until("it's gone", || test.state(id).is_none());
    assert!(!test.temp_folder(id).exists());
    assert_eq!(test.reopen().items().len(), 0, "and from the saved queue");
}

#[test]
fn a_failed_download_can_be_retried() {
    let test = Test::new();
    let id = test.add("a");
    wait_until("it runs", || test.state(id) == Some(QueueState::Running));
    test.runner
        .complete(id, JobOutcome::Failed(DownloadError::NoFile));
    wait_until("it fails", || test.state(id) == Some(QueueState::Failed));
    let item = test
        .queue
        .items()
        .into_iter()
        .find(|item| item.id == id)
        .unwrap();
    assert_eq!(item.error, Some(DownloadError::NoFile));

    test.queue.retry(id).unwrap();
    wait_until("it runs again", || test.runner.started() == [id, id]);
    assert_eq!(test.queue.items()[0].error, None);
}

#[test]
fn pause_all_stops_running_downloads_and_holds_waiting_ones_until_resume_all() {
    let test = Test::new();
    let [first, second, third] = ["a", "b", "c"].map(|id| test.add(id));
    wait_until("two run", || test.runner.started().len() == 2);

    test.queue.stop_all().unwrap();
    wait_until("all three stop", || {
        [first, second, third]
            .iter()
            .all(|id| test.state(*id) == Some(QueueState::Stopped))
    });
    assert_eq!(
        test.runner.started().len(),
        2,
        "the waiting one never started"
    );
    assert!(test.temp_folder(first).exists(), "partial files are kept");
    assert_eq!(test.queue.store.state_of(third), Some(QueueState::Stopped));

    test.queue.resume_all().unwrap();
    wait_until("two run again", || test.runner.started().len() == 4);
    let mut resumed = test.runner.started()[2..].to_vec();
    resumed.sort_unstable();
    assert_eq!(resumed, [first, second], "they start in their order");
    assert_eq!(test.state(third), Some(QueueState::Waiting));
}

#[test]
fn pause_all_leaves_a_cancelled_download_to_be_removed() {
    let test = Test::new();
    let id = test.add("a");
    wait_until("it runs", || test.state(id) == Some(QueueState::Running));
    test.queue.cancel(id).unwrap();
    test.queue.stop_all().unwrap();
    wait_until("it's gone", || test.state(id).is_none());
    assert!(!test.temp_folder(id).exists());
}

#[test]
fn retry_failed_retries_every_failed_download_and_nothing_else() {
    let test = Test::new();
    let [first, second, held] = ["a", "b", "c"].map(|id| test.add(id));
    test.queue.stop(held).unwrap();
    wait_until("two run", || test.runner.started().len() == 2);
    for id in [first, second] {
        test.runner
            .complete(id, JobOutcome::Failed(DownloadError::NoFile));
    }
    wait_until("both fail", || {
        test.state(first) == Some(QueueState::Failed)
            && test.state(second) == Some(QueueState::Failed)
    });

    test.queue.retry_failed().unwrap();
    wait_until("both run again", || test.runner.started().len() == 4);
    assert!(
        test.queue.items().iter().all(|item| item.error.is_none()),
        "their errors are cleared"
    );
    assert_eq!(
        test.state(held),
        Some(QueueState::Stopped),
        "a stopped one stays"
    );
}

#[test]
fn download_next_moves_a_waiting_download_to_the_front_for_good() {
    let test = Test::new();
    test.settings
        .update(|settings| settings.downloads.at_once = 1)
        .unwrap();
    let [first, second, third, fourth] = ["a", "b", "c", "d"].map(|id| test.add(id));
    wait_until("the first runs", || test.runner.started() == [first]);
    assert_eq!(
        test.queue.download_next(first),
        Err(DownloadError::WrongState),
        "only a waiting one moves"
    );
    assert_eq!(test.queue.download_next(99), Err(DownloadError::NotFound));

    test.queue.download_next(third).unwrap();
    test.queue.download_next(fourth).unwrap();
    let saved: Vec<i64> = test
        .queue
        .store
        .load()
        .unwrap()
        .iter()
        .map(|item| item.id)
        .collect();
    assert_eq!(
        saved,
        [fourth, third, first, second],
        "the order is saved, the latest moved first"
    );

    test.runner.complete(first, test.finished_file("a.mkv"));
    wait_until("the next one starts", || test.runner.started().len() == 2);
    assert_eq!(test.runner.started()[1], fourth);
    test.runner.complete(fourth, test.finished_file("d.mkv"));
    wait_until("then the one after", || test.runner.started().len() == 3);
    assert_eq!(test.runner.started()[2], third);
    test.queue.stop_all().unwrap();
}

/// A queue item in `state`, for tests that only need its state (`run.rs`'s too).
pub(super) fn item_in(state: QueueState) -> QueueItem {
    QueueItem {
        id: 1,
        video: video("a"),
        preset: None,
        folder: String::new(),
        state,
        overwrite: false,
        added_at: 0,
        result: None,
        error: None,
        progress: None,
    }
}

#[test]
fn the_tray_pauses_while_anything_runs_or_waits_else_resumes() {
    let action = |states: &[QueueState]| {
        let items: Vec<QueueItem> = states.iter().map(|state| item_in(*state)).collect();
        WholeQueueAction::for_items(&items)
    };
    assert_eq!(action(&[]), None);
    assert_eq!(action(&[QueueState::Done, QueueState::Failed]), None);
    assert_eq!(
        action(&[QueueState::Stopped, QueueState::Done]),
        Some(WholeQueueAction::ResumeAll)
    );
    assert_eq!(
        action(&[QueueState::Stopped, QueueState::Waiting]),
        Some(WholeQueueAction::PauseAll)
    );
    assert_eq!(
        action(&[QueueState::Running]),
        Some(WholeQueueAction::PauseAll)
    );
}

#[test]
fn refuses_a_video_already_in_the_queue() {
    let test = Test::new();
    test.add("a");
    assert_eq!(
        test.queue.add(video("a"), "best-audio", false),
        Err(DownloadError::AlreadyQueued)
    );
    assert!(matches!(
        test.queue.add(video("b"), "no-such-preset", false),
        Err(DownloadError::Preset {
            error: PresetError::NotFound
        })
    ));
}

#[test]
fn keeps_the_preset_as_it_was_when_added() {
    let test = Test::new();
    let id = test.add("a");
    let presets = PresetManager::new(Arc::clone(&test.settings));
    let mut edited = presets.get("best-video").unwrap();
    edited.name = "Renamed".to_owned();
    presets.save(edited).unwrap();
    let item = test
        .queue
        .items()
        .into_iter()
        .find(|item| item.id == id)
        .unwrap();
    let preset = item.preset.unwrap();
    assert_eq!(preset.name, "Best Video");
    assert_eq!(preset.output, built_in_presets()[0].output);
}

#[test]
fn comes_back_after_a_restart_with_running_items_stopped() {
    let test = Test::new();
    test.settings
        .update(|settings| settings.downloads.at_once = 1)
        .unwrap();
    let [first, second] = ["a", "b"].map(|id| test.add(id));
    wait_until("the first runs", || {
        test.state(first) == Some(QueueState::Running)
    });

    // As if the app had closed while the first was running.
    let reopened = test.reopen();
    let states: Vec<_> = reopened
        .items()
        .iter()
        .map(|item| (item.id, item.state))
        .collect();
    assert_eq!(states[0], (first, QueueState::Stopped));
    assert_eq!(states[1].0, second);
    test.queue.cancel(first).unwrap();
    reopened.cancel(second).unwrap();
}

#[test]
fn clearing_removes_only_finished_downloads() {
    let test = Test::new();
    let [done, failed] = ["a", "b"].map(|id| test.add(id));
    wait_until("both run", || test.runner.started().len() == 2);
    test.runner.complete(done, test.finished_file("a.mkv"));
    test.runner
        .complete(failed, JobOutcome::Failed(DownloadError::NoFile));
    wait_until("both end", || {
        test.state(done) == Some(QueueState::Done) && test.state(failed) == Some(QueueState::Failed)
    });

    test.queue.clear_finished().unwrap();
    let ids: Vec<_> = test.queue.items().iter().map(|item| item.id).collect();
    assert_eq!(ids, [failed]);
    assert_eq!(test.queue.store.state_of(done), None);
    assert!(
        test.history
            .last_download("youtube", "a")
            .unwrap()
            .is_some(),
        "it stays in the history"
    );
}

#[test]
fn undo_puts_cleared_downloads_back_where_they_were() {
    let test = Test::new();
    test.settings
        .update(|settings| settings.downloads.at_once = 1)
        .unwrap();
    let [first, failed, moved] = ["a", "b", "c"].map(|id| test.add(id));
    // Its place in the line no longer follows its ID.
    test.queue.download_next(moved).unwrap();
    test.runner.complete(first, test.finished_file("a.mkv"));
    test.runner.complete(moved, test.finished_file("c.mkv"));
    test.runner
        .complete(failed, JobOutcome::Failed(DownloadError::NoFile));
    wait_until("all end", || {
        test.state(first) == Some(QueueState::Done)
            && test.state(moved) == Some(QueueState::Done)
            && test.state(failed) == Some(QueueState::Failed)
    });
    let before = test.queue.items();
    let order: Vec<_> = before.iter().map(|item| item.id).collect();
    assert_eq!(order, [moved, first, failed]);

    assert_eq!(test.queue.clear_finished().unwrap(), 2);
    assert_eq!(test.queue.restore_cleared().unwrap(), 2);
    assert_eq!(
        test.queue.items(),
        before,
        "the same IDs, places and results"
    );
    assert_eq!(test.reopen().items(), before, "saved as before");
    assert_eq!(test.queue.restore_cleared().unwrap(), 0, "only once");
}

#[test]
fn undo_leaves_out_a_download_queued_again() {
    let test = Test::new();
    let done = test.add("a");
    wait_until("it runs", || test.runner.started().len() == 1);
    test.runner.complete(done, test.finished_file("a.mkv"));
    wait_until("it's done", || test.state(done) == Some(QueueState::Done));
    test.queue.clear_finished().unwrap();
    let again = test.add("a");

    assert_eq!(test.queue.restore_cleared().unwrap(), 0);
    let ids: Vec<_> = test.queue.items().iter().map(|item| item.id).collect();
    assert_eq!(ids, [again]);
    test.queue.cancel(again).unwrap();
}

#[test]
fn checks_and_saves_the_options() {
    let test = Test::new();
    let saved = test
        .queue
        .set_options(DownloadSettings {
            folder: Some(" ".to_owned()),
            at_once: 9,
            rate_limit: Some(" 4.2m ".to_owned()),
            cookies_browser: Some(crate::settings::CookiesBrowser::Firefox),
            post_file_name: "  {site} {post} {number} ".to_owned(),
            post_pause: crate::settings::PauseRange { min: 30, max: 5 },
        })
        .unwrap();
    assert_eq!(saved.at_once, MAX_AT_ONCE, "capped");
    assert_eq!(saved.post_file_name, "{site} {post} {number}", "trimmed");
    assert_eq!(saved.rate_limit.as_deref(), Some("4.2M"));
    assert_eq!(saved.folder, None, "blank is the default folder");
    assert_eq!(
        saved.post_pause,
        crate::settings::PauseRange { min: 5, max: 5 },
        "the shortest pause is never longer than the longest"
    );
    assert_eq!(test.queue.options(), saved);
    assert_eq!(
        test.settings.get().downloads,
        saved,
        "saved in settings.json"
    );

    let blank = DownloadSettings {
        rate_limit: Some("   ".to_owned()),
        ..saved.clone()
    };
    assert_eq!(test.queue.set_options(blank).unwrap().rate_limit, None);

    let losing_files = DownloadSettings {
        post_file_name: "{account} {date}".to_owned(),
        ..test.queue.options()
    };
    assert!(matches!(
        test.queue.set_options(losing_files),
        Err(DownloadError::PostFileName { .. })
    ));
    assert_eq!(
        test.queue.options().post_file_name,
        "{site} {post} {number}",
        "the name before stays"
    );
}

#[test]
fn reads_speed_limits_as_yt_dlp_does() {
    assert_eq!(normalize_rate_limit(None), Ok(None));
    assert_eq!(
        normalize_rate_limit(Some("500k")),
        Ok(Some("500K".to_owned()))
    );
    assert_eq!(
        normalize_rate_limit(Some("1000")),
        Ok(Some("1000".to_owned()))
    );
    assert_eq!(
        normalize_rate_limit(Some("1.5G")),
        Ok(Some("1.5G".to_owned()))
    );
    for invalid in ["fast", "2 MB", "-1M", "0", "2T", "M"] {
        assert!(
            matches!(
                normalize_rate_limit(Some(invalid)),
                Err(DownloadError::InvalidRateLimit { .. })
            ),
            "{invalid} should be refused"
        );
    }
}

#[test]
fn raising_downloads_at_once_starts_more_right_away() {
    let test = Test::new();
    test.settings
        .update(|settings| settings.downloads.at_once = 1)
        .unwrap();
    let [first, second] = ["a", "b"].map(|id| test.add(id));
    wait_until("the first runs", || test.runner.started() == [first]);
    let options = DownloadSettings {
        at_once: 2,
        ..test.queue.options()
    };
    test.queue.set_options(options).unwrap();
    wait_until("the second runs", || {
        test.runner.started() == [first, second]
    });
}

#[test]
fn deletes_only_old_logs() {
    let folder = tempfile::tempdir().unwrap();
    fs::write(folder.path().join("1.log"), b"log").unwrap();
    fs::write(folder.path().join("notes.txt"), b"keep").unwrap();
    prune_logs(folder.path(), Duration::from_secs(3600));
    assert!(folder.path().join("1.log").exists(), "a new log stays");
    prune_logs(folder.path(), Duration::ZERO);
    assert!(!folder.path().join("1.log").exists());
    assert!(
        folder.path().join("notes.txt").exists(),
        "only logs are deleted"
    );
}

fn post(post_id: &str) -> VideoInfo {
    VideoInfo {
        site: "instagram".to_owned(),
        site_name: "instagram.com".to_owned(),
        video_id: post_id.to_owned(),
        url: format!("https://www.instagram.com/p/{post_id}/"),
        title: format!("Post {post_id}"),
        uploader: Some("someone".to_owned()),
        duration_seconds: None,
        upload_date: None,
        thumbnail_url: None,
        best_height: None,
        post: Some(crate::metadata::PostContents {
            images: 2,
            videos: 1,
        }),
        files: None,
    }
}

#[test]
fn runs_one_instagram_post_at_a_time() {
    let test = Test::new();
    let first = test.queue.add_post(post("First")).unwrap().id;
    let second = test.queue.add_post(post("Second")).unwrap().id;
    let video = test.add("a");
    wait_until("two run", || test.runner.started().len() == 2);
    let mut running = test.runner.started();
    running.sort_unstable();
    assert_eq!(
        running,
        [first, video],
        "the video goes ahead of the second post"
    );
    assert_eq!(test.state(second), Some(QueueState::Waiting));

    test.runner
        .complete(first, JobOutcome::Failed(DownloadError::NoFile));
    wait_until("the second post starts", || {
        test.runner.started().contains(&second)
    });
}

#[test]
fn downloads_a_post_into_its_sites_folder_at_a_safe_pace() {
    let test = Test::new();
    let id = test.queue.add_post(post("Abc123")).unwrap().id;
    wait_until("it runs", || test.runner.started().contains(&id));

    let job = test.runner.job(id);
    let site_folder = test.folder.path().join("by-site").join("Instagram");
    assert_eq!(job.folder, site_folder);
    assert_eq!(job.preset, None);
    let post_job = job.post.unwrap();
    assert_eq!(post_job.files, 3);
    assert_eq!(post_job.pause, Some(crate::settings::PauseRange::default()));

    fs::create_dir_all(&site_folder).unwrap();
    let files: Vec<PathBuf> = ["1.jpg", "2.mp4", "3.jpg"]
        .iter()
        .map(|name| site_folder.join(name))
        .collect();
    for file in &files {
        fs::write(file, b"image").unwrap();
    }
    test.runner.complete(
        id,
        JobOutcome::Finished {
            files: files.clone(),
            format: None,
        },
    );
    wait_until("it's done", || test.state(id) == Some(QueueState::Done));

    let result = test
        .queue
        .items()
        .into_iter()
        .find(|item| item.id == id)
        .unwrap()
        .result
        .unwrap();
    assert_eq!(result.file_path, files[0].display().to_string());
    assert_eq!(result.file_size, Some(15), "every file counts");
    let last = test
        .history
        .last_download("instagram", "Abc123")
        .unwrap()
        .unwrap();
    assert_eq!(last.preset, "", "a post has no preset");
    assert_eq!(last.file_count, 3);
    assert_eq!(
        test.history.files_of(result.download_id).unwrap().len(),
        3,
        "every file is in the history"
    );
}

#[test]
fn only_paces_sites_that_need_it() {
    let test = Test::new();
    let mut tweet = post("2105698284354367897");
    tweet.site = "twitter".to_owned();
    tweet.url = "https://x.com/NASA/status/2105698284354367897".to_owned();
    let id = test.queue.add_post(tweet).unwrap().id;
    wait_until("it runs", || test.runner.started().contains(&id));
    let job = test.runner.job(id);
    assert!(job.folder.ends_with("X"));
    assert_eq!(job.post.unwrap().pause, None);
    test.queue.cancel(id).unwrap();
}

#[test]
fn a_video_isnt_added_as_a_post() {
    let test = Test::new();
    assert_eq!(
        test.queue.add_post(video("a")),
        Err(DownloadError::WrongState)
    );
    assert!(test.queue.items().is_empty());
}

#[test]
fn downloads_a_file_hosts_link_without_a_preset() {
    let test = Test::new();
    let share = VideoInfo {
        site: "mediafire".to_owned(),
        site_name: "mediafire.com".to_owned(),
        video_id: "abc123".to_owned(),
        url: "https://www.mediafire.com/file/abc123".to_owned(),
        title: "Pack.zip".to_owned(),
        duration_seconds: None,
        best_height: None,
        files: Some(crate::metadata::FileContents {
            count: 1,
            bytes: 2048,
        }),
        ..video("abc123")
    };
    let id = test.queue.add_post(share).unwrap().id;
    wait_until("it runs", || test.runner.started().contains(&id));
    let job = test.runner.job(id);
    assert_eq!(job.preset, None);
    assert_eq!(job.post, None, "it isn't paced as a post");
    assert_eq!(
        job.files,
        Some(crate::metadata::FileContents {
            count: 1,
            bytes: 2048
        })
    );
    test.queue.cancel(id).unwrap();
}

#[test]
fn a_preset_without_a_folder_saves_where_the_catch_all_says() {
    let test = Test::new();
    let everything = test.folder.path().join("everything");
    test.settings
        .update(|settings| {
            for preset in &mut settings.presets.list {
                preset.folder = None;
            }
            settings.downloads.folder = Some(everything.display().to_string());
        })
        .unwrap();
    let item = test.queue.add(video("a"), "best-video", false).unwrap();
    assert_eq!(
        item.folder,
        test.folder
            .path()
            .join("by-site")
            .join("YouTube")
            .display()
            .to_string()
    );
    wait_until("it runs", || test.runner.started().contains(&item.id));
    test.queue.cancel(item.id).unwrap();

    test.settings
        .update(|settings| {
            for rule in &mut settings.rules.list {
                rule.enabled = false;
            }
        })
        .unwrap();
    let preset = test.queue.presets.get("best-video");
    assert_eq!(
        test.queue.destination(&video("b"), preset.as_ref()).folder,
        everything.display().to_string(),
        "with the catch-all off, the download folder"
    );
}

#[test]
fn a_rule_picks_the_folder() {
    use crate::rules::{DestinationSource, FolderRule, RuleCondition, RuleField, RuleOperator};

    let test = Test::new();
    let by_uploader = test.folder.path().join("by-uploader");
    test.settings
        .update(|settings| {
            settings.rules.list.insert(
                0,
                FolderRule {
                    id: "rule-1".to_owned(),
                    enabled: true,
                    conditions: vec![RuleCondition {
                        field: RuleField::Site,
                        operator: RuleOperator::Is,
                        value: "YouTube".to_owned(),
                    }],
                    folder: by_uploader.join("{uploader}").display().to_string(),
                    preset_id: None,
                    download_right_away: None,
                },
            );
        })
        .unwrap();
    let preset = test.queue.presets.get("best-video");
    let destination = test.queue.destination(&video("a"), preset.as_ref());
    assert_eq!(destination.source, DestinationSource::Rule { number: 1 });
    let item = test.queue.add(video("a"), "best-video", false).unwrap();
    assert_eq!(
        item.folder,
        by_uploader.join("Unknown").display().to_string(),
        "a video without an uploader"
    );
    assert_eq!(item.folder, destination.folder);
    wait_until("it runs", || test.runner.started().contains(&item.id));
    test.queue.cancel(item.id).unwrap();
}
