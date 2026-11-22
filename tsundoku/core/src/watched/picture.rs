//! Pictures picked by hand for watched sources (Edit → Picture → Custom): one of the source's own pictures (its
//! listing's, an item's, or a file of a post's), saved in the data folder's `pictures\` since sites' links expire
//! (Instagram signs its image links), with the area of it to show (`PictureCrop`), which the UI applies, so it can be
//! cropped again later. A source without one shows its listing's picture, which each check keeps current.

use std::fs;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use super::WatchedError;

/// The largest picture read from a site.
const DOWNLOAD_LIMIT: u64 = 15 << 20;
/// The smallest share of the picture's width or height the area may be: a 1000 px picture shows at least 10 px of it.
const SMALLEST_SHARE: f64 = 0.01;

/// A watched source's picture, picked by hand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SourcePicture {
    /// Its name in the data folder's `pictures\` (`file_name`), which the window reads through the `picture` protocol.
    pub file: String,
    pub crop: PictureCrop,
}

/// The area of a picture shown, as shares of its width and height: its left and top edges, its width and height. Its
/// shape is the frame's (a circle's square, a playlist's 16:9), which the UI keeps when it works it out.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PictureCrop {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl PictureCrop {
    /// The area kept inside the picture: a width or height between 1% and all of it, then its edges moved in where it
    /// reaches past the picture. A value that isn't a number takes the whole picture's.
    pub fn checked(self) -> Self {
        let share = |value: f64| {
            if value.is_finite() {
                value.clamp(SMALLEST_SHARE, 1.0)
            } else {
                1.0
            }
        };
        let width = share(self.width);
        let height = share(self.height);
        let edge = |value: f64, size: f64| {
            if value.is_finite() {
                value.clamp(0.0, 1.0 - size)
            } else {
                0.0
            }
        };
        Self {
            x: edge(self.x, width),
            y: edge(self.y, height),
            width,
            height,
        }
    }
}

/// The kinds of pictures kept, told by their first bytes rather than what the site says they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureKind {
    Jpeg,
    Png,
    Webp,
    Gif,
}

impl PictureKind {
    /// What `bytes` is, or `None` when it isn't one of the pictures kept (a page about an error, say).
    pub fn of(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
            Some(Self::Webp)
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            Some(Self::Gif)
        } else {
            None
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Gif => "gif",
        }
    }

    fn from_extension(extension: &str) -> Option<Self> {
        match extension {
            "jpg" => Some(Self::Jpeg),
            "png" => Some(Self::Png),
            "webp" => Some(Self::Webp),
            "gif" => Some(Self::Gif),
            _ => None,
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
            Self::Gif => "image/gif",
        }
    }
}

/// A picked picture's file name: the source's ID, then a hash of the picture, so picking another gives another name
/// and the window never shows the one before from its cache.
pub fn file_name(source: i64, bytes: &[u8], kind: PictureKind) -> String {
    let hash = Sha256::digest(bytes);
    let hex: String = hash[..8].iter().map(|byte| format!("{byte:02x}")).collect();
    format!("watched-{source}-{hex}.{}", kind.extension())
}

/// The kind of picture `name` holds, when it's a name `file_name` gives: the only names the window may ask the
/// pictures folder for.
pub fn kind_of_file(name: &str) -> Option<PictureKind> {
    let rest = name.strip_prefix("watched-")?;
    let (source, rest) = rest.split_once('-')?;
    let (hash, extension) = rest.split_once('.')?;
    let is_source =
        !source.is_empty() && source.chars().all(|character| character.is_ascii_digit());
    let is_hash = hash.len() == 16
        && hash
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character));
    if is_source && is_hash {
        PictureKind::from_extension(extension)
    } else {
        None
    }
}

/// Reads a picture from its site, as the history's thumbnails are read.
pub fn download(url: &str) -> Result<Vec<u8>, WatchedError> {
    let unavailable = |detail: String| WatchedError::PictureUnavailable { detail };
    let client = crate::hosts::client().map_err(|error| unavailable(error.to_string()))?;
    let response = client
        .get(url)
        .send()
        .map_err(|error| unavailable(error.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(unavailable(format!("the site answered {status}")));
    }
    let mut bytes = Vec::new();
    response
        .take(DOWNLOAD_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| unavailable(error.to_string()))?;
    if bytes.len() as u64 > DOWNLOAD_LIMIT {
        return Err(unavailable("it's larger than 15 MB".to_owned()));
    }
    Ok(bytes)
}

/// Writes a picture into `folder`, beside its place first so a half-written one never shows. Its name.
pub(super) fn save(folder: &Path, source: i64, bytes: &[u8]) -> Result<String, WatchedError> {
    let kind = PictureKind::of(bytes).ok_or_else(|| WatchedError::PictureUnavailable {
        detail: "the site's answer isn't a picture".to_owned(),
    })?;
    let name = file_name(source, bytes, kind);
    let storage = |error: std::io::Error| WatchedError::Storage {
        detail: error.to_string(),
    };
    fs::create_dir_all(folder).map_err(storage)?;
    let partial = folder.join(format!("{name}.part"));
    fs::write(&partial, bytes).map_err(storage)?;
    fs::rename(&partial, folder.join(&name)).map_err(|error| {
        let _ = fs::remove_file(&partial);
        storage(error)
    })?;
    Ok(name)
}

/// Deletes a picture that's no longer shown, if it's there. A name `file_name` doesn't give is left alone.
pub(super) fn delete(folder: &Path, name: &str) {
    if kind_of_file(name).is_some() {
        let _ = fs::remove_file(folder.join(name));
    }
}
