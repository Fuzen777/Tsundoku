//! `settings.json` in the data folder: what the user chose, kept across launches.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::atomic_file::write_atomically;
use crate::data_folder;
use crate::post_names;
use crate::presets::PresetsSettings;
use crate::rules::{RulesSettings, catch_all_rule, posts_folder_rule, with_catch_all_first};
use crate::sites::{self, PacedSite};
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
    pub rules: RulesSettings,
    /// A choice from a newer version, which this one can't read, only resets the lists' arrangements.
    #[serde(deserialize_with = "default_if_unreadable")]
    pub lists: ListArrangements,
    /// A view from a newer version, which this one can't read, only resets the pickers' layouts.
    #[serde(deserialize_with = "default_if_unreadable")]
    pub pickers: PickerLayouts,
    pub adding: AddingSettings,
    pub background: BackgroundSettings,
    pub appearance: AppearanceSettings,
    /// The version that ran last, so the first start on a newer one can say what changed (`changelog::whats_new`).
    pub last_run_version: Option<String>,
}

impl Settings {
    /// Reads `settings.json`. A file from before folder rules (0.1.10 and older) gets a rule for posts in the posts
    /// folder it chose, if it chose one, below the catch-all. Rules from before 0.1.86, where the first matching rule
    /// decided and the catch-all was last, are reversed, so the last one decides what the first did. The catch-all is
    /// put first, and the posts rule before 0.1.30 becomes it. Pacing from before 0.1.147 is carried over
    /// (`SitePacing::carried_over`). A backup's settings are read the same way.
    pub(crate) fn from_json(contents: &[u8]) -> Option<Self> {
        let mut settings: Self = serde_json::from_slice(contents).ok()?;
        if let Ok(before) = serde_json::from_slice::<BeforePacing>(contents)
            && before.downloads.pacing.is_none()
        {
            settings.downloads.pacing = SitePacing::carried_over(&before.downloads);
        }
        let earlier: BeforeRules = serde_json::from_slice(contents).unwrap_or_default();
        if earlier.rules.is_none()
            && let Some(posts_folder) = earlier
                .downloads
                .posts_folder
                .as_deref()
                .map(str::trim)
                .filter(|folder| !folder.is_empty())
        {
            settings.rules.list = vec![catch_all_rule(), posts_folder_rule(posts_folder)];
            settings.rules.last_wins = true;
        }
        if !settings.rules.last_wins {
            settings.rules.list.reverse();
            settings.rules.last_wins = true;
        }
        settings.rules.list = with_catch_all_first(std::mem::take(&mut settings.rules.list));
        Some(settings)
    }
}

/// What `settings.json` held before folder rules, to carry the posts folder over into a rule.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct BeforeRules {
    rules: Option<serde::de::IgnoredAny>,
    downloads: DownloadsBeforeRules,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct DownloadsBeforeRules {
    posts_folder: Option<String>,
}

/// What `settings.json` held before per-site pacing (0.1.147), to carry the pauses over.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct BeforePacing {
    downloads: DownloadsBeforePacing,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct DownloadsBeforePacing {
    pacing: Option<serde::de::IgnoredAny>,
    /// Instagram's pause.
    post_pause: Option<PauseRange>,
    site_pauses: SitePausesBeforePacing,
}

/// The other paced sites' pauses, each `None` for none.
#[derive(Default, Deserialize)]
#[serde(default)]
struct SitePausesBeforePacing {
    x: Option<PauseRange>,
    pixiv: Option<PauseRange>,
    tumblr: Option<PauseRange>,
}

/// How links get added besides pasting them. Settings → Adding Links edits it; the watcher and the shortcut start
/// off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AddingSettings {
    /// Links copied in any app go to the pending links (`pending::watcher`) while Tsundoku runs.
    pub watch_clipboard: bool,
    /// Copied links start downloading once read, with the default preset, instead of waiting to be started. Links
    /// added with the clipboard shortcut too.
    pub download_right_away: bool,
    /// A shortcut that adds the links on the clipboard from any app, if one is picked.
    pub clipboard_shortcut: Option<ClipboardShortcut>,
    /// Windows notifications while the window isn't in front: for each copied link added or started, for what the
    /// shortcut adds, and for the tray's Start All Pending.
    pub notify: bool,
    /// A Windows notification while the window isn't in front when a download fails; several failing together are one.
    pub notify_failures: bool,
    /// A Windows notification while the window isn't in front when every download started has ended
    /// (`downloads::run`).
    pub notify_finished: bool,
    /// Pages of the sites `click_n_load::SENDERS` lists can send a folder's links to Pending with their Click'n'Load
    /// button, through a receiver on this PC only. Off unless turned on, and in settings saved before it existed.
    pub click_n_load: bool,
}

impl Default for AddingSettings {
    fn default() -> Self {
        Self {
            watch_clipboard: false,
            download_right_away: false,
            clipboard_shortcut: None,
            notify: true,
            notify_failures: true,
            notify_finished: true,
            click_n_load: false,
        }
    }
}

/// The keys of the clipboard shortcut, from a few that Windows and common apps leave free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ClipboardShortcut {
    CtrlAltD,
    CtrlShiftD,
    CtrlAltV,
    AltShiftD,
}

impl ClipboardShortcut {
    /// The keys as Tauri's global shortcuts read them, and as the UI shows them.
    pub fn keys(self) -> &'static str {
        match self {
            Self::CtrlAltD => "Ctrl+Alt+D",
            Self::CtrlShiftD => "Ctrl+Shift+D",
            Self::CtrlAltV => "Ctrl+Alt+V",
            Self::AltShiftD => "Alt+Shift+D",
        }
    }
}

