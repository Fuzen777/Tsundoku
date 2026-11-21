//! Asking yt-dlp what a link is (`yt-dlp --dump-single-json`), and reading its answer into a `VideoInfo`, or a
//! `PreviewError` saying why it can't be downloaded. Tested against real answers saved by `Scripts\Capture-Fixtures.ps1`.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::cancel::CancelToken;
use crate::passwords::{Password, VIDEO_PASSWORD_OPTION};
use crate::process::{self, OutputLine, ProgramCommand, RunError};
use crate::room::{self, FormatSize, FormatSizes};
use crate::tools::{ToolCacheFolder, ToolError, ToolId, start_error};

/// Some sites take a while to answer; a fetch still running after this long is given up.
pub(crate) const FETCH_TIMEOUT: Duration = Duration::from_secs(2 * 60);

/// What yt-dlp writes when YouTube extraction runs without a JavaScript runtime (Deno).
const NO_JS_RUNTIME_WARNING: &str = "No supported JavaScript runtime";

/// A single video, as yt-dlp describes it, or a post of images and videos, as gallery-dl describes it (see `post`).
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
    /// The channel's or account's name as the site shows it ("National Geographic").
    pub uploader: Option<String>,
    /// The handle behind it, when the site has one ("natgeo"): `{account}` in a rule's folder, and the Uploader
    /// condition matches it too. `None` when the site gives none, or only a number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub account: Option<String>,
    pub duration_seconds: Option<f64>,
    /// As YYYY-MM-DD.
    pub upload_date: Option<String>,
    pub thumbnail_url: Option<String>,
    /// The tallest video format, or `None` for audio only or a post.
    pub best_height: Option<u32>,
    /// What a post holds, when the link is a post of images and videos (from gallery-dl). `None` for a video.
    #[serde(default)]
    pub post: Option<PostContents>,
    /// What a file host's link holds (`hosts`): one file, or a shared folder's. `None` for a video or a post.
    #[serde(default)]
    pub files: Option<FileContents>,
    /// A post's files, in the order the site gives them, and which of them its download takes. Empty for a video, a
    /// file host's link, or a post read before Tsundoku listed them, whose download takes every file.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post_files: Vec<PostFile>,
    /// A file host's files, when its link holds more than one (a shared folder), in the order the host gives them, and
    /// which of them its download takes. Empty for one file, a video, a post, or a link read before Tsundoku listed
    /// them, whose download takes every file.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shared_files: Vec<SharedFile>,
    /// The playlist, channel or account it was picked from (`listing`), for `{playlist}` in a rule's folder. `None` for
    /// a link added on its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub playlist: Option<String>,
}

impl VideoInfo {
    /// The numbers of the post's files its download takes, when it leaves some out (`PostFile::picked`); `None` when
    /// it takes them all, or isn't a post.
    pub fn picked_post_files(&self) -> Option<Vec<u32>> {
        let picked: Vec<u32> = self
            .post_files
            .iter()
            .filter(|file| file.picked)
            .map(|file| file.number)
            .collect();
        (picked.len() < self.post_files.len()).then_some(picked)
    }

    /// How many files a download of this post makes: those picked, or all it holds. `None` for anything but a post.
    pub fn post_file_count(&self) -> Option<u32> {
        let post = self.post?;
        Some(match self.picked_post_files() {
            Some(picked) => u32::try_from(picked.len()).unwrap_or(u32::MAX),
            None => post.images + post.videos + post.others,
        })
    }

    /// The paths of the shared folder's files its download takes, when it leaves some out (`SharedFile::picked`);
    /// `None` when it takes them all, or the link lists none.
    pub fn picked_shared_files(&self) -> Option<Vec<String>> {
        let picked: Vec<String> = self
            .shared_files
            .iter()
            .filter(|file| file.picked)
            .map(|file| file.path.clone())
            .collect();
        (picked.len() < self.shared_files.len()).then_some(picked)
    }

    /// What a file host's download takes: the count and size of the files picked, or the whole share (`files`) when
    /// none is left out. `None` for anything but a file host's link.
    pub fn picked_contents(&self) -> Option<FileContents> {
        let files = self.files?;
        if self.shared_files.iter().all(|file| file.picked) {
            return Some(files);
        }
        let picked = self.shared_files.iter().filter(|file| file.picked);
        Some(FileContents {
            count: u32::try_from(picked.clone().count()).unwrap_or(u32::MAX),
            bytes: picked.map(|file| file.size).sum(),
        })
    }

