//! OneDrive (personal): links to a file or a shared folder, read without signing in, the way OneDrive's own page for
//! a public link reads them. An anonymous token comes from Microsoft's token service (`TOKEN_URL`); every request
//! after it names the shared link itself (`/shares/u!<link>`), so it reaches only what the link shares. The answer
//! gives each file's name, size, date, SHA-1 and an address to download it from, which expires, so it's asked again
//! when the download starts. Short links (1drv.ms) and the onedrive.live.com links they lead to are both read.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::blocking::Client;
use reqwest::header::AUTHORIZATION;
use reqwest::{StatusCode, Url};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;

use super::{
    Address, FileCheck, FileSource, Host, HostFacts, Page, RemoteFile, Share, check_cancel,
    code_error, network_error, safe_name,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;
use crate::sites;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "OneDrive",
    site: "onedrive",
    domains: &[
        "onedrive.live.com",
        "1drv.ms",
        "sdrv.ms",
        "skydrive.live.com",
    ],
    sends_attachments: false,
    browser_agent: false,
    read,
    page_error: None,
    window: None,
};

/// Where an anonymous token comes from, for the app ID of OneDrive's own web page.
const TOKEN_URL: &str = "https://api-badgerp.svc.ms/v1.0/token";
const APP_ID: &str = "5cbed6ac-a083-4e14-b191-b4ba07653de2";
/// The API that reads personal OneDrive's shared links.
const API: &str = "https://my.microsoftpersonalcontent.com/_api/v2.0";
/// How long a token is used before asking for another. They last days; an hour keeps a long run from meeting the end
/// of one, and a refused token is replaced at once anyway.
const TOKEN_LIFETIME: Duration = Duration::from_secs(60 * 60);
/// The domains short links are on.
const SHORT_DOMAINS: [&str; 2] = ["1drv.ms", "sdrv.ms"];

/// The token every request is sent with, and when it was asked for.
static TOKEN: Mutex<Option<(String, Instant)>> = Mutex::new(None);
/// The last shared folder listed for its files' download addresses, so a folder's downloads don't each list it again.
static LISTING: Mutex<Option<Listing>> = Mutex::new(None);
/// How long a listing's addresses are used before listing the folder again. They last about an hour.
const LISTING_LIFETIME: Duration = Duration::from_secs(10 * 60);

/// A shared folder's own files, as its listing gave them.
struct Listing {
    share: String,
    since: Instant,
    files: Vec<Item>,
}

/// A file to download from OneDrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneDriveFile {
    /// The shared link, as the API names it ("u!" and the link in base64).
    pub share: String,
    /// The file's ID inside a shared folder, or `None` for the shared file itself.
    pub item: Option<String>,
}

/// Reads a OneDrive link: the file's name, size and date, or every file in the folder and its subfolders.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let share = parse_link(link).ok_or_else(|| Host::OneDrive.link_not_recognized())?;
    let root: Item = get(client, &item_url(&share, None), cancel)?;
    let upload_date = root.modified.as_deref().and_then(date_part);
    let files = if root.file.is_some() {
        vec![RemoteFile {
            path: PathBuf::from(safe_name(&root.name)),
            size: root.size,
            source: FileSource::OneDrive(OneDriveFile {
                share: share.clone(),
                item: None,
            }),
        }]
    } else if root.folder.is_some() {
        folder_files(&share, &root, &mut |url| get(client, url, cancel))?
    } else {
        return Err(PreviewError::NoVideo {
            detail: format!(
                "the OneDrive link is to \"{}\", which isn't a file",
                root.name
            ),
        });
    };
    Ok(Share {
        host: Host::OneDrive,
        id: root.id,
        url: link.to_owned(),
        title: root.name,
        upload_date,
        files,
    })
}

