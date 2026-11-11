//! Box: public shared links ("app.box.com/s/<shared name>", or a company's "<company>.app.box.com/s/…", and vanity
//! links "/v/<name>"), read as Box's own shared page reads them, without signing in. The page carries its data as JSON
//! (`Box.postStreamData`): a file's name and size, or a folder's items a page at a time, each subfolder at
//! "/s/<shared name>/folder/<id>". A file downloads from `index.php?rm=box_download_shared_file`, which sends it on to
//! Box's download servers. Every request names the shared link, so nothing outside it is reached.

use std::collections::HashSet;
use std::path::PathBuf;

use reqwest::Url;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::Value;

use super::{
    FileSource, Host, HostFacts, Page, RemoteFile, Share, check_cancel, code_error, date_of,
    network_error, safe_name,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "Box",
    site: "box",
    domains: &["box.com", "box.net"],
    // Its downloads come as attachments; anything else is a page saying why.
    sends_attachments: true,
    // It answers Tsundoku saying who it is as it answers a browser.
    browser_agent: false,
    read,
    page_error: Some(page_error),
};

const STREAM_DATA: &str = "Box.postStreamData = ";
const SHARED_ITEM: &str = "/app-api/enduserapp/shared-item";
const SHARED_FOLDER: &str = "/app-api/enduserapp/shared-folder";
/// The most pages of one folder read, as a guard against a page that never ends.
const MOST_PAGES: u64 = 1000;

/// A file to download from Box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxFile {
    /// The link's site, "https://app.box.com" or a company's.
    pub origin: String,
    pub shared_name: String,
    pub file_id: u64,
}

/// Where a file downloads from: Box's address for a shared file, which redirects to its download servers.
pub(super) fn download_url(file: &BoxFile) -> String {
    format!(
        "{}/index.php?rm=box_download_shared_file&shared_name={}&file_id=f_{}",
        file.origin, file.shared_name, file.file_id
    )
}

