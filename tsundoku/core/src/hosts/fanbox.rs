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
    Address, Answer, FileCheck, FileSource, Host, HostFacts, RemoteFile, Share, probe, safe_name,
};
use crate::cancel::CancelToken;
use crate::fanbox_account;
use crate::metadata::{PreviewError, VideoInfo, VideoTag, tags_from};

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
}

/// A post read: its files as a share, and the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Post {
    pub share: Share,
    pub details: PostDetails,
}

impl Post {
    /// The post as the preview card and the queue know it: its files, with its creator, cover and tags.
    pub fn video_info(&self) -> VideoInfo {
        let mut video = self.share.video_info();
        video.uploader = self.details.name.clone();
        video.account = Some(self.details.creator.clone()).filter(|creator| !creator.is_empty());
        video.thumbnail_url = self.details.cover.clone();
        video.tags = self.details.tags.clone();
        video
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

/// Reads a Fanbox post's files.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    read_post(client, link, cancel).map(|post| post.share)
}

/// Reads a Fanbox post: its files, with each image's size asked of Fanbox's file server, and what it is.
pub fn read_post(client: &Client, link: &str, cancel: &CancelToken) -> Result<Post, PreviewError> {
    let id = post_id(link).ok_or_else(|| Host::Fanbox.link_not_recognized())?;
    let answer = ask(&format!("{POST_INFO}{id}"))?;
    let listed = listed_post(&answer)?;
    if listed.files.is_empty() {
        return Err(PreviewError::NoVideo {
            detail: "this Fanbox post has no images or files".to_owned(),
        });
    }
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
            url: format!(
                "https://www.fanbox.cc/@{}/posts/{}",
                listed.details.creator, listed.id
            ),
            title: listed.title,
            upload_date: listed.date,
            files,
        },
        details: listed.details,
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
    let api = API.get().ok_or_else(|| PreviewError::Failed {
        detail: "Fanbox can't be read without Tsundoku's window".to_owned(),
    })?;
    let answer = api
        .get(&[address.to_owned()])
        .map_err(|detail| PreviewError::Network { detail })?
        .pop()
        .ok_or_else(|| PreviewError::Network {
            detail: "Fanbox didn't answer".to_owned(),
        })?
        .map_err(|detail| PreviewError::Network { detail })?;
    answer_body(&answer)
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
    let date = text(post, "publishedDatetime").and_then(|date| {
        let day: String = date.chars().take(10).filter(char::is_ascii_digit).collect();
        (day.len() == 8).then_some(day)
    });
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
        details: PostDetails {
            name: post.get("user").and_then(|user| text(user, "name")),
            cover: text(post, "coverImageUrl"),
            tags: tags_from(tags.iter().map(|tag| VideoTag::named(tag))),
            creator,
        },
        id,
        date,
    })
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
                        let answer = match probe(client, Host::Fanbox, &files[index].url, cancel) {
                            Ok(Answer::File(found)) => Ok(found.size),
                            Ok(Answer::Page(page)) => Err(Host::Fanbox.page_error(&page)),
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
