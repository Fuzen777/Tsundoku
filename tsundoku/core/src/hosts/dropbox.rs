//! Dropbox: links to a file (/s/, /scl/fi/) or a shared folder (/sh/, /scl/fo/), read without signing in. A file
//! downloads from its link with dl=1, which leads to dl.dropboxusercontent.com; asking for its first byte gives its
//! name and size. A folder's contents come from the list Dropbox's own page asks for
//! (`list_shared_link_folder_entries`), a page of entries at a time: each file with its size and its own link, each
//! subfolder with the hash to list it by. Links made since 2023 (/scl/) only work with their `rlkey`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::blocking::Client;
use reqwest::header::{CONTENT_TYPE, COOKIE};
use reqwest::{StatusCode, Url};
use serde::Deserialize;
use serde_json::Value;

use super::{
    Answer, FileSource, Host, HostFacts, Page, RemoteFile, SendChecked, Share, check_cancel,
    code_error, form, percent_decode, probe, safe_name, title_of,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;
use crate::sites;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "Dropbox",
    // yt-dlp's name for it, so what it downloaded from Dropbox before still counts.
    site: "dropbox",
    domains: &["dropbox.com", "dropboxusercontent.com"],
    sends_attachments: true,
    browser_agent: false,
    read,
    page_error: Some(page_error),
    window_link: None,
    download_file_id: None,
    unlock: None,
};

const SITE: &str = "https://www.dropbox.com";

/// A file to download from Dropbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropboxFile {
    /// Its own link, with dl=1.
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    File,
    Folder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DropboxLink {
    kind: Kind,
    /// The link's key, the history's ID.
    key: String,
    /// Whether it's an old link (/s/, /sh/), which needs no `rlkey`.
    old: bool,
    has_rlkey: bool,
}

/// Reads a Dropbox link: the file's name and size, or every file in the folder and its subfolders.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let parsed = parse_link(link).ok_or_else(|| Host::Dropbox.link_not_recognized())?;
    if !parsed.old && !parsed.has_rlkey {
        return Err(PreviewError::LinkIncomplete {
            site: Host::Dropbox.name().to_owned(),
        });
    }
    if parsed.kind == Kind::Folder {
        return read_folder(client, link, parsed, cancel);
    }
    let url = download_link(link).ok_or_else(|| Host::Dropbox.link_not_recognized())?;
    let found = match probe(client, Host::Dropbox, &url, cancel)? {
        Answer::File(found) => found,
        Answer::Page(page) => return Err(page_error(&page)),
    };
    let title = found.name.unwrap_or_else(|| parsed.key.clone());
    Ok(Share {
        host: Host::Dropbox,
        id: parsed.key,
        url: link.to_owned(),
        upload_date: found.modified,
        files: vec![RemoteFile {
            path: PathBuf::from(safe_name(&title)),
            size: found.size,
            source: FileSource::Dropbox(DropboxFile { url }),
        }],
        title,
    })
}

fn read_folder(
    client: &Client,
    link: &str,
    parsed: DropboxLink,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    // An old /sh/ link leads to its /scl/fo/ form, which the list needs.
    let root = match FolderLink::of(link) {
        Some(root) => root,
        None => resolve(client, link, cancel)?,
    };
    let token = token();
    let mut name = None;
    let mut files = Vec::new();
    let mut seen = HashSet::from([root.secure_hash.clone()]);
    let mut pending = vec![(root, PathBuf::new())];
    while let Some((folder, path)) = pending.pop() {
        let mut voucher = None;
        loop {
            let answer = list(client, &token, &folder, voucher.as_deref(), cancel)?;
            if name.is_none() {
                name = answer
                    .folder
                    .as_ref()
                    .and_then(|folder| folder.filename.clone());
            }
            let more = answer.has_more_entries && !answer.entries.is_empty();
            voucher = answer.voucher();
            for entry in answer.entries {
                if entry.is_cloud_doc || entry.is_symlink {
                    continue;
                }
                let entry_path = path.join(safe_name(&entry.filename));
                if entry.is_dir {
                    if let Some(subfolder) = FolderLink::of(&entry.href)
                        && seen.insert(subfolder.secure_hash.clone())
                    {
                        pending.push((subfolder, entry_path));
                    }
                } else if let Some(url) = download_link(&entry.href) {
                    files.push(RemoteFile {
                        path: entry_path,
                        size: entry.bytes.unwrap_or(0),
                        source: FileSource::Dropbox(DropboxFile { url }),
                    });
                }
            }
            if !more || voucher.is_none() {
                break;
            }
        }
    }
    let title = name.unwrap_or_else(|| parsed.key.clone());
    let top = PathBuf::from(safe_name(&title));
    for file in &mut files {
        file.path = top.join(&file.path);
    }
    files.sort_by(|first, second| first.path.cmp(&second.path));
    Ok(Share {
        host: Host::Dropbox,
        id: parsed.key,
        url: link.to_owned(),
        title,
        upload_date: None,
        files,
    })
}

