//! Asking gallery-dl what a link is (`gallery-dl -j`), for the image posts yt-dlp can't download, such as Instagram
//! photos and carousels. Its answer is read into the same `VideoInfo` as yt-dlp's, with `post` describing the files.
//! Tested against real answers saved under `tests/fixtures/gallery`.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use crate::cancel::CancelToken;
use crate::metadata::{
    PixivDetails, PixivWorkType, PostContents, PreviewError, TOO_MANY_REQUESTS_PHRASES, VideoInfo,
    VideoTag, tags_from,
};
use crate::pixiv_account::{self, GalleryDlConfig};
use crate::process::{self, OutputLine, ProgramCommand, RunError};
use crate::site_pictures;
use crate::tools::{ToolCacheFolder, ToolId, start_error};

/// Some sites take a while to answer; a fetch still running after this long is given up.
const FETCH_TIMEOUT: Duration = Duration::from_secs(2 * 60);

/// gallery-dl's exit code for a link none of its extractors accepts.
const UNSUPPORTED_EXIT_CODE: i32 = 64;

/// The longest title made from a post's caption.
const TITLE_LENGTH: usize = 120;

/// File extensions counted as videos. X gives its animated GIFs as MP4.
const VIDEO_EXTENSIONS: [&str; 6] = ["mp4", "webm", "mov", "m4v", "mkv", "m3u8"];
/// File extensions counted as images. Anything else, such as a .zip or a .pdf on Pixeldrain or Gofile, is a file of
/// another kind, which has no picture to show.
const IMAGE_EXTENSIONS: [&str; 13] = [
    "jpg", "jpeg", "png", "gif", "webp", "avif", "bmp", "heic", "heif", "jxl", "tif", "tiff", "svg",
];

/// gallery-dl's option that lists the cover of each of X's videos and animated GIFs (`extractor.twitter.previews`).
const X_COVERS: &str = "extractor.twitter.previews=true";

/// What a post's file is, by its extension, or its MIME type when the extension says nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileKind {
    Image,
    Video,
    /// Neither: an archive, a document, a program.
    Other,
}

/// The programs a fetch runs: gallery-dl, and where to read cookies from.
#[derive(Debug, Clone, Default)]
pub struct GalleryTools<'a> {
    pub gallery_dl: Option<&'a Path>,
    /// The `--cookies-from-browser` value (`browsers::cookies_for`), or `None` for no cookies.
    pub cookies: Option<String>,
    /// Where gallery-dl keeps its cache, or `None` for its usual folder.
    pub caches: Option<&'a ToolCacheFolder>,
}

/// Runs gallery-dl on the link and reads its answer. `link` must come from `metadata::normalize_link`.
pub fn fetch(
    tools: &GalleryTools,
    link: &str,
    cancel: &CancelToken,
) -> Result<VideoInfo, PreviewError> {
    let json = run(tools, link, &[], FETCH_TIMEOUT, cancel)?;
    parse_answer(&json, link)
}

/// Runs gallery-dl on the link, with `extra` arguments after Tsundoku's own (`listing` adds how many posts to list),
/// and returns its answer, or why it failed. `link` must come from `metadata::normalize_link`.
pub(crate) fn run(
    tools: &GalleryTools,
    link: &str,
    extra: &[&str],
    timeout: Duration,
    cancel: &CancelToken,
) -> Result<String, PreviewError> {
    run_logged(tools, link, extra, timeout, cancel).map(|answer| answer.json)
}

/// What gallery-dl answered for a link, and what it logged meanwhile.
pub(crate) struct LoggedAnswer {
    /// Its `--dump-json`.
    pub json: String,
    /// What it wrote to stderr: with `--verbose`, its debug lines too, such as where it is in an account's posts
    /// (`last_cursor`).
    pub log: Vec<String>,
}

/// As `run`, with what gallery-dl logged.
pub(crate) fn run_logged(
    tools: &GalleryTools,
    link: &str,
    extra: &[&str],
    timeout: Duration,
    cancel: &CancelToken,
) -> Result<LoggedAnswer, PreviewError> {
    run_watched(tools, link, extra, timeout, cancel, &mut |_| {})
}

/// As `run_logged`, handing each line gallery-dl logs to `on_log` as it writes it, for a read that may say it waits
/// (`waits_until`) long before it answers.
pub(crate) fn run_watched(
    tools: &GalleryTools,
    link: &str,
    extra: &[&str],
    timeout: Duration,
    cancel: &CancelToken,
    on_log: &mut dyn FnMut(&str),
) -> Result<LoggedAnswer, PreviewError> {
    let Some(gallery_dl) = tools.gallery_dl else {
        return Err(PreviewError::ImagesNotSetUp);
    };
    let mut json = String::new();
    let mut log = Vec::new();
    // Kept until the run ends, which deletes it.
    let config = pixiv_account::gallery_dl_config(link, None);
    let status = process::run(
        &command(tools, gallery_dl, extra, link, config.as_ref()),
        cancel,
        Some(timeout),
        |line| match line {
            OutputLine::Stdout(text) => json.push_str(&text),
            OutputLine::Stderr(text) => {
                on_log(&text);
                log.push(text);
            }
        },
    )
    .map_err(|error| match error {
        RunError::Start { source, .. } => PreviewError::Tool {
            tool: ToolId::GalleryDl,
            error: start_error(source, gallery_dl),
        },
        RunError::Cancelled => PreviewError::Cancelled,
        RunError::TimedOut => PreviewError::Network {
            detail: format!("no answer within {} seconds", timeout.as_secs()),
        },
        RunError::Wait(source) => PreviewError::Failed {
            detail: source.to_string(),
        },
    })?;

    if !status.success() {
        let stderr: Vec<&str> = log.iter().map(String::as_str).collect();
        return Err(classify_exit(status.code(), &stderr));
    }
    Ok(LoggedAnswer { json, log })
}

/// gallery-dl's options that make a read of X stop once X's limit on the list's requests is used up, rather than wait
/// for it to reset (its `ratelimit`, "wait" unless told): the answer then ends with the error `stopped_at_limit`
/// knows, after the posts read before it. X allows 50 requests per list every 15 minutes, and gallery-dl stops with 1
/// to 5 of them left.
pub(crate) const STOP_AT_LIMIT: [&str; 2] = ["-o", "ratelimit=abort"];

/// gallery-dl's line when it waits for a site's limit to reset before asking again ("Waiting for 14 minutes until
/// 01:12:17 (rate limit)"), as X's own header says when that is: the local time of day it waits until, in seconds
/// since midnight. `None` for any other line.
pub(crate) fn waits_until(line: &str) -> Option<u32> {
    let (_, rest) = line.split_once("] Waiting ")?;
    let (_, time) = rest.strip_suffix(" (rate limit)")?.rsplit_once("until ")?;
    let mut parts = time.splitn(3, ':').map(|part| {
        (part.len() == 2)
            .then(|| part.parse::<u32>().ok())
            .flatten()
    });
    let (hours, minutes, seconds) = (parts.next()??, parts.next()??, parts.next()??);
    (hours < 24 && minutes < 60 && seconds < 60).then_some(hours * 3600 + minutes * 60 + seconds)
}

/// The longest cursor taken from gallery-dl's log.
const CURSOR_LENGTH: usize = 1024;

/// Where gallery-dl was in an account's posts when it stopped, from the last "Cursor: …" line it logged with
/// `--verbose`: Instagram logs it each time it moves on to the next page of 12, and X to the next page of either of an
/// account's lists (`listing::x_media`), so it's the page after the last one read whole. `-o cursor=<it>` starts a
/// later read on that page. `None` when it logged none, or one that doesn't look like a cursor (letters, digits and
/// `-_=+/.:%`), which isn't passed on.
pub(crate) fn last_cursor(log: &[String]) -> Option<String> {
    let cursor = log
        .iter()
        .rev()
        .find_map(|line| line.split_once("[debug] Cursor: "))?
        .1
        .trim();
    let looks_like_one = !cursor.is_empty()
        && cursor != "None"
        && cursor.len() <= CURSOR_LENGTH
        && cursor
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_=+/.:%".contains(character));
    looks_like_one.then(|| cursor.to_owned())
}

