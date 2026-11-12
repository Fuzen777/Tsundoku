//! upload.ee: a file's page ("upload.ee/files/<id>/<name>.html"), read without signing in, asked as Tsundoku. The
//! page links the file's download ("upload.ee/download/<id>/<token>/<name>"), whose token changes with each visit, so
//! the page is read again when the download starts. The page rounds the size, so the download is asked for its first
//! byte, which gives the exact size and name.

use std::path::PathBuf;

use reqwest::StatusCode;
use reqwest::Url;
use reqwest::blocking::Client;

use super::{
    Address, Answer, FileCheck, FileSource, Host, HostFacts, Page, RemoteFile, SendChecked, Share,
    check_cancel, code_error, html_text, percent_decode, probe, safe_name,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "upload.ee",
    site: "uploadee",
    domains: &["upload.ee"],
    sends_attachments: true,
    // It answers Tsundoku saying who it is as it answers a browser.
    browser_agent: false,
    read,
    page_error: None,
    window: None,
};

/// A file to download from upload.ee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadEeFile {
    /// Its page, which links the download.
    pub page: String,
    pub id: String,
}

/// Reads an upload.ee link: the file's name and size.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let file = parse_link(link).ok_or_else(|| Host::UploadEe.link_not_recognized())?;
    let download = download_link(client, &file, cancel)?;
    let found = match probe(client, Host::UploadEe, &download, cancel)? {
        Answer::File(found) => found,
        Answer::Page(page) => return Err(Host::UploadEe.page_error(&page)),
    };
    let name = found
        .name
        .or_else(|| download.rsplit('/').next().map(percent_decode))
        .unwrap_or_else(|| file.id.clone());
    Ok(Share {
        host: Host::UploadEe,
        id: file.id.clone(),
        url: file.page.clone(),
        title: name.clone(),
        upload_date: found.modified,
        files: vec![RemoteFile {
            path: PathBuf::from(safe_name(&name)),
            size: found.size,
            source: FileSource::UploadEe(file),
        }],
    })
}

/// Reads the file's page again for a download link with a fresh token.
pub(super) fn address(
    client: &Client,
    file: &UploadEeFile,
    cancel: &CancelToken,
) -> Result<Address, PreviewError> {
    Ok(Address {
        url: download_link(client, file, cancel)?,
        check: FileCheck::None,
    })
}

/// The download link on the file's page.
fn download_link(
    client: &Client,
    file: &UploadEeFile,
    cancel: &CancelToken,
) -> Result<String, PreviewError> {
    check_cancel(cancel)?;
    let response = client.get(&file.page).send_checked(FACTS.name)?;
    let page = Page::read(response);
    if page.status == StatusCode::NOT_FOUND || is_gone(&page.html) {
        return Err(PreviewError::Unavailable {
            detail: "upload.ee says the file doesn't exist, or was deleted".to_owned(),
        });
    }
    if let Some(error) = code_error(Host::UploadEe, page.status) {
        return Err(error);
    }
    linked_download(&page.html, &file.id).ok_or_else(|| PreviewError::Failed {
        detail: "upload.ee's page had no download link".to_owned(),
    })
}

/// Whether the page says the file is gone.
fn is_gone(html: &str) -> bool {
    [
        "File does not exist",
        "There is no such file",
        "File was deleted",
        "does not exist on disk",
    ]
    .iter()
    .any(|phrase| html.contains(phrase))
}

/// The page's link to the file's download: "https://www.upload.ee/download/<id>/<token>/<name>".
fn linked_download(html: &str, id: &str) -> Option<String> {
    let marker = format!("upload.ee/download/{id}/");
    html.match_indices(&marker).find_map(|(index, _)| {
        let start = html[..index].rfind(['"', '\''])? + 1;
        let end = index + html[index..].find(['"', '\'', ' ', '>'])?;
        let link = html_text(&html[start..end]);
        let url = Url::parse(&link).ok()?;
        let host = url.host_str()?.to_ascii_lowercase();
        (matches!(url.scheme(), "https" | "http") && Host::UploadEe.owns(&host)).then_some(link)
    })
}

/// The file's ID and page in "upload.ee/files/<id>/<name>.html".
fn parse_link(link: &str) -> Option<UploadEeFile> {
    let url = Url::parse(link).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !Host::UploadEe.owns(&host) {
        return None;
    }
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    match segments.as_slice() {
        ["files", id, name, ..]
            if !id.is_empty() && id.chars().all(|character| character.is_ascii_digit()) =>
        {
            Some(UploadEeFile {
                page: format!("https://www.upload.ee/files/{id}/{name}"),
                id: (*id).to_owned(),
            })
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
            parse_link("https://upload.ee/files/17307242/Everedit-min-241025.7z.html"),
            Some(UploadEeFile {
                page: "https://www.upload.ee/files/17307242/Everedit-min-241025.7z.html".to_owned(),
                id: "17307242".to_owned(),
            })
        );
        assert_eq!(parse_link("https://www.upload.ee/rules.html"), None);
        assert_eq!(parse_link("https://www.upload.ee/files/abc/x.html"), None);
        assert_eq!(
            Host::of("https://www.upload.ee/files/17307242/a.7z.html"),
            Some(Host::UploadEe)
        );
        // Parts of upload.ee's page for a file, on 2026-11-11.
        let page = r#"File: <b>Everedit-min-241025.7z</b><br />Size: 11.24 MB<br />
            <a id="d_l" href="https://www.upload.ee/download/17307242/c09ba8f1b3e9237b6889/Everedit-min-241025.7z" class="dl">Download</a>"#;
        assert_eq!(
            linked_download(page, "17307242").as_deref(),
            Some(
                "https://www.upload.ee/download/17307242/c09ba8f1b3e9237b6889/Everedit-min-241025.7z"
            )
        );
        assert_eq!(linked_download(page, "1"), None);
        assert!(is_gone("<title>UPLOAD.EE - File does not exist</title>"));
        assert!(!is_gone(page));
    }
}
