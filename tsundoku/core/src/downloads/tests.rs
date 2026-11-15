//! The queue's state machine, run against a fake runner the tests finish, fail or let be cancelled.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use super::*;
use crate::data_folder::DataPaths;
use crate::listing::ListingKind;
use crate::passwords::Password;
use crate::presets::built_in_presets;
use crate::rules::DownloadKind;
use crate::settings::{PauseRange, SitePace, SitePacing};

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
            shared: Vec::new(),
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
fn adds_many_at_once_with_one_change() {
    let test = Test::new();
    test.add("a");
    let changes_before = test.listener.changes.load(Ordering::SeqCst);
    let pick = |video, preset_id: Option<&str>| QueuePick {
        video,
        preset_id: preset_id.map(str::to_owned),
        overwrite: false,
        folder: None,
    };
    let added = test.queue.add_many(
        vec![
            pick(video("b"), Some("best-video")),
            pick(post("Post"), None),
            // Already in the queue: skipped.
            pick(video("a"), Some("best-video")),
            pick(video("c"), Some("best-audio")),
        ],
        None,
    );
    assert_eq!(added, Ok(3));
    assert_eq!(
        test.listener.changes.load(Ordering::SeqCst) - changes_before,
        1,
        "the UI hears of them once"
    );
    let items = test.queue.items();
    let ids: Vec<&str> = items
        .iter()
        .map(|item| item.video.video_id.as_str())
        .collect();
    assert_eq!(ids, ["a", "b", "Post", "c"]);
    assert_eq!(
        items[3].preset.as_ref().map(|preset| preset.id.as_str()),
        Some("best-audio")
    );
    assert_eq!(items[2].preset, None);

    // A video without a preset, or a preset that's gone, fails without stopping the rest.
    let added = test.queue.add_many(
        vec![
            pick(video("d"), None),
            pick(video("e"), Some("no-such-preset")),
            pick(video("f"), Some("best-video")),
        ],
        None,
    );
    assert_eq!(added, Err(DownloadError::WrongState));
    assert!(test.queue.contains("youtube", "f"));
}

/// A channel as its picker passes it with its picks.
fn nasa() -> QueueSource {
    QueueSource {
        url: "https://www.youtube.com/@NASA/videos".to_owned(),
        kind: ListingKind::Channel,
        name: "NASA".to_owned(),
        site_name: "youtube.com".to_owned(),
        thumbnail_url: Some("https://yt3.ggpht.com/nasa.jpg".to_owned()),
    }
}

/// Picks from one channel, all with a preset, added together with the channel as their source.
fn add_from_nasa(test: &Test, video_ids: &[&str]) -> Vec<i64> {
    let picks = video_ids
        .iter()
        .map(|id| QueuePick {
            video: video(id),
            preset_id: Some("best-video".to_owned()),
            overwrite: false,
            folder: None,
        })
        .collect();
    test.queue.add_many(picks, Some(nasa())).unwrap();
    video_ids
        .iter()
        .map(|video_id| {
            test.queue
                .items()
                .iter()
                .find(|item| item.video.video_id == *video_id)
                .unwrap()
                .id
        })
        .collect()
}

#[test]
fn downloads_picked_together_keep_their_source_through_a_restart() {
    let test = Test::new();
    test.settings
        .update(|settings| settings.downloads.at_once = 1)
        .unwrap();
    let alone = test.add("a");
    let picked = add_from_nasa(&test, &["b", "c"]);
    // Already in the queue on its own: it keeps having no source.
    test.queue
        .add_many(
            vec![QueuePick {
                video: video("a"),
                preset_id: Some("best-video".to_owned()),
                overwrite: false,
                folder: None,
            }],
            Some(nasa()),
        )
        .unwrap();
    for items in [test.queue.items(), test.queue.store.load().unwrap()] {
        let sources: Vec<(&str, Option<&str>)> = items
            .iter()
            .map(|item| {
                (
                    item.video.video_id.as_str(),
                    item.source.as_ref().map(|source| source.name.as_str()),
                )
            })
            .collect();
        assert_eq!(
            sources,
            [("a", None), ("b", Some("NASA")), ("c", Some("NASA"))]
        );
        assert_eq!(items[1].source, Some(nasa()));
    }

    // Removed from the queue and put back, it keeps it too.
    wait_until("the first runs", || test.runner.started() == [alone]);
    test.runner.complete(alone, test.finished_file("a.mkv"));
    wait_until("the next runs", || {
        test.runner.started() == [alone, picked[0]]
    });
    test.runner.complete(picked[0], test.finished_file("b.mkv"));
    wait_until("it's done", || {
        test.state(picked[0]) == Some(QueueState::Done)
    });
    test.queue.remove_finished(picked[0]).unwrap();
    test.queue.restore_cleared().unwrap();
    let restored = test.queue.store.load().unwrap();
    let restored = restored.iter().find(|item| item.id == picked[0]).unwrap();
    assert_eq!(restored.source, Some(nasa()));
    test.queue.stop_all().unwrap();
}

#[test]
fn a_sources_downloads_pause_resume_and_retry_together_and_nothing_else() {
    let test = Test::new();
    test.settings
        .update(|settings| settings.downloads.at_once = 1)
        .unwrap();
    let alone = test.add("a");
    let picked = add_from_nasa(&test, &["b", "c", "d"]);
    wait_until("the first runs", || test.runner.started() == [alone]);

    // The waiting ones are held; the download added on its own goes on.
    test.queue.stop_these(&picked).unwrap();
    for id in &picked {
        assert_eq!(test.state(*id), Some(QueueState::Stopped));
    }
    assert_eq!(test.state(alone), Some(QueueState::Running));

    // Resumed, they wait their turn; a running one stops keeping its partial files, and none of the others starts.
    test.queue.resume_these(&picked).unwrap();
    test.runner.complete(alone, test.finished_file("a.mkv"));
    wait_until("the first of them runs", || {
        test.state(picked[0]) == Some(QueueState::Running)
    });
    test.queue.stop_these(&picked).unwrap();
    wait_until("it stops", || {
        test.state(picked[0]) == Some(QueueState::Stopped)
    });
    assert!(test.temp_folder(picked[0]).exists());
    assert_eq!(test.runner.started(), [alone, picked[0]]);

    // Retry takes only the failed one.
    test.queue.resume_these(&picked[..1]).unwrap();
    wait_until("it runs again", || test.runner.started().len() == 3);
    test.runner
        .complete(picked[0], JobOutcome::Failed(DownloadError::NoFile));
    wait_until("it fails", || {
        test.state(picked[0]) == Some(QueueState::Failed)
    });
    test.queue.retry_these(&picked).unwrap();
    wait_until("it runs once more", || test.runner.started().len() == 4);
    assert_eq!(test.runner.started()[3], picked[0]);
    assert_eq!(test.state(picked[1]), Some(QueueState::Stopped));
    assert_eq!(test.state(picked[2]), Some(QueueState::Stopped));
    test.queue.stop_all().unwrap();
}

