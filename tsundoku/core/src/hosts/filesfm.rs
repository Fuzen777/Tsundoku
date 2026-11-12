//! Files.fm: public links to a folder ("files.fm/u/<hash>") or a file ("files.fm/f/<hash>", or its "down.php?i=<hash>"),
//! read as its pages read them, asked as Tsundoku. A page carries its files as JSON for its script (`objFilesData`:
//! each file's hash, name, size and date) and the folder's name (`objMainShareParams`). A file downloads once its page
//! has been opened: the page sets the session's cookies, and `down.php?i=<hash>` with them redirects to an address on
//! one of Files.fm's servers, signed for a while, so it's asked when the download starts.

use std::path::PathBuf;

use reqwest::blocking::{Client, Response};
use reqwest::header::{COOKIE, LOCATION, SET_COOKIE};
use reqwest::{StatusCode, Url};
use serde::Deserialize;
use serde_json::Value;

use super::{
    Address, FileCheck, FileSource, Host, HostFacts, Page, RemoteFile, SendChecked, Share,
    check_cancel, client_without_redirects, code_error, safe_name,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "Files.fm",
    site: "filesfm",
    // failiem.lv is its Latvian name, which its download servers use.
    domains: &["files.fm", "failiem.lv"],
    // Its download servers send each file as an attachment.
    sends_attachments: true,
    // It answers Tsundoku saying who it is as it answers a browser.
    browser_agent: false,
    read,
    page_error: None,
    window_link: None,
    download_file_id: None,
};

const SITE: &str = "https://files.fm";

/// A file to download from Files.fm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesFmFile {
    pub hash: String,
}

/// Reads a Files.fm link: the folder's files with their sizes, or the file's.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let item = parse_link(link).ok_or_else(|| Host::FilesFm.link_not_recognized())?;
    let (page, _) = open_page(client, &item.page(), cancel)?;
    let entries = files_data(&page.html).ok_or_else(gone)?;
    let (title, files) = match &item {
        Item::Folder(hash) => {
            if has_subfolders(&page.html) {
                return Err(PreviewError::NestedFolders {
                    site: FACTS.name.to_owned(),
                });
            }
            let title = folder_name(&page.html).unwrap_or_else(|| hash.clone());
            let top = PathBuf::from(safe_name(&title));
            let files = entries
                .iter()
                .map(|entry| entry.remote(top.join(safe_name(&entry.display_name))))
                .collect();
            (title, files)
        }
        Item::File(hash) => {
            let entry = entries
                .iter()
                .find(|entry| entry.hash == *hash)
                .ok_or_else(gone)?;
            let name = entry.display_name.clone();
            (
                name.clone(),
                vec![entry.remote(PathBuf::from(safe_name(&name)))],
            )
        }
    };
    let upload_date = entries
        .iter()
        .filter_map(|entry| entry.date_created.get(..10))
        .max()
        .map(str::to_owned);
    let mut files: Vec<RemoteFile> = files;
    files.sort_by(|first, second| first.path.cmp(&second.path));
    Ok(Share {
        host: Host::FilesFm,
        id: item.hash().to_owned(),
        url: item.page(),
        title,
        upload_date,
        files,
    })
}

/// Asks Files.fm where a file downloads from now: its page sets the cookies `down.php` needs, which then redirects to
/// the file on one of its servers.
pub(super) fn address(
    client: &Client,
    file: &FilesFmFile,
    cancel: &CancelToken,
) -> Result<Address, PreviewError> {
    let (_, cookies) = open_page(client, &Item::File(file.hash.clone()).page(), cancel)?;
    check_cancel(cancel)?;
    let response = client_without_redirects(Host::FilesFm)?
        .get(format!("{SITE}/down.php?i={}", file.hash))
        .header(COOKIE, cookies)
        .send_checked(FACTS.name)?;
    let location = response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|location| response.url().join(location).ok());
    match location {
        Some(url) if is_download_server(&url) => Ok(Address {
            url: url.to_string(),
            check: FileCheck::None,
        }),
        _ => Err(
            code_error(Host::FilesFm, response.status()).unwrap_or_else(|| PreviewError::Failed {
                detail: "Files.fm didn't give the file's address".to_owned(),
            }),
        ),
    }
}

/// One of Files.fm's download servers ("fv2-1.failiem.lv/down.php?…").
fn is_download_server(url: &Url) -> bool {
    url.scheme() == "https"
        && url.path() == "/down.php"
        && url
            .host_str()
            .is_some_and(|host| Host::FilesFm.owns(&host.to_ascii_lowercase()))
}

