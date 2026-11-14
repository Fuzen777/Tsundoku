//! Channels, playlists and accounts: links that hold many videos or posts, which the preview card lists for the user to
//! pick from. `listing_link` tells one from its address and gives the page to read (a YouTube channel's Videos tab, an
//! Instagram account's posts, an X account's Media tab); `read` lists what it holds without reading each item, newest
//! first for a channel or an account, in its own order for a playlist. `mark` says which items the history or the queue
//! has. Another site's playlist is listed when yt-dlp answers one with a link and ID for each item (`from_yt_dlp`);
//! otherwise, and for image sites' accounts other than Instagram's and X's, the link is refused as
//! `PreviewError::NotListable`.
//!
//! Tested against real answers saved under `tests/fixtures/listing`.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::cancel::CancelToken;
use crate::gallery::{self, GalleryTools};
use crate::history::PastDownload;
use crate::metadata::{self, FetchTools, Fetched, PreviewError, VideoInfo};
use crate::sites;

/// How many items a channel or an account lists: its newest. A playlist is listed whole.
pub const NEWEST_COUNT: u32 = 50;

/// The most older items a site pins above its newest: Instagram pins up to 3 posts, X one. A check of a listing
/// reaches what it had seen before only past this many known items in a row (`before_known_ground`), so pinned posts
/// it knows don't end it before the new ones under them.
pub const PINNED_AT_MOST: usize = 3;

/// A listing can take a while: a long playlist is read 100 videos per request, Instagram waits between requests.
const LISTING_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// The longest title made from a listing's name.
const TITLE_LENGTH: usize = 200;

/// YouTube's first path segments that start a channel's address: `/@name`, `/channel/UC…`, `/c/name`, `/user/name`.
const YOUTUBE_CHANNEL_PREFIXES: [&str; 3] = ["channel", "c", "user"];

/// Instagram's first path segments that aren't an account's name.
const INSTAGRAM_RESERVED: [&str; 17] = [
    "p",
    "reel",
    "reels",
    "tv",
    "stories",
    "explore",
    "accounts",
    "direct",
    "about",
    "legal",
    "developer",
    "web",
    "api",
    "graphql",
    "challenge",
    "s",
    "share",
];

/// X's first path segments that aren't an account's name, lowercase. Its handles are 1 to 15 letters, digits or "_",
/// so pages with a "-" or a "." in their name never look like one.
const X_RESERVED: [&str; 26] = [
    "home",
    "explore",
    "notifications",
    "messages",
    "i",
    "settings",
    "search",
    "compose",
    "login",
    "logout",
    "signup",
    "tos",
    "privacy",
    "hashtag",
    "intent",
    "share",
    "account",
    "jobs",
    "about",
    "download",
    "lists",
    "bookmarks",
    "communities",
    "premium",
    "rules",
    "help",
];

/// X's account tabs whose link lists the account: its posts, its replies, its highlights and its media. Its likes and
/// who it follows aren't its own posts.
const X_ACCOUNT_TABS: [&str; 3] = ["media", "with_replies", "highlights"];

/// The longest X handle.
const X_HANDLE_LENGTH: usize = 15;

/// What a listing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ListingKind {
    /// A YouTube channel's Videos tab.
    Channel,
    /// A playlist, in its own order, on YouTube or another site.
    Playlist,
    /// An Instagram account's posts and reels, as on its grid, or an X account's posts with pictures or videos, as on
    /// its Media tab.
    Account,
}

/// A link to a listing, told from its address: what it is, and the page to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingLink {
    pub kind: ListingKind,
    /// The page that lists the items: a channel's Videos tab, an account's posts.
    pub link: String,
}

impl ListingLink {
    /// Whether gallery-dl reads it (an account of images), rather than yt-dlp.
    fn reads_images(&self) -> bool {
        self.kind == ListingKind::Account
    }
}

/// What a listing holds, and what it is.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Listing {
    pub kind: ListingKind,
    /// The channel's, playlist's or account's name.
    pub title: String,
    /// Who made a playlist. `None` for a channel or an account, whose title already says.
    pub uploader: Option<String>,
    /// The site as people know it ("youtube.com").
    pub site_name: String,
    /// The listing's own page.
    pub url: String,
    /// The channel's or account's picture, or a playlist's first thumbnail.
    pub thumbnail_url: Option<String>,
    /// How many it holds in all, when the site says, which is more than `items` when only the newest were listed.
    pub total: Option<u32>,
    /// Only the newest `NEWEST_COUNT` were asked for (a channel or an account); a playlist is listed whole.
    pub newest_only: bool,
    pub items: Vec<ListedItem>,
}

