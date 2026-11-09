//! Copy Details on a failed download: what a bug report needs, as plain text, with the user's name blanked out of
//! it, since paths in the error and the log name the user's folder.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use ts_rs::TS;

use crate::tools::{ToolId, ToolsOverview};

/// How many of the log's last lines a report holds.
pub const LOG_LINES: usize = 50;
/// How much of the end of a log is read for its last lines: logs hold every progress line, so they can be long.
const LOG_TAIL_BYTES: u64 = 64 * 1024;
/// What stands in for the user's name.
pub const USER_PLACEHOLDER: &str = "<user>";
/// A name this short is blanked only inside the user folder's path: as a word anywhere, it would blank ordinary words.
const SHORTEST_BLANKED_WORD: usize = 3;

/// An error as the UI shows it: its sentence, then the technical detail under it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShownError {
    pub message: String,
    pub detail: Option<String>,
}

/// What Copy Details puts together for a failed download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadReport {
    pub app_version: String,
    /// Such as "Windows 11 Home 24H2 (build 26100.4061)".
    pub windows: String,
    /// Each tool's name and version, as Settings → Tools reads it, or `None` when it isn't set up or won't run.
    pub tools: Vec<(&'static str, Option<String>)>,
    pub link: String,
    pub error: ShownError,
    /// The end of the download's log, or `None` when it has none.
    pub log: Option<String>,
}

impl DownloadReport {
    /// The report as plain text, with `user`'s name blanked out everywhere.
    pub fn text(&self, user: &UserNames) -> String {
        let mut lines = vec![
            format!("Tsundoku {}", self.app_version),
            self.windows.clone(),
            String::new(),
            "Tools:".to_owned(),
        ];
        for (name, version) in &self.tools {
            lines.push(match version {
                Some(version) => format!("  {name} {version}"),
                None => format!("  {name}: not set up or won't run"),
            });
        }
        lines.push(String::new());
        lines.push(format!("Link: {}", self.link));
        lines.push(format!("Error: {}", self.error.message));
        if let Some(detail) = self
            .error
            .detail
            .as_deref()
            .filter(|detail| !detail.is_empty())
        {
            lines.push(detail.to_owned());
        }
        lines.push(String::new());
        match &self.log {
            Some(log) => {
                lines.push(format!("Log, last {LOG_LINES} lines:"));
                lines.push(log.clone());
            }
            None => lines.push("No log.".to_owned()),
        }
        user.blank(&lines.join("\n"))
    }
}

/// Each tool's name and version from Settings → Tools' overview.
pub fn tool_versions(overview: &ToolsOverview) -> Vec<(&'static str, Option<String>)> {
    overview
        .tools
        .iter()
        .map(|tool| (tool_name(tool.id), tool.version.clone()))
        .collect()
}

fn tool_name(id: ToolId) -> &'static str {
    match id {
        ToolId::YtDlp => "yt-dlp",
        ToolId::Ffmpeg => "ffmpeg",
        ToolId::Deno => "Deno",
        ToolId::GalleryDl => "gallery-dl",
    }
}

/// The last `count` lines of the log at `path`, or `None` if it can't be read. Only its end is read.
pub fn log_tail(path: &Path, count: usize) -> Option<String> {
    read_end(path, LOG_TAIL_BYTES).map(|(text, _)| last_lines(&text, count))
}

/// The end of the text file at `path`, at most `bytes` of it, from the first whole line, and whether there's more
/// before it; `None` if it can't be read.
pub fn read_end(path: &Path, bytes: u64) -> Option<(String, bool)> {
    let mut file = File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    let start = length.saturating_sub(bytes);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut read = Vec::new();
    file.read_to_end(&mut read).ok()?;
    let text = String::from_utf8_lossy(&read);
    // Reading from the middle of the file, the first line is probably cut.
    let text = if start > 0 {
        text.split_once('\n').map_or("", |(_, rest)| rest)
    } else {
        &text
    };
    Some((text.to_owned(), start > 0))
}

/// The last `count` lines, without the empty ones at the end.
fn last_lines(text: &str, count: usize) -> String {
    let lines: Vec<&str> = text.trim_end().lines().map(str::trim_end).collect();
    lines[lines.len().saturating_sub(count)..].join("\n")
}

/// The user's folder and name, blanked out of a report so it can be shared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserNames {
    /// The user's folder, such as C:\Users\Zen.
    pub folder: Option<PathBuf>,
    /// The user folder's name, and the account's name when it differs (a Microsoft account's folder is often cut short).
    pub names: Vec<String>,
}

