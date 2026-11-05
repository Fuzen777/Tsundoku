//! Asking yt-dlp what a link is (`yt-dlp --dump-single-json`), and reading its answer into a `VideoInfo`, or a
//! `PreviewError` saying why it can't be downloaded. Tested against real answers saved by `Scripts\Capture-Fixtures.ps1`.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::cancel::CancelToken;
use crate::process::{self, OutputLine, ProgramCommand, RunError};
use crate::tools::{ToolError, start_error};

/// Some sites take a while to answer; a fetch still running after this long is given up.
const FETCH_TIMEOUT: Duration = Duration::from_secs(2 * 60);

/// What yt-dlp writes when YouTube extraction runs without a JavaScript runtime (Deno).
const NO_JS_RUNTIME_WARNING: &str = "No supported JavaScript runtime";

/// A single video, as yt-dlp describes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VideoInfo {
    /// yt-dlp's name for the site, in lowercase, as in its download archive ("youtube").
    pub site: String,
    /// The site as people know it ("youtube.com").
    pub site_name: String,
    /// The site's ID for the video. With `site`, it identifies the video whatever link led to it.
    pub video_id: String,
    /// The video's own page.
    pub url: String,
    pub title: String,
    pub uploader: Option<String>,
    pub duration_seconds: Option<f64>,
    /// As YYYY-MM-DD.
    pub upload_date: Option<String>,
    pub thumbnail_url: Option<String>,
    /// The tallest video format, or `None` for audio only.
    pub best_height: Option<u32>,
}

/// Why a link can't be previewed or downloaded. The UI turns each into a sentence; `detail` is yt-dlp's own error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum PreviewError {
    /// Not an http or https link.
    #[error("not a link")]
    InvalidLink,
    /// A playlist or a channel, refused until the playlist picker exists.
    #[error("a playlist or channel")]
    Playlist,
    /// Live now, or just ended and still being processed.
    #[error("a live stream")]
    LiveStream,
    /// A premiere or live stream that hasn't started.
    #[error("not started yet: {detail}")]
    Upcoming { detail: String },
    #[error("unsupported site")]
    UnsupportedSite,
    /// Private, removed, blocked in this country, or never existed.
    #[error("unavailable: {detail}")]
    Unavailable { detail: String },
    /// Needs a signed-in account, or an age check.
    #[error("sign-in required: {detail}")]
    SignInRequired { detail: String },
    #[error("network error: {detail}")]
    Network { detail: String },
    /// yt-dlp couldn't read the site's page, which an update of yt-dlp often fixes.
    #[error("site error: {detail}")]
    SiteError { detail: String },
    #[error("failed: {detail}")]
    Failed { detail: String },
    /// yt-dlp itself couldn't run.
    #[error("yt-dlp couldn't run: {error}")]
    Tool { error: ToolError },
    #[error("cancelled")]
    Cancelled,
}

/// The programs a fetch runs: yt-dlp, and Deno and ffmpeg when they're set up.
#[derive(Debug, Clone, Default)]
pub struct FetchTools<'a> {
    pub yt_dlp: Option<&'a Path>,
    pub deno: Option<&'a Path>,
    pub ffmpeg: Option<&'a Path>,
}

/// What a fetch found.
#[derive(Debug, Clone, PartialEq)]
pub struct Fetched {
    pub video: VideoInfo,
    /// yt-dlp ran without Deno, so YouTube may have offered fewer formats.
    pub missing_js_runtime: bool,
}