/// One video or post of a listing, as the listing describes it: enough to show it and to download it, without reading
/// it on its own.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListedItem {
    /// What a download of it starts with, as if its own link had been read. Its `upload_date` is set only when the site
    /// gives the exact day.
    pub video: VideoInfo,
    /// The day it was posted, as YYYY-MM-DD, when the site only says roughly ("3 weeks ago" on YouTube).
    pub approximate_date: Option<String>,
    /// The latest download of it, when the history has it (`mark`).
    pub last_download: Option<PastDownload>,
    /// It's in the download queue and not done yet (`mark`).
    pub in_queue: bool,
}

/// What a link turned out to be, when it may be either: one video or post, or a listing.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    One(Fetched),
    Listing(Listing),
}

/// The listing `link` is, from its address alone, with the page to read, or `None` for anything else, a single video
/// in a playlist (`watch?v=…&list=…`) included. `link` must come from `metadata::normalize_link`.
pub fn listing_link(link: &str) -> Option<ListingLink> {
    let host = sites::host(link)?;
    let (path, query) = path_and_query(link);
    let segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();

    if sites::on_domain(&host, "youtube.com") && !host.starts_with("music.") {
        if segments.first() == Some(&"playlist") {
            let list = query_value(query, "list").filter(|list| is_youtube_id(list))?;
            return Some(ListingLink {
                kind: ListingKind::Playlist,
                link: format!("https://www.youtube.com/playlist?list={list}"),
            });
        }
        let channel = match segments.as_slice() {
            [name, ..] if name.len() > 1 && name.starts_with('@') => *name,
            [prefix, name, ..] if YOUTUBE_CHANNEL_PREFIXES.contains(prefix) => {
                return is_youtube_id(name).then(|| ListingLink {
                    kind: ListingKind::Channel,
                    link: format!("https://www.youtube.com/{prefix}/{name}/videos"),
                });
            }
            _ => return None,
        };
        // Whatever tab the link opens (Shorts, Live, Playlists…), the Videos tab is listed.
        return is_youtube_id(&channel[1..]).then(|| ListingLink {
            kind: ListingKind::Channel,
            link: format!("https://www.youtube.com/{channel}/videos"),
        });
    }

    if sites::on_domain(&host, "instagram.com") {
        let account = match segments.as_slice() {
            [name] | [name, "posts" | "reels"] => *name,
            _ => return None,
        };
        let is_account = !INSTAGRAM_RESERVED.contains(&account.to_ascii_lowercase().as_str())
            && account
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "._".contains(character));
        return is_account.then(|| ListingLink {
            kind: ListingKind::Account,
            link: format!("https://www.instagram.com/{account}/posts/"),
        });
    }

    if on_x(&host) {
        let account = match segments.as_slice() {
            [name] => *name,
            [name, tab] if X_ACCOUNT_TABS.contains(tab) => *name,
            _ => return None,
        };
        // Whatever tab the link opens, the Media tab is listed: the account's own posts that have something to
        // download, without its text-only posts or what it reposted.
        return is_x_handle(account).then(|| ListingLink {
            kind: ListingKind::Account,
            link: format!("https://x.com/{account}/media"),
        });
    }
    None
}

/// Whether a host is X's, under either of its domains.
fn on_x(host: &str) -> bool {
    sites::on_domain(host, "x.com") || sites::on_domain(host, "twitter.com")
}

/// An account's own page, on the site of its listing's page (`link`).
fn account_page(link: &str, name: &str) -> String {
    if sites::host(link).is_some_and(|host| on_x(&host)) {
        format!("https://x.com/{name}")
    } else {
        format!("https://www.instagram.com/{name}/")
    }
}

/// Whether a link's first path segment is an X handle rather than one of X's own pages.
fn is_x_handle(name: &str) -> bool {
    (1..=X_HANDLE_LENGTH).contains(&name.len())
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        && !X_RESERVED.contains(&name.to_ascii_lowercase().as_str())
}

