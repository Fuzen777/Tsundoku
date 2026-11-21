//! The passwords Tsundoku tries by itself on a link or an archive that needs one, before it asks: Settings →
//! Passwords' list first, in its order, then the ones used before. Those are kept only while Save Past Passwords is
//! on, in `tsundoku.db` (`past_passwords`), apart from the list: a password that opened a link or an archive, with
//! the site it came from and when it last opened something. A link is tried with its own site's only, since trying
//! one sends it to the site; an archive is opened here, so it's tried with every one, its site's first.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;

use super::Password;
use crate::cancel::CancelToken;
use crate::database::Database;
use crate::history::HistoryError;
use crate::metadata::PreviewError;
use crate::settings::SettingsStore;
use crate::sites;
use crate::sync::lock;

/// The most passwords tried by themselves on one link: each try asks its site again, the list's first.
pub const LINK_TRIES_AT_MOST: usize = 20;

/// How many links' passwords are kept in memory for their downloads (`KnownPasswords::keep_for_download`).
const KEPT_FOR_DOWNLOADS: usize = 50;

/// The passwords to try on an archive, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArchivePasswords {
    pub passwords: Vec<String>,
    /// How many of them, from the first, are Settings → Passwords' list.
    pub listed: usize,
}

/// Settings → Passwords' list and the passwords used before.
pub struct KnownPasswords {
    database: Arc<Database>,
    settings: Arc<SettingsStore>,
    /// The password a link the preview card read opened with, by its video's site and ID, for the download added from
    /// the card: in memory only, the latest last.
    for_downloads: Mutex<Vec<(String, String, Password)>>,
}

impl std::fmt::Debug for KnownPasswords {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KnownPasswords")
            .finish_non_exhaustive()
    }
}

impl KnownPasswords {
    pub fn new(database: Arc<Database>, settings: Arc<SettingsStore>) -> Self {
        Self {
            database,
            settings,
            for_downloads: Mutex::new(Vec::new()),
        }
    }

    /// The passwords to try by themselves on `link` when it needs one: the list's, then the ones used before on its
    /// site, the latest first, none twice, `LINK_TRIES_AT_MOST` at most.
    pub fn for_link(&self, link: &str) -> Vec<Password> {
        let mut passwords = self.listed();
        if self.saves_past() {
            let past = self.past("WHERE site = ?1 ORDER BY used_at DESC, rowid DESC", link);
            add_new(&mut passwords, past);
        }
        passwords.truncate(LINK_TRIES_AT_MOST);
        passwords.into_iter().filter_map(Password::new).collect()
    }

    /// The passwords to try on an archive a download from `link` brought: the list's, then the download's `own` (the
    /// one typed for its archives, the one that opened its link), then every one used before, its site's first and
    /// the latest first, none twice.
    pub fn for_archive(&self, link: &str, own: &[&Password]) -> ArchivePasswords {
        let mut passwords = self.listed();
        let listed = passwords.len();
        add_new(
            &mut passwords,
            own.iter().map(|password| password.reveal().to_owned()),
        );
        if self.saves_past() {
            let past = self.past("ORDER BY (site = ?1) DESC, used_at DESC, rowid DESC", link);
            add_new(&mut passwords, past);
        }
        ArchivePasswords { passwords, listed }
    }

    /// Keeps a password that opened `link`, or an archive its download brought, for its site's next ones: only while
    /// Save Past Passwords is on, and never one the list has, which is tried anyway. One kept already is now the
    /// latest used.
    pub fn remember(&self, link: &str, password: &Password) {
        if !self.saves_past()
            || self
                .listed()
                .iter()
                .any(|listed| listed == password.reveal())
        {
            return;
        }
        // Not kept if the database can't be written: it's asked for again.
        let _ = self.database.lock().execute(
            "INSERT INTO past_passwords (site, password, used_at) VALUES (?1, ?2, ?3)
            ON CONFLICT (site, password) DO UPDATE SET used_at = excluded.used_at",
            params![site_key(link), password.reveal(), now_seconds()],
        );
    }

    /// Keeps the password that opened a link on the preview card until its download is added, which takes it
    /// (`kept_for_download`): the card itself never gets a password it didn't type.
    pub fn keep_for_download(&self, site: &str, video_id: &str, password: Password) {
        let mut kept = lock(&self.for_downloads);
        kept.retain(|(kept_site, kept_id, _)| kept_site != site || kept_id != video_id);
        kept.push((site.to_owned(), video_id.to_owned(), password));
        let extra = kept.len().saturating_sub(KEPT_FOR_DOWNLOADS);
        kept.drain(..extra);
    }

    /// The password the preview card's read of a video opened its link with, if it needed one.
    pub fn kept_for_download(&self, site: &str, video_id: &str) -> Option<Password> {
        lock(&self.for_downloads)
            .iter()
            .find(|(kept_site, kept_id, _)| kept_site == site && kept_id == video_id)
            .map(|(_, _, password)| password.clone())
    }

