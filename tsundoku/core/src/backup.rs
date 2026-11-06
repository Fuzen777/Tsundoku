//! Back Up and Restore, in Settings → About: the settings (with the presets and folder rules) and the history in one
//! zip file, to move them to another PC or keep them safe. The tools, their caches, the logs, the queue and the pending
//! links stay out. Tsundoku keeps no cookies of its own (the tools read them from the browser each time), so a backup
//! holds none.
//!
//! A backup holds `manifest.json` (`Manifest`), `settings.json` without the tools' setup, `tsundoku.db` with only the
//! history's rows, and `archive.txt`.

use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use zip::result::ZipError;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::app_info::compare_versions;
use crate::atomic_file::write_atomically;
use crate::data_folder::DataPaths;
use crate::database::Database;
use crate::history::HistoryError;
use crate::settings::{Settings, SettingsStore};

const MANIFEST_ENTRY: &str = "manifest.json";
const SETTINGS_ENTRY: &str = "settings.json";
const DATABASE_ENTRY: &str = "tsundoku.db";
const ARCHIVE_ENTRY: &str = "archive.txt";

/// How a backup is laid out. A version that changes the layout raises it, and older versions refuse its backups.
const FORMAT: u32 = 1;

/// What a backup says about itself, in its `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: u32,
    /// The version of Tsundoku that made it.
    pub app_version: String,
    /// The version of its database's tables (`PRAGMA user_version`).
    pub schema_version: i64,
    /// When it was made, in seconds since 1970.
    pub created_at: u64,
}

/// Why making or restoring a backup failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum BackupError {
    /// The backup couldn't be written where it was asked.
    #[error("couldn't save the backup: {detail}")]
    Write { detail: String },
    /// The file couldn't be opened.
    #[error("couldn't open the backup: {detail}")]
    Read { detail: String },
    /// The file isn't a Tsundoku backup, or it's damaged.
    #[error("not a Tsundoku backup: {detail}")]
    NotABackup { detail: String },
    /// Made by a newer Tsundoku, whose settings or history this one may not read.
    #[error("made by Tsundoku {version}")]
    NewerVersion { version: String },
    /// The current data couldn't be saved first, or replaced. Nothing was changed.
    #[error("couldn't restore: {detail}")]
    Storage { detail: String },
}

/// What a backup is made from, and what restoring one replaces.
pub struct BackupData<'a> {
    pub paths: &'a DataPaths,
    pub settings: &'a SettingsStore,
    pub database: &'a Database,
    /// The running app's version, from package.json.
    pub app_version: &'a str,
    /// A portable copy, which keeps a download folder that isn't a full path beside itself.
    pub portable: bool,
}

/// Saves a backup to `destination`, written beside it first so a failure never leaves half a backup there.
pub fn create(data: &BackupData, destination: &Path) -> Result<(), BackupError> {
    let working = WorkingFolder::new(data.paths).map_err(write_error)?;
    let database_copy = working.path().join(DATABASE_ENTRY);
    let schema_version = copy_history(data.database, &database_copy)?;
    let manifest = Manifest {
        format: FORMAT,
        app_version: data.app_version.to_owned(),
        schema_version,
        created_at: seconds_since_epoch(),
    };
    let archive = match fs::read(data.paths.archive_file()) {
        Ok(archive) => Some(archive),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(write_error(error)),
    };

    let mut temporary_name = destination.file_name().unwrap_or_default().to_os_string();
    temporary_name.push(".tmp");
    let temporary = destination.with_file_name(temporary_name);
    let written = write_zip(
        &temporary,
        &manifest,
        &settings_to_back_up(&data.settings.get()),
        &database_copy,
        archive.as_deref(),
    )
    // On Windows, rename replaces a file already there, which the save dialog asked about.
    .and_then(|()| fs::rename(&temporary, destination));
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written.map_err(write_error)
}

