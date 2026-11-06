//! yt-dlp, ffmpeg, Deno and gallery-dl: finding copies already on this PC, downloading the official builds into the
//! data folder's `tools\`, checking that they run, and keeping the app's copies up to date.

mod catalog;
mod checksum;
mod github;
mod install;
mod locate;
mod version;

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use self::catalog::spec;
use self::github::{GitHub, Release};
use crate::cancel::CancelToken;
use crate::data_folder::DataPaths;
use crate::process::{self, ProgramCommand, RunError};
use crate::settings::{SettingsError, SettingsStore, ToolCaches, ToolSetup, UpdateChannel};
use crate::sync::lock;

/// The folder of gallery-dl's cache in the data folder's `cache\`. yt-dlp's and Deno's are made by the tools.
const GALLERY_DL_CACHE: &str = "gallery-dl";

/// The longest a tool may take to print its version. yt-dlp's first start unpacks itself, and an antivirus may scan
/// it first.
const VERSION_TIMEOUT: Duration = Duration::from_secs(60);
const UPDATE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// The tools that update themselves (`ToolId::UPDATED_DAILY`) are checked at most this often.
const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// How long a release looked up on GitHub is reused, so the setup's sizes and downloads take one request per tool.
const RELEASE_CACHE_LIFETIME: Duration = Duration::from_secs(10 * 60);

/// Windows error codes meaning Windows Security, an antivirus or an app control policy stopped a program from running.
const BLOCKED_ERROR_CODES: [i32; 4] = [
    5,    // ERROR_ACCESS_DENIED
    225,  // ERROR_VIRUS_INFECTED
    226,  // ERROR_VIRUS_DELETED
    4551, // ERROR_SYSTEM_INTEGRITY_POLICY_VIOLATION (Smart App Control)
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ToolId {
    /// The default, so errors saved before they named their tool read as yt-dlp's.
    #[default]
    YtDlp,
    Ffmpeg,
    Deno,
    /// Downloads image posts, such as Instagram photos. Set up with the others, but optional like Deno: downloads of
    /// videos work without it.
    GalleryDl,
}

impl ToolId {
    pub const ALL: [Self; 4] = [Self::YtDlp, Self::Ffmpeg, Self::Deno, Self::GalleryDl];
    /// The tools the first-run setup sets up: all of them. Only yt-dlp and ffmpeg are needed to leave it.
    pub const SETUP: [Self; 4] = Self::ALL;
    /// The tools whose app copies update themselves once a day, since sites' changes break them: yt-dlp, and gallery-dl
    /// for Instagram and X. ffmpeg's and Deno's updates are only offered.
    pub const UPDATED_DAILY: [Self; 2] = [Self::YtDlp, Self::GalleryDl];
}

/// Why a tool can't be set up, updated or run. The UI turns each into a sentence; `detail` fields carry the
/// technical cause, shown under it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum ToolError {
    /// GitHub couldn't be reached, or answered with an error.
    #[error("network error: {detail}")]
    Network { detail: String },
    /// GitHub allows 60 requests an hour from a network without signing in.
    #[error("GitHub's limit on requests was reached")]
    RateLimited,
    /// The latest release lacks a file the app needs.
    #[error("the latest release is missing {file}")]
    MissingFromRelease { file: String },
    #[error("the release lists no checksum for {file}")]
    ChecksumMissing { file: String },
    /// The download isn't the file the release describes.
    #[error("the download doesn't match its checksum")]
    ChecksumMismatch,
    /// Windows Security or an antivirus removed the file or stopped it from running.
    #[error("Windows Security or an antivirus blocked or removed it")]
    Blocked,
    #[error("{path} doesn't exist")]
    FileMissing { path: String },
    /// yt-dlp needs ffprobe in the same folder as ffmpeg.
    #[error("ffprobe isn't in ffmpeg's folder")]
    FfprobeMissing,
    /// The file runs, but its version isn't what the tool prints.
    #[error("not the expected program; it printed {output:?}")]
    NotThisTool { output: String },
    #[error("couldn't run it: {detail}")]
    WontRun { detail: String },
    #[error("the update failed: {detail}")]
    UpdateFailed { detail: String },
    /// The tool is already being installed or updated.
    #[error("already being installed or updated")]
    Busy,
    /// Its old copy kept running (previews, reads, downloads) for as long as an install waits to replace it.
    #[error("in use for too long to be replaced")]
    InUse,
    #[error("cancelled")]
    Cancelled,
    /// Only the app's own copy is updated by the app.
    #[error("not the app's own copy")]
    NotManaged,
    /// A file couldn't be written, moved or deleted.
    #[error("file error: {detail}")]
    Storage { detail: String },
    /// A bug: the work stopped unexpectedly.
    #[error("unexpected error: {detail}")]
    Unexpected { detail: String },
}

impl From<io::Error> for ToolError {
    fn from(error: io::Error) -> Self {
        Self::Storage {
            detail: error.to_string(),
        }
    }
}

