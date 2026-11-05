//! The history of every download: `tsundoku.db` (SQLite) is its source of truth, and yt-dlp's `archive.txt` is
//! rewritten from it after every change, one `<site> <id>` line per video, so yt-dlp can use it too.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::atomic_file::write_atomically;
use crate::metadata::VideoInfo;
use crate::sync::lock;

/// Each entry upgrades the database by one version, recorded in `PRAGMA user_version`. Never change an entry once
/// released; add one.
const MIGRATIONS: &[&str] = &[
    // 1: videos, and one row per finished download of each.
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
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum HistoryError {
    #[error("database error: {detail}")]
    Database { detail: String },
    /// The database was made by a newer version of Tsundoku, which this one can't read safely.
    #[error("the history is from a newer version of Tsundoku (version {version})")]
    NewerVersion {
        #[ts(type = "number")]
        version: i64,
    },
    #[error("no such download")]
    NotFound,
    /// The downloaded file was moved or deleted.
    #[error("{path} isn't there anymore")]
    FileMissing { path: String },
    #[error("couldn't write archive.txt: {detail}")]
    Archive { detail: String },
    /// Windows couldn't open the file or its folder.
    #[error("couldn't open it: {detail}")]
    OpenFailed { detail: String },
}

impl From<rusqlite::Error> for HistoryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database {
            detail: error.to_string(),
        }
    }
}

/// A finished download, to record.
#[derive(Debug, Clone, PartialEq)]
pub struct NewDownload {
    pub preset: String,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub format: Option<String>,
    /// In seconds since 1970.
    pub downloaded_at: i64,
}

/// A download in the history.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PastDownload {
    #[ts(type = "number")]
    pub id: i64,
    /// The preset's name when it was downloaded.
    pub preset: String,
    pub file_path: String,
    #[ts(type = "number | null")]
    pub file_size: Option<i64>,
    /// Resolution and codecs, such as "1080p · avc1 + mp4a".
    pub format: Option<String>,
    /// In seconds since 1970.
    #[ts(type = "number")]
    pub downloaded_at: i64,
    /// Whether the file is still where it was saved.
    pub file_exists: bool,
}

/// A video in the history, with its latest download.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryVideo {
    #[ts(type = "number")]
    pub id: i64,
    #[serde(flatten)]
    pub video: VideoInfo,
    /// In seconds since 1970.
    #[ts(type = "number")]
    pub first_downloaded_at: i64,
    pub last_download: PastDownload,
    pub download_count: u32,
}

/// What the history view asks for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct HistoryQuery {
    /// Words that must each appear in the title or the uploader.
    pub search: Option<String>,
    /// Only this site (`VideoInfo::site`).
    pub site: Option<String>,
    /// Only videos downloaded at least once with this preset.
    pub preset: Option<String>,
    pub sort: HistorySort,
    pub descending: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HistorySort {
    Title,
    Uploader,
    Site,
    Duration,
    /// By the latest download.
    #[default]
    Downloaded,
    /// By the latest download's size.
    Size,
}

impl HistorySort {
    fn column(self) -> &'static str {
        match self {
            Self::Title => "v.title COLLATE NOCASE",
            Self::Uploader => "v.uploader COLLATE NOCASE",
            Self::Site => "v.site_name COLLATE NOCASE",
            Self::Duration => "v.duration_seconds",
            Self::Downloaded => "d.downloaded_at",
            Self::Size => "d.file_size",
        }
    }
}

/// The video's columns, in the order `video_from_row` reads them from 0, then the download's from `DOWNLOAD_OFFSET`.
const VIDEO_COLUMNS: &str =
    "v.id, v.site, v.site_name, v.video_id, v.url, v.title, v.uploader, v.duration_seconds,
    v.upload_date, v.thumbnail_url, v.best_height, v.first_downloaded_at";
const DOWNLOAD_COLUMNS: &str =
    "d.id, d.preset, d.file_path, d.file_size, d.format, d.downloaded_at";
const DOWNLOAD_OFFSET: usize = 12;

pub struct History {
    connection: Mutex<Connection>,
    archive_path: PathBuf,
}

