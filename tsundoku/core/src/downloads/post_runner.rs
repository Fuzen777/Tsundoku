//! Downloading a post of images, such as an Instagram carousel, with gallery-dl: every file of the post, named as
//! Settings → Posts of Images says (`post_names`), into the item's folder. gallery-dl writes the path of each file it saves, which gives the progress and
//! the files made.

use std::path::PathBuf;

use super::runner::{
    JobOutcome, JobSpec, KEPT_ERROR_LINES, PostJob, make_folders, open_log, seconds_since_epoch,
    write_log,
};
use super::{DownloadError, DownloadProgress, DownloadStage};
use crate::cancel::CancelToken;
use crate::gallery::classify_download_failure;
use crate::metadata::PreviewError;
use crate::pixiv_account::{self, GalleryDlConfig};
use crate::pixiv_warning;
use crate::process::{self, OutputLine, ProgramCommand, RunError};
use crate::tools::{ToolId, start_error};

/// What gallery-dl writes for a file it already has, before its path.
const SKIPPED_PREFIX: &str = "# ";

pub(super) fn run(
    job: &JobSpec,
    post: &PostJob,
    cancel: &CancelToken,
    on_progress: &mut dyn FnMut(DownloadProgress),
) -> JobOutcome {
    let Some(gallery_dl) = &post.gallery_dl else {
        return JobOutcome::Failed(DownloadError::YtDlp {
            error: PreviewError::ImagesNotSetUp,
        });
    };
    if let Err(error) = make_folders(job) {
        return JobOutcome::Failed(error);
    }
    if let Some(error) = pixiv_warning::refusal(&job.url) {
        return JobOutcome::Failed(DownloadError::YtDlp { error });
    }
    // Pixiv's sign-in, kept until the run ends, which deletes it.
    let config = pixiv_account::gallery_dl_config(&job.url, job.ffmpeg.as_deref());
    let args = arguments(job, post, config.as_ref());
    let command = ProgramCommand::new(gallery_dl).args(&args);

    // Appended to, so a resumed or retried download keeps the earlier runs' output above.
    let mut log = open_log(&job.log_path);
    write_log(
        &mut log,
        &format!(
            "=== {} gallery-dl {}",
            seconds_since_epoch(),
            args.join(" ")
        ),
    );
    on_progress(DownloadProgress::at(DownloadStage::Starting));

    let mut files = Vec::new();
    let mut errors = Vec::new();
    let result = process::run(&command, cancel, None, |line| {
        write_log(&mut log, line.text());
        match line {
            OutputLine::Stdout(text) => {
                if let Some(file) = saved_file(&text) {
                    files.push(file);
                    on_progress(progress(files.len(), post.files));
                }
            }
            OutputLine::Stderr(text) => {
                if errors.len() == KEPT_ERROR_LINES {
                    errors.remove(0);
                }
                errors.push(text);
            }
        }
    });

    pixiv_warning::note_download(&job.url, u32::try_from(files.len()).unwrap_or(u32::MAX));
    match result {
        Err(RunError::Cancelled) => JobOutcome::Cancelled,
        Err(RunError::Start { source, .. }) => JobOutcome::Failed(DownloadError::YtDlp {
            error: PreviewError::Tool {
                tool: ToolId::GalleryDl,
                error: start_error(source, gallery_dl),
            },
        }),
        Err(other) => JobOutcome::Failed(DownloadError::Unexpected {
            detail: other.to_string(),
        }),
        Ok(status) if !status.success() => {
            let lines: Vec<&str> = errors.iter().map(String::as_str).collect();
            JobOutcome::Failed(DownloadError::YtDlp {
                error: classify_download_failure(status.code(), &lines),
            })
        }
        Ok(_) if files.is_empty() => JobOutcome::Failed(DownloadError::NoFile),
        Ok(_) => JobOutcome::Finished {
            files,
            format: None,
            shared: Vec::new(),
        },
    }
}

