//! A password a person typed for a link that needs one: kept with its link (a pending link, a queue item, the history's
//! video), given to the reader and the download beside the link, and never written anywhere else. Its `Debug` hides
//! it, it has no `Display` or `Serialize`, and what a download logs has it blanked (`hidden_in_arguments`). Before a
//! person is asked for one, the ones Tsundoku knows are tried (`known`): Settings → Passwords' list, then the ones
//! used before.

pub mod known;

use std::fmt;

pub use self::known::{ArchivePasswords, KnownPasswords, read_trying};

/// What stands in for a password in a log or a report.
pub const PASSWORD_PLACEHOLDER: &str = "<password>";

/// yt-dlp's option for a video's password, followed by the password.
pub(crate) const VIDEO_PASSWORD_OPTION: &str = "--video-password";

/// A password, as typed. Empty means none: `Password::new` gives `None` for it.
#[derive(Clone, PartialEq, Eq)]
pub struct Password(String);

impl Password {
    /// The password typed, or `None` for an empty one. Spaces are kept: a password may have them.
    pub fn new(text: impl Into<String>) -> Option<Self> {
        let text = text.into();
        (!text.is_empty()).then_some(Self(text))
    }

    /// The password itself, for the request or the tool that needs it.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(PASSWORD_PLACEHOLDER)
    }
}

/// A tool's arguments as a download's log writes them: the value after each `--video-password` (Tsundoku's own, or one
/// a preset's extra arguments added) blanked out.
pub(crate) fn hidden_in_arguments(arguments: &[String]) -> String {
    let mut shown = Vec::with_capacity(arguments.len());
    let mut hide_next = false;
    for argument in arguments {
        if hide_next {
            shown.push(PASSWORD_PLACEHOLDER.to_owned());
            hide_next = false;
            continue;
        }
        if let Some((option, _)) = argument.split_once('=')
            && is_video_password_option(option)
        {
            shown.push(format!("{option}={PASSWORD_PLACEHOLDER}"));
            continue;
        }
        hide_next = is_video_password_option(argument);
        shown.push(argument.clone());
    }
    shown.join(" ")
}

/// `--video-password`, or an abbreviation of it yt-dlp would take (`--video-pass`).
fn is_video_password_option(argument: &str) -> bool {
    argument.len() >= "--video-p".len() && VIDEO_PASSWORD_OPTION.starts_with(argument)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_password_is_none_and_debug_hides_one() {
        assert_eq!(Password::new(""), None);
        let password = Password::new(" two words ").unwrap();
        assert_eq!(password.reveal(), " two words ");
        assert_eq!(format!("{password:?}"), "<password>");
        assert_eq!(format!("{:?}", Some(password)), "Some(<password>)");
    }

    #[test]
    fn a_logged_command_never_holds_the_password() {
        let arguments: Vec<String> = [
            "--ignore-config",
            "--video-password",
            "hunter2",
            "--video-pass=hunter3",
            "--video-p",
            "hunter4",
            "--",
            "https://player.vimeo.com/video/68375962",
        ]
        .map(str::to_owned)
        .to_vec();
        let logged = hidden_in_arguments(&arguments);
        assert_eq!(
            logged,
            "--ignore-config --video-password <password> --video-pass=<password> --video-p <password> -- \
             https://player.vimeo.com/video/68375962"
        );
        assert!(!logged.contains("hunter"));
        assert_eq!(
            hidden_in_arguments(&["--video".to_owned(), "x".to_owned()]),
            "--video x"
        );
    }
}
