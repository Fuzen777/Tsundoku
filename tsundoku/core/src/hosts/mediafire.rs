//! MediaFire: links to a file or a folder. Its public API (`/api/1.4`) gives a file's name, size and SHA-256 and a
//! folder's contents without signing in; the address a file downloads from is the Download button on its page.
//! MediaFire may put that page behind Cloudflare's browser check, which Tsundoku doesn't try to pass: the person opens
//! the page in the check window (`check`), and the Download button's address caught there, or copied from a browser
//! (`download<n>.mediafire.com/…`), is used as it is while it still leads to the file.

use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::{
    Address, Answer, FileCheck, FileSource, Host, HostFacts, Page, RemoteFile, Share, check_cancel,
    code_error, network_error, probe, safe_name, status_error,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "MediaFire",
    site: "mediafire",
    domains: &["mediafire.com"],
    sends_attachments: false,
    browser_agent: true,
    read,
    // Its download addresses are probed with their own pages in mind (`page_error` below); a download's page says
    // no more than its status.
    page_error: None,
};

const SITE: &str = "https://www.mediafire.com";
/// Files and folders asked for at once when listing a folder: the most the API gives.
const CHUNK_SIZE: u32 = 1000;

/// A file to download from MediaFire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaFireFile {
    pub quick_key: String,
    /// In lowercase hex, when MediaFire gives one (older files have an MD5 instead, which isn't checked).
    pub sha256: Option<String>,
    /// The link itself, when it was the Download button's own (`download<n>.mediafire.com/<token>/<key>/<name>`).
    pub direct: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MediaFireLink {
    /// A file's key, and the link when it's the Download button's own.
    File {
        key: String,
        direct: Option<String>,
    },
    Folder(String),
}

/// Reads a MediaFire link: the file's name, size and date, or every file in the folder and its subfolders.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    match parse_link(link).ok_or_else(|| Host::MediaFire.link_not_recognized())? {
        MediaFireLink::File {
            key: quick_key,
            direct,
        } => {
            let answer: FileInfoAnswer = api(
                client,
                "file/get_info.php",
                &[("quick_key", &quick_key)],
                cancel,
            )?;
            let info = answer.file_info;
            if info.password_protected.eq_ignore_ascii_case("yes") {
                return Err(PreviewError::PasswordProtected {
                    site: Host::MediaFire.name().to_owned(),
                });
            }
            Ok(Share {
                host: Host::MediaFire,
                id: quick_key,
                url: link.to_owned(),
                title: info.filename.clone(),
                upload_date: info.created.get(..10).map(str::to_owned),
                files: vec![info.into_remote(PathBuf::new(), direct)],
            })
        }
        MediaFireLink::Folder(folder_key) => {
            let answer: FolderInfoAnswer = api(
                client,
                "folder/get_info.php",
                &[("folder_key", &folder_key)],
                cancel,
            )?;
            let FolderInfo { name, created } = answer.folder_info;
            let mut files = Vec::new();
            let mut folders = vec![(folder_key.clone(), PathBuf::from(safe_name(&name)))];
            while let Some((key, path)) = folders.pop() {
                for file in folder_content::<ContentFile>(client, &key, "files", cancel)? {
                    files.push(file.into_remote(path.clone()));
                }
                for folder in folder_content::<ContentFolder>(client, &key, "folders", cancel)? {
                    let sub_path = path.join(safe_name(&folder.name));
                    folders.push((folder.folderkey, sub_path));
                }
            }
            files.sort_by(|first, second| first.path.cmp(&second.path));
            Ok(Share {
                host: Host::MediaFire,
                id: folder_key,
                url: link.to_owned(),
                title: name,
                upload_date: created.get(..10).map(str::to_owned),
                files,
            })
        }
    }
}