fn command(
    tools: &GalleryTools,
    gallery_dl: &Path,
    extra: &[&str],
    link: &str,
    config: Option<&GalleryDlConfig>,
) -> ProgramCommand {
    let mut command = ProgramCommand::new(gallery_dl).args([
        // gallery-dl's spelling of yt-dlp's --ignore-config: a config file of the user's can't change the answer.
        "--config-ignore",
        // Every file's details as JSON, without downloading.
        "--dump-json",
        // X's videos come with their cover as a file of its own after them (`is_cover`), which is their only picture:
        // gallery-dl says nothing of it in the video's own details. For X alone, since other sites' `previews` add
        // files that are theirs to download.
        "-o",
        X_COVERS,
    ]);
    if let Some(cookies) = &tools.cookies {
        command = command.arg("--cookies-from-browser").arg(cookies);
    }
    if let Some(caches) = tools.caches {
        command = command.args(caches.gallery_dl_args());
    }
    // Pixiv's sign-in, in a file rather than on the command line.
    if let Some(config) = config {
        command = command.args(config.args());
    }
    // After "--", a link can't be taken for an option, whatever it starts with.
    command.args(extra).arg("--").arg(link)
}

/// Why gallery-dl stopped with an error code. Errors met while reading a site come in the answer instead.
fn classify_exit(code: Option<i32>, stderr: &[&str]) -> PreviewError {
    let detail = stderr
        .iter()
        .rev()
        .find(|line| line.contains("[error]"))
        .or(stderr.last())
        .map(|line| {
            line.rsplit_once("[error]")
                .map_or(*line, |(_, message)| message)
                .trim()
                .to_owned()
        })
        .unwrap_or_default();
    if code == Some(UNSUPPORTED_EXIT_CODE) || detail.starts_with("Unsupported URL") {
        PreviewError::UnsupportedSite
    } else if detail.is_empty() {
        PreviewError::Failed {
            detail: "gallery-dl stopped without saying why".to_owned(),
        }
    } else {
        PreviewError::Failed { detail }
    }
}

/// The account behind an account's posts, as gallery-dl describes it in each of them (Instagram's `user`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AccountInfo {
    pub username: Option<String>,
    pub full_name: Option<String>,
    /// Its profile picture.
    pub picture: Option<String>,
    /// How many posts it has in all (Instagram). `None` on X, which doesn't say.
    pub post_count: Option<u32>,
    /// X only: how many photos and videos the site counts on the account, as its Media tab words it ("372 photos &
    /// videos"). Files rather than posts, and some of them in posts its lists don't hold (`account_info`).
    pub media_count: Option<u32>,
}

/// The posts of gallery-dl's answer for an account's page, each as reading its own link would describe it, and the
/// account.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FoundPosts {
    pub posts: Vec<VideoInfo>,
    pub account: Option<AccountInfo>,
    /// How many posts gallery-dl answered, those without a file to download included, as `--post-range` counts them.
    pub answered: usize,
}

/// A post of gallery-dl's answer: its metadata (`[2, metadata]`, or its first file's when there's none) and its files.
struct PostMessages<'a> {
    post: &'a Value,
    files: Vec<FoundFile<'a>>,
}

/// One of a post's files (`[3, url, details]`).
struct FoundFile<'a> {
    url: &'a str,
    details: &'a Value,
    /// The address of its cover, when the answer gives it as a message of its own after the file: X's videos.
    cover: Option<&'a str>,
}

/// gallery-dl's answer, sorted into posts.
struct SortedAnswer<'a> {
    posts: Vec<PostMessages<'a>>,
    /// `[6, url, metadata]` messages: links to other pages, such as a profile's posts.
    links_elsewhere: usize,
    /// How many posts the answer described, those left out for having no file included.
    answered: usize,
}

/// Reads gallery-dl's answer: a list of messages, each `[2, metadata]` for a post, `[3, url, metadata]` for one of
/// its files, `[6, url, metadata]` for a link to another page (a profile's posts), or `[-1, {error, message}]`. A page
/// of several posts (an account, a search) or of links to others is refused as `NotListable`: an Instagram account is
/// listed by `listing` instead, before it gets here.
pub(crate) fn parse_answer(json: &str, link: &str) -> Result<VideoInfo, PreviewError> {
    let messages = read_messages(json)?;
    let answer = sort_answer(&messages)?;
    match answer.posts.as_slice() {
        [] if answer.links_elsewhere > 0 => Err(crate::listing::not_listable(link)),
        [] => Err(PreviewError::Unavailable {
            detail: "gallery-dl found nothing to download".to_owned(),
        }),
        [only] => video_of(only, link),
        _ => Err(crate::listing::not_listable(link)),
    }
}

/// Reads gallery-dl's answer for an account's posts (`--post-range` limits how many): every post with its files, in
/// the answer's order, and the account they're from.
pub(crate) fn parse_posts(json: &str, link: &str) -> Result<FoundPosts, PreviewError> {
    found_posts(&read_messages(json)?, link)
}

/// As `parse_posts`, for a read told to stop at the site's limit on its requests (`STOP_AT_LIMIT`): the posts read
/// before it stopped there, and whether it did.
pub(crate) fn parse_posts_before_limit(
    json: &str,
    link: &str,
) -> Result<(FoundPosts, bool), PreviewError> {
    let mut messages = read_messages(json)?;
    let limited = messages.last().is_some_and(stopped_at_limit);
    if limited {
        messages.pop();
    }
    Ok((found_posts(&messages, link)?, limited))
}

/// Whether a message of gallery-dl's answer is its error for a site's limit used up, which `STOP_AT_LIMIT` makes it
/// end a read of X with: `[-1, {"error": "AbortExtraction", "message": "Rate limit exceeded"}]`.
fn stopped_at_limit(message: &Value) -> bool {
    let Some([kind, error]) = message.as_array().map(Vec::as_slice) else {
        return false;
    };
    kind.as_i64() == Some(-1)
        && text(error, "error").as_deref() == Some("AbortExtraction")
        && text(error, "message").is_some_and(|message| message.starts_with("Rate limit exceeded"))
}

fn found_posts(messages: &[Value], link: &str) -> Result<FoundPosts, PreviewError> {
    let answer = sort_answer(messages)?;
    let account = answer
        .posts
        .first()
        .and_then(|first| account_info(first.post));
    let posts = answer
        .posts
        .iter()
        .map(|post| video_of(post, link))
        .collect::<Result<_, _>>()?;
    Ok(FoundPosts {
        posts,
        account,
        answered: answer.answered,
    })
}

/// The account whose posts are listed, from a post's `user`, in each site's fields: Instagram's `username`,
/// `full_name`, `profile_pic_url` and `media_count` (its posts), X's `name` (the handle), `nick`, `profile_image` and
/// `media_count`.
///
/// X's `media_count` (its own `tweet_counts.media_tweets`, which its Media tab shows as "372 photos & videos") is kept
/// apart from the posts' count, since it isn't one. Against both of an account's lists read whole on 2026-11-20, it was
/// never below their files: within 1 to 3 of them on accounts of photos (IRURI 372 against 371 files in 369 posts,
/// rustlang 110 against 109 files in 92 posts), and 23 to 35 above on zen_browser (141 against 118 files in 112 posts)
/// and rerundotio (393 against 358 in 340). It also counts what neither list holds: X's search gave zen_browser 16
/// posts of an animated GIF and 8 replies with a video that the lists don't have, 142 files in all.
fn account_info(post: &Value) -> Option<AccountInfo> {
    let user = post.get("user").filter(|user| user.is_object())?;
    let category = text(post, "category");
    let on_x = category.as_deref() == Some("twitter");
    if category.as_deref() == Some("pixiv") {
        // A Pixiv artist: `account` (the handle), `name` and `profile_image_urls.medium`; Pixiv doesn't say how many
        // works.
        return Some(AccountInfo {
            username: text(user, "account"),
            full_name: text(user, "name"),
            picture: user
                .get("profile_image_urls")
                .and_then(|pictures| text(pictures, "medium")),
            post_count: None,
            media_count: None,
        });
    }
    let (username, full_name, picture) = if on_x {
        ("name", "nick", "profile_image")
    } else {
        ("username", "full_name", "profile_pic_url")
    };
    let count = user
        .get("media_count")
        .and_then(Value::as_u64)
        .and_then(|count| u32::try_from(count).ok());
    Some(AccountInfo {
        username: text(user, username),
        full_name: text(user, full_name),
        picture: text(user, picture),
        post_count: count.filter(|_| !on_x),
        media_count: count.filter(|_| on_x),
    })
}

fn read_messages(json: &str) -> Result<Vec<Value>, PreviewError> {
    serde_json::from_str(json).map_err(|error| PreviewError::Failed {
        detail: format!("gallery-dl's answer can't be read: {error}"),
    })
}

