//! A check only a person can pass, such as Cloudflare's browser check. Tsundoku never tries to get past one itself:
//! every request a reader sends goes through one step that knows the checking service's markers (`hosts::checked`),
//! and a file behind one waits in the queue until the person opens the check in Tsundoku's check window, a browser
//! view of the site's own page. Once they've done what it asks and started the download on the page, the window
//! offers that download to the queue (`downloads::window_offer`), which hands the file its address or the file the
//! window saved, by the file's ID (`windowed::caught_link`, `windowed::saved_link`). Nothing here knows a site's
//! layout: a page is a file's page on the site, and the window takes the download it starts.

use std::path::Path;

use reqwest::StatusCode;
use reqwest::header::HeaderMap;

use super::windowed::{self, Caught};
use super::{FileSource, Host, Page, RemoteFile, Share};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

/// The pages a download needs a person to open, one after another: a page for each file behind the check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserCheck {
    /// The site's name, as people know it ("MediaFire").
    pub site: String,
    pub pages: Vec<CheckPage>,
}

/// A file's page on its site, which starts the file's download once the check is passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckPage {
    /// The page to open.
    pub url: String,
    host: Host,
    /// The file's ID, by which what the window gives reaches it: the host's own, or the link's.
    file_id: String,
}

impl CheckPage {
    /// The page a person opens for `file` of `share`: its own page on the site when the host has one a person can
    /// open (MediaFire's and Yandex Disk's file pages, a link read through the window), else the link's own for a
    /// share of one file. `None` for a file of a folder without a page of its own.
    pub(crate) fn for_file(share: &Share, file: &RemoteFile) -> Option<Self> {
        let (url, file_id) = match &file.source {
            FileSource::MediaFire(file) => (
                super::mediafire::file_page(&file.quick_key),
                file.quick_key.clone(),
            ),
            FileSource::YandexDisk(file) => (file.page(), file.path.clone()),
            FileSource::Windowed(file) => (file.page.clone(), file.id.clone()),
            _ if share.files.len() == 1 => (share.url.clone(), share.id.clone()),
            _ => return None,
        };
        Some(Self {
            url,
            host: share.host,
            file_id,
        })
    }

    /// The page of a known window site's link (`window_sites`), without asking the site: for a development build's
    /// test of the window on a page of its own (`moved_to`).
    pub fn of_window_link(link: &str) -> Option<Self> {
        let host = Host::of(link)?;
        host.window_link(link)?;
        let share = windowed::share(host, link);
        Self::for_file(&share, share.files.first()?)
    }

    /// The same file's page at another address: for a development build's test of a waiting download's window on a
    /// page of its own.
    pub fn moved_to(&self, url: &str) -> Self {
        Self {
            url: url.to_owned(),
            ..self.clone()
        }
    }

    /// Whether `host`, a page's or a download's host name, is on the page's site: the check window doesn't leave
    /// it, and only a download from it is the page's file.
    pub fn on_site(&self, host: &str) -> bool {
        self.host.owns(host)
    }

    /// The file's ID, by which what the window gives reaches it.
    pub fn file_id(&self) -> &str {
        &self.file_id
    }

    /// The site's name.
    pub fn site(&self) -> &'static str {
        self.host.name()
    }

    /// Whether `link` gets past this page: what the window gave its file (`windowed::caught_link`, `saved_link`), or a
    /// download address of the host's own a person pasted (MediaFire's `download<n>.mediafire.com/…`).
    pub fn takes(&self, link: &str) -> bool {
        match windowed::caught(link) {
            Some((id, _)) => id == self.file_id,
            None => self.host.download_file_id(link).as_deref() == Some(self.file_id.as_str()),
        }
    }

    /// The link that hands the download the address the window caught for this page's file.
    pub fn caught_link(&self, address: &str) -> String {
        windowed::caught_link(&self.file_id, address)
    }

    /// The link that hands the download the file the window saved from this page.
    pub fn saved_link(&self, path: &Path) -> String {
        windowed::saved_link(&self.file_id, path)
    }

    /// What `links` give this page's file, the latest first: a file the window saved, or an address.
    pub(crate) fn caught_in<'a>(&self, links: &'a [String]) -> Option<Caught<'a>> {
        links
            .iter()
            .rev()
            .find_map(|link| match windowed::caught(link) {
                Some((id, caught)) if id == self.file_id => Some(caught),
                Some(_) => None,
                None => self.takes(link).then_some(Caught::Address(link.as_str())),
            })
    }
}

