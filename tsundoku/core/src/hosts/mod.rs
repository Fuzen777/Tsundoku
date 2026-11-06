//! File hosts Tsundoku reads itself, without a tool: MEGA and MediaFire. A host's reader turns a share link into the
//! files behind it (`Share`), once for the preview card and again when the download starts, since the addresses
//! files download from expire. `downloads::file_runner` then saves them.

pub mod mediafire;
pub mod mega;

use std::path::PathBuf;
use std::time::Duration;

use reqwest::blocking::{Client, Response};

use crate::cancel::CancelToken;
use crate::metadata::{FileContents, PreviewError, VideoInfo};
use crate::rules::safe_folder_name;
use crate::sites;

/// How long connecting may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// How long a request may wait for its answer, or a download for its next bytes.
const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// Sent with every request. MediaFire only shows its download button to a browser.
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:140.0) Gecko/20100101 Firefox/140.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    Mega,
    MediaFire,
}

impl Host {
    pub const ALL: [Self; 2] = [Self::Mega, Self::MediaFire];

    /// The host a link belongs to, by its domain. `link` must come from `metadata::normalize_link`.
    pub fn of(link: &str) -> Option<Self> {
        let host = sites::host(link)?;
        Self::ALL.into_iter().find(|candidate| {
            candidate
                .domains()
                .iter()
                .any(|domain| sites::on_domain(&host, domain))
        })
    }

    /// The name people know it by, shown in Supported Sites and used for its folder.
    pub fn name(self) -> &'static str {
        match self {
            Self::Mega => "MEGA",
            Self::MediaFire => "MediaFire",
        }
    }

    /// The site in the history, in lowercase as the tools' names are.
    pub fn site(self) -> &'static str {
        match self {
            Self::Mega => "mega",
            Self::MediaFire => "mediafire",
        }
    }

    fn domains(self) -> &'static [&'static str] {
        match self {
            Self::Mega => &["mega.nz", "mega.co.nz"],
            Self::MediaFire => &["mediafire.com"],
        }
    }

    /// Lists the files behind `link`. An empty shared folder has nothing to download.
    pub fn read(
        self,
        client: &Client,
        link: &str,
        cancel: &CancelToken,
    ) -> Result<Share, PreviewError> {
        let share = match self {
            Self::Mega => mega::read(client, link, cancel),
            Self::MediaFire => mediafire::read(client, link, cancel),
        }?;
        if share.files.is_empty() {
            return Err(PreviewError::NoVideo {
                detail: format!("the {} folder is empty", self.name()),
            });
        }
        Ok(share)
    }
}

/// What a share link holds: one file, or the files of a shared folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Share {
    pub host: Host,
    /// The host's ID for the file or folder, the history's key with the host's site.
    pub id: String,
    /// The link to download it again, with anything it needs to be read (MEGA's key).
    pub url: String,
    /// The file's name, or the folder's.
    pub title: String,
    /// As YYYY-MM-DD, when the host says.
    pub upload_date: Option<String>,
    pub files: Vec<RemoteFile>,
}

impl Share {
    pub fn total_bytes(&self) -> u64 {
        self.files.iter().map(|file| file.size).sum()
    }

    /// The share as the preview card and the queue know it.
    pub fn video_info(&self) -> VideoInfo {
        VideoInfo {
            site: self.host.site().to_owned(),
            site_name: self.host.domains()[0].to_owned(),
            video_id: self.id.clone(),
            url: self.url.clone(),
            title: self.title.clone(),
            uploader: None,
            duration_seconds: None,
            upload_date: self.upload_date.clone(),
            thumbnail_url: None,
            best_height: None,
            post: None,
            files: Some(FileContents {
                count: u32::try_from(self.files.len()).unwrap_or(u32::MAX),
                bytes: self.total_bytes(),
            }),
        }
    }
}

/// One file of a share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    /// Where it goes inside the download's folder: a shared folder's own folder and subfolders, then the file's
    /// name, each made safe for Windows.
    pub path: PathBuf,
    pub size: u64,
    pub source: FileSource,
}

/// How to fetch a file's download address once its download starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSource {
    Mega(mega::MegaFile),
    MediaFire(mediafire::MediaFireFile),
}