/// Reads a Box link: the file's name and size, or every file in the folder and its subfolders.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let parsed = parse_link(link).ok_or_else(|| Host::Box.link_not_recognized())?;
    let data = page_data(client, &parsed.page(), cancel)?;
    let item: SharedItem = field(&data, SHARED_ITEM)?;
    let shared_name = item.shared_name.clone();
    let origin = parsed.origin.clone();
    let source = |file_id| {
        FileSource::Box(BoxFile {
            origin: origin.clone(),
            shared_name: shared_name.clone(),
            file_id,
        })
    };
    if item.item_type == "file" {
        let file: FilePage = field(
            &data,
            &format!("/app-api/enduserapp/item/f_{}", item.item_id),
        )?;
        let entry = file.items.into_iter().next().ok_or_else(unexpected)?;
        return Ok(Share {
            host: Host::Box,
            id: item.shared_name,
            url: link.to_owned(),
            upload_date: entry.date.and_then(date_of),
            files: vec![RemoteFile {
                path: PathBuf::from(safe_name(&entry.name)),
                size: entry.item_size,
                source: source(entry.id),
            }],
            title: entry.name,
        });
    }
    let root: FolderPage = field(&data, SHARED_FOLDER)?;
    let title = root.current_folder_name.clone();
    let mut files = Vec::new();
    let mut seen = HashSet::from([root.current_folder_id]);
    let mut pending = vec![(
        root.current_folder_id,
        PathBuf::from(safe_name(&title)),
        Some(root),
    )];
    while let Some((folder_id, path, first)) = pending.pop() {
        let mut page = match first {
            Some(page) => page,
            None => field(
                &page_data(
                    client,
                    &format!("{}/folder/{folder_id}", parsed.base()),
                    cancel,
                )?,
                SHARED_FOLDER,
            )?,
        };
        let mut number = 1;
        loop {
            for entry in std::mem::take(&mut page.items) {
                let entry_path = path.join(safe_name(&entry.name));
                match entry.kind.as_str() {
                    "folder" if seen.insert(entry.id) => pending.push((entry.id, entry_path, None)),
                    // Box Notes are documents Box shows, not files to download.
                    "file" if !entry.name.ends_with(".boxnote") => files.push(RemoteFile {
                        path: entry_path,
                        size: entry.item_size,
                        source: source(entry.id),
                    }),
                    _ => {}
                }
            }
            if number >= page.page_count.min(MOST_PAGES) {
                break;
            }
            number += 1;
            let url = format!("{}/folder/{folder_id}?page={number}", parsed.base());
            page = field(&page_data(client, &url, cancel)?, SHARED_FOLDER)?;
        }
    }
    files.sort_by(|first, second| first.path.cmp(&second.path));
    Ok(Share {
        host: Host::Box,
        id: item.shared_name,
        url: link.to_owned(),
        upload_date: None,
        title,
        files,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedItem {
    #[serde(rename = "itemID")]
    item_id: u64,
    item_type: String,
    shared_name: String,
}

#[derive(Debug, Deserialize)]
struct FilePage {
    #[serde(default)]
    items: Vec<Entry>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FolderPage {
    #[serde(rename = "currentFolderID", default)]
    current_folder_id: u64,
    #[serde(default)]
    current_folder_name: String,
    #[serde(default = "one")]
    page_count: u64,
    #[serde(default)]
    items: Vec<Entry>,
}

fn one() -> u64 {
    1
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    #[serde(rename = "type", default)]
    kind: String,
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    item_size: u64,
    /// Seconds since 1970.
    #[serde(default)]
    date: Option<i64>,
}

/// The JSON a Box page carries, or why there's none: a password, or a link removed or not shared.
fn page_data(client: &Client, url: &str, cancel: &CancelToken) -> Result<Value, PreviewError> {
    check_cancel(cancel)?;
    let response = client.get(url).send().map_err(network_error)?;
    let page = Page::read(response);
    stream_data(&page.html).ok_or_else(|| page_error(&page))
}

/// The page's `Box.postStreamData`, the JSON that follows it.
fn stream_data(html: &str) -> Option<Value> {
    let start = html.find(STREAM_DATA)? + STREAM_DATA.len();
    serde_json::Deserializer::from_str(&html[start..])
        .into_iter::<Value>()
        .next()?
        .ok()
}

fn field<T: serde::de::DeserializeOwned>(data: &Value, key: &str) -> Result<T, PreviewError> {
    serde_json::from_value(data.get(key).cloned().ok_or_else(unexpected)?).map_err(|error| {
        PreviewError::Failed {
            detail: format!("Box's page wasn't as expected: {error}"),
        }
    })
}

fn unexpected() -> PreviewError {
    PreviewError::Failed {
        detail: "Box's page wasn't as expected".to_owned(),
    }
}

/// Why Box answered with a page instead of a file or a shared item.
fn page_error(page: &Page) -> PreviewError {
    let site = || Host::Box.name().to_owned();
    if page.html.contains("passwordRequired") || page.html.contains("incorrectPassword") {
        PreviewError::PasswordProtected { site: site() }
    } else if page.status.as_u16() == 404
        || page.html.contains("has been removed or is unavailable")
        || page
            .html
            .contains("This shared file or folder link has been removed")
    {
        PreviewError::Unavailable {
            detail: "Box says the shared link was removed or isn't shared with everyone".to_owned(),
        }
    } else {
        code_error(Host::Box, page.status).unwrap_or_else(|| PreviewError::Failed {
            detail: "Box answered with a page instead of the file".to_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoxLink {
    /// "https://app.box.com", or a company's site.
    origin: String,
    /// "s" for a shared link, "v" for a vanity one.
    kind: String,
    name: String,
    /// A folder inside the shared one ("/folder/<id>").
    folder: Option<u64>,
}

impl BoxLink {
    fn base(&self) -> String {
        format!("{}/{}/{}", self.origin, self.kind, self.name)
    }

    fn page(&self) -> String {
        match self.folder {
            Some(folder) => format!("{}/folder/{folder}", self.base()),
            None => self.base(),
        }
    }
}

/// Reads a Box link: "…box.com/s/<shared name>[/folder/<id>|/file/<id>]" or "/v/<vanity name>", on app.box.com or a
/// company's box.com site (box.net links are Box's old domain).
fn parse_link(link: &str) -> Option<BoxLink> {
    let url = Url::parse(link).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !Host::Box.owns(&host) {
        return None;
    }
    let host = host.replace("box.net", "box.com");
    let host = if host == "box.com" || host == "www.box.com" {
        "app.box.com".to_owned()
    } else {
        host
    };
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    let is_name = |name: &str| {
        !name.is_empty()
            && name.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
    };
    let (kind, name, rest) = match segments.as_slice() {
        [kind @ ("s" | "shared" | "v"), name, rest @ ..] if is_name(name) => (*kind, *name, rest),
        _ => return None,
    };
    let folder = match rest {
        ["folder", id, ..] => Some(id.parse().ok()?),
        _ => None,
    };
    Some(BoxLink {
        origin: format!("https://{host}"),
        kind: if kind == "v" { "v" } else { "s" }.to_owned(),
        name: name.to_owned(),
        folder,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_its_links() {
        let link = |origin: &str, kind: &str, name: &str, folder: Option<u64>| {
            Some(BoxLink {
                origin: origin.to_owned(),
                kind: kind.to_owned(),
                name: name.to_owned(),
                folder,
            })
        };
        for (url, expected) in [
            (
                "https://app.box.com/s/01nfw2cbbp4hi4my8tuoik8eazecd9wm",
                link(
                    "https://app.box.com",
                    "s",
                    "01nfw2cbbp4hi4my8tuoik8eazecd9wm",
                    None,
                ),
            ),
            (
                "https://app.box.com/s/1cwdnolsmtf0s04o0hshbv4vxiuqcmi9/folder/5012107469",
                link(
                    "https://app.box.com",
                    "s",
                    "1cwdnolsmtf0s04o0hshbv4vxiuqcmi9",
                    Some(5012107469),
                ),
            ),
            (
                "https://acme.app.box.com/s/abc123/file/42",
                link("https://acme.app.box.com", "s", "abc123", None),
            ),
            (
                "https://ucla.box.com/v/datasets",
                link("https://ucla.box.com", "v", "datasets", None),
            ),
            (
                "https://www.box.net/shared/abc123",
                link("https://app.box.com", "s", "abc123", None),
            ),
            ("https://www.box.com/pricing", None),
            ("https://app.box.com/folder/0", None),
        ] {
            assert_eq!(parse_link(url), expected, "{url}");
        }
        assert_eq!(Host::of("https://app.box.com/s/abc"), Some(Host::Box));
        assert_eq!(
            Host::of("https://www.dropbox.com/s/abc/a.mp4"),
            Some(Host::Dropbox)
        );
        assert_eq!(
            download_url(&BoxFile {
                origin: "https://app.box.com".to_owned(),
                shared_name: "01nf".to_owned(),
                file_id: 1154742568995
            }),
            "https://app.box.com/index.php?rm=box_download_shared_file&shared_name=01nf&file_id=f_1154742568995"
        );
    }

    #[test]
    fn reads_a_pages_data() {
        // A shared folder's page, trimmed.
        let html = r#"<script>Box.postStreamData = {"/app-api/enduserapp/shared-item": {"itemID": 5012107469,
            "itemType": "folder", "sharedName": "1cwd"}, "/app-api/enduserapp/shared-folder": {"currentFolderID": 5012107469,
            "currentFolderName": "osgeopy-data", "pageCount": 2, "pageNumber": 1, "items": [
            {"typedID": "f_40587456005", "type": "file", "id": 40587456005, "name": "osgeopy-data-misc.zip",
             "itemSize": 1995, "date": 1459229508},
            {"type": "folder", "id": 7, "name": "Sub"},
            {"type": "file", "id": 9, "name": "README.boxnote", "itemSize": 2113}]}};</script><p>more</p>"#;
        let data = stream_data(html).unwrap();
        let item: SharedItem = field(&data, SHARED_ITEM).unwrap();
        assert_eq!(
            (item.item_type.as_str(), item.shared_name.as_str()),
            ("folder", "1cwd")
        );
        let folder: FolderPage = field(&data, SHARED_FOLDER).unwrap();
        assert_eq!(folder.page_count, 2);
        assert_eq!(folder.items[0].item_size, 1995);
        assert_eq!(
            folder.items[0].date.and_then(date_of).as_deref(),
            Some("2016-03-29")
        );
        assert_eq!(folder.items[1].kind, "folder");
        assert_eq!(stream_data("<html>no data</html>"), None);
    }

    #[test]
    fn explains_its_pages() {
        let page = |status: u16, html: &str| Page {
            url: "https://app.box.com/s/x".to_owned(),
            status: reqwest::StatusCode::from_u16(status).unwrap(),
            html: html.to_owned(),
            challenged: false,
        };
        assert_eq!(
            page_error(&page(403, r#"{"status": 403, "passwordRequired": true}"#)),
            PreviewError::PasswordProtected {
                site: "Box".to_owned()
            }
        );
        assert!(matches!(
            page_error(&page(
                404,
                "This shared file or folder link has been removed or is unavailable to you."
            )),
            PreviewError::Unavailable { .. }
        ));
    }
}
