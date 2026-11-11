//! Downloading a file host's link (`hosts`) without a tool: every file of the share, one after another, into the
//! item's folder, a shared folder keeping its own folders. Each file downloads into the item's temporary folder,
//! continuing what an earlier run left there, is decrypted as it arrives (MEGA), checked once complete (MEGA's MAC,
//! MediaFire's SHA-256), then moved into place. A file behind a check only a person can pass (`hosts::check`) is
//! left for later while the others download, and the run ends asking for the check.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use reqwest::StatusCode;
use reqwest::blocking::Client;
use reqwest::header::RANGE;
use sha1::Sha1;
use sha2::{Digest, Sha256};

use super::runner::{JobOutcome, JobSpec, make_folders, open_log, seconds_since_epoch, write_log};
use super::{DownloadError, DownloadProgress, DownloadStage};
use crate::cancel::CancelToken;
use crate::hosts::check::{BrowserCheck, CheckPage};
use crate::hosts::{self, Address, FileCheck, Host, Page, RemoteFile};
use crate::metadata::PreviewError;

/// How much is read from the network or the disk at a time.
const BUFFER_SIZE: usize = 256 * 1024;
/// How often progress is reported.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(500);

/// Why a run ended early.
enum Stop {
    Cancelled,
    Failed(DownloadError),
    /// Files a person has to open the check for; the others are downloaded.
    Check(BrowserCheck),
}

impl From<DownloadError> for Stop {
    fn from(error: DownloadError) -> Self {
        Self::Failed(error)
    }
}

impl From<PreviewError> for Stop {
    fn from(error: PreviewError) -> Self {
        match error {
            PreviewError::Cancelled => Self::Cancelled,
            error => Self::Failed(DownloadError::YtDlp { error }),
        }
    }
}

pub(super) fn run(
    job: &JobSpec,
    cancel: &CancelToken,
    on_progress: &mut dyn FnMut(DownloadProgress),
) -> JobOutcome {
    // Appended to, so a resumed or retried download keeps the earlier runs' lines above.
    let mut log = open_log(&job.log_path);
    let outcome = download(job, cancel, &mut log, on_progress);
    match outcome {
        Ok(files) => JobOutcome::Finished {
            files,
            format: None,
        },
        Err(Stop::Cancelled) => JobOutcome::Cancelled,
        Err(Stop::Check(check)) => {
            write_log(
                &mut log,
                &format!(
                    "waiting for a person to open {}'s check ({} files)",
                    check.site,
                    check.pages.len()
                ),
            );
            JobOutcome::NeedsCheck(check)
        }
        Err(Stop::Failed(error)) => {
            write_log(&mut log, &format!("failed: {error}"));
            JobOutcome::Failed(error)
        }
    }
}

