//! Fanbox (fanbox.cc): a creator's post, its images and files. Fanbox answers its API only to a browser, so a post is
//! read through `FanboxApi`, which the app crate gives: a hidden window on the sign-in window's profile, signed in to
//! Fanbox there once (`fanbox_account`), asking `post.info` as Fanbox's own pages do (`web_reader.rs`). Its files are on
//! `downloads.fanbox.cc`, which answers Tsundoku's own client as it answers a browser, so they download as any host's.
//!
//! A post (`https://<creator>.fanbox.cc/posts/<id>`, `https://www.fanbox.cc/@<creator>/posts/<id>`, or Pixiv's old
//! `https://www.pixiv.net/fanbox/creator/<user>/post/<id>`) is a share of its images, in the post's order and named by
//! their place in it, then its files under their own names. A post for a plan the account doesn't support has nothing
//! to download (`PreviewError::FanboxLocked`, with the plan's fee). gallery-dl's Fanbox code (GPL) was read for the
//! protocol only.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

use reqwest::Url;
use reqwest::blocking::Client;
use serde_json::Value;

use super::{
    Address, Answer, FileCheck, FileSource, Host, HostFacts, RemoteFile, SendChecked, Share,
    check_cancel, probe, safe_name,
};
use crate::cancel::CancelToken;
use crate::fanbox_account;
use crate::metadata::{FanboxDetails, FileContents, PreviewError, VideoInfo, VideoTag, tags_from};
use crate::sites;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "Fanbox",
    site: "fanbox",
    domains: &["fanbox.cc"],
    sends_attachments: false,
    browser_agent: false,
    read,
    page_error: None,
    window_link: None,
    download_file_id: None,
    unlock: None,
};

/// The page of Fanbox's own its API is asked from.
pub const API_PAGE: &str = "https://www.fanbox.cc/robots.txt";
const POST_INFO: &str = "https://api.fanbox.cc/post.info?postId=";
/// Images whose sizes are asked at once while a post is read.
const PROBES_AT_ONCE: usize = 4;

/// Fanbox's answer to one of its API's addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiAnswer {
    pub status: u16,
    pub body: String,
}

/// Asks Fanbox's API as its own pages do, from the sign-in window's session.
pub trait FanboxApi: Send + Sync {
    /// Each address's answer in turn, or why Fanbox couldn't be asked.
    fn get(&self, addresses: &[String]) -> Result<Vec<Result<ApiAnswer, String>>, String>;

    /// The session's `SESSION_COOKIE`, which Fanbox's file server asks for a supported post's files, or `None` when
    /// the window isn't signed in.
    fn session(&self) -> Result<Option<String>, String> {
        Ok(None)
    }
}

/// The cookie Fanbox keeps its session in, which its file server (`downloads.fanbox.cc`) asks for the files of a post
/// for supporters: without it, they answer 403.
pub const SESSION_COOKIE: &str = "FANBOXSESSID";
/// How long the session's cookie is kept in memory before the window is asked for it again.
const SESSION_KEPT: std::time::Duration = std::time::Duration::from_secs(5 * 60);

static SESSION: std::sync::Mutex<Option<(String, std::time::Instant)>> =
    std::sync::Mutex::new(None);

/// The `Cookie` header that gives Fanbox's file server the signed-in session, for the Fanbox host's client
/// (`hosts::client_for`), which only ever asks Fanbox's own servers. `None` when the window isn't signed in. The value
/// is only ever sent there, and never written anywhere.
pub(crate) fn session_header() -> Option<String> {
    let mut kept = SESSION
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((value, at)) = kept.as_ref()
        && at.elapsed() < SESSION_KEPT
    {
        return Some(format!("{SESSION_COOKIE}={value}"));
    }
    let value = API.get()?.session().ok().flatten()?;
    *kept = Some((value.clone(), std::time::Instant::now()));
    Some(format!("{SESSION_COOKIE}={value}"))
}

static API: OnceLock<Box<dyn FanboxApi>> = OnceLock::new();

/// Gives the reader Fanbox's API is asked through, once, at startup.
pub fn start(api: Box<dyn FanboxApi>) {
    let _ = API.set(api);
}

/// A file of a post: where it downloads from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FanboxFile {
    pub url: String,
}