/// Why Dropbox answered with a page instead of the file. Its pages say little before their script runs, so the title
/// tells them apart: "Dropbox - Error - Simplify your life", "... Invalid Link ...", "... No Access ...", or
/// "Log in to Dropbox" when the link is private or its rlkey is wrong.
pub(super) fn page_error(page: &Page) -> PreviewError {
    if matches!(page.status.as_u16(), 429 | 509)
        && let Some(error) = code_error(Host::Dropbox, page.status)
    {
        return error;
    }
    let title = title_of(&page.html).unwrap_or_default();
    let said = title
        .trim_start_matches("Dropbox - ")
        .trim_end_matches(" - Simplify your life")
        .trim();
    if title.contains("Log in") {
        return PreviewError::Unavailable {
            detail: "Dropbox asks to sign in: the link is private, or its rlkey is wrong"
                .to_owned(),
        };
    }
    if ["Error", "Invalid Link", "No Access", "Not Found"]
        .iter()
        .any(|word| said.contains(word))
    {
        return PreviewError::Unavailable {
            detail: format!("Dropbox: {said}"),
        };
    }
    code_error(Host::Dropbox, page.status).unwrap_or_else(|| PreviewError::Failed {
        detail: "Dropbox answered with a page instead of the file".to_owned(),
    })
}

/// A shared folder or one of its subfolders, as the list asks for it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FolderLink {
    link_key: String,
    /// Each subfolder has its own.
    secure_hash: String,
    /// The subfolder's path from the shared folder, without a leading "/".
    sub_path: String,
    rlkey: String,
}

impl FolderLink {
    /// Reads "dropbox.com/scl/fo/<link key>/<secure hash>/<sub path>?rlkey=<rlkey>".
    fn of(link: &str) -> Option<Self> {
        let url = Url::parse(link).ok()?;
        let rlkey = query(&url, "rlkey")?;
        let segments: Vec<&str> = url
            .path_segments()?
            .filter(|segment| !segment.is_empty())
            .collect();
        let segments = segments.strip_prefix(&["e"][..]).unwrap_or(&segments);
        let ["scl", "fo", link_key, secure_hash, sub_path @ ..] = segments else {
            return None;
        };
        Some(Self {
            link_key: (*link_key).to_owned(),
            secure_hash: (*secure_hash).to_owned(),
            sub_path: sub_path
                .iter()
                .map(|segment| percent_decode(segment))
                .collect::<Vec<_>>()
                .join("/"),
            rlkey,
        })
    }
}

/// Follows an old folder link to its /scl/fo/ form.
fn resolve(client: &Client, link: &str, cancel: &CancelToken) -> Result<FolderLink, PreviewError> {
    check_cancel(cancel)?;
    let response = client.get(link).send_checked(FACTS.name)?;
    match FolderLink::of(response.url().as_str()) {
        Some(folder) => Ok(folder),
        None => Err(page_error(&Page::read(response))),
    }
}

#[derive(Deserialize)]
struct ListAnswer {
    #[serde(default)]
    entries: Vec<ListEntry>,
    #[serde(default)]
    has_more_entries: bool,
    /// What asks for the next page.
    #[serde(default)]
    next_request_voucher: Option<Value>,
    #[serde(default)]
    folder: Option<ListFolder>,
}

impl ListAnswer {
    fn voucher(&self) -> Option<String> {
        match self.next_request_voucher.as_ref()? {
            Value::Null => None,
            Value::String(text) => Some(text.clone()),
            other => Some(other.to_string()),
        }
    }
}

