//! The Internet Archive (archive.org): an item ("archive.org/details/<identifier>" or "/download/<identifier>") as a
//! folder of the files its uploader gave it, or one of its files ("/download/<identifier>/<file>"), read through its
//! public metadata API (`/metadata/<identifier>`), which lists every file with its size and SHA-1.
//!
//! The copies archive.org makes of each file (other formats, OCR text, thumbnails) and its own records of the item
//! (`_meta.xml`, `_files.xml`, the torrent) are left out of an item: they repeat what the item holds. A link to one of
//! them still downloads it. Video and audio items (and collections) are declined, so yt-dlp reads them as it did
//! before this host existed, with the presets.

use std::path::PathBuf;

use reqwest::Url;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::Value;

use super::{
    Address, FileCheck, FileSource, Host, HostFacts, RemoteFile, SendChecked, Share, check_cancel,
    code_error, percent_decode, safe_name,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "Internet Archive",
    // yt-dlp's name for it, so what it downloaded from archive.org before still counts.
    site: "archiveorg",
    domains: &["archive.org"],
    sends_attachments: false,
    // It answers Tsundoku saying who it is as it answers a browser.
    browser_agent: false,
    read,
    page_error: None,
    window: None,
};

const SITE: &str = "https://archive.org";
/// Items yt-dlp reads, with the presets: video, audio and live concerts. Collections are left to it too, as before.
const LEFT_TO_TOOLS: [&str; 4] = ["movies", "audio", "etree", "collection"];
/// Extensions of files yt-dlp downloads with a preset, when a link names one.
const MEDIA_EXTENSIONS: [&str; 18] = [
    "mp4", "m4v", "mkv", "webm", "mov", "avi", "mpg", "mpeg", "ogv", "flv", "mp3", "m4a", "ogg",
    "oga", "opus", "flac", "wav", "aac",
];

/// A file of an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveFile {
    pub identifier: String,
    /// Its name in the item, with any folders ("19033-h/images/i001.jpg").
    pub name: String,
    /// In lowercase hex.
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ArchiveLink {
    Item(String),
    File { identifier: String, name: String },
}

/// Reads an archive.org link: an item's own files, or the one file it names. A video or audio item, a collection, and
/// any other page of archive.org (the Wayback Machine's) are declined (`UnsupportedSite`), for the tools to read.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let parsed = parse_link(link).ok_or(PreviewError::UnsupportedSite)?;
    let identifier = match &parsed {
        ArchiveLink::Item(identifier) | ArchiveLink::File { identifier, .. } => identifier.clone(),
    };
    if let ArchiveLink::File { name, .. } = &parsed
        && is_media(name)
    {
        return Err(PreviewError::UnsupportedSite);
    }
    let item = metadata(client, &identifier, cancel)?;
    let title = item.title().unwrap_or_else(|| identifier.clone());
    let upload_date = item
        .text("publicdate")
        .and_then(|date| date.get(..10).map(str::to_owned));
    let (id, files) = match parsed {
        ArchiveLink::Item(_) => {
            if item
                .text("mediatype")
                .is_some_and(|kind| LEFT_TO_TOOLS.contains(&kind.as_str()))
            {
                return Err(PreviewError::UnsupportedSite);
            }
            (identifier.clone(), item.own_files(&identifier, &title)?)
        }
        ArchiveLink::File { name, .. } => {
            let file = item
                .files
                .iter()
                .find(|file| file.name == name)
                .ok_or_else(|| PreviewError::Unavailable {
                    detail: format!("the Internet Archive item has no file \"{name}\""),
                })?;
            let only = name.rsplit('/').next().unwrap_or(&name);
            (
                format!("{identifier}/{name}"),
                vec![file.remote(&identifier, PathBuf::from(safe_name(only)))],
            )
        }
    };
    Ok(Share {
        host: Host::ArchiveOrg,
        id,
        url: link.to_owned(),
        title,
        upload_date,
        files,
    })
}