/// Asks OneDrive where a file downloads from now, and its SHA-1 to check it by: the shared file's own answer, or for
/// a file in a shared folder, the folder's listing, since OneDrive answers "item not found" when asked for an item
/// inside a shared folder by its ID.
pub(super) fn address(
    client: &Client,
    file: &OneDriveFile,
    cancel: &CancelToken,
) -> Result<Address, PreviewError> {
    let item: Item = match file.item.as_deref() {
        None => get(client, &item_url(&file.share, None), cancel)?,
        Some(id) => listed_file(&file.share, id, &mut |url| get(client, url, cancel))?,
    };
    let url = item
        .download_url
        .filter(|url| url.starts_with("https://"))
        .ok_or_else(|| PreviewError::Unavailable {
            detail: format!("OneDrive gave no address to download \"{}\"", item.name),
        })?;
    let check = item
        .file
        .and_then(|file| file.hashes)
        .and_then(|hashes| hashes.sha1)
        .filter(|hash| {
            hash.len() == 40 && hash.chars().all(|character| character.is_ascii_hexdigit())
        })
        .map_or(FileCheck::None, |hash| {
            FileCheck::Sha1(hash.to_ascii_lowercase())
        });
    Ok(Address { url, check })
}

/// A file of a shared folder as the folder's listing gives it now, from the last listing while it's recent.
fn listed_file(
    share: &str,
    id: &str,
    list: &mut dyn FnMut(&str) -> Result<Children, PreviewError>,
) -> Result<Item, PreviewError> {
    let recent =
        |listing: &Listing| listing.share == share && listing.since.elapsed() < LISTING_LIFETIME;
    if let Ok(held) = LISTING.lock()
        && let Some(listing) = held.as_ref().filter(|listing| recent(listing))
        && let Some(item) = listing.files.iter().find(|item| item.id == id)
    {
        return Ok(item.clone());
    }
    let mut files = Vec::new();
    let mut next = Some(children_url(share, None));
    while let Some(url) = next.take() {
        let page = list(&url)?;
        files.extend(page.value.into_iter().filter(|item| item.file.is_some()));
        next = page.next.filter(|next| is_own_page(next, share));
    }
    let found = files.iter().find(|item| item.id == id).cloned();
    if let Ok(mut held) = LISTING.lock() {
        *held = Some(Listing {
            share: share.to_owned(),
            since: Instant::now(),
            files,
        });
    }
    found.ok_or_else(|| PreviewError::Unavailable {
        detail: "the file is no longer in the shared OneDrive folder".to_owned(),
    })
}

/// Every file in a shared folder and its subfolders, under the folder's own name, each subfolder listed once.
/// `list` reads one page of a folder's contents from its address.
fn folder_files(
    share: &str,
    root: &Item,
    list: &mut dyn FnMut(&str) -> Result<Children, PreviewError>,
) -> Result<Vec<RemoteFile>, PreviewError> {
    let top = PathBuf::from(safe_name(&root.name));
    let mut files = Vec::new();
    let mut seen = HashSet::from([root.id.clone()]);
    let mut pending = vec![(children_url(share, None), top)];
    while let Some((first_page, path)) = pending.pop() {
        let in_subfolder = first_page != children_url(share, None);
        let mut next = Some(first_page);
        while let Some(url) = next.take() {
            let page = list(&url).map_err(|error| match error {
                // OneDrive answers "item not found" for anything below the shared folder's own level.
                PreviewError::Unavailable { .. } if in_subfolder => PreviewError::NestedFolders {
                    site: Host::OneDrive.name().to_owned(),
                },
                error => error,
            })?;
            for child in page.value {
                let child_path = path.join(safe_name(&child.name));
                if let Some(facet) = &child.folder {
                    // An empty subfolder (OneNote's recycle bin, often) has nothing to list.
                    let empty =
                        facet.get("childCount").and_then(serde_json::Value::as_u64) == Some(0);
                    if !empty && seen.insert(child.id.clone()) {
                        pending.push((children_url(share, Some(&child.id)), child_path));
                    }
                } else if child.file.is_some() {
                    files.push(RemoteFile {
                        path: child_path,
                        size: child.size,
                        source: FileSource::OneDrive(OneDriveFile {
                            share: share.to_owned(),
                            item: Some(child.id),
                        }),
                    });
                }
                // Anything else (a OneNote notebook) has no file to download.
            }
            next = match page.next {
                // The next page is only asked of the same API, about the same shared link, so the token never goes
                // elsewhere.
                Some(next) if is_own_page(&next, share) => Some(next),
                Some(next) => {
                    return Err(PreviewError::Failed {
                        detail: format!("OneDrive's list continued somewhere unexpected: {next}"),
                    });
                }
                None => None,
            };
        }
    }
    files.sort_by(|first, second| first.path.cmp(&second.path));
    Ok(files)
}