/// gallery-dl's arguments for a post: its files straight into the item's folder under the names Settings gives,
/// partial files in the temporary folder, plain output with one path per line, then the link.
pub(crate) fn arguments(
    job: &JobSpec,
    post: &PostJob,
    config: Option<&GalleryDlConfig>,
) -> Vec<String> {
    let mut args: Vec<String> = [
        // A config file of the user's can't change where files go or what is written.
        "--config-ignore",
        // One line per file, without the progress a terminal gets, in UTF-8 whatever Windows' code page, written
        // as each file is saved.
        "-o",
        "output.mode=pipe",
        "-o",
        r#"output.stdout={"encoding": "utf-8", "line_buffering": true}"#,
    ]
    .iter()
    .map(|arg| (*arg).to_owned())
    .collect();
    args.extend([
        "-o".to_owned(),
        format!("downloader.part-directory={}", job.temp_folder.display()),
        // Exactly this folder, without gallery-dl's own subfolders.
        "--directory".to_owned(),
        job.folder.display().to_string(),
        "--filename".to_owned(),
        post.file_name.clone(),
    ]);
    if let Some(range) = &post.range {
        // Only the files picked, by their place in the post, which the preview listed in the same order.
        let numbers: Vec<String> = range.iter().map(u32::to_string).collect();
        args.extend(["--range".to_owned(), numbers.join(",")]);
    }
    if let Some(cookies) = &job.cookies {
        args.extend(["--cookies-from-browser".to_owned(), cookies.clone()]);
    }
    if let Some(caches) = &job.caches {
        args.extend(caches.gallery_dl_args());
    }
    if let Some(rate_limit) = &job.rate_limit {
        args.extend(["--limit-rate".to_owned(), rate_limit.clone()]);
    }
    if let Some(pause) = post.pause {
        // Before each file, and between the requests that find them.
        args.extend(["--sleep".to_owned(), pause.to_arg()]);
        args.extend(["--sleep-request".to_owned(), pause.to_arg()]);
    }
    if job.overwrite {
        args.push("--no-skip".to_owned());
    }
    if let Some(config) = config {
        args.extend(config.args());
    }
    // After "--", the link can't be taken for an option.
    args.extend(["--".to_owned(), job.url.clone()]);
    args
}

