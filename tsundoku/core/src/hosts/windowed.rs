//! Files only the check window can give: a host whose link's own page is behind a check (Cloudflare's, a captcha, or
//! a proof of work its page computes), so Tsundoku's client can't read it. Tsundoku never tries to get past the check:
//! the link reads as one file named after the link (its ID, or a name the link carries), its size unknown, and its
//! download waits for the person to open the link's page in the check window, pass what it asks and start the
//! download there. Then, as the host's `WindowFacts::download` says, either the window catches the file's address as
//! the page goes to it, and the address gives the file's name and size (`Share::resolve_caught`) and the file downloads
//! from it like any other host's (`WindowDownload::CatchAddress`), or, for a host whose address only works in the
//! window's own browser session, the window lets the page's download run and saves the file into the download's
//! temporary folder, which the download then moves into place (`WindowDownload::SaveInWindow`). Tsundoku's own client
//! never takes the window's cookies.

use std::path::Path;

use std::path::PathBuf;

use reqwest::blocking::Client;

use super::{Address, Answer, FileCheck, FileSource, Host, RemoteFile, Share, probe, safe_name};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

/// What a host read through the window knows about its links and its downloads.
pub(crate) struct WindowFacts {
    /// The link's file: its ID, the page to open, and a name when the link carries one. `None` for a link that isn't
    /// to a file.
    pub link: fn(&str) -> Option<WindowLink>,
    /// Whether `download`, an address the window caught on the page of the file with `id`, is that file's: anything
    /// else (another file, an advert's download, one of the site's own pages) is left alone.
    pub takes: fn(download: &str, id: &str) -> bool,
    /// How the file comes once the person has started its download on the page.
    pub download: WindowDownload,
    /// The page gives the first click on Download to an advert, so the window's title says to click again.
    pub first_click_ad: bool,
}

/// How a file read through the window comes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowDownload {
    /// The window catches the file's address, and Tsundoku downloads it with progress.
    CatchAddress,
    /// The address works only in the window's own session (Workupload's), so the window saves the file itself.
    SaveInWindow,
}

/// What the window hands a download for a file it saved: `SAVED_PREFIX`, the file's ID, `|`, then where it saved it.
const SAVED_PREFIX: &str = "tsundoku-saved:";

/// The link that hands a download the file the window saved for the file with `id`.
pub fn saved_link(id: &str, path: &Path) -> String {
    format!("{SAVED_PREFIX}{id}|{}", path.display())
}

/// The file ID and the path in a link from `saved_link`.
pub(crate) fn saved_file(link: &str) -> Option<(&str, &Path)> {
    let (id, path) = link.strip_prefix(SAVED_PREFIX)?.split_once('|')?;
    Some((id, Path::new(path)))
}

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
    /// The address the window caught, or one pasted, while it leads to the file.
    pub direct: Option<String>,
    /// The file the window saved, for a host whose download runs in the window.
    pub saved: Option<PathBuf>,
}

impl WindowedFile {
    /// Whether `download` is this file's (`WindowFacts::takes`), or hands over the file the window saved for it.
    pub(crate) fn takes(&self, download: &str) -> bool {
        if let Some((id, _)) = saved_file(download) {
            return id == self.id;
        }
        self.host
            .facts()
            .window
            .as_ref()
            .is_some_and(|window| (window.takes)(download, &self.id))
    }
}

/// The share of a link only the window can read: one file, named after the link, its size unknown (0) until the
/// window catches its address. `None` when the host isn't read through the window, or the link isn't to a file.
pub(super) fn share(host: Host, link: &str) -> Option<Share> {
    let window = host.facts().window.as_ref()?;
    let found = (window.link)(link)?;
    let name = found.name.clone().unwrap_or_else(|| found.id.clone());
    Some(Share {
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
                direct: None,
                saved: None,
            }),
        }],
    })
}

/// Where the file downloads from: the address the window caught. Without one, its download waits for the window
/// (`PreviewError::BrowserCheck`, which `file_runner` holds for the check with the file's page).
pub(super) fn address(file: &WindowedFile) -> Result<Address, PreviewError> {
    match &file.direct {
        Some(url) => Ok(Address {
            url: url.clone(),
            check: FileCheck::None,
        }),
        None => Err(PreviewError::BrowserCheck {
            site: file.host.name().to_owned(),
        }),
    }
}

impl Share {
    /// Gives each file read through the window its name and size, from the address caught for it, before it
    /// downloads. An address that no longer leads to the file is dropped, so the file waits for the window again. A
    /// share of one file takes the file's name as its title.
    pub fn resolve_caught(
        &mut self,
        client: &Client,
        cancel: &CancelToken,
    ) -> Result<(), PreviewError> {
        let single = self.files.len() == 1;
        let mut title = None;
        for file in &mut self.files {
            let FileSource::Windowed(windowed) = &mut file.source else {
                continue;
            };
            // A file the window saved is here already: its own name and size.
            if let Some(saved) = &windowed.saved {
                match std::fs::metadata(saved) {
                    Ok(metadata) if metadata.is_file() => {
                        file.size = metadata.len();
                        if let Some(name) = saved.file_name().and_then(|name| name.to_str()) {
                            file.path = PathBuf::from(safe_name(name));
                            if single {
                                title = Some(name.to_owned());
                            }
                        }
                    }
                    _ => windowed.saved = None,
                }
                continue;
            }
            let Some(direct) = windowed.direct.clone() else {
                continue;
            };
            match probe(client, &direct, cancel)? {
                Answer::File(found) => {
                    file.size = found.size;
                    if let Some(name) = found.name {
                        file.path = PathBuf::from(safe_name(&name));
                        if single {
                            title = Some(name);
                        }
                    }
                }
                Answer::Page(_) => windowed.direct = None,
            }
        }
        if let Some(title) = title {
            self.title = title;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_comes_only_from_the_window() {
        let mut file = WindowedFile {
            host: Host::MediaFire,
            id: "abc".to_owned(),
            page: "https://example.com/abc".to_owned(),
            direct: None,
            saved: None,
        };
        assert_eq!(
            address(&file),
            Err(PreviewError::BrowserCheck {
                site: "MediaFire".to_owned()
            })
        );
        file.direct = Some("https://files.example.com/abc/a.zip".to_owned());
        assert_eq!(
            address(&file).map(|address| address.url),
            Ok("https://files.example.com/abc/a.zip".to_owned())
        );
        assert!(
            share(Host::MediaFire, "https://www.mediafire.com/file/abc").is_none(),
            "a host read without the window"
        );
    }

    #[test]
    fn a_file_the_window_saved_is_handed_over_by_its_link() {
        let folder = tempfile::tempdir().unwrap();
        let saved = folder.path().join("Sample file.png");
        std::fs::write(&saved, "12345").unwrap();
        let link = saved_link("uDjfcJwACXx", &saved);
        assert_eq!(saved_file(&link), Some(("uDjfcJwACXx", saved.as_path())));
        assert_eq!(saved_file("https://example.com/a"), None);

        let mut share = share(Host::Workupload, "https://workupload.com/file/uDjfcJwACXx").unwrap();
        assert_eq!(
            share.title, "uDjfcJwACXx",
            "named after the link until the file is known"
        );
        assert_eq!(share.files[0].size, 0, "its size unknown");
        share.use_caught_links(&[link]);
        share
            .resolve_caught(&crate::hosts::client().unwrap(), &CancelToken::new())
            .unwrap();
        assert_eq!(share.title, "Sample file.png");
        assert_eq!(share.files[0].size, 5);
        assert_eq!(share.files[0].path, PathBuf::from("Sample file.png"));
    }
}
