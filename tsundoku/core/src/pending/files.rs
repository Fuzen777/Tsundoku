//! Links read from a file dropped on the window: a text file, a browser's bookmarks export (the Netscape bookmark
//! format Firefox, Zen and Chrome export) or an Internet shortcut (`.url`). Only http and https links are kept, each
//! once, in the order the file has them. The UI hands over the file's name, then its bytes once `check` has passed it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::links_in;

/// The largest file read: a bookmarks export with a few thousand bookmarks and their icons fits.
pub const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// What a dropped file is, by its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkFileKind {
    /// `.txt`: every link in the text.
    Text,
    /// `.html` or `.htm`: every `<a href>`, as a bookmarks export has them.
    Bookmarks,
    /// `.url`: the `URL=` line under `[InternetShortcut]`.
    Shortcut,
}

impl LinkFileKind {
    pub fn of(name: &str) -> Option<Self> {
        let (_, extension) = name.rsplit_once('.')?;
        match extension.to_ascii_lowercase().as_str() {
            "txt" => Some(Self::Text),
            "html" | "htm" => Some(Self::Bookmarks),
            "url" => Some(Self::Shortcut),
            _ => None,
        }
    }
}

/// Why a dropped file gave no links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum LinkFileError {
    /// Not a `.txt`, `.html`, `.htm` or `.url` file.
    #[error("{name} isn't a file Tsundoku reads links from")]
    UnsupportedType { name: String },
    /// Larger than `MAX_FILE_BYTES`.
    #[error("{name} is larger than {max_bytes} bytes")]
    TooLarge {
        name: String,
        #[serde(rename = "maxBytes")]
        #[ts(type = "number")]
        max_bytes: u64,
    },
    /// Read, but with no http or https link in it.
    #[error("no links found in {name}")]
    NoLinks { name: String },
}

/// Whether a file of this name and size can be read, and as what, before its bytes are read.
pub fn check(name: &str, size: u64) -> Result<LinkFileKind, LinkFileError> {
    let kind = LinkFileKind::of(name).ok_or_else(|| LinkFileError::UnsupportedType {
        name: name.to_owned(),
    })?;
    if size > MAX_FILE_BYTES {
        return Err(LinkFileError::TooLarge {
            name: name.to_owned(),
            max_bytes: MAX_FILE_BYTES,
        });
    }
    Ok(kind)
}

/// The http and https links in a dropped file, each once. A file with none is `NoLinks`.
pub fn links_in_file(name: &str, bytes: &[u8]) -> Result<Vec<String>, LinkFileError> {
    let kind = check(name, bytes.len() as u64)?;
    let text = decode(bytes);
    let links = match kind {
        LinkFileKind::Text => text_links(&text),
        LinkFileKind::Bookmarks => bookmark_links(&text),
        LinkFileKind::Shortcut => shortcut_links(&text),
    };
    if links.is_empty() {
        return Err(LinkFileError::NoLinks {
            name: name.to_owned(),
        });
    }
    Ok(links)
}

/// The file's text: UTF-16 when it starts with that byte order mark (Notepad's "Unicode"), else UTF-8, with any byte
/// that isn't replaced rather than refused.
fn decode(bytes: &[u8]) -> String {
    let utf16 = |bytes: &[u8], from_pair: fn([u8; 2]) -> u16| {
        let (pairs, _) = bytes.as_chunks::<2>();
        char::decode_utf16(pairs.iter().map(|pair| from_pair(*pair)))
            .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect()
    };
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, u16::from_le_bytes),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, u16::from_be_bytes),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// Whether text starts as an http or https link, ignoring case.
fn is_web_link(text: &str) -> bool {
    let start = text.get(..8).unwrap_or(text).to_ascii_lowercase();
    start.starts_with("http://") || start.starts_with("https://")
}

/// The words of a text that are http or https links, as `links_in` takes them from pasted text, but only those
/// written with their scheme: a text file's other words ("notes.txt", "v1.2") aren't links to guess at.
fn text_links(text: &str) -> Vec<String> {
    let words: Vec<&str> = text
        .split_whitespace()
        .filter(|word| {
            let lowercase = word.to_ascii_lowercase();
            lowercase.contains("http://") || lowercase.contains("https://")
        })
        .collect();
    links_in(&words.join("\n"))
        .into_iter()
        .filter(|link| is_web_link(link))
        .collect()
}

