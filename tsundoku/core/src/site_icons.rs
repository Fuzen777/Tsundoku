//! Sites' icons (their favicons), shown before a site's name on a channel's, playlist's or account's picker and on its
//! folded line. The app crate's `siteicon` protocol serves them, named by the site's host (`api.ts`'s `siteIconUrl`).
//!
//! An icon is read once, from the addresses the site's home page names (`<link rel="icon">`, the one nearest 32 px
//! but not smaller first), then `/favicon.ico`, then the page's `apple-touch-icon`, and kept in the data folder's
//! `site-icons\` (`DataPaths::site_icons_folder`) as `<host>.<ext>`, to be read again once it's `KEEP_FOR` old (the
//! kept one still serves when that fails). A site without one is asked again `ASK_AGAIN_AFTER` later at the earliest.
//! Only https addresses of a host with a name (not an IP address or `localhost`) are asked.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use regex::Regex;
use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::header::ACCEPT;

use crate::atomic_file::write_atomically;
use crate::hosts;

/// How long a kept icon serves before it's read again.
const KEEP_FOR: Duration = Duration::from_secs(30 * 24 * 60 * 60);
/// How long a site whose icon couldn't be read isn't asked again.
const ASK_AGAIN_AFTER: Duration = Duration::from_secs(60 * 60);
/// The most of a home page read for its icons' addresses: they're in its head.
const LARGEST_PAGE: u64 = 1024 * 1024;
/// The largest icon kept.
const LARGEST_ICON: u64 = 512 * 1024;
/// The icon's size the UI wants, in pixels: shown at 16 CSS pixels, on screens scaled up to 200%.
const WANTED_SIZE: u32 = 32;
/// The most addresses tried for one site.
const TRIES_AT_MOST: usize = 5;

/// What an icon's file is, told by its first bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Png,
    Ico,
    Gif,
    Jpeg,
    Webp,
    Svg,
}

impl IconKind {
    const ALL: [Self; 6] = [
        Self::Png,
        Self::Ico,
        Self::Gif,
        Self::Jpeg,
        Self::Webp,
        Self::Svg,
    ];

    /// What a file is, by its first bytes; `None` for anything that isn't a picture, such as a page answered in its
    /// place.
    pub fn of(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if bytes.starts_with(&[0, 0, 1, 0]) {
            Some(Self::Ico)
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            Some(Self::Gif)
        } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            Some(Self::Jpeg)
        } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
            Some(Self::Webp)
        } else {
            let start = &bytes[..bytes.len().min(1024)];
            let text = String::from_utf8_lossy(start);
            let text = text.trim_start_matches('\u{feff}').trim_start();
            ((text.starts_with("<svg") || text.starts_with("<?xml")) && text.contains("<svg"))
                .then_some(Self::Svg)
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Ico => "image/x-icon",
            Self::Gif => "image/gif",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Svg => "image/svg+xml",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Ico => "ico",
            Self::Gif => "gif",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Svg => "svg",
        }
    }
}

/// An icon as the protocol serves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteIcon {
    pub kind: IconKind,
    pub bytes: Vec<u8>,
}

/// The sites' icons kept in a folder, read from the sites when missing or old.
pub struct SiteIcons {
    folder: PathBuf,
    /// The sites whose icon couldn't be read, and when.
    missing: Mutex<HashMap<String, Instant>>,
}

impl SiteIcons {
    pub fn new(folder: PathBuf) -> Self {
        Self {
            folder,
            missing: Mutex::default(),
        }
    }

    /// The icon of the site at `host` ("www.youtube.com"): the one kept, else read from the site and kept. Why not,
    /// otherwise.
    pub fn icon(&self, host: &str) -> Result<SiteIcon, String> {
        let host = checked_host(host).ok_or_else(|| format!("{host} isn't a site's name"))?;
        let kept = self.kept(&host);
        if let Some((icon, true)) = &kept {
            return Ok(icon.clone());
        }
        let asked_lately = self
            .missing()
            .get(&host)
            .is_some_and(|when| when.elapsed() < ASK_AGAIN_AFTER);
        let read = if asked_lately {
            Err(format!("{host} had no icon an hour ago"))
        } else {
            read_icon_of(&host)
        };
        match read {
            Ok(icon) => {
                self.missing().remove(&host);
                self.keep(&host, &icon);
                Ok(icon)
            }
            Err(why) => {
                if !asked_lately {
                    self.missing().insert(host, Instant::now());
                }
                kept.map(|(icon, _)| icon).ok_or(why)
            }
        }
    }

