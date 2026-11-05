//! yt-dlp, ffmpeg and Deno: finding copies already on this PC, downloading the official builds into the data
//! folder's `tools\`, checking that they run, and keeping the app's copies up to date.

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
use crate::settings::{SettingsError, SettingsStore, ToolSetup, UpdateChannel};
use crate::sync::lock;

/// The longest a tool may take to print its version. yt-dlp's first start unpacks itself, and an antivirus may scan
/// it first.
const VERSION_TIMEOUT: Duration = Duration::from_secs(60);
const UPDATE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// yt-dlp is checked for updates at startup at most this often.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ToolId {
    YtDlp,
    Ffmpeg,
    Deno,
}

impl ToolId {
    pub const ALL: [Self; 3] = [Self::YtDlp, Self::Ffmpeg, Self::Deno];
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
    /// yt-dlp, ffmpeg and Deno, in that order.
    pub tools: Vec<ToolStatus>,
    pub yt_dlp_channel: UpdateChannel,
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
            tools: for_each_tool(|id| match tools.get(id) {
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
        }
    }

    /// The copies already on this PC that run, for the first-run setup.
    pub fn detect(&self) -> Vec<DetectedTool> {
        let path_variable = env::var_os("PATH");
        let tools_folder = self.paths.tools_folder();
        for_each_tool(|id| DetectedTool {
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

    /// The size of each tool's download, asked of GitHub. A size is `None` when GitHub couldn't be asked.
    pub fn downloads(&self) -> Vec<ToolDownload> {
        let github = GitHub::new(&self.app_version).ok();
        for_each_tool(|id| ToolDownload {
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
        let checksum = install::install_release(
            id,
            &release,
            &github,
            &self.paths.tools_folder(),
            &operation.cancel,
            &|stage| operation.report(stage),
        )?;

        operation.report(ToolStage::Checking);
        let executable = locate::managed_executable(&self.paths.tools_folder(), id);
        let version = read_version(id, &executable).map_err(missing_means_blocked)?;
        let setup = ToolSetup::Managed {
            checksum: Some(checksum),
        };
        self.settings.update(|settings| {
            settings.tools.set(id, Some(setup.clone()));
            if id == ToolId::YtDlp {
                // Just downloaded, so already up to date.
                settings.tools.yt_dlp_last_update_check = Some(seconds_since_epoch());
            }
        })?;
        Ok(tool_status(id, &setup, &executable, Ok(version)))
    }

    /// Sets up a copy already on this PC, such as one found at setup or picked in Settings.
    pub fn use_copy(&self, id: ToolId, path: PathBuf) -> Result<ToolStatus, ToolError> {
        let version = read_version(id, &path)?;
        let managed = locate::managed_executable(&self.paths.tools_folder(), id);
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
        let executable = locate::managed_executable(&self.paths.tools_folder(), ToolId::YtDlp);
        let command = ProgramCommand::new(&executable).args([
            "--ignore-config",
            "--update-to",
            tools.yt_dlp_channel.yt_dlp_update_target(),
        ]);
        let result =
            process::run_collecting_with_cancel(&command, &operation.cancel, UPDATE_TIMEOUT);
        // Recorded even when it failed, so a failing update isn't retried at every start.
        let _ = self.settings.update(|settings| {
            settings.tools.yt_dlp_last_update_check = Some(seconds_since_epoch())
        });

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

    /// Whether the app's copy of yt-dlp is due its startup update check.
    pub fn yt_dlp_update_due(&self) -> bool {
        let tools = self.settings.get().tools;
        matches!(tools.yt_dlp, Some(ToolSetup::Managed { .. }))
            && tools.yt_dlp_last_update_check.is_none_or(|last_check| {
                seconds_since_epoch().saturating_sub(last_check) >= UPDATE_CHECK_INTERVAL.as_secs()
            })
    }

    /// Asks GitHub whether a newer build than the app's copy exists. For ffmpeg and Deno, whose updates are offered,
    /// never automatic.
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

    fn executable_for(&self, id: ToolId, setup: &ToolSetup) -> PathBuf {
        match setup {
            ToolSetup::Managed { .. } => locate::managed_executable(&self.paths.tools_folder(), id),
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
        let release = github.latest_release(spec(id).release.repository)?;
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

/// Runs `work` for each tool at once, since each mostly waits on a program or the network, and returns the results
/// in the tools' order.
fn for_each_tool<T: Send>(work: impl Fn(ToolId) -> T + Sync) -> Vec<T> {
    thread::scope(|scope| {
        let work = &work;
        let handles = ToolId::ALL.map(|id| scope.spawn(move || work(id)));
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
            manager.check_for_update(ToolId::Deno),
            Err(ToolError::NotManaged)
        );
        assert!(!manager.yt_dlp_update_due());
    }

    #[test]
    fn checks_yt_dlp_for_updates_once_a_day() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        let set_last_check = |last_check| {
            manager
                .settings
                .update(|settings| {
                    settings
                        .tools
                        .set(ToolId::YtDlp, Some(ToolSetup::Managed { checksum: None }));
                    settings.tools.yt_dlp_last_update_check = last_check;
                })
                .unwrap();
        };

        set_last_check(None);
        assert!(manager.yt_dlp_update_due());
        set_last_check(Some(seconds_since_epoch() - 60));
        assert!(!manager.yt_dlp_update_due());
        set_last_check(Some(
            seconds_since_epoch() - UPDATE_CHECK_INTERVAL.as_secs(),
        ));
        assert!(manager.yt_dlp_update_due());
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
