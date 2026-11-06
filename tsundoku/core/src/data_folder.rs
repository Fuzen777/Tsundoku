//! Where the app keeps its data: `settings.json`, `tools\`, the history, the queue and the logs.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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
}

#[cfg(test)]
mod tests {
    use super::*;

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