    /// How many passwords used before are kept, one used on several sites counting once.
    pub fn past_count(&self) -> Result<u32, HistoryError> {
        Ok(self.database.lock().query_row(
            "SELECT count(DISTINCT password) FROM past_passwords",
            [],
            |row| row.get(0),
        )?)
    }

    /// Forgets every password used before. The list stays, and so does each download's own.
    pub fn forget_past(&self) -> Result<(), HistoryError> {
        self.database
            .lock()
            .execute("DELETE FROM past_passwords", [])?;
        Ok(())
    }

    fn saves_past(&self) -> bool {
        self.settings.get().downloads.save_past_passwords
    }

    /// Settings → Passwords' list, in its order.
    fn listed(&self) -> Vec<String> {
        self.settings.get().downloads.unpack.passwords
    }

    /// The passwords used before that `filter_and_order` takes, given the site of `link`. None if they can't be read.
    fn past(&self, filter_and_order: &str, link: &str) -> Vec<String> {
        let connection = self.database.lock();
        let read = connection
            .prepare(&format!(
                "SELECT password FROM past_passwords {filter_and_order}"
            ))
            .and_then(|mut statement| {
                statement
                    .query_map([site_key(link)], |row| row.get(0))?
                    .collect::<Result<Vec<String>, _>>()
            });
        read.unwrap_or_default()
    }
}

/// Adds the `passwords` that `known` doesn't have yet, in their order.
fn add_new(known: &mut Vec<String>, passwords: impl IntoIterator<Item = String>) {
    for password in passwords {
        if !password.is_empty() && !known.contains(&password) {
            known.push(password);
        }
    }
}

