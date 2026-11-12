//! Which downloaded files are archives, and which files make up one archive: a split archive's parts sit in the same
//! folder as its first, named after it ("Album.part1.rar", "Album.part2.rar"; "Album.rar", "Album.r00"; "Album.7z.001",
//! "Album.7z.002"), and may come from several downloads.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

/// The kinds of archive Tsundoku unpacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    Zip,
    SevenZip,
    Rar,
}

/// How an archive's parts are named.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Naming {
    /// One file: "Album.zip", "Album.7z".
    Single,
    /// "Album.part1.rar", or "Album.part01.rar" with `width` digits.
    RarParts { width: usize },
    /// "Album.rar", which may go on in "Album.r00" to "Album.r99", then "Album.s00": rar's older way. Unrar finds the
    /// next part itself, so a rar that isn't split is one of these with no other part.
    RarOld,
    /// "Album.7z.001", "Album.zip.001": one archive cut into pieces of `width` digits, joined back as they are.
    Pieces { width: usize },
    /// "Album.z01", "Album.z02"… then "Album.zip": a zip spanned over several files, which Tsundoku doesn't unpack.
    ZipSpanned,
}

/// One archive and the parts of it found so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveSet {
    pub format: Format,
    folder: PathBuf,
    /// Its name without part number or extension, as written: "Album" for "Album.part01.rar". A folder holding what
    /// it holds is named after it.
    pub stem: String,
    naming: Naming,
    /// Its parts in the folder, by number: 1 for the first.
    parts: Vec<(u32, PathBuf)>,
}

/// Whether every part of an archive is here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Completeness {
    /// Its parts as far as its names tell: a rar's own headers then say whether one is missing.
    Complete,
    /// The part of this name isn't here.
    Missing(String),
    /// A kind of split Tsundoku doesn't read.
    Unsupported(String),
}

/// What a file's name says about it as part of an archive.
struct Part {
    format: Format,
    stem: String,
    naming: Naming,
    number: u32,
}

static RAR_PART: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(.+)\.part(\d{1,4})\.rar$").expect("valid pattern"));
static RAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(.+)\.rar$").expect("valid pattern"));
static RAR_OLD_PART: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(.+)\.([rs])(\d{2})$").expect("valid pattern"));
static PIECE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(.+)\.(7z|zip)\.(\d{3,4})$").expect("valid pattern"));
static SEVEN_ZIP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(.+)\.7z$").expect("valid pattern"));
static ZIP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(.+)\.zip$").expect("valid pattern"));
static ZIP_SPAN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(.+)\.z(\d{2})$").expect("valid pattern"));

/// What a file name says, if it's an archive's: zip and 7z by their extension, rar's parts both ways.
fn part_of(name: &str) -> Option<Part> {
    let part = |format, stem: &str, naming, number| {
        Some(Part {
            format,
            stem: stem.to_owned(),
            naming,
            number,
        })
    };
    if let Some(found) = RAR_PART.captures(name) {
        let digits = &found[2];
        let number = digits.parse().ok()?;
        return part(
            Format::Rar,
            &found[1],
            Naming::RarParts {
                width: digits.len(),
            },
            number,
        );
    }
    if let Some(found) = RAR.captures(name) {
        return part(Format::Rar, &found[1], Naming::RarOld, 1);
    }
    if let Some(found) = RAR_OLD_PART.captures(name) {
        let number: u32 = found[3].parse().ok()?;
        // .r00 is the second part, .s00 the 102nd.
        let first = if found[2].eq_ignore_ascii_case("r") {
            2
        } else {
            102
        };
        return part(Format::Rar, &found[1], Naming::RarOld, first + number);
    }
    if let Some(found) = PIECE.captures(name) {
        let format = if found[2].eq_ignore_ascii_case("7z") {
            Format::SevenZip
        } else {
            Format::Zip
        };
        let digits = &found[3];
        let number = digits.parse().ok()?;
        // The pieces' stem keeps no extension: "Album" for "Album.7z.001".
        return part(
            format,
            &found[1],
            Naming::Pieces {
                width: digits.len(),
            },
            number,
        );
    }
    if let Some(found) = SEVEN_ZIP.captures(name) {
        return part(Format::SevenZip, &found[1], Naming::Single, 1);
    }
    if let Some(found) = ZIP.captures(name) {
        return part(Format::Zip, &found[1], Naming::Single, 1);
    }
    if let Some(found) = ZIP_SPAN.captures(name) {
        let number = found[2].parse().ok()?;
        return part(Format::Zip, &found[1], Naming::ZipSpanned, number);
    }
    None
}

