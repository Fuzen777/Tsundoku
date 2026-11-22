//! The watched sources and their items in `tsundoku.db` (`watched_sources`, `watched_items`).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::picture::SourcePicture;
use super::{ItemKey, NewItemsStart, WatchedError};
use crate::database::Database;
use crate::listing::ListingKind;
use crate::metadata::{PreviewError, VideoInfo};
use crate::rules::RuleUnpacking;

/// Where an item of a source stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemState {
    /// Found by a check and waiting on the source's card.
    New,
    /// Left out of a download, or skipped: listed apart, and never new again.
    Skipped,
    /// Sent to the queue.
    Queued,
    /// Known without being shown: downloaded or queued before the source was added, or older than what was asked for
    /// when it was.
    Seen,
}

impl ItemState {
    fn name(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Skipped => "skipped",
            Self::Queued => "queued",
            Self::Seen => "seen",
        }
    }

    /// A state this version doesn't know (from a newer one) is taken as seen, so the item never shows as new again.
    fn from_name(name: &str) -> Self {
        match name {
            "new" => Self::New,
            "skipped" => Self::Skipped,
            "queued" => Self::Queued,
            _ => Self::Seen,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct StoredSource {
    pub id: i64,
    pub url: String,
    pub kind: ListingKind,
    pub name: String,
    pub site_name: String,
    pub thumbnail_url: Option<String>,
    pub picture: Option<SourcePicture>,
    pub folder: Option<String>,
    pub preset_id: Option<String>,
    pub new_items_start: Option<NewItemsStart>,
    pub unpack: Option<RuleUnpacking>,
    pub added_at: i64,
    pub last_checked_at: Option<i64>,
    pub last_error: Option<PreviewError>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct StoredItem {
    pub video: VideoInfo,
    pub approximate_date: Option<String>,
    pub state: ItemState,
    pub first_seen_at: i64,
    /// Its place among the items a check or the adding found at once: 0 for the first listed. Saved below every item
    /// the source has, so the latest find comes first.
    pub position: i64,
}

pub(super) struct WatchedStore {
    database: Arc<Database>,
}

const SOURCE_COLUMNS: &str = "id, url, kind, name, site_name, thumbnail_url, folder, preset, added_at, last_checked_at, last_error, picture, new_items_start, unpack";

impl WatchedStore {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    /// Every source, in the order added.
    pub fn sources(&self) -> Result<Vec<StoredSource>, WatchedError> {
        let connection = self.database.lock();
        let mut statement = connection
            .prepare(&format!(
                "SELECT {SOURCE_COLUMNS} FROM watched_sources ORDER BY id"
            ))
            .map_err(storage_error)?;
        let rows = statement
            .query_map([], source_from_row)
            .map_err(storage_error)?;
        rows.collect::<Result<_, _>>().map_err(storage_error)
    }

    pub fn source(&self, id: i64) -> Result<Option<StoredSource>, WatchedError> {
        let connection = self.database.lock();
        connection
            .query_row(
                &format!("SELECT {SOURCE_COLUMNS} FROM watched_sources WHERE id = ?1"),
                [id],
                source_from_row,
            )
            .optional()
            .map_err(storage_error)
    }

    pub fn find_by_url(&self, url: &str) -> Result<Option<i64>, WatchedError> {
        let connection = self.database.lock();
        connection
            .query_row(
                "SELECT id FROM watched_sources WHERE url = ?1",
                [url],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)
    }

    /// The Unpack Archives of the source whose listing's page is `url`, or `None` when it has none or no source has
    /// that page.
    pub fn unpacking_of(&self, url: &str) -> Result<Option<RuleUnpacking>, WatchedError> {
        let connection = self.database.lock();
        let name: Option<Option<String>> = connection
            .query_row(
                "SELECT unpack FROM watched_sources WHERE url = ?1",
                [url],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)?;
        Ok(name.flatten().as_deref().and_then(unpacking_from_name))
    }

    /// A source's items, latest found first, each find in the listing's order.
    pub fn items(&self, source: i64) -> Result<Vec<StoredItem>, WatchedError> {
        let connection = self.database.lock();
        let mut statement = connection
            .prepare(
                "SELECT video, approximate_date, state, first_seen_at, position FROM watched_items
                WHERE source = ?1 ORDER BY position",
            )
            .map_err(storage_error)?;
        let rows = statement
            .query_map([source], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .map_err(storage_error)?;
        let mut items = Vec::new();
        for row in rows {
            let (video, approximate_date, state, first_seen_at, position) =
                row.map_err(storage_error)?;
            // An item whose video can't be read back (from a newer version) is left out.
            let Some(video) = from_json::<VideoInfo>(&video) else {
                continue;
            };
            items.push(StoredItem {
                video,
                approximate_date,
                state: ItemState::from_name(&state),
                first_seen_at,
                position,
            });
        }
        Ok(items)
    }

    /// How many items of each source the history has, by source ID; a source with none is left out. The history's
    /// tables are in the same database, and a video counts as the history view counts it: with a download.
    pub fn downloaded_counts(&self) -> Result<HashMap<i64, u32>, WatchedError> {
        let connection = self.database.lock();
        let mut statement = connection
            .prepare(
                "SELECT w.source, COUNT(*) FROM watched_items w
                WHERE EXISTS (SELECT 1 FROM videos v JOIN downloads d ON d.video = v.id
                    WHERE v.site = w.site AND v.video_id = w.item_id)
                GROUP BY w.source",
            )
            .map_err(storage_error)?;
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, u32>(1)?)))
            .map_err(storage_error)?;
        rows.collect::<Result<_, _>>().map_err(storage_error)
    }

    /// Saves a new source with its first items, in one transaction. Its ID.
    pub fn insert_source(
        &self,
        source: &StoredSource,
        items: &[StoredItem],
    ) -> Result<i64, WatchedError> {
        let mut connection = self.database.lock();
        let transaction = connection.transaction().map_err(storage_error)?;
        transaction
            .execute(
                "INSERT INTO watched_sources (url, kind, name, site_name, thumbnail_url, folder, preset, added_at,
                    last_checked_at, last_error, new_items_start, unpack)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10, ?11)",
                params![
                    source.url,
                    kind_name(source.kind),
                    source.name,
                    source.site_name,
                    source.thumbnail_url,
                    source.folder,
                    source.preset_id,
                    source.added_at,
                    source.last_checked_at,
                    source.new_items_start.map(NewItemsStart::name),
                    source.unpack.map(unpacking_name),
                ],
            )
            .map_err(storage_error)?;
        let id = transaction.last_insert_rowid();
        insert_items_into(&transaction, id, items)?;
        transaction.commit().map_err(storage_error)?;
        Ok(id)
    }

    /// Saves the new items a check found, with the source's name and picture as the listing has them now.
    pub fn insert_items(
        &self,
        source: i64,
        items: &[StoredItem],
        name: &str,
        thumbnail_url: Option<&str>,
    ) -> Result<(), WatchedError> {
        let mut connection = self.database.lock();
        let transaction = connection.transaction().map_err(storage_error)?;
        insert_items_into(&transaction, source, items)?;
        transaction
            .execute(
                "UPDATE watched_sources SET name = ?2, thumbnail_url = COALESCE(?3, thumbnail_url) WHERE id = ?1",
                params![source, name, thumbnail_url],
            )
            .map_err(storage_error)?;
        transaction.commit().map_err(storage_error)
    }

    /// Gives the source's items the pictures a check read for them, where they had none
    /// (`VideoInfo::take_missing_pictures`). Items the source doesn't have are passed over.
    pub fn add_pictures(&self, source: i64, read: &[&VideoInfo]) -> Result<(), WatchedError> {
        let mut connection = self.database.lock();
        let transaction = connection.transaction().map_err(storage_error)?;
        for video in read {
            let kept: Option<String> = transaction
                .query_row(
                    "SELECT video FROM watched_items WHERE source = ?1 AND site = ?2 AND item_id = ?3",
                    params![source, video.site, video.video_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(storage_error)?;
            let Some(mut kept) = kept.and_then(|json| from_json::<VideoInfo>(&json)) else {
                continue;
            };
            if !kept.take_missing_pictures(video) {
                continue;
            }
            transaction
                .execute(
                    "UPDATE watched_items SET video = ?4 WHERE source = ?1 AND site = ?2 AND item_id = ?3",
                    params![source, video.site, video.video_id, to_json(&kept)],
                )
                .map_err(storage_error)?;
        }
        transaction.commit().map_err(storage_error)
    }

    /// Records a check: when, and its error or none.
    pub fn set_checked(
        &self,
        source: i64,
        at: i64,
        error: Option<&PreviewError>,
    ) -> Result<(), WatchedError> {
        let connection = self.database.lock();
        connection
            .execute(
                "UPDATE watched_sources SET last_checked_at = ?2, last_error = ?3 WHERE id = ?1",
                params![source, at, error.map(to_json)],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    pub fn set_choices(
        &self,
        source: i64,
        folder: Option<String>,
        preset_id: Option<String>,
        new_items_start: Option<NewItemsStart>,
        unpack: Option<RuleUnpacking>,
    ) -> Result<(), WatchedError> {
        let connection = self.database.lock();
        let changed = connection
            .execute(
                "UPDATE watched_sources SET folder = ?2, preset = ?3, new_items_start = ?4, unpack = ?5 WHERE id = ?1",
                params![
                    source,
                    folder,
                    preset_id,
                    new_items_start.map(NewItemsStart::name),
                    unpack.map(unpacking_name),
                ],
            )
            .map_err(storage_error)?;
        if changed == 0 {
            return Err(WatchedError::NotFound);
        }
        Ok(())
    }

    /// Sets the picture picked for a source, or none for its listing's.
    pub fn set_picture(
        &self,
        source: i64,
        picture: Option<&SourcePicture>,
    ) -> Result<(), WatchedError> {
        let connection = self.database.lock();
        let changed = connection
            .execute(
                "UPDATE watched_sources SET picture = ?2 WHERE id = ?1",
                params![source, picture.map(to_json)],
            )
            .map_err(storage_error)?;
        if changed == 0 {
            return Err(WatchedError::NotFound);
        }
        Ok(())
    }

    /// The files of the pictures picked for the sources.
    pub fn picture_files(&self) -> Result<HashSet<String>, WatchedError> {
        Ok(self
            .sources()?
            .into_iter()
            .filter_map(|source| source.picture)
            .map(|picture| picture.file)
            .collect())
    }

    pub fn set_state(
        &self,
        source: i64,
        keys: &[ItemKey],
        state: ItemState,
    ) -> Result<(), WatchedError> {
        let mut connection = self.database.lock();
        let transaction = connection.transaction().map_err(storage_error)?;
        for key in keys {
            transaction
                .execute(
                    "UPDATE watched_items SET state = ?4 WHERE source = ?1 AND site = ?2 AND item_id = ?3",
                    params![source, key.site, key.video_id, state.name()],
                )
                .map_err(storage_error)?;
        }
        transaction.commit().map_err(storage_error)
    }

    /// Forgets a source and its items.
    pub fn delete_source(&self, source: i64) -> Result<(), WatchedError> {
        let connection = self.database.lock();
        let deleted = connection
            .execute("DELETE FROM watched_sources WHERE id = ?1", [source])
            .map_err(storage_error)?;
        if deleted == 0 {
            return Err(WatchedError::NotFound);
        }
        Ok(())
    }
}

/// Saves items, leaving any the source already has as they are, placed above the items it has, in their order.
fn insert_items_into(
    connection: &Connection,
    source: i64,
    items: &[StoredItem],
) -> Result<(), WatchedError> {
    let lowest: i64 = connection
        .query_row(
            "SELECT COALESCE(MIN(position), 0) FROM watched_items WHERE source = ?1",
            [source],
            |row| row.get(0),
        )
        .map_err(storage_error)?;
    let first = lowest - i64::try_from(items.len()).unwrap_or(i64::MAX);
    for item in items {
        connection
            .execute(
                "INSERT OR IGNORE INTO watched_items
                    (source, site, item_id, video, approximate_date, state, first_seen_at, position)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    source,
                    item.video.site,
                    item.video.video_id,
                    to_json(&item.video),
                    item.approximate_date,
                    item.state.name(),
                    item.first_seen_at,
                    first + item.position,
                ],
            )
            .map_err(storage_error)?;
    }
    Ok(())
}

fn source_from_row(row: &Row<'_>) -> rusqlite::Result<StoredSource> {
    Ok(StoredSource {
        id: row.get(0)?,
        url: row.get(1)?,
        kind: kind_from_name(&row.get::<_, String>(2)?),
        name: row.get(3)?,
        site_name: row.get(4)?,
        thumbnail_url: row.get(5)?,
        folder: row.get(6)?,
        preset_id: row.get(7)?,
        added_at: row.get(8)?,
        last_checked_at: row.get(9)?,
        last_error: row
            .get::<_, Option<String>>(10)?
            .and_then(|json| from_json(&json)),
        // One that can't be read back (from a newer version) is left out: the listing's picture shows.
        picture: row
            .get::<_, Option<String>>(11)?
            .and_then(|json| from_json(&json)),
        new_items_start: row
            .get::<_, Option<String>>(12)?
            .as_deref()
            .and_then(NewItemsStart::from_name),
        unpack: row
            .get::<_, Option<String>>(13)?
            .as_deref()
            .and_then(unpacking_from_name),
    })
}

/// A source's Unpack Archives as `watched_sources.unpack` holds it.
fn unpacking_name(unpacking: RuleUnpacking) -> &'static str {
    match unpacking {
        RuleUnpacking::Off => "off",
        RuleUnpacking::Smart => "smart",
        RuleUnpacking::ArchiveFolder => "archiveFolder",
        RuleUnpacking::InPlace => "inPlace",
    }
}

/// `None` for a name this version doesn't know (from a newer one): the rules and Settings decide.
fn unpacking_from_name(name: &str) -> Option<RuleUnpacking> {
    match name {
        "off" => Some(RuleUnpacking::Off),
        "smart" => Some(RuleUnpacking::Smart),
        "archiveFolder" => Some(RuleUnpacking::ArchiveFolder),
        "inPlace" => Some(RuleUnpacking::InPlace),
        _ => None,
    }
}

fn kind_name(kind: ListingKind) -> &'static str {
    match kind {
        ListingKind::Channel => "channel",
        ListingKind::Playlist => "playlist",
        ListingKind::Account => "account",
    }
}

fn kind_from_name(name: &str) -> ListingKind {
    match name {
        "channel" => ListingKind::Channel,
        "account" => ListingKind::Account,
        _ => ListingKind::Playlist,
    }
}

fn to_json(value: &impl Serialize) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

fn from_json<T: DeserializeOwned>(json: &str) -> Option<T> {
    serde_json::from_str(json).ok()
}

fn storage_error(error: rusqlite::Error) -> WatchedError {
    WatchedError::Storage {
        detail: error.to_string(),
    }
}
