//! Asking gallery-dl what a link is (`gallery-dl -j`), for the image posts yt-dlp can't download, such as Instagram
//! photos and carousels. Its answer is read into the same `VideoInfo` as yt-dlp's, with `post` describing the files.
//! Tested against real answers saved under `tests/fixtures/gallery`.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use crate::cancel::CancelToken;
use crate::metadata::{PostContents, PreviewError, TOO_MANY_REQUESTS_PHRASES, VideoInfo};
use crate::process::{self, OutputLine, ProgramCommand, RunError};
use crate::tools::{ToolCacheFolder, ToolId, start_error};

/// Some sites take a while to answer; a fetch still running after this long is given up.
const FETCH_TIMEOUT: Duration = Duration::from_secs(2 * 60);

/// gallery-dl's exit code for a link none of its extractors accepts.
const UNSUPPORTED_EXIT_CODE: i32 = 64;

/// The longest title made from a post's caption.
const TITLE_LENGTH: usize = 120;

/// File extensions counted as videos. X gives its animated GIFs as MP4.
const VIDEO_EXTENSIONS: [&str; 6] = ["mp4", "webm", "mov", "m4v", "mkv", "m3u8"];

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
    let Some(gallery_dl) = tools.gallery_dl else {
        return Err(PreviewError::ImagesNotSetUp);
    };
    let output = process::run_collecting_with_cancel(
        &command(tools, gallery_dl, extra, link),
        cancel,
        timeout,
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

    if !output.status.success() {
        let stderr: Vec<&str> = output
            .lines
            .iter()
            .filter_map(|line| match line {
                OutputLine::Stderr(text) => Some(text.as_str()),
                OutputLine::Stdout(_) => None,
            })
            .collect();
        return Err(classify_exit(output.status.code(), &stderr));
    }
    Ok(output.stdout().collect())
}

fn command(tools: &GalleryTools, gallery_dl: &Path, extra: &[&str], link: &str) -> ProgramCommand {
    let mut command = ProgramCommand::new(gallery_dl).args([
        // gallery-dl's spelling of yt-dlp's --ignore-config: a config file of the user's can't change the answer.
        "--config-ignore",
        // Every file's details as JSON, without downloading.
        "--dump-json",
    ]);
    if let Some(cookies) = &tools.cookies {
        command = command.arg("--cookies-from-browser").arg(cookies);
    }
    if let Some(caches) = tools.caches {
        command = command.args(caches.gallery_dl_args());
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
    /// How many posts it has in all.
    pub post_count: Option<u32>,
}

/// The posts of gallery-dl's answer for an account's page, each as reading its own link would describe it, and the
/// account.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FoundPosts {
    pub posts: Vec<VideoInfo>,
    pub account: Option<AccountInfo>,
}

/// A post of gallery-dl's answer: its metadata (`[2, metadata]`, or its first file's when there's none) and its files.
struct PostMessages<'a> {
    post: &'a Value,
    files: Vec<(&'a str, &'a Value)>,
}

/// gallery-dl's answer, sorted into posts.
struct SortedAnswer<'a> {
    posts: Vec<PostMessages<'a>>,
    /// `[6, url, metadata]` messages: links to other pages, such as a profile's posts.
    links_elsewhere: usize,
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
    let messages = read_messages(json)?;
    let answer = sort_answer(&messages)?;
    let account = answer
        .posts
        .first()
        .and_then(|first| first.post.get("user"))
        .filter(|user| user.is_object())
        .map(|user| AccountInfo {
            username: text(user, "username"),
            full_name: text(user, "full_name"),
            picture: text(user, "profile_pic_url"),
            post_count: user
                .get("media_count")
                .and_then(Value::as_u64)
                .and_then(|count| u32::try_from(count).ok()),
        });
    let posts = answer
        .posts
        .iter()
        .map(|post| video_of(post, link))
        .collect::<Result<_, _>>()?;
    Ok(FoundPosts { posts, account })
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
                    match posts.last_mut() {
                        Some(last) => last.files.push((url, &parts[2])),
                        None => posts.push(PostMessages {
                            post: &parts[2],
                            files: vec![(url, &parts[2])],
                        }),
                    }
                }
            }
            (Some(6), _) => links_elsewhere += 1,
            _ => {}
        }
    }
    posts.retain(|post| !post.files.is_empty());
    Ok(SortedAnswer {
        posts,
        links_elsewhere,
    })
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

    let videos = files.iter().filter(|(_, file)| is_video(file)).count();
    let contents = PostContents {
        images: count(files.len() - videos),
        videos: count(videos),
    };
    let uploader = uploader(post);
    let title = caption_title(post)
        .or_else(|| uploader.as_ref().map(|name| format!("Post by {name}")))
        .unwrap_or_else(|| post_id.clone());

    Ok(VideoInfo {
        site_name: site_name(link).unwrap_or_else(|| site.clone()),
        url: text(post, "post_url").unwrap_or_else(|| link.to_owned()),
        thumbnail_url: thumbnail(files),
        upload_date: text(post, "date")
            .and_then(|date| date.get(..10).map(str::to_owned))
            .filter(|date| date.as_bytes().get(4) == Some(&b'-')),
        site,
        video_id: post_id,
        title,
        uploader,
        duration_seconds: None,
        best_height: None,
        post: Some(contents),
        files: None,
        post_files: post_files(files),
    })
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
    ) || mentions(&[
        "login page",
        "log in",
        "logged in",
        "cookies needed",
        "sign in",
    ]) {
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
    } else if mentions(&["timed out", "connection"]) {
        PreviewError::Network { detail }
    } else {
        // The site answered in a way gallery-dl didn't expect, which an update of gallery-dl often fixes.
        PreviewError::SiteError {
            tool: ToolId::GalleryDl,
            detail,
        }
    }
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

