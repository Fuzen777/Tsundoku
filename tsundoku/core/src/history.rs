//! The history of every download: its tables in `tsundoku.db` (database.rs) are its source of truth, and yt-dlp's `archive.txt` is
//! rewritten from it after every change, one `<site> <id>` line per video, so yt-dlp can use it too.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::atomic_file::write_atomically;
use crate::database::Database;
use crate::metadata::{FileContents, PostContents, VideoInfo};
use crate::rules::DownloadKind;
use crate::thumbnails::{self, ThumbnailJob, ThumbnailSource};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
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
    /// The file, or a post's first file.
    pub file_path: String,
    /// Every file of a post, in order, the first being `file_path`. Empty for a video.
    pub files: Vec<SavedFile>,
    pub file_size: Option<i64>,
    pub format: Option<String>,
    /// In seconds since 1970.
    pub downloaded_at: i64,
    /// Video or audio by its preset, a post, or a file host's files: what the history's Kind filter matches.
    pub kind: DownloadKind,
}

/// One file of a post, to record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedFile {
    pub path: String,
    pub size: Option<i64>,
}

/// One file of a download in the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryFile {
    /// Its place in the download, from 0: what Open File and Show in Folder name.
    pub position: u32,
    pub path: String,
    #[ts(type = "number | null")]
    pub size: Option<i64>,
    /// Whether the file is still where it was saved.
    pub exists: bool,
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
    /// How many files it made: 1 for a video, every file for a post (`History::files_of` lists them).
    pub file_count: u32,
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
    /// The thumbnail kept in the data folder's `thumbnails\`, by its name (`thumbnails::file_name`), once made.
    pub thumbnail_file: Option<String>,
}

/// One page of the history view's results.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryPage {
    pub videos: Vec<HistoryVideo>,
    /// How many videos match the query, across every page.
    pub total: u32,
}

/// What the history can be filtered by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryFilters {
    /// Every site in the history, by name.
    pub sites: Vec<HistorySite>,
    /// Every preset a download was made with, by name.
    pub presets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistorySite {
    /// What `HistoryQuery::site` takes.
    pub site: String,
    pub site_name: String,
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
    /// Only videos downloaded at least once as this kind: video, audio, a post or a file host's files.
    pub kind: Option<DownloadKind>,
    /// Only videos whose latest download's file isn't there anymore: those showing File Missing.
    pub only_missing: bool,
    /// Only videos last downloaded at or after this time, in seconds since 1970.
    #[ts(type = "number | null")]
    pub downloaded_from: Option<i64>,
    /// Only videos last downloaded before this time, in seconds since 1970.
    #[ts(type = "number | null")]
    pub downloaded_until: Option<i64>,
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
    /// By the latest download's preset.
    Preset,
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
            Self::Preset => "d.preset COLLATE NOCASE",
        }
    }
}

/// The video's columns, in the order `video_from_row` reads them from 0, then the download's from `DOWNLOAD_OFFSET`.
const VIDEO_COLUMNS: &str =
    "v.id, v.site, v.site_name, v.video_id, v.url, v.title, v.uploader, v.duration_seconds,
    v.upload_date, v.thumbnail_url, v.best_height, v.first_downloaded_at, v.post_images, v.post_videos,
    v.file_count, v.file_bytes";
const DOWNLOAD_COLUMNS: &str =
    "d.id, d.preset, d.file_path, d.file_size, d.format, d.downloaded_at,
    (SELECT COUNT(*) FROM download_files f WHERE f.download = d.id)";
const DOWNLOAD_OFFSET: usize = 16;
/// How many columns `DOWNLOAD_COLUMNS` has.
const DOWNLOAD_WIDTH: usize = 7;
/// Each video (`v`) with its latest download (`d`), for the history view.
const VIDEOS_WITH_LATEST_DOWNLOAD: &str = "FROM videos v
    JOIN downloads d ON d.id =
        (SELECT l.id FROM downloads l WHERE l.video = v.id ORDER BY l.downloaded_at DESC, l.id DESC LIMIT 1)";

pub struct History {
    database: Arc<Database>,
    archive_path: PathBuf,
    /// Where the videos' thumbnails are kept (`thumbnails`), whose files Forget deletes. None in tests that don't
    /// keep any.
    thumbnails_folder: Option<PathBuf>,
}

impl History {
    /// The history in `database`. Rewrites `archive.txt` from it, in case a crash left the two apart.
    pub fn new(database: Arc<Database>, archive_path: PathBuf) -> Result<Self, HistoryError> {
        let history = Self {
            database,
            archive_path,
            thumbnails_folder: None,
        };
        {
            let connection = history.database.lock();
            // The downloads last found without their file, for Only Missing Files: a temporary table, kept only
            // while the app runs, since a file can come back at any time.
            connection.execute_batch(
                "CREATE TEMP TABLE IF NOT EXISTS missing_files (download INTEGER PRIMARY KEY)",
            )?;
            history.write_archive(&connection)?;
        }
        Ok(history)
    }

