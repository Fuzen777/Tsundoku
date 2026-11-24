//! Patreon (patreon.com): a post's images, attachments and the pictures in its text, which Tsundoku downloads itself.
//! Patreon answers its API only to a browser (gallery-dl got 403 on Fuzen's PC, signed in or not, even posing as
//! Chrome), so a post is read through `PatreonApi`, which the app crate gives: a hidden window on the sign-in window's
//! profile, asking `/api/posts/<id>` from one of Patreon's own pages (`web_reader.rs`), as `fanbox` does. Its files are
//! on Patreon's file server (`patreonusercontent.com`), behind signed addresses that expire, so a download reads the
//! post again for fresh ones.
//!
//! What a post holds is read by `crate::patreon`. A post whose point is a video or an audio is yt-dlp's
//! (`preview::read_patreon`), and so is a creator's page, which this host leaves to the tools (`UnsupportedSite`).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

use reqwest::blocking::Client;
use serde_json::Value;

use super::{
    Address, Answer, ApiAnswer, FileCheck, FileSource, Host, HostFacts, RemoteFile, SendChecked,
    Share, check_cancel, probe, safe_name,
};
use crate::cancel::CancelToken;
use crate::metadata::{PreviewError, VideoInfo, VideoTag, tags_from};
use crate::patreon::{self, PostFacts};

pub(super) const FACTS: HostFacts = HostFacts {
    name: "Patreon",
    site: "patreon",
    domains: &["patreon.com"],
    sends_attachments: false,
    browser_agent: false,
    read,
    page_error: None,
    window_link: None,
    download_file_id: None,
    unlock: None,
};

/// The page of Patreon's own its API is asked from.
pub const API_PAGE: &str = "https://www.patreon.com/robots.txt";
/// Files whose sizes are asked at once while a post is read.
const PROBES_AT_ONCE: usize = 4;

/// Asks Patreon's API as its own pages do, from Tsundoku's window.
pub trait PatreonApi: Send + Sync {
    /// Each address's answer in turn, or why Patreon couldn't be asked.
    fn get(&self, addresses: &[String]) -> Result<Vec<Result<ApiAnswer, String>>, String>;
}

static API: OnceLock<Box<dyn PatreonApi>> = OnceLock::new();

/// Gives the reader Patreon's API is asked through, once, at startup.
pub fn start(api: Box<dyn PatreonApi>) {
    let _ = API.set(api);
}

/// A file of a post: where it downloads from, as the post gave it when it was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatreonFile {
    pub url: String,
}

/// Where a file downloads from: the address the post gave, which the download read moments before.
pub(super) fn address(file: &PatreonFile) -> Address {
    Address {
        url: file.url.clone(),
        check: FileCheck::None,
    }
}

/// Reads a post's files, for its download, which sends the links inside it to Pending. A creator's page, or any link
/// that isn't a post's, is the tools' (`UnsupportedSite`).
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    if patreon::post_id(link).is_none() {
        return Err(PreviewError::UnsupportedSite);
    }
    let (post, facts) = ask_post(link)?;
    if let Some(locked) = facts.locked {
        return Err(locked);
    }
    let share = share_of(client, &post, link, facts.media, cancel)?;
    crate::post_links::send(&facts.links);
    if share.files.is_empty() {
        return Err(patreon::nothing_of_its_own(facts.links.len()));
    }
    Ok(share)
}

/// Reads a post for the preview card (`preview::read_patreon`): the post as the card shows it, its files to pick from,
/// or why it has none to download (it's for paying members, or it has no images or files of its own), with its facts.
/// `Err` only when Patreon couldn't be read at all.
pub fn read_for_preview(
    link: &str,
    cancel: &CancelToken,
) -> Result<(Result<VideoInfo, PreviewError>, PostFacts), PreviewError> {
    let (post, facts) = ask_post(link)?;
    if let Some(locked) = &facts.locked {
        return Ok((Err(locked.clone()), facts));
    }
    let client = super::client_for(Host::Patreon)?;
    let video = share_of(&client, &post, link, facts.media, cancel).and_then(|share| {
        if share.files.is_empty() {
            Err(patreon::nothing_of_its_own(facts.links.len()))
        } else {
            Ok(video_info(&share, &post))
        }
    });
    Ok((video, facts))
}

