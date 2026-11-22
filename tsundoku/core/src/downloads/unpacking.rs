//! The step after a download: unpacking the archives among its files (`unpack`) when Settings → Archives says to. A
//! split archive whose other parts come with other downloads is unpacked by the download that brings its last part;
//! the others leave theirs as they are. An archive that can't be unpacked fails the download with its files kept, so
//! Retry unpacks again without downloading. One with a password is tried with the passwords Tsundoku knows
//! (`passwords::known`), and the queue is told which opened it when it isn't one of Settings' list.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::runner::{open_log, seconds_since_epoch, write_log};
use super::{DownloadError, DownloadProgress, DownloadStage, JobOutcome, JobSpec};
use crate::cancel::CancelToken;
use crate::room::DiskSpace;
use crate::settings::{AfterUnpacking, UnpackPlace};
use crate::unpack::{self, ArchiveSet, Completeness, Request, UnpackError};

/// How often progress is told while unpacking: every this much of an archive.
const PROGRESS_STEPS: u64 = 200;

/// What unpacking takes from Settings → Archives, from the rule that says where to unpack, and from the passwords
/// Tsundoku knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnpackJob {
    pub place: UnpackPlace,
    pub after: AfterUnpacking,
    /// Tried in order on an archive with a password (`KnownPasswords::for_archive`).
    pub passwords: Vec<String>,
    /// How many of them, from the first, are Settings → Passwords' list. The others are the download's own and the
    /// ones used before.
    pub listed: usize,
    /// Told each password that opened an archive and isn't the list's, to keep it for the next ones.
    pub opened: Option<OpenedWith>,
}

/// What's told a password, not one of Settings' list, that opened an archive (`UnpackJob::opened`).
#[derive(Clone)]
pub struct OpenedWith(Arc<dyn Fn(&str) + Send + Sync>);

impl OpenedWith {
    pub fn new(tell: impl Fn(&str) + Send + Sync + 'static) -> Self {
        Self(Arc::new(tell))
    }

    /// Tells it `password` opened an archive.
    pub fn tell(&self, password: &str) {
        (self.0)(password);
    }
}

impl fmt::Debug for OpenedWith {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OpenedWith")
    }
}

/// The same one only: what two of them do can't be compared.
impl PartialEq for OpenedWith {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for OpenedWith {}

/// A download's files as unpacking left them, so Resume or Retry unpacks the rest without downloading again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DownloadedFiles {
    /// Files where they stay: those that aren't archives, what was unpacked, parts kept, and parts waiting for
    /// another download to bring the rest.
    pub settled: Vec<PathBuf>,
    /// The parts of the archives still to unpack.
    pub archives: Vec<PathBuf>,
    /// For a file host's link, the paths in the share of the files the download took (`JobOutcome::Finished`'s
    /// `shared`), carried through unpacking to the history, which can't tell them from what the archives held.
    pub shared: Vec<String>,
}

impl DownloadedFiles {
    /// Every file, as it is when unpacking won't run.
    pub fn all(self) -> Vec<PathBuf> {
        let mut all = self.settled;
        all.extend(self.archives);
        all
    }
}

/// What became of one archive.
enum Fate {
    /// What it was unpacked to, in place of its parts that are gone.
    Unpacked(Vec<PathBuf>),
    /// Left as it is: waiting for parts, a kind Tsundoku doesn't read, or another download unpacks it.
    Left,
    /// Still to unpack: it failed, or the download stopped first.
    Pending,
}

