//! Writing what an archive holds: every entry's path made safe for Windows and kept inside the folder, its bytes
//! counted for progress and checked for a stop, its date kept; then putting it beside the archive, and the archive
//! away.

use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::Failure;
use crate::cancel::CancelToken;
use crate::rules::without_refused_characters;

/// How much is read at a time.
const BUFFER_SIZE: usize = 256 * 1024;

/// Names Windows keeps for devices, which no file may take, whatever its extension.
const DEVICE_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Where an archive's entries are written, and how far it has got.
pub(super) struct Output<'a> {
    folder: PathBuf,
    cancel: &'a CancelToken,
    /// Told the bytes written so far.
    progress: &'a mut dyn FnMut(u64),
    written: u64,
    /// Entries left out: a link, or a name that would leave the folder.
    pub(super) left_out: Vec<String>,
}

/// A file being written from an archive that hands its bytes over in pieces (rar).
pub(super) struct EntryFile {
    path: PathBuf,
    file: BufWriter<File>,
}

impl<'a> Output<'a> {
    pub(super) fn new(
        folder: &Path,
        cancel: &'a CancelToken,
        progress: &'a mut dyn FnMut(u64),
    ) -> Self {
        Self {
            folder: folder.to_path_buf(),
            cancel,
            progress,
            written: 0,
            left_out: Vec::new(),
        }
    }

    /// Where an entry named `name` goes, or `None` when it's left out.
    fn place_of(&mut self, name: &str) -> Option<PathBuf> {
        let place = safe_relative(name).map(|relative| self.folder.join(relative));
        if place.is_none() {
            self.left_out.push(name.to_owned());
        }
        place
    }

    pub(super) fn directory(&mut self, name: &str) -> Result<(), Failure> {
        if let Some(path) = self.place_of(name) {
            fs::create_dir_all(&path).map_err(|error| Failure::write(&path, &error))?;
        }
        Ok(())
    }

    /// Writes an entry whose bytes `data` gives. A read that fails is the archive's fault (`Failure::Damaged`), a
    /// write that fails the drive's.
    pub(super) fn file(
        &mut self,
        name: &str,
        data: &mut dyn Read,
        modified: Option<SystemTime>,
    ) -> Result<(), Failure> {
        let Some(mut entry) = self.open(name)? else {
            // Read through all the same: a solid archive's next entry comes after it.
            return io::copy(data, &mut io::sink())
                .map(|_| ())
                .map_err(|error| Failure::read(&error));
        };
        let mut buffer = vec![0; BUFFER_SIZE];
        loop {
            let read = match data.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    entry.discard();
                    return Err(Failure::read(&error));
                }
            };
            if let Err(failure) = entry.write(&buffer[..read]).and_then(|()| self.wrote(read)) {
                entry.discard();
                return Err(failure);
            }
        }
        entry.close(modified)
    }

    /// Starts writing the entry `name`, `None` when it's left out.
    pub(super) fn open(&mut self, name: &str) -> Result<Option<EntryFile>, Failure> {
        let Some(path) = self.place_of(name) else {
            return Ok(None);
        };
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder).map_err(|error| Failure::write(folder, &error))?;
        }
        let file = File::create(&path).map_err(|error| Failure::write(&path, &error))?;
        Ok(Some(EntryFile {
            path,
            file: BufWriter::with_capacity(BUFFER_SIZE, file),
        }))
    }

    /// Counts `count` more bytes written, and stops if asked.
    pub(super) fn wrote(&mut self, count: usize) -> Result<(), Failure> {
        if self.cancel.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        self.written += count as u64;
        (self.progress)(self.written);
        Ok(())
    }

    /// Notes an entry left out, such as a link.
    pub(super) fn leave_out(&mut self, name: &str) {
        self.left_out.push(name.to_owned());
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

impl EntryFile {
    pub(super) fn write(&mut self, data: &[u8]) -> Result<(), Failure> {
        self.file
            .write_all(data)
            .map_err(|error| Failure::write(&self.path, &error))
    }

    /// Finishes the file, with the date it has in the archive when it has one.
    pub(super) fn close(self, modified: Option<SystemTime>) -> Result<(), Failure> {
        let file = self
            .file
            .into_inner()
            .map_err(|error| Failure::write(&self.path, error.error()))?;
        if let Some(modified) = modified {
            // A date that can't be set leaves the file dated now, which is no reason to fail.
            let _ = file.set_modified(modified);
        }
        Ok(())
    }

    /// Deletes what was written of it.
    pub(super) fn discard(self) {
        let path = self.path.clone();
        drop(self.file);
        let _ = fs::remove_file(path);
    }
}

/// An entry's path inside the archive as a path inside the folder: each name made one Windows takes, "." and empty
/// names dropped. `None` for a path that would leave the folder ("..") or has no name.
pub(super) fn safe_relative(name: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for part in name.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return None,
            part => path.push(safe_part(part)),
        }
    }
    (!path.as_os_str().is_empty()).then_some(path)
}

/// One name of a path as Windows takes it: no character it refuses, no dot or space at the end, no device's name. A dot
/// at the start stays (".gitignore", macOS's "._" files).
fn safe_part(part: &str) -> String {
    let name = match without_refused_characters(part).trim_end_matches(['.', ' ']) {
        "" => "_".to_owned(),
        kept => kept.to_owned(),
    };
    let device = name.split('.').next().unwrap_or_default();
    if DEVICE_NAMES
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(device.trim_end()))
    {
        format!("_{name}")
    } else {
        name
    }
}