fn download(
    job: &JobSpec,
    cancel: &CancelToken,
    log: &mut Option<File>,
    on_progress: &mut dyn FnMut(DownloadProgress),
) -> Result<Vec<PathBuf>, Stop> {
    make_folders(job)?;
    // The part after "#" is MEGA's key, which stays out of the log.
    let link = job.url.split('#').next().unwrap_or_default();
    write_log(log, &format!("=== {} {link}", seconds_since_epoch()));
    on_progress(DownloadProgress::at(DownloadStage::Starting));

    let host = Host::of(&job.url).ok_or(PreviewError::UnsupportedSite)?;
    let client = hosts::client()?;
    let mut share = host.read(&client, &job.url, cancel)?;
    share.use_caught_links(&job.caught_links);
    write_log(
        log,
        &format!("{} files, {} bytes", share.files.len(), share.total_bytes()),
    );

    let rate = job.rate_limit.as_deref().and_then(bytes_per_second);
    let mut meter = Meter::new(share.total_bytes(), rate);
    let mut saved = Vec::new();
    // Files behind a check only a person can pass, left for later.
    let mut behind: Option<BrowserCheck> = None;
    // A rule's name only for a share of one file, which might have become a folder since it was added.
    let single_name = job
        .shared_file_name
        .as_deref()
        .filter(|_| share.files.len() == 1);
    for (index, file) in share.files.iter().enumerate() {
        let destination = destination_of(&job.folder, &file.path, single_name);
        if fs::metadata(&destination)
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() == file.size)
        {
            write_log(
                log,
                &format!("{}: already in the folder", file.path.display()),
            );
            meter.skip(file.size);
            saved.push(destination);
            continue;
        }
        let part = job.temp_folder.join(format!("{index}.part"));
        let fetched = file
            .source
            .address(&client, cancel)
            .map_err(Stop::from)
            .and_then(|address| {
                write_log(
                    log,
                    &format!("{}: {} bytes", file.path.display(), file.size),
                );
                fetch(
                    &client,
                    host,
                    &address,
                    file,
                    &part,
                    cancel,
                    &mut meter,
                    log,
                    on_progress,
                )
                .map(|()| address)
            });
        let address = match fetched {
            Ok(address) => address,
            Err(stop) => {
                let page = behind_check(stop, file)?;
                write_log(
                    log,
                    &format!("{}: behind a check, left for later", file.path.display()),
                );
                behind
                    .get_or_insert_with(|| BrowserCheck {
                        site: host.name().to_owned(),
                        pages: Vec::new(),
                    })
                    .pages
                    .push(page);
                continue;
            }
        };
        // Checking a file and moving it is quick between a folder's files, so only the last one shows Finishing.
        if index + 1 == share.files.len() {
            on_progress(DownloadProgress::at(DownloadStage::Finishing));
        }
        if !check(&address.check, &part, cancel)? {
            // A damaged file is downloaded again from the start.
            let _ = fs::remove_file(&part);
            return Err(DownloadError::Damaged {
                file: file.path.display().to_string(),
            }
            .into());
        }
        saved.push(place(&part, &destination)?);
    }
    if let Some(check) = behind {
        return Err(Stop::Check(check));
    }
    on_progress(DownloadProgress::at(DownloadStage::Finishing));
    Ok(saved)
}

/// The page to open for a file the host put behind a check only a person can pass, or the run's end as it was.
fn behind_check(stop: Stop, file: &RemoteFile) -> Result<CheckPage, Stop> {
    match (&stop, CheckPage::of(&file.source)) {
        (
            Stop::Failed(DownloadError::YtDlp {
                error: PreviewError::BrowserCheck { .. },
            }),
            Some(page),
        ) => Ok(page),
        _ => Err(stop),
    }
}

/// Downloads a file to `part`, continuing what's already there when the host allows.
#[expect(
    clippy::too_many_arguments,
    reason = "one file's download, with everything around it"
)]
fn fetch(
    client: &Client,
    host: Host,
    address: &Address,
    file: &RemoteFile,
    part: &Path,
    cancel: &CancelToken,
    meter: &mut Meter,
    log: &mut Option<File>,
    on_progress: &mut dyn FnMut(DownloadProgress),
) -> Result<(), Stop> {
    let mut offset = fs::metadata(part).map_or(0, |metadata| metadata.len());
    if offset > file.size {
        offset = 0;
    }
    if offset == file.size && offset > 0 {
        meter.skip(offset);
        return Ok(());
    }
    let mut request = client.get(&address.url);
    if offset > 0 {
        request = request.header(RANGE, format!("bytes={offset}-"));
    }
    let mut response = request.send().map_err(hosts::network_error)?;
    if let Some(error) = hosts::status_error(host, &response) {
        return Err(error.into());
    }
    // A page about an error, such as Google Drive's download limit, mustn't be saved as part of the file.
    if host.sends_attachments() && !hosts::is_attachment(response.headers()) {
        return Err(host.page_error(&Page::read(response)).into());
    }
    if offset > 0 {
        if response.status() == StatusCode::PARTIAL_CONTENT {
            write_log(log, &format!("continuing at {offset} bytes"));
        } else {
            // The host sent the whole file instead.
            offset = 0;
        }
    }
    meter.skip(offset);
    meter.start_transfer();

    let mut output = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(part)
        .map_err(|error| file_error(part, &error))?;
    output
        .set_len(offset)
        .map_err(|error| file_error(part, &error))?;
    output
        .seek(SeekFrom::Start(offset))
        .map_err(|error| file_error(part, &error))?;
    let mut decryptor = match &address.check {
        FileCheck::Mega(key) => Some(key.decryptor(offset)),
        _ => None,
    };
    let mut buffer = vec![0; BUFFER_SIZE];
    loop {
        if cancel.is_cancelled() {
            return Err(Stop::Cancelled);
        }
        let count = response
            .read(&mut buffer)
            .map_err(|error| PreviewError::Network {
                detail: error.to_string(),
            })?;
        if count == 0 {
            break;
        }
        let data = &mut buffer[..count];
        if let Some(decryptor) = &mut decryptor {
            decryptor.apply(data);
        }
        output
            .write_all(data)
            .map_err(|error| file_error(part, &error))?;
        offset += count as u64;
        meter.add(count as u64, cancel, on_progress)?;
    }
    output.flush().map_err(|error| file_error(part, &error))?;
    if offset != file.size {
        // What arrived is kept, so Retry continues from there.
        return Err(PreviewError::Network {
            detail: format!(
                "{} stopped sending after {offset} of {} bytes",
                host.name(),
                file.size
            ),
        }
        .into());
    }
    Ok(())
}