/// Whether `next`, the address of a list's next page, is on OneDrive's API and about the shared link: it may name the
/// share's owner first ("my.microsoftpersonalcontent.com/personal/<id>/_api/v2.0/shares('u!…')/…").
fn is_own_page(next: &str, share: &str) -> bool {
    let Ok(url) = Url::parse(next) else {
        return false;
    };
    let api = Url::parse(API).ok();
    url.scheme() == "https"
        && url.host_str() == api.as_ref().and_then(Url::host_str)
        && url.path().contains("/_api/v2.0/")
        && url.path().contains(share)
}

/// The shared item itself, or an item inside a shared folder.
fn item_url(share: &str, item: Option<&str>) -> String {
    match item {
        None => format!("{API}/shares/{share}/driveitem"),
        Some(item) => format!("{API}/shares/{share}/items/{}", path_part(item)),
    }
}

/// What's directly in the shared folder, or in one of its subfolders.
fn children_url(share: &str, folder: Option<&str>) -> String {
    format!("{}/children", item_url(share, folder))
}

/// An ID as part of an address: its "!" stays, as OneDrive writes it.
fn path_part(id: &str) -> String {
    id.bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'!' | b'-' | b'_' | b'.') {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

#[derive(Debug, Clone, Deserialize)]
struct Item {
    id: String,
    name: String,
    /// A folder's is everything in it.
    #[serde(default)]
    size: u64,
    #[serde(rename = "lastModifiedDateTime", default)]
    modified: Option<String>,
    #[serde(default)]
    file: Option<FileFacet>,
    /// Only whether there is one matters.
    #[serde(default)]
    folder: Option<serde_json::Value>,
    #[serde(rename = "@content.downloadUrl", default)]
    download_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct FileFacet {
    #[serde(default)]
    hashes: Option<Hashes>,
}

#[derive(Debug, Clone, Deserialize)]
struct Hashes {
    #[serde(rename = "sha1Hash", default)]
    sha1: Option<String>,
}

/// One page of a folder's contents.
#[derive(Debug, Deserialize)]
struct Children {
    #[serde(default)]
    value: Vec<Item>,
    #[serde(rename = "@odata.nextLink", default)]
    next: Option<String>,
}

#[derive(Deserialize)]
struct ErrorAnswer {
    error: ErrorBody,
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    code: String,
    #[serde(default)]
    message: String,
}

#[derive(Deserialize)]
struct TokenAnswer {
    token: String,
}

/// Asks the API for `url` with the anonymous token, asking for a new token once if it's refused.
fn get<T: DeserializeOwned>(
    client: &Client,
    url: &str,
    cancel: &CancelToken,
) -> Result<T, PreviewError> {
    let mut fresh = false;
    loop {
        let token = token(client, cancel, fresh)?;
        check_cancel(cancel)?;
        let response = client
            .get(url)
            .header(AUTHORIZATION, format!("Badger {token}"))
            .header("Prefer", "autoredeem")
            .send()
            .map_err(network_error)?;
        let status = response.status();
        if status.is_success() {
            return response.json().map_err(|error| PreviewError::Failed {
                detail: format!("OneDrive's answer wasn't as expected: {error}"),
            });
        }
        if status == StatusCode::UNAUTHORIZED && !fresh {
            forget_token();
            fresh = true;
            continue;
        }
        let page = Page::read(response);
        return Err(api_error(page.status, &page.html));
    }
}

/// The anonymous token: the one in hand while it's recent, else a new one.
fn token(client: &Client, cancel: &CancelToken, fresh: bool) -> Result<String, PreviewError> {
    if !fresh
        && let Ok(held) = TOKEN.lock()
        && let Some((token, since)) = held.as_ref()
        && since.elapsed() < TOKEN_LIFETIME
    {
        return Ok(token.clone());
    }
    check_cancel(cancel)?;
    let response = client
        .post(TOKEN_URL)
        .json(&json!({ "appId": APP_ID }))
        .send()
        .map_err(network_error)?;
    if let Some(error) = code_error(Host::OneDrive, response.status()) {
        return Err(error);
    }
    let answer: TokenAnswer = response.json().map_err(|error| PreviewError::Failed {
        detail: format!("OneDrive's token for public links wasn't as expected: {error}"),
    })?;
    if let Ok(mut held) = TOKEN.lock() {
        *held = Some((answer.token.clone(), Instant::now()));
    }
    Ok(answer.token)
}

fn forget_token() {
    if let Ok(mut held) = TOKEN.lock() {
        *held = None;
    }
}

/// What an answer that isn't a success means. OneDrive says why in JSON: `{"error": {"code": "itemNotFound", ...}}`.
fn api_error(status: StatusCode, body: &str) -> PreviewError {
    let (code, message) = serde_json::from_str::<ErrorAnswer>(body)
        .map(|answer| (answer.error.code, answer.error.message))
        .unwrap_or_default();
    let detail = match (code.is_empty(), message.is_empty()) {
        (true, _) => format!("OneDrive answered {status}"),
        (false, true) => format!("OneDrive answered {status}: {code}"),
        (false, false) => format!("OneDrive answered {status}: {code} ({message})"),
    };
    match (status.as_u16(), code.as_str()) {
        (_, "accessDenied") if message.contains("Password is required") => {
            PreviewError::PasswordProtected {
                site: Host::OneDrive.name().to_owned(),
            }
        }
        (_, "itemNotFound") | (404 | 410, _) => PreviewError::Unavailable {
            detail: format!("{detail}: deleted, or no longer shared"),
        },
        (_, "accessDenied" | "unauthenticated" | "forbidden") | (401 | 403, _) => {
            PreviewError::Unavailable {
                detail: format!("{detail}: the link is private, expired or turned off"),
            }
        }
        (_, "activityLimitReached" | "quotaLimitReached") | (429 | 509, _) => {
            PreviewError::LimitReached {
                site: Host::OneDrive.name().to_owned(),
                detail,
            }
        }
        (_, "invalidRequest" | "badArgument") | (400, _) => Host::OneDrive.link_not_recognized(),
        (500..=599, _) => PreviewError::Network { detail },
        _ => PreviewError::Failed { detail },
    }
}

/// The date of a time such as "2019-05-06T08:37:28Z", as YYYY-MM-DD.
fn date_part(time: &str) -> Option<String> {
    let date = time.get(..10)?;
    let bytes = date.as_bytes();
    let is_date = bytes.iter().enumerate().all(|(index, byte)| match index {
        4 | 7 => *byte == b'-',
        _ => byte.is_ascii_digit(),
    });
    is_date.then(|| date.to_owned())
}

/// A shared link as the API names it: "u!" and the link in unpadded URL-safe base64.
fn share_id(link: &str) -> String {
    format!("u!{}", URL_SAFE_NO_PAD.encode(link))
}

/// Reads a OneDrive link into the shared link the API is asked about:
///
/// - a short link, "1drv.ms/u/s!<id>", "1drv.ms/f/s!<id>" (and "/b/", "/w/"…), or since 2024
///   "1drv.ms/f/c/<cid>/<id>" and "1drv.ms/u/c/<cid>/<id>", as it is;
/// - the address a short link leads to ("onedrive.live.com/:u:/g/personal/<cid>/<id>?…&redeem=<short link in
///   base64>"), as the short link it names;
/// - "onedrive.live.com/?cid=<cid>&id=<item>&authkey=<key>", "onedrive.live.com/redir?resid=<item>&authkey=<key>"
///   and its "/embed" and "/download" forms, as the redir link.
fn parse_link(link: &str) -> Option<String> {
    let mut url = Url::parse(link).ok()?;
    url.set_fragment(None);
    let host = sites::host(link)?;
    let segments: Vec<String> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .map(str::to_owned)
        .collect();
    if SHORT_DOMAINS
        .iter()
        .any(|domain| sites::on_domain(&host, domain))
    {
        // A kind of item ("u", "f"…) and its ID at least.
        return (segments.len() >= 2 && segments[0].len() <= 2).then(|| share_id(url.as_str()));
    }
    if !Host::OneDrive.owns(&host) {
        return None;
    }
    if let Some(redeem) = query(&url, "redeem") {
        let redeem = redeem.trim_end_matches('=');
        let is_base64 = !redeem.is_empty()
            && redeem.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            });
        return is_base64.then(|| format!("u!{redeem}"));
    }
    let item = query(&url, "resid").or_else(|| query(&url, "id"));
    let path_is_known = segments.is_empty()
        || matches!(
            segments.first().map(String::as_str),
            Some("redir" | "embed" | "download" | "redir.aspx" | "embed.aspx" | "download.aspx")
        );
    if let Some(item) = item.filter(|item| item.contains('!'))
        && path_is_known
    {
        let mut redir = Url::parse("https://onedrive.live.com/redir").ok()?;
        redir.query_pairs_mut().append_pair("resid", &item);
        if let Some(authkey) = query(&url, "authkey") {
            redir.query_pairs_mut().append_pair("authkey", &authkey);
        }
        return Some(share_id(redir.as_str()));
    }
    // A sharing link on onedrive.live.com itself ("/:u:/g/personal/<cid>/<id>"), shared without a short link.
    let is_sharing_link = segments.len() >= 4
        && segments[0].starts_with(':')
        && segments[0].ends_with(':')
        && segments[1] == "g"
        && segments[2] == "personal";
    is_sharing_link.then(|| share_id(url.as_str()))
}