/// A post as the preview card and the queue know it: its files, with its creator, picture and tags.
fn video_info(share: &Share, post: &Value) -> VideoInfo {
    let mut video = share.video_info();
    video.uploader = patreon::creator_name(post);
    video.account = patreon::creator_handle(post);
    video.thumbnail_url = post
        .get("image")
        .and_then(|image| image.get("large_url"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            share.files.iter().find_map(|file| match &file.source {
                FileSource::Patreon(found) if is_picture(&file.path) => Some(found.url.clone()),
                _ => None,
            })
        });
    video.tags = tags_from(
        post.get("tags")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(VideoTag::named),
    );
    video
}

fn is_picture(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["jpg", "jpeg", "png", "gif", "webp"].contains(&extension.to_ascii_lowercase().as_str())
        })
}

/// Asks Patreon's API for the post `link` names, and gives it (`patreon::post_from_api`) with its facts.
fn ask_post(link: &str) -> Result<(Value, PostFacts), PreviewError> {
    let id = patreon::post_id(link).ok_or_else(|| Host::Patreon.link_not_recognized())?;
    let api = API.get().ok_or_else(|| PreviewError::Failed {
        detail: "Patreon can't be read without Tsundoku's window".to_owned(),
    })?;
    let answer = api
        .get(&[patreon::api_address(&id)])
        .map_err(|detail| PreviewError::Failed {
            detail: format!("Patreon couldn't be asked: {detail}"),
        })?
        .pop()
        .ok_or_else(|| PreviewError::Failed {
            detail: "Patreon didn't answer".to_owned(),
        })?
        .map_err(|detail| PreviewError::Failed {
            detail: format!("Patreon couldn't be asked: {detail}"),
        })?;
    let post = patreon::post_from_api(&answer_json(&answer)?)?;
    let facts = patreon::facts(&post);
    Ok((post, facts))
}

/// An answer of Patreon's API as JSON, or what its status says went wrong.
fn answer_json(answer: &ApiAnswer) -> Result<Value, PreviewError> {
    match answer.status {
        200..=299 => {}
        404 => {
            return Err(PreviewError::Unavailable {
                detail: "Patreon has no such post".to_owned(),
            });
        }
        429 => {
            return Err(PreviewError::too_many_requests(format!(
                "Patreon answered {}",
                answer.status
            )));
        }
        status => {
            // Patreon's own refusal says why ("You do not have permission to view this post.", its 403 for a post it
            // shows neither as free nor as locked to a window not signed in, 2026-11-24); a page in its place, such as
            // Cloudflare's, doesn't.
            let reason = serde_json::from_str::<Value>(&answer.body)
                .ok()
                .and_then(|refusal| {
                    refusal
                        .pointer("/errors/0/title")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                });
            return Err(PreviewError::Failed {
                detail: match reason {
                    Some(reason) => format!("Patreon answered {status}: {reason}"),
                    None => format!("Patreon answered {status}"),
                },
            });
        }
    }
    serde_json::from_str(&answer.body).map_err(|error| PreviewError::Failed {
        detail: format!("Patreon's answer isn't JSON: {error}"),
    })
}

/// The files of a post Tsundoku downloads itself (`patreon::files`) as a share, each with its size asked of
/// Patreon's file server.
fn share_of(
    client: &Client,
    post: &Value,
    link: &str,
    media: bool,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let listed = patreon::files(post, media);
    let sizes = sizes(client, &listed, cancel)?;
    let id = post
        .get("id")
        .and_then(|id| {
            id.as_str()
                .map(str::to_owned)
                .or_else(|| id.as_u64().map(|id| id.to_string()))
        })
        .or_else(|| patreon::post_id(link))
        .unwrap_or_default();
    let date = post
        .get("published_at")
        .and_then(Value::as_str)
        .and_then(crate::metadata::day_of_timestamp);
    Ok(Share {
        host: Host::Patreon,
        url: post
            .get("url")
            .and_then(Value::as_str)
            .filter(|url| patreon::post_id(url).is_some())
            .map_or_else(|| link.to_owned(), str::to_owned),
        title: patreon::title(post).unwrap_or_else(|| format!("Patreon post {id}")),
        id,
        upload_date: date,
        files: listed
            .into_iter()
            .zip(sizes)
            .map(|(file, size)| RemoteFile {
                path: PathBuf::from(safe_name(&file.name)),
                size,
                source: FileSource::Patreon(PatreonFile { url: file.url }),
            })
            .collect(),
    })
}

