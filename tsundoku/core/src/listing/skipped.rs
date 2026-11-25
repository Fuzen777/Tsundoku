//! The items of a channel's, playlist's or account's list left unticked when its picker downloaded others (Download,
//! Download and Load More), kept in `tsundoku.db` (`skipped_items`, migration 26) by the listing's page (`Listing::url`,
//! as `QueueSource::url` names it), so the picker opens with them unticked, marked Skipped, as a watched source keeps
//! its Skipped Before. An item picked since leaves the list; one downloaded since shows as downloaded anyway.

use std::sync::Arc;

use rusqlite::params;

use crate::database::Database;
use crate::history::HistoryError;
use crate::watched::ItemKey;

/// The skipped items of every listing, by its page.
pub struct SkippedItems {
    database: Arc<Database>,
}

impl std::fmt::Debug for SkippedItems {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SkippedItems")
            .finish_non_exhaustive()
    }
}

impl SkippedItems {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    /// The listing at `listing` was downloaded from: `skipped` were left unticked, `picked` were taken (and leave the
    /// list if they were on it), in one go.
    pub fn record(
        &self,
        listing: &str,
        skipped: &[ItemKey],
        picked: &[ItemKey],
        now: i64,
    ) -> Result<(), HistoryError> {
        let mut connection = self.database.lock();
        let transaction = connection.transaction()?;
        for key in picked {
            transaction.execute(
                "DELETE FROM skipped_items WHERE listing = ?1 AND site = ?2 AND video_id = ?3",
                params![listing, key.site, key.video_id],
            )?;
        }
        for key in skipped {
            transaction.execute(
                "INSERT INTO skipped_items (listing, site, video_id, skipped_at) VALUES (?1, ?2, ?3, ?4)
                    ON CONFLICT (listing, site, video_id) DO UPDATE SET skipped_at = excluded.skipped_at",
                params![listing, key.site, key.video_id, now],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// The items skipped in the listing at `listing`, the latest first.
    pub fn of(&self, listing: &str) -> Result<Vec<ItemKey>, HistoryError> {
        let connection = self.database.lock();
        let mut statement = connection.prepare(
            "SELECT site, video_id FROM skipped_items WHERE listing = ?1 ORDER BY skipped_at DESC, rowid DESC",
        )?;
        let keys = statement
            .query_map([listing], |row| {
                Ok(ItemKey {
                    site: row.get(0)?,
                    video_id: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHANNEL: &str = "https://www.youtube.com/@someone/videos";
    const OTHER: &str = "https://www.youtube.com/@another/videos";

    fn key(id: &str) -> ItemKey {
        ItemKey {
            site: "youtube".to_owned(),
            video_id: id.to_owned(),
        }
    }

    #[test]
    fn keeps_what_a_listing_skipped_until_it_is_picked() {
        let skipped = SkippedItems::new(Arc::new(Database::open_in_memory().unwrap()));
        assert!(skipped.of(CHANNEL).unwrap().is_empty());

        skipped
            .record(CHANNEL, &[key("a"), key("b")], &[key("c")], 10)
            .unwrap();
        skipped.record(OTHER, &[key("z")], &[], 10).unwrap();
        assert_eq!(skipped.of(CHANNEL).unwrap().len(), 2);
        assert_eq!(
            skipped.of(OTHER).unwrap(),
            [key("z")],
            "each listing its own"
        );

        skipped
            .record(CHANNEL, &[key("d"), key("a")], &[key("b")], 20)
            .unwrap();
        let now = skipped.of(CHANNEL).unwrap();
        assert_eq!(now.len(), 2, "b was picked; a skipped again is kept once");
        assert!(now.contains(&key("a")) && now.contains(&key("d")));
        assert!(!now.contains(&key("b")));
    }
}
