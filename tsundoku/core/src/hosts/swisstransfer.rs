//! SwissTransfer: a transfer's link ("swisstransfer.com/dl/<link>", or the older "/d/<link>"), read the way its own
//! download page reads it, asked as Tsundoku. The page carries its data as JSON for its script (Inertia's
//! `data-page`): which page it is (`component`: the transfer, its password form or an error), and the transfer's
//! title and files with their folders and sizes. `/api/1/links/<link>/files/<file>` gives a file's address, signed
//! for an hour, so it's asked when the download starts. Every request names the link, so nothing outside it is
//! reached. Transfers expire after up to 30 days.

use std::path::PathBuf;

use reqwest::blocking::Client;
use reqwest::{StatusCode, Url};
use serde::Deserialize;

use super::{
    Address, FileCheck, FileSource, Host, HostFacts, Page, RemoteFile, SendChecked, Share, between,
    check_cancel, code_error, safe_name,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "SwissTransfer",
    site: "swisstransfer",
    domains: &["swisstransfer.com"],
    // Its addresses ask the storage to send each file as an attachment.
    sends_attachments: true,
    // It answers Tsundoku saying who it is as it answers a browser.
    browser_agent: false,
    read,
    page_error: None,
    window_link: None,
    download_file_id: None,
};

const SITE: &str = "https://www.swisstransfer.com";
/// Where the page's data starts.
const PAGE_DATA: &str = "<script data-page=\"app\" type=\"application/json\">";

/// A file to download from SwissTransfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwissTransferFile {
    pub link_id: String,
    pub file_id: String,
}

/// Reads a SwissTransfer link: the transfer's files with their folders and sizes.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let parsed = parse_link(link).ok_or_else(|| Host::SwissTransfer.link_not_recognized())?;
    let mut data = open_page(client, &parsed.page(), cancel)?;
    // An older "/d/" link to a transfer of the current site leads nowhere: its "/dl/" page has it.
    if parsed.old_form && data.is_missing() {
        data = open_page(client, &parsed.current_page(), cancel)?;
    }
    let (link_id, transfer) = data.transfer()?;
    let several = transfer.files.len() > 1;
    let title = transfer
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            (!several)
                .then(|| transfer.files.first().map(|file| file.name().to_owned()))
                .flatten()
        })
        .unwrap_or_else(|| format!("SwissTransfer {link_id}"));
    let top = PathBuf::from(safe_name(&title));
    let mut files: Vec<RemoteFile> = transfer
        .files
        .iter()
        .map(|file| {
            // A file sent inside a folder carries the folder in its path ("Photos/a.jpg").
            let inside = file
                .path
                .split('/')
                .filter(|part| !part.is_empty())
                .map(safe_name)
                .collect::<PathBuf>();
            RemoteFile {
                path: if several { top.join(inside) } else { inside },
                size: file.size,
                source: FileSource::SwissTransfer(SwissTransferFile {
                    link_id: link_id.clone(),
                    file_id: file.id.clone(),
                }),
            }
        })
        .collect();
    files.sort_by(|first, second| first.path.cmp(&second.path));
    Ok(Share {
        host: Host::SwissTransfer,
        id: link_id.clone(),
        url: format!("{SITE}/dl/{link_id}"),
        title,
        upload_date: None,
        files,
    })
}

/// Asks SwissTransfer where a file downloads from now.
pub(super) fn address(
    client: &Client,
    file: &SwissTransferFile,
    cancel: &CancelToken,
) -> Result<Address, PreviewError> {
    check_cancel(cancel)?;
    let response = client
        .get(format!(
            "{SITE}/api/1/links/{}/files/{}",
            file.link_id, file.file_id
        ))
        .send_checked(FACTS.name)?;
    if let Some(error) = code_error(Host::SwissTransfer, response.status()) {
        return Err(match error {
            PreviewError::Failed { detail } | PreviewError::Unavailable { detail } => {
                PreviewError::Unavailable {
                    detail: format!("{detail}: the transfer may have expired"),
                }
            }
            error => error,
        });
    }
    let answer: FileAnswer = response.json().map_err(|error| PreviewError::Failed {
        detail: format!("SwissTransfer's answer wasn't as expected: {error}"),
    })?;
    match answer.data.map(|data| data.url) {
        Some(url) if answer.result == "success" && url.starts_with("https://") => Ok(Address {
            url,
            check: FileCheck::None,
        }),
        _ => Err(PreviewError::Unavailable {
            detail: format!(
                "SwissTransfer gave no address to download from ({})",
                answer.result
            ),
        }),
    }
}

#[derive(Debug, Deserialize)]
struct FileAnswer {
    #[serde(default)]
    result: String,
    #[serde(default)]
    data: Option<FileAddress>,
}

#[derive(Debug, Deserialize)]
struct FileAddress {
    url: String,
}

/// The data a SwissTransfer page carries for its script.
#[derive(Debug, Deserialize)]
struct PageData {
    /// Which page it is: "link/show" for a transfer, "link/password", or "errors/…".
    #[serde(default)]
    component: String,
    #[serde(default)]
    props: Props,
}

