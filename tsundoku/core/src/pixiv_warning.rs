//! Pixiv's warnings. Pixiv sends an account that downloads too much too fast a message from its office (pixiv事務局)
//! pointing to its rules, and may lock or delete an account it has to warn again. Tsundoku reads the account's latest
//! message threads at the first Pixiv request of a run, again once an hour while it goes on using Pixiv, and after
//! every `FILES_BETWEEN_CHECKS` files or about `REQUESTS_BETWEEN_CHECKS` requests, through the sign-in window's own
//! session (a `MessageReader`, which the app crate gives: Pixiv's messages need the website's sign-in, not the
//! token gallery-dl reads with). A warning is a thread from Pixiv's office whose latest message links Pixiv's rules
//! (`policies.pixiv.net`) and names their article 14, as PixivBatchDownloader tells one (its code read for the
//! protocol only).
//!
//! A warning sent in the past day (`RECENT`), and newer than the last one the person went on past (`dismiss`, kept in
//! the accounts folder), stops every Pixiv read and download until they go on: `refusal` fails a gallery-dl run on a
//! Pixiv link with `PreviewError::PixivWarned`, the queue holds Pixiv's downloads (`Hold::PixivWarning`), Pending
//! leaves Pixiv's links waiting, and the app crate stops the ones running, shows the warning and resumes them once the
//! person goes on (`WarningListener`).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::metadata::PreviewError;
use crate::site_limits::now_ms;
use crate::sync::lock;
use crate::{pixiv_account, sites};

/// The name of the thread Pixiv's office writes in (user 11's).
const OFFICE: &str = "pixiv事務局";
/// What a warning's message holds: a link to Pixiv's rules, and the number of their article on such use.
const RULES_ADDRESS: &str = "policies.pixiv.net";
const RULE_NUMBER: &str = "14";
/// How long after it was sent a warning stops Pixiv's downloads: one from before then is old news.
const RECENT: Duration = Duration::from_secs(24 * 60 * 60);
/// Files downloaded from Pixiv between two checks.
pub const FILES_BETWEEN_CHECKS: u32 = 100;
/// Requests to Pixiv between two checks, counted roughly: one for each gallery-dl run, and one for each 30 works a
/// list answered (`WORKS_PER_REQUEST`).
pub const REQUESTS_BETWEEN_CHECKS: u32 = 300;
/// Works in one of Pixiv's list pages.
const WORKS_PER_REQUEST: u32 = 30;
/// How long a check lasts while Tsundoku goes on using Pixiv.
const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);
/// Where the person reads their messages.
pub const MESSAGES_PAGE: &str = "https://www.pixiv.net/messages.php";
/// The page the reader opens, on Pixiv's own site, before asking for the threads from it.
pub const READER_PAGE: &str = "https://www.pixiv.net/robots.txt";
/// The latest message threads, asked from `READER_PAGE` as the website asks them.
pub const THREADS_ADDRESS: &str = "/rpc/index.php?mode=latest_message_threads2&num=5&offset=0";
/// The file in the accounts folder that keeps the warning the person went on past.
const DISMISSED_FILE: &str = "pixiv-warning.json";

/// A warning Pixiv sent the account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PixivWarning {
    /// When Pixiv sent it, in milliseconds since 1970.
    #[ts(type = "number")]
    pub sent_at: i64,
}

/// Why the messages couldn't be read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum MessagesProblem {
    /// Pixiv answered that the sign-in window's session isn't signed in: signing in again fixes it.
    NotSignedIn,
    Failed {
        detail: String,
    },
}

/// The warning check, as Settings → Sign-Ins and the warning's notice show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PixivWarningStatus {
    /// The warning found, which stops Pixiv's downloads until the person goes on past it.
    pub warning: Option<PixivWarning>,
    /// When the messages were last read, in milliseconds since 1970.
    #[ts(type = "number | null")]
    pub checked_at: Option<i64>,
    /// Why the last check couldn't read them.
    pub problem: Option<MessagesProblem>,
    /// A check runs now.
    pub checking: bool,
}

/// Pixiv's answer to `THREADS_ADDRESS`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessagesAnswer {
    pub status: u16,
    pub body: String,
}

/// Reads the account's latest message threads as the website's signed-in session does.
pub trait MessageReader: Send + Sync {
    /// Pixiv's answer, or why there's none.
    fn read(&self) -> Result<MessagesAnswer, String>;
}

