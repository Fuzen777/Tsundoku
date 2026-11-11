//! 1fichier: public file links ("1fichier.com/?<id>", or the older "<id>.1fichier.com"), downloaded as a free
//! download without an account, asked as Tsundoku. `check_links.pl` gives a file's name and size for the preview. A
//! download then opens the file's page, which either counts down before the free download (`var ct = 60;`), which
//! Tsundoku waits out (`countdown`), or says how long to wait since this connection's last free download ("You must
//! wait 240 minutes"), which holds the download until then (`PreviewError::WaitBetweenDownloads`). Once the countdown
//! is over, posting the page's form answers with the address the file downloads from. 1fichier gives each connection
//! one such address at a time, so it's kept while the app runs, for a download that stopped partway to go on with.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::blocking::Client;
use reqwest::header::{CONTENT_TYPE, COOKIE};

use super::{
    Address, Answer, FileCheck, FileSource, Host, HostFacts, Page, RemoteFile, Share, between,
    check_cancel, code_error, form, html_text, network_error, probe, safe_name,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "1fichier",
    site: "1fichier",
    domains: &["1fichier.com"],
    sends_attachments: false,
    // It answers Tsundoku saying who it is as it answers a browser.
    browser_agent: false,
    read,
    page_error: None,
};

const SITE: &str = "https://1fichier.com";
/// Its pages in English, for the messages read from them.
const ENGLISH: &str = "LG=en";
/// How long to wait when 1fichier says a free download from this connection is still running, without a time.
const ONE_AT_A_TIME_WAIT: u64 = 5 * 60;

/// The address each file was last given, by its ID: 1fichier gives one free download at a time, so a download that
/// stopped partway goes on from it rather than waiting for the next.
static GIVEN: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// A file to download from 1fichier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneFichierFile {
    pub id: String,
}

/// Reads a 1fichier link: the file's name and size.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let id = parse_link(link).ok_or_else(|| Host::OneFichier.link_not_recognized())?;
    let page = page_link(&id);
    check_cancel(cancel)?;
    let response = client
        .post(format!("{SITE}/check_links.pl"))
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(form(&[("links[]", &page)]))
        .send()
        .map_err(network_error)?;
    if let Some(error) = code_error(Host::OneFichier, response.status()) {
        return Err(error);
    }
    let answer = response.text().map_err(network_error)?;
    let (name, size) = match listed(&answer, &id) {
        Some(Listed::File { name, size }) => (name, size),
        Some(Listed::Missing) => return Err(missing()),
        // A password or the owner's choice of who may download it: the page says which.
        Some(Listed::Restricted) => return Err(restriction(&open_page(client, &id, cancel)?)),
        None => {
            return Err(PreviewError::Failed {
                detail: "1fichier's answer didn't name the file".to_owned(),
            });
        }
    };
    Ok(Share {
        host: Host::OneFichier,
        id: id.clone(),
        url: page,
        title: name.clone(),
        upload_date: None,
        files: vec![RemoteFile {
            path: PathBuf::from(safe_name(&name)),
            size,
            source: FileSource::OneFichier(OneFichierFile { id }),
        }],
    })
}

/// Opens the file's page for a free download: how long it counts down first, or why it can't start now.
pub(super) fn countdown(
    client: &Client,
    file: &OneFichierFile,
    cancel: &CancelToken,
) -> Result<Duration, PreviewError> {
    if given_still_works(client, &file.id, cancel)? {
        return Ok(Duration::ZERO);
    }
    let page = open_page(client, &file.id, cancel)?;
    if let Some(problem) = page_problem(&page.html) {
        return Err(problem);
    }
    if !page.html.contains("id=\"f1\"") {
        return Err(unexpected("its page had no free download"));
    }
    Ok(Duration::from_secs(
        countdown_seconds(&page.html).unwrap_or(0),
    ))
}

/// Asks 1fichier for the file's address, once its countdown is over.
pub(super) fn address(
    client: &Client,
    file: &OneFichierFile,
    cancel: &CancelToken,
) -> Result<Address, PreviewError> {
    let url = match given(&file.id) {
        Some(url) => url,
        None => {
            check_cancel(cancel)?;
            let response = client
                .post(page_link(&file.id))
                .header(COOKIE, ENGLISH)
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body("")
                .send()
                .map_err(network_error)?;
            let page = Page::read(response);
            if page.status == StatusCode::NOT_FOUND {
                return Err(missing());
            }
            if let Some(problem) = page_problem(&page.html) {
                return Err(problem);
            }
            let url = download_link(&page.html)
                .ok_or_else(|| unexpected("it gave no address to download from"))?;
            remember(&file.id, &url);
            url
        }
    };
    Ok(Address {
        url,
        check: FileCheck::None,
    })
}