#[derive(Debug, Default, Deserialize)]
struct Props {
    #[serde(default)]
    link: Option<LinkProps>,
    #[serde(default)]
    transfer: Option<Transfer>,
}

#[derive(Debug, Deserialize)]
struct LinkProps {
    id: String,
}

#[derive(Debug, Deserialize)]
struct Transfer {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    files: Vec<TransferFile>,
}

#[derive(Debug, Deserialize)]
struct TransferFile {
    id: String,
    /// Its name, after any folders it was sent in.
    path: String,
    #[serde(default)]
    size: u64,
}

impl TransferFile {
    fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

impl PageData {
    fn missing() -> Self {
        Self {
            component: "errors/not-found".to_owned(),
            props: Props::default(),
        }
    }

    /// The page of a link SwissTransfer doesn't know (any longer).
    fn is_missing(&self) -> bool {
        matches!(
            self.component.as_str(),
            "errors/not-found" | "errors/transfer/not-found"
        )
    }

    /// The link's ID and its transfer, or why the page doesn't show one.
    fn transfer(self) -> Result<(String, Transfer), PreviewError> {
        let unavailable = |why: &str| PreviewError::Unavailable {
            detail: format!("SwissTransfer {why}"),
        };
        match self.component.as_str() {
            "link/show" => {}
            "link/password" => {
                return Err(PreviewError::PasswordProtected {
                    site: FACTS.name.to_owned(),
                });
            }
            "errors/not-found" | "errors/transfer/not-found" => {
                return Err(unavailable(
                    "doesn't know this transfer: it expired, or was deleted",
                ));
            }
            "errors/transfer/unavailable" => {
                return Err(unavailable(
                    "says the transfer is no longer available: it expired, or its downloads are used up",
                ));
            }
            "errors/transfer/infected" => {
                return Err(unavailable("found a virus in the transfer"));
            }
            "errors/transfer/country-blocked" => {
                return Err(unavailable("doesn't offer the transfer in this country"));
            }
            "errors/transfer/waiting-for-antivirus" => {
                // Downloadable once checked, so the queue tries again a little later.
                return Err(PreviewError::Network {
                    detail: "SwissTransfer is still checking the transfer for viruses".to_owned(),
                });
            }
            other => {
                return Err(PreviewError::Failed {
                    detail: format!("SwissTransfer showed an unexpected page ({other})"),
                });
            }
        }
        match (self.props.link, self.props.transfer) {
            (Some(link), Some(transfer)) => Ok((link.id, transfer)),
            _ => Err(PreviewError::Failed {
                detail: "SwissTransfer's page had no transfer".to_owned(),
            }),
        }
    }
}

/// The page at `url` and the data it carries. A link SwissTransfer doesn't know redirects to its "not found" page.
fn open_page(client: &Client, url: &str, cancel: &CancelToken) -> Result<PageData, PreviewError> {
    check_cancel(cancel)?;
    let response = client.get(url).send_checked(FACTS.name)?;
    let page = Page::read(response);
    // Its "not found" page answers 404.
    if page.status == StatusCode::NOT_FOUND {
        return Ok(page_data(&page.html).unwrap_or_else(PageData::missing));
    }
    if let Some(error) = code_error(Host::SwissTransfer, page.status) {
        return Err(error);
    }
    page_data(&page.html).ok_or_else(|| PreviewError::Failed {
        detail: "SwissTransfer's page had none of its data".to_owned(),
    })
}

fn page_data(html: &str) -> Option<PageData> {
    serde_json::from_str(between(html, PAGE_DATA, "</script>")?).ok()
}

/// A SwissTransfer link's ID, and whether it's the older "/d/" form.
#[derive(Debug, PartialEq, Eq)]
struct Parsed {
    id: String,
    old_form: bool,
}

impl Parsed {
    fn page(&self) -> String {
        if self.old_form {
            format!("{SITE}/d/{}", self.id)
        } else {
            self.current_page()
        }
    }