/// What a link's passwords are kept under: its site (`sites::site_of`), so every address of a site shares them.
fn site_key(link: &str) -> String {
    sites::site_of(link)
        .map(|site| site.key)
        .unwrap_or_default()
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

/// Reads a link that may need a password, trying the known ones by itself before the person is asked: first with its
/// `own` (the one the person typed, or the one kept for the link), then, when it still needs one and the person
/// didn't just type `own`, with each of `others` in turn (`KnownPasswords::for_link`, asked for only then). Answers
/// what was read and the password that opened it. A link none opens is `PasswordNeeded`, `wrong` when the person
/// typed the one tried, with how many were `tried` besides; any other failure of a try ends them, as does `cancel`.
pub fn read_trying<T>(
    own: Option<Password>,
    typed: bool,
    others: impl FnOnce() -> Vec<Password>,
    cancel: &CancelToken,
    mut read: impl FnMut(Option<&Password>) -> Result<T, PreviewError>,
) -> Result<(T, Option<Password>), PreviewError> {
    match read(own.as_ref()) {
        Ok(found) => return Ok((found, own)),
        // The known ones were tried before the person was asked.
        Err(PreviewError::PasswordNeeded { .. }) if typed => {
            return Err(PreviewError::PasswordNeeded {
                wrong: true,
                tried: 0,
            });
        }
        Err(PreviewError::PasswordNeeded { .. }) => {}
        Err(error) => return Err(error),
    }
    let mut tried = 0;
    for password in others() {
        if own.as_ref() == Some(&password) {
            continue;
        }
        if cancel.is_cancelled() {
            return Err(PreviewError::Cancelled);
        }
        match read(Some(&password)) {
            Ok(found) => return Ok((found, Some(password))),
            Err(PreviewError::PasswordNeeded { .. }) => tried += 1,
            Err(error) => return Err(error),
        }
    }
    Err(PreviewError::PasswordNeeded {
        wrong: false,
        tried,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIMEO: &str = "https://vimeo.com/1";
    const OTHER_VIMEO: &str = "https://player.vimeo.com/video/2";
    const ELSEWHERE: &str = "https://example.org/watch/3";

    struct Test {
        _folder: tempfile::TempDir,
        database: Arc<Database>,
        settings: Arc<SettingsStore>,
        known: KnownPasswords,
    }

    impl Test {
        /// With `listed` as Settings' list, and Save Past Passwords as `saves_past`.
        fn new(listed: &[&str], saves_past: bool) -> Self {
            let folder = tempfile::tempdir().unwrap();
            let database = Arc::new(Database::open_in_memory().unwrap());
            let settings =
                Arc::new(SettingsStore::open(folder.path().join("settings.json")).unwrap());
            settings
                .update(|settings| {
                    settings.downloads.unpack.passwords = listed
                        .iter()
                        .map(|password| (*password).to_owned())
                        .collect();
                    settings.downloads.save_past_passwords = saves_past;
                })
                .unwrap();
            let known = KnownPasswords::new(Arc::clone(&database), Arc::clone(&settings));
            Self {
                _folder: folder,
                database,
                settings,
                known,
            }
        }

        /// A password that opened something from `link`, at `used_at`.
        fn used(&self, link: &str, password: &str, used_at: i64) {
            self.known.remember(link, &password_of(password));
            self.database
                .lock()
                .execute(
                    "UPDATE past_passwords SET used_at = ?3 WHERE site = ?1 AND password = ?2",
                    params![site_key(link), password, used_at],
                )
                .unwrap();
        }

        fn for_link(&self, link: &str) -> Vec<String> {
            self.known
                .for_link(link)
                .iter()
                .map(|password| password.reveal().to_owned())
                .collect()
        }
    }

    fn password_of(text: &str) -> Password {
        Password::new(text).unwrap()
    }

    #[test]
    fn a_link_is_tried_with_the_list_then_its_sites_past_passwords() {
        let test = Test::new(&["first", "second"], true);
        test.used(VIMEO, "older", 10);
        test.used(OTHER_VIMEO, "newer", 20);
        test.used(ELSEWHERE, "another site's", 30);
        assert_eq!(
            test.for_link(VIMEO),
            ["first", "second", "newer", "older"],
            "the list in its order, then the site's own, the latest first, whatever the address on it"
        );
        assert_eq!(
            test.for_link(ELSEWHERE),
            ["first", "second", "another site's"],
            "a site is never sent another's"
        );
        assert_eq!(test.known.past_count().unwrap(), 3);
    }

    #[test]
    fn past_passwords_are_kept_only_while_asked_for_and_never_the_lists() {
        let test = Test::new(&["listed"], false);
        test.known.remember(VIMEO, &password_of("typed"));
        assert_eq!(test.known.past_count().unwrap(), 0, "off to start with");
        assert_eq!(test.for_link(VIMEO), ["listed"]);

        test.settings
            .update(|settings| settings.downloads.save_past_passwords = true)
            .unwrap();
        test.known.remember(VIMEO, &password_of("typed"));
        test.known.remember(VIMEO, &password_of("typed"));
        test.known.remember(VIMEO, &password_of("listed"));
        assert_eq!(
            test.known.past_count().unwrap(),
            1,
            "once, and the list's isn't one"
        );
        assert_eq!(test.for_link(VIMEO), ["listed", "typed"]);

        // Turned off again: kept, but neither tried nor added to.
        test.settings
            .update(|settings| settings.downloads.save_past_passwords = false)
            .unwrap();
        test.known.remember(VIMEO, &password_of("later"));
        assert_eq!(test.for_link(VIMEO), ["listed"]);
        assert_eq!(test.known.past_count().unwrap(), 1);

        test.known.forget_past().unwrap();
        assert_eq!(test.known.past_count().unwrap(), 0);
        assert_eq!(
            test.settings.get().downloads.unpack.passwords,
            ["listed"],
            "the list stays"
        );
    }

    #[test]
    fn a_link_is_tried_with_so_many_at_most_the_lists_first() {
        let listed: Vec<String> = (1..=LINK_TRIES_AT_MOST - 1)
            .map(|number| format!("listed {number}"))
            .collect();
        let listed: Vec<&str> = listed.iter().map(String::as_str).collect();
        let test = Test::new(&listed, true);
        test.used(VIMEO, "latest", 20);
        test.used(VIMEO, "older", 10);
        let tried = test.for_link(VIMEO);
        assert_eq!(tried.len(), LINK_TRIES_AT_MOST);
        assert_eq!(tried[..listed.len()], listed[..]);
        assert_eq!(tried.last().map(String::as_str), Some("latest"));
    }

    #[test]
    fn an_archive_is_tried_with_the_list_its_downloads_own_then_every_past_one() {
        let test = Test::new(&["listed"], true);
        test.used(ELSEWHERE, "another site's", 30);
        test.used(VIMEO, "this site's", 10);
        let typed = password_of("typed for it");
        let links = password_of("listed");
        let tried = test.known.for_archive(VIMEO, &[&typed, &links]);
        assert_eq!(
            tried,
            ArchivePasswords {
                passwords: ["listed", "typed for it", "this site's", "another site's"]
                    .map(str::to_owned)
                    .to_vec(),
                listed: 1,
            },
            "none twice, and its own site's before the others'"
        );

        let test = Test::new(&[], false);
        test.used(VIMEO, "never kept", 10);
        assert_eq!(
            test.known.for_archive(VIMEO, &[&typed]),
            ArchivePasswords {
                passwords: vec!["typed for it".to_owned()],
                listed: 0,
            }
        );
    }

    #[test]
    fn the_cards_password_is_kept_for_its_download() {
        let test = Test::new(&[], false);
        assert_eq!(test.known.kept_for_download("vimeo", "1"), None);
        test.known
            .keep_for_download("vimeo", "1", password_of("old"));
        test.known
            .keep_for_download("vimeo", "1", password_of("new"));
        assert_eq!(
            test.known.kept_for_download("vimeo", "1"),
            Some(password_of("new"))
        );
        for number in 0..KEPT_FOR_DOWNLOADS {
            test.known
                .keep_for_download("vimeo", &format!("more {number}"), password_of("x"));
        }
        assert_eq!(
            test.known.kept_for_download("vimeo", "1"),
            None,
            "only the latest are kept"
        );
        assert_eq!(test.known.past_count().unwrap(), 0, "in memory only");
    }

    /// Reads a link that opens with `opens_with` (`None` for one that needs no password), noting each password tried.
    fn reading<'a>(
        opens_with: Option<&'a str>,
        tried: &'a Mutex<Vec<Option<String>>>,
    ) -> impl FnMut(Option<&Password>) -> Result<&'static str, PreviewError> + 'a {
        move |password| {
            let password = password.map(|password| password.reveal().to_owned());
            lock(tried).push(password.clone());
            if opens_with.is_none() || password.as_deref() == opens_with {
                Ok("read")
            } else {
                Err(PreviewError::PasswordNeeded {
                    wrong: password.is_some(),
                    tried: 0,
                })
            }
        }
    }

    fn others(passwords: &[&str]) -> impl FnOnce() -> Vec<Password> {
        let passwords: Vec<Password> = passwords.iter().map(|text| password_of(text)).collect();
        move || passwords
    }

    #[test]
    fn a_link_that_needs_no_password_is_read_once() {
        let tried = Mutex::new(Vec::new());
        let read = read_trying(
            None,
            false,
            || panic!("the known passwords aren't asked for"),
            &CancelToken::new(),
            reading(None, &tried),
        );
        assert_eq!(read, Ok(("read", None)));
        assert_eq!(*lock(&tried), [None]);
    }

    #[test]
    fn the_known_passwords_are_tried_in_order_until_one_opens_the_link() {
        let tried = Mutex::new(Vec::new());
        let read = read_trying(
            None,
            false,
            others(&["one", "two", "three"]),
            &CancelToken::new(),
            reading(Some("two"), &tried),
        );
        assert_eq!(read, Ok(("read", Some(password_of("two")))));
        assert_eq!(
            *lock(&tried),
            [None, Some("one".to_owned()), Some("two".to_owned())],
            "and no further"
        );
    }

    #[test]
    fn a_link_none_opens_asks_saying_how_many_were_tried() {
        let tried = Mutex::new(Vec::new());
        // The one kept for the link stopped working: the others are tried, but not it again.
        let read = read_trying(
            Some(password_of("kept")),
            false,
            others(&["one", "kept", "two"]),
            &CancelToken::new(),
            reading(Some("something else"), &tried),
        );
        assert_eq!(
            read,
            Err(PreviewError::PasswordNeeded {
                wrong: false,
                tried: 2
            })
        );
        assert_eq!(
            *lock(&tried),
            [
                Some("kept".to_owned()),
                Some("one".to_owned()),
                Some("two".to_owned())
            ]
        );
    }

    #[test]
    fn a_typed_password_that_is_wrong_says_so_without_trying_the_others_again() {
        let tried = Mutex::new(Vec::new());
        let read = read_trying(
            Some(password_of("typo")),
            true,
            || panic!("they were tried before asking"),
            &CancelToken::new(),
            reading(Some("right"), &tried),
        );
        assert_eq!(
            read,
            Err(PreviewError::PasswordNeeded {
                wrong: true,
                tried: 0
            })
        );
        assert_eq!(*lock(&tried), [Some("typo".to_owned())]);
    }

    #[test]
    fn another_failure_or_a_cancel_ends_the_tries() {
        let mut reads = 0;
        let read: Result<((), _), _> = read_trying(
            None,
            false,
            others(&["one", "two"]),
            &CancelToken::new(),
            |password| {
                reads += 1;
                Err(match password {
                    None => PreviewError::PasswordNeeded {
                        wrong: false,
                        tried: 0,
                    },
                    Some(_) => PreviewError::too_many_requests("429".to_owned()),
                })
            },
        );
        assert_eq!(
            read,
            Err(PreviewError::too_many_requests("429".to_owned())),
            "the site's answer, not a question"
        );
        assert_eq!(reads, 2);

        let cancel = CancelToken::new();
        let tried = Mutex::new(Vec::new());
        let mut read = reading(Some("two"), &tried);
        let stopped = read_trying(None, false, others(&["one", "two"]), &cancel, |password| {
            cancel.cancel();
            read(password)
        });
        assert_eq!(stopped, Err(PreviewError::Cancelled));
        assert_eq!(*lock(&tried), [None]);
    }
}