impl UserNames {
    /// The user running Tsundoku.
    pub fn current() -> Self {
        let folder = dirs::home_dir();
        let folder_name = folder
            .as_deref()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned());
        let account = std::env::var("USERNAME").ok();
        Self::new(folder, [folder_name, account].into_iter().flatten())
    }

    pub fn new(folder: Option<PathBuf>, names: impl IntoIterator<Item = String>) -> Self {
        let mut unique: Vec<String> = Vec::new();
        for name in names {
            let name = name.trim().to_owned();
            if !name.is_empty() && !unique.iter().any(|known| equal_ignoring_case(known, &name)) {
                unique.push(name);
            }
        }
        Self {
            folder,
            names: unique,
        }
    }

    /// `text` with the user's folder written with `<user>` for its name (C:\Users\<user>), however its slashes are
    /// written and in any letter case, and the user's name as a word anywhere else.
    pub fn blank(&self, text: &str) -> String {
        let mut text = text.to_owned();
        if let Some(folder) = self
            .folder
            .as_deref()
            .map(|folder| folder.to_string_lossy())
            && let Some((parent, _)) = folder.rsplit_once(['\\', '/'])
        {
            let blanked = format!("{parent}\\{USER_PLACEHOLDER}");
            let backslashes = folder.replace('/', "\\");
            let blanked_backslashes = blanked.replace('/', "\\");
            for (form, replacement) in [
                (backslashes.clone(), blanked_backslashes.clone()),
                (
                    backslashes.replace('\\', "/"),
                    blanked_backslashes.replace('\\', "/"),
                ),
                // As JSON and Python's messages write it.
                (
                    backslashes.replace('\\', "\\\\"),
                    blanked_backslashes.replace('\\', "\\\\"),
                ),
            ] {
                text = replace_words_ignoring_case(&text, &form, &replacement);
            }
        }
        for name in &self.names {
            if name.chars().count() >= SHORTEST_BLANKED_WORD {
                text = replace_words_ignoring_case(&text, name, USER_PLACEHOLDER);
            }
        }
        text
    }
}

/// `text` with every `needle` that stands as a word of its own (no letter or digit right before or after it),
/// compared ignoring case, replaced by `replacement`.
fn replace_words_ignoring_case(text: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return text.to_owned();
    }
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    let mut previous: Option<char> = None;
    while let Some(character) = rest.chars().next() {
        if let Some(length) = match_length(rest, needle) {
            let next = rest[length..].chars().next();
            if !previous.is_some_and(char::is_alphanumeric)
                && !next.is_some_and(char::is_alphanumeric)
            {
                result.push_str(replacement);
                previous = rest[..length].chars().next_back();
                rest = &rest[length..];
                continue;
            }
        }
        result.push(character);
        previous = Some(character);
        rest = &rest[character.len_utf8()..];
    }
    result
}

/// How many bytes of the start of `text` are `needle`, compared ignoring case, or `None` if it doesn't start with it.
fn match_length(text: &str, needle: &str) -> Option<usize> {
    let mut characters = text.char_indices();
    for wanted in needle.chars() {
        let (_, found) = characters.next()?;
        if !found.to_lowercase().eq(wanted.to_lowercase()) {
            return None;
        }
    }
    Some(characters.next().map_or(text.len(), |(index, _)| index))
}

