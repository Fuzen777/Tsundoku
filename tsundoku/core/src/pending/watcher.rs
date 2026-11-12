//! The clipboard watcher: while Settings → Adding Links has it on, links copied in any app go to the pending links
//! (`PendingLinks::catch`). Only a copy that's nothing but links counts, so copying a paragraph or a message with a
//! link in it doesn't. Direct links to a file (an image, a PDF) are left out, and so is whatever Tsundoku itself or a
//! password manager copied (`Clipboard::text`).

use std::sync::Weak;
use std::thread;
use std::time::Duration;

use super::PendingLinks;
use crate::hosts::Host;
use crate::metadata;

/// How often the clipboard is looked at. Until something is copied, only its change count is read.
const LOOK_EVERY: Duration = Duration::from_millis(500);

/// Endings of direct links to a file, which gallery-dl would read as a post of that one file. A copied link to an
/// image or a document is rarely meant as a download.
const FILE_ENDINGS: &[&str] = &[
    "jpg", "jpeg", "jpe", "png", "gif", "bmp", "svg", "webp", "avif", "heic", "heif", "jxl", "tif",
    "tiff", "ico", "psd", "mp4", "m4v", "mov", "mkv", "webm", "ogg", "ogv", "oga", "wav", "mp3",
    "m4a", "flac", "opus", "zip", "rar", "7z", "pdf", "swf", "txt",
];

/// The system clipboard, as the watcher reads it (`clipboard::SystemClipboard` on Windows). Tests use a fake.
pub trait Clipboard: Send {
    /// A number that changes whenever something is copied, or `None` when it can't be read.
    fn change_count(&mut self) -> Option<u32>;
    /// What was copied, as text. `None` when it isn't text, and when Tsundoku itself or a program that marks its
    /// copies private (a password manager) copied it.
    fn text(&mut self) -> Option<String>;
}

/// Watches the clipboard on a thread of its own for as long as the pending links exist, catching copied links while
/// the watcher is on.
pub fn watch_clipboard(mut clipboard: impl Clipboard + 'static, links: Weak<PendingLinks>) {
    thread::spawn(move || {
        let mut watcher = Watcher::default();
        while let Some(pending) = links.upgrade() {
            let on = pending.options().watch_clipboard;
            if let Some(copied) = watcher.look(&mut clipboard, on) {
                // A link that can't be saved now isn't worth an error nobody asked for; the next copy tries again.
                let _ = pending.catch(&copied);
            }
            drop(pending);
            thread::sleep(LOOK_EVERY);
        }
    });
}

/// What the watcher remembers between looks.
#[derive(Default)]
struct Watcher {
    /// The change count last seen, or `None` while the watcher is off: whatever was copied before it's turned on is
    /// left alone.
    last_count: Option<u32>,
    /// The text last read, so a copy the clipboard announces twice is caught once.
    last_text: Option<String>,
}

impl Watcher {
    /// The links just copied, if anything was copied since the last look and it's nothing but links.
    fn look(&mut self, clipboard: &mut impl Clipboard, on: bool) -> Option<Vec<String>> {
        if !on {
            self.last_count = None;
            return None;
        }
        let count = clipboard.change_count()?;
        let previous = self.last_count.replace(count);
        if previous.is_none_or(|previous| previous == count) {
            return None;
        }
        let text = clipboard.text()?;
        if self.last_text.as_deref() == Some(text.as_str()) {
            return None;
        }
        let links = copied_links(&text);
        self.last_text = Some(text);
        (!links.is_empty()).then_some(links)
    }
}

/// The links in a copy that's nothing but links (one, or several separated by spaces or lines), each once and
/// without direct links to a file. A copy holding anything else, such as a sentence with a link in it, gives none.
pub fn copied_links(text: &str) -> Vec<String> {
    let mut links = Vec::new();
    for word in text.split_whitespace() {
        let word = word
            .trim_start_matches(['<', '"', '\''])
            .trim_end_matches(['>', '"', '\'']);
        let Some(link) = has_web_scheme(word)
            .then(|| metadata::normalize_link(word))
            .flatten()
        else {
            return Vec::new();
        };
        if !is_file_link(&link) && !links.contains(&link) {
            links.push(link);
        }
    }
    links
}

/// Whether a word starts with "http://" or "https://", in any case: a copied word without it isn't taken for a link.
pub(crate) fn has_web_scheme(word: &str) -> bool {
    ["http://", "https://"].iter().any(|scheme| {
        word.get(..scheme.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(scheme))
    })
}

