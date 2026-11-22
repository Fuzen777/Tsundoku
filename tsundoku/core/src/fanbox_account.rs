//! Tsundoku's sign-in to Fanbox (Settings → Sign-Ins). Fanbox signs in with the pixiv account, and keeps its session
//! in a cookie of its own, so the sign-in is the sign-in window's (the app crate's `accounts.rs`), on the profile Pixiv's
//! sign-in uses: the person continues there as their pixiv account once, and Fanbox's API is then read from that
//! window's session (`web_reader.rs`), never with Tsundoku's own client, since Fanbox answers anything else with its
//! site's check. What's kept here is only that the person signed in, and when: `accounts\fanbox.json`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, PoisonError, RwLock};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The file the sign-in is kept in, inside the accounts folder.
const ACCOUNT_FILE: &str = "fanbox.json";

/// Fanbox's sign-in, as Settings shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FanboxSignIn {
    /// When the person signed in, in milliseconds since 1970.
    #[ts(type = "number")]
    pub signed_in_at: i64,
}

struct Account {
    folder: PathBuf,
    signed_in: RwLock<Option<FanboxSignIn>>,
}

static ACCOUNT: OnceLock<Account> = OnceLock::new();

/// Reads the sign-in kept in `folder` (the data folder's accounts folder), once, at startup.
pub fn open(folder: &Path) {
    let signed_in = fs::read_to_string(folder.join(ACCOUNT_FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok());
    let _ = ACCOUNT.set(Account {
        folder: folder.to_path_buf(),
        signed_in: RwLock::new(signed_in),
    });
}

/// The sign-in, if the person signed in to Fanbox.
pub fn signed_in() -> Option<FanboxSignIn> {
    *ACCOUNT
        .get()?
        .signed_in
        .read()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Keeps that the person signed in at `at` (milliseconds since 1970).
pub fn keep(at: i64) -> Result<FanboxSignIn, String> {
    let account = ACCOUNT.get().ok_or("the data folder isn't open")?;
    let sign_in = FanboxSignIn { signed_in_at: at };
    fs::create_dir_all(&account.folder).map_err(|error| error.to_string())?;
    fs::write(
        account.folder.join(ACCOUNT_FILE),
        serde_json::to_vec(&sign_in).unwrap_or_default(),
    )
    .map_err(|error| error.to_string())?;
    *account
        .signed_in
        .write()
        .unwrap_or_else(PoisonError::into_inner) = Some(sign_in);
    Ok(sign_in)
}

/// Forgets the sign-in, as when the window's profile is deleted (signing out of Pixiv) or Fanbox answers that it's
/// signed out.
pub fn forget() {
    let Some(account) = ACCOUNT.get() else {
        return;
    };
    *account
        .signed_in
        .write()
        .unwrap_or_else(PoisonError::into_inner) = None;
    let _ = fs::remove_file(account.folder.join(ACCOUNT_FILE));
}

/// Whether Fanbox's answer to a request needing a sign-in says the session is signed in: 401 is signed out.
pub fn answer_signed_in(status: u16) -> Option<bool> {
    match status {
        200..=299 => Some(true),
        401 => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_a_signed_out_answer() {
        assert_eq!(answer_signed_in(200), Some(true));
        assert_eq!(answer_signed_in(401), Some(false));
        assert_eq!(
            answer_signed_in(403),
            None,
            "a check in front of the site says nothing"
        );
    }
}