/// Every `<a>` tag's `href` that's an http or https link (a bookmarks export also holds `place:` queries and
/// `javascript:` bookmarklets), with its HTML entities decoded.
fn bookmark_links(html: &str) -> Vec<String> {
    let mut links: Vec<String> = Vec::new();
    let lowercase = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lowercase[from..].find("<a") {
        let start = from + found + 2;
        from = start;
        // `<a` must be the whole tag name: not `<abbr>` or `<address>`.
        if !html[start..].starts_with(|character: char| character.is_ascii_whitespace()) {
            continue;
        }
        let (href, end) = href_in_tag(html, start);
        from = end;
        if let Some(href) = href {
            let href = decode_entities(href.trim());
            if is_web_link(&href)
                && let Some(link) = links_in(&href).into_iter().next()
                && !links.contains(&link)
            {
                links.push(link);
            }
        }
    }
    links
}

/// Reads a tag's attributes from `start` (just after its name) to its closing `>`: the `href` attribute's value, if
/// it has one, and where the tag ends.
fn href_in_tag(html: &str, start: usize) -> (Option<&str>, usize) {
    let bytes = html.as_bytes();
    let mut position = start;
    let mut href = None;
    loop {
        while position < bytes.len() && bytes[position].is_ascii_whitespace() {
            position += 1;
        }
        if position >= bytes.len() {
            return (href, position);
        }
        if bytes[position] == b'>' {
            return (href, position + 1);
        }
        let name_start = position;
        while position < bytes.len()
            && !bytes[position].is_ascii_whitespace()
            && !matches!(bytes[position], b'=' | b'>')
        {
            position += 1;
        }
        let name = &html[name_start..position];
        while position < bytes.len() && bytes[position].is_ascii_whitespace() {
            position += 1;
        }
        if position >= bytes.len() || bytes[position] != b'=' {
            // An attribute without a value, or a stray character such as the `/` of `<a/>`.
            if position == name_start {
                position += 1;
            }
            continue;
        }
        position += 1;
        while position < bytes.len() && bytes[position].is_ascii_whitespace() {
            position += 1;
        }
        let value = match bytes.get(position) {
            Some(&quote @ (b'"' | b'\'')) => {
                let value_start = position + 1;
                let value_end = html[value_start..]
                    .find(quote as char)
                    .map_or(html.len(), |offset| value_start + offset);
                position = (value_end + 1).min(html.len());
                &html[value_start..value_end]
            }
            _ => {
                let value_start = position;
                while position < bytes.len()
                    && !bytes[position].is_ascii_whitespace()
                    && bytes[position] != b'>'
                {
                    position += 1;
                }
                &html[value_start..position]
            }
        };
        if href.is_none() && name.eq_ignore_ascii_case("href") {
            href = Some(value);
        }
    }
}

