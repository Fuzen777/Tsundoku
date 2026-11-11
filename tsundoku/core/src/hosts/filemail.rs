//! Filemail: a transfer's link ("filemail.com/d/<id>"), read the way its download page reads it, asked as Tsundoku.
//! `api/transfer/get` gives the transfer (whether it expired, needs a password or was found infected) and its files
//! with their sizes, MD5s and download addresses on one of Filemail's servers; the transfer is read again when a
//! download starts, for a fresh address. Transfers expire after 7 days on the free plan.

use std::path::PathBuf;

use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::header::ACCEPT;
use serde::Deserialize;

use super::{
    Address, FileCheck, FileSource, Host, HostFacts, RemoteFile, Share, check_cancel, date_of,
    network_error, safe_name,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "Filemail",
    site: "filemail",
    domains: &["filemail.com"],
    sends_attachments: true,
    // It answers Tsundoku saying who it is as it answers a browser.
    browser_agent: false,
    read,
    page_error: None,
    window: None,
};

const API: &str = "https://www.filemail.com/api/transfer/get";
/// The most files asked for at once; a transfer with more says so rather than downloading part of it.
const FILES_LIMIT: &str = "1000";
/// The subject Filemail gives a transfer sent without one.
const NO_SUBJECT: &str = "ANONYMOUS";

/// A file to download from Filemail, known by its name and size: its `fileid` changes with every answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilemailFile {
    pub transfer_id: String,
    pub name: String,
    pub size: u64,
}

/// Reads a Filemail link: the transfer's files with their sizes.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let id = parse_link(link).ok_or_else(|| Host::Filemail.link_not_recognized())?;
    let transfer = get(client, &id, cancel)?;
    let several = transfer.files.len() > 1;
    let title = transfer
        .subject
        .as_deref()
        .map(str::trim)
        .filter(|subject| !subject.is_empty() && *subject != NO_SUBJECT)
        .map(str::to_owned)
        .or_else(|| {
            (!several)
                .then(|| transfer.files.first().map(|file| file.filename.clone()))
                .flatten()
        })
        .unwrap_or_else(|| format!("Filemail {id}"));
    let top = PathBuf::from(safe_name(&title));
    let mut files: Vec<RemoteFile> = transfer
        .files
        .iter()
        .map(|file| {
            let name = PathBuf::from(safe_name(&file.filename));
            RemoteFile {
                path: if several { top.join(name) } else { name },
                size: file.filesize,
                source: FileSource::Filemail(FilemailFile {
                    transfer_id: id.clone(),
                    name: file.filename.clone(),
                    size: file.filesize,
                }),
            }
        })
        .collect();
    files.sort_by(|first, second| first.path.cmp(&second.path));
    Ok(Share {
        host: Host::Filemail,
        id,
        url: link.to_owned(),
        title,
        upload_date: transfer.sentdate.and_then(|sent| date_of(sent / 1000)),
        files,
    })
}

/// Reads the transfer again for the file's address now, and its MD5 to check it by.
pub(super) fn address(
    client: &Client,
    file: &FilemailFile,
    cancel: &CancelToken,
) -> Result<Address, PreviewError> {
    let transfer = get(client, &file.transfer_id, cancel)?;
    let found = transfer
        .files
        .into_iter()
        .find(|listed| listed.filename == file.name && listed.filesize == file.size)
        .ok_or_else(|| PreviewError::Unavailable {
            detail: "the file is no longer in the Filemail transfer".to_owned(),
        })?;
    if !found.downloadurl.starts_with("https://") {
        return Err(PreviewError::Failed {
            detail: "Filemail gave no address to download from".to_owned(),
        });
    }
    Ok(Address {
        url: found.downloadurl,
        check: match found.md5.filter(|md5| md5.len() == 32) {
            Some(md5) => FileCheck::Md5(md5.to_ascii_lowercase()),
            None => FileCheck::None,
        },
    })
}