/// The listing kind of `text`'s link, for the preview card to say what it's reading meanwhile.
pub fn kind_of(text: &str) -> Option<ListingKind> {
    let link = metadata::normalize_link(text)?;
    listing_link(&link).map(|listing| listing.kind)
}

/// Lists what a listing link holds, with the tool that reads it: yt-dlp for a channel or a playlist, gallery-dl for an
/// account, which asks for its newest posts only (`--post-range`).
pub fn read(
    videos: &FetchTools,
    images: &GalleryTools,
    listing: &ListingLink,
    cancel: &CancelToken,
) -> Result<Listing, PreviewError> {
    if listing.reads_images() {
        let range = format!("1-{NEWEST_COUNT}");
        let read = gallery::run(
            images,
            &listing.link,
            &["--post-range", &range],
            LISTING_TIMEOUT,
            cancel,
        )
        .and_then(|json| from_gallery_dl(&json, &listing.link));
        return read.map_err(|error| signed_out_failure(error, images.cookies.is_some()));
    }
    let newest = NEWEST_COUNT.to_string();
    let mut extra = vec![
        // YouTube's listings say "3 weeks ago" rather than a date: yt-dlp turns it into a rough one.
        "--extractor-args",
        "youtubetab:approximate_date",
    ];
    if listing.kind == ListingKind::Channel {
        extra.extend(["--playlist-end", newest.as_str()]);
    }
    let answer = metadata::run(videos, &listing.link, &extra, LISTING_TIMEOUT, cancel)?;
    from_yt_dlp(&answer.json, Some(listing.kind), &listing.link)
}

/// Reads a link that isn't known to be a listing with yt-dlp: a video, or a listing if yt-dlp answers with one (another
/// site's playlist).
pub fn read_any(
    tools: &FetchTools,
    link: &str,
    cancel: &CancelToken,
) -> Result<Found, PreviewError> {
    let answer = metadata::run(tools, link, &[], metadata::FETCH_TIMEOUT, cancel)?;
    if metadata::is_listing(&answer.json) {
        return from_yt_dlp(&answer.json, None, link).map(Found::Listing);
    }
    Ok(Found::One(Fetched {
        video: metadata::parse_info(&answer.json)?,
        missing_js_runtime: answer.missing_js_runtime,
    }))
}

/// Marks each item that the history has (`downloaded`, by site and ID as the duplicate check does) or that the queue
/// has and isn't done with (`queued`).
pub fn mark<E>(
    listing: &mut Listing,
    downloaded: impl Fn(&str, &str) -> Result<Option<PastDownload>, E>,
    queued: impl Fn(&str, &str) -> bool,
) -> Result<(), E> {
    for item in &mut listing.items {
        let video = &item.video;
        item.last_download = downloaded(&video.site, &video.video_id)?;
        item.in_queue = queued(&video.site, &video.video_id);
    }
    Ok(())
}

/// What a check of a listing found before it reached known ground (`before_known_ground`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewItems<T> {
    /// The new items met, in the listing's order.
    pub new: Vec<T>,
    /// The check stopped at known ground. When it didn't, the listing may have ended before what was seen last, so some
    /// new items may be missing.
    pub reached_known: bool,
}

/// The new items of a listing whose newest come first, up to where it reaches what was seen before: more than
/// `PINNED_AT_MOST` known items in a row, rather than the first known one, since that many older items may be pinned
/// above the newest. A known item among new ones is skipped. The check stops there, without looking at what follows.
pub fn before_known_ground<T>(
    items: impl IntoIterator<Item = T>,
    is_known: impl Fn(&T) -> bool,
) -> NewItems<T> {
    let mut new = Vec::new();
    let mut known_in_a_row = 0;
    for item in items {
        if is_known(&item) {
            known_in_a_row += 1;
            if known_in_a_row > PINNED_AT_MOST {
                return NewItems {
                    new,
                    reached_known: true,
                };
            }
        } else {
            known_in_a_row = 0;
            new.push(item);
        }
    }
    NewItems {
        new,
        reached_known: false,
    }
}

