//! Short links (Bitly's, TinyURL's): a link that only redirects to another address. Before a link is read, `resolve`
//! asks a short link where it leads, once and as Tsundoku, without following the redirect, and the address it names
//! is then read as if it had been pasted (a mirror page included: `preview` asks `mirrors` next). The preview, the
//! queue, the history and the folder rules only see that address.
//!
//! Only the sites in `SITES` are followed, each listed once a real link of it resolved through this code. Tsundoku
//! never reads a page for where it leads: a site that shows one first (a preview, a warning) says so
//! (`PreviewError::ShortLinkPage`), and the person opens it in a browser.

use reqwest::Url;
use reqwest::blocking::Response;
use reqwest::header::LOCATION;

use crate::cancel::CancelToken;
use crate::hosts::{self, SendChecked, check_cancel};
use crate::metadata::{self, PreviewError};
use crate::sites;

/// The most redirects followed for one link, through short-link sites only (http to https, Bitly's bit.ly to
/// bitly.com, a short link to another).
const MOST_HOPS: usize = 5;

/// A short-link site: its name in messages, and the domains its links are on.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ShortLinkSite {
    pub name: &'static str,
    pub domains: &'static [&'static str],
}

/// The sites followed, each tried with a real link before it was listed: on 2026-11-12, each answered Tsundoku with a
/// redirect to the link's address (most to a Google Drive file or folder, from GitHub READMEs). Left out: v.gd, which
/// shows a preview page first, and shorturl.at, which answered three links of four with a page.
static SITES: [ShortLinkSite; 11] = [
    site("Bitly", &["bit.ly", "bitly.com"]),
    site("TinyURL", &["tinyurl.com"]),
    site("is.gd", &["is.gd"]),
    // X's, for the links in posts.
    site("X", &["t.co"]),
    site("Cuttly", &["cutt.ly"]),
    site("RB.GY", &["rb.gy"]),
    site("Tiny.cc", &["tiny.cc"]),
    site("T.LY", &["t.ly"]),
    // Google's old short links, which still lead where they did.
    site("Google", &["goo.gl"]),
    // Hootsuite's.
    site("Ow.ly", &["ow.ly"]),
    // Buffer's.
    site("Buffer", &["buff.ly"]),
];

const fn site(name: &'static str, domains: &'static [&'static str]) -> ShortLinkSite {
    ShortLinkSite { name, domains }
}

/// Where a short link leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortLink {
    /// The short-link site's name, for messages ("Bitly").
    pub site: &'static str,
    /// The first address off the short-link sites, as `metadata::normalize_link` gives it.
    pub target: String,
}

impl ShortLink {
    /// The site it leads to, as a person reads it: the target's host without "www.".
    pub fn target_site(&self) -> String {
        let host = sites::host(&self.target).unwrap_or_default();
        host.strip_prefix("www.").unwrap_or(&host).to_owned()
    }
}

/// Where `link` (from `metadata::normalize_link`) leads, when it's a short link of a listed site; `None` for any other
/// link, and for a short-link site's own pages without a path (its home page).
pub fn resolve(link: &str, cancel: &CancelToken) -> Option<Result<ShortLink, PreviewError>> {
    resolve_among(&SITES, link, cancel)
}

fn resolve_among(
    listed: &'static [ShortLinkSite],
    link: &str,
    cancel: &CancelToken,
) -> Option<Result<ShortLink, PreviewError>> {
    let address = web_link(link)?;
    let site = site_of(listed, &address)?;
    if !has_path(&address) {
        return None;
    }
    Some(follow(listed, site, address, cancel))
}

/// Asks each short link where it leads until an address off the short-link sites, at most `MOST_HOPS` times.
fn follow(
    listed: &'static [ShortLinkSite],
    first: &'static ShortLinkSite,
    mut address: Url,
    cancel: &CancelToken,
) -> Result<ShortLink, PreviewError> {
    let client = hosts::tsundoku_client_without_redirects()?;
    let mut asked = first;
    for _ in 0..MOST_HOPS {
        check_cancel(cancel)?;
        let response = client.get(address.as_str()).send_checked(asked.name)?;
        let next = next_address(first, asked, &address, &response)?;
        match site_of(listed, &next) {
            // A short-link site's home page: it didn't know the link.
            Some(_) if !has_path(&next) => return Err(unknown_link(asked)),
            Some(next_site) => {
                asked = next_site;
                address = next;
            }
            None => {
                let target = metadata::normalize_link(next.as_str()).ok_or_else(|| {
                    PreviewError::Failed {
                        detail: format!(
                            "{}'s link leads to {next}, which isn't a web page",
                            first.name
                        ),
                    }
                })?;
                return Ok(ShortLink {
                    site: first.name,
                    target,
                });
            }
        }
    }
    Err(PreviewError::Failed {
        detail: format!(
            "{}'s link went through {MOST_HOPS} short links without leading anywhere else",
            first.name
        ),
    })
}

