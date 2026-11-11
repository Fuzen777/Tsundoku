//! TransferNow: a transfer's link ("transfernow.net/dl/<transfer>", or "/dl/<transfer>/<secret>" for one sent by
//! email), read the way its download page reads it, asked as Tsundoku. `downloads/metadata` gives the transfer's
//! files with their sizes, whether it needs a password and whether it's still there (`status` "ENABLED");
//! `downloads/link` gives a file's address, signed for a while, so it's asked when the download starts. Every request
//! names the transfer. Transfers expire after 7 days on the free plan.

use std::path::PathBuf;

use reqwest::blocking::Client;
use reqwest::{StatusCode, Url};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use super::{
    Address, FileCheck, FileSource, Host, HostFacts, RemoteFile, Share, check_cancel, code_error,
    network_error, safe_name,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "TransferNow",
    site: "transfernow",
    domains: &["transfernow.net"],
    // Its addresses ask the storage to send each file as an attachment.
    sends_attachments: true,
    // It answers Tsundoku saying who it is as it answers a browser.
    browser_agent: false,
    read,
    page_error: None,
};

const API: &str = "https://www.transfernow.net/api/transfer/downloads";

/// What names a transfer: its ID, and the secret a link sent by email carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferKey {
    pub id: String,
    pub secret: String,
}

/// A file to download from TransferNow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferNowFile {
    pub transfer: TransferKey,
    pub file_id: String,
}

/// Reads a TransferNow link: the transfer's files with their sizes.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let key = parse_link(link).ok_or_else(|| Host::TransferNow.link_not_recognized())?;
    let transfer: Transfer = ask(client, "metadata", &key, None, cancel)?;
    transfer.downloadable()?;
    let several = transfer.files.len() > 1;
    let title = transfer
        .transfer_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            (!several)
                .then(|| transfer.files.first().map(|file| file.name.clone()))
                .flatten()
        })
        .unwrap_or_else(|| format!("TransferNow {}", key.id));
    let top = PathBuf::from(safe_name(&title));
    let mut files: Vec<RemoteFile> = transfer
        .files
        .iter()
        .map(|file| {
            // A file sent inside a folder carries the folder in its name ("Photos/a.jpg").
            let inside: PathBuf = file
                .name
                .split('/')
                .filter(|part| !part.is_empty())
                .map(safe_name)
                .collect();
            RemoteFile {
                path: if several { top.join(inside) } else { inside },
                size: file.size,
                source: FileSource::TransferNow(TransferNowFile {
                    transfer: key.clone(),
                    file_id: file.id.clone(),
                }),
            }
        })
        .collect();
    files.sort_by(|first, second| first.path.cmp(&second.path));
    Ok(Share {
        host: Host::TransferNow,
        id: key.id.clone(),
        url: link.to_owned(),
        title,
        upload_date: transfer
            .validity
            .and_then(|validity| validity.from)
            .and_then(|from| from.get(..10).map(str::to_owned)),
        files,
    })
}