/// Replaces the settings (with the presets and folder rules) and the history with a backup's, after saving the
/// current ones as an automatic backup in the data folder's `backups\`, whose path it returns, so a wrong restore can
/// be undone. Which copies of the tools run and Start with Windows stay as they are: they're about this PC. The app
/// should restart afterwards, so everything reads what was restored.
pub fn restore(data: &BackupData, backup: &Path) -> Result<PathBuf, BackupError> {
    let contents = BackupContents::read(backup, data.paths, data.app_version)?;

    let automatic = data.paths.backups_folder().join(format!(
        "Tsundoku Backup {} (Before Restore).zip",
        local_timestamp()
    ));
    fs::create_dir_all(data.paths.backups_folder()).map_err(storage_error)?;
    create(data, &automatic).map_err(|error| BackupError::Storage {
        detail: format!("couldn't save the current data first: {error}"),
    })?;

    let current = data.settings.get();
    let restored = settings_to_restore(contents.settings, &current, data.portable);
    data.settings
        .update(|settings| *settings = restored)
        .map_err(storage_error)?;
    if let Err(error) = replace_history(data.database, &contents.database) {
        // Best effort: the history stayed as it was, so the settings go back too.
        let _ = data.settings.update(|settings| *settings = current);
        return Err(error);
    }
    if let Some(archive) = contents.archive {
        // The history opens on the next start rewriting archive.txt from the database anyway.
        let _ = write_atomically(&data.paths.archive_file(), &archive);
    }
    Ok(automatic)
}

/// A consistent copy of the database, made with SQLite's `VACUUM INTO` while it's open, keeping only the history:
/// the queue and the pending links are about this PC's downloads in progress. Returns its tables' version.
fn copy_history(database: &Database, copy: &Path) -> Result<i64, BackupError> {
    let copy_name = path_text(copy).map_err(write_error)?;
    let schema_version = {
        let connection = database.lock();
        connection
            .execute("VACUUM INTO ?1", [copy_name])
            .map_err(write_error)?;
        connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(write_error)?
    };
    let copy = Connection::open(copy).map_err(write_error)?;
    // One file, without the write-ahead log beside it.
    copy.pragma_update_and_check(None, "journal_mode", "DELETE", |row| {
        row.get::<_, String>(0)
    })
    .map_err(write_error)?;
    copy.execute_batch("DELETE FROM queue_items; DELETE FROM pending_links; VACUUM;")
        .map_err(write_error)?;
    Ok(schema_version)
}

/// The settings as `settings.json` has them, without which copies of the tools run, which is about this PC.
fn settings_to_back_up(settings: &Settings) -> Vec<u8> {
    let mut value = serde_json::to_value(settings).expect("settings always serialize to JSON");
    if let Some(fields) = value.as_object_mut() {
        fields.remove("tools");
    }
    serde_json::to_vec_pretty(&value).expect("settings always serialize to JSON")
}

/// A backup's settings, with what's about this PC kept from `current`: the tools' setup, Start with Windows,
/// whose entry among Windows' startup apps is only changed when the setting is saved, and the version that ran last. A portable copy's download
/// folder beside it ("Downloads") goes back to the default in a copy that isn't portable.
fn settings_to_restore(mut restored: Settings, current: &Settings, portable: bool) -> Settings {
    restored.tools = current.tools.clone();
    restored.background.start_with_windows = current.background.start_with_windows;
    // What's New follows the version that ran here, not the one the backup was made with.
    restored.last_run_version = current.last_run_version.clone();
    let folder_beside_app = restored
        .downloads
        .folder
        .as_deref()
        .is_some_and(|folder| Path::new(folder.trim()).is_relative());
    if folder_beside_app && !portable {
        restored.downloads.folder = None;
    }
    restored
}

