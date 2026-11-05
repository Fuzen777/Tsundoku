//! GitHub releases, where all three tools publish their official builds.

use std::fmt::Write as _;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use reqwest::StatusCode;
use reqwest::blocking::{Client, Response};
use reqwest::header::ACCEPT;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::ToolError;
use crate::cancel::CancelToken;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// The longest wait for an answer or for the next part of a download.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// The shortest time between two progress reports during a download.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub published_at: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ReleaseAsset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
}

impl Release {
    pub fn asset(&self, name: &str) -> Result<&ReleaseAsset, ToolError> {
        self.assets
            .iter()
            .find(|asset| asset.name == name)
            .ok_or_else(|| ToolError::MissingFromRelease {
                file: name.to_owned(),
            })
    }

    /// What the UI calls the release: its tag without a leading "v", or its date for FFmpeg-Builds, whose tag is
    /// always "latest".
    pub fn label(&self) -> String {
        if self.tag_name == "latest"
            && let Some(date) = self
                .published_at
                .as_deref()
                .and_then(|published| published.get(..10))
        {
            return date.to_owned();
        }
        self.tag_name
            .strip_prefix('v')
            .unwrap_or(&self.tag_name)
            .to_owned()
    }
}

pub(crate) struct GitHub {
    client: Client,
}

impl GitHub {
    pub fn new(app_version: &str) -> Result<Self, ToolError> {
        let client = Client::builder()
            .user_agent(format!(
                "Tsundoku/{app_version} (+https://github.com/Fuzen777/Tsundoku)"
            ))
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(READ_TIMEOUT)
            .build()
            .map_err(network_error)?;
        Ok(Self { client })
    }

    pub fn latest_release(&self, repository: &str) -> Result<Release, ToolError> {
        let url = format!("https://api.github.com/repos/{repository}/releases/latest");
        let request = self
            .client
            .get(&url)
            .header(ACCEPT, "application/vnd.github+json");
        let response = checked(request.send().map_err(network_error)?)?;
        response.json().map_err(network_error)
    }

    pub fn text(&self, url: &str) -> Result<String, ToolError> {
        let response = checked(self.client.get(url).send().map_err(network_error)?)?;
        response.text().map_err(network_error)
    }

    /// Downloads `url` to `destination`, reporting the bytes received and the total when known, and returns the
    /// SHA-256 of what was received, in lowercase hex.
    pub fn download(
        &self,
        url: &str,
        destination: &Path,
        cancel: &CancelToken,
        mut on_progress: impl FnMut(u64, Option<u64>),
    ) -> Result<String, ToolError> {
        let mut response = checked(self.client.get(url).send().map_err(network_error)?)?;
        let total = response.content_length();
        let mut file = File::create(destination)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0; 64 * 1024];
        let mut received = 0;
        on_progress(received, total);
        let mut last_report = Instant::now();

        loop {
            if cancel.is_cancelled() {
                return Err(ToolError::Cancelled);
            }
            let count = response
                .read(&mut buffer)
                .map_err(|error| ToolError::Network {
                    detail: error.to_string(),
                })?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
            file.write_all(&buffer[..count])?;
            received += count as u64;
            if last_report.elapsed() >= PROGRESS_INTERVAL {
                on_progress(received, total);
                last_report = Instant::now();
            }
        }

        file.sync_all()?;
        on_progress(received, total);
        Ok(to_hex(&hasher.finalize()))
    }
}

/// Passes a successful answer through and turns any other into an error. GitHub allows 60 requests an hour from a
/// network without signing in, and answers 403 or 429 past that.
fn checked(response: Response) -> Result<Response, ToolError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let out_of_requests = response
        .headers()
        .get("x-ratelimit-remaining")
        .is_some_and(|remaining| remaining == "0");
    if status == StatusCode::TOO_MANY_REQUESTS
        || (status == StatusCode::FORBIDDEN && out_of_requests)
    {
        return Err(ToolError::RateLimited);
    }
    Err(ToolError::Network {
        detail: format!("GitHub answered {status} for {}", response.url()),
    })
}

/// The error with its causes, which carry the useful part ("error sending request" alone says little).
fn network_error(error: reqwest::Error) -> ToolError {
    let mut detail = error.to_string();
    let mut source = std::error::Error::source(&error);
    while let Some(cause) = source {
        let _ = write!(detail, ": {cause}");
        source = cause.source();
    }
    ToolError::Network { detail }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag_name: &str) -> Release {
        Release {
            tag_name: tag_name.to_owned(),
            published_at: Some("2026-11-04T20:58:27Z".to_owned()),
            assets: Vec::new(),
        }
    }

    #[test]
    fn labels_releases_by_tag_or_date() {
        assert_eq!(release("v2.9.8").label(), "2.9.8");
        assert_eq!(release("2026.10.26").label(), "2026.10.26");
        assert_eq!(release("latest").label(), "2026-11-04");
    }

    #[test]
    fn hashes_in_lowercase_hex() {
        assert_eq!(to_hex(&[0x00, 0xab, 0x0f]), "00ab0f");
    }
}
