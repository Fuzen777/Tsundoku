//! WeTransfer: a transfer's link ("we.tl/t-…", or "wetransfer.com/downloads/<transfer>/<hash>"), read the way
//! WeTransfer's own download page reads it, without signing in. `prepare-download` gives the transfer's files with
//! their sizes, and whether it can still be downloaded; `download` gives a file's address, which expires, so it's
//! asked when the download starts. Every request names the transfer and its security hash, so nothing outside it is
//! reached. Transfers expire after a few days.

use std::path::PathBuf;

use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, CONTENT_TYPE, LOCATION};
use reqwest::{StatusCode, Url};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::{
    Address, FileCheck, FileSource, Host, HostFacts, RemoteFile, Share, check_cancel, code_error,
    network_error, safe_name,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "WeTransfer",
    site: "wetransfer",
    domains: &["wetransfer.com", "we.tl"],
    sends_attachments: false,
    // It answers Tsundoku saying who it is as it answers a browser.
    browser_agent: false,
    read,
    page_error: None,
    window: None,
};

const API: &str = "https://wetransfer.com/api/v4/transfers";

/// A file to download from WeTransfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeTransferFile {
    pub transfer: TransferKey,
    pub file_id: String,
}

/// What names a transfer: its ID and security hash, and the recipient's ID when it was sent by email.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferKey {
    pub id: String,
    pub security_hash: String,
    pub recipient_id: Option<String>,
}

impl TransferKey {
    fn body(&self, more: Value) -> Value {
        let mut body = json!({ "security_hash": self.security_hash });
        if let Some(recipient) = &self.recipient_id {
            body["recipient_id"] = json!(recipient);
        }
        if let (Value::Object(body), Value::Object(more)) = (&mut body, more) {
            body.extend(more);
        }
        body
    }
}

/// Reads a WeTransfer link: the transfer's files with their sizes.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let key = match parse_link(link) {
        Some(Parsed::Transfer(key)) => key,
        Some(Parsed::Short) => resolve(link, cancel)?,
        None => return Err(Host::WeTransfer.link_not_recognized()),
    };
    let transfer: Transfer = post(
        client,
        &format!("{API}/{}/prepare-download", key.id),
        &key.body(json!({})),
        cancel,
    )?;
    transfer.downloadable()?;
    let several = transfer.items.len() > 1;
    let top = PathBuf::from(safe_name(&transfer.display_name));
    let mut files: Vec<RemoteFile> = transfer
        .items
        .iter()
        .filter(|item| item.item_type.as_deref().is_none_or(|kind| kind == "file"))
        .map(|item| {
            // A file sent inside a folder carries the folder in its name ("Photos/a.jpg").
            let inside = item
                .name
                .split('/')
                .filter(|part| !part.is_empty())
                .map(safe_name)
                .collect::<PathBuf>();
            RemoteFile {
                path: if several { top.join(inside) } else { inside },
                size: item.size,
                source: FileSource::WeTransfer(WeTransferFile {
                    transfer: key.clone(),
                    file_id: item.id.clone(),
                }),
            }
        })
        .collect();
    files.sort_by(|first, second| first.path.cmp(&second.path));
    Ok(Share {
        host: Host::WeTransfer,
        id: key.id,
        url: link.to_owned(),
        upload_date: transfer
            .uploaded_at
            .as_deref()
            .and_then(|date| date.get(..10))
            .map(str::to_owned),
        title: transfer.display_name,
        files,
    })
}

/// Asks WeTransfer where a file downloads from now.
pub(super) fn address(
    client: &Client,
    file: &WeTransferFile,
    cancel: &CancelToken,
) -> Result<Address, PreviewError> {
    let answer: DownloadAnswer = post(
        client,
        &format!("{API}/{}/download", file.transfer.id),
        &file.transfer.body(json!({
            "intent": "single_file",
            "file_ids": [file.file_id],
        })),
        cancel,
    )?;
    if !answer.direct_link.starts_with("https://") {
        return Err(PreviewError::Failed {
            detail: "WeTransfer gave no address to download from".to_owned(),
        });
    }
    Ok(Address {
        url: answer.direct_link,
        check: FileCheck::None,
    })
}