/// Whether the address given for the file before still leads to it. One that doesn't is forgotten.
fn given_still_works(
    client: &Client,
    id: &str,
    cancel: &CancelToken,
) -> Result<bool, PreviewError> {
    let Some(url) = given(id) else {
        return Ok(false);
    };
    if matches!(probe(client, &url, cancel)?, Answer::File(_)) {
        return Ok(true);
    }
    forget(id);
    Ok(false)
}

fn given(id: &str) -> Option<String> {
    let given = GIVEN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    given
        .iter()
        .find(|(file, _)| file == id)
        .map(|(_, url)| url.clone())
}

fn remember(id: &str, url: &str) {
    let mut given = GIVEN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    given.retain(|(file, _)| file != id);
    given.push((id.to_owned(), url.to_owned()));
}

fn forget(id: &str) {
    GIVEN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .retain(|(file, _)| file != id);
}

/// The file's page, in English.
fn open_page(client: &Client, id: &str, cancel: &CancelToken) -> Result<Page, PreviewError> {
    check_cancel(cancel)?;
    let response = client
        .get(page_link(id))
        .header(COOKIE, ENGLISH)
        .send()
        .map_err(network_error)?;
    let page = Page::read(response);
    if page.status == StatusCode::NOT_FOUND {
        return Err(missing());
    }
    if let Some(error) = code_error(Host::OneFichier, page.status) {
        return Err(error);
    }
    Ok(page)
}

/// What `check_links.pl` says about a file.
#[derive(Debug, PartialEq, Eq)]
enum Listed {
    File {
        name: String,
        size: u64,
    },
    /// "NOT FOUND" or "BAD LINK".
    Missing,
    /// "PRIVATE": a password, or kept for subscribers.
    Restricted,
}

/// The line about the file with `id` in `check_links.pl`'s answer: "<link>;<name>;<size>", or "<link>;;;<status>".
fn listed(answer: &str, id: &str) -> Option<Listed> {
    answer.lines().find_map(|line| {
        let (link, rest) = line.trim().split_once(';')?;
        if parse_link(link).as_deref() != Some(id) {
            return None;
        }
        if let Some(status) = rest.strip_prefix(";;") {
            return Some(match status.trim() {
                "PRIVATE" => Listed::Restricted,
                _ => Listed::Missing,
            });
        }
        let (name, size) = rest.rsplit_once(';')?;
        let name = html_text(name.trim());
        Some(Listed::File {
            name: if name.is_empty() { id.to_owned() } else { name },
            size: size.trim().parse().ok()?,
        })
    })
}

/// Why the page doesn't lead to a free download now, if it says.
fn page_problem(html: &str) -> Option<PreviewError> {
    let text = html.to_lowercase();
    if html.contains("name=\"pass\"") {
        return Some(PreviewError::PasswordProtected {
            site: FACTS.name.to_owned(),
        });
    }
    if text.contains("must wait") || text.contains("devez attendre") {
        let seconds = minutes_named(&text)
            .map(|minutes| minutes * 60)
            .or_else(|| countdown_seconds(html))
            .unwrap_or(ONE_AT_A_TIME_WAIT);
        return Some(PreviewError::WaitBetweenDownloads {
            site: FACTS.name.to_owned(),
            seconds,
        });
    }
    if text.contains("one file at a time") || text.contains("un seul fichier") {
        return Some(PreviewError::WaitBetweenDownloads {
            site: FACTS.name.to_owned(),
            seconds: ONE_AT_A_TIME_WAIT,
        });
    }
    if text.contains("free download is limited") || text.contains("gratuit est limité") {
        return Some(PreviewError::LimitReached {
            site: FACTS.name.to_owned(),
            detail: "1fichier limits free downloads for now".to_owned(),
        });
    }
    if text.contains("reserved access to the subscribers") || text.contains("accès aux abonnés") {
        return Some(PreviewError::SubscribersOnly {
            site: FACTS.name.to_owned(),
        });
    }
    None
}

/// Why `check_links.pl` called the file private, from its page.
fn restriction(page: &Page) -> PreviewError {
    page_problem(&page.html).unwrap_or_else(|| PreviewError::Unavailable {
        detail: "1fichier keeps this file private".to_owned(),
    })
}

/// The first number of minutes the text names ("You must wait 240 minutes").
fn minutes_named(text: &str) -> Option<u64> {
    text.match_indices(" minute").find_map(|(index, _)| {
        let before = &text[..index];
        let digits = before.len()
            - before
                .trim_end_matches(|character: char| character.is_ascii_digit())
                .len();
        before[before.len() - digits..].parse().ok()
    })
}