/// The file a line of gallery-dl's output names: one it saved, or one it already had ("# " and its path).
fn saved_file(line: &str) -> Option<PathBuf> {
    let path = line.strip_prefix(SKIPPED_PREFIX).unwrap_or(line).trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

fn progress(done: usize, expected: u32) -> DownloadProgress {
    DownloadProgress {
        // A post can hold more files than its preview counted.
        fraction: (expected > 0).then(|| (done as f64 / f64::from(expected)).min(1.0)),
        ..DownloadProgress::at(DownloadStage::Downloading)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::PauseRange;

    fn job(pause: Option<PauseRange>, overwrite: bool) -> (JobSpec, PostJob) {
        let job = JobSpec {
            id: 9,
            url: "https://www.instagram.com/p/DfPQchkG1tg/".to_owned(),
            preset: None,
            folder: PathBuf::from(r"D:\Pictures\Instagram"),
            temp_folder: PathBuf::from(r"D:\Pictures\Instagram\.tsundoku-9"),
            log_path: PathBuf::from(r"C:\data\logs\9.log"),
            overwrite,
            yt_dlp: None,
            ffmpeg: None,
            deno: None,
            caches: None,
            rate_limit: Some("2M".to_owned()),
            cookies: Some("firefox:C:\\Profiles\\main".to_owned()),
            post: None,
            files: None,
            files_pause: None,
            picked_files: None,
            shared_file_name: None,
            caught_links: Vec::new(),
            window_offer: None,
            unpack: None,
            unpack_only: None,
            password: None,
        };
        let post = PostJob {
            gallery_dl: Some(PathBuf::from("gallery-dl.exe")),
            pause,
            files: 3,
            range: None,
            file_name: "{username|'natgeo'} 2026-11-04 DfPQchkG1tg {num|filename}.{extension}"
                .to_owned(),
        };
        (job, post)
    }

    fn pair<'a>(args: &'a [String], option: &str) -> Option<&'a str> {
        args.windows(2)
            .find(|pair| pair[0] == option)
            .map(|pair| pair[1].as_str())
    }

    #[test]
    fn keeps_gallery_dls_cache_where_chosen() {
        let (mut job, post) = job(None, false);
        assert!(
            !arguments(&job, &post, None)
                .iter()
                .any(|arg| arg.starts_with("cache.file="))
        );
        job.caches = Some(crate::tools::ToolCacheFolder::new(PathBuf::from(
            r"C:\data\cache",
        )));
        let args = arguments(&job, &post, None);
        assert!(args.contains(&r"cache.file=C:\data\cache\gallery-dl\cache.sqlite3".to_owned()));
        assert_eq!(args[args.len() - 1], job.url);
    }

    #[test]
    fn saves_into_the_folder_and_ends_with_the_link() {
        let (job, post) = job(Some(PauseRange { min: 10, max: 20 }), false);
        let args = arguments(&job, &post, None);
        assert_eq!(args[0], "--config-ignore");
        assert_eq!(pair(&args, "--directory"), Some(r"D:\Pictures\Instagram"));
        assert_eq!(
            pair(&args, "--filename"),
            Some("{username|'natgeo'} 2026-11-04 DfPQchkG1tg {num|filename}.{extension}")
        );
        assert!(
            args.contains(
                &r"downloader.part-directory=D:\Pictures\Instagram\.tsundoku-9".to_owned()
            )
        );
        assert_eq!(
            pair(&args, "--cookies-from-browser"),
            Some("firefox:C:\\Profiles\\main")
        );
        assert_eq!(pair(&args, "--limit-rate"), Some("2M"));
        assert_eq!(pair(&args, "--sleep"), Some("10-20"));
        assert_eq!(pair(&args, "--sleep-request"), Some("10-20"));
        assert!(!args.contains(&"--no-skip".to_owned()));
        assert_eq!(
            &args[args.len() - 2..],
            ["--", "https://www.instagram.com/p/DfPQchkG1tg/"]
        );
    }

    #[test]
    fn downloads_only_the_files_picked() {
        let (job, mut post) = job(None, false);
        assert_eq!(pair(&arguments(&job, &post, None), "--range"), None);
        post.range = Some(vec![1, 3, 6]);
        let args = arguments(&job, &post, None);
        assert_eq!(pair(&args, "--range"), Some("1,3,6"));
        assert_eq!(
            &args[args.len() - 2..],
            ["--", "https://www.instagram.com/p/DfPQchkG1tg/"]
        );
    }

    #[test]
    fn pauses_only_when_asked_and_replaces_files_when_asked() {
        let (job, post) = job(None, true);
        let args = arguments(&job, &post, None);
        assert_eq!(pair(&args, "--sleep"), None);
        assert!(args.contains(&"--no-skip".to_owned()));
    }

    #[test]
    fn reads_saved_and_skipped_files() {
        assert_eq!(
            saved_file(r"D:\Pictures\Instagram\3780_1.jpg"),
            Some(PathBuf::from(r"D:\Pictures\Instagram\3780_1.jpg"))
        );
        assert_eq!(
            saved_file(r"# D:\Pictures\Instagram\3780_2.jpg"),
            Some(PathBuf::from(r"D:\Pictures\Instagram\3780_2.jpg"))
        );
        assert_eq!(saved_file("  "), None);
    }

    #[test]
    fn counts_progress_by_files() {
        assert_eq!(progress(1, 4).fraction, Some(0.25));
        assert_eq!(progress(5, 4).fraction, Some(1.0));
        assert_eq!(progress(1, 0).fraction, None);
    }
}
