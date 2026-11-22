//! Tsundoku's own sign-in to Pixiv (Settings → Sign-Ins), for what a browser's cookies can't open: gallery-dl reads
//! Pixiv through the login Pixiv's phone app uses, which needs a token (gallery-dl's `refresh-token`) from signing in
//! once. The sign-in window (the app crate's `accounts.rs`) shows Pixiv's own login page at `PixivLogin::url`; Pixiv
//! ends it on a one-time code (`code_in`), which `exchange` turns into the token within its 30 seconds. The token is
//! kept encrypted for this Windows account (`protect`) in the data folder's `accounts\pixiv.json`, never in
//! settings.json or a backup, and handed to gallery-dl in a config file of its own for each run (`gallery_dl_config`),
//! never on its command line or in a log. The window keeps its own browser profile (`DataPaths::accounts_folder`), so
//! the sign-in stays there for Fanbox and Pixiv's messages too.
//!
//! The login's parameters are the phone app's (`CLIENT_ID`, `CLIENT_SECRET`), as gallery-dl's own `oauth:pixiv` and
//! other Pixiv tools use them; gallery-dl's code (GPL) was read for the protocol only.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError, RwLock};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::metadata::PreviewError;
use crate::{hosts, sites};

/// Where Pixiv's login for its phone app starts.
const LOGIN_URL: &str = "https://app-api.pixiv.net/web/v1/login";
/// Where the login ends, with the one-time code in its query, before Pixiv sends the page on to the app's own
/// `pixiv://account/login?code=…`, which a browser can't open.
const CALLBACK_HOST: &str = "app-api.pixiv.net";
const CALLBACK_PATH: &str = "/web/v1/users/auth/pixiv/callback";
const APP_SCHEME: &str = "pixiv";
/// Where the code becomes the token.
const TOKEN_URL: &str = "https://oauth.secure.pixiv.net/auth/token";
/// Pixiv's phone app's own client, which its login and gallery-dl's reads go by.
const CLIENT_ID: &str = "MOBrBDS8blbauoSck0ZfDbtuzpyT";
const CLIENT_SECRET: &str = "lsACyCD94FhDUtGTXi3QzcFE2uU1hqtDaKeqrdwj";
/// The app the token request says it comes from, as the login expects.
const APP_AGENT: &str = "PixivAndroidApp/5.0.234 (Android 11; Pixel 5)";
/// The file the sign-in is kept in, inside the accounts folder.
const ACCOUNT_FILE: &str = "pixiv.json";

/// The pixiv account Tsundoku is signed in to, as Settings shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PixivAccount {
    /// Its name, as Pixiv shows it.
    pub name: String,
    /// Its handle (pixiv ID), such as "someone".
    pub account: String,
}

/// Why signing in or out didn't work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum AccountError {
    /// Pixiv couldn't be reached.
    #[error("Pixiv couldn't be reached: {detail}")]
    Network { detail: String },
    /// Pixiv refused the code, which happens when it's more than 30 seconds old: signing in again works.
    #[error("Pixiv refused the sign-in: {detail}")]
    Refused { detail: String },
    /// The sign-in couldn't be saved or read in the data folder.
    #[error("the sign-in couldn't be kept: {detail}")]
    Storage { detail: String },
}

/// One login in progress: the secret its code is checked against (PKCE's verifier), known only to Tsundoku.
pub struct PixivLogin {
    verifier: String,
}

impl PixivLogin {
    /// A new login, with a verifier of 32 random bytes.
    pub fn start() -> Result<Self, AccountError> {
        let bytes = random_bytes::<32>().map_err(|detail| AccountError::Storage { detail })?;
        Ok(Self {
            verifier: URL_SAFE_NO_PAD.encode(bytes),
        })
    }

    /// Pixiv's login page for it, asking for a code only this login's verifier can turn into a token.
    pub fn url(&self) -> String {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(self.verifier.as_bytes()));
        format!(
            "{LOGIN_URL}?code_challenge={challenge}&code_challenge_method=S256&client=pixiv-android"
        )
    }

    /// Turns the code Pixiv ended the login on into the token and the account it's for.
    pub fn exchange(&self, code: &str) -> Result<SignedIn, AccountError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| AccountError::Network {
                detail: error.to_string(),
            })?;
        let response = client
            .post(TOKEN_URL)
            .header(reqwest::header::USER_AGENT, APP_AGENT)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(hosts::form(&[
                ("client_id", CLIENT_ID),
                ("client_secret", CLIENT_SECRET),
                ("code", code),
                ("code_verifier", self.verifier.as_str()),
                ("grant_type", "authorization_code"),
                ("include_policy", "true"),
                (
                    "redirect_uri",
                    "https://app-api.pixiv.net/web/v1/users/auth/pixiv/callback",
                ),
            ]))
            .send()
            .map_err(|error| AccountError::Network {
                detail: error.to_string(),
            })?;
        let status = response.status();
        let answer: Value = response.json().map_err(|error| AccountError::Network {
            detail: format!("{status}: {error}"),
        })?;
        signed_in_from(&answer).ok_or_else(|| AccountError::Refused {
            detail: answer
                .get("error")
                .map_or_else(|| status.to_string(), Value::to_string),
        })
    }
}