/// The link to give yt-dlp: trimmed, with "https://" added when it has none ("youtu.be/…"). Only http and https
/// links with a host are accepted, so pasted text, a search or a file path never reaches yt-dlp.
pub fn normalize_link(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.chars().any(char::is_whitespace) {
        return None;
    }
    let lowercase = text.to_ascii_lowercase();
    let link = if lowercase.starts_with("https://") || lowercase.starts_with("http://") {
        text.to_owned()
    } else if lowercase.contains("://") {
        return None;
    } else {
        format!("https://{text}")
    };
    let after_scheme = &link[link.find("://")? + 3..];
    let host = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    // Letters (any script), digits, dots, hyphens, a port's colon or an IPv6 address's brackets, with a dot inside.
    let is_host = host.contains('.')
        && !host.starts_with(['-', '.'])
        && host
            .chars()
            .all(|character| character.is_alphanumeric() || "-.:[]".contains(character));
    is_host.then_some(link)
}

/// Runs yt-dlp on the link and reads its answer. `link` must come from `normalize_link`.
pub fn fetch(
    tools: &FetchTools,
    link: &str,
    cancel: &CancelToken,
) -> Result<Fetched, PreviewError> {
    let Some(yt_dlp) = tools.yt_dlp else {
        return Err(PreviewError::Tool {
            error: ToolError::FileMissing {
                path: "yt-dlp".to_owned(),
            },
        });
    };
    let output =
        process::run_collecting_with_cancel(&command(tools, yt_dlp, link), cancel, FETCH_TIMEOUT)
            .map_err(|error| match error {
            RunError::Start { source, .. } => PreviewError::Tool {
                error: start_error(source, yt_dlp),
            },
            RunError::Cancelled => PreviewError::Cancelled,
            RunError::TimedOut => PreviewError::Network {
                detail: format!("no answer within {} seconds", FETCH_TIMEOUT.as_secs()),
            },
            RunError::Wait(source) => PreviewError::Failed {
                detail: source.to_string(),
            },
        })?;

    let stderr: Vec<&str> = output
        .lines
        .iter()
        .filter_map(|line| match line {
            OutputLine::Stderr(text) => Some(text.as_str()),
            OutputLine::Stdout(_) => None,
        })
        .collect();
    if !output.status.success() {
        return Err(classify_failure(&stderr));
    }
    let json: String = output.stdout().collect();
    Ok(Fetched {
        video: parse_info(&json)?,
        missing_js_runtime: stderr
            .iter()
            .any(|line| line.contains(NO_JS_RUNTIME_WARNING)),
    })
}

fn command(tools: &FetchTools, yt_dlp: &Path, link: &str) -> ProgramCommand {
    let mut command = ProgramCommand::new(yt_dlp).args([
        "--ignore-config",
        // A watch?v=…&list=… link means the video, not its playlist.
        "--no-playlist",
        // A playlist or channel is refused, so its videos needn't be read one by one.
        "--flat-playlist",
        "--dump-single-json",
        // yt-dlp's messages in UTF-8 rather than the Windows code page.
        "--encoding",
        "utf-8",
    ]);
    command = match tools.deno {
        Some(deno) => command
            .arg("--js-runtimes")
            .arg(format!("deno:{}", deno.display())),
        // Without Deno set up, none at all: a copy found on PATH would make the "Deno missing" warning wrong.
        None => command.arg("--no-js-runtimes"),
    };
    if let Some(ffmpeg) = tools.ffmpeg {
        command = command.arg("--ffmpeg-location").arg(ffmpeg);
    }
    // After "--", a link can't be taken for an option, whatever it starts with.
    command.arg("--").arg(link)
}

/// The fields of yt-dlp's answer that Tsundoku reads; the rest is skipped.
#[derive(Deserialize)]
struct InfoJson {
    #[serde(rename = "_type")]
    kind: Option<String>,
    id: Option<String>,
    title: Option<String>,
    extractor: Option<String>,
    extractor_key: Option<String>,
    webpage_url: Option<String>,
    webpage_url_domain: Option<String>,
    uploader: Option<String>,
    channel: Option<String>,
    duration: Option<f64>,
    upload_date: Option<String>,
    thumbnail: Option<String>,
    live_status: Option<String>,
    height: Option<u32>,
    vcodec: Option<String>,
    #[serde(default)]
    formats: Vec<FormatJson>,
}