/// Where `asked`'s answer to `address` leads, or why it leads nowhere Tsundoku follows. `first` is the site of the
/// link the person gave, which a page's error names.
fn next_address(
    first: &ShortLinkSite,
    asked: &ShortLinkSite,
    address: &Url,
    response: &Response,
) -> Result<Url, PreviewError> {
    let status = response.status();
    let detail = || format!("{} answered {status}", asked.name);
    if status.is_redirection() {
        let location = response
            .headers()
            .get(LOCATION)
            .map(|value| String::from_utf8_lossy(value.as_bytes()).trim().to_owned())
            .filter(|location| !location.is_empty())
            .ok_or_else(|| PreviewError::Failed {
                detail: format!("{} without saying where it leads", detail()),
            })?;
        let next = address
            .join(&location)
            .map_err(|error| PreviewError::Failed {
                detail: format!(
                    "{}'s link leads to an address that can't be read ({error}): {location}",
                    asked.name
                ),
            })?;
        return match web_link(next.as_str()) {
            Some(next) => Ok(next),
            None => Err(PreviewError::Failed {
                detail: format!(
                    "{}'s link leads to a {}: address, not a web page",
                    asked.name,
                    next.scheme()
                ),
            }),
        };
    }
    Err(match status.as_u16() {
        404 | 410 => unknown_link(asked),
        // A preview or a warning, which Tsundoku doesn't read for where it leads.
        200..=299 => PreviewError::ShortLinkPage {
            site: first.name.to_owned(),
        },
        429 => PreviewError::too_many_requests(detail()),
        500..=599 => PreviewError::Network { detail: detail() },
        _ => PreviewError::Failed { detail: detail() },
    })
}

fn unknown_link(site: &ShortLinkSite) -> PreviewError {
    PreviewError::Unavailable {
        detail: format!("{} doesn't know this link", site.name),
    }
}