/// Whether a link points straight at a file, such as `…/photo.jpg`. A file host's link never does: what it shares is
/// meant to be downloaded, whatever its name.
fn is_file_link(link: &str) -> bool {
    if Host::of(link).is_some() {
        return false;
    }
    let address = link.split(['?', '#']).next().unwrap_or(link);
    let path = address
        .split_once("://")
        .map_or(address, |(_, rest)| rest)
        .split_once('/')
        .map_or("", |(_, path)| path);
    let name = path.rsplit('/').next().unwrap_or("");
    name.rsplit_once('.').is_some_and(|(_, ending)| {
        FILE_ENDINGS
            .iter()
            .any(|known| known.eq_ignore_ascii_case(ending))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clipboard the test copies to; `None` text stands for a copy that isn't text or is private.
    #[derive(Default)]
    struct FakeClipboard {
        count: u32,
        text: Option<String>,
    }

    impl FakeClipboard {
        fn copy(&mut self, text: Option<&str>) {
            self.count += 1;
            self.text = text.map(str::to_owned);
        }
    }

    impl Clipboard for FakeClipboard {
        fn change_count(&mut self) -> Option<u32> {
            Some(self.count)
        }

        fn text(&mut self) -> Option<String> {
            self.text.clone()
        }
    }

    const POST: &str = "https://www.instagram.com/p/DfPQchkG1tg/";
    const VIDEO: &str = "https://www.youtube.com/watch?v=jNQXAC9IVRw";

    #[test]
    fn takes_a_copy_of_links_only() {
        assert_eq!(copied_links(POST), [POST]);
        assert_eq!(
            copied_links(&format!("  {POST}\r\n{VIDEO}\n{POST}\n")),
            [POST, VIDEO]
        );
        assert_eq!(copied_links(&format!("<{VIDEO}>")), [VIDEO]);
        assert_eq!(copied_links(&format!("HTTPS://{}", &VIDEO[8..])).len(), 1);
    }

    #[test]
    fn leaves_a_copy_with_anything_else_in_it() {
        assert!(copied_links(&format!("Look at this: {POST}")).is_empty());
        assert!(copied_links(&format!("{POST} {VIDEO} lol")).is_empty());
        assert!(
            copied_links("youtu.be/jNQXAC9IVRw").is_empty(),
            "a word without https:// isn't taken"
        );
        assert!(copied_links("").is_empty());
        assert!(copied_links("C:\\Videos\\clip.mp4").is_empty());
    }

    #[test]
    fn leaves_out_direct_links_to_a_file() {
        assert!(copied_links("https://example.com/photos/cat.JPG").is_empty());
        assert!(copied_links("https://example.com/report.pdf?download=1").is_empty());
        assert!(copied_links("https://cdn.example.com/a/b/clip.mp4#t=3").is_empty());
        assert_eq!(
            copied_links(&format!("https://example.com/cat.png {POST}")),
            [POST],
            "the other links of the copy are still taken"
        );
        assert_eq!(
            copied_links("https://www.reddit.com/r/videos/"),
            ["https://www.reddit.com/r/videos/"]
        );
        assert_eq!(copied_links("https://example.com"), ["https://example.com"]);
        let shared = "https://www.mediafire.com/file/abc123/holiday.zip";
        assert_eq!(
            copied_links(shared),
            [shared],
            "a file host's link is meant to be downloaded"
        );
        let button = "https://download2390.mediafire.com/gf8b0u3kzm4g/abc123/holiday.zip";
        assert_eq!(
            copied_links(button),
            [button],
            "so is the link behind its Download button, which browser add-ons copy"
        );
    }

    #[test]
    fn catches_each_copy_made_while_on() {
        let mut clipboard = FakeClipboard::default();
        let mut watcher = Watcher::default();
        clipboard.copy(Some(POST));
        assert_eq!(
            watcher.look(&mut clipboard, true),
            None,
            "what was copied before the watcher started is left alone"
        );
        assert_eq!(watcher.look(&mut clipboard, true), None, "nothing new");

        clipboard.copy(Some(VIDEO));
        assert_eq!(
            watcher.look(&mut clipboard, true),
            Some(vec![VIDEO.to_owned()])
        );
        assert_eq!(watcher.look(&mut clipboard, true), None);

        clipboard.copy(Some("just some words"));
        assert_eq!(watcher.look(&mut clipboard, true), None);
        clipboard.copy(None);
        assert_eq!(
            watcher.look(&mut clipboard, true),
            None,
            "not text, or copied in Tsundoku"
        );
        clipboard.copy(Some(POST));
        assert_eq!(
            watcher.look(&mut clipboard, true),
            Some(vec![POST.to_owned()])
        );
    }

    #[test]
    fn catches_a_copy_announced_twice_once() {
        let mut clipboard = FakeClipboard::default();
        let mut watcher = Watcher::default();
        watcher.look(&mut clipboard, true);
        clipboard.copy(Some(VIDEO));
        assert!(watcher.look(&mut clipboard, true).is_some());
        clipboard.copy(Some(VIDEO));
        assert_eq!(watcher.look(&mut clipboard, true), None);
    }

    #[test]
    fn leaves_copies_alone_while_off_and_what_was_copied_before_it_turned_on() {
        let mut clipboard = FakeClipboard::default();
        let mut watcher = Watcher::default();
        watcher.look(&mut clipboard, true);
        clipboard.copy(Some(POST));
        assert_eq!(watcher.look(&mut clipboard, false), None);
        clipboard.copy(Some(VIDEO));
        assert_eq!(watcher.look(&mut clipboard, false), None);
        assert_eq!(
            watcher.look(&mut clipboard, true),
            None,
            "turned on with a link already copied"
        );
        clipboard.copy(Some(POST));
        assert_eq!(
            watcher.look(&mut clipboard, true),
            Some(vec![POST.to_owned()])
        );
    }
}
