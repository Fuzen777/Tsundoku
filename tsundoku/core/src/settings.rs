//! `settings.json` in the data folder: what the user chose, kept across launches.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::atomic_file::write_atomically;
use crate::presets::PresetsSettings;
use crate::sync::lock;
use crate::tools::ToolId;

/// Everything stored in `settings.json`. Missing fields take their defaults and unknown ones are ignored, so a file
/// from an older or newer version still opens.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub tools: ToolsSettings,
    pub presets: PresetsSettings,
    pub downloads: DownloadSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DownloadSettings {
    /// How many downloads run at the same time.
    pub at_once: u32,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self { at_once: 2 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolsSettings {
    pub yt_dlp: Option<ToolSetup>,
    pub ffmpeg: Option<ToolSetup>,
    pub deno: Option<ToolSetup>,
    pub yt_dlp_channel: UpdateChannel,
    /// When yt-dlp was last checked for updates, in seconds since 1970, so the startup check runs at most once a day.
    pub yt_dlp_last_update_check: Option<u64>,
}

impl ToolsSettings {
    pub fn get(&self, id: ToolId) -> Option<&ToolSetup> {
        match id {
            ToolId::YtDlp => self.yt_dlp.as_ref(),
            ToolId::Ffmpeg => self.ffmpeg.as_ref(),
            ToolId::Deno => self.deno.as_ref(),
        }
    }

    pub fn set(&mut self, id: ToolId, setup: Option<ToolSetup>) {
        match id {
            ToolId::YtDlp => self.yt_dlp = setup,
            ToolId::Ffmpeg => self.ffmpeg = setup,
            ToolId::Deno => self.deno = setup,
        }
    }
}

/// Which copy of a tool the app runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "camelCase")]
pub enum ToolSetup {
    /// The app's own copy, in the data folder's `tools\`, which the app keeps up to date.
    Managed {
        /// The SHA-256 of the release file it came from, to tell whether a newer build exists. Unknown for a copy
        /// that was already in `tools\` when it was set up.
        #[serde(default)]
        checksum: Option<String>,
    },
    /// A copy the user picked, such as one installed by winget. The app never updates it.
    External { path: PathBuf },
}

/// Which yt-dlp releases the app's copy follows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UpdateChannel {
    #[default]
    Stable,
    /// Built every day, so site fixes arrive sooner.
    Nightly,
}

impl UpdateChannel {
    /// The target for `yt-dlp --update-to`: the channel's latest release. Naming the release (`@latest`) rather than
    /// just the channel lets yt-dlp go back from a nightly build to an older stable one, which it otherwise refuses
    /// as a downgrade.
    pub fn yt_dlp_update_target(self) -> &'static str {
        match self {
            Self::Stable => "stable@latest",
            Self::Nightly => "nightly@latest",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("couldn't read {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("couldn't save {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
}

/// The settings in memory, saved to `settings.json` after every change.
#[derive(Debug)]
pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    /// Opens the settings file, or starts from the defaults when there is none yet. A file that can't be read as
    /// settings is renamed to `settings.invalid.json`, kept for a look, and the app starts from the defaults.
    pub fn open(path: PathBuf) -> Result<Self, SettingsError> {
        let settings = match fs::read(&path) {
            Ok(contents) => serde_json::from_slice(&contents).unwrap_or_else(|_| {
                let _ = fs::rename(&path, path.with_extension("invalid.json"));
                Settings::default()
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Settings::default(),
            Err(source) => return Err(SettingsError::Read { path, source }),
        };
        Ok(Self {
            path,
            current: Mutex::new(settings),
        })
    }

    pub fn get(&self) -> Settings {
        lock(&self.current).clone()
    }

    /// Changes the settings and saves them. If saving fails, the settings in memory stay as they were.
    pub fn update<R>(&self, change: impl FnOnce(&mut Settings) -> R) -> Result<R, SettingsError> {
        // The settings stay whole even after a panic here: `change` works on a copy.
        let mut current = lock(&self.current);
        let mut changed = current.clone();
        let result = change(&mut changed);
        let contents =
            serde_json::to_vec_pretty(&changed).expect("settings always serialize to JSON");
        write_atomically(&self.path, &contents).map_err(|source| SettingsError::Write {
            path: self.path.clone(),
            source,
        })?;
        *current = changed;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_in(folder: &tempfile::TempDir) -> SettingsStore {
        SettingsStore::open(folder.path().join("settings.json")).unwrap()
    }

    #[test]
    fn starts_from_the_defaults_without_a_file() {
        let folder = tempfile::tempdir().unwrap();
        assert_eq!(store_in(&folder).get(), Settings::default());
    }

    #[test]
    fn saved_settings_come_back() {
        let folder = tempfile::tempdir().unwrap();
        let store = store_in(&folder);
        store
            .update(|settings| {
                settings.tools.set(
                    ToolId::YtDlp,
                    Some(ToolSetup::Managed {
                        checksum: Some("abc".into()),
                    }),
                );
                settings.tools.set(
                    ToolId::Ffmpeg,
                    Some(ToolSetup::External {
                        path: r"D:\ffmpeg.exe".into(),
                    }),
                );
                settings.tools.yt_dlp_channel = UpdateChannel::Nightly;
            })
            .unwrap();

        let reopened = store_in(&folder).get();
        assert_eq!(reopened, store.get());
        assert_eq!(reopened.tools.yt_dlp_channel, UpdateChannel::Nightly);
        assert_eq!(reopened.tools.get(ToolId::Deno), None);
    }

    #[test]
    fn ignores_unknown_fields_and_fills_missing_ones() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(
            folder.path().join("settings.json"),
            r#"{ "fromTheFuture": 1, "tools": { "deno": { "source": "managed" } } }"#,
        )
        .unwrap();

        let settings = store_in(&folder).get();
        assert_eq!(
            settings.tools.get(ToolId::Deno),
            Some(&ToolSetup::Managed { checksum: None })
        );
        assert_eq!(settings.tools.yt_dlp_channel, UpdateChannel::Stable);
    }

    #[test]
    fn sets_an_unreadable_file_aside() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("settings.json"), "not json").unwrap();

        assert_eq!(store_in(&folder).get(), Settings::default());
        assert_eq!(
            fs::read_to_string(folder.path().join("settings.invalid.json")).unwrap(),
            "not json"
        );
        assert!(!folder.path().join("settings.json").exists());
    }
}