/// The address the file downloads from: the link itself while it's the Download button's and still leads to the file
/// (its token runs out), else the address behind the Download button on the file's page.
pub(super) fn address(
    client: &Client,
    file: &MediaFireFile,
    cancel: &CancelToken,
) -> Result<Address, PreviewError> {
    let check = file
        .sha256
        .clone()
        .map_or(FileCheck::None, FileCheck::Sha256);
    if let Some(direct) = &file.direct {
        // Once its token has run out, the link leads back to the file's page, which is asked instead.
        if let Ok(Answer::File(_)) = probe(client, direct, cancel) {
            return Ok(Address {
                url: direct.clone(),
                check,
            });
        }
    }
    check_cancel(cancel)?;
    let page = file_page(&file.quick_key);
    let response = client.get(&page).send().map_err(network_error)?;
    if !response.status().is_success() {
        return Err(page_error(&Page::read(response)));
    }
    let is_page = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_none_or(|value| value.starts_with("text/html"));
    // Some links lead straight to the file.
    if !is_page {
        return Ok(Address {
            url: response.url().to_string(),
            check,
        });
    }
    let page = Page::read(response);
    match download_button(&page.html) {
        Some(url) => Ok(Address { url, check }),
        None => Err(page_error(&page)),
    }
}

#[derive(Deserialize)]
struct FileInfoAnswer {
    file_info: FileInfo,
}

#[derive(Deserialize)]
struct FileInfo {
    quickkey: String,
    filename: String,
    #[serde(deserialize_with = "number_or_text")]
    size: u64,
    #[serde(default)]
    hash: String,
    /// "2024-05-01 12:34:56".
    #[serde(default)]
    created: String,
    #[serde(default)]
    password_protected: String,
}

impl FileInfo {
    fn into_remote(self, folder: PathBuf, direct: Option<String>) -> RemoteFile {
        RemoteFile {
            path: folder.join(safe_name(&self.filename)),
            size: self.size,
            source: FileSource::MediaFire(MediaFireFile {
                sha256: sha256(&self.hash),
                quick_key: self.quickkey,
                direct,
            }),
        }
    }
}

#[derive(Deserialize)]
struct FolderInfoAnswer {
    folder_info: FolderInfo,
}

#[derive(Deserialize)]
struct FolderInfo {
    name: String,
    /// "YYYY-MM-DD HH:MM:SS".
    #[serde(default)]
    created: String,
}

#[derive(Deserialize)]
struct FolderContentAnswer {
    folder_content: FolderContent,
}

#[derive(Deserialize)]
struct FolderContent {
    #[serde(default)]
    files: Vec<Value>,
    #[serde(default)]
    folders: Vec<Value>,
    #[serde(default)]
    more_chunks: String,
}

#[derive(Deserialize)]
struct ContentFile {
    quickkey: String,
    filename: String,
    #[serde(deserialize_with = "number_or_text")]
    size: u64,
    #[serde(default)]
    hash: String,
}

impl ContentFile {
    fn into_remote(self, folder: PathBuf) -> RemoteFile {
        FileInfo {
            quickkey: self.quickkey,
            filename: self.filename,
            size: self.size,
            hash: self.hash,
            created: String::new(),
            password_protected: String::new(),
        }
        .into_remote(folder, None)
    }
}

#[derive(Deserialize)]
struct ContentFolder {
    folderkey: String,
    name: String,
}

/// Every file or subfolder (`kind` "files" or "folders") directly in a folder, a chunk at a time.
fn folder_content<T: DeserializeOwned>(
    client: &Client,
    folder_key: &str,
    kind: &str,
    cancel: &CancelToken,
) -> Result<Vec<T>, PreviewError> {
    let mut items = Vec::new();
    for chunk in 1.. {
        let chunk = chunk.to_string();
        let chunk_size = CHUNK_SIZE.to_string();
        let answer: FolderContentAnswer = api(
            client,
            "folder/get_content.php",
            &[
                ("folder_key", folder_key),
                ("content_type", kind),
                ("chunk", &chunk),
                ("chunk_size", &chunk_size),
                ("filter", "public"),
            ],
            cancel,
        )?;
        let content = answer.folder_content;
        let listed = if kind == "files" {
            content.files
        } else {
            content.folders
        };
        for item in listed {
            items.push(serde_json::from_value(item).map_err(unexpected_answer)?);
        }
        if !content.more_chunks.eq_ignore_ascii_case("yes") {
            break;
        }
    }
    Ok(items)
}