    fn current_page(&self) -> String {
        format!("{SITE}/dl/{}", self.id)
    }
}

/// The link's ID in "/dl/<id>" or "/d/<id>", after a language ("/fr/d/<id>") if there is one.
fn parse_link(link: &str) -> Option<Parsed> {
    let url = Url::parse(link).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !Host::SwissTransfer.owns(&host) {
        return None;
    }
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    let (kind, id) = match segments.as_slice() {
        [kind, id, ..] if matches!(*kind, "d" | "dl") => (*kind, *id),
        [language, kind, id, ..] if language.len() == 2 && matches!(*kind, "d" | "dl") => {
            (*kind, *id)
        }
        _ => return None,
    };
    let is_id = id.len() == 36
        && id
            .chars()
            .all(|character| character.is_ascii_hexdigit() || character == '-');
    is_id.then(|| Parsed {
        id: id.to_ascii_lowercase(),
        old_form: kind == "d",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SwissTransfer's page for a transfer of one file on 2026-11-11, trimmed.
    const TRANSFER_PAGE: &str = r#"<html><head></head><body>
    <script data-page="app" type="application/json">{"component":"link\/show","props":{"errors":{},"user":null,
    "link":{"id":"01a1cbc9-34ef-733b-9656-12f2d26023dd","report_url":"https:\/\/infomaniak.com\/en\/report_abuse"},
    "transfer":{"id":"01a1cbc9-34ec-7370-b496-c32660920f00","account_id":null,"title":"SampleArchive","message":null,
    "total_size":16725559,"expires_at":1794510000,"files":[{"id":"01a1cbc9-34f1-7321-95ec-2dce43052a46",
    "path":"0ff81356666095988ed08738b9bab5bd9a6c9e857311ab8665e0cab1d1730cd9.jpg","size":16725559,
    "mime_type":"image\/jpeg","mime_type_group":"image"}],"type":"link"}},"url":"\/dl\/01a1cbc9-34ef-733b-9656-12f2d26023dd","version":"app:1.3.0"}</script>
    </body></html>"#;

    fn page(component: &str) -> String {
        format!(r#"{PAGE_DATA}{{"component":"{component}","props":{{"errors":{{}}}}}}</script>"#)
    }

    #[test]
    fn reads_its_links() {
        for (link, parsed) in [
            (
                "https://www.swisstransfer.com/dl/01a1cbc9-34ef-733b-9656-12f2d26023dd",
                Some(("01a1cbc9-34ef-733b-9656-12f2d26023dd", false)),
            ),
            (
                "https://swisstransfer.com/d/01A1CBC9-34EF-733B-9656-12F2D26023DD",
                Some(("01a1cbc9-34ef-733b-9656-12f2d26023dd", true)),
            ),
            (
                "https://www.swisstransfer.com/fr/d/0a1b2c3d-4e5f-6a7b-8c9d-0e1f2a3b4c5d",
                Some(("0a1b2c3d-4e5f-6a7b-8c9d-0e1f2a3b4c5d", true)),
            ),
            ("https://www.swisstransfer.com/", None),
            ("https://www.swisstransfer.com/dl/not-an-id", None),
            ("https://www.swisstransfer.com/en/transfers", None),
        ] {
            assert_eq!(
                parse_link(link).map(|parsed| (parsed.id, parsed.old_form)),
                parsed.map(|(id, old)| (id.to_owned(), old)),
                "{link}"
            );
        }
        assert_eq!(
            Host::of("https://www.swisstransfer.com/dl/01a1cbc9-34ef-733b-9656-12f2d26023dd"),
            Some(Host::SwissTransfer)
        );
    }

    #[test]
    fn reads_a_transfer_from_its_page() {
        let data = page_data(TRANSFER_PAGE).unwrap();
        assert!(!data.is_missing());
        let (link_id, transfer) = data.transfer().unwrap();
        assert_eq!(link_id, "01a1cbc9-34ef-733b-9656-12f2d26023dd");
        assert_eq!(transfer.title.as_deref(), Some("SampleArchive"));
        assert_eq!(transfer.files.len(), 1);
        assert_eq!(transfer.files[0].size, 16_725_559);
        assert_eq!(transfer.files[0].name(), transfer.files[0].path);
        let nested = TransferFile {
            id: "a".to_owned(),
            path: "Photos/2024/a.jpg".to_owned(),
            size: 1,
        };
        assert_eq!(nested.name(), "a.jpg");
    }

    #[test]
    fn explains_the_pages_without_a_transfer() {
        assert!(matches!(
            page_data(&page("link/password")).unwrap().transfer(),
            Err(PreviewError::PasswordProtected { site }) if site == "SwissTransfer"
        ));
        for component in ["errors/not-found", "errors/transfer/not-found"] {
            let data = page_data(&page(component)).unwrap();
            assert!(data.is_missing());
            assert!(matches!(
                data.transfer(),
                Err(PreviewError::Unavailable { detail }) if detail.contains("expired")
            ));
        }
        assert!(matches!(
            page_data(&page("errors/transfer/unavailable")).unwrap().transfer(),
            Err(PreviewError::Unavailable { detail }) if detail.contains("used up")
        ));
        assert!(matches!(
            page_data(&page("errors/transfer/waiting-for-antivirus"))
                .unwrap()
                .transfer(),
            Err(PreviewError::Network { .. })
        ));
        assert!(page_data("<html>Maintenance</html>").is_none());
    }

    #[test]
    fn reads_a_files_address() {
        let answer: FileAnswer = serde_json::from_str(
            r#"{"result":"success","data":{"url":"https:\/\/x.swisstransfer.infomaniak.cloud\/t\/f?response-content-disposition=attachment"}}"#,
        )
        .unwrap();
        assert_eq!(answer.result, "success");
        assert!(
            answer
                .data
                .unwrap()
                .url
                .starts_with("https://x.swisstransfer")
        );
    }
}