#[derive(Deserialize)]
struct ListEntry {
    filename: String,
    #[serde(default)]
    is_dir: bool,
    #[serde(default)]
    bytes: Option<u64>,
    /// The entry's own link: a subfolder's has its hash.
    #[serde(default)]
    href: String,
    /// A Dropbox Paper document, which isn't a file.
    #[serde(default)]
    is_cloud_doc: bool,
    #[serde(default)]
    is_symlink: bool,
}

#[derive(Deserialize)]
struct ListFolder {
    #[serde(default)]
    filename: Option<String>,
}

/// One page of what's directly in a folder.
fn list(
    client: &Client,
    token: &str,
    folder: &FolderLink,
    voucher: Option<&str>,
    cancel: &CancelToken,
) -> Result<ListAnswer, PreviewError> {
    check_cancel(cancel)?;
    let mut fields = vec![
        ("is_xhr", "true"),
        ("t", token),
        ("link_key", folder.link_key.as_str()),
        ("secure_hash", folder.secure_hash.as_str()),
        ("link_type", "c"),
        ("sub_path", folder.sub_path.as_str()),
        ("rlkey", folder.rlkey.as_str()),
    ];
    fields.extend(voucher.map(|voucher| ("voucher", voucher)));
    let response = client
        .post(format!("{SITE}/list_shared_link_folder_entries"))
        .header(COOKIE, format!("t={token}"))
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(form(&fields))
        .send_checked(FACTS.name)?;
    let is_json = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/json"));
    if response.status() != StatusCode::OK || !is_json {
        return Err(page_error(&Page::read(response)));
    }
    response.json().map_err(|error| PreviewError::Failed {
        detail: format!("Dropbox's list wasn't as expected: {error}"),
    })
}

/// Fields as a form sends them.
/// Dropbox only checks that a request's "t" field matches its "t" cookie, so that another site can't post for
/// someone signed in. Any value does.
fn token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!("{nanos:024x}")
}

/// The link with dl=1, which downloads instead of showing Dropbox's page. An embedded link ("/e/...") becomes a
/// plain one.
fn download_link(link: &str) -> Option<String> {
    let mut url = Url::parse(link).ok()?;
    if let Some(rest) = url.path().strip_prefix("/e/") {
        let path = format!("/{rest}");
        url.set_path(&path);
    }
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| !matches!(key.as_ref(), "dl" | "raw" | "preview"))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    url.query_pairs_mut()
        .clear()
        .extend_pairs(kept)
        .append_pair("dl", "1");
    url.set_fragment(None);
    Some(url.into())
}

fn query(url: &Url, name: &str) -> Option<String> {
    url.query_pairs()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
        .filter(|value| !value.is_empty())
}