/// The countdown's length in the page's script: "var ct = 60;", or "var ct = 240*60;" on the page of a wait.
fn countdown_seconds(html: &str) -> Option<u64> {
    between(html, "var ct = ", ";")?
        .split('*')
        .map(|factor| factor.trim().parse::<u64>().ok())
        .try_fold(1u64, |product, factor| product.checked_mul(factor?))
}

/// The address of the free download on the page the form answers with: its "ok" button's link.
fn download_link(html: &str) -> Option<String> {
    html.match_indices("<a href=\"")
        .find_map(|(start, opening)| {
            let tag = &html[start..start + html[start..].find('>')?];
            if !tag.contains("class=\"ok ") {
                return None;
            }
            let href = html_text(between(&tag[opening.len() - 1..], "\"", "\"")?);
            let url = reqwest::Url::parse(&href).ok()?;
            let host = url.host_str()?.to_ascii_lowercase();
            (matches!(url.scheme(), "https" | "http") && Host::OneFichier.owns(&host))
                .then_some(href)
        })
}

fn missing() -> PreviewError {
    PreviewError::Unavailable {
        detail: "1fichier says the file doesn't exist, or its owner deleted it".to_owned(),
    }
}

fn unexpected(what: &str) -> PreviewError {
    PreviewError::Failed {
        detail: format!("1fichier wasn't as expected: {what}"),
    }
}

/// The file's page.
fn page_link(id: &str) -> String {
    format!("{SITE}/?{id}")
}

/// The file's ID in "1fichier.com/?<id>" (with "&…" after it, such as an affiliate's code), or "<id>.1fichier.com".
fn parse_link(link: &str) -> Option<String> {
    let url = reqwest::Url::parse(link).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if !Host::OneFichier.owns(&host) {
        return None;
    }
    let id = match host.strip_suffix(".1fichier.com") {
        Some(sub) if sub != "www" && !sub.contains('.') => sub.to_owned(),
        _ => {
            if url.path() != "/" {
                return None;
            }
            url.query()?.split('&').next()?.to_ascii_lowercase()
        }
    };
    (id.len() >= 5
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric()))
    .then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Parts of 1fichier's pages as they answered on 2026-11-11.
    const COUNTDOWN_PAGE: &str = r#"<script type="text/javascript">
        var ct = 60;
        function ctt() { if(ct > 0) { ct--; $('#dlw').html('Free download in &#x23F3; ' + ct); } }
</script>
<form id="f1" accept-charset="UTF-8" method="post" action="https://1fichier.com/?9a0heqxqk0tmnaavr0nw" class="alc">
    <span class="tier-name">SampleArchive.7z</span>
    <input type="checkbox" name="dl_no_ssl" />
    <button id="dlw" class="btn-general btn-orange" disabled="disabled">Free download</button>
</form>"#;
    const WAIT_PAGE: &str = r#"<script type="text/javascript">
        var ct = 240*60;
