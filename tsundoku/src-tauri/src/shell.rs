//! The one place that opens files and folders, or shows files in their folder, through Windows' shell.
//!
//! Folders open in Windows' default folder app (File Explorer, unless another file manager took over), with File
//! Explorer itself (`explorer.exe`) as the fallback. A file manager that was set as the folder handler and then
//! uninstalled leaves the default verb naming a program that's gone, and `ShellExecute` and `SHOpenFolderAndSelectItems`
//! then show Windows' "Application not found" and wait until it's closed: the default app is only asked in ways that
//! show nothing (`folder_app`). A file still opens in the app Windows associates with it, which can wait on a dialog the
//! same way, so each function here runs only on a blocking thread (`in_background`), never in a sync command, which
//! Tauri runs on the window's own thread.

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

/// Shows the files in their folders: a window for each folder they're in, in their order. In the default folder app,
/// with all of that folder's files selected; when it can't (`folder_app::show`), in File Explorer with the first of
/// them selected, since `explorer.exe /select` takes one. Fails before opening anything if one of them is gone.
pub fn show_in_folders(paths: &[PathBuf]) -> io::Result<()> {
    for path in paths {
        check_exists(path)?;
    }
    #[cfg(windows)]
    {
        for (folder, files) in by_folder(paths) {
            let folder = shell_path(folder);
            let files: Vec<PathBuf> = files.into_iter().map(shell_path).collect();
            if let Err(reason) = folder_app::show(&folder, &files) {
                using_file_explorer(&folder, &reason);
                explorer(&select_argument(&files[0]))?;
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        tauri_plugin_opener::reveal_items_in_dir(paths).map_err(io::Error::other)
    }
}

/// Opens a folder in the default folder app, or in File Explorer when that fails (`folder_app::open`).
pub fn open_folder(folder: &Path) -> io::Result<()> {
    check_exists(folder)?;
    #[cfg(windows)]
    {
        let folder = shell_path(folder);
        folder_app::open(&folder).or_else(|reason| {
            using_file_explorer(&folder, &reason);
            explorer(&folder_argument(&folder))
        })
    }
    #[cfg(not(windows))]
    {
        tauri_plugin_opener::open_path(folder, None::<&str>).map_err(io::Error::other)
    }
}

/// Opens a file in the app Windows associates with it, or a folder as `open_folder` does.
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

/// Says in the app's output why a folder goes to File Explorer rather than the default folder app.
#[cfg(windows)]
fn using_file_explorer(folder: &Path, reason: &str) {
    eprintln!(
        "Opening {} in File Explorer: the default folder app {reason}",
        folder.display()
    );
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

/// The path as Windows' shell reads it: without the `\\?\` prefix of a verbatim path, which File Explorer doesn't take.
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

/// The paths by the folder they're in: the folders in the order their first path comes, each with its paths in their
/// order, folders compared ignoring case as Windows does.
#[cfg(windows)]
fn by_folder(paths: &[PathBuf]) -> Vec<(&Path, Vec<&Path>)> {
    let mut folders: Vec<(&Path, Vec<&Path>)> = Vec::new();
    let mut places = std::collections::HashMap::new();
    for path in paths {
        let folder = path.parent().unwrap_or(path);
        let place = *places
            .entry(folder.to_string_lossy().to_lowercase())
            .or_insert_with(|| {
                folders.push((folder, Vec::new()));
                folders.len() - 1
            });
        folders[place].1.push(path);
    }
    folders
}

/// Whether a folder's default verb can run: a COM handler (`delegate`) takes it with or without a program, else its
/// `program` must be there. Without either, the shell would show "Application not found".
#[cfg(windows)]
fn verb_can_run(program: Option<&Path>, delegate: bool, exists: impl Fn(&Path) -> bool) -> bool {
    delegate || program.is_some_and(exists)
}

/// Windows' default folder app: what a folder's default verb runs (File Explorer, unless another file manager took
/// over), asked only in ways that can't show "Application not found" when that program is gone.
#[cfg(windows)]
mod folder_app {
    use std::path::{Path, PathBuf};

    use windows::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
    };
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::{
        ASSOCF_NONE, ASSOCSTR, ASSOCSTR_DELEGATEEXECUTE, ASSOCSTR_EXECUTABLE,
        BHID_AssociationArray, ILCreateFromPathW, ILFree, IQueryAssociations, IShellItem,
        SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHCreateItemFromParsingName, SHELLEXECUTEINFOW,
        SHOpenFolderAndSelectItems, ShellExecuteExW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{HSTRING, PCWSTR, PWSTR};

    /// Opens the folder with its default verb, as a double-click would. `SEE_MASK_FLAG_NO_UI` makes a missing program
    /// fail (`CO_E_APPNOTFOUND`) instead of showing "Application not found"; `SEE_MASK_NOASYNC` waits for the verb to
    /// start, since this thread ends its COM right after.
    pub fn open(folder: &Path) -> Result<(), String> {
        let _com = Com::start();
        let file = HSTRING::from(folder.as_os_str());
        let mut request = SHELLEXECUTEINFOW {
            cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC,
            lpFile: PCWSTR(file.as_ptr()),
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        // SAFETY: `request` is a whole SHELLEXECUTEINFOW whose string (`file`) outlives the call.
        unsafe { ShellExecuteExW(&mut request) }
            .map_err(|error| format!("didn't open it ({error})"))
    }

    /// Opens the folder in the default folder app with the files selected. `SHOpenFolderAndSelectItems` shows
    /// "Application not found" itself when the default verb's program is gone, with no way to stop it, so the verb is
    /// checked first (`can_open`).
    pub fn show(folder: &Path, files: &[PathBuf]) -> Result<(), String> {
        let _com = Com::start();
        can_open(folder)?;
        let folder_id = ItemId::of(folder)?;
        let file_ids = files
            .iter()
            .map(|file| ItemId::of(file))
            .collect::<Result<Vec<_>, _>>()?;
        let selected: Vec<*const ITEMIDLIST> =
            file_ids.iter().map(|id| id.0.cast_const()).collect();
        // SAFETY: every ID was made by the shell and is freed only once the call has returned.
        unsafe { SHOpenFolderAndSelectItems(folder_id.0, Some(&selected), 0) }
            .map_err(|error| format!("didn't show the files ({error})"))
    }

    /// Whether the folder's default verb can run (`super::verb_can_run`), read from the associations the shell uses
    /// for this folder, which shows nothing. When the verb's program is gone, the shell names none: asking for it
    /// answers `CO_E_APPNOTFOUND`, though the verb's command still names it.
    pub fn can_open(folder: &Path) -> Result<(), String> {
        let path = HSTRING::from(folder.as_os_str());
        let asked = |error: windows::core::Error| format!("couldn't be asked about it ({error})");
        // SAFETY: plain COM calls, on a thread whose COM `show` started (or the test's).
        let associations: IQueryAssociations = unsafe {
            let item: IShellItem = SHCreateItemFromParsingName(&path, None).map_err(asked)?;
            item.BindToHandler(None, &BHID_AssociationArray)
                .map_err(asked)?
        };
        let program = association(&associations, ASSOCSTR_EXECUTABLE);
        let delegate = association(&associations, ASSOCSTR_DELEGATEEXECUTE).is_ok();
        if super::verb_can_run(
            program.as_deref().ok().map(Path::new),
            delegate,
            Path::is_file,
        ) {
            return Ok(());
        }
        Err(match program {
            Ok(program) => format!("is gone ({program})"),
            Err(error) => format!("is gone ({error})"),
        })
    }

    /// One of the default verb's values, as text.
    fn association(
        associations: &IQueryAssociations,
        value: ASSOCSTR,
    ) -> windows::core::Result<String> {
        let mut length = 0u32;
        // SAFETY: the first call only gives the length, the second writes at most that many characters into `text`.
        unsafe {
            associations.GetString(ASSOCF_NONE, value, PCWSTR::null(), None, &mut length)?;
            let mut text = vec![0u16; length as usize];
            associations.GetString(
                ASSOCF_NONE,
                value,
                PCWSTR::null(),
                Some(PWSTR(text.as_mut_ptr())),
                &mut length,
            )?;
            let end = text
                .iter()
                .position(|&unit| unit == 0)
                .unwrap_or(text.len());
            Ok(String::from_utf16_lossy(&text[..end]))
        }
    }

    /// COM on this thread, single-threaded as the shell's calls ask, ended when dropped. A thread already in another
    /// apartment keeps it (`RPC_E_CHANGED_MODE`), and the calls still work there.
    pub struct Com {
        started: bool,
    }

    impl Com {
        pub fn start() -> Self {
            // SAFETY: paired with `CoUninitialize` in `drop` when it succeeded, on the same thread.
            let result =
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
            Self {
                started: result.is_ok(),
            }
        }
    }

    impl Drop for Com {
        fn drop(&mut self) {
            if self.started {
                // SAFETY: this thread's `CoInitializeEx` succeeded (S_OK or S_FALSE, each needing its own call).
                unsafe { CoUninitialize() }
            }
        }
    }

    /// The shell's ID for a path, freed when dropped.
    struct ItemId(*mut ITEMIDLIST);

    impl ItemId {
        fn of(path: &Path) -> Result<Self, String> {
            // SAFETY: the path is a whole string for the call; the ID it gives is freed in `drop`.
            let id = unsafe { ILCreateFromPathW(&HSTRING::from(path.as_os_str())) };
            if id.is_null() {
                Err(format!("couldn't find {}", path.display()))
            } else {
                Ok(Self(id))
            }
        }
    }

    impl Drop for ItemId {
        fn drop(&mut self) {
            // SAFETY: made by ILCreateFromPathW and freed once.
            unsafe { ILFree(Some(self.0.cast_const())) }
        }
    }
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
    fn groups_the_files_by_folder_in_their_order() {
        let paths = [
            PathBuf::from(r"D:\Videos\b.mp4"),
            PathBuf::from(r"D:\Posts\1.jpg"),
            PathBuf::from(r"d:\videos\a.mp4"),
            PathBuf::from(r"D:\Posts\2.jpg"),
            PathBuf::from(r"D:\Videos\Sub\c.mp4"),
        ];
        assert_eq!(
            by_folder(&paths),
            [
                (
                    Path::new(r"D:\Videos"),
                    vec![Path::new(r"D:\Videos\b.mp4"), Path::new(r"d:\videos\a.mp4")]
                ),
                (
                    Path::new(r"D:\Posts"),
                    vec![Path::new(r"D:\Posts\1.jpg"), Path::new(r"D:\Posts\2.jpg")]
                ),
                (
                    Path::new(r"D:\Videos\Sub"),
                    vec![Path::new(r"D:\Videos\Sub\c.mp4")]
                ),
            ]
        );
    }

    #[test]
    fn a_folder_verb_runs_with_its_program_or_a_com_handler() {
        let explorer = Path::new(r"C:\WINDOWS\Explorer.exe");
        let gone = Path::new(r"C:\Program Files\GPSoftware\Directory Opus\dopusrt.exe");
        let there = |path: &Path| path == explorer;
        // Windows' own: File Explorer's program and its COM handler.
        assert!(verb_can_run(Some(explorer), true, there));
        // Another file manager, installed.
        assert!(verb_can_run(Some(explorer), false, there));
        // A COM handler alone, as File Explorer's `explore` verb has.
        assert!(verb_can_run(None, true, there));
        // An uninstalled file manager: the shell names no program (CO_E_APPNOTFOUND), or one that's gone.
        assert!(!verb_can_run(None, false, there));
        assert!(!verb_can_run(Some(gone), false, there));
    }

    /// The default folder app on this PC, then as Windows ships it:
    /// `SHELL_LIVE_FOLDER=<folder holding two files or more> cargo test -p tsundoku --lib shell::tests::folder_app_live -- --ignored --nocapture`.
    /// Prints whether this PC's folder handler can open the folder, opening nothing. Then, with `HKEY_CLASSES_ROOT`
    /// pointed at the machine's own classes for this process only (`RegOverridePredefKey`; nothing is written), so a
    /// handler set for this user is left out, shows the folder's first two files, both selected, and opens the folder
    /// above it (the folder itself would only bring its window forward): two File Explorer windows, to close by hand.
    #[test]
    #[ignore = "opens File Explorer windows"]
    fn folder_app_live() {
        use windows::Win32::System::Registry::{
            HKEY, HKEY_CLASSES_ROOT, HKEY_LOCAL_MACHINE, KEY_READ, RegOpenKeyExW,
            RegOverridePredefKey,
        };

        let folder =
            PathBuf::from(std::env::var_os("SHELL_LIVE_FOLDER").expect("SHELL_LIVE_FOLDER"));
        let mut files: Vec<PathBuf> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file())
            .collect();
        files.sort();
        files.truncate(2);
        assert_eq!(files.len(), 2, "the folder needs two files");

        let _com = folder_app::Com::start();
        println!(
            "This PC's folder handler: {:?}",
            folder_app::can_open(&folder)
        );
        let mut machine_classes = HKEY::default();
        // SAFETY: plain registry calls; the override lasts as long as this test's process.
        unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                windows::core::w!(r"Software\Classes"),
                None,
                KEY_READ,
                &mut machine_classes,
            )
            .ok()
            .unwrap();
            RegOverridePredefKey(HKEY_CLASSES_ROOT, Some(machine_classes))
                .ok()
                .unwrap();
        }
        assert_eq!(folder_app::can_open(&folder), Ok(()));
        folder_app::show(&folder, &files).unwrap();
        let above = folder.parent().expect("a folder above SHELL_LIVE_FOLDER");
        folder_app::open(above).unwrap();
        println!(
            "Windows' own: showed {files:?}, then opened {}",
            above.display()
        );
    }
}