/// The archives `files` belong to, each once, in the order their first file comes: a part of one brings in the parts
/// beside it, whichever download brought them. A zip whose ".z01" part is beside it is a spanned zip.
pub fn sets_among(files: &[PathBuf]) -> Vec<ArchiveSet> {
    let mut seen = HashSet::new();
    let mut sets = Vec::new();
    for file in files {
        let (Some(folder), Some(name)) = (file.parent(), file.file_name()) else {
            continue;
        };
        let Some(mut part) = part_of(&name.to_string_lossy()) else {
            continue;
        };
        let siblings = names_in(folder);
        if part.format == Format::Zip
            && part.naming == Naming::Single
            && siblings.iter().any(|sibling| {
                part_of(sibling).is_some_and(|other| {
                    other.naming == Naming::ZipSpanned && same_stem(&other.stem, &part.stem)
                })
            })
        {
            part.naming = Naming::ZipSpanned;
        }
        let key = (
            folder.to_path_buf(),
            part.format,
            part.stem.to_lowercase(),
            part.naming,
        );
        if !seen.insert(key) {
            continue;
        }
        let mut parts: Vec<(u32, PathBuf)> = siblings
            .iter()
            .filter_map(|sibling| {
                let other = part_of(sibling)?;
                let spanned_end = part.naming == Naming::ZipSpanned
                    && other.format == Format::Zip
                    && other.naming == Naming::Single;
                // A spanned zip's ".zip" is its last part.
                let number = if spanned_end { u32::MAX } else { other.number };
                (other.format == part.format
                    && (other.naming == part.naming || spanned_end)
                    && same_stem(&other.stem, &part.stem))
                .then(|| (number, folder.join(sibling)))
            })
            .collect();
        parts.sort_by_key(|(number, _)| *number);
        sets.push(ArchiveSet {
            format: part.format,
            folder: folder.to_path_buf(),
            stem: part.stem,
            naming: part.naming,
            parts,
        });
    }
    sets
}

/// Windows doesn't tell names apart by case.
fn same_stem(one: &str, other: &str) -> bool {
    one.to_lowercase() == other.to_lowercase()
}

/// The names of the files in `folder`.
fn names_in(folder: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

impl ArchiveSet {
    /// The folder it's in, where what it holds goes.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// Its first part, which opens it, if it's here.
    pub fn first(&self) -> Option<&Path> {
        self.parts
            .first()
            .filter(|(number, _)| *number == 1)
            .map(|(_, path)| path.as_path())
    }

    /// Its first part's file name, or what it would be: how the archive is named to the user.
    pub fn name(&self) -> String {
        self.first().and_then(Path::file_name).map_or_else(
            || self.part_name(1),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    /// Every part found, in order.
    pub fn parts(&self) -> impl Iterator<Item = &Path> {
        self.parts.iter().map(|(_, path)| path.as_path())
    }

    /// Whether `path` is one of its parts.
    pub fn holds(&self, path: &Path) -> bool {
        self.parts.iter().any(|(_, part)| part == path)
    }

    /// The parts from the first on with no gap: pieces join only while none is missing.
    pub(super) fn leading_parts(&self) -> Vec<&Path> {
        self.parts
            .iter()
            .enumerate()
            .take_while(|(index, (number, _))| *number as usize == index + 1)
            .map(|(_, (_, path))| path.as_path())
            .collect()
    }

    /// What its names tell of its parts: the first must be here, and pieces must add up to the archive's whole size
    /// (a 7z's start header gives it; a zip's must end in its directory).
    pub fn completeness(&self) -> Completeness {
        if self.naming == Naming::ZipSpanned {
            return Completeness::Unsupported(
                "a zip split into .z01, .z02… parts (spanned)".to_owned(),
            );
        }
        if self.first().is_none() {
            return Completeness::Missing(self.part_name(1));
        }
        let Naming::Pieces { .. } = self.naming else {
            return Completeness::Complete;
        };
        let leading = self.leading_parts();
        let next = || Completeness::Missing(self.part_name(leading.len() as u32 + 1));
        let size: u64 = leading
            .iter()
            .map(|part| fs::metadata(part).map_or(0, |metadata| metadata.len()))
            .sum();
        match self.format {
            Format::SevenZip => match seven_zip_size(leading[0]) {
                Some(whole) if size >= whole => Completeness::Complete,
                _ => next(),
            },
            // A zip's directory is at its end: until the last piece is here, it can't be found.
            Format::Zip => {
                if super::zips::opens(&leading) {
                    Completeness::Complete
                } else {
                    next()
                }
            }
            Format::Rar => Completeness::Complete,
        }
    }

    /// The name its part `number` has, or would have.
    pub(super) fn part_name(&self, number: u32) -> String {
        let stem = &self.stem;
        match self.naming {
            Naming::RarParts { width } => format!("{stem}.part{number:0width$}.rar"),
            Naming::RarOld => match number {
                0 | 1 => format!("{stem}.rar"),
                2..=101 => format!("{stem}.r{:02}", number - 2),
                _ => format!("{stem}.s{:02}", number - 102),
            },
            Naming::Pieces { width } => {
                let extension = match self.format {
                    Format::SevenZip => "7z",
                    _ => "zip",
                };
                format!("{stem}.{extension}.{number:0width$}")
            }
            Naming::Single | Naming::ZipSpanned => {
                let extension = match self.format {
                    Format::SevenZip => "7z",
                    Format::Rar => "rar",
                    Format::Zip => "zip",
                };
                format!("{stem}.{extension}")
            }
        }
    }
}

/// A 7z archive's whole size, from its start header: the signature, then the next header's offset and size after the
/// first 32 bytes.
fn seven_zip_size(first: &Path) -> Option<u64> {
    let mut header = [0_u8; 32];
    File::open(first).ok()?.read_exact(&mut header).ok()?;
    if header[..6] != [b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C] {
        return None;
    }
    let read =
        |range: std::ops::Range<usize>| header[range].try_into().ok().map(u64::from_le_bytes);
    let offset = read(12..20)?;
    let size = read(20..28)?;
    32_u64.checked_add(offset)?.checked_add(size)
}