impl From<SettingsError> for ToolError {
    fn from(error: SettingsError) -> Self {
        Self::Storage {
            detail: error.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ToolSetupKind {
    /// The app's own copy, which it keeps up to date.
    Managed,
    /// A copy the user picked, which the app never updates.
    External,
}

/// A tool as it's set up now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolStatus {
    pub id: ToolId,
    /// Which copy runs, or `None` while the tool isn't set up.
    pub setup: Option<ToolSetupKind>,
    pub path: Option<String>,
    pub version: Option<String>,
    /// Why the copy that's set up doesn't run.
    pub problem: Option<ToolError>,
}

/// Every tool's status, for Settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolsOverview {
    /// yt-dlp, ffmpeg, Deno and gallery-dl, in that order.
    pub tools: Vec<ToolStatus>,
    pub yt_dlp_channel: UpdateChannel,
    /// Where the tools keep their caches; an install from before the first-run setup asked uses their usual folders.
    pub caches: ToolCaches,
    /// The data folder's `cache\`, where they are with `ToolCaches::DataFolder`.
    pub cache_folder: String,
}

/// Where the tools keep their caches when the data folder holds them (`ToolCaches::DataFolder`): one folder each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCacheFolder {
    root: PathBuf,
}

impl ToolCacheFolder {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// yt-dlp's `--cache-dir`: the sites' player code it has worked out, so it isn't worked out again each time.
    pub fn yt_dlp_args(&self) -> [String; 2] {
        [
            "--cache-dir".to_owned(),
            self.root.join("yt-dlp").display().to_string(),
        ]
    }

    /// gallery-dl's cache database (sign-ins and sites' tokens), set with `-o`.
    pub fn gallery_dl_args(&self) -> [String; 2] {
        [
            "-o".to_owned(),
            format!(
                "cache.file={}",
                self.root
                    .join(GALLERY_DL_CACHE)
                    .join("cache.sqlite3")
                    .display()
            ),
        ]
    }

    /// `DENO_DIR` for the Deno yt-dlp starts, which keeps the scripts it downloads there.
    pub fn deno_variable(&self) -> (&'static str, PathBuf) {
        ("DENO_DIR", self.root.join("deno"))
    }
}

/// A copy of a tool found on this PC, offered at setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FoundCopy {
    pub path: String,
    pub version: String,
    /// The app's own copy, left from an earlier setup.
    pub is_managed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DetectedTool {
    pub id: ToolId,
    /// The best copy that runs, or `None` if there's none.
    pub found: Option<FoundCopy>,
}

/// How big a tool's download is, for the setup screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolDownload {
    pub id: ToolId,
    /// In bytes, or `None` when GitHub couldn't be asked.
    #[ts(type = "number | null")]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolUpdateCheck {
    pub id: ToolId,
    pub update_available: bool,
    /// The latest release: a version, or a date for ffmpeg's daily builds.
    pub latest: String,
}

/// Sent while a tool is installed or updated. The last one for an operation has no stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolProgress {
    pub id: ToolId,
    pub stage: Option<ToolStage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum ToolStage {
    /// Asking GitHub for the latest release.
    Connecting,
    Downloading {
        #[ts(type = "number")]
        received: u64,
        #[ts(type = "number | null")]
        total: Option<u64>,
    },
    /// Comparing the download with its checksum.
    Verifying,
    /// Unpacking and putting it in place.
    Installing,
    /// Making sure it runs.
    Checking,
    /// yt-dlp updating itself.
    Updating,
    /// Waiting for the runs of the old copy (previews, reads, downloads) to end before replacing it.
    Waiting,
}

/// Runs everything to do with the tools. Shared by the app's commands; every method may be called from any thread.
pub struct ToolManager {
    paths: DataPaths,
    settings: Arc<SettingsStore>,
    app_version: String,
    /// The tools being installed or updated, each with what cancels it.
    operations: Mutex<HashMap<ToolId, CancelToken>>,
    releases: Mutex<HashMap<ToolId, (Instant, Release)>>,
}

impl ToolManager {
    /// `app_version` goes in the User-Agent of requests to GitHub.
    pub fn new(paths: DataPaths, settings: Arc<SettingsStore>, app_version: String) -> Self {
        Self {
            paths,
            settings,
            app_version,
            operations: Mutex::default(),
            releases: Mutex::default(),
        }
    }

    /// Whether the first-run setup is needed: downloads can't start until yt-dlp and ffmpeg are set up. Deno is
    /// optional.
    pub fn needs_setup(&self) -> bool {
        let tools = self.settings.get().tools;
        tools.yt_dlp.is_none() || tools.ffmpeg.is_none()
    }

    /// The program to run for a tool, or `None` while it isn't set up.
    pub fn executable(&self, id: ToolId) -> Option<PathBuf> {
        let tools = self.settings.get().tools;
        tools.get(id).map(|setup| self.executable_for(id, setup))
    }

