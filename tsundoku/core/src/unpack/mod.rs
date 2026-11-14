//! Unpacking the archives a download brings: zip, 7z and rar, in one file or split into parts (which may come from
//! several downloads), with a password from the list in Settings → Archives when one is needed. What an archive holds
//! goes beside it as Settings → Archives or a rule says (`UnpackPlace`): its one file or folder as it is and several in
//! a folder named after it, always in such a folder, or always as it is. See `DOCUMENTATION/Docs/Download Pipeline.md`.

mod files;
mod joined;
mod rar;
mod sets;
mod sevenz;
mod zips;

#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use self::sets::{ArchiveSet, Completeness, Format, sets_among};
use crate::cancel::CancelToken;
use crate::room::{self, DiskSpace};
use crate::settings::{AfterUnpacking, UnpackPlace};
use crate::sync::lock;

/// Why an archive couldn't be unpacked, as Failed shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum UnpackProblem {
    /// It has a password, and the list in Settings → Archives has none.
    #[error("it has a password")]
    NeedsPassword,
    /// None of the list's `tried` passwords opens it.
    #[error("none of the {tried} passwords opens it")]
    WrongPassword { tried: u32 },
    /// A part it continues in isn't in its folder (the parts' names didn't tell, as a rar's older ones may not).
    #[error("its part {part} isn't there")]
    MissingPart { part: String },
    #[error("it's damaged: {detail}")]
    Damaged { detail: String },
    /// A kind of archive, or of compression, Tsundoku doesn't read.
    #[error("Tsundoku can't unpack it: {detail}")]
    Unsupported { detail: String },
    /// What it holds doesn't fit on its drive.
    #[error("{drive} has {free} bytes free, and it needs {needed}")]
    NoRoom {
        drive: String,
        #[ts(type = "number")]
        free: u64,
        #[ts(type = "number")]
        needed: u64,
    },
    /// A file couldn't be written, or moved beside the archive.
    #[error("couldn't write {path}: {detail}")]
    Write { path: String, detail: String },
}

/// Why unpacking an archive ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnpackError {
    /// It was asked to stop: what it had unpacked is gone, the archive is as it was.
    Cancelled,
    Problem(UnpackProblem),
}

/// Why a reader couldn't read an archive, before the passwords are all tried.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Failure {
    NeedsPassword,
    WrongPassword,
    /// The part of this name isn't there.
    Missing(String),
    Damaged(String),
    Unsupported(String),
    NoRoom(room::Shortfall),
    Write {
        path: String,
        detail: String,
    },
    Cancelled,
}

impl Failure {
    /// The archive couldn't be read: cut short, or the drive failed.
    fn read(error: &io::Error) -> Self {
        Self::Damaged(error.to_string())
    }

    fn write(path: &Path, error: &io::Error) -> Self {
        Self::Write {
            path: path.display().to_string(),
            detail: error.to_string(),
        }
    }
}

/// What an archive holds, as its directory tells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Listing {
    /// The size of its files once unpacked.
    bytes: u64,
}

/// What unpacking an archive takes.
pub struct Request<'a> {
    pub set: &'a ArchiveSet,
    /// Tried in order when it has a password.
    pub passwords: &'a [String],
    /// Where what it holds goes beside it.
    pub place: UnpackPlace,
    /// Where it's unpacked before being put beside the archive: a folder made and removed here, on the archive's
    /// drive, so putting it beside the archive is a move.
    pub work_folder: &'a Path,
    pub disks: &'a dyn DiskSpace,
    pub cancel: &'a CancelToken,
}

/// An archive unpacked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unpacked {
    /// What it was unpacked to: its one file or folder, the folder made for what it holds, or the archive's folder
    /// when it held several things put there as they are.
    pub placed: PathBuf,
    /// Every file unpacked, in a stable order.
    pub files: Vec<PathBuf>,
    /// The size of what it holds.
    pub bytes: u64,
    /// Entries left out: links, and names that would leave its folder.
    pub left_out: Vec<String>,
    /// Which of the list's passwords opened it, from 1; `None` when it has none.
    pub password: Option<usize>,
}

/// The archives being unpacked, by their first part: a split archive whose last parts come with several downloads
/// at once is unpacked by one of them.
static CLAIMED: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

/// An archive claimed for unpacking, released when dropped.
pub struct Claim(PathBuf);

/// Claims `set` for unpacking, or `None` when another download is unpacking it.
pub fn claim(set: &ArchiveSet) -> Option<Claim> {
    let first = set.first()?.to_path_buf();
    let newly = lock(&CLAIMED)
        .get_or_insert_with(HashSet::new)
        .insert(first.clone());
    // Made only once claimed, and outside the lock: dropping one takes it.
    newly.then(|| Claim(first))
}

