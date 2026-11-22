//! Thumbnails kept in the data folder's `thumbnails\`, so the history keeps its pictures for good: a site's picture can
//! stop loading (Instagram signs its image links, and they expire) or go with a deleted post. Each is a 224×126 JPEG,
//! twice the history row's 112 px and cropped to 16:9 as the row shows it, a few KB, made by ffmpeg from a post's own
//! first file, the site's picture, or a frame (or the cover) of the download's own file. A worker makes them in the
//! background, newest first, so a new download gets one within seconds and the rest of the history follows, best
//! effort. `videos.thumbnail_file` holds each one's name, or "" when none can be made.

use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::history::History;
use crate::process::{ProgramCommand, run_collecting};
use crate::sync::lock;
use crate::tools::{ToolId, ToolManager};

/// The thumbnail's size: twice the history row's 112 px wide, 16:9.
pub const WIDTH: u32 = 224;
pub const HEIGHT: u32 = 126;
/// The largest picture read from a site.
const DOWNLOAD_LIMIT: u64 = 15 << 20;
/// How long ffmpeg may take over one.
const FFMPEG_TIMEOUT: Duration = Duration::from_secs(60);
/// How long the worker waits for the app to settle after it starts, and between looks for new downloads.
const STARTUP_DELAY: Duration = Duration::from_secs(5);
const IDLE_WAIT: Duration = Duration::from_secs(15);
/// The pause between two thumbnails, so a long history is caught up gently.
const PAUSE: Duration = Duration::from_millis(250);

/// A thumbnail's file name for a video: its site, then a hash of its site and ID, which makes any ID safe as a name.
pub fn file_name(site: &str, video_id: &str) -> String {
    let site: String = site
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(24)
        .collect::<String>()
        .to_ascii_lowercase();
    let hash = Sha256::digest(format!("{site}\n{video_id}"));
    let hex: String = hash[..8].iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{site}-{hex}.jpg")
}

/// Whether `name` is one `file_name` gives: the only names the window may ask the thumbnails folder for.
pub fn is_file_name(name: &str) -> bool {
    let Some((site, rest)) = name.split_once('-') else {
        return false;
    };
    let Some(hash) = rest.strip_suffix(".jpg") else {
        return false;
    };
    site.len() <= 24
        && site
            .chars()
            .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        && hash.len() == 16
        && hash
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}

/// What a thumbnail can be made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThumbnailSource {
    /// A file on this PC: a post's image or video, or a downloaded video or song.
    File(PathBuf),
    /// The site's picture.
    Link(String),
}

/// A video in the history without a kept thumbnail, with what one can be made from, best first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailJob {
    pub video_row: i64,
    pub file_name: String,
    pub sources: Vec<ThumbnailSource>,
}

/// Why a source gave no thumbnail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MakeError {
    /// It never will: the file is gone, the site no longer has the picture, or ffmpeg can't read it.
    Gone,
    /// It might later: the network, the site or ffmpeg isn't there right now.
    Unavailable,
}

/// Makes one thumbnail at `output` from a source.
pub trait ThumbnailMaker: Send + Sync {
    fn make(&self, source: &ThumbnailSource, output: &Path) -> Result<(), MakeError>;
}

/// What came of a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Made, from the first source that worked.
    Made,
    /// No source can ever make one (or there's none): remembered, so it isn't tried again.
    Impossible,
    /// Left for the next start, since a source might work then.
    Later,
}

/// Tries each source in turn until one makes the thumbnail at `output`.
pub fn make_from_sources(
    maker: &dyn ThumbnailMaker,
    sources: &[ThumbnailSource],
    output: &Path,
) -> Outcome {
    let mut later = false;
    for source in sources {
        match maker.make(source, output) {
            Ok(()) => return Outcome::Made,
            Err(MakeError::Unavailable) => later = true,
            Err(MakeError::Gone) => {}
        }
    }
    if later {
        Outcome::Later
    } else {
        Outcome::Impossible
    }
}

