//! MultiUp: a mirror page ("multiup.io/download/<hash>/<name>", or "/<language>/mirror/<hash>/<name>") that lists
//! the same file on several file hosts. Tsundoku opens the first mirror on a host it reads itself, in the order of
//! `PREFERRED` (hosts that give a checksum and make no one wait first), skipping those MultiUp found dead, and reads
//! that link as if it had been pasted: the preview, the queue and the history then know the mirror's host and file.
//! A page whose mirrors are all elsewhere says which hosts it has.

use reqwest::StatusCode;
use reqwest::Url;
use reqwest::blocking::Client;

use super::{Host, HostFacts, Page, Share, check_cancel, client_for, code_error, html_text};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "MultiUp",
    site: "multiup",
    domains: &["multiup.io", "multiup.org"],
    sends_attachments: false,
    // It answers Tsundoku saying who it is as it answers a browser.
    browser_agent: false,
    read,
    page_error: None,
};

const SITE: &str = "https://multiup.io";

/// The hosts a mirror is taken from first: those that give a checksum or answer at once, then those with a check, a
/// countdown or a wait between downloads.
const PREFERRED: [Host; 15] = [
    Host::GoogleDrive,
    Host::Dropbox,
    Host::OneDrive,
    Host::PCloud,
    Host::Box,
    Host::YandexDisk,
    Host::Mega,
    Host::Icedrive,
    Host::FilesFm,
    Host::SwissTransfer,
    Host::WeTransfer,
    Host::TransferNow,
    Host::UploadEe,
    Host::MediaFire,
    Host::OneFichier,
];

/// A mirror on MultiUp's page.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Mirror {
    link: String,
    /// Its host, as MultiUp names it ("clicknupload.click").
    host_name: String,
    /// MultiUp found the file gone from it.
    dead: bool,
}

/// Reads the mirror page and the first mirror on a host Tsundoku reads itself.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let page_link = mirror_page(link).ok_or_else(|| Host::MultiUp.link_not_recognized())?;
    check_cancel(cancel)?;
    let page = Page::read(
        client
            .get(&page_link)
            .send()
            .map_err(super::network_error)?,
    );
    if page.status == StatusCode::NOT_FOUND {
        return Err(PreviewError::Unavailable {
            detail: "MultiUp doesn't know this file".to_owned(),
        });
    }
    if let Some(error) = code_error(Host::MultiUp, page.status) {
        return Err(error);
    }
    let found = mirrors(&page.html);
    let (host, mirror) =
        first_supported(&found).ok_or_else(|| PreviewError::NoSupportedMirror {
            site: FACTS.name.to_owned(),
            mirrors: mirror_hosts(&found),
        })?;
    host.read(&client_for(host)?, &mirror.link, cancel)
}

/// The first mirror on one of `PREFERRED`'s hosts, in that order, leaving out the dead ones.
fn first_supported(mirrors: &[Mirror]) -> Option<(Host, &Mirror)> {
    PREFERRED.iter().find_map(|preferred| {
        mirrors
            .iter()
            .filter(|mirror| !mirror.dead)
            .find(|mirror| Host::of(&mirror.link) == Some(*preferred))
            .map(|mirror| (*preferred, mirror))
    })
}

/// The hosts a page's mirrors are on, each once.
fn mirror_hosts(mirrors: &[Mirror]) -> Vec<String> {
    let mut hosts: Vec<String> = Vec::new();
    for mirror in mirrors {
        if !hosts.contains(&mirror.host_name) {
            hosts.push(mirror.host_name.clone());
        }
    }
    hosts
}

/// The mirrors on the page: its "host" buttons, each with `link`, `nameHost` and `validity` attributes.
fn mirrors(html: &str) -> Vec<Mirror> {
    html.match_indices("<a ")
        .filter_map(|(start, _)| {
            let tag = &html[start..start + html[start..].find('>')?];
            let link = html_text(attribute(tag, "link")?);
            let url = Url::parse(&link).ok()?;
            if !matches!(url.scheme(), "https" | "http") {
                return None;
            }
            let host_name = attribute(tag, "nameHost")
                .map(html_text)
                .or_else(|| url.host_str().map(str::to_owned))?;
            Some(Mirror {
                link,
                host_name,
                dead: attribute(tag, "validity").is_some_and(|validity| validity == "invalid"),
            })
        })
        .collect()
}

