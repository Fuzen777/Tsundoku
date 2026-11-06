//! MediaFire: links to a file or a folder. Its public API (`/api/1.4`) gives a file's name, size and SHA-256 and a
//! folder's contents without signing in; the address a file downloads from is the Download button on its page.

use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::{
    Address, FileCheck, FileSource, Host, RemoteFile, Share, check_cancel, network_error,
    safe_name, status_error,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

const SITE: &str = "https://www.mediafire.com";
/// Files and folders asked for at once when listing a folder: the most the API gives.
const CHUNK_SIZE: u32 = 1000;

/// A file to download from MediaFire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaFireFile {
    pub quick_key: String,
    /// In lowercase hex, when MediaFire gives one (older files have an MD5 instead, which isn't checked).
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MediaFireLink {
    File(String),
    Folder(String),
}

/// Reads a MediaFire link: the file's name, size and date, or every file in the folder and its subfolders.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    match parse_link(link).ok_or(PreviewError::UnsupportedSite)? {
        MediaFireLink::File(quick_key) => {
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
                files: vec![info.into_remote(PathBuf::new())],
            })
        }
        MediaFireLink::Folder(folder_key) => {
            let answer: FolderInfoAnswer = api(
                client,
                "folder/get_info.php",
                &[("folder_key", &folder_key)],
                cancel,
            )?;
            let name = answer.folder_info.name;
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
                upload_date: None,
                files,
            })
        }
    }
}

/// Reads the file's page for the address behind its Download button.
pub(super) fn address(
    client: &Client,
    file: &MediaFireFile,
    cancel: &CancelToken,
) -> Result<Address, PreviewError> {
    check_cancel(cancel)?;
    let page = format!("{SITE}/file/{}", file.quick_key);
    let response = client.get(&page).send().map_err(network_error)?;
    if let Some(error) = status_error(Host::MediaFire, &response) {
        return Err(error);
    }
    let check = file
        .sha256
        .clone()
        .map_or(FileCheck::None, FileCheck::Sha256);
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
    let final_url = response.url().to_string();
    let html = response.text().map_err(network_error)?;
    match download_button(&html) {
        Some(url) => Ok(Address { url, check }),
        None => Err(page_error(&final_url, &html)),
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
    fn into_remote(self, folder: PathBuf) -> RemoteFile {
        RemoteFile {
            path: folder.join(safe_name(&self.filename)),
            size: self.size,
            source: FileSource::MediaFire(MediaFireFile {
                sha256: sha256(&self.hash),
                quick_key: self.quickkey,
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
        .into_remote(folder)
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
/// "mediafire.com/?<key>", or "mediafire.com/folder/<key>/<name>".
fn parse_link(link: &str) -> Option<MediaFireLink> {
    let after_scheme = link.split_once("://").map_or(link, |(_, rest)| rest);
    let without_fragment = after_scheme.split('#').next().unwrap_or_default();
    let (address, query) = without_fragment
        .split_once('?')
        .unwrap_or((without_fragment, ""));
    let path = address.split_once('/').map_or("", |(_, path)| path);
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
    match segments.as_slice() {
        ["file" | "file_premium" | "download" | "view", key, ..] if is_key(key) => {
            Some(MediaFireLink::File((*key).to_owned()))
        }
        ["folder", key, ..] if is_key(key) => Some(MediaFireLink::Folder((*key).to_owned())),
        [] | ["i"] if is_key(query) => Some(MediaFireLink::File(query.to_owned())),
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

/// Why a file's page has no Download button.
fn page_error(final_url: &str, html: &str) -> PreviewError {
    if final_url.contains("error.php") || html.contains("Something appears to be missing") {
        PreviewError::Unavailable {
            detail: "MediaFire says the file was removed".to_owned(),
        }
    } else if html.contains("challenge-platform") || html.contains("Just a moment...") {
        PreviewError::Failed {
            detail: "MediaFire asked for a browser check before the download".to_owned(),
        }
    } else {
        PreviewError::Failed {
            detail: "MediaFire's page has no Download button".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_file_and_folder_links() {
        for (link, expected) in [
            (
                "https://www.mediafire.com/file/x7k2j9q4zbbwm1n/Pack_v2.zip/file",
                Some(MediaFireLink::File("x7k2j9q4zbbwm1n".to_owned())),
            ),
            (
                "https://www.mediafire.com/file_premium/x7k2j9q4zbbwm1n/Pack_v2.zip",
                Some(MediaFireLink::File("x7k2j9q4zbbwm1n".to_owned())),
            ),
            (
                "https://www.mediafire.com/download/x7k2j9q4zbbwm1n",
                Some(MediaFireLink::File("x7k2j9q4zbbwm1n".to_owned())),
            ),
            (
                "https://mediafire.com/?x7k2j9q4zbbwm1n",
                Some(MediaFireLink::File("x7k2j9q4zbbwm1n".to_owned())),
            ),
            (
                "https://www.mediafire.com/folder/ab12cd34ef56g/My_Folder",
                Some(MediaFireLink::Folder("ab12cd34ef56g".to_owned())),
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

    #[test]
    fn explains_a_page_without_a_button() {
        assert!(matches!(
            page_error("https://www.mediafire.com/error.php?errno=320", ""),
            PreviewError::Unavailable { .. }
        ));
        assert!(matches!(
            page_error("https://www.mediafire.com/file/x", "<title>Just a moment...</title>"),
            PreviewError::Failed { detail } if detail.contains("browser check")
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
        let remote = file.file_info.into_remote(PathBuf::from("Folder"));
        assert_eq!(remote.path, PathBuf::from("Folder").join("Pack v2.zip"));
        assert_eq!(remote.size, 12_345_678);
        assert_eq!(
            remote.source,
            FileSource::MediaFire(MediaFireFile {
                quick_key: "x7k2j9q4zbbwm1n".to_owned(),
                sha256: Some(
                    "0f343b0931126a20f133d67c2b018a3b1a7fcd4e1f7e6f1f4e4b9ab1e3f2c3d4".to_owned()
                ),
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