/// Where a file downloads from: archive.org's own address for it, which sends the request on to the server holding it.
pub(super) fn address(
    _client: &Client,
    file: &ArchiveFile,
    _cancel: &CancelToken,
) -> Result<Address, PreviewError> {
    let mut url =
        Url::parse(&format!("{SITE}/download/")).map_err(|error| PreviewError::Failed {
            detail: error.to_string(),
        })?;
    url.path_segments_mut()
        .map_err(|()| PreviewError::Failed {
            detail: "archive.org's address can't take a path".to_owned(),
        })?
        .pop_if_empty()
        .push(&file.identifier)
        .extend(file.name.split('/'));
    Ok(Address {
        url: url.to_string(),
        check: file.sha1.clone().map_or(FileCheck::None, FileCheck::Sha1),
    })
}

#[derive(Debug, Default, Deserialize)]
struct Item {
    #[serde(default)]
    metadata: serde_json::Map<String, Value>,
    #[serde(default)]
    files: Vec<ItemFile>,
    #[serde(default)]
    is_dark: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ItemFile {
    name: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    format: String,
    /// As text ("74735").
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    sha1: Option<String>,
    /// "true" for a file only lent, not downloaded (a borrowable book's).
    #[serde(default)]
    private: Option<String>,
}

impl ItemFile {
    /// Uploaded with the item, rather than made by archive.org from it or about it.
    fn is_own(&self) -> bool {
        self.source == "original"
            && !matches!(self.format.as_str(), "Item Tile" | "Metadata")
            && !self.name.starts_with("__ia_")
    }

    fn is_lent(&self) -> bool {
        self.private.as_deref() == Some("true")
    }

    fn remote(&self, identifier: &str, path: PathBuf) -> RemoteFile {
        RemoteFile {
            path,
            size: self
                .size
                .as_deref()
                .and_then(|size| size.trim().parse().ok())
                .unwrap_or_default(),
            source: FileSource::ArchiveOrg(ArchiveFile {
                identifier: identifier.to_owned(),
                name: self.name.clone(),
                sha1: self
                    .sha1
                    .as_deref()
                    .filter(|hash| {
                        hash.len() == 40
                            && hash.chars().all(|character| character.is_ascii_hexdigit())
                    })
                    .map(str::to_ascii_lowercase),
            }),
        }
    }
}

impl Item {
    /// A metadata field as text: archive.org gives most as a string, some as a list of them.
    fn text(&self, field: &str) -> Option<String> {
        match self.metadata.get(field)? {
            Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_owned()),
            Value::Array(values) => values
                .iter()
                .find_map(|value| value.as_str().map(str::to_owned)),
            _ => None,
        }
    }

    fn title(&self) -> Option<String> {
        self.text("title")
    }

    /// The files the uploader gave the item, under a folder named after it, keeping the item's own folders.
    fn own_files(&self, identifier: &str, title: &str) -> Result<Vec<RemoteFile>, PreviewError> {
        let own: Vec<&ItemFile> = self.files.iter().filter(|file| file.is_own()).collect();
        if !own.is_empty() && own.iter().all(|file| file.is_lent()) {
            return Err(PreviewError::Unavailable {
                detail: "the Internet Archive only lends this item to read online".to_owned(),
            });
        }
        let top = PathBuf::from(safe_name(title));
        let mut files: Vec<RemoteFile> = own
            .into_iter()
            .filter(|file| !file.is_lent())
            .map(|file| {
                let inside: PathBuf = file
                    .name
                    .split('/')
                    .filter(|part| !part.is_empty())
                    .map(safe_name)
                    .collect();
                file.remote(identifier, top.join(inside))
            })
            .collect();
        files.sort_by(|first, second| first.path.cmp(&second.path));
        Ok(files)
    }
}