    /// The icon kept for `host`, and whether it's fresh enough to serve without asking the site.
    fn kept(&self, host: &str) -> Option<(SiteIcon, bool)> {
        IconKind::ALL.into_iter().find_map(|kind| {
            let path = self.folder.join(format!("{host}.{}", kind.extension()));
            let bytes = fs::read(&path).ok()?;
            let fresh = fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                .is_some_and(|age| age < KEEP_FOR);
            Some((SiteIcon { kind, bytes }, fresh))
        })
    }

    /// Keeps an icon read, in place of any other kept for the site. Best effort: an icon that can't be written is still
    /// served.
    fn keep(&self, host: &str, icon: &SiteIcon) {
        if fs::create_dir_all(&self.folder).is_err() {
            return;
        }
        for kind in IconKind::ALL {
            if kind != icon.kind {
                let _ = fs::remove_file(self.folder.join(format!("{host}.{}", kind.extension())));
            }
        }
        let path = self
            .folder
            .join(format!("{host}.{}", icon.kind.extension()));
        let _ = write_atomically(&path, &icon.bytes);
    }

    fn missing(&self) -> MutexGuard<'_, HashMap<String, Instant>> {
        self.missing
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// `host` in lowercase when it's a site's name: letters, digits, dots and hyphens, two labels at least, the last with
/// a letter (so no IP address), and not `localhost`.
fn checked_host(host: &str) -> Option<String> {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let labels: Vec<&str> = host.split('.').collect();
    let fine = host.len() <= 253
        && labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
        })
        && labels.last().is_some_and(|last| {
            last.chars()
                .any(|character| character.is_ascii_alphabetic())
        })
        && labels.last() != Some(&"localhost");
    fine.then_some(host)
}

/// Reads the icon of the site at `host`: the addresses its home page names, then `/favicon.ico`.
fn read_icon_of(host: &str) -> Result<SiteIcon, String> {
    let client = hosts::client().map_err(|error| error.to_string())?;
    let home = Url::parse(&format!("https://{host}/")).map_err(|error| error.to_string())?;
    let mut tries = read_page(&client, home.clone())
        .map(|(page, at)| icon_addresses(&page, &at))
        .unwrap_or_default();
    if let Ok(fallback) = home.join("/favicon.ico")
        && !tries.contains(&fallback)
    {
        // Before the touch icons, which are much larger.
        let place = tries.iter().position(is_touch_icon).unwrap_or(tries.len());
        tries.insert(place, fallback);
    }
    let mut why = format!("{host} has no icon");
    for address in tries.into_iter().take(TRIES_AT_MOST) {
        match read_icon(&client, address) {
            Ok(icon) => return Ok(icon),
            Err(error) => why = error,
        }
    }
    Err(why)
}

/// A home page's text, and where it ended up after redirects.
fn read_page(client: &Client, address: Url) -> Result<(String, Url), String> {
    let response = client
        .get(address)
        .header(ACCEPT, "text/html")
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("the page answered {}", response.status()));
    }
    let at = response.url().clone();
    let mut bytes = Vec::new();
    response
        .take(LARGEST_PAGE)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok((String::from_utf8_lossy(&bytes).into_owned(), at))
}

fn read_icon(client: &Client, address: Url) -> Result<SiteIcon, String> {
    let response = client
        .get(address.clone())
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("{address} answered {}", response.status()));
    }
    let mut bytes = Vec::new();
    response
        .take(LARGEST_ICON + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > LARGEST_ICON {
        return Err(format!("{address} is larger than 512 KB"));
    }
    let kind = IconKind::of(&bytes).ok_or_else(|| format!("{address} isn't a picture"))?;
    Ok(SiteIcon { kind, bytes })
}