/// What a post is, besides its files, for the preview card and the rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostDetails {
    pub creator: String,
    /// The creator's name, as Fanbox shows it.
    pub name: Option<String>,
    pub cover: Option<String>,
    pub tags: Vec<VideoTag>,
    /// The least plan that opens it, a month, in yen.
    pub fee: u32,
}

/// A post read: its files as a share, and the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Post {
    pub share: Share,
    pub details: PostDetails,
    /// The links inside it (`post_links`), which go to Pending when it's downloaded (Fuzen's choice).
    pub links: Vec<String>,
}

/// Where the links inside a post go: Pending, read as links a page sent (the app crate's `PendingLinks::add_from_page`).
type LinkSink = Box<dyn Fn(Vec<String>) + Send + Sync>;

static POST_LINKS: OnceLock<LinkSink> = OnceLock::new();

/// Gives where the links inside a downloaded post go, once, at startup.
pub fn on_post_links(sink: LinkSink) {
    let _ = POST_LINKS.set(sink);
}

/// Sends a post's links to Pending, if it has any.
fn send_links(post: &Post) {
    if let (false, Some(sink)) = (post.links.is_empty(), POST_LINKS.get()) {
        sink(post.links.clone());
    }
}

impl Post {
    /// The post as the preview card and the queue know it: its files, with its creator, cover and tags.
    pub fn video_info(&self) -> VideoInfo {
        let mut video = self.share.video_info();
        video.uploader = self.details.name.clone();
        video.account = Some(self.details.creator.clone()).filter(|creator| !creator.is_empty());
        video.thumbnail_url = self.details.cover.clone();
        video.tags = self.details.tags.clone();
        video.fanbox = Some(FanboxDetails {
            fee: self.details.fee,
            locked: false,
        });
        video
    }
}

/// A post's own page, as Fanbox writes it.
pub fn post_page(creator: &str, id: &str) -> String {
    format!("https://www.fanbox.cc/@{creator}/posts/{id}")
}

/// A post as a list of Fanbox's gives it (a creator's posts, the home and supporting feeds): what a download of it
/// starts with. Its files are read when its download starts, so how many there are is unknown until then (0). A post
/// the account can't open is `locked`, with its plan's fee.
pub(crate) fn listed_video(item: &Value) -> Option<VideoInfo> {
    let id = text(item, "id")?;
    let creator = text(item, "creatorId").unwrap_or_default();
    let tags: Vec<String> = item
        .get("tags")
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let fee = item
        .get("feeRequired")
        .and_then(Value::as_u64)
        .and_then(|fee| u32::try_from(fee).ok())
        .unwrap_or(0);
    let title = text(item, "title").unwrap_or_else(|| format!("Fanbox post {id}"));
    Some(VideoInfo {
        site: FACTS.site.to_owned(),
        site_name: FACTS.domains[0].to_owned(),
        url: post_page(&creator, &id),
        title,
        uploader: item.get("user").and_then(|user| text(user, "name")),
        account: Some(creator).filter(|creator| !creator.is_empty()),
        duration_seconds: None,
        upload_date: text(item, "publishedDatetime").and_then(|date| day_of(&date)),
        thumbnail_url: item
            .get("cover")
            .and_then(|cover| text(cover, "url"))
            .or_else(|| text(item, "coverImageUrl")),
        best_height: None,
        post: None,
        files: Some(FileContents { count: 0, bytes: 0 }),
        post_files: Vec::new(),
        shared_files: Vec::new(),
        playlist: None,
        tags: tags_from(tags.iter().map(|tag| VideoTag::named(tag))),
        pixiv: None,
        fanbox: Some(FanboxDetails {
            fee,
            locked: item.get("isRestricted").and_then(Value::as_bool) == Some(true),
        }),
        video_id: id,
    })
}

/// A date Fanbox gives ("2026-11-20T17:30:00+09:00") as YYYYMMDD, the day where it was posted.
fn day_of(date: &str) -> Option<String> {
    let day: String = date.chars().take(10).filter(char::is_ascii_digit).collect();
    (day.len() == 8).then_some(day)
}