fn equal_ignoring_case(left: &str, right: &str) -> bool {
    left.chars()
        .flat_map(char::to_lowercase)
        .eq(right.chars().flat_map(char::to_lowercase))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zen() -> UserNames {
        UserNames::new(Some(PathBuf::from(r"C:\Users\Zen")), ["Zen".to_owned()])
    }

    #[test]
    fn blanks_the_user_folder_however_its_written() {
        let user = zen();
        assert_eq!(
            user.blank(r"Saved C:\Users\Zen\Videos\a.mkv"),
            r"Saved C:\Users\<user>\Videos\a.mkv"
        );
        assert_eq!(
            user.blank(r"c:\users\ZEN\AppData\Local"),
            r"C:\Users\<user>\AppData\Local"
        );
        assert_eq!(
            user.blank("file:///C:/Users/Zen/Downloads"),
            "file:///C:/Users/<user>/Downloads"
        );
        assert_eq!(
            user.blank(r#"{"filename": "C:\\Users\\Zen\\Videos\\a.mkv"}"#),
            r#"{"filename": "C:\\Users\\<user>\\Videos\\a.mkv"}"#
        );
    }

    #[test]
    fn blanks_the_bare_name_as_a_word_only() {
        let user = zen();
        assert_eq!(
            user.blank("Signed in as zen, uploaded by Zen."),
            "Signed in as <user>, uploaded by <user>."
        );
        assert_eq!(
            user.blank("https://www.instagram.com/zen_photos/"),
            "https://www.instagram.com/<user>_photos/"
        );
        assert_eq!(
            user.blank("Zenith and citizen stay"),
            "Zenith and citizen stay"
        );
        assert_eq!(
            user.blank(r"C:\Users\Zenith\Videos"),
            r"C:\Users\Zenith\Videos",
            "another user's folder"
        );
    }

    #[test]
    fn blanks_a_short_name_only_in_the_users_folder() {
        let user = UserNames::new(Some(PathBuf::from(r"C:\Users\Al")), ["Al".to_owned()]);
        assert_eq!(
            user.blank(r"Al wrote C:\Users\Al\Videos and C:/Users/al"),
            r"Al wrote C:\Users\<user>\Videos and C:/Users/<user>"
        );
    }

    #[test]
    fn blanks_the_account_name_too_and_any_letters() {
        let user = UserNames::new(
            Some(PathBuf::from(r"D:\Profiles\aranu")),
            ["aranu".to_owned(), "Aranis".to_owned(), "ARANU".to_owned()],
        );
        assert_eq!(user.names, ["aranu", "Aranis"]);
        assert_eq!(
            user.blank(r"D:\Profiles\aranu\x by Aranis"),
            r"D:\Profiles\<user>\x by <user>"
        );

        let accented = UserNames::new(None, ["José".to_owned()]);
        assert_eq!(accented.blank("JOSÉ and josé"), "<user> and <user>");
    }

    #[test]
    fn keeps_the_last_lines_of_a_log() {
        assert_eq!(last_lines("a\nb\nc\n\n", 2), "b\nc");
        assert_eq!(last_lines("a\r\nb\r\n", 5), "a\nb");
        assert_eq!(last_lines("", 5), "");

        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("1.log");
        let lines: Vec<String> = (0..20_000).map(|line| format!("line {line}")).collect();
        std::fs::write(&path, lines.join("\n")).unwrap();
        let tail = log_tail(&path, 3).unwrap();
        assert_eq!(tail, "line 19997\nline 19998\nline 19999");
        assert_eq!(log_tail(&folder.path().join("missing.log"), 3), None);

        let (end, cut) = read_end(&path, 12).unwrap();
        assert!(cut, "there's more before it");
        assert!(!end.is_empty() && std::fs::read_to_string(&path).unwrap().ends_with(&end));
        let (whole, cut) = read_end(&path, 1_000_000).unwrap();
        assert!(!cut);
        assert_eq!(whole, std::fs::read_to_string(&path).unwrap());
    }

    #[test]
    fn writes_a_report_with_the_user_blanked_out() {
        let report = DownloadReport {
            app_version: "0.1.36".to_owned(),
            windows: "Windows 11 Home 24H2 (build 26100.4061)".to_owned(),
            tools: vec![
                ("yt-dlp", Some("2026.10.26".to_owned())),
                ("gallery-dl", None),
            ],
            link: "https://youtu.be/jNQXAC9IVRw".to_owned(),
            error: ShownError {
                message: "Couldn't create the download folder.".to_owned(),
                detail: Some(r"C:\Users\Zen\Videos: access denied".to_owned()),
            },
            log: Some(r"[download] Destination: C:\Users\Zen\Videos\a.mkv".to_owned()),
        };
        assert_eq!(
            report.text(&zen()),
            [
                "Tsundoku 0.1.36",
                "Windows 11 Home 24H2 (build 26100.4061)",
                "",
                "Tools:",
                "  yt-dlp 2026.10.26",
                "  gallery-dl: not set up or won't run",
                "",
                "Link: https://youtu.be/jNQXAC9IVRw",
                "Error: Couldn't create the download folder.",
                r"C:\Users\<user>\Videos: access denied",
                "",
                "Log, last 50 lines:",
                r"[download] Destination: C:\Users\<user>\Videos\a.mkv",
            ]
            .join("\n")
        );
    }
}
