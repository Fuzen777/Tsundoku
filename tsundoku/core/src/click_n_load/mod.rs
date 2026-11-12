//! Click'n'Load: the way a protected link folder (FileCrypt's) hands its links to a download manager on the same PC,
//! once the person passed its check in their own browser. The page probes `http://127.0.0.1:9666/jdcheck.js`, then
//! posts the links to `/flash/addcrypted2` (encrypted, `decode`) or `/flash/add` (plain). Tsundoku answers as
//! JDownloader does, so the page's button works, takes links only from the sites in `SENDERS`, and puts them in
//! Pending (`PendingLinks::add_from_page`), where they're read and never start by themselves.
//!
//! The receiver (`server`) listens on this PC only, while Settings → Adding Links has it on. JDownloader listens on the
//! same port, so only one of them can at a time (`StartError::PortTaken`).

pub mod decode;
mod server;
#[cfg(test)]
mod tests;

use reqwest::Url;
use serde::Serialize;
use ts_rs::TS;

pub use decode::{DecodeError, decode};
pub use server::{Receiver, StartError};

use crate::sites;

/// The port Click'n'Load pages send to.
pub const PORT: u16 = 9666;

/// What `/jdcheck.js` answers: the page looks for `jdownloader` to know a receiver runs.
const JDCHECK: &str = "jdownloader=true;\r\nvar version='42707';\r\n";
/// What `/flash/` answers.
const FLASH: &str = "JDownloader";
/// What a post that worked answers.
const SUCCESS: &str = "success\r\n";

/// A site whose Click'n'Load button Tsundoku takes links from: its name in messages, and its domains.
#[derive(Debug, PartialEq, Eq)]
pub struct SenderSite {
    pub name: &'static str,
    pub domains: &'static [&'static str],
}

/// The sites taken from. A site is listed once a real click of its button sent links here.
pub static SENDERS: [SenderSite; 1] = [
    // Listed from its documented Click'n'Load form, on 2026-11-12: its button calls `CNLPOP(id, key, crypted, name)`,
    // which its script (static.filecrypt.to/js/container/cnl.js, read once as Tsundoku that day) hands to its own
    // helper page, opened over http in a pop-up (`http://filecrypt.cc/helper.html`), by `postMessage`; that page
    // posts to the receiver, so the sender is FileCrypt's own address. A real click needs its captcha passed in a
    // person's browser, so it's left for Fuzen (TESTING.md).
    SenderSite {
        name: "FileCrypt",
        domains: &["filecrypt.cc", "filecrypt.co", "filecrypt.to"],
    },
];

/// The sites a receiver takes links from: `SENDERS`, and in development a test page's host.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Senders {
    /// A host whose pages count as a sender, named "Test Page": for a developer's own page, never set in a release
    /// build (the app crate reads it from `TSUNDOKU_CNL_TEST_SENDER` in debug builds only).
    test_host: Option<String>,
}

impl Senders {
    /// The tested list alone.
    pub fn listed() -> Self {
        Self::default()
    }

    /// The tested list, and pages on `host` as a test sender.
    pub fn with_test_host(host: &str) -> Self {
        Self {
            test_host: Some(host.trim().to_ascii_lowercase()).filter(|host| !host.is_empty()),
        }
    }

    /// The name of the sender `host` belongs to, if it's one.
    fn site_of(&self, host: &str) -> Option<&str> {
        if let Some(listed) = SENDERS.iter().find(|site| {
            site.domains
                .iter()
                .any(|domain| sites::on_domain(host, domain))
        }) {
            return Some(listed.name);
        }
        self.test_host
            .as_deref()
            .filter(|test_host| host.eq_ignore_ascii_case(test_host))
            .map(|_| "Test Page")
    }

    /// The tested list's names, for Settings.
    pub fn names() -> Vec<String> {
        SENDERS.iter().map(|site| site.name.to_owned()).collect()
    }
}

