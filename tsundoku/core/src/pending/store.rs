//! The pending links in `tsundoku.db` (`pending_links`), so they come back after a restart.

use std::sync::Arc;

use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{Connection, params};

use super::{Entry, LinkOrigin, PendingState, StartWhenRead};
use crate::database::Database;
use crate::downloads::DownloadError;
use crate::metadata::Fetched;

pub(crate) struct PendingStore {
    database: Arc<Database>,
}

impl PendingStore {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    /// Every link, oldest first. One that was being read comes back waiting.
    pub fn load(&self) -> Result<Vec<Entry>, DownloadError> {
        let connection = self.database.lock();
        let mut statement = connection
            .prepare(
                "SELECT id, link, state, video, missing_js_runtime, error, preset, start_when_read, added_at, caught,
                    folder
                FROM pending_links ORDER BY id",
            )
            .map_err(storage_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, bool>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, StartWhenRead>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, LinkOrigin>(9)?,
                    row.get::<_, Option<String>>(10)?,
                ))
            })
            .map_err(storage_error)?;

        let mut entries = Vec::new();
        for row in rows {
            let (
                id,
                link,
                state,
                video,
                missing_js_runtime,
                error,
                preset_id,
                start_when_read,
                added_at,
                origin,
                folder,
            ) = row.map_err(storage_error)?;
            let fetched = video
                .and_then(|json| serde_json::from_str(&json).ok())
                .map(|video| Fetched {
                    video,
                    missing_js_runtime,
                });
            let error = error.and_then(|json| serde_json::from_str(&json).ok());
            // A link whose details can't be read back (from a newer version, say) is read again.
            let state = match state_from_name(&state) {
                Some(PendingState::Ready) if fetched.is_some() => PendingState::Ready,
                Some(PendingState::Failed) if error.is_some() => PendingState::Failed,
                _ => PendingState::Waiting,
            };
            entries.push(Entry {
                id,
                link,
                state,
                fetched,
                error,
                preset_id,
                folder,
                start_when_read,
                added_at,
                origin,
            });
        }
        Ok(entries)
    }

    /// Adds a link and returns its ID: a new one when `id` is 0, else `id`, for Undo putting back a removed link.
    pub fn insert(&self, entry: &Entry) -> Result<i64, DownloadError> {
        insert_into(&self.database.lock(), entry)
    }

    /// Adds new links in one transaction, since a dropped file can hold thousands, and gives each its ID.
    pub fn insert_all(&self, entries: &mut [Entry]) -> Result<(), DownloadError> {
        if entries.is_empty() {
            return Ok(());
        }
        let mut connection = self.database.lock();
        let transaction = connection.transaction().map_err(storage_error)?;
        for entry in entries.iter_mut() {
            entry.id = insert_into(&transaction, entry)?;
        }
        transaction.commit().map_err(storage_error)
    }

    /// Saves everything about a link but its address and when it was added.
    pub fn update(&self, entry: &Entry) -> Result<(), DownloadError> {
        let connection = self.database.lock();
        connection
            .execute(
                "UPDATE pending_links SET state = ?2, video = ?3, missing_js_runtime = ?4, error = ?5, preset = ?6,
                    start_when_read = ?7, folder = ?8
                WHERE id = ?1",
                params![
                    entry.id,
                    state_name(entry.state),
                    video_json(entry),
                    missing_js_runtime(entry),
                    entry.error.as_ref().map(to_json),
                    entry.preset_id,
                    entry.start_when_read,
                    entry.folder,
                ],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<(), DownloadError> {
        self.database
            .lock()
            .execute("DELETE FROM pending_links WHERE id = ?1", [id])
            .map_err(storage_error)?;
        Ok(())
    }
}

fn insert_into(connection: &Connection, entry: &Entry) -> Result<i64, DownloadError> {
    connection
        .query_row(
            "INSERT INTO pending_links
                (id, link, state, video, missing_js_runtime, error, preset, start_when_read, added_at, caught, folder)
            VALUES (?10, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?11) RETURNING id",
            params![
                entry.link,
                state_name(entry.state),
                video_json(entry),
                missing_js_runtime(entry),
                entry.error.as_ref().map(to_json),
                entry.preset_id,
                entry.start_when_read,
                entry.added_at,
                entry.origin,
                (entry.id != 0).then_some(entry.id),
                entry.folder,
            ],
            |row| row.get(0),
        )
        .map_err(storage_error)
}

/// A link being read is saved as waiting, so it's read again after a restart.
fn state_name(state: PendingState) -> &'static str {
    match state {
        PendingState::Waiting | PendingState::Reading => "waiting",
        PendingState::Ready => "ready",
        PendingState::Failed => "failed",
    }
}

/// 0 waits, 1 was started before it was read, 2 was sent and follows Download Right Away. The column was a yes or no
/// before 0.1.35, so older versions read 2 as started.
impl ToSql for StartWhenRead {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(match self {
            Self::No => 0_i64,
            Self::Yes => 1,
            Self::IfRightAway => 2,
        }))
    }
}

impl FromSql for StartWhenRead {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Ok(match i64::column_result(value)? {
            1 => Self::Yes,
            2 => Self::IfRightAway,
            _ => Self::No,
        })
    }
}

/// Where a link came from, in the `caught` column: 0 added by hand, 1 copied, 2 from a file. The column was a yes or
/// no before 0.1.54, so older versions read a link from a file as copied.
impl ToSql for LinkOrigin {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(match self {
            Self::Added => 0_i64,
            Self::Copied => 1,
            Self::File => 2,
        }))
    }
}

impl FromSql for LinkOrigin {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Ok(match i64::column_result(value)? {
            0 => Self::Added,
            2 => Self::File,
            _ => Self::Copied,
        })
    }
}

fn state_from_name(name: &str) -> Option<PendingState> {
    Some(match name {
        "waiting" => PendingState::Waiting,
        "ready" => PendingState::Ready,
        "failed" => PendingState::Failed,
        _ => return None,
    })
}

fn video_json(entry: &Entry) -> Option<String> {
    entry
        .fetched
        .as_ref()
        .map(|fetched| to_json(&fetched.video))
}

fn missing_js_runtime(entry: &Entry) -> bool {
    entry
        .fetched
        .as_ref()
        .is_some_and(|fetched| fetched.missing_js_runtime)
}

fn to_json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).expect("pending values always serialize to JSON")
}

fn storage_error(error: rusqlite::Error) -> DownloadError {
    DownloadError::Storage {
        detail: error.to_string(),
    }
}