fn write_zip(
    path: &Path,
    manifest: &Manifest,
    settings: &[u8],
    database: &Path,
    archive: Option<&[u8]>,
) -> io::Result<()> {
    let mut zip = ZipWriter::new(File::create(path)?);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let manifest = serde_json::to_vec_pretty(manifest).expect("the manifest serializes to JSON");
    for (name, contents) in [(MANIFEST_ENTRY, &manifest[..]), (SETTINGS_ENTRY, settings)] {
        zip.start_file(name, options).map_err(io::Error::other)?;
        zip.write_all(contents)?;
    }
    zip.start_file(DATABASE_ENTRY, options)
        .map_err(io::Error::other)?;
    io::copy(&mut File::open(database)?, &mut zip)?;
    if let Some(archive) = archive {
        zip.start_file(ARCHIVE_ENTRY, options)
            .map_err(io::Error::other)?;
        zip.write_all(archive)?;
    }
    zip.finish().map_err(io::Error::other)?.sync_all()
}

/// A backup, read and checked before anything is replaced.
struct BackupContents {
    settings: Settings,
    /// Its database, upgraded to this version's tables, in `_working`.
    database: PathBuf,
    archive: Option<Vec<u8>>,
    _working: WorkingFolder,
}

impl BackupContents {
    fn read(backup: &Path, paths: &DataPaths, app_version: &str) -> Result<Self, BackupError> {
        let file = File::open(backup).map_err(|error| BackupError::Read {
            detail: error.to_string(),
        })?;
        let mut zip = ZipArchive::new(BufReader::new(file)).map_err(not_a_backup)?;
        let manifest: Manifest = serde_json::from_slice(&required_entry(&mut zip, MANIFEST_ENTRY)?)
            .map_err(not_a_backup)?;
        if manifest.format > FORMAT
            || compare_versions(&manifest.app_version, app_version) == Ordering::Greater
        {
            return Err(BackupError::NewerVersion {
                version: manifest.app_version,
            });
        }
        let settings = Settings::from_json(&required_entry(&mut zip, SETTINGS_ENTRY)?)
            .ok_or_else(|| not_a_backup("its settings can't be read"))?;

        let working = WorkingFolder::new(paths).map_err(storage_error)?;
        let database = working.path().join(DATABASE_ENTRY);
        let mut database_file = File::create(&database).map_err(storage_error)?;
        io::copy(
            &mut zip.by_name(DATABASE_ENTRY).map_err(missing_or_damaged)?,
            &mut database_file,
        )
        .map_err(not_a_backup)?;
        drop(database_file);
        // Opening it brings a backup from an older version up to this one's tables, and refuses one from a newer
        // version. It's closed again before its rows are copied.
        Database::open(&database).map_err(|error| match error {
            HistoryError::NewerVersion { .. } => BackupError::NewerVersion {
                version: manifest.app_version.clone(),
            },
            error => not_a_backup(error),
        })?;
        let archive = match zip.by_name(ARCHIVE_ENTRY) {
            Ok(mut entry) => {
                let mut archive = Vec::new();
                entry.read_to_end(&mut archive).map_err(not_a_backup)?;
                Some(archive)
            }
            Err(ZipError::FileNotFound) => None,
            Err(error) => return Err(not_a_backup(error)),
        };
        Ok(Self {
            settings,
            database,
            archive,
            _working: working,
        })
    }
}

fn required_entry<R: Read + io::Seek>(
    zip: &mut ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>, BackupError> {
    let mut contents = Vec::new();
    zip.by_name(name)
        .map_err(missing_or_damaged)?
        .read_to_end(&mut contents)
        .map_err(not_a_backup)?;
    Ok(contents)
}

/// Replaces the history's rows with those of the database at `backup`, in one transaction. Finished downloads leave
/// the queue: they name their download in the history by its ID, which the backup's history may give another.
fn replace_history(database: &Database, backup: &Path) -> Result<(), BackupError> {
    let backup = path_text(backup).map_err(storage_error)?;
    let mut connection = database.lock();
    connection
        .execute("ATTACH DATABASE ?1 AS backup", [backup])
        .map_err(storage_error)?;
    let replaced = connection.transaction().and_then(|transaction| {
        // Both databases have every migration's tables, so their columns come in the same order. Deleting the videos
        // deletes their downloads and files too.
        transaction.execute_batch(
            "DELETE FROM main.videos;
            INSERT INTO main.videos SELECT * FROM backup.videos;
            INSERT INTO main.downloads SELECT * FROM backup.downloads;
            INSERT INTO main.download_files SELECT * FROM backup.download_files;
            DELETE FROM main.queue_items WHERE state = 'done';",
        )?;
        transaction.commit()
    });
    let detached = connection.execute("DETACH DATABASE backup", []);
    replaced.map_err(storage_error)?;
    detached.map_err(storage_error)?;
    Ok(())
}

/// A folder in the data folder's `backups\` for a backup's files while it's made or read, deleted afterwards.
struct WorkingFolder(PathBuf);

impl WorkingFolder {
    fn new(paths: &DataPaths) -> io::Result<Self> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = paths
            .backups_folder()
            .join(format!(".working-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for WorkingFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn path_text(path: &Path) -> io::Result<&str> {
    path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} isn't valid Unicode", path.display()),
        )
    })
}

