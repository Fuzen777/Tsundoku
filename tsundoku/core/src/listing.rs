//! Channels, playlists and accounts: links that hold many videos or posts, which the preview card lists for the user to
//! pick from. `listing_link` tells one from its address and gives the page to read (a YouTube channel's Videos tab, an
//! Instagram account's posts, an X account's Media tab); `read` lists what it holds without reading each item, newest
//! first for a channel or an account, in its own order for a playlist. `read_more` lists the items after those, for the
//! picker's Load More and Load All. `read_for_check` reads one for a watched source's check, an account a page
//! first. An X account's Media tab is read as its two lists, photos and videos, merged by date (`x_media`), which X
//! lets be read only so far every 15 minutes: a read past that waits (`read_more`'s `on_pause`). What a picker loaded
//! is kept for the next time its listing is opened (`saved`).
//! `mark` says which items the history or the queue has. Another site's playlist is
//! listed when yt-dlp answers one with a link and ID for each item (`from_yt_dlp`);
//! otherwise, and for image sites' accounts other than Instagram's and X's, the link is refused as
//! `PreviewError::NotListable`.
//!
//! Tested against real answers saved under `tests/fixtures/listing`.

use std::collections::HashSet;
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::cancel::CancelToken;
use crate::gallery::{self, AccountInfo, GalleryTools};
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

/// How many more items Load More lists on a YouTube channel: as many as the first read.
pub const MORE_COUNT: u32 = NEWEST_COUNT;

/// The most new items Load New may be asked for.
const NEW_COUNT_AT_MOST: u32 = 1000;

/// How an account's posts are read after the first ones: from gallery-dl's cursor (`gallery::last_cursor`), the page
/// where the last read stopped, so the newer pages aren't asked for again. On Instagram the cursor only moves on after
/// a whole page, so each read asks for whole pages and the next one starts on the page after; an X account's two lists
/// each have their own (`x_media`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Paging {
    /// How many posts the site answers per request, at most.
    page: u32,
    /// Load More's posts.
    next: u32,
    /// Each of Load New's reads, handed over as it ends, so a stop keeps the ones before.
    step: u32,
    /// Each of Load All's reads, handed over as it ends.
    all_step: u32,
    /// Between two reads.
    pause: Duration,
    /// How many of the newest posts a watched source's check reads first (`read_for_check`): what the site's first
    /// answer holds, less one, since gallery-dl only sees that a range has ended at the post after it, and asking for
    /// that post from the next page costs another request (and on Instagram its 6 to 12 seconds first).
    check_first: u32,
}

/// Instagram answers 12 posts per request and asks to slow down when read quickly. gallery-dl already waits 6 to 12
/// seconds between its own requests, but a new run's first request doesn't, so Load All waits 10 seconds between runs.
const INSTAGRAM_PAGING: Paging = Paging {
    page: 12,
    next: 48,
    step: 96,
    all_step: 96,
    pause: Duration::from_secs(10),
    // One page, pinned posts first, so up to 3 of these are pinned: a check reaches what it saw before within them
    // (`before_known_ground`) unless 8 or so posts came since.
    check_first: 12 - 1,
};

/// X is asked for 50 posts per request (gallery-dl's `limit`) and answers up to that many, usually fewer (14 to 45
/// posts with photos, 20 with videos), without waits between requests, but only 50 requests per list every 15 minutes
/// (`x_media`). Its Media tab is two lists, photos and videos, each read from its own cursor and merged. (When
/// gallery-dl 1.32.20 read the tab as one, photos then videos, a cursor saved in the photos failed once they ended: X
/// refused it for the videos as "Dependency: Unspecified", seen 2026-11-15 on x.com/rustlang. A list's own cursor
/// started its own list again exactly, 2026-11-21 on x.com/NASA.) A read ends inside a page, which the next one asks
/// for again: one request in 9 with Load New's reads, one in 17 with Load All's.
const X_PAGING: Paging = Paging {
    page: 50,
    next: 50,
    step: 200,
    all_step: 400,
    pause: Duration::ZERO,
    check_first: X_CHECK_FIRST,
};

/// Pixiv answers 30 works per request through its app's API, which gallery-dl asks 1 to 2 seconds apart for Pixiv
/// (`pixiv_account::gallery_dl_config`). A read can't start where the last one stopped (Pixiv's pages go by offset,
/// which gallery-dl doesn't take), so each one reads the pages before it again: Load All reads 600 works at a time, so
/// an artist of 1,000 works takes two reads, about 55 requests, against 34 in one go.
const PIXIV_PAGING: Paging = Paging {
    page: 30,
    next: 60,
    step: 150,
    all_step: 600,
    pause: Duration::from_secs(2),
    // One page, nothing pinned: a check reaches what it saw before within it unless 29 or more works came since.
    check_first: 30 - 1,
};

/// How far X lets an account be listed at a time (`x_media`).
const X_PACE: ListingPace = ListingPace {
    items: x_media::POSTS_PER_WINDOW,
    pause_seconds: x_media::WINDOW.as_secs() as u32,
};