/// Calls MediaFire's API and returns its `response`, or its error.
fn api<T: DeserializeOwned>(
    client: &Client,
    path: &str,
    parameters: &[(&str, &str)],
    cancel: &CancelToken,
) -> Result<T, PreviewError> {
    check_cancel(cancel)?;
    let url = reqwest::Url::parse_with_params(
        &format!("{SITE}/api/1.4/{path}"),
        parameters.iter().chain(&[("response_format", "json")]),
    )
    .map_err(|error| PreviewError::Failed {
        detail: error.to_string(),
    })?;
    let response = client.get(url).send().map_err(network_error)?;
    // MediaFire answers its errors with a JSON body too, which says more than the status.
    let status = status_error(Host::MediaFire, &response);
    let body: Value = match response.json() {
        Ok(body) => body,
        Err(error) => return Err(status.unwrap_or_else(|| unexpected_answer(error))),
    };
    let answer = body.get("response").cloned().unwrap_or(Value::Null);
    if answer.get("result").and_then(Value::as_str) != Some("Success") {
        return Err(api_error(&answer));
    }
    serde_json::from_value(answer).map_err(unexpected_answer)
}

/// MediaFire's API errors, sorted as a preview's errors are.
fn api_error(answer: &Value) -> PreviewError {
    let code = answer.get("error").and_then(|code| {
        code.as_i64()
            .or_else(|| code.as_str().and_then(|text| text.parse().ok()))
    });
    let message = answer
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("no message");
    let detail = match code {
        Some(code) => format!("MediaFire error {code}: {message}"),
        None => format!("MediaFire: {message}"),
    };
    match code {
        // Unknown or invalid key, removed, private, or taken down.
        Some(105 | 110 | 112 | 114 | 293) => PreviewError::Unavailable { detail },
        _ => PreviewError::Failed { detail },
    }
}

fn unexpected_answer(error: impl std::fmt::Display) -> PreviewError {
    PreviewError::Failed {
        detail: format!("MediaFire's answer wasn't as expected: {error}"),
    }
}

/// MediaFire gives sizes as text ("1234").
fn number_or_text<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    match Value::deserialize(deserializer)? {
        Value::Number(number) => number
            .as_u64()
            .ok_or_else(|| serde::de::Error::custom("not a size")),
        Value::String(text) => text.trim().parse().map_err(serde::de::Error::custom),
        _ => Err(serde::de::Error::custom("not a size")),
    }
}

/// The hash, if it's a SHA-256.
fn sha256(hash: &str) -> Option<String> {
    (hash.len() == 64 && hash.chars().all(|character| character.is_ascii_hexdigit()))
        .then(|| hash.to_ascii_lowercase())
}

/// Reads a MediaFire link: "mediafire.com/file/<key>/<name>/file" (or /file_premium/, /download/, /view/, /i/?<key>),
/// "mediafire.com/?<key>", "mediafire.com/folder/<key>/<name>", or the Download button's own link,
/// "download<n>.mediafire.com/<token>/<key>/<name>", which browser add-ons copy. Its token runs out, so the file is
/// read by its key, and the link is kept to download from while it still works.
fn parse_link(link: &str) -> Option<MediaFireLink> {
    let after_scheme = link.split_once("://").map_or(link, |(_, rest)| rest);
    let without_fragment = after_scheme.split('#').next().unwrap_or_default();
    let (address, query) = without_fragment
        .split_once('?')
        .unwrap_or((without_fragment, ""));
    let (host, path) = address.split_once('/').unwrap_or((address, ""));
    let segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let is_key = |text: &str| {
        !text.is_empty()
            && text
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
    };
    let is_download_server = host
        .to_ascii_lowercase()
        .strip_suffix(".mediafire.com")
        .and_then(|server| server.strip_prefix("download"))
        .is_some_and(|number| number.chars().all(|character| character.is_ascii_digit()));
    match segments.as_slice() {
        [_token, key, ..] if is_download_server && is_key(key) => Some(MediaFireLink::File {
            key: (*key).to_owned(),
            direct: Some(format!("https://{after_scheme}")),
        }),
        ["file" | "file_premium" | "download" | "view", key, ..] if is_key(key) => {
            Some(MediaFireLink::File {
                key: (*key).to_owned(),
                direct: None,
            })
        }
        ["folder", key, ..] if is_key(key) => Some(MediaFireLink::Folder((*key).to_owned())),
        [] | ["i"] if is_key(query) => Some(MediaFireLink::File {
            key: query.to_owned(),
            direct: None,
        }),
        _ => None,
    }
}

/// A file's page, with its Download button: what a person opens in the check window.
pub(super) fn file_page(quick_key: &str) -> String {
    format!("{SITE}/file/{quick_key}")
}

