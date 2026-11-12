//! Files only the check window can give: a link from a known window site (`window_sites`), or any link whose own page
//! a built-in reader found behind a check only a person can pass (`check`). Tsundoku never tries to get past the
//! check: the link reads as one file named after the link (its ID, or the file name its address ends with), its size
//! unknown, and its download waits for the person to open the link's page in the check window and start the download
//! there. The window then offers that download to the queue, which decides, per download, whether Tsundoku fetches
//! the file itself or the window saves it (`downloads::window_offer`). Tsundoku's own client never takes the window's
//! cookies.

use std::path::{Path, PathBuf};

use reqwest::Url;

use super::{FileSource, Host, RemoteFile, Share, percent_decode, safe_name};
use crate::metadata::PreviewError;

/// A link's file, as the window opens it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowLink {
    pub id: String,
    pub page: String,
    pub name: Option<String>,
}

/// A file to download through the check window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowedFile {
    pub host: Host,
    pub id: String,
    /// The page the window opens.
    pub page: String,
}

/// What the window hands a download for an address it caught: `CAUGHT_PREFIX`, the file's ID, `|`, the address.
const CAUGHT_PREFIX: &str = "tsundoku-caught:";
/// What the window hands a download for a file it saved: `SAVED_PREFIX`, the file's ID, `|`, where it saved it.
const SAVED_PREFIX: &str = "tsundoku-saved:";

/// The link that hands a download the address caught for the file with `id` (`CheckPage::file_id`).
pub fn caught_link(id: &str, address: &str) -> String {
    format!("{CAUGHT_PREFIX}{id}|{address}")
}

/// The link that hands a download the file the window saved for the file with `id`.
pub fn saved_link(id: &str, path: &Path) -> String {
    format!("{SAVED_PREFIX}{id}|{}", path.display())
}

/// Whether `link` hands a download a file the window saved.
pub(crate) fn is_saved_link(link: &str) -> bool {
    link.starts_with(SAVED_PREFIX)
}

/// What a link from the window gives a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Caught<'a> {
    /// An address its download started from, to ask again.
    Address(&'a str),
    /// The file itself, saved by the window.
    Saved(&'a Path),
}

/// The file ID and what a link from `caught_link` or `saved_link` gives it.
pub(crate) fn caught(link: &str) -> Option<(&str, Caught<'_>)> {
    if let Some(rest) = link.strip_prefix(CAUGHT_PREFIX) {
        let (id, address) = rest.split_once('|')?;
        return Some((id, Caught::Address(address)));
    }
    let (id, path) = link.strip_prefix(SAVED_PREFIX)?.split_once('|')?;
    Some((id, Caught::Saved(Path::new(path))))
}

/// The share of a link only the window can read: one file, named after the link, its size unknown (0) until its
/// download starts. A known window site's link is named by its ID; any other host's by the end of its address.
pub(crate) fn share(host: Host, link: &str) -> Share {
    let found = host.window_link(link).unwrap_or_else(|| any_link(link));
    let name = found.name.clone().unwrap_or_else(|| found.id.clone());
    Share {
        host,
        id: found.id.clone(),
        url: link.to_owned(),
        title: name.clone(),
        upload_date: None,
        files: vec![RemoteFile {
            path: PathBuf::from(safe_name(&name)),
            size: 0,
            source: FileSource::Windowed(WindowedFile {
                host,
                id: found.id,
                page: found.page,
            }),
        }],
    }
}

/// Any link as one file the window opens: its address without the scheme and host as its ID, and the last part of
/// its path that looks like a file's name ("Pack.zip") as its name.
fn any_link(link: &str) -> WindowLink {
    let url = Url::parse(link).ok();
    let id = url
        .as_ref()
        .map(|url| {
            let mut id = url.path().trim_matches('/').to_owned();
            if let Some(query) = url.query() {
                id.push('?');
                id.push_str(query);
            }
            id
        })
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| link.to_owned());
    let name = url
        .as_ref()
        .and_then(|url| url.path_segments())
        .and_then(|mut segments| {
            segments
                .rfind(|segment| segment.contains('.'))
                .map(percent_decode)
        });
    WindowLink {
        id,
        page: link.to_owned(),
        name,
    }
}

/// Where the file downloads from: nothing Tsundoku's client can ask. Its download waits for the window
/// (`PreviewError::BrowserCheck`, which `file_runner` holds for the check with the file's page).
pub(super) fn address(file: &WindowedFile) -> Result<super::Address, PreviewError> {
    Err(super::check::needs_person(file.host.name()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_from_the_window_reach_their_own_file() {
        let saved = Path::new(r"D:\Temp\Sample file.png");
        assert_eq!(
            caught(&saved_link("Xk3mP9qR2tW", saved)),
            Some(("Xk3mP9qR2tW", Caught::Saved(saved)))
        );
        assert_eq!(
            caught(&caught_link(
                "file/abc",
                "https://download1.example.com/a?b=c|d"
            )),
            Some((
                "file/abc",
                Caught::Address("https://download1.example.com/a?b=c|d")
            )),
            "an address may hold a \"|\"; an ID doesn't, a link's own being percent-encoded"
        );
        assert_eq!(caught("https://example.com/a"), None);
    }

    #[test]
    fn any_hosts_link_behind_a_check_is_one_file_its_page_opens() {
        let share = share(
            Host::MediaFire,
            "https://www.mediafire.com/file/abc123/Pack%20v2.zip/file",
        );
        assert_eq!(share.title, "Pack v2.zip");
        assert_eq!(share.id, "file/abc123/Pack%20v2.zip/file");
        assert_eq!(share.files[0].size, 0, "its size unknown");
        let FileSource::Windowed(file) = &share.files[0].source else {
            panic!("a file the window gives");
        };
        assert_eq!(
            file.page,
            "https://www.mediafire.com/file/abc123/Pack%20v2.zip/file"
        );
        assert_eq!(
            address(file),
            Err(PreviewError::BrowserCheck {
                site: "MediaFire".to_owned()
            })
        );
        assert_eq!(
            super::share(Host::Dropbox, "https://www.dropbox.com/scl/fo/abc?rlkey=x").title,
            "scl/fo/abc?rlkey=x",
            "named after its address when it ends with no file name"
        );
    }
}