#[derive(Debug, Deserialize)]
struct Answer {
    #[serde(default)]
    transfer: Option<Transfer>,
    #[serde(default)]
    errorcode: Option<i64>,
    #[serde(default)]
    errormessage: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Transfer {
    #[serde(default)]
    subject: Option<String>,
    /// In milliseconds since 1970.
    #[serde(default)]
    sentdate: Option<i64>,
    #[serde(default)]
    isexpired: bool,
    #[serde(default)]
    passwordprotected: bool,
    #[serde(default)]
    blockdownloads: bool,
    #[serde(default)]
    infected: bool,
    #[serde(default)]
    numberoffiles: Option<usize>,
    #[serde(default)]
    files: Vec<TransferFile>,
}

#[derive(Debug, Deserialize)]
struct TransferFile {
    filename: String,
    #[serde(default)]
    filesize: u64,
    #[serde(default)]
    downloadurl: String,
    #[serde(default)]
    md5: Option<String>,
}

/// The transfer, or why it can't be downloaded.
fn get(client: &Client, id: &str, cancel: &CancelToken) -> Result<Transfer, PreviewError> {
    check_cancel(cancel)?;
    let url = Url::parse_with_params(
        API,
        [
            ("filesLimit", FILES_LIMIT),
            ("skipreg", "false"),
            ("transferid", id),
        ],
    )
    .map_err(|error| PreviewError::Failed {
        detail: error.to_string(),
    })?;
    let response = client
        .get(url)
        .header(ACCEPT, "application/json")
        .send()
        .map_err(network_error)?;
    let status = response.status();
    // An unknown transfer is an answer with an error code, sent with status 500.
    let answer: Answer = response.json().map_err(|error| {
        if status.is_server_error() {
            PreviewError::Network {
                detail: format!("Filemail answered {status}"),
            }
        } else {
            PreviewError::Failed {
                detail: format!("Filemail's answer wasn't as expected: {error}"),
            }
        }
    })?;
    answer.transfer().and_then(Transfer::downloadable)
}

impl Answer {
    fn transfer(self) -> Result<Transfer, PreviewError> {
        match (self.transfer, self.errorcode) {
            (Some(transfer), None) => Ok(transfer),
            (_, Some(1002)) => Err(PreviewError::Unavailable {
                detail: "Filemail doesn't know this transfer".to_owned(),
            }),
            (_, code) => Err(PreviewError::Failed {
                detail: format!(
                    "Filemail: {} ({})",
                    self.errormessage.unwrap_or_default(),
                    code.unwrap_or_default()
                ),
            }),
        }
    }
}

impl Transfer {
    fn downloadable(self) -> Result<Self, PreviewError> {
        let unavailable = |why: &str| {
            Err(PreviewError::Unavailable {
                detail: format!("Filemail's transfer {why}"),
            })
        };
        if self.isexpired {
            return unavailable("has expired");
        }
        if self.infected {
            return unavailable("was found to hold a virus");
        }
        if self.blockdownloads {
            return unavailable("can't be downloaded: its sender blocked downloads");
        }
        if self.passwordprotected {
            return Err(PreviewError::PasswordProtected {
                site: FACTS.name.to_owned(),
            });
        }
        if self
            .numberoffiles
            .is_some_and(|count| count > self.files.len())
        {
            return Err(PreviewError::Failed {
                detail: format!(
                    "Filemail listed {} of the transfer's {} files",
                    self.files.len(),
                    self.numberoffiles.unwrap_or_default()
                ),
            });
        }
        Ok(self)
    }
}

/// The transfer's ID in "filemail.com/d/<id>" or "/t/<id>".
fn parse_link(link: &str) -> Option<String> {
    let url = Url::parse(link).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !Host::Filemail.owns(&host) {
        return None;
    }
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    match segments.as_slice() {
        ["d" | "t", id, ..]
            if id.len() >= 8
                && id
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric()) =>
        {
            Some(id.to_ascii_lowercase())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_its_links() {
        assert_eq!(
            parse_link("https://www.filemail.com/d/abcdefghijklmno").as_deref(),
            Some("abcdefghijklmno")
        );
        assert_eq!(
            parse_link("https://filemail.com/t/ABCDEFGHIJ").as_deref(),
            Some("abcdefghij")
        );
        assert_eq!(parse_link("https://www.filemail.com/pricing"), None);
        assert_eq!(
            Host::of("https://www.filemail.com/d/abcdefghijklmno"),
            Some(Host::Filemail)
        );
    }

    #[test]
    fn reads_a_transfer_and_why_it_cant_be_downloaded() {
        // Filemail's answer for a transfer of one file on 2026-11-11, trimmed.
        let answer: Answer = serde_json::from_str(
            r#"{"transfer": {"from": "ANONYMOUS", "subject": "ANONYMOUS", "sentdate": 1794437211061,
            "isexpired": false, "status": "STATUS_COMPLETE", "size": 16614003, "numberoffiles": 1,
            "passwordprotected": false, "blockdownloads": false, "infected": false,
            "files": [{"filename": "Sample.7z", "filesize": 16614003,
              "downloadurl": "https://2111.filemail.com/api/file/get?filekey=abc", "fileid": "abc",
              "md5": "d35773532665d5447ab1aaba67e9be6a"}]}}"#,
        )
        .unwrap();
        let transfer = answer.transfer().and_then(Transfer::downloadable).unwrap();
        assert_eq!(transfer.files[0].filesize, 16_614_003);
        assert_eq!(
            date_of(transfer.sentdate.unwrap() / 1000).as_deref(),
            Some("2026-11-11")
        );

        let unknown: Answer = serde_json::from_str(
            r#"{"responsestatus": "InvalidParameter", "errorcode": 1002, "errormessage": "Wrong transferid (x)"}"#,
        )
        .unwrap();
        assert!(matches!(
            unknown.transfer(),
            Err(PreviewError::Unavailable { .. })
        ));
        for (field, expected) in [("isexpired", "expired"), ("infected", "virus")] {
            let answer: Answer = serde_json::from_str(&format!(
                r#"{{"transfer": {{"{field}": true, "files": []}}}}"#
            ))
            .unwrap();
            assert!(matches!(
                answer.transfer().and_then(Transfer::downloadable),
                Err(PreviewError::Unavailable { detail }) if detail.contains(expected)
            ));
        }
        let partial: Answer = serde_json::from_str(
            r#"{"transfer": {"numberoffiles": 2, "files": [{"filename": "a", "fileid": "a"}]}}"#,
        )
        .unwrap();
        assert!(matches!(
            partial.transfer().and_then(Transfer::downloadable),
            Err(PreviewError::Failed { .. })
        ));
    }
}
