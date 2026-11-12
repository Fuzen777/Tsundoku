//! Sites read only through the check window: their links' own pages are behind a check Tsundoku's client never gets
//! past (Cloudflare's, a proof of work their page runs), so Tsundoku reads nothing of them. A link from one of them
//! reads as one file named after the link's ID, its size unknown, and its download waits for the person to open the
//! link's page in the check window and start the download there (`windowed`, `check`).
//!
//! An entry is the site's name, its domains and the shape of its file links: nothing about its pages, so a change to
//! their layout changes nothing here. An entry is added only once a real download went through the window with a real
//! link. A link from a site that is neither a host Tsundoku reads nor an entry here isn't offered the window.

use reqwest::StatusCode;
use reqwest::Url;
use reqwest::blocking::Client;

use super::check::needs_person;
use super::windowed::WindowLink;
use super::{Host, HostFacts, SendChecked, Share, check_cancel};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

/// The shape of a site's file links.
pub(crate) struct LinkShape {
    /// The paths of its file links, "{id}" standing for the file's ID: "file/{id}".
    pub paths: &'static [&'static str],
    /// The file's page, which the window opens: "https://workupload.com/file/{id}".
    pub page: &'static str,
    /// The fewest characters a file's ID has, all letters or digits.
    pub shortest_id: usize,
}

/// The sites, in Supported Sites' order.
pub(super) static LIST: [HostFacts; 2] = [
    // Its page first runs a proof of work in the browser ("Are you a human?"). Tried 2026-11-11 with a translation
    // file: the window saved it, since its file server answers only the window's own session.
    entry(
        "Workupload",
        "workupload",
        &["workupload.com"],
        &LinkShape {
            paths: &["file/{id}"],
            page: "https://workupload.com/file/{id}",
            shortest_id: 6,
        },
    ),
    // Its page answers Tsundoku's client with Cloudflare's check. Tried 2026-11-12 with an archive: its file server
    // answered Tsundoku with the file.
    entry(
        "Buzzheavier",
        "buzzheavier",
        &["buzzheavier.com"],
        &LinkShape {
            paths: &["{id}", "f/{id}"],
            page: "https://buzzheavier.com/{id}",
            shortest_id: 8,
        },
    ),
];

const fn entry(
    name: &'static str,
    site: &'static str,
    domains: &'static [&'static str],
    shape: &'static LinkShape,
) -> HostFacts {
    HostFacts {
        name,
        site,
        domains,
        sends_attachments: false,
        browser_agent: false,
        read,
        page_error: None,
        window_link: Some(shape),
        download_file_id: None,
        unlock: None,
    }
}

/// A site of the list, by its place in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSite(pub(super) usize);

/// Opens the link's page, as Tsundoku: a site that doesn't know the file says so (404); anything else, its check or
/// a page past it, is left to the check window, which the link reads as (`Host::read`).
fn read(client: &Client, link: &str, cancel: &CancelToken) -> Result<Share, PreviewError> {
    let host = Host::of(link).ok_or(PreviewError::UnsupportedSite)?;
    let found = host
        .window_link(link)
        .ok_or_else(|| host.link_not_recognized())?;
    check_cancel(cancel)?;
    let response = client.get(&found.page).send_checked(host.name())?;
    if response.status() == StatusCode::NOT_FOUND {
        return Err(PreviewError::Unavailable {
            detail: format!("{} doesn't know this file", host.name()),
        });
    }
    Err(needs_person(host.name()))
}

/// The file a link names, if its path has the shape's form.
pub(super) fn link_of(shape: &LinkShape, link: &str) -> Option<WindowLink> {
    let url = Url::parse(link).ok()?;
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    let id = shape.paths.iter().find_map(|path| {
        let pattern: Vec<&str> = path.split('/').collect();
        if pattern.len() != segments.len() {
            return None;
        }
        let mut id = None;
        for (expected, segment) in pattern.iter().zip(&segments) {
            if *expected == "{id}" {
                id = Some(*segment);
            } else if !expected.eq_ignore_ascii_case(segment) {
                return None;
            }
        }
        id
    })?;
    let is_id = id.len() >= shape.shortest_id
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric());
    is_id.then(|| WindowLink {
        id: id.to_owned(),
        page: shape.page.replace("{id}", id),
        name: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_its_sites_links() {
        for (link, page) in [
            (
                "https://workupload.com/file/Xk3mP9qR2tW",
                Some("https://workupload.com/file/Xk3mP9qR2tW"),
            ),
            ("https://workupload.com/start/Xk3mP9qR2tW", None),
            ("https://workupload.com/file/abc", None),
            ("https://workupload.com/", None),
            (
                "https://buzzheavier.com/q8w2e4r6t1y3",
                Some("https://buzzheavier.com/q8w2e4r6t1y3"),
            ),
            (
                "https://buzzheavier.com/f/q8w2e4r6t1y3",
                Some("https://buzzheavier.com/q8w2e4r6t1y3"),
            ),
            ("https://buzzheavier.com/help", None),
            ("https://buzzheavier.com/", None),
        ] {
            let host = Host::of(link).expect("a site of the list");
            assert_eq!(
                host.window_link(link).map(|found| found.page).as_deref(),
                page,
                "{link}"
            );
        }
        assert_eq!(
            Host::of("https://buzzheavier.com/q8w2e4r6t1y3").map(Host::name),
            Some("Buzzheavier")
        );
        assert_eq!(
            Host::of("https://workupload.com/file/Xk3mP9qR2tW").map(Host::site),
            Some("workupload"),
            "its history site, as before"
        );
        assert_eq!(Host::of("https://notbuzzheavier.com/q8w2e4r6t1y3"), None);
    }

    #[test]
    fn a_link_read_through_the_window_is_one_file_named_after_it() {
        let host = Host::of("https://workupload.com/file/Xk3mP9qR2tW").unwrap();
        let share = super::super::windowed::share(host, "https://workupload.com/file/Xk3mP9qR2tW");
        assert_eq!(share.title, "Xk3mP9qR2tW");
        assert_eq!((share.files.len(), share.files[0].size), (1, 0));
        assert_eq!(share.video_info().site, "workupload");
    }
}
