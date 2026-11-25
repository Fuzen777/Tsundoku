//! Where the app keeps its data: `settings.json`, `tools\`, the history, the queue and the logs.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;
use ts_rs::TS;

/// The environment variable a development build reads its data folder from. See AGENTS.md → Data and Testing.
pub const DATA_FOLDER_VARIABLE: &str = "TSUNDOKU_DATA_DIR";

/// The file that makes a copy portable: beside `Tsundoku.exe`, it keeps the data in `PORTABLE_DATA_FOLDER` there
/// instead of the user's app data.
pub const PORTABLE_MARKER: &str = ".portable.txt";

/// A portable copy's data folder, beside its exe.
pub const PORTABLE_DATA_FOLDER: &str = "data";

static PORTABLE_FOLDER: OnceLock<PathBuf> = OnceLock::new();

/// The folder of a portable copy, whose exe is `exe`: its own folder when `PORTABLE_MARKER` is in it, `None` for an
/// installed copy.
pub fn portable_folder_of(exe: &Path) -> Option<PathBuf> {
    let folder = exe.parent()?;
    folder
        .join(PORTABLE_MARKER)
        .is_file()
        .then(|| folder.to_path_buf())
}

/// Records, once at startup, that this copy is portable and lives in `folder`.
pub fn set_portable_folder(folder: PathBuf) {
    let _ = PORTABLE_FOLDER.set(folder);
}

/// The folder this copy lives in when it's portable (`set_portable_folder`), `None` for an installed copy. A download
/// folder that isn't a full path is inside it, so it follows the copy to another drive.
pub fn portable_folder() -> Option<&'static Path> {
    PORTABLE_FOLDER.get().map(PathBuf::as_path)
}

/// Which build is running. A development build keeps its data apart from an installed copy's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildKind {
    Development,
    Release,
}

/// The data folder for this build. A release build uses `release_folder`: the app's own folder in `%LocalAppData%`,
/// named after its identifier, which the uninstaller offers to delete. (`%LocalAppData%\Tsundoku` is where the
/// installer puts the app itself.) A development build uses the folder named by `TSUNDOKU_DATA_DIR`, or
/// `%LocalAppData%\Tsundoku Dev` without it. `None` only if Windows has no local app data folder.
pub fn resolve(build: BuildKind, release_folder: PathBuf) -> Option<PathBuf> {
    resolve_from(
        build,
        release_folder,
        std::env::var_os(DATA_FOLDER_VARIABLE),
        dirs::data_local_dir(),
    )
}

fn resolve_from(
    build: BuildKind,
    release_folder: PathBuf,
    folder_override: Option<OsString>,
    local_app_data: Option<PathBuf>,
) -> Option<PathBuf> {
    match build {
        BuildKind::Release => Some(release_folder),
        BuildKind::Development => folder_override
            .filter(|folder| !folder.is_empty())
            // Rebuilt from its parts, so "D:/Dev/data" gets Windows' separators like the paths joined to it.
            .map(|folder| Path::new(&folder).components().collect())
            .or_else(|| local_app_data.map(|folder| folder.join("Tsundoku Dev"))),
    }
}

/// The files and folders inside the data folder.
#[derive(Debug, Clone)]
pub struct DataPaths {
    root: PathBuf,
}

impl DataPaths {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    /// The app's own copies of yt-dlp, ffmpeg, Deno and gallery-dl, one folder each, unless the first-run setup picked
    /// another folder for them (`ToolsSettings::folder`).
    pub fn tools_folder(&self) -> PathBuf {
        self.root.join("tools")
    }

    /// The tools' caches, when the first-run setup chose to keep them here (`ToolCaches::DataFolder`).
    pub fn cache_folder(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// The history's SQLite database.
    pub fn database_file(&self) -> PathBuf {
        self.root.join("tsundoku.db")
    }

    /// yt-dlp's download archive, rewritten from the history after every change.
    pub fn archive_file(&self) -> PathBuf {
        self.root.join("archive.txt")
    }

    /// One log per download, with yt-dlp's output.
    pub fn logs_folder(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// The automatic backups a restore makes of what it replaces, and the files of a backup while it's made or read.
    pub fn backups_folder(&self) -> PathBuf {
        self.root.join("backups")
    }

    /// The history's thumbnails (thumbnails.rs), kept so they don't go blank when a site's picture does.
    pub fn thumbnails_folder(&self) -> PathBuf {
        self.root.join("thumbnails")
    }

    /// The pictures picked by hand for watched sources (`watched::picture`).
    pub fn pictures_folder(&self) -> PathBuf {
        self.root.join("pictures")
    }

    /// Sites' icons (`site_icons`), read once and kept, one file per site.
    pub fn site_icons_folder(&self) -> PathBuf {
        self.root.join("site-icons")
    }

    /// The check window's browser profile (its cookies, such as a check passed, and its cache), kept apart from the
    /// app's own WebView.
    pub fn browser_folder(&self) -> PathBuf {
        self.root.join("browser")
    }

    /// Tsundoku's own sign-ins (`pixiv_account`): Pixiv's token, encrypted, and the config files gallery-dl reads it
    /// from for a moment. Never in a backup.
    pub fn accounts_folder(&self) -> PathBuf {
        self.root.join("accounts")
    }

    /// The sign-in window's browser profile, where the pixiv account stays signed in (for Fanbox and Pixiv's
    /// messages), apart from the check window's.
    pub fn accounts_browser_folder(&self) -> PathBuf {
        self.accounts_folder().join("browser")
    }

    /// What the data folder holds for the history, by part, as it is on the disk now.
    pub fn history_data(&self) -> HistoryData {
        let database = self.database_file();
        let database = ["", "-wal", "-shm"]
            .into_iter()
            .map(|ending| {
                let mut name = database.clone().into_os_string();
                name.push(ending);
                file_size(Path::new(&name))
            })
            .sum();
        let (thumbnails, thumbnail_count) = folder_contents(&self.thumbnails_folder());
        let (logs, log_count) = folder_contents(&self.logs_folder());
        HistoryData {
            database,
            thumbnails,
            thumbnail_count,
            logs,
            log_count,
            archive: file_size(&self.archive_file()),
        }
    }
}

/// What the data folder holds for the history (`DataPaths::history_data`): the tools, their caches, the settings and
/// the check window's profile aren't counted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryData {
    /// `tsundoku.db` with its `-wal` and `-shm` files, in bytes: the history, with the queue, Pending and Watched,
    /// which share it.
    #[ts(type = "number")]
    pub database: u64,
    /// The kept thumbnails (`thumbnails\`), in bytes, and how many.
    #[ts(type = "number")]
    pub thumbnails: u64,
    pub thumbnail_count: u32,
    /// The downloads' logs (`logs\`, kept 30 days), in bytes, and how many.
    #[ts(type = "number")]
    pub logs: u64,
    pub log_count: u32,
    /// yt-dlp's `archive.txt`, in bytes.
    #[ts(type = "number")]
    pub archive: u64,
}

/// A file's size, 0 when it isn't there.
fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map_or(0, |metadata| metadata.len())
}

