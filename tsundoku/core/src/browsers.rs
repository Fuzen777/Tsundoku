//! The browsers on this PC, their profiles, and which sites each profile is signed in to, so yt-dlp and gallery-dl
//! read cookies from the right place without the user typing a profile path.
//!
//! Only cookie hosts, names and last-use times are read, never values. A browser keeps recent changes in its cookie
//! database's write-ahead log, so the database is read from a copy made with its `-wal` and `-shm` files. Chromium
//! browsers lock the database while they run, so their profiles count as unreadable until they're closed; Firefox
//! and its forks can be read while running. (The approach comes from Fuzen's RJ Library.)

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::settings::CookiesBrowser;
use crate::sites::{self, SignInSite};

/// Chromium counts time in microseconds from 1601, Unix from 1970.
const CHROMIUM_EPOCH_OFFSET_SECONDS: i64 = 11_644_473_600;

/// A browser profile found on this PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BrowserProfile {
    pub browser: CookiesBrowser,
    /// The profile's name in the browser, such as "Default" or "Work".
    pub name: String,
    /// The profile folder, which holds the cookie database.
    #[ts(type = "string")]
    pub path: PathBuf,
    /// The profile the browser opens by default.
    pub default: bool,
}

impl BrowserProfile {
    /// The value for yt-dlp's and gallery-dl's `--cookies-from-browser`: the browser family's name and the profile
    /// folder. Both tools read Firefox's forks as Firefox, given the folder.
    pub fn cookies_arg(&self) -> String {
        format!("{}:{}", self.browser.yt_dlp_name(), self.path.display())
    }

    fn cookie_database(&self) -> Option<PathBuf> {
        let candidates: &[&[&str]] = if self.browser.is_firefox_family() {
            &[&["cookies.sqlite"]]
        } else {
            &[&["Network", "Cookies"], &["Cookies"]]
        };
        candidates
            .iter()
            .map(|parts| {
                parts
                    .iter()
                    .fold(self.path.clone(), |path, part| path.join(part))
            })
            .find(|path| path.is_file())
    }
}

/// The folders browsers keep their profiles in: `%APPDATA%` (roaming) and `%LOCALAPPDATA%` (local).
#[derive(Debug, Clone)]
pub struct BrowserRoots {
    pub roaming: PathBuf,
    pub local: PathBuf,
}

impl BrowserRoots {
    /// This user's folders, or `None` where Windows doesn't say.
    pub fn of_this_user() -> Option<Self> {
        Some(Self {
            roaming: dirs::config_dir()?,
            local: dirs::data_local_dir()?,
        })
    }
}

/// Where each Firefox-family browser keeps `profiles.ini`, under `%APPDATA%`.
const FIREFOX_FOLDERS: [(CookiesBrowser, &str); 5] = [
    (CookiesBrowser::Firefox, "Mozilla/Firefox"),
    (CookiesBrowser::Zen, "zen"),
    (CookiesBrowser::LibreWolf, "librewolf"),
    (CookiesBrowser::Floorp, "Floorp"),
    (CookiesBrowser::Waterfox, "Waterfox"),
];

/// Where each Chromium browser keeps its `User Data` folder, under `%LOCALAPPDATA%`.
const CHROMIUM_FOLDERS: [(CookiesBrowser, &str); 5] = [
    (CookiesBrowser::Chrome, "Google/Chrome/User Data"),
    (CookiesBrowser::Edge, "Microsoft/Edge/User Data"),
    (
        CookiesBrowser::Brave,
        "BraveSoftware/Brave-Browser/User Data",
    ),
    (CookiesBrowser::Vivaldi, "Vivaldi/User Data"),
    (CookiesBrowser::Chromium, "Chromium/User Data"),
];

/// Every browser profile with a cookie database, Firefox's family first.
pub fn find_profiles(roots: &BrowserRoots) -> Vec<BrowserProfile> {
    let mut profiles = Vec::new();
    for (browser, folder) in FIREFOX_FOLDERS {
        profiles.extend(firefox_profiles(browser, &roots.roaming.join(folder)));
    }
    for (browser, folder) in CHROMIUM_FOLDERS {
        profiles.extend(chromium_profiles(browser, &roots.local.join(folder)));
    }
    profiles.extend(opera_profile(
        &roots.roaming.join("Opera Software/Opera Stable"),
    ));
    profiles.retain(|profile| profile.cookie_database().is_some());
    profiles
}

