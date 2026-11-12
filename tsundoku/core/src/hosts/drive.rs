//! Google Drive: links to a file or a shared folder, read without signing in. A file downloads from
//! drive.usercontent.google.com, where `confirm=t` skips the warning that a large file can't be scanned for viruses;
//! asking for its first byte gives its name and size. A folder's contents come from its embedded view
//! (`embeddedfolderview`), a plain list of names and links with no sizes, so each file is asked for its first byte
//! too, several at once. Google Docs, Sheets and Slides have no file to download and are left out.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

use reqwest::blocking::Client;
use reqwest::{StatusCode, Url};

use super::{
    Answer, FileSource, Host, HostFacts, Page, RemoteFile, SendChecked, Share, between,
    check_cancel, code_error, html_text, network_error, probe, safe_name, title_of,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;
use crate::sites;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "Google Drive",
    // yt-dlp's name for it, so what it downloaded from Google Drive before still counts.
    site: "googledrive",
    domains: &["drive.google.com", "drive.usercontent.google.com"],
    sends_attachments: true,
    browser_agent: false,
    read,
    page_error: Some(page_error),
    window_link: None,
    download_file_id: None,
    unlock: None,
};

const DOWNLOAD: &str = "https://drive.usercontent.google.com/download";
const FOLDER_VIEW: &str = "https://drive.google.com/embeddedfolderview";
/// Files asked for their size at once when reading a folder.
const PROBES_AT_ONCE: usize = 8;

/// A file or folder on Google Drive: its ID, with the resource key that links shared before 2021 may need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveFile {
    pub id: String,
    pub resource_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DriveLink {
    File(DriveFile),
    Folder(DriveFile),
}

/// Reads a Google Drive link: the file's name, size and date, or every file in the folder and its subfolders.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    match parse_link(link).ok_or_else(|| Host::GoogleDrive.link_not_recognized())? {
        DriveLink::File(file) => {
            let found = match probe(client, Host::GoogleDrive, &download_url(&file), cancel)? {
                Answer::File(found) => found,
                Answer::Page(page) => return Err(page_error(&page)),
            };
            let title = found.name.unwrap_or_else(|| file.id.clone());
            Ok(Share {
                host: Host::GoogleDrive,
                id: file.id.clone(),
                url: link.to_owned(),
                upload_date: found.modified,
                files: vec![RemoteFile {
                    path: PathBuf::from(safe_name(&title)),
                    size: found.size,
                    source: FileSource::GoogleDrive(file),
                }],
                title,
            })
        }
        DriveLink::Folder(folder) => {
            let root = folder_view(client, &folder, cancel)?;
            let mut seen = HashSet::from([folder.id.clone()]);
            let mut listed = Vec::new();
            let mut pending = vec![(PathBuf::from(safe_name(&root.name)), root.entries)];
            while let Some((path, entries)) = pending.pop() {
                for entry in entries {
                    let entry_path = path.join(safe_name(&entry.name));
                    match entry.link {
                        DriveLink::File(file) => listed.push((entry_path, file)),
                        DriveLink::Folder(subfolder) => {
                            if seen.insert(subfolder.id.clone()) {
                                let view = folder_view(client, &subfolder, cancel)?;
                                pending.push((entry_path, view.entries));
                            }
                        }
                    }
                }
            }
            let sizes = sizes(client, &listed, cancel)?;
            let mut files: Vec<RemoteFile> = listed
                .into_iter()
                .zip(sizes)
                .map(|((path, file), size)| RemoteFile {
                    path,
                    size,
                    source: FileSource::GoogleDrive(file),
                })
                .collect();
            files.sort_by(|first, second| first.path.cmp(&second.path));
            Ok(Share {
                host: Host::GoogleDrive,
                id: folder.id,
                url: link.to_owned(),
                title: root.name,
                upload_date: None,
                files,
            })
        }
    }
}

/// Where a file downloads from. `confirm=t` answers the virus scan warning ahead.
pub(super) fn download_url(file: &DriveFile) -> String {
    let mut url = format!("{DOWNLOAD}?id={}&export=download&confirm=t", file.id);
    if let Some(key) = &file.resource_key {
        url.push_str("&resourcekey=");
        url.push_str(key);
    }
    url
}