/// What a login gives: the token and the account it's for.
pub struct SignedIn {
    token: String,
    account: PixivAccount,
}

/// The token request's answer as a sign-in, when it holds a token.
fn signed_in_from(answer: &Value) -> Option<SignedIn> {
    let token = answer.get("refresh_token")?.as_str()?.to_owned();
    let user = answer.get("user");
    let text = |field: &str| {
        user.and_then(|user| user.get(field))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    (!token.is_empty()).then(|| SignedIn {
        token,
        account: PixivAccount {
            name: text("name"),
            account: text("account"),
        },
    })
}

/// The one-time code in an address the login window goes to at its end: Pixiv's callback page, or the app's own
/// `pixiv://` link it sends that page on to. `None` for any other address.
pub fn code_in(address: &str) -> Option<String> {
    let url = reqwest::Url::parse(address).ok()?;
    let is_callback = (url.scheme() == "https"
        && url.host_str() == Some(CALLBACK_HOST)
        && url.path() == CALLBACK_PATH)
        || url.scheme() == APP_SCHEME;
    if !is_callback {
        return None;
    }
    url.query_pairs()
        .find(|(name, _)| name == "code")
        .map(|(_, code)| code.into_owned())
        .filter(|code| !code.is_empty())
}

/// The sign-in as it's kept in the accounts folder: the account, and the token encrypted for this Windows account.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Kept {
    name: String,
    account: String,
    /// The token as `protect` gives it, in base64.
    token: String,
}

/// The sign-in while the app runs: the accounts folder (where it's kept, and where gallery-dl's config files are
/// written for a moment) and, when signed in, the account and its token.
struct Accounts {
    folder: PathBuf,
    signed_in: RwLock<Option<SignedIn>>,
}

static ACCOUNTS: OnceLock<Accounts> = OnceLock::new();

/// Reads the sign-in kept in `folder` (the data folder's accounts folder), once, at startup. A sign-in that can't be
/// read (another Windows account's, or a damaged file) counts as none, so signing in again replaces it.
pub fn open(folder: &Path) {
    let signed_in = read_kept(folder);
    let _ = ACCOUNTS.set(Accounts {
        folder: folder.to_path_buf(),
        signed_in: RwLock::new(signed_in),
    });
}

fn read_kept(folder: &Path) -> Option<SignedIn> {
    let text = fs::read_to_string(folder.join(ACCOUNT_FILE)).ok()?;
    let kept: Kept = serde_json::from_str(&text).ok()?;
    let sealed = STANDARD.decode(kept.token).ok()?;
    let token = String::from_utf8(unprotect(&sealed).ok()?).ok()?;
    Some(SignedIn {
        token,
        account: PixivAccount {
            name: kept.name,
            account: kept.account,
        },
    })
}

/// The account Tsundoku is signed in to, if any.
pub fn account() -> Option<PixivAccount> {
    let accounts = ACCOUNTS.get()?;
    let signed_in = accounts
        .signed_in
        .read()
        .unwrap_or_else(PoisonError::into_inner);
    signed_in
        .as_ref()
        .map(|signed_in| signed_in.account.clone())
}

/// Keeps a sign-in, replacing any before it, and gives its account.
pub fn keep(signed_in: SignedIn) -> Result<PixivAccount, AccountError> {
    let accounts = ACCOUNTS.get().ok_or_else(|| AccountError::Storage {
        detail: "the data folder isn't open".to_owned(),
    })?;
    let storage = |error: std::io::Error| AccountError::Storage {
        detail: error.to_string(),
    };
    let sealed =
        protect(signed_in.token.as_bytes()).map_err(|detail| AccountError::Storage { detail })?;
    let kept = Kept {
        name: signed_in.account.name.clone(),
        account: signed_in.account.account.clone(),
        token: STANDARD.encode(sealed),
    };
    fs::create_dir_all(&accounts.folder).map_err(storage)?;
    let path = accounts.folder.join(ACCOUNT_FILE);
    let written = path.with_extension("json.new");
    fs::write(
        &written,
        serde_json::to_vec_pretty(&kept).unwrap_or_default(),
    )
    .map_err(storage)?;
    fs::rename(&written, &path).map_err(storage)?;
    let account = signed_in.account.clone();
    *accounts
        .signed_in
        .write()
        .unwrap_or_else(PoisonError::into_inner) = Some(signed_in);
    Ok(account)
}

