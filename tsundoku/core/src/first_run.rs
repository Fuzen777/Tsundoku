//! The first-run setup's choices, made before the tools are set up so nothing lands anywhere unasked: where the tools
//! go, where they keep their caches and where downloads go. An install set up before these were asked (0.1.19 and
//! older) isn't asked; it keeps the data folder's `tools\`, the tools' usual cache folders and `Downloads\Tsundoku`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::data_folder::DataPaths;
use crate::settings::{
    BESIDE_APP_DOWNLOADS, Settings, SettingsError, SettingsStore, ToolCaches,
    default_download_folder,
};
use crate::tools;

/// What the first-run choices offer, with the folders each option means on this PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FirstRunOptions {
    /// Whether the choices are still to be made: on a new install, until they're saved.
    pub needed: bool,
    /// The data folder's `tools\`, where the tools go unless another folder is picked.
    pub tools_folder: String,
    /// The data folder's `cache\`, for `ToolCaches::DataFolder`.
    pub cache_folder: String,
    /// `Downloads\Tsundoku`.
    pub default_download_folder: String,
    /// A portable copy's `Downloads` folder beside it, or `None` for an installed copy.
    pub beside_app_folder: Option<String>,
    /// For a portable copy, an installed Tsundoku's `tools\` in AppData, which the two can share; `None` for an
    /// installed copy, whose data folder is there already.
    pub installed_tools_folder: Option<String>,
    /// Whether that folder already holds Tsundoku's copies of the tools, so the setup picks it and uses them.
    pub installed_tools_found: bool,
    /// What was chosen, once saved, so going back from the tools shows it, even after a restart.
    pub chosen: Option<FirstRunChoices>,
}

/// What the user chose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FirstRunChoices {
    /// A folder for the tools, or `None` for the data folder's `tools\`.
    pub tools_folder: Option<String>,
    pub caches: ToolCaches,
    pub download_folder: DownloadFolderChoice,
}

/// Where downloads go.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum DownloadFolderChoice {
    /// `Downloads\Tsundoku`.
    Default,
    /// A portable copy's `Downloads` folder beside it, saved without the copy's own folder so it follows the copy
    /// to another drive. On an installed copy, the default.
    BesideApp,
    Chosen {
        path: String,
    },
}

/// The choices on offer. `portable_folder` is the copy's own folder when it's portable, and `installed_data_folder`
/// an installed copy's data folder in AppData, which a portable copy can share its tools with.
pub fn options(
    settings: &Settings,
    paths: &DataPaths,
    portable_folder: Option<&Path>,
    installed_data_folder: Option<&Path>,
) -> FirstRunOptions {
    let installed_tools_folder = portable_folder
        .and(installed_data_folder)
        .map(|folder| DataPaths::new(folder.to_path_buf()).tools_folder());
    FirstRunOptions {
        needed: needed(settings),
        tools_folder: paths.tools_folder().display().to_string(),
        cache_folder: paths.cache_folder().display().to_string(),
        default_download_folder: default_download_folder().display().to_string(),
        beside_app_folder: portable_folder
            .map(|folder| folder.join(BESIDE_APP_DOWNLOADS).display().to_string()),
        installed_tools_found: installed_tools_folder
            .as_deref()
            .is_some_and(tools::has_own_copies),
        installed_tools_folder: installed_tools_folder.map(|folder| folder.display().to_string()),
        chosen: chosen(settings, portable_folder),
    }
}

/// The choices as saved, or `None` until they are.
fn chosen(settings: &Settings, portable_folder: Option<&Path>) -> Option<FirstRunChoices> {
    let caches = settings.tools.caches?;
    let download_folder = match settings.downloads.folder.as_deref().map(str::trim) {
        None | Some("") => DownloadFolderChoice::Default,
        Some(BESIDE_APP_DOWNLOADS) if portable_folder.is_some() => DownloadFolderChoice::BesideApp,
        Some(path) => DownloadFolderChoice::Chosen {
            path: path.to_owned(),
        },
    };
    Some(FirstRunChoices {
        tools_folder: settings
            .tools
            .folder
            .as_ref()
            .map(|folder| folder.display().to_string()),
        caches,
        download_folder,
    })
}

/// Saves the choices. After this, the setup goes on to the tools.
pub fn save(
    store: &SettingsStore,
    choices: FirstRunChoices,
    portable_folder: Option<&Path>,
) -> Result<(), SettingsError> {
    let tools_folder = choices
        .tools_folder
        .map(|folder| folder.trim().to_owned())
        .filter(|folder| !folder.is_empty())
        .map(PathBuf::from);
    let download_folder = match choices.download_folder {
        DownloadFolderChoice::Default => None,
        DownloadFolderChoice::BesideApp => portable_folder.map(|_| BESIDE_APP_DOWNLOADS.to_owned()),
        DownloadFolderChoice::Chosen { path } => {
            Some(path.trim().to_owned()).filter(|path| !path.is_empty())
        }
    };
    store.update(|settings| {
        settings.tools.folder = tools_folder;
        settings.tools.caches = Some(choices.caches);
        settings.downloads.folder = download_folder;
    })
}