/// An Instagram account can't be listed signed out: Instagram answers gallery-dl in ways it doesn't expect (a
/// `KeyError` about a query) rather than with its login page. Without cookies, such a failure asks for a sign-in, whose
/// fix is turning cookies on.
fn signed_out_failure(error: PreviewError, has_cookies: bool) -> PreviewError {
    match error {
        PreviewError::Network { detail }
        | PreviewError::SiteError { detail, .. }
        | PreviewError::Failed { detail }
        | PreviewError::Unavailable { detail }
            if !has_cookies =>
        {
            PreviewError::sign_in(detail)
        }
        other => other,
    }
}

/// The refusal for a page of many videos or posts that can't be listed, naming its site.
pub(crate) fn not_listable(link: &str) -> PreviewError {
    let site = sites::sign_in_site(link)
        .map(|site| site.name.to_owned())
        .or_else(|| site_name(link))
        .unwrap_or_default();
    PreviewError::NotListable { site }
}

/// Reads yt-dlp's answer for a playlist (`--flat-playlist --dump-single-json`): each entry with a link and an ID of its
/// own becomes an item, keyed as reading its link would key it. A listing with an entry that has neither, or with
/// listings inside it, is refused as `NotListable`, since its items couldn't be downloaded on their own. `kind` is
/// `None` when the link wasn't known to be a listing.
pub(crate) fn from_yt_dlp(
    json: &str,
    kind: Option<ListingKind>,
    link: &str,
) -> Result<Listing, PreviewError> {
    let answer: Value = serde_json::from_str(json).map_err(|error| PreviewError::Failed {
        detail: format!("yt-dlp's answer can't be read: {error}"),
    })?;
    let rough_dates = text(&answer, "extractor_key").as_deref() == Some("YoutubeTab");
    let owner = text(&answer, "channel").or_else(|| text(&answer, "uploader"));
    let owner_account = text(&answer, "uploader_id").and_then(|id| metadata::handle(&id));
    let entries = answer
        .get("entries")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();

    let mut items: Vec<ListedItem> = Vec::with_capacity(entries.len());
    for entry in entries {
        // A premiere that hasn't started, or a live stream, can't be downloaded as a video yet.
        if matches!(
            text(entry, "live_status").as_deref(),
            Some("is_upcoming" | "is_live" | "post_live")
        ) {
            continue;
        }
        let Some(mut item) = item_from_entry(entry, rough_dates) else {
            return Err(not_listable(link));
        };
        if kind == Some(ListingKind::Channel) {
            if item.video.uploader.is_none() {
                item.video.uploader.clone_from(&owner);
            }
            if item.video.account.is_none() {
                item.video.account.clone_from(&owner_account);
            }
        }
        if let Some(other) = items.iter().find(|other| other.video.url == item.video.url) {
            if other.video.site == item.video.site && other.video.video_id == item.video.video_id {
                // A playlist may hold a video twice: it's listed once.
                continue;
            }
            // Two items at one address: a post with several videos, which one link downloads whole.
            return Err(not_listable(link));
        }
        items.push(item);
    }

    let kind = kind.unwrap_or(ListingKind::Playlist);
    let url = text(&answer, "webpage_url").unwrap_or_else(|| link.to_owned());
    let title = shorten(
        &text(&answer, "title")
            .map(|title| channel_title(&title, owner.as_deref(), kind))
            .or_else(|| owner.clone())
            .unwrap_or_else(|| url.clone()),
    );
    name_items(&mut items, &title);
    Ok(Listing {
        kind,
        title,
        uploader: (kind == ListingKind::Playlist).then_some(owner).flatten(),
        site_name: text(&answer, "webpage_url_domain")
            .or_else(|| site_name(&url))
            .unwrap_or_default(),
        thumbnail_url: picture(&answer, kind).or_else(|| {
            items
                .first()
                .and_then(|item| item.video.thumbnail_url.clone())
        }),
        total: count_field(&answer, "playlist_count")
            .filter(|total| *total as usize >= items.len()),
        newest_only: kind == ListingKind::Channel,
        url,
        items,
    })
}