/// Puts what an archive was unpacked to (`unpacked`, a folder of its own) beside it in `folder`: the one file or
/// folder it held as it is, several in a folder named `stem`. A name that's taken gets a number ("Album (2)"). Answers
/// where it went and every file now there, in a stable order.
pub(super) fn put_beside(
    unpacked: &Path,
    folder: &Path,
    stem: &str,
) -> Result<(PathBuf, Vec<PathBuf>), Failure> {
    let mut top: Vec<PathBuf> = fs::read_dir(unpacked)
        .map_err(|error| Failure::write(unpacked, &error))?
        .flatten()
        .map(|entry| entry.path())
        .collect();
    let placed = match top.as_mut_slice() {
        [] => {
            let _ = fs::remove_dir(unpacked);
            return Ok((folder.to_path_buf(), Vec::new()));
        }
        [only] => {
            let name = only.file_name().map(Path::new).unwrap_or(Path::new(stem));
            let target = if only.is_dir() {
                free_folder_name(&folder.join(name))
            } else {
                crate::downloads::free_name(&folder.join(name))
            };
            fs::rename(&*only, &target).map_err(|error| Failure::write(&target, &error))?;
            let _ = fs::remove_dir(unpacked);
            target
        }
        _ => {
            let target = free_folder_name(&folder.join(safe_part(stem)));
            fs::rename(unpacked, &target).map_err(|error| Failure::write(&target, &error))?;
            target
        }
    };
    Ok((placed.clone(), files_under(&placed)))
}

/// `path`, or the first of "Name (2)", "Name (3)"… that isn't taken: a folder's name has no extension to keep.
fn free_folder_name(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_owned();
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    (2..)
        .map(|number| path.with_file_name(format!("{name} ({number})")))
        .find(|candidate| !candidate.exists())
        .unwrap_or_else(|| path.to_owned())
}

/// Every file under `path` (or `path` itself), folders first by name, then their files by name.
pub(super) fn files_under(path: &Path) -> Vec<PathBuf> {
    if !path.is_dir() {
        return vec![path.to_owned()];
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(path)
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default();
    entries.sort();
    let (folders, files): (Vec<PathBuf>, Vec<PathBuf>) =
        entries.into_iter().partition(|entry| entry.is_dir());
    let mut all: Vec<PathBuf> = folders
        .iter()
        .flat_map(|folder| files_under(folder))
        .collect();
    all.extend(files);
    all
}

/// Moves `paths` to the Recycle Bin, where they can be put back. Windows deletes for good what has no Recycle Bin (a
/// network folder).
#[cfg(windows)]
pub(super) fn recycle(paths: &[&Path]) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::{
        FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, SHFILEOPSTRUCTW,
        SHFileOperationW,
    };

    if paths.is_empty() {
        return Ok(());
    }
    // Each path ends with a NUL, and the list with another.
    let mut from: Vec<u16> = Vec::new();
    for path in paths {
        from.extend(path.as_os_str().encode_wide());
        from.push(0);
    }
    from.push(0);
    let mut operation = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: from.as_ptr(),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT) as u16,
        ..Default::default()
    };
    // SAFETY: `operation` is fully set up, and `from`, the double-NUL-terminated list it points to, outlives the call.
    let result = unsafe { SHFileOperationW(&mut operation) };
    if result == 0 && operation.fAnyOperationsAborted == 0 {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "the Recycle Bin refused them ({result:#x})"
        )))
    }
}

/// Elsewhere (the tests), deleting stands in for the Recycle Bin.
#[cfg(not(windows))]
pub(super) fn recycle(paths: &[&Path]) -> io::Result<()> {
    paths.iter().try_for_each(fs::remove_file)
}

/// A date written as Windows' FILETIME (100-nanosecond steps since 1601, in UTC), as rar and 7z keep them.
pub(super) fn from_file_time(ticks: u64) -> Option<SystemTime> {
    /// Seconds from 1601 to 1970.
    const TO_1970: u64 = 11_644_473_600;
    let seconds = ticks / 10_000_000;
    let nanoseconds = (ticks % 10_000_000) * 100;
    let since_1970 = seconds.checked_sub(TO_1970)?;
    SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::new(
        since_1970,
        u32::try_from(nanoseconds).ok()?,
    ))
}

/// A zip's date, written as the time of day where it was made (MS-DOS's), as this PC's local time, as Windows'
/// Explorer reads it.
#[cfg(windows)]
pub(super) fn from_local_time(
    year: u16,
    month: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
) -> Option<SystemTime> {
    use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows_sys::Win32::System::Time::{SystemTimeToFileTime, TzSpecificLocalTimeToSystemTime};

    let local = SYSTEMTIME {
        wYear: year,
        wMonth: month,
        wDayOfWeek: 0,
        wDay: day,
        wHour: hour,
        wMinute: minute,
        wSecond: second,
        wMilliseconds: 0,
    };
    let mut universal = SYSTEMTIME::default();
    let mut file_time = FILETIME::default();
    // SAFETY: each pointer is to a local that outlives the call; a null time zone means the PC's own.
    let converted = unsafe {
        TzSpecificLocalTimeToSystemTime(std::ptr::null(), &local, &mut universal) != 0
            && SystemTimeToFileTime(&universal, &mut file_time) != 0
    };
    converted
        .then(|| u64::from(file_time.dwHighDateTime) << 32 | u64::from(file_time.dwLowDateTime))
        .and_then(from_file_time)
}

/// Elsewhere (the tests), the zip's time is taken as UTC.
#[cfg(not(windows))]
pub(super) fn from_local_time(
    year: u16,
    month: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
) -> Option<SystemTime> {
    // Howard Hinnant's days_from_civil.
    let (year, month) = (i64::from(year), i64::from(month));
    let shifted_year = if month <= 2 { year - 1 } else { year };
    let era = shifted_year.div_euclid(400);
    let year_of_era = shifted_year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    let seconds =
        days * 86_400 + i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second);
    SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(u64::try_from(seconds).ok()?))
}
