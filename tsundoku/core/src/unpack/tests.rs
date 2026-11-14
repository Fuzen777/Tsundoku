//! Zip and 7z archives are made here; rar archives can't be, so real ones are in `tests/fixtures/archives` (see its
//! README).

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use sevenz_rust2::encoder_options::AesEncoderOptions;
use sevenz_rust2::{ArchiveEntry, ArchiveWriter, EncoderMethod, Password};
use tempfile::TempDir;
use zip::write::SimpleFileOptions;
use zip::{AesMode, CompressionMethod, ZipWriter};

use super::*;
use crate::room::DriveSpace;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/archives/");

/// A drive with `free` bytes, or one Windows won't tell about.
struct Disk(Option<u64>);

impl DiskSpace for Disk {
    fn free(&self, _folder: &Path) -> Option<DriveSpace> {
        self.0.map(|free| DriveSpace {
            drive: "D:".to_owned(),
            free,
        })
    }
}

const ROOMY: Disk = Disk(None);

/// `length` bytes that don't compress, so archives of them are as big as they are.
fn noise(length: usize) -> Vec<u8> {
    let mut state = 0x2545_F491_u32;
    (0..length)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 24) as u8
        })
        .collect()
}

fn touch(folder: &Path, names: &[&str]) -> Vec<PathBuf> {
    names
        .iter()
        .map(|name| {
            let path = folder.join(name);
            File::create(&path).unwrap();
            path
        })
        .collect()
}

fn fixture(name: &str, folder: &Path, as_name: &str) -> PathBuf {
    let path = folder.join(as_name);
    fs::copy(format!("{FIXTURES}{name}"), &path).unwrap();
    path
}

