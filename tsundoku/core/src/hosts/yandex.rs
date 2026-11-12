//! Yandex Disk: public links to a file or a folder ("disk.yandex.ru/d/<key>", "/i/<key>", "yadi.sk/d/<key>", on any of
//! Yandex's country domains), read through Yandex Disk's public resources API without signing in. A folder's contents
//! come a page at a time, each file with its size and SHA-256, and each subfolder is listed by its path inside the
//! link. A file's download address (`/resources/download`) expires, so it's asked when the download starts.
//!
//! A single video or audio file is declined (`UnsupportedSite`), so yt-dlp reads it with the presets as it did before
//! this host existed. When Yandex answers a download with its captcha, the file waits for a person to open its page in
//! the check window (`hosts::check`).

use std::path::PathBuf;

use reqwest::Url;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use super::{
    Address, FileCheck, FileSource, Host, HostFacts, Page, RemoteFile, SendChecked, Share,
    check_cancel, code_error, safe_name,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "Yandex Disk",
    // yt-dlp's name for it, so what it downloaded from Yandex Disk before still counts.
    site: "yandexdisk",
    domains: &[
        "disk.yandex.ru",
        "disk.yandex.com",
        "disk.yandex.by",
        "disk.yandex.kz",
        "disk.yandex.ua",
        "disk.yandex.com.tr",
        "yadi.sk",
    ],
    // A page instead of the file is its captcha, or an error.
    sends_attachments: true,
    // It answers Tsundoku saying who it is as it answers a browser.
    browser_agent: false,
    read,
    page_error: Some(page_error),
    window: None,
};

const API: &str = "https://cloud-api.yandex.net/v1/disk/public/resources";
/// Items asked for at once when listing a folder.
const PAGE_SIZE: u32 = 200;
/// What yt-dlp reads, with the presets, when the link is to one such file.
const LEFT_TO_TOOLS: [&str; 2] = ["video", "audio"];

/// A file to download from Yandex Disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YandexFile {
    /// The public link, as the API takes it.
    pub public_key: String,
    /// Its path inside the link, "/" for the shared file itself.
    pub path: String,
    pub name: String,
    pub sha256: Option<String>,
    /// The address of its download caught in the check window, after Yandex's captcha.
    pub direct: Option<String>,
}

impl YandexFile {
    /// The file's page on Yandex Disk, for the check window.
    pub(crate) fn page(&self) -> String {
        if self.path == "/" {
            self.public_key.clone()
        } else {
            format!("{}{}", self.public_key.trim_end_matches('/'), self.path)
        }
    }
}

/// Reads a Yandex Disk link: the file's name, size and date, or every file in the folder and its subfolders.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let (public_key, inner) =
        parse_link(link).ok_or_else(|| Host::YandexDisk.link_not_recognized())?;
    let root: Resource = get(client, &public_key, &inner, 0, cancel)?;
    let mut files = Vec::new();
    if root.kind == "dir" {
        let top = PathBuf::from(safe_name(&root.name));
        let mut pending = vec![(root.path.clone(), top)];
        while let Some((path, folder)) = pending.pop() {
            for item in list(client, &public_key, &path, cancel)? {
                let item_path = folder.join(safe_name(&item.name));
                if item.kind == "dir" {
                    pending.push((item.path.clone(), item_path));
                } else {
                    files.push(item.remote(&public_key, item_path));
                }
            }
        }
        files.sort_by(|first, second| first.path.cmp(&second.path));
    } else {
        if root
            .media_type
            .as_deref()
            .is_some_and(|kind| LEFT_TO_TOOLS.contains(&kind))
        {
            return Err(PreviewError::UnsupportedSite);
        }
        files.push(root.remote(&public_key, PathBuf::from(safe_name(&root.name))));
    }
    if files.is_empty() {
        return Err(PreviewError::NoVideo {
            detail: "the Yandex Disk folder is empty".to_owned(),
        });
    }
    Ok(Share {
        host: Host::YandexDisk,
        id: link_id(&public_key),
        url: link.to_owned(),
        upload_date: root
            .created
            .as_deref()
            .and_then(|date| date.get(..10))
            .map(str::to_owned),
        title: root.name,
        files,
    })
}