/// What Tsundoku does when its window closes and when Windows starts. Settings → Running in the Background edits it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct BackgroundSettings {
    /// Closing the window hides it and keeps Tsundoku running in the tray, so downloads and the clipboard watcher go
    /// on. Quit in the tray's menu ends it.
    pub keep_in_tray: bool,
    /// Tsundoku starts when the user signs in to Windows: in the tray when `keep_in_tray` is on, else with its window.
    pub start_with_windows: bool,
}

/// How Tsundoku looks. Settings → Appearance edits it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AppearanceSettings {
    /// Light or dark, or whichever Windows uses. The window (its title bar and Mica) and the UI's colors both follow
    /// it.
    pub theme: Theme,
    /// The color of primary buttons, selections, progress bars and links.
    pub accent: Accent,
    /// The color picked for `Accent::Custom`, as `#rrggbb`, kept when another accent is picked. The UI works out its
    /// shades for light and dark.
    pub custom_accent: String,
}

/// Ai-iro's fill, what the custom accent starts as.
pub const DEFAULT_CUSTOM_ACCENT: &str = "#165e83";

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            accent: Accent::default(),
            custom_accent: DEFAULT_CUSTOM_ACCENT.to_owned(),
        }
    }
}

impl AppearanceSettings {
    /// The settings with a custom accent that's a `#rrggbb` color, in lowercase, else the one it starts as.
    pub fn checked(mut self) -> Self {
        let color = self.custom_accent.trim().to_ascii_lowercase();
        let valid = color.len() == 7
            && color.starts_with('#')
            && color[1..].chars().all(|digit| digit.is_ascii_hexdigit());
        self.custom_accent = if valid {
            color
        } else {
            DEFAULT_CUSTOM_ACCENT.to_owned()
        };
        self
    }
}

/// A `#rrggbb` color as Windows' `COLORREF` (`0x00bbggrr`), for its color picker; `None` when it isn't one.
pub fn color_ref_of(color: &str) -> Option<u32> {
    let digits = color.trim().strip_prefix('#')?;
    if digits.len() != 6 || !digits.chars().all(|digit| digit.is_ascii_hexdigit()) {
        return None;
    }
    let red_green_blue = u32::from_str_radix(digits, 16).ok()?;
    let (red, green, blue) = (
        red_green_blue >> 16,
        (red_green_blue >> 8) & 0xff,
        red_green_blue & 0xff,
    );
    Some(blue << 16 | green << 8 | red)
}

/// Windows' `COLORREF` (`0x00bbggrr`) as a `#rrggbb` color, in lowercase.
pub fn color_of_color_ref(color_ref: u32) -> String {
    let (red, green, blue) = (
        color_ref & 0xff,
        (color_ref >> 8) & 0xff,
        (color_ref >> 16) & 0xff,
    );
    format!("#{red:02x}{green:02x}{blue:02x}")
}

/// Settings → Appearance's accent colors, each with a shade for light and one for dark (`index.css`, `data-accent`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Accent {
    /// Ai-iro, Japanese indigo: the one Tsundoku ships with.
    #[default]
    Indigo,
    Teal,
    Green,
    Amber,
    Rose,
    Purple,
    /// The color in `AppearanceSettings::custom_accent`.
    Custom,
}

/// Whether Tsundoku is light or dark.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Theme {
    /// Whichever Windows uses for apps, following it when it changes.
    #[default]
    Auto,
    Light,
    Dark,
}

/// How downloads run. Settings → Downloads edits it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct DownloadSettings {
    /// Where downloads go when no rule matches and their preset has no folder of its own, or `None` for a Tsundoku
    /// folder in Downloads (`default_download_folder`). A rule's folder that isn't a full path is inside it.
    pub folder: Option<String>,
    /// How many downloads run at the same time.
    pub at_once: u32,
    /// yt-dlp's `--limit-rate`, such as "2M", or `None` for no limit.
    pub rate_limit: Option<String>,
    /// Where yt-dlp and gallery-dl read cookies from, for links that need a signed-in account: `Automatic` finds
    /// the browser profile signed in to the link's site (`browsers::cookies_for`), a browser its profile, `None`
    /// none at all.
    pub cookies_browser: Option<CookiesBrowser>,
    /// How the files of a post are named, with placeholders such as `{account}` and `{number}` (`post_names`).
    /// The extension is added to it.
    pub post_file_name: String,
    /// For each site that limits how fast it's read (`PacedSite`), how many of its downloads run at once and the pause
    /// before each file of its posts, so the account isn't blocked (Settings → Pacing). Missing in settings saved
    /// before 0.1.147, which `Settings::from_json` carries `postPause` and `sitePauses` over from.
    pub pacing: SitePacing,
    /// The most files from that site (Instagram) downloaded in an hour, or `None` for no limit: once the past hour
    /// holds this many, its next download waits (`site_limits::hourly_limit_end`).
    pub hourly_file_limit: Option<u32>,
    /// Unpacking the archives downloads bring (Settings → Archives).
    pub unpack: UnpackSettings,
    /// Save Past Passwords (Settings → Passwords): a password that opens a link or an archive is kept, apart from the
    /// list (`UnpackSettings::passwords`), and tried by itself after it (`passwords::known`). Off unless turned on, and
    /// in settings saved before 0.2.6.
    pub save_past_passwords: bool,
}