/// Where Click'n'Load's links go, and who hears of a refused sender. The app crate puts them in Pending and tells the
/// person; tests keep them.
pub trait ClickNLoadListener: Send + Sync {
    /// `site` (a sender's name) sent `links`. Whether they were kept.
    fn received(&self, site: &str, links: &[String]) -> bool;
    /// A page that isn't a sender (its host, or none known) tried to send links.
    fn refused(&self, host: Option<&str>);
}

/// Whether the receiver listens, for Settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum ReceiverState {
    /// Settings has it off.
    Off,
    Listening,
    /// Another app listens on the port (JDownloader, say).
    PortTaken,
    /// It couldn't listen for another reason.
    Failed {
        detail: String,
    },
}

/// Settings → Adding Links' Click'n'Load line: whether it listens, on which port, and the sites it takes links from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClickNLoadStatus {
    pub state: ReceiverState,
    pub port: u16,
    pub senders: Vec<String>,
}

/// A request, as `server` read it.
#[derive(Debug, Default)]
pub(crate) struct Request {
    pub method: String,
    /// Without its query.
    pub path: String,
    /// Names in lowercase.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// An answer, as `server` writes it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub headers: Vec<(&'static str, String)>,
    pub body: String,
}

impl Reply {
    pub(crate) fn text(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            headers: Vec::new(),
            body: body.into(),
        }
    }

    fn with(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }
}

/// Who sent a request, as far as its headers say.
#[derive(Debug, PartialEq, Eq)]
enum Sender {
    /// The `Origin` header (not "null"): `origin` as sent, for CORS, and its host.
    Origin {
        origin: String,
        host: Option<String>,
    },
    /// The `Referer` header's host.
    Referer(Option<String>),
    /// Neither.
    Unknown,
}

impl Sender {
    fn of(request: &Request) -> Self {
        if let Some(origin) = request
            .header("origin")
            .map(str::trim)
            .filter(|origin| !origin.is_empty() && *origin != "null")
        {
            return Self::Origin {
                origin: origin.to_owned(),
                host: host_of(origin),
            };
        }
        match request
            .header("referer")
            .map(str::trim)
            .filter(|referer| !referer.is_empty())
        {
            Some(referer) => Self::Referer(host_of(referer)),
            None => Self::Unknown,
        }
    }

    /// The host the headers name; `None` when they name none, or one that can't be read.
    fn host(&self) -> Option<&str> {
        match self {
            Self::Origin { host, .. } | Self::Referer(host) => host.as_deref(),
            Self::Unknown => None,
        }
    }
}

/// The host of an http or https address.
fn host_of(address: &str) -> Option<String> {
    Url::parse(address)
        .ok()
        .filter(|url| matches!(url.scheme(), "https" | "http"))
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
}