/// ffmpeg's arguments to make a thumbnail of `input` at `output`: a representative frame among the first 30 (a video
/// can start black; an image or a song's cover is one frame), scaled to cover 224×126 and cropped to it.
pub fn ffmpeg_args(input: &Path, output: &Path) -> Vec<String> {
    [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-y",
        "-i",
        &input.display().to_string(),
        "-frames:v",
        "1",
        "-vf",
        &format!(
            "thumbnail=30,scale={WIDTH}:{HEIGHT}:force_original_aspect_ratio=increase,crop={WIDTH}:{HEIGHT}"
        ),
        "-q:v",
        "5",
        "-update",
        "1",
        &output.display().to_string(),
    ]
    .iter()
    .map(|arg| (*arg).to_owned())
    .collect()
}

/// Makes thumbnails with Tsundoku's ffmpeg, reading a site's picture with `reqwest` first.
pub struct FfmpegMaker {
    pub tools: Arc<ToolManager>,
    /// Where pictures from sites are saved while ffmpeg reads them.
    pub work_folder: PathBuf,
}

impl ThumbnailMaker for FfmpegMaker {
    fn make(&self, source: &ThumbnailSource, output: &Path) -> Result<(), MakeError> {
        let ffmpeg = self
            .tools
            .executable(ToolId::Ffmpeg)
            .ok_or(MakeError::Unavailable)?;
        match source {
            ThumbnailSource::File(path) => {
                if !path.exists() {
                    return Err(MakeError::Gone);
                }
                render(&ffmpeg, path, output)
            }
            ThumbnailSource::Link(url) => {
                fs::create_dir_all(&self.work_folder).map_err(|_| MakeError::Unavailable)?;
                let picture = self
                    .work_folder
                    .join(format!(".download-{}", std::process::id()));
                download(url, &picture)?;
                let made = render(&ffmpeg, &picture, output);
                let _ = fs::remove_file(&picture);
                made
            }
        }
    }
}

/// Saves a site's picture at `path`. A refusal (4xx) means the link is gone for good.
fn download(url: &str, path: &Path) -> Result<(), MakeError> {
    let client = crate::hosts::client().map_err(|_| MakeError::Unavailable)?;
    let response = client.get(url).send().map_err(|_| MakeError::Unavailable)?;
    let status = response.status();
    if status.is_client_error() {
        return Err(MakeError::Gone);
    }
    if !status.is_success() {
        return Err(MakeError::Unavailable);
    }
    let mut bytes = Vec::new();
    response
        .take(DOWNLOAD_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| MakeError::Unavailable)?;
    if bytes.len() as u64 > DOWNLOAD_LIMIT {
        return Err(MakeError::Gone);
    }
    fs::write(path, bytes).map_err(|_| MakeError::Unavailable)
}

/// Runs ffmpeg on `input`, writing beside `output` first so a half-written thumbnail never shows.
fn render(ffmpeg: &Path, input: &Path, output: &Path) -> Result<(), MakeError> {
    let partial = output.with_extension("part.jpg");
    let command = ProgramCommand::new(ffmpeg).args(ffmpeg_args(input, &partial));
    let made = run_collecting(&command, FFMPEG_TIMEOUT)
        .map_err(|_| MakeError::Unavailable)?
        .status
        .success();
    if made && partial.exists() {
        fs::rename(&partial, output).map_err(|_| MakeError::Unavailable)
    } else {
        let _ = fs::remove_file(&partial);
        Err(MakeError::Gone)
    }
}

/// The background worker that keeps the history's thumbnails.
pub struct Thumbnails {
    folder: PathBuf,
    history: Arc<History>,
    maker: Arc<dyn ThumbnailMaker>,
    /// Videos tried since the app started, so one that can't be made now isn't tried again until the next start.
    tried: Mutex<HashSet<i64>>,
    woken: (Mutex<bool>, Condvar),
}

impl Thumbnails {
    pub fn new(
        folder: PathBuf,
        history: Arc<History>,
        maker: Arc<dyn ThumbnailMaker>,
    ) -> Arc<Self> {
        Arc::new(Self {
            folder,
            history,
            maker,
            tried: Mutex::new(HashSet::new()),
            woken: (Mutex::new(false), Condvar::new()),
        })
    }

    /// Starts the worker on a thread of its own: it settles the folder with the history once, then makes the missing
    /// thumbnails, newest first, and looks for new downloads every 15 seconds or when woken.
    pub fn start(self: &Arc<Self>) {
        let thumbnails = Arc::clone(self);
        thread::spawn(move || {
            thread::sleep(STARTUP_DELAY);
            let _ = thumbnails.settle();
            loop {
                let _ = thumbnails.catch_up(PAUSE);
                thumbnails.wait(IDLE_WAIT);
            }
        });
    }