/// Unpacking the zip, 7z and rar archives downloads bring, beside them (`unpack`). Settings → Archives edits it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct UnpackSettings {
    pub enabled: bool,
    /// Where what an archive holds goes. Missing in settings saved before 0.1.136, which unpacked as `Smart` does.
    pub place: UnpackPlace,
    /// What happens to an archive once unpacked.
    pub after: AfterUnpacking,
    /// Settings → Passwords' list, tried in order on an archive or a link that needs a password, before the ones used
    /// before (`passwords::known`): no blank one, none twice. Kept here, where it was while it was only for archives.
    pub passwords: Vec<String>,
}

impl Default for UnpackSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            place: UnpackPlace::Smart,
            after: AfterUnpacking::RecycleBin,
            passwords: Vec::new(),
        }
    }
}

impl UnpackSettings {
    /// The passwords as they're tried: trimmed of the line breaks around them (not of spaces, which a password may
    /// hold), without blank ones or one twice.
    pub fn normalized_passwords(passwords: Vec<String>) -> Vec<String> {
        let mut kept: Vec<String> = Vec::new();
        for password in passwords {
            let password = password.trim_matches(['\r', '\n']).to_owned();
            if !password.trim().is_empty() && !kept.contains(&password) {
                kept.push(password);
            }
        }
        kept
    }
}

/// Where what an archive holds goes, in the archive's folder (`unpack::files::put_beside`). A name already taken
/// gets " (2)", " (3)"…
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UnpackPlace {
    /// Its one file or folder as it is, several in a folder named after the archive: never a folder in a folder, never
    /// a pile of loose files.
    #[default]
    Smart,
    /// Always in a folder named after the archive, even its one file.
    ArchiveFolder,
    /// Always as it is, beside the archive: several files stay loose.
    InPlace,
}

/// What happens to an archive's parts once what it holds is beside them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AfterUnpacking {
    Keep,
    /// Where they can be put back from.
    RecycleBin,
    Delete,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self {
            folder: None,
            at_once: 2,
            rate_limit: None,
            cookies_browser: Some(CookiesBrowser::Automatic),
            post_file_name: post_names::DEFAULT_POST_FILE_NAME.to_owned(),
            pacing: SitePacing::default(),
            hourly_file_limit: None,
            unpack: UnpackSettings::default(),
            save_past_passwords: false,
        }
    }
}

impl DownloadSettings {
    /// The highest hourly limit on a paced site's files that can be set.
    pub const HIGHEST_HOURLY_LIMIT: u32 = 10_000;

    /// An hourly limit within 1 to `HIGHEST_HOURLY_LIMIT`, or none for 0.
    pub fn normalized_hourly_limit(limit: Option<u32>) -> Option<u32> {
        limit
            .filter(|limit| *limit > 0)
            .map(|limit| limit.min(Self::HIGHEST_HOURLY_LIMIT))
    }

    /// The pause before each file of a post at `link`, and between its site's links in Pending: its site's
    /// (`pacing`), none elsewhere or when the site's is 0 seconds.
    pub fn pause_for(&self, link: &str) -> Option<PauseRange> {
        let site = sites::paced_site(link)?;
        Some(self.pacing.of(site).pause).filter(|pause| pause.max > 0)
    }

    /// The download folder: the chosen one, or a Tsundoku folder in Downloads. A portable copy's folder that isn't a
    /// full path (`BESIDE_APP_DOWNLOADS`) is inside the copy's own folder.
    pub fn resolved_folder(&self) -> PathBuf {
        resolve_download_folder(self.folder.as_deref(), data_folder::portable_folder())
    }
}

/// The download folder a portable copy keeps beside itself, chosen at the first-run setup. Saved without the copy's
/// own folder, so it follows the copy to another drive.
pub const BESIDE_APP_DOWNLOADS: &str = "Downloads";

fn resolve_download_folder(folder: Option<&str>, portable_folder: Option<&Path>) -> PathBuf {
    let Some(folder) = folder.map(str::trim).filter(|folder| !folder.is_empty()) else {
        return default_download_folder();
    };
    match portable_folder {
        Some(portable_folder) if Path::new(folder).is_relative() => portable_folder.join(folder),
        _ => PathBuf::from(folder),
    }
}

/// Where downloads go when no folder is chosen: a Tsundoku folder in Downloads, or in the user's folder if Windows
/// has no Downloads folder.
pub fn default_download_folder() -> PathBuf {
    dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_default()
        .join("Tsundoku")
}

/// How fast each site that limits how fast it's read (`PacedSite`) is read: one download at a time each to start
/// with, Instagram's files 3 to 5 seconds apart, the others' 0 to 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct SitePacing {
    pub instagram: SitePace,
    pub x: SitePace,
    pub pixiv: SitePace,
    pub tumblr: SitePace,
}

impl Default for SitePacing {
    fn default() -> Self {
        Self {
            instagram: SitePace {
                at_once: 1,
                pause: PauseRange::INSTAGRAM,
            },
            x: SitePace::default(),
            pixiv: SitePace::default(),
            tumblr: SitePace::default(),
        }
    }
}

impl SitePacing {
    /// The site's pace.
    pub fn of(&self, site: PacedSite) -> SitePace {
        match site {
            PacedSite::Instagram => self.instagram,
            PacedSite::X => self.x,
            PacedSite::Pixiv => self.pixiv,
            PacedSite::Tumblr => self.tumblr,
        }
    }

    /// Each site's downloads at once within 1 to `most_at_once`, and its pause within `PauseRange`'s bounds.
    pub fn normalized(self, most_at_once: u32) -> Self {
        let normalized = |pace: SitePace| SitePace {
            at_once: pace.at_once.clamp(1, most_at_once),
            pause: pace.pause.normalized(),
        };
        Self {
            instagram: normalized(self.instagram),
            x: normalized(self.x),
            pixiv: normalized(self.pixiv),
            tumblr: normalized(self.tumblr),
        }
    }