/// Whether a downloaded file is what the host sent: MEGA's MAC, MediaFire's SHA-256 or OneDrive's SHA-1. Google Drive
/// and Dropbox files are only checked by their size.
fn check(check: &FileCheck, part: &Path, cancel: &CancelToken) -> Result<bool, Stop> {
    match check {
        FileCheck::None => Ok(true),
        FileCheck::Mega(key) => {
            let mut checker = key.mac_checker();
            read_file(part, cancel, &mut |data| checker.update(data))?;
            Ok(checker.matches())
        }
        FileCheck::Sha256(expected) => Ok(&digest::<Sha256>(part, cancel)? == expected),
        FileCheck::Sha1(expected) => Ok(&digest::<Sha1>(part, cancel)? == expected),
    }
}

/// A file's hash, in lowercase hex.
fn digest<Hasher: Digest>(part: &Path, cancel: &CancelToken) -> Result<String, Stop> {
    let mut hasher = Hasher::new();
    read_file(part, cancel, &mut |data| hasher.update(data))?;
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Feeds a file to `consume`, a buffer at a time.
fn read_file(
    path: &Path,
    cancel: &CancelToken,
    consume: &mut dyn FnMut(&[u8]),
) -> Result<(), Stop> {
    let mut input = File::open(path).map_err(|error| file_error(path, &error))?;
    let mut buffer = vec![0; BUFFER_SIZE];
    loop {
        if cancel.is_cancelled() {
            return Err(Stop::Cancelled);
        }
        let count = input
            .read(&mut buffer)
            .map_err(|error| file_error(path, &error))?;
        if count == 0 {
            return Ok(());
        }
        consume(&buffer[..count]);
    }
}

/// Moves a finished file to `destination`, making its folders. A different file already there is kept, and this one
/// takes the next free name ("Name (2).zip").
fn place(part: &Path, destination: &Path) -> Result<PathBuf, DownloadError> {
    let folder = destination.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(folder).map_err(|error| DownloadError::Folder {
        path: folder.display().to_string(),
        detail: error.to_string(),
    })?;
    let destination = free_name(destination);
    fs::rename(part, &destination).map_err(|error| file_error(&destination, &error))?;
    Ok(destination)
}

/// Where a share's file goes in the download's folder: its own path, or, named by a rule (`name`), that name with the
/// file's own extension in the same subfolder.
fn destination_of(folder: &Path, path: &Path, name: Option<&str>) -> PathBuf {
    let own = folder.join(path);
    match name {
        Some(name) => {
            let extension = path
                .extension()
                .map(|extension| format!(".{}", extension.to_string_lossy()))
                .unwrap_or_default();
            own.with_file_name(format!("{name}{extension}"))
        }
        None => own,
    }
}

/// `path`, or the first of "Name (2).ext", "Name (3).ext"… that isn't taken.
pub(crate) fn free_name(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_owned();
    }
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    (2..)
        .map(|number| path.with_file_name(format!("{stem} ({number}){extension}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or_else(|| path.to_owned())
}

fn file_error(path: &Path, error: &std::io::Error) -> DownloadError {
    DownloadError::Folder {
        path: path.display().to_string(),
        detail: error.to_string(),
    }
}

/// A speed limit as the settings write it ("500K", "4.2M", "1G", "800"), in bytes per second.
fn bytes_per_second(limit: &str) -> Option<f64> {
    let limit = limit.trim();
    let (number, multiplier) = match limit.chars().last()?.to_ascii_uppercase() {
        'K' => (&limit[..limit.len() - 1], 1024.0),
        'M' => (&limit[..limit.len() - 1], 1024.0 * 1024.0),
        'G' => (&limit[..limit.len() - 1], 1024.0 * 1024.0 * 1024.0),
        _ => (limit, 1.0),
    };
    let value: f64 = number.parse().ok()?;
    (value > 0.0).then_some(value * multiplier)
}

/// Counts what's downloaded across the share's files: the progress, the speed, the time left, and the speed limit.
struct Meter {
    total: u64,
    done: u64,
    /// The speed limit, in bytes per second.
    rate: Option<f64>,
    /// When downloading started, and how much arrived since: what the speed limit paces.
    paced_since: Instant,
    paced_bytes: u64,
    last_report: Instant,
    bytes_since_report: u64,
    speed: Option<f64>,
}

impl Meter {
    fn new(total: u64, rate: Option<f64>) -> Self {
        Self {
            total,
            done: 0,
            rate,
            paced_since: Instant::now(),
            paced_bytes: 0,
            last_report: Instant::now(),
            bytes_since_report: 0,
            speed: None,
        }
    }

    /// Counts bytes that didn't need downloading: a file already there, or the part an earlier run fetched.
    fn skip(&mut self, count: u64) {
        self.done += count;
    }

    /// Starts timing afresh once a file's bytes begin to arrive, so the time spent asking for its address and
    /// connecting neither lowers the speed shown nor lets the next bytes come faster than the speed limit.
    fn start_transfer(&mut self) {
        let now = Instant::now();
        self.paced_since = now;
        self.paced_bytes = 0;
        self.last_report = now;
        self.bytes_since_report = 0;
    }

    /// Counts bytes just downloaded, waits if they came faster than the speed limit, and reports now and then.
    fn add(
        &mut self,
        count: u64,
        cancel: &CancelToken,
        on_progress: &mut dyn FnMut(DownloadProgress),
    ) -> Result<(), Stop> {
        self.done += count;
        self.paced_bytes += count;
        self.bytes_since_report += count;
        if let Some(rate) = self.rate {
            let due = Duration::from_secs_f64(self.paced_bytes as f64 / rate);
            while self.paced_since.elapsed() < due {
                if cancel.is_cancelled() {
                    return Err(Stop::Cancelled);
                }
                thread::sleep((due - self.paced_since.elapsed()).min(Duration::from_millis(100)));
            }
        }
        let elapsed = self.last_report.elapsed();
        if elapsed >= PROGRESS_INTERVAL {
            let speed = self.bytes_since_report as f64 / elapsed.as_secs_f64();
            // Smoothed, so the time left doesn't jump about.
            self.speed = Some(
                self.speed
                    .map_or(speed, |previous| previous * 0.7 + speed * 0.3),
            );
            self.last_report = Instant::now();
            self.bytes_since_report = 0;
            on_progress(self.progress());
        }
        Ok(())
    }

    fn progress(&self) -> DownloadProgress {
        let left = self.total.saturating_sub(self.done);
        DownloadProgress {
            fraction: (self.total > 0).then(|| (self.done as f64 / self.total as f64).min(1.0)),
            downloaded_bytes: Some(self.done),
            total_bytes: Some(self.total),
            speed: self.speed,
            eta: self
                .speed
                .filter(|speed| *speed > 0.0)
                .map(|speed| (left as f64 / speed).round() as u64),
            ..DownloadProgress::at(DownloadStage::Downloading)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_speed_limits() {
        assert_eq!(bytes_per_second("500K"), Some(512_000.0));
        assert_eq!(bytes_per_second("2M"), Some(2_097_152.0));
        assert_eq!(bytes_per_second("1.5k"), Some(1536.0));
        assert_eq!(bytes_per_second("800"), Some(800.0));
        assert_eq!(bytes_per_second("fast"), None);
        assert_eq!(bytes_per_second("0"), None);
    }

    #[test]
    fn a_taken_name_gets_a_number() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("Pack.zip");
        assert_eq!(free_name(&path), path);
        fs::write(&path, "one").unwrap();
        assert_eq!(free_name(&path), folder.path().join("Pack (2).zip"));
        fs::write(folder.path().join("Pack (2).zip"), "two").unwrap();
        assert_eq!(free_name(&path), folder.path().join("Pack (3).zip"));
    }

    #[test]
    fn names_a_file_as_a_rule_says_keeping_its_extension() {
        let folder = Path::new(r"D:\Files");
        let path = Path::new("SomefileRJ123456.rar");
        assert_eq!(destination_of(folder, path, None), folder.join(path));
        assert_eq!(
            destination_of(folder, path, Some("RJ123456")),
            folder.join("RJ123456.rar")
        );
        assert_eq!(
            destination_of(folder, Path::new("README"), Some("Notes")),
            folder.join("Notes"),
            "a file without an extension"
        );
    }

    #[test]
    fn places_a_file_in_its_folder() {
        let folder = tempfile::tempdir().unwrap();
        let part = folder.path().join("0.part");
        fs::write(&part, "data").unwrap();
        let destination = folder.path().join("Shared").join("Sub").join("a.txt");
        assert_eq!(place(&part, &destination).unwrap(), destination);
        assert_eq!(fs::read_to_string(&destination).unwrap(), "data");
        assert!(!part.exists());
    }

    #[test]
    fn checks_a_sha256_and_a_sha1() {
        let folder = tempfile::tempdir().unwrap();
        let part = folder.path().join("0.part");
        fs::write(&part, "abc").unwrap();
        let right = FileCheck::Sha256(
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_owned(),
        );
        let wrong = FileCheck::Sha256("00".repeat(32));
        let cancel = CancelToken::new();
        assert!(matches!(check(&right, &part, &cancel), Ok(true)));
        assert!(matches!(check(&wrong, &part, &cancel), Ok(false)));
        assert!(matches!(check(&FileCheck::None, &part, &cancel), Ok(true)));
        let sha1 = FileCheck::Sha1("a9993e364706816aba3e25717850c26c9cd0d89d".to_owned());
        assert!(matches!(check(&sha1, &part, &cancel), Ok(true)));
        let wrong_sha1 = FileCheck::Sha1("00".repeat(20));
        assert!(matches!(check(&wrong_sha1, &part, &cancel), Ok(false)));
    }

    #[test]
    fn a_file_behind_a_check_is_left_for_later_when_it_has_a_page_to_open() {
        use crate::hosts::FileSource;
        let check = || {
            Stop::Failed(DownloadError::YtDlp {
                error: PreviewError::BrowserCheck {
                    site: "MediaFire".to_owned(),
                },
            })
        };
        let file = |source| RemoteFile {
            path: PathBuf::from("a.zip"),
            size: 1,
            source,
        };
        let mediafire = file(FileSource::MediaFire(hosts::mediafire::MediaFireFile {
            quick_key: "abc123".to_owned(),
            sha256: None,
            direct: None,
        }));
        assert!(matches!(
            behind_check(check(), &mediafire),
            Ok(page) if page.url == "https://www.mediafire.com/file/abc123"
        ));
        let dropbox = file(FileSource::Dropbox(hosts::dropbox::DropboxFile {
            url: "https://www.dropbox.com/scl/fi/a/a.zip?dl=1".to_owned(),
        }));
        assert!(
            matches!(behind_check(check(), &dropbox), Err(Stop::Failed(_))),
            "no page a person could open: it fails"
        );
        assert!(matches!(
            behind_check(Stop::Failed(DownloadError::NoFile), &mediafire),
            Err(Stop::Failed(DownloadError::NoFile))
        ));
    }

    #[test]
    fn reports_progress_across_files() {
        let mut meter = Meter::new(1000, None);
        meter.skip(250);
        let progress = meter.progress();
        assert_eq!(progress.stage, DownloadStage::Downloading);
        assert_eq!(progress.fraction, Some(0.25));
        assert_eq!(progress.downloaded_bytes, Some(250));
        assert_eq!(progress.total_bytes, Some(1000));
        assert_eq!(progress.eta, None, "no speed yet");
    }
}