/// An attribute's value in a tag (` name="value"`).
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!(" {name}=\"");
    let start = tag.find(&marker)? + marker.len();
    let length = tag[start..].find('"')?;
    Some(&tag[start..start + length])
}

/// The mirror list's address for a MultiUp link: "/en/mirror/<hash>/<name>".
fn mirror_page(link: &str) -> Option<String> {
    let url = Url::parse(link).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !Host::MultiUp.owns(&host) {
        return None;
    }
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    let rest = match segments.as_slice() {
        ["download" | "mirror", rest @ ..] => rest,
        [language, "download" | "mirror", rest @ ..] if language.len() == 2 => rest,
        _ => return None,
    };
    let (hash, name) = match rest {
        [hash, name, ..] => (*hash, *name),
        [hash] => (*hash, "file"),
        _ => return None,
    };
    let is_hash = hash.len() == 32 && hash.chars().all(|character| character.is_ascii_hexdigit());
    is_hash.then(|| format!("{SITE}/en/mirror/{}/{name}", hash.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mirror on MultiUp's page on 2026-11-11, with two made up around it.
    const PAGE: &str = r#"<footer class="panel-footer">
        <a href="https://clicknupload.click/8qa3vg6x38zq" target="_blank"
           nameHost="clicknupload.click"
           link="https://clicknupload.click/8qa3vg6x38zq"
           validity="valid"
           dateLastChecked="6/17/18, 4:33 AM"
           id="88215855"
           class="host btn btn-md btn-default btn-block btn-3d hvr-bounce-to-right">Download (0)</a>
        <a href="https://1fichier.com/?abcdefghij" nameHost="1fichier.com" link="https://1fichier.com/?abcdefghij" validity="valid" class="host btn">Download</a>
        <a href="https://mega.nz/file/AbCdEf12#key" nameHost="mega.nz" link="https://mega.nz/file/AbCdEf12#key" validity="invalid" class="host btn">Download</a>
        <a href="https://www.mediafire.com/file/abc123" nameHost="mediafire.com" link="https://www.mediafire.com/file/abc123" validity="unknown" class="host btn">Download</a>
        <a href="/en/upload/from-links">Upload</a>
    </footer>"#;

    #[test]
    fn reads_its_links() {
        for (link, page) in [
            (
                "https://multiup.io/download/080e3fb87debea6e9818d55aa1c68b43/test.txt",
                Some("https://multiup.io/en/mirror/080e3fb87debea6e9818d55aa1c68b43/test.txt"),
            ),
            (
                "https://multiup.org/fr/mirror/080E3FB87DEBEA6E9818D55AA1C68B43/test.txt",
                Some("https://multiup.io/en/mirror/080e3fb87debea6e9818d55aa1c68b43/test.txt"),
            ),
            (
                "https://multiup.io/en/download/080e3fb87debea6e9818d55aa1c68b43",
                Some("https://multiup.io/en/mirror/080e3fb87debea6e9818d55aa1c68b43/file"),
            ),
            ("https://multiup.io/en/upload/from-links", None),
            ("https://multiup.io/download/nothex/test.txt", None),
        ] {
            assert_eq!(mirror_page(link).as_deref(), page, "{link}");
        }
        assert_eq!(
            Host::of("https://multiup.io/download/080e3fb87debea6e9818d55aa1c68b43/test.txt"),
            Some(Host::MultiUp)
        );
    }

    #[test]
    fn opens_the_first_live_mirror_on_a_host_it_reads() {
        let found = mirrors(PAGE);
        assert_eq!(found.len(), 4, "the host buttons only");
        assert!(found[2].dead);
        let (host, mirror) = first_supported(&found).unwrap();
        assert_eq!(
            host,
            Host::MediaFire,
            "MEGA's is dead, and 1fichier comes last"
        );
        assert_eq!(mirror.link, "https://www.mediafire.com/file/abc123");
        assert_eq!(
            mirror_hosts(&found),
            [
                "clicknupload.click",
                "1fichier.com",
                "mega.nz",
                "mediafire.com"
            ]
        );
        assert_eq!(first_supported(&found[..1]), None, "clicknupload only");
    }
}
