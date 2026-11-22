//! What a listing's picker loaded past its first items (Load More, Load New, Load All), kept in `tsundoku.db`
//! (`saved_listings`, `saved_listing_items`) with where its next items start, so opening the listing again goes on from
//! there instead of from its newest 50: an X account of thousands of posts takes hours to list, 15 minutes of waiting
//! at a time (`x_media`). The preview card reads the listing's newest items as always, then adds what was kept
//! (`with_saved`), unless the two don't meet anymore (`meets`). Kept for `KEEP_FOR` after the last load.

use std::collections::HashSet;
use std::sync::Arc;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{ListedItem, Listing, ListingKind, ListingMore, ListingPace, MoreItems};
use crate::database::Database;
use crate::history::HistoryError;
use crate::metadata::VideoInfo;

/// How long what a picker loaded is kept after its last load, in seconds: 30 days.
const KEEP_FOR: i64 = 30 * 24 * 60 * 60;

/// A listing without its items, as it's kept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Header {
    kind: ListingKind,
    title: String,
    uploader: Option<String>,
    site_name: String,
    url: String,
    thumbnail_url: Option<String>,
    media_total: Option<u32>,
    pace: Option<ListingPace>,
    newest_only: bool,
}

/// One item as it's kept: what the history and the queue have of it is asked again each time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedItem {
    video: VideoInfo,
    approximate_date: Option<String>,
}

/// The listings whose pickers loaded more, by their page (`ListingMore::link`).
pub struct SavedListings {
    database: Arc<Database>,
}

impl std::fmt::Debug for SavedListings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SavedListings")
            .finish_non_exhaustive()
    }
}

impl SavedListings {
    /// Opens them, forgetting those not loaded for `KEEP_FOR`.
    pub fn new(database: Arc<Database>, now: i64) -> Self {
        let listings = Self { database };
        // Left for the next start if it fails: `load` never answers a listing kept too long.
        let _ = listings.database.lock().execute(
            "DELETE FROM saved_listings WHERE saved_at < ?1",
            [now - KEEP_FOR],
        );
        listings
    }

    /// Keeps a listing as the preview card shows it, before its picker loads more of it: what it is, its items, and
    /// where its next ones start. Items kept before stay.
    pub fn start(&self, link: &str, listing: &Listing, now: i64) -> Result<(), HistoryError> {
        let header = Header {
            kind: listing.kind,
            title: listing.title.clone(),
            uploader: listing.uploader.clone(),
            site_name: listing.site_name.clone(),
            url: listing.url.clone(),
            thumbnail_url: listing.thumbnail_url.clone(),
            media_total: listing.media_total,
            pace: listing.pace,
            newest_only: listing.newest_only,
        };
        let mut connection = self.database.lock();
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO saved_listings (link, listing, more, total, saved_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (link) DO UPDATE SET
                 listing = excluded.listing, more = excluded.more, total = excluded.total,
                 saved_at = excluded.saved_at",
            params![
                link,
                json(&header)?,
                listing.more.as_ref().map(json).transpose()?,
                listing.total,
                now
            ],
        )?;
        add_items(&transaction, link, &listing.items)?;
        transaction.commit()?;
        Ok(())
    }