    /// The pacing in settings saved before 0.1.147: Instagram's pause (`postPause`), unless it was still the 5 to 10
    /// seconds it started with, then takes the new one to start with, as does a site that had no pause
    /// (`sitePauses`). Each site starts with one download at a time.
    fn carried_over(earlier: &DownloadsBeforePacing) -> Self {
        let defaults = Self::default();
        let with_pause = |pause: Option<PauseRange>, pace: SitePace| SitePace {
            pause: pause.unwrap_or(pace.pause),
            ..pace
        };
        Self {
            instagram: with_pause(
                earlier
                    .post_pause
                    .filter(|pause| *pause != PauseRange::INSTAGRAM_BEFORE),
                defaults.instagram,
            ),
            x: with_pause(earlier.site_pauses.x, defaults.x),
            pixiv: with_pause(earlier.site_pauses.pixiv, defaults.pixiv),
            tumblr: with_pause(earlier.site_pauses.tumblr, defaults.tumblr),
        }
    }
}

/// How fast one site is read: how many of its downloads run at once, each of them pausing before each of its files
/// on its own, so two at once read twice as fast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct SitePace {
    /// 1 to `downloads::MAX_AT_ONCE`; Downloads at Once (`DownloadSettings::at_once`) still bounds every download.
    pub at_once: u32,
    /// Before each file of each of its posts, and between its links read in Pending; 0 to 0 for none.
    pub pause: PauseRange,
}

impl Default for SitePace {
    fn default() -> Self {
        Self {
            at_once: 1,
            pause: PauseRange::SHORT,
        }
    }
}

/// A random wait between `min` and `max` seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct PauseRange {
    pub min: u32,
    pub max: u32,
}

impl PauseRange {
    /// The longest pause that can be set, in seconds.
    pub const LONGEST: u32 = 600;

    /// Instagram's to start with: one file every 3 to 5 seconds.
    pub const INSTAGRAM: Self = Self { min: 3, max: 5 };

    /// The other paced sites' to start with.
    pub const SHORT: Self = Self { min: 0, max: 2 };

    /// Instagram's to start with before 0.1.147.
    const INSTAGRAM_BEFORE: Self = Self { min: 5, max: 10 };

    /// Within 0 to `LONGEST`, with `min` no more than `max`.
    pub fn normalized(self) -> Self {
        let max = self.max.min(Self::LONGEST);
        Self {
            min: self.min.min(max),
            max,
        }
    }

    /// As gallery-dl's `--sleep` takes it: "10-20", or "10" when both ends are the same.
    pub fn to_arg(self) -> String {
        if self.min == self.max {
            self.min.to_string()
        } else {
            format!("{}-{}", self.min, self.max)
        }
    }
}

impl Default for PauseRange {
    fn default() -> Self {
        Self::SHORT
    }
}

/// The browsers cookies can be read from on Windows, or `Automatic` for whichever is signed in to the link's site.
/// Chrome, Edge and other Chromium browsers encrypt their cookies with a key only they can use (app-bound
/// encryption) and lock them while running, so they often can't be read; Firefox and its forks work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CookiesBrowser {
    Automatic,
    Firefox,
    Zen,
    LibreWolf,
    Floorp,
    Waterfox,
    Chrome,
    Edge,
    Brave,
    Opera,
    Vivaldi,
    Chromium,
}

