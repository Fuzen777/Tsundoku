//! Running one download: yt-dlp with the preset's arguments, its progress read as it goes and its output kept in
//! the item's log. A trait, so the queue's tests can use a runner that doesn't download.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use super::{DownloadError, DownloadProgress, DownloadStage, progress};
use crate::cancel::CancelToken;
use crate::metadata::{PreviewError, classify_failure};
use crate::presets::Preset;
use crate::process::{self, OutputLine, ProgramCommand, RunError};
use crate::settings::CookiesBrowser;
use crate::tools::{ToolError, start_error};

/// The most recent lines of yt-dlp's error output kept to explain a failure.
const KEPT_ERROR_LINES: usize = 50;

/// Everything one run of a download needs.
#[derive(Debug, Clone)]
pub struct JobSpec {
    pub id: i64,
    pub url: String,
    pub preset: Preset,
    /// Where the file goes.
    pub folder: PathBuf,
    /// Where yt-dlp works: in the destination folder, so finishing is a rename on the same drive. Kept by Stop so
    /// Resume continues the partial files; deleted by Cancel and on success.
    pub temp_folder: PathBuf,
    pub log_path: PathBuf,
    /// Replace a file of the same name (Download Again with the same preset).
    pub overwrite: bool,
    pub yt_dlp: Option<PathBuf>,
    pub ffmpeg: Option<PathBuf>,
    pub deno: Option<PathBuf>,
    /// yt-dlp's `--limit-rate`, such as "2M".
    pub rate_limit: Option<String>,
    pub cookies_browser: Option<CookiesBrowser>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JobOutcome {
    Finished {
        file_path: PathBuf,
        /// Resolution and codecs, such as "1080p · avc1 + mp4a", or the audio's format.
        format: Option<String>,
    },
    Cancelled,
    Failed(DownloadError),
}

pub trait JobRunner: Send + Sync {
    /// Runs the download to its end, or until `cancel` is set.
    fn run(
        &self,
        job: &JobSpec,
        cancel: &CancelToken,
        on_progress: &mut dyn FnMut(DownloadProgress),
    ) -> JobOutcome;
}

/// Downloads with yt-dlp.
pub struct YtDlpRunner;

impl JobRunner for YtDlpRunner {
    fn run(
        &self,
        job: &JobSpec,
        cancel: &CancelToken,
        on_progress: &mut dyn FnMut(DownloadProgress),
    ) -> JobOutcome {
        let Some(yt_dlp) = &job.yt_dlp else {
            return JobOutcome::Failed(DownloadError::YtDlp {
                error: PreviewError::Tool {
                    error: ToolError::FileMissing {
                        path: "yt-dlp".to_owned(),
                    },
                },
            });
        };
        for folder in [&job.folder, &job.temp_folder] {
            if let Err(error) = fs::create_dir_all(folder) {
                return JobOutcome::Failed(DownloadError::Folder {
                    path: folder.display().to_string(),
                    detail: error.to_string(),
                });
            }
        }
        let result_file = job.temp_folder.join("tsundoku-result.json");
        let _ = fs::remove_file(&result_file);
        let args = match arguments(job, &result_file) {
            Ok(args) => args,
            Err(error) => return JobOutcome::Failed(DownloadError::Preset { error }),
        };
        let command = ProgramCommand::new(yt_dlp).args(&args);

        // Appended to, so a resumed or retried download keeps the earlier runs' output above.
        let mut log = open_log(&job.log_path);
        write_log(
            &mut log,
            &format!("=== {} yt-dlp {}", seconds_since_epoch(), args.join(" ")),
        );
        on_progress(DownloadProgress::at(DownloadStage::Starting));

        let mut errors = Vec::new();
        let result = process::run(&command, cancel, None, |line| {
            if let Some(progress) = progress::parse_line(line.text()) {
                on_progress(progress);
                return;
            }
            write_log(&mut log, line.text());
            if let OutputLine::Stderr(text) = line {
                if errors.len() == KEPT_ERROR_LINES {
                    errors.remove(0);
                }
                errors.push(text);
            }
        });

        match result {
            Err(RunError::Cancelled) => JobOutcome::Cancelled,
            Err(RunError::Start { source, .. }) => JobOutcome::Failed(DownloadError::YtDlp {
                error: PreviewError::Tool {
                    error: start_error(source, yt_dlp),
                },
            }),
            Err(other) => JobOutcome::Failed(DownloadError::Unexpected {
                detail: other.to_string(),
            }),
            Ok(status) if !status.success() => {
                let lines: Vec<&str> = errors.iter().map(String::as_str).collect();
                JobOutcome::Failed(DownloadError::YtDlp {
                    error: classify_failure(&lines),
                })
            }
            Ok(_) => read_result(&result_file),
        }
    }
}

/// yt-dlp's arguments for a download: Tsundoku's own (progress, paths, tools), then the preset's, then the link.
pub(crate) fn arguments(
    job: &JobSpec,
    result_file: &Path,
) -> Result<Vec<String>, crate::presets::PresetError> {
    let [download_template, postprocess_template] = progress::templates();
    let mut args: Vec<String> = [
        "--ignore-config",
        "--no-playlist",
        "--encoding",
        "utf-8",
        "--newline",
        "--progress",
        "--progress-delta",
        "0.5",
        "--progress-template",
        &download_template,
        "--progress-template",
        &postprocess_template,
        // The final path, once the file is in place. Unlike --print, this keeps yt-dlp's normal output for the log.
        "--print-to-file",
        "after_move:%(.{filepath,height,vcodec,acodec,ext})j",
    ]
    .iter()
    .map(|arg| (*arg).to_owned())
    .collect();
    args.push(result_file.display().to_string());
    args.extend(["-P".to_owned(), format!("home:{}", job.folder.display())]);
    args.extend([
        "-P".to_owned(),
        format!("temp:{}", job.temp_folder.display()),
    ]);
    if let Some(ffmpeg) = &job.ffmpeg {
        args.extend(["--ffmpeg-location".to_owned(), ffmpeg.display().to_string()]);
    }
    match &job.deno {
        Some(deno) => args.extend([
            "--js-runtimes".to_owned(),
            format!("deno:{}", deno.display()),
        ]),
        None => args.push("--no-js-runtimes".to_owned()),
    }
    if let Some(browser) = job.cookies_browser {
        args.extend([
            "--cookies-from-browser".to_owned(),
            browser.yt_dlp_name().to_owned(),
        ]);
    }
    if let Some(rate_limit) = &job.rate_limit {
        args.extend(["--limit-rate".to_owned(), rate_limit.clone()]);
    }
    if job.overwrite {
        args.push("--force-overwrites".to_owned());
    }
    args.extend(job.preset.to_args()?);
    // After "--", the link can't be taken for an option.
    args.extend(["--".to_owned(), job.url.clone()]);
    Ok(args)
}

#[derive(Deserialize)]
struct ResultJson {
    filepath: Option<String>,
    height: Option<u32>,
    vcodec: Option<String>,
    acodec: Option<String>,
    ext: Option<String>,
}

fn read_result(result_file: &Path) -> JobOutcome {
    let result: Option<ResultJson> = fs::read_to_string(result_file)
        .ok()
        .and_then(|text| serde_json::from_str(text.lines().last()?).ok());
    match result {
        Some(ResultJson {
            filepath: Some(file_path),
            height,
            vcodec,
            acodec,
            ext,
        }) => JobOutcome::Finished {
            file_path: PathBuf::from(file_path),
            format: format_label(height, vcodec.as_deref(), acodec.as_deref(), ext.as_deref()),
        },
        _ => JobOutcome::Failed(DownloadError::NoFile),
    }
}

/// "1080p · avc1 + mp4a" for a video. For audio, its file type ("mp3"): after a conversion, yt-dlp still reports
/// the codec it converted from.
pub(crate) fn format_label(
    height: Option<u32>,
    vcodec: Option<&str>,
    acodec: Option<&str>,
    ext: Option<&str>,
) -> Option<String> {
    let known = |codec: Option<&str>| {
        codec
            .filter(|codec| !matches!(*codec, "none" | "NA" | ""))
            .map(|codec| codec.split('.').next().unwrap_or(codec).to_owned())
    };
    match known(vcodec) {
        Some(video) => {
            let codecs = match known(acodec) {
                Some(audio) => format!("{video} + {audio}"),
                None => video,
            };
            Some(match height {
                Some(height) => format!("{height}p · {codecs}"),
                None => codecs,
            })
        }
        None => ext.map(str::to_owned),
    }
}

fn open_log(path: &Path) -> Option<File> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder).ok()?;
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}