/// Asks Fanbox's API for each address in turn, 1 to 2 seconds apart, and gives each answer's `body`, or the first
/// failure.
pub(crate) fn ask_all(addresses: &[String]) -> Result<Vec<Value>, PreviewError> {
    let api = API.get().ok_or_else(no_window)?;
    api.get(addresses)
        .map_err(|detail| PreviewError::Network { detail })?
        .into_iter()
        .map(|answer| {
            answer
                .map_err(|detail| PreviewError::Network { detail })
                .and_then(|answer| answer_body(&answer))
        })
        .collect()
}

fn no_window() -> PreviewError {
    PreviewError::Failed {
        detail: "Fanbox can't be read without Tsundoku's window".to_owned(),
    }
}

/// The post a link names, by its ID.
pub fn post_id(link: &str) -> Option<String> {
    let url = Url::parse(link).ok()?;
    let host = url.host_str()?;
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect();
    let id = match (host, segments.as_slice()) {
        ("www.fanbox.cc", [creator, "posts", id]) if creator.starts_with('@') => *id,
        ("www.pixiv.net", ["fanbox", "creator", _, "post", id]) => *id,
        (host, ["posts", id])
            if host.ends_with(".fanbox.cc")
                && host != "www.fanbox.cc"
                && host != "api.fanbox.cc" =>
        {
            *id
        }
        _ => return None,
    };
    (!id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit())).then(|| id.to_owned())
}

/// Reads a Fanbox post's files, for its download, which sends the links inside it to Pending.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let post = read_post(client, link, cancel)?;
    send_links(&post);
    with_files(post).map(|post| post.share)
}

