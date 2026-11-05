//! Finding copies of a tool on this PC: the app's own in `tools\`, and the first one on PATH.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::ToolId;
use super::catalog::{executable_name, spec};

/// A copy of a tool that could be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub path: PathBuf,
    /// The app's own copy, left from an earlier setup.
    pub is_managed: bool,
}

/// The copies of a tool to offer at setup, best first: the app's own copy, then the first one on PATH.
pub(crate) fn candidates(
    id: ToolId,
    tools_folder: &Path,
    path_variable: Option<&OsStr>,
) -> Vec<Candidate> {
    let mut found = Vec::new();
    let managed = managed_executable(tools_folder, id);
    if is_complete(id, &managed) {
        found.push(Candidate {
            path: managed.clone(),
            is_managed: true,
        });
    }
    if let Some(path_variable) = path_variable
        && let Ok(on_path) = which::which_in(spec(id).command, Some(path_variable), tools_folder)
        && on_path != managed
        && is_complete(id, &on_path)
    {
        found.push(Candidate {
            path: on_path,
            is_managed: false,
        });
    }
    found
}

/// Where the app's own copy of a tool lives.
pub(crate) fn managed_executable(tools_folder: &Path, id: ToolId) -> PathBuf {
    let tool = spec(id);
    tools_folder
        .join(tool.folder)
        .join(executable_name(tool.command))
}

/// yt-dlp looks for ffprobe beside ffmpeg, so a copy of ffmpeg only counts with ffprobe in the same folder.
pub(crate) fn ffprobe_beside(ffmpeg: &Path) -> PathBuf {
    ffmpeg.with_file_name(executable_name("ffprobe"))
}

fn is_complete(id: ToolId, executable: &Path) -> bool {
    executable.is_file() && (id != ToolId::Ffmpeg || ffprobe_beside(executable).is_file())
}

#[cfg(all(test, windows))]
mod tests {
    use std::fs;

    use super::*;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"").unwrap();
    }

    #[test]
    fn finds_a_copy_on_path() {
        let folder = tempfile::tempdir().unwrap();
        let bin = folder.path().join("bin");
        touch(&bin.join("yt-dlp.exe"));

        let found = candidates(
            ToolId::YtDlp,
            &folder.path().join("tools"),
            Some(bin.as_os_str()),
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path.file_name(), Some(OsStr::new("yt-dlp.exe")));
        assert!(!found[0].is_managed);
    }

    #[test]
    fn needs_ffprobe_beside_ffmpeg() {
        let folder = tempfile::tempdir().unwrap();
        let bin = folder.path().join("bin");
        let tools = folder.path().join("tools");
        touch(&bin.join("ffmpeg.exe"));
        assert!(candidates(ToolId::Ffmpeg, &tools, Some(bin.as_os_str())).is_empty());

        touch(&bin.join("ffprobe.exe"));
        assert_eq!(
            candidates(ToolId::Ffmpeg, &tools, Some(bin.as_os_str())).len(),
            1
        );
    }

    #[test]
    fn offers_the_apps_own_copy_first() {
        let folder = tempfile::tempdir().unwrap();
        let bin = folder.path().join("bin");
        let tools = folder.path().join("tools");
        touch(&bin.join("deno.exe"));
        touch(&tools.join("deno").join("deno.exe"));

        let found = candidates(ToolId::Deno, &tools, Some(bin.as_os_str()));
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].path, managed_executable(&tools, ToolId::Deno));
        assert!(found[0].is_managed);
    }

    #[test]
    fn finds_nothing_without_copies() {
        let folder = tempfile::tempdir().unwrap();
        assert!(
            candidates(
                ToolId::YtDlp,
                folder.path(),
                Some(folder.path().as_os_str())
            )
            .is_empty()
        );
        assert!(candidates(ToolId::YtDlp, folder.path(), None).is_empty());
    }
}