/// Told about the warning check.
pub trait WarningListener: Send + Sync {
    /// The check's status changed.
    fn changed(&self, status: &PixivWarningStatus);
    /// A warning was found: Pixiv's downloads must stop.
    fn warned(&self, warning: PixivWarning);
    /// The person went on past the warning: Pixiv's downloads may go on.
    fn dismissed(&self);
}

struct Warnings {
    folder: PathBuf,
    reader: Box<dyn MessageReader>,
    listener: Box<dyn WarningListener>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    warning: Option<PixivWarning>,
    /// When the warning the person last went on past was sent, in milliseconds since 1970.
    dismissed: Option<i64>,
    checked_at: Option<i64>,
    /// When the last check ended, whether it could read the messages or not.
    last_check: Option<Instant>,
    problem: Option<MessagesProblem>,
    checking: bool,
    /// Since the last check.
    files: u32,
    requests: u32,
}

impl State {
    fn status(&self) -> PixivWarningStatus {
        PixivWarningStatus {
            warning: self.warning,
            checked_at: self.checked_at,
            problem: self.problem.clone(),
            checking: self.checking,
        }
    }

    /// Whether Pixiv's use since the last check calls for another.
    fn due(&self, now: Instant) -> bool {
        !self.checking
            && (self
                .last_check
                .is_none_or(|last| now.duration_since(last) >= CHECK_EVERY)
                || self.files >= FILES_BETWEEN_CHECKS
                || self.requests >= REQUESTS_BETWEEN_CHECKS)
    }
}

static WARNINGS: OnceLock<Warnings> = OnceLock::new();

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Dismissed {
    sent_at: i64,
}

/// Starts the check, once, at startup: `folder` is the data folder's accounts folder.
pub fn start(folder: &Path, reader: Box<dyn MessageReader>, listener: Box<dyn WarningListener>) {
    let dismissed = fs::read_to_string(folder.join(DISMISSED_FILE))
        .ok()
        .and_then(|text| serde_json::from_str::<Dismissed>(&text).ok())
        .map(|dismissed| dismissed.sent_at);
    let _ = WARNINGS.set(Warnings {
        folder: folder.to_path_buf(),
        reader,
        listener,
        state: Mutex::new(State {
            dismissed,
            ..State::default()
        }),
    });
}

/// The check's status: none has run before it's started.
pub fn status() -> PixivWarningStatus {
    WARNINGS.get().map_or_else(
        || State::default().status(),
        |warnings| lock(&warnings.state).status(),
    )
}

fn on_pixiv(link: &str) -> bool {
    sites::host(link).is_some_and(|host| sites::on_domain(&host, "pixiv.net"))
}

/// Whether `link` is on Pixiv while a warning stops it.
pub fn holds(link: &str) -> bool {
    on_pixiv(link)
        && WARNINGS
            .get()
            .is_some_and(|warnings| lock(&warnings.state).warning.is_some())
}

/// The error a read or download of `link` fails with while a warning stops Pixiv's.
pub fn refusal(link: &str) -> Option<PreviewError> {
    holds(link).then_some(PreviewError::PixivWarned)
}

/// A read of `link` answered `posts` posts: counted towards the next check when it's on Pixiv.
pub(crate) fn note_read(link: &str, posts: u32) {
    note(link, 0, 1 + posts / WORKS_PER_REQUEST);
}

/// A download of `link` saved `files` files: counted towards the next check when it's on Pixiv.
pub(crate) fn note_download(link: &str, files: u32) {
    note(link, files, 1);
}

/// Counts Pixiv's use, and checks the messages on a thread of its own when it's time.
fn note(link: &str, files: u32, requests: u32) {
    if !on_pixiv(link) || pixiv_account::account().is_none() {
        return;
    }
    let Some(warnings) = WARNINGS.get() else {
        return;
    };
    {
        let mut state = lock(&warnings.state);
        state.files = state.files.saturating_add(files);
        state.requests = state.requests.saturating_add(requests);
        if !state.due(Instant::now()) {
            return;
        }
        state.checking = true;
    }
    thread::spawn(|| {
        if let Some(warnings) = WARNINGS.get() {
            run_check(warnings);
        }
    });
}