/// Forgets the sign-in: its file, and the token in memory.
pub fn forget() -> Result<(), AccountError> {
    let Some(accounts) = ACCOUNTS.get() else {
        return Ok(());
    };
    *accounts
        .signed_in
        .write()
        .unwrap_or_else(PoisonError::into_inner) = None;
    match fs::remove_file(accounts.folder.join(ACCOUNT_FILE)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(AccountError::Storage {
            detail: error.to_string(),
        }),
        _ => Ok(()),
    }
}

/// A config file gallery-dl reads the token from for one run (`-c`), deleted once the run ends (when this is
/// dropped).
pub struct GalleryDlConfig {
    path: PathBuf,
}

impl GalleryDlConfig {
    /// gallery-dl's arguments that read it, before the link.
    pub fn args(&self) -> [String; 2] {
        ["-c".to_owned(), self.path.display().to_string()]
    }
}

impl Drop for GalleryDlConfig {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// The file names of gallery-dl config files in use, counted so two runs at once never share one.
static NEXT_CONFIG: Mutex<u64> = Mutex::new(0);

/// gallery-dl's config file for a run on `link`, for a link on Pixiv while signed in: the token, tags with their English
/// names (`tags: original`), and for a download (`ffmpeg`, Tsundoku's ffmpeg) its animations made into videos
/// (`ugoira_postprocessor`). `None` otherwise, or when it can't be written (gallery-dl then says Pixiv needs a sign-in).
pub fn gallery_dl_config(link: &str, ffmpeg: Option<&Path>) -> Option<GalleryDlConfig> {
    let on_pixiv = sites::host(link).is_some_and(|host| sites::on_domain(&host, "pixiv.net"));
    if !on_pixiv {
        return None;
    }
    let accounts = ACCOUNTS.get()?;
    let token = accounts
        .signed_in
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()?
        .token
        .clone();
    let number = {
        let mut next = NEXT_CONFIG.lock().unwrap_or_else(PoisonError::into_inner);
        *next += 1;
        *next
    };
    let path = accounts
        .folder
        .join(format!("gallery-dl-{}-{number}.json", std::process::id()));
    let mut config = serde_json::json!({
        "extractor": {
            "pixiv": {
                "refresh-token": token,
                "tags": "original",
                // Pixiv flags many requests with no pause between them (Fuzen's experience): its reads (a listing's
                // pages, each animation's frames' timings) wait as its files do to start with.
                "sleep-request": [1.0, 2.0],
                // An animation as its zip of frames, one request, rather than each frame on its own.
                "ugoira": true,
            },
            "pixiv-novel": { "refresh-token": token },
        }
    });
    if let Some(ffmpeg) = ffmpeg {
        config["extractor"]["pixiv"]["postprocessors"] =
            serde_json::json!([ugoira_postprocessor(ffmpeg)]);
    }
    fs::create_dir_all(&accounts.folder).ok()?;
    fs::write(&path, config.to_string()).ok()?;
    Some(GalleryDlConfig { path })
}

/// gallery-dl's post processor that makes a Pixiv animation's zip of frames a video: VP9 in WebM, lossless and in full
/// colour (4:4:4, as Pixiv's frames are), each frame shown as long as Pixiv says (Fuzen's choice: quality first, then
/// size). On two of Pixiv's animations it kept every frame exactly at 0.3 to 1.5 times the size of Pixiv's zip; the
/// lossy settings, and gallery-dl's own `--ugoira vp9-lossless`, which is 4:2:0, halved the colour detail. The zip
/// goes once the video is made.
fn ugoira_postprocessor(ffmpeg: &Path) -> Value {
    serde_json::json!({
        "name": "ugoira",
        "extension": "webm",
        "ffmpeg-location": ffmpeg.display().to_string(),
        "ffmpeg-args": ["-c:v", "libvpx-vp9", "-lossless", "1", "-pix_fmt", "yuv444p", "-row-mt", "1", "-an"],
        "keep-files": false,
    })
}

/// A sign-in error on a Pixiv link as the one Pixiv's own sign-in fixes (`PreviewError::PixivSignInNeeded`), saying
/// whether Tsundoku is signed in already (so Pixiv refused that sign-in). Any other error is left as it is.
pub fn explain(error: PreviewError, link: &str) -> PreviewError {
    let on_pixiv = sites::host(link).is_some_and(|host| sites::on_domain(&host, "pixiv.net"));
    match error {
        PreviewError::SignInRequired { .. } if on_pixiv => PreviewError::PixivSignInNeeded {
            refused: account().is_some(),
        },
        other => other,
    }
}

/// Deletes gallery-dl config files a run left behind when the app stopped mid-run, at startup.
pub fn remove_left_configs(folder: &Path) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("gallery-dl-") && name.ends_with(".json") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// `data` encrypted so only this Windows account on this PC can read it back (DPAPI).
#[cfg(windows)]
fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).map_err(|error| error.to_string())?,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // SAFETY: `input` points at `data`, which outlives the call and isn't written; `output` is filled by the call and
    // freed by `take_blob`.
    let done = unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if done == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(take_blob(output))
}

