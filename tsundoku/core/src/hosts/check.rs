//! A check only a person can pass, such as Cloudflare's browser check in front of some MediaFire files' pages.
//! Tsundoku never tries to get past one itself: the download waits in the queue until the person opens the check in
//! Tsundoku's check window, a browser view of the site's own page. Once they've done what it asks and started the
//! download on the page, the window catches that download's address, which the download then uses as the file's
//! direct link (MediaFire's `MediaFireFile::direct`).

use super::{FileSource, Host, Page, Share};
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
    /// The host's ID for the file, to know the download the page starts.
    file_id: String,
}

impl CheckPage {
    /// The page for a file of a share, if its host has one a person can open: MediaFire's file page.
    pub(crate) fn of(source: &FileSource) -> Option<Self> {
        match source {
            FileSource::MediaFire(file) => Some(Self {
                url: super::mediafire::file_page(&file.quick_key),
                host: Host::MediaFire,
                file_id: file.quick_key.clone(),
            }),
            FileSource::Mega(_)
            | FileSource::GoogleDrive(_)
            | FileSource::Dropbox(_)
            | FileSource::OneDrive(_)
            | FileSource::ArchiveOrg(_)
            | FileSource::WeTransfer(_)
            | FileSource::PCloud(_) => None,
        }
    }

    /// Whether `host`, a page's host name, is on the page's site, such as MediaFire's download servers for its file
    /// pages: the check window doesn't leave the site.
    pub fn on_site(&self, host: &str) -> bool {
        self.host.owns(host)
    }

    /// Whether `download`, the address of a download a page started, is this page's file: anything else (another
    /// file, an advert's download) is left alone.
    pub fn takes(&self, download: &str) -> bool {
        caught_file(self.host, download).is_some_and(|id| id == self.file_id)
    }
}

impl BrowserCheck {
    /// Whether one of its pages takes `download` (`CheckPage::takes`).
    pub fn takes(&self, download: &str) -> bool {
        self.pages.iter().any(|page| page.takes(download))
    }

    /// Leaves out the pages `download` gets past, once it's been caught or pasted. Whether any did.
    pub fn passed(&mut self, download: &str) -> bool {
        let before = self.pages.len();
        self.pages.retain(|page| !page.takes(download));
        self.pages.len() < before
    }
}

/// The file a download address caught on a host's page gets, by the host's ID for it: MediaFire's Download button
/// leads to `download<n>.mediafire.com/<token>/<key>/<name>`.
fn caught_file(host: Host, download: &str) -> Option<String> {
    match host {
        Host::MediaFire => super::mediafire::direct_link_key(download),
        Host::Mega
        | Host::GoogleDrive
        | Host::Dropbox
        | Host::OneDrive
        | Host::ArchiveOrg
        | Host::WeTransfer
        | Host::PCloud => None,
    }
}

impl Share {
    /// Gives each file the address caught for it in the check window or pasted (`links`), to download from while it
    /// still leads to the file. A later link for a file replaces an earlier one.
    pub fn use_caught_links(&mut self, links: &[String]) {
        for file in &mut self.files {
            let FileSource::MediaFire(file) = &mut file.source else {
                continue;
            };
            if let Some(link) = links.iter().rev().find(|link| {
                super::mediafire::direct_link_key(link).as_deref() == Some(&file.quick_key)
            }) {
                file.direct = Some(link.clone());
            }
        }
    }
}

/// Whether a page a host answered is Cloudflare's check for a browser ("Just a moment..."). Cloudflare marks its
/// challenges with `cf-mitigated: challenge`; without the header, a 403 or 503 carrying the challenge's own options
/// (`_cf_chl_opt`) or its title is one too. Its "challenge-platform" script is on ordinary pages of some sites
/// (every MediaFire page), so it isn't a sign.
pub(crate) fn is_browser_check(page: &Page) -> bool {
    page.challenged
        || (matches!(page.status.as_u16(), 403 | 503)
            && (page.html.contains("_cf_chl_opt")
                || page.html.contains("<title>Just a moment...</title>")))
}