impl BrowserCheck {
    /// Whether one of its pages takes `link` (`CheckPage::takes`).
    pub fn takes(&self, link: &str) -> bool {
        self.pages.iter().any(|page| page.takes(link))
    }

    /// Leaves out the pages `link` gets past, once it's been caught or pasted. Whether any did.
    pub fn passed(&mut self, link: &str) -> bool {
        let before = self.pages.len();
        self.pages.retain(|page| !page.takes(link));
        self.pages.len() < before
    }
}

/// Whether a page a site answered is a check only a person can pass. Only the checking service's own markers count,
/// never a site's layout: Cloudflare's check for a browser ("Just a moment..."), marked with `cf-mitigated: challenge`
/// (`is_marked`), or, without the header, a 403 or 503 carrying the challenge's own options or its title
/// (`is_check_page`). Every request a reader sends goes through `hosts::checked`, which applies the same test.
pub(crate) fn is_browser_check(page: &Page) -> bool {
    page.challenged || (may_be_check_page(page.status) && is_check_page(&page.html))
}

/// Whether an answer's headers mark it as Cloudflare's check (`cf-mitigated: challenge`).
pub(crate) fn is_marked(headers: &HeaderMap) -> bool {
    headers
        .get("cf-mitigated")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("challenge"))
}

/// Whether an answer with `status` may be a check its headers don't mark: Cloudflare answers its checks with 403 or
/// 503.
pub(crate) fn may_be_check_page(status: StatusCode) -> bool {
    matches!(status.as_u16(), 403 | 503)
}

/// Whether a page is Cloudflare's check: its challenge's options (`_cf_chl_opt`) or its title. Its "challenge-platform"
/// script is on ordinary pages of some sites (every MediaFire page), so it isn't a sign.
pub(crate) fn is_check_page(html: &str) -> bool {
    html.contains("_cf_chl_opt") || html.contains("<title>Just a moment...</title>")
}

/// The error for a check on `site`, which a person passes in the check window.
pub(crate) fn needs_person(site: &str) -> PreviewError {
    PreviewError::BrowserCheck {
        site: site.to_owned(),
    }
}

