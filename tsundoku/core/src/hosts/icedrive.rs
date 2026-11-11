//! Icedrive: public links to a file or a folder ("icedrive.net/s/<code>"), read without signing in. The link's page
//! names the shared file (`previewItem('<id>'`) or folder (`'folder-<id>'`); Icedrive's download API
//! (`request=download-multi`, asked as its page asks it) then gives every file under it with its folder, size, date
//! and an address to download it from, which expires, so a file's is asked again when its download starts.

use std::path::PathBuf;

use reqwest::blocking::Client;
use serde::Deserialize;

use super::{
    Address, FileCheck, FileSource, Host, HostFacts, Page, RemoteFile, Share, between,
    check_cancel, code_error, date_of, network_error, safe_name, title_of,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "Icedrive",
    site: "icedrive",
    domains: &["icedrive.net"],
    sends_attachments: false,
    // It answers Tsundoku saying who it is as it answers a browser.
    browser_agent: false,
    read,
    page_error: None,
};

const SITE: &str = "https://icedrive.net";
const API: &str = "https://api.icedrive.net/API/Internal/V2/";

/// A file to download from Icedrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcedriveFile {
    pub file_id: u64,
}

/// Reads an Icedrive link: the file's name, size and date, or every file in the folder and its subfolders.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let code = parse_link(link).ok_or_else(|| Host::Icedrive.link_not_recognized())?;
    check_cancel(cancel)?;
    let response = client
        .get(format!("{SITE}/s/{code}"))
        .send()
        .map_err(network_error)?;
    let page = Page::read(response);
    if !page.status.is_success() {
        return Err(unavailable(code_error(Host::Icedrive, page.status)));
    }
    let item = shared_item(&page.html).ok_or_else(|| unavailable(None))?;
    let title = title_of(&page.html)
        .map(|title| title.trim_end_matches(" - Icedrive").to_owned())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| code.clone());
    let entries = download_multi(client, &item, cancel)?;
    let upload_date = entries
        .iter()
        .filter_map(|entry| entry.moddate)
        .max()
        .and_then(date_of);
    let mut files: Vec<RemoteFile> = entries.into_iter().map(Entry::remote).collect();
    files.sort_by(|first, second| first.path.cmp(&second.path));
    if files.is_empty() {
        return Err(PreviewError::NoVideo {
            detail: "the Icedrive folder is empty".to_owned(),
        });
    }
    Ok(Share {
        host: Host::Icedrive,
        id: code,
        url: link.to_owned(),
        title,
        upload_date,
        files,
    })
}

/// Asks Icedrive where a file downloads from now.
pub(super) fn address(
    client: &Client,
    file: &IcedriveFile,
    cancel: &CancelToken,
) -> Result<Address, PreviewError> {
    let entry = download_multi(client, &format!("file-{}", file.file_id), cancel)?
        .into_iter()
        .next()
        .ok_or_else(|| unavailable(None))?;
    if !entry.url.starts_with("https://") {
        return Err(PreviewError::Failed {
            detail: "Icedrive gave no address to download from".to_owned(),
        });
    }
    Ok(Address {
        url: entry.url,
        check: FileCheck::None,
    })
}

#[derive(Debug, Deserialize)]
struct DownloadAnswer {
    #[serde(default)]
    error: bool,
    #[serde(default)]
    message: String,
    #[serde(default)]
    urls: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    id: u64,
    filename: String,
    #[serde(default)]
    filesize: u64,
    /// The folders from the shared folder down ("MatSim/Benchmark"), empty for a shared file.
    #[serde(default)]
    path: String,
    /// Seconds since 1970.
    #[serde(default)]
    moddate: Option<i64>,
    #[serde(default)]
    url: String,
}

impl Entry {
    fn remote(self) -> RemoteFile {
        let path: PathBuf = self
            .path
            .split('/')
            .filter(|part| !part.is_empty())
            .map(safe_name)
            .chain([safe_name(&self.filename)])
            .collect();
        RemoteFile {
            path,
            size: self.filesize,
            source: FileSource::Icedrive(IcedriveFile { file_id: self.id }),
        }
    }
}

