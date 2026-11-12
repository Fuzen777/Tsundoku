//! Buzzheavier: a file's link ("buzzheavier.com/<id>"), whose page answers Tsundoku's client with Cloudflare's check
//! for a browser, so Tsundoku reads nothing of it. It's read through the check window (`windowed`): the person opens
//! the page, passes what Cloudflare asks if anything, and clicks Download, whose first click goes to an advert. The
//! page then goes to the file's address ("ts.buzzheavier.com/d/<id>?v=<token>"), which answers Tsundoku's own client
//! with the file, so the window catches it and Tsundoku downloads it (`WindowDownload::CatchAddress`).

use reqwest::StatusCode;
use reqwest::Url;
use reqwest::blocking::Client;

use super::windowed::{WindowDownload, WindowFacts, WindowLink};
use super::{Host, HostFacts, Page, Share, check_cancel, network_error};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "Buzzheavier",
    site: "buzzheavier",
    domains: &["buzzheavier.com"],
    sends_attachments: true,
    browser_agent: false,
    read,
    page_error: None,
    window: Some(WindowFacts {
        link: window_link,
        takes,
        download: WindowDownload::CatchAddress,
        first_click_ad: true,
    }),
};

const SITE: &str = "https://buzzheavier.com";

/// Opens the link's page, which Cloudflare's check answers: the check window reads it.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let found = window_link(link).ok_or_else(|| Host::Buzzheavier.link_not_recognized())?;
    check_cancel(cancel)?;
    let page = Page::read(client.get(&found.page).send().map_err(network_error)?);
    if page.status == StatusCode::NOT_FOUND {
        return Err(PreviewError::Unavailable {
            detail: "Buzzheavier doesn't know this file".to_owned(),
        });
    }
    // Its check, or a page that hasn't been seen without one: either way, the window reads it.
    Err(PreviewError::BrowserCheck {
        site: FACTS.name.to_owned(),
    })
}

/// The file's ID in "buzzheavier.com/<id>" or "buzzheavier.com/f/<id>".
fn window_link(link: &str) -> Option<WindowLink> {
    let url = Url::parse(link).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !Host::Buzzheavier.owns(&host) {
        return None;
    }
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    let id = match segments.as_slice() {
        [id] | ["f", id] => *id,
        _ => return None,
    };
    let is_id = id.len() >= 8
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric());
    is_id.then(|| WindowLink {
        id: id.to_owned(),
        page: format!("{SITE}/{id}"),
        name: None,
    })
}

/// Whether `download` is the file's address: "/d/<id>" on one of Buzzheavier's file servers ("ts.buzzheavier.com"),
/// never its pages on buzzheavier.com itself.
fn takes(download: &str, id: &str) -> bool {
    let Ok(url) = Url::parse(download) else {
        return false;
    };
    let on_file_server = url.host_str().is_some_and(|host| {
        let host = host.to_ascii_lowercase();
        host.strip_suffix(".buzzheavier.com")
            .is_some_and(|server| !server.is_empty() && server != "www" && !server.contains('.'))
    });
    let segments: Vec<&str> = url.path_segments().map_or_else(Vec::new, Iterator::collect);
    url.scheme() == "https" && on_file_server && segments.as_slice() == ["d", id]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_its_links() {
        for (link, id) in [
            ("https://buzzheavier.com/1jj7ddo8an5t", Some("1jj7ddo8an5t")),
            (
                "https://buzzheavier.com/f/1jj7ddo8an5t",
                Some("1jj7ddo8an5t"),
            ),
            ("https://buzzheavier.com/help", None),
            ("https://buzzheavier.com/", None),
        ] {
            assert_eq!(
                window_link(link).map(|found| found.id),
                id.map(str::to_owned),
                "{link}"
            );
        }
        assert_eq!(
            Host::of("https://buzzheavier.com/1jj7ddo8an5t"),
            Some(Host::Buzzheavier)
        );
    }

    #[test]
    fn takes_only_its_files_address() {
        // Where its Download button led on 2026-11-12, after a first click that opened an advert.
        assert!(takes(
            "https://ts.buzzheavier.com/d/1jj7ddo8an5t?v=KSHh5-P1bhGf",
            "1jj7ddo8an5t"
        ));
        assert!(
            !takes("https://buzzheavier.com/1jj7ddo8an5t", "1jj7ddo8an5t"),
            "its page"
        );
        assert!(!takes(
            "https://buzzheavier.com/d/1jj7ddo8an5t",
            "1jj7ddo8an5t"
        ));
        assert!(!takes(
            "https://ts.buzzheavier.com/d/otherfile123?v=x",
            "1jj7ddo8an5t"
        ));
        assert!(
            !takes(
                "https://file-link-cukhu.ivdtfa.space/token/p06s4d",
                "1jj7ddo8an5t"
            ),
            "an advert"
        );
    }
}