/// A log that can't be written is skipped: the download matters more than its log.
fn write_log(log: &mut Option<File>, line: &str) {
    if let Some(file) = log
        && writeln!(file, "{line}").is_err()
    {
        *log = None;
    }
}

fn seconds_since_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::built_in_presets;

    fn job(preset_id: &str, overwrite: bool) -> JobSpec {
        JobSpec {
            id: 7,
            url: "https://www.youtube.com/watch?v=jNQXAC9IVRw".to_owned(),
            preset: built_in_presets()
                .into_iter()
                .find(|preset| preset.id == preset_id)
                .unwrap(),
            folder: PathBuf::from(r"D:\Videos"),
            temp_folder: PathBuf::from(r"D:\Videos\.tsundoku-7"),
            log_path: PathBuf::from(r"C:\data\logs\7.log"),
            overwrite,
            yt_dlp: Some(PathBuf::from("yt-dlp.exe")),
            ffmpeg: Some(PathBuf::from(r"C:\tools\ffmpeg\ffmpeg.exe")),
            deno: None,
            rate_limit: None,
            cookies_browser: None,
        }
    }

    #[test]
    fn puts_tsundokus_arguments_first_and_the_link_last() {
        let args = arguments(
            &job("best-video", false),
            Path::new(r"D:\Videos\.tsundoku-7\result.json"),
        )
        .unwrap();
        assert_eq!(&args[..2], ["--ignore-config", "--no-playlist"]);
        assert!(
            args.windows(2)
                .any(|pair| pair == ["-P", r"home:D:\Videos"])
        );
        assert!(
            args.windows(2)
                .any(|pair| pair == ["-P", r"temp:D:\Videos\.tsundoku-7"])
        );
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--ffmpeg-location", r"C:\tools\ffmpeg\ffmpeg.exe"])
        );
        assert!(args.contains(&"--no-js-runtimes".to_owned()));
        assert!(!args.contains(&"--force-overwrites".to_owned()));
        let preset_start = args.iter().position(|arg| arg == "-f").unwrap();
        let print = args
            .iter()
            .position(|arg| arg == "--print-to-file")
            .unwrap();
        assert!(
            print < preset_start,
            "the preset comes after Tsundoku's own arguments, so it can override them"
        );
        assert_eq!(
            &args[args.len() - 2..],
            ["--", "https://www.youtube.com/watch?v=jNQXAC9IVRw"]
        );
    }

    #[test]
    fn adds_the_cookies_browser_and_speed_limit_before_the_preset() {
        let mut job = job("best-video", false);
        job.cookies_browser = Some(CookiesBrowser::Firefox);
        job.rate_limit = Some("2M".to_owned());
        let args = arguments(&job, Path::new("result.json")).unwrap();
        let cookies = args
            .iter()
            .position(|arg| arg == "--cookies-from-browser")
            .unwrap();
        let limit = args.iter().position(|arg| arg == "--limit-rate").unwrap();
        let preset = args.iter().position(|arg| arg == "-f").unwrap();
        assert_eq!(
            (args[cookies + 1].as_str(), args[limit + 1].as_str()),
            ("firefox", "2M")
        );
        assert!(
            cookies < preset && limit < preset,
            "so a preset's extra arguments can change them"
        );
    }

    #[test]
    fn overwrites_when_asked() {
        let args = arguments(&job("audio-mp3", true), Path::new("result.json")).unwrap();
        assert!(args.contains(&"--force-overwrites".to_owned()));
    }

    #[test]
    fn labels_formats() {
        let av1 = Some("av01.0.00M.08.0.110.05.01.06.0");
        assert_eq!(
            format_label(Some(240), av1, Some("opus"), Some("mkv")).as_deref(),
            Some("240p · av01 + opus")
        );
        assert_eq!(
            format_label(
                Some(1080),
                Some("avc1.640028"),
                Some("mp4a.40.2"),
                Some("mp4")
            )
            .as_deref(),
            Some("1080p · avc1 + mp4a")
        );
        assert_eq!(
            format_label(None, Some("none"), Some("opus"), Some("mp3")).as_deref(),
            Some("mp3")
        );
        assert_eq!(
            format_label(Some(304), None, None, Some("mp4")).as_deref(),
            Some("mp4")
        );
    }

    #[test]
    fn reads_the_final_path() {
        let folder = tempfile::tempdir().unwrap();
        let result_file = folder.path().join("result.json");
        fs::write(
            &result_file,
            r#"{"filepath": "D:\\Videos\\Me at the zoo [jNQXAC9IVRw].mkv", "height": 240, "vcodec": "av01.0", "acodec": "opus", "ext": "mkv"}"#,
        )
        .unwrap();
        assert_eq!(
            read_result(&result_file),
            JobOutcome::Finished {
                file_path: PathBuf::from(r"D:\Videos\Me at the zoo [jNQXAC9IVRw].mkv"),
                format: Some("240p · av01 + opus".to_owned()),
            }
        );
        assert_eq!(
            read_result(&folder.path().join("missing.json")),
            JobOutcome::Failed(DownloadError::NoFile)
        );
    }
}