/// Reads a Fanbox post for the preview card. A post with nothing of its own to download but links sends them to
/// Pending at once, since there's nothing to download it with.
pub fn read_for_preview(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Post, PreviewError> {
    let post = read_post(client, link, cancel)?;
    if post.share.files.is_empty() {
        send_links(&post);
    }
    with_files(post)
}

/// The post, or `NoVideo` when it has no images or files, saying where its links went.
fn with_files(post: Post) -> Result<Post, PreviewError> {
    if !post.share.files.is_empty() {
        return Ok(post);
    }
    let detail = match post.links.len() {
        0 => "this Fanbox post has no images or files".to_owned(),
        1 => "this Fanbox post has no images or files of its own; its link went to Pending"
            .to_owned(),
        count => format!(
            "this Fanbox post has no images or files of its own; its {count} links went to Pending"
        ),
    };
    Err(PreviewError::NoVideo { detail })
}

/// Reads a Fanbox post: its files, with each image's size asked of Fanbox's file server, and what it is.
pub fn read_post(client: &Client, link: &str, cancel: &CancelToken) -> Result<Post, PreviewError> {
    let id = post_id(link).ok_or_else(|| Host::Fanbox.link_not_recognized())?;
    let answer = ask(&format!("{POST_INFO}{id}"))?;
    let listed = listed_post(&answer)?;
    let sizes = sizes(client, &listed.files, cancel)?;
    let files = listed
        .files
        .into_iter()
        .zip(sizes)
        .map(|(file, size)| RemoteFile {
            path: PathBuf::from(file.name),
            size,
            source: FileSource::Fanbox(FanboxFile { url: file.url }),
        })
        .collect();
    Ok(Post {
        share: Share {
            host: Host::Fanbox,
            id: listed.id.clone(),
            url: post_page(&listed.details.creator, &listed.id),
            title: listed.title,
            upload_date: listed.date,
            files,
        },
        details: listed.details,
        links: listed.links,
    })
}

/// Where a file downloads from: the address the post gave, which doesn't expire.
pub(super) fn address(file: &FanboxFile) -> Address {
    Address {
        url: file.url.clone(),
        check: FileCheck::None,
    }
}

/// Asks Fanbox's API for one address, and gives its answer's `body`.
fn ask(address: &str) -> Result<Value, PreviewError> {
    ask_all(&[address.to_owned()])?
        .pop()
        .ok_or_else(|| PreviewError::Network {
            detail: "Fanbox didn't answer".to_owned(),
        })
}

/// The `body` of an answer of Fanbox's API, or what its status says went wrong.
fn answer_body(answer: &ApiAnswer) -> Result<Value, PreviewError> {
    match answer.status {
        200..=299 => {}
        401 => {
            // The window's session is signed out.
            let refused = fanbox_account::signed_in().is_some();
            fanbox_account::forget();
            return Err(PreviewError::FanboxSignInNeeded { refused });
        }
        404 => {
            return Err(PreviewError::Unavailable {
                detail: "Fanbox has no such post".to_owned(),
            });
        }
        429 => {
            return Err(PreviewError::too_many_requests(format!(
                "Fanbox answered {}",
                answer.status
            )));
        }
        status => {
            return Err(PreviewError::Failed {
                detail: format!("Fanbox answered {status}"),
            });
        }
    }
    let json: Value = serde_json::from_str(&answer.body).map_err(|error| PreviewError::Failed {
        detail: format!("Fanbox's answer isn't JSON: {error}"),
    })?;
    json.get("body")
        .cloned()
        .ok_or_else(|| PreviewError::Failed {
            detail: "Fanbox's answer has no body".to_owned(),
        })
}

/// A post as `post.info` gives it, before its images' sizes are known.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Listed {
    id: String,
    title: String,
    /// As YYYYMMDD.
    date: Option<String>,
    files: Vec<ListedFile>,
    details: PostDetails,
    links: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ListedFile {
    name: String,
    url: String,
    /// Fanbox gives a file's size, not an image's.
    size: Option<u64>,
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// A post from `post.info`'s body: a locked one is `FanboxLocked`, with its plan's fee.
fn listed_post(body: &Value) -> Result<Listed, PreviewError> {
    let post = body.get("post").unwrap_or(body);
    let id = text(post, "id").ok_or_else(|| PreviewError::Failed {
        detail: "Fanbox's post has no ID".to_owned(),
    })?;
    let creator = text(post, "creatorId").unwrap_or_default();
    let content = post.get("body").filter(|content| content.is_object());
    if post.get("isRestricted").and_then(Value::as_bool) == Some(true) || content.is_none() {
        return Err(PreviewError::FanboxLocked {
            fee: post
                .get("feeRequired")
                .and_then(Value::as_u64)
                .and_then(|fee| u32::try_from(fee).ok())
                .unwrap_or(0),
        });
    }
    let content = content.unwrap_or(&Value::Null);
    let date = text(post, "publishedDatetime").and_then(|date| day_of(&date));
    let fee = post
        .get("feeRequired")
        .and_then(Value::as_u64)
        .and_then(|fee| u32::try_from(fee).ok())
        .unwrap_or(0);
    let tags: Vec<String> = post
        .get("tags")
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Ok(Listed {
        title: text(post, "title").unwrap_or_else(|| format!("Fanbox post {id}")),
        files: post_files(&id, content),
        links: post_links(&id, content),
        details: PostDetails {
            name: post.get("user").and_then(|user| text(user, "name")),
            cover: text(post, "coverImageUrl"),
            tags: tags_from(tags.iter().map(|tag| VideoTag::named(tag))),
            creator,
            fee,
        },
        id,
        date,
    })
}

/// The links inside a post, in its order and each once, that something may download: an article's links, the pages it
/// embeds (a YouTube video, a tweet, another Fanbox post) and the links written in its text. Links to a channel's,
/// an account's or a creator's page are left out (a creator's own profiles, which every post may repeat), and so is
/// the post itself.
fn post_links(id: &str, content: &Value) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut written = String::new();
    for block in content
        .get("blocks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(line) = block.get("text").and_then(Value::as_str) {
            written.push_str(line);
            written.push('\n');
        }
        for link in block
            .get("links")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            found.extend(link.get("url").and_then(Value::as_str).map(str::to_owned));
        }
    }
    if let Some(body_text) = content.get("text").and_then(Value::as_str) {
        written.push_str(body_text);
    }
    // A link ending a sentence leaves its full stop or colon out.
    found.extend(
        crate::pending::links_in_copy(&written)
            .into_iter()
            .map(|link| link.trim_end_matches(['.', ':']).to_owned()),
    );
    let embeds = content
        .get("embedMap")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|map| map.values())
        .chain(content.get("video"));
    found.extend(embeds.filter_map(embed_link));
    for embed in content
        .get("urlEmbedMap")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|map| map.values())
    {
        let link = match text(embed, "type").as_deref() {
            Some("fanbox.post") => embed
                .get("postInfo")
                .and_then(|post| Some(post_page(&text(post, "creatorId")?, &text(post, "id")?))),
            _ => text(embed, "url"),
        };
        found.extend(link);
    }
    let mut links: Vec<String> = Vec::new();
    for link in found {
        let Some(link) = crate::metadata::normalize_link(&link) else {
            continue;
        };
        let is_this_post = post_id(&link).as_deref() == Some(id);
        if !is_this_post && crate::listing::listing_link(&link).is_none() && !links.contains(&link)
        {
            links.push(link);
        }
    }
    links
}