/// Why Google Drive answered with a page instead of the file.
pub(super) fn page_error(page: &Page) -> PreviewError {
    let site = Host::GoogleDrive.name().to_owned();
    if sites::host(&page.url).is_some_and(|host| host == "accounts.google.com") {
        return PreviewError::Unavailable {
            detail: "Google Drive only shows this to people it's shared with".to_owned(),
        };
    }
    if let Some(message) = between(&page.html, "class=\"uc-error-subcaption\">", "</p>") {
        let message = html_text(message.trim());
        return if message.contains("Too many users") || message.contains("quota") {
            PreviewError::LimitReached {
                site,
                detail: message,
            }
        } else {
            PreviewError::Failed { detail: message }
        };
    }
    if page.status == StatusCode::RANGE_NOT_SATISFIABLE {
        // Asking a Google Doc, Sheet or Slides for its first byte.
        return PreviewError::Failed {
            detail: "Google Docs, Sheets and Slides can't be downloaded as files".to_owned(),
        };
    }
    code_error(Host::GoogleDrive, page.status).unwrap_or_else(|| PreviewError::Failed {
        detail: "Google Drive answered with a page instead of the file".to_owned(),
    })
}

/// A folder's name and what's directly in it.
struct FolderView {
    name: String,
    entries: Vec<Entry>,
}

struct Entry {
    name: String,
    link: DriveLink,
}

fn folder_view(
    client: &Client,
    folder: &DriveFile,
    cancel: &CancelToken,
) -> Result<FolderView, PreviewError> {
    check_cancel(cancel)?;
    let mut url = format!("{FOLDER_VIEW}?id={}", folder.id);
    if let Some(key) = &folder.resource_key {
        url.push_str("&resourcekey=");
        url.push_str(key);
    }
    let response = client.get(&url).send_checked(FACTS.name)?;
    let signing_in =
        sites::host(response.url().as_str()).is_some_and(|host| host == "accounts.google.com");
    if !response.status().is_success() || signing_in {
        return Err(page_error(&Page::read(response)));
    }
    let html = response.text().map_err(network_error)?;
    parse_folder_view(&html).ok_or_else(|| PreviewError::Failed {
        detail: "Google Drive's folder view wasn't as expected".to_owned(),
    })
}

/// Reads a folder's embedded view: its name in the title, then one "flip-entry" per file or folder, with its link
/// and name. Folders come first, then files, each by name.
fn parse_folder_view(html: &str) -> Option<FolderView> {
    if !html.contains("flip-entries") {
        return None;
    }
    let name = title_of(html)?;
    let entries = html
        .split("id=\"entry-")
        .skip(1)
        .filter_map(|entry| {
            let href = html_text(between(entry, "href=\"", "\"")?);
            let name = html_text(between(entry, "class=\"flip-entry-title\">", "</div>")?.trim());
            // A Google Doc's link is on docs.google.com, which isn't a link to a file or folder.
            let link = parse_link(&href)?;
            Some(Entry { name, link })
        })
        .collect();
    Some(FolderView { name, entries })
}