    /// Asks the worker to look for videos without a thumbnail now, such as after a download finished.
    pub fn wake(&self) {
        *lock(&self.woken.0) = true;
        self.woken.1.notify_all();
    }

    fn wait(&self, timeout: Duration) {
        let woken = lock(&self.woken.0);
        let (mut woken, _) = self
            .woken
            .1
            .wait_timeout_while(woken, timeout, |woken| !*woken)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *woken = false;
    }

    /// Makes the folder and the history agree: a thumbnail the history names but the folder lacks (a restored backup,
    /// a folder emptied by hand) is made again, and files no video names, or left halfway, are deleted.
    pub fn settle(&self) -> Result<(), crate::history::HistoryError> {
        let named = self.history.thumbnail_files()?;
        let on_disk: HashSet<String> = fs::read_dir(&self.folder)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        let missing: Vec<i64> = named
            .iter()
            .filter(|(_, name)| !on_disk.contains(name))
            .map(|(row, _)| *row)
            .collect();
        self.history.clear_thumbnails(&missing)?;
        let kept: HashSet<&str> = named.iter().map(|(_, name)| name.as_str()).collect();
        for name in &on_disk {
            if !kept.contains(name.as_str()) {
                let _ = fs::remove_file(self.folder.join(name));
            }
        }
        Ok(())
    }