/// A Firefox-family browser's profiles, from `profiles.ini`. The default is the one an install uses
/// (`[Install…] Default=`, in `installs.ini` or `profiles.ini`), else the one marked `Default=1`.
fn firefox_profiles(browser: CookiesBrowser, folder: &Path) -> Vec<BrowserProfile> {
    let Ok(profiles_ini) = fs::read_to_string(folder.join("profiles.ini")) else {
        return Vec::new();
    };
    let installs_ini = fs::read_to_string(folder.join("installs.ini")).unwrap_or_default();
    let sections = parse_ini(&profiles_ini);
    // installs.ini names its sections by install alone; profiles.ini repeats them as "Install<id>".
    let install_defaults: Vec<String> = parse_ini(&installs_ini)
        .into_iter()
        .chain(
            sections
                .iter()
                .filter(|(name, _)| name.starts_with("Install"))
                .cloned(),
        )
        .filter_map(|(_, keys)| value(&keys, "Default").map(normalize_ini_path))
        .collect();

    let mut profiles: Vec<(BrowserProfile, bool)> = sections
        .iter()
        .filter(|(name, _)| name.starts_with("Profile"))
        .filter_map(|(_, keys)| {
            let path = value(keys, "Path")?;
            let relative = value(keys, "IsRelative").is_none_or(|flag| flag == "1");
            let full_path = if relative {
                folder.join(normalize_ini_path(path))
            } else {
                PathBuf::from(path)
            };
            let profile = BrowserProfile {
                browser,
                name: value(keys, "Name").unwrap_or(path).to_owned(),
                path: full_path,
                default: install_defaults.contains(&normalize_ini_path(path)),
            };
            Some((profile, value(keys, "Default") == Some("1")))
        })
        .collect();
    if !profiles.iter().any(|(profile, _)| profile.default) {
        for (profile, marked) in &mut profiles {
            profile.default = *marked;
        }
    }
    profiles.into_iter().map(|(profile, _)| profile).collect()
}

/// A Chromium browser's profiles, from `Local State`: their names, and the last one used as the default.
fn chromium_profiles(browser: CookiesBrowser, user_data: &Path) -> Vec<BrowserProfile> {
    let Ok(text) = fs::read_to_string(user_data.join("Local State")) else {
        return Vec::new();
    };
    let Ok(state) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let profile = &state["profile"];
    let last_used = profile["last_used"].as_str().unwrap_or("Default");
    let Some(info_cache) = profile["info_cache"].as_object() else {
        return Vec::new();
    };
    info_cache
        .iter()
        .map(|(folder, info)| BrowserProfile {
            browser,
            name: info["name"].as_str().unwrap_or(folder).to_owned(),
            path: user_data.join(folder),
            default: folder == last_used,
        })
        .collect()
}

/// Opera keeps one profile, in its own folder or (newer versions) a `Default` folder inside it.
fn opera_profile(folder: &Path) -> Option<BrowserProfile> {
    let path = [folder.join("Default"), folder.to_path_buf()]
        .into_iter()
        .find(|path| path.join("Network/Cookies").is_file() || path.join("Cookies").is_file())?;
    Some(BrowserProfile {
        browser: CookiesBrowser::Opera,
        name: "Default".to_owned(),
        path,
        default: true,
    })
}

type IniSection = (String, Vec<(String, String)>);

/// An INI file's sections, in order, with their keys.
fn parse_ini(text: &str) -> Vec<IniSection> {
    let mut sections: Vec<IniSection> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            sections.push((name.to_owned(), Vec::new()));
        } else if let (Some((key, value)), Some((_, keys))) =
            (line.split_once('='), sections.last_mut())
        {
            keys.push((key.trim().to_owned(), value.trim().to_owned()));
        }
    }
    sections
}

fn value<'a>(keys: &'a [(String, String)], key: &str) -> Option<&'a str> {
    keys.iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

/// `Profiles/abc.default` and `Profiles\abc.default` name the same folder.
fn normalize_ini_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// A cookie as far as Tsundoku reads it: where it's from, its name, and when the browser last sent it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookieUse {
    pub host: String,
    pub name: String,
    /// Seconds since 1970.
    pub last_used: i64,
}