fn seconds_since_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

/// The local date and time, as "2026-11-06 15.30.12", for an automatic backup's name.
#[cfg(windows)]
fn local_timestamp() -> String {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;

    // SAFETY: GetLocalTime only writes the SYSTEMTIME it's given, which lives through the call.
    let time = unsafe {
        let mut time: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut time);
        time
    };
    format!(
        "{:04}-{:02}-{:02} {:02}.{:02}.{:02}",
        time.wYear, time.wMonth, time.wDay, time.wHour, time.wMinute, time.wSecond
    )
}

#[cfg(not(windows))]
fn local_timestamp() -> String {
    seconds_since_epoch().to_string()
}

fn write_error(error: impl ToString) -> BackupError {
    BackupError::Write {
        detail: error.to_string(),
    }
}

fn not_a_backup(error: impl ToString) -> BackupError {
    BackupError::NotABackup {
        detail: error.to_string(),
    }
}

fn missing_or_damaged(error: ZipError) -> BackupError {
    match error {
        ZipError::FileNotFound => not_a_backup("a file is missing from it"),
        error => not_a_backup(error),
    }
}

fn storage_error(error: impl ToString) -> BackupError {
    BackupError::Storage {
        detail: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Theme, ToolSetup};
    use crate::tools::ToolId;

    /// A data folder with its settings and database, as the app opens them.
    struct TestData {
        folder: tempfile::TempDir,
        paths: DataPaths,
        settings: SettingsStore,
        database: Database,
    }

    impl TestData {
        fn new() -> Self {
            let folder = tempfile::tempdir().unwrap();
            let paths = DataPaths::new(folder.path().join("data"));
            fs::create_dir_all(paths.root()).unwrap();
            let settings = SettingsStore::open(paths.settings_file()).unwrap();
            let database = Database::open(&paths.database_file()).unwrap();
            Self {
                folder,
                paths,
                settings,
                database,
            }
        }

        fn data(&self) -> BackupData<'_> {
            BackupData {
                paths: &self.paths,
                settings: &self.settings,
                database: &self.database,
                app_version: "0.1.35",
                portable: false,
            }
        }

        fn add_video(&self, video_id: &str) {
            let connection = self.database.lock();
            connection
                .execute(
                    "INSERT INTO videos (site, site_name, video_id, url, title, first_downloaded_at)
                    VALUES ('youtube', 'youtube.com', ?1, 'https://youtu.be/' || ?1, ?1, 1)",
                    [video_id],
                )
                .unwrap();
            let video = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO downloads (video, preset, file_path, downloaded_at)
                    VALUES (?1, 'best', 'D:\\Videos\\' || ?2 || '.mkv', 2)",
                    rusqlite::params![video, video_id],
                )
                .unwrap();
            fs::write(
                self.paths.archive_file(),
                format!("{}youtube {video_id}\n", self.archive()),
            )
            .unwrap();
        }

        fn add_queue_item(&self, state: &str) {
            self.database
                .lock()
                .execute(
                    "INSERT INTO queue_items (video, preset, folder, state, overwrite, added_at)
                    VALUES ('{}', 'null', 'D:\\Videos', ?1, 0, 1)",
                    [state],
                )
                .unwrap();
        }

        fn video_ids(&self) -> Vec<String> {
            let connection = self.database.lock();
            let mut statement = connection
                .prepare("SELECT video_id FROM videos ORDER BY id")
                .unwrap();
            statement
                .query_map([], |row| row.get(0))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        }

        fn count(&self, table: &str) -> i64 {
            self.database
                .lock()
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap()
        }

        fn archive(&self) -> String {
            fs::read_to_string(self.paths.archive_file()).unwrap_or_default()
        }

        fn backup_path(&self) -> PathBuf {
            self.folder.path().join("Tsundoku Backup.zip")
        }
    }

    fn entry_names(path: &Path) -> Vec<String> {
        let zip = ZipArchive::new(File::open(path).unwrap()).unwrap();
        let mut names: Vec<String> = zip
            .file_names()
            .map(|name| name.unwrap().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_restored_backup_brings_back_the_settings_and_the_history() {
        let test = TestData::new();
        test.settings
            .update(|settings| {
                settings.appearance.theme = Theme::Dark;
                settings.downloads.folder = Some(r"D:\Videos".to_owned());
                settings.rules.list[0].folder = "Sites\\{site}".to_owned();
            })
            .unwrap();
        test.add_video("first");
        test.add_video("second");
        let backed_up = test.settings.get();
        create(&test.data(), &test.backup_path()).unwrap();

        // Afterwards, things change: another theme, a tool set up, one video forgotten and another downloaded, and the
        // queue holding a finished download and a waiting one.
        test.settings
            .update(|settings| {
                settings.appearance.theme = Theme::Light;
                settings.downloads.folder = None;
                settings.tools.set(
                    ToolId::YtDlp,
                    Some(ToolSetup::Managed {
                        checksum: Some("abc".to_owned()),
                    }),
                );
            })
            .unwrap();
        test.database
            .lock()
            .execute("DELETE FROM videos WHERE video_id = 'first'", [])
            .unwrap();
        test.add_video("third");
        test.add_queue_item("done");
        test.add_queue_item("waiting");
        let changed = test.settings.get();

        let automatic = restore(&test.data(), &test.backup_path()).unwrap();

        let restored = test.settings.get();
        assert_eq!(restored.appearance, backed_up.appearance);
        assert_eq!(restored.downloads, backed_up.downloads);
        assert_eq!(restored.rules, backed_up.rules);
        assert_eq!(restored.presets, backed_up.presets);
        assert_eq!(
            restored.tools, changed.tools,
            "this PC's tools stay set up as they were"
        );
        assert_eq!(test.video_ids(), ["first", "second"]);
        assert_eq!(test.count("downloads"), 2);
        assert_eq!(test.archive(), "youtube first\nyoutube second\n");
        assert_eq!(
            test.count("queue_items"),
            1,
            "the finished download leaves the queue, the waiting one stays"
        );
        assert_eq!(
            SettingsStore::open(test.paths.settings_file())
                .unwrap()
                .get(),
            restored,
            "the restored settings are saved"
        );

        // The automatic backup holds what was replaced, so restoring it undoes the restore.
        assert!(automatic.starts_with(test.paths.backups_folder()));
        restore(&test.data(), &automatic).unwrap();
        assert_eq!(test.video_ids(), ["second", "third"]);
        assert_eq!(test.settings.get().appearance.theme, Theme::Light);
        assert_eq!(test.settings.get().downloads.folder, None);

        let working: Vec<_> = fs::read_dir(test.paths.backups_folder())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with('.'))
            .collect();
        assert!(working.is_empty(), "no working folder is left: {working:?}");
    }

    #[test]
    fn a_backup_holds_the_history_without_the_queue_or_the_tools_setup() {
        let test = TestData::new();
        test.add_video("first");
        test.add_queue_item("waiting");
        test.settings
            .update(|settings| {
                settings.tools.set(
                    ToolId::Ffmpeg,
                    Some(ToolSetup::External {
                        path: r"C:\Users\Someone\ffmpeg.exe".into(),
                    }),
                )
            })
            .unwrap();

        create(&test.data(), &test.backup_path()).unwrap();

        assert_eq!(
            entry_names(&test.backup_path()),
            [
                "archive.txt",
                "manifest.json",
                "settings.json",
                "tsundoku.db"
            ]
        );
        let mut zip = ZipArchive::new(File::open(test.backup_path()).unwrap()).unwrap();
        let manifest: Manifest =
            serde_json::from_slice(&required_entry(&mut zip, MANIFEST_ENTRY).unwrap()).unwrap();
        assert_eq!(manifest.format, FORMAT);
        assert_eq!(manifest.app_version, "0.1.35");
        assert!(manifest.schema_version > 0);
        let settings: serde_json::Value =
            serde_json::from_slice(&required_entry(&mut zip, SETTINGS_ENTRY).unwrap()).unwrap();
        assert!(settings.get("tools").is_none());
        assert!(settings.get("presets").is_some());

        let copy = test.folder.path().join("copy.db");
        fs::write(&copy, required_entry(&mut zip, DATABASE_ENTRY).unwrap()).unwrap();
        let copy = Connection::open(copy).unwrap();
        let count = |table: &str| -> i64 {
            copy.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
        };
        assert_eq!(count("videos"), 1);
        assert_eq!(count("queue_items"), 0);
        assert!(
            !test.folder.path().join("Tsundoku Backup.zip.tmp").exists(),
            "the temporary file is gone"
        );
    }

    #[test]
    fn refuses_a_backup_from_a_newer_version() {
        let test = TestData::new();
        test.add_video("first");
        create(&test.data(), &test.backup_path()).unwrap();

        let older = BackupData {
            app_version: "0.1.34",
            ..test.data()
        };
        assert_eq!(
            restore(&older, &test.backup_path()),
            Err(BackupError::NewerVersion {
                version: "0.1.35".to_owned()
            })
        );
        assert_eq!(test.video_ids(), ["first"]);
        assert!(
            !test.paths.backups_folder().exists()
                || fs::read_dir(test.paths.backups_folder())
                    .unwrap()
                    .next()
                    .is_none(),
            "nothing is saved or replaced"
        );
    }

    #[test]
    fn refuses_a_file_that_isnt_a_backup() {
        let test = TestData::new();
        test.settings
            .update(|settings| settings.appearance.theme = Theme::Dark)
            .unwrap();

        let text = test.folder.path().join("notes.zip");
        fs::write(&text, "not a zip").unwrap();
        assert!(matches!(
            restore(&test.data(), &text),
            Err(BackupError::NotABackup { .. })
        ));

        let other_zip = test.folder.path().join("other.zip");
        let mut zip = ZipWriter::new(File::create(&other_zip).unwrap());
        zip.start_file("readme.txt", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"hello").unwrap();
        zip.finish().unwrap();
        assert!(matches!(
            restore(&test.data(), &other_zip),
            Err(BackupError::NotABackup { .. })
        ));

        assert!(matches!(
            restore(&test.data(), &test.folder.path().join("missing.zip")),
            Err(BackupError::Read { .. })
        ));
        assert_eq!(test.settings.get().appearance.theme, Theme::Dark);
    }

    #[test]
    fn a_portable_copys_download_folder_beside_it_only_stays_in_a_portable_copy() {
        let mut backed_up = Settings::default();
        backed_up.downloads.folder = Some("Downloads".to_owned());
        backed_up.background.start_with_windows = true;
        let mut current = Settings::default();
        current
            .tools
            .set(ToolId::Deno, Some(ToolSetup::Managed { checksum: None }));

        let installed = settings_to_restore(backed_up.clone(), &current, false);
        assert_eq!(installed.downloads.folder, None);
        assert!(
            !installed.background.start_with_windows,
            "Start with Windows stays as this PC has it"
        );
        assert_eq!(installed.tools, current.tools);

        let portable = settings_to_restore(backed_up, &current, true);
        assert_eq!(portable.downloads.folder.as_deref(), Some("Downloads"));
    }
}