    /// Makes a thumbnail for each video without one that wasn't tried since the app started, newest first, pausing
    /// `pause` between them.
    pub fn catch_up(&self, pause: Duration) -> Result<(), crate::history::HistoryError> {
        let jobs = self.history.needing_thumbnails()?;
        for job in jobs {
            if !lock(&self.tried).insert(job.video_row) {
                continue;
            }
            if fs::create_dir_all(&self.folder).is_err() {
                return Ok(());
            }
            let output = self.folder.join(&job.file_name);
            match make_from_sources(self.maker.as_ref(), &job.sources, &output) {
                Outcome::Made => self.history.set_thumbnail(job.video_row, &job.file_name)?,
                Outcome::Impossible => self.history.set_thumbnail(job.video_row, "")?,
                Outcome::Later => {}
            }
            thread::sleep(pause);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_a_thumbnail_safely_and_the_same_each_time() {
        let name = file_name("youtube", "jNQXAC9IVRw");
        assert_eq!(name, file_name("youtube", "jNQXAC9IVRw"));
        assert_ne!(name, file_name("vimeo", "jNQXAC9IVRw"), "the site matters");
        assert!(name.starts_with("youtube-") && name.ends_with(".jpg"));
        assert!(is_file_name(&name));
        let odd = file_name("Generic Site/../", r"..\..\evil:id");
        assert!(is_file_name(&odd), "{odd}");
        assert!(!odd.contains(['/', '\\', ':']));
    }

    #[test]
    fn takes_only_its_own_names() {
        for name in [
            "",
            "youtube.jpg",
            "youtube-0123456789abcdef.png",
            "youtube-0123456789ABCDEF.jpg",
            "you/tube-0123456789abcdef.jpg",
            "..-0123456789abcdef.jpg",
            r"..\youtube-0123456789abcdef.jpg",
            "youtube-0123456789abcde.jpg",
        ] {
            assert!(!is_file_name(name), "{name}");
        }
        assert!(is_file_name("youtube-0123456789abcdef.jpg"));
    }

    /// Answers each source as it's told, and records what it was asked.
    struct FakeMaker {
        answers: Vec<(ThumbnailSource, Result<(), MakeError>)>,
        asked: Mutex<Vec<ThumbnailSource>>,
    }

    impl ThumbnailMaker for FakeMaker {
        fn make(&self, source: &ThumbnailSource, output: &Path) -> Result<(), MakeError> {
            lock(&self.asked).push(source.clone());
            let answer = self
                .answers
                .iter()
                .find(|(known, _)| known == source)
                .map(|(_, answer)| answer.clone())
                .unwrap_or(Err(MakeError::Gone));
            if answer.is_ok() {
                fs::write(output, b"jpeg").unwrap();
            }
            answer
        }
    }

    fn fake(answers: Vec<(ThumbnailSource, Result<(), MakeError>)>) -> FakeMaker {
        FakeMaker {
            answers,
            asked: Mutex::new(Vec::new()),
        }
    }

    #[test]
    fn tries_each_source_in_turn() {
        let file = ThumbnailSource::File(PathBuf::from(r"C:\gone.jpg"));
        let link = ThumbnailSource::Link("https://example.com/a.jpg".to_owned());
        let folder = tempfile::tempdir().unwrap();
        let output = folder.path().join("out.jpg");

        let second_works = fake(vec![(link.clone(), Ok(()))]);
        assert_eq!(
            make_from_sources(&second_works, &[file.clone(), link.clone()], &output),
            Outcome::Made
        );
        assert_eq!(*lock(&second_works.asked), [file.clone(), link.clone()]);

        let all_gone = fake(Vec::new());
        assert_eq!(
            make_from_sources(&all_gone, &[file.clone(), link.clone()], &output),
            Outcome::Impossible
        );
        assert_eq!(
            make_from_sources(&all_gone, &[], &output),
            Outcome::Impossible,
            "nothing to make one from"
        );

        let offline = fake(vec![(link.clone(), Err(MakeError::Unavailable))]);
        assert_eq!(
            make_from_sources(&offline, &[file, link], &output),
            Outcome::Later,
            "tried again at the next start"
        );
    }

    use crate::database::Database;
    use crate::history::{HistoryQuery, NewDownload, SavedFile};
    use crate::metadata::{FileContents, PostContents, VideoInfo};
    use crate::rules::DownloadKind;

    struct TestThumbnails {
        history: Arc<History>,
        folder: tempfile::TempDir,
    }

    impl TestThumbnails {
        fn new() -> Self {
            let folder = tempfile::tempdir().unwrap();
            let database = Arc::new(Database::open_in_memory().unwrap());
            let history = History::new(database, folder.path().join("archive.txt"))
                .unwrap()
                .with_thumbnails_folder(folder.path().join("thumbnails"));
            Self {
                history: Arc::new(history),
                folder,
            }
        }

        fn thumbnails(&self, maker: &Arc<FakeMaker>) -> Arc<Thumbnails> {
            Thumbnails::new(
                self.folder.path().join("thumbnails"),
                Arc::clone(&self.history),
                Arc::clone(maker) as Arc<dyn ThumbnailMaker>,
            )
        }

        /// Records a download of `kind`, its file at `file`, and answers the video's row.
        fn record(
            &self,
            video_id: &str,
            kind: DownloadKind,
            picture: Option<&str>,
            file: &str,
        ) -> i64 {
            let video = VideoInfo {
                site: "instagram".to_owned(),
                site_name: "Instagram".to_owned(),
                video_id: video_id.to_owned(),
                url: format!("https://example.com/{video_id}"),
                title: video_id.to_owned(),
                uploader: None,
                account: None,
                duration_seconds: None,
                upload_date: None,
                thumbnail_url: picture.map(str::to_owned),
                best_height: None,
                post: (kind == DownloadKind::Post).then_some(PostContents {
                    images: 1,
                    videos: 0,
                    others: 0,
                }),
                files: (kind == DownloadKind::File).then_some(FileContents { count: 1, bytes: 1 }),
                post_files: Vec::new(),
                shared_files: Vec::new(),
                playlist: None,
                tags: Vec::new(),
                pixiv: None,
                fanbox: None,
            };
            self.history
                .record(
                    &video,
                    &NewDownload {
                        preset: String::new(),
                        file_path: file.to_owned(),
                        files: vec![SavedFile {
                            path: file.to_owned(),
                            size: None,
                        }],
                        file_size: None,
                        format: None,
                        downloaded_at: 1,
                        kind,
                        share_paths: Vec::new(),
                    },
                )
                .unwrap();
            self.row(video_id).id
        }

        fn row(&self, video_id: &str) -> crate::history::HistoryVideo {
            self.history
                .page(&HistoryQuery::default(), 0, 100)
                .unwrap()
                .videos
                .into_iter()
                .find(|video| video.video.video_id == video_id)
                .unwrap()
        }
    }

    #[test]
    fn makes_each_videos_thumbnail_from_its_best_source() {
        let test = TestThumbnails::new();
        test.record(
            "post",
            DownloadKind::Post,
            Some("https://cdn/post.jpg"),
            r"C:\post.jpg",
        );
        test.record(
            "video",
            DownloadKind::Video,
            Some("https://cdn/video.jpg"),
            r"C:\video.mkv",
        );
        test.record("share", DownloadKind::File, None, r"C:\pack.zip");
        let jobs = test.history.needing_thumbnails().unwrap();
        let sources: Vec<_> = jobs.iter().map(|job| job.sources.clone()).collect();
        assert_eq!(
            sources,
            [
                vec![],
                vec![
                    ThumbnailSource::Link("https://cdn/video.jpg".to_owned()),
                    ThumbnailSource::File(PathBuf::from(r"C:\video.mkv")),
                ],
                vec![
                    ThumbnailSource::File(PathBuf::from(r"C:\post.jpg")),
                    ThumbnailSource::Link("https://cdn/post.jpg".to_owned()),
                ],
            ],
            "newest first: a post's own file before its picture, a video's picture before its file"
        );

        let maker = Arc::new(fake(vec![
            (ThumbnailSource::File(PathBuf::from(r"C:\post.jpg")), Ok(())),
            (
                ThumbnailSource::Link("https://cdn/video.jpg".to_owned()),
                Err(MakeError::Unavailable),
            ),
        ]));
        let thumbnails = test.thumbnails(&maker);
        thumbnails.catch_up(Duration::ZERO).unwrap();

        let post = test.row("post");
        assert_eq!(post.thumbnail_file, Some(file_name("instagram", "post")));
        assert!(
            test.folder
                .path()
                .join("thumbnails")
                .join(post.thumbnail_file.unwrap())
                .exists()
        );
        assert_eq!(test.row("share").thumbnail_file, None);
        assert_eq!(test.row("video").thumbnail_file, None);
        let still_needed: Vec<i64> = test
            .history
            .needing_thumbnails()
            .unwrap()
            .iter()
            .map(|job| job.video_row)
            .collect();
        assert_eq!(
            still_needed,
            [test.row("video").id],
            "a share can't have one and isn't asked again; a video whose picture couldn't be read now is"
        );

        // Not again until the next start.
        let asked = lock(&maker.asked).len();
        thumbnails.catch_up(Duration::ZERO).unwrap();
        assert_eq!(lock(&maker.asked).len(), asked);
    }

    #[test]
    fn settles_the_folder_with_the_history() {
        let test = TestThumbnails::new();
        let kept = test.record("kept", DownloadKind::Post, None, r"C:\a.jpg");
        let lost = test.record("lost", DownloadKind::Post, None, r"C:\b.jpg");
        let folder = test.folder.path().join("thumbnails");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(file_name("instagram", "kept")), b"jpeg").unwrap();
        fs::write(folder.join("orphan-0123456789abcdef.jpg"), b"jpeg").unwrap();
        test.history
            .set_thumbnail(kept, &file_name("instagram", "kept"))
            .unwrap();
        test.history
            .set_thumbnail(lost, &file_name("instagram", "lost"))
            .unwrap();

        test.thumbnails(&Arc::new(fake(Vec::new())))
            .settle()
            .unwrap();

        assert!(test.row("kept").thumbnail_file.is_some());
        assert_eq!(test.row("lost").thumbnail_file, None, "made again");
        assert!(
            !folder.join("orphan-0123456789abcdef.jpg").exists(),
            "no video names it"
        );

        test.history.forget(kept).unwrap();
        assert!(
            !folder.join(file_name("instagram", "kept")).exists(),
            "Forget deletes it"
        );
    }

    #[test]
    fn asks_ffmpeg_for_one_cropped_frame() {
        let args = ffmpeg_args(Path::new(r"C:\in.mp4"), Path::new(r"C:\out.part.jpg"));
        let joined = args.join(" ");
        assert!(joined.contains(r"-i C:\in.mp4"));
        assert!(joined.contains("-frames:v 1"));
        assert!(joined.contains(
            "thumbnail=30,scale=224:126:force_original_aspect_ratio=increase,crop=224:126"
        ));
        assert_eq!(args.last().unwrap(), r"C:\out.part.jpg");
    }
}