/// Where a file downloads from, and how to check what arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub url: String,
    pub check: FileCheck,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileCheck {
    /// MEGA's files arrive encrypted: they're decrypted with the key as they're saved, then checked with its MAC.
    Mega(mega::FileKey),
    /// The SHA-256 MediaFire gives, in lowercase hex.
    Sha256(String),
    None,
}

impl FileSource {
    /// Asks the host where the file downloads from now. These addresses expire, so it's asked once per run.
    pub fn address(&self, client: &Client, cancel: &CancelToken) -> Result<Address, PreviewError> {
        match self {
            Self::Mega(file) => mega::address(client, file, cancel),
            Self::MediaFire(file) => mediafire::address(client, file, cancel),
        }
    }
}

/// The client every host request goes through.
pub fn client() -> Result<Client, PreviewError> {
    Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(READ_TIMEOUT)
        .build()
        .map_err(|error| PreviewError::Failed {
            detail: error.to_string(),
        })
}

/// Lists the files behind a host's link, for the preview card.
pub fn read(link: &str, cancel: &CancelToken) -> Result<Share, PreviewError> {
    let host = Host::of(link).ok_or(PreviewError::UnsupportedSite)?;
    host.read(&client()?, link, cancel)
}

/// A file or folder name from a host, safe to use on Windows.
pub(crate) fn safe_name(name: &str) -> String {
    safe_folder_name(name, "file")
}

/// Stops between requests once `cancel` is set.
pub(crate) fn check_cancel(cancel: &CancelToken) -> Result<(), PreviewError> {
    if cancel.is_cancelled() {
        Err(PreviewError::Cancelled)
    } else {
        Ok(())
    }
}

pub(crate) fn network_error(error: reqwest::Error) -> PreviewError {
    PreviewError::Network {
        detail: error.to_string(),
    }
}

/// A host's answer that isn't a success, sorted as a preview's errors are.
pub(crate) fn status_error(host: Host, response: &Response) -> Option<PreviewError> {
    let status = response.status();
    if status.is_success() {
        return None;
    }
    let detail = format!("{} answered {status}", host.name());
    Some(match status.as_u16() {
        404 | 410 => PreviewError::Unavailable { detail },
        429 | 509 => PreviewError::LimitReached {
            site: host.name().to_owned(),
            detail,
        },
        500..=599 => PreviewError::Network { detail },
        _ => PreviewError::Failed { detail },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_its_hosts_by_domain() {
        for (link, host) in [
            ("https://mega.nz/file/AbCdEf12#key", Some(Host::Mega)),
            ("https://MEGA.co.nz/#!AbCdEf12!key", Some(Host::Mega)),
            (
                "https://www.mediafire.com/file/abc123/name.zip/file",
                Some(Host::MediaFire),
            ),
            ("https://mediafire.com/?abc123", Some(Host::MediaFire)),
            ("https://notmega.nz/file/AbCdEf12", None),
            ("https://www.youtube.com/watch?v=jNQXAC9IVRw", None),
        ] {
            assert_eq!(Host::of(link), host, "{link}");
        }
    }

    #[test]
    fn a_share_becomes_a_preview_of_its_files() {
        let share = Share {
            host: Host::MediaFire,
            id: "abc123".to_owned(),
            url: "https://www.mediafire.com/file/abc123".to_owned(),
            title: "Pack".to_owned(),
            upload_date: Some("2024-05-01".to_owned()),
            files: [3, 4]
                .map(|size| RemoteFile {
                    path: PathBuf::from(format!("{size}.zip")),
                    size,
                    source: FileSource::MediaFire(mediafire::MediaFireFile {
                        quick_key: size.to_string(),
                        sha256: None,
                    }),
                })
                .to_vec(),
        };
        let video = share.video_info();
        assert_eq!(
            (video.site.as_str(), video.video_id.as_str()),
            ("mediafire", "abc123")
        );
        assert_eq!(video.site_name, "mediafire.com");
        assert_eq!(video.files, Some(FileContents { count: 2, bytes: 7 }));
        assert_eq!(video.post, None);
    }
}