/// A page and the cookies it set.
fn open_page(
    client: &Client,
    url: &str,
    cancel: &CancelToken,
) -> Result<(Page, String), PreviewError> {
    check_cancel(cancel)?;
    let response = client.get(url).send_checked(FACTS.name)?;
    let cookies = cookies_set(&response);
    let page = Page::read(response);
    if page.status == StatusCode::NOT_FOUND {
        return Err(gone());
    }
    if let Some(error) = code_error(Host::FilesFm, page.status) {
        return Err(error);
    }
    Ok((page, cookies))
}

/// The cookies an answer set, as a Cookie header sends them back.
fn cookies_set(response: &Response) -> String {
    response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|cookie| cookie.split(';').next())
        .map(str::trim)
        .filter(|pair| pair.contains('='))
        .collect::<Vec<_>>()
        .join("; ")
}

/// A file as the page's data gives it.
#[derive(Debug, Deserialize)]
struct Entry {
    hash: String,
    display_name: String,
    /// In bytes, as text.
    #[serde(default)]
    size: Value,
    /// "2018-07-29 20:23:02".
    #[serde(default)]
    date_created: String,
}

impl Entry {
    fn size(&self) -> u64 {
        match &self.size {
            Value::String(text) => text.trim().parse().unwrap_or(0),
            Value::Number(number) => number.as_u64().unwrap_or(0),
            _ => 0,
        }
    }

    fn remote(&self, path: PathBuf) -> RemoteFile {
        RemoteFile {
            path,
            size: self.size(),
            source: FileSource::FilesFm(FilesFmFile {
                hash: self.hash.clone(),
            }),
        }
    }
}

/// The JSON a page's script assigns to `name`, from its first assignment of an object or a list (`objMainShareParams`
/// is declared `null` first).
fn script_value(html: &str, name: &str) -> Option<Value> {
    let assignment = format!("{name} = ");
    let start = html.match_indices(&assignment).find_map(|(index, _)| {
        let start = index + assignment.len();
        html[start..].starts_with(['{', '[']).then_some(start)
    })?;
    let mut values = serde_json::Deserializer::from_str(&html[start..]).into_iter::<Value>();
    values.next()?.ok()
}

/// The page's files, in its order. A page whose upload was deleted has none of its data.
fn files_data(html: &str) -> Option<Vec<Entry>> {
    let entries: Vec<Value> = match script_value(html, "objFilesData")? {
        Value::Object(map) => map.into_iter().map(|(_, entry)| entry).collect(),
        Value::Array(list) => list,
        _ => return None,
    };
    let mut entries: Vec<(u64, Entry)> = entries
        .into_iter()
        .filter_map(|entry| {
            let order = entry.get("file_order").and_then(Value::as_u64).unwrap_or(0);
            Some((order, serde_json::from_value(entry).ok()?))
        })
        .collect();
    entries.sort_by_key(|(order, _)| *order);
    Some(entries.into_iter().map(|(_, entry)| entry).collect())
}