/// The page an embed shows (`serviceProvider` and its ID), when Tsundoku can read it.
fn embed_link(embed: &Value) -> Option<String> {
    let id = text(embed, "videoId").or_else(|| text(embed, "contentId"))?;
    match text(embed, "serviceProvider")?.as_str() {
        "youtube" => Some(format!("https://www.youtube.com/watch?v={id}")),
        "vimeo" => Some(format!("https://vimeo.com/{id}")),
        "soundcloud" => Some(format!("https://soundcloud.com/{id}")),
        "twitter" => Some(format!("https://x.com/i/status/{id}")),
        _ => None,
    }
}

/// A post's images, in the post's order, named by their place in it, then its files under their own names.
fn post_files(id: &str, content: &Value) -> Vec<ListedFile> {
    let in_order = |list: &str, map: &str, block_key: &str| -> Vec<Value> {
        if let Some(items) = content.get(list).and_then(Value::as_array) {
            return items.clone();
        }
        let Some(map) = content.get(map).and_then(Value::as_object) else {
            return Vec::new();
        };
        // An article's images and files follow its blocks.
        let mut ordered: Vec<Value> = content
            .get("blocks")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|block| block.get(block_key).and_then(Value::as_str))
            .filter_map(|key| map.get(key).cloned())
            .collect();
        for item in map.values() {
            if !ordered.contains(item) {
                ordered.push(item.clone());
            }
        }
        ordered
    };
    let images = in_order("images", "imageMap", "imageId");
    let count = images.len();
    let digits = count.to_string().len().max(2);
    let mut files: Vec<ListedFile> = images
        .iter()
        .enumerate()
        .filter_map(|(index, image)| {
            let url = text(image, "originalUrl")?;
            let extension = text(image, "extension").unwrap_or_else(|| "jpeg".to_owned());
            Some(ListedFile {
                name: safe_name(&format!("{id} {:0digits$}.{extension}", index + 1)),
                url,
                size: None,
            })
        })
        .collect();
    files.extend(
        in_order("files", "fileMap", "fileId")
            .iter()
            .filter_map(|file| {
                let url = text(file, "url")?;
                let name = text(file, "name").unwrap_or_else(|| "file".to_owned());
                let name = match text(file, "extension") {
                    Some(extension)
                        if !name
                            .to_lowercase()
                            .ends_with(&format!(".{}", extension.to_lowercase())) =>
                    {
                        format!("{name}.{extension}")
                    }
                    _ => name,
                };
                Some(ListedFile {
                    name: safe_name(&name),
                    url,
                    size: file.get("size").and_then(Value::as_u64),
                })
            }),
    );
    files
}

/// An image's size, as its file server answers a HEAD request for it, else as `probe` asks it. Some of a supported
/// post's images say no size at all, answering with the whole image, sent in pieces: their size is 0, unknown until
/// they download (`FileSource::size_may_be_unknown`).
fn size_of(client: &Client, url: &str, cancel: &CancelToken) -> Result<Answer, PreviewError> {
    check_cancel(cancel)?;
    let head = client.head(url).send_checked(FACTS.name)?;
    let length = head
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok());
    let unknown = || {
        Answer::File(super::Found {
            name: None,
            size: 0,
            modified: None,
        })
    };
    match length {
        Some(size) if head.status().is_success() && size > 0 => Ok(Answer::File(super::Found {
            name: None,
            size,
            modified: None,
        })),
        _ => match probe(client, Host::Fanbox, url, cancel)? {
            Answer::Page(page) if page.status.is_success() => Ok(unknown()),
            answer => Ok(answer),
        },
    }
}