/// The listed site `address` is on.
fn site_of(listed: &'static [ShortLinkSite], address: &Url) -> Option<&'static ShortLinkSite> {
    let host = address.host_str()?;
    listed.iter().find(|site| {
        site.domains
            .iter()
            .any(|domain| sites::on_domain(host, domain))
    })
}

/// Whether an address has a path beyond "/", as a short link does and a site's home page doesn't.
fn has_path(address: &Url) -> bool {
    !address.path().trim_matches('/').is_empty()
}

/// An http or https address with a host.
fn web_link(link: &str) -> Option<Url> {
    Url::parse(link)
        .ok()
        .filter(|url| matches!(url.scheme(), "https" | "http") && url.host_str().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::test_server::{Reply, TestServer};

    /// The test server, on this PC, counting as a short-link site.
    static TEST_SITES: [ShortLinkSite; 1] = [site("Shorty", &["127.0.0.1"])];

    fn resolved(server: &TestServer, path: &str) -> Option<Result<ShortLink, PreviewError>> {
        resolve_among(&TEST_SITES, &server.url(path), &CancelToken::new())
    }

    fn redirect(status: u16, location: &str) -> Reply {
        Reply::new(status, "").with("Location", location)
    }

    fn short_link(target: &str) -> ShortLink {
        ShortLink {
            site: "Shorty",
            target: target.to_owned(),
        }
    }

    #[test]
    fn knows_short_links_by_their_domain() {
        for (link, name) in [
            ("https://bit.ly/3abcDEF", Some("Bitly")),
            ("http://BIT.LY/3abcDEF", Some("Bitly")),
            ("https://bitly.com/3abcDEF", Some("Bitly")),
            ("https://tinyurl.com/2p8z7k4j", Some("TinyURL")),
            ("https://www.tinyurl.com/2p8z7k4j", Some("TinyURL")),
            ("https://notbit.ly/3abcDEF", None),
            ("https://www.youtube.com/watch?v=jNQXAC9IVRw", None),
        ] {
            let address = web_link(link).unwrap();
            assert_eq!(
                site_of(&SITES, &address).map(|site| site.name),
                name,
                "{link}"
            );
        }
        // The site's own home page isn't a short link, and other links aren't asked anything.
        let cancel = CancelToken::new();
        for link in [
            "https://bit.ly/",
            "https://tinyurl.com",
            "https://example.com/abc",
            "ftp://bit.ly/abc",
        ] {
            assert_eq!(resolve(link, &cancel), None, "{link}");
        }
    }

    #[test]
    fn follows_a_redirect_to_its_target() {
        let server = TestServer::start(|request| match request.path.as_str() {
            "/abc" => redirect(301, "https://www.example.com/video?v=1"),
            "/temporary" => redirect(302, "http://example.com/a%20b"),
            _ => Reply::new(500, "never asked"),
        });
        assert_eq!(
            resolved(&server, "/abc"),
            Some(Ok(short_link("https://www.example.com/video?v=1")))
        );
        assert_eq!(
            resolved(&server, "/temporary"),
            Some(Ok(short_link("http://example.com/a%20b")))
        );
        assert_eq!(resolved(&server, "/"), None, "the site's home page");
        assert_eq!(
            short_link("https://www.example.com/video").target_site(),
            "example.com"
        );
    }

    #[test]
    fn follows_a_short_link_to_another_one_and_a_relative_location() {
        let server = TestServer::start(|request| match request.path.as_str() {
            "/first" => redirect(301, "/second"),
            "/second" => {
                let port = request.header("host").unwrap_or_default().to_owned();
                redirect(307, &format!("http://{port}/third"))
            }
            "/third" => redirect(308, "https://drive.google.com/file/d/abc/view"),
            _ => Reply::new(500, "never asked"),
        });
        assert_eq!(
            resolved(&server, "/first"),
            Some(Ok(short_link("https://drive.google.com/file/d/abc/view")))
        );
    }

    #[test]
    fn says_why_a_short_link_leads_nowhere() {
        let server = TestServer::start(|request| match request.path.as_str() {
            "/gone" => Reply::new(404, "Not Found"),
            "/removed" => Reply::new(410, "Gone"),
            "/preview" => Reply::new(200, "<a href=\"https://example.com/\">Continue</a>"),
            "/loop" => redirect(302, "/loop"),
            "/script" => redirect(302, "javascript:alert(1)"),
            "/nowhere" => Reply::new(301, ""),
            "/home" => redirect(301, "/"),
            "/busy" => Reply::new(429, ""),
            "/down" => Reply::new(502, ""),
            _ => Reply::new(403, ""),
        });
        let error = |path: &str| resolved(&server, path).unwrap().unwrap_err();
        let unknown = PreviewError::Unavailable {
            detail: "Shorty doesn't know this link".to_owned(),
        };
        assert_eq!(error("/gone"), unknown);
        assert_eq!(error("/removed"), unknown);
        assert_eq!(error("/home"), unknown, "sent to its home page");
        assert_eq!(
            error("/preview"),
            PreviewError::ShortLinkPage {
                site: "Shorty".to_owned()
            },
            "a page is never read for where it leads"
        );
        assert!(
            matches!(error("/loop"), PreviewError::Failed { detail } if detail.contains("5 short links"))
        );
        assert!(
            matches!(error("/script"), PreviewError::Failed { detail } if detail.contains("javascript:"))
        );
        assert!(
            matches!(error("/nowhere"), PreviewError::Failed { detail } if detail.contains("without saying where"))
        );
        assert!(matches!(
            error("/busy"),
            PreviewError::TooManyRequests { .. }
        ));
        assert!(matches!(error("/down"), PreviewError::Network { .. }));
        assert!(matches!(error("/forbidden"), PreviewError::Failed { .. }));
    }

    #[test]
    fn a_check_stays_a_check() {
        let server = TestServer::start(|_| {
            Reply::new(403, "<title>Just a moment...</title>").with("cf-mitigated", "challenge")
        });
        assert_eq!(
            resolved(&server, "/abc"),
            Some(Err(PreviewError::BrowserCheck {
                site: "Shorty".to_owned()
            }))
        );
    }

    #[test]
    fn asks_once_as_tsundoku() {
        let server = TestServer::start(|request| {
            let agent = request.header("user-agent").unwrap_or_default();
            if agent.starts_with("Tsundoku/") {
                redirect(301, "https://example.com/")
            } else {
                Reply::new(400, "")
            }
        });
        assert_eq!(
            resolved(&server, "/abc"),
            Some(Ok(short_link("https://example.com/")))
        );
    }
}

/// A live check, run by hand when a short-link site changes: `SHORT_LINKS="<link> <link>" cargo test -p tsundoku-core
/// --lib short_links_live -- --ignored --nocapture`. It needs the network.
#[cfg(test)]
mod short_links_live {
    use super::*;

    #[test]
    #[ignore = "needs the network"]
    fn short_links_live_resolves_each_link() {
        let links = std::env::var("SHORT_LINKS").unwrap_or_default();
        for link in links.split_whitespace() {
            let link = metadata::normalize_link(link).unwrap_or_else(|| link.to_owned());
            println!(
                "{link}
  {:?}",
                resolve(&link, &CancelToken::new())
            );
        }
    }
}