/// Reads the messages now, unless a check runs already, and gives the status after (Settings' Check Now).
pub fn check() -> PixivWarningStatus {
    let Some(warnings) = WARNINGS.get() else {
        return status();
    };
    {
        let mut state = lock(&warnings.state);
        if state.checking {
            return state.status();
        }
        state.checking = true;
    }
    warnings.listener.changed(&status());
    run_check(warnings)
}

fn run_check(warnings: &Warnings) -> PixivWarningStatus {
    let now = now_ms();
    let found = warnings
        .reader
        .read()
        .map_err(|detail| MessagesProblem::Failed { detail })
        .and_then(|answer| warning_in(&answer, now));
    let (status, new_warning) = {
        let mut state = lock(&warnings.state);
        state.checking = false;
        state.last_check = Some(Instant::now());
        state.files = 0;
        state.requests = 0;
        let mut new_warning = None;
        match found {
            Ok(found) => {
                state.checked_at = Some(now);
                state.problem = None;
                let dismissed = state.dismissed;
                // A warning stays until the person goes on past it, even once it's a day old.
                if let Some(found) =
                    found.filter(|found| dismissed.is_none_or(|sent| found.sent_at > sent))
                    && state.warning != Some(found)
                {
                    state.warning = Some(found);
                    new_warning = Some(found);
                }
            }
            Err(problem) => state.problem = Some(problem),
        }
        (state.status(), new_warning)
    };
    warnings.listener.changed(&status);
    if let Some(warning) = new_warning {
        warnings.listener.warned(warning);
    }
    status
}

/// The person goes on past the warning: Pixiv's downloads and reads go on, and that warning won't stop them again.
pub fn dismiss() -> PixivWarningStatus {
    let Some(warnings) = WARNINGS.get() else {
        return status();
    };
    let (status, dismissed) = {
        let mut state = lock(&warnings.state);
        let dismissed = state.warning.take();
        if let Some(warning) = dismissed {
            state.dismissed = Some(warning.sent_at);
            let kept = serde_json::to_vec(&Dismissed {
                sent_at: warning.sent_at,
            })
            .unwrap_or_default();
            let _ = fs::create_dir_all(&warnings.folder)
                .and_then(|()| fs::write(warnings.folder.join(DISMISSED_FILE), kept));
        }
        (state.status(), dismissed.is_some())
    };
    warnings.listener.changed(&status);
    if dismissed {
        warnings.listener.dismissed();
    }
    status
}

/// The newest warning among the threads Pixiv answered, sent within `RECENT` of `now` (milliseconds since 1970).
pub fn warning_in(
    answer: &MessagesAnswer,
    now: i64,
) -> Result<Option<PixivWarning>, MessagesProblem> {
    // A page instead of Pixiv's JSON (a check in front of the site) isn't Pixiv saying who's signed in.
    let Ok(json) = serde_json::from_str::<Value>(&answer.body) else {
        return Err(MessagesProblem::Failed {
            detail: format!("Pixiv answered {}", answer.status),
        });
    };
    let refused = matches!(answer.status, 401 | 403);
    if json.get("error").and_then(Value::as_bool) == Some(true)
        || !(200..300).contains(&answer.status)
    {
        let message = json
            .get("message")
            .and_then(Value::as_str)
            .filter(|message| !message.is_empty())
            .map_or_else(
                || format!("Pixiv answered {}", answer.status),
                str::to_owned,
            );
        return Err(if refused {
            MessagesProblem::NotSignedIn
        } else {
            MessagesProblem::Failed { detail: message }
        });
    }
    let threads = json
        .pointer("/body/message_threads")
        .and_then(Value::as_array)
        .ok_or_else(|| MessagesProblem::Failed {
            detail: "Pixiv's answer has no message threads".to_owned(),
        })?;
    let recent = i64::try_from(RECENT.as_millis()).unwrap_or(i64::MAX);
    Ok(threads
        .iter()
        .filter(|thread| is_warning(thread))
        .filter_map(sent_at)
        .filter(|sent| now.saturating_sub(*sent) < recent)
        .max()
        .map(|sent_at| PixivWarning { sent_at }))
}