/// Decodes the entities a link in HTML may hold: `&amp;` and the other named ones links use, and numbered ones.
fn decode_entities(text: &str) -> String {
    let mut decoded = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        decoded.push_str(&rest[..at]);
        rest = &rest[at..];
        let entity = rest
            .find(';')
            .filter(|&end| end <= 10)
            .and_then(|end| Some((entity_char(&rest[1..end])?, end)));
        match entity {
            Some((character, end)) => {
                decoded.push(character);
                rest = &rest[end + 1..];
            }
            None => {
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }
    decoded.push_str(rest);
    decoded
}

fn entity_char(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let number = name.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

/// The `URL=` line under `[InternetShortcut]`, as Windows and browsers write a `.url` file, if it's an http or https
/// link.
fn shortcut_links(text: &str) -> Vec<String> {
    let mut in_shortcut = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some(section) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            in_shortcut = section.trim().eq_ignore_ascii_case("InternetShortcut");
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if in_shortcut && key.trim().eq_ignore_ascii_case("URL") {
            let value = value.trim();
            return if is_web_link(value) {
                links_in(value)
            } else {
                Vec::new()
            };
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/link_files/");

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!("{FIXTURES}{name}")).unwrap()
    }

    #[test]
    fn reads_a_firefox_bookmarks_export() {
        let links = links_in_file("bookmarks.html", &fixture("bookmarks-firefox.html")).unwrap();
        assert_eq!(
            links,
            [
                "https://www.youtube.com/watch?v=jNQXAC9IVRw&t=1",
                "https://www.instagram.com/p/DPmL8vYjK3h/",
                "http://example.com/old",
                "https://x.com/NASA/status/1234567890123456789",
            ],
            "place: queries, the bookmarklet, the data: link and the repeated link are left out",
        );
    }

    #[test]
    fn reads_a_chrome_bookmarks_export_with_icons() {
        let links = links_in_file("Bookmarks.HTM", &fixture("bookmarks-chrome.html")).unwrap();
        assert_eq!(
            links,
            [
                "https://www.youtube.com/shorts/aqz-KE-bpKQ",
                "https://mega.nz/file/AbCdEfGh#key-part",
            ]
        );
    }

    #[test]
    fn reads_every_link_in_a_text_file() {
        let links = links_in_file("links.txt", &fixture("links.txt")).unwrap();
        assert_eq!(
            links,
            [
                "https://youtu.be/jNQXAC9IVRw",
                "https://www.youtube.com/watch?v=aqz-KE-bpKQ",
                "http://example.com/page?a=1&b=2",
            ],
            "words without a scheme, such as notes.txt and youtu.be/abc, aren't taken",
        );
    }

    #[test]
    fn reads_a_utf16_text_file() {
        let text = "Vidéo : https://youtu.be/jNQXAC9IVRw\r\n";
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(
            links_in_file("notes.txt", &bytes).unwrap(),
            ["https://youtu.be/jNQXAC9IVRw"]
        );
    }

    #[test]
    fn replaces_bytes_that_arent_utf8() {
        let mut bytes = b"caf\xE9 https://youtu.be/jNQXAC9IVRw".to_vec();
        bytes.push(b'\n');
        assert_eq!(
            links_in_file("notes.txt", &bytes).unwrap(),
            ["https://youtu.be/jNQXAC9IVRw"]
        );
    }

    #[test]
    fn reads_an_internet_shortcut() {
        assert_eq!(
            links_in_file("Me at the zoo.url", &fixture("shortcut.url")).unwrap(),
            ["https://www.youtube.com/watch?v=jNQXAC9IVRw"]
        );
        let elsewhere = b"[DEFAULT]\r\nBASEURL=https://example.com/\r\n[InternetShortcut]\r\nURL=file:///C:/notes.txt\r\n";
        assert_eq!(
            links_in_file("notes.url", elsewhere),
            Err(LinkFileError::NoLinks {
                name: "notes.url".into()
            }),
            "only the URL= line under [InternetShortcut] counts, and only an http or https one",
        );
    }

    #[test]
    fn says_when_a_file_has_no_links() {
        assert_eq!(
            links_in_file(
                "bookmarks.html",
                b"<html><a href=\"javascript:void(0)\">x</a></html>"
            ),
            Err(LinkFileError::NoLinks {
                name: "bookmarks.html".into()
            })
        );
    }

    #[test]
    fn refuses_other_files_and_large_ones() {
        assert_eq!(
            check("video.mp4", 10),
            Err(LinkFileError::UnsupportedType {
                name: "video.mp4".into()
            })
        );
        assert_eq!(
            check("README", 10),
            Err(LinkFileError::UnsupportedType {
                name: "README".into()
            })
        );
        assert_eq!(
            check("bookmarks.html", MAX_FILE_BYTES + 1),
            Err(LinkFileError::TooLarge {
                name: "bookmarks.html".into(),
                max_bytes: MAX_FILE_BYTES,
            })
        );
        assert_eq!(check("a.TXT", MAX_FILE_BYTES), Ok(LinkFileKind::Text));
    }

    #[test]
    fn reads_attributes_in_any_form() {
        let html = "<p><abbr href=\"https://no.example\">x</abbr> <A\nTARGET=_blank HREF=https://example.com/a?x=1&amp;y=2>a</A>\
            <a data-x='>' href='https://example.com/b'>b</a><a href>c</a><a/></p>";
        assert_eq!(
            bookmark_links(html),
            ["https://example.com/a?x=1&y=2", "https://example.com/b"]
        );
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(
            decode_entities("a&amp;b&#38;c&#x26;d&nbsp;e&"),
            "a&b&c&d&nbsp;e&"
        );
    }
}