/// The profile's cookies for `domains` and their subdomains: names and last uses only. Fails when the database
/// can't be copied (a running Chromium browser) or read.
pub fn cookie_uses(profile: &BrowserProfile, domains: &[String]) -> io::Result<Vec<CookieUse>> {
    let database = profile
        .cookie_database()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no cookie database"))?;
    let copy = DatabaseCopy::new(&database)?;
    let connection = Connection::open(&copy.path).map_err(io::Error::other)?;
    let (query, offset) = if profile.browser.is_firefox_family() {
        ("SELECT host, name, lastAccessed FROM moz_cookies", 0)
    } else {
        (
            "SELECT host_key, name, last_access_utc FROM cookies",
            CHROMIUM_EPOCH_OFFSET_SECONDS,
        )
    };
    let mut statement = connection.prepare(query).map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(CookieUse {
                host: row.get(0)?,
                name: row.get(1)?,
                last_used: row.get::<_, i64>(2)? / 1_000_000 - offset,
            })
        })
        .map_err(io::Error::other)?;
    let mut uses = Vec::new();
    for row in rows {
        let cookie = row.map_err(io::Error::other)?;
        if domains
            .iter()
            .any(|domain| sites::on_domain(&cookie.host, domain))
        {
            uses.push(cookie);
        }
    }
    Ok(uses)
}

/// A temporary copy of a cookie database with its write-ahead log, deleted when dropped. Opening it read-write lets
/// SQLite apply the log, which holds the browser's latest sign-ins.
struct DatabaseCopy {
    path: PathBuf,
}

impl DatabaseCopy {
    fn new(database: &Path) -> io::Result<Self> {
        static COUNT: AtomicU64 = AtomicU64::new(0);
        let name = format!(
            "tsundoku-cookies-{}-{}.sqlite",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        );
        let copy = Self {
            path: std::env::temp_dir().join(name),
        };
        fs::copy(database, &copy.path)?;
        for suffix in ["-wal", "-shm"] {
            let side = with_suffix(database, suffix);
            if side.is_file() {
                fs::copy(side, with_suffix(&copy.path, suffix))?;
            }
        }
        Ok(copy)
    }
}

impl Drop for DatabaseCopy {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = fs::remove_file(with_suffix(&self.path, suffix));
        }
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// When a profile last used a sign-in cookie of `site`, if it holds one.
pub fn signed_in_since(uses: &[CookieUse], site: &SignInSite) -> Option<i64> {
    uses.iter()
        .filter(|cookie| site.cookies.contains(&cookie.name.as_str()))
        .filter(|cookie| {
            site.domains
                .iter()
                .any(|domain| sites::on_domain(&cookie.host, domain))
        })
        .map(|cookie| cookie.last_used)
        .max()
}

/// The profile to read `link`'s cookies from: among `profiles` (of `only` when given), the one signed in to the
/// link's site that used it last, or for a site Tsundoku doesn't know, the one that used any of its cookies last.
/// `read` gives a profile's cookies for the link's domains; profiles it can't read are skipped. With no match, the
/// default profile of `only`, so a browser the user picked is still used.
pub fn choose_profile<'a>(
    profiles: &'a [BrowserProfile],
    only: Option<CookiesBrowser>,
    link: &str,
    read: impl Fn(&BrowserProfile, &[String]) -> io::Result<Vec<CookieUse>>,
) -> Option<&'a BrowserProfile> {
    let domains = sites::cookie_domains(link);
    let site = sites::sign_in_site(link);
    let candidates: Vec<&BrowserProfile> = profiles
        .iter()
        .filter(|profile| only.is_none_or(|browser| profile.browser == browser))
        .collect();
    let freshest = candidates
        .iter()
        .filter_map(|profile| {
            let uses = read(profile, &domains).ok()?;
            let score = match site {
                Some(site) => signed_in_since(&uses, site),
                None => uses.iter().map(|cookie| cookie.last_used).max(),
            }?;
            Some((score, *profile))
        })
        // The first of equal scores wins, so the found order breaks ties.
        .fold(
            None,
            |best: Option<(i64, &BrowserProfile)>, (score, profile)| match best {
                Some((best_score, _)) if best_score >= score => best,
                _ => Some((score, profile)),
            },
        )
        .map(|(_, profile)| profile);
    freshest.or_else(|| {
        only?;
        candidates
            .iter()
            .find(|profile| profile.default)
            .or(candidates.first())
            .copied()
    })
}