impl CookiesBrowser {
    /// The browser family's name for `--cookies-from-browser`: Firefox's forks read as Firefox, given their profile
    /// folder. `Automatic` never reaches a tool; it resolves to a profile first.
    pub fn yt_dlp_name(self) -> &'static str {
        match self {
            Self::Automatic
            | Self::Firefox
            | Self::Zen
            | Self::LibreWolf
            | Self::Floorp
            | Self::Waterfox => "firefox",
            Self::Chrome => "chrome",
            Self::Edge => "edge",
            Self::Brave => "brave",
            Self::Opera => "opera",
            Self::Vivaldi => "vivaldi",
            Self::Chromium => "chromium",
        }
    }

    /// Whether the browser keeps Firefox's cookie database (`cookies.sqlite`).
    pub fn is_firefox_family(self) -> bool {
        matches!(
            self,
            Self::Firefox | Self::Zen | Self::LibreWolf | Self::Floorp | Self::Waterfox
        )
    }

    /// Whether yt-dlp finds the browser's profile by name alone. Firefox's forks need their profile folder.
    pub fn known_to_yt_dlp(self) -> bool {
        !matches!(
            self,
            Self::Automatic | Self::Zen | Self::LibreWolf | Self::Floorp | Self::Waterfox
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolsSettings {
    pub yt_dlp: Option<ToolSetup>,
    pub ffmpeg: Option<ToolSetup>,
    pub deno: Option<ToolSetup>,
    pub gallery_dl: Option<ToolSetup>,
    pub yt_dlp_channel: UpdateChannel,
    /// When yt-dlp was last checked for updates, in seconds since 1970, so its update runs at most once a day.
    pub yt_dlp_last_update_check: Option<u64>,
    /// The same for gallery-dl.
    pub gallery_dl_last_update_check: Option<u64>,
    /// Where the app's own copies go, picked at the first-run setup, or `None` for the data folder's `tools\`.
    pub folder: Option<PathBuf>,
    /// Where yt-dlp, gallery-dl and Deno keep their caches. `None` until the first-run setup asks; an install set up
    /// before it asked (0.1.19 and older) leaves them in their usual folders, as before.
    pub caches: Option<ToolCaches>,
}

/// Where yt-dlp, gallery-dl and Deno keep their caches (sign-ins, sites' player code, downloaded scripts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ToolCaches {
    /// In the data folder's `cache\`, so they stay with Tsundoku and go when it's removed.
    DataFolder,
    /// Where each tool keeps them on its own, in the user's folder.
    UsualFolders,
}

impl ToolsSettings {
    pub fn get(&self, id: ToolId) -> Option<&ToolSetup> {
        match id {
            ToolId::YtDlp => self.yt_dlp.as_ref(),
            ToolId::Ffmpeg => self.ffmpeg.as_ref(),
            ToolId::Deno => self.deno.as_ref(),
            ToolId::GalleryDl => self.gallery_dl.as_ref(),
        }
    }

    pub fn set(&mut self, id: ToolId, setup: Option<ToolSetup>) {
        match id {
            ToolId::YtDlp => self.yt_dlp = setup,
            ToolId::Ffmpeg => self.ffmpeg = setup,
            ToolId::Deno => self.deno = setup,
            ToolId::GalleryDl => self.gallery_dl = setup,
        }
    }

    /// When a tool that updates itself once a day (`ToolId::UPDATED_DAILY`) was last checked, or `None` for the others.
    pub fn last_update_check(&self, id: ToolId) -> Option<u64> {
        match id {
            ToolId::YtDlp => self.yt_dlp_last_update_check,
            ToolId::GalleryDl => self.gallery_dl_last_update_check,
            ToolId::Ffmpeg | ToolId::Deno => None,
        }
    }

    /// Records when a tool that updates itself once a day was checked, in seconds since 1970. The others keep no record.
    pub fn set_last_update_check(&mut self, id: ToolId, seconds: u64) {
        match id {
            ToolId::YtDlp => self.yt_dlp_last_update_check = Some(seconds),
            ToolId::GalleryDl => self.gallery_dl_last_update_check = Some(seconds),
            ToolId::Ffmpeg | ToolId::Deno => {}
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

/// Reads a part of the settings, or takes its defaults when it can't be read, rather than setting the whole file aside.
fn default_if_unreadable<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(T::deserialize(value).unwrap_or_default())
}

/// How the Pending, Queue and Watched views group and sort their lists, each chosen in its own toolbar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct ListArrangements {
    pub pending: ListArrangement,
    pub queue: ListArrangement,
    pub watched: WatchedArrangement,
}

/// How one list is grouped and sorted. By default, ungrouped and oldest first, as the list is stored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListArrangement {
    pub group: ListGroup,
    /// Grouped by where each item stands first (waiting, running, done… in the queue; read, downloaded before,
    /// failed… in Pending), then by `group` within each.
    pub by_status: bool,
    pub sort: ListSort,
    /// Newest or Z first.
    pub descending: bool,
}

/// What a list's items are grouped by, besides where they stand (`ListArrangement::by_status`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ListGroup {
    #[default]
    None,
    /// The folder the download goes to.
    Destination,
    Site,
    /// The folder rule that picks the folder, or the preset's folder, or the download folder.
    Rule,
}

/// A `ListArrangement` as `settings.json` may hold it: before 0.1.69, grouping by status was one of the groups.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct StoredListArrangement {
    group: StoredListGroup,
    by_status: bool,
    sort: ListSort,
    descending: bool,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
enum StoredListGroup {
    #[default]
    None,
    Destination,
    Site,
    Rule,
    Status,
}

impl<'de> Deserialize<'de> for ListArrangement {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        StoredListArrangement::deserialize(deserializer).map(Self::from)
    }
}

impl From<StoredListArrangement> for ListArrangement {
    fn from(stored: StoredListArrangement) -> Self {
        let group = match stored.group {
            StoredListGroup::None | StoredListGroup::Status => ListGroup::None,
            StoredListGroup::Destination => ListGroup::Destination,
            StoredListGroup::Site => ListGroup::Site,
            StoredListGroup::Rule => ListGroup::Rule,
        };
        Self {
            group,
            by_status: stored.by_status || matches!(stored.group, StoredListGroup::Status),
            sort: stored.sort,
            descending: stored.descending,
        }
    }
}

/// How the Watched view shows its sources. By default, a card each, grouped by site, the ones with the most new items
/// first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct WatchedArrangement {
    pub show: WatchedShow,
    pub group: WatchedGroup,
    pub sort: WatchedSort,
    /// The most, the latest or Z first.
    pub descending: bool,
}

impl Default for WatchedArrangement {
    fn default() -> Self {
        Self {
            show: WatchedShow::Sources,
            group: WatchedGroup::Site,
            sort: WatchedSort::MostNew,
            descending: true,
        }
    }
}

/// What the Watched view lists.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WatchedShow {
    /// A card per source, with its new items.
    #[default]
    Sources,
    /// What's new on every source, in one picker.
    AllNew,
}

/// What the watched sources are grouped by, under the failed checks' group, which always comes first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WatchedGroup {
    None,
    #[default]
    Site,
}

/// What the watched sources are sorted by, within their groups.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WatchedSort {
    /// How many new items it has.
    #[default]
    MostNew,
    Name,
    LastChecked,
    /// When its newest item was posted.
    LatestPost,
    /// When it was watched.
    Added,
}

/// How the pickers show what they offer, each chosen with the picker's own toggle and size buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct PickerLayouts {
    /// A channel's, playlist's or account's items, on the preview card and under a watched source.
    pub items: PickerLayout,
    /// A post's pictures and videos, on the preview card and in Pending.
    pub post_files: PickerLayout,
    /// The downloads picked from a channel, playlist or account, under its heading in the Queue (the same choice for
    /// every heading).
    pub queue_sources: PickerLayout,
}