/// Answers a request the receiver on `port` read: the guards, then the route.
pub(crate) fn answer(
    request: &Request,
    port: u16,
    senders: &Senders,
    listener: &dyn ClickNLoadListener,
) -> Reply {
    // Only an address of this PC, so a site whose name resolves here can't reach the receiver.
    let host = request
        .header("host")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if ![
        format!("127.0.0.1:{port}"),
        format!("localhost:{port}"),
        format!("[::1]:{port}"),
    ]
    .contains(&host)
    {
        return Reply::text(
            403,
            "Tsundoku only answers requests to this PC's own address.",
        );
    }
    let sender = Sender::of(request);
    let named_site = sender.host().and_then(|host| senders.site_of(host));
    // CORS for a listed sender's page, so a page posting with fetch can read the answer.
    let cors = |reply: Reply| match (&sender, named_site) {
        (Sender::Origin { origin, .. }, Some(_)) => reply
            .with("Access-Control-Allow-Origin", origin.clone())
            .with("Vary", "Origin"),
        _ => reply.with("Vary", "Origin"),
    };
    let path = request.path.trim_end_matches('/');
    match (request.method.as_str(), path) {
        ("OPTIONS", _) => match (&sender, named_site) {
            (Sender::Origin { .. }, Some(_)) => {
                let mut reply = cors(Reply::text(204, ""))
                    .with("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                    .with("Access-Control-Allow-Private-Network", "true")
                    .with("Access-Control-Max-Age", "600");
                if let Some(asked) = request.header("access-control-request-headers") {
                    reply = reply.with("Access-Control-Allow-Headers", asked.to_owned());
                }
                reply
            }
            _ => refuse(&sender, listener),
        },
        ("GET", "/jdcheck.js" | "/flash") => {
            // A probe without Origin or Referer is answered: it says nothing but that a receiver runs.
            if !matches!(sender, Sender::Unknown) && named_site.is_none() {
                return refuse(&sender, listener);
            }
            let reply = if path == "/flash" {
                Reply::text(200, FLASH)
            } else {
                Reply {
                    content_type: "text/javascript; charset=utf-8",
                    ..Reply::text(200, JDCHECK)
                }
            };
            cors(reply)
        }
        ("POST", "/flash/addcrypted2" | "/flash/add" | "/flash/addcrypted") => {
            if request
                .header("content-type")
                .is_some_and(|kind| kind.to_ascii_lowercase().starts_with("multipart/"))
            {
                return cors(Reply::text(
                    400,
                    "Tsundoku reads Click'n'Load forms sent as application/x-www-form-urlencoded.",
                ));
            }
            let form = Form::read(&request.body);
            // Without Origin or Referer, the form's own source names the site, unverified.
            let site = match (&sender, named_site) {
                (_, Some(site)) => Some(site),
                (Sender::Unknown, None) => form
                    .get("source")
                    .and_then(host_of)
                    .and_then(|host| senders.site_of(&host)),
                _ => None,
            };
            let Some(site) = site else {
                let host = sender
                    .host()
                    .map(str::to_owned)
                    .or_else(|| form.get("source").and_then(host_of));
                listener.refused(host.as_deref());
                return cors(refusal());
            };
            cors(receive(path, site, &form, listener))
        }
        (
            _,
            "/jdcheck.js" | "/flash" | "/flash/addcrypted2" | "/flash/add" | "/flash/addcrypted",
        ) => cors(Reply::text(405, "Not allowed.").with("Allow", "GET, POST, OPTIONS")),
        _ => cors(Reply::text(404, "Not found.")),
    }
}

/// A listed site's post: its links, decoded and handed on.
fn receive(path: &str, site: &str, form: &Form, listener: &dyn ClickNLoadListener) -> Reply {
    let links = match path {
        "/flash/addcrypted2" => decode(
            form.get("crypted").unwrap_or_default(),
            form.get("jk").unwrap_or_default(),
        ),
        "/flash/add" => match decode::web_links_in(form.get("urls").unwrap_or_default()) {
            links if links.is_empty() => Err(DecodeError::NoLinks),
            links => Ok(links),
        },
        _ => {
            return Reply::text(
                400,
                "Tsundoku doesn't read DLC containers. Use the page's Click'n'Load button instead.",
            );
        }
    };
    match links {
        Ok(links) if listener.received(site, &links) => Reply::text(200, SUCCESS),
        Ok(_) => Reply::text(500, "Tsundoku couldn't keep these links. Try again."),
        Err(error) => Reply::text(400, error.to_string()),
    }
}

/// What a page that isn't a sender gets.
fn refusal() -> Reply {
    let names: Vec<&str> = SENDERS.iter().map(|site| site.name).collect();
    Reply::text(
        403,
        format!(
            "Tsundoku only takes Click'n'Load links from {}.",
            names.join(", ")
        ),
    )
}

fn refuse(sender: &Sender, listener: &dyn ClickNLoadListener) -> Reply {
    listener.refused(sender.host());
    refusal().with("Vary", "Origin")
}

/// A form's fields, from an `application/x-www-form-urlencoded` body.
struct Form(Vec<(String, String)>);

impl Form {
    fn read(body: &[u8]) -> Self {
        let mut url = Url::parse("http://form.invalid/").expect("an address");
        url.set_query(Some(&String::from_utf8_lossy(body)));
        Self(url.query_pairs().into_owned().collect())
    }

    /// A field's first value.
    fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}