/// Asks Yandex Disk where a file downloads from now, or uses the address caught past its captcha.
pub(super) fn address(
    client: &Client,
    file: &YandexFile,
    cancel: &CancelToken,
) -> Result<Address, PreviewError> {
    let check = file
        .sha256
        .clone()
        .map_or(FileCheck::None, FileCheck::Sha256);
    if let Some(direct) = &file.direct {
        return Ok(Address {
            url: direct.clone(),
            check,
        });
    }
    check_cancel(cancel)?;
    let url = Url::parse_with_params(
        &format!("{API}/download"),
        [
            ("public_key", file.public_key.as_str()),
            ("path", file.path.as_str()),
        ],
    )
    .map_err(|error| PreviewError::Failed {
        detail: error.to_string(),
    })?;
    let answer: DownloadAnswer = answer(client.get(url).send_checked(FACTS.name)?)?;
    Ok(Address {
        url: answer.href,
        check,
    })
}

/// The file a download address caught in the check window gets: Yandex's downloader names it in `filename`.
pub(crate) fn caught_file(download: &str) -> Option<String> {
    let url = Url::parse(download).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !(host.starts_with("downloader.disk.yandex.") || host.ends_with(".storage.yandex.net")) {
        return None;
    }
    url.query_pairs()
        .find(|(key, _)| key == "filename")
        .map(|(_, value)| value.into_owned())
}