    /// Every tool's status. Runs each tool to read its version, so it takes a moment.
    pub fn overview(&self) -> ToolsOverview {
        let tools = self.settings.get().tools;
        ToolsOverview {
            tools: for_each_tool(&ToolId::ALL, |id| match tools.get(id) {
                Some(setup) => {
                    let executable = self.executable_for(id, setup);
                    tool_status(id, setup, &executable, read_version(id, &executable))
                }
                None => ToolStatus {
                    id,
                    setup: None,
                    path: None,
                    version: None,
                    problem: None,
                },
            }),
            yt_dlp_channel: tools.yt_dlp_channel,
            caches: tools.caches.unwrap_or(ToolCaches::UsualFolders),
            cache_folder: self.paths.cache_folder().display().to_string(),
        }
    }

    /// The copies already on this PC that run, for the first-run setup's tools.
    pub fn detect(&self) -> Vec<DetectedTool> {
        let path_variable = env::var_os("PATH");
        let tools_folder = self.tools_folder();
        for_each_tool(&ToolId::SETUP, |id| DetectedTool {
            id,
            found: locate::candidates(id, &tools_folder, path_variable.as_deref())
                .into_iter()
                .find_map(|candidate| {
                    let version = read_version(id, &candidate.path).ok()?;
                    Some(FoundCopy {
                        path: candidate.path.display().to_string(),
                        version,
                        is_managed: candidate.is_managed,
                    })
                }),
        })
    }

    /// The size of each of the first-run setup's downloads, asked of GitHub. A size is `None` when GitHub couldn't be
    /// asked.
    pub fn downloads(&self) -> Vec<ToolDownload> {
        let github = GitHub::new(&self.app_version).ok();
        for_each_tool(&ToolId::SETUP, |id| ToolDownload {
            id,
            size: github
                .as_ref()
                .and_then(|github| self.latest_release(id, github, false).ok())
                .and_then(|release| {
                    release
                        .asset(spec(id).release.asset)
                        .ok()
                        .map(|asset| asset.size)
                }),
        })
    }

    /// Downloads the tool's latest official build into `tools\` and sets it up as the app's own copy, replacing any
    /// earlier one.
    pub fn install(
        &self,
        id: ToolId,
        on_progress: &dyn Fn(ToolProgress),
    ) -> Result<ToolStatus, ToolError> {
        let operation = self.begin(id, on_progress)?;
        operation.report(ToolStage::Connecting);
        let github = GitHub::new(&self.app_version)?;
        let release = self.latest_release(id, &github, false)?;
        let checksum = install::release_checksum(id, &release, &github)?;
        install::install_release(
            id,
            &release,
            &checksum,
            &github,
            &self.tools_folder(),
            &operation.cancel,
            &|stage| operation.report(stage),
        )?;
        operation.report(ToolStage::Checking);
        self.adopt_installed(id, checksum)
    }

    /// Sets up a copy already on this PC, such as one found at setup or picked in Settings.
    pub fn use_copy(&self, id: ToolId, path: PathBuf) -> Result<ToolStatus, ToolError> {
        let version = read_version(id, &path)?;
        let managed = locate::managed_executable(&self.tools_folder(), id);
        let setup = if is_same_file(&path, &managed) {
            match self.settings.get().tools.get(id) {
                Some(current @ ToolSetup::Managed { .. }) => current.clone(),
                _ => ToolSetup::Managed { checksum: None },
            }
        } else {
            ToolSetup::External { path: path.clone() }
        };
        self.settings
            .update(|settings| settings.tools.set(id, Some(setup.clone())))?;
        Ok(tool_status(
            id,
            &setup,
            &self.executable_for(id, &setup),
            Ok(version),
        ))
    }

    /// Stops the tool's install or update, if one is running.
    pub fn cancel(&self, id: ToolId) {
        if let Some(cancel) = lock(&self.operations).get(&id) {
            cancel.cancel();
        }
    }

    /// Updates the app's copy of yt-dlp to the latest release of its channel, with yt-dlp's own updater.
    pub fn update_yt_dlp(
        &self,
        on_progress: &dyn Fn(ToolProgress),
    ) -> Result<ToolStatus, ToolError> {
        let tools = self.settings.get().tools;
        let Some(setup @ ToolSetup::Managed { .. }) = tools.yt_dlp else {
            return Err(ToolError::NotManaged);
        };
        let operation = self.begin(ToolId::YtDlp, on_progress)?;
        operation.report(ToolStage::Updating);
        let executable = locate::managed_executable(&self.tools_folder(), ToolId::YtDlp);
        let command = ProgramCommand::new(&executable).args([
            "--ignore-config",
            "--update-to",
            tools.yt_dlp_channel.yt_dlp_update_target(),
        ]);
        let result =
            process::run_collecting_with_cancel(&command, &operation.cancel, UPDATE_TIMEOUT);
        // Recorded even when it failed, so a failing update isn't retried at every start.
        self.record_update_check(ToolId::YtDlp);

        let output = result.map_err(|error| match error {
            RunError::Start { source, .. } => start_error(source, &executable),
            RunError::Cancelled => ToolError::Cancelled,
            other => ToolError::UpdateFailed {
                detail: other.to_string(),
            },
        })?;
        if !output.status.success() {
            return Err(ToolError::UpdateFailed {
                detail: output.last_line().unwrap_or_default().to_owned(),
            });
        }
        operation.report(ToolStage::Checking);
        let version = read_version(ToolId::YtDlp, &executable).map_err(missing_means_blocked)?;
        Ok(tool_status(ToolId::YtDlp, &setup, &executable, Ok(version)))
    }