impl History {
    /// Opens the history, creating or upgrading the database as needed, and rewrites `archive.txt` from it in case a
    /// crash left the two apart.
    pub fn open(database_path: &Path, archive_path: PathBuf) -> Result<Self, HistoryError> {
        Self::with_connection(Connection::open(database_path)?, archive_path)
    }

    /// A history kept in memory, for tests.
    pub fn open_in_memory(archive_path: PathBuf) -> Result<Self, HistoryError> {
        Self::with_connection(Connection::open_in_memory()?, archive_path)
    }

    fn with_connection(
        mut connection: Connection,
        archive_path: PathBuf,
    ) -> Result<Self, HistoryError> {
        connection.pragma_update(None, "foreign_keys", true)?;
        // Write-ahead logging: a crash mid-write can't damage the database, and reads don't wait on writes.
        connection
            .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
        migrate(&mut connection)?;
        let history = Self {
            connection: Mutex::new(connection),
            archive_path,
        };
        history.write_archive(&lock(&history.connection))?;
        Ok(history)
    }

    /// Records a finished download, adding the video or refreshing its details, then rewrites `archive.txt`.
    /// Returns the download's ID.
    pub fn record(&self, video: &VideoInfo, download: &NewDownload) -> Result<i64, HistoryError> {
        let mut connection = lock(&self.connection);
        let transaction = connection.transaction()?;
        let video_row: i64 = transaction.query_row(
            "INSERT INTO videos (site, site_name, video_id, url, title, uploader, duration_seconds, upload_date,
                thumbnail_url, best_height, first_downloaded_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT (site, video_id) DO UPDATE SET site_name = excluded.site_name, url = excluded.url,
                title = excluded.title, uploader = excluded.uploader, duration_seconds = excluded.duration_seconds,
                upload_date = excluded.upload_date, thumbnail_url = excluded.thumbnail_url,
                best_height = excluded.best_height
            RETURNING id",
            params![
                video.site,
                video.site_name,
                video.video_id,
                video.url,
                video.title,
                video.uploader,
                video.duration_seconds,
                video.upload_date,
                video.thumbnail_url,
                video.best_height,
                download.downloaded_at,
            ],
            |row| row.get(0),
        )?;
        let download_id: i64 = transaction.query_row(
            "INSERT INTO downloads (video, preset, file_path, file_size, format, downloaded_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6) RETURNING id",
            params![
                video_row,
                download.preset,
                download.file_path,
                download.file_size,
                download.format,
                download.downloaded_at,
            ],
            |row| row.get(0),
        )?;
        transaction.commit()?;
        self.write_archive(&connection)?;
        Ok(download_id)
    }

    /// The latest download of a video, found by its site and ID: the duplicate check.
    pub fn last_download(
        &self,
        site: &str,
        video_id: &str,
    ) -> Result<Option<PastDownload>, HistoryError> {
        let connection = lock(&self.connection);
        let download = connection
            .query_row(
                &format!(
                    "SELECT {DOWNLOAD_COLUMNS} FROM downloads d JOIN videos v ON v.id = d.video
                    WHERE v.site = ?1 AND v.video_id = ?2 ORDER BY d.downloaded_at DESC, d.id DESC LIMIT 1"
                ),
                params![site, video_id],
                |row| download_from_row(row, 0),
            )
            .optional()?;
        Ok(download)
    }

    /// Every download of a video in the history, newest first.
    pub fn downloads_of(&self, video_row: i64) -> Result<Vec<PastDownload>, HistoryError> {
        let connection = lock(&self.connection);
        let mut statement = connection.prepare(&format!(
            "SELECT {DOWNLOAD_COLUMNS} FROM downloads d WHERE d.video = ?1 ORDER BY d.downloaded_at DESC, d.id DESC"
        ))?;
        let downloads = statement
            .query_map([video_row], |row| download_from_row(row, 0))?
            .collect::<Result<_, _>>()?;
        Ok(downloads)
    }