    /// Records a finished download, adding the video or refreshing its details, then rewrites `archive.txt`.
    /// Returns the download's ID.
    pub fn record(&self, video: &VideoInfo, download: &NewDownload) -> Result<i64, HistoryError> {
        let mut connection = self.database.lock();
        let transaction = connection.transaction()?;
        let video_row: i64 = transaction.query_row(
            "INSERT INTO videos (site, site_name, video_id, url, title, uploader, duration_seconds, upload_date,
                thumbnail_url, best_height, first_downloaded_at, post_images, post_videos, file_count, file_bytes)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ON CONFLICT (site, video_id) DO UPDATE SET site_name = excluded.site_name, url = excluded.url,
                title = excluded.title, uploader = excluded.uploader, duration_seconds = excluded.duration_seconds,
                upload_date = excluded.upload_date, thumbnail_url = excluded.thumbnail_url,
                best_height = excluded.best_height, post_images = excluded.post_images,
                post_videos = excluded.post_videos, file_count = excluded.file_count,
                file_bytes = excluded.file_bytes
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
                video.post.map(|post| post.images),
                video.post.map(|post| post.videos),
                video.files.map(|files| files.count),
                video.files.map(|files| files.bytes as i64),
            ],
            |row| row.get(0),
        )?;
        let download_id: i64 = transaction.query_row(
            "INSERT INTO downloads (video, preset, file_path, file_size, format, downloaded_at, kind)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING id",
            params![
                video_row,
                download.preset,
                download.file_path,
                download.file_size,
                download.format,
                download.downloaded_at,
                download.kind.as_str(),
            ],
            |row| row.get(0),
        )?;
        // A post's files come in the order of the files it took, which gives each its number in the post. Files that
        // don't match them in count (the post changed since it was read) stay unnumbered, like a whole post's.
        let numbers: Vec<u32> = video
            .post_files
            .iter()
            .filter(|file| file.picked)
            .map(|file| file.number)
            .collect();
        let numbered = numbers.len() == download.files.len();
        for (position, file) in download.files.iter().enumerate() {
            transaction.execute(
                "INSERT INTO download_files (download, position, file_path, file_size, post_number)
                VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    download_id,
                    position as i64,
                    file.path,
                    file.size,
                    numbered.then(|| numbers[position]),
                ],
            )?;
        }
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
        let download = self
            .database
            .lock()
            .query_row(
                &format!(
                    "SELECT {DOWNLOAD_COLUMNS} FROM downloads d JOIN videos v ON v.id = d.video
                    WHERE v.site = ?1 AND v.video_id = ?2 ORDER BY d.downloaded_at DESC, d.id DESC LIMIT 1"
                ),
                params![site, video_id],
                |row| download_from_row(row, 0),
            )
            .optional()?;
        Ok(download.map(PastDownload::with_file_checked))
    }

    /// The downloads from `site` (as the history names it, "instagram") that finished since `since`, in seconds since
    /// 1970: when each finished and how many files it made, for the hourly limit on a paced site's files.
    pub fn recent_downloads(
        &self,
        site: &str,
        since: i64,
    ) -> Result<Vec<(i64, u32)>, HistoryError> {
        let connection = self.database.lock();
        let mut statement = connection.prepare(
            "SELECT d.downloaded_at, MAX(1, (SELECT COUNT(*) FROM download_files f WHERE f.download = d.id))
            FROM downloads d JOIN videos v ON v.id = d.video
            WHERE v.site = ?1 AND d.downloaded_at >= ?2 ORDER BY d.downloaded_at",
        )?;
        let downloads = statement
            .query_map(params![site, since], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        Ok(downloads)
    }

    /// Whether a video downloaded before has exactly this address (its page's, as the tool that read it gave it).
    pub fn has_link(&self, url: &str) -> Result<bool, HistoryError> {
        let connection = self.database.lock();
        let found = connection.query_row(
            "SELECT EXISTS (SELECT 1 FROM videos v JOIN downloads d ON d.video = v.id WHERE v.url = ?1)",
            [url],
            |row| row.get(0),
        )?;
        Ok(found)
    }

    /// The files of a post that its downloads so far left out, by their number (`PostFile::number`), in the post's
    /// order: what the strip marks Not Picked Last Time and ticks to start with. `None` when it was never downloaded,
    /// when a download took every file (or was made before numbers were kept), or when the preview didn't list its
    /// files.
    pub fn left_out_files(&self, video: &VideoInfo) -> Result<Option<Vec<u32>>, HistoryError> {
        if video.post_files.is_empty() {
            return Ok(None);
        }
        let connection = self.database.lock();
        let mut statement = connection.prepare(
            "SELECT f.post_number FROM download_files f JOIN downloads d ON d.id = f.download
            JOIN videos v ON v.id = d.video WHERE v.site = ?1 AND v.video_id = ?2",
        )?;
        let taken: Vec<Option<u32>> = statement
            .query_map(params![video.site, video.video_id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        if taken.is_empty() || taken.contains(&None) {
            return Ok(None);
        }
        let left_out: Vec<u32> = video
            .post_files
            .iter()
            .map(|file| file.number)
            .filter(|number| !taken.contains(&Some(*number)))
            .collect();
        Ok((!left_out.is_empty()).then_some(left_out))
    }

    /// Every download of a video in the history, newest first.
    pub fn downloads_of(&self, video_row: i64) -> Result<Vec<PastDownload>, HistoryError> {
        let downloads: Vec<PastDownload> = {
            let connection = self.database.lock();
            let mut statement = connection.prepare(&format!(
                "SELECT {DOWNLOAD_COLUMNS} FROM downloads d WHERE d.video = ?1 ORDER BY d.downloaded_at DESC, d.id DESC"
            ))?;
            statement
                .query_map([video_row], |row| download_from_row(row, 0))?
                .collect::<Result<_, _>>()?
        };
        Ok(downloads
            .into_iter()
            .map(PastDownload::with_file_checked)
            .collect())
    }

    /// One page of the videos matching the query, each with its latest download, from the `offset`th on, and how
    /// many match in all. Whether each file is still there is checked for this page's videos only, once the database
    /// is free again. With `only_missing`, the first page checks every video the other filters take (`note_missing`),
    /// and the pages after it go by that.
    pub fn page(
        &self,
        query: &HistoryQuery,
        offset: u32,
        limit: u32,
    ) -> Result<HistoryPage, HistoryError> {
        if query.only_missing && offset == 0 {
            self.note_missing(query)?;
        }
        let (filter, mut values) = filter_of(query);
        let direction = if query.descending { "DESC" } else { "ASC" };
        let (videos, total): (Vec<HistoryVideo>, u32) = {
            let connection = self.database.lock();
            let total = connection.query_row(
                &format!("SELECT COUNT(*) {VIDEOS_WITH_LATEST_DOWNLOAD} {filter}"),
                params_from_iter(&values),
                |row| row.get(0),
            )?;
            values.push(Value::Integer(limit.into()));
            values.push(Value::Integer(offset.into()));
            let mut statement = connection.prepare(&format!(
                "SELECT {VIDEO_COLUMNS}, {DOWNLOAD_COLUMNS}, (SELECT COUNT(*) FROM downloads c WHERE c.video = v.id),
                    nullif(v.thumbnail_file, '')
                {VIDEOS_WITH_LATEST_DOWNLOAD}
                {filter}
                ORDER BY {column} {direction}, d.downloaded_at DESC, v.id DESC
                LIMIT ?{limit} OFFSET ?{offset}",
                column = query.sort.column(),
                limit = values.len() - 1,
                offset = values.len(),
            ))?;
            let videos = statement
                .query_map(params_from_iter(&values), |row| {
                    Ok(HistoryVideo {
                        id: row.get(0)?,
                        video: video_from_row(row)?,
                        first_downloaded_at: row.get(11)?,
                        last_download: download_from_row(row, DOWNLOAD_OFFSET)?,
                        download_count: row.get(DOWNLOAD_OFFSET + DOWNLOAD_WIDTH)?,
                        thumbnail_file: row.get(DOWNLOAD_OFFSET + DOWNLOAD_WIDTH + 1)?,
                    })
                })?
                .collect::<Result<_, _>>()?;
            (videos, total)
        };
        let videos = videos
            .into_iter()
            .map(|video| HistoryVideo {
                last_download: video.last_download.with_file_checked(),
                ..video
            })
            .collect();
        Ok(HistoryPage { videos, total })
    }

    /// The IDs of every video matching the query, in its order: what ticking them all in the history view ticks.
    pub fn ids(&self, query: &HistoryQuery) -> Result<Vec<i64>, HistoryError> {
        if query.only_missing {
            self.note_missing(query)?;
        }
        let (filter, values) = filter_of(query);
        let connection = self.database.lock();
        let mut statement = connection.prepare(&format!(
            "SELECT v.id {VIDEOS_WITH_LATEST_DOWNLOAD} {filter}
            ORDER BY {column} {direction}, d.downloaded_at DESC, v.id DESC",
            column = query.sort.column(),
            direction = if query.descending { "DESC" } else { "ASC" },
        ))?;
        let ids = statement
            .query_map(params_from_iter(&values), |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    /// Notes, in `missing_files`, which of the videos the query's other filters take have lost their latest
    /// download's file. The disk is asked for each, several at a time, once the database is free.
    fn note_missing(&self, query: &HistoryQuery) -> Result<(), HistoryError> {
        let others = HistoryQuery {
            only_missing: false,
            ..query.clone()
        };
        let (filter, values) = filter_of(&others);
        let candidates: Vec<(i64, String)> = {
            let connection = self.database.lock();
            let mut statement = connection.prepare(&format!(
                "SELECT d.id, d.file_path {VIDEOS_WITH_LATEST_DOWNLOAD} {filter}"
            ))?;
            statement
                .query_map(params_from_iter(&values), |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?
                .collect::<Result<_, _>>()?
        };
        let missing = missing_files(&candidates);
        let mut connection = self.database.lock();
        let transaction = connection.transaction()?;
        transaction.execute("DELETE FROM temp.missing_files", [])?;
        {
            let mut insert =
                transaction.prepare("INSERT INTO temp.missing_files (download) VALUES (?1)")?;
            for download in missing {
                insert.execute([download])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// The files of the videos' latest downloads that are still there, for showing them in their folders: a post's
    /// first file stands for it, as on its row.
    pub fn latest_files(&self, video_rows: &[i64]) -> Result<Vec<PathBuf>, HistoryError> {
        let paths: Vec<String> = {
            let connection = self.database.lock();
            let mut statement = connection.prepare(&format!(
                "SELECT d.file_path {VIDEOS_WITH_LATEST_DOWNLOAD} WHERE v.id = ?1"
            ))?;
            let mut paths = Vec::new();
            for video_row in video_rows {
                if let Some(path) = statement
                    .query_row([video_row], |row| row.get(0))
                    .optional()?
                {
                    paths.push(path);
                }
            }
            paths
        };
        Ok(paths
            .into_iter()
            .map(PathBuf::from)
            .filter(|path| path.exists())
            .collect())
    }

    /// The sites and presets in the history, for the history view's filters.
    pub fn filters(&self) -> Result<HistoryFilters, HistoryError> {
        let connection = self.database.lock();
        let mut sites = connection.prepare(
            "SELECT site, MIN(site_name) FROM videos GROUP BY site ORDER BY MIN(site_name) COLLATE NOCASE",
        )?;
        let sites = sites
            .query_map([], |row| {
                Ok(HistorySite {
                    site: row.get(0)?,
                    site_name: row.get(1)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        let mut presets = connection
            // A post's download has no preset.
            .prepare("SELECT DISTINCT preset FROM downloads WHERE preset <> '' ORDER BY preset COLLATE NOCASE")?;
        let presets = presets
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(HistoryFilters { sites, presets })
    }

    /// Where a download's file was saved, if it's still there.
    pub fn download_path(&self, download_id: i64) -> Result<PathBuf, HistoryError> {
        let connection = self.database.lock();
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

    /// Every file of a download, in order, and whether each is still there. A video's download has one.
    pub fn files_of(&self, download_id: i64) -> Result<Vec<HistoryFile>, HistoryError> {
        let connection = self.database.lock();
        let mut statement = connection.prepare(
            "SELECT position, file_path, file_size FROM download_files WHERE download = ?1 ORDER BY position",
        )?;
        let mut files: Vec<HistoryFile> = statement
            .query_map([download_id], |row| {
                let path: String = row.get(1)?;
                Ok(HistoryFile {
                    position: row.get(0)?,
                    exists: Path::new(&path).exists(),
                    path,
                    size: row.get(2)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        if files.is_empty() {
            let (path, size): (String, Option<i64>) = connection
                .query_row(
                    "SELECT file_path, file_size FROM downloads WHERE id = ?1",
                    [download_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?
                .ok_or(HistoryError::NotFound)?;
            files.push(HistoryFile {
                position: 0,
                exists: Path::new(&path).exists(),
                path,
                size,
            });
        }
        Ok(files)
    }

    /// Where one file of a download was saved, if it's still there: `position` from `files_of`.
    pub fn file_path(&self, download_id: i64, position: u32) -> Result<PathBuf, HistoryError> {
        let files = self.files_of(download_id)?;
        let file = files
            .into_iter()
            .find(|file| file.position == position)
            .ok_or(HistoryError::NotFound)?;
        if file.exists {
            Ok(PathBuf::from(file.path))
        } else {
            Err(HistoryError::FileMissing { path: file.path })
        }
    }

    /// Removes a video and its downloads from the history and from `archive.txt`, so it can be downloaded again.
    /// Its files stay where they are.
    pub fn forget(&self, video_row: i64) -> Result<(), HistoryError> {
        match self.forget_all(&[video_row])? {
            0 => Err(HistoryError::NotFound),
            _ => Ok(()),
        }
    }

    /// Forgets several videos at once, as `forget` does each, with `archive.txt` rewritten once, and answers how many
    /// were in the history.
    pub fn forget_all(&self, video_rows: &[i64]) -> Result<u32, HistoryError> {
        let mut connection = self.database.lock();
        let transaction = connection.transaction()?;
        let mut removed = 0;
        let mut thumbnails = Vec::new();
        {
            let mut thumbnail = transaction.prepare(
                "SELECT thumbnail_file FROM videos WHERE id = ?1 AND thumbnail_file <> ''",
            )?;
            let mut delete = transaction.prepare("DELETE FROM videos WHERE id = ?1")?;
            for video_row in video_rows {
                if let Some(name) = thumbnail
                    .query_row([video_row], |row| row.get::<_, String>(0))
                    .optional()?
                {
                    thumbnails.push(name);
                }
                removed += delete.execute([video_row])?;
            }
        }
        transaction.commit()?;
        self.write_archive(&connection)?;
        // Its thumbnail goes with it; one left behind would only take room.
        if let Some(folder) = &self.thumbnails_folder {
            for name in thumbnails {
                let _ = std::fs::remove_file(folder.join(name));
            }
        }
        Ok(removed as u32)
    }

    /// The history with the folder its thumbnails are kept in, so Forget deletes them too.
    pub fn with_thumbnails_folder(self, folder: PathBuf) -> Self {
        Self {
            thumbnails_folder: Some(folder),
            ..self
        }
    }

    /// Every video without a thumbnail kept (none made yet, and not one that can't be), newest first, with what one
    /// can be made from: a post's first file, then its picture on the site; a video's or a song's picture on the site,
    /// then a frame or the cover of its own file; nothing for a file host's files.
    pub fn needing_thumbnails(&self) -> Result<Vec<ThumbnailJob>, HistoryError> {
        let connection = self.database.lock();
        let mut statement = connection.prepare(&format!(
            "SELECT v.id, v.site, v.video_id, v.thumbnail_url, v.post_images IS NOT NULL,
                v.file_count IS NOT NULL, d.file_path
            {VIDEOS_WITH_LATEST_DOWNLOAD}
            WHERE v.thumbnail_file IS NULL
            ORDER BY v.id DESC"
        ))?;
        let jobs = statement
            .query_map([], |row| {
                let site: String = row.get(1)?;
                let video_id: String = row.get(2)?;
                let link = row.get::<_, Option<String>>(3)?.map(ThumbnailSource::Link);
                let file = ThumbnailSource::File(PathBuf::from(row.get::<_, String>(6)?));
                let (is_post, is_share): (bool, bool) = (row.get(4)?, row.get(5)?);
                let sources = if is_share {
                    Vec::new()
                } else if is_post {
                    std::iter::once(file).chain(link).collect()
                } else {
                    link.into_iter().chain(std::iter::once(file)).collect()
                };
                Ok(ThumbnailJob {
                    video_row: row.get(0)?,
                    file_name: thumbnails::file_name(&site, &video_id),
                    sources,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(jobs)
    }

    /// Notes a video's kept thumbnail by its name, or "" when none can be made, so it isn't tried again.
    pub fn set_thumbnail(&self, video_row: i64, file_name: &str) -> Result<(), HistoryError> {
        self.database.lock().execute(
            "UPDATE videos SET thumbnail_file = ?2 WHERE id = ?1",
            params![video_row, file_name],
        )?;
        Ok(())
    }

    /// Every kept thumbnail's video and name.
    pub fn thumbnail_files(&self) -> Result<Vec<(i64, String)>, HistoryError> {
        let connection = self.database.lock();
        let mut statement = connection
            .prepare("SELECT id, thumbnail_file FROM videos WHERE thumbnail_file <> ''")?;
        let files = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        Ok(files)
    }

    /// Forgets that these videos' thumbnails were made, so they're made again: their files are gone.
    pub fn clear_thumbnails(&self, video_rows: &[i64]) -> Result<(), HistoryError> {
        let mut connection = self.database.lock();
        let transaction = connection.transaction()?;
        {
            let mut clear =
                transaction.prepare("UPDATE videos SET thumbnail_file = NULL WHERE id = ?1")?;
            for video_row in video_rows {
                clear.execute([video_row])?;
            }
        }
        transaction.commit()?;
        Ok(())
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
        post: match (row.get(12)?, row.get(13)?) {
            (Some(images), Some(videos)) => Some(PostContents { images, videos }),
            _ => None,
        },
        files: match (row.get(14)?, row.get::<_, Option<i64>>(15)?) {
            (Some(count), Some(bytes)) => Some(FileContents {
                count,
                bytes: bytes.max(0) as u64,
            }),
            _ => None,
        },
        post_files: Vec::new(),
    })
}

/// The WHERE clause for a query's search and filters over `VIDEOS_WITH_LATEST_DOWNLOAD`, empty for none, and the
/// values its `?1`, `?2`… stand for.
fn filter_of(query: &HistoryQuery) -> (String, Vec<Value>) {
    let mut conditions = Vec::new();
    let mut values = Vec::new();
    for word in query
        .search
        .as_deref()
        .unwrap_or_default()
        .split_whitespace()
    {
        values.push(Value::Text(format!("%{}%", escape_like(word))));
        let index = values.len();
        conditions.push(format!(
            "(v.title LIKE ?{index} ESCAPE '\\' OR v.uploader LIKE ?{index} ESCAPE '\\')"
        ));
    }
    if let Some(site) = &query.site {
        values.push(Value::Text(site.clone()));
        conditions.push(format!("v.site = ?{}", values.len()));
    }
    if let Some(preset) = &query.preset {
        values.push(Value::Text(preset.clone()));
        conditions.push(format!(
            "EXISTS (SELECT 1 FROM downloads p WHERE p.video = v.id AND p.preset = ?{})",
            values.len()
        ));
    }
    if let Some(kind) = query.kind {
        values.push(Value::Text(kind.as_str().to_owned()));
        conditions.push(format!(
            "EXISTS (SELECT 1 FROM downloads k WHERE k.video = v.id AND k.kind = ?{})",
            values.len()
        ));
    }
    if query.only_missing {
        conditions.push("d.id IN (SELECT download FROM temp.missing_files)".to_owned());
    }
    if let Some(from) = query.downloaded_from {
        values.push(Value::Integer(from));
        conditions.push(format!("d.downloaded_at >= ?{}", values.len()));
    }
    if let Some(until) = query.downloaded_until {
        values.push(Value::Integer(until));
        conditions.push(format!("d.downloaded_at < ?{}", values.len()));
    }
    let filter = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };
    (filter, values)
}

/// The IDs of the files that aren't there anymore. The disk is asked about several at a time, since a slow or network
/// drive answers each slowly.
fn missing_files(files: &[(i64, String)]) -> Vec<i64> {
    let chunk = files.len().div_ceil(8).max(1);
    std::thread::scope(|scope| {
        let checks: Vec<_> = files
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .filter(|(_, path)| !Path::new(path).exists())
                        .map(|(id, _)| *id)
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        checks
            .into_iter()
            .flat_map(|check| check.join().unwrap_or_default())
            .collect()
    })
}

/// How many folders the files are in, each counted once, ignoring letter case as Windows does.
pub fn folder_count(files: &[PathBuf]) -> usize {
    let folders: std::collections::HashSet<String> = files
        .iter()
        .filter_map(|file| file.parent())
        .map(|folder| folder.to_string_lossy().to_lowercase())
        .collect();
    folders.len()
}

impl PastDownload {
    /// With `file_exists` checked on the disk: done once the database is free, since the disk can be slow.
    fn with_file_checked(self) -> Self {
        Self {
            file_exists: Path::new(&self.file_path).exists(),
            ..self
        }
    }
}

/// Reads `DOWNLOAD_COLUMNS` starting at column `offset`. Its `file_exists` is false until `with_file_checked`.
fn download_from_row(row: &Row, offset: usize) -> rusqlite::Result<PastDownload> {
    Ok(PastDownload {
        id: row.get(offset)?,
        preset: row.get(offset + 1)?,
        file_exists: false,
        file_path: row.get(offset + 2)?,
        file_size: row.get(offset + 3)?,
        format: row.get(offset + 4)?,
        downloaded_at: row.get(offset + 5)?,
        // A download of one file has no rows in `download_files`.
        file_count: row.get::<_, u32>(offset + 6)?.max(1),
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

    impl History {
        /// Every video matching the query, as one page.
        fn search(&self, query: &HistoryQuery) -> Result<Vec<HistoryVideo>, HistoryError> {
            Ok(self.page(query, 0, u32::MAX)?.videos)
        }
    }

    struct TestHistory {
        history: History,
        folder: tempfile::TempDir,
    }

    impl TestHistory {
        fn new() -> Self {
            let folder = tempfile::tempdir().unwrap();
            let database = Arc::new(Database::open_in_memory().unwrap());
            let history = History::new(database, folder.path().join("archive.txt")).unwrap();
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
            post: None,
            files: None,
            post_files: Vec::new(),
        }
    }

    fn download(preset: &str, downloaded_at: i64, file_size: i64) -> NewDownload {
        NewDownload {
            preset: preset.to_owned(),
            file_path: format!(r"C:\does\not\exist\{preset}-{downloaded_at}.mkv"),
            file_size: Some(file_size),
            format: Some("1080p · avc1 + mp4a".to_owned()),
            files: Vec::new(),
            downloaded_at,
            kind: DownloadKind::Video,
        }
    }

    fn titles(videos: &[HistoryVideo]) -> Vec<&str> {
        videos
            .iter()
            .map(|video| video.video.title.as_str())
            .collect()
    }

    #[test]
    fn writes_an_empty_archive_for_an_empty_history() {
        assert_eq!(TestHistory::new().archive(), "");
    }

    #[test]
    fn keeps_the_history_across_opens() {
        let folder = tempfile::tempdir().unwrap();
        let database = folder.path().join("tsundoku.db");
        let archive = folder.path().join("archive.txt");
        History::new(
            Arc::new(Database::open(&database).unwrap()),
            archive.clone(),
        )
        .unwrap()
        .record(
            &video("youtube", "jNQXAC9IVRw", "Me at the zoo", "jawed", 19.0),
            &download("Best Video", 1, 10),
        )
        .unwrap();

        fs::remove_file(&archive).unwrap();
        let reopened = History::new(
            Arc::new(Database::open(&database).unwrap()),
            archive.clone(),
        )
        .unwrap();
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
        let downloads: i64 = test
            .history
            .database
            .lock()
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
    fn lists_the_sites_and_presets_to_filter_by() {
        let test = TestHistory::new();
        test.history
            .record(
                &video("youtube", "1", "A", "x", 1.0),
                &download("Best Video", 1, 1),
            )
            .unwrap();
        test.history
            .record(
                &video("soundcloud", "2", "B", "x", 1.0),
                &download("audio mp3", 2, 1),
            )
            .unwrap();
        test.history
            .record(
                &video("youtube", "3", "C", "x", 1.0),
                &download("Best Video", 3, 1),
            )
            .unwrap();

        let filters = test.history.filters().unwrap();
        let sites: Vec<_> = filters
            .sites
            .iter()
            .map(|site| (site.site.as_str(), site.site_name.as_str()))
            .collect();
        assert_eq!(
            sites,
            [("soundcloud", "soundcloud.com"), ("youtube", "youtube.com")]
        );
        assert_eq!(
            filters.presets,
            ["audio mp3", "Best Video"],
            "ignoring case"
        );
    }

    #[test]
    fn sorts_by_preset() {
        let test = TestHistory::new();
        test.history
            .record(
                &video("youtube", "1", "A", "x", 1.0),
                &download("MP3", 1, 1),
            )
            .unwrap();
        test.history
            .record(
                &video("youtube", "2", "B", "x", 1.0),
                &download("best video", 2, 1),
            )
            .unwrap();
        let query = HistoryQuery {
            sort: HistorySort::Preset,
            ..HistoryQuery::default()
        };
        assert_eq!(titles(&test.history.search(&query).unwrap()), ["B", "A"]);
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

    #[test]
    fn counts_a_sites_recent_files() {
        let test = TestHistory::new();
        let mut post = video("instagram", "Abc123", "A post", "someone", 0.0);
        post.post = Some(PostContents {
            images: 2,
            videos: 1,
        });
        let three_files = NewDownload {
            files: ["1.jpg", "2.mp4", "3.jpg"]
                .iter()
                .map(|name| SavedFile {
                    path: (*name).to_owned(),
                    size: Some(5),
                })
                .collect(),
            ..download("", 200, 15)
        };
        test.history.record(&post, &three_files).unwrap();
        let reel = video("instagram", "Reel1", "A reel", "someone", 9.0);
        test.history
            .record(&reel, &download("Best Video", 300, 5))
            .unwrap();
        test.history
            .record(&reel, &download("Best Video", 50, 5))
            .unwrap();
        let other = video("youtube", "a", "A video", "someone", 9.0);
        test.history
            .record(&other, &download("Best Video", 250, 5))
            .unwrap();

        assert_eq!(
            test.history.recent_downloads("instagram", 100).unwrap(),
            [(200, 3), (300, 1)],
            "a post's files, one for a video, only since then and only that site's"
        );
    }

    #[test]
    fn keeps_every_file_of_a_post() {
        let test = TestHistory::new();
        let files: Vec<PathBuf> = ["1.jpg", "2.mp4", "3.jpg"]
            .iter()
            .map(|name| test.folder.path().join(name))
            .collect();
        for file in &files {
            fs::write(file, b"image").unwrap();
        }
        let mut post = video("instagram", "Abc123", "A post", "someone", 0.0);
        post.duration_seconds = None;
        post.post = Some(PostContents {
            images: 2,
            videos: 1,
        });
        let saved = NewDownload {
            preset: String::new(),
            file_path: files[0].display().to_string(),
            files: files
                .iter()
                .map(|file| SavedFile {
                    path: file.display().to_string(),
                    size: Some(5),
                })
                .collect(),
            file_size: Some(15),
            format: None,
            downloaded_at: 1,
            kind: DownloadKind::Post,
        };
        let id = test.history.record(&post, &saved).unwrap();

        let found = test.history.search(&HistoryQuery::default()).unwrap();
        assert_eq!(found[0].video.post, post.post, "what the post held is kept");
        assert_eq!(found[0].last_download.file_count, 3);
        assert_eq!(
            test.history.filters().unwrap().presets,
            Vec::<String>::new(),
            "a post's empty preset isn't a filter"
        );

        let listed = test.history.files_of(id).unwrap();
        assert_eq!(
            listed.iter().map(|file| file.position).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert!(
            listed
                .iter()
                .all(|file| file.exists && file.size == Some(5))
        );
        assert_eq!(test.history.file_path(id, 1), Ok(files[1].clone()));
        fs::remove_file(&files[2]).unwrap();
        assert!(!test.history.files_of(id).unwrap()[2].exists);
        assert!(matches!(
            test.history.file_path(id, 2),
            Err(HistoryError::FileMissing { .. })
        ));
        assert_eq!(test.history.file_path(id, 7), Err(HistoryError::NotFound));

        test.history.forget(found[0].id).unwrap();
        assert_eq!(
            test.history.files_of(id),
            Err(HistoryError::NotFound),
            "forgotten with its video"
        );
    }

    fn carousel() -> VideoInfo {
        VideoInfo {
            post: Some(PostContents {
                images: 2,
                videos: 2,
            }),
            post_files: (1..=4)
                .map(|number| crate::metadata::PostFile {
                    number,
                    video: number > 2,
                    thumbnail_url: None,
                    picked: true,
                })
                .collect(),
            ..video("instagram", "Abc", "A carousel", "someone", 0.0)
        }
    }

    fn saved(paths: &[&str]) -> NewDownload {
        NewDownload {
            file_path: paths[0].to_owned(),
            files: paths
                .iter()
                .map(|path| SavedFile {
                    path: (*path).to_owned(),
                    size: Some(1),
                })
                .collect(),
            ..download("", 1, 2)
        }
    }

    fn picking(post: &VideoInfo, numbers: &[u32]) -> VideoInfo {
        let mut picked = post.clone();
        for file in &mut picked.post_files {
            file.picked = numbers.contains(&file.number);
        }
        picked
    }

    #[test]
    fn knows_which_files_of_a_post_were_left_out() {
        let test = TestHistory::new();
        let post = carousel();
        assert_eq!(
            test.history.left_out_files(&post),
            Ok(None),
            "never downloaded"
        );

        let first = test
            .history
            .record(
                &picking(&post, &[1, 3]),
                &saved(&[r"C:\a 1.jpg", r"C:\a 3.mp4"]),
            )
            .unwrap();
        assert_eq!(test.history.left_out_files(&post), Ok(Some(vec![2, 4])));
        let numbers: Vec<Option<u32>> = test
            .history
            .database
            .lock()
            .prepare("SELECT post_number FROM download_files WHERE download = ?1 ORDER BY position")
            .unwrap()
            .query_map([first], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(numbers, [Some(1), Some(3)]);

        test.history
            .record(&picking(&post, &[4]), &saved(&[r"C:\a 4.mp4"]))
            .unwrap();
        assert_eq!(
            test.history.left_out_files(&post),
            Ok(Some(vec![2])),
            "what every download took counts"
        );

        test.history
            .record(&picking(&post, &[2]), &saved(&[r"C:\a 2.jpg"]))
            .unwrap();
        assert_eq!(test.history.left_out_files(&post), Ok(None), "all taken");
    }

    #[test]
    fn a_post_downloaded_without_its_files_listed_left_nothing_out() {
        let test = TestHistory::new();
        let listed = carousel();
        let unlisted = VideoInfo {
            post_files: Vec::new(),
            ..listed.clone()
        };
        test.history
            .record(&unlisted, &saved(&[r"C:\a.jpg", r"C:\b.jpg"]))
            .unwrap();
        assert_eq!(test.history.left_out_files(&listed), Ok(None));
        assert_eq!(
            test.history.left_out_files(&unlisted),
            Ok(None),
            "nothing to compare with"
        );

        // A post whose files don't match what was picked in count (it changed since it was read) isn't numbered.
        let other = VideoInfo {
            video_id: "Other".to_owned(),
            ..picking(&listed, &[1, 2])
        };
        test.history
            .record(&other, &saved(&[r"C:\o 1.jpg"]))
            .unwrap();
        let other_listed = VideoInfo {
            video_id: "Other".to_owned(),
            ..listed
        };
        assert_eq!(test.history.left_out_files(&other_listed), Ok(None));
    }

    #[test]
    fn keeps_what_a_file_hosts_link_held() {
        let test = TestHistory::new();
        let files: Vec<PathBuf> = ["Pack/a.zip", "Pack/b.zip"]
            .iter()
            .map(|name| test.folder.path().join(name))
            .collect();
        fs::create_dir_all(test.folder.path().join("Pack")).unwrap();
        for file in &files {
            fs::write(file, b"zip").unwrap();
        }
        let mut share = video("mega", "AbCdEf12", "Pack", "", 0.0);
        share.uploader = None;
        share.duration_seconds = None;
        share.files = Some(FileContents { count: 2, bytes: 6 });
        let saved = NewDownload {
            preset: String::new(),
            file_path: files[0].display().to_string(),
            files: files
                .iter()
                .map(|file| SavedFile {
                    path: file.display().to_string(),
                    size: Some(3),
                })
                .collect(),
            file_size: Some(6),
            format: None,
            downloaded_at: 1,
            kind: DownloadKind::File,
        };
        let id = test.history.record(&share, &saved).unwrap();

        let found = test.history.search(&HistoryQuery::default()).unwrap();
        assert_eq!(found[0].video.files, share.files);
        assert_eq!(found[0].video.post, None);
        assert_eq!(found[0].last_download.file_count, 2);
        assert_eq!(test.history.files_of(id).unwrap().len(), 2);
    }

    /// Fills a history with `count` videos, a third of them Instagram posts of 1 to 8 files and a tenth of the rest
    /// downloaded twice, over three years. Their files are made in `files_folder`, except one in ten, which is missing.
    fn seed(history: &History, files_folder: &Path, count: usize) {
        const WORDS: [&str; 16] = [
            "zoo", "cats", "tour", "live", "remix", "trailer", "review", "guide", "music",
            "travel", "sunset", "city", "night", "cooking", "dance", "story",
        ];
        let mut connection = history.database.lock();
        let transaction = connection.transaction().unwrap();
        for index in 0..count {
            let is_post = index % 3 == 0;
            let (site, site_name) = if is_post {
                ("instagram", "Instagram")
            } else {
                [
                    ("youtube", "YouTube"),
                    ("vimeo", "Vimeo"),
                    ("soundcloud", "SoundCloud"),
                ][index % 3]
            };
            let title = format!(
                "{} {} {} #{index}",
                WORDS[index % 16],
                WORDS[(index / 16) % 16],
                WORDS[(index / 7) % 16]
            );
            let downloaded_at = 1_700_000_000 + (index as i64 * 18_917) % 94_000_000;
            let video_row: i64 = transaction
                .query_row(
                    "INSERT INTO videos (site, site_name, video_id, url, title, uploader, duration_seconds,
                        upload_date, thumbnail_url, best_height, first_downloaded_at, post_images, post_videos)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '2024-01-01', ?8, ?9, ?10, ?11, ?12) RETURNING id",
                    params![
                        site,
                        site_name,
                        format!("id{index}"),
                        format!("https://{site}.com/id{index}"),
                        title,
                        format!("uploader{}", index % 200),
                        (!is_post).then_some(30.0 + (index % 600) as f64),
                        format!("https://example.com/{index}.jpg"),
                        (!is_post).then_some(1080),
                        downloaded_at,
                        is_post.then_some(1 + index as i64 % 5),
                        is_post.then_some(index as i64 % 3),
                    ],
                    |row| row.get(0),
                )
                .unwrap();
            let file_count = if is_post { 1 + index % 8 } else { 1 };
            let downloads = if !is_post && index % 10 == 1 { 2 } else { 1 };
            for download in 0..downloads {
                let paths: Vec<String> = (0..file_count)
                    .map(|file| {
                        files_folder
                            .join(format!("{index}-{download}-{file}.bin"))
                            .display()
                            .to_string()
                    })
                    .collect();
                if index % 10 != 7 {
                    for path in &paths {
                        fs::write(path, b"x").unwrap();
                    }
                }
                let download_id: i64 = transaction
                    .query_row(
                        "INSERT INTO downloads (video, preset, file_path, file_size, format, downloaded_at, kind)
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING id",
                        params![
                            video_row,
                            if is_post { "" } else if download == 0 { "Best Video" } else { "Best Audio" },
                            paths[0],
                            1_000_000 + index as i64,
                            (!is_post).then_some("1080p · avc1 + mp4a"),
                            downloaded_at + download as i64 * 1000,
                            if is_post { "post" } else if download == 0 { "video" } else { "audio" },
                        ],
                        |row| row.get(0),
                    )
                    .unwrap();
                if is_post {
                    for (position, path) in paths.iter().enumerate() {
                        transaction
                            .execute(
                                "INSERT INTO download_files (download, position, file_path, file_size)
                                VALUES (?1, ?2, ?3, 1)",
                                params![download_id, position as i64, path],
                            )
                            .unwrap();
                    }
                }
            }
        }
        transaction.commit().unwrap();
    }

    /// The median time of `runs` calls, in milliseconds.
    fn median_milliseconds(runs: usize, mut action: impl FnMut()) -> f64 {
        let mut times: Vec<f64> = (0..runs)
            .map(|_| {
                let started = std::time::Instant::now();
                action();
                started.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        times.sort_by(f64::total_cmp);
        times[runs / 2]
    }

    /// How long the history view's queries take with 5,000 videos. Run with
    /// `cargo test -p tsundoku-core --release history::tests::measure_at_scale -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement, not a check"]
    fn measure_at_scale() {
        let test = TestHistory::new();
        let files = tempfile::tempdir().unwrap();
        seed(&test.history, files.path(), 5000);
        let query = |search: Option<&str>, sort: HistorySort| HistoryQuery {
            search: search.map(str::to_owned),
            sort,
            descending: true,
            ..HistoryQuery::default()
        };
        for (name, query) in [
            (
                "everything, newest first",
                query(None, HistorySort::Downloaded),
            ),
            (
                "search \"zoo\"",
                query(Some("zoo"), HistorySort::Downloaded),
            ),
            (
                "search \"zoo cats\"",
                query(Some("zoo cats"), HistorySort::Downloaded),
            ),
            ("everything, by title", query(None, HistorySort::Title)),
            ("everything, by size", query(None, HistorySort::Size)),
            (
                "only posts",
                HistoryQuery {
                    kind: Some(DownloadKind::Post),
                    ..query(None, HistorySort::Downloaded)
                },
            ),
            (
                "only missing files (each first page checks them all)",
                HistoryQuery {
                    only_missing: true,
                    ..query(None, HistorySort::Downloaded)
                },
            ),
            (
                "downloaded in a year",
                HistoryQuery {
                    downloaded_from: Some(1_700_000_000),
                    downloaded_until: Some(1_731_536_000),
                    ..query(None, HistorySort::Downloaded)
                },
            ),
        ] {
            let mut found = 0;
            let whole =
                median_milliseconds(9, || found = test.history.search(&query).unwrap().len());
            let first_page = median_milliseconds(9, || {
                test.history.page(&query, 0, 100).unwrap();
            });
            let last_page = median_milliseconds(9, || {
                test.history
                    .page(&query, found.saturating_sub(100) as u32, 100)
                    .unwrap();
            });
            println!(
                "{name}: {found} videos; all at once {whole:.1} ms, first page {first_page:.1} ms, last page {last_page:.1} ms"
            );
        }
    }

    #[test]
    fn pages_through_the_history_in_order_with_its_total() {
        let test = TestHistory::new();
        // Equal titles and dates, so the order rests on the video's row ID across page boundaries.
        for index in 0..7 {
            test.history
                .record(
                    &video("youtube", &index.to_string(), "Same", "x", 1.0),
                    &download("p", 1, 1),
                )
                .unwrap();
        }
        test.history
            .record(
                &video("vimeo", "other", "Other", "x", 1.0),
                &download("p", 1, 1),
            )
            .unwrap();
        let query = HistoryQuery {
            site: Some("youtube".to_owned()),
            sort: HistorySort::Title,
            ..HistoryQuery::default()
        };
        let whole: Vec<i64> = test
            .history
            .search(&query)
            .unwrap()
            .iter()
            .map(|video| video.id)
            .collect();
        let mut paged = Vec::new();
        for offset in [0, 3, 6] {
            let page = test.history.page(&query, offset, 3).unwrap();
            assert_eq!(page.total, 7, "the total counts every page, filtered");
            paged.extend(page.videos.iter().map(|video| video.id));
        }
        assert_eq!(paged, whole, "pages neither repeat nor skip a video");
        let past_the_end = test.history.page(&query, 9, 3).unwrap();
        assert!(past_the_end.videos.is_empty());
        assert_eq!(past_the_end.total, 7);
    }

    /// Records a video of `kind` downloaded at `downloaded_at`, its file made in the test's folder unless `missing`.
    fn record_kind(
        test: &TestHistory,
        video_id: &str,
        kind: DownloadKind,
        downloaded_at: i64,
        missing: bool,
    ) -> i64 {
        let mut subject = video("youtube", video_id, video_id, "x", 1.0);
        match kind {
            DownloadKind::Post => {
                subject.post = Some(PostContents {
                    images: 1,
                    videos: 0,
                })
            }
            DownloadKind::File => subject.files = Some(FileContents { count: 1, bytes: 1 }),
            _ => {}
        }
        let file = test.folder.path().join(format!("{video_id}.bin"));
        if !missing {
            fs::write(&file, b"x").unwrap();
        }
        let mut saved = download("p", downloaded_at, 1);
        saved.file_path = file.display().to_string();
        saved.kind = kind;
        test.history.record(&subject, &saved).unwrap();
        test.history
            .search(&HistoryQuery::default())
            .unwrap()
            .iter()
            .find(|found| found.video.video_id == video_id)
            .unwrap()
            .id
    }

    fn sorted_titles(test: &TestHistory, query: HistoryQuery) -> Vec<String> {
        let query = HistoryQuery {
            sort: HistorySort::Title,
            ..query
        };
        titles(&test.history.search(&query).unwrap())
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn filters_by_kind() {
        let test = TestHistory::new();
        record_kind(&test, "a-video", DownloadKind::Video, 1, false);
        record_kind(&test, "b-audio", DownloadKind::Audio, 2, false);
        record_kind(&test, "c-post", DownloadKind::Post, 3, false);
        record_kind(&test, "d-file", DownloadKind::File, 4, false);
        // Downloaded again as audio: it's both.
        let mut again = download("Best Audio", 5, 1);
        again.kind = DownloadKind::Audio;
        test.history
            .record(&video("youtube", "a-video", "a-video", "x", 1.0), &again)
            .unwrap();

        let of_kind = |kind| {
            sorted_titles(
                &test,
                HistoryQuery {
                    kind: Some(kind),
                    ..HistoryQuery::default()
                },
            )
        };
        assert_eq!(of_kind(DownloadKind::Video), ["a-video"]);
        assert_eq!(of_kind(DownloadKind::Audio), ["a-video", "b-audio"]);
        assert_eq!(of_kind(DownloadKind::Post), ["c-post"]);
        assert_eq!(of_kind(DownloadKind::File), ["d-file"]);
    }

    #[test]
    fn filters_by_when_it_was_last_downloaded() {
        let test = TestHistory::new();
        record_kind(&test, "a", DownloadKind::Video, 100, false);
        record_kind(&test, "b", DownloadKind::Video, 200, false);
        record_kind(&test, "c", DownloadKind::Video, 300, false);
        // Its latest download is the one that counts.
        test.history
            .record(
                &video("youtube", "a", "a", "x", 1.0),
                &download("p", 400, 1),
            )
            .unwrap();
        let between = |from: Option<i64>, until: Option<i64>| {
            sorted_titles(
                &test,
                HistoryQuery {
                    downloaded_from: from,
                    downloaded_until: until,
                    ..HistoryQuery::default()
                },
            )
        };
        assert_eq!(between(Some(200), None), ["a", "b", "c"]);
        assert_eq!(between(Some(201), None), ["a", "c"]);
        assert_eq!(between(None, Some(300)), ["b"], "the end is left out");
        assert_eq!(between(Some(250), Some(350)), ["c"]);
    }

    #[test]
    fn lists_only_videos_whose_file_is_missing_page_after_page() {
        let test = TestHistory::new();
        for index in 0..5 {
            record_kind(
                &test,
                &format!("gone{index}"),
                DownloadKind::Video,
                index,
                true,
            );
            record_kind(
                &test,
                &format!("here{index}"),
                DownloadKind::Video,
                index,
                false,
            );
        }
        let query = HistoryQuery {
            only_missing: true,
            sort: HistorySort::Title,
            ..HistoryQuery::default()
        };
        let first = test.history.page(&query, 0, 3).unwrap();
        let second = test.history.page(&query, 3, 3).unwrap();
        assert_eq!((first.total, second.total), (5, 5));
        let paged: Vec<_> = first
            .videos
            .iter()
            .chain(&second.videos)
            .map(|video| video.video.title.as_str())
            .collect();
        assert_eq!(paged, ["gone0", "gone1", "gone2", "gone3", "gone4"]);
        assert!(paged.iter().all(|title| title.starts_with("gone")));

        // A file that comes back leaves the list from the next first page on, and one that goes joins it.
        fs::write(test.folder.path().join("gone0.bin"), b"x").unwrap();
        fs::remove_file(test.folder.path().join("here4.bin")).unwrap();
        let again = test.history.page(&query, 0, 10).unwrap();
        assert_eq!(
            titles(&again.videos),
            ["gone1", "gone2", "gone3", "gone4", "here4"]
        );
        assert_eq!(
            test.history.ids(&query).unwrap().len(),
            5,
            "ticking them all ticks those"
        );
    }

    #[test]
    fn gives_every_matching_videos_id_in_order() {
        let test = TestHistory::new();
        let a = record_kind(&test, "a", DownloadKind::Video, 1, false);
        let b = record_kind(&test, "b", DownloadKind::Audio, 2, false);
        let c = record_kind(&test, "c", DownloadKind::Video, 3, false);
        let query = HistoryQuery {
            sort: HistorySort::Downloaded,
            descending: true,
            ..HistoryQuery::default()
        };
        assert_eq!(test.history.ids(&query).unwrap(), [c, b, a]);
        let videos_only = HistoryQuery {
            kind: Some(DownloadKind::Video),
            ..query
        };
        assert_eq!(test.history.ids(&videos_only).unwrap(), [c, a]);
    }

    #[test]
    fn forgets_several_videos_at_once() {
        let test = TestHistory::new();
        let a = record_kind(&test, "a", DownloadKind::Video, 1, false);
        let b = record_kind(&test, "b", DownloadKind::Video, 2, false);
        record_kind(&test, "c", DownloadKind::Video, 3, false);

        assert_eq!(test.history.forget_all(&[a, b, 12345]).unwrap(), 2);
        assert_eq!(test.archive(), "youtube c\n");
        assert!(test.folder.path().join("a.bin").exists(), "files stay");
        assert_eq!(test.history.forget(a), Err(HistoryError::NotFound));
    }

    #[test]
    fn finds_the_files_to_show_in_their_folders() {
        let test = TestHistory::new();
        let a = record_kind(&test, "a", DownloadKind::Video, 1, false);
        let gone = record_kind(&test, "gone", DownloadKind::Video, 2, true);
        let files = test.history.latest_files(&[a, gone, 999]).unwrap();
        assert_eq!(
            files,
            [test.folder.path().join("a.bin")],
            "only files still there"
        );

        let in_folders = [
            PathBuf::from(r"C:\One\a.mkv"),
            PathBuf::from(r"C:\one\b.mkv"),
            PathBuf::from(r"C:\Two\c.mkv"),
        ];
        assert_eq!(folder_count(&in_folders), 2, "ignoring letter case");
        assert_eq!(folder_count(&[]), 0);
    }

    #[test]
    fn checks_whether_a_pages_files_are_still_there() {
        let test = TestHistory::new();
        let file = test.folder.path().join("here.mkv");
        fs::write(&file, b"video").unwrap();
        let mut here = download("p", 2, 5);
        here.file_path = file.display().to_string();
        test.history
            .record(&video("youtube", "here", "Here", "x", 1.0), &here)
            .unwrap();
        test.history
            .record(
                &video("youtube", "gone", "Gone", "x", 1.0),
                &download("p", 1, 5),
            )
            .unwrap();
        let page = test
            .history
            .page(
                &HistoryQuery {
                    sort: HistorySort::Title,
                    ..HistoryQuery::default()
                },
                0,
                10,
            )
            .unwrap();
        let exists: Vec<_> = page
            .videos
            .iter()
            .map(|video| (video.video.title.as_str(), video.last_download.file_exists))
            .collect();
        assert_eq!(exists, [("Gone", false), ("Here", true)]);
    }

    #[test]
    fn lists_a_videos_one_file() {
        let test = TestHistory::new();
        let id = test
            .history
            .record(
                &video("youtube", "a", "A", "x", 1.0),
                &download("Best Video", 1, 10),
            )
            .unwrap();
        let files = test.history.files_of(id).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].size, Some(10));
        assert!(!files[0].exists);
        assert_eq!(
            test.history
                .last_download("youtube", "a")
                .unwrap()
                .unwrap()
                .file_count,
            1
        );
    }
}