/// Each file's size: Fanbox's own for a file, asked of its file server for an image, several at once.
fn sizes(
    client: &Client,
    files: &[ListedFile],
    cancel: &CancelToken,
) -> Result<Vec<u64>, PreviewError> {
    let unknown: Vec<usize> = (0..files.len())
        .filter(|index| files[*index].size.is_none())
        .collect();
    let next = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let found: Vec<(usize, Result<u64, PreviewError>)> = thread::scope(|scope| {
        let workers: Vec<_> = (0..PROBES_AT_ONCE.min(unknown.len()))
            .map(|_| {
                scope.spawn(|| {
                    let mut answers = Vec::new();
                    while !failed.load(Ordering::Relaxed) {
                        let Some(&index) = unknown.get(next.fetch_add(1, Ordering::Relaxed)) else {
                            break;
                        };
                        let answer = match size_of(client, &files[index].url, cancel) {
                            Ok(Answer::File(found)) => Ok(found.size),
                            Ok(Answer::Page(page)) => Err(match Host::Fanbox.page_error(&page) {
                                PreviewError::Failed { .. } => PreviewError::Failed {
                                    detail: format!(
                                        "{}: Fanbox's file server answered {} ({})",
                                        files[index].name,
                                        page.status,
                                        sites::host(&page.url).unwrap_or_default()
                                    ),
                                },
                                other => other,
                            }),
                            Err(error) => Err(error),
                        };
                        if answer.is_err() {
                            failed.store(true, Ordering::Relaxed);
                        }
                        answers.push((index, answer));
                    }
                    answers
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap_or_default())
            .collect()
    });
    let mut sizes: Vec<Option<u64>> = files.iter().map(|file| file.size).collect();
    let mut first_error: Option<(usize, PreviewError)> = None;
    for (index, answer) in found {
        match answer {
            Ok(size) => sizes[index] = Some(size),
            Err(error) => {
                if first_error.as_ref().is_none_or(|(first, _)| index < *first) {
                    first_error = Some((index, error));
                }
            }
        }
    }
    if let Some((_, error)) = first_error {
        return Err(error);
    }
    sizes
        .into_iter()
        .map(|size| size.ok_or(PreviewError::Cancelled))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_posts_id_from_each_kind_of_link() {
        for link in [
            "https://someone.fanbox.cc/posts/12961757",
            "https://www.fanbox.cc/@someone/posts/12961757",
            "https://www.pixiv.net/fanbox/creator/20390859/post/12961757",
            "https://someone.fanbox.cc/posts/12961757?utm_source=x",
        ] {
            assert_eq!(post_id(link).as_deref(), Some("12961757"), "{link}");
        }
        for link in [
            "https://someone.fanbox.cc/",
            "https://www.fanbox.cc/@someone",
            "https://www.fanbox.cc/home/supporting",
            "https://api.fanbox.cc/posts/1",
            "https://someone.fanbox.cc/posts/abc",
        ] {
            assert_eq!(post_id(link), None, "{link}");
        }
    }

    fn article() -> Value {
        serde_json::json!({"post": {
            "id": "12961757",
            "title": "A Post",
            "creatorId": "someone",
            "type": "article",
            "feeRequired": 0,
            "isRestricted": false,
            "publishedDatetime": "2026-11-20T17:30:00+09:00",
            "coverImageUrl": "https://pixiv.pximg.net/c/1200x630_90_a2_g5/fanbox/public/images/post/12961757/cover/x.jpeg",
            "tags": ["#tag", "Other"],
            "user": {"userId": "1", "name": "Some One", "iconUrl": "https://pixiv.pximg.net/x.jpeg"},
            "body": {
                "blocks": [
                    {"type": "p", "text": "Hello"},
                    {"type": "image", "imageId": "second"},
                    {"type": "file", "fileId": "zip"},
                    {"type": "image", "imageId": "first"}
                ],
                "imageMap": {
                    "first": {"id": "first", "extension": "png", "originalUrl": "https://downloads.fanbox.cc/images/post/12961757/first.png"},
                    "second": {"id": "second", "extension": "jpeg", "originalUrl": "https://downloads.fanbox.cc/images/post/12961757/second.jpeg"}
                },
                "fileMap": {
                    "zip": {"id": "zip", "name": "Extras", "extension": "zip", "size": 1234, "url": "https://downloads.fanbox.cc/files/post/12961757/zip.zip"}
                },
                "embedMap": {}, "urlEmbedMap": {}
            }
        }})
    }

    #[test]
    fn lists_an_articles_images_in_its_order_then_its_files() {
        let listed = listed_post(&article()).unwrap();
        assert_eq!(listed.title, "A Post");
        assert_eq!(listed.date.as_deref(), Some("20261120"));
        let files: Vec<(&str, Option<u64>)> = listed
            .files
            .iter()
            .map(|file| (file.name.as_str(), file.size))
            .collect();
        assert_eq!(
            files,
            [
                ("12961757 01.jpeg", None),
                ("12961757 02.png", None),
                ("Extras.zip", Some(1234))
            ]
        );
        assert_eq!(listed.details.creator, "someone");
        assert_eq!(listed.details.name.as_deref(), Some("Some One"));
        assert_eq!(
            listed
                .details
                .tags
                .iter()
                .map(|tag| tag.name.as_str())
                .collect::<Vec<_>>(),
            ["tag", "Other"]
        );
    }

    #[test]
    fn lists_an_image_posts_images() {
        let body = serde_json::json!({"post": {
            "id": "5", "title": "Pictures", "creatorId": "someone", "type": "image", "isRestricted": false,
            "feeRequired": 0,
            "body": {"text": "", "images": [
                {"id": "a", "extension": "jpeg", "originalUrl": "https://downloads.fanbox.cc/images/post/5/a.jpeg"},
                {"id": "b", "extension": "gif", "originalUrl": "https://downloads.fanbox.cc/images/post/5/b.gif"}
            ]}
        }});
        let names: Vec<String> = listed_post(&body)
            .unwrap()
            .files
            .into_iter()
            .map(|file| file.name)
            .collect();
        assert_eq!(names, ["5 01.jpeg", "5 02.gif"]);
    }

    #[test]
    fn finds_the_links_inside_a_post() {
        let content = serde_json::json!({
            "blocks": [
                {"type": "p", "text": "Files: https://mega.nz/file/abc#key and https://example.com/page."},
                {"type": "p", "text": "My pixiv", "links": [
                    {"offset": 0, "length": 8, "url": "https://www.pixiv.net/users/123"},
                    {"offset": 0, "length": 8, "url": "https://drive.google.com/file/d/xyz/view"}
                ]},
                {"type": "p", "text": "Again: https://mega.nz/file/abc#key"}
            ],
            "embedMap": {"e1": {"id": "e1", "serviceProvider": "youtube", "contentId": "jNQXAC9IVRw"}},
            "urlEmbedMap": {
                "u1": {"id": "u1", "type": "default", "url": "https://someone.fanbox.cc/"},
                "u2": {"id": "u2", "type": "fanbox.post", "postInfo": {"id": "99", "creatorId": "someone"}},
                "u3": {"id": "u3", "type": "fanbox.post", "postInfo": {"id": "7", "creatorId": "someone"}}
            }
        });
        assert_eq!(
            post_links("7", &content),
            [
                "https://drive.google.com/file/d/xyz/view",
                "https://mega.nz/file/abc#key",
                "https://example.com/page",
                "https://www.youtube.com/watch?v=jNQXAC9IVRw",
                "https://www.fanbox.cc/@someone/posts/99",
            ],
            "a profile and the post itself are left out, and the MEGA link is there once"
        );
    }

    #[test]
    fn says_a_locked_posts_plan() {
        let body = serde_json::json!({"post": {
            "id": "7", "title": "For Supporters", "creatorId": "someone", "type": "image",
            "isRestricted": true, "feeRequired": 500, "body": null
        }});
        assert_eq!(
            listed_post(&body),
            Err(PreviewError::FanboxLocked { fee: 500 })
        );
    }

    #[test]
    fn tells_what_fanboxs_status_means() {
        let answer = |status: u16| ApiAnswer {
            status,
            body: r#"{"body": {"post": {}}}"#.to_owned(),
        };
        assert!(answer_body(&answer(200)).is_ok());
        assert!(matches!(
            answer_body(&answer(401)),
            Err(PreviewError::FanboxSignInNeeded { .. })
        ));
        assert!(matches!(
            answer_body(&answer(404)),
            Err(PreviewError::Unavailable { .. })
        ));
        assert!(matches!(
            answer_body(&answer(429)),
            Err(PreviewError::TooManyRequests { .. })
        ));
    }
}