/// An entry of yt-dlp's flat listing as an item, or `None` when it has no link, ID or site of its own, or is a listing
/// itself.
fn item_from_entry(entry: &Value, rough_dates: bool) -> Option<ListedItem> {
    let kind = text(entry, "_type");
    if matches!(kind.as_deref(), Some("playlist" | "multi_video")) {
        return None;
    }
    // A flat entry is a link to read later ("url"), whose `url` is its page; a full one has its page in `webpage_url`
    // and a file in `url`.
    let page = text(entry, "webpage_url").or_else(|| {
        matches!(kind.as_deref(), Some("url" | "url_transparent"))
            .then(|| text(entry, "url"))
            .flatten()
    })?;
    let url = metadata::normalize_link(&page)?;
    let site = text(entry, "ie_key").or_else(|| text(entry, "extractor_key"))?;
    if is_listing_extractor(&site) {
        return None;
    }
    let video_id = text(entry, "id")?;

    let exact_date = text(entry, "upload_date").and_then(|date| metadata::iso_date(&date));
    let timestamp_date = entry
        .get("timestamp")
        .and_then(Value::as_i64)
        .map(date_of_timestamp);
    let (upload_date, approximate_date) = match (exact_date, timestamp_date) {
        (Some(date), _) => (Some(date), None),
        (None, Some(date)) if rough_dates => (None, Some(date)),
        (None, date) => (date, None),
    };
    let duration = entry
        .get("duration")
        .and_then(Value::as_f64)
        .filter(|seconds| *seconds > 0.0);

    Some(ListedItem {
        video: VideoInfo {
            site: site.to_lowercase(),
            site_name: site_name(&url).unwrap_or_else(|| site.to_lowercase()),
            // Some sites list only links (SoundCloud): their last part names the item better than its number.
            title: text(entry, "title")
                .or_else(|| last_segment(&url))
                .unwrap_or_else(|| video_id.clone()),
            video_id,
            uploader: text(entry, "uploader").or_else(|| text(entry, "channel")),
            account: text(entry, "uploader_id").and_then(|id| metadata::handle(&id)),
            duration_seconds: duration,
            upload_date,
            thumbnail_url: text(entry, "thumbnail").or_else(|| entry_thumbnail(entry)),
            best_height: None,
            post: None,
            post_files: Vec::new(),
            files: None,
            url,
            // The listing's own name, once it's known (`name_items`).
            shared_files: Vec::new(),
            playlist: None,
        },
        approximate_date,
        last_download: None,
        in_queue: false,
    })
}

/// Whether yt-dlp's extractor for an entry reads listings rather than videos, such as `YoutubeTab`: its entry is a
/// listing inside the listing.
fn is_listing_extractor(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "tab", "playlist", "channel", "user", "album", "set", "series", "season",
    ]
    .iter()
    .any(|ending| key.ends_with(ending))
}

/// A channel's listing is titled "Name - Videos" by yt-dlp: the channel's name is enough.
fn channel_title(title: &str, owner: Option<&str>, kind: ListingKind) -> String {
    if kind == ListingKind::Channel
        && let Some(owner) = owner
        && title.starts_with(owner)
    {
        return owner.to_owned();
    }
    title.to_owned()
}

/// A listing's picture: a channel's square avatar, the smallest one yt-dlp gives, or a playlist's first thumbnail.
fn picture(answer: &Value, kind: ListingKind) -> Option<String> {
    let thumbnails = answer.get("thumbnails")?.as_array()?;
    if kind != ListingKind::Channel {
        return thumbnails
            .iter()
            .rev()
            .find_map(|thumbnail| text(thumbnail, "url"));
    }
    thumbnails
        .iter()
        .filter(|thumbnail| {
            let side = |field| thumbnail.get(field).and_then(Value::as_u64);
            side("width").is_some() && side("width") == side("height")
        })
        .min_by_key(|thumbnail| thumbnail.get("width").and_then(Value::as_u64))
        .and_then(|thumbnail| text(thumbnail, "url"))
}

/// The first of an entry's thumbnails wide enough for a list's row, else its largest.
fn entry_thumbnail(entry: &Value) -> Option<String> {
    let thumbnails = entry.get("thumbnails")?.as_array()?;
    thumbnails
        .iter()
        .find(|thumbnail| {
            thumbnail
                .get("width")
                .and_then(Value::as_u64)
                .is_some_and(|width| width >= 320)
        })
        .or_else(|| thumbnails.last())
        .and_then(|thumbnail| text(thumbnail, "url"))
}