/// Every file of `link` as a check's pages, for a development build to open the check window on a link of its choice.
pub fn for_link(link: &str, cancel: &CancelToken) -> Result<BrowserCheck, PreviewError> {
    let share = super::read(link, cancel)?;
    let pages: Vec<CheckPage> = share
        .files
        .iter()
        .filter_map(|file| CheckPage::of(&file.source))
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

    use reqwest::StatusCode;

    use super::*;
    use crate::hosts::RemoteFile;
    use crate::hosts::mediafire::MediaFireFile;

    const PAGE: &str = "https://www.mediafire.com/file/x7k2j9q4zbbwm1n";
    const DOWNLOAD: &str =
        "https://download2390.mediafire.com/gf8b0u3kzm4g/x7k2j9q4zbbwm1n/Pack_v2.zip";

    fn mediafire(key: &str) -> FileSource {
        FileSource::MediaFire(MediaFireFile {
            quick_key: key.to_owned(),
            sha256: None,
            direct: None,
        })
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
            Host::MediaFire.page_error(&page(403, true, "")),
            PreviewError::BrowserCheck {
                site: "MediaFire".to_owned()
            }
        );
        assert_eq!(
            Host::GoogleDrive.page_error(&page(503, true, "")),
            PreviewError::BrowserCheck {
                site: "Google Drive".to_owned()
            },
            "any host's check is one"
        );
    }

    #[test]
    fn a_files_page_takes_only_its_own_download() {
        let page = CheckPage::of(&mediafire("x7k2j9q4zbbwm1n")).unwrap();
        assert_eq!(page.url, PAGE);
        assert!(page.on_site("www.mediafire.com") && page.on_site("download2390.mediafire.com"));
        assert!(!page.on_site("econventa.com") && !page.on_site("notmediafire.com"));
        assert!(page.takes(DOWNLOAD));
        assert!(
            !page.takes("https://download1.mediafire.com/abc/otherkey123/Other.zip"),
            "another file"
        );
        assert!(
            !page.takes("https://ads.example.com/x7k2j9q4zbbwm1n/setup.exe"),
            "an advert"
        );
        assert!(!page.takes(PAGE), "the page itself");
        assert_eq!(
            CheckPage::of(&FileSource::Dropbox(crate::hosts::dropbox::DropboxFile {
                url: "https://www.dropbox.com/scl/fi/a/b.zip?dl=1".to_owned()
            })),
            None,
            "no page to open"
        );

        let mut check = BrowserCheck {
            site: "MediaFire".to_owned(),
            pages: ["x7k2j9q4zbbwm1n", "otherkey123"]
                .map(|key| CheckPage::of(&mediafire(key)).unwrap())
                .to_vec(),
        };
        assert!(check.takes(DOWNLOAD));
        assert!(!check.passed("https://download1.mediafire.com/abc/nokey/None.zip"));
        assert!(check.passed(DOWNLOAD));
        assert_eq!(check.pages.len(), 1);
        assert!(!check.takes(DOWNLOAD), "passed already");
    }

    #[test]
    fn a_caught_link_becomes_its_files_direct_link() {
        let file = |key: &str| RemoteFile {
            path: PathBuf::from(format!("{key}.zip")),
            size: 1,
            source: mediafire(key),
        };
        let mut share = Share {
            host: Host::MediaFire,
            id: "folder".to_owned(),
            url: "https://www.mediafire.com/folder/folder".to_owned(),
            title: "Folder".to_owned(),
            upload_date: None,
            files: vec![file("x7k2j9q4zbbwm1n"), file("otherkey123")],
        };
        let newer = DOWNLOAD.replace("gf8b0u3kzm4g", "newtoken");
        share.use_caught_links(&[DOWNLOAD.to_owned(), newer.clone()]);
        let direct: Vec<Option<String>> = share
            .files
            .iter()
            .map(|file| match &file.source {
                FileSource::MediaFire(file) => file.direct.clone(),
                _ => None,
            })
            .collect();
        assert_eq!(
            direct,
            [Some(newer), None],
            "the latest link, for its own file only"
        );
    }
}