    /// Updates the app's copy of gallery-dl to its latest release, unless it's that release already. Through the app's
    /// own install rather than gallery-dl's `--update`, which looks for its GitHub releases: they've had no files since
    /// gallery-dl moved to Codeberg. Waits for its runs to end before replacing it (`ToolStage::Waiting`).
    pub fn update_gallery_dl(
        &self,
        on_progress: &dyn Fn(ToolProgress),
    ) -> Result<ToolStatus, ToolError> {
        self.update_to_latest(
            ToolId::GalleryDl,
            &ReleaseHosts { manager: self },
            on_progress,
        )
        .map(|update| update.status)
    }

    /// Whether the app's copy of a tool that updates itself (`ToolId::UPDATED_DAILY`) is due its daily update.
    pub fn update_due(&self, id: ToolId) -> bool {
        let tools = self.settings.get().tools;
        ToolId::UPDATED_DAILY.contains(&id)
            && matches!(tools.get(id), Some(ToolSetup::Managed { .. }))
            && tools.last_update_check(id).is_none_or(|last_check| {
                seconds_since_epoch().saturating_sub(last_check) >= UPDATE_CHECK_INTERVAL.as_secs()
            })
    }

    /// Runs the daily update of each tool that's due one (yt-dlp, then gallery-dl), one after the other. A tool that's
    /// running (a preview, a read or a download) is left for a later call, so no run has its program replaced under it.
    /// Returns whether a tool changed.
    pub fn run_due_updates(&self, on_progress: &dyn Fn(ToolProgress)) -> bool {
        let mut changed = false;
        for id in ToolId::UPDATED_DAILY {
            if !self.update_due(id) || self.is_running(id) {
                continue;
            }
            changed |= if id == ToolId::YtDlp {
                self.update_yt_dlp(on_progress).is_ok()
            } else {
                self.update_to_latest(id, &ReleaseHosts { manager: self }, on_progress)
                    .is_ok_and(|update| update.changed)
            };
        }
        changed
    }

    /// Asks the release host whether a newer build than the app's copy exists. For ffmpeg and Deno, whose updates are
    /// offered, never automatic.
    pub fn check_for_update(&self, id: ToolId) -> Result<ToolUpdateCheck, ToolError> {
        let Some(ToolSetup::Managed { checksum }) = self.settings.get().tools.get(id).cloned()
        else {
            return Err(ToolError::NotManaged);
        };
        let github = GitHub::new(&self.app_version)?;
        let release = self.latest_release(id, &github, true)?;
        let latest_checksum = install::release_checksum(id, &release, &github)?;
        Ok(ToolUpdateCheck {
            id,
            update_available: checksum.as_deref() != Some(latest_checksum.as_str()),
            latest: release.label(),
        })
    }

    pub fn set_yt_dlp_channel(&self, channel: UpdateChannel) -> Result<(), ToolError> {
        self.settings
            .update(|settings| settings.tools.yt_dlp_channel = channel)?;
        Ok(())
    }

    /// Settings → Tools: where the tools keep their caches from their next run on.
    pub fn set_caches(&self, caches: ToolCaches) -> Result<(), ToolError> {
        self.settings
            .update(|settings| settings.tools.caches = Some(caches))?;
        Ok(())
    }

    /// Where the tools keep their caches when the data folder holds them, or `None` for their usual folders.
    pub fn cache_folder(&self) -> Option<ToolCacheFolder> {
        (self.settings.get().tools.caches == Some(ToolCaches::DataFolder)).then(|| {
            let root = self.paths.cache_folder();
            // gallery-dl uses a cache file only if its folder exists; yt-dlp and Deno make their own.
            let _ = fs::create_dir_all(root.join(GALLERY_DL_CACHE));
            ToolCacheFolder::new(root)
        })
    }