/// The file's key in the Download button's own link (`download<n>.mediafire.com/<token>/<key>/<name>`), for a link
/// caught in the check window or pasted. `None` for any other link.
pub(super) fn direct_link_key(link: &str) -> Option<String> {
    match parse_link(link)? {
        MediaFireLink::File {
            key,
            direct: Some(_),
        } => Some(key),
        _ => None,
    }
}

/// The address behind the page's Download button: its link, or the base64 one MediaFire sometimes scrambles it into.
fn download_button(html: &str) -> Option<String> {
    let id = html.find("id=\"downloadButton\"")?;
    let tag_start = html[..id].rfind('<')?;
    let tag_end = id + html[id..].find('>')?;
    let tag = &html[tag_start..tag_end];
    let scrambled = attribute(tag, "data-scrambled-url")
        .and_then(|value| STANDARD.decode(value.trim()).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok());
    scrambled
        .into_iter()
        .chain(attribute(tag, "href").map(|value| value.replace("&amp;", "&")))
        .find(|url| url.starts_with("https://") || url.starts_with("http://"))
}

/// An attribute's value in an HTML tag, quoted with double quotes.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!(" {name}=\"");
    let start = tag.find(&marker)? + marker.len();
    let length = tag[start..].find('"')?;
    Some(&tag[start..start + length])
}

