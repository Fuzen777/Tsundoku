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
    // 13: how a queue item's post's files are named by a rule (`rules::file_name`), or NULL for Settings' name.
    "ALTER TABLE queue_items ADD COLUMN file_name TEXT;",
    // 14: the rule picked by hand for one link or download, by its ID, which decides everything it sets before any
    // other rule (`rules::picked_rule`); NULL when none was.
    "ALTER TABLE pending_links ADD COLUMN rule TEXT;
    ALTER TABLE queue_items ADD COLUMN rule TEXT;",
    // 15: the password a person typed for a link that needs one (`passwords`): a pending link's, a queue item's, and
    // the history's for each video, so Retry and Download Again don't ask again. NULL for none.
    "ALTER TABLE pending_links ADD COLUMN password TEXT;
    ALTER TABLE queue_items ADD COLUMN password TEXT;
    ALTER TABLE videos ADD COLUMN password TEXT;",
    // 16: the paths in its shared folder (`hosts::shared_path`) of the files a file host's download took, as a JSON
    // list, so a folder downloaded again shows which files it didn't take. Kept on the download rather than on its
    // files, which unpacking replaces with what the archives held. NULL for downloads made before, which took every
    // file, and for videos and posts.
    "ALTER TABLE downloads ADD COLUMN share_paths TEXT;",
    // 17: the channel, playlist or account a queue item was picked from (`downloads::QueueSource`), as JSON, so the
    // Queue keeps showing the downloads of one source together. NULL for a link added on its own.
    "ALTER TABLE queue_items ADD COLUMN source TEXT;",
    // 18: what a listing's picker loaded past its first items (listing/saved.rs), by the listing's page: what the
    // listing is and where its next items start, as JSON, and each item once, in the order they were listed. Kept so
    // opening the listing again goes on from there; forgotten 30 days after its last load.
    "CREATE TABLE saved_listings (
        link TEXT PRIMARY KEY,
        listing TEXT NOT NULL,
        more TEXT,
        total INTEGER,
        saved_at INTEGER NOT NULL
    );
    CREATE TABLE saved_listing_items (
        id INTEGER PRIMARY KEY,
        link TEXT NOT NULL REFERENCES saved_listings (link) ON DELETE CASCADE,
        site TEXT NOT NULL,
        video_id TEXT NOT NULL,
        item TEXT NOT NULL,
        UNIQUE (link, site, video_id)
    );",
    // 19: the passwords used before (`passwords::known`), kept while Save Past Passwords is on: one that opened a
    // link or an archive, with the site it came from (`sites::site_of`'s key) and when it last opened something, in
    // seconds since 1970. They're tried by themselves after Settings → Passwords' list.
    "CREATE TABLE past_passwords (
        site TEXT NOT NULL,
        password TEXT NOT NULL,
        used_at INTEGER NOT NULL,
        PRIMARY KEY (site, password)
    );",
    // 20: what gave a queue item its folder when the rules did (`downloads::FolderFrom`: a rule by its ID, the preset's
    // folder or the download folder), as JSON, so the Downloads view groups it by the rule it was added with rather than
    // the one that matches now. NULL for a folder picked for it, and for items added before.
    "ALTER TABLE queue_items ADD COLUMN folder_from TEXT;",
    // 21: the picture picked by hand for a watched source (`watched::picture::SourcePicture`: its file in the data
    // folder's `pictures\` and the area of it shown), as JSON; NULL for the listing's own picture.
    "ALTER TABLE watched_sources ADD COLUMN picture TEXT;",
    // 22: a watched source's New Items Start (`watched::NewItemsStart`: "ticked" or "unticked"); NULL for Settings →
    // Watched's choice, as every source watched before has it.
    "ALTER TABLE watched_sources ADD COLUMN new_items_start TEXT;",
    // 23: Pixiv works picked from an artist's works, the following feed or a tag search (0.2.19 to 0.2.22) were kept
    // with the listing's page as their own (`gallery::post_page`), so downloading one read the whole listing: each gets
    // its work's page back, wherever it's kept.
    "UPDATE queue_items
        SET video = json_set(video, '$.url', 'https://www.pixiv.net/artworks/' || json_extract(video, '$.videoId'))
        WHERE json_extract(video, '$.site') = 'pixiv' AND (json_extract(video, '$.url') LIKE 'https://www.pixiv.net/users/%'
            OR json_extract(video, '$.url') LIKE 'https://www.pixiv.net/bookmark_new_illust.php%'
            OR json_extract(video, '$.url') LIKE 'https://www.pixiv.net/tags/%');
    UPDATE pending_links
        SET video = json_set(video, '$.url', 'https://www.pixiv.net/artworks/' || json_extract(video, '$.videoId'))
        WHERE json_extract(video, '$.site') = 'pixiv' AND (json_extract(video, '$.url') LIKE 'https://www.pixiv.net/users/%'
            OR json_extract(video, '$.url') LIKE 'https://www.pixiv.net/bookmark_new_illust.php%'
            OR json_extract(video, '$.url') LIKE 'https://www.pixiv.net/tags/%');
    UPDATE watched_items
        SET video = json_set(video, '$.url', 'https://www.pixiv.net/artworks/' || item_id)
        WHERE site = 'pixiv' AND (json_extract(video, '$.url') LIKE 'https://www.pixiv.net/users/%'
            OR json_extract(video, '$.url') LIKE 'https://www.pixiv.net/bookmark_new_illust.php%'
            OR json_extract(video, '$.url') LIKE 'https://www.pixiv.net/tags/%');
    UPDATE saved_listing_items
        SET item = json_set(item, '$.video.url', 'https://www.pixiv.net/artworks/' || video_id)
        WHERE site = 'pixiv' AND (json_extract(item, '$.video.url') LIKE 'https://www.pixiv.net/users/%'
            OR json_extract(item, '$.video.url') LIKE 'https://www.pixiv.net/bookmark_new_illust.php%'
            OR json_extract(item, '$.video.url') LIKE 'https://www.pixiv.net/tags/%');
    UPDATE videos SET url = 'https://www.pixiv.net/artworks/' || video_id
        WHERE site = 'pixiv' AND (url LIKE 'https://www.pixiv.net/users/%'
            OR url LIKE 'https://www.pixiv.net/bookmark_new_illust.php%' OR url LIKE 'https://www.pixiv.net/tags/%');",
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
    fn keeps_no_share_paths_for_older_downloads() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("tsundoku.db");
        let paths_migration = MIGRATIONS
            .iter()
            .position(|migration| migration.contains("ADD COLUMN share_paths"))
            .unwrap();
        {
            let connection = Connection::open(&path).unwrap();
            for migration in &MIGRATIONS[..paths_migration] {
                connection.execute_batch(migration).unwrap();
            }
            connection
                .pragma_update(None, "user_version", paths_migration as i64)
                .unwrap();
            connection
                .execute_batch(
                    "INSERT INTO videos (id, site, site_name, video_id, url, title, first_downloaded_at, file_count,
                        file_bytes)
                    VALUES (1, 'mega', 'mega.nz', 'c', 'https://c', 'C', 1, 2, 10);
                    INSERT INTO downloads (id, video, preset, file_path, downloaded_at, kind)
                    VALUES (1, 1, '', 'C\\a.zip', 1, 'file');
                    INSERT INTO download_files (download, position, file_path, file_size)
                    VALUES (1, 0, 'C\\a.zip', 5), (1, 1, 'C\\b.zip', 5);",
                )
                .unwrap();
        }
        let database = Database::open(&path).unwrap();
        let connection = database.lock();
        let paths: Vec<Option<String>> = connection
            .prepare("SELECT share_paths FROM downloads")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(paths, [None], "the download is kept, without paths");
        let files: i64 = connection
            .query_row("SELECT COUNT(*) FROM download_files", [], |row| row.get(0))
            .unwrap();
        assert_eq!(files, 2, "with its files");
    }

    #[test]
    fn gives_pixiv_works_listed_before_their_own_page() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("tsundoku.db");
        let pages_migration = MIGRATIONS
            .iter()
            .position(|migration| migration.contains("https://www.pixiv.net/artworks/"))
            .unwrap();
        let feed = "https://www.pixiv.net/bookmark_new_illust.php";
        let video = |id: &str, url: &str| {
            format!(r#"{{"site": "pixiv", "videoId": "{id}", "url": "{url}", "title": "Work"}}"#)
        };
        {
            let connection = Connection::open(&path).unwrap();
            for migration in &MIGRATIONS[..pages_migration] {
                connection.execute_batch(migration).unwrap();
            }
            connection
                .pragma_update(None, "user_version", pages_migration as i64)
                .unwrap();
            let insert_item = |id: &str, url: &str| {
                connection
                    .execute(
                        "INSERT INTO queue_items (video, preset, folder, state, overwrite, added_at)
                        VALUES (?1, 'null', 'D:\\', 'waiting', 0, 1)",
                        [video(id, url)],
                    )
                    .unwrap();
            };
            insert_item("1", feed);
            insert_item("2", "https://www.pixiv.net/users/11/artworks");
            insert_item(
                "3",
                "https://www.pixiv.net/tags/%E5%8E%9F%E7%A5%9E/artworks?s_mode=s_tag",
            );
            insert_item("4", "https://www.pixiv.net/en/artworks/4");
            connection
                .execute(
                    "INSERT INTO pending_links (link, state, video, added_at) VALUES (?1, 'read', ?2, 1)",
                    [feed.to_owned(), video("5", feed)],
                )
                .unwrap();
            connection
                .execute_batch(
                    "INSERT INTO watched_sources (id, url, kind, name, site_name, added_at)
                    VALUES (1, 'https://www.pixiv.net/bookmark_new_illust.php', 'account', 'Following', 'pixiv.net', 1);
                    INSERT INTO saved_listings (link, listing, saved_at)
                    VALUES ('https://www.pixiv.net/bookmark_new_illust.php', '{}', 1);
                    INSERT INTO videos (site, site_name, video_id, url, title, first_downloaded_at)
                    VALUES ('pixiv', 'pixiv.net', '8', 'https://www.pixiv.net/bookmark_new_illust.php', 'Work', 1),
                        ('youtube', 'youtube.com', '9', 'https://www.pixiv.net/users/9', 'Not Pixiv', 1);",
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO watched_items (source, site, item_id, video, state, first_seen_at, position)
                    VALUES (1, 'pixiv', '6', ?1, 'new', 1, 0)",
                    [video("6", feed)],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO saved_listing_items (link, site, video_id, item) VALUES (?1, 'pixiv', '7', ?2)",
                    [
                        feed.to_owned(),
                        format!(r#"{{"video": {}, "approximateDate": null}}"#, video("7", feed)),
                    ],
                )
                .unwrap();
        }
        let database = Database::open(&path).unwrap();
        let connection = database.lock();
        let urls = |sql: &str| -> Vec<String> {
            connection
                .prepare(sql)
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(
            urls("SELECT json_extract(video, '$.url') FROM queue_items ORDER BY id"),
            [
                "https://www.pixiv.net/artworks/1",
                "https://www.pixiv.net/artworks/2",
                "https://www.pixiv.net/artworks/3",
                "https://www.pixiv.net/en/artworks/4"
            ]
        );
        assert_eq!(
            urls("SELECT json_extract(video, '$.url') FROM pending_links"),
            ["https://www.pixiv.net/artworks/5"]
        );
        assert_eq!(
            urls("SELECT json_extract(video, '$.url') FROM watched_items"),
            ["https://www.pixiv.net/artworks/6"]
        );
        assert_eq!(
            urls("SELECT json_extract(item, '$.video.url') FROM saved_listing_items"),
            ["https://www.pixiv.net/artworks/7"]
        );
        assert_eq!(
            urls("SELECT url FROM videos ORDER BY id"),
            [
                "https://www.pixiv.net/artworks/8",
                "https://www.pixiv.net/users/9"
            ]
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