/// The `--cookies-from-browser` value for `link` under the user's choice: `None` reads no cookies; `Automatic` the
/// profile `choose_profile` finds, or none; a browser its profile `choose_profile` finds, or the browser's name
/// alone for yt-dlp to look itself when no profile is found (only for browsers yt-dlp knows).
pub fn cookies_for(choice: Option<CookiesBrowser>, link: &str) -> Option<String> {
    let choice = choice?;
    let only = (choice != CookiesBrowser::Automatic).then_some(choice);
    let profiles = BrowserRoots::of_this_user()
        .map(|roots| find_profiles(&roots))
        .unwrap_or_default();
    match choose_profile(&profiles, only, link, cookie_uses) {
        Some(profile) => Some(profile.cookies_arg()),
        None => only
            .filter(|browser| browser.known_to_yt_dlp())
            .map(|browser| browser.yt_dlp_name().to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(folder: &Path) -> BrowserRoots {
        BrowserRoots {
            roaming: folder.join("Roaming"),
            local: folder.join("Local"),
        }
    }

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "").unwrap();
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn finds_firefox_family_profiles_and_the_one_an_install_uses() {
        let folder = tempfile::tempdir().unwrap();
        let roots = roots(folder.path());
        let zen = roots.roaming.join("zen");
        write(
            &zen.join("profiles.ini"),
            "[General]\nStartWithLastProfile=1\n\n[Profile1]\nName=Old\nIsRelative=1\nPath=Profiles/old.default\nDefault=1\n\n\
             [Profile0]\nName=Default (release)\nIsRelative=1\nPath=Profiles\\abc.Default (release)\n",
        );
        write(
            &zen.join("installs.ini"),
            "[F0DC299D809B9700]\nDefault=Profiles/abc.Default (release)\nLocked=1\n",
        );
        touch(&zen.join("Profiles/old.default/cookies.sqlite"));
        touch(&zen.join("Profiles/abc.Default (release)/cookies.sqlite"));
        // A profile without cookies is left out.
        write(
            &roots.roaming.join("Mozilla/Firefox/profiles.ini"),
            "[Profile0]\nName=default\nIsRelative=1\nPath=Profiles/x.default\nDefault=1\n",
        );

        let profiles = find_profiles(&roots);
        assert_eq!(profiles.len(), 2);
        let release = profiles
            .iter()
            .find(|p| p.name == "Default (release)")
            .unwrap();
        assert_eq!(release.browser, CookiesBrowser::Zen);
        assert!(release.default);
        assert!(release.path.ends_with("Profiles/abc.Default (release)"));
        assert!(!profiles.iter().find(|p| p.name == "Old").unwrap().default);
        assert!(release.cookies_arg().starts_with("firefox:"));
    }

    #[test]
    fn falls_back_to_the_profile_marked_default() {
        let folder = tempfile::tempdir().unwrap();
        let firefox = folder.path().join("Firefox");
        write(
            &firefox.join("profiles.ini"),
            "[Profile0]\nName=a\nPath=Profiles/a\n[Profile1]\nName=b\nPath=Profiles/b\nDefault=1\n",
        );
        let profiles = firefox_profiles(CookiesBrowser::Firefox, &firefox);
        assert_eq!(
            profiles
                .iter()
                .map(|p| (p.name.as_str(), p.default))
                .collect::<Vec<_>>(),
            [("a", false), ("b", true)]
        );
    }

    #[test]
    fn finds_chromium_profiles_and_the_last_used() {
        let folder = tempfile::tempdir().unwrap();
        let roots = roots(folder.path());
        let user_data = roots.local.join("Microsoft/Edge/User Data");
        write(
            &user_data.join("Local State"),
            r#"{"profile":{"last_used":"Profile 1","info_cache":{"Default":{"name":"Personal"},"Profile 1":{"name":"Work"}}}}"#,
        );
        touch(&user_data.join("Default/Network/Cookies"));
        touch(&user_data.join("Profile 1/Network/Cookies"));
        touch(
            &roots
                .roaming
                .join("Opera Software/Opera Stable/Default/Network/Cookies"),
        );

        let profiles = find_profiles(&roots);
        let names: Vec<_> = profiles
            .iter()
            .map(|p| (p.browser, p.name.as_str(), p.default))
            .collect();
        assert_eq!(
            names,
            [
                (CookiesBrowser::Edge, "Personal", false),
                (CookiesBrowser::Edge, "Work", true),
                (CookiesBrowser::Opera, "Default", true),
            ]
        );
        assert_eq!(
            profiles[1].cookies_arg(),
            format!("edge:{}", user_data.join("Profile 1").display())
        );
    }

    fn profile(browser: CookiesBrowser, name: &str, default: bool) -> BrowserProfile {
        BrowserProfile {
            browser,
            name: name.to_owned(),
            path: PathBuf::from(name),
            default,
        }
    }

    fn cookie(host: &str, name: &str, last_used: i64) -> CookieUse {
        CookieUse {
            host: host.to_owned(),
            name: name.to_owned(),
            last_used,
        }
    }

    /// Reads made-up cookies: profile "a" visited Instagram signed out lately, "b" signed in earlier.
    fn read(profile: &BrowserProfile, _: &[String]) -> io::Result<Vec<CookieUse>> {
        match profile.name.as_str() {
            "a" => Ok(vec![
                cookie(".instagram.com", "csrftoken", 300),
                cookie(".example.com", "id", 50),
            ]),
            "b" => Ok(vec![
                cookie(".instagram.com", "sessionid", 200),
                cookie("example.com", "id", 40),
            ]),
            _ => Err(io::Error::other("locked")),
        }
    }

    #[test]
    fn chooses_the_profile_signed_in_to_the_site() {
        let profiles = [
            profile(CookiesBrowser::Chrome, "locked", true),
            profile(CookiesBrowser::Firefox, "a", true),
            profile(CookiesBrowser::Zen, "b", false),
        ];
        let chosen =
            |only, link| choose_profile(&profiles, only, link, read).map(|p| p.name.as_str());
        assert_eq!(chosen(None, "https://www.instagram.com/p/1/"), Some("b"));
        // Unknown sites go by the freshest cookie of any name.
        assert_eq!(chosen(None, "https://www.example.com/v/1"), Some("a"));
        // Signed in nowhere: no cookies, unless a browser was picked, then its default profile.
        assert_eq!(chosen(None, "https://x.com/a/status/1"), None);
        assert_eq!(
            chosen(Some(CookiesBrowser::Firefox), "https://x.com/a/status/1"),
            Some("a")
        );
        assert_eq!(
            chosen(Some(CookiesBrowser::Firefox), "https://instagram.com/p/1/"),
            Some("a")
        );
        assert_eq!(
            chosen(Some(CookiesBrowser::Brave), "https://instagram.com/p/1/"),
            None
        );
    }

    #[test]
    fn reads_cookie_names_from_a_copy_of_the_database() {
        let folder = tempfile::tempdir().unwrap();
        let firefox = profile(CookiesBrowser::Firefox, "ff", true);
        let firefox = BrowserProfile {
            path: folder.path().join("ff"),
            ..firefox
        };
        fs::create_dir_all(&firefox.path).unwrap();
        let connection = Connection::open(firefox.path.join("cookies.sqlite")).unwrap();
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;
                 CREATE TABLE moz_cookies (host TEXT, name TEXT, value TEXT, lastAccessed INTEGER);
                 INSERT INTO moz_cookies VALUES ('.instagram.com', 'sessionid', 'secret', 1700000000000000);
                 INSERT INTO moz_cookies VALUES ('.youtube.com', 'LOGIN_INFO', 'secret', 1600000000000000);",
            )
            .unwrap();
        // The connection stays open, so the rows are still in the write-ahead log.
        let uses = cookie_uses(&firefox, &["instagram.com".to_owned()]).unwrap();
        assert_eq!(uses, [cookie(".instagram.com", "sessionid", 1_700_000_000)]);
        drop(connection);

        let chrome = BrowserProfile {
            browser: CookiesBrowser::Chrome,
            path: folder.path().join("chrome"),
            ..firefox
        };
        let database = chrome.path.join("Network/Cookies");
        fs::create_dir_all(database.parent().unwrap()).unwrap();
        Connection::open(&database)
            .unwrap()
            .execute_batch(&format!(
                "CREATE TABLE cookies (host_key TEXT, name TEXT, encrypted_value BLOB, last_access_utc INTEGER);
                 INSERT INTO cookies VALUES ('.x.com', 'auth_token', x'00', {});",
                (1_700_000_000 + CHROMIUM_EPOCH_OFFSET_SECONDS) * 1_000_000
            ))
            .unwrap();
        let uses = cookie_uses(&chrome, &["x.com".to_owned()]).unwrap();
        assert_eq!(uses, [cookie(".x.com", "auth_token", 1_700_000_000)]);
    }
}