    /// The videos matching the query, each with its latest download.
    pub fn search(&self, query: &HistoryQuery) -> Result<Vec<HistoryVideo>, HistoryError> {
        let mut conditions = Vec::new();
        let mut values = Vec::new();
        for word in query
            .search
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
        {
            values.push(format!("%{}%", escape_like(word)));
            let index = values.len();
            conditions.push(format!(
                "(v.title LIKE ?{index} ESCAPE '\\' OR v.uploader LIKE ?{index} ESCAPE '\\')"
            ));
        }
        if let Some(site) = &query.site {
            values.push(site.clone());
            conditions.push(format!("v.site = ?{}", values.len()));
        }
        if let Some(preset) = &query.preset {
            values.push(preset.clone());
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM downloads p WHERE p.video = v.id AND p.preset = ?{})",
                values.len()
            ));
        }
        let filter = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };
        let direction = if query.descending { "DESC" } else { "ASC" };

        let connection = lock(&self.connection);
        let mut statement = connection.prepare(&format!(
            "SELECT {VIDEO_COLUMNS}, {DOWNLOAD_COLUMNS},
                (SELECT COUNT(*) FROM downloads c WHERE c.video = v.id)
            FROM videos v
            JOIN downloads d ON d.id =
                (SELECT l.id FROM downloads l WHERE l.video = v.id ORDER BY l.downloaded_at DESC, l.id DESC LIMIT 1)
            {filter}
            ORDER BY {column} {direction}, d.downloaded_at DESC, v.id DESC",
            column = query.sort.column(),
        ))?;
        let videos = statement
            .query_map(params_from_iter(values), |row| {
                Ok(HistoryVideo {
                    id: row.get(0)?,
                    video: video_from_row(row)?,
                    first_downloaded_at: row.get(11)?,
                    last_download: download_from_row(row, DOWNLOAD_OFFSET)?,
                    download_count: row.get(DOWNLOAD_OFFSET + 6)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(videos)
    }

    /// Where a download's file was saved, if it's still there.
    pub fn download_path(&self, download_id: i64) -> Result<PathBuf, HistoryError> {
        let connection = lock(&self.connection);
        let path: String = connection
            .query_row(
                "SELECT file_path FROM downloads WHERE id = ?1",
                [download_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(HistoryError::NotFound)?;
        let path = PathBuf::from(path);
        if path.exists() {
            Ok(path)
        } else {
            Err(HistoryError::FileMissing {
                path: path.display().to_string(),
            })
        }
    }

    /// Removes a video and its downloads from the history and from `archive.txt`, so it can be downloaded again.
    /// Its files stay where they are.
    pub fn forget(&self, video_row: i64) -> Result<(), HistoryError> {
        let connection = lock(&self.connection);
        let removed = connection.execute("DELETE FROM videos WHERE id = ?1", [video_row])?;
        if removed == 0 {
            return Err(HistoryError::NotFound);
        }
        self.write_archive(&connection)
    }

    /// Rewrites `archive.txt` from the database: one `<site> <id>` line per video, in the order they were first
    /// downloaded, as yt-dlp's `--download-archive` writes it.
    fn write_archive(&self, connection: &Connection) -> Result<(), HistoryError> {
        let mut statement = connection.prepare("SELECT site, video_id FROM videos ORDER BY id")?;
        let lines = statement
            .query_map([], |row| {
                Ok(format!(
                    "{} {}\n",
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?
                ))
            })?
            .collect::<Result<String, _>>()?;
        write_atomically(&self.archive_path, lines.as_bytes()).map_err(|error| {
            HistoryError::Archive {
                detail: error.to_string(),
            }
        })
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

fn video_from_row(row: &Row) -> rusqlite::Result<VideoInfo> {
    Ok(VideoInfo {
        site: row.get(1)?,
        site_name: row.get(2)?,
        video_id: row.get(3)?,
        url: row.get(4)?,
        title: row.get(5)?,
        uploader: row.get(6)?,
        duration_seconds: row.get(7)?,
        upload_date: row.get(8)?,
        thumbnail_url: row.get(9)?,
        best_height: row.get(10)?,
    })
}

/// Reads `DOWNLOAD_COLUMNS` starting at column `offset`.
fn download_from_row(row: &Row, offset: usize) -> rusqlite::Result<PastDownload> {
    let file_path: String = row.get(offset + 2)?;
    Ok(PastDownload {
        id: row.get(offset)?,
        preset: row.get(offset + 1)?,
        file_exists: Path::new(&file_path).exists(),
        file_path,
        file_size: row.get(offset + 3)?,
        format: row.get(offset + 4)?,
        downloaded_at: row.get(offset + 5)?,
    })
}

/// Makes `%`, `_` and `\` match themselves in a LIKE pattern that uses `ESCAPE '\'`.
fn escape_like(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    struct TestHistory {
        history: History,
        folder: tempfile::TempDir,
    }

    impl TestHistory {
        fn new() -> Self {
            let folder = tempfile::tempdir().unwrap();
            let history = History::open_in_memory(folder.path().join("archive.txt")).unwrap();
            Self { history, folder }
        }

        fn archive(&self) -> String {
            fs::read_to_string(self.folder.path().join("archive.txt")).unwrap()
        }
    }

    fn video(site: &str, video_id: &str, title: &str, uploader: &str, duration: f64) -> VideoInfo {
        VideoInfo {
            site: site.to_owned(),
            site_name: format!("{site}.com"),
            video_id: video_id.to_owned(),
            url: format!("https://{site}.com/{video_id}"),
            title: title.to_owned(),
            uploader: Some(uploader.to_owned()),
            duration_seconds: Some(duration),
            upload_date: Some("2005-04-24".to_owned()),
            thumbnail_url: None,
            best_height: Some(1080),
        }
    }

    fn download(preset: &str, downloaded_at: i64, file_size: i64) -> NewDownload {
        NewDownload {
            preset: preset.to_owned(),
            file_path: format!(r"C:\does\not\exist\{preset}-{downloaded_at}.mkv"),
            file_size: Some(file_size),
            format: Some("1080p · avc1 + mp4a".to_owned()),
            downloaded_at,
        }
    }

    fn titles(videos: &[HistoryVideo]) -> Vec<&str> {
        videos
            .iter()
            .map(|video| video.video.title.as_str())
            .collect()
    }

    #[test]
    fn creates_the_database_at_the_latest_version() {
        let test = TestHistory::new();
        let version: i64 = lock(&test.history.connection)
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
        assert_eq!(test.archive(), "");
    }

    #[test]
    fn keeps_the_history_across_opens() {
        let folder = tempfile::tempdir().unwrap();
        let database = folder.path().join("tsundoku.db");
        let archive = folder.path().join("archive.txt");
        History::open(&database, archive.clone())
            .unwrap()
            .record(
                &video("youtube", "jNQXAC9IVRw", "Me at the zoo", "jawed", 19.0),
                &download("Best Video", 1, 10),
            )
            .unwrap();

        fs::remove_file(&archive).unwrap();
        let reopened = History::open(&database, archive.clone()).unwrap();
        assert!(
            reopened
                .last_download("youtube", "jNQXAC9IVRw")
                .unwrap()
                .is_some()
        );
        assert_eq!(
            fs::read_to_string(&archive).unwrap(),
            "youtube jNQXAC9IVRw\n",
            "rewritten on open"
        );
    }

    #[test]
    fn refuses_a_database_from_a_newer_version() {
        let folder = tempfile::tempdir().unwrap();
        let database = folder.path().join("tsundoku.db");
        Connection::open(&database)
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        let error = History::open(&database, folder.path().join("archive.txt")).err();
        assert_eq!(error, Some(HistoryError::NewerVersion { version: 99 }));
    }

    #[test]
    fn finds_the_latest_download_by_site_and_id() {
        let test = TestHistory::new();
        let zoo = video("youtube", "jNQXAC9IVRw", "Me at the zoo", "jawed", 19.0);
        test.history
            .record(&zoo, &download("Best Video", 100, 10))
            .unwrap();
        test.history
            .record(&zoo, &download("Best Audio", 200, 5))
            .unwrap();

        let last = test
            .history
            .last_download("youtube", "jNQXAC9IVRw")
            .unwrap()
            .unwrap();
        assert_eq!(
            (last.preset.as_str(), last.downloaded_at),
            ("Best Audio", 200)
        );
        assert!(!last.file_exists);
        assert_eq!(
            test.history.last_download("youtube", "other").unwrap(),
            None
        );
        assert_eq!(
            test.history.last_download("vimeo", "jNQXAC9IVRw").unwrap(),
            None,
            "the site matters"
        );
    }

    #[test]
    fn keeps_one_video_per_site_and_id_with_each_download() {
        let test = TestHistory::new();
        test.history
            .record(
                &video("youtube", "a", "Old Title", "x", 1.0),
                &download("Best Video", 100, 10),
            )
            .unwrap();
        test.history
            .record(
                &video("youtube", "a", "New Title", "x", 1.0),
                &download("Best Audio", 200, 5),
            )
            .unwrap();

        let videos = test.history.search(&HistoryQuery::default()).unwrap();
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].video.title, "New Title", "details are refreshed");
        assert_eq!(
            videos[0].first_downloaded_at, 100,
            "the first download date is kept"
        );
        assert_eq!(videos[0].download_count, 2);
        assert_eq!(videos[0].last_download.preset, "Best Audio");
        let presets: Vec<_> = test
            .history
            .downloads_of(videos[0].id)
            .unwrap()
            .into_iter()
            .map(|d| d.preset)
            .collect();
        assert_eq!(presets, ["Best Audio", "Best Video"]);
    }

    #[test]
    fn writes_the_archive_in_yt_dlps_format() {
        let test = TestHistory::new();
        test.history
            .record(
                &video("youtube", "jNQXAC9IVRw", "a", "x", 1.0),
                &download("p", 1, 1),
            )
            .unwrap();
        test.history
            .record(
                &video("soundcloud", "62986583", "b", "x", 1.0),
                &download("p", 2, 1),
            )
            .unwrap();
        test.history
            .record(
                &video("youtube", "jNQXAC9IVRw", "a", "x", 1.0),
                &download("q", 3, 1),
            )
            .unwrap();
        assert_eq!(test.archive(), "youtube jNQXAC9IVRw\nsoundcloud 62986583\n");
    }

    #[test]
    fn forgetting_removes_the_video_and_its_archive_line() {
        let test = TestHistory::new();
        test.history
            .record(&video("youtube", "a", "A", "x", 1.0), &download("p", 1, 1))
            .unwrap();
        test.history
            .record(&video("youtube", "b", "B", "x", 1.0), &download("p", 2, 1))
            .unwrap();
        let a = test
            .history
            .search(&HistoryQuery::default())
            .unwrap()
            .into_iter()
            .find(|v| v.video.video_id == "a");

        test.history.forget(a.unwrap().id).unwrap();

        assert_eq!(test.archive(), "youtube b\n");
        assert_eq!(test.history.last_download("youtube", "a").unwrap(), None);
        let downloads: i64 = lock(&test.history.connection)
            .query_row("SELECT COUNT(*) FROM downloads", [], |row| row.get(0))
            .unwrap();
        assert_eq!(downloads, 1, "its downloads went with it");
        assert_eq!(test.history.forget(12345), Err(HistoryError::NotFound));
    }

    #[test]
    fn searches_titles_and_uploaders_word_by_word() {
        let test = TestHistory::new();
        test.history
            .record(
                &video("youtube", "1", "Me at the zoo", "jawed", 19.0),
                &download("p", 1, 1),
            )
            .unwrap();
        test.history
            .record(
                &video("youtube", "2", "100% Cats", "Felix", 60.0),
                &download("p", 2, 1),
            )
            .unwrap();
        test.history
            .record(
                &video("youtube", "3", "Zoo Tour", "someone", 30.0),
                &download("p", 3, 1),
            )
            .unwrap();
        let search = |text: &str| {
            let query = HistoryQuery {
                search: Some(text.to_owned()),
                ..HistoryQuery::default()
            };
            titles(&test.history.search(&query).unwrap())
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            search("ZOO"),
            ["Me at the zoo", "Zoo Tour"],
            "ignoring case, oldest download first"
        );
        assert_eq!(search("zoo jawed"), ["Me at the zoo"]);
        assert_eq!(search("felix"), ["100% Cats"]);
        assert_eq!(search("100%"), ["100% Cats"], "% is matched literally");
        assert_eq!(search("_"), Vec::<String>::new(), "_ is matched literally");
        assert_eq!(search("   ").len(), 3);
    }

    #[test]
    fn filters_by_site_and_preset() {
        let test = TestHistory::new();
        test.history
            .record(
                &video("youtube", "1", "A", "x", 1.0),
                &download("Best Video", 1, 1),
            )
            .unwrap();
        test.history
            .record(
                &video("youtube", "1", "A", "x", 1.0),
                &download("Best Audio", 2, 1),
            )
            .unwrap();
        test.history
            .record(
                &video("soundcloud", "2", "B", "x", 1.0),
                &download("Best Audio", 3, 1),
            )
            .unwrap();
        test.history
            .record(
                &video("youtube", "3", "C", "x", 1.0),
                &download("1080p MP4", 4, 1),
            )
            .unwrap();

        let query = |site: Option<&str>, preset: Option<&str>| HistoryQuery {
            site: site.map(str::to_owned),
            preset: preset.map(str::to_owned),
            sort: HistorySort::Title,
            ..HistoryQuery::default()
        };
        assert_eq!(
            titles(&test.history.search(&query(Some("youtube"), None)).unwrap()),
            ["A", "C"]
        );
        assert_eq!(
            titles(
                &test
                    .history
                    .search(&query(None, Some("Best Video")))
                    .unwrap()
            ),
            ["A"]
        );
        assert_eq!(
            titles(
                &test
                    .history
                    .search(&query(None, Some("Best Audio")))
                    .unwrap()
            ),
            ["A", "B"]
        );
        assert_eq!(
            titles(
                &test
                    .history
                    .search(&query(Some("soundcloud"), Some("Best Video")))
                    .unwrap()
            )
            .len(),
            0
        );
    }

    #[test]
    fn sorts_by_any_column_both_ways() {
        let test = TestHistory::new();
        test.history
            .record(
                &video("youtube", "1", "beta", "Zed", 30.0),
                &download("p", 300, 2),
            )
            .unwrap();
        test.history
            .record(
                &video("vimeo", "2", "Alpha", "amy", 10.0),
                &download("p", 100, 3),
            )
            .unwrap();
        test.history
            .record(
                &video("dailymotion", "3", "gamma", "Bob", 20.0),
                &download("p", 200, 1),
            )
            .unwrap();
        let sorted = |sort: HistorySort, descending: bool| {
            let query = HistoryQuery {
                sort,
                descending,
                ..HistoryQuery::default()
            };
            titles(&test.history.search(&query).unwrap())
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            sorted(HistorySort::Title, false),
            ["Alpha", "beta", "gamma"],
            "ignoring case"
        );
        assert_eq!(sorted(HistorySort::Title, true), ["gamma", "beta", "Alpha"]);
        assert_eq!(
            sorted(HistorySort::Uploader, false),
            ["Alpha", "gamma", "beta"]
        );
        assert_eq!(sorted(HistorySort::Site, false), ["gamma", "Alpha", "beta"]);
        assert_eq!(
            sorted(HistorySort::Duration, false),
            ["Alpha", "gamma", "beta"]
        );
        assert_eq!(
            sorted(HistorySort::Downloaded, true),
            ["beta", "gamma", "Alpha"]
        );
        assert_eq!(sorted(HistorySort::Size, false), ["gamma", "beta", "Alpha"]);
    }

    #[test]
    fn finds_a_downloads_file_only_while_it_exists() {
        let test = TestHistory::new();
        let file = test.folder.path().join("video.mkv");
        fs::write(&file, b"video").unwrap();
        let mut saved = download("p", 1, 5);
        saved.file_path = file.display().to_string();
        let id = test
            .history
            .record(&video("youtube", "a", "A", "x", 1.0), &saved)
            .unwrap();

        assert_eq!(test.history.download_path(id), Ok(file.clone()));
        assert!(
            test.history
                .last_download("youtube", "a")
                .unwrap()
                .unwrap()
                .file_exists
        );
        fs::remove_file(&file).unwrap();
        assert!(matches!(
            test.history.download_path(id),
            Err(HistoryError::FileMissing { .. })
        ));
        assert_eq!(test.history.download_path(999), Err(HistoryError::NotFound));
    }
}