/// A thread from Pixiv's office whose latest message is a warning.
fn is_warning(thread: &Value) -> bool {
    let content = thread
        .get("latest_content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    thread.get("is_official").and_then(Value::as_bool) == Some(true)
        && thread.get("thread_name").and_then(Value::as_str) == Some(OFFICE)
        && content.contains(RULES_ADDRESS)
        && content.contains(RULE_NUMBER)
}

/// When a thread's latest message was sent, in milliseconds: Pixiv gives seconds, as text.
fn sent_at(thread: &Value) -> Option<i64> {
    let value = thread.get("modified_at")?;
    let seconds = value
        .as_i64()
        .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))?;
    seconds.checked_mul(1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_795_000_000_000;

    fn answer(threads: &str) -> MessagesAnswer {
        MessagesAnswer {
            status: 200,
            body: format!(
                r#"{{"error": false, "message": "", "body": {{"total": 2, "message_threads": [{threads}]}}}}"#
            ),
        }
    }

    fn thread(name: &str, official: bool, content: &str, seconds: i64) -> String {
        serde_json::json!({
            "thread_id": "1",
            "modified_at": seconds.to_string(),
            "unread_num": "1",
            "member_num": "2",
            "latest_content": content,
            "is_pair": true,
            "thread_name": name,
            "pixiv_id": "pixiv",
            "is_official": official,
        })
        .to_string()
    }

    const WARNING: &str = "Your account was found to have made a large number of requests, which goes against \
        Article 14 of our Terms of Use: https://policies.pixiv.net/en.html#terms";

    #[test]
    fn finds_a_warning_from_pixivs_office_sent_in_the_past_day() {
        let sent = NOW / 1000 - 3600;
        let threads = format!(
            "{},{}",
            thread("Someone", false, "Thank you!", NOW / 1000 - 60),
            thread(OFFICE, true, WARNING, sent)
        );
        assert_eq!(
            warning_in(&answer(&threads), NOW),
            Ok(Some(PixivWarning {
                sent_at: sent * 1000
            }))
        );
    }

    #[test]
    fn leaves_other_messages_and_old_warnings_alone() {
        let day = 24 * 60 * 60;
        for threads in [
            thread(
                OFFICE,
                true,
                "Pixiv's new features: https://policies.pixiv.net/",
                NOW / 1000,
            ),
            thread(OFFICE, true, "Article 14 applies to everyone.", NOW / 1000),
            thread(OFFICE, false, WARNING, NOW / 1000),
            thread("Someone", true, WARNING, NOW / 1000),
            thread(OFFICE, true, WARNING, NOW / 1000 - day - 1),
        ] {
            assert_eq!(warning_in(&answer(&threads), NOW), Ok(None), "{threads}");
        }
        assert_eq!(warning_in(&answer(""), NOW), Ok(None));
    }

    #[test]
    fn tells_a_signed_out_session_from_other_failures() {
        let signed_out = MessagesAnswer {
            status: 403,
            body: r#"{"error":true,"message":"You're not logged in.","body":[]}"#.to_owned(),
        };
        assert_eq!(
            warning_in(&signed_out, NOW),
            Err(MessagesProblem::NotSignedIn)
        );
        let page = MessagesAnswer {
            status: 403,
            body: "<html>Just a moment...</html>".to_owned(),
        };
        assert_eq!(
            warning_in(&page, NOW),
            Err(MessagesProblem::Failed {
                detail: "Pixiv answered 403".to_owned()
            }),
            "a check's page"
        );
        let error = MessagesAnswer {
            status: 200,
            body: r#"{"error":true,"message":"Something went wrong","body":[]}"#.to_owned(),
        };
        assert_eq!(
            warning_in(&error, NOW),
            Err(MessagesProblem::Failed {
                detail: "Something went wrong".to_owned()
            })
        );
    }

    #[test]
    fn checks_when_first_used_hourly_and_after_enough_use() {
        let now = Instant::now();
        let mut state = State::default();
        assert!(state.due(now), "never checked");
        state.last_check = Some(now);
        assert!(!state.due(now));
        assert!(state.due(now + CHECK_EVERY));
        state.files = FILES_BETWEEN_CHECKS;
        assert!(state.due(now));
        state.files = 0;
        state.requests = REQUESTS_BETWEEN_CHECKS;
        assert!(state.due(now));
        state.checking = true;
        assert!(!state.due(now), "one check at a time");
    }

    #[test]
    fn refuses_only_pixivs_links() {
        assert!(!holds("https://www.instagram.com/p/x/"));
        assert_eq!(
            refusal("https://www.pixiv.net/artworks/1"),
            None,
            "no warning"
        );
    }
}