/// Why a download answered with a page: Yandex's captcha ("showcaptcha"), which a person passes in the check window.
fn page_error(page: &Page) -> PreviewError {
    if page.url.contains("showcaptcha")
        || page.html.contains("smart-captcha")
        || page.html.contains("showcaptcha")
    {
        PreviewError::BrowserCheck {
            site: Host::YandexDisk.name().to_owned(),
        }
    } else {
        code_error(Host::YandexDisk, page.status).unwrap_or_else(|| PreviewError::Failed {
            detail: "Yandex Disk answered with a page instead of the file".to_owned(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct Resource {
    #[serde(default)]
    name: String,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    size: u64,
    /// "2019-07-04T03:03:37+00:00".
    #[serde(default)]
    created: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    media_type: Option<String>,
    #[serde(rename = "_embedded", default)]
    embedded: Option<Listing>,
}

impl Resource {
    fn remote(&self, public_key: &str, path: PathBuf) -> RemoteFile {
        RemoteFile {
            path,
            size: self.size,
            source: FileSource::YandexDisk(YandexFile {
                public_key: public_key.to_owned(),
                path: if self.path.is_empty() {
                    "/".to_owned()
                } else {
                    self.path.clone()
                },
                name: self.name.clone(),
                sha256: self
                    .sha256
                    .as_deref()
                    .filter(|hash| {
                        hash.len() == 64
                            && hash.chars().all(|character| character.is_ascii_hexdigit())
                    })
                    .map(str::to_ascii_lowercase),
                direct: None,
            }),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct Listing {
    #[serde(default)]
    items: Vec<Resource>,
    #[serde(default)]
    total: u64,
}

#[derive(Deserialize)]
struct DownloadAnswer {
    href: String,
}

#[derive(Deserialize)]
struct ErrorAnswer {
    #[serde(default)]
    error: String,
    #[serde(default)]
    description: String,
}

/// One resource of the link, with a page of its contents from `offset` when it's a folder.
fn get<T: DeserializeOwned>(
    client: &Client,
    public_key: &str,
    path: &str,
    offset: u64,
    cancel: &CancelToken,
) -> Result<T, PreviewError> {
    check_cancel(cancel)?;
    let offset = offset.to_string();
    let limit = PAGE_SIZE.to_string();
    let mut parameters = vec![
        ("public_key", public_key),
        ("limit", &limit),
        ("offset", &offset),
    ];
    if !path.is_empty() && path != "/" {
        parameters.push(("path", path));
    }
    let url = Url::parse_with_params(API, parameters).map_err(|error| PreviewError::Failed {
        detail: error.to_string(),
    })?;
    answer(client.get(url).send_checked(FACTS.name)?)
}

/// Everything directly in a folder of the link, a page at a time.
fn list(
    client: &Client,
    public_key: &str,
    path: &str,
    cancel: &CancelToken,
) -> Result<Vec<Resource>, PreviewError> {
    let mut items = Vec::new();
    loop {
        let folder: Resource = get(client, public_key, path, items.len() as u64, cancel)?;
        let listing = folder.embedded.unwrap_or_default();
        let got = listing.items.len();
        items.extend(listing.items);
        if got == 0 || items.len() as u64 >= listing.total {
            return Ok(items);
        }
    }
}

/// The API's answer, or its error: `DiskNotFoundError` for a link that's gone, `DiskResourceDownloadLimitExceededError`
/// for one whose downloads are used up for now.
fn answer<T: DeserializeOwned>(response: reqwest::blocking::Response) -> Result<T, PreviewError> {
    let status = response.status();
    if status.is_success() {
        return response.json().map_err(|error| PreviewError::Failed {
            detail: format!("Yandex Disk's answer wasn't as expected: {error}"),
        });
    }
    let error: ErrorAnswer = response.json().unwrap_or(ErrorAnswer {
        error: String::new(),
        description: String::new(),
    });
    Err(api_error(status, &error.error, &error.description))
}

fn api_error(status: reqwest::StatusCode, code: &str, description: &str) -> PreviewError {
    let detail = format!("Yandex Disk answered {status}: {code} ({description})");
    match code {
        "DiskNotFoundError" | "DiskResourceNotFoundError" => PreviewError::Unavailable { detail },
        "DiskResourceDownloadLimitExceededError" => PreviewError::LimitReached {
            site: Host::YandexDisk.name().to_owned(),
            detail,
        },
        _ if status.as_u16() == 429 => PreviewError::LimitReached {
            site: Host::YandexDisk.name().to_owned(),
            detail,
        },
        _ => code_error(Host::YandexDisk, status).unwrap_or(PreviewError::Failed { detail }),
    }
}

/// The link's key, the history's ID: what follows /d/ or /i/.
fn link_id(public_key: &str) -> String {
    public_key
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(public_key)
        .to_owned()
}

/// Reads a Yandex Disk link into the public link the API takes and the path inside it: "…/d/<key>" or "…/i/<key>",
/// with what follows the key in a /d/ link as the path ("/d/<key>/Sub/File.pdf").
fn parse_link(link: &str) -> Option<(String, String)> {
    let url = Url::parse(link).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !Host::YandexDisk.owns(&host) {
        return None;
    }
    let segments: Vec<String> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .map(super::percent_decode)
        .collect();
    let is_key = |key: &str| {
        key.len() >= 6
            && key.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            })
    };
    match segments.as_slice() {
        [kind, key, rest @ ..] if matches!(kind.as_str(), "d" | "i") && is_key(key) => {
            let public_key = format!("https://{host}/{kind}/{key}");
            let path = if rest.is_empty() {
                "/".to_owned()
            } else {
                format!("/{}", rest.join("/"))
            };
            Some((public_key, path))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_its_links() {
        for (link, expected) in [
            (
                "https://yadi.sk/d/lEw8uRm140L_eQ",
                Some(("https://yadi.sk/d/lEw8uRm140L_eQ", "/")),
            ),
            (
                "https://disk.yandex.ru/i/_7_4AJNX3Y9UNA",
                Some(("https://disk.yandex.ru/i/_7_4AJNX3Y9UNA", "/")),
            ),
            (
                "https://disk.yandex.ru/d/92lTwgcTnrqteg/%D1%82%D0%B5%D1%81%D1%82",
                Some(("https://disk.yandex.ru/d/92lTwgcTnrqteg", "/тест")),
            ),
            (
                "https://disk.yandex.com/d/abcdef123?w=1",
                Some(("https://disk.yandex.com/d/abcdef123", "/")),
            ),
            ("https://disk.yandex.ru/client/disk", None),
            ("https://yandex.ru/d/abcdef123", None),
        ] {
            assert_eq!(
                parse_link(link),
                expected.map(|(key, path)| (key.to_owned(), path.to_owned())),
                "{link}"
            );
        }
        assert_eq!(
            Host::of("https://yadi.sk/d/lEw8uRm140L_eQ"),
            Some(Host::YandexDisk)
        );
        assert_eq!(
            link_id("https://yadi.sk/d/lEw8uRm140L_eQ"),
            "lEw8uRm140L_eQ"
        );
    }

    #[test]
    fn reads_a_folder_page_and_its_errors() {
        // Yandex Disk's answer for a folder, trimmed.
        let folder: Resource = serde_json::from_str(
            r#"{"name": "first-order-model", "type": "dir", "path": "/", "created": "2019-07-04T03:03:37+00:00",
            "_embedded": {"total": 9, "items": [
                {"name": "bair-cpk.pth.tar", "type": "file", "path": "/bair-cpk.pth.tar", "size": 750804537,
                 "media_type": "compressed",
                 "sha256": "AB0F343B0931126A20F133D67C2B018A3B1A7FCD4E1F7E6F1F4E4B9AB1E3F2C3"},
                {"name": "Материалы курса", "type": "dir", "path": "/Материалы курса"}]}}"#,
        )
        .unwrap();
        let listing = folder.embedded.unwrap();
        assert_eq!(listing.total, 9);
        let file = listing.items[0].remote(
            "https://yadi.sk/d/lEw8uRm140L_eQ",
            PathBuf::from("bair-cpk.pth.tar"),
        );
        let FileSource::YandexDisk(source) = &file.source else {
            panic!("a Yandex Disk file");
        };
        assert_eq!(source.path, "/bair-cpk.pth.tar");
        assert_eq!(
            source.page(),
            "https://yadi.sk/d/lEw8uRm140L_eQ/bair-cpk.pth.tar"
        );
        assert_eq!(
            source.sha256.as_deref(),
            Some("ab0f343b0931126a20f133d67c2b018a3b1a7fcd4e1f7e6f1f4e4b9ab1e3f2c3")
        );
        assert_eq!(listing.items[1].kind, "dir");

        let status = reqwest::StatusCode::NOT_FOUND;
        assert!(matches!(
            api_error(status, "DiskNotFoundError", "Resource not found."),
            PreviewError::Unavailable { .. }
        ));
        assert!(matches!(
            api_error(reqwest::StatusCode::FORBIDDEN, "DiskResourceDownloadLimitExceededError", "limit"),
            PreviewError::LimitReached { site, .. } if site == "Yandex Disk"
        ));
    }

    #[test]
    fn its_captcha_is_a_check_and_the_download_past_it_is_caught() {
        let page = Page {
            url: "https://disk.yandex.ru/showcaptcha?cc=1&retpath=x".to_owned(),
            status: reqwest::StatusCode::OK,
            html: "<html>captcha</html>".to_owned(),
            challenged: false,
        };
        assert_eq!(
            page_error(&page),
            PreviewError::BrowserCheck {
                site: "Yandex Disk".to_owned()
            }
        );
        assert_eq!(
            caught_file(
                "https://downloader.disk.yandex.ru/disk/abc/def?uid=0&filename=bair-cpk.pth.tar&disposition=attachment"
            )
            .as_deref(),
            Some("bair-cpk.pth.tar")
        );
        assert_eq!(
            caught_file("https://ads.example.com/?filename=bair-cpk.pth.tar"),
            None
        );
    }
}