/// Unpacks the archives among `files.archives`, writing what happens in the download's log. Finished with every file
/// now there (in the order the download made them, what an archive held in place of its first part), or
/// `NotUnpacked` when it was stopped or an archive couldn't be unpacked.
pub(super) fn run(
    job: &JobSpec,
    settings: &UnpackJob,
    files: DownloadedFiles,
    format: Option<String>,
    disks: &dyn DiskSpace,
    cancel: &CancelToken,
    on_progress: &mut dyn FnMut(DownloadProgress),
) -> JobOutcome {
    let DownloadedFiles {
        settled,
        archives,
        shared,
    } = files;
    let archives: Vec<PathBuf> = archives.into_iter().filter(|path| path.exists()).collect();
    let sets = unpack::sets_among(&archives);
    if sets.is_empty() {
        let mut files = settled;
        files.extend(archives);
        return JobOutcome::Finished {
            files,
            format,
            shared,
        };
    }

    let mut log = open_log(&job.log_path);
    write_log(
        &mut log,
        &format!("=== {} unpacking", seconds_since_epoch()),
    );
    let work_folder = job.temp_folder.join("unpacking");
    let mut error = None;
    let mut stopped = false;
    let mut fates = Vec::with_capacity(sets.len());
    for set in &sets {
        let fate = if stopped {
            Fate::Pending
        } else {
            match unpack_one(
                set,
                settings,
                &work_folder,
                disks,
                cancel,
                &mut log,
                on_progress,
            ) {
                Ok(fate) => fate,
                Err(UnpackError::Cancelled) => {
                    write_log(&mut log, "stopped");
                    stopped = true;
                    Fate::Pending
                }
                Err(UnpackError::Problem(problem)) => {
                    write_log(
                        &mut log,
                        &format!("{}: couldn't unpack it: {problem}", set.name()),
                    );
                    error.get_or_insert(DownloadError::Unpack {
                        archive: set.name(),
                        problem,
                    });
                    Fate::Pending
                }
            }
        };
        fates.push(fate);
    }

    let left = arrange(settled, &archives, &sets, fates, shared);
    if stopped || error.is_some() {
        JobOutcome::NotUnpacked { files: left, error }
    } else {
        JobOutcome::Finished {
            shared: left.shared.clone(),
            files: left.all(),
            format,
        }
    }
}

fn unpack_one(
    set: &ArchiveSet,
    settings: &UnpackJob,
    work_folder: &Path,
    disks: &dyn DiskSpace,
    cancel: &CancelToken,
    log: &mut Option<std::fs::File>,
    on_progress: &mut dyn FnMut(DownloadProgress),
) -> Result<Fate, UnpackError> {
    let name = set.name();
    match set.completeness() {
        Completeness::Missing(part) => {
            write_log(
                log,
                &format!(
                    "{name}: left as it is until {part} is in its folder (another download may bring it)"
                ),
            );
            return Ok(Fate::Left);
        }
        Completeness::Unsupported(kind) => {
            write_log(
                log,
                &format!("{name}: left as it is: Tsundoku doesn't unpack {kind}"),
            );
            return Ok(Fate::Left);
        }
        Completeness::Complete => {}
    }
    let Some(_claim) = unpack::claim(set) else {
        write_log(log, &format!("{name}: another download is unpacking it"));
        return Ok(Fate::Left);
    };
    write_log(
        log,
        &format!("{name}: unpacking ({} parts)", set.parts().count()),
    );
    on_progress(DownloadProgress::at(DownloadStage::Unpacking));
    let mut told = 0_u64;
    let mut progress = |written: u64, total: u64| {
        let step = (total / PROGRESS_STEPS).max(1);
        if written < told + step && written < total {
            return;
        }
        told = written;
        on_progress(DownloadProgress {
            fraction: (total > 0).then(|| written as f64 / total as f64),
            downloaded_bytes: Some(written),
            total_bytes: Some(total),
            ..DownloadProgress::at(DownloadStage::Unpacking)
        });
    };
    let unpacked = unpack::unpack(
        &Request {
            set,
            passwords: &settings.passwords,
            place: settings.place,
            work_folder,
            disks,
            cancel,
        },
        &mut progress,
    )?;
    // Never the password itself: its place in the list, or that it isn't in it.
    let password = match unpacked.password {
        None => String::new(),
        Some(number) if number <= settings.listed => {
            format!(", with password {number} from the list")
        }
        Some(number) => {
            if let (Some(opened), Some(password)) =
                (&settings.opened, settings.passwords.get(number - 1))
            {
                opened.tell(password);
            }
            ", with a password that isn't in the list".to_owned()
        }
    };
    write_log(
        log,
        &format!(
            "{name}: unpacked {} files ({} bytes) to {}{password}",
            unpacked.files.len(),
            unpacked.bytes,
            unpacked.placed.display()
        ),
    );
    for entry in &unpacked.left_out {
        write_log(
            log,
            &format!("{name}: left out {entry} (a link, or a name outside its folder)"),
        );
    }
    on_progress(DownloadProgress::at(DownloadStage::Finishing));
    let put_away = match settings.after {
        AfterUnpacking::Keep => None,
        AfterUnpacking::RecycleBin => Some("moved its parts to the Recycle Bin"),
        AfterUnpacking::Delete => Some("deleted its parts"),
    };
    if let Some(done) = put_away {
        match unpack::put_away(set, settings.after) {
            Ok(()) => write_log(log, &format!("{name}: {done}")),
            Err(why) => write_log(log, &format!("{name}: its parts are kept: {why}")),
        }
    }
    let mut files = unpacked.files;
    if files.is_empty() {
        // It held no file: what it was unpacked to stands for it.
        files.push(unpacked.placed);
    }
    Ok(Fate::Unpacked(files))
}