/// A new install asks until the choices are saved. One whose tools are set up already (from before they were asked)
/// never does.
fn needed(settings: &Settings) -> bool {
    let tools = &settings.tools;
    tools.caches.is_none() && (tools.yt_dlp.is_none() || tools.ffmpeg.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ToolSetup;
    use crate::tools::ToolId;

    fn store_in(folder: &tempfile::TempDir) -> SettingsStore {
        SettingsStore::open(folder.path().join("settings.json")).unwrap()
    }

    #[test]
    fn a_new_install_asks_until_the_choices_are_saved() {
        let folder = tempfile::tempdir().unwrap();
        let store = store_in(&folder);
        let paths = DataPaths::new(folder.path().to_path_buf());
        let offered = options(&store.get(), &paths, None, None);
        assert!(offered.needed);
        assert_eq!(offered.beside_app_folder, None);
        assert_eq!(offered.chosen, None);
        assert!(offered.tools_folder.ends_with("tools"));

        save(
            &store,
            FirstRunChoices {
                tools_folder: Some(r"  D:\Tools  ".to_owned()),
                caches: ToolCaches::DataFolder,
                download_folder: DownloadFolderChoice::Chosen {
                    path: r"D:\Videos".to_owned(),
                },
            },
            None,
        )
        .unwrap();
        let settings = store.get();
        let offered = options(&settings, &paths, None, None);
        assert!(!offered.needed);
        assert_eq!(
            offered.chosen,
            Some(FirstRunChoices {
                tools_folder: Some(r"D:\Tools".to_owned()),
                caches: ToolCaches::DataFolder,
                download_folder: DownloadFolderChoice::Chosen {
                    path: r"D:\Videos".to_owned(),
                },
            }),
            "shown again when going back"
        );
        assert_eq!(settings.tools.folder, Some(PathBuf::from(r"D:\Tools")));
        assert_eq!(settings.tools.caches, Some(ToolCaches::DataFolder));
        assert_eq!(settings.downloads.folder.as_deref(), Some(r"D:\Videos"));
    }

    #[test]
    fn an_install_set_up_before_the_choices_isnt_asked() {
        let folder = tempfile::tempdir().unwrap();
        let store = store_in(&folder);
        store
            .update(|settings| {
                settings
                    .tools
                    .set(ToolId::YtDlp, Some(ToolSetup::Managed { checksum: None }));
                settings
                    .tools
                    .set(ToolId::Ffmpeg, Some(ToolSetup::Managed { checksum: None }));
            })
            .unwrap();
        let paths = DataPaths::new(folder.path().to_path_buf());
        assert!(!options(&store.get(), &paths, None, None).needed);
    }

    #[test]
    fn a_portable_copy_can_share_an_installed_copys_tools() {
        let folder = tempfile::tempdir().unwrap();
        let store = store_in(&folder);
        let paths = DataPaths::new(folder.path().join("data"));
        let installed = tempfile::tempdir().unwrap();
        let portable = folder.path();

        let offered = options(&store.get(), &paths, None, Some(installed.path()));
        assert_eq!(
            offered.installed_tools_folder, None,
            "an installed copy's data folder is there already"
        );

        let offered = options(&store.get(), &paths, Some(portable), Some(installed.path()));
        let tools_folder = installed.path().join("tools");
        assert_eq!(
            offered.installed_tools_folder,
            Some(tools_folder.display().to_string())
        );
        assert!(!offered.installed_tools_found);

        std::fs::create_dir_all(tools_folder.join("yt-dlp")).unwrap();
        std::fs::write(tools_folder.join("yt-dlp").join("yt-dlp.exe"), "").unwrap();
        assert!(
            options(&store.get(), &paths, Some(portable), Some(installed.path()))
                .installed_tools_found
        );
    }

    #[test]
    fn downloads_beside_the_app_only_for_a_portable_copy() {
        let folder = tempfile::tempdir().unwrap();
        let store = store_in(&folder);
        let paths = DataPaths::new(folder.path().to_path_buf());
        let portable = Path::new(r"E:\Tsundoku");
        assert_eq!(
            options(&store.get(), &paths, Some(portable), None).beside_app_folder,
            Some(r"E:\Tsundoku\Downloads".to_owned())
        );

        let choices = FirstRunChoices {
            tools_folder: None,
            caches: ToolCaches::UsualFolders,
            download_folder: DownloadFolderChoice::BesideApp,
        };
        save(&store, choices.clone(), Some(portable)).unwrap();
        let settings = store.get();
        assert_eq!(
            settings.downloads.folder.as_deref(),
            Some(BESIDE_APP_DOWNLOADS)
        );
        assert_eq!(settings.tools.folder, None);
        assert_eq!(
            options(&settings, &paths, Some(portable), None).chosen,
            Some(choices.clone())
        );

        save(&store, choices, None).unwrap();
        assert_eq!(
            store.get().downloads.folder,
            None,
            "an installed copy has no folder beside it"
        );
    }
}