/// Sorts gallery-dl's messages into posts, each with the files that follow it. An error message fails the whole
/// answer. A post with no file (a text post) is left out.
fn sort_answer(messages: &[Value]) -> Result<SortedAnswer<'_>, PreviewError> {
    let mut posts: Vec<PostMessages> = Vec::new();
    let mut links_elsewhere = 0;
    for message in messages {
        let Some(parts) = message.as_array() else {
            continue;
        };
        match (parts.first().and_then(Value::as_i64), parts.len()) {
            (Some(-1), _) => return Err(classify_error(parts.get(1).unwrap_or(&Value::Null))),
            (Some(2), _) => {
                if let Some(post) = parts.last() {
                    // gallery-dl may describe a post again before more of its files: still the same post.
                    let same_post = posts
                        .last()
                        .is_some_and(|last| same_post_id(last.post, post));
                    if !same_post {
                        posts.push(PostMessages {
                            post,
                            files: Vec::new(),
                        });
                    }
                }
            }
            (Some(3), 3) => {
                if let Some(url) = parts[1].as_str() {
                    let details = &parts[2];
                    if is_cover(details) {
                        // Never a file of its own: a download doesn't ask for covers, so it would get one file less.
                        let video = posts
                            .last_mut()
                            .and_then(|last| last.files.last_mut())
                            .filter(|file| kind_of(file.details) == FileKind::Video);
                        if let Some(video) = video {
                            video.cover.get_or_insert(url);
                        }
                        continue;
                    }
                    let file = FoundFile {
                        url,
                        details,
                        cover: None,
                    };
                    match posts.last_mut() {
                        Some(last) => last.files.push(file),
                        None => posts.push(PostMessages {
                            post: details,
                            files: vec![file],
                        }),
                    }
                }
            }
            (Some(6), _) => links_elsewhere += 1,
            _ => {}
        }
    }
    let answered = posts.len();
    posts.retain(|post| !post.files.is_empty());
    Ok(SortedAnswer {
        posts,
        links_elsewhere,
        answered,
    })
}

/// Whether a file of the answer is the cover of the video before it rather than one of the post's files: on X, where
/// `X_COVERS` asks for them, a file whose `type` is "preview".
fn is_cover(file: &Value) -> bool {
    text(file, "category").as_deref() == Some("twitter")
        && text(file, "type").as_deref() == Some("preview")
}

/// Whether two posts' metadata name the same post, by their site and ID. Without an ID to tell them apart, they're
/// taken as one.
fn same_post_id(first: &Value, second: &Value) -> bool {
    let key = |post: &Value| {
        let site = text(post, "category")?;
        let id = post_id(&site, post)?;
        Some((site, id))
    };
    match (key(first), key(second)) {
        (Some(first), Some(second)) => first == second,
        _ => true,
    }
}

/// A post as the preview card shows it.
fn video_of(found: &PostMessages, link: &str) -> Result<VideoInfo, PreviewError> {
    let (post, files) = (found.post, found.files.as_slice());
    let site = text(post, "category").ok_or_else(|| PreviewError::Failed {
        detail: "gallery-dl's answer has no site".to_owned(),
    })?;
    let post_id = post_id(&site, post).ok_or_else(|| PreviewError::Failed {
        detail: "gallery-dl's answer has no post ID".to_owned(),
    })?;

    let of_kind = |kind| {
        count(
            files
                .iter()
                .filter(|file| kind_of(file.details) == kind)
                .count(),
        )
    };
    let contents = PostContents {
        images: of_kind(FileKind::Image),
        videos: of_kind(FileKind::Video),
        others: of_kind(FileKind::Other),
    };
    let uploader = uploader(post);
    let account = account(&site, post);
    let title = caption_title(post)
        .or_else(|| uploader.as_ref().map(|name| format!("Post by {name}")))
        .or_else(|| lone_file_name(files))
        .unwrap_or_else(|| post_id.clone());

    Ok(VideoInfo {
        site_name: site_name(link).unwrap_or_else(|| site.clone()),
        url: post_page(&site, post, &post_id).unwrap_or_else(|| link.to_owned()),
        thumbnail_url: thumbnail(files),
        upload_date: text(post, "date")
            .and_then(|date| date.get(..10).map(str::to_owned))
            .filter(|date| date.as_bytes().get(4) == Some(&b'-')),
        site,
        video_id: post_id,
        title,
        uploader,
        account,
        duration_seconds: None,
        best_height: None,
        post: Some(contents),
        files: None,
        post_files: post_files(files),
        shared_files: Vec::new(),
        playlist: None,
        tags: post_tags(post),
        pixiv: pixiv_details(post),
    })
}

/// A Pixiv work's rating, kind, AI flag and series, from gallery-dl's `rating` (its words for Pixiv's `x_restrict`),
/// `type`, `illust_ai_type` (2 for made by AI) and `series`. `None` on any other site.
fn pixiv_details(post: &Value) -> Option<PixivDetails> {
    if text(post, "category").as_deref() != Some("pixiv") {
        return None;
    }
    let work_type = match text(post, "type").as_deref() {
        Some("illust") => Some(PixivWorkType::Illustration),
        Some("manga") => Some(PixivWorkType::Manga),
        Some("ugoira") => Some(PixivWorkType::Animation),
        _ => None,
    };
    Some(PixivDetails {
        rating: text(post, "rating"),
        work_type,
        ai: post.get("illust_ai_type").and_then(Value::as_i64) == Some(2),
        series: post
            .get("series")
            .filter(|series| series.is_object())
            .and_then(|series| text(series, "title")),
    })
}

/// A post's tags, from its `tags` (Instagram's and Tumblr's lists of names, Pixiv's names or, with its original tags,
/// names with their translations, a booru's names separated by spaces), else its `hashtags` (X, Bluesky).
fn post_tags(post: &Value) -> Vec<VideoTag> {
    let tags = ["tags", "hashtags"]
        .into_iter()
        .filter_map(|field| post.get(field))
        .find(|tags| tags.as_array().is_some_and(|tags| !tags.is_empty()) || tags.is_string());
    let Some(tags) = tags else {
        return Vec::new();
    };
    if let Some(names) = tags.as_str() {
        return tags_from(names.split_whitespace().map(VideoTag::named));
    }
    let listed = tags
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tag| match tag {
            Value::String(name) => Some(VideoTag::named(name)),
            Value::Object(_) => Some(VideoTag {
                name: text(tag, "name")?,
                english: text(tag, "translated_name"),
            }),
            _ => None,
        });
    tags_from(listed)
}

/// Turns an error gallery-dl met while reading the site into the reason the link can't be downloaded.
fn classify_error(error: &Value) -> PreviewError {
    classify_message(
        &text(error, "error").unwrap_or_default(),
        &text(error, "message").unwrap_or_default(),
    )
}

/// Why a download of a post failed, from gallery-dl's error output: its last error line, such as
/// "[instagram][error] HttpError: '401 Unauthorized' for 'https://…'", sorted as an error in a preview's answer is.
pub(crate) fn classify_download_failure(code: Option<i32>, stderr: &[&str]) -> PreviewError {
    let error_line = stderr
        .iter()
        .rev()
        .find_map(|line| line.rsplit_once("[error]").map(|(_, error)| error.trim()));
    match error_line {
        Some(error)
            if code != Some(UNSUPPORTED_EXIT_CODE) && !error.starts_with("Unsupported URL") =>
        {
            match error.split_once(": ") {
                Some((kind, message)) if !kind.contains(' ') => classify_message(kind, message),
                _ => classify_message("", error),
            }
        }
        _ => classify_exit(code, stderr),
    }
}