/// Every file of `link` as a check's pages, for a development build to open the check window on a link of its choice.
pub fn for_link(link: &str, cancel: &CancelToken) -> Result<BrowserCheck, PreviewError> {
    let share = super::read(link, cancel)?;
    let pages: Vec<CheckPage> = share
        .files
        .iter()
        .filter_map(|file| CheckPage::for_file(&share, file))
        .collect();
    if pages.is_empty() {
        return Err(PreviewError::Failed {
            detail: format!("{} has no page to open for its files", share.host.name()),
        });
    }
    Ok(BrowserCheck {
        site: share.host.name().to_owned(),
        pages,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::hosts::mediafire::MediaFireFile;

    const PAGE: &str = "https://www.mediafire.com/file/x7k2j9q4zbbwm1n";
    const DOWNLOAD: &str =
        "https://download2390.mediafire.com/gf8b0u3kzm4g/x7k2j9q4zbbwm1n/Pack_v2.zip";

    fn mediafire(key: &str) -> RemoteFile {
        RemoteFile {
            path: PathBuf::from(format!("{key}.zip")),
            size: 1,
            source: FileSource::MediaFire(MediaFireFile {
                quick_key: key.to_owned(),
                sha256: None,
                direct: None,
            }),
        }
    }

    fn share(host: Host, files: Vec<RemoteFile>) -> Share {
        Share {
            host,
            id: "folder".to_owned(),
            url: "https://example.com/folder".to_owned(),
            title: "Folder".to_owned(),
            upload_date: None,
            files,
        }
    }

    fn page(status: u16, challenged: bool, html: &str) -> Page {
        Page {
            url: PAGE.to_owned(),
            status: StatusCode::from_u16(status).unwrap(),
            html: html.to_owned(),
            challenged,
        }
    }

    #[test]
    fn knows_cloudflares_check_from_an_ordinary_page() {
        assert!(
            is_browser_check(&page(403, true, "")),
            "marked by Cloudflare"
        );
        assert!(is_browser_check(&page(
            403,
            false,
            "<title>Just a moment...</title><script>window._cf_chl_opt = {};</script>"
        )));
        assert!(is_browser_check(&page(
            503,
            false,
            "<script>window._cf_chl_opt={cvId: '3'}</script>"
        )));
        assert!(
            !is_browser_check(&page(
                200,
                false,
                r#"<script src="/cdn-cgi/challenge-platform/scripts/jsd/main.js"></script><a id="downloadButton">"#
            )),
            "every MediaFire page loads the challenge platform's script"
        );
        assert!(!is_browser_check(&page(403, false, "Forbidden")));
        assert_eq!(
            Host::GoogleDrive.page_error(&page(503, true, "")),
            PreviewError::BrowserCheck {
                site: "Google Drive".to_owned()
            },
            "any host's check is one"
        );
    }

    #[test]
    fn a_files_page_takes_only_what_reaches_its_own_file() {
        let folder = share(
            Host::MediaFire,
            vec![mediafire("x7k2j9q4zbbwm1n"), mediafire("otherkey123")],
        );
        let page = CheckPage::for_file(&folder, &folder.files[0]).unwrap();
        assert_eq!(page.url, PAGE);
        assert!(page.on_site("www.mediafire.com") && page.on_site("download2390.mediafire.com"));
        assert!(!page.on_site("econventa.com") && !page.on_site("notmediafire.com"));
        let caught = page.caught_link("https://download9.mediafire.com/t/x/Pack.zip");
        assert!(page.takes(&caught));
        assert!(
            page.takes(DOWNLOAD),
            "a download link of MediaFire's own, pasted"
        );
        assert!(
            !page.takes("https://download1.mediafire.com/abc/otherkey123/Other.zip"),
            "another file"
        );
        assert!(!page.takes(PAGE), "the page itself");
        assert!(
            !page.takes(&windowed::caught_link("otherkey123", DOWNLOAD)),
            "what the window gave another file"
        );
        assert_eq!(
            page.caught_in(&[caught.clone(), DOWNLOAD.to_owned()]),
            Some(Caught::Address(DOWNLOAD)),
            "the latest"
        );
        assert_eq!(
            page.caught_in(&[caught]),
            Some(Caught::Address(
                "https://download9.mediafire.com/t/x/Pack.zip"
            ))
        );

        let dropbox_file = || RemoteFile {
            path: PathBuf::from("a.zip"),
            size: 1,
            source: FileSource::Dropbox(crate::hosts::dropbox::DropboxFile {
                url: "https://www.dropbox.com/scl/fi/a/a.zip?dl=1".to_owned(),
            }),
        };
        let dropbox_folder = share(Host::Dropbox, vec![dropbox_file(), dropbox_file()]);
        assert_eq!(
            CheckPage::for_file(&dropbox_folder, &dropbox_folder.files[0]),
            None,
            "a folder's file with no page of its own"
        );
        let one = share(Host::Dropbox, vec![dropbox_file()]);
        assert_eq!(
            CheckPage::for_file(&one, &one.files[0]).map(|page| page.url),
            Some("https://example.com/folder".to_owned()),
            "the link's own page, for a share of one file"
        );

        let mut check = BrowserCheck {
            site: "MediaFire".to_owned(),
            pages: folder
                .files
                .iter()
                .filter_map(|file| CheckPage::for_file(&folder, file))
                .collect(),
        };
        assert!(check.takes(DOWNLOAD));
        assert!(!check.passed("https://download1.mediafire.com/abc/nokey/None.zip"));
        assert!(check.passed(DOWNLOAD));
        assert_eq!(check.pages.len(), 1);
        assert!(!check.takes(DOWNLOAD), "passed already");
    }
}