/// The folder's name.
fn folder_name(html: &str) -> Option<String> {
    let params = script_value(html, "objMainShareParams")?;
    let name = params
        .pointer("/all_files/item_info/item_name")?
        .as_str()?
        .trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// Whether the folder has folders inside it, which its page lists apart from its files (`objUploads`).
fn has_subfolders(html: &str) -> bool {
    script_value(html, "objUploads").is_some_and(|uploads| match uploads {
        Value::Array(list) => !list.is_empty(),
        Value::Object(map) => !map.is_empty(),
        _ => false,
    })
}

fn gone() -> PreviewError {
    PreviewError::Unavailable {
        detail: "Files.fm no longer shares it".to_owned(),
    }
}

/// What a Files.fm link names.
#[derive(Debug, PartialEq, Eq)]
enum Item {
    Folder(String),
    File(String),
}

impl Item {
    fn hash(&self) -> &str {
        match self {
            Self::Folder(hash) | Self::File(hash) => hash,
        }
    }

    fn page(&self) -> String {
        match self {
            Self::Folder(hash) => format!("{SITE}/u/{hash}"),
            Self::File(hash) => format!("{SITE}/f/{hash}"),
        }
    }
}

/// The folder in "/u/<hash>" (a file shown in it, "#/view/<file>", is still the folder) or the file in "/f/<hash>" and
/// "down.php?i=<hash>", on files.fm or failiem.lv.
fn parse_link(link: &str) -> Option<Item> {
    let url = Url::parse(link).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !Host::FilesFm.owns(&host) {
        return None;
    }
    let is_hash = |hash: &str| {
        hash.len() >= 6
            && hash
                .chars()
                .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
    };
    if url.path() == "/down.php" {
        let hash = url
            .query_pairs()
            .find(|(key, _)| key == "i")
            .map(|(_, value)| value.into_owned())?;
        return is_hash(&hash).then_some(Item::File(hash));
    }
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    match segments.as_slice() {
        ["u", hash, ..] if is_hash(hash) => Some(Item::Folder((*hash).to_owned())),
        ["f", hash, ..] if is_hash(hash) => Some(Item::File((*hash).to_owned())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parts of a Files.fm folder's page on 2026-11-11.
    const FOLDER_PAGE: &str = r#"<script>
        var objMainShareParams = null;
        objMainShareParams = {"show_type_select":false,"type":"all_files","all_files":{"view_url":"https:\/\/files.fm\/u\/q7spbhat","item_info":{"upload_hash":"q7spbhat","item_name":"4.15.0-29"}}};
		var objFilesData = {"1222405472":{"id":"item_haqkygnw","hash":"haqkygnw","ratio":1,"width":null,"file_unordered_index":0,"display_name":"linux-cloud-tools-4.15.0-29-generic_4.15.0-29.31_amd64.deb","views":"0","downloads":"264","size":"108584","modified_date":"2018-07-29 04:44:13","date_created":"2018-07-29 20:22:05","file_order":1,"ext":"deb"},"1222405777":{"id":"item_eh7jyxe7","hash":"eh7jyxe7","ratio":1,"width":null,"file_unordered_index":4,"display_name":"linux-image-unsigned-4.15.0-29-generic_4.15.0-29.31_amd64.deb","views":"1","downloads":"269","size":"7981528","modified_date":"2018-07-29 04:42:12","date_created":"2018-07-29 20:23:02","file_order":0,"ext":"deb"}};
        var objUploads = [];
    </script>"#;

    #[test]
    fn reads_its_links() {
        for (link, item) in [
            (
                "https://files.fm/u/q7spbhat",
                Some(Item::Folder("q7spbhat".to_owned())),
            ),
            (
                "https://files.fm/u/q7spbhat#/view/eh7jyxe7",
                Some(Item::Folder("q7spbhat".to_owned())),
            ),
            (
                "https://files.fm/f/eh7jyxe7",
                Some(Item::File("eh7jyxe7".to_owned())),
            ),
            (
                "https://fv2-1.files.fm/down.php?i=eh7jyxe7&n=a.deb",
                Some(Item::File("eh7jyxe7".to_owned())),
            ),
            (
                "https://failiem.lv/u/q7spbhat",
                Some(Item::Folder("q7spbhat".to_owned())),
            ),
            ("https://files.fm/", None),
            ("https://files.fm/pricing", None),
        ] {
            assert_eq!(parse_link(link), item, "{link}");
        }
        assert_eq!(Host::of("https://files.fm/u/q7spbhat"), Some(Host::FilesFm));
    }

    #[test]
    fn reads_a_folders_files_from_its_page() {
        let entries = files_data(FOLDER_PAGE).unwrap();
        let names: Vec<(&str, u64)> = entries
            .iter()
            .map(|entry| (entry.hash.as_str(), entry.size()))
            .collect();
        assert_eq!(names, [("eh7jyxe7", 7_981_528), ("haqkygnw", 108_584)]);
        assert_eq!(folder_name(FOLDER_PAGE).as_deref(), Some("4.15.0-29"));
        assert!(!has_subfolders(FOLDER_PAGE));
        assert!(has_subfolders(
            r#"var objUploads = [{"upload_hash":"abcdefgh"}];"#
        ));
        assert!(files_data("<title>File upload & sharing.</title>").is_none());
    }

    #[test]
    fn takes_only_its_own_download_servers() {
        for (address, taken) in [
            (
                "https://fv2-1.failiem.lv/down.php?i=haqkygnw&download_checksum=a2&download_timestamp=1",
                true,
            ),
            ("https://files.fm/f/haqkygnw", false),
            ("https://example.com/down.php?i=haqkygnw", false),
        ] {
            assert_eq!(
                is_download_server(&Url::parse(address).unwrap()),
                taken,
                "{address}"
            );
        }
    }
}