/// Every file under `item` ("file-<id>" or "folder-<id>"), as Icedrive's page asks for a download.
fn download_multi(
    client: &Client,
    item: &str,
    cancel: &CancelToken,
) -> Result<Vec<Entry>, PreviewError> {
    check_cancel(cancel)?;
    let url = reqwest::Url::parse_with_params(
        API,
        [
            ("request", "download-multi"),
            ("items", item),
            ("public", "1"),
        ],
    )
    .map_err(|error| PreviewError::Failed {
        detail: error.to_string(),
    })?;
    let response = client
        .get(url)
        .header("X-Requested-With", "XMLHttpRequest")
        .send()
        .map_err(network_error)?;
    if let Some(error) = code_error(Host::Icedrive, response.status()) {
        return Err(error);
    }
    let answer: DownloadAnswer = response.json().map_err(|error| PreviewError::Failed {
        detail: format!("Icedrive's answer wasn't as expected: {error}"),
    })?;
    if answer.error {
        return Err(unavailable(Some(PreviewError::Failed {
            detail: format!("Icedrive: {}", answer.message),
        })));
    }
    Ok(answer.urls)
}

/// A link Icedrive no longer shares, or the error that says why.
fn unavailable(error: Option<PreviewError>) -> PreviewError {
    match error {
        Some(PreviewError::Failed { detail }) | Some(PreviewError::Unavailable { detail }) => {
            PreviewError::Unavailable {
                detail: format!("{detail}: Icedrive no longer shares it"),
            }
        }
        Some(error) => error,
        None => PreviewError::Unavailable {
            detail: "Icedrive no longer shares it".to_owned(),
        },
    }
}

/// What a share page names: "file-<id>" for a file (`previewItem('<id>'`), "folder-<id>" for a folder.
fn shared_item(html: &str) -> Option<String> {
    if let Some(folder) = between(html, "'folder-", "'") {
        return folder
            .chars()
            .all(|character| character.is_ascii_digit())
            .then(|| format!("folder-{folder}"));
    }
    let file = between(html, "previewItem('", "'")?;
    (!file.is_empty() && file.chars().all(|character| character.is_ascii_digit()))
        .then(|| format!("file-{file}"))
}

/// The share's code in "icedrive.net/s/<code>".
fn parse_link(link: &str) -> Option<String> {
    let url = reqwest::Url::parse(link).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !Host::Icedrive.owns(&host) {
        return None;
    }
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    match segments.as_slice() {
        ["s", code, ..]
            if !code.is_empty()
                && code
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric()) =>
        {
            Some((*code).to_owned())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_its_links_and_pages() {
        assert_eq!(
            parse_link("https://icedrive.net/s/VbCSVjRPBPfybgwF5hwhjS1PZaWv").as_deref(),
            Some("VbCSVjRPBPfybgwF5hwhjS1PZaWv")
        );
        assert_eq!(parse_link("https://icedrive.net/pricing"), None);
        assert_eq!(Host::of("https://icedrive.net/s/abc"), Some(Host::Icedrive));
        assert_eq!(
            shared_item(r#"<div onclick="previewItem('1438591788', false);"></div>"#).as_deref(),
            Some("file-1438591788")
        );
        assert_eq!(
            shared_item(r#"<div onclick="$download.init(['folder-156757519'], 'public');"></div>"#)
                .as_deref(),
            Some("folder-156757519")
        );
        assert_eq!(shared_item("<html>Not found</html>"), None);
    }

    #[test]
    fn reads_a_folders_files() {
        // Icedrive's answer for a folder, trimmed.
        let answer: DownloadAnswer = serde_json::from_str(
            r#"{"error": false, "urls": [
            {"id": 1372310260, "filename": "set1_MatSim.zip", "filesize": 374473444, "folderId": 157336626,
             "path": "MatSim/Benchmark", "moddate": 1667686265, "url": "https://ice-eu-1.icedrive.io/download?p=x"},
            {"id": 7, "filename": "a:b.txt", "filesize": 3, "path": "", "url": "https://ice-eu-1.icedrive.io/download?p=y"}]}"#,
        )
        .unwrap();
        let files: Vec<RemoteFile> = answer.urls.into_iter().map(Entry::remote).collect();
        assert_eq!(
            files[0].path,
            PathBuf::from("MatSim")
                .join("Benchmark")
                .join("set1_MatSim.zip")
        );
        assert_eq!(files[0].size, 374473444);
        assert_eq!(files[1].path, PathBuf::from("a_b.txt"));
        assert_eq!(
            files[1].source,
            FileSource::Icedrive(IcedriveFile { file_id: 7 })
        );
        let failed: DownloadAnswer =
            serde_json::from_str(r#"{"error": true, "message": "No files found"}"#).unwrap();
        assert!(failed.error && failed.urls.is_empty());
    }
}