    /// Takes the pictures a later read of the same item has and this one lacks: its own, and those of its post's
    /// files, each by its number. Whether it took any. For an item kept from before its pictures could be read, such
    /// as an X video listed before 0.2.2.
    pub fn take_missing_pictures(&mut self, read: &Self) -> bool {
        let mut took_any = false;
        if self.thumbnail_url.is_none() && read.thumbnail_url.is_some() {
            self.thumbnail_url.clone_from(&read.thumbnail_url);
            took_any = true;
        }
        for file in &mut self.post_files {
            if file.thumbnail_url.is_some() {
                continue;
            }
            let picture = read
                .post_files
                .iter()
                .find(|other| other.number == file.number && other.video == file.video)
                .and_then(|other| other.thumbnail_url.as_ref());
            if let Some(picture) = picture {
                file.thumbnail_url = Some(picture.clone());
                took_any = true;
            }
        }
        took_any
    }
}

/// The files of a post, such as an Instagram carousel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PostContents {
    pub images: u32,
    pub videos: u32,
    /// Files that are neither, such as a .zip on Pixeldrain. Missing from posts read before 0.1.106.
    #[serde(default)]
    pub others: u32,
}

/// One file of a post, as the strip of the preview card and of a pending link shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PostFile {
    /// Its place in the post, from 1, in the order the site gives the files: what gallery-dl's `--range` picks it by.
    pub number: u32,
    /// A video rather than an image.
    pub video: bool,
    /// A small picture of it: the image itself, or a video's cover when the site gives one (Instagram and X do).
    pub thumbnail_url: Option<String>,
    /// Whether the download takes it. Every file does to start with; a strip unticks the ones not wanted.
    #[serde(default = "picked_by_default")]
    pub picked: bool,
    /// Its file's name with its extension, when the site gives one: what the list shows for a post of files that
    /// aren't all pictures (a folder on Gofile or Pixeldrain). `None` for a post read before 0.1.133.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    /// Its size in bytes, when the site gives it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub size: Option<u64>,
}

fn picked_by_default() -> bool {
    true
}

/// One file of a shared folder, as the list of the preview card and of a pending link shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SharedFile {
    /// Where it goes in the download's folder: the shared folder's own folder and subfolders, then its name, joined
    /// with "/" (`hosts::shared_path`). What the download picks it by, and the history remembers it by.
    pub path: String,
    #[ts(type = "number")]
    pub size: u64,
    /// Whether the download takes it. Every file does to start with; a list unticks the ones not wanted.
    #[serde(default = "picked_by_default")]
    pub picked: bool,
}

/// The files behind a file host's link, such as a MEGA folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileContents {
    pub count: u32,
    /// Their size together.
    #[ts(type = "number")]
    pub bytes: u64,
}