/// How many of an X account's newest posts a watched source's check reads first, from each of its two lists: X's
/// first answer for a Media tab holds 20 to 40 posts (21, 26 and 39 seen), though 50 are asked for, so the newest 50
/// take a second page or more, half a second to a second each, after the request for the account itself.
/// Pinned posts aren't on the Media tab, so a check reaches what it saw before within these unless 16 or so came since.
const X_CHECK_FIRST: u32 = 19;

/// How long one read after the first may take: up to 8 of Instagram's pages, each after gallery-dl's 6 to 12 seconds,
/// and a minute's wait or more whenever Instagram asks to slow down.
const MORE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// How long Load All may read a YouTube channel in one go: tens of thousands of videos take a while.
const ALL_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);

/// How many of a channel's videos Load All hands over at a time, as yt-dlp reads them.
const ENTRIES_PER_BATCH: u32 = 100;

/// How long counting a channel's videos may take (`count_more`): one request for its playlist's first page.
const COUNT_TIMEOUT: Duration = Duration::from_secs(60);

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
    /// An Instagram account's posts and reels, as on its grid, an X account's posts with pictures or videos, as on
    /// its Media tab, or a Pixiv artist's works; also Pixiv's lists of many artists' works, newest first: the
    /// following feed and a tag's search.
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
    /// An X account only: how many photos and videos the site says it holds, as its Media tab words it ("372 photos &
    /// videos"). It counts files, not posts, and more of them than its posts listed whole hold (`gallery::account_info`
    /// has the measures), so `total` stays empty beside it and the picker shows it in the site's words.
    pub media_total: Option<u32>,
    /// How far the site lets it be listed before it pauses (an X account), for the picker to say how long listing it
    /// all takes.
    pub pace: Option<ListingPace>,
    /// Only the newest `NEWEST_COUNT` were asked for (a channel or an account; an X account lists that many of each
    /// of its two lists, so up to twice as many); a playlist is listed whole.
    pub newest_only: bool,
    pub items: Vec<ListedItem>,
    /// Where the items after these start, for Load More, Load New and Load All, or `None` when the listing is whole: a
    /// playlist, or a channel or an account with fewer than `NEWEST_COUNT`.
    pub more: Option<ListingMore>,
    /// Posts the read met past `items` without knowing their place among them yet (an X account's:
    /// `x_media::Merged::set_aside`). They aren't listed, and the UI never gets them: a watched source's check takes
    /// their pictures for the items it has (`WatchedStore::add_pictures`).
    #[serde(skip)]
    #[ts(skip)]
    pub also_read: Vec<VideoInfo>,
}

/// How far a site lets a listing be read at a time: X answers so many requests for an account's posts every 15
/// minutes (`x_media`), so listing a big account goes in stretches with pauses between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListingPace {
    /// About how many items are listed before the site pauses it.
    pub items: u32,
    /// How long a pause lasts, at most.
    pub pause_seconds: u32,
}

/// Where a listing's next items start, for the picker's Load More, Load New and Load All (`read_more`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListingMore {
    /// The page that lists the items, as `listing_link` gives it.
    pub link: String,
    /// The listing's name, which its next items take as their `playlist` too (`name_items`).
    pub title: String,
    /// How many of its items were read so far, in its order: a read without a `cursor` starts after them.
    pub read: u32,
    /// Where gallery-dl stopped in an Instagram account's posts (`gallery::last_cursor`): the next read starts there,
    /// without reading the newer pages again. It may start with posts read before, which the picker keeps once.
    pub cursor: Option<String>,
    /// A YouTube channel's ID ("UC…"), for `count_more` to read how many videos its Videos tab holds, which the tab
    /// doesn't say.
    pub channel_id: Option<String>,
    /// Where an X account's two lists of posts, with photos and with videos, were read up to (`x_media`): its next
    /// posts are read from there rather than from `read`.
    pub media: Option<MediaPlaces>,
}

/// Where an X account's posts with photos and its posts with videos were read up to, each list on its own
/// (`x_media`): where its next read starts, or `None` once that list ended and all of it was handed over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MediaPlaces {
    pub photos: Option<ListPlace>,
    pub videos: Option<ListPlace>,
    /// The ID of the oldest post handed over so far, or `None` before any: a read that starts on a page the last one
    /// stopped in passes over the posts down to it. As text, since the UI's numbers can't hold an X post's ID.
    pub handed_down_to: Option<String>,
}

/// Where one of an X account's lists is read from next.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListPlace {
    /// gallery-dl's cursor for the page the read starts on (`gallery::last_cursor`), or `None` for the list's first.
    pub cursor: Option<String>,
    /// How many posts from that page on were read already, which the read passes over (`--post-range`): only when a
    /// read ended on the page it started on.
    pub skip: u32,
}

/// What the picker asks `read_more` for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum MoreRead {
    /// The next items (Load More).
    Next,
    /// On past what the history and the queue have, until at least `count` items they don't have are listed, or the
    /// end (Load New): what's new since the last download, when the newest were downloaded.
    New { count: u32 },
    /// All the rest (Load All).
    All,
}

