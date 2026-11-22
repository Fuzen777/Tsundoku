//! Fanbox's lists: a creator's posts (`www.fanbox.cc/@<creator>/posts`, from `<creator>.fanbox.cc` or
//! `www.fanbox.cc/@<creator>`), and the signed-in account's feeds: Home (`www.fanbox.cc/home`, the creators it follows
//! and supports) and Supporting (`www.fanbox.cc/home/supporting`). Read through Fanbox's API in Tsundoku's window
//! (`hosts::fanbox::ask_all`), 10 posts a request, 1 to 2 seconds apart. A creator's pages are known at once
//! (`post.paginateCreator` gives the address of each); a feed's page gives the next one's (`nextUrl`). Either way, the
//! next page's address is the listing's `ListingMore.cursor`. A post the account can't open is listed too, marked
//! with its plan (`VideoInfo.fanbox`), and nothing of it is downloaded.

use serde_json::Value;

use super::{
    Listing, ListingKind, ListingLink, ListingMore, MoreItems, MoreRead, NEW_COUNT_AT_MOST,
    NEWEST_COUNT, name_items, newest_first,
};
use crate::cancel::CancelToken;
use crate::hosts::fanbox::{ask_all, listed_video};
use crate::metadata::{PreviewError, VideoInfo};

/// How many posts each of Fanbox's list pages holds.
const PAGE: u32 = 10;
/// How many of a list's newest posts a watched source's check reads first: one page.
pub(super) const CHECK_FIRST: u32 = PAGE;
/// Load More's posts, and each batch Load New and Load All hand over.
const NEXT: u32 = NEWEST_COUNT;

const HOME: &str = "https://www.fanbox.cc/home";
const SUPPORTING: &str = "https://www.fanbox.cc/home/supporting";
const HOME_TITLE: &str = "Fanbox Home";
const SUPPORTING_TITLE: &str = "Fanbox Supporting";

/// Which of Fanbox's lists a listing is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum List {
    /// A creator's posts, by the creator's ID.
    Creator(String),
    Home,
    Supporting,
}

/// The Fanbox list a link is, with the page to read: `host` and `segments` are the link's, without empty ones.
pub(super) fn listing_link(host: &str, segments: &[&str]) -> Option<ListingLink> {
    let list = match (host, segments) {
        ("www.fanbox.cc", ["home"]) => List::Home,
        ("www.fanbox.cc", ["home", "supporting"]) => List::Supporting,
        ("www.fanbox.cc", [creator] | [creator, "posts"]) => {
            List::Creator(creator.strip_prefix('@')?.to_owned())
        }
        (host, [] | ["posts"]) => List::Creator(creator_host(host)?.to_owned()),
        _ => return None,
    };
    if let List::Creator(creator) = &list
        && !is_creator_id(creator)
    {
        return None;
    }
    Some(ListingLink {
        kind: ListingKind::Account,
        link: page_of(&list),
    })
}

/// A creator's own subdomain (`<creator>.fanbox.cc`), not one of Fanbox's.
fn creator_host(host: &str) -> Option<&str> {
    let creator = host.strip_suffix(".fanbox.cc")?;
    (!["www", "api", "downloads"].contains(&creator)).then_some(creator)
}

fn is_creator_id(creator: &str) -> bool {
    !creator.is_empty()
        && creator
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_".contains(character))
}

fn page_of(list: &List) -> String {
    match list {
        List::Creator(creator) => format!("https://www.fanbox.cc/@{creator}/posts"),
        List::Home => HOME.to_owned(),
        List::Supporting => SUPPORTING.to_owned(),
    }
}

fn list_of(link: &str) -> Option<List> {
    match link {
        HOME => Some(List::Home),
        SUPPORTING => Some(List::Supporting),
        _ => link
            .strip_prefix("https://www.fanbox.cc/@")?
            .strip_suffix("/posts")
            .filter(|creator| is_creator_id(creator))
            .map(|creator| List::Creator(creator.to_owned())),
    }
}

/// Whether a listing's page is one of Fanbox's.
pub(super) fn is_fanbox(link: &str) -> bool {
    list_of(link).is_some()
}

