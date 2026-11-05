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
    outcomes: Mutex<HashMap<i64, JobOutcome>>,
}

impl FakeRunner {
    fn complete(&self, id: i64, outcome: JobOutcome) {
        lock(&self.outcomes).insert(id, outcome);
    }

    fn started(&self) -> Vec<i64> {
        lock(&self.started).clone()
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
            file_path: path,
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
    assert_eq!(item.preset.name, "Best Video");
    assert_eq!(item.preset.output, built_in_presets()[0].output);
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
fn checks_and_saves_the_options() {
    let test = Test::new();
    let saved = test
        .queue
        .set_options(DownloadSettings {
            at_once: 9,
            rate_limit: Some(" 4.2m ".to_owned()),
            cookies_browser: Some(crate::settings::CookiesBrowser::Firefox),
        })
        .unwrap();
    assert_eq!(saved.at_once, MAX_AT_ONCE, "capped");
    assert_eq!(saved.rate_limit.as_deref(), Some("4.2M"));
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
