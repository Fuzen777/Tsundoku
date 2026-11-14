//! The one place that opens files and folders, or shows files in their folder, through Windows' shell.
//!
//! Folders open in File Explorer itself (`explorer.exe`), never through the folder's default verb: when another file
//! manager was set as the folder handler and then uninstalled, `ShellExecute` and `SHOpenFolderAndSelectItems` show
//! Windows' "Application not found" and wait until it's closed. A file still opens in the app Windows associates with
//! it, which can wait on a dialog the same way, so each function here runs only on a blocking thread (`in_background`),
//! never in a sync command, which Tauri runs on the window's own thread.

use std::io;
use std::path::{Path, PathBuf};

/// Runs `open` on a blocking thread and waits for it there, so neither the window nor the async runtime's threads wait
/// on Windows' shell. `failed` turns a panic in `open` into the command's error.
pub async fn in_background<T: Send + 'static, E: Send + 'static>(
    open: impl FnOnce() -> Result<T, E> + Send + 'static,
    failed: impl FnOnce(String) -> E,
) -> Result<T, E> {
    tauri::async_runtime::spawn_blocking(open)
        .await
        .unwrap_or_else(|error| Err(failed(error.to_string())))
}

/// Shows the files in File Explorer: a window for each folder they're in, in their order, with the first of its files
/// selected. Fails before opening anything if one of them is gone.
pub fn show_in_folders(paths: &[PathBuf]) -> io::Result<()> {
    for path in paths {
        check_exists(path)?;
    }
    #[cfg(windows)]
    {
        for path in first_of_each_folder(paths) {
            explorer(&select_argument(&shell_path(path)))?;
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        tauri_plugin_opener::reveal_items_in_dir(paths).map_err(io::Error::other)
    }
}

/// Opens a folder in File Explorer.
pub fn open_folder(folder: &Path) -> io::Result<()> {
    check_exists(folder)?;
    #[cfg(windows)]
    {
        explorer(&folder_argument(&shell_path(folder)))
    }
    #[cfg(not(windows))]
    {
        tauri_plugin_opener::open_path(folder, None::<&str>).map_err(io::Error::other)
    }
}

/// Opens a file in the app Windows associates with it, or a folder in File Explorer.
pub fn open_file(file: &Path) -> io::Result<()> {
    if file.is_dir() {
        return open_folder(file);
    }
    check_exists(file)?;
    tauri_plugin_opener::open_path(file, None::<&str>).map_err(io::Error::other)
}

fn check_exists(path: &Path) -> io::Result<()> {
    if path.exists() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} isn't there anymore", path.display()),
        ))
    }
}

/// Starts File Explorer with `argument`, without waiting: it hands the window to the running Explorer and exits, with 1
/// even when it worked, so its exit code says nothing.
#[cfg(windows)]
fn explorer(argument: &str) -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    let windows_folder =
        std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    Command::new(windows_folder.join("explorer.exe"))
        // Explorer reads its command line itself: the path is quoted once, and Windows paths can't hold a quote.
        .raw_arg(argument)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
}

/// The path as File Explorer reads it: without the `\\?\` prefix of a verbatim path, which it doesn't take.
/// `\\?\C:\…` becomes `C:\…` and `\\?\UNC\server\share\…` becomes `\\server\share\…`; a relative path is made whole.
#[cfg(windows)]
fn shell_path(path: &Path) -> PathBuf {
    use std::path::{Component, Prefix};

    let mut components = path.components();
    if let Some(Component::Prefix(prefix)) = components.next() {
        let root = match prefix.kind() {
            Prefix::VerbatimDisk(drive) => Some(format!(r"{}:\", char::from(drive))),
            Prefix::VerbatimUNC(server, share) => Some(format!(
                r"\\{}\{}\",
                server.to_string_lossy(),
                share.to_string_lossy()
            )),
            _ => None,
        };
        if let Some(root) = root {
            // What follows the prefix starts at the root (`\Users\…`), which joins after the drive or the share.
            return Path::new(&root).join(components.as_path());
        }
    }
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Explorer's argument for opening the folder holding `file` with `file` selected.
#[cfg(windows)]
fn select_argument(file: &Path) -> String {
    format!("/select,{}", quoted(file))
}

/// Explorer's argument for opening `folder`.
#[cfg(windows)]
fn folder_argument(folder: &Path) -> String {
    quoted(folder)
}

/// The path in quotes, so a space or a comma (Explorer's separator) stays part of it. Explorer doesn't read a backslash
/// before a quote as an escape, so a drive's root (`"D:\"`) works as it is.
#[cfg(windows)]
fn quoted(path: &Path) -> String {
    format!("\"{}\"", path.display())
}

/// The first of the paths in each folder, in their order, folders compared ignoring case as Windows does.
#[cfg(windows)]
fn first_of_each_folder(paths: &[PathBuf]) -> Vec<&Path> {
    let mut folders = std::collections::HashSet::new();
    paths
        .iter()
        .filter(|path| {
            let folder = path.parent().unwrap_or(path);
            folders.insert(folder.to_string_lossy().to_lowercase())
        })
        .map(PathBuf::as_path)
        .collect()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn quotes_a_path_with_spaces_and_commas_once() {
        assert_eq!(
            select_argument(Path::new(
                r"D:\Videos\Me, at the zoo\Me at the zoo [jNQXAC9IVRw].mp4"
            )),
            r#"/select,"D:\Videos\Me, at the zoo\Me at the zoo [jNQXAC9IVRw].mp4""#
        );
        assert_eq!(
            folder_argument(Path::new(r"D:\Dev\tsundoku\downloads")),
            r#""D:\Dev\tsundoku\downloads""#
        );
    }

    #[test]
    fn drops_the_prefix_of_a_verbatim_path() {
        assert_eq!(
            shell_path(Path::new(r"\\?\C:\Users\Zen\Downloads\clip.mp4")),
            PathBuf::from(r"C:\Users\Zen\Downloads\clip.mp4")
        );
        assert_eq!(
            shell_path(Path::new(r"\\?\UNC\server\share\clip.mp4")),
            PathBuf::from(r"\\server\share\clip.mp4")
        );
        assert_eq!(shell_path(Path::new(r"\\?\D:\")), PathBuf::from(r"D:\"));
        assert_eq!(
            shell_path(Path::new(r"\\server\share\clip.mp4")),
            PathBuf::from(r"\\server\share\clip.mp4")
        );
        assert_eq!(
            shell_path(Path::new(r"D:\Videos\clip.mp4")),
            PathBuf::from(r"D:\Videos\clip.mp4")
        );
    }

    #[test]
    fn opens_one_window_per_folder_with_its_first_file() {
        let paths = [
            PathBuf::from(r"D:\Videos\b.mp4"),
            PathBuf::from(r"D:\Posts\1.jpg"),
            PathBuf::from(r"d:\videos\a.mp4"),
            PathBuf::from(r"D:\Posts\2.jpg"),
            PathBuf::from(r"D:\Videos\Sub\c.mp4"),
        ];
        assert_eq!(
            first_of_each_folder(&paths),
            [
                Path::new(r"D:\Videos\b.mp4"),
                Path::new(r"D:\Posts\1.jpg"),
                Path::new(r"D:\Videos\Sub\c.mp4"),
            ]
        );
    }
}
