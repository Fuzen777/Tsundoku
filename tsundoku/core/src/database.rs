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
    // 3: what a post held, and every file of a download that made several (a post of images). A download of one file
    // has no rows here: its `file_path` is the file.
    "ALTER TABLE videos ADD COLUMN post_images INTEGER;
    ALTER TABLE videos ADD COLUMN post_videos INTEGER;
    CREATE TABLE download_files (
        download INTEGER NOT NULL REFERENCES downloads (id) ON DELETE CASCADE,
        position INTEGER NOT NULL,
        file_path TEXT NOT NULL,
        file_size INTEGER,
        PRIMARY KEY (download, position)
    );",
    // 4: what a file host's link held (a MEGA folder, say): how many files, and their size together.
    "ALTER TABLE videos ADD COLUMN file_count INTEGER;
    ALTER TABLE videos ADD COLUMN file_bytes INTEGER;",
    // 5: links waiting to be started (pending/store.rs). The video and error are JSON, set once the link is read.
    // AUTOINCREMENT, so a removed link's ID is never given to another while the UI may still name it.
    "CREATE TABLE pending_links (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        link TEXT NOT NULL,
        state TEXT NOT NULL,
        video TEXT,
        missing_js_runtime INTEGER NOT NULL DEFAULT 0,
        error TEXT,
        preset TEXT,
        start_when_read INTEGER NOT NULL DEFAULT 0,
        added_at INTEGER NOT NULL
    );",
    // 6: links copied in another app and caught from the clipboard, which leave quietly when nothing can download them.
    "ALTER TABLE pending_links ADD COLUMN caught INTEGER NOT NULL DEFAULT 0;",
    // 7: the queue's order. Items are added at 0 and start by position, then ID; Download Next gives one a position
    // below every other.
    "ALTER TABLE queue_items ADD COLUMN position INTEGER NOT NULL DEFAULT 0;",
    // 8: what each download is (`DownloadKind`: video, audio, post or file), for the history's Kind filter. Downloads
    // from before are worked out from their video and format: a post or a file host's files by what the video held,
    // and audio when the format is only an audio file's type, or names no codecs and the preset says audio.
    "ALTER TABLE downloads ADD COLUMN kind TEXT NOT NULL DEFAULT 'video';
    UPDATE downloads SET kind = CASE
        WHEN (SELECT post_images FROM videos v WHERE v.id = downloads.video) IS NOT NULL THEN 'post'
        WHEN (SELECT file_count FROM videos v WHERE v.id = downloads.video) IS NOT NULL THEN 'file'
        WHEN lower(coalesce(format, '')) IN ('mp3', 'm4a', 'opus', 'ogg', 'oga', 'flac', 'wav', 'aac', 'alac', 'mka')
            THEN 'audio'
        WHEN instr(coalesce(format, ''), ' · ') = 0 AND instr(coalesce(format, ''), ' + ') = 0
            AND lower(preset) LIKE '%audio%' THEN 'audio'
        ELSE 'video'
    END;",
    // 9: each file's number in its post (`PostFile::number`), so a post downloaded again shows which files were left
    // out. NULL for posts downloaded before, which took every file.
    "ALTER TABLE download_files ADD COLUMN post_number INTEGER;",
    // 10: the thumbnail kept in the data folder's `thumbnails\` for each video (thumbnails.rs), by its name: NULL until
    // it's made, "" when none can be.
    "ALTER TABLE videos ADD COLUMN thumbnail_file TEXT;",
    // 11: a folder picked for one link or download instead of the one the rules give: a pending link's (NULL when
    // none), and whether a queue item's folder was picked so.
    "ALTER TABLE pending_links ADD COLUMN folder TEXT;
    ALTER TABLE queue_items ADD COLUMN folder_chosen INTEGER NOT NULL DEFAULT 0;",
    // 12: watched channels, playlists and accounts (watched/store.rs), and every item each has met, with its state:
    // new, skipped, queued or seen. The last error and each item's video are JSON. AUTOINCREMENT, so a removed source's
    // ID is never given to another while the UI may still name it.
    "CREATE TABLE watched_sources (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        url TEXT NOT NULL UNIQUE,
        kind TEXT NOT NULL,
        name TEXT NOT NULL,
        site_name TEXT NOT NULL,
        thumbnail_url TEXT,
        folder TEXT,
        preset TEXT,
        added_at INTEGER NOT NULL,
        last_checked_at INTEGER,
        last_error TEXT
    );
    CREATE TABLE watched_items (
        source INTEGER NOT NULL REFERENCES watched_sources (id) ON DELETE CASCADE,
        site TEXT NOT NULL,
        item_id TEXT NOT NULL,
        video TEXT NOT NULL,
        approximate_date TEXT,
        state TEXT NOT NULL,
        first_seen_at INTEGER NOT NULL,
        position INTEGER NOT NULL,
        PRIMARY KEY (source, site, item_id)
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
    fn works_out_what_older_downloads_were() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("tsundoku.db");
        let kinds_migration = MIGRATIONS
            .iter()
            .position(|migration| migration.contains("ADD COLUMN kind"))
            .unwrap();
        {
            let connection = Connection::open(&path).unwrap();
            for migration in &MIGRATIONS[..kinds_migration] {
                connection.execute_batch(migration).unwrap();
            }
            connection
                .pragma_update(None, "user_version", kinds_migration as i64)
                .unwrap();
            connection
                .execute_batch(
                    "INSERT INTO videos (id, site, site_name, video_id, url, title, first_downloaded_at, post_images,
                        post_videos, file_count, file_bytes)
                    VALUES (1, 'youtube', 'YouTube', 'a', 'https://a', 'A', 1, NULL, NULL, NULL, NULL),
                        (2, 'instagram', 'Instagram', 'b', 'https://b', 'B', 1, 2, 0, NULL, NULL),
                        (3, 'mega', 'MEGA', 'c', 'https://c', 'C', 1, NULL, NULL, 2, 10);
                    INSERT INTO downloads (id, video, preset, file_path, format, downloaded_at) VALUES
                        (1, 1, 'Best Video', 'a.mkv', '1080p · avc1 + mp4a', 1),
                        (2, 1, 'Audio MP3', 'a.mp3', 'mp3', 2),
                        (3, 1, 'Best Audio', 'a.webm', 'webm', 3),
                        (4, 1, 'My Audio Copy', 'a.m4a', NULL, 4),
                        (5, 1, 'Old Videos', 'a.mp4', 'mp4', 5),
                        (6, 2, '', 'b.jpg', NULL, 6),
                        (7, 3, '', 'c.zip', NULL, 7);",
                )
                .unwrap();
        }
        let database = Database::open(&path).unwrap();
        let connection = database.lock();
        let mut statement = connection
            .prepare("SELECT kind FROM downloads ORDER BY id")
            .unwrap();
        let kinds: Vec<String> = statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            kinds,
            ["video", "audio", "audio", "audio", "video", "post", "file"]
        );
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