/// A zip of `entries` (a name ending in "/" is a folder), each with AES's password when one is given.
fn make_zip(path: &Path, entries: &[(&str, &[u8])], password: Option<&str>) {
    let mut writer = ZipWriter::new(File::create(path).unwrap());
    for (name, data) in entries {
        if name.ends_with('/') {
            writer
                .add_directory(*name, SimpleFileOptions::default())
                .unwrap();
            continue;
        }
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let options = match password {
            Some(password) => options.with_aes_encryption(AesMode::Aes256, password),
            None => options,
        };
        writer.start_file(*name, options).unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap();
}

/// A 7z of `entries`, with AES's password when one is given, its names hidden too when `hide_names`.
fn make_7z(path: &Path, entries: &[(&str, &[u8])], password: Option<&str>, hide_names: bool) {
    let mut writer = ArchiveWriter::create(path).unwrap();
    if let Some(password) = password {
        writer.set_content_methods(vec![
            AesEncoderOptions::new(Password::from(password)).into(),
            EncoderMethod::LZMA2.into(),
        ]);
        writer.set_encrypt_header(hide_names);
    }
    for (name, data) in entries {
        if let Some(folder) = name.strip_suffix('/') {
            writer
                .push_archive_entry::<&[u8]>(ArchiveEntry::new_directory(folder), None)
                .unwrap();
        } else {
            writer
                .push_archive_entry(ArchiveEntry::new_file(name), Some(*data))
                .unwrap();
        }
    }
    writer.finish().unwrap();
}

/// Cuts `archive` into pieces of `size` bytes named "<archive>.001", "<archive>.002"…, and deletes it.
fn cut(archive: &Path, size: usize) -> Vec<PathBuf> {
    let bytes = fs::read(archive).unwrap();
    fs::remove_file(archive).unwrap();
    bytes
        .chunks(size)
        .enumerate()
        .map(|(index, piece)| {
            let path = PathBuf::from(format!("{}.{:03}", archive.display(), index + 1));
            fs::write(&path, piece).unwrap();
            path
        })
        .collect()
}

fn the_set(files: &[PathBuf]) -> ArchiveSet {
    let mut sets = sets_among(files);
    assert_eq!(sets.len(), 1, "{sets:?}");
    sets.remove(0)
}

fn run(set: &ArchiveSet, passwords: &[&str], folder: &Path) -> Result<Unpacked, UnpackError> {
    run_with(set, passwords, folder, &ROOMY, &CancelToken::new())
}

fn run_with(
    set: &ArchiveSet,
    passwords: &[&str],
    folder: &Path,
    disks: &dyn DiskSpace,
    cancel: &CancelToken,
) -> Result<Unpacked, UnpackError> {
    let passwords: Vec<String> = passwords
        .iter()
        .map(|&password| password.to_owned())
        .collect();
    let work_folder = folder.join(".tsundoku-1").join("unpacking");
    unpack(
        &Request {
            set,
            passwords: &passwords,
            place: UnpackPlace::Smart,
            work_folder: &work_folder,
            disks,
            cancel,
        },
        &mut |_, _| {},
    )
}

/// The files under `folder`, relative to it with "/" between names, leaving out Tsundoku's own folders.
fn tree(folder: &Path) -> Vec<String> {
    let mut found: Vec<String> = files::files_under(folder)
        .iter()
        .map(|path| {
            path.strip_prefix(folder)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .filter(|path| !path.starts_with(".tsundoku-"))
        .collect();
    found.sort();
    found
}

fn problem_of(result: Result<Unpacked, UnpackError>) -> UnpackProblem {
    match result {
        Err(UnpackError::Problem(problem)) => problem,
        other => panic!("expected a problem, got {other:?}"),
    }
}

#[test]
fn split_archives_are_found_with_their_parts() {
    let folder = TempDir::new().unwrap();
    let files = touch(
        folder.path(),
        &[
            "Album.part1.rar",
            "Album.part2.rar",
            "notes.txt",
            "Old.rar",
            "Old.r00",
            "Old.r01",
            "Pics.7z.001",
            "Pics.7z.002",
            "Single.zip",
            "Spanned.z01",
            "Spanned.zip",
        ],
    );
    // A part brought by another download is found beside it.
    touch(folder.path(), &["Album.part3.rar"]);
    let sets = sets_among(&files);
    let names: Vec<(String, Format, usize)> = sets
        .iter()
        .map(|set| (set.name(), set.format, set.parts().count()))
        .collect();
    assert_eq!(
        names,
        [
            ("Album.part1.rar".to_owned(), Format::Rar, 3),
            ("Old.rar".to_owned(), Format::Rar, 3),
            ("Pics.7z.001".to_owned(), Format::SevenZip, 2),
            ("Single.zip".to_owned(), Format::Zip, 1),
            ("Spanned.z01".to_owned(), Format::Zip, 2),
        ]
    );
    assert_eq!(sets[0].stem, "Album");
    assert_eq!(sets[2].stem, "Pics");
    assert_eq!(
        sets[4].completeness(),
        Completeness::Unsupported("a zip split into .z01, .z02… parts (spanned)".to_owned())
    );
}

#[test]
fn an_archive_without_its_first_part_waits_for_it() {
    let folder = TempDir::new().unwrap();
    let files = touch(folder.path(), &["Album.part02.rar", "Album.part03.rar"]);
    let set = the_set(&files);
    assert_eq!(
        set.name(),
        "Album.part01.rar",
        "named as its first part would be"
    );
    assert_eq!(
        set.completeness(),
        Completeness::Missing("Album.part01.rar".to_owned())
    );
}

#[test]
fn pieces_are_complete_once_they_add_up() {
    let folder = TempDir::new().unwrap();
    let data = noise(50_000);
    let seven_zip = folder.path().join("Pics.7z");
    make_7z(
        &seven_zip,
        &[("a.bin", &data), ("b.bin", &data)],
        None,
        false,
    );
    let mut pieces = cut(&seven_zip, 30_000);
    assert!(pieces.len() >= 2);
    let last = pieces.pop().unwrap();
    let saved = fs::read(&last).unwrap();
    fs::remove_file(&last).unwrap();
    let set = the_set(&pieces);
    assert_eq!(
        set.completeness(),
        Completeness::Missing(last.file_name().unwrap().to_string_lossy().into_owned())
    );
    fs::write(&last, saved).unwrap();
    assert_eq!(the_set(&pieces).completeness(), Completeness::Complete);

    let zip = folder.path().join("Docs.zip");
    make_zip(&zip, &[("a.bin", &data)], None);
    let mut pieces = cut(&zip, 30_000);
    let last = pieces.pop().unwrap();
    let saved = fs::read(&last).unwrap();
    fs::remove_file(&last).unwrap();
    assert!(matches!(
        the_set(&pieces).completeness(),
        Completeness::Missing(_)
    ));
    fs::write(&last, saved).unwrap();
    assert_eq!(the_set(&pieces).completeness(), Completeness::Complete);
}

#[test]
fn several_files_go_in_a_folder_named_after_the_archive() {
    let folder = TempDir::new().unwrap();
    let zip = folder.path().join("Album.zip");
    make_zip(
        &zip,
        &[("01.jpg", b"one"), ("sub/", b""), ("sub/02.jpg", b"two")],
        None,
    );
    let unpacked = run(&the_set(std::slice::from_ref(&zip)), &[], folder.path()).unwrap();
    assert_eq!(unpacked.placed, folder.path().join("Album"));
    assert_eq!(unpacked.password, None);
    assert_eq!(unpacked.bytes, 6);
    assert_eq!(
        tree(folder.path()),
        ["Album.zip", "Album/01.jpg", "Album/sub/02.jpg"]
    );
    assert_eq!(
        unpacked.files,
        [
            folder.path().join("Album").join("sub").join("02.jpg"),
            folder.path().join("Album").join("01.jpg"),
        ],
        "folders first"
    );
    assert_eq!(
        fs::read(folder.path().join("Album/sub/02.jpg")).unwrap(),
        b"two"
    );
    assert!(zip.exists(), "the archive is left as it is");
    assert!(
        !folder.path().join(".tsundoku-1").join("unpacking").exists(),
        "the work folder is gone"
    );

    // Unpacked again, the folder's name is taken.
    let again = run(&the_set(&[zip]), &[], folder.path()).unwrap();
    assert_eq!(again.placed, folder.path().join("Album (2)"));
}

/// Unpacks `set` beside it as `place` says, with no password.
fn run_placed(set: &ArchiveSet, folder: &Path, place: UnpackPlace) -> Unpacked {
    unpack(
        &Request {
            set,
            passwords: &[],
            place,
            work_folder: &folder.join(".tsundoku-1").join("unpacking"),
            disks: &ROOMY,
            cancel: &CancelToken::new(),
        },
        &mut |_, _| {},
    )
    .unwrap()
}

#[test]
fn a_folder_named_after_the_archive_holds_even_its_one_file() {
    let folder = TempDir::new().unwrap();
    let zip = folder.path().join("Movie.zip");
    make_zip(&zip, &[("Movie.mkv", b"video")], None);
    let unpacked = run_placed(
        &the_set(std::slice::from_ref(&zip)),
        folder.path(),
        UnpackPlace::ArchiveFolder,
    );
    assert_eq!(unpacked.placed, folder.path().join("Movie"));
    assert_eq!(
        unpacked.files,
        [folder.path().join("Movie").join("Movie.mkv")]
    );

    let again = run_placed(&the_set(&[zip]), folder.path(), UnpackPlace::ArchiveFolder);
    assert_eq!(
        again.placed,
        folder.path().join("Movie (2)"),
        "its name taken"
    );
}

#[test]
fn in_place_puts_everything_beside_the_archive_as_it_is() {
    let folder = TempDir::new().unwrap();
    let zip = folder.path().join("Album.zip");
    make_zip(
        &zip,
        &[
            ("Art/", b""),
            ("Art/cover.jpg", b"cover"),
            ("02.flac", b"two"),
            ("01.flac", b"one"),
        ],
        None,
    );
    fs::write(folder.path().join("01.flac"), b"taken").unwrap();
    let unpacked = run_placed(
        &the_set(std::slice::from_ref(&zip)),
        folder.path(),
        UnpackPlace::InPlace,
    );
    assert_eq!(
        unpacked.placed,
        folder.path(),
        "several things: the archive's folder"
    );
    assert_eq!(
        unpacked.files,
        [
            folder.path().join("Art").join("cover.jpg"),
            folder.path().join("01 (2).flac"),
            folder.path().join("02.flac"),
        ],
        "folders first, then files, a name taken numbered"
    );
    assert_eq!(
        fs::read(folder.path().join("01.flac")).unwrap(),
        b"taken",
        "never replaced"
    );
    assert!(
        !folder.path().join(".tsundoku-1").join("unpacking").exists(),
        "the work folder is gone"
    );

    let again = run_placed(&the_set(&[zip]), folder.path(), UnpackPlace::InPlace);
    assert_eq!(
        again.files[0],
        folder.path().join("Art (2)").join("cover.jpg")
    );

    let single = folder.path().join("Single.zip");
    make_zip(&single, &[("Inner/", b""), ("Inner/a.txt", b"a")], None);
    let unpacked = run_placed(&the_set(&[single]), folder.path(), UnpackPlace::InPlace);
    assert_eq!(
        unpacked.placed,
        folder.path().join("Inner"),
        "its one thing"
    );
}

#[test]
fn one_file_or_one_folder_is_put_beside_the_archive_as_it_is() {
    let folder = TempDir::new().unwrap();
    let zip = folder.path().join("Movie.zip");
    make_zip(&zip, &[("Movie.mkv", b"video")], None);
    fs::write(folder.path().join("Movie.mkv"), b"taken").unwrap();
    let unpacked = run(&the_set(&[zip]), &[], folder.path()).unwrap();
    assert_eq!(unpacked.placed, folder.path().join("Movie (2).mkv"));
    assert_eq!(unpacked.files, [folder.path().join("Movie (2).mkv")]);

    let zip = folder.path().join("Wrapped.zip");
    make_zip(&zip, &[("Inner/", b""), ("Inner/a.txt", b"a")], None);
    let unpacked = run(&the_set(&[zip]), &[], folder.path()).unwrap();
    assert_eq!(unpacked.placed, folder.path().join("Inner"));
    assert_eq!(unpacked.files, [folder.path().join("Inner").join("a.txt")]);
}

#[test]
fn names_that_leave_the_folder_or_windows_refuses_are_made_safe() {
    let folder = TempDir::new().unwrap();
    let zip = folder.path().join("Odd.zip");
    make_zip(
        &zip,
        &[
            ("../escaped.txt", b"no"),
            ("CON.txt", b"device"),
            ("what?.txt", b"question"),
            ("notes.txt. ", b"dot and space at the end"),
            (".gitignore", b"dot at the start"),
            ("ok.txt", b"fine"),
        ],
        None,
    );
    let unpacked = run(&the_set(&[zip]), &[], folder.path()).unwrap();
    assert_eq!(unpacked.left_out, ["../escaped.txt"]);
    assert_eq!(
        tree(folder.path()),
        [
            "Odd.zip",
            "Odd/.gitignore",
            "Odd/_CON.txt",
            "Odd/notes.txt",
            "Odd/ok.txt",
            "Odd/what_.txt"
        ]
    );
    assert!(!folder.path().parent().unwrap().join("escaped.txt").exists());
}

#[test]
fn passwords_are_tried_in_order() {
    let folder = TempDir::new().unwrap();
    let zip = folder.path().join("Locked.zip");
    make_zip(
        &zip,
        &[("a.txt", b"secret"), ("b.txt", b"more")],
        Some("hunter2"),
    );
    let set = the_set(&[zip]);
    assert_eq!(
        problem_of(run(&set, &[], folder.path())),
        UnpackProblem::NeedsPassword
    );
    assert_eq!(
        problem_of(run(&set, &["one", "two"], folder.path())),
        UnpackProblem::WrongPassword { tried: 2 }
    );
    assert_eq!(
        tree(folder.path()),
        ["Locked.zip"],
        "nothing is left behind"
    );
    let unpacked = run(&set, &["one", "hunter2"], folder.path()).unwrap();
    assert_eq!(unpacked.password, Some(2));
    assert_eq!(
        fs::read(folder.path().join("Locked/a.txt")).unwrap(),
        b"secret"
    );
}

#[test]
fn seven_zip_archives_unpack_with_or_without_hidden_names() {
    let folder = TempDir::new().unwrap();
    let plain = folder.path().join("Plain.7z");
    make_7z(
        &plain,
        &[("a.txt", b"alpha"), ("b.txt", b"beta")],
        None,
        false,
    );
    run(&the_set(&[plain]), &[], folder.path()).unwrap();
    assert_eq!(
        fs::read(folder.path().join("Plain/b.txt")).unwrap(),
        b"beta"
    );

    for (name, hide_names) in [("Data.7z", false), ("Hidden.7z", true)] {
        let archive = folder.path().join(name);
        make_7z(
            &archive,
            &[("x.txt", b"x"), ("y.txt", b"y")],
            Some("pass"),
            hide_names,
        );
        let set = the_set(&[archive]);
        assert_eq!(
            problem_of(run(&set, &[], folder.path())),
            UnpackProblem::NeedsPassword,
            "{name}"
        );
        assert_eq!(
            problem_of(run(&set, &["wrong"], folder.path())),
            UnpackProblem::WrongPassword { tried: 1 },
            "{name}"
        );
        let unpacked = run(&set, &["wrong", "pass"], folder.path()).unwrap();
        assert_eq!(unpacked.password, Some(2), "{name}");
        assert_eq!(unpacked.files.len(), 2, "{name}");
    }
}

#[test]
fn pieces_unpack_as_one_archive() {
    let folder = TempDir::new().unwrap();
    let data = noise(200_000);
    let archive = folder.path().join("Big.7z");
    make_7z(&archive, &[("data.bin", &data)], None, false);
    let pieces = cut(&archive, 60_000);
    let unpacked = run(&the_set(&pieces[..1]), &[], folder.path()).unwrap();
    assert_eq!(unpacked.placed, folder.path().join("data.bin"));
    assert_eq!(fs::read(folder.path().join("data.bin")).unwrap(), data);

    let archive = folder.path().join("Big.zip");
    make_zip(&archive, &[("zipped.bin", &data)], None);
    let pieces = cut(&archive, 70_000);
    run(&the_set(&pieces[1..2]), &[], folder.path()).unwrap();
    assert_eq!(fs::read(folder.path().join("zipped.bin")).unwrap(), data);
}

#[test]
fn rar_parts_unpack_together() {
    let folder = TempDir::new().unwrap();
    let parts: Vec<PathBuf> = (1..=3)
        .map(|number| {
            fixture(
                &format!("rar5-vols.part{number}.rar"),
                folder.path(),
                &format!("Album.part{number}.rar"),
            )
        })
        .collect();
    let set = the_set(&parts[1..2]);
    assert_eq!(set.completeness(), Completeness::Complete);
    let unpacked = run(&set, &[], folder.path()).unwrap();
    assert_eq!(
        unpacked.placed,
        folder.path().join("vols"),
        "its one folder"
    );
    assert_eq!(unpacked.bytes, 205_000 + 2_050);
    let sizes: Vec<u64> = unpacked
        .files
        .iter()
        .map(|file| fs::metadata(file).unwrap().len())
        .collect();
    assert_eq!(sizes, [205_000, 2_050]);
    assert_eq!(
        tree(&folder.path().join("vols")),
        ["bigfile.txt", "smallfile.txt"]
    );
}

#[test]
fn older_rar_parts_unpack_together() {
    let folder = TempDir::new().unwrap();
    let parts = [
        fixture("rar3-old.rar", folder.path(), "Old.rar"),
        fixture("rar3-old.r00", folder.path(), "Old.r00"),
        fixture("rar3-old.r01", folder.path(), "Old.r01"),
    ];
    let unpacked = run(&the_set(&parts[2..]), &[], folder.path()).unwrap();
    assert_eq!(unpacked.files.len(), 2);
    assert_eq!(
        fs::metadata(folder.path().join("vols/bigfile.txt"))
            .unwrap()
            .len(),
        205_000
    );
}

#[test]
fn a_rar_part_its_names_didnt_tell_about_is_missing() {
    let folder = TempDir::new().unwrap();
    let part = fixture("missing.part1.rar", folder.path(), "Set.part1.rar");
    let set = the_set(&[part]);
    assert_eq!(set.completeness(), Completeness::Complete);
    assert_eq!(
        problem_of(run(&set, &[], folder.path())),
        UnpackProblem::MissingPart {
            part: "Set.part2.rar".to_owned()
        }
    );
    assert_eq!(tree(folder.path()), ["Set.part1.rar"]);
}

#[test]
fn rar_passwords_are_tried_for_names_and_data() {
    // Rar 5 with its names shown and hidden, and rar 3 with its names hidden.
    for (fixture_name, files) in [
        ("rar5-psw.rar", 2),
        ("rar5-hpsw.rar", 2),
        ("rar3-comment-hpsw.rar", 2),
    ] {
        let folder = TempDir::new().unwrap();
        let archive = fixture(fixture_name, folder.path(), "Locked.rar");
        let set = the_set(&[archive]);
        assert_eq!(
            problem_of(run(&set, &[], folder.path())),
            UnpackProblem::NeedsPassword,
            "{fixture_name}"
        );
        assert_eq!(
            problem_of(run(&set, &["nope"], folder.path())),
            UnpackProblem::WrongPassword { tried: 1 },
            "{fixture_name}"
        );
        assert_eq!(tree(folder.path()), ["Locked.rar"], "{fixture_name}");
        let unpacked = run(&set, &["nope", "password"], folder.path()).unwrap();
        assert_eq!(unpacked.password, Some(2), "{fixture_name}");
        assert_eq!(unpacked.files.len(), files, "{fixture_name}");
    }
}

#[test]
fn a_damaged_archive_says_so() {
    let folder = TempDir::new().unwrap();
    let zip = folder.path().join("Broken.zip");
    make_zip(&zip, &[("a.bin", &noise(10_000))], None);
    let mut bytes = fs::read(&zip).unwrap();
    // Its data is garbled; its directory, at the end, is kept.
    for byte in &mut bytes[60..400] {
        *byte = byte.wrapping_add(1);
    }
    fs::write(&zip, bytes).unwrap();
    assert!(matches!(
        problem_of(run(&the_set(&[zip]), &[], folder.path())),
        UnpackProblem::Damaged { .. }
    ));
    assert_eq!(tree(folder.path()), ["Broken.zip"]);

    let not_rar = folder.path().join("Fake.rar");
    fs::write(&not_rar, b"this isn't a rar archive").unwrap();
    assert!(matches!(
        problem_of(run(&the_set(&[not_rar]), &[], folder.path())),
        UnpackProblem::Damaged { .. }
    ));
}

#[test]
fn an_archive_too_big_for_its_drive_isnt_unpacked() {
    let folder = TempDir::new().unwrap();
    let zip = folder.path().join("Big.zip");
    make_zip(&zip, &[("a.bin", &[0_u8; 1_000])], None);
    let problem = problem_of(run_with(
        &the_set(&[zip]),
        &[],
        folder.path(),
        &Disk(Some(50_000_000)),
        &CancelToken::new(),
    ));
    assert!(
        matches!(
            problem,
            UnpackProblem::NoRoom {
                free: 50_000_000,
                ..
            }
        ),
        "{problem:?}"
    );
}

#[test]
fn a_stop_leaves_nothing_behind() {
    let folder = TempDir::new().unwrap();
    let zip = folder.path().join("Album.zip");
    make_zip(&zip, &[("a.txt", b"a"), ("b.txt", b"b")], None);
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(
        run_with(&the_set(&[zip]), &[], folder.path(), &ROOMY, &cancel),
        Err(UnpackError::Cancelled)
    );
    assert_eq!(tree(folder.path()), ["Album.zip"]);
}

#[test]
fn progress_counts_up_to_the_whole_size() {
    let folder = TempDir::new().unwrap();
    let zip = folder.path().join("Album.zip");
    make_zip(
        &zip,
        &[("a.bin", &[1_u8; 3_000]), ("b.bin", &[2_u8; 5_000])],
        None,
    );
    let set = the_set(&[zip]);
    let mut seen = Vec::new();
    unpack(
        &Request {
            set: &set,
            passwords: &[],
            place: UnpackPlace::Smart,
            work_folder: &folder.path().join("work"),
            disks: &ROOMY,
            cancel: &CancelToken::new(),
        },
        &mut |written, total| seen.push((written, total)),
    )
    .unwrap();
    assert_eq!(seen.last(), Some(&(8_000, 8_000)));
    assert!(seen.windows(2).all(|pair| pair[0].0 <= pair[1].0));
}

#[test]
fn parts_are_put_away_as_settings_say() {
    let folder = TempDir::new().unwrap();
    let files = touch(folder.path(), &["A.7z.001", "A.7z.002"]);
    let set = the_set(&files);
    put_away(&set, AfterUnpacking::Keep).unwrap();
    assert!(files.iter().all(|file| file.exists()));
    put_away(&set, AfterUnpacking::Delete).unwrap();
    assert!(files.iter().all(|file| !file.exists()));
}

#[test]
fn an_archive_is_unpacked_by_one_download_at_a_time() {
    let folder = TempDir::new().unwrap();
    let files = touch(folder.path(), &["Shared.part1.rar"]);
    let set = the_set(&files);
    let held = claim(&set).unwrap();
    assert!(claim(&set).is_none());
    drop(held);
    assert!(claim(&set).is_some());
}

/// Unpacks every archive in the folder `UNPACK_LIVE` names beside it, trying the passwords in `UNPACK_PASSWORDS` (one
/// per line), and prints what came of each: for archives made by other programs (7-Zip, WinRAR). The archives are
/// kept. `UNPACK_LIVE=<folder> cargo test -p tsundoku-core --lib unpack_live -- --ignored --nocapture`
#[test]
#[ignore = "unpacks the archives in the folder UNPACK_LIVE names"]
fn unpack_live() {
    let folder = PathBuf::from(std::env::var("UNPACK_LIVE").expect("UNPACK_LIVE names a folder"));
    let passwords: Vec<String> = std::env::var("UNPACK_PASSWORDS")
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect();
    let mut files: Vec<PathBuf> = fs::read_dir(&folder)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .collect();
    files.sort();
    let work = folder.join(".tsundoku-live").join("unpacking");
    for set in sets_among(&files) {
        let name = set.name();
        match set.completeness() {
            Completeness::Complete => {}
            other => {
                println!("{name}: left as it is: {other:?}");
                continue;
            }
        }
        let result = unpack(
            &Request {
                set: &set,
                passwords: &passwords,
                place: UnpackPlace::Smart,
                work_folder: &work,
                disks: &crate::room::SystemDisks,
                cancel: &CancelToken::new(),
            },
            &mut |_, _| {},
        );
        match result {
            Ok(unpacked) => println!(
                "{name}: {} files, {} bytes, to {} (password {:?}, left out {:?})",
                unpacked.files.len(),
                unpacked.bytes,
                unpacked.placed.display(),
                unpacked.password,
                unpacked.left_out
            ),
            Err(error) => println!("{name}: {error:?}"),
        }
    }
    let _ = fs::remove_dir_all(folder.join(".tsundoku-live"));
}