fn classify_message(kind: &str, message: &str) -> PreviewError {
    let kind = kind.to_owned();
    let detail = if message.is_empty() {
        kind.clone()
    } else {
        format!("{kind}: {message}")
    };
    let lowercase = message.to_lowercase();
    let mentions = |phrases: &[&str]| phrases.iter().any(|phrase| lowercase.contains(phrase));

    if matches!(
        kind.as_str(),
        "AuthRequired" | "AuthorizationError" | "AuthenticationError" | "LoginRequired"
    ) || missing_signed_in_field(&kind, message)
        || mentions(&[
            "login page",
            "log in",
            "logged in",
            "cookies needed",
            "sign in",
        ])
    {
        PreviewError::sign_in(detail)
    } else if kind == "NotFoundError"
        || mentions(&[
            "'400 bad request'",
            "'404 not found'",
            "'410 gone'",
            "could not be found",
        ])
    {
        PreviewError::Unavailable { detail }
    } else if mentions(&["'401", "'403"]) {
        PreviewError::sign_in(detail)
    } else if mentions(&TOO_MANY_REQUESTS_PHRASES) {
        PreviewError::too_many_requests(detail)
    } else if kind != "KeyError" && mentions(&["timed out", "connection"]) {
        // A KeyError is about the shape of the site's answer, whatever word its key holds.
        PreviewError::Network { detail }
    } else {
        // The site answered in a way gallery-dl didn't expect, which an update of gallery-dl often fixes.
        PreviewError::SiteError {
            tool: ToolId::GalleryDl,
            detail,
        }
    }
}

/// What Instagram's API answers gallery-dl without a signed-in account: a `KeyError` about a field whose name ends in
/// `_connection` (`'xdt_api__v1__feed__user_timeline_graphql_connection'`,
/// `'PolarisProfilePostsTabContentQuery_connection'`), which only a signed-in answer has. It asks for a sign-in rather
/// than being taken for a dropped connection, so it's never tried again by itself.
fn missing_signed_in_field(kind: &str, message: &str) -> bool {
    kind == "KeyError"
        && message
            .trim()
            .trim_matches(['\'', '"'])
            .ends_with("_connection")
}

/// The post's ID on its site: with the site, it identifies the post whatever link led to it. Instagram's shortcode
/// and X's tweet ID are also what yt-dlp gives for the same post, so the two tools share the history's keys.
fn post_id(site: &str, post: &Value) -> Option<String> {
    let fields: &[&str] = match site {
        "instagram" => &["post_shortcode", "shortcode"],
        "twitter" => &["tweet_id"],
        _ => &["post_id", "id", "index"],
    };
    fields.iter().find_map(|field| text(post, field))
}

/// The post's own page: gallery-dl's `post_url` (Instagram's), or on X, whose answer has none, the address made from
/// its author's handle and its ID, so a post listed from an account downloads from its own page. `None` keeps the link
/// that was read.
fn post_page(site: &str, post: &Value, post_id: &str) -> Option<String> {
    text(post, "post_url").or_else(|| {
        (site == "twitter").then(|| {
            // X opens a post at /i/status/<ID> whoever posted it.
            let author = account(site, post).unwrap_or_else(|| "i".to_owned());
            format!("https://x.com/{author}/status/{post_id}")
        })
    })
}

/// The handle of who posted, apart from their display name: Instagram's `username`, X's `author.name` (its `nick` is
/// the display name), and on other sites the first field they give one in. The same fields as `{account}` in post
/// file names (`post_names`).
fn account(site: &str, post: &Value) -> Option<String> {
    let fields: &[(&str, &str)] = match site {
        "instagram" => &[("", "username")],
        "twitter" => &[("author", "name"), ("user", "name")],
        _ => &[
            ("", "username"),
            ("author", "name"),
            ("author", "username"),
            ("author", "handle"),
            ("user", "account"),
            ("user", "name"),
            ("user", "username"),
            ("blog", "name"),
        ],
    };
    fields.iter().find_map(|&(object, field)| {
        if object.is_empty() {
            text(post, field)
        } else {
            post.get(object)
                .filter(|inner| inner.is_object())
                .and_then(|inner| text(inner, field))
        }
    })
}

/// The person who posted: their display name, or their account name.
fn uploader(post: &Value) -> Option<String> {
    let author = post.get("author").filter(|author| author.is_object());
    text(post, "fullname")
        .or_else(|| author.and_then(|author| text(author, "nick")))
        .or_else(|| author.and_then(|author| text(author, "name")))
        .or_else(|| text(post, "username"))
        .or_else(|| text(post, "author"))
        // Pixiv's and Fanbox's artist.
        .or_else(|| {
            post.get("user")
                .filter(|user| user.is_object())
                .and_then(|user| text(user, "name"))
        })
}

/// The caption's first line, shortened, as the post's title.
fn caption_title(post: &Value) -> Option<String> {
    let caption = text(post, "description")
        .or_else(|| text(post, "content"))
        // A Pixiv work's title, its caption being HTML.
        .or_else(|| text(post, "title"))?;
    let line = caption
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    Some(if line.chars().count() > TITLE_LENGTH {
        let shortened: String = line.chars().take(TITLE_LENGTH - 1).collect();
        format!("{}…", shortened.trim_end())
    } else {
        line.to_owned()
    })
}

/// The first image (X's at its small size, since an X account's list shows 50 of them), or a video's cover. A file of
/// another kind has none: its address is the file itself, which may be gigabytes.
fn thumbnail(files: &[FoundFile]) -> Option<String> {
    files.iter().find_map(|file| match kind_of(file.details) {
        FileKind::Image => Some(small_image(file.url)),
        FileKind::Video => cover(file),
        FileKind::Other => None,
    })
}

/// A file's picture when it isn't the file itself: Instagram's `display_url`, which its videos have as their cover,
/// the cover X lists after a video (`is_cover`), at its small size, or a Pixiv animation's first frame.
fn cover(file: &FoundFile) -> Option<String> {
    text(file.details, "display_url")
        .or_else(|| file.cover.map(small_image))
        // A Pixiv animation's first frame, from its zip's address.
        .or_else(|| site_pictures::pixiv_preview(file.url))
}

/// The name of a post's only file when it's neither an image nor a video, such as a .zip shared on Pixeldrain: the
/// post's best title when it has no caption or uploader.
fn lone_file_name(files: &[FoundFile]) -> Option<String> {
    let [file] = files else {
        return None;
    };
    if kind_of(file.details) != FileKind::Other {
        return None;
    }
    file_name(file.details)
}

/// A file's name with its extension, as the site gives it: `name`, else `filename` and `extension`.
fn file_name(file: &Value) -> Option<String> {
    text(file, "name").or_else(|| {
        let name = text(file, "filename")?;
        Some(match text(file, "extension") {
            Some(extension) => format!("{name}.{extension}"),
            None => name,
        })
    })
}

/// The post's files for the strip, numbered from 1 in the answer's order, which is the order gallery-dl's `--range`
/// counts in when it downloads them, all picked, with their names and sizes when the site gives them.
fn post_files(files: &[FoundFile]) -> Vec<crate::metadata::PostFile> {
    files
        .iter()
        .zip(1..)
        .map(|(file, number)| {
            let kind = kind_of(file.details);
            crate::metadata::PostFile {
                number,
                video: kind == FileKind::Video,
                thumbnail_url: cover(file)
                    .or_else(|| (kind == FileKind::Image).then(|| small_image(file.url)))
                    .filter(|url| url.starts_with("https://")),
                picked: true,
                name: file_name(file.details),
                size: file.details.get("size").and_then(Value::as_u64),
            }
        })
        .collect()
}

/// An image's address, asking X for its small size rather than the original, which can be 4K, and Pixiv for its smaller
/// copy (`site_pictures::pixiv_preview`). X's covers take the same sizes as its images.
fn small_image(url: &str) -> String {
    if url.starts_with("https://pbs.twimg.com/") {
        url.replace("name=orig", "name=small")
    } else {
        site_pictures::pixiv_preview(url).unwrap_or_else(|| url.to_owned())
    }
}

fn kind_of(file: &Value) -> FileKind {
    // A Pixiv animation (ugoira): a zip of frames with their timings, which its download makes a video of.
    if file.get("frames").is_some_and(Value::is_array) {
        return FileKind::Video;
    }
    let extension = text(file, "extension").map(|extension| extension.to_ascii_lowercase());
    let mime = text(file, "mime_type").map(|mime| mime.to_ascii_lowercase());
    let mime_is = |start: &str| mime.as_deref().is_some_and(|mime| mime.starts_with(start));
    match extension.as_deref() {
        Some(extension) if VIDEO_EXTENSIONS.contains(&extension) => FileKind::Video,
        Some(extension) if IMAGE_EXTENSIONS.contains(&extension) => FileKind::Image,
        // A file without an extension is taken as an image, as sites' images often are.
        None => FileKind::Image,
        Some(_) if mime_is("video/") => FileKind::Video,
        Some(_) if mime_is("image/") => FileKind::Image,
        Some(_) => FileKind::Other,
    }
}

