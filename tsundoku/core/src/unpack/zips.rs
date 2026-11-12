//! Zip archives, through the `zip` crate: deflate (and deflate64, bzip2), with ZipCrypto's or AES's password.

use std::path::Path;

use zip::ZipArchive;
use zip::result::ZipError;

use super::files::{Output, from_local_time};
use super::joined::Joined;
use super::{Failure, Listing};

/// Whether pieces of a zip ("Album.zip.001"…) open as one: its directory, at its end, is there.
pub(super) fn opens(parts: &[&Path]) -> bool {
    Joined::open(parts)
        .ok()
        .is_some_and(|joined| ZipArchive::new(joined).is_ok())
}

fn open(parts: &[&Path]) -> Result<ZipArchive<Joined>, Failure> {
    let joined = Joined::open(parts).map_err(|error| Failure::read(&error))?;
    ZipArchive::new(joined).map_err(failure)
}

/// What it holds. Its directory isn't encrypted, so this never needs the password.
pub(super) fn list(parts: &[&Path]) -> Result<Listing, Failure> {
    let mut archive = open(parts)?;
    let mut listing = Listing::default();
    for index in 0..archive.len() {
        let entry = archive.by_index_raw(index).map_err(failure)?;
        if entry.is_file() {
            listing.bytes = listing.bytes.saturating_add(entry.size());
        }
    }
    Ok(listing)
}

pub(super) fn unpack(
    parts: &[&Path],
    password: Option<&str>,
    output: &mut Output,
) -> Result<(), Failure> {
    let mut archive = open(parts)?;
    for index in 0..archive.len() {
        if output.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        let entry = match password {
            Some(password) => archive.by_index_decrypt(index, password.as_bytes()),
            None => archive.by_index(index),
        };
        let mut entry = entry.map_err(failure)?;
        let name = entry
            .name()
            .map(|name| name.into_owned())
            .map_err(failure)?;
        if entry.is_symlink() {
            output.leave_out(&name);
        } else if entry.is_dir() {
            output.directory(&name)?;
        } else {
            let modified = entry.last_modified().and_then(|date| {
                from_local_time(
                    date.year(),
                    u16::from(date.month()),
                    u16::from(date.day()),
                    u16::from(date.hour()),
                    u16::from(date.minute()),
                    u16::from(date.second()),
                )
            });
            output.file(&name, &mut entry, modified)?;
        }
    }
    Ok(())
}

fn failure(error: ZipError) -> Failure {
    match error {
        ZipError::InvalidPassword => Failure::WrongPassword,
        ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED) => Failure::NeedsPassword,
        ZipError::UnsupportedArchive(detail) => Failure::Unsupported(detail.to_owned()),
        ZipError::CompressionMethodNotSupported(method) => {
            Failure::Unsupported(format!("compression method {method}"))
        }
        ZipError::Io(error) => Failure::read(&error),
        other => Failure::Damaged(other.to_string()),
    }
}