/// Why a link can't be previewed or downloaded. The UI turns each into a sentence; `detail` is yt-dlp's own error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum PreviewError {
    /// Not an http or https link.
    #[error("not a link")]
    InvalidLink,
    /// A playlist, channel or account, which the preview card lists (`listing`) but a pending link can't be.
    #[error("a playlist, channel or account")]
    Playlist,
    /// A page of many videos or posts that can't be listed: an account on an image site other than Instagram, or a
    /// listing whose items have no link of their own. `site` is its name ("X") or its domain.
    #[error("{site} page that can't be listed")]
    NotListable { site: String },
    /// Live now, or just ended and still being processed.
    #[error("a live stream")]
    LiveStream,
    /// A premiere or live stream that hasn't started.
    #[error("not started yet: {detail}")]
    Upcoming { detail: String },
    #[error("unsupported site")]
    UnsupportedSite,
    /// yt-dlp found no video at the link, which may be a post of images.
    #[error("no video: {detail}")]
    NoVideo { detail: String },
    /// Private, removed, blocked in this country, or never existed.
    #[error("unavailable: {detail}")]
    Unavailable { detail: String },
    /// Needs a signed-in account, or an age check. The preview adds which site and whether a browser is signed in
    /// to it, for the hint on the card.
    #[error("sign-in required: {detail}")]
    SignInRequired {
        detail: String,
        /// The site's name, when it's one of `sites::SIGN_IN_SITES`.
        #[serde(default)]
        site: Option<String>,
        #[serde(default)]
        cookies: SignInCookies,
    },
    /// A file host's free downloads for this connection are used up for now, such as MEGA's transfer quota.
    #[error("{site}'s download limit is reached: {detail}")]
    LimitReached { site: String, detail: String },
    /// A file host makes free downloads wait between each other, and says for how long (1fichier's "You must wait
    /// 240 minutes"): the queue holds the download until then.
    #[error("{site} makes free downloads wait {seconds} seconds")]
    WaitBetweenDownloads {
        site: String,
        #[ts(type = "number")]
        seconds: u64,
    },
    /// A mirror page (MultiUp's, mirrored.to's) whose mirrors are all on sites Tsundoku doesn't read: `mirrors`
    /// names them.
    #[error("{site} page with no mirror Tsundoku reads")]
    NoSupportedMirror { site: String, mirrors: Vec<String> },
    /// A short link (`short_links`) whose site answers with a page (a preview, a warning) instead of leading to its
    /// address. Tsundoku doesn't read a page for where it leads, so the person opens it in a browser and copies the
    /// link it ends on.
    #[error("{site} link that shows a page before leading anywhere")]
    ShortLinkPage { site: String },
    /// A short link leading to a site nothing downloads from: `site` is the short-link site's name, `target` the host
    /// it leads to.
    #[error("{site} link leading to {target}, which nothing downloads from")]
    ShortLinkToUnsupportedSite { site: String, target: String },
    /// A file its owner keeps for the host's paying subscribers.
    #[error("{site} file for subscribers only")]
    SubscribersOnly { site: String },
    /// A link that needs a password, which Tsundoku can't enter for this site yet (`HostFacts::unlock`).
    #[error("{site} link with a password")]
    PasswordProtected { site: String },
    /// A link that needs a password Tsundoku can enter: none was given, or the one given didn't open it (`wrong`).
    /// Before the person is asked, the passwords Tsundoku knows are tried (`passwords::read_trying`).
    /// The preview card, Pending and the queue ask for it.
    #[error("needs a password")]
    PasswordNeeded {
        /// A password the person typed was tried and didn't open it.
        wrong: bool,
        /// How many passwords Tsundoku tried by itself before asking, none of which opened it.
        #[serde(default)]
        tried: u32,
    },
    /// A link missing part of itself, such as a MEGA link without its key, or with a key that doesn't open it.
    #[error("incomplete {site} link")]
    LinkIncomplete { site: String },
    /// The host's page asks for a browser check (Cloudflare's), which Tsundoku doesn't try to pass. A link copied
    /// from the page's Download button in a browser still works.
    #[error("{site} asked for a browser check")]
    BrowserCheck { site: String },
    /// A link on a file host that isn't to a file or folder Tsundoku can read, such as the host's home page or a
    /// form of link it doesn't know yet. Unlike `UnsupportedSite`, the clipboard watcher keeps it in Pending, so a
    /// file the user meant to download doesn't go missing.
    #[error("{site} link Tsundoku doesn't recognize")]
    LinkNotRecognized { site: String },
    /// A shared folder with folders inside it, whose contents the host doesn't give Tsundoku (OneDrive's anonymous
    /// reading stops at the shared folder's own level).
    #[error("{site} folder with subfolders Tsundoku can't read")]
    NestedFolders { site: String },
    #[error("network error: {detail}")]
    Network { detail: String },
    /// The site answered "too many requests" (HTTP 429): its downloads and reads wait a while (`site_limits`). The
    /// preview card fills in the site and until when, for its message; the queue and the pending links wait instead
    /// of failing.
    #[error("too many requests: {detail}")]
    TooManyRequests {
        detail: String,
        #[serde(default)]
        site: Option<String>,
        /// In milliseconds since 1970.
        #[serde(default)]
        #[ts(type = "number | null")]
        until: Option<i64>,
    },
    /// The site lets a listing be read only so far at a time, and that's used up for now: X's 50 requests per list
    /// every 15 minutes (`listing::x_media`). Unlike `TooManyRequests`, nothing else on the site waits for it: its
    /// downloads go on. A listing's Load More, Load New and Load All sit the wait out instead of failing with it.
    #[error("{site} lists no more for now")]
    ListingPaused { site: String },
    /// The tool couldn't read the site's page, which an update of the tool often fixes.
    #[error("site error: {detail}")]
    SiteError {
        /// yt-dlp or gallery-dl.
        #[serde(default)]
        tool: ToolId,
        detail: String,
    },
    /// yt-dlp found no video, and gallery-dl, which downloads images, isn't set up.
    #[error("no video, and image downloads aren't set up")]
    ImagesNotSetUp,
    #[error("failed: {detail}")]
    Failed { detail: String },
    /// yt-dlp or gallery-dl itself couldn't run.
    #[error("the tool couldn't run: {error}")]
    Tool {
        #[serde(default)]
        tool: ToolId,
        error: ToolError,
    },
    #[error("cancelled")]
    Cancelled,
}

/// Where cookies stood when a link needed a signed-in account.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SignInCookies {
    /// Not looked at: a download, or a site whose sign-in cookies Tsundoku doesn't know.
    #[default]
    Unknown,
    /// Settings → Cookies is None.
    Off,
    /// No browser profile is signed in to the site.
    NotSignedIn,
    /// A profile is signed in, yet the site still refused: the sign-in may have expired.
    SignedIn,
}