#[test]
fn a_sources_downloads_can_be_cancelled_together_leaving_the_finished_ones() {
    let test = Test::new();
    test.settings
        .update(|settings| settings.downloads.at_once = 1)
        .unwrap();
    let alone = test.add("a");
    wait_until("it runs", || test.runner.started() == [alone]);
    test.runner.complete(alone, test.finished_file("a.mkv"));
    wait_until("it's done", || test.state(alone) == Some(QueueState::Done));
    let picked = add_from_nasa(&test, &["b", "c", "d"]);
    let [first, second, third] = picked[..] else {
        unreachable!("three were picked")
    };
    wait_until("the first runs", || {
        test.state(first) == Some(QueueState::Running)
    });
    test.runner.complete(first, test.finished_file("b.mkv"));
    wait_until("the second runs", || {
        test.state(second) == Some(QueueState::Running)
    });
    test.queue.stop(third).unwrap();

    test.queue.cancel_these(&picked).unwrap();
    assert_eq!(test.state(third), None, "one not running goes at once");
    wait_until("the running one is gone", || test.state(second).is_none());
    assert!(!test.temp_folder(second).exists());
    assert_eq!(
        test.state(first),
        Some(QueueState::Done),
        "a finished one stays"
    );
    assert_eq!(test.state(alone), Some(QueueState::Done));
    assert_eq!(test.runner.started(), [alone, first, second]);
    let saved: Vec<i64> = test
        .queue
        .store
        .load()
        .unwrap()
        .iter()
        .map(|item| item.id)
        .collect();
    assert_eq!(saved, [alone, first]);

    // Clear Finished for them then takes the finished one only, and Undo puts it back.
    assert_eq!(test.queue.clear_these(&picked), Ok(1));
    assert_eq!(test.state(alone), Some(QueueState::Done));
    assert_eq!(test.state(first), None);
    assert_eq!(test.queue.restore_cleared(), Ok(1));
    assert_eq!(test.state(first), Some(QueueState::Done));
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
fn a_download_can_go_to_a_folder_picked_for_it() {
    let test = Test::new();
    test.settings
        .update(|settings| settings.downloads.at_once = 1)
        .unwrap();
    let first = test.add("a");
    let picked = test.folder.path().join("picked").display().to_string();
    let second = test
        .queue
        .add_in(
            video("b"),
            "best-video",
            false,
            Some(picked.clone()),
            None,
            None,
        )
        .unwrap();
    assert_eq!(second.folder, picked);
    assert!(second.folder_chosen);

    wait_until("the first runs", || {
        test.state(first) == Some(QueueState::Running)
    });
    assert_eq!(
        test.queue.set_folder(first, Some(picked.clone())),
        Err(DownloadError::WrongState),
        "not while it runs"
    );

    let back = test.queue.set_folder(second.id, None).unwrap();
    assert!(!back.folder_chosen);
    assert_ne!(back.folder, picked, "back in the rules' folder");
    let relative = test
        .queue
        .set_folder(second.id, Some("Clips".to_owned()))
        .unwrap();
    assert!(relative.folder_chosen);
    assert!(
        Path::new(&relative.folder).is_absolute() && relative.folder.ends_with("Clips"),
        "inside the download folder: {}",
        relative.folder
    );

    let reopened = test.reopen();
    let kept = reopened
        .items()
        .into_iter()
        .find(|item| item.id == second.id)
        .unwrap();
    assert_eq!((kept.folder, kept.folder_chosen), (relative.folder, true));
}

#[test]
fn picking_another_folder_for_a_stopped_download_drops_its_partial_files() {
    let test = Test::new();
    let id = test.add("a");
    wait_until("it runs", || test.state(id) == Some(QueueState::Running));
    test.queue.stop(id).unwrap();
    wait_until("it stops", || test.state(id) == Some(QueueState::Stopped));
    assert!(test.temp_folder(id).exists());

    let picked = test.folder.path().join("elsewhere").display().to_string();
    test.queue.set_folder(id, Some(picked)).unwrap();
    assert!(!test.temp_folder(id).exists());
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
fn an_archive_that_wasnt_unpacked_is_only_unpacked_again() {
    let test = Test::new();
    let id = test.add("a");
    wait_until("it runs", || test.runner.started().len() == 1);
    assert!(
        test.runner.job(id).unpack.is_some(),
        "unpacking is on by default"
    );
    assert_eq!(test.runner.job(id).unpack_only, None);
    let archive = test.folder.path().join("downloads").join("Album.rar");
    // The runner makes this folder only after it has noted the start, so it may not be there yet.
    fs::create_dir_all(archive.parent().unwrap()).unwrap();
    fs::write(&archive, b"rar").unwrap();
    let files = DownloadedFiles {
        settled: Vec::new(),
        archives: vec![archive.clone()],
        shared: Vec::new(),
    };
    let problem = crate::unpack::UnpackProblem::NeedsPassword;
    test.runner.complete(
        id,
        JobOutcome::NotUnpacked {
            files: files.clone(),
            error: Some(DownloadError::Unpack {
                archive: "Album.rar".to_owned(),
                problem: problem.clone(),
            }),
        },
    );
    wait_until("it fails", || test.state(id) == Some(QueueState::Failed));
    let item = test
        .queue
        .items()
        .into_iter()
        .find(|item| item.id == id)
        .unwrap();
    assert_eq!(
        item.error,
        Some(DownloadError::Unpack {
            archive: "Album.rar".to_owned(),
            problem,
        })
    );

    test.queue.retry(id).unwrap();
    wait_until("it runs again", || test.runner.started().len() == 2);
    let second = lock(&test.runner.jobs)[1].clone();
    assert_eq!(second.unpack_only, Some(files.clone()), "only unpacks");

    // Stopped while unpacking, it resumes unpacking; once the archive is gone, it downloads again.
    test.runner.complete(
        id,
        JobOutcome::NotUnpacked {
            files: files.clone(),
            error: None,
        },
    );
    wait_until("it stops", || test.state(id) == Some(QueueState::Stopped));
    fs::remove_file(&archive).unwrap();
    test.queue.resume(id).unwrap();
    wait_until("it runs a third time", || test.runner.started().len() == 3);
    let third = lock(&test.runner.jobs)[2].clone();
    assert_eq!(third.unpack_only, None);
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
        folder_chosen: false,
        rule_id: None,
        source: None,
        file_name: None,
        state,
        overwrite: false,
        added_at: 0,
        result: None,
        error: None,
        progress: None,
        hold: None,
        recovery: Recovery::default(),
        caught_links: Vec::new(),
        unpack_files: None,
        password: None,
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
fn removes_one_finished_download_and_puts_it_back() {
    let test = Test::new();
    let first = test.add("a");
    let second = test.add("b");
    wait_until("both run", || test.runner.started().len() == 2);
    assert!(
        matches!(
            test.queue.remove_finished(first),
            Err(DownloadError::WrongState)
        ),
        "not while it runs"
    );
    test.runner.complete(first, test.finished_file("a.mkv"));
    test.runner.complete(second, test.finished_file("b.mkv"));
    wait_until("both are done", || {
        test.state(first) == Some(QueueState::Done) && test.state(second) == Some(QueueState::Done)
    });
    let before = test.queue.items();

    test.queue.remove_finished(first).unwrap();
    let ids: Vec<_> = test.queue.items().iter().map(|item| item.id).collect();
    assert_eq!(ids, [second], "only that one");
    assert_eq!(test.reopen().items().len(), 1, "removed for good");

    assert_eq!(test.queue.restore_cleared().unwrap(), 1);
    assert_eq!(test.queue.items(), before, "back in its place");
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
            pacing: SitePacing {
                instagram: SitePace {
                    at_once: 0,
                    pause: PauseRange { min: 30, max: 5 },
                },
                x: SitePace {
                    at_once: 9,
                    pause: PauseRange { min: 2, max: 900 },
                },
                ..SitePacing::default()
            },
            hourly_file_limit: Some(0),
            unpack: crate::settings::UnpackSettings {
                enabled: false,
                place: crate::settings::UnpackPlace::InPlace,
                after: crate::settings::AfterUnpacking::Delete,
                passwords: ["", "  ", "one", " two ", "one", "three\r\n"]
                    .map(str::to_owned)
                    .to_vec(),
            },
        })
        .unwrap();
    assert_eq!(saved.at_once, MAX_AT_ONCE, "capped");
    assert_eq!(saved.hourly_file_limit, None, "0 is no limit");
    assert_eq!(
        saved.unpack.passwords,
        ["one", " two ", "three"],
        "blank ones and repeats dropped, spaces kept"
    );
    assert!(!saved.unpack.enabled);
    assert_eq!(saved.unpack.place, crate::settings::UnpackPlace::InPlace);
    assert_eq!(saved.post_file_name, "{site} {post} {number}", "trimmed");
    assert_eq!(saved.rate_limit.as_deref(), Some("4.2M"));
    assert_eq!(saved.folder, None, "blank is the default folder");
    assert_eq!(
        saved.pacing.instagram,
        SitePace {
            at_once: 1,
            pause: PauseRange { min: 5, max: 5 },
        },
        "one at a time at least, and the shortest pause never longer than the longest"
    );
    assert_eq!(
        saved.pacing.x,
        SitePace {
            at_once: MAX_AT_ONCE,
            pause: PauseRange { min: 2, max: 600 },
        },
        "capped"
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
        account: None,
        duration_seconds: None,
        upload_date: None,
        thumbnail_url: None,
        best_height: None,
        post: Some(crate::metadata::PostContents {
            images: 2,
            videos: 1,
            others: 0,
        }),
        files: None,
        post_files: Vec::new(),
        shared_files: Vec::new(),
        playlist: None,
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

/// A post on X.
fn x_post(post_id: &str) -> VideoInfo {
    VideoInfo {
        site: "twitter".to_owned(),
        site_name: "x.com".to_owned(),
        url: format!("https://x.com/someone/status/{post_id}"),
        ..post(post_id)
    }
}

#[test]
fn runs_as_many_of_a_sites_downloads_at_once_as_its_pacing_lets_it() {
    let test = Test::new();
    test.settings
        .update(|settings| settings.downloads.at_once = 4)
        .unwrap();
    let first = test.queue.add_post(x_post("1")).unwrap().id;
    let second = test.queue.add_post(x_post("2")).unwrap().id;
    let instagram = test.queue.add_post(post("Ig")).unwrap().id;
    wait_until("one X post and the Instagram one run", || {
        test.runner.started().len() == 2
    });
    assert!(
        test.runner.started().contains(&instagram),
        "another site's post goes ahead"
    );
    assert_eq!(
        test.runner.job(first).post.unwrap().pause,
        Some(PauseRange::SHORT),
        "X pauses a little to start with"
    );
    assert_eq!(test.state(second), Some(QueueState::Waiting));

    let pace = SitePace {
        at_once: 3,
        pause: PauseRange { min: 2, max: 4 },
    };
    let options = DownloadSettings {
        pacing: SitePacing {
            x: pace,
            ..test.queue.options().pacing
        },
        ..test.queue.options()
    };
    test.queue.set_options(options).unwrap();
    let third = test.queue.add_post(x_post("3")).unwrap().id;
    let fourth = test.queue.add_post(x_post("4")).unwrap().id;
    wait_until("three X posts run, with the Instagram one", || {
        test.runner.started().len() == 4
    });
    assert!(test.runner.started().contains(&second));
    assert!(test.runner.started().contains(&third));
    assert_eq!(
        test.runner.job(third).post.unwrap().pause,
        Some(pace.pause),
        "each with the whole pause of its own"
    );
    assert_eq!(test.state(fourth), Some(QueueState::Waiting));
    test.runner
        .complete(first, JobOutcome::Failed(DownloadError::NoFile));
    wait_until("the next X post starts", || {
        test.runner.started().contains(&fourth)
    });
}

#[test]
fn a_sites_videos_count_among_its_downloads_at_once_and_a_pause_of_nothing_is_none() {
    let test = Test::new();
    let no_pause = SitePace {
        at_once: 1,
        pause: PauseRange { min: 0, max: 0 },
    };
    test.settings
        .update(|settings| {
            settings.downloads.at_once = 3;
            settings.downloads.pacing.x = no_pause;
        })
        .unwrap();
    let x_video = VideoInfo {
        post: None,
        ..x_post("Video")
    };
    let video = test.queue.add(x_video, "best-video", false).unwrap().id;
    let post_id = test.queue.add_post(x_post("Post")).unwrap().id;
    wait_until("the video runs", || test.runner.started() == [video]);
    assert_eq!(test.state(post_id), Some(QueueState::Waiting));
    test.runner
        .complete(video, JobOutcome::Failed(DownloadError::NoFile));
    wait_until("the post starts", || {
        test.runner.started().contains(&post_id)
    });
    assert_eq!(test.runner.job(post_id).post.unwrap().pause, None);
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
    assert_eq!(post_job.pause, Some(PauseRange::INSTAGRAM));

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
            shared: Vec::new(),
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
fn an_x_post_goes_to_xs_folder_with_xs_own_pause() {
    let test = Test::new();
    let mut tweet = post("2105698284354367897");
    tweet.site = "twitter".to_owned();
    tweet.url = "https://x.com/NASA/status/2105698284354367897".to_owned();
    let id = test.queue.add_post(tweet).unwrap().id;
    wait_until("it runs", || test.runner.started().contains(&id));
    let job = test.runner.job(id);
    assert!(job.folder.ends_with("X"));
    assert_eq!(job.post.unwrap().pause, Some(PauseRange::SHORT));
    test.queue.cancel(id).unwrap();
}

fn too_many_requests() -> JobOutcome {
    JobOutcome::Failed(DownloadError::YtDlp {
        error: PreviewError::too_many_requests(
            "HttpError: '429 Too Many Requests' for 'https://www.instagram.com/api/'".to_owned(),
        ),
    })
}

impl Test {
    fn item(&self, id: i64) -> QueueItem {
        self.queue
            .items()
            .into_iter()
            .find(|item| item.id == id)
            .unwrap()
    }

    fn times_started(&self, id: i64) -> usize {
        self.runner
            .started()
            .iter()
            .filter(|started| **started == id)
            .count()
    }

    fn cancel_all(&self) {
        for item in self.queue.items() {
            if item.state != QueueState::Done {
                self.queue.cancel(item.id).unwrap();
            }
        }
    }
}

#[test]
fn a_site_that_asks_to_slow_down_waits_while_others_go_on() {
    let test = Test::new();
    let first = test.queue.add_post(post("First")).unwrap().id;
    let video_a = test.add("a");
    wait_until("both run", || test.runner.started().len() == 2);

    test.runner.complete(first, too_many_requests());
    // Its hold is worked out once the run has settled, so that's what to wait for.
    wait_until("the post waits again", || test.item(first).hold.is_some());
    let item = test.item(first);
    assert_eq!(item.state, QueueState::Waiting);
    assert_eq!(item.error, None, "it didn't fail");
    let Some(Hold::SlowDown { site, until }) = item.hold else {
        panic!("{:?}", item.hold);
    };
    assert_eq!(site, "Instagram");
    let minutes = (until - now_ms()) as f64 / 60_000.0;
    assert!((4.9..=5.0).contains(&minutes), "{minutes} minutes");
    assert!(
        temp_folder(&test.item(first)).exists(),
        "its partial files are kept"
    );
    assert!(
        test.queue
            .site_wait("https://www.instagram.com/p/Other/")
            .is_some(),
        "reads of the site wait too"
    );

    let second = test.queue.add_post(post("Second")).unwrap().id;
    let video_b = test.add("b");
    wait_until("another site's download starts", || {
        test.runner.started().contains(&video_b)
    });
    assert!(matches!(
        test.item(second).hold,
        Some(Hold::SlowDown { .. })
    ));
    assert_eq!(test.times_started(first), 1, "it doesn't start meanwhile");
    assert_eq!(test.times_started(second), 0);

    test.runner.complete(video_a, test.finished_file("a.mkv"));
    wait_until("a slot frees", || {
        test.state(video_a) == Some(QueueState::Done)
    });
    assert_eq!(test.times_started(first), 1, "still waiting");
    test.queue.end_site_waits();
    wait_until("it starts again once the wait ends", || {
        test.times_started(first) == 2
    });
    assert_eq!(test.item(first).hold, None);
    assert_eq!(
        test.state(second),
        Some(QueueState::Waiting),
        "one Instagram post at a time still"
    );
    test.cancel_all();
}

#[test]
fn the_hourly_limit_holds_instagram_until_its_old_files_are_an_hour_old() {
    let test = Test::new();
    test.settings
        .update(|settings| settings.downloads.hourly_file_limit = Some(4))
        .unwrap();
    let ten_minutes_ago = seconds_since_epoch() - 600;
    let before = NewDownload {
        preset: String::new(),
        file_path: "1.jpg".to_owned(),
        files: ["1.jpg", "2.jpg", "3.mp4"]
            .iter()
            .map(|name| SavedFile {
                path: (*name).to_owned(),
                size: None,
            })
            .collect(),
        file_size: None,
        format: None,
        downloaded_at: ten_minutes_ago,
        kind: DownloadKind::Post,
        share_paths: Vec::new(),
    };
    test.history.record(&post("Before"), &before).unwrap();

    // Three files more would make six.
    let id = test.queue.add_post(post("Next")).unwrap().id;
    let video = test.add("a");
    wait_until("another site's download starts", || {
        test.runner.started().contains(&video)
    });
    assert_eq!(
        test.item(id).hold,
        Some(Hold::HourlyLimit {
            site: "Instagram".to_owned(),
            limit: 4,
            until: (ten_minutes_ago + 3600) * 1000,
        })
    );
    assert_eq!(test.times_started(id), 0);

    let options = DownloadSettings {
        hourly_file_limit: Some(6),
        ..test.queue.options()
    };
    test.queue.set_options(options).unwrap();
    wait_until("it starts once there's room", || {
        test.times_started(id) == 1
    });
    test.cancel_all();
}

fn lost_connection() -> JobOutcome {
    JobOutcome::Failed(DownloadError::YtDlp {
        error: PreviewError::Network {
            detail: "Connection reset by peer".to_owned(),
        },
    })
}

fn limit_reached() -> JobOutcome {
    JobOutcome::Failed(DownloadError::YtDlp {
        error: PreviewError::LimitReached {
            site: "MEGA".to_owned(),
            detail: "MEGA answered -17".to_owned(),
        },
    })
}

/// How far off a hold's end is, in whole seconds.
fn seconds_left(hold: &Hold) -> i64 {
    (hold.until() - now_ms() + 500) / 1000
}

impl Test {
    /// Waits until the download is held, and gives its hold.
    fn held(&self, id: i64) -> Hold {
        wait_until("it waits", || self.item(id).hold.is_some());
        let item = self.item(id);
        assert_eq!(item.state, QueueState::Waiting);
        assert_eq!(item.error, None, "it didn't fail");
        item.hold.unwrap()
    }
}

#[test]
fn a_lost_connection_is_tried_again_three_times_then_fails() {
    let test = Test::new();
    let id = test.add("a");
    for (attempt, seconds) in [(1, 30), (2, 120), (3, 480)] {
        wait_until("it runs", || test.times_started(id) == attempt as usize);
        test.runner.complete(id, lost_connection());
        let hold = test.held(id);
        assert!(
            matches!(hold, Hold::Retry { attempt: shown, attempts: 3, .. } if shown == attempt),
            "{hold:?}"
        );
        assert_eq!(seconds_left(&hold), seconds);
        assert!(
            temp_folder(&test.item(id)).exists(),
            "its partial files are kept"
        );
        test.queue.try_now(id).unwrap();
    }
    wait_until("it runs a fourth time", || test.times_started(id) == 4);
    test.runner.complete(id, lost_connection());
    wait_until("it fails", || test.state(id) == Some(QueueState::Failed));
    assert!(matches!(
        test.item(id).error,
        Some(DownloadError::YtDlp {
            error: PreviewError::Network { .. }
        })
    ));

    test.queue.retry(id).unwrap();
    wait_until("it runs again", || test.times_started(id) == 5);
    test.runner.complete(id, lost_connection());
    assert!(
        matches!(test.held(id), Hold::Retry { attempt: 1, .. }),
        "Retry starts the automatic retries over"
    );
    test.cancel_all();
}

#[test]
fn a_used_up_download_limit_waits_longer_each_time_and_never_fails() {
    let test = Test::new();
    let id = test.add("a");
    for (run, minutes) in [(1, 30), (2, 60), (3, 120), (4, 240), (5, 240)] {
        wait_until("it runs", || test.times_started(id) == run);
        test.runner.complete(id, limit_reached());
        let hold = test.held(id);
        assert!(
            matches!(&hold, Hold::LimitReached { site, .. } if site == "MEGA"),
            "{hold:?}"
        );
        assert_eq!(seconds_left(&hold), minutes * 60);
        test.queue.try_now(id).unwrap();
    }
    wait_until("it runs again", || test.times_started(id) == 6);
    test.runner.complete(id, test.finished_file("a.mkv"));
    wait_until("it's done", || test.state(id) == Some(QueueState::Done));
    assert_eq!(
        test.item(id).recovery,
        Recovery::default(),
        "a finished download starts its retries over"
    );
}

#[test]
fn pause_all_then_resume_all_tries_a_waiting_download_afresh() {
    let test = Test::new();
    let id = test.add("a");
    wait_until("it runs", || test.times_started(id) == 1);
    test.runner.complete(id, limit_reached());
    test.held(id);

    test.queue.stop_all().unwrap();
    assert_eq!(test.state(id), Some(QueueState::Stopped));
    assert_eq!(
        test.queue.try_now(id),
        Err(DownloadError::WrongState),
        "only a held download can be tried now"
    );
    test.queue.resume_all().unwrap();
    wait_until("it runs at once", || test.times_started(id) == 2);
    test.runner.complete(id, limit_reached());
    assert_eq!(
        seconds_left(&test.held(id)),
        30 * 60,
        "back to the first wait"
    );
    test.cancel_all();
}

#[test]
fn try_now_ends_a_sites_wait() {
    let test = Test::new();
    let post_id = test.queue.add_post(post("First")).unwrap().id;
    wait_until("it runs", || test.times_started(post_id) == 1);
    test.runner.complete(post_id, too_many_requests());
    assert!(matches!(test.held(post_id), Hold::SlowDown { .. }));
    let video = test.add("a");
    wait_until("the video runs", || test.times_started(video) == 1);
    assert_eq!(
        test.queue.try_now(video),
        Err(DownloadError::WrongState),
        "a running download isn't held"
    );

    test.queue.try_now(post_id).unwrap();
    wait_until("it starts again", || test.times_started(post_id) == 2);
    assert!(
        test.queue
            .site_wait("https://www.instagram.com/p/Other/")
            .is_none(),
        "the site's reads go on too"
    );
    test.cancel_all();
}

#[test]
fn downloads_only_the_files_of_a_post_picked() {
    let test = Test::new();
    let files = |picked: &[u32]| -> Vec<crate::metadata::PostFile> {
        (1..=3)
            .map(|number| crate::metadata::PostFile {
                number,
                video: number == 2,
                thumbnail_url: None,
                picked: picked.contains(&number),
                name: None,
                size: None,
            })
            .collect()
    };
    let picked = VideoInfo {
        post_files: files(&[1, 3]),
        ..post("Picked")
    };
    let id = test.queue.add_post(picked).unwrap().id;
    wait_until("it runs", || test.runner.started().contains(&id));
    let post_job = test.runner.job(id).post.unwrap();
    assert_eq!(post_job.range, Some(vec![1, 3]));
    assert_eq!(post_job.files, 2, "progress counts the files picked");
    test.queue.cancel(id).unwrap();

    let none = VideoInfo {
        post_files: files(&[]),
        ..post("None")
    };
    assert_eq!(test.queue.add_post(none), Err(DownloadError::NothingPicked));

    let all = VideoInfo {
        post_files: files(&[1, 2, 3]),
        ..post("All")
    };
    let id = test.queue.add_post(all).unwrap().id;
    wait_until("it runs", || test.runner.started().contains(&id));
    assert_eq!(test.runner.job(id).post.unwrap().range, None, "every file");
    test.queue.cancel(id).unwrap();
}

/// A MEGA folder of three files of 10, 20 and 30 bytes, those at `picked` (from 0) ticked.
fn shared_folder(id: &str, picked: &[usize]) -> VideoInfo {
    VideoInfo {
        site: "mega".to_owned(),
        site_name: "mega.nz".to_owned(),
        url: format!("https://mega.nz/folder/{id}#key"),
        duration_seconds: None,
        best_height: None,
        files: Some(crate::metadata::FileContents {
            count: 3,
            bytes: 60,
        }),
        shared_files: ["Pack/a.zip", "Pack/Sub/b.zip", "Pack/c.txt"]
            .iter()
            .enumerate()
            .map(|(index, path)| crate::metadata::SharedFile {
                path: (*path).to_owned(),
                size: 10 * (index as u64 + 1),
                picked: picked.contains(&index),
            })
            .collect(),
        ..video(id)
    }
}

#[test]
fn downloads_only_the_files_of_a_shared_folder_picked() {
    let test = Test::new();
    let id = test
        .queue
        .add_post(shared_folder("Picked", &[0, 2]))
        .unwrap()
        .id;
    wait_until("it runs", || test.runner.started().contains(&id));
    let job = test.runner.job(id);
    assert_eq!(
        job.picked_files,
        Some(vec!["Pack/a.zip".to_owned(), "Pack/c.txt".to_owned()])
    );
    assert_eq!(
        job.files.map(|files| files.count),
        Some(3),
        "the whole folder, as read"
    );
    test.queue.cancel(id).unwrap();

    assert_eq!(
        test.queue.add_post(shared_folder("None", &[])),
        Err(DownloadError::NothingPicked)
    );

    let all = test
        .queue
        .add_post(shared_folder("All", &[0, 1, 2]))
        .unwrap()
        .id;
    wait_until("it runs", || test.runner.started().contains(&all));
    assert_eq!(test.runner.job(all).picked_files, None, "every file");
    test.queue.cancel(all).unwrap();
}

#[test]
fn records_which_files_of_a_shared_folder_a_download_took() {
    let test = Test::new();
    let folder = shared_folder("Taken", &[1]);
    let id = test.queue.add_post(folder.clone()).unwrap().id;
    wait_until("it runs", || test.runner.started().contains(&id));
    let job = test.runner.job(id);
    fs::create_dir_all(job.folder.join("Pack").join("Sub")).unwrap();
    // Saved under another name, since a file of that name was there: the history keeps its path in the share.
    let file = job.folder.join("Pack").join("Sub").join("b (2).zip");
    fs::write(&file, b"twenty bytes of data").unwrap();
    test.runner.complete(
        id,
        JobOutcome::Finished {
            files: vec![file],
            format: None,
            shared: vec!["Pack/Sub/b.zip".to_owned()],
        },
    );
    wait_until("it's done", || test.state(id) == Some(QueueState::Done));
    assert_eq!(
        test.history.left_out_shared_files(&folder),
        Ok(Some(vec![0, 2]))
    );
}

#[test]
fn a_shared_folders_download_knows_what_it_took_once_unpacked() {
    let test = Test::new();
    let folder = shared_folder("Unpacked", &[1]);
    let id = test.queue.add_post(folder.clone()).unwrap().id;
    wait_until("it runs", || test.runner.started().contains(&id));
    let job = test.runner.job(id);
    // The archive taken is gone, unpacked into two files.
    let unpacked = job.folder.join("Pack").join("Sub").join("b");
    fs::create_dir_all(&unpacked).unwrap();
    let files = ["one.txt", "two.txt"].map(|name| unpacked.join(name));
    for file in &files {
        fs::write(file, b"text").unwrap();
    }
    test.runner.complete(
        id,
        JobOutcome::Finished {
            files: files.to_vec(),
            format: None,
            shared: vec!["Pack/Sub/b.zip".to_owned()],
        },
    );
    wait_until("it's done", || test.state(id) == Some(QueueState::Done));
    assert_eq!(
        test.history.left_out_shared_files(&folder),
        Ok(Some(vec![0, 2])),
        "the archive counts as taken"
    );
}

#[test]
fn a_shared_folder_needs_room_for_the_files_picked_only() {
    let folder = shared_folder("Room", &[0]);
    assert_eq!(
        crate::room::expected_bytes(&folder, None),
        Some(10),
        "the file picked"
    );
    assert_eq!(
        crate::room::expected_bytes(&shared_folder("Whole", &[0, 1, 2]), None),
        Some(60)
    );
    assert_eq!(
        folder.picked_contents(),
        Some(crate::metadata::FileContents {
            count: 1,
            bytes: 10
        })
    );
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

/// A drive whose free space the test sets.
struct FakeDisk {
    free: Mutex<u64>,
}

impl crate::room::DiskSpace for FakeDisk {
    fn free(&self, _folder: &Path) -> Option<crate::room::DriveSpace> {
        Some(crate::room::DriveSpace {
            drive: "D:".to_owned(),
            free: *lock(&self.free),
        })
    }
}

const GB: u64 = 1024 * 1024 * 1024;

impl Test {
    /// Has the queue see a drive with `free` bytes, and gives it, to change later.
    fn disk_with(&self, free: u64) -> Arc<FakeDisk> {
        let disk = Arc::new(FakeDisk {
            free: Mutex::new(free),
        });
        self.queue.use_disk_space(disk.clone());
        disk
    }

    /// Adds a file host's link to a file of `bytes`, whose size is known before it starts.
    fn add_share(&self, id: &str, bytes: u64) -> i64 {
        let share = VideoInfo {
            site: "mega".to_owned(),
            site_name: "mega.nz".to_owned(),
            url: format!("https://mega.nz/file/{id}#key"),
            duration_seconds: None,
            best_height: None,
            files: Some(crate::metadata::FileContents { count: 1, bytes }),
            ..video(id)
        };
        self.queue.add_post(share).unwrap().id
    }

    /// Waits until the download waits for room, and gives its hold.
    fn held_for_room(&self, id: i64) -> Hold {
        wait_until("it waits for room", || {
            matches!(self.item(id).hold, Some(Hold::NotEnoughRoom { .. }))
        });
        assert_eq!(self.item(id).state, QueueState::Waiting);
        self.item(id).hold.unwrap()
    }
}

#[test]
fn a_download_without_room_waits_while_others_go_on() {
    let test = Test::new();
    let disk = test.disk_with(GB);
    let big = test.add_share("big", 5 * GB);
    let small = test.add_share("small", GB / 10);
    wait_until("the small one runs", || {
        test.runner.started().contains(&small)
    });
    match test.held_for_room(big) {
        Hold::NotEnoughRoom {
            drive,
            free,
            needed,
            others,
            ..
        } => {
            assert_eq!(drive, "D:");
            assert_eq!(free, GB);
            assert!(needed > 5 * GB, "with a margin");
            assert_eq!(others, 0, "checked before the small one started");
        }
        other => panic!("held for {other:?}"),
    }
    assert!(!test.runner.started().contains(&big));

    *lock(&disk.free) = 10 * GB;
    test.queue.check_room_again();
    wait_until("the big one runs once there's room", || {
        test.runner.started().contains(&big)
    });
    assert_eq!(test.item(big).hold, None);
    test.cancel_all();
}

#[test]
fn a_finished_download_leaves_room_for_the_next() {
    let test = Test::new();
    test.disk_with(3 * GB);
    let first = test.add_share("first", 2 * GB);
    wait_until("the first runs", || test.runner.started().contains(&first));
    let second = test.add_share("second", GB);
    assert!(
        matches!(test.held_for_room(second), Hold::NotEnoughRoom { others, .. } if others == 2 * GB),
        "the first's share is set aside"
    );

    test.runner.complete(first, test.finished_file("first.zip"));
    wait_until("the second runs", || {
        test.runner.started().contains(&second)
    });
    test.cancel_all();
}

#[test]
fn a_download_of_unknown_size_starts_as_before() {
    let test = Test::new();
    test.disk_with(0);
    let id = test.add("unknown-size");
    wait_until("it runs", || test.runner.started().contains(&id));
    test.cancel_all();
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
        test.queue
            .destination(&video("b"), preset.as_ref(), None)
            .folder,
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
            settings.rules.list.push(FolderRule {
                id: "rule-1".to_owned(),
                name: String::new(),
                enabled: true,
                conditions: vec![RuleCondition {
                    field: RuleField::Site,
                    operator: RuleOperator::Is,
                    value: "YouTube".to_owned(),
                    case_sensitive: false,
                    join: crate::rules::ConditionJoin::All,
                }],
                folder: by_uploader.join("{uploader}").display().to_string(),
                file_name: String::new(),
                missing_values: crate::rules::MissingValues::StandIn,
                preset_id: None,
                download_right_away: None,
                unpack: None,
                join: crate::rules::ConditionJoin::All,
            });
        })
        .unwrap();
    let preset = test.queue.presets.get("best-video");
    let destination = test.queue.destination(&video("a"), preset.as_ref(), None);
    assert_eq!(destination.source, DestinationSource::Rule { number: 2 });
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

#[test]
fn a_rule_picked_for_a_download_gives_its_folder_and_name_and_the_item_keeps_it() {
    use crate::rules::FolderRule;

    let test = Test::new();
    let picked_folder = test.folder.path().join("picked");
    test.settings
        .update(|settings| {
            // The video runs, so the post waits.
            settings.downloads.at_once = 1;
            // Off and matching nothing: only picking it makes it decide.
            settings.rules.list.push(FolderRule {
                id: "rule-1".to_owned(),
                enabled: false,
                folder: picked_folder.display().to_string(),
                file_name: "picked {post} {number}".to_owned(),
                conditions: vec![crate::rules::RuleCondition {
                    field: crate::rules::RuleField::Title,
                    operator: crate::rules::RuleOperator::Is,
                    value: "nothing".to_owned(),
                    case_sensitive: false,
                    join: crate::rules::ConditionJoin::All,
                }],
                ..crate::rules::catch_all_rule()
            });
        })
        .unwrap();
    let picked = Some("rule-1".to_owned());
    let video_item = test
        .queue
        .add_in(video("a"), "best-video", false, None, picked.clone(), None)
        .unwrap();
    assert_eq!(video_item.folder, picked_folder.display().to_string());
    assert!(
        !video_item.folder_chosen,
        "the rules' folder, from the rule picked"
    );
    let post_item = test
        .queue
        .add_post_in(post("Picked"), None, picked.clone(), None)
        .unwrap();
    assert_eq!(post_item.folder, picked_folder.display().to_string());
    assert_eq!(
        post_item.file_name.as_deref(),
        Some("picked {post} {number}")
    );
    let usual = test.queue.add_post(post("Usual")).unwrap();
    assert_ne!(usual.folder, post_item.folder, "only where it was picked");

    let elsewhere = test.folder.path().join("elsewhere").display().to_string();
    test.queue
        .set_folder(post_item.id, Some(elsewhere))
        .unwrap();
    let back = test.queue.set_folder(post_item.id, None).unwrap();
    assert_eq!(
        back.folder,
        picked_folder.display().to_string(),
        "the usual folder is the picked rule's"
    );
    let kept = test
        .reopen()
        .items()
        .into_iter()
        .find(|item| item.id == video_item.id)
        .unwrap();
    assert_eq!(kept.rule_id, picked);
}

#[test]
fn a_rule_names_a_posts_files_and_the_item_keeps_it() {
    use crate::rules::FolderRule;

    let test = Test::new();
    test.settings
        .update(|settings| {
            settings.rules.list.push(FolderRule {
                id: "rule-1".to_owned(),
                file_name: "{site} {post} {number}".to_owned(),
                ..crate::rules::catch_all_rule()
            });
        })
        .unwrap();
    let item = test.queue.add_post(post("Named")).unwrap();
    assert_eq!(item.file_name.as_deref(), Some("{site} {post} {number}"));
    let job = test.queue.job_for(&item);
    let format = job.post.map(|post| post.file_name).unwrap_or_default();
    assert!(
        format.ends_with(" Named {num|filename}.{extension}"),
        "named by the rule: {format}"
    );
    let reopened = test.reopen();
    let kept = reopened
        .items()
        .into_iter()
        .find(|queued| queued.id == item.id);
    assert_eq!(
        kept.and_then(|queued| queued.file_name).as_deref(),
        Some("{site} {post} {number}"),
        "saved with the item"
    );
    test.queue.cancel(item.id).unwrap();

    let video = test.queue.add(video("a"), "best-video", false).unwrap();
    assert_eq!(video.file_name, None, "a video keeps its preset's name");
    test.queue.cancel(video.id).unwrap();
}

#[test]
fn a_rule_says_whether_and_where_archives_are_unpacked() {
    use crate::rules::{FolderRule, RuleCondition, RuleField, RuleOperator, RuleUnpacking};
    use crate::settings::UnpackPlace;

    let test = Test::new();
    let unpacking_of = |item: &QueueItem| {
        test.queue
            .job_for(item)
            .unpack
            .map(|job| (job.place, job.after))
    };
    test.settings
        .update(|settings| {
            settings.downloads.unpack.place = UnpackPlace::ArchiveFolder;
            settings.downloads.unpack.after = crate::settings::AfterUnpacking::Keep;
        })
        .unwrap();
    let item = test
        .queue
        .add_post(shared_folder("pack", &[0, 1, 2]))
        .unwrap();
    assert_eq!(
        unpacking_of(&item),
        Some((
            UnpackPlace::ArchiveFolder,
            crate::settings::AfterUnpacking::Keep
        )),
        "Settings"
    );

    let on_mega = |unpack: RuleUnpacking| FolderRule {
        id: format!("rule-{unpack:?}"),
        folder: String::new(),
        conditions: vec![RuleCondition {
            field: RuleField::Site,
            operator: RuleOperator::Is,
            value: "MEGA".to_owned(),
            case_sensitive: false,
            join: crate::rules::ConditionJoin::All,
        }],
        unpack: Some(unpack),
        ..crate::rules::catch_all_rule()
    };
    test.settings
        .update(|settings| settings.rules.list.push(on_mega(RuleUnpacking::InPlace)))
        .unwrap();
    assert_eq!(
        unpacking_of(&item),
        Some((UnpackPlace::InPlace, crate::settings::AfterUnpacking::Keep)),
        "the rule, read when the download runs, with Settings' after"
    );

    test.settings
        .update(|settings| {
            settings.downloads.unpack.enabled = false;
        })
        .unwrap();
    assert_eq!(
        unpacking_of(&item).map(|(place, _)| place),
        Some(UnpackPlace::InPlace),
        "a rule unpacks even while Settings doesn't"
    );

    test.settings
        .update(|settings| {
            settings.downloads.unpack.enabled = true;
            settings.rules.list.push(on_mega(RuleUnpacking::Off));
        })
        .unwrap();
    assert_eq!(unpacking_of(&item), None, "a rule leaves them packed");
    test.queue.cancel(item.id).unwrap();

    let picked = test
        .queue
        .add_post_in(
            shared_folder("picked", &[0]),
            None,
            Some("rule-InPlace".to_owned()),
            None,
        )
        .unwrap();
    assert_eq!(
        unpacking_of(&picked).map(|(place, _)| place),
        Some(UnpackPlace::InPlace),
        "the rule picked for it says first"
    );
    test.queue.cancel(picked.id).unwrap();
}

#[test]
fn a_rule_names_a_shared_file_with_what_its_pattern_captured() {
    use crate::rules::{FolderRule, RuleCondition, RuleField, RuleOperator};

    let test = Test::new();
    test.settings
        .update(|settings| {
            settings.rules.list.push(FolderRule {
                id: "rule-1".to_owned(),
                file_name: "{1}".to_owned(),
                conditions: vec![RuleCondition {
                    field: RuleField::Title,
                    operator: RuleOperator::Matches,
                    value: r"(RJ\d+)".to_owned(),
                    case_sensitive: false,
                    join: crate::rules::ConditionJoin::All,
                }],
                ..crate::rules::catch_all_rule()
            });
        })
        .unwrap();
    let share = VideoInfo {
        site: "mediafire".to_owned(),
        site_name: "mediafire.com".to_owned(),
        video_id: "abc123".to_owned(),
        url: "https://www.mediafire.com/file/abc123".to_owned(),
        title: "SomefileRJ123456.rar".to_owned(),
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
    assert_eq!(
        test.runner.job(id).shared_file_name.as_deref(),
        Some("RJ123456")
    );
    test.queue.cancel(id).unwrap();
}

/// A MediaFire folder's two files behind Cloudflare's check, as a run finds them.
fn mediafire_check() -> crate::hosts::check::BrowserCheck {
    let file = |key: &str| crate::hosts::RemoteFile {
        path: PathBuf::from(format!("{key}.zip")),
        size: 1024,
        source: crate::hosts::FileSource::MediaFire(crate::hosts::mediafire::MediaFireFile {
            quick_key: key.to_owned(),
            sha256: None,
            direct: None,
        }),
    };
    let share = crate::hosts::Share {
        host: crate::hosts::Host::MediaFire,
        id: "folderkey".to_owned(),
        url: "https://www.mediafire.com/folder/folderkey/Pack".to_owned(),
        title: "Pack".to_owned(),
        upload_date: None,
        files: vec![file("firstkey1"), file("secondkey2")],
    };
    crate::hosts::check::BrowserCheck {
        site: "MediaFire".to_owned(),
        pages: share
            .files
            .iter()
            .filter_map(|file| crate::hosts::check::CheckPage::for_file(&share, file))
            .collect(),
    }
}

/// The Download button's own link for a MediaFire file.
fn direct_link(key: &str) -> String {
    format!("https://download2390.mediafire.com/token/{key}/{key}.zip")
}

impl FakeRunner {
    /// The job of the item's latest run.
    fn last_job(&self, id: i64) -> JobSpec {
        lock(&self.jobs)
            .iter()
            .rev()
            .find(|job| job.id == id)
            .cloned()
            .unwrap()
    }
}

impl Test {
    /// Adds a MediaFire folder, and waits until its run is told it needs the check.
    fn held_for_check(&self) -> i64 {
        let folder = VideoInfo {
            site: "mediafire".to_owned(),
            site_name: "mediafire.com".to_owned(),
            url: "https://www.mediafire.com/folder/folderkey/Pack".to_owned(),
            duration_seconds: None,
            best_height: None,
            files: Some(crate::metadata::FileContents {
                count: 3,
                bytes: 3072,
            }),
            ..video("folderkey")
        };
        let id = self.queue.add_post(folder).unwrap().id;
        wait_until("it runs", || self.times_started(id) == 1);
        self.runner
            .complete(id, JobOutcome::NeedsCheck(mediafire_check()));
        id
    }
}

#[test]
fn a_download_behind_a_check_waits_for_a_person_and_continues_with_what_the_window_caught() {
    let test = Test::new();
    let id = test.held_for_check();
    let other = test.add("other");
    assert_eq!(
        test.held(id),
        Hold::BrowserCheck {
            site: "MediaFire".to_owned(),
            files: 2
        }
    );
    wait_until("the others carry on", || {
        test.runner.started().contains(&other)
    });
    assert_eq!(test.item(id).error, None, "it didn't fail");
    assert_eq!(
        test.queue.try_now(id),
        Err(DownloadError::WrongState),
        "only the person gets it past"
    );
    test.queue.end_site_waits();
    assert_eq!(test.times_started(id), 1, "it never starts by itself");

    let check = test.queue.check_for(id).unwrap();
    assert_eq!(check.pages.len(), 2);
    assert_eq!(
        check.pages[0].url,
        "https://www.mediafire.com/file/firstkey1"
    );
    assert_eq!(test.queue.check_for(other), Err(DownloadError::WrongState));

    // The person passed the first file's check, then closed the window.
    test.queue
        .passed_check(id, vec![direct_link("firstkey1")])
        .unwrap();
    assert!(
        matches!(
            test.item(id).hold,
            Some(Hold::BrowserCheck { files: 1, .. })
        ),
        "the second file still waits"
    );
    assert_eq!(test.times_started(id), 1);
    test.queue
        .passed_check(id, vec![direct_link("secondkey2")])
        .unwrap();
    wait_until("it runs again", || test.times_started(id) == 2);
    assert_eq!(
        test.runner.last_job(id).caught_links,
        [direct_link("firstkey1"), direct_link("secondkey2")]
    );
    test.runner
        .complete(id, test.finished_file("firstkey1.zip"));
    wait_until("it's done", || test.state(id) == Some(QueueState::Done));
    assert_eq!(test.item(id).caught_links, Vec::<String>::new());
    test.cancel_all();
}

/// A Workupload link, read through the check window: one file named after the link, its size unknown.
const WORKUPLOAD_LINK: &str = "https://workupload.com/file/abcdefgh";

/// The check a Workupload link's run finds: its page.
fn workupload_check() -> crate::hosts::check::BrowserCheck {
    let host = crate::hosts::Host::of(WORKUPLOAD_LINK).unwrap();
    let share = crate::hosts::windowed::share(host, WORKUPLOAD_LINK);
    crate::hosts::check::BrowserCheck {
        site: "Workupload".to_owned(),
        pages: vec![crate::hosts::check::CheckPage::for_file(&share, &share.files[0]).unwrap()],
    }
}

/// The download the person started on the Workupload page, as the window saw it.
fn window_offer() -> WindowOffer {
    WindowOffer {
        address: "https://f1.workupload.com/download/abcdefgh".to_owned(),
        name: Some("Sample.png".to_owned()),
        total: Some(3),
    }
}

/// What the window was told about the downloads it offered.
type Answers = Arc<Mutex<Vec<WindowAnswer>>>;

impl Test {
    /// Adds a Workupload link, and waits until its run is told it needs the check.
    fn held_for_window(&self) -> i64 {
        let file = VideoInfo {
            site: "workupload".to_owned(),
            site_name: "workupload.com".to_owned(),
            url: WORKUPLOAD_LINK.to_owned(),
            // Named after the link until its file is known, as a link read through the window is.
            title: "abcdefgh".to_owned(),
            duration_seconds: None,
            best_height: None,
            files: Some(crate::metadata::FileContents { count: 1, bytes: 0 }),
            ..video("abcdefgh")
        };
        let id = self.queue.add_post(file).unwrap().id;
        wait_until("it runs", || self.times_started(id) == 1);
        self.runner
            .complete(id, JobOutcome::NeedsCheck(workupload_check()));
        wait_until("it waits for the check", || {
            self.queue.check_for(id).is_ok()
        });
        id
    }

    /// The person starts the file's download in the check window, which offers it to the download.
    fn offer(&self, id: i64) -> Answers {
        let answers = Answers::default();
        let told = Arc::clone(&answers);
        self.queue.offer_window_download(
            id,
            "abcdefgh",
            window_offer(),
            Box::new(move |answer| lock(&told).push(answer)),
        );
        answers
    }

    /// Plays a run that took the offer and found the window's download isn't one Tsundoku can fetch: the window saves
    /// the file, and the run ends asking for the check. Where the window saves it.
    fn left_to_window(&self, id: i64, answers: &Answers) -> PathBuf {
        wait_until("a run takes the offer", || self.times_started(id) == 2);
        let job = self.runner.last_job(id);
        let offer = job.window_offer.expect("the run answers the window");
        offer.leave_to_window();
        let Some(WindowAnswer::SaveIn(path)) = lock(answers).first().cloned() else {
            panic!("the window saves the file");
        };
        self.runner
            .complete(id, JobOutcome::NeedsCheck(workupload_check()));
        wait_until("it waits for the window", || {
            self.state(id) == Some(QueueState::Waiting)
        });
        path
    }
}

#[test]
fn a_download_started_in_the_window_that_tsundoku_can_fetch_is_fetched() {
    let test = Test::new();
    let id = test.held_for_window();
    let answers = test.offer(id);
    wait_until("it runs at once", || test.times_started(id) == 2);
    let offer = test.runner.last_job(id).window_offer.unwrap();
    assert_eq!(offer.file_id, "abcdefgh");
    assert_eq!(offer.offer, window_offer());
    assert!(
        offer.fetch(),
        "the run's first request answered with the file"
    );
    assert_eq!(
        *lock(&answers),
        [WindowAnswer::Fetching],
        "the window stops its own"
    );
    assert!(!offer.fetch(), "answered once");
    let caught = crate::hosts::windowed::caught_link("abcdefgh", &window_offer().address);
    assert_eq!(
        test.item(id).caught_links,
        std::slice::from_ref(&caught),
        "kept for its next runs"
    );

    // It lost its connection: the next run tries the address again, without waiting for the check.
    test.runner.complete(id, lost_connection());
    wait_until("it waits to try again", || {
        matches!(test.item(id).hold, Some(Hold::Retry { .. }))
    });
    test.queue.try_now(id).unwrap();
    wait_until("it runs again", || test.times_started(id) == 3);
    let job = test.runner.last_job(id);
    assert_eq!(job.caught_links, [caught]);
    assert!(job.window_offer.is_none());
    test.runner.complete(id, test.finished_file("Sample.png"));
    wait_until("it's done", || test.state(id) == Some(QueueState::Done));
    assert_eq!(
        *lock(&answers),
        [WindowAnswer::Fetching],
        "nothing more told"
    );
}

#[test]
fn a_file_the_window_saves_is_handed_to_its_download() {
    let test = Test::new();
    let id = test.held_for_window();
    let answers = test.offer(id);
    let path = test.left_to_window(id, &answers);
    let folder = path.parent().unwrap();
    assert!(
        folder.is_dir() && folder == test.runner.last_job(id).temp_folder,
        "the window saves into the temporary folder"
    );
    assert_eq!(
        path.file_name().unwrap(),
        "Sample.png",
        "under the site's name"
    );
    assert_eq!(test.item(id).video.title, "Sample.png", "named at once");
    assert_eq!(
        test.item(id).video.files.map(|contents| contents.bytes),
        Some(3)
    );
    assert_eq!(
        test.held(id),
        Hold::WindowDownload {
            site: "Workupload".to_owned()
        }
    );
    assert!(test.queue.waits_for_window(id));
    test.queue.window_download_progress(id, 1, Some(3));
    let progress = test.item(id).progress.unwrap();
    assert_eq!(
        (progress.downloaded_bytes, progress.total_bytes),
        (Some(1), Some(3))
    );
    std::fs::write(&path, "png").unwrap();
    let link = crate::hosts::windowed::saved_link("abcdefgh", &path);
    test.queue.passed_check(id, vec![link.clone()]).unwrap();
    wait_until("it runs again", || test.times_started(id) == 3);
    assert_eq!(test.runner.last_job(id).caught_links, [link]);
    assert!(!test.queue.waits_for_window(id));
    test.cancel_all();
}

#[test]
fn a_file_saved_while_its_run_still_runs_is_kept() {
    let test = Test::new();
    let id = test.held_for_window();
    let answers = test.offer(id);
    wait_until("a run takes the offer", || test.times_started(id) == 2);
    test.runner
        .last_job(id)
        .window_offer
        .unwrap()
        .leave_to_window();
    let Some(WindowAnswer::SaveIn(path)) = lock(&answers).first().cloned() else {
        panic!("the window saves the file");
    };
    // The window's download ends before the run does.
    std::fs::write(&path, "png").unwrap();
    let link = crate::hosts::windowed::saved_link("abcdefgh", &path);
    test.queue.passed_check(id, vec![link.clone()]).unwrap();
    test.runner
        .complete(id, JobOutcome::NeedsCheck(workupload_check()));
    wait_until("it runs again, without waiting for the check", || {
        test.times_started(id) == 3
    });
    assert_eq!(test.runner.last_job(id).caught_links, [link]);
    test.cancel_all();
}

#[test]
fn an_offer_no_run_takes_in_time_is_left_to_the_window() {
    let test = Test::new();
    test.settings
        .update(|settings| settings.downloads.at_once = 1)
        .unwrap();
    let id = test.held_for_window();
    let busy = test.add("busy");
    wait_until("the other runs", || test.runner.started().contains(&busy));
    let answers = test.offer(id);
    assert_eq!(test.times_started(id), 1, "no room");
    wait_until("the wait runs out", || !lock(&answers).is_empty());
    assert!(matches!(
        lock(&answers).first(),
        Some(WindowAnswer::SaveIn(path)) if path.ends_with("Sample.png")
    ));
    assert_eq!(
        test.held(id),
        Hold::WindowDownload {
            site: "Workupload".to_owned()
        }
    );
    test.cancel_all();
}

#[test]
fn stopping_a_download_refuses_what_the_window_offered() {
    let test = Test::new();
    test.settings
        .update(|settings| settings.downloads.at_once = 1)
        .unwrap();
    let id = test.held_for_window();
    let busy = test.add("busy");
    wait_until("the other runs", || test.runner.started().contains(&busy));
    let answers = test.offer(id);
    test.queue.stop(id).unwrap();
    assert_eq!(*lock(&answers), [WindowAnswer::Refuse], "at once");

    // Stopped while its run had the offer.
    test.queue.resume(id).unwrap();
    test.runner.complete(busy, test.finished_file("busy.mp4"));
    wait_until("it runs again", || test.times_started(id) == 2);
    test.runner
        .complete(id, JobOutcome::NeedsCheck(workupload_check()));
    wait_until("it waits for the check", || {
        test.queue.check_for(id).is_ok()
    });
    let answers = test.offer(id);
    wait_until("a run takes the offer", || test.times_started(id) == 3);
    test.queue.stop(id).unwrap();
    wait_until("the window is told", || !lock(&answers).is_empty());
    assert_eq!(*lock(&answers), [WindowAnswer::Refuse]);
    assert_eq!(test.state(id), Some(QueueState::Stopped));

    let other = test.add("other");
    let refused = Answers::default();
    let told = Arc::clone(&refused);
    test.queue.offer_window_download(
        other,
        "abcdefgh",
        window_offer(),
        Box::new(move |answer| lock(&told).push(answer)),
    );
    assert_eq!(
        *lock(&refused),
        [WindowAnswer::Refuse],
        "a download that doesn't wait for that file"
    );
    test.cancel_all();
}

#[test]
fn closing_the_window_before_it_saves_anything_waits_for_the_check_again() {
    let test = Test::new();
    let id = test.held_for_window();
    let answers = test.offer(id);
    wait_until("a run takes the offer", || test.times_started(id) == 2);
    // Closed while the run asks: the offer is the run's, so the window's close changes nothing yet.
    test.queue.withdraw_window_offer(id);
    assert!(lock(&answers).is_empty());
    test.runner
        .last_job(id)
        .window_offer
        .unwrap()
        .leave_to_window();
    // The window, gone, can't save it.
    test.queue.window_download_withdrawn(id);
    test.runner
        .complete(id, JobOutcome::NeedsCheck(workupload_check()));
    wait_until("it waits for the check again", || {
        test.queue.check_for(id).is_ok()
    });
    assert_eq!(
        test.held(id),
        Hold::BrowserCheck {
            site: "Workupload".to_owned(),
            files: 1
        }
    );
    assert_eq!(test.item(id).error, None, "it didn't fail");
    test.cancel_all();
}

#[test]
fn a_window_download_that_stops_fails_its_download_and_leaves_nothing() {
    let test = Test::new();
    let id = test.held_for_window();
    let answers = test.offer(id);
    let path = test.left_to_window(id, &answers);
    std::fs::write(path.with_extension("png.crdownload"), "half").unwrap();
    test.queue.window_download_stopped(id);
    assert_eq!(test.state(id), Some(QueueState::Failed));
    assert_eq!(
        test.item(id).error,
        Some(DownloadError::WindowDownloadStopped {
            site: "Workupload".to_owned()
        })
    );
    assert!(
        !path.parent().unwrap().exists(),
        "what the window left is deleted"
    );
    assert!(
        !test.queue.waits_for_window(id),
        "the window's watch sees it no longer waits, and closes"
    );
    test.queue.retry(id).unwrap();
    wait_until("Retry runs it again", || test.times_started(id) == 3);
    test.cancel_all();
}

#[test]
fn removing_a_download_while_the_window_saves_it_cleans_up() {
    let test = Test::new();
    let id = test.held_for_window();
    let answers = test.offer(id);
    let path = test.left_to_window(id, &answers);
    std::fs::write(path.with_extension("png.crdownload"), "half").unwrap();
    test.queue.cancel(id).unwrap();
    assert!(!path.parent().unwrap().exists());
    assert!(
        !test.queue.waits_for_window(id),
        "the window's watch sees it's gone, and closes"
    );
    // The window's own end of the download comes too late to change anything.
    test.queue.window_download_stopped(id);
    assert_eq!(test.state(id), None);
}

#[test]
fn a_pasted_download_link_gets_a_waiting_download_past_its_check() {
    let test = Test::new();
    let id = test.held_for_check();
    test.held(id);
    let pasted = VideoInfo {
        site: "mediafire".to_owned(),
        site_name: "mediafire.com".to_owned(),
        url: direct_link("firstkey1"),
        files: Some(crate::metadata::FileContents {
            count: 1,
            bytes: 1024,
        }),
        ..video("firstkey1")
    };
    assert!(
        !test.queue.contains_video(&pasted) && test.queue.check_takes(&pasted.url),
        "the preview card offers to download it"
    );
    let given = test.queue.add_post(pasted.clone()).unwrap();
    assert_eq!(given.id, id, "given to the waiting download, not added");
    assert_eq!(test.queue.items().len(), 1);
    assert!(matches!(
        test.item(id).hold,
        Some(Hold::BrowserCheck { files: 1, .. })
    ));
    assert!(
        test.queue.add_post(pasted).is_ok(),
        "a file outside the check is added as usual"
    );
    assert_eq!(test.queue.items().len(), 2);

    // Resume and Retry try afresh: the run finds the check again if it's still there.
    test.queue.stop(id).unwrap();
    test.queue.resume(id).unwrap();
    wait_until("it runs again", || test.times_started(id) == 2);
    assert_eq!(
        test.runner.last_job(id).caught_links,
        [crate::hosts::windowed::caught_link(
            "firstkey1",
            &direct_link("firstkey1")
        )],
        "keeping what was given, for its own file"
    );
    test.cancel_all();
}

fn needs_a_password() -> JobOutcome {
    JobOutcome::Failed(DownloadError::YtDlp {
        error: PreviewError::PasswordNeeded { wrong: true },
    })
}

#[test]
fn a_download_that_needs_a_password_waits_for_one_then_starts_with_it() {
    let test = Test::new();
    let id = test.add("locked");
    wait_until("it runs", || test.times_started(id) == 1);
    assert_eq!(test.runner.last_job(id).password, None);
    test.runner.complete(id, needs_a_password());
    wait_until("it waits for a password", || {
        test.item(id).hold == Some(Hold::NeedsPassword { wrong: false })
    });
    let item = test.item(id);
    assert_eq!(
        (item.state, item.error),
        (QueueState::Waiting, None),
        "not a failure"
    );
    assert_eq!(
        test.queue.try_now(id),
        Err(DownloadError::WrongState),
        "only a password starts it"
    );
    thread::sleep(Duration::from_millis(1200));
    assert_eq!(
        test.times_started(id),
        1,
        "and it never tries again by itself"
    );

    test.queue
        .enter_password(id, Password::new("not it").unwrap())
        .unwrap();
    wait_until("it runs with the password", || test.times_started(id) == 2);
    assert_eq!(
        test.runner
            .last_job(id)
            .password
            .as_ref()
            .map(Password::reveal),
        Some("not it")
    );
    test.runner.complete(id, needs_a_password());
    wait_until("it asks again, saying it was wrong", || {
        test.item(id).hold == Some(Hold::NeedsPassword { wrong: true })
    });
    assert_eq!(test.item(id).password, None, "a wrong one isn't kept");
    let saved: Option<String> = test
        .database
        .lock()
        .query_row(
            "SELECT password FROM queue_items WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(saved, None, "nor saved");
    assert_eq!(
        test.queue
            .enter_password(id + 100, Password::new("x").unwrap()),
        Err(DownloadError::NotFound)
    );

    test.queue
        .enter_password(id, Password::new("youtube-dl").unwrap())
        .unwrap();
    wait_until("it runs again", || test.times_started(id) == 3);
    test.runner.complete(id, test.finished_file("locked.mkv"));
    wait_until("it finishes", || test.state(id) == Some(QueueState::Done));
    assert_eq!(
        test.history
            .password_for("youtube", "locked")
            .unwrap()
            .as_ref()
            .map(Password::reveal),
        Some("youtube-dl"),
        "the history keeps it for Download Again"
    );
    assert_eq!(
        test.history
            .password_for_link("https://www.youtube.com/watch?v=locked")
            .unwrap()
            .as_ref()
            .map(Password::reveal),
        Some("youtube-dl")
    );

    // Download Again: the new download opens with the history's password, without asking.
    test.queue.clear_finished().unwrap();
    let again = test.add("locked");
    wait_until("it runs", || test.times_started(again) == 1);
    assert_eq!(
        test.runner
            .last_job(again)
            .password
            .as_ref()
            .map(Password::reveal),
        Some("youtube-dl")
    );
    test.queue.cancel(again).unwrap();
}

#[test]
fn a_downloads_password_is_saved_with_it_and_never_sent_to_the_ui() {
    let test = Test::new();
    let id = test
        .queue
        .add_in(
            video("kept"),
            "best-video",
            false,
            None,
            None,
            Password::new("hunter2"),
        )
        .unwrap()
        .id;
    wait_until("it runs", || test.times_started(id) == 1);
    assert_eq!(
        test.runner
            .last_job(id)
            .password
            .as_ref()
            .map(Password::reveal),
        Some("hunter2")
    );
    test.queue.stop(id).unwrap();
    wait_until("it stops", || test.state(id) == Some(QueueState::Stopped));
    let item = test.item(id);
    assert_eq!(
        item.password.as_ref().map(Password::reveal),
        Some("hunter2")
    );
    let json = serde_json::to_string(&item).unwrap();
    assert!(
        !json.contains("hunter2") && !json.contains("password"),
        "{json}"
    );
    assert!(!format!("{item:?}").contains("hunter2"), "nor in its Debug");

    let reopened = test.reopen();
    let after_restart = reopened
        .items()
        .into_iter()
        .find(|item| item.id == id)
        .unwrap();
    assert_eq!(
        after_restart.password.as_ref().map(Password::reveal),
        Some("hunter2"),
        "kept across a restart"
    );
    // Resume runs with it again.
    reopened.resume(id).unwrap();
    wait_until("it runs again", || test.times_started(id) == 2);
    assert!(
        lock(&test.runner.jobs)
            .iter()
            .all(|job| job.password.as_ref().map(Password::reveal) == Some("hunter2"))
    );
    reopened.cancel(id).unwrap();
}