/// What `protect` encrypted, read back.
#[cfg(windows)]
fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptUnprotectData,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).map_err(|error| error.to_string())?,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // SAFETY: as in `protect`.
    let done = unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if done == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(take_blob(output))
}

/// A blob Windows allocated, copied out and freed.
#[cfg(windows)]
fn take_blob(blob: windows_sys::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB) -> Vec<u8> {
    if blob.pbData.is_null() {
        return Vec::new();
    }
    // SAFETY: Windows gave `cbData` bytes at `pbData`, which stay until `LocalFree`.
    let copied = unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize) }.to_vec();
    // SAFETY: `pbData` came from Windows' LocalAlloc and is freed once.
    unsafe { windows_sys::Win32::Foundation::LocalFree(blob.pbData.cast()) };
    copied
}

/// Random bytes from Windows' own generator.
#[cfg(windows)]
fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    use windows_sys::Win32::Security::Cryptography::{
        BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
    };
    let mut bytes = [0u8; N];
    let length = u32::try_from(N).map_err(|error| error.to_string())?;
    // SAFETY: the buffer holds `length` bytes; no algorithm handle is needed with the system's preferred generator.
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            bytes.as_mut_ptr(),
            length,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status < 0 {
        return Err(format!("no random bytes (status {status:#x})"));
    }
    Ok(bytes)
}

/// Tsundoku only runs on Windows; elsewhere, where only its tests build, nothing is kept.
#[cfg(not(windows))]
fn protect(_data: &[u8]) -> Result<Vec<u8>, String> {
    Err("only on Windows".to_owned())
}

#[cfg(not(windows))]
fn unprotect(_data: &[u8]) -> Result<Vec<u8>, String> {
    Err("only on Windows".to_owned())
}

#[cfg(not(windows))]
fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    Err("only on Windows".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_code_at_the_end_of_the_login() {
        assert_eq!(
            code_in(
                "https://app-api.pixiv.net/web/v1/users/auth/pixiv/callback?state=abc&code=Xy-12_z"
            )
            .as_deref(),
            Some("Xy-12_z")
        );
        assert_eq!(
            code_in("pixiv://account/login?code=Ab9&via=login").as_deref(),
            Some("Ab9")
        );
        assert_eq!(
            code_in("https://accounts.pixiv.net/login?code=nope"),
            None,
            "not the callback"
        );
        assert_eq!(
            code_in(
                "https://app-api.pixiv.net.evil.example/web/v1/users/auth/pixiv/callback?code=x"
            ),
            None
        );
        assert_eq!(
            code_in("https://app-api.pixiv.net/web/v1/users/auth/pixiv/callback?state=abc"),
            None
        );
    }

    #[test]
    fn asks_for_a_code_only_its_verifier_opens() {
        let login = PixivLogin {
            verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_owned(),
        };
        // RFC 7636's own example of a verifier and its S256 challenge.
        assert_eq!(
            login.url(),
            "https://app-api.pixiv.net/web/v1/login?code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM\
             &code_challenge_method=S256&client=pixiv-android"
        );
    }

    #[test]
    fn reads_the_token_and_the_account() {
        let answer = serde_json::json!({
            "access_token": "a", "refresh_token": "r-token",
            "user": {"id": "11", "name": "Some One", "account": "someone"}
        });
        let signed_in = signed_in_from(&answer).unwrap();
        assert_eq!(signed_in.token, "r-token");
        assert_eq!(
            signed_in.account,
            PixivAccount {
                name: "Some One".to_owned(),
                account: "someone".to_owned(),
            }
        );
        assert!(signed_in_from(&serde_json::json!({"error": "invalid_grant"})).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn keeps_the_token_encrypted_for_this_windows_account() {
        let sealed = protect(b"secret token").unwrap();
        assert!(!sealed.windows(6).any(|part| part == b"secret"));
        assert_eq!(unprotect(&sealed).unwrap(), b"secret token");
        assert_ne!(random_bytes::<32>().unwrap(), random_bytes::<32>().unwrap());
    }
}