#[derive(Debug, Deserialize)]
struct Transfer {
    #[serde(default)]
    state: String,
    #[serde(default)]
    display_name: String,
    /// "2026-11-05T16:11:41Z".
    #[serde(default)]
    uploaded_at: Option<String>,
    #[serde(default)]
    password_protected: bool,
    /// "anonymous" when anyone with the link may download; otherwise the downloader must confirm an email address.
    #[serde(default)]
    downloader_email_verification: Option<String>,
    #[serde(default)]
    items: Vec<Item>,
}

impl Transfer {
    /// Why it can't be downloaded, if it can't.
    fn downloadable(&self) -> Result<(), PreviewError> {
        let site = || Host::WeTransfer.name().to_owned();
        if self.password_protected {
            return Err(PreviewError::PasswordProtected { site: site() });
        }
        if self.state != "downloadable" {
            return Err(PreviewError::Unavailable {
                detail: format!("WeTransfer says the transfer is {}", self.state),
            });
        }
        if self
            .downloader_email_verification
            .as_deref()
            .is_some_and(|verification| verification != "anonymous")
        {
            return Err(PreviewError::Unavailable {
                detail:
                    "WeTransfer asks whoever downloads this transfer to confirm an email address"
                        .to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct Item {
    id: String,
    name: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    item_type: Option<String>,
}

#[derive(Deserialize)]
struct DownloadAnswer {
    direct_link: String,
}

/// Asks WeTransfer's API, sending `body` as JSON.
fn post<T: DeserializeOwned>(
    client: &Client,
    url: &str,
    body: &Value,
    cancel: &CancelToken,
) -> Result<T, PreviewError> {
    check_cancel(cancel)?;
    let response = client
        .post(url)
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .map_err(network_error)?;
    let status = response.status();
    if status == StatusCode::NOT_FOUND {
        return Err(PreviewError::Unavailable {
            detail: "WeTransfer says the transfer doesn't exist: it expired or was deleted"
                .to_owned(),
        });
    }
    if let Some(error) = code_error(Host::WeTransfer, status) {
        return Err(error);
    }
    response.json().map_err(|error| PreviewError::Failed {
        detail: format!("WeTransfer's answer wasn't as expected: {error}"),
    })
}

/// The transfer a short link leads to, from where it redirects.
fn resolve(link: &str, cancel: &CancelToken) -> Result<TransferKey, PreviewError> {
    check_cancel(cancel)?;
    let response = super::client_without_redirects(Host::WeTransfer)?
        .get(link)
        .send()
        .map_err(network_error)?;
    let location = response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    match parse_link(location) {
        Some(Parsed::Transfer(key)) => Ok(key),
        _ => Err(PreviewError::Unavailable {
            detail: "WeTransfer's short link leads to no transfer: it expired or was deleted"
                .to_owned(),
        }),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Parsed {
    /// "we.tl/t-<code>", which redirects to the transfer.
    Short,
    Transfer(TransferKey),
}

/// Reads a WeTransfer link: "we.tl/t-<code>", or "wetransfer.com/downloads/<transfer>/<hash>" (on any of its
/// subdomains), with the recipient's ID between them when the transfer was sent by email.
fn parse_link(link: &str) -> Option<Parsed> {
    let url = Url::parse(link).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    if host == "we.tl" {
        return matches!(segments.as_slice(), [code] if code.starts_with("t-") && code.len() > 2)
            .then_some(Parsed::Short);
    }
    if !Host::WeTransfer.owns(&host) {
        return None;
    }
    let is_hex = |text: &str| {
        !text.is_empty() && text.chars().all(|character| character.is_ascii_hexdigit())
    };
    let key = |id: &str, recipient: Option<&str>, hash: &str| {
        Parsed::Transfer(TransferKey {
            id: id.to_owned(),
            security_hash: hash.to_owned(),
            recipient_id: recipient.map(str::to_owned),
        })
    };
    match segments.as_slice() {
        ["downloads", id, hash] if is_hex(id) && is_hex(hash) => Some(key(id, None, hash)),
        ["downloads", id, recipient, hash] if is_hex(id) && is_hex(recipient) && is_hex(hash) => {
            Some(key(id, Some(recipient), hash))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "b7bbf8d32a5bcb88def79c05861a343720261105161137";

    fn key(recipient: Option<&str>) -> Parsed {
        Parsed::Transfer(TransferKey {
            id: ID.to_owned(),
            security_hash: "c0775b".to_owned(),
            recipient_id: recipient.map(str::to_owned),
        })
    }

    #[test]
    fn reads_its_links() {
        for (link, expected) in [
            ("https://we.tl/t-N7unwF1G6sAeqj5c", Some(Parsed::Short)),
            (
                &format!("https://wetransfer.com/downloads/{ID}/c0775b") as &str,
                Some(key(None)),
            ),
            (
                &format!(
                    "https://rendermedia.wetransfer.com/downloads/{ID}/c0775b?t_exp=1796487098&t_s=download_link"
                ),
                Some(key(None)),
            ),
            (
                &format!("https://wetransfer.com/downloads/{ID}/0123abcd/c0775b"),
                Some(key(Some("0123abcd"))),
            ),
            ("https://we.tl/", None),
            ("https://wetransfer.com/pricing", None),
            ("https://wetransfer.com/downloads/notHex/c0775b", None),
        ] {
            assert_eq!(parse_link(link), expected, "{link}");
        }
        assert_eq!(
            Host::of("https://we.tl/t-N7unwF1G6sAeqj5c"),
            Some(Host::WeTransfer)
        );
        let body = TransferKey {
            id: ID.to_owned(),
            security_hash: "c0775b".to_owned(),
            recipient_id: Some("0123".to_owned()),
        }
        .body(json!({"intent": "single_file", "file_ids": ["f1"]}));
        assert_eq!(
            body,
            json!({"security_hash": "c0775b", "recipient_id": "0123", "intent": "single_file", "file_ids": ["f1"]})
        );
    }

    #[test]
    fn reads_a_transfer_and_says_when_it_cant_be_downloaded() {
        // WeTransfer's answer, trimmed.
        let transfer: Transfer = serde_json::from_str(
            r#"{"state": "downloadable", "display_name": "random-newlines-example.pptx", "uploaded_at": "2026-11-05T16:11:41Z",
            "password_protected": false, "downloader_email_verification": "anonymous", "size": 37866,
            "items": [{"id": "435f80d3853626b5b79a8353cee7496020261105161141", "name": "random-newlines-example.pptx",
            "size": 37866, "item_type": "file"}]}"#,
        )
        .unwrap();
        assert_eq!(transfer.downloadable(), Ok(()));
        assert_eq!(transfer.items[0].size, 37866);

        let expired: Transfer = serde_json::from_str(
            r#"{"state": "expired", "display_name": "NAV_VIDEO.MOV", "deleted_at": "2026-11-05 20:49:56 UTC", "items": []}"#,
        )
        .unwrap();
        assert!(matches!(
            expired.downloadable(),
            Err(PreviewError::Unavailable { detail }) if detail.contains("expired")
        ));
        let locked: Transfer =
            serde_json::from_str(r#"{"state": "downloadable", "password_protected": true}"#)
                .unwrap();
        assert_eq!(
            locked.downloadable(),
            Err(PreviewError::PasswordProtected {
                site: "WeTransfer".to_owned()
            })
        );
        let verified: Transfer = serde_json::from_str(
            r#"{"state": "downloadable", "downloader_email_verification": "verified"}"#,
        )
        .unwrap();
        assert!(matches!(
            verified.downloadable(),
            Err(PreviewError::Unavailable { .. })
        ));
    }
}