/// What yt-dlp says about Instagram when it can't tell a missing sign-in from a slow-down, in lowercase.
const SIGN_IN_OR_SLOW_DOWN: &str = "rate-limit reached or login required";

impl PreviewError {
    /// A sign-in error whose message may as well mean the site asked to slow down (`SIGN_IN_OR_SLOW_DOWN`).
    pub fn may_be_slow_down(&self) -> bool {
        matches!(self, Self::SignInRequired { detail, .. }
            if detail.to_lowercase().contains(SIGN_IN_OR_SLOW_DOWN))
    }

    /// A sign-in error with only the tool's message; the preview fills in the rest.
    pub fn sign_in(detail: String) -> Self {
        Self::SignInRequired {
            detail,
            site: None,
            cookies: SignInCookies::Unknown,
        }
    }

    /// A link that needs a password, as a tool or a host answers it: `wrong` when one was given and didn't open it.
    pub fn password_needed(wrong: bool) -> Self {
        Self::PasswordNeeded { wrong, tried: 0 }
    }

    /// A "too many requests" with only the tool's message; the preview fills in the rest.
    pub fn too_many_requests(detail: String) -> Self {
        Self::TooManyRequests {
            detail,
            site: None,
            until: None,
        }
    }
}

/// What yt-dlp's and gallery-dl's errors say when a site answered "too many requests", in lowercase. A message that
/// could also mean a missing sign-in ("rate-limit reached or login required") is sorted as one first, and
/// `browsers::slow_down_if_signed_in` makes it a slow-down again when a browser is signed in to Instagram.
pub(crate) const TOO_MANY_REQUESTS_PHRASES: [&str; 7] = [
    "error 429",
    "'429",
    "429 too many",
    "too many requests",
    "rate limit",
    "rate-limit",
    "please wait a few minutes before you try again",
];

/// What yt-dlp's extractors say when the video password given didn't open the video, in lowercase. Tsundoku never
/// gives yt-dlp an account's password (it signs in with cookies), so these are about the video's.
const WRONG_PASSWORD_PHRASES: [&str; 6] = [
    "wrong video password",
    "wrong password",
    "wrong passcode",
    "invalid video password",
    "invalid password",
    "incorrect password",
];

/// The programs a fetch runs: yt-dlp, and Deno and ffmpeg when they're set up, and where to read cookies from.
#[derive(Debug, Clone, Default)]
pub struct FetchTools<'a> {
    pub yt_dlp: Option<&'a Path>,
    pub deno: Option<&'a Path>,
    pub ffmpeg: Option<&'a Path>,
    /// The `--cookies-from-browser` value (`browsers::cookies_for`), or `None` for no cookies.
    pub cookies: Option<String>,
    /// Whether yt-dlp may look for a video on any web page (its generic extractor). Off for links copied in other
    /// apps, so a link to a site yt-dlp doesn't know is turned down at once, without visiting it.
    pub any_page: bool,
    /// Where yt-dlp and its Deno keep their caches, or `None` for their usual folders.
    pub caches: Option<&'a ToolCacheFolder>,
    /// The video's password (`--video-password`), for a link that needs one.
    pub password: Option<&'a Password>,
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
    let answer = run(tools, link, &[], FETCH_TIMEOUT, cancel)?;
    Ok(Fetched {
        video: parse_info(&answer.json)?,
        missing_js_runtime: answer.missing_js_runtime,
    })
}

/// What yt-dlp answered for a link.
pub(crate) struct Answer {
    /// Its `--dump-single-json`.
    pub json: String,
    /// It ran without Deno, so YouTube may have offered fewer formats.
    pub missing_js_runtime: bool,
}

/// Runs yt-dlp on the link, with `extra` arguments after Tsundoku's own (`listing` adds how many to list), and returns
/// its answer, or why it failed. `link` must come from `normalize_link`.
pub(crate) fn run(
    tools: &FetchTools,
    link: &str,
    extra: &[&str],
    timeout: Duration,
    cancel: &CancelToken,
) -> Result<Answer, PreviewError> {
    let Some(yt_dlp) = tools.yt_dlp else {
        return Err(PreviewError::Tool {
            tool: ToolId::YtDlp,
            error: ToolError::FileMissing {
                path: "yt-dlp".to_owned(),
            },
        });
    };
    let output = process::run_collecting_with_cancel(
        &command(tools, yt_dlp, Output::Whole, extra, link),
        cancel,
        timeout,
    )
    .map_err(|error| run_failed(error, yt_dlp, timeout))?;

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
    Ok(Answer {
        json: output.stdout().collect(),
        missing_js_runtime: stderr
            .iter()
            .any(|line| line.contains(NO_JS_RUNTIME_WARNING)),
    })
}