    /// Updates the app's copy of `id` to the latest build `releases` has, unless it's that build already, and records the
    /// daily check.
    fn update_to_latest(
        &self,
        id: ToolId,
        releases: &dyn Releases,
        on_progress: &dyn Fn(ToolProgress),
    ) -> Result<ToolUpdate, ToolError> {
        let Some(ToolSetup::Managed { checksum: current }) =
            self.settings.get().tools.get(id).cloned()
        else {
            return Err(ToolError::NotManaged);
        };
        let operation = self.begin(id, on_progress)?;
        operation.report(ToolStage::Connecting);
        let installed = releases.latest(id).and_then(|(release, latest)| {
            if current.as_deref() == Some(latest.as_str()) {
                return Ok(None);
            }
            releases.install(
                id,
                &release,
                &latest,
                &self.tools_folder(),
                &operation.cancel,
                &|stage| operation.report(stage),
            )?;
            Ok(Some(latest))
        });
        // Recorded even when it failed, so a failing update isn't retried at every start; not when the old copy stayed
        // in use, so it's tried again once it's free.
        if !matches!(installed, Err(ToolError::InUse)) {
            self.record_update_check(id);
        }

        match installed? {
            None => {
                let setup = ToolSetup::Managed { checksum: current };
                let executable = locate::managed_executable(&self.tools_folder(), id);
                Ok(ToolUpdate {
                    status: tool_status(id, &setup, &executable, read_version(id, &executable)),
                    changed: false,
                })
            }
            Some(checksum) => {
                operation.report(ToolStage::Checking);
                Ok(ToolUpdate {
                    status: self.adopt_installed(id, checksum)?,
                    changed: true,
                })
            }
        }
    }

    /// Sets up the copy just installed in the tool's folder as the app's own, once it runs. `checksum` is its release
    /// file's.
    fn adopt_installed(&self, id: ToolId, checksum: String) -> Result<ToolStatus, ToolError> {
        let executable = locate::managed_executable(&self.tools_folder(), id);
        let version = read_version(id, &executable).map_err(missing_means_blocked)?;
        let setup = ToolSetup::Managed {
            checksum: Some(checksum),
        };
        self.settings.update(|settings| {
            settings.tools.set(id, Some(setup.clone()));
            // Just downloaded, so already up to date.
            settings
                .tools
                .set_last_update_check(id, seconds_since_epoch());
        })?;
        Ok(tool_status(id, &setup, &executable, Ok(version)))
    }

    /// Records that a tool that updates itself was just checked. A record that can't be saved only means an earlier
    /// check next time.
    fn record_update_check(&self, id: ToolId) {
        let _ = self.settings.update(|settings| {
            settings
                .tools
                .set_last_update_check(id, seconds_since_epoch());
        });
    }

    /// Whether a program from the app's copy of the tool is running.
    fn is_running(&self, id: ToolId) -> bool {
        process::is_running_from(&self.tools_folder().join(spec(id).folder))
    }

    /// The folder the app's own copies go in: the one picked at the first-run setup, or the data folder's `tools\`.
    fn tools_folder(&self) -> PathBuf {
        self.settings
            .get()
            .tools
            .folder
            .filter(|folder| !folder.as_os_str().is_empty())
            .unwrap_or_else(|| self.paths.tools_folder())
    }

    fn executable_for(&self, id: ToolId, setup: &ToolSetup) -> PathBuf {
        match setup {
            ToolSetup::Managed { .. } => locate::managed_executable(&self.tools_folder(), id),
            ToolSetup::External { path } => path.clone(),
        }
    }

    /// The tool's latest release, reused for a few minutes unless `fresh` is asked.
    fn latest_release(
        &self,
        id: ToolId,
        github: &GitHub,
        fresh: bool,
    ) -> Result<Release, ToolError> {
        if !fresh
            && let Some((fetched, release)) = lock(&self.releases).get(&id)
            && fetched.elapsed() < RELEASE_CACHE_LIFETIME
        {
            return Ok(release.clone());
        }
        let release = github.latest_release(&spec(id).release)?;
        lock(&self.releases).insert(id, (Instant::now(), release.clone()));
        Ok(release)
    }

    /// Marks the tool as busy until the returned operation ends, so one tool never has two installs at once.
    fn begin<'a>(
        &'a self,
        id: ToolId,
        on_progress: &'a dyn Fn(ToolProgress),
    ) -> Result<Operation<'a>, ToolError> {
        let mut operations = lock(&self.operations);
        if operations.contains_key(&id) {
            return Err(ToolError::Busy);
        }
        let cancel = CancelToken::new();
        operations.insert(id, cancel.clone());
        Ok(Operation {
            manager: self,
            id,
            cancel,
            on_progress,
        })
    }
}

/// What an update did: the copy's status, and whether it's a new build.
struct ToolUpdate {
    status: ToolStatus,
    changed: bool,
}

/// Where an update finds a tool's latest build and installs it: the release hosts, or a stand-in in tests.
trait Releases {
    /// The latest release, and the SHA-256 it lists for the tool's file.
    fn latest(&self, id: ToolId) -> Result<(Release, String), ToolError>;

    /// Downloads the tool's file from `release`, checks it against `checksum` and puts it in its folder under
    /// `tools_folder` once its old copy isn't running.
    fn install(
        &self,
        id: ToolId,
        release: &Release,
        checksum: &str,
        tools_folder: &Path,
        cancel: &CancelToken,
        report: &dyn Fn(ToolStage),
    ) -> Result<(), ToolError>;
}

/// The tools' release hosts: GitHub, and Codeberg for gallery-dl.
struct ReleaseHosts<'a> {
    manager: &'a ToolManager,
}