/// Reads a Dropbox link: "dropbox.com/s/<key>/<name>", "dropbox.com/scl/fi/<key>/<name>?rlkey=...",
/// "dropbox.com/sh/<key>/<hash>" or "dropbox.com/scl/fo/<key>/<hash>?rlkey=...", also under "/e/" and on
/// dl.dropboxusercontent.com.
fn parse_link(link: &str) -> Option<DropboxLink> {
    let url = Url::parse(link).ok()?;
    if !sites::host(link).is_some_and(|host| Host::Dropbox.owns(&host)) {
        return None;
    }
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    let segments = segments.strip_prefix(&["e"][..]).unwrap_or(&segments);
    let (kind, key, old) = match segments {
        ["s", key, ..] => (Kind::File, key, true),
        ["scl", "fi", key, ..] => (Kind::File, key, false),
        ["sh", key, ..] => (Kind::Folder, key, true),
        ["scl", "fo", key, ..] => (Kind::Folder, key, false),
        _ => return None,
    };
    let is_key = !key.is_empty()
        && key
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    is_key.then(|| DropboxLink {
        kind,
        key: (*key).to_owned(),
        old,
        has_rlkey: query(&url, "rlkey").is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_file_and_folder_links() {
        let link = |kind, key: &str, old, has_rlkey| {
            Some(DropboxLink {
                kind,
                key: key.to_owned(),
                old,
                has_rlkey,
            })
        };
        for (address, expected) in [
            (
                "https://www.dropbox.com/s/nelirfsxnmcfbfh/youtube-dl%20test%20video.mp4?dl=0",
                link(Kind::File, "nelirfsxnmcfbfh", true, false),
            ),
            (
                "https://www.dropbox.com/scl/fi/r2kd2skcy5ylbbta5y1pz/DJI_0003.MP4?dl=0&rlkey=wcdgqangn7t3lnmmv6li9mu9h",
                link(Kind::File, "r2kd2skcy5ylbbta5y1pz", false, true),
            ),
            (
                "https://www.dropbox.com/e/scl/fi/r2kd2skcy5ylbbta5y1pz/DJI_0003.MP4?rlkey=wcdgqangn7t3lnmmv6li9mu9h",
                link(Kind::File, "r2kd2skcy5ylbbta5y1pz", false, true),
            ),
            (
                "https://www.dropbox.com/scl/fi/r2kd2skcy5ylbbta5y1pz/DJI_0003.MP4?dl=0",
                link(Kind::File, "r2kd2skcy5ylbbta5y1pz", false, false),
            ),
            (
                "https://dl.dropboxusercontent.com/s/nelirfsxnmcfbfh/video.mp4",
                link(Kind::File, "nelirfsxnmcfbfh", true, false),
            ),
            (
                "https://www.dropbox.com/sh/2mgpiuq7kv8nqdf/AABy-fW4dkydT4GmWi2mdOUDa?dl=0",
                link(Kind::Folder, "2mgpiuq7kv8nqdf", true, false),
            ),
            (
                "https://www.dropbox.com/scl/fo/ekhsvhqu2gb4q2zqx8e15/AB064iyJJcW2xnO5dKE5q6g?rlkey=jw3wvtrff2cc6htr2skdgk7uh&dl=0",
                link(Kind::Folder, "ekhsvhqu2gb4q2zqx8e15", false, true),
            ),
            ("https://www.dropbox.com/home", None),
            ("https://www.dropbox.com/s/", None),
            ("https://notdropbox.com/s/nelirfsxnmcfbfh/a.mp4", None),
        ] {
            assert_eq!(parse_link(address), expected, "{address}");
        }
    }

    #[test]
    fn downloads_with_dl_1() {
        assert_eq!(
            download_link("https://www.dropbox.com/scl/fi/r2kd2skcy5ylbbta5y1pz/DJI_0003.MP4?dl=0&rlkey=wcdgqangn7t3lnmmv6li9mu9h&st=ab12").as_deref(),
            Some("https://www.dropbox.com/scl/fi/r2kd2skcy5ylbbta5y1pz/DJI_0003.MP4?rlkey=wcdgqangn7t3lnmmv6li9mu9h&st=ab12&dl=1")
        );
        assert_eq!(
            download_link("https://www.dropbox.com/e/scl/fi/abc/A%20B.mp4?rlkey=x#top").as_deref(),
            Some("https://www.dropbox.com/scl/fi/abc/A%20B.mp4?rlkey=x&dl=1")
        );
        assert_eq!(
            download_link("https://www.dropbox.com/s/nelirfsxnmcfbfh/video.mp4").as_deref(),
            Some("https://www.dropbox.com/s/nelirfsxnmcfbfh/video.mp4?dl=1")
        );
    }

    #[test]
    fn reads_folder_and_subfolder_links() {
        assert_eq!(
            FolderLink::of(
                "https://www.dropbox.com/scl/fo/ekhsvhqu2gb4q2zqx8e15/AB064iyJJcW2xnO5dKE5q6g?rlkey=jw3wvtrff2cc6htr2skdgk7uh&dl=0"
            ),
            Some(FolderLink {
                link_key: "ekhsvhqu2gb4q2zqx8e15".to_owned(),
                secure_hash: "AB064iyJJcW2xnO5dKE5q6g".to_owned(),
                sub_path: String::new(),
                rlkey: "jw3wvtrff2cc6htr2skdgk7uh".to_owned(),
            })
        );
        let subfolder = FolderLink::of(
            "https://www.dropbox.com/scl/fo/ekhsvhqu2gb4q2zqx8e15/AA-KoGTy5DJmhZmzjq5Y_xc/4.5%20Food%20Drive/RAW?rlkey=jw3wvtrff2cc6htr2skdgk7uh&dl=0",
        )
        .unwrap();
        assert_eq!(subfolder.secure_hash, "AA-KoGTy5DJmhZmzjq5Y_xc");
        assert_eq!(subfolder.sub_path, "4.5 Food Drive/RAW");
        assert_eq!(
            FolderLink::of("https://www.dropbox.com/sh/2mgpiuq7kv8nqdf/AABy-fW4dkydT4GmWi2mdOUDa"),
            None
        );
    }

    #[test]
    fn reads_a_page_of_the_list() {
        let answer: ListAnswer = serde_json::from_str(
            r#"{"entries": [
                {"filename": "4.5 Food Drive", "is_dir": true, "folder_id": "z-fFzBxvSgAAAAAAAAArEA",
                 "href": "https://www.dropbox.com/scl/fo/ekhsvhqu2gb4q2zqx8e15/AN6Bn6VE6kGX2z_srgVhFXA/4.5%20Food%20Drive?rlkey=jw3wvtrff2cc6htr2skdgk7uh&dl=0",
                 "is_confidential_folder": false},
                {"filename": "Drone Shot.mp4", "is_dir": false, "bytes": 132414034, "ts": 1586203885,
                 "file_id": "id:z-fFzBxvSgAAAAAAAAAkcA",
                 "href": "https://www.dropbox.com/scl/fo/ekhsvhqu2gb4q2zqx8e15/AIKpu75g9JMNnCK-uIUqZIM/Drone%20Shot.mp4?rlkey=jw3wvtrff2cc6htr2skdgk7uh&dl=0",
                 "is_symlink": false, "is_cloud_doc": false}],
              "total_num_entries": 33, "has_more_entries": true, "next_request_voucher": "{\"page_size\": 75}",
              "folder": {"filename": "Food Drive", "is_dir": true}}"#,
        )
        .unwrap();
        assert_eq!(answer.entries.len(), 2);
        assert!(answer.entries[0].is_dir);
        assert_eq!(answer.entries[1].bytes, Some(132_414_034));
        assert_eq!(answer.voucher().as_deref(), Some("{\"page_size\": 75}"));
        assert_eq!(
            answer.folder.and_then(|folder| folder.filename).as_deref(),
            Some("Food Drive")
        );
        assert_eq!(
            download_link(&answer.entries[1].href).as_deref(),
            Some(
                "https://www.dropbox.com/scl/fo/ekhsvhqu2gb4q2zqx8e15/AIKpu75g9JMNnCK-uIUqZIM/Drone%20Shot.mp4?rlkey=jw3wvtrff2cc6htr2skdgk7uh&dl=1"
            )
        );
    }

    #[test]
    fn sends_fields_as_a_form() {
        assert_eq!(
            form(&[("sub_path", "4.5 Food Drive/RAW"), ("voucher", "{\"a\":1}")]),
            "sub_path=4.5+Food+Drive%2FRAW&voucher=%7B%22a%22%3A1%7D"
        );
        assert_eq!(token().len(), 24);
    }

    #[test]
    fn explains_the_pages_it_gets_instead_of_a_file() {
        let page = |status: u16, title: &str| Page {
            url: SITE.to_owned(),
            status: StatusCode::from_u16(status).unwrap(),
            html: format!("<html><head><title>{title}</title></head></html>"),
            challenged: false,
        };
        assert!(matches!(
            page_error(&page(200, "Dropbox - Invalid Link - Simplify your life")),
            PreviewError::Unavailable { detail } if detail == "Dropbox: Invalid Link"
        ));
        assert!(matches!(
            page_error(&page(200, "Dropbox - Error - Simplify your life")),
            PreviewError::Unavailable { .. }
        ));
        assert!(matches!(
            page_error(&page(200, "Log in to Dropbox")),
            PreviewError::Unavailable { detail } if detail.contains("rlkey")
        ));
        assert!(matches!(
            page_error(&page(429, "Dropbox - Error - Simplify your life")),
            PreviewError::LimitReached { site, .. } if site == "Dropbox"
        ));
        assert!(matches!(
            page_error(&page(200, "Dropbox")),
            PreviewError::Failed { .. }
        ));
    }
}