impl Default for PickerLayouts {
    fn default() -> Self {
        Self {
            items: PickerLayout::default(),
            post_files: PickerLayout {
                view: PickerView::Gallery,
                size: 1,
                hide_downloaded: false,
            },
            queue_sources: PickerLayout::default(),
        }
    }
}

/// One picker's view, and how big its pictures are in Masonry and Gallery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct PickerLayout {
    pub view: PickerView,
    /// A step of the UI's sizes, from 0, the smallest. The UI keeps it within its steps.
    pub size: u8,
    /// What the history or the queue has is hidden: the listing picker's Hide Downloaded.
    pub hide_downloaded: bool,
}

impl Default for PickerLayout {
    fn default() -> Self {
        Self {
            view: PickerView::Gallery,
            size: 2,
            hide_downloaded: false,
        }
    }
}

/// How a picker lays out what it offers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PickerView {
    /// A row each: the picture, the title and its details.
    List,
    /// Pictures as they are, in columns.
    Masonry,
    /// Pictures cut to the same shape, in a grid.
    #[default]
    Gallery,
}

/// What a list's items are sorted by, within their groups.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ListSort {
    /// When it was added.
    #[default]
    Added,
    Title,
    Uploader,
    Site,
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
            Ok(contents) => Settings::from_json(&contents).unwrap_or_else(|| {
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
    use crate::rules::FolderRule;

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
    fn keeps_a_custom_accent_only_when_it_is_a_color() {
        let with = |color: &str| {
            AppearanceSettings {
                custom_accent: color.to_owned(),
                ..AppearanceSettings::default()
            }
            .checked()
            .custom_accent
        };
        assert_eq!(with(" #A1B2C3 "), "#a1b2c3");
        assert_eq!(with("#12345"), DEFAULT_CUSTOM_ACCENT);
        assert_eq!(with("red"), DEFAULT_CUSTOM_ACCENT);
        assert_eq!(with("#12345g"), DEFAULT_CUSTOM_ACCENT);
    }

    #[test]
    fn turns_colors_into_windows_color_refs_and_back() {
        assert_eq!(color_ref_of("#165E83"), Some(0x00835e16));
        assert_eq!(color_of_color_ref(0x00835e16), "#165e83");
        assert_eq!(
            color_of_color_ref(color_ref_of("#000000").unwrap()),
            "#000000"
        );
        assert_eq!(color_ref_of("#12345"), None);
        assert_eq!(color_ref_of("165e83"), None);
    }

    #[test]
    fn reads_list_arrangements() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(
            folder.path().join("settings.json"),
            r#"{ "lists": {
                "pending": { "group": "destination", "sort": "title", "descending": true },
                "queue": { "group": "rule", "byStatus": true }
            } }"#,
        )
        .unwrap();

        let lists = store_in(&folder).get().lists;
        assert_eq!(
            lists.pending,
            ListArrangement {
                group: ListGroup::Destination,
                by_status: false,
                sort: ListSort::Title,
                descending: true,
            }
        );
        assert_eq!(
            lists.queue,
            ListArrangement {
                group: ListGroup::Rule,
                by_status: true,
                ..ListArrangement::default()
            }
        );
        assert_eq!(
            lists.watched,
            WatchedArrangement::default(),
            "a file from before Watched had its arrangement"
        );
    }

    #[test]
    fn reads_the_watched_arrangement() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(
            folder.path().join("settings.json"),
            r#"{ "lists": { "watched": { "show": "allNew", "group": "none", "sort": "latestPost" } } }"#,
        )
        .unwrap();
        assert_eq!(
            store_in(&folder).get().lists.watched,
            WatchedArrangement {
                show: WatchedShow::AllNew,
                group: WatchedGroup::None,
                sort: WatchedSort::LatestPost,
                descending: true,
            }
        );

        fs::write(
            folder.path().join("settings.json"),
            r#"{ "lists": { "queue": { "group": "rule" }, "watched": { "sort": "fromANewerVersion" } } }"#,
        )
        .unwrap();
        let lists = store_in(&folder).get().lists;
        assert_eq!(
            lists,
            ListArrangements::default(),
            "a sort from a newer version resets the arrangements"
        );
    }

    #[test]
    fn reads_picker_layouts() {
        let folder = tempfile::tempdir().unwrap();
        assert_eq!(
            store_in(&folder).get().pickers,
            PickerLayouts {
                items: PickerLayout {
                    view: PickerView::Gallery,
                    size: 2,
                    hide_downloaded: false,
                },
                post_files: PickerLayout {
                    view: PickerView::Gallery,
                    size: 1,
                    hide_downloaded: false,
                },
                queue_sources: PickerLayout {
                    view: PickerView::Gallery,
                    size: 2,
                    hide_downloaded: false,
                },
            }
        );

        fs::write(
            folder.path().join("settings.json"),
            r#"{ "pickers": { "items": { "view": "masonry", "size": 4, "hideDownloaded": true }, "postFiles": { "view": "list" } } }"#,
        )
        .unwrap();
        let pickers = store_in(&folder).get().pickers;
        assert_eq!(
            pickers.items,
            PickerLayout {
                view: PickerView::Masonry,
                size: 4,
                hide_downloaded: true,
            }
        );
        assert_eq!(pickers.post_files.view, PickerView::List);
        assert!(
            !pickers.post_files.hide_downloaded,
            "settings from before Hide Downloaded show everything"
        );
        // From before the Queue had views: its own starts as Gallery.
        assert_eq!(pickers.queue_sources, PickerLayout::default());
    }

    #[test]
    fn a_picker_view_from_a_newer_version_resets_only_the_pickers() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(
            folder.path().join("settings.json"),
            r#"{
                "pickers": { "items": { "view": "fromTheFuture" } },
                "adding": { "watchClipboard": true }
            }"#,
        )
        .unwrap();

        let settings = store_in(&folder).get();
        assert_eq!(settings.pickers, PickerLayouts::default());
        assert!(settings.adding.watch_clipboard);
        assert!(!folder.path().join("settings.invalid.json").exists());
    }

    #[test]
    fn saved_picker_layouts_come_back() {
        let folder = tempfile::tempdir().unwrap();
        let layouts = PickerLayouts {
            items: PickerLayout {
                view: PickerView::List,
                size: 0,
                hide_downloaded: false,
            },
            post_files: PickerLayout {
                view: PickerView::Masonry,
                size: 5,
                hide_downloaded: false,
            },
            queue_sources: PickerLayout {
                view: PickerView::List,
                size: 3,
                hide_downloaded: false,
            },
        };
        store_in(&folder)
            .update(|settings| settings.pickers = layouts)
            .unwrap();
        assert_eq!(store_in(&folder).get().pickers, layouts);
    }

    #[test]
    fn grouping_by_status_from_before_0_1_69_becomes_its_own_switch() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(
            folder.path().join("settings.json"),
            r#"{ "lists": { "queue": { "group": "status", "sort": "site" } } }"#,
        )
        .unwrap();
        let queue = store_in(&folder).get().lists.queue;
        assert_eq!(
            queue,
            ListArrangement {
                group: ListGroup::None,
                by_status: true,
                sort: ListSort::Site,
                descending: false,
            }
        );
    }

    #[test]
    fn a_list_arrangement_from_a_newer_version_resets_only_the_arrangements() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(
            folder.path().join("settings.json"),
            r#"{
                "lists": { "queue": { "group": "fromTheFuture" } },
                "adding": { "watchClipboard": true }
            }"#,
        )
        .unwrap();

        let settings = store_in(&folder).get();
        assert_eq!(settings.lists, ListArrangements::default());
        assert!(settings.adding.watch_clipboard);
        assert!(!folder.path().join("settings.invalid.json").exists());
    }

    #[test]
    fn reads_adding_links_saved_before_the_shortcut_and_notifications() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(
            folder.path().join("settings.json"),
            r#"{ "adding": { "watchClipboard": true, "downloadRightAway": true } }"#,
        )
        .unwrap();

        let settings = store_in(&folder).get();
        assert_eq!(
            settings.adding,
            AddingSettings {
                watch_clipboard: true,
                download_right_away: true,
                clipboard_shortcut: None,
                notify: true,
                notify_failures: true,
                notify_finished: true,
                click_n_load: false,
            },
            "Click'n'Load stays off in settings saved before it"
        );
        assert_eq!(settings.background, BackgroundSettings::default());
    }

    #[test]
    fn carries_pauses_from_before_per_site_pacing_over() {
        let read = |json: &str| Settings::from_json(json.as_bytes()).unwrap().downloads;
        let untouched = read(
            r#"{ "downloads": { "postPause": { "min": 5, "max": 10 },
                "sitePauses": { "x": null, "pixiv": null, "tumblr": null } } }"#,
        );
        assert_eq!(
            untouched.pacing,
            SitePacing::default(),
            "the pause Instagram started with, and no pause, take the new ones to start with"
        );
        assert_eq!(untouched.pacing.instagram.pause, PauseRange::INSTAGRAM);
        assert_eq!(untouched.pacing.x.pause, PauseRange::SHORT);

        let chosen = read(
            r#"{ "downloads": { "postPause": { "min": 10, "max": 20 },
                "sitePauses": { "pixiv": { "min": 4, "max": 8 } } } }"#,
        );
        assert_eq!(
            chosen.pacing.instagram,
            SitePace {
                at_once: 1,
                pause: PauseRange { min: 10, max: 20 },
            }
        );
        assert_eq!(chosen.pacing.pixiv.pause, PauseRange { min: 4, max: 8 });
        assert_eq!(chosen.pacing.tumblr, SitePace::default());

        let saved = serde_json::to_vec(&Settings {
            downloads: DownloadSettings {
                pacing: SitePacing {
                    instagram: SitePace {
                        at_once: 2,
                        pause: PauseRange::INSTAGRAM_BEFORE,
                    },
                    ..SitePacing::default()
                },
                ..DownloadSettings::default()
            },
            ..Settings::default()
        })
        .unwrap();
        let again = Settings::from_json(&saved).unwrap().downloads.pacing;
        assert_eq!(
            again.instagram.pause,
            PauseRange::INSTAGRAM_BEFORE,
            "5 to 10 seconds chosen since is kept"
        );
        assert_eq!(again.instagram.at_once, 2);

        let fresh = read("{}");
        assert_eq!(fresh.pacing, SitePacing::default());
    }

    #[test]
    fn paces_each_site_with_its_own_pause() {
        let mut downloads = DownloadSettings::default();
        assert_eq!(
            downloads.pause_for("https://www.instagram.com/p/DfPQchkG1tg/"),
            Some(PauseRange::INSTAGRAM)
        );
        assert_eq!(
            downloads.pause_for("https://twitter.com/NASA/status/1"),
            Some(PauseRange::SHORT)
        );
        assert_eq!(
            downloads.pause_for("https://www.youtube.com/watch?v=1"),
            None
        );
        downloads.pacing.x.pause = PauseRange { min: 0, max: 0 };
        assert_eq!(
            downloads.pause_for("https://x.com/NASA/status/1"),
            None,
            "0 seconds is no pause"
        );
    }

    #[test]
    fn names_each_clipboard_shortcut_as_saved() {
        let saved = serde_json::to_string(&Some(ClipboardShortcut::CtrlShiftD)).unwrap();
        assert_eq!(saved, r#""ctrlShiftD""#);
        assert_eq!(ClipboardShortcut::AltShiftD.keys(), "Alt+Shift+D");
    }

    #[test]
    fn follows_windows_theme_until_one_is_picked() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(
            folder.path().join("settings.json"),
            r#"{ "background": { "keepInTray": true } }"#,
        )
        .unwrap();
        assert_eq!(store_in(&folder).get().appearance.theme, Theme::Auto);

        let store = store_in(&folder);
        store
            .update(|settings| settings.appearance.theme = Theme::Dark)
            .unwrap();
        assert_eq!(store_in(&folder).get().appearance.theme, Theme::Dark);

        let saved = serde_json::to_string(&Theme::Light).unwrap();
        assert_eq!(saved, r#""light""#);
    }

    #[test]
    fn downloads_go_to_a_tsundoku_folder_unless_one_is_chosen() {
        let mut downloads = DownloadSettings::default();
        assert!(default_download_folder().ends_with("Tsundoku"));
        assert_eq!(downloads.resolved_folder(), default_download_folder());

        downloads.folder = Some(r"D:\Tsundoku".to_owned());
        assert_eq!(downloads.resolved_folder(), PathBuf::from(r"D:\Tsundoku"));

        downloads.folder = Some("  ".to_owned());
        assert_eq!(
            downloads.resolved_folder(),
            default_download_folder(),
            "blank is the default"
        );
    }

    #[test]
    fn a_portable_copys_downloads_beside_it_follow_it() {
        let portable = Path::new(r"E:\Tsundoku");
        assert_eq!(
            resolve_download_folder(Some(BESIDE_APP_DOWNLOADS), Some(portable)),
            PathBuf::from(r"E:\Tsundoku\Downloads")
        );
        assert_eq!(
            resolve_download_folder(Some(r"D:\Videos"), Some(portable)),
            PathBuf::from(r"D:\Videos"),
            "a full path stays as it is"
        );
        assert_eq!(
            resolve_download_folder(None, Some(portable)),
            default_download_folder()
        );
    }

    #[test]
    fn reads_tools_settings_saved_before_the_first_run_choices() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(
            folder.path().join("settings.json"),
            r#"{ "tools": { "ytDlp": { "source": "managed" } } }"#,
        )
        .unwrap();
        let tools = store_in(&folder).get().tools;
        assert_eq!(tools.folder, None);
        assert_eq!(tools.caches, None);

        let saved = serde_json::to_string(&Some(ToolCaches::DataFolder)).unwrap();
        assert_eq!(saved, r#""dataFolder""#);
    }

    #[test]
    fn carries_the_posts_folder_into_a_rule() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(
            folder.path().join("settings.json"),
            r#"{ "downloads": { "postsFolder": "D:\\Posts", "atOnce": 3 } }"#,
        )
        .unwrap();
        let settings = store_in(&folder).get();
        assert_eq!(
            settings.rules.list,
            vec![catch_all_rule(), posts_folder_rule(r"D:\Posts")]
        );
        assert_eq!(settings.downloads.at_once, 3);

        fs::write(
            folder.path().join("settings.json"),
            r#"{ "downloads": { "postsFolder": "D:\\Posts" }, "rules": { "list": [] } }"#,
        )
        .unwrap();
        assert_eq!(
            store_in(&folder).get().rules.list,
            vec![FolderRule {
                enabled: false,
                ..catch_all_rule()
            }],
            "saved rules stay as they are, with the catch-all back but off"
        );

        fs::write(
            folder.path().join("settings.json"),
            r#"{ "rules": { "list": [
                { "id": "posts", "enabled": true, "conditions": [], "folder": "{site}" },
                { "id": "rule-1", "enabled": true, "conditions": [], "folder": "Mine" }
            ] } }"#,
        )
        .unwrap();
        let ids: Vec<String> = store_in(&folder)
            .get()
            .rules
            .list
            .into_iter()
            .map(|rule| rule.id)
            .collect();
        assert_eq!(
            ids,
            ["catch-all", "rule-1"],
            "the posts rule becomes the catch-all, first"
        );

        fs::write(folder.path().join("settings.json"), "{}").unwrap();
        assert_eq!(store_in(&folder).get().rules.list, vec![catch_all_rule()]);
    }

    #[test]
    fn reverses_rules_from_before_the_last_one_won() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(
            folder.path().join("settings.json"),
            r#"{ "rules": { "list": [
                { "id": "rule-1", "enabled": true, "conditions": [], "folder": "First" },
                { "id": "rule-2", "enabled": true, "conditions": [], "folder": "Second" },
                { "id": "catch-all", "enabled": true, "conditions": [], "folder": "{site}" }
            ] } }"#,
        )
        .unwrap();
        let store = store_in(&folder);
        let rules = store.get().rules;
        let ids: Vec<&str> = rules.list.iter().map(|rule| rule.id.as_str()).collect();
        assert_eq!(
            ids,
            ["catch-all", "rule-2", "rule-1"],
            "rule-1 decided first, so it's last"
        );
        assert!(rules.last_wins);

        store.update(|_| {}).unwrap();
        let ids: Vec<String> = store_in(&folder)
            .get()
            .rules
            .list
            .into_iter()
            .map(|rule| rule.id)
            .collect();
        assert_eq!(
            ids,
            ["catch-all", "rule-2", "rule-1"],
            "saved in the new order, so it isn't reversed again"
        );
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