/// The files in a folder and the folders inside it: their size together and how many. Links to folders aren't
/// followed, and a folder that isn't there holds nothing.
fn folder_contents(folder: &Path) -> (u64, u32) {
    let (mut bytes, mut count) = (0, 0);
    let mut folders = vec![folder.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                folders.push(entry.path());
            } else if let Some(length) = kind
                .is_file()
                .then(|| entry.metadata().ok())
                .flatten()
                .map(|metadata| metadata.len())
            {
                bytes += length;
                count += 1;
            }
        }
    }
    (bytes, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_what_the_data_folder_holds_for_the_history() {
        let root = tempfile::tempdir().unwrap();
        let paths = DataPaths::new(root.path().to_path_buf());
        assert_eq!(paths.history_data(), HistoryData::default());

        let write = |path: PathBuf, bytes: usize| {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, vec![b'x'; bytes]).unwrap();
        };
        write(paths.database_file(), 4096);
        write(root.path().join("tsundoku.db-wal"), 1000);
        write(root.path().join("tsundoku.db-shm"), 32);
        write(paths.thumbnails_folder().join("youtube-1.jpg"), 6000);
        write(paths.thumbnails_folder().join("youtube-2.jpg"), 5000);
        write(paths.logs_folder().join("1.log"), 300);
        write(paths.logs_folder().join("old").join("2.log"), 20);
        write(paths.archive_file(), 18);
        // Not the history's: the tools, the settings, the check window's profile.
        write(paths.tools_folder().join("yt-dlp").join("yt-dlp.exe"), 9999);
        write(paths.settings_file(), 999);
        write(paths.browser_folder().join("Cookies"), 999);

        assert_eq!(
            paths.history_data(),
            HistoryData {
                database: 4096 + 1000 + 32,
                thumbnails: 11_000,
                thumbnail_count: 2,
                logs: 320,
                log_count: 2,
                archive: 18,
            }
        );
    }

    fn local_app_data() -> Option<PathBuf> {
        Some(PathBuf::from(r"C:\Users\Someone\AppData\Local"))
    }

    fn release_folder() -> PathBuf {
        PathBuf::from(r"C:\Users\Someone\AppData\Local\io.github.fuzen777.tsundoku")
    }

    #[test]
    fn release_builds_use_the_apps_own_folder_and_ignore_the_override() {
        let folder = resolve_from(
            BuildKind::Release,
            release_folder(),
            Some(r"D:\Dev\data".into()),
            local_app_data(),
        );
        assert_eq!(folder, Some(release_folder()));
    }

    #[test]
    fn development_builds_use_the_override() {
        let folder = resolve_from(
            BuildKind::Development,
            release_folder(),
            Some(r"D:\Dev\data".into()),
            local_app_data(),
        );
        assert_eq!(folder, Some(PathBuf::from(r"D:\Dev\data")));
    }

    #[cfg(windows)]
    #[test]
    fn an_override_gets_windows_separators() {
        let folder = resolve_from(
            BuildKind::Development,
            release_folder(),
            Some("D:/Dev/data".into()),
            local_app_data(),
        );
        assert_eq!(folder.unwrap().as_os_str(), r"D:\Dev\data");
    }

    #[test]
    fn a_copy_is_portable_with_the_marker_beside_its_exe() {
        let folder = tempfile::tempdir().unwrap();
        let exe = folder.path().join("Tsundoku.exe");
        assert_eq!(portable_folder_of(&exe), None);

        std::fs::write(folder.path().join(PORTABLE_MARKER), "").unwrap();
        assert_eq!(portable_folder_of(&exe), Some(folder.path().to_path_buf()));

        std::fs::create_dir(folder.path().join("inside")).unwrap();
        assert_eq!(
            portable_folder_of(&folder.path().join("inside").join("Tsundoku.exe")),
            None,
            "only the exe's own folder counts"
        );
    }

    #[test]
    fn development_builds_fall_back_to_their_own_folder() {
        let expected = Some(PathBuf::from(
            r"C:\Users\Someone\AppData\Local\Tsundoku Dev",
        ));
        assert_eq!(
            resolve_from(
                BuildKind::Development,
                release_folder(),
                None,
                local_app_data()
            ),
            expected
        );
        assert_eq!(
            resolve_from(
                BuildKind::Development,
                release_folder(),
                Some("".into()),
                local_app_data()
            ),
            expected
        );
    }
}