/// A touch icon, which `icon_addresses` marks with a fragment so it can be put after `/favicon.ico`.
const TOUCH_ICON_MARK: &str = "touch-icon";

fn is_touch_icon(address: &Url) -> bool {
    address.fragment() == Some(TOUCH_ICON_MARK)
}

/// The icons a page names, in the order to try them: its `icon` links, the one nearest `WANTED_SIZE` pixels but not
/// smaller first, then those of unknown size, then the smaller ones, largest first; then its touch icons
/// (`apple-touch-icon`), the smallest first, marked with a fragment (`TOUCH_ICON_MARK`). Only http and https
/// addresses, each once; relative ones are taken from where the page is.
fn icon_addresses(page: &str, at: &Url) -> Vec<Url> {
    static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<link\b[^>]*>").unwrap());
    static ATTRIBUTE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?is)([a-z][a-z0-9:_-]*)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+))"#).unwrap()
    });
    let mut icons: Vec<((u8, i64), Url)> = Vec::new();
    for link in LINK.find_iter(page) {
        let mut rel = String::new();
        let mut href = None;
        let mut size = None;
        for attribute in ATTRIBUTE.captures_iter(link.as_str()) {
            let value = attribute
                .get(2)
                .or_else(|| attribute.get(3))
                .or_else(|| attribute.get(4))
                .map_or("", |value| value.as_str());
            match attribute[1].to_ascii_lowercase().as_str() {
                "rel" => rel = value.to_ascii_lowercase(),
                "href" => href = Some(value.trim().replace("&amp;", "&")),
                "sizes" => size = size_in(value),
                _ => {}
            }
        }
        let tokens: Vec<&str> = rel.split_whitespace().collect();
        let touch = tokens
            .iter()
            .any(|token| token.starts_with("apple-touch-icon"));
        if !touch && !tokens.contains(&"icon") {
            continue;
        }
        let Some(mut address) = href
            .filter(|href| !href.is_empty())
            .and_then(|href| at.join(&href).ok())
            .filter(|address| matches!(address.scheme(), "http" | "https"))
        else {
            continue;
        };
        let rank = if touch {
            address.set_fragment(Some(TOUCH_ICON_MARK));
            (3, i64::from(size.unwrap_or(u32::MAX)))
        } else {
            match size {
                Some(size) if size >= WANTED_SIZE => (0, i64::from(size)),
                None => (1, 0),
                Some(size) => (2, -i64::from(size)),
            }
        };
        if !icons.iter().any(|(_, known)| *known == address) {
            icons.push((rank, address));
        }
    }
    icons.sort_by_key(|(rank, _)| *rank);
    icons.into_iter().map(|(_, address)| address).collect()
}