fn query(url: &Url, name: &str) -> Option<String> {
    url.query_pairs()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.into_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHORT_FILE: &str = "https://1drv.ms/u/s!AiV6XqkxJHE2g1zyXt4mCKNbpdiw?e=ZJAhIl";

    #[test]
    fn names_a_shared_link_as_the_api_does() {
        assert_eq!(
            share_id(SHORT_FILE),
            "u!aHR0cHM6Ly8xZHJ2Lm1zL3UvcyFBaVY2WHFreEpIRTJnMXp5WHQ0bUNLTmJwZGl3P2U9WkpBaEls"
        );
    }

    #[test]
    fn reads_short_and_long_links() {
        let redir = |query: &str| share_id(&format!("https://onedrive.live.com/redir?{query}"));
        for (link, expected) in [
            (SHORT_FILE, Some(share_id(SHORT_FILE))),
            (
                "https://1drv.ms/f/s!AhIXJn_J-blW00SqrairNetmeVu4#top",
                Some(share_id("https://1drv.ms/f/s!AhIXJn_J-blW00SqrairNetmeVu4")),
            ),
            (
                "https://1drv.ms/f/c/2d90e71fb9eb254f/EnMm8c2mP_FPv6lwt1jy01YB6bQhoPQ25vtzAhycYisERw?e=DiI2Ab",
                Some(share_id(
                    "https://1drv.ms/f/c/2d90e71fb9eb254f/EnMm8c2mP_FPv6lwt1jy01YB6bQhoPQ25vtzAhycYisERw?e=DiI2Ab",
                )),
            ),
            (
                "https://1drv.ms/u/c/c0bd25b6b048b01d/EbLgD7bES4FDvUN37Grxn8QBF5gIBBc7YV2qklF08GCiBw",
                Some(share_id(
                    "https://1drv.ms/u/c/c0bd25b6b048b01d/EbLgD7bES4FDvUN37Grxn8QBF5gIBBc7YV2qklF08GCiBw",
                )),
            ),
            // Where that short file link leads: it names the short link again.
            (
                "https://onedrive.live.com/:u:/g/personal/36712431A95E7A25/s!AiV6XqkxJHE2g1zyXt4mCKNbpdiw?resid=36712431A95E7A25!476&e=ZJAhIl&migratedtospo=true&redeem=aHR0cHM6Ly8xZHJ2Lm1zL3UvcyFBaVY2WHFreEpIRTJnMXp5WHQ0bUNLTmJwZGl3P2U9WkpBaEls",
                Some(share_id(SHORT_FILE)),
            ),
            (
                "https://onedrive.live.com/?cid=36712431A95E7A25&id=36712431A95E7A25%21476&authkey=%21AFyXt4mCKNbpdiw",
                Some(redir(
                    "resid=36712431A95E7A25%21476&authkey=%21AFyXt4mCKNbpdiw",
                )),
            ),
            (
                "https://onedrive.live.com/redir?resid=36712431A95E7A25!476&authkey=!AFyXt4mCKNbpdiw&ithint=file%2czip",
                Some(redir(
                    "resid=36712431A95E7A25%21476&authkey=%21AFyXt4mCKNbpdiw",
                )),
            ),
            (
                "https://onedrive.live.com/embed?cid=36712431A95E7A25&resid=36712431A95E7A25%21476&authkey=!AFyXt4mCKNbpdiw",
                Some(redir(
                    "resid=36712431A95E7A25%21476&authkey=%21AFyXt4mCKNbpdiw",
                )),
            ),
            (
                "https://onedrive.live.com/download?resid=36712431A95E7A25!476",
                Some(redir("resid=36712431A95E7A25%21476")),
            ),
            (
                "https://onedrive.live.com/:f:/g/personal/56B9F9C97F261712/EhIXJn_J-blW00Sq",
                Some(share_id(
                    "https://onedrive.live.com/:f:/g/personal/56B9F9C97F261712/EhIXJn_J-blW00Sq",
                )),
            ),
            ("https://onedrive.live.com/about/en-us/", None),
            ("https://onedrive.live.com/?id=root", None),
            ("https://onedrive.live.com/?redeem=not%20base64", None),
            ("https://1drv.ms/", None),
            ("https://1drv.ms/download", None),
            ("https://not1drv.ms/u/s!AiV6XqkxJHE2g1zy", None),
        ] {
            assert_eq!(parse_link(link), expected, "{link}");
        }
    }

    #[test]
    fn asks_about_the_share_and_what_it_holds() {
        let share = share_id(SHORT_FILE);
        assert_eq!(
            item_url(&share, None),
            format!("{API}/shares/{share}/driveitem")
        );
        assert_eq!(
            children_url(&share, Some("56B9F9C97F261712!10693")),
            format!("{API}/shares/{share}/items/56B9F9C97F261712!10693/children")
        );
        assert_eq!(path_part("a b/c!d"), "a%20b%2Fc!d");
    }

    /// The shared file's answer, trimmed.
    const FILE_ANSWER: &str = r#"{
        "@content.downloadUrl": "https://my.microsoftpersonalcontent.com/personal/36712431a95e7a25/_layouts/15/download.aspx?UniqueId=a95e7a25-2431-2071-8036-dc0100000000&Translate=false&tempauth=v1e.abc&ApiVersion=2.0",
        "createdDateTime": "2019-05-06T12:14:47Z",
        "id": "36712431A95E7A25!476",
        "lastModifiedDateTime": "2019-05-06T08:37:28Z",
        "name": "kitti_256.zip",
        "size": 4223583373,
        "file": {"fileExtension": ".zip", "hashes": {"quickXorHash": "wcj1c6TB4IOfLkpa8ZfgzZPjnrM=", "sha1Hash": "EC9ACBB908802722F55970DBE94FB8482EE30C45"}}
    }"#;

    #[test]
    fn reads_a_shared_file() {
        let item: Item = serde_json::from_str(FILE_ANSWER).unwrap();
        assert_eq!(item.name, "kitti_256.zip");
        assert_eq!(item.size, 4_223_583_373);
        assert!(item.file.is_some() && item.folder.is_none());
        assert_eq!(
            item.modified.as_deref().and_then(date_part).as_deref(),
            Some("2019-05-06")
        );
        assert_eq!(
            item.file
                .and_then(|file| file.hashes)
                .and_then(|hashes| hashes.sha1)
                .as_deref(),
            Some("EC9ACBB908802722F55970DBE94FB8482EE30C45")
        );
        assert!(item.download_url.unwrap().contains("download.aspx"));
        assert_eq!(date_part("yesterday"), None);
    }

    fn folder(id: &str, name: &str) -> String {
        format!(r#"{{"id": "{id}", "name": "{name}", "size": 5, "folder": {{"childCount": 1}}}}"#)
    }

    fn file(id: &str, name: &str, size: u64) -> String {
        format!(
            r#"{{"id": "{id}", "name": "{name}", "size": {size}, "lastModifiedDateTime": "2024-06-07T06:48:10Z", "file": {{"hashes": {{"sha1Hash": "00"}}}}, "@content.downloadUrl": "https://example.com/{id}"}}"#
        )
    }

    fn page(items: &[String], next: Option<&str>) -> Children {
        let next = next.map_or("null".to_owned(), |next| format!("\"{next}\""));
        serde_json::from_str(&format!(
            r#"{{"value": [{}], "@odata.nextLink": {next}}}"#,
            items.join(",")
        ))
        .unwrap()
    }

    #[test]
    fn reads_a_folder_with_its_subfolders() {
        let share = "u!abc";
        let root: Item = serde_json::from_str(&folder("ROOT!1", "Event: Texture")).unwrap();
        let second_page = format!("{API}/shares/{share}/driveitem/children?$skiptoken=2");
        let mut asked = Vec::new();
        let files = folder_files(share, &root, &mut |url| {
            asked.push(url.to_owned());
            Ok(if url == children_url(share, None) {
                page(
                    &[
                        file("ROOT!2", "pretrained_models.zip", 132_626_825),
                        folder("ROOT!3", "coco"),
                        // A notebook, which has no file.
                        r#"{"id": "ROOT!9", "name": "Notes", "package": {"type": "oneNote"}}"#
                            .to_owned(),
                    ],
                    Some(&second_page),
                )
            } else if url == second_page {
                page(&[file("ROOT!4", "test_sets.zip", 1_984_938_887)], None)
            } else if url == children_url(share, Some("ROOT!3")) {
                // A subfolder that names the shared folder again isn't listed twice.
                page(
                    &[file("ROOT!5", "a.json", 7), folder("ROOT!1", "loop")],
                    None,
                )
            } else {
                panic!("asked for {url}")
            })
        })
        .unwrap();
        assert_eq!(asked.len(), 3);
        let listed: Vec<(PathBuf, u64)> = files
            .iter()
            .map(|file| (file.path.clone(), file.size))
            .collect();
        let top = PathBuf::from(safe_name("Event: Texture"));
        assert_eq!(
            listed,
            [
                (top.join("coco").join("a.json"), 7),
                (top.join("pretrained_models.zip"), 132_626_825),
                (top.join("test_sets.zip"), 1_984_938_887),
            ]
        );
        assert_eq!(
            files[0].source,
            FileSource::OneDrive(OneDriveFile {
                share: share.to_owned(),
                item: Some("ROOT!5".to_owned()),
            })
        );
    }

    #[test]
    fn follows_pages_only_on_its_own_api() {
        let root: Item = serde_json::from_str(&folder("ROOT!1", "Pack")).unwrap();
        let error = folder_files("u!abc", &root, &mut |_| {
            Ok(page(&[], Some("https://example.com/steal-the-token")))
        })
        .unwrap_err();
        assert!(matches!(error, PreviewError::Failed { .. }), "{error:?}");

        // A big folder's next page names the share's owner first, as OneDrive gave it for a folder of 760 images.
        let share = "u!aHR0cHM6Ly8xZHJ2Lm1zL2YvcyFBaXR0bkdtNnZSS0x3aUw1NWY3ZjB4ZHB1RDlf";
        let owners_page = format!(
            "https://my.microsoftpersonalcontent.com/personal/8b12bdba699c6d2b/_api/v2.0/shares(%27{share}%27)/driveitem/children?%24skiptoken=UGFnZWQ9VFJVRQ"
        );
        assert!(is_own_page(&owners_page, share));
        assert!(is_own_page(
            &format!("{API}/shares/{share}/driveitem/children?$skiptoken=2"),
            share
        ));
        assert!(
            !is_own_page(&owners_page, "u!another"),
            "another share's list"
        );
        assert!(!is_own_page(
            &owners_page.replace("my.microsoftpersonalcontent.com", "example.com"),
            share
        ));
        assert!(!is_own_page(&owners_page.replace("https:", "http:"), share));

        let mut asked = Vec::new();
        let files = folder_files(share, &root, &mut |url| {
            asked.push(url.to_owned());
            Ok(if asked.len() == 1 {
                page(&[file("A!1", "1.bmp", 1)], Some(&owners_page))
            } else {
                page(&[file("A!2", "2.bmp", 2)], None)
            })
        })
        .unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(asked[1], owners_page);
    }

    #[test]
    fn finds_a_folders_file_in_the_folders_list() {
        let share = "u!listed";
        let asked = std::cell::Cell::new(0);
        let mut list = |url: &str| {
            asked.set(asked.get() + 1);
            assert!(url.starts_with(&children_url(share, None)), "{url}");
            Ok(if url == children_url(share, None) {
                page(
                    &[folder("F!9", "Sub"), file("F!1", "a.zip", 1)],
                    Some(&format!(
                        "{API}/shares/{share}/driveitem/children?$skiptoken=2"
                    )),
                )
            } else {
                page(&[file("F!2", "b.zip", 2)], None)
            })
        };
        let second = listed_file(share, "F!2", &mut list).unwrap();
        assert_eq!(
            second.download_url.as_deref(),
            Some("https://example.com/F!2")
        );
        let first = listed_file(share, "F!1", &mut list).unwrap();
        assert_eq!(first.name, "a.zip");
        assert_eq!(
            asked.get(),
            2,
            "the second file comes from the same listing"
        );
        assert!(matches!(
            listed_file(share, "F!404", &mut list),
            Err(PreviewError::Unavailable { .. })
        ));
    }

    #[test]
    fn says_so_when_it_cant_read_a_subfolder() {
        let root: Item = serde_json::from_str(&folder("ROOT!1", "Pack")).unwrap();
        let error = folder_files("u!abc", &root, &mut |url| {
            if url == children_url("u!abc", None) {
                Ok(page(
                    &[
                        folder("ROOT!2", "coco"),
                        r#"{"id": "ROOT!3", "name": "OneNote_RecycleBin", "folder": {"childCount": 0}}"#
                            .to_owned(),
                    ],
                    None,
                ))
            } else if url == children_url("u!abc", Some("ROOT!3")) {
                panic!("an empty subfolder isn't listed")
            } else {
                Err(api_error(
                    StatusCode::NOT_FOUND,
                    r#"{"error": {"code": "itemNotFound", "message": "Item not found"}}"#,
                ))
            }
        })
        .unwrap_err();
        assert_eq!(
            error,
            PreviewError::NestedFolders {
                site: "OneDrive".to_owned()
            }
        );
    }

    #[test]
    fn explains_its_errors() {
        let answer = |code: &str| {
            format!(r#"{{"error": {{"code": "{code}", "message": "Item does not exist"}}}}"#)
        };
        let error =
            |status: u16, body: &str| api_error(StatusCode::from_u16(status).unwrap(), body);
        assert!(matches!(
            error(404, &answer("itemNotFound")),
            PreviewError::Unavailable { detail } if detail.contains("itemNotFound (Item does not exist)")
        ));
        assert!(matches!(
            error(403, &answer("accessDenied")),
            PreviewError::Unavailable { detail } if detail.contains("private")
        ));
        assert!(matches!(error(401, "{}"), PreviewError::Unavailable { .. }));
        assert_eq!(
            error(
                403,
                r#"{"error": {"code": "accessDenied", "message": "Password is required to access this share."}}"#
            ),
            PreviewError::PasswordProtected {
                site: "OneDrive".to_owned()
            }
        );
        assert!(matches!(
            error(429, &answer("activityLimitReached")),
            PreviewError::LimitReached { site, .. } if site == "OneDrive"
        ));
        assert_eq!(
            error(400, &answer("invalidRequest")),
            PreviewError::LinkNotRecognized {
                site: "OneDrive".to_owned()
            }
        );
        assert!(matches!(error(503, "<html>"), PreviewError::Network { .. }));
        assert!(matches!(error(418, ""), PreviewError::Failed { .. }));
    }
}
