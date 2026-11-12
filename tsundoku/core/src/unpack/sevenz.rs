//! 7z archives, through `sevenz-rust2`: LZMA, LZMA2 (on several threads), PPMd, bzip2 and deflate, with AES's
//! password, names hidden too.

use std::path::Path;

use sevenz_rust2::{ArchiveReader, Error, Password};

use super::files::{Output, from_file_time};
use super::joined::Joined;
use super::{Failure, Listing};

fn open(parts: &[&Path], password: Option<&str>) -> Result<ArchiveReader<Joined>, Failure> {
    let joined = Joined::open(parts).map_err(|error| Failure::read(&error))?;
    let password = password.map_or_else(Password::empty, Password::from);
    ArchiveReader::new(joined, password).map_err(failure)
}

/// What it holds. Its names may be encrypted too, so this may need the password.
pub(super) fn list(parts: &[&Path], password: Option<&str>) -> Result<Listing, Failure> {
    let reader = open(parts, password)?;
    let mut listing = Listing::default();
    for entry in &reader.archive().files {
        if !entry.is_directory && !entry.is_anti_item {
            listing.bytes = listing.bytes.saturating_add(entry.size);
        }
    }
    Ok(listing)
}

pub(super) fn unpack(
    parts: &[&Path],
    password: Option<&str>,
    output: &mut Output,
) -> Result<(), Failure> {
    let mut reader = open(parts, password)?;
    // What stopped the walk from inside it: a write that failed, or a stop.
    let mut stopped: Option<Failure> = None;
    let walked = reader.for_each_entries(|entry, data| {
        if entry.is_anti_item {
            return Ok(true);
        }
        let written = if entry.is_directory {
            output.directory(&entry.name)
        } else {
            let modified = entry
                .has_last_modified_date
                .then(|| from_file_time(u64::from(entry.last_modified_date)))
                .flatten();
            output.file(&entry.name, data, modified)
        };
        match written {
            Ok(()) => Ok(true),
            Err(failure) => {
                stopped = Some(failure);
                Err(Error::Other("stopped".into()))
            }
        }
    });
    match (walked, stopped) {
        (_, Some(failure)) => Err(failure),
        (Err(error), None) => Err(failure(error)),
        (Ok(()), None) => Ok(()),
    }
}

fn failure(error: Error) -> Failure {
    match error {
        Error::PasswordRequired => Failure::NeedsPassword,
        Error::MaybeBadPassword(_) => Failure::WrongPassword,
        Error::UnsupportedCompressionMethod(method) => {
            Failure::Unsupported(format!("compression method {method}"))
        }
        Error::Unsupported(detail) => Failure::Unsupported(detail.into_owned()),
        Error::ExternalUnsupported => Failure::Unsupported("headers in another stream".to_owned()),
        Error::BadSignature(_) => Failure::Damaged("not a 7z archive".to_owned()),
        Error::FileOpen(error, _) => Failure::read(&error),
        other => Failure::Damaged(other.to_string()),
    }
}