    /// Adds a batch the picker loaded to a listing kept with `start`, with where its next items start now. Nothing
    /// is kept for a listing never started.
    pub fn add(&self, link: &str, batch: &MoreItems, now: i64) -> Result<(), HistoryError> {
        let mut connection = self.database.lock();
        let transaction = connection.transaction()?;
        let kept = transaction.execute(
            "UPDATE saved_listings SET more = ?2, total = coalesce(?3, total), saved_at = ?4 WHERE link = ?1",
            params![
                link,
                batch.more.as_ref().map(json).transpose()?,
                batch.total,
                now
            ],
        )?;
        if kept > 0 {
            add_items(&transaction, link, &batch.items)?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// The listing as it was kept: its items in the order they were listed, none marked, and where its next ones
    /// start. `None` when nothing is kept for it, or was kept too long ago, which is then forgotten.
    pub fn load(&self, link: &str, now: i64) -> Result<Option<Listing>, HistoryError> {
        let connection = self.database.lock();
        let row = connection
            .query_row(
                "SELECT listing, more, total, saved_at FROM saved_listings WHERE link = ?1",
                [link],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<u32>>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((header, more, total, saved_at)) = row else {
            return Ok(None);
        };
        // One this version can't read (from a newer one) is as good as none.
        let header = serde_json::from_str::<Header>(&header).ok();
        let more = more.map(|more| serde_json::from_str::<ListingMore>(&more));
        let (Some(header), Ok(more)) = (header, more.transpose()) else {
            forget(&connection, link)?;
            return Ok(None);
        };
        if saved_at < now - KEEP_FOR {
            forget(&connection, link)?;
            return Ok(None);
        }
        let mut statement = connection
            .prepare("SELECT item FROM saved_listing_items WHERE link = ?1 ORDER BY id")?;
        let items = statement
            .query_map([link], |row| row.get::<_, String>(0))?
            .filter_map(|item| match item {
                Ok(item) => serde_json::from_str::<SavedItem>(&item).ok().map(Ok),
                Err(error) => Some(Err(error)),
            })
            .map(|item| {
                item.map(|item| ListedItem {
                    video: item.video,
                    approximate_date: item.approximate_date,
                    last_download: None,
                    in_queue: false,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut items: Vec<ListedItem> = items;
        if header.kind == ListingKind::Account {
            super::sort_newest_first(&mut items);
        }
        Ok(Some(Listing {
            kind: header.kind,
            title: header.title,
            uploader: header.uploader,
            site_name: header.site_name,
            url: header.url,
            thumbnail_url: header.thumbnail_url,
            total,
            media_total: header.media_total,
            pace: header.pace,
            newest_only: header.newest_only,
            items,
            more,
            also_read: Vec::new(),
        }))
    }

    /// Forgets what was kept for a listing.
    pub fn forget(&self, link: &str) -> Result<(), HistoryError> {
        forget(&self.database.lock(), link)
    }
}

fn forget(connection: &Connection, link: &str) -> Result<(), HistoryError> {
    connection.execute("DELETE FROM saved_listings WHERE link = ?1", [link])?;
    Ok(())
}

/// Keeps the items a listing doesn't have yet, after those it has.
fn add_items(
    connection: &Connection,
    link: &str,
    items: &[ListedItem],
) -> Result<(), HistoryError> {
    let mut insert = connection.prepare(
        "INSERT OR IGNORE INTO saved_listing_items (link, site, video_id, item) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for item in items {
        let saved = SavedItem {
            video: item.video.clone(),
            approximate_date: item.approximate_date.clone(),
        };
        insert.execute(params![
            link,
            item.video.site,
            item.video.video_id,
            json(&saved)?
        ])?;
    }
    Ok(())
}

fn json(value: &impl Serialize) -> Result<String, HistoryError> {
    serde_json::to_string(value).map_err(|error| HistoryError::Database {
        detail: error.to_string(),
    })
}

/// An item's site and ID, which tell a listing's items apart.
fn key(item: &ListedItem) -> (&str, &str) {
    (item.video.site.as_str(), item.video.video_id.as_str())
}

/// Whether what a listing's picker loaded before (`saved`) still goes with the listing just read (`fresh`, its newest
/// items). It doesn't when `fresh` is whole, so `saved` adds nothing, or when none of its items is among those kept:
/// more came since than the first read lists, so there'd be a gap between the two. What was kept is then forgotten.
pub fn meets(fresh: &Listing, saved: &Listing) -> bool {
    let listed: HashSet<_> = fresh.items.iter().map(key).collect();
    fresh.more.is_some() && saved.items.iter().any(|item| listed.contains(&key(item)))
}

/// A listing just read (`fresh`) with what its picker loaded before (`saved`, which `meets` it): the items `fresh`
/// doesn't have after its own, an account's by date, and the next ones starting where `saved` stopped.
pub fn with_saved(mut fresh: Listing, saved: Listing) -> Listing {
    let older: Vec<ListedItem> = {
        let listed: HashSet<_> = fresh.items.iter().map(key).collect();
        saved
            .items
            .into_iter()
            .filter(|item| !listed.contains(&key(item)))
            .collect()
    };
    fresh.items.extend(older);
    if fresh.kind == ListingKind::Account {
        super::sort_newest_first(&mut fresh.items);
    }
    fresh.more = saved.more;
    fresh.total = fresh.total.or(saved.total);
    fresh
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINK: &str = "https://x.com/someone/media";
    const NOW: i64 = 1_800_000_000;

    fn item(id: &str, date: &str) -> ListedItem {
        ListedItem {
            video: VideoInfo {
                site: "twitter".to_owned(),
                site_name: "x.com".to_owned(),
                video_id: id.to_owned(),
                url: format!("https://x.com/someone/status/{id}"),
                title: format!("Post {id}"),
                uploader: None,
                account: None,
                duration_seconds: None,
                upload_date: Some(date.to_owned()),
                thumbnail_url: None,
                best_height: None,
                post: None,
                files: None,
                post_files: Vec::new(),
                shared_files: Vec::new(),
                playlist: Some("someone".to_owned()),
                tags: Vec::new(),
                pixiv: None,
                fanbox: None,
            },
            approximate_date: None,
            last_download: None,
            in_queue: false,
        }
    }

    fn more(read: u32) -> ListingMore {
        ListingMore {
            link: LINK.to_owned(),
            title: "someone".to_owned(),
            read,
            cursor: Some(format!("page-after-{read}")),
            channel_id: None,
            media: None,
        }
    }

    fn listing(items: Vec<ListedItem>, more: Option<ListingMore>) -> Listing {
        Listing {
            kind: ListingKind::Account,
            title: "someone".to_owned(),
            uploader: None,
            site_name: "x.com".to_owned(),
            url: "https://x.com/someone".to_owned(),
            thumbnail_url: Some("https://pbs.twimg.com/picture.jpg".to_owned()),
            total: None,
            media_total: Some(372),
            pace: Some(ListingPace {
                items: 1_000,
                pause_seconds: 900,
            }),
            newest_only: true,
            items,
            more,
            also_read: Vec::new(),
        }
    }

    fn listings() -> SavedListings {
        SavedListings::new(Arc::new(Database::open_in_memory().unwrap()), NOW)
    }

    fn ids(listing: &Listing) -> Vec<&str> {
        listing
            .items
            .iter()
            .map(|item| item.video.video_id.as_str())
            .collect()
    }

    #[test]
    fn keeps_a_listing_and_what_its_picker_loads() {
        let saved = listings();
        assert_eq!(saved.load(LINK, NOW).unwrap(), None);

        let shown = listing(
            vec![item("9", "2026-11-09"), item("8", "2026-11-08")],
            Some(more(2)),
        );
        saved.start(LINK, &shown, NOW).unwrap();
        assert_eq!(saved.load(LINK, NOW).unwrap(), Some(shown.clone()));

        // A batch that starts with an item listed before, which is kept once.
        let batch = MoreItems {
            items: vec![item("8", "2026-11-08"), item("7", "2026-11-07")],
            more: Some(more(3)),
            total: None,
        };
        saved.add(LINK, &batch, NOW + 60).unwrap();
        let kept = saved.load(LINK, NOW + 60).unwrap().unwrap();
        assert_eq!(ids(&kept), ["9", "8", "7"]);
        assert_eq!(kept.more, Some(more(3)));
        assert_eq!(kept.media_total, Some(372));

        // The last batch: nothing more, and how many there are.
        let last = MoreItems {
            items: vec![item("6", "2026-11-06")],
            more: None,
            total: Some(4),
        };
        saved.add(LINK, &last, NOW + 120).unwrap();
        let kept = saved.load(LINK, NOW + 120).unwrap().unwrap();
        assert_eq!(
            (ids(&kept).len(), &kept.more, kept.total),
            (4, &None, Some(4))
        );
    }

    #[test]
    fn keeps_nothing_for_a_listing_never_started() {
        let saved = listings();
        let batch = MoreItems {
            items: vec![item("7", "2026-11-07")],
            more: Some(more(3)),
            total: None,
        };
        saved.add(LINK, &batch, NOW).unwrap();
        assert_eq!(saved.load(LINK, NOW).unwrap(), None);
    }

    #[test]
    fn forgets_what_was_kept_too_long_ago() {
        let database = Arc::new(Database::open_in_memory().unwrap());
        let saved = SavedListings::new(Arc::clone(&database), NOW);
        let shown = listing(vec![item("9", "2026-11-09")], Some(more(1)));
        saved.start(LINK, &shown, NOW).unwrap();
        assert!(saved.load(LINK, NOW + KEEP_FOR).unwrap().is_some());
        assert_eq!(saved.load(LINK, NOW + KEEP_FOR + 1).unwrap(), None);

        saved.start(LINK, &shown, NOW).unwrap();
        let count = |table: &str| {
            database
                .lock()
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap()
        };
        assert_eq!(
            (count("saved_listings"), count("saved_listing_items")),
            (1, 1)
        );
        // The next start forgets it, with its items.
        SavedListings::new(Arc::clone(&database), NOW + KEEP_FOR + 1);
        assert_eq!(
            (count("saved_listings"), count("saved_listing_items")),
            (0, 0)
        );
    }

    #[test]
    fn forgets_a_listing_when_told() {
        let saved = listings();
        saved
            .start(
                LINK,
                &listing(vec![item("9", "2026-11-09")], Some(more(1))),
                NOW,
            )
            .unwrap();
        saved.forget(LINK).unwrap();
        assert_eq!(saved.load(LINK, NOW).unwrap(), None);
    }

    #[test]
    fn adds_what_was_kept_to_the_listing_just_read() {
        // A new post came since: it leads, and the older ones kept follow by date.
        let fresh = listing(
            vec![
                item("10", "2026-11-10"),
                item("9", "2026-11-09"),
                item("8", "2026-11-08"),
            ],
            Some(more(3)),
        );
        let kept = listing(
            vec![
                item("9", "2026-11-09"),
                item("8", "2026-11-08"),
                item("7", "2026-11-07"),
                item("6", "2026-11-06"),
            ],
            Some(more(4)),
        );
        assert!(meets(&fresh, &kept));
        let merged = with_saved(fresh, kept);
        assert_eq!(ids(&merged), ["10", "9", "8", "7", "6"]);
        assert_eq!(
            merged.more,
            Some(more(4)),
            "the next ones start where the picker stopped"
        );

        // Kept whole: nothing more to load.
        let fresh = listing(vec![item("9", "2026-11-09")], Some(more(1)));
        let mut kept = listing(vec![item("9", "2026-11-09"), item("8", "2026-11-08")], None);
        kept.total = Some(2);
        assert!(meets(&fresh, &kept));
        let merged = with_saved(fresh, kept);
        assert_eq!(
            (ids(&merged), &merged.more, merged.total),
            (vec!["9", "8"], &None, Some(2))
        );
    }

    #[test]
    fn leaves_out_what_was_kept_when_it_no_longer_meets_the_listing() {
        // Everything listed is newer than what was kept: posts between the two may be missing.
        let fresh = listing(vec![item("20", "2026-11-20")], Some(more(1)));
        let kept = listing(vec![item("9", "2026-11-09")], Some(more(4)));
        assert!(!meets(&fresh, &kept));

        // The listing is whole as read.
        let whole = listing(vec![item("9", "2026-11-09")], None);
        assert!(!meets(&whole, &kept));
    }

    #[test]
    fn a_channels_items_keep_their_order() {
        let mut fresh = listing(
            vec![item("b", "2026-11-01"), item("a", "2026-11-05")],
            Some(more(2)),
        );
        fresh.kind = ListingKind::Channel;
        let mut kept = listing(
            vec![item("a", "2026-11-05"), item("c", "2026-11-09")],
            Some(more(3)),
        );
        kept.kind = ListingKind::Channel;
        let merged = with_saved(fresh, kept);
        assert_eq!(
            ids(&merged),
            ["b", "a", "c"],
            "as listed, whatever their dates"
        );
    }
}