impl Drop for Claim {
    fn drop(&mut self) {
        if let Some(claimed) = lock(&CLAIMED).as_mut() {
            claimed.remove(&self.0);
        }
    }
}

/// Unpacks an archive whose parts are all here (`ArchiveSet::completeness`) beside it, trying none then each password,
/// telling `progress` the bytes written and the whole size. The archive is left as it is.
pub fn unpack(
    request: &Request,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Unpacked, UnpackError> {
    let candidates = std::iter::once(None).chain(
        request
            .passwords
            .iter()
            .map(|password| Some(password.as_str())),
    );
    let mut tried = 0_u32;
    for (index, password) in candidates.enumerate() {
        match (password, attempt(request, password, progress)) {
            (_, Ok(mut unpacked)) => {
                unpacked.password = password.map(|_| index);
                return Ok(unpacked);
            }
            (_, Err(Failure::Cancelled)) => return Err(UnpackError::Cancelled),
            (None, Err(Failure::NeedsPassword)) => {}
            // A wrong password reads as damage where the archive can't check it (zip's older encryption, rar 4).
            (
                Some(_),
                Err(Failure::NeedsPassword | Failure::WrongPassword | Failure::Damaged(_)),
            ) => {
                tried += 1;
            }
            (_, Err(failure)) => return Err(UnpackError::Problem(problem(failure))),
        }
    }
    Err(UnpackError::Problem(if tried == 0 {
        UnpackProblem::NeedsPassword
    } else {
        UnpackProblem::WrongPassword { tried }
    }))
}

/// One try at unpacking, with `password` or none: listed, checked for room, unpacked into the work folder and put
/// beside the archive.
fn attempt(
    request: &Request,
    password: Option<&str>,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Unpacked, Failure> {
    let set = request.set;
    if request.cancel.is_cancelled() {
        return Err(Failure::Cancelled);
    }
    let first = set
        .first()
        .ok_or_else(|| Failure::Missing(set.part_name(1)))?;
    let leading = set.leading_parts();
    let listing = match set.format {
        Format::Zip => zips::list(&leading)?,
        Format::SevenZip => sevenz::list(&leading, password)?,
        Format::Rar => rar::list(first, password)?,
    };
    if let Some(space) = request.disks.free(set.folder())
        && let Some(shortfall) = room::shortfall(&space, listing.bytes, 0)
    {
        return Err(Failure::NoRoom(shortfall));
    }

    let work = request.work_folder;
    let _ = fs::remove_dir_all(work);
    fs::create_dir_all(work).map_err(|error| Failure::write(work, &error))?;
    let total = listing.bytes;
    let mut report = |written: u64| progress(written.min(total), total);
    let mut output = files::Output::new(work, request.cancel, &mut report);
    let unpacked = match set.format {
        Format::Zip => zips::unpack(&leading, password, &mut output),
        Format::SevenZip => sevenz::unpack(&leading, password, &mut output),
        Format::Rar => rar::unpack(first, password, &mut output),
    };
    let left_out = std::mem::take(&mut output.left_out);
    let placed =
        unpacked.and_then(|()| files::put_beside(work, set.folder(), &set.stem, request.place));
    let _ = fs::remove_dir_all(work);
    let (placed, files) = placed?;
    Ok(Unpacked {
        placed,
        files,
        bytes: listing.bytes,
        left_out,
        password: None,
    })
}

fn problem(failure: Failure) -> UnpackProblem {
    match failure {
        Failure::NeedsPassword => UnpackProblem::NeedsPassword,
        Failure::WrongPassword => UnpackProblem::WrongPassword { tried: 1 },
        Failure::Missing(part) => UnpackProblem::MissingPart { part },
        Failure::Damaged(detail) => UnpackProblem::Damaged { detail },
        Failure::Unsupported(detail) => UnpackProblem::Unsupported { detail },
        Failure::NoRoom(shortfall) => UnpackProblem::NoRoom {
            drive: shortfall.drive,
            free: shortfall.free,
            needed: shortfall.needed,
        },
        Failure::Write { path, detail } => UnpackProblem::Write { path, detail },
        // Handled before: a stop isn't a problem.
        Failure::Cancelled => UnpackProblem::Damaged {
            detail: "stopped".to_owned(),
        },
    }
}

/// Puts an unpacked archive's parts away as Settings → Archives says: kept, in the Recycle Bin, or deleted. Answers why
/// they weren't, when they weren't.
pub fn put_away(set: &ArchiveSet, after: AfterUnpacking) -> Result<(), String> {
    let parts: Vec<&Path> = set.parts().collect();
    match after {
        AfterUnpacking::Keep => Ok(()),
        AfterUnpacking::RecycleBin => files::recycle(&parts).map_err(|error| error.to_string()),
        AfterUnpacking::Delete => parts
            .iter()
            .try_for_each(fs::remove_file)
            .map_err(|error| error.to_string()),
    }
}