/// A list's newest `count` posts, a page at a time.
pub(super) fn read(link: &str, count: u32, cancel: &CancelToken) -> Result<Listing, PreviewError> {
    let list = list_of(link).ok_or_else(|| super::not_listable(link))?;
    let (title, picture, mut pages) = match &list {
        List::Creator(creator) => {
            let bodies = ask_all(&[
                format!("https://api.fanbox.cc/creator.get?creatorId={creator}"),
                format!("https://api.fanbox.cc/post.paginateCreator?creatorId={creator}"),
            ])?;
            let user = bodies.first().and_then(|body| body.get("user"));
            let text = |key: &str| {
                user.and_then(|user| user.get(key))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            };
            let pages = bodies.get(1).map(page_urls).unwrap_or_default();
            (
                text("name").unwrap_or_else(|| creator.clone()),
                text("iconUrl"),
                Pages::Known(pages),
            )
        }
        List::Home => (HOME_TITLE.to_owned(), None, Pages::Next(feed_first(&list))),
        List::Supporting => (
            SUPPORTING_TITLE.to_owned(),
            None,
            Pages::Next(feed_first(&list)),
        ),
    };
    let (videos, next) = read_pages(&mut pages, count, cancel)?;
    let mut items = newest_first(videos);
    name_items(&mut items, &title);
    let read = super::count_of(items.len());
    let more = next.map(|cursor| ListingMore {
        link: link.to_owned(),
        title: title.clone(),
        read,
        cursor: Some(cursor),
        channel_id: None,
        media: None,
    });
    Ok(Listing {
        kind: ListingKind::Account,
        title,
        uploader: None,
        site_name: "fanbox.cc".to_owned(),
        url: link.to_owned(),
        thumbnail_url: picture,
        total: more.is_none().then_some(read),
        media_total: None,
        pace: None,
        newest_only: true,
        items,
        more,
        also_read: Vec::new(),
    })
}

/// A watched source's check: the newest page, then the newest `NEWEST_COUNT` when that page doesn't reach what the
/// source saw before.
pub(super) fn read_for_check(
    link: &str,
    cancel: &CancelToken,
    reached: &dyn Fn(&Listing) -> bool,
) -> Result<Listing, PreviewError> {
    let first = read(link, CHECK_FIRST, cancel)?;
    if first.more.is_none() || reached(&first) {
        return Ok(first);
    }
    read(link, NEWEST_COUNT, cancel)
}

/// Load More (the next `NEXT`), Load New (until `count` new ones) and Load All (the rest), from where the listing
/// stopped, a batch of `NEXT` at a time.
pub(super) fn read_more(
    more: &ListingMore,
    read: MoreRead,
    cancel: &CancelToken,
    on_batch: &mut dyn FnMut(MoreItems) -> Result<u32, PreviewError>,
) -> Result<(), PreviewError> {
    let list = list_of(&more.link).ok_or_else(|| super::not_listable(&more.link))?;
    let cursor = more
        .cursor
        .clone()
        .ok_or_else(|| super::not_listable(&more.link))?;
    let mut pages = match &list {
        // A creator's pages are asked again, since new posts move them: the listing goes on from the page it stopped
        // before, or as far into the list as it had read when that page is gone.
        List::Creator(creator) => {
            let bodies = ask_all(&[format!(
                "https://api.fanbox.cc/post.paginateCreator?creatorId={creator}"
            )])?;
            let all = bodies.first().map(page_urls).unwrap_or_default();
            let from = all
                .iter()
                .position(|page| *page == cursor)
                .unwrap_or((more.read / PAGE) as usize);
            Pages::Known(all.into_iter().skip(from).collect())
        }
        List::Home | List::Supporting => Pages::Next(Some(cursor)),
    };
    let wanted = match read {
        MoreRead::New { count } => Some(count.clamp(1, NEW_COUNT_AT_MOST)),
        MoreRead::Next | MoreRead::All => None,
    };
    let mut from = more.clone();
    let mut found = 0;
    loop {
        let (videos, next) = read_pages(&mut pages, NEXT, cancel)?;
        let mut items = newest_first(videos);
        name_items(&mut items, &from.title);
        let next = next.map(|cursor| ListingMore {
            read: from.read + super::count_of(items.len()),
            cursor: Some(cursor),
            ..from.clone()
        });
        found += on_batch(MoreItems {
            items,
            more: next.clone(),
            total: None,
        })?;
        let enough = match (read, wanted) {
            (MoreRead::Next, _) => true,
            (_, Some(wanted)) => found >= wanted,
            (_, None) => false,
        };
        match next {
            Some(next) if !enough => from = next,
            _ => return Ok(()),
        }
    }
}

/// Where a list's next pages are.
enum Pages {
    /// A creator's: every page's address, the next first.
    Known(Vec<String>),
    /// A feed's: the next page's address, or `None` past its end.
    Next(Option<String>),
}

fn feed_first(list: &List) -> Option<String> {
    let feed = if *list == List::Home {
        "post.listHome"
    } else {
        "post.listSupporting"
    };
    Some(format!("https://api.fanbox.cc/{feed}?limit={PAGE}"))
}