/// The item's metadata. An identifier archive.org doesn't know answers `{}`; a dark item is hidden.
fn metadata(client: &Client, identifier: &str, cancel: &CancelToken) -> Result<Item, PreviewError> {
    check_cancel(cancel)?;
    let response = client
        .get(format!("{SITE}/metadata/{identifier}"))
        .send_checked(FACTS.name)?;
    if let Some(error) = code_error(Host::ArchiveOrg, response.status()) {
        return Err(error);
    }
    let item: Item = response.json().map_err(|error| PreviewError::Failed {
        detail: format!("the Internet Archive's answer wasn't as expected: {error}"),
    })?;
    if item.metadata.is_empty() || item.is_dark == Some(true) {
        return Err(PreviewError::Unavailable {
            detail: "the Internet Archive has no such item, or it was taken down".to_owned(),
        });
    }
    Ok(item)
}

/// Whether a file's name says it's video or audio, which yt-dlp downloads with a preset.
fn is_media(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        MEDIA_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
    })
}

/// Reads an archive.org link: "archive.org/details/<identifier>[/…]", "archive.org/download/<identifier>", and
/// "archive.org/download/<identifier>/<file>" (or a server's ".../items/<identifier>/<file>"). Anything else on
/// archive.org (the Wayback Machine, its blog) is `None`.
fn parse_link(link: &str) -> Option<ArchiveLink> {
    let url = Url::parse(link).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let segments: Vec<String> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .map(percent_decode)
        .collect();
    let is_identifier = |text: &str| {
        !text.is_empty()
            && text.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
            })
    };
    let file_of = |identifier: &str, rest: &[String]| {
        if rest.is_empty() {
            ArchiveLink::Item(identifier.to_owned())
        } else {
            ArchiveLink::File {
                identifier: identifier.to_owned(),
                name: rest.join("/"),
            }
        }
    };
    let on_site = host == "archive.org" || host == "www.archive.org";
    let is_server =
        host.ends_with(".archive.org") && (host.starts_with("ia") || host.starts_with("dn"));
    match segments.as_slice() {
        [first, identifier, ..] if on_site && first == "details" && is_identifier(identifier) => {
            Some(ArchiveLink::Item(identifier.clone()))
        }
        [first, identifier, rest @ ..]
            if on_site && first == "download" && is_identifier(identifier) =>
        {
            Some(file_of(identifier, rest))
        }
        [_, items, identifier, rest @ ..]
            if is_server && items == "items" && is_identifier(identifier) && !rest.is_empty() =>
        {
            Some(file_of(identifier, rest))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(identifier: &str, name: &str) -> Option<ArchiveLink> {
        Some(ArchiveLink::File {
            identifier: identifier.to_owned(),
            name: name.to_owned(),
        })
    }

    #[test]
    fn reads_its_links() {
        let item = |identifier: &str| Some(ArchiveLink::Item(identifier.to_owned()));
        for (link, expected) in [
            (
                "https://archive.org/details/alicesadventures19033gut",
                item("alicesadventures19033gut"),
            ),
            (
                "https://www.archive.org/details/alicesadventures19033gut/page/n5/mode/2up",
                item("alicesadventures19033gut"),
            ),
            (
                "https://archive.org/download/alicesadventures19033gut",
                item("alicesadventures19033gut"),
            ),
            (
                "https://archive.org/download/alicesadventures19033gut/19033-h/images/cover.jpg",
                file("alicesadventures19033gut", "19033-h/images/cover.jpg"),
            ),
            (
                "https://archive.org/download/some_item/A%20Book%20%282%29.pdf",
                file("some_item", "A Book (2).pdf"),
            ),
            (
                "https://ia801604.us.archive.org/6/items/alicesadventures19033gut/19033.txt",
                file("alicesadventures19033gut", "19033.txt"),
            ),
            (
                "https://web.archive.org/web/2020/https://example.com/",
                None,
            ),
            ("https://archive.org/search?query=alice", None),
            ("https://archive.org/details/", None),
            ("https://blog.archive.org/2024/01/01/news/", None),
        ] {
            assert_eq!(parse_link(link), expected, "{link}");
        }
        assert_eq!(
            Host::of("https://archive.org/details/x"),
            Some(Host::ArchiveOrg)
        );
        assert!(is_media("Night of the Living Dead.mp4") && is_media("track.FLAC"));
        assert!(!is_media("19033.txt") && !is_media("README"));
    }

    /// archive.org's answer for a Project Gutenberg book, trimmed, with a file only lent added.
    fn gutenberg() -> Item {
        serde_json::from_str(
            r#"{"metadata": {"identifier": "alicesadventures19033gut", "title": "Alice's Adventures in Wonderland",
            "mediatype": "texts", "publicdate": "2006-12-07", "collection": ["gutenberg"]},
            "files": [
              {"name": "19033-8.txt", "source": "original", "format": "Text", "size": "74735",
               "sha1": "8b14b6b0b0b5b5ad780763561fb2d94c46b4e836"},
              {"name": "19033-h/images/cover.jpg", "source": "original", "format": "JPEG", "size": "102933"},
              {"name": "19033-8_djvu.txt", "source": "derivative", "format": "DjVuTXT", "size": "70000"},
              {"name": "__ia_thumb.jpg", "source": "original", "format": "Item Tile", "size": "9000"},
              {"name": "alicesadventures19033gut_meta.xml", "source": "metadata", "format": "Metadata"},
              {"name": "alicesadventures19033gut_archive.torrent", "source": "metadata", "format": "Archive BitTorrent"},
              {"name": "lent.pdf", "source": "original", "format": "Text PDF", "private": "true"}
            ]}"#,
        )
        .unwrap()
    }

    #[test]
    fn an_item_is_the_files_its_uploader_gave_it() {
        let item = gutenberg();
        assert_eq!(
            item.title().as_deref(),
            Some("Alice's Adventures in Wonderland")
        );
        assert_eq!(item.text("collection").as_deref(), Some("gutenberg"));
        let files = item
            .own_files(
                "alicesadventures19033gut",
                "Alice's Adventures in Wonderland",
            )
            .unwrap();
        let paths: Vec<PathBuf> = files.iter().map(|file| file.path.clone()).collect();
        let top = PathBuf::from("Alice's Adventures in Wonderland");
        assert_eq!(
            paths,
            [
                top.join("19033-8.txt"),
                top.join("19033-h").join("images").join("cover.jpg"),
            ],
            "no derived copies, thumbnail, records or lent files"
        );
        assert_eq!(files[0].size, 74735);
        let FileSource::ArchiveOrg(text) = &files[0].source else {
            panic!("an archive.org file");
        };
        let address = address(&Client::new(), text, &CancelToken::new()).unwrap();
        assert_eq!(
            address.url,
            "https://archive.org/download/alicesadventures19033gut/19033-8.txt"
        );
        assert_eq!(
            address.check,
            FileCheck::Sha1("8b14b6b0b0b5b5ad780763561fb2d94c46b4e836".to_owned())
        );
        let FileSource::ArchiveOrg(cover) = &files[1].source else {
            panic!("an archive.org file");
        };
        let named = ArchiveFile {
            name: "A Book (2).pdf".to_owned(),
            ..cover.clone()
        };
        assert_eq!(
            super::address(&Client::new(), &named, &CancelToken::new())
                .unwrap()
                .url,
            "https://archive.org/download/alicesadventures19033gut/A%20Book%20(2).pdf"
        );
    }

    #[test]
    fn a_book_only_lent_says_so() {
        let lent: Item = serde_json::from_str(
            r#"{"metadata": {"title": "A Lent Book", "mediatype": "texts"},
            "files": [{"name": "book.pdf", "source": "original", "format": "Text PDF", "private": "true"},
                      {"name": "book_meta.xml", "source": "metadata", "format": "Metadata"}]}"#,
        )
        .unwrap();
        assert!(matches!(
            lent.own_files("lent", "A Lent Book"),
            Err(PreviewError::Unavailable { detail }) if detail.contains("lends")
        ));
    }
}