#[derive(Deserialize)]
struct FormatJson {
    height: Option<u32>,
    vcodec: Option<String>,
}

pub(crate) fn parse_info(json: &str) -> Result<VideoInfo, PreviewError> {
    let info: InfoJson = serde_json::from_str(json).map_err(|error| PreviewError::Failed {
        detail: format!("yt-dlp's answer can't be read: {error}"),
    })?;
    if matches!(info.kind.as_deref(), Some("playlist" | "multi_video")) {
        return Err(PreviewError::Playlist);
    }
    match info.live_status.as_deref() {
        Some("is_live" | "post_live") => return Err(PreviewError::LiveStream),
        Some("is_upcoming") => {
            return Err(PreviewError::Upcoming {
                detail: String::new(),
            });
        }
        _ => {}
    }
    let (Some(video_id), Some(extractor_key)) = (info.id, info.extractor_key) else {
        return Err(PreviewError::Failed {
            detail: "yt-dlp's answer has no video ID".to_owned(),
        });
    };

    // A format is video when it has a height and isn't marked audio-only; some sites (archive.org) give no codec.
    let is_video =
        |height: Option<u32>, vcodec: Option<&str>| height.is_some() && vcodec != Some("none");
    let best_height = if info.formats.is_empty() {
        info.height
            .filter(|_| is_video(info.height, info.vcodec.as_deref()))
    } else {
        info.formats
            .iter()
            .filter(|format| is_video(format.height, format.vcodec.as_deref()))
            .filter_map(|format| format.height)
            .max()
    };

    Ok(VideoInfo {
        site: extractor_key.to_lowercase(),
        site_name: info
            .webpage_url_domain
            .or(info.extractor)
            .unwrap_or_else(|| extractor_key.clone()),
        url: info.webpage_url.unwrap_or_default(),
        title: info.title.unwrap_or_else(|| video_id.clone()),
        video_id,
        uploader: info.uploader.or(info.channel),
        duration_seconds: info.duration,
        upload_date: info.upload_date.as_deref().and_then(iso_date),
        thumbnail_url: info.thumbnail,
        best_height,
    })
}

/// "20050424" to "2005-04-24".
fn iso_date(compact: &str) -> Option<String> {
    (compact.len() == 8 && compact.chars().all(|character| character.is_ascii_digit()))
        .then(|| format!("{}-{}-{}", &compact[..4], &compact[4..6], &compact[6..]))
}