/// The link's host without "www.", as the site's name ("instagram.com", "x.com").
fn site_name(link: &str) -> Option<String> {
    let host = link.split("://").nth(1)?.split(['/', '?', '#']).next()?;
    let host = host.strip_prefix("www.").unwrap_or(host);
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// A field as text: a string, or a number written out, since sites give IDs as either.
fn text(value: &Value, field: &str) -> Option<String> {
    match value.get(field)? {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = format!(
            "{}/tests/fixtures/gallery/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
    }

    #[test]
    fn takes_the_last_cursor_logged() {
        let log = |lines: &[&str]| -> Vec<String> {
            lines.iter().map(|line| (*line).to_owned()).collect()
        };
        assert_eq!(
            last_cursor(&log(&[
                "[instagram][debug] Cursor: QVFDaWRf-first_page==",
                "[urllib3.connectionpool][debug] https://www.instagram.com:443 \"GET /api/v1/feed/user/1/ HTTP/1.1\" 200 None",
                "[instagram][debug] Cursor: 3511227538461587341_25025320",
                "[instagram][debug] Cursor: None",
                "[instagram][debug] Cursor: 3502010472215348212_25025320",
            ]))
            .as_deref(),
            Some("3502010472215348212_25025320")
        );
        assert_eq!(
            last_cursor(&log(&[
                "[twitter][debug] Cursor: DAABCgABGUjK5Zw__-sKAAIZSJ4JK1uQAAgAAwAAAAIAAA"
            ]))
            .as_deref(),
            Some("DAABCgABGUjK5Zw__-sKAAIZSJ4JK1uQAAgAAwAAAAIAAA")
        );
        assert_eq!(
            last_cursor(&log(&["[instagram][debug] Cursor: None"])),
            None
        );
        assert_eq!(
            last_cursor(&log(&["[gallery-dl][debug] Version 1.32.19"])),
            None
        );
        assert_eq!(
            last_cursor(&log(&["[instagram][debug] Cursor: a b\"; rm"])),
            None,
            "not a cursor"
        );
    }

    #[test]
    fn reads_until_when_gallery_dl_waits() {
        assert_eq!(
            waits_until("[twitter][info] Waiting for 14 minutes until 01:12:17 (rate limit)"),
            Some(3600 + 12 * 60 + 17)
        );
        assert_eq!(
            waits_until("[twitter][info] Waiting for 37 seconds until 23:59:59 (rate limit)"),
            Some(24 * 3600 - 1)
        );
        assert_eq!(
            waits_until("[twitter][info] Waiting for 1h 2min until 00:00:00 (rate limit)"),
            Some(0)
        );
        // Not a wait for a limit, or not a time of day.
        for line in [
            "[urllib3.connectionpool][debug] https://x.com:443 \"GET /i/api/graphql/a/UserPhotoTimeline HTTP/1.1\" 200 None",
            "[twitter][debug] Sleeping 6.21 seconds (request)",
            "[twitter][info] Waiting for 14 minutes until 01:12:17 (extractor)",
            "[twitter][info] Waiting for 14 minutes until 25:12:17 (rate limit)",
            "[twitter][info] Waiting for 14 minutes until 1:12 (rate limit)",
            "Waiting for 14 minutes until 01:12:17 (rate limit)",
        ] {
            assert_eq!(waits_until(line), None, "{line}");
        }
    }

    #[test]
    fn keeps_the_posts_read_before_a_sites_limit() {
        let link = "https://x.com/NASA/media?filter=photos";
        let whole = fixture("twitter-images.json");
        let (found, limited) = parse_posts_before_limit(&whole, link).unwrap();
        assert_eq!((found.posts.len(), limited), (1, false));

        // As a read told to stop at X's limit ends: its posts, then gallery-dl's error.
        let mut messages: Vec<Value> = serde_json::from_str(&whole).unwrap();
        messages.push(serde_json::json!([
            -1,
            {"error": "AbortExtraction", "message": "Rate limit exceeded"}
        ]));
        let cut_short = serde_json::to_string(&messages).unwrap();
        let (found, limited) = parse_posts_before_limit(&cut_short, link).unwrap();
        assert_eq!((found.posts.len(), limited), (1, true));
        assert_eq!(found.posts[0].video_id, "2105698284354367897");
        // Read as any other answer, the error fails it.
        assert!(matches!(
            parse_posts(&cut_short, link),
            Err(PreviewError::TooManyRequests { .. })
        ));

        // The limit met at the first request: nothing read.
        let at_once = r#"[[-1, {"error": "AbortExtraction", "message": "Rate limit exceeded"}]]"#;
        let (found, limited) = parse_posts_before_limit(at_once, link).unwrap();
        assert_eq!((found.posts.len(), found.answered, limited), (0, 0, true));

        // Any other error still fails the read.
        let refused =
            r#"[[-1, {"error": "AbortExtraction", "message": "Dependency: Unspecified"}]]"#;
        assert!(parse_posts_before_limit(refused, link).is_err());
    }

    #[test]
    fn lists_a_posts_files_in_order() {
        let carousel = parse_answer(
            &fixture("instagram-carousel.json"),
            "https://www.instagram.com/p/DfPQchkG1tg/",
        )
        .unwrap();
        let numbers: Vec<u32> = carousel.post_files.iter().map(|file| file.number).collect();
        assert_eq!(numbers, [1, 2, 3, 4, 5, 6]);
        let videos: Vec<bool> = carousel.post_files.iter().map(|file| file.video).collect();
        assert_eq!(videos, [false, true, true, true, true, false]);
        assert!(
            carousel
                .post_files
                .iter()
                .all(|file| file.picked && file.thumbnail_url.is_some()),
            "Instagram gives every file a picture, its videos' covers too"
        );
        assert_eq!(carousel.picked_post_files(), None);
        assert_eq!(carousel.post_file_count(), Some(6));

        let video = parse_answer(
            &fixture("twitter-video.json"),
            "https://x.com/NASA/status/2118397868562071609",
        )
        .unwrap();
        assert_eq!(
            video.post_files.len(),
            1,
            "the cover listed after the video isn't a file of the post"
        );
        assert_eq!(
            video.post_files[0].thumbnail_url.as_deref(),
            Some(
                "https://pbs.twimg.com/amplify_video_thumb/2118397784684470272/img/jd3D4CZ8mj74hxWc"
            ),
            "X's cover for the video"
        );
        assert_eq!(video.thumbnail_url, video.post_files[0].thumbnail_url);

        let images = parse_answer(
            r#"[[2, {"category": "twitter", "tweet_id": 1}],
                [3, "https://pbs.twimg.com/media/A?format=jpg&name=orig", {"extension": "jpg"}]]"#,
            "https://x.com/a/status/1",
        )
        .unwrap();
        assert_eq!(
            images.post_files[0].thumbnail_url.as_deref(),
            Some("https://pbs.twimg.com/media/A?format=jpg&name=small")
        );
        assert_eq!(
            images.thumbnail_url.as_deref(),
            Some("https://pbs.twimg.com/media/A?format=jpg&name=small"),
            "the post's picture too, as an account's list shows many"
        );
    }

    #[test]
    fn an_x_videos_cover_is_its_picture_and_never_a_file() {
        // As gallery-dl answers with `X_COVERS`: each video and animated GIF (an MP4 on X) is followed by its cover.
        let file = |kind: &str, extension: &str, number: u32| {
            format!(
                r#"{{"category": "twitter", "tweet_id": 5, "type": "{kind}", "extension": "{extension}",
                    "num": {number}, "count": 5}}"#
            )
        };
        let json = format!(
            r#"[[2, {{"category": "twitter", "tweet_id": 5, "author": {{"name": "nasa"}}, "count": 5}}],
                [3, "https://pbs.twimg.com/media/A?format=jpg&name=orig", {photo}],
                [3, "https://video.twimg.com/amplify_video/1/vid/avc1/1280x720/b.mp4?tag=14", {video}],
                [3, "https://pbs.twimg.com/amplify_video_thumb/1/img/B?format=jpg&name=orig", {video_cover}],
                [3, "https://video.twimg.com/tweet_video/c.mp4", {gif}],
                [3, "https://pbs.twimg.com/tweet_video_thumb/C?format=jpg&name=orig", {gif_cover}]]"#,
            photo = file("photo", "jpg", 1),
            video = file("video", "mp4", 2),
            video_cover = file("preview", "jpg", 3),
            gif = file("animated_gif", "mp4", 4),
            gif_cover = file("preview", "jpg", 5),
        );
        let post = parse_answer(&json, "https://x.com/nasa/status/5").unwrap();
        assert_eq!(
            post.post,
            Some(PostContents {
                images: 1,
                videos: 2,
                others: 0,
            }),
            "the covers aren't counted: a download doesn't get them"
        );
        let numbers: Vec<u32> = post.post_files.iter().map(|file| file.number).collect();
        assert_eq!(numbers, [1, 2, 3], "numbered as a download counts them");
        let pictures: Vec<Option<&str>> = post
            .post_files
            .iter()
            .map(|file| file.thumbnail_url.as_deref())
            .collect();
        assert_eq!(
            pictures,
            [
                Some("https://pbs.twimg.com/media/A?format=jpg&name=small"),
                Some("https://pbs.twimg.com/amplify_video_thumb/1/img/B?format=jpg&name=small"),
                Some("https://pbs.twimg.com/tweet_video_thumb/C?format=jpg&name=small"),
            ],
            "each at its small size"
        );

        // A post of one video: its cover is the post's picture.
        let json = format!(
            r#"[[2, {{"category": "twitter", "tweet_id": 6}}],
                [3, "https://video.twimg.com/ext_tw_video/2/pu/vid/avc1/720x1280/d.mp4", {video}],
                [3, "https://pbs.twimg.com/ext_tw_video_thumb/2/pu/img/D?format=jpg&name=orig", {cover}]]"#,
            video = file("video", "mp4", 1),
            cover = file("preview", "jpg", 2),
        );
        let post = parse_answer(&json, "https://x.com/nasa/status/6").unwrap();
        assert_eq!(
            post.thumbnail_url.as_deref(),
            Some("https://pbs.twimg.com/ext_tw_video_thumb/2/pu/img/D?format=jpg&name=small")
        );
        assert_eq!(post.post_file_count(), Some(1));

        // A cover with no video before it is still no file.
        let json = format!(
            r#"[[2, {{"category": "twitter", "tweet_id": 7}}],
                [3, "https://pbs.twimg.com/media/E?format=jpg&name=orig", {photo}],
                [3, "https://pbs.twimg.com/amplify_video_thumb/3/img/F?format=jpg&name=orig", {cover}]]"#,
            photo = file("photo", "jpg", 1),
            cover = file("preview", "jpg", 2),
        );
        let post = parse_answer(&json, "https://x.com/nasa/status/7").unwrap();
        assert_eq!(post.post_files.len(), 1);
        assert_eq!(
            post.post_files[0].thumbnail_url.as_deref(),
            Some("https://pbs.twimg.com/media/E?format=jpg&name=small"),
            "a photo keeps its own picture"
        );

        // Another site's file of that type is one of its files.
        let json = r#"[[2, {"category": "example", "id": "1"}],
            [3, "https://cdn.example/1.mp4", {"category": "example", "extension": "mp4"}],
            [3, "https://cdn.example/1.jpg", {"category": "example", "type": "preview", "extension": "jpg"}]]"#;
        let post = parse_answer(json, "https://example.com/post/1").unwrap();
        assert_eq!(post.post_files.len(), 2);
    }

    #[test]
    fn counts_the_files_picked() {
        let mut carousel = parse_answer(
            &fixture("instagram-carousel.json"),
            "https://www.instagram.com/p/DfPQchkG1tg/",
        )
        .unwrap();
        for file in &mut carousel.post_files {
            file.picked = file.number % 2 == 1;
        }
        assert_eq!(carousel.picked_post_files(), Some(vec![1, 3, 5]));
        assert_eq!(carousel.post_file_count(), Some(3));
    }

    #[test]
    fn reads_an_instagram_carousel() {
        let link = "https://www.instagram.com/p/DfPQchkG1tg/";
        let post = parse_answer(&fixture("instagram-carousel.json"), link).unwrap();
        assert_eq!(post.site, "instagram");
        assert_eq!(post.site_name, "instagram.com");
        assert_eq!(post.video_id, "DfPQchkG1tg");
        assert_eq!(post.upload_date.as_deref(), Some("2026-11-02"));
        assert_eq!(
            post.post,
            Some(PostContents {
                images: 2,
                videos: 4,
                others: 0,
            })
        );
        assert!(post.thumbnail_url.is_some());
        assert_eq!(post.uploader.as_deref(), Some("Instagram"));
        assert_eq!(post.account.as_deref(), Some("instagram"));
        assert_eq!(post.best_height, None);
    }

    #[test]
    fn reads_an_instagram_reel_under_the_same_key_as_yt_dlp() {
        let link = "https://www.instagram.com/reel/DfH-Y3JugI4/";
        let post = parse_answer(&fixture("instagram-reel.json"), link).unwrap();
        // yt-dlp's archive writes "instagram DfH-Y3JugI4" for this reel too.
        assert_eq!(
            (post.site.as_str(), post.video_id.as_str()),
            ("instagram", "DfH-Y3JugI4")
        );
        assert_eq!(
            post.post,
            Some(PostContents {
                images: 0,
                videos: 1,
                others: 0,
            })
        );
        // The reel's poster.
        assert!(post.thumbnail_url.is_some());
    }

    #[test]
    fn an_x_accounts_count_is_of_its_photos_and_videos_and_instagrams_of_its_posts() {
        let on_x = serde_json::json!({
            "category": "twitter",
            "user": {"name": "iruri_rapulras", "nick": "IRURI", "profile_image": "https://pbs.twimg.com/a.jpg",
                     "media_count": 372, "statuses_count": 13781},
        });
        let account = account_info(&on_x).unwrap();
        assert_eq!(account.username.as_deref(), Some("iruri_rapulras"));
        assert_eq!(account.full_name.as_deref(), Some("IRURI"));
        assert_eq!(account.media_count, Some(372));
        assert_eq!(
            account.post_count, None,
            "X never says how many posts: 372 photos and videos were 369 posts"
        );

        let on_instagram = serde_json::json!({
            "category": "instagram",
            "user": {"username": "instagram", "full_name": "Instagram", "media_count": 8634},
        });
        let account = account_info(&on_instagram).unwrap();
        assert_eq!(account.post_count, Some(8634));
        assert_eq!(account.media_count, None);

        // Without a count, neither.
        let bare = serde_json::json!({"category": "twitter", "user": {"name": "someone"}});
        let account = account_info(&bare).unwrap();
        assert_eq!((account.post_count, account.media_count), (None, None));
    }

    #[test]
    fn reads_posts_on_x() {
        let images = parse_answer(
            &fixture("twitter-images.json"),
            "https://x.com/NASA/status/2105698284354367897",
        )
        .unwrap();
        assert_eq!(
            (images.site.as_str(), images.video_id.as_str()),
            ("twitter", "2105698284354367897")
        );
        assert_eq!(images.site_name, "x.com");
        assert_eq!(images.account.as_deref(), Some("NASA"));
        assert_eq!(
            images.post,
            Some(PostContents {
                images: 4,
                videos: 0,
                others: 0,
            })
        );

        let video = parse_answer(
            &fixture("twitter-video.json"),
            "https://x.com/NASA/status/2118397868562071609",
        )
        .unwrap();
        assert_eq!(video.video_id, "2118397868562071609");
        assert_eq!(
            video.post,
            Some(PostContents {
                images: 0,
                videos: 1,
                others: 0,
            })
        );
    }

    #[test]
    fn an_x_post_is_at_its_own_page() {
        // X's answer has no post_url: the post's page is made from its author and ID, whatever link was read.
        let images = parse_answer(
            &fixture("twitter-images.json"),
            "https://twitter.com/NASA/status/2105698284354367897/photo/2?s=20",
        )
        .unwrap();
        assert_eq!(images.url, "https://x.com/NASA/status/2105698284354367897");
        let without_author = parse_answer(
            r#"[[2, {"category": "twitter", "tweet_id": 1}],
                [3, "https://pbs.twimg.com/media/A?format=jpg&name=orig", {"extension": "jpg"}]]"#,
            "https://x.com/someone/status/1",
        )
        .unwrap();
        assert_eq!(without_author.url, "https://x.com/i/status/1");

        // Another site without post_url keeps the link that was read.
        let other = parse_answer(
            r#"[[2, {"category": "example", "id": "1"}], [3, "https://cdn.example/1.png", {"extension": "png"}]]"#,
            "https://example.com/post/1",
        )
        .unwrap();
        assert_eq!(other.url, "https://example.com/post/1");
    }

    #[test]
    fn refuses_pages_of_several_posts() {
        // An Instagram account is listed before gallery-dl reads it as a post; this is what any other profile gives.
        let link = "https://www.instagram.com/instagram/";
        assert_eq!(
            parse_answer(&fixture("instagram-profile.json"), link),
            Err(PreviewError::NotListable {
                site: "Instagram".to_owned()
            })
        );

        let json = r#"[
            [2, {"category": "twitter", "tweet_id": 1, "content": "first"}],
            [3, "https://pbs.example/a.jpg", {"category": "twitter", "tweet_id": 1, "extension": "jpg"}],
            [2, {"category": "twitter", "tweet_id": 2, "content": "second"}],
            [3, "https://pbs.example/b.jpg", {"category": "twitter", "tweet_id": 2, "extension": "jpg"}]
        ]"#;
        assert_eq!(
            parse_answer(json, "https://x.com/NASA/media"),
            Err(PreviewError::NotListable {
                site: "X".to_owned()
            })
        );
    }

    #[test]
    fn a_post_described_again_stays_one_post() {
        let json = r#"[
            [2, {"category": "twitter", "tweet_id": 7, "content": "caption"}],
            [3, "https://pbs.example/a.jpg", {"category": "twitter", "tweet_id": 7, "extension": "jpg"}],
            [2, {"category": "twitter", "tweet_id": 7, "content": "caption"}],
            [3, "https://pbs.example/b.jpg", {"category": "twitter", "tweet_id": 7, "extension": "jpg"}]
        ]"#;
        let post = parse_answer(json, "https://x.com/NASA/status/7").unwrap();
        assert_eq!(
            post.post,
            Some(PostContents {
                images: 2,
                videos: 0,
                others: 0,
            })
        );
    }

    #[test]
    fn a_file_thats_neither_an_image_nor_a_video_is_a_file() {
        // Pixeldrain's answer for a .zip, trimmed: its address is the file itself.
        let file = r#"{"category": "pixeldrain", "subcategory": "file", "id": "VUo4mjaM", "name": "YAOCTRU_v11.0.zip",
            "filename": "YAOCTRU_v11.0", "extension": "zip", "mime_type": "application/zip", "size": 10315,
            "date": "2025-08-23 17:00:54", "url": "https://pixeldrain.com/api/file/VUo4mjaM?download"}"#;
        let json = format!(
            r#"[[2, {file}], [3, "https://pixeldrain.com/api/file/VUo4mjaM?download", {file}]]"#
        );
        let post = parse_answer(&json, "https://pixeldrain.com/u/VUo4mjaM").unwrap();
        assert_eq!(
            post.post,
            Some(PostContents {
                images: 0,
                videos: 0,
                others: 1,
            })
        );
        assert_eq!(
            post.title, "YAOCTRU_v11.0.zip",
            "named by its file, not its ID"
        );
        assert_eq!(
            post.thumbnail_url, None,
            "the file isn't fetched as a picture"
        );
        assert_eq!(post.post_files[0].thumbnail_url, None);
        assert!(!post.post_files[0].video);
        assert_eq!(
            post.post_files[0].name.as_deref(),
            Some("YAOCTRU_v11.0.zip")
        );
        assert_eq!(post.post_files[0].size, Some(10315));
        assert_eq!(post.post_file_count(), Some(1));

        // Gofile's answer for a public folder holding one .zip, trimmed: its name and size come with it too.
        let folder = r#"{"category": "gofile", "subcategory": "folder", "id": "a36c7569-4c79-42d5-9353-b55c05b7dbd3",
            "name": "Folder", "type": "folder"}"#;
        let file = r#"{"category": "gofile", "subcategory": "folder", "id": "c995c0c2-ae12-4579-b5e1-040680a0644c",
            "name": "AI-onnx.zip", "filename": "AI-onnx", "extension": "zip", "mimetype": "application/zip",
            "size": 111638792, "num": 1, "type": "file"}"#;
        let json = format!(
            r#"[[2, {folder}], [3, "https://store4.gofile.io/download/web/c995/AI-onnx.zip", {file}]]"#
        );
        let post = parse_answer(&json, "https://gofile.io/d/Folder").unwrap();
        assert_eq!(post.post_files[0].name.as_deref(), Some("AI-onnx.zip"));
        assert_eq!(post.post_files[0].size, Some(111_638_792));

        let kinds: Vec<FileKind> = [
            r#"{"extension": "JPG"}"#,
            r#"{"extension": "webm"}"#,
            r#"{"extension": "bin", "mime_type": "video/mp4"}"#,
            r#"{"extension": "dat", "mime_type": "image/png"}"#,
            r#"{"extension": "pdf", "mime_type": "application/pdf"}"#,
            r#"{}"#,
        ]
        .iter()
        .map(|file| kind_of(&serde_json::from_str(file).unwrap()))
        .collect();
        assert_eq!(
            kinds,
            [
                FileKind::Image,
                FileKind::Video,
                FileKind::Video,
                FileKind::Image,
                FileKind::Other,
                FileKind::Image
            ]
        );
    }

    #[test]
    fn classifies_real_failures() {
        let missing = parse_answer(
            &fixture("instagram-missing.json"),
            "https://www.instagram.com/p/AAAAAAAAAAA/",
        );
        assert!(
            matches!(missing, Err(PreviewError::Unavailable { .. })),
            "{missing:?}"
        );
        for (name, link) in [
            (
                "instagram-posts-signed-out.json",
                "https://www.instagram.com/instagram/posts/",
            ),
            ("twitter-media-signed-out.json", "https://x.com/NASA/media"),
            (
                "pixiv-artworks-signed-out.json",
                "https://www.pixiv.net/users/11/artworks",
            ),
        ] {
            let result = parse_answer(&fixture(name), link);
            assert!(
                matches!(
                    result,
                    Err(PreviewError::SignInRequired { .. } | PreviewError::Unavailable { .. })
                ),
                "{name}: {result:?}"
            );
        }
    }

    #[test]
    fn reads_a_post_from_its_messages() {
        let json = r#"[
            [2, {"category": "instagram", "post_shortcode": "Abc123", "post_url": "https://www.instagram.com/p/Abc123/",
                 "username": "someone", "fullname": "Some One", "date": "2026-11-02 10:20:30",
                 "description": "\n  First line of the caption\nSecond line", "count": 3}],
            [3, "https://cdn.example/1.jpg", {"category": "instagram", "num": 1, "extension": "jpg"}],
            [3, "https://cdn.example/2.mp4", {"category": "instagram", "num": 2, "extension": "mp4",
                 "display_url": "https://cdn.example/2-poster.jpg"}],
            [3, "https://cdn.example/3.webp", {"category": "instagram", "num": 3, "extension": "webp"}]
        ]"#;
        let post = parse_answer(json, "https://instagram.com/p/Abc123/?img_index=2").unwrap();
        assert_eq!(post.site, "instagram");
        assert_eq!(post.site_name, "instagram.com");
        assert_eq!(post.video_id, "Abc123");
        assert_eq!(post.url, "https://www.instagram.com/p/Abc123/");
        assert_eq!(post.title, "First line of the caption");
        assert_eq!(post.uploader.as_deref(), Some("Some One"));
        assert_eq!(post.account.as_deref(), Some("someone"));
        assert_eq!(post.upload_date.as_deref(), Some("2026-11-02"));
        assert_eq!(
            post.thumbnail_url.as_deref(),
            Some("https://cdn.example/1.jpg")
        );
        assert_eq!(
            post.post,
            Some(PostContents {
                images: 2,
                videos: 1,
                others: 0,
            })
        );
    }

    #[test]
    fn reads_a_posts_tags_whichever_way_its_site_gives_them() {
        let tags_of = |post: &str| {
            let json = format!(
                r#"[[2, {post}], [3, "https://cdn.example/1.jpg", {{"category": "pixiv", "num": 0, "extension": "jpg"}}]]"#
            );
            let post = parse_answer(&json, "https://www.pixiv.net/artworks/1").unwrap();
            post.tags
                .into_iter()
                .map(|tag| match tag.english {
                    Some(english) => format!("{}={english}", tag.name),
                    None => tag.name,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            tags_of(r##"{"category": "pixiv", "id": 1, "tags": ["#cats", " dogs ", "Cats", ""]}"##),
            ["cats", "dogs"],
            "names, without a hash, each once"
        );
        assert_eq!(
            tags_of(
                r#"{"category": "pixiv", "id": 1, "tags": [{"name": "原神", "translated_name": "Genshin Impact"},
                    {"name": "オリジナル", "translated_name": null}]}"#
            ),
            ["原神=Genshin Impact", "オリジナル"],
            "names with their translations"
        );
        assert_eq!(
            tags_of(r#"{"category": "danbooru", "id": 1, "tags": "1girl  solo"}"#),
            ["1girl", "solo"],
            "a booru's names separated by spaces"
        );
        assert_eq!(
            tags_of(r#"{"category": "twitter", "tweet_id": 1, "hashtags": ["space"]}"#),
            ["space"],
            "X's hashtags"
        );
        assert!(tags_of(r#"{"category": "twitter", "tweet_id": 1}"#).is_empty());
    }

    #[test]
    fn reads_a_pixiv_works_details() {
        let json = r#"[
            [2, {"category": "pixiv", "id": 7, "title": "Work", "type": "manga", "rating": "R-18",
                 "illust_ai_type": 2, "series": {"id": 1, "title": "Series"},
                 "user": {"id": 3, "name": "Artist", "account": "artist"}}],
            [3, "https://i.pximg.net/img-original/img/2026/01/01/00/00/00/7_p0.png", {"category": "pixiv", "num": 0, "extension": "png"}]
        ]"#;
        let work = parse_answer(json, "https://www.pixiv.net/artworks/7").unwrap();
        assert_eq!(work.title, "Work");
        assert_eq!(work.uploader.as_deref(), Some("Artist"));
        assert_eq!(work.account.as_deref(), Some("artist"));
        assert_eq!(
            work.pixiv,
            Some(PixivDetails {
                rating: Some("R-18".to_owned()),
                work_type: Some(PixivWorkType::Manga),
                ai: true,
                series: Some("Series".to_owned()),
            })
        );
        assert_eq!(
            work.thumbnail_url.as_deref(),
            Some(
                "https://i.pximg.net/c/540x540_70/img-master/img/2026/01/01/00/00/00/7_p0_master1200.jpg"
            )
        );
        let tweet = parse_answer(
            r#"[[2, {"category": "twitter", "tweet_id": 1}], [3, "https://pbs.example/a.jpg", {"category": "twitter", "extension": "jpg"}]]"#,
            "https://x.com/a/status/1",
        )
        .unwrap();
        assert_eq!(tweet.pixiv, None);
    }

    #[test]
    fn names_a_post_without_a_caption_by_its_author() {
        let json = r#"[
            [2, {"category": "twitter", "tweet_id": 123, "author": {"name": "nasa", "nick": "NASA"}, "content": ""}],
            [3, "https://pbs.example/a.jpg", {"category": "twitter", "tweet_id": 123, "extension": "jpg"}]
        ]"#;
        let post = parse_answer(json, "https://x.com/NASA/status/123").unwrap();
        assert_eq!(post.video_id, "123");
        assert_eq!(post.title, "Post by NASA");
    }

    #[test]
    fn shortens_long_captions() {
        let caption = "a".repeat(300);
        let json = format!(
            r#"[[2, {{"category": "x", "id": "1", "description": "{caption}"}}],
                [3, "https://cdn.example/1.png", {{"extension": "png"}}]]"#
        );
        let post = parse_answer(&json, "https://example.com/1").unwrap();
        assert_eq!(post.title.chars().count(), TITLE_LENGTH);
        assert!(post.title.ends_with('…'));
    }

    #[test]
    fn classifies_errors_in_the_answer() {
        let error = |kind: &str, message: &str| {
            parse_answer(
                &format!(r#"[[-1, {{"error": "{kind}", "message": "{message}"}}]]"#),
                "https://x.com/a",
            )
            .unwrap_err()
        };
        assert!(matches!(
            error(
                "AuthRequired",
                "authenticated cookies needed to access this timeline"
            ),
            PreviewError::SignInRequired { .. }
        ));
        assert!(matches!(
            error(
                "AbortExtraction",
                "HTTP redirect to login page (https://www.instagram.com/accounts/login/)"
            ),
            PreviewError::SignInRequired { .. }
        ));
        assert!(matches!(
            error("NotFoundError", "Requested post could not be found"),
            PreviewError::Unavailable { .. }
        ));
        assert!(matches!(
            error(
                "HttpError",
                "'429 Too Many Requests' for 'https://x.com/i/api'"
            ),
            PreviewError::TooManyRequests { .. }
        ));
        assert!(matches!(
            error(
                "HttpError",
                "Please wait a few minutes before you try again."
            ),
            PreviewError::TooManyRequests { .. }
        ));
        assert!(matches!(
            error("HttpError", "Connection reset by peer"),
            PreviewError::Network { .. }
        ));
        assert_eq!(
            error("KeyError", "'data'"),
            PreviewError::SiteError {
                tool: ToolId::GalleryDl,
                detail: "KeyError: 'data'".to_owned()
            }
        );
    }

    #[test]
    fn classifies_failed_downloads_by_their_last_error() {
        assert!(matches!(
            classify_download_failure(
                Some(4),
                &[
                    "[instagram][info] Downloading 1 of 3",
                    "[instagram][error] HttpError: '401 Unauthorized' for 'https://www.instagram.com/api/'",
                ]
            ),
            PreviewError::SignInRequired { .. }
        ));
        assert!(matches!(
            classify_download_failure(
                Some(4),
                &["[instagram][error] HttpError: '429 Too Many Requests' for 'https://x'"]
            ),
            PreviewError::TooManyRequests { .. }
        ));
        assert_eq!(
            classify_download_failure(Some(64), &["[gallery-dl][error] Unsupported URL 'x'"]),
            PreviewError::UnsupportedSite
        );
        assert!(matches!(
            classify_download_failure(Some(1), &["Traceback (most recent call last):"]),
            PreviewError::Failed { .. }
        ));
    }

    #[test]
    fn instagram_signed_out_asks_for_a_sign_in_not_a_connection() {
        // What Instagram's API answers without a signed-in account, read from a preview's answer and a download's
        // error line.
        let answer = parse_answer(
            r#"[[-1, {"error": "KeyError", "message": "'xdt_api__v1__feed__user_timeline_graphql_connection'"}]]"#,
            "https://www.instagram.com/p/DfPQchkG1tg/",
        );
        assert!(
            matches!(answer, Err(PreviewError::SignInRequired { .. })),
            "{answer:?}"
        );
        assert!(matches!(
            classify_download_failure(
                Some(4),
                &["[instagram][error] KeyError: 'PolarisProfilePostsTabContentQuery_connection'"]
            ),
            PreviewError::SignInRequired { .. }
        ));

        // Other KeyErrors stay the site's, even when their key holds the word.
        for key in ["'data'", "'connection_id'", "'edge_connection_count'"] {
            assert!(
                matches!(
                    classify_message("KeyError", key),
                    PreviewError::SiteError { .. }
                ),
                "{key}"
            );
        }
        // A dropped connection is still one.
        assert!(matches!(
            classify_download_failure(
                Some(4),
                &["[instagram][error] HttpError: Connection aborted. ('Connection reset by peer')"]
            ),
            PreviewError::Network { .. }
        ));
    }

    #[test]
    fn an_unsupported_link_is_an_exit_code() {
        assert_eq!(
            classify_exit(
                Some(64),
                &["[gallery-dl][error] Unsupported URL 'https://example.com/page'"]
            ),
            PreviewError::UnsupportedSite
        );
        assert_eq!(
            classify_exit(Some(1), &["[gallery-dl][error] Something broke"]),
            PreviewError::Failed {
                detail: "Something broke".to_owned()
            }
        );
    }

    #[test]
    fn puts_the_link_after_the_options() {
        let tools = GalleryTools {
            gallery_dl: Some(Path::new("gallery-dl.exe")),
            cookies: Some("firefox".to_owned()),
            caches: None,
        };
        let args: Vec<String> = command(&tools, Path::new("gallery-dl.exe"), &[], "-x", None)
            .arguments()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "--config-ignore",
                "--dump-json",
                "-o",
                "extractor.twitter.previews=true",
                "--cookies-from-browser",
                "firefox",
                "--",
                "-x"
            ]
        );
        let args: Vec<String> = command(
            &tools,
            Path::new("gallery-dl.exe"),
            &["--post-range", "1-50"],
            "-x",
            None,
        )
        .arguments()
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
        assert_eq!(
            &args[args.len() - 4..],
            ["--post-range", "1-50", "--", "-x"]
        );
    }
}