/// Why a file's page has no Download button, or answered with an error.
fn page_error(page: &Page) -> PreviewError {
    if page.url.contains("error.php") || page.html.contains("Something appears to be missing") {
        PreviewError::Unavailable {
            detail: "MediaFire says the file was removed".to_owned(),
        }
    } else if super::check::is_browser_check(page) {
        // Cloudflare's check page, answered with 403 (MediaFire sends some files' pages to "?action=check" for it).
        PreviewError::BrowserCheck {
            site: Host::MediaFire.name().to_owned(),
        }
    } else if let Some(error) = code_error(Host::MediaFire, page.status) {
        error
    } else {
        PreviewError::Failed {
            detail: "MediaFire's page has no Download button".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(key: &str) -> MediaFireLink {
        MediaFireLink::File {
            key: key.to_owned(),
            direct: None,
        }
    }

    #[test]
    fn reads_file_and_folder_links() {
        for (link, expected) in [
            (
                "https://www.mediafire.com/file/x7k2j9q4zbbwm1n/Pack_v2.zip/file",
                Some(file("x7k2j9q4zbbwm1n")),
            ),
            (
                "https://www.mediafire.com/file_premium/x7k2j9q4zbbwm1n/Pack_v2.zip",
                Some(file("x7k2j9q4zbbwm1n")),
            ),
            (
                "https://www.mediafire.com/download/x7k2j9q4zbbwm1n",
                Some(file("x7k2j9q4zbbwm1n")),
            ),
            (
                "https://mediafire.com/?x7k2j9q4zbbwm1n",
                Some(file("x7k2j9q4zbbwm1n")),
            ),
            (
                "https://www.mediafire.com/folder/ab12cd34ef56g/My_Folder",
                Some(MediaFireLink::Folder("ab12cd34ef56g".to_owned())),
            ),
            (
                "https://download2390.mediafire.com/gf8b0u3kzm4gV2h1mR7wq0Xc_KdPj/x7k2j9q4zbbwm1n/Pack_v2.zip",
                Some(MediaFireLink::File {
                    key: "x7k2j9q4zbbwm1n".to_owned(),
                    direct: Some("https://download2390.mediafire.com/gf8b0u3kzm4gV2h1mR7wq0Xc_KdPj/x7k2j9q4zbbwm1n/Pack_v2.zip".to_owned()),
                }),
            ),
            (
                "http://DOWNLOAD1.MediaFire.com/abc/x7k2j9q4zbbwm1n/Pack%20v2.7z?a=1#top",
                Some(MediaFireLink::File {
                    key: "x7k2j9q4zbbwm1n".to_owned(),
                    direct: Some("https://DOWNLOAD1.MediaFire.com/abc/x7k2j9q4zbbwm1n/Pack%20v2.7z?a=1#top".to_owned()),
                }),
            ),
            ("https://download2390.mediafire.com/abc", None),
            (
                "https://www.mediafire.com/abc/x7k2j9q4zbbwm1n/Pack_v2.zip",
                None,
            ),
            ("https://www.mediafire.com/", None),
            ("https://www.mediafire.com/help/", None),
            ("https://www.mediafire.com/?sharekey=abc&x=1", None),
        ] {
            assert_eq!(parse_link(link), expected, "{link}");
        }
    }

    #[test]
    fn finds_the_download_button() {
        let page = r#"<div class="download_link"><a class="input popsok" aria-label="Download file"
            href="https://download2390.mediafire.com/abc/x7k2j9q4zbbwm1n/Pack_v2.zip?a=1&amp;b=2" id="downloadButton"
            rel="nofollow">Download (12.3MB)</a></div>"#;
        assert_eq!(
            download_button(page).as_deref(),
            Some("https://download2390.mediafire.com/abc/x7k2j9q4zbbwm1n/Pack_v2.zip?a=1&b=2")
        );

        let scrambled = format!(
            r#"<a class="input popsok" href="javascript:void(0)" data-scrambled-url="{}" id="downloadButton">"#,
            STANDARD.encode("https://download1.mediafire.com/x/file.7z")
        );
        assert_eq!(
            download_button(&scrambled).as_deref(),
            Some("https://download1.mediafire.com/x/file.7z")
        );

        assert_eq!(download_button("<html><body>No button</body></html>"), None);
    }

    fn page(url: &str, status: u16, html: &str) -> Page {
        Page {
            url: url.to_owned(),
            status: reqwest::StatusCode::from_u16(status).unwrap(),
            html: html.to_owned(),
            challenged: false,
        }
    }

    #[test]
    fn explains_a_page_without_a_button() {
        assert!(matches!(
            page_error(&page(
                "https://www.mediafire.com/error.php?errno=320",
                200,
                ""
            )),
            PreviewError::Unavailable { .. }
        ));
        // Some files' pages send a browser to "?action=check", which Cloudflare answers with 403 and its check.
        assert_eq!(
            page_error(&page(
                "https://www.mediafire.com/file/x?action=check",
                403,
                "<title>Just a moment...</title><script>window._cf_chl_opt = {};</script>"
            )),
            PreviewError::BrowserCheck {
                site: "MediaFire".to_owned()
            }
        );
        assert!(matches!(
            page_error(&page("https://www.mediafire.com/file/x", 403, "Forbidden")),
            PreviewError::Failed { detail } if detail == "MediaFire answered 403 Forbidden"
        ));
        // Every MediaFire page loads Cloudflare's challenge-platform script, check or not.
        assert!(matches!(
            page_error(&page(
                "https://www.mediafire.com/file/x",
                200,
                "/cdn-cgi/challenge-platform/scripts/jsd/main.js"
            )),
            PreviewError::Failed { detail } if detail.contains("no Download button")
        ));
    }

    #[test]
    fn reads_the_apis_answers() {
        let file: FileInfoAnswer = serde_json::from_str(
            r#"{"action": "file/get_info", "file_info": {"quickkey": "x7k2j9q4zbbwm1n", "filename": "Pack v2.zip",
            "created": "2024-05-01 12:34:56", "size": "12345678", "password_protected": "no",
            "hash": "0F343B0931126A20F133D67C2B018A3B1A7FCD4E1F7E6F1F4E4B9AB1E3F2C3D4"}, "result": "Success"}"#,
        )
        .unwrap();
        let remote = file.file_info.into_remote(PathBuf::from("Folder"), None);
        assert_eq!(remote.path, PathBuf::from("Folder").join("Pack v2.zip"));
        assert_eq!(remote.size, 12_345_678);
        assert_eq!(
            remote.source,
            FileSource::MediaFire(MediaFireFile {
                quick_key: "x7k2j9q4zbbwm1n".to_owned(),
                sha256: Some(
                    "0f343b0931126a20f133d67c2b018a3b1a7fcd4e1f7e6f1f4e4b9ab1e3f2c3d4".to_owned()
                ),
                direct: None,
            })
        );
        assert_eq!(
            sha256("d41d8cd98f00b204e9800998ecf8427e"),
            None,
            "an MD5 isn't checked"
        );

        assert!(matches!(
            api_error(&serde_json::json!({"result": "Error", "error": 110, "message": "Unknown or invalid QuickKey"})),
            PreviewError::Unavailable { detail } if detail == "MediaFire error 110: Unknown or invalid QuickKey"
        ));
        assert!(matches!(
            api_error(&serde_json::json!({"result": "Error", "error": "999", "message": "Odd"})),
            PreviewError::Failed { .. }
        ));
    }
}
