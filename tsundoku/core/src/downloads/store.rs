//! The queue in `tsundoku.db` (`queue_items`), so it comes back after a restart.

use std::sync::Arc;

use rusqlite::params;

use super::{DownloadError, QueueItem, QueueState, Recovery};
use crate::database::Database;

pub(crate) struct QueueStore {
    database: Arc<Database>,
}

impl QueueStore {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    /// Every item, in the queue's order: those Download Next moved, the latest moved first, then the rest oldest first.
    pub fn load(&self) -> Result<Vec<QueueItem>, DownloadError> {
        let connection = self.database.lock();
        let mut statement = connection
            .prepare(
                "SELECT id, video, preset, folder, state, overwrite, added_at, result, error, folder_chosen, file_name
                FROM queue_items ORDER BY position, id",
            )
            .map_err(storage_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, bool>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, bool>(9)?,
                    row.get::<_, Option<String>>(10)?,
                ))
            })
            .map_err(storage_error)?;

        let mut items = Vec::new();
        for row in rows {
            let (
                id,
                video,
                preset,
                folder,
                state,
                overwrite,
                added_at,
                result,
                error,
                folder_chosen,
                file_name,
            ) = row.map_err(storage_error)?;
            // An item that can't be read (from a newer version, say) is skipped rather than blocking the queue.
            let (Ok(video), Ok(preset), Some(state)) = (
                serde_json::from_str(&video),
                serde_json::from_str(&preset),
                state_from_name(&state),
            ) else {
                continue;
            };
            items.push(QueueItem {
                id,
                video,
                preset,
                folder,
                folder_chosen,
                file_name,
                state,
                overwrite,
                added_at,
                result: result.and_then(|json| serde_json::from_str(&json).ok()),
                error: error.and_then(|json| serde_json::from_str(&json).ok()),
                progress: None,
                hold: None,
                recovery: Recovery::default(),
            });
        }
        Ok(items)
    }

    /// Adds an item and returns its ID. The `id` given is ignored.
    pub fn insert(&self, item: &QueueItem) -> Result<i64, DownloadError> {
        self.insert_row(item, None)
    }

    /// Undo for Clear Finished: puts back an item `take` deleted, with its ID and at the position `take` gave.
    pub fn restore(&self, item: &QueueItem, position: i64) -> Result<(), DownloadError> {
        self.insert_row(item, Some(position)).map(|_| ())
    }

    /// A new item gets a new ID at position 0; one put back keeps its ID and its position.
    fn insert_row(&self, item: &QueueItem, put_back_at: Option<i64>) -> Result<i64, DownloadError> {
        let connection = self.database.lock();
        connection
            .query_row(
                "INSERT INTO queue_items
                    (id, video, preset, folder, state, overwrite, added_at, result, error, position, folder_chosen,
                    file_name)
                VALUES (?9, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?10, ?11, ?12) RETURNING id",
                params![
                    to_json(&item.video),
                    to_json(&item.preset),
                    item.folder,
                    state_name(item.state),
                    item.overwrite,
                    item.added_at,
                    item.result.as_ref().map(to_json),
                    item.error.as_ref().map(to_json),
                    put_back_at.map(|_| item.id),
                    put_back_at.unwrap_or(0),
                    item.folder_chosen,
                    item.file_name,
                ],
                |row| row.get(0),
            )
            .map_err(storage_error)
    }

    /// Saves an item's state, result and error.
    pub fn update(&self, item: &QueueItem) -> Result<(), DownloadError> {
        let connection = self.database.lock();
        connection
            .execute(
                "UPDATE queue_items SET state = ?2, result = ?3, error = ?4 WHERE id = ?1",
                params![
                    item.id,
                    state_name(item.state),
                    item.result.as_ref().map(to_json),
                    item.error.as_ref().map(to_json),
                ],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    /// Saves an item's folder and whether it was picked for it.
    pub fn set_folder(&self, item: &QueueItem) -> Result<(), DownloadError> {
        self.database
            .lock()
            .execute(
                "UPDATE queue_items SET folder = ?2, folder_chosen = ?3 WHERE id = ?1",
                params![item.id, item.folder, item.folder_chosen],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    /// Download Next: puts the item before every other. Items are added at position 0, and a moved one takes one less
    /// than the lowest, so the latest moved comes first.
    pub fn move_to_front(&self, id: i64) -> Result<(), DownloadError> {
        self.database
            .lock()
            .execute(
                "UPDATE queue_items SET position = (SELECT MIN(position) FROM queue_items) - 1 WHERE id = ?1",
                [id],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<(), DownloadError> {
        self.database
            .lock()
            .execute("DELETE FROM queue_items WHERE id = ?1", [id])
            .map_err(storage_error)?;
        Ok(())
    }

    /// Deletes an item for Clear Finished, and returns its position in the queue's order for Undo (`restore`): 0
    /// when it was already gone.
    pub fn take(&self, id: i64) -> Result<i64, DownloadError> {
        use rusqlite::OptionalExtension;
        let position: Option<i64> = self
            .database
            .lock()
            .query_row(
                "DELETE FROM queue_items WHERE id = ?1 RETURNING position",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)?;
        Ok(position.unwrap_or(0))
    }

    #[cfg(test)]
    pub fn state_of(&self, id: i64) -> Option<QueueState> {
        use rusqlite::OptionalExtension;
        let connection = self.database.lock();
        let name: Option<String> = connection
            .query_row("SELECT state FROM queue_items WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .optional()
            .ok()?;
        state_from_name(&name?)
    }
}

fn state_name(state: QueueState) -> &'static str {
    match state {
        QueueState::Waiting => "waiting",
        QueueState::Running => "running",
        QueueState::Stopped => "stopped",
        QueueState::Done => "done",
        QueueState::Failed => "failed",
    }
}

fn state_from_name(name: &str) -> Option<QueueState> {
    Some(match name {
        "waiting" => QueueState::Waiting,
        "running" => QueueState::Running,
        "stopped" => QueueState::Stopped,
        "done" => QueueState::Done,
        "failed" => QueueState::Failed,
        _ => return None,
    })
}

fn to_json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).expect("queue values always serialize to JSON")
}

fn storage_error(error: rusqlite::Error) -> DownloadError {
    DownloadError::Storage {
        detail: error.to_string(),
    }
}