impl Releases for ReleaseHosts<'_> {
    fn latest(&self, id: ToolId) -> Result<(Release, String), ToolError> {
        let github = GitHub::new(&self.manager.app_version)?;
        let release = self.manager.latest_release(id, &github, true)?;
        let checksum = install::release_checksum(id, &release, &github)?;
        Ok((release, checksum))
    }

    fn install(
        &self,
        id: ToolId,
        release: &Release,
        checksum: &str,
        tools_folder: &Path,
        cancel: &CancelToken,
        report: &dyn Fn(ToolStage),
    ) -> Result<(), ToolError> {
        let github = GitHub::new(&self.manager.app_version)?;
        install::install_release(id, release, checksum, &github, tools_folder, cancel, report)
    }
}

/// An install or update in progress. When it ends, however it ends, the tool is no longer busy and a last progress
/// report without a stage tells the UI.
struct Operation<'a> {
    manager: &'a ToolManager,
    id: ToolId,
    cancel: CancelToken,
    on_progress: &'a dyn Fn(ToolProgress),
}

impl Operation<'_> {
    fn report(&self, stage: ToolStage) {
        (self.on_progress)(ToolProgress {
            id: self.id,
            stage: Some(stage),
        });
    }
}

impl Drop for Operation<'_> {
    fn drop(&mut self) {
        lock(&self.manager.operations).remove(&self.id);
        (self.on_progress)(ToolProgress {
            id: self.id,
            stage: None,
        });
    }
}

/// Whether `tools_folder` holds one of Tsundoku's own copies of the first-run setup's tools, such as an installed
/// Tsundoku's, which the setup then offers to use.
pub fn has_own_copies(tools_folder: &Path) -> bool {
    ToolId::SETUP
        .iter()
        .any(|&id| locate::managed_executable(tools_folder, id).is_file())
}

/// Runs `work` for each of `ids` at once, since each mostly waits on a program or the network, and returns the
/// results in the order of `ids`.
fn for_each_tool<T: Send>(ids: &[ToolId], work: impl Fn(ToolId) -> T + Sync) -> Vec<T> {
    thread::scope(|scope| {
        let work = &work;
        let handles: Vec<_> = ids
            .iter()
            .map(|&id| scope.spawn(move || work(id)))
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    })
}

/// Runs the tool to read its version, which also tells it's the right program and that it runs.
fn read_version(id: ToolId, executable: &Path) -> Result<String, ToolError> {
    if !executable.is_file() {
        return Err(ToolError::FileMissing {
            path: executable.display().to_string(),
        });
    }
    if id == ToolId::Ffmpeg && !locate::ffprobe_beside(executable).is_file() {
        return Err(ToolError::FfprobeMissing);
    }
    let command = ProgramCommand::new(executable).args(spec(id).version_args.iter().copied());
    let output =
        process::run_collecting(&command, VERSION_TIMEOUT).map_err(|error| match error {
            RunError::Start { source, .. } => start_error(source, executable),
            other => ToolError::WontRun {
                detail: other.to_string(),
            },
        })?;
    let first_line = output.stdout().next().unwrap_or_default();
    version::parse(id, first_line).ok_or_else(|| ToolError::NotThisTool {
        output: output
            .lines
            .first()
            .map(|line| line.text().to_owned())
            .unwrap_or_default(),
    })
}

/// Why a tool couldn't start: its file is gone, something blocked it, or another cause.
pub(crate) fn start_error(error: io::Error, executable: &Path) -> ToolError {
    let blocked = cfg!(windows)
        && error
            .raw_os_error()
            .is_some_and(|code| BLOCKED_ERROR_CODES.contains(&code));
    if error.kind() == io::ErrorKind::NotFound {
        ToolError::FileMissing {
            path: executable.display().to_string(),
        }
    } else if blocked || error.kind() == io::ErrorKind::PermissionDenied {
        ToolError::Blocked
    } else {
        ToolError::WontRun {
            detail: error.to_string(),
        }
    }
}

/// A file gone right after the app wrote it was taken by an antivirus.
fn missing_means_blocked(error: ToolError) -> ToolError {
    match error {
        ToolError::FileMissing { .. } | ToolError::FfprobeMissing => ToolError::Blocked,
        other => other,
    }
}

/// The status of a tool that's set up, from reading its version.
fn tool_status(
    id: ToolId,
    setup: &ToolSetup,
    executable: &Path,
    version: Result<String, ToolError>,
) -> ToolStatus {
    let (version, problem) = match version {
        Ok(version) => (Some(version), None),
        Err(problem) => (None, Some(problem)),
    };
    ToolStatus {
        id,
        setup: Some(match setup {
            ToolSetup::Managed { .. } => ToolSetupKind::Managed,
            ToolSetup::External { .. } => ToolSetupKind::External,
        }),
        path: Some(executable.display().to_string()),
        version,
        problem,
    }
}

fn is_same_file(first: &Path, second: &Path) -> bool {
    match (fs::canonicalize(first), fs::canonicalize(second)) {
        (Ok(first), Ok(second)) => first == second,
        _ => first == second,
    }
}

