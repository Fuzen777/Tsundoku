//! Workupload: a file's link ("workupload.com/file/<id>"), whose page first runs a proof of work in the browser
//! ("Are you a human?", solved by its own script) before it shows the file. Tsundoku's client never computes it, so
//! it reads nothing of the page: the link is read through the check window (`windowed`), where the page's own script
//! runs as in any browser, and the person clicks Download; the window catches the file's address.

use reqwest::StatusCode;
use reqwest::Url;
use reqwest::blocking::Client;

use super::windowed::{WindowDownload, WindowFacts, WindowLink};
use super::{Host, HostFacts, Page, Share, check_cancel, network_error};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "Workupload",
    site: "workupload",
    domains: &["workupload.com"],
    sends_attachments: true,
    // It answers Tsundoku saying who it is as it answers a browser.
    browser_agent: false,
    read,
    page_error: None,
    window: Some(WindowFacts {
        link: window_link,
        takes,
        // Its download server sends the file only to the window's own session.
        download: WindowDownload::SaveInWindow,
    }),
};

const SITE: &str = "https://workupload.com";

/// Opens the link's page, which only a browser running its proof of work gets past: the check window reads it.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let found = window_link(link).ok_or_else(|| Host::Workupload.link_not_recognized())?;
    check_cancel(cancel)?;
    let page = Page::read(client.get(&found.page).send().map_err(network_error)?);
    if page.status == StatusCode::NOT_FOUND {
        return Err(PreviewError::Unavailable {
            detail: "Workupload doesn't know this file".to_owned(),
        });
    }
    // Its check, or a page past it, which a client without the check's cookie hasn't been seen to get: either way, the
    // window reads it.
    Err(PreviewError::BrowserCheck {
        site: FACTS.name.to_owned(),
    })
}

/// The file's ID in "workupload.com/file/<id>".
fn window_link(link: &str) -> Option<WindowLink> {
    let url = Url::parse(link).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !Host::Workupload.owns(&host) {
        return None;
    }
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    let ["file", id, ..] = segments.as_slice() else {
        return None;
    };
    let is_id = id.len() >= 6
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric());
    is_id.then(|| WindowLink {
        id: (*id).to_owned(),
        page: format!("{SITE}/file/{id}"),
        name: None,
    })
}

/// Whether `download` is the file's own: its Download button leads, through "/start/<id>", to
/// "f<n>.workupload.com/download/<id>".
fn takes(download: &str, id: &str) -> bool {
    let Ok(url) = Url::parse(download) else {
        return false;
    };
    let on_file_server = url.host_str().is_some_and(|host| {
        let host = host.to_ascii_lowercase();
        host.strip_suffix(".workupload.com").is_some_and(|server| {
            server.len() > 1
                && server.starts_with('f')
                && server[1..]
                    .chars()
                    .all(|character| character.is_ascii_digit())
        })
    });
    let segments: Vec<&str> = url.path_segments().map_or_else(Vec::new, Iterator::collect);
    url.scheme() == "https" && on_file_server && segments.as_slice() == ["download", id]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_its_links_and_takes_only_its_files_download() {
        assert_eq!(
            window_link("https://workupload.com/file/uDjfcJwACXx").map(|found| found.page),
            Some("https://workupload.com/file/uDjfcJwACXx".to_owned())
        );
        assert_eq!(
            window_link("https://workupload.com/start/uDjfcJwACXx"),
            None
        );
        assert_eq!(window_link("https://workupload.com/"), None);
        assert_eq!(
            Host::of("https://workupload.com/file/uDjfcJwACXx"),
            Some(Host::Workupload)
        );
        // Where its Download button led on 2026-11-11: "/start/<id>", then the file's server.
        assert!(takes(
            "https://f66.workupload.com/download/uDjfcJwACXx",
            "uDjfcJwACXx"
        ));
        assert!(!takes(
            "https://workupload.com/start/uDjfcJwACXx",
            "uDjfcJwACXx"
        ));
        assert!(!takes(
            "https://f66.workupload.com/download/otherFile12",
            "uDjfcJwACXx"
        ));
        assert!(!takes(
            "https://f66.example.com/download/uDjfcJwACXx",
            "uDjfcJwACXx"
        ));
    }
}