/// Asks TransferNow where a file downloads from now.
pub(super) fn address(
    client: &Client,
    file: &TransferNowFile,
    cancel: &CancelToken,
) -> Result<Address, PreviewError> {
    let answer: LinkAnswer = ask(client, "link", &file.transfer, Some(&file.file_id), cancel)?;
    if !answer.url.starts_with("https://") {
        return Err(PreviewError::Failed {
            detail: "TransferNow gave no address to download from".to_owned(),
        });
    }
    Ok(Address {
        url: answer.url,
        check: FileCheck::None,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Transfer {
    #[serde(default)]
    transfer_name: Option<String>,
    #[serde(default)]
    need_password: bool,
    #[serde(default)]
    status: String,
    #[serde(default)]
    validity: Option<Validity>,
    #[serde(default)]
    files: Vec<TransferFile>,
}

#[derive(Debug, Deserialize)]
struct Validity {
    /// When it was sent, as an ISO date and time.
    #[serde(default)]
    from: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TransferFile {
    id: String,
    name: String,
    #[serde(default)]
    size: u64,
}

#[derive(Debug, Deserialize)]
struct LinkAnswer {
    #[serde(default)]
    url: String,
}

impl Transfer {
    /// Why the transfer can't be downloaded, if it can't.
    fn downloadable(&self) -> Result<(), PreviewError> {
        if !self.status.eq_ignore_ascii_case("ENABLED") {
            return Err(PreviewError::Unavailable {
                detail: format!(
                    "TransferNow's transfer has expired or was deleted ({})",
                    self.status
                ),
            });
        }
        if self.need_password {
            return Err(PreviewError::PasswordProtected {
                site: FACTS.name.to_owned(),
            });
        }
        Ok(())
    }
}

/// Asks `downloads/<what>` about the transfer, or one of its files.
fn ask<T: DeserializeOwned>(
    client: &Client,
    what: &str,
    key: &TransferKey,
    file_id: Option<&str>,
    cancel: &CancelToken,
) -> Result<T, PreviewError> {
    check_cancel(cancel)?;
    let mut parameters = vec![
        ("transferId", key.id.as_str()),
        ("userSecret", key.secret.as_str()),
        ("preview", "false"),
    ];
    if let Some(file_id) = file_id {
        parameters.push(("fileId", file_id));
    }
    let url = Url::parse_with_params(&format!("{API}/{what}"), parameters).map_err(|error| {
        PreviewError::Failed {
            detail: error.to_string(),
        }
    })?;
    let response = client.get(url).send().map_err(network_error)?;
    if response.status() == StatusCode::NOT_FOUND {
        return Err(PreviewError::Unavailable {
            detail: "TransferNow doesn't know this transfer: it expired, or was deleted".to_owned(),
        });
    }
    if let Some(error) = code_error(Host::TransferNow, response.status()) {
        return Err(error);
    }
    response.json().map_err(|error| PreviewError::Failed {
        detail: format!("TransferNow's answer wasn't as expected: {error}"),
    })
}

/// The transfer in "/dl/<transfer>[/<secret>]" (after a language or not), or "dltransfer?utm_source=<transfer>
/// &utm_medium=<secret>".
fn parse_link(link: &str) -> Option<TransferKey> {
    let url = Url::parse(link).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !Host::TransferNow.owns(&host) {
        return None;
    }
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    let is_part = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
    };
    let rest = match segments.as_slice() {
        ["dl", rest @ ..] => rest,
        [language, "dl", rest @ ..] if language.len() == 2 => rest,
        ["dltransfer"] | [_, "dltransfer"] => {
            let query = |name: &str| {
                url.query_pairs()
                    .find(|(key, _)| key == name)
                    .map(|(_, value)| value.into_owned())
            };
            let id = query("utm_source")?;
            let secret = query("utm_medium").unwrap_or_default();
            return (is_part(&id) && (secret.is_empty() || is_part(&secret)))
                .then_some(TransferKey { id, secret });
        }
        _ => return None,
    };
    match rest {
        [id] if is_part(id) => Some(TransferKey {
            id: (*id).to_owned(),
            secret: String::new(),
        }),
        [id, secret, ..] if is_part(id) && is_part(secret) => Some(TransferKey {
            id: (*id).to_owned(),
            secret: (*secret).to_owned(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_its_links() {
        let key = |id: &str, secret: &str| {
            Some(TransferKey {
                id: id.to_owned(),
                secret: secret.to_owned(),
            })
        };
        for (link, expected) in [
            (
                "https://www.transfernow.net/dl/20261111abcDEF12",
                key("20261111abcDEF12", ""),
            ),
            (
                "https://www.transfernow.net/dl/20220721G3M9rK3u/MYmmBkAa",
                key("20220721G3M9rK3u", "MYmmBkAa"),
            ),
            (
                "https://www.transfernow.net/fr/dl/20220721G3M9rK3u",
                key("20220721G3M9rK3u", ""),
            ),
            (
                "https://www.transfernow.net/en/dltransfer?utm_source=20220721G3M9rK3u&utm_medium=MYmmBkAa",
                key("20220721G3M9rK3u", "MYmmBkAa"),
            ),
            ("https://www.transfernow.net/en/pricing", None),
        ] {
            assert_eq!(parse_link(link), expected, "{link}");
        }
        assert_eq!(
            Host::of("https://www.transfernow.net/dl/20261111abcDEF12"),
            Some(Host::TransferNow)
        );
    }

    #[test]
    fn reads_a_transfer() {
        // TransferNow's answer for a transfer of one file on 2026-11-11, without its sender.
        let transfer: Transfer = serde_json::from_str(
            r#"{"transferId": "20261111abcDEF12", "needPassword": false,
            "validity": {"from": "2026-11-11T22:45:49.545Z", "to": "2026-11-18T22:45:46.777Z"},
            "files": [{"id": "yfoTNL", "name": "Sample.7z", "size": 16614003}],
            "status": "ENABLED", "limitedDownloads": false, "size": 16614003}"#,
        )
        .unwrap();
        assert!(transfer.downloadable().is_ok());
        assert_eq!(transfer.files[0].size, 16_614_003);
        let expired: Transfer =
            serde_json::from_str(r#"{"status": "DELETED", "needPassword": false, "files": []}"#)
                .unwrap();
        assert!(matches!(
            expired.downloadable(),
            Err(PreviewError::Unavailable { .. })
        ));
        let locked: Transfer =
            serde_json::from_str(r#"{"status": "ENABLED", "needPassword": true, "files": []}"#)
                .unwrap();
        assert_eq!(
            locked.downloadable(),
            Err(PreviewError::PasswordProtected {
                site: "TransferNow".to_owned()
            })
        );
    }
}