/// Items after a listing's first ones, as Load More or Load All read them, handed over a batch at a time
/// (`read_more`).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MoreItems {
    /// In the listing's order: newest first for an account, whose posts of one read are sorted by date. May hold items
    /// listed before, which the picker keeps once.
    pub items: Vec<ListedItem>,
    /// Where the items after these start, or `None` once the listing's end was reached.
    pub more: Option<ListingMore>,
    /// How many the listing holds in all, when the site says, or once the end was reached.
    pub total: Option<u32>,
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

    if sites::on_domain(&host, "fanbox.cc") {
        return fanbox::listing_link(&host, &segments);
    }

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

    if sites::on_domain(&host, "pixiv.net") {
        return pixiv_listing(&segments, query);
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

/// A Pixiv listing, from its address's segments (a language first, such as "en", left out) and query: an artist's
/// works (`/users/<id>`, with any of its tabs of works), the works of the artists the account follows
/// (`/bookmark_new_illust.php`), or a tag's newest works (`/tags/<tag>`, with any of its tabs of works, keeping how the
/// tag is matched, `s_mode`).
fn pixiv_listing(segments: &[&str], query: &str) -> Option<ListingLink> {
    let segments = match segments {
        [language, rest @ ..]
            if language.len() == 2 && language.chars().all(|c| c.is_ascii_lowercase()) =>
        {
            rest
        }
        all => all,
    };
    let link = match segments {
        ["users", id] | ["users", id, "artworks" | "illustrations" | "manga"]
            if !id.is_empty() && id.chars().all(|character| character.is_ascii_digit()) =>
        {
            format!("https://www.pixiv.net/users/{id}/artworks")
        }
        ["bookmark_new_illust.php"] => PIXIV_FOLLOWING.to_owned(),
        ["tags", tag] | ["tags", tag, "artworks" | "illustrations" | "manga" | "top"]
            if !tag.is_empty() =>
        {
            let mode = query_value(query, "s_mode")
                .filter(|mode| ["s_tag", "s_tag_full", "s_tc"].contains(mode))
                .map(|mode| format!("?s_mode={mode}"))
                .unwrap_or_default();
            format!("https://www.pixiv.net/tags/{tag}/artworks{mode}")
        }
        _ => return None,
    };
    Some(ListingLink {
        kind: ListingKind::Account,
        link,
    })
}

/// The works of the artists the Pixiv account follows, newest first.
const PIXIV_FOLLOWING: &str = "https://www.pixiv.net/bookmark_new_illust.php";

/// Whether a listing's page is Pixiv's.
fn on_pixiv(link: &str) -> bool {
    sites::host(link).is_some_and(|host| sites::on_domain(&host, "pixiv.net"))
}

/// The name of a Pixiv listing of many artists' works, which no artist names: the following feed's, or a tag's.
/// `None` for an artist's works and anything else.
fn pixiv_list_title(link: &str) -> Option<String> {
    if !on_pixiv(link) {
        return None;
    }
    if link == PIXIV_FOLLOWING {
        return Some("Pixiv Following".to_owned());
    }
    let (path, _) = path_and_query(link);
    let tag = path.strip_prefix("tags/")?.split('/').next()?;
    let decoded = reqwest::Url::parse(&format!("https://x/{tag}"))
        .ok()
        .and_then(|url| url.path_segments()?.next().map(str::to_owned))
        .map(|segment| percent_decoded(&segment))
        .unwrap_or_else(|| tag.to_owned());
    Some(format!("#{decoded}"))
}

/// Text with its %-escapes decoded as UTF-8, or as it is when they don't make UTF-8.
fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escaped = (bytes[index] == b'%')
            .then(|| text.get(index + 1..index + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escaped {
            Some(byte) => {
                decoded.push(byte);
                index += 3;
            }
            None => {
                decoded.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).unwrap_or_else(|_| text.to_owned())
}

/// Whether a host is X's, under either of its domains.
fn on_x(host: &str) -> bool {
    sites::on_domain(host, "x.com") || sites::on_domain(host, "twitter.com")
}

/// Whether a listing's page is on X.
fn is_on_x(link: &str) -> bool {
    sites::host(link).is_some_and(|host| on_x(&host))
}

/// An account's own page, on the site of its listing's page (`link`).
fn account_page(link: &str, name: &str) -> String {
    if on_pixiv(link) {
        // A Pixiv artist's page goes by its number, which the listing's own page has.
        return link.to_owned();
    }
    if is_on_x(link) {
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
    if fanbox::is_fanbox(&listing.link) {
        return fanbox::read(&listing.link, NEWEST_COUNT, cancel);
    }
    if listing.reads_images() {
        return read_account(images, listing, NEWEST_COUNT, cancel);
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

/// An account's newest `count` posts, with gallery-dl: an X account's from both its lists (`read_x_account`).
fn read_account(
    images: &GalleryTools,
    listing: &ListingLink,
    count: u32,
    cancel: &CancelToken,
) -> Result<Listing, PreviewError> {
    if is_on_x(&listing.link) {
        return read_x_account(images, &listing.link, count, cancel)
            .map_err(|error| signed_out_failure(error, images.cookies.is_some()));
    }
    let range = format!("1-{count}");
    // Its debug lines say where it stopped, for Load More to start there (`gallery::last_cursor`).
    let extra = ["--post-range", range.as_str(), "--verbose"];
    let read = gallery::run_logged(images, &listing.link, &extra, LISTING_TIMEOUT, cancel)
        .and_then(|answer| {
            let mut found = from_gallery_dl(&answer.json, &listing.link, count)?;
            if let Some(more) = found.more.as_mut() {
                more.cursor = gallery::last_cursor(&answer.log);
            }
            Ok(found)
        });
    read.map_err(|error| signed_out_failure(error, images.cookies.is_some()))
}

/// An X account's newest posts: the newest `count` of each of its lists (photos and videos), merged by date down to
/// where the list that reaches less far back stopped (`x_media`), so `count` of them or more. When X's limit on a
/// list's requests is used up, it lists what it read before, or is refused as `ListingPaused` when that's nothing.
fn read_x_account(
    images: &GalleryTools,
    link: &str,
    count: u32,
    cancel: &CancelToken,
) -> Result<Listing, PreviewError> {
    let lists = x_media::GalleryReader {
        images,
        media_tab: link,
        cancel,
    };
    let mut reader = x_media::MediaReader::new(&lists, x_media::FROM_THE_START);
    let step = reader.step(count, &|_| {})?;
    if step.posts.is_empty() && step.limited {
        return Err(listing_paused(link));
    }
    let items = listed(step.posts);
    let read = count_of(items.len());
    let mut listing = account_listing(reader.account(), link, items, |title, _| {
        step.next.map(|places| ListingMore {
            link: link.to_owned(),
            title: title.to_owned(),
            read,
            cursor: None,
            channel_id: None,
            media: Some(places),
        })
    });
    listing.also_read = reader.set_aside();
    Ok(listing)
}

/// The refusal for a listing whose site lists no more for now, naming the site.
fn listing_paused(link: &str) -> PreviewError {
    PreviewError::ListingPaused {
        site: shown_site(link),
    }
}

/// A listing's site as messages name it: its name ("X") when it's one Tsundoku knows, else its domain.
pub(crate) fn shown_site(link: &str) -> String {
    sites::sign_in_site(link)
        .map(|site| site.name.to_owned())
        .or_else(|| site_name(link))
        .unwrap_or_default()
}

/// Reads a listing for a watched source's check, which only needs its items up to what the source saw before. An
/// account is read a page at a time by gallery-dl (on Instagram waiting 6 to 12 seconds before each; on X each takes a
/// second or so), so the posts its site's first answer holds are read first (`Paging::check_first`), and its newest
/// `NEWEST_COUNT`, as the preview card reads them, only when `reached` says those don't reach what was seen before and
/// the account has more. A channel or a playlist is read as `read` does: YouTube answers 50 at once.
pub fn read_for_check(
    videos: &FetchTools,
    images: &GalleryTools,
    listing: &ListingLink,
    cancel: &CancelToken,
    reached: &dyn Fn(&Listing) -> bool,
) -> Result<Listing, PreviewError> {
    if fanbox::is_fanbox(&listing.link) {
        return fanbox::read_for_check(&listing.link, cancel, reached);
    }
    if !listing.reads_images() {
        return read(videos, images, listing, cancel);
    }
    let paging = paging_of(&listing.link);
    read_in_steps(
        paging.check_first,
        &mut |count| read_account(images, listing, count, cancel),
        &mut || pause(paging.pause, cancel),
        reached,
    )
}

/// `read_for_check`'s steps for an account: its newest `first_count` posts, then its newest `NEWEST_COUNT` if it must,
/// with how it reads (`read`, the newest so many) and waits between two reads (`pause`) given, for the tests.
fn read_in_steps(
    first_count: u32,
    read: &mut dyn FnMut(u32) -> Result<Listing, PreviewError>,
    pause: &mut dyn FnMut() -> Result<(), PreviewError>,
    reached: &dyn Fn(&Listing) -> bool,
) -> Result<Listing, PreviewError> {
    let first = read(first_count)?;
    if first.more.is_none() || reached(&first) {
        return Ok(first);
    }
    // A new run's first request doesn't wait as gallery-dl's later ones do (Instagram's).
    pause()?;
    read(NEWEST_COUNT)
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

/// Lists the items after those a listing read so far (`more`): the next ones (`MoreRead::Next`), on until enough new
/// ones (`MoreRead::New`) or all the rest (`MoreRead::All`), handing them to `on_batch` as they're read. `on_batch`
/// answers how many of a batch's items the history and the queue don't have, once it marked them, for Load New to
/// know when it has enough; it can stop the read with an error, which it then returns.
///
/// - A YouTube channel: yt-dlp from the entry after `more.read`, the next `MORE_COUNT` (`--playlist-items`); or the
///   rest as yt-dlp reads them (`metadata::run_entries`), handed over `ENTRIES_PER_BATCH` at a time, or for Load New,
///   once some new ones came, as many at a time as are still wanted, so it stops close to the count asked for.
/// - An Instagram account: gallery-dl from `more.cursor`, the page where the last read stopped, so the pages before
///   aren't read again (Instagram is the site that asks to slow down), in whole pages (`Paging`): Load More's posts, or
///   reads of `Paging::step` with `Paging::pause` between, each handed over once it ends, so a stop keeps what was
///   read. Load New asks for as many pages as are still wanted once some new ones came. Without a cursor, from the post
///   after `more.read` (`--post-range`).
/// - An X account: each of its two lists, photos and videos, from its place in `more.media` (the page where its last
///   read stopped), merged by date (`x_media`): Load More in reads of 75 of each until 50 posts or more are handed
///   over (a read starts with up to a page listed before), Load New in reads of 200 of each (then the whole pages
///   still wanted), Load All in reads of 400 of each. X answers 50 requests per list every 15 minutes: a read that
///   meets that limit hands over what it read, and the next one waits until X answers again, `on_pause` being told
///   until when (milliseconds since 1970), then `None` once it reads on.
///
/// A playlist is listed whole, so it has nothing more. The first batch may repeat items read before, when a read
/// starts on a page it ended in; later ones leave out what this read handed over already.
pub fn read_more(
    videos: &FetchTools,
    images: &GalleryTools,
    more: &ListingMore,
    read: MoreRead,
    cancel: &CancelToken,
    on_batch: &mut dyn FnMut(MoreItems) -> Result<u32, PreviewError>,
    on_pause: &(dyn Fn(Option<i64>) + Sync),
) -> Result<(), PreviewError> {
    // Only a link `listing_link` gives is read: the picker hands back what `read` gave it.
    let listing = listing_link(&more.link)
        .filter(|listing| listing.link == more.link && listing.kind != ListingKind::Playlist)
        .ok_or_else(|| not_listable(&more.link))?;
    if fanbox::is_fanbox(&more.link) {
        return fanbox::read_more(more, read, cancel, on_batch);
    }
    let wanted = match read {
        MoreRead::New { count } => Some(count.clamp(1, NEW_COUNT_AT_MOST)),
        MoreRead::Next | MoreRead::All => None,
    };
    if !listing.reads_images() {
        return match read {
            MoreRead::Next => on_batch(channel_more(videos, more, cancel)?).map(|_| ()),
            MoreRead::New { .. } | MoreRead::All => {
                channel_rest(videos, more, wanted, cancel, on_batch)
            }
        };
    }
    let paging = paging_of(&more.link);
    let on_x = is_on_x(&more.link);
    let lists = x_media::GalleryReader {
        images,
        media_tab: &more.link,
        cancel,
    };
    // An X account's two lists, which keep what they read from one read to the next.
    let mut x_reader = on_x.then(|| {
        let from = more.media.clone().unwrap_or(x_media::FROM_THE_START);
        x_media::MediaReader::new(&lists, from)
    });
    let mut from = more.clone();
    let mut handed = HashSet::new();
    let mut found = 0;
    let mut nothing_in_a_row = 0;
    loop {
        let count = match (read, wanted) {
            // An X list's read starts with the posts of its page listed before, half a page of them or so.
            (MoreRead::Next, _) if on_x => paging.next + paging.page / 2,
            (MoreRead::Next, _) => paging.next,
            (_, Some(wanted)) if found > 0 => {
                whole_pages(wanted.saturating_sub(found), paging.page)
            }
            (MoreRead::All, _) => paging.all_step,
            _ => paging.step,
        };
        let batch = match x_reader.as_mut() {
            Some(reader) => x_more(reader, &from, count, on_pause),
            None => account_more(images, &from, count, cancel),
        };
        let mut batch =
            batch.map_err(|error| signed_out_failure(error, images.cookies.is_some()))?;
        batch
            .items
            .retain(|item| handed.insert((item.video.site.clone(), item.video.video_id.clone())));
        // A read may hand nothing over and still move on (X's limit met at once, or a page listed before), but not
        // time after time.
        nothing_in_a_row = if batch.items.is_empty() {
            nothing_in_a_row + 1
        } else {
            0
        };
        let next = batch.more.clone();
        found += on_batch(batch)?;
        let enough = match (read, wanted) {
            // Load More's posts may take an X account a second read.
            (MoreRead::Next, _) => !on_x || handed.len() >= paging.next as usize,
            (_, Some(wanted)) => found >= wanted,
            (_, None) => false,
        };
        match next {
            Some(next) if !enough && nothing_in_a_row < READS_OF_NOTHING_AT_MOST => from = next,
            _ => return Ok(()),
        }
        pause(paging.pause, cancel)?;
    }
}

/// How many reads in a row may hand nothing over before `read_more` stops: an X list stopped at its limit hands over
/// nothing once, and the read after it waits and goes on.
const READS_OF_NOTHING_AT_MOST: u32 = 4;

/// How an account's site is read after the first posts.
fn paging_of(link: &str) -> Paging {
    if on_instagram(link) {
        INSTAGRAM_PAGING
    } else if on_pixiv(link) {
        PIXIV_PAGING
    } else {
        X_PAGING
    }
}

/// `count` posts rounded up to whole pages of `page`, at least one page.
fn whole_pages(count: u32, page: u32) -> u32 {
    count.div_ceil(page).max(1) * page
}

/// How many items a listing holds in all, when its first read didn't say and another read can, for Load All to say how
/// many it would list: a YouTube channel's Videos tab, as the channel's playlist of its uploads that are videos says
/// (`videos_playlist`), which holds the same videos. `None` for a listing no read can count, or when the count is less
/// than was read. An Instagram account says at once. An X account never says how many posts it has: what it counts is
/// its photos and videos, which its first read gives as `Listing.media_total`.
pub fn count_more(
    videos: &FetchTools,
    more: &ListingMore,
    cancel: &CancelToken,
) -> Result<Option<u32>, PreviewError> {
    let channel = listing_link(&more.link)
        .is_some_and(|listing| listing.link == more.link && listing.kind == ListingKind::Channel);
    let Some(playlist) = more
        .channel_id
        .as_deref()
        .and_then(videos_playlist)
        .filter(|_| channel)
    else {
        return Ok(None);
    };
    let answer = metadata::run(
        videos,
        &playlist,
        &["--playlist-items", "1"],
        COUNT_TIMEOUT,
        cancel,
    )?;
    let answer: Value =
        serde_json::from_str(&answer.json).map_err(|error| PreviewError::Failed {
            detail: format!("yt-dlp's answer can't be read: {error}"),
        })?;
    Ok(count_field(&answer, "playlist_count").filter(|total| *total >= more.read))
}

/// The playlist of a YouTube channel's uploads that are videos, without Shorts and live streams: "UULF" and the
/// channel's ID after "UC". It holds the Videos tab's videos and says how many there are, which the tab doesn't. `None`
/// for anything that isn't a channel's ID.
fn videos_playlist(channel_id: &str) -> Option<String> {
    let id = channel_id.strip_prefix("UC").filter(|id| {
        id.len() == 22
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    })?;
    Some(format!("https://www.youtube.com/playlist?list=UULF{id}"))
}

/// The next `MORE_COUNT` of a channel's videos after `more.read`.
fn channel_more(
    videos: &FetchTools,
    more: &ListingMore,
    cancel: &CancelToken,
) -> Result<MoreItems, PreviewError> {
    let window = Window {
        start: more.read,
        count: MORE_COUNT,
    };
    let items = format!("{}:{}", window.start + 1, window.start + window.count);
    let answer = metadata::run(
        videos,
        &more.link,
        &[
            "--extractor-args",
            "youtubetab:approximate_date",
            "--playlist-items",
            &items,
        ],
        MORE_TIMEOUT,
        cancel,
    )?;
    let mut found = from_yt_dlp_window(
        &answer.json,
        Some(ListingKind::Channel),
        &more.link,
        Some(window),
    )?;
    name_items(&mut found.items, &more.title);
    Ok(MoreItems {
        items: found.items,
        more: found.more.map(|next| ListingMore {
            title: more.title.clone(),
            channel_id: more.channel_id.clone(),
            ..next
        }),
        total: found.total,
    })
}

/// A channel's videos after `more.read` as yt-dlp reads them, handed over `ENTRIES_PER_BATCH` at a time, then a last
/// batch with no `more`. For Load New (`wanted`), it stops once `on_batch` counted that many new ones, handing over
/// only as many at a time as are still wanted once some came. When the read fails or is stopped, what was read is
/// handed over first, with where it stopped as its `more`.
fn channel_rest(
    videos: &FetchTools,
    more: &ListingMore,
    wanted: Option<u32>,
    cancel: &CancelToken,
    on_batch: &mut dyn FnMut(MoreItems) -> Result<u32, PreviewError>,
) -> Result<(), PreviewError> {
    let items_from = format!("{}:", more.read + 1);
    let mut read = more.read;
    let mut batch: Vec<ListedItem> = Vec::new();
    let mut found = 0;
    let mut refused = None;
    let mut enough = false;
    let stop = cancel.clone();
    let mut hand_over = |items: Vec<ListedItem>, read: u32, refused: &mut Option<PreviewError>| {
        let next = ListingMore {
            read,
            ..more.clone()
        };
        match on_batch(MoreItems {
            items,
            more: Some(next),
            total: None,
        }) {
            Ok(new) => new,
            Err(error) => {
                *refused = Some(error);
                stop.cancel();
                0
            }
        }
    };
    let result = metadata::run_entries(
        videos,
        &more.link,
        &[
            "--extractor-args",
            "youtubetab:approximate_date",
            "--playlist-items",
            &items_from,
        ],
        ALL_TIMEOUT,
        cancel,
        &mut |line| {
            if refused.is_some() || enough {
                return;
            }
            let Ok(entry) = serde_json::from_str::<Value>(line) else {
                return;
            };
            read += 1;
            batch.extend(channel_entry(entry, &more.title));
            let size = match wanted {
                Some(wanted) if found > 0 => {
                    wanted.saturating_sub(found).clamp(1, ENTRIES_PER_BATCH)
                }
                _ => ENTRIES_PER_BATCH,
            };
            if count_of(batch.len()) >= size {
                found += hand_over(std::mem::take(&mut batch), read, &mut refused);
                if wanted.is_some_and(|wanted| found >= wanted) {
                    enough = true;
                    stop.cancel();
                }
            }
        },
    );
    if let Some(error) = refused {
        return Err(error);
    }
    if enough {
        return Ok(());
    }
    if let Err(error) = result {
        if !batch.is_empty() {
            hand_over(std::mem::take(&mut batch), read, &mut refused);
        }
        return Err(refused.unwrap_or(error));
    }
    on_batch(MoreItems {
        items: batch,
        more: None,
        total: Some(read),
    })
    .map(|_| ())
}

/// One of a channel's videos from yt-dlp's line for it (`metadata::run_entries`), as `from_yt_dlp` makes it from the
/// whole answer, or `None` for a premiere or a live stream, or an entry without a link and ID of its own. The line's
/// `upload_date` is yt-dlp's day of the rough date, so it's left out: the date stays rough.
fn channel_entry(mut entry: Value, title: &str) -> Option<ListedItem> {
    if matches!(
        text(&entry, "live_status").as_deref(),
        Some("is_upcoming" | "is_live" | "post_live")
    ) {
        return None;
    }
    if let Some(fields) = entry.as_object_mut() {
        fields.remove("upload_date");
    }
    let mut item = item_from_entry(&entry, true)?;
    if item.video.uploader.is_none() {
        item.video.uploader =
            text(&entry, "playlist_channel").or_else(|| text(&entry, "playlist_uploader"));
    }
    if item.video.account.is_none() {
        item.video.account =
            text(&entry, "playlist_uploader_id").and_then(|id| metadata::handle(&id));
    }
    item.video.playlist = Some(title.to_owned());
    Some(item)
}

/// An Instagram account's posts after those read so far, at most `count`: from `more.cursor` when there's one, else
/// from the post after `more.read`. The answer's end comes before `count` once the account's end is reached. The next
/// read starts from where this one stopped.
fn account_more(
    images: &GalleryTools,
    more: &ListingMore,
    count: u32,
    cancel: &CancelToken,
) -> Result<MoreItems, PreviewError> {
    let cursor = more
        .cursor
        .as_deref()
        .map(|cursor| format!("cursor={cursor}"));
    let range = match &cursor {
        Some(_) => format!("1-{count}"),
        None => format!("{}-{}", more.read + 1, more.read + count),
    };
    // Its debug lines say where it stopped, for the next read to start there (`gallery::last_cursor`).
    let mut extra = vec!["--post-range", range.as_str(), "--verbose"];
    if let Some(cursor) = &cursor {
        extra.extend(["-o", cursor.as_str()]);
    }
    let answer = gallery::run_logged(images, &more.link, &extra, MORE_TIMEOUT, cancel)?;
    let found = gallery::parse_posts(&answer.json, &more.link)?;
    let answered = count_of(found.answered);
    let total = found
        .account
        .and_then(|account| account.post_count)
        .filter(|total| *total as usize >= found.posts.len());
    let mut items = newest_first(found.posts);
    name_items(&mut items, &more.title);

    let reached_end = answered < count;
    let next = (!reached_end).then(|| ListingMore {
        read: more.read + answered,
        // A cursor that didn't move would read the same posts again: from `read` instead.
        cursor: gallery::last_cursor(&answer.log).filter(|next| more.cursor.as_ref() != Some(next)),
        ..more.clone()
    });
    Ok(MoreItems {
        items,
        more: next,
        total,
    })
}

/// An X account's next posts, from one step of its lists' reader (`x_media::MediaReader::step`): at most `count` of
/// each list read. `more` is where the listing stood before it.
fn x_more(
    reader: &mut x_media::MediaReader,
    more: &ListingMore,
    count: u32,
    on_pause: &(dyn Fn(Option<i64>) + Sync),
) -> Result<MoreItems, PreviewError> {
    let step = reader.step(count, on_pause)?;
    let mut items = listed(step.posts);
    name_items(&mut items, &more.title);
    let next = step.next.map(|places| ListingMore {
        read: more.read + count_of(items.len()),
        media: Some(places),
        ..more.clone()
    });
    Ok(MoreItems {
        items,
        more: next,
        total: None,
    })
}

/// Waits `length`, unless it's cancelled meanwhile.
fn pause(length: Duration, cancel: &CancelToken) -> Result<(), PreviewError> {
    let until = Instant::now() + length;
    while Instant::now() < until {
        if cancel.is_cancelled() {
            return Err(PreviewError::Cancelled);
        }
        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// Marks each item that the history has (`downloaded`, by site and ID as the duplicate check does) or that the queue
/// has and isn't done with (`queued`).
pub fn mark<E>(
    listing: &mut Listing,
    downloaded: impl Fn(&str, &str) -> Result<Option<PastDownload>, E>,
    queued: impl Fn(&str, &str) -> bool,
) -> Result<(), E> {
    mark_items(&mut listing.items, downloaded, queued)
}

/// As `mark`, for items read after a listing's first ones.
pub fn mark_items<E>(
    items: &mut [ListedItem],
    downloaded: impl Fn(&str, &str) -> Result<Option<PastDownload>, E>,
    queued: impl Fn(&str, &str) -> bool,
) -> Result<(), E> {
    for item in items {
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
    PreviewError::NotListable {
        site: shown_site(link),
    }
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
    let window = (kind == Some(ListingKind::Channel)).then_some(Window {
        start: 0,
        count: NEWEST_COUNT,
    });
    from_yt_dlp_window(json, kind, link, window)
}

/// Which of a listing's entries a read asked for: at most `count` after the first `start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Window {
    start: u32,
    count: u32,
}

/// Reads yt-dlp's answer for a playlist as `from_yt_dlp` does, for a read of `window`'s entries, or of them all
/// without one: when it answered as many as asked, the listing has `more` after them.
fn from_yt_dlp_window(
    json: &str,
    kind: Option<ListingKind>,
    link: &str,
    window: Option<Window>,
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
    let total =
        count_field(&answer, "playlist_count").filter(|total| *total as usize >= items.len());
    let more = window
        .filter(|window| count_of(entries.len()) >= window.count)
        .map(|window| ListingMore {
            link: link.to_owned(),
            title: title.clone(),
            read: window.start + count_of(entries.len()),
            cursor: None,
            // A channel's tab is its channel's ID too.
            channel_id: (kind == ListingKind::Channel)
                .then(|| text(&answer, "channel_id").or_else(|| text(&answer, "id")))
                .flatten()
                .filter(|id| videos_playlist(id).is_some()),
            media: None,
        })
        .filter(|more| total.is_none_or(|total| more.read < total));
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
        total,
        media_total: None,
        pace: None,
        newest_only: kind == ListingKind::Channel,
        url,
        items,
        more,
        also_read: Vec::new(),
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
            tags: Vec::new(),
            pixiv: None,
            fanbox: None,
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

/// Reads gallery-dl's answer for an account's newest `asked` posts: each post becomes an item, as reading its own link
/// would make it, newest first (pinned posts come first in the answer). An answer of all the posts asked may have more.
fn from_gallery_dl(json: &str, link: &str, asked: u32) -> Result<Listing, PreviewError> {
    let found = gallery::parse_posts(json, link)?;
    let answered = count_of(found.answered);
    Ok(account_listing(
        found.account,
        link,
        newest_first(found.posts),
        |title, total| {
            (answered >= asked)
                .then(|| ListingMore {
                    link: link.to_owned(),
                    title: title.to_owned(),
                    read: answered,
                    cursor: None,
                    channel_id: None,
                    media: None,
                })
                .filter(|more| total.is_none_or(|total| more.read < total))
        },
    ))
}

/// An account's listing of `items` (newest first), named after the account, else the name in its link (`link`, the
/// listing's page), with where its next items start: `more`, given the listing's title and how many posts the account
/// says it has.
fn account_listing(
    account: Option<AccountInfo>,
    link: &str,
    mut items: Vec<ListedItem>,
    more: impl FnOnce(&str, Option<u32>) -> Option<ListingMore>,
) -> Listing {
    // A Pixiv listing of many artists is named after itself, not after its newest work's artist.
    let account = match pixiv_list_title(link) {
        Some(title) => AccountInfo {
            full_name: Some(title),
            ..AccountInfo::default()
        },
        None => account.unwrap_or_default(),
    };
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
    let total = account
        .post_count
        .filter(|total| *total as usize >= items.len());
    let more = more(&title, total);
    Listing {
        kind: ListingKind::Account,
        title,
        uploader: None,
        site_name: site_name(link).unwrap_or_default(),
        url: account
            .username
            .as_ref()
            .map_or_else(|| link.to_owned(), |name| account_page(link, name)),
        thumbnail_url: account.picture,
        total,
        media_total: account.media_count,
        pace: is_on_x(link).then_some(X_PACE),
        newest_only: true,
        items,
        more,
        also_read: Vec::new(),
    }
}

/// An account's posts as items, in their order.
fn listed(posts: Vec<VideoInfo>) -> Vec<ListedItem> {
    posts
        .into_iter()
        .map(|video| ListedItem {
            video,
            approximate_date: None,
            last_download: None,
            in_queue: false,
        })
        .collect()
}

/// An account's posts as items, newest first. Stable, so posts of one day keep the answer's order; a post without a
/// date goes last. Pinned posts, which come first in the answer, go where their date puts them.
fn newest_first(posts: Vec<VideoInfo>) -> Vec<ListedItem> {
    let mut items = listed(posts);
    sort_newest_first(&mut items);
    items
}

/// Puts an account's items in the order of their dates, newest first: stable, and one without a date goes last.
fn sort_newest_first(items: &mut [ListedItem]) {
    items.sort_by(|first, second| {
        let date = |item: &ListedItem| item.video.upload_date.clone();
        match (date(first), date(second)) {
            (Some(first), Some(second)) => second.cmp(&first),
            (first, second) => second.is_some().cmp(&first.is_some()),
        }
    });
}

/// Whether a listing's page is on Instagram.
fn on_instagram(link: &str) -> bool {
    sites::host(link).is_some_and(|host| sites::on_domain(&host, "instagram.com"))
}

/// A count as the listings keep it.
fn count_of(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
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

mod fanbox;
pub mod saved;
mod x_media;

#[cfg(test)]
mod tests;
