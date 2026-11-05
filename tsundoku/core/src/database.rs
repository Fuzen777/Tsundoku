//! `tsundoku.db`: the SQLite database the history and the download queue share, and its migrations.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use crate::history::HistoryError;
use crate::sync::lock;

/// Each entry upgrades the database by one version, recorded in `PRAGMA user_version`. Never change an entry once
/// released; add one.
const MIGRATIONS: &[&str] = &[
    // 1: videos, and one row per finished download of each (history.rs).
    "CREATE TABLE videos (
        id INTEGER PRIMARY KEY,
        site TEXT NOT NULL,
        site_name TEXT NOT NULL,
        video_id TEXT NOT NULL,
        url TEXT NOT NULL,
        title TEXT NOT NULL,
        uploader TEXT,
        duration_seconds REAL,
        upload_date TEXT,
        thumbnail_url TEXT,
        best_height INTEGER,
        first_downloaded_at INTEGER NOT NULL,
        UNIQUE (site, video_id)
    );
    CREATE TABLE downloads (
        id INTEGER PRIMARY KEY,
        video INTEGER NOT NULL REFERENCES videos (id) ON DELETE CASCADE,
        preset TEXT NOT NULL,
        file_path TEXT NOT NULL,
        file_size INTEGER,
        format TEXT,
        downloaded_at INTEGER NOT NULL
    );
    CREATE INDEX downloads_by_video ON downloads (video, downloaded_at);",
    // 2: the download queue (downloads/store.rs). The video, preset, result and error are JSON. AUTOINCREMENT, so an
    // ID, which names the item's temporary folder and log, is never reused.
    "CREATE TABLE queue_items (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        video TEXT NOT NULL,
        preset TEXT NOT NULL,
        folder TEXT NOT NULL,
        state TEXT NOT NULL,
        overwrite INTEGER NOT NULL,
        added_at INTEGER NOT NULL,
        result TEXT,
        error TEXT
    );",
];

pub struct Database {
    connection: Mutex<Connection>,
}

impl Database {
    /// Opens the database, creating or upgrading it as needed. A database from a newer version of Tsundoku is
    /// refused rather than read.
    pub fn open(path: &Path) -> Result<Self, HistoryError> {
        Self::with_connection(Connection::open(path)?)
    }

    /// A database kept in memory, for tests.
    pub fn open_in_memory() -> Result<Self, HistoryError> {
        Self::with_connection(Connection::open_in_memory()?)
    }

    fn with_connection(mut connection: Connection) -> Result<Self, HistoryError> {
        connection.pragma_update(None, "foreign_keys", true)?;
        // Write-ahead logging: a crash mid-write can't damage the database, and reads don't wait on writes.
        connection
            .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
        migrate(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Connection> {
        lock(&self.connection)
    }
}

fn migrate(connection: &mut Connection) -> Result<(), HistoryError> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let latest = MIGRATIONS.len() as i64;
    if version > latest {
        return Err(HistoryError::NewerVersion { version });
    }
    for (index, migration) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(migration)?;
        transaction.pragma_update(None, "user_version", index as i64 + 1)?;
        transaction.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(database: &Database) -> i64 {
        database
            .lock()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn creates_the_database_at_the_latest_version() {
        let database = Database::open_in_memory().unwrap();
        assert_eq!(version(&database), MIGRATIONS.len() as i64);
    }

    #[test]
    fn upgrades_an_older_database() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("tsundoku.db");
        {
            let connection = Connection::open(&path).unwrap();
            connection.execute_batch(MIGRATIONS[0]).unwrap();
            connection.pragma_update(None, "user_version", 1).unwrap();
            connection
                .execute(
                    "INSERT INTO videos (site, site_name, video_id, url, title, first_downloaded_at)
                    VALUES ('youtube', 'youtube.com', 'a', 'https://youtube.com/a', 'A', 1)",
                    [],
                )
                .unwrap();
        }
        let database = Database::open(&path).unwrap();
        assert_eq!(version(&database), MIGRATIONS.len() as i64);
        let videos: i64 = database
            .lock()
            .query_row("SELECT COUNT(*) FROM videos", [], |row| row.get(0))
            .unwrap();
        assert_eq!(videos, 1, "the data survives");
    }

    #[test]
    fn refuses_a_database_from_a_newer_version() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("tsundoku.db");
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        assert_eq!(
            Database::open(&path).err(),
            Some(HistoryError::NewerVersion { version: 99 })
        );
    }
}