/// Reads pages until `count` posts or the list's end, and gives them with the next page's address, if any.
fn read_pages(
    pages: &mut Pages,
    count: u32,
    cancel: &CancelToken,
) -> Result<(Vec<VideoInfo>, Option<String>), PreviewError> {
    let mut videos = Vec::new();
    match pages {
        Pages::Known(addresses) => {
            let take = (count.div_ceil(PAGE) as usize).min(addresses.len());
            let asked: Vec<String> = addresses.drain(..take).collect();
            check(cancel)?;
            for body in ask_all(&asked)? {
                videos.extend(posts_in(&body));
            }
            Ok((videos, addresses.first().cloned()))
        }
        Pages::Next(next) => {
            while let Some(address) = next.take() {
                check(cancel)?;
                let body = ask_all(std::slice::from_ref(&address))?
                    .pop()
                    .unwrap_or(Value::Null);
                videos.extend(posts_in(&body));
                *next = body
                    .get("nextUrl")
                    .and_then(Value::as_str)
                    .filter(|url| url.starts_with("https://api.fanbox.cc/"))
                    .map(str::to_owned);
                if super::count_of(videos.len()) >= count {
                    break;
                }
            }
            Ok((videos, next.clone()))
        }
    }
}

fn check(cancel: &CancelToken) -> Result<(), PreviewError> {
    if cancel.is_cancelled() {
        Err(PreviewError::Cancelled)
    } else {
        Ok(())
    }
}

/// The pages' addresses `post.paginateCreator` gives, only Fanbox's own.
fn page_urls(body: &Value) -> Vec<String> {
    let pages = body
        .get("pageUrls")
        .or(Some(body))
        .and_then(Value::as_array);
    pages
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|url| url.starts_with("https://api.fanbox.cc/"))
        .map(str::to_owned)
        .collect()
}

/// A page's posts: a creator's page holds them as `posts`, a feed's as `items`.
fn posts_in(body: &Value) -> Vec<VideoInfo> {
    let posts = body
        .get("posts")
        .or_else(|| body.get("items"))
        .or(Some(body))
        .and_then(Value::as_array);
    posts
        .into_iter()
        .flatten()
        .filter_map(listed_video)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(link: &str) -> Option<String> {
        let link = crate::metadata::normalize_link(link)?;
        super::super::listing_link(&link).map(|listing| listing.link)
    }

    #[test]
    fn knows_a_creators_pages_and_the_feeds() {
        for link in [
            "https://someone.fanbox.cc/",
            "https://someone.fanbox.cc/posts",
            "https://www.fanbox.cc/@someone",
            "https://www.fanbox.cc/@someone/posts",
        ] {
            assert_eq!(
                listing(link).as_deref(),
                Some("https://www.fanbox.cc/@someone/posts"),
                "{link}"
            );
        }
        assert_eq!(listing("https://www.fanbox.cc/home").as_deref(), Some(HOME));
        assert_eq!(
            listing("https://www.fanbox.cc/home/supporting").as_deref(),
            Some(SUPPORTING)
        );
        for link in [
            "https://someone.fanbox.cc/posts/12961757",
            "https://www.fanbox.cc/@someone/posts/1",
            "https://api.fanbox.cc/",
            "https://www.fanbox.cc/",
            "https://downloads.fanbox.cc/",
        ] {
            assert_eq!(listing(link), None, "{link}");
        }
    }

    #[test]
    fn reads_a_pages_posts_and_their_plans() {
        let page = serde_json::json!({"posts": [
            {"id": "2", "title": "Open", "creatorId": "someone", "feeRequired": 0, "isRestricted": false,
             "publishedDatetime": "2026-11-21T10:00:00+09:00", "user": {"name": "Some One"},
             "cover": {"type": "cover_image", "url": "https://pixiv.pximg.net/c/x.jpeg"}, "tags": []},
            {"id": "1", "title": "Supporters", "creatorId": "someone", "feeRequired": 500, "isRestricted": true,
             "publishedDatetime": "2026-11-20T10:00:00+09:00", "user": {"name": "Some One"}, "cover": null}
        ]});
        let videos = posts_in(&page);
        assert_eq!(videos.len(), 2);
        assert_eq!(videos[0].url, "https://www.fanbox.cc/@someone/posts/2");
        assert_eq!(videos[0].upload_date.as_deref(), Some("20261121"));
        assert_eq!(
            videos[0].thumbnail_url.as_deref(),
            Some("https://pixiv.pximg.net/c/x.jpeg")
        );
        assert_eq!(videos[0].uploader.as_deref(), Some("Some One"));
        assert_eq!(
            videos[1].fanbox,
            Some(crate::metadata::FanboxDetails {
                fee: 500,
                locked: true
            })
        );
        let feed = serde_json::json!({"items": [page["posts"][0].clone()], "nextUrl": null});
        assert_eq!(posts_in(&feed).len(), 1);
        assert_eq!(
            page_urls(
                &serde_json::json!({"pageUrls": ["https://api.fanbox.cc/post.listCreator?x", "https://example.com/"]})
            ),
            ["https://api.fanbox.cc/post.listCreator?x"]
        );
    }
}