fn seconds_since_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager_in(folder: &tempfile::TempDir) -> ToolManager {
        let paths = DataPaths::new(folder.path().to_path_buf());
        let settings = Arc::new(SettingsStore::open(paths.settings_file()).unwrap());
        ToolManager::new(paths, settings, "0.0.0".into())
    }

    #[test]
    fn needs_setup_until_yt_dlp_and_ffmpeg_are_set_up() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        assert!(manager.needs_setup());

        manager
            .settings
            .update(|settings| {
                settings
                    .tools
                    .set(ToolId::YtDlp, Some(ToolSetup::Managed { checksum: None }));
                settings.tools.set(
                    ToolId::Ffmpeg,
                    Some(ToolSetup::External {
                        path: "ffmpeg.exe".into(),
                    }),
                );
            })
            .unwrap();
        assert!(!manager.needs_setup());
    }

    #[test]
    fn puts_its_own_copies_in_the_folder_picked_at_setup() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        assert_eq!(manager.tools_folder(), folder.path().join("tools"));

        manager
            .settings
            .update(|settings| {
                settings.tools.folder = Some(PathBuf::from(r"D:\Tools"));
                settings
                    .tools
                    .set(ToolId::YtDlp, Some(ToolSetup::Managed { checksum: None }));
            })
            .unwrap();
        assert_eq!(manager.tools_folder(), PathBuf::from(r"D:\Tools"));
        assert_eq!(
            manager.executable(ToolId::YtDlp),
            Some(locate::managed_executable(
                Path::new(r"D:\Tools"),
                ToolId::YtDlp
            ))
        );
    }

    #[test]
    fn keeps_the_caches_in_the_data_folder_once_chosen() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        assert_eq!(
            manager.cache_folder(),
            None,
            "their usual folders until chosen"
        );
        assert_eq!(manager.overview().caches, ToolCaches::UsualFolders);

        manager.set_caches(ToolCaches::DataFolder).unwrap();
        let cache_root = folder.path().join("cache");
        assert_eq!(
            manager.cache_folder(),
            Some(ToolCacheFolder::new(cache_root.clone()))
        );
        assert!(
            cache_root.join("gallery-dl").is_dir(),
            "made for gallery-dl"
        );
        assert_eq!(manager.overview().caches, ToolCaches::DataFolder);

        manager.set_caches(ToolCaches::UsualFolders).unwrap();
        assert_eq!(manager.cache_folder(), None);
    }

    #[test]
    fn reports_a_missing_file() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        let missing = folder.path().join("yt-dlp.exe");
        let error = manager
            .use_copy(ToolId::YtDlp, missing.clone())
            .unwrap_err();
        assert_eq!(
            error,
            ToolError::FileMissing {
                path: missing.display().to_string()
            }
        );
    }

    #[test]
    fn only_updates_the_apps_own_copy() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        assert_eq!(manager.update_yt_dlp(&|_| {}), Err(ToolError::NotManaged));
        assert_eq!(
            manager.update_gallery_dl(&|_| {}),
            Err(ToolError::NotManaged)
        );
        assert_eq!(
            manager.check_for_update(ToolId::Deno),
            Err(ToolError::NotManaged)
        );
        assert!(!manager.update_due(ToolId::YtDlp));

        manager
            .settings
            .update(|settings| {
                settings.tools.set(
                    ToolId::GalleryDl,
                    Some(ToolSetup::External {
                        path: "gallery-dl.exe".into(),
                    }),
                );
            })
            .unwrap();
        assert!(
            !manager.update_due(ToolId::GalleryDl),
            "never the user's copy"
        );
        assert!(!manager.run_due_updates(&|_| {}));
    }

    /// Sets the tool up as the app's own copy from the release file with `checksum`, last checked at `last_check`.
    fn set_managed(manager: &ToolManager, id: ToolId, checksum: &str, last_check: Option<u64>) {
        manager
            .settings
            .update(|settings| {
                settings.tools.set(
                    id,
                    Some(ToolSetup::Managed {
                        checksum: Some(checksum.to_owned()),
                    }),
                );
                settings.tools.yt_dlp_last_update_check = None;
                settings.tools.gallery_dl_last_update_check = None;
                if let Some(seconds) = last_check {
                    settings.tools.set_last_update_check(id, seconds);
                }
            })
            .unwrap();
    }

    #[test]
    fn updates_yt_dlp_and_gallery_dl_once_a_day() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        for id in ToolId::UPDATED_DAILY {
            set_managed(&manager, id, "abc", None);
            assert!(manager.update_due(id), "{id:?} never checked");
            set_managed(&manager, id, "abc", Some(seconds_since_epoch() - 60));
            assert!(!manager.update_due(id), "{id:?} checked a minute ago");
            set_managed(
                &manager,
                id,
                "abc",
                Some(seconds_since_epoch() - UPDATE_CHECK_INTERVAL.as_secs()),
            );
            assert!(manager.update_due(id), "{id:?} checked a day ago");
        }
        for id in [ToolId::Ffmpeg, ToolId::Deno] {
            set_managed(&manager, id, "abc", None);
            assert!(!manager.update_due(id), "{id:?}'s updates are only offered");
        }
    }

    /// A release host that has `latest`, and records each install asked of it, which ends with `install_result`.
    struct FakeReleases {
        latest: Result<String, ToolError>,
        install_result: Result<(), ToolError>,
        installs: Mutex<Vec<String>>,
    }

    impl FakeReleases {
        fn new(latest: Result<&str, ToolError>, install_result: Result<(), ToolError>) -> Self {
            Self {
                latest: latest.map(str::to_owned),
                install_result,
                installs: Mutex::default(),
            }
        }
    }

    impl Releases for FakeReleases {
        fn latest(&self, _id: ToolId) -> Result<(Release, String), ToolError> {
            let release = Release {
                tag_name: "v1.32.0".into(),
                published_at: None,
                assets: Vec::new(),
            };
            self.latest.clone().map(|checksum| (release, checksum))
        }

        fn install(
            &self,
            _id: ToolId,
            _release: &Release,
            checksum: &str,
            _tools_folder: &Path,
            _cancel: &CancelToken,
            _report: &dyn Fn(ToolStage),
        ) -> Result<(), ToolError> {
            lock(&self.installs).push(checksum.to_owned());
            self.install_result.clone()
        }
    }

    #[test]
    fn leaves_a_copy_that_is_already_the_latest_build() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        set_managed(&manager, ToolId::GalleryDl, "abc", None);
        let releases = FakeReleases::new(Ok("abc"), Ok(()));
        let ends = Mutex::new(0);
        let on_progress = |progress: ToolProgress| {
            if progress.stage.is_none() {
                *lock(&ends) += 1;
            }
        };

        let update = manager
            .update_to_latest(ToolId::GalleryDl, &releases, &on_progress)
            .unwrap();
        assert!(!update.changed);
        assert_eq!(update.status.setup, Some(ToolSetupKind::Managed));
        assert!(lock(&releases.installs).is_empty(), "nothing downloaded");
        assert!(
            !manager.update_due(ToolId::GalleryDl),
            "the check is recorded"
        );
        assert_eq!(*lock(&ends), 1, "the operation ended");
        assert_eq!(
            manager.settings.get().tools.gallery_dl,
            Some(ToolSetup::Managed {
                checksum: Some("abc".into())
            })
        );
    }

    #[test]
    fn installs_a_newer_build_and_records_a_failed_check() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        set_managed(&manager, ToolId::GalleryDl, "abc", None);
        let releases = FakeReleases::new(
            Ok("def"),
            Err(ToolError::Network {
                detail: "offline".into(),
            }),
        );
        assert!(matches!(
            manager.update_to_latest(ToolId::GalleryDl, &releases, &|_| {}),
            Err(ToolError::Network { .. })
        ));
        assert_eq!(*lock(&releases.installs), ["def"]);
        assert!(
            !manager.update_due(ToolId::GalleryDl),
            "a failed update isn't retried at once"
        );
        assert_eq!(
            manager.settings.get().tools.gallery_dl,
            Some(ToolSetup::Managed {
                checksum: Some("abc".into())
            }),
            "the old copy stays"
        );

        set_managed(&manager, ToolId::GalleryDl, "abc", None);
        let unreachable = FakeReleases::new(Err(ToolError::RateLimited), Ok(()));
        assert_eq!(
            manager
                .update_to_latest(ToolId::GalleryDl, &unreachable, &|_| {})
                .err(),
            Some(ToolError::RateLimited)
        );
        assert!(!manager.update_due(ToolId::GalleryDl));
    }

    #[test]
    fn tries_again_soon_when_the_old_copy_stayed_in_use() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        set_managed(&manager, ToolId::GalleryDl, "abc", None);
        let releases = FakeReleases::new(Ok("def"), Err(ToolError::InUse));
        assert_eq!(
            manager
                .update_to_latest(ToolId::GalleryDl, &releases, &|_| {})
                .err(),
            Some(ToolError::InUse)
        );
        assert!(manager.update_due(ToolId::GalleryDl), "not recorded");
    }

    #[test]
    fn a_busy_tool_refuses_a_second_operation_and_reports_its_end() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        let reports = Mutex::new(Vec::new());
        let on_progress = |progress: ToolProgress| lock(&reports).push(progress);

        let operation = manager.begin(ToolId::Deno, &on_progress).unwrap();
        assert!(matches!(
            manager.begin(ToolId::Deno, &on_progress),
            Err(ToolError::Busy)
        ));
        assert!(manager.begin(ToolId::Ffmpeg, &on_progress).is_ok());
        manager.cancel(ToolId::Deno);
        assert!(operation.cancel.is_cancelled());
        drop(operation);

        assert!(manager.begin(ToolId::Deno, &on_progress).is_ok());
        let ends: Vec<_> = lock(&reports)
            .iter()
            .filter(|progress| progress.stage.is_none())
            .map(|p| p.id)
            .collect();
        assert_eq!(ends, [ToolId::Ffmpeg, ToolId::Deno, ToolId::Deno]);
    }
}