/// Runs yt-dlp on a listing's page as `run` does, but hands each of its entries to `on_entry` as soon as yt-dlp has read
/// it, as a line of JSON (`--lazy-playlist --dump-json`), rather than all of them at the end: for a channel too long to
/// wait for. An entry's line has the listing's own fields too, prefixed `playlist_`. `link` must come from
/// `normalize_link`.
pub(crate) fn run_entries(
    tools: &FetchTools,
    link: &str,
    extra: &[&str],
    timeout: Duration,
    cancel: &CancelToken,
    on_entry: &mut dyn FnMut(&str),
) -> Result<(), PreviewError> {
    let Some(yt_dlp) = tools.yt_dlp else {
        return Err(PreviewError::Tool {
            tool: ToolId::YtDlp,
            error: ToolError::FileMissing {
                path: "yt-dlp".to_owned(),
            },
        });
    };
    let mut stderr = Vec::new();
    let status = process::run(
        &command(tools, yt_dlp, Output::Entries, extra, link),
        cancel,
        Some(timeout),
        |line| match line {
            OutputLine::Stdout(text) => on_entry(&text),
            OutputLine::Stderr(text) => stderr.push(text),
        },
    )
    .map_err(|error| run_failed(error, yt_dlp, timeout))?;
    if !status.success() {
        let stderr: Vec<&str> = stderr.iter().map(String::as_str).collect();
        return Err(classify_failure(&stderr));
    }
    Ok(())
}

/// Why yt-dlp couldn't be run to its end.
fn run_failed(error: RunError, yt_dlp: &Path, timeout: Duration) -> PreviewError {
    match error {
        RunError::Start { source, .. } => PreviewError::Tool {
            tool: ToolId::YtDlp,
            error: start_error(source, yt_dlp),
        },
        RunError::Cancelled => PreviewError::Cancelled,
        RunError::TimedOut => PreviewError::Network {
            detail: format!("no answer within {} seconds", timeout.as_secs()),
        },
        RunError::Wait(source) => PreviewError::Failed {
            detail: source.to_string(),
        },
    }
}

/// How yt-dlp writes what it read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Output {
    /// One JSON answer at the end, a listing's entries inside it.
    Whole,
    /// A listing's entries one per line, each as soon as it's read.
    Entries,
}