</script>
<form id="f1" accept-charset="UTF-8" method="post" action="https://1fichier.com/?9a0heqxqk0tmnaavr0nw" class="alc">
		<div class="notice notice-alert">
			<b>Warning!</b>
        <br>Without subscription, you must wait between downloads.
        <br>You must wait 240 minutes or subscribe for less than 0,03€ per day !
		</div>"#;
    const WAIT_PAGE_FRENCH: &str = r#"<div class="notice notice-alert">
			<b>Attention !</b>
				<br>Sans souscription à une offre payante, vous devez attendre entre chaque téléchargement.
				<br>Vous devez attendre encore 238 minutes ou souscrire, à moins de 0,03€ / jour !
		</div>"#;
    const DOWNLOAD_PAGE: &str = r#"<div class="notice">
              You are downloading as a guest, at limited speed, with ads and waiting time.
          </div>
          <div class="alc">
            <a href="https://1fichier.com/console/abo.pl?abo=50"><b>Premium Light</b></a>
            <a href="https://a-7.1fichier.com/c1283715733" style="display:inline-block;float:none" class="ok btn-general btn-orange">Click here to download</a>
          </div>"#;
    const LIMITED_PAGE: &str = r#"<div class="notice">
            <b>Le téléchargement gratuit est limité.</b>
            <br>Stocker et distribuer vos fichiers a un coût réel : serveurs, bande passante, énergie.
          </div>
          <div class="alc">
            <a href="https://1fichier.com/console/abo.pl?abo=50" class="btn-general btn-orange">Passer en Premium Light</a>
          </div>"#;

    #[test]
    fn reads_its_links() {
        for (link, id) in [
            (
                "https://1fichier.com/?9a0heqxqk0tmnaavr0nw",
                Some("9a0heqxqk0tmnaavr0nw"),
            ),
            (
                "https://www.1fichier.com/?9A0HEQXQK0TMNAAVR0NW&af=12345",
                Some("9a0heqxqk0tmnaavr0nw"),
            ),
            (
                "https://9a0heqxqk0tmnaavr0nw.1fichier.com/",
                Some("9a0heqxqk0tmnaavr0nw"),
            ),
            ("https://1fichier.com/", None),
            ("https://1fichier.com/dir/AbCdEf12", None),
            ("https://1fichier.com/tarifs.html", None),
            ("https://img.1fichier.com/?x", None),
        ] {
            assert_eq!(parse_link(link).as_deref(), id, "{link}");
        }
        assert_eq!(
            Host::of("https://1fichier.com/?9a0heqxqk0tmnaavr0nw"),
            Some(Host::OneFichier)
        );
    }

    #[test]
    fn reads_what_check_links_says() {
        let id = "9a0heqxqk0tmnaavr0nw";
        assert_eq!(
            listed(
                "https://1fichier.com/?9a0heqxqk0tmnaavr0nw;SampleArchive.7z;16614003\n",
                id
            ),
            Some(Listed::File {
                name: "SampleArchive.7z".to_owned(),
                size: 16_614_003
            })
        );
        assert_eq!(
            listed(
                "https://1fichier.com/?9a0heqxqk0tmnaavr0nw;a;b &amp; c.zip;12\n",
                id
            ),
            Some(Listed::File {
                name: "a;b & c.zip".to_owned(),
                size: 12
            })
        );
        assert_eq!(
            listed("https://1fichier.com/?9a0heqxqk0tmnaavr0nw;;;NOT FOUND", id),
            Some(Listed::Missing)
        );
        assert_eq!(
            listed("https://1fichier.com/?9a0heqxqk0tmnaavr0nw;;;PRIVATE", id),
            Some(Listed::Restricted)
        );
        assert_eq!(listed("https://1fichier.com/?zzzzzzzzzz;x.zip;1", id), None);
    }

    #[test]
    fn reads_the_countdown_and_the_wait_between_downloads() {
        assert_eq!(page_problem(COUNTDOWN_PAGE), None);
        assert_eq!(countdown_seconds(COUNTDOWN_PAGE), Some(60));
        assert_eq!(countdown_seconds(WAIT_PAGE), Some(240 * 60));
        assert_eq!(
            page_problem(WAIT_PAGE),
            Some(PreviewError::WaitBetweenDownloads {
                site: "1fichier".to_owned(),
                seconds: 240 * 60
            })
        );
        assert_eq!(
            page_problem(WAIT_PAGE_FRENCH),
            Some(PreviewError::WaitBetweenDownloads {
                site: "1fichier".to_owned(),
                seconds: 238 * 60
            }),
            "a page in French says it too"
        );
        assert_eq!(minutes_named("you must wait 1 minute"), Some(1));
        assert_eq!(minutes_named("a few minutes"), None);
    }

    #[test]
    fn reads_the_download_address_and_why_there_is_none() {
        assert_eq!(page_problem(DOWNLOAD_PAGE), None);
        assert_eq!(
            download_link(DOWNLOAD_PAGE).as_deref(),
            Some("https://a-7.1fichier.com/c1283715733")
        );
        assert_eq!(download_link(LIMITED_PAGE), None, "only the ok button");
        assert!(matches!(
            page_problem(LIMITED_PAGE),
            Some(PreviewError::LimitReached { site, .. }) if site == "1fichier"
        ));
        assert_eq!(
            download_link(r#"<a href="https://example.com/c1" class="ok btn-general">x</a>"#),
            None,
            "only 1fichier's own servers"
        );
        assert_eq!(
            page_problem(r#"<form method="post"><input type="password" name="pass" /></form>"#),
            Some(PreviewError::PasswordProtected {
                site: "1fichier".to_owned()
            })
        );
        assert_eq!(
            page_problem(
                "<div>Access to this file is protected. The owner of this file has reserved access to the subscribers of our services.</div>"
            ),
            Some(PreviewError::SubscribersOnly {
                site: "1fichier".to_owned()
            })
        );
        assert_eq!(
            page_problem(
                "<div>Without premium status, you can download only one file at a time</div>"
            ),
            Some(PreviewError::WaitBetweenDownloads {
                site: "1fichier".to_owned(),
                seconds: ONE_AT_A_TIME_WAIT
            })
        );
    }

    #[test]
    fn keeps_the_address_given_for_each_file() {
        remember("test-a", "https://a-1.1fichier.com/c1");
        remember("test-b", "https://a-2.1fichier.com/c2");
        remember("test-a", "https://a-3.1fichier.com/c3");
        assert_eq!(
            given("test-a").as_deref(),
            Some("https://a-3.1fichier.com/c3")
        );
        forget("test-a");
        assert_eq!(given("test-a"), None);
        assert_eq!(
            given("test-b").as_deref(),
            Some("https://a-2.1fichier.com/c2")
        );
        forget("test-b");
    }
}
