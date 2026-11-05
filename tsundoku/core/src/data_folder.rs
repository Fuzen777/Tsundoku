//! Where the app keeps its data: `settings.json` and `tools\`, and later the history and logs.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The environment variable a development build reads its data folder from. See AGENTS.md → Data and Testing.
pub const DATA_FOLDER_VARIABLE: &str = "TSUNDOKU_DATA_DIR";

/// Which build is running. A development build keeps its data apart from an installed copy's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildKind {
    Development,
    Release,
}

/// The data folder for this build: `%LocalAppData%\Tsundoku` for a release build, and for a development build the
/// folder named by `TSUNDOKU_DATA_DIR`, or `%LocalAppData%\Tsundoku Dev` without it. `None` only if Windows has no
/// local app data folder.
pub fn resolve(build: BuildKind) -> Option<PathBuf> {
    resolve_from(
        build,
        std::env::var_os(DATA_FOLDER_VARIABLE),
        dirs::data_local_dir(),
    )
}

fn resolve_from(
    build: BuildKind,
    folder_override: Option<OsString>,
    local_app_data: Option<PathBuf>,
) -> Option<PathBuf> {
    match build {
        BuildKind::Release => local_app_data.map(|folder| folder.join("Tsundoku")),
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

    /// The app's own copies of yt-dlp, ffmpeg and Deno, one folder each.
    pub fn tools_folder(&self) -> PathBuf {
        self.root.join("tools")
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

    #[test]
    fn release_builds_ignore_the_override() {
        let folder = resolve_from(
            BuildKind::Release,
            Some(r"D:\Dev\data".into()),
            local_app_data(),
        );
        assert_eq!(
            folder,
            Some(PathBuf::from(r"C:\Users\Someone\AppData\Local\Tsundoku"))
        );
    }

    #[test]
    fn development_builds_use_the_override() {
        let folder = resolve_from(
            BuildKind::Development,
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
            Some("D:/Dev/data".into()),
            local_app_data(),
        );
        assert_eq!(folder.unwrap().as_os_str(), r"D:\Dev\data");
    }

    #[test]
    fn development_builds_fall_back_to_their_own_folder() {
        let expected = Some(PathBuf::from(
            r"C:\Users\Someone\AppData\Local\Tsundoku Dev",
        ));
        assert_eq!(
            resolve_from(BuildKind::Development, None, local_app_data()),
            expected
        );
        assert_eq!(
            resolve_from(BuildKind::Development, Some("".into()), local_app_data()),
            expected
        );
    }
}