/// The files once unpacking is over, in the order the download made them: what an archive held where its first part
/// among them was, its parts that are still there after it, and the parts of archives still to unpack set apart, with
/// the paths in the share the download took (`shared`) as they were.
fn arrange(
    mut settled: Vec<PathBuf>,
    archives: &[PathBuf],
    sets: &[ArchiveSet],
    fates: Vec<Fate>,
    shared: Vec<String>,
) -> DownloadedFiles {
    let mut pending = Vec::new();
    let mut placed = vec![false; sets.len()];
    for file in archives {
        let Some(index) = sets.iter().position(|set| set.holds(file)) else {
            settled.push(file.clone());
            continue;
        };
        match &fates[index] {
            Fate::Unpacked(files) => {
                if !placed[index] {
                    placed[index] = true;
                    settled.extend(files.iter().cloned());
                }
                if file.exists() {
                    settled.push(file.clone());
                }
            }
            Fate::Left => settled.push(file.clone()),
            Fate::Pending => pending.push(file.clone()),
        }
    }
    DownloadedFiles {
        settled,
        archives: pending,
        shared,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    struct Roomy;

    impl DiskSpace for Roomy {
        fn free(&self, _folder: &Path) -> Option<crate::room::DriveSpace> {
            None
        }
    }

    fn zip(path: &Path, entries: &[(&str, &[u8])]) {
        use std::io::Write;
        let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, data) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }

    fn settings(after: AfterUnpacking) -> UnpackJob {
        UnpackJob {
            place: UnpackPlace::Smart,
            after,
            passwords: Vec::new(),
            listed: 0,
            opened: None,
        }
    }

    /// A zip of one file, opened by `password`.
    fn locked_zip(path: &Path, password: &str) {
        use std::io::Write;
        let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .with_aes_encryption(zip::AesMode::Aes256, password);
        writer.start_file("inside.txt", options).unwrap();
        writer.write_all(b"inside").unwrap();
        writer.finish().unwrap();
    }

    #[test]
    fn a_password_that_isnt_the_lists_is_told_back_and_never_logged() {
        use std::sync::Mutex;

        let told = Arc::new(Mutex::new(Vec::<String>::new()));
        let job = |passwords: &[&str], listed: usize| {
            let told = Arc::clone(&told);
            UnpackJob {
                passwords: passwords.iter().map(|text| (*text).to_owned()).collect(),
                listed,
                opened: Some(OpenedWith::new(move |password| {
                    told.lock().unwrap().push(password.to_owned());
                })),
                ..settings(AfterUnpacking::Keep)
            }
        };
        let log_after = |job: &UnpackJob| {
            let folder = TempDir::new().unwrap();
            let archive = folder.path().join("Locked.zip");
            locked_zip(&archive, "open-sesame");
            let outcome = run_on(folder.path(), downloaded(&[&archive]), job);
            assert!(
                matches!(outcome, JobOutcome::Finished { .. }),
                "{outcome:?}"
            );
            fs::read_to_string(folder.path().join("logs").join("1.log")).unwrap()
        };

        // The list's second password opens it: nothing to keep.
        let log = log_after(&job(&["nope", "open-sesame", "used before"], 2));
        assert!(log.contains("with password 2 from the list"), "{log}");
        assert!(told.lock().unwrap().is_empty());

        // One used before, tried after the list's, opens it: the queue is told which.
        let log = log_after(&job(&["nope", "used before", "open-sesame"], 1));
        assert!(
            log.contains("with a password that isn't in the list"),
            "{log}"
        );
        assert!(!log.contains("open-sesame"), "{log}");
        assert_eq!(*told.lock().unwrap(), ["open-sesame"]);
    }

    fn run_on(folder: &Path, files: DownloadedFiles, settings: &UnpackJob) -> JobOutcome {
        let spec = JobSpec {
            id: 1,
            url: "https://example.com/album".to_owned(),
            preset: None,
            folder: folder.to_path_buf(),
            temp_folder: folder.join(".tsundoku-1"),
            log_path: folder.join("logs").join("1.log"),
            overwrite: false,
            yt_dlp: None,
            ffmpeg: None,
            deno: None,
            caches: None,
            rate_limit: None,
            cookies: None,
            post: None,
            files: None,
            files_pause: None,
            picked_files: None,
            shared_file_name: None,
            caught_links: Vec::new(),
            window_offer: None,
            unpack: Some(settings.clone()),
            unpack_only: None,
            password: None,
        };
        run(
            &spec,
            settings,
            files,
            None,
            &Roomy,
            &CancelToken::new(),
            &mut |_| {},
        )
    }

    fn downloaded(files: &[&Path]) -> DownloadedFiles {
        DownloadedFiles {
            settled: Vec::new(),
            archives: files.iter().map(|file| file.to_path_buf()).collect(),
            shared: Vec::new(),
        }
    }

    #[test]
    fn archives_are_unpacked_in_place_of_their_parts() {
        let folder = TempDir::new().unwrap();
        let cover = folder.path().join("cover.jpg");
        fs::write(&cover, b"cover").unwrap();
        let album = folder.path().join("Album.zip");
        zip(&album, &[("01.flac", b"one"), ("02.flac", b"two")]);
        let outcome = run_on(
            folder.path(),
            downloaded(&[&album, &cover]),
            &settings(AfterUnpacking::Delete),
        );
        let JobOutcome::Finished { files, .. } = outcome else {
            panic!("{outcome:?}");
        };
        assert_eq!(
            files,
            [
                folder.path().join("Album").join("01.flac"),
                folder.path().join("Album").join("02.flac"),
                cover,
            ]
        );
        assert!(!album.exists(), "deleted once unpacked");
        let log = fs::read_to_string(folder.path().join("logs").join("1.log")).unwrap();
        assert!(
            log.contains("Album.zip: unpacked 2 files (6 bytes)"),
            "{log}"
        );
        assert!(log.contains("Album.zip: deleted its parts"), "{log}");
    }

    #[test]
    fn kept_parts_stay_among_the_files() {
        let folder = TempDir::new().unwrap();
        let album = folder.path().join("Album.zip");
        zip(&album, &[("only.txt", b"only")]);
        let JobOutcome::Finished { files, .. } = run_on(
            folder.path(),
            downloaded(&[&album]),
            &settings(AfterUnpacking::Keep),
        ) else {
            panic!();
        };
        assert_eq!(files, [folder.path().join("only.txt"), album]);
    }

    #[test]
    fn a_part_waiting_for_the_rest_is_left_as_it_is() {
        let folder = TempDir::new().unwrap();
        let part = folder.path().join("Album.part2.rar");
        fs::write(&part, b"not yet").unwrap();
        let JobOutcome::Finished { files, .. } = run_on(
            folder.path(),
            downloaded(&[&part]),
            &settings(AfterUnpacking::Delete),
        ) else {
            panic!();
        };
        assert_eq!(files, std::slice::from_ref(&part));
        assert!(part.exists());
        let log = fs::read_to_string(folder.path().join("logs").join("1.log")).unwrap();
        assert!(
            log.contains("until Album.part1.rar is in its folder"),
            "{log}"
        );
    }

    #[test]
    fn an_archive_that_fails_is_kept_for_retry() {
        let folder = TempDir::new().unwrap();
        let notes = folder.path().join("notes.txt");
        fs::write(&notes, b"notes").unwrap();
        let fine = folder.path().join("Fine.zip");
        zip(&fine, &[("a.txt", b"a"), ("b.txt", b"b")]);
        let broken = folder.path().join("Broken.rar");
        fs::write(&broken, b"not a rar").unwrap();
        let outcome = run_on(
            folder.path(),
            downloaded(&[&notes, &broken, &fine]),
            &settings(AfterUnpacking::Delete),
        );
        let JobOutcome::NotUnpacked {
            files,
            error: Some(DownloadError::Unpack { archive, .. }),
        } = outcome
        else {
            panic!("{outcome:?}");
        };
        assert_eq!(archive, "Broken.rar");
        assert_eq!(
            files,
            DownloadedFiles {
                settled: vec![
                    notes.clone(),
                    folder.path().join("Fine").join("a.txt"),
                    folder.path().join("Fine").join("b.txt"),
                ],
                archives: vec![broken.clone()],
                shared: Vec::new(),
            },
            "the other archive is unpacked all the same"
        );

        // Retry, the archive fixed: only it is unpacked, and every file is there.
        fs::remove_file(&broken).unwrap();
        let fixed = folder.path().join("Broken.zip");
        zip(&fixed, &[("c.txt", b"c")]);
        let retry = DownloadedFiles {
            archives: vec![fixed],
            ..files
        };
        let JobOutcome::Finished { files, .. } =
            run_on(folder.path(), retry, &settings(AfterUnpacking::Delete))
        else {
            panic!();
        };
        assert_eq!(files.len(), 4);
        assert_eq!(files[3], folder.path().join("c.txt"));
    }

    #[test]
    fn a_shared_folders_paths_ride_through_unpacking() {
        let folder = TempDir::new().unwrap();
        let album = folder.path().join("Album.zip");
        zip(&album, &[("01.flac", b"one"), ("02.flac", b"two")]);
        let broken = folder.path().join("Broken.rar");
        fs::write(&broken, b"not a rar").unwrap();
        let shared = vec!["Pack/Album.zip".to_owned(), "Pack/Broken.rar".to_owned()];
        let outcome = run_on(
            folder.path(),
            DownloadedFiles {
                shared: shared.clone(),
                ..downloaded(&[&album, &broken])
            },
            &settings(AfterUnpacking::Delete),
        );
        let JobOutcome::NotUnpacked { files, .. } = outcome else {
            panic!("{outcome:?}");
        };
        assert_eq!(files.shared, shared, "kept for Retry");

        fs::remove_file(&broken).unwrap();
        let retry = DownloadedFiles {
            archives: Vec::new(),
            ..files
        };
        let JobOutcome::Finished {
            files,
            shared: taken,
            ..
        } = run_on(folder.path(), retry, &settings(AfterUnpacking::Delete))
        else {
            panic!();
        };
        assert_eq!(files.len(), 2, "what the album held");
        assert_eq!(
            taken, shared,
            "the files the download took, not what they held"
        );
    }

    #[test]
    fn puts_what_an_archive_holds_where_the_job_says() {
        let unpacked_with = |place: UnpackPlace, entries: &[(&str, &[u8])]| {
            let folder = TempDir::new().unwrap();
            let archive = folder.path().join("Pack.zip");
            zip(&archive, entries);
            let job = UnpackJob {
                place,
                ..settings(AfterUnpacking::Delete)
            };
            let JobOutcome::Finished { files, .. } =
                run_on(folder.path(), downloaded(&[&archive]), &job)
            else {
                panic!();
            };
            files
                .iter()
                .map(|file| {
                    file.strip_prefix(folder.path())
                        .unwrap()
                        .display()
                        .to_string()
                        .replace('\\', "/")
                })
                .collect::<Vec<String>>()
        };
        let one: &[(&str, &[u8])] = &[("only.txt", b"only")];
        let several: &[(&str, &[u8])] = &[
            ("Art/cover.jpg", b"cover"),
            ("b.txt", b"b"),
            ("a.txt", b"a"),
        ];

        assert_eq!(unpacked_with(UnpackPlace::Smart, one), ["only.txt"]);
        assert_eq!(
            unpacked_with(UnpackPlace::Smart, several),
            ["Pack/Art/cover.jpg", "Pack/a.txt", "Pack/b.txt"]
        );
        assert_eq!(
            unpacked_with(UnpackPlace::ArchiveFolder, one),
            ["Pack/only.txt"]
        );
        assert_eq!(
            unpacked_with(UnpackPlace::ArchiveFolder, several),
            ["Pack/Art/cover.jpg", "Pack/a.txt", "Pack/b.txt"]
        );
        assert_eq!(unpacked_with(UnpackPlace::InPlace, one), ["only.txt"]);
        assert_eq!(
            unpacked_with(UnpackPlace::InPlace, several),
            ["Art/cover.jpg", "a.txt", "b.txt"],
            "folders first, then files, each by name"
        );
    }
}