/// The person who posted: their display name, or their account name.
fn uploader(post: &Value) -> Option<String> {
    let author = post.get("author").filter(|author| author.is_object());
    text(post, "fullname")
        .or_else(|| author.and_then(|author| text(author, "nick")))
        .or_else(|| author.and_then(|author| text(author, "name")))
        .or_else(|| text(post, "username"))
        .or_else(|| text(post, "author"))
}

/// The caption's first line, shortened, as the post's title.
fn caption_title(post: &Value) -> Option<String> {
    let caption = text(post, "description").or_else(|| text(post, "content"))?;
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

/// The first image, or a video's poster (Instagram gives one as `display_url`).
fn thumbnail(files: &[(&str, &Value)]) -> Option<String> {
    files.iter().find_map(|(url, file)| {
        if !is_video(file) {
            return Some((*url).to_owned());
        }
        text(file, "display_url")
    })
}

/// The post's files for the strip, numbered from 1 in the answer's order, which is the order gallery-dl's `--range`
/// counts in when it downloads them, all picked.
fn post_files(files: &[(&str, &Value)]) -> Vec<crate::metadata::PostFile> {
    files
        .iter()
        .zip(1..)
        .map(|((url, file), number)| {
            let video = is_video(file);
            crate::metadata::PostFile {
                number,
                video,
                thumbnail_url: text(file, "display_url")
                    .or_else(|| (!video).then(|| small_image(url)))
                    .filter(|url| url.starts_with("https://")),
                picked: true,
            }
        })
        .collect()
}

/// An image's address, asking X for its small size rather than the original, which can be 4K.
fn small_image(url: &str) -> String {
    if url.starts_with("https://pbs.twimg.com/") {
        url.replace("name=orig", "name=small")
    } else {
        url.to_owned()
    }
}

fn is_video(file: &Value) -> bool {
    text(file, "extension").is_some_and(|extension| {
        VIDEO_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
    })
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
        assert_eq!(video.post_files.len(), 1);
        assert_eq!(
            video.post_files[0].thumbnail_url, None,
            "X gives no cover for a video"
        );

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
                videos: 4
            })
        );
        assert!(post.thumbnail_url.is_some());
        assert!(post.uploader.is_some());
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
                videos: 1
            })
        );
        // The reel's poster.
        assert!(post.thumbnail_url.is_some());
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
        assert_eq!(
            images.post,
            Some(PostContents {
                images: 4,
                videos: 0
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
                videos: 1
            })
        );
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
                videos: 0
            })
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
        assert_eq!(post.upload_date.as_deref(), Some("2026-11-02"));
        assert_eq!(
            post.thumbnail_url.as_deref(),
            Some("https://cdn.example/1.jpg")
        );
        assert_eq!(
            post.post,
            Some(PostContents {
                images: 2,
                videos: 1
            })
        );
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
        let args: Vec<String> = command(&tools, Path::new("gallery-dl.exe"), &[], "-x")
            .arguments()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "--config-ignore",
                "--dump-json",
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