fn command(
    tools: &FetchTools,
    yt_dlp: &Path,
    output: Output,
    extra: &[&str],
    link: &str,
) -> ProgramCommand {
    let mut command = ProgramCommand::new(yt_dlp).args([
        "--ignore-config",
        // A watch?v=…&list=… link means the video, not its playlist.
        "--no-playlist",
        // A playlist or a channel is listed without reading each of its videos.
        "--flat-playlist",
    ]);
    command = match output {
        Output::Whole => command.arg("--dump-single-json"),
        // Without --lazy-playlist, yt-dlp reads every entry before it writes the first.
        Output::Entries => command.args(["--lazy-playlist", "--dump-json"]),
    };
    command = command.args([
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
    if let Some(cookies) = &tools.cookies {
        command = command.arg("--cookies-from-browser").arg(cookies);
    }
    if !tools.any_page {
        command = command.args(["--ies", "default,-generic"]);
    }
    if let Some(caches) = tools.caches {
        let (name, folder) = caches.deno_variable();
        command = command.args(caches.yt_dlp_args()).variable(name, folder);
    }
    if let Some(password) = tools.password {
        command = command.arg(VIDEO_PASSWORD_OPTION).arg(password.reveal());
    }
    // After "--", a link can't be taken for an option, whatever it starts with.
    command.args(extra).arg("--").arg(link)
}

/// Whether yt-dlp's answer is a listing (a playlist or channel) rather than one video.
pub(crate) fn is_listing(json: &str) -> bool {
    #[derive(Deserialize)]
    struct Kind {
        #[serde(rename = "_type")]
        kind: Option<String>,
    }
    serde_json::from_str::<Kind>(json)
        .is_ok_and(|answer| matches!(answer.kind.as_deref(), Some("playlist" | "multi_video")))
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
    uploader_id: Option<String>,
    channel: Option<String>,
    duration: Option<f64>,
    upload_date: Option<String>,
    thumbnail: Option<String>,
    live_status: Option<String>,
    height: Option<u32>,
    vcodec: Option<String>,
    acodec: Option<String>,
    filesize: Option<f64>,
    filesize_approx: Option<f64>,
    #[serde(default)]
    formats: Vec<FormatJson>,
}

#[derive(Deserialize)]
struct FormatJson {
    height: Option<u32>,
    vcodec: Option<String>,
    acodec: Option<String>,
    filesize: Option<f64>,
    filesize_approx: Option<f64>,
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

    let sizes = format_sizes(
        &info.formats,
        &FormatJson {
            height: info.height,
            vcodec: info.vcodec.clone(),
            acodec: info.acodec.clone(),
            filesize: info.filesize,
            filesize_approx: info.filesize_approx,
        },
    );
    let video = VideoInfo {
        site: extractor_key.to_lowercase(),
        site_name: info
            .webpage_url_domain
            .or(info.extractor)
            .unwrap_or_else(|| extractor_key.clone()),
        url: info.webpage_url.unwrap_or_default(),
        title: info.title.unwrap_or_else(|| video_id.clone()),
        video_id,
        uploader: info.uploader.or(info.channel),
        account: info.uploader_id.as_deref().and_then(handle),
        duration_seconds: info.duration,
        upload_date: info.upload_date.as_deref().and_then(iso_date),
        thumbnail_url: info.thumbnail,
        best_height,
        post: None,
        files: None,
        post_files: Vec::new(),
        shared_files: Vec::new(),
        playlist: None,
    };
    // Kept for when it's downloaded, to check there's room for it first.
    room::remember_sizes(&video, sizes);
    Ok(video)
}

/// What each format weighs, for `room`. An answer without a list of formats is its own one format (`own`).
fn format_sizes(formats: &[FormatJson], own: &FormatJson) -> FormatSizes {
    let formats = if formats.is_empty() {
        vec![format_size(own)]
    } else {
        formats.iter().map(format_size).collect()
    };
    FormatSizes { formats }
}

/// A format's size: its `filesize`, else its `filesize_approx`. It's video as for `best_height`, and has sound unless
/// its `acodec` is "none".
fn format_size(format: &FormatJson) -> FormatSize {
    FormatSize {
        height: format
            .height
            .filter(|_| format.vcodec.as_deref() != Some("none")),
        audio: format.acodec.as_deref() != Some("none"),
        bytes: format
            .filesize
            .or(format.filesize_approx)
            .filter(|bytes| *bytes >= 1.0)
            .map(|bytes| bytes as u64),
    }
}

/// yt-dlp's `uploader_id` as a handle: "@natgeo" becomes "natgeo". `None` for one that's only a number (TikTok) or a
/// YouTube channel ID ("UC" and 22 more), which aren't names anyone goes by.
pub(crate) fn handle(id: &str) -> Option<String> {
    let id = id.trim().trim_start_matches('@');
    let channel_id = id.len() == 24 && id.starts_with("UC");
    let number = id.chars().all(|character| character.is_ascii_digit());
    (!id.is_empty() && !number && !channel_id).then(|| id.to_owned())
}

/// "20050424" to "2005-04-24".
pub(crate) fn iso_date(compact: &str) -> Option<String> {
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
    } else if mentions(&WRONG_PASSWORD_PHRASES) {
        // Before the sign-in phrases: Screencast-O-Matic's says "Unable to login: Invalid password".
        PreviewError::password_needed(true)
    } else if mentions(&[VIDEO_PASSWORD_OPTION]) {
        // yt-dlp names its option only where it can take the video's password ("use the --video-password option").
        PreviewError::password_needed(false)
    } else if mentions(&[
        "unsupported url",
        "no suitable extractor",
        // yt-dlp's refusal of a site it dropped as used for piracy (GoFile since 2026): unsupported by yt-dlp, so
        // gallery-dl, which still reads some of them, is asked next.
        "primarily used for piracy",
    ]) {
        PreviewError::UnsupportedSite
    } else if mentions(&[
        "no video formats found",
        "no video could be found",
        "there's no video",
    ]) {
        // A post of images only, such as an Instagram photo: gallery-dl may download it.
        PreviewError::NoVideo { detail }
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
        PreviewError::sign_in(detail)
    } else if mentions(&TOO_MANY_REQUESTS_PHRASES) {
        // Before "unavailable": YouTube's says "This content isn't available, try again later. The current session
        // has been rate-limited…".
        PreviewError::too_many_requests(detail)
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
        PreviewError::SiteError {
            tool: ToolId::YtDlp,
            detail,
        }
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

    #[test]
    fn reads_the_uploaders_handle() {
        let video = parse_info(
            r#"{"id": "jNQXAC9IVRw", "extractor_key": "Youtube", "uploader": "jawed", "uploader_id": "@jawed"}"#,
        )
        .unwrap();
        assert_eq!(video.account.as_deref(), Some("jawed"));
        assert_eq!(handle("NASA").as_deref(), Some("NASA"));
        assert_eq!(handle("6812314569234"), None, "a number");
        assert_eq!(handle("UC4QobU6STFB0P71PMvOGN5A"), None, "a channel ID");
        assert_eq!(handle(" @ "), None);
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
    fn keeps_what_the_formats_weigh_for_the_room_check() {
        let answer = r#"{ "id": "sized", "extractor_key": "Youtube", "formats": [
            { "height": null, "vcodec": "none", "acodec": "opus", "filesize": 3000000 },
            { "height": 1080, "vcodec": "avc1", "acodec": "none", "filesize_approx": 90000000.5 },
            { "height": 720, "vcodec": "avc1", "acodec": "none" }
        ] }"#;
        let video = parse_info(answer).unwrap();
        let preset = |id: &str| {
            crate::presets::built_in_presets()
                .into_iter()
                .find(|preset| preset.id == id)
        };
        assert_eq!(
            room::expected_bytes(&video, preset("best-video").as_ref()),
            Some(93_000_000)
        );
        assert_eq!(
            room::expected_bytes(&video, preset("audio-mp3").as_ref()),
            Some(3_000_000)
        );

        let one_format = parse_info(
            r#"{ "id": "one", "extractor_key": "Vimeo", "height": 480, "vcodec": "h264", "acodec": "aac",
                "filesize": 20000000 }"#,
        )
        .unwrap();
        assert_eq!(
            room::expected_bytes(&one_format, preset("best-video").as_ref()),
            Some(20_000_000)
        );
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
        // Vimeo's public test video (player.vimeo.com/video/68375962), without its password and with a wrong one.
        assert_eq!(
            failure("password-needed"),
            PreviewError::password_needed(false)
        );
        assert_eq!(
            failure("password-wrong"),
            PreviewError::password_needed(true)
        );
    }

    #[test]
    fn tells_a_video_password_from_an_accounts() {
        let classify = |line: &str| classify_failure(&[line]);
        for line in [
            "ERROR: [wistia] abc: This content is password-protected. Use the --video-password option",
            "ERROR: [dropbox] abc: Password protected video, use --video-password <password>",
            "ERROR: [zoom] abc: This video is protected by a passcode, use the --video-password option",
        ] {
            assert_eq!(
                classify(line),
                PreviewError::password_needed(false),
                "{line}"
            );
        }
        for line in [
            "ERROR: [loom] abc: Invalid video password",
            "ERROR: [sproutvideo] abc: Incorrect password",
            "ERROR: [screencastomatic] abc: Unable to login: Invalid password",
        ] {
            assert_eq!(
                classify(line),
                PreviewError::password_needed(true),
                "{line}"
            );
        }
        // Vimeo's own sign-in names --password (the account's), which isn't a video password.
        assert!(matches!(
            classify(
                "ERROR: [vimeo] 68375962: The web client only works when logged-in. Use --cookies,                  --cookies-from-browser, --username and --password, --netrc-cmd, or --netrc (vimeo) to provide                  account credentials."
            ),
            PreviewError::SignInRequired { .. }
        ));
    }

    #[test]
    fn gives_yt_dlp_the_videos_password_before_the_link() {
        let password = Password::new("youtube-dl").unwrap();
        let tools = FetchTools {
            yt_dlp: Some(Path::new("yt-dlp.exe")),
            password: Some(&password),
            ..FetchTools::default()
        };
        let args: Vec<String> = command(
            &tools,
            Path::new("yt-dlp.exe"),
            Output::Whole,
            &[],
            "https://player.vimeo.com/video/1",
        )
        .arguments()
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
        let at = args
            .iter()
            .position(|arg| arg == "--video-password")
            .unwrap();
        assert_eq!(args[at + 1], "youtube-dl");
        assert_eq!(
            &args[args.len() - 2..],
            ["--", "https://player.vimeo.com/video/1"]
        );
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
        assert_eq!(
            classify(
                "ERROR: No suitable extractor found for URL https://github.com/Fuzen777/Tsundoku"
            ),
            PreviewError::UnsupportedSite
        );
        assert_eq!(
            classify(
                "ERROR: [Piracy] This website is no longer supported since it has been determined to be primarily used for piracy."
            ),
            PreviewError::UnsupportedSite,
            "yt-dlp's refusal of GoFile, which gallery-dl still reads"
        );
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
            classify("ERROR: unable to download video data: HTTP Error 429: Too Many Requests"),
            PreviewError::TooManyRequests { .. }
        ));
        assert!(matches!(
            classify(
                "ERROR: [instagram] DfH-Y3JugI4: Unable to download webpage: HTTP Error 429: Too Many Requests (caused by <HTTPError 429: Too Many Requests>)"
            ),
            PreviewError::TooManyRequests { .. }
        ));
        assert!(matches!(
            classify(
                "ERROR: [youtube] abc: This content isn't available, try again later. The current session has been rate-limited by YouTube for up to an hour."
            ),
            PreviewError::TooManyRequests { .. }
        ));
        assert!(
            matches!(
                classify(
                    "ERROR: [instagram] abc: Requested content is not available, rate-limit reached or login required. Use --cookies"
                ),
                PreviewError::SignInRequired { .. }
            ),
            "a message that may mean a missing sign-in asks for one"
        );
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
            cookies: Some("firefox".to_owned()),
            any_page: true,
            caches: None,
            password: None,
        };
        let command = command(
            &tools,
            Path::new("yt-dlp.exe"),
            Output::Whole,
            &[],
            "https://example.com/v",
        );
        let args: Vec<_> = command
            .arguments()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(&args[args.len() - 2..], ["--", "https://example.com/v"]);
        assert!(args.contains(&"--no-js-runtimes".to_owned()));
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--cookies-from-browser", "firefox"])
        );
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--ffmpeg-location", r"C:\tools\ffmpeg.exe"])
        );
        assert!(!args.contains(&"--ies".to_owned()));
    }

    #[test]
    fn puts_extra_arguments_before_the_link() {
        let tools = FetchTools {
            yt_dlp: Some(Path::new("yt-dlp.exe")),
            ..FetchTools::default()
        };
        let command = command(
            &tools,
            Path::new("yt-dlp.exe"),
            Output::Whole,
            &["--playlist-end", "50"],
            "https://www.youtube.com/@jawed/videos",
        );
        let args: Vec<_> = command
            .arguments()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            &args[args.len() - 4..],
            [
                "--playlist-end",
                "50",
                "--",
                "https://www.youtube.com/@jawed/videos"
            ]
        );
    }

    #[test]
    fn lists_entries_one_per_line_as_they_are_read() {
        let tools = FetchTools {
            yt_dlp: Some(Path::new("yt-dlp.exe")),
            ..FetchTools::default()
        };
        let arguments = |output| -> Vec<String> {
            command(
                &tools,
                Path::new("yt-dlp.exe"),
                output,
                &[],
                "https://www.youtube.com/@jawed/videos",
            )
            .arguments()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
        };
        let whole = arguments(Output::Whole);
        assert!(whole.contains(&"--dump-single-json".to_owned()));
        assert!(!whole.contains(&"--lazy-playlist".to_owned()));
        let entries = arguments(Output::Entries);
        assert!(entries.contains(&"--lazy-playlist".to_owned()));
        assert!(entries.contains(&"--dump-json".to_owned()));
        assert!(!entries.contains(&"--dump-single-json".to_owned()));
        assert!(entries.contains(&"--flat-playlist".to_owned()));
    }

    #[test]
    fn tells_a_listing_from_a_video() {
        assert!(is_listing(&fixture("youtube-playlist.json")));
        assert!(is_listing(&fixture("youtube-channel.json")));
        assert!(!is_listing(&fixture("youtube-watch.json")));
        assert!(!is_listing("not json"));
    }

    #[test]
    fn can_leave_out_the_generic_extractor() {
        let tools = FetchTools {
            yt_dlp: Some(Path::new("yt-dlp.exe")),
            deno: None,
            ffmpeg: None,
            cookies: None,
            any_page: false,
            caches: None,
            password: None,
        };
        let command = command(
            &tools,
            Path::new("yt-dlp.exe"),
            Output::Whole,
            &[],
            "https://example.com/v",
        );
        let args: Vec<_> = command
            .arguments()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--ies", "default,-generic"])
        );
        assert_eq!(&args[args.len() - 2..], ["--", "https://example.com/v"]);
    }

    #[test]
    fn keeps_yt_dlps_and_denos_caches_where_chosen() {
        let caches = ToolCacheFolder::new(std::path::PathBuf::from(r"C:\data\cache"));
        let tools = FetchTools {
            yt_dlp: Some(Path::new("yt-dlp.exe")),
            caches: Some(&caches),
            ..FetchTools::default()
        };
        let command = command(
            &tools,
            Path::new("yt-dlp.exe"),
            Output::Whole,
            &[],
            "https://example.com/v",
        );
        let args: Vec<_> = command
            .arguments()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--cache-dir", r"C:\data\cache\yt-dlp"])
        );
        assert_eq!(&args[args.len() - 2..], ["--", "https://example.com/v"]);
        assert_eq!(
            command.variables(),
            [(
                std::ffi::OsString::from("DENO_DIR"),
                std::ffi::OsString::from(r"C:\data\cache\deno")
            )]
        );
    }
}