/// Each file's size, as Patreon's file server answers a HEAD request for it, else as `probe` asks it, several at once.
/// A file whose size the server doesn't say is 0, unknown until it downloads (`FileSource::size_may_be_unknown`). A
/// file the server refuses fails the read, saying which.
fn sizes(
    client: &Client,
    files: &[patreon::PostFile],
    cancel: &CancelToken,
) -> Result<Vec<u64>, PreviewError> {
    let next = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let found: Vec<(usize, Result<u64, PreviewError>)> = thread::scope(|scope| {
        let workers: Vec<_> = (0..PROBES_AT_ONCE.min(files.len()))
            .map(|_| {
                scope.spawn(|| {
                    let mut answers = Vec::new();
                    while !failed.load(Ordering::Relaxed) {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(file) = files.get(index) else {
                            break;
                        };
                        let answer = size_of(client, file, cancel);
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
    let mut sizes = vec![0; files.len()];
    for (index, answer) in found {
        sizes[index] = answer?;
    }
    Ok(sizes)
}

/// A file's size, as its server answers a HEAD request, else as `probe` asks it; 0 when neither says.
fn size_of(
    client: &Client,
    file: &patreon::PostFile,
    cancel: &CancelToken,
) -> Result<u64, PreviewError> {
    check_cancel(cancel)?;
    let head = client.head(&file.url).send_checked(FACTS.name)?;
    let length = head
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok());
    if let Some(size) = length.filter(|size| head.status().is_success() && *size > 0) {
        return Ok(size);
    }
    match probe(client, Host::Patreon, &file.url, cancel)? {
        Answer::File(found) => Ok(found.size),
        Answer::Page(page) if page.status.is_success() => Ok(0),
        Answer::Page(page) => Err(PreviewError::Failed {
            detail: format!(
                "{}: Patreon's file server answered {} ({})",
                file.name,
                page.status,
                crate::sites::host(&page.url).unwrap_or_default()
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_patreons_answers() {
        let ok = ApiAnswer {
            status: 200,
            body: r#"{"data": {"id": "1"}}"#.to_owned(),
        };
        assert_eq!(answer_json(&ok).unwrap()["data"]["id"], "1");
        let missing = ApiAnswer {
            status: 404,
            body: String::new(),
        };
        assert!(matches!(
            answer_json(&missing),
            Err(PreviewError::Unavailable { .. })
        ));
        let refused = ApiAnswer {
            status: 403,
            body: "<html>Just a moment...</html>".to_owned(),
        };
        assert_eq!(
            answer_json(&refused),
            Err(PreviewError::Failed {
                detail: "Patreon answered 403".to_owned()
            })
        );
        // Patreon's answer for https://www.patreon.com/posts/custom-firefox-153639294 to a window not signed in.
        let forbidden = ApiAnswer {
            status: 403,
            body: r#"{"errors":[{"id":"22e5f7d5-7b34-4f70-a4fa-a8f7d9bf9af9","code_name":"ViewForbidden","code":6,"title":"You do not have permission to view this post.","detail":"You do not have permission to view post with id 153639294.","status":"403"}]}"#.to_owned(),
        };
        assert_eq!(
            answer_json(&forbidden),
            Err(PreviewError::Failed {
                detail: "Patreon answered 403: You do not have permission to view this post."
                    .to_owned()
            })
        );
    }

    #[test]
    fn leaves_a_creators_page_to_the_tools() {
        let client = super::super::client_for(Host::Patreon).unwrap();
        assert_eq!(
            read(
                &client,
                "https://www.patreon.com/kurzgesagt",
                &CancelToken::new()
            ),
            Err(PreviewError::UnsupportedSite)
        );
        assert_eq!(
            Host::of("https://www.patreon.com/posts/free-post-12497641"),
            Some(Host::Patreon)
        );
    }
}