/// Each file's size, asking several files at once. Stops at the first that fails, and gives the error of the
/// earliest file that did.
fn sizes(
    client: &Client,
    files: &[(PathBuf, DriveFile)],
    cancel: &CancelToken,
) -> Result<Vec<u64>, PreviewError> {
    let next = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let answers: Vec<(usize, Result<u64, PreviewError>)> = thread::scope(|scope| {
        let workers: Vec<_> = (0..PROBES_AT_ONCE.min(files.len()))
            .map(|_| {
                scope.spawn(|| {
                    let mut answers = Vec::new();
                    while !failed.load(Ordering::Relaxed) {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some((_, file)) = files.get(index) else {
                            break;
                        };
                        let answer =
                            match probe(client, Host::GoogleDrive, &download_url(file), cancel) {
                                Ok(Answer::File(found)) => Ok(found.size),
                                Ok(Answer::Page(page)) => Err(page_error(&page)),
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
    let mut sizes = vec![None; files.len()];
    let mut first_error: Option<(usize, PreviewError)> = None;
    for (index, answer) in answers {
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
        .map(|size| {
            size.ok_or_else(|| PreviewError::Failed {
                detail: "Google Drive didn't give every file's size".to_owned(),
            })
        })
        .collect()
}

/// Reads a Google Drive link: "drive.google.com/file/d/<id>/view" (or /edit, /preview, under /u/<n>/),
/// "drive.google.com/open?id=<id>", "drive.google.com/uc?id=<id>", "drive.usercontent.google.com/download?id=<id>",
/// "drive.google.com/drive/folders/<id>" (or /drive/u/<n>/folders/<id>, /drive/mobile/folders/<id>), or
/// "drive.google.com/folderview?id=<id>". A "resourcekey" in the query goes with the ID.
fn parse_link(link: &str) -> Option<DriveLink> {
    let url = Url::parse(link).ok()?;
    if !sites::host(link).is_some_and(|host| Host::GoogleDrive.owns(&host)) {
        return None;
    }
    let query = |name: &str| {
        url.query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
    };
    let resource_key = query("resourcekey").filter(|key| is_id(key));
    let item = |id: &str| {
        is_id(id).then(|| DriveFile {
            id: id.to_owned(),
            resource_key: resource_key.clone(),
        })
    };
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    match segments.as_slice() {
        ["file", "d", id, ..] | ["file", "u", _, "d", id, ..] => item(id).map(DriveLink::File),
        ["drive", .., "folders", id] | ["drive", .., "folders", id, _] => {
            item(id).map(DriveLink::Folder)
        }
        ["open" | "uc" | "download"] => item(&query("id")?).map(DriveLink::File),
        ["folderview" | "embeddedfolderview"] => item(&query("id")?).map(DriveLink::Folder),
        _ => None,
    }
}

/// Drive's IDs and resource keys: letters, digits, "-" and "_".
fn is_id(text: &str) -> bool {
    text.len() >= 10
        && text
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: &str) -> DriveFile {
        DriveFile {
            id: id.to_owned(),
            resource_key: None,
        }
    }

    #[test]
    fn reads_file_and_folder_links() {
        let id = "1l_5RK28JRL19wpT22B-DY9We3TVXnnQQ";
        for link in [
            format!("https://drive.google.com/file/d/{id}/view?usp=sharing"),
            format!("https://drive.google.com/file/d/{id}"),
            format!("https://drive.google.com/file/u/1/d/{id}/edit"),
            format!("https://drive.google.com/open?id={id}"),
            format!("https://drive.google.com/uc?export=download&id={id}"),
            format!("https://drive.usercontent.google.com/download?id={id}&export=download"),
        ] {
            assert_eq!(parse_link(&link), Some(DriveLink::File(file(id))), "{link}");
        }
        let folder = "15uNXeRBIhVvZJIhL4yTw4IsStMhUaaxl";
        for link in [
            format!("https://drive.google.com/drive/folders/{folder}?usp=sharing"),
            format!("https://drive.google.com/drive/u/0/folders/{folder}"),
            format!("https://drive.google.com/drive/mobile/folders/{folder}/details"),
            format!("https://drive.google.com/folderview?id={folder}"),
            format!("https://drive.google.com/embeddedfolderview?id={folder}#list"),
        ] {
            assert_eq!(
                parse_link(&link),
                Some(DriveLink::Folder(file(folder))),
                "{link}"
            );
        }
        assert_eq!(
            parse_link(&format!(
                "https://drive.google.com/file/d/{id}/view?resourcekey=0-Ab_cD-eF12gH"
            )),
            Some(DriveLink::File(DriveFile {
                id: id.to_owned(),
                resource_key: Some("0-Ab_cD-eF12gH".to_owned()),
            }))
        );
        for link in [
            "https://drive.google.com/",
            "https://drive.google.com/drive/my-drive",
            "https://drive.google.com/open?id=short",
            "https://docs.google.com/document/d/1TFYNzuZJTgNGzGmjraZ58ZVOh9_YoKeBnU-opWgXQL4/edit",
        ] {
            assert_eq!(parse_link(link), None, "{link}");
        }
    }

    #[test]
    fn asks_for_the_file_past_the_virus_scan_warning() {
        assert_eq!(
            download_url(&file("0B9P1L--7Wd2vU3VUVlFnbTgtS2c")),
            "https://drive.usercontent.google.com/download?id=0B9P1L--7Wd2vU3VUVlFnbTgtS2c&export=download&confirm=t"
        );
        let with_key = DriveFile {
            resource_key: Some("0-Ab_cD-eF12gH".to_owned()),
            ..file("0B9P1L--7Wd2vU3VUVlFnbTgtS2c")
        };
        assert!(download_url(&with_key).ends_with("&confirm=t&resourcekey=0-Ab_cD-eF12gH"));
    }

    #[test]
    fn reads_a_folders_embedded_view() {
        let html = r#"<html><head><title>Trip &amp; more</title></head><body><div class="flip-list-header"></div>
            <div class="flip-entries">
            <div class="flip-entry" id="entry-1oow464Z4Et95RAwtRZn6dLOFb_YYpLwL" tabindex="0" role="link"><div class="flip-entry-info"><a href="https://drive.google.com/drive/folders/1oow464Z4Et95RAwtRZn6dLOFb_YYpLwL" target="_blank"><div class="flip-entry-list-icon"><div aria-label="Folder" class="icon-color-1 drive-sprite-folder-list-shared-icon"></div></div><div class="flip-entry-title">deep_folder</div></a></div><div class="flip-entry-last-modified"><div>6/25/22</div></div></div>
            <div class="flip-entry" id="entry-1eE-5jm2G8N5FL4HHeexrL0-Lg15bc0-D" tabindex="0" role="link"><div class="flip-entry-info"><a href="https://drive.google.com/file/d/1eE-5jm2G8N5FL4HHeexrL0-Lg15bc0-D/view?usp=drive_web" target="_blank"><div class="flip-entry-list-icon"><img src="https://drive-thirdparty.googleusercontent.com/16/type/video/webm" alt=""/></div><div class="flip-entry-title">A Walk &#39;through&#39; our forest.webm</div></a></div><div class="flip-entry-last-modified"><div>10/22/19</div></div></div>
            <div class="flip-entry" id="entry-1TFYNzuZJTgNGzGmjraZ58ZVOh9_YoKeBnU-opWgXQL4" tabindex="0" role="link"><div class="flip-entry-info"><a href="https://docs.google.com/document/d/1TFYNzuZJTgNGzGmjraZ58ZVOh9_YoKeBnU-opWgXQL4/edit?usp=drive_web" target="_blank"><div class="flip-entry-list-icon"><img src="https://drive-thirdparty.googleusercontent.com/16/type/application/vnd.google-apps.document" alt=""/></div><div class="flip-entry-title">gdown.docx</div></a></div><div class="flip-entry-last-modified"><div>1/24/24</div></div></div>
            </div></body></html>"#;
        let view = parse_folder_view(html).unwrap();
        assert_eq!(view.name, "Trip & more");
        let entries: Vec<(&str, &DriveLink)> = view
            .entries
            .iter()
            .map(|entry| (entry.name.as_str(), &entry.link))
            .collect();
        assert_eq!(
            entries,
            [
                (
                    "deep_folder",
                    &DriveLink::Folder(file("1oow464Z4Et95RAwtRZn6dLOFb_YYpLwL"))
                ),
                (
                    "A Walk 'through' our forest.webm",
                    &DriveLink::File(file("1eE-5jm2G8N5FL4HHeexrL0-Lg15bc0-D"))
                ),
            ]
        );
        assert!(parse_folder_view("<html><title>Sign in</title></html>").is_none());
    }

    #[test]
    fn explains_the_pages_it_gets_instead_of_a_file() {
        let page = |url: &str, status: u16, html: &str| Page {
            url: url.to_owned(),
            status: StatusCode::from_u16(status).unwrap(),
            html: html.to_owned(),
            challenged: false,
        };
        assert!(matches!(
            page_error(&page("https://accounts.google.com/ServiceLogin?service=wise", 200, "")),
            PreviewError::Unavailable { detail } if detail.contains("shared with")
        ));
        assert!(matches!(
            page_error(&page(
                DOWNLOAD,
                200,
                r#"<p class="uc-error-subcaption">Too many users have viewed or downloaded this file recently.</p>"#
            )),
            PreviewError::LimitReached { site, .. } if site == "Google Drive"
        ));
        assert!(matches!(
            page_error(&page(
                DOWNLOAD,
                404,
                "<title>Error 404 (Not Found)!!1</title>"
            )),
            PreviewError::Unavailable { .. }
        ));
        assert!(matches!(
            page_error(&page(DOWNLOAD, 416, "")),
            PreviewError::Failed { detail } if detail.contains("Google Docs")
        ));
        assert!(matches!(
            page_error(&page(DOWNLOAD, 200, "<title>Google Drive - Virus scan warning</title>")),
            PreviewError::Failed { detail } if detail.contains("instead of the file")
        ));
    }
}