/// Reads gallery-dl's answer for an account's posts: each post becomes an item, as reading its own link would make it,
/// newest first (pinned posts come first in the answer).
pub(crate) fn from_gallery_dl(json: &str, link: &str) -> Result<Listing, PreviewError> {
    let found = gallery::parse_posts(json, link)?;
    let mut items: Vec<ListedItem> = found
        .posts
        .into_iter()
        .map(|video| ListedItem {
            video,
            approximate_date: None,
            last_download: None,
            in_queue: false,
        })
        .collect();
    // Stable, so posts of one day keep the answer's order; a post without a date goes last.
    items.sort_by(|first, second| {
        let date = |item: &ListedItem| item.video.upload_date.clone();
        match (date(first), date(second)) {
            (Some(first), Some(second)) => second.cmp(&first),
            (first, second) => second.is_some().cmp(&first.is_some()),
        }
    });

    let account = found.account.unwrap_or_default();
    let name_in_link = sites::host(link).and_then(|_| {
        path_and_query(link)
            .0
            .split('/')
            .find(|part| !part.is_empty())
            .map(str::to_owned)
    });
    let title = account
        .full_name
        .clone()
        .or_else(|| account.username.clone())
        .or(name_in_link)
        .unwrap_or_else(|| link.to_owned());
    let title = shorten(&title);
    name_items(&mut items, &title);
    Ok(Listing {
        kind: ListingKind::Account,
        title,
        uploader: None,
        site_name: site_name(link).unwrap_or_default(),
        url: account
            .username
            .as_ref()
            .map_or_else(|| link.to_owned(), |name| account_page(link, name)),
        thumbnail_url: account.picture,
        total: account
            .post_count
            .filter(|total| *total as usize >= items.len()),
        newest_only: true,
        items,
    })
}

/// Gives every item the listing's name, which `{playlist}` puts in a rule's folder once it's picked.
fn name_items(items: &mut [ListedItem], title: &str) {
    for item in items {
        item.video.playlist = Some(title.to_owned());
    }
}

/// The path (without its leading "/") and the query of a link.
fn path_and_query(link: &str) -> (&str, &str) {
    let after_scheme = link.split_once("://").map_or(link, |(_, rest)| rest);
    let without_fragment = after_scheme.split('#').next().unwrap_or_default();
    let (address, query) = without_fragment
        .split_once('?')
        .unwrap_or((without_fragment, ""));
    let path = address.split_once('/').map_or("", |(_, path)| path);
    (path, query)
}

/// The last part of a link's path ("world-on-fire" for "https://soundcloud.com/band/world-on-fire").
fn last_segment(link: &str) -> Option<String> {
    path_and_query(link)
        .0
        .rsplit('/')
        .find(|part| !part.is_empty())
        .map(str::to_owned)
}

/// A query parameter's value, as it is (YouTube's IDs need no decoding).
fn query_value<'a>(query: &'a str, name: &str) -> Option<&'a str> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value)
        .filter(|value| !value.is_empty())
}

/// Letters, digits, "-", "_" and "." as YouTube's IDs and handles have, or "%" for a handle in another script, as a
/// browser copies it.
fn is_youtube_id(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|character| character.is_alphanumeric() || "-_.%".contains(character))
}

/// The link's host without "www.", as the site's name ("youtube.com").
fn site_name(link: &str) -> Option<String> {
    let host = sites::host(link)?;
    let host = host.strip_prefix("www.").unwrap_or(&host);
    (!host.is_empty()).then(|| host.to_owned())
}

/// Seconds since 1970 to the day, as YYYY-MM-DD, in UTC.
fn date_of_timestamp(seconds: i64) -> String {
    // Howard Hinnant's days-to-civil algorithm.
    let days = seconds.div_euclid(86_400);
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn shorten(title: &str) -> String {
    let title = title.trim();
    if title.chars().count() <= TITLE_LENGTH {
        return title.to_owned();
    }
    let shortened: String = title.chars().take(TITLE_LENGTH - 1).collect();
    format!("{}…", shortened.trim_end())
}

fn count_field(value: &Value, field: &str) -> Option<u32> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|count| u32::try_from(count).ok())
}

/// A field as text: a string, or a number written out.
fn text(value: &Value, field: &str) -> Option<String> {
    match value.get(field)? {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