/// The first size a `sizes` attribute gives ("32x32"), `WANTED_SIZE` for "any" (a drawing, which fits any size).
fn size_in(sizes: &str) -> Option<u32> {
    let first = sizes.split_whitespace().next()?.to_ascii_lowercase();
    if first == "any" {
        return Some(WANTED_SIZE);
    }
    first.split('x').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n rest";

    #[test]
    fn tells_pictures_from_pages_by_their_first_bytes() {
        assert_eq!(IconKind::of(PNG), Some(IconKind::Png));
        assert_eq!(IconKind::of(&[0, 0, 1, 0, 1, 0]), Some(IconKind::Ico));
        assert_eq!(IconKind::of(b"GIF89a..."), Some(IconKind::Gif));
        assert_eq!(
            IconKind::of(&[0xff, 0xd8, 0xff, 0xe0]),
            Some(IconKind::Jpeg)
        );
        assert_eq!(
            IconKind::of(b"RIFF\x10\0\0\0WEBPVP8 "),
            Some(IconKind::Webp)
        );
        assert_eq!(
            IconKind::of(
                b"\xef\xbb\xbf <?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"/>"
            ),
            Some(IconKind::Svg)
        );
        assert_eq!(
            IconKind::of(b"<svg viewBox=\"0 0 1 1\"/>"),
            Some(IconKind::Svg)
        );
        assert_eq!(IconKind::of(b"<!DOCTYPE html><html>"), None);
        assert_eq!(IconKind::of(b"<?xml version=\"1.0\"?><rss>"), None);
        assert_eq!(IconKind::of(b""), None);
    }

    #[test]
    fn asks_only_sites_by_name() {
        assert_eq!(
            checked_host("www.YouTube.com").as_deref(),
            Some("www.youtube.com")
        );
        assert_eq!(checked_host("x.com.").as_deref(), Some("x.com"));
        assert_eq!(
            checked_host("danbooru.donmai.us").as_deref(),
            Some("danbooru.donmai.us")
        );
        for refused in [
            "",
            "localhost",
            "app.localhost",
            "127.0.0.1",
            "192.168.1.10",
            "example",
            "exa mple.com",
            "example.com/favicon.ico",
            "user@example.com",
            "example.com:8080",
            "-bad.com",
            "a..com",
        ] {
            assert_eq!(checked_host(refused), None, "{refused}");
        }
    }

    #[test]
    fn takes_the_icon_nearest_32_pixels_then_the_others() {
        let page = r##"<html><head>
            <link rel="apple-touch-icon" sizes="180x180" href="/touch-180.png">
            <link rel="apple-touch-icon" sizes="120x120" href="/touch-120.png">
            <link rel="mask-icon" href="/pinned.svg" color="#000">
            <link rel="stylesheet" href="/style.css">
            <LINK REL="Shortcut Icon" HREF='//cdn.example.com/favicon.ico?v=1&amp;b=2'>
            <link rel="icon" href="img/icon-16.png" sizes="16x16">
            <link rel=icon href=/icon-96.png sizes=96x96>
            <link sizes="32x32" href="https://static.example.com/icon-32.png" rel="icon" />
            <link rel="icon" href="javascript:alert(1)">
            <link rel="icon" href="/icon-96.png" sizes="96x96">
        </head></html>"##;
        let at = Url::parse("https://www.example.com/home/").unwrap();
        let addresses: Vec<String> = icon_addresses(page, &at)
            .iter()
            .map(Url::to_string)
            .collect();
        assert_eq!(
            addresses,
            [
                "https://static.example.com/icon-32.png",
                "https://www.example.com/icon-96.png",
                "https://cdn.example.com/favicon.ico?v=1&b=2",
                "https://www.example.com/home/img/icon-16.png",
                "https://www.example.com/touch-120.png#touch-icon",
                "https://www.example.com/touch-180.png#touch-icon",
            ]
        );
        assert!(is_touch_icon(&Url::parse(&addresses[4]).unwrap()));
        assert!(!is_touch_icon(&Url::parse(&addresses[0]).unwrap()));
    }

    #[test]
    fn reads_sizes_as_sites_write_them() {
        assert_eq!(size_in("32x32"), Some(32));
        assert_eq!(size_in("48X48 96x96"), Some(48));
        assert_eq!(size_in("any"), Some(WANTED_SIZE));
        assert_eq!(size_in(""), None);
        assert_eq!(size_in("big"), None);
    }

    #[test]
    fn serves_a_kept_icon_without_asking_the_site() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("www.example.com.png"), PNG).unwrap();
        let icons = SiteIcons::new(folder.path().to_path_buf());
        assert_eq!(
            icons.icon("WWW.Example.com").unwrap(),
            SiteIcon {
                kind: IconKind::Png,
                bytes: PNG.to_vec()
            }
        );
        assert!(icons.icon("127.0.0.1").is_err());
    }

    #[test]
    fn keeps_one_icon_per_site() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("example.com.ico"), [0, 0, 1, 0]).unwrap();
        let icons = SiteIcons::new(folder.path().to_path_buf());
        icons.keep(
            "example.com",
            &SiteIcon {
                kind: IconKind::Png,
                bytes: PNG.to_vec(),
            },
        );
        assert!(!folder.path().join("example.com.ico").exists());
        assert_eq!(
            fs::read(folder.path().join("example.com.png")).unwrap(),
            PNG
        );
    }
}