/// Turns yt-dlp's error (its last `ERROR:` line) into the reason the link can't be downloaded.
pub(crate) fn classify_failure(stderr: &[&str]) -> PreviewError {
    let Some(line) = stderr
        .iter()
        .rev()
        .find(|line| line.starts_with("ERROR:"))
        .or(stderr.last())
    else {
        return PreviewError::Failed {
            detail: "yt-dlp stopped without saying why".to_owned(),
        };
    };
    let detail = line
        .strip_prefix("ERROR:")
        .unwrap_or(line)
        .trim()
        .to_owned();
    let text = detail.to_lowercase();
    let mentions = |phrases: &[&str]| phrases.iter().any(|phrase| text.contains(phrase));

    if mentions(&["is not a valid url"]) {
        PreviewError::InvalidLink
    } else if mentions(&["unsupported url"]) {
        PreviewError::UnsupportedSite
    } else if mentions(&[
        "will begin in",
        "premieres in",
        "live event will begin",
        "is_upcoming",
    ]) {
        PreviewError::Upcoming { detail }
    } else if mentions(&[
        "sign in",
        "logged-in",
        "log in",
        "login required",
        "confirm your age",
        "age-restricted",
        "members-only",
        "join this channel",
        "private video",
    ]) {
        PreviewError::SignInRequired { detail }
    } else if mentions(&[
        "unavailable",
        "not available",
        "in your country",
        "no longer available",
        "has been removed",
        "does not exist",
        "http error 404",
        "http error 410",
    ]) {
        PreviewError::Unavailable { detail }
    } else if mentions(&[
        "unable to download webpage",
        "getaddrinfo",
        "failed to resolve",
        "timed out",
        "connection",
        "transporterror",
        "ssl",
    ]) {
        PreviewError::Network { detail }
    } else if detail.starts_with('[') {
        // "[youtube] abc: …": the site's extractor failed in a way yt-dlp didn't expect.
        PreviewError::SiteError { detail }
    } else {
        PreviewError::Failed { detail }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = format!(
            "{}/tests/fixtures/metadata/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
    }

    fn failure(name: &str) -> PreviewError {
        let text = fixture(&format!("{name}.txt"));
        classify_failure(&text.lines().collect::<Vec<_>>())
    }

    #[test]
    fn reads_a_youtube_video() {
        let video = parse_info(&fixture("youtube-watch.json")).unwrap();
        assert_eq!(video.site, "youtube");
        assert_eq!(video.site_name, "youtube.com");
        assert_eq!(video.video_id, "jNQXAC9IVRw");
        assert_eq!(video.url, "https://www.youtube.com/watch?v=jNQXAC9IVRw");
        assert_eq!(video.title, "Me at the zoo");
        assert_eq!(video.uploader.as_deref(), Some("jawed"));
        assert_eq!(video.duration_seconds, Some(19.0));
        assert_eq!(video.upload_date.as_deref(), Some("2005-04-24"));
        assert!(
            video
                .thumbnail_url
                .is_some_and(|url| url.starts_with("https://i.ytimg.com/"))
        );
        assert_eq!(video.best_height, Some(240));
    }

    /// The duplicate check relies on every link to a video giving the same site and ID.
    #[test]
    fn every_link_to_a_video_gives_the_same_site_and_id() {
        for name in [
            "youtube-short-link",
            "youtube-shorts",
            "youtube-watch-in-playlist",
        ] {
            let video = parse_info(&fixture(&format!("{name}.json"))).unwrap();
            assert_eq!(
                (video.site.as_str(), video.video_id.as_str()),
                ("youtube", "jNQXAC9IVRw"),
                "{name}"
            );
        }
    }

    #[test]
    fn reads_other_sites() {
        let archive = parse_info(&fixture("archive-org.json")).unwrap();
        assert_eq!(archive.site, "archiveorg");
        assert_eq!(archive.site_name, "archive.org");
        assert_eq!(
            archive.best_height,
            Some(304),
            "formats without a codec still count as video"
        );
        assert_eq!(archive.duration_seconds, None);

        let audio = parse_info(&fixture("soundcloud.json")).unwrap();
        assert_eq!(audio.site, "soundcloud");
        assert_eq!(audio.best_height, None, "audio only");
        assert_eq!(audio.duration_seconds, Some(143.206));
    }

    #[test]
    fn refuses_playlists_channels_and_live_streams() {
        assert_eq!(
            parse_info(&fixture("youtube-playlist.json")),
            Err(PreviewError::Playlist)
        );
        assert_eq!(
            parse_info(&fixture("youtube-channel.json")),
            Err(PreviewError::Playlist)
        );
        assert_eq!(
            parse_info(&fixture("youtube-live.json")),
            Err(PreviewError::LiveStream)
        );
    }

    #[test]
    fn rejects_an_answer_it_cant_read() {
        assert!(matches!(
            parse_info("null"),
            Err(PreviewError::Failed { .. })
        ));
        assert!(matches!(parse_info("{}"), Err(PreviewError::Failed { .. })));
    }

    #[test]
    fn classifies_real_failures() {
        assert!(matches!(
            failure("unavailable"),
            PreviewError::Unavailable { .. }
        ));
        assert!(matches!(
            failure("not-found"),
            PreviewError::Unavailable { .. }
        ));
        assert!(matches!(
            failure("sign-in"),
            PreviewError::SignInRequired { .. }
        ));
        assert_eq!(failure("unsupported"), PreviewError::UnsupportedSite);
        assert!(matches!(
            failure("no-such-host"),
            PreviewError::Network { .. }
        ));
    }

    #[test]
    fn keeps_yt_dlps_error_as_the_detail() {
        assert_eq!(
            failure("unavailable"),
            PreviewError::Unavailable {
                detail: "[youtube] aaaaaaaaaaa: This video is unavailable".to_owned()
            }
        );
    }

    #[test]
    fn classifies_known_messages() {
        let classify = |line: &str| classify_failure(&[line]);
        assert!(matches!(
            classify(
                "ERROR: [youtube] abc: Sign in to confirm your age. This video may be inappropriate for some users."
            ),
            PreviewError::SignInRequired { .. }
        ));
        assert!(matches!(
            classify("ERROR: [youtube] abc: Sign in to confirm you’re not a bot."),
            PreviewError::SignInRequired { .. }
        ));
        assert!(matches!(
            classify(
                "ERROR: [youtube] abc: Join this channel to get access to members-only content like this video."
            ),
            PreviewError::SignInRequired { .. }
        ));
        assert!(matches!(
            classify("ERROR: [youtube] abc: This live event will begin in 3 hours."),
            PreviewError::Upcoming { .. }
        ));
        assert!(matches!(
            classify(
                "ERROR: [youtube] abc: The uploader has not made this video available in your country"
            ),
            PreviewError::Unavailable { .. }
        ));
        assert_eq!(
            classify("ERROR: [generic] 'abc' is not a valid URL"),
            PreviewError::InvalidLink
        );
        assert!(matches!(
            classify("ERROR: [youtube] abc: Failed to extract any player response"),
            PreviewError::SiteError { .. }
        ));
        assert!(matches!(
            classify("Traceback (most recent call last):"),
            PreviewError::Failed { .. }
        ));
        assert!(matches!(classify_failure(&[]), PreviewError::Failed { .. }));
    }

    #[test]
    fn normalizes_links() {
        assert_eq!(
            normalize_link("  https://www.youtube.com/watch?v=jNQXAC9IVRw \n").as_deref(),
            Some("https://www.youtube.com/watch?v=jNQXAC9IVRw")
        );
        assert_eq!(
            normalize_link("youtu.be/jNQXAC9IVRw").as_deref(),
            Some("https://youtu.be/jNQXAC9IVRw")
        );
        assert_eq!(
            normalize_link("HTTP://Example.com/a").as_deref(),
            Some("HTTP://Example.com/a")
        );
        assert_eq!(normalize_link(""), None);
        assert_eq!(normalize_link("not a link"), None);
        assert_eq!(normalize_link("hello"), None);
        assert_eq!(normalize_link("--exec calc"), None);
        assert_eq!(normalize_link("-x"), None);
        assert_eq!(normalize_link("ytsearch:cats"), None);
        assert_eq!(normalize_link("file:///C:/Windows/notepad.exe"), None);
        assert_eq!(normalize_link(r"C:\Videos\clip.mp4"), None);
    }

    #[test]
    fn puts_the_link_after_the_options() {
        let tools = FetchTools {
            yt_dlp: Some(Path::new("yt-dlp.exe")),
            deno: None,
            ffmpeg: Some(Path::new(r"C:\tools\ffmpeg.exe")),
        };
        let command = command(&tools, Path::new("yt-dlp.exe"), "https://example.com/v");
        let args: Vec<_> = command
            .arguments()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(&args[args.len() - 2..], ["--", "https://example.com/v"]);
        assert!(args.contains(&"--no-js-runtimes".to_owned()));
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--ffmpeg-location", r"C:\tools\ffmpeg.exe"])
        );
    }
}
