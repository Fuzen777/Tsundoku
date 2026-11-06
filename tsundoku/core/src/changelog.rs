//! What's New after an update: the sections of CHANGELOG.md newer than the version Tsundoku last ran, up to the one
//! running. The app crate embeds CHANGELOG.md when it's built, so a copy shows its own changes without the network.

use std::cmp::Ordering;

use serde::Serialize;
use ts_rs::TS;

use crate::app_info::compare_versions;
use crate::settings::{SettingsError, SettingsStore};

/// One version's section of CHANGELOG.md.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChangelogSection {
    /// Such as "0.1.47".
    pub version: String,
    /// As the heading gives it, such as "2026-11-06".
    pub date: Option<String>,
    /// The section's Markdown under its heading: `### Added` and the like, then a list of changes.
    pub body: String,
}

/// Every version's section, in the order the file has them (the newest first). A section starts at a `## <version>`
/// heading, with " · <date>" after the version if it's dated, and runs to the next `## ` heading; anything before the
/// first one (the title and the introduction) is left out, and so is a `## ` heading that doesn't start with a version.
pub fn sections(changelog: &str) -> Vec<ChangelogSection> {
    let mut sections: Vec<ChangelogSection> = Vec::new();
    let mut current: Option<(ChangelogSection, Vec<&str>)> = None;
    for line in changelog.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            if let Some((section, lines)) = current.take() {
                sections.push(with_body(section, &lines));
            }
            current = section_heading(heading).map(|section| (section, Vec::new()));
        } else if let Some((_, lines)) = current.as_mut() {
            lines.push(line);
        }
    }
    if let Some((section, lines)) = current {
        sections.push(with_body(section, &lines));
    }
    sections
}

fn section_heading(heading: &str) -> Option<ChangelogSection> {
    let (version, date) = match heading.split_once('·') {
        Some((version, date)) => (version.trim(), Some(date.trim().to_owned())),
        None => (heading.trim(), None),
    };
    let is_version = version.starts_with(|character: char| character.is_ascii_digit())
        && version.contains('.')
        && !version.contains(char::is_whitespace);
    is_version.then(|| ChangelogSection {
        version: version.to_owned(),
        date: date.filter(|date| !date.is_empty()),
        body: String::new(),
    })
}

fn with_body(section: ChangelogSection, lines: &[&str]) -> ChangelogSection {
    ChangelogSection {
        body: lines.join("\n").trim().to_owned(),
        ..section
    }
}

/// The sections to show on a start of `current` after `last_run`, newest first: those newer than `last_run`, up to
/// `current`. Without a `last_run` (an install from before Tsundoku kept it), just `current`'s own.
pub fn changes_since(
    changelog: &str,
    last_run: Option<&str>,
    current: &str,
) -> Vec<ChangelogSection> {
    let mut shown: Vec<ChangelogSection> = sections(changelog)
        .into_iter()
        .filter(|section| {
            let up_to_current = compare_versions(&section.version, current) != Ordering::Greater;
            match last_run {
                Some(last_run) => {
                    up_to_current
                        && compare_versions(&section.version, last_run) == Ordering::Greater
                }
                None => compare_versions(&section.version, current) == Ordering::Equal,
            }
        })
        .collect();
    shown.sort_by(|left, right| compare_versions(&right.version, &left.version));
    shown
}

/// At startup: a first install (whose tools aren't set up yet) has nothing new to tell, so it records the running
/// version at once, before its setup ends.
pub fn note_start(
    settings: &SettingsStore,
    current: &str,
    first_install: bool,
) -> Result<(), SettingsError> {
    if first_install && settings.get().last_run_version.is_none() {
        record(settings, current)?;
    }
    Ok(())
}

/// What's New for the running `current` version: what changed since the version that ran last. When there's
/// nothing to show (the same version, an older one, or no section for it), `current` is recorded right away;
/// otherwise once the card is dismissed (`dismiss`), so a start in the tray, with the window never opened, doesn't
/// lose it.
pub fn whats_new(
    settings: &SettingsStore,
    changelog: &str,
    current: &str,
) -> Vec<ChangelogSection> {
    let shown = changes_since(
        changelog,
        settings.get().last_run_version.as_deref(),
        current,
    );
    if shown.is_empty() {
        // Best effort: if it can't be saved, the next start works it out again.
        let _ = record(settings, current);
    }
    shown
}

/// The What's New card was dismissed: it doesn't show again until a newer version runs.
pub fn dismiss(settings: &SettingsStore, current: &str) -> Result<(), SettingsError> {
    record(settings, current)
}

fn record(settings: &SettingsStore, current: &str) -> Result<(), SettingsError> {
    if settings.get().last_run_version.as_deref() == Some(current) {
        return Ok(());
    }
    settings.update(|settings| settings.last_run_version = Some(current.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHANGELOG: &str = "# Changelog

All notable changes to Tsundoku. Versions follow [Semantic Versioning](https://semver.org/).

## 0.2.0 · 2026-12-01

### Added

- **Big thing**: it does it.
- **Other thing**: it does it too.

### Fixed

- **A bug**: gone.

## 0.1.10 · 2026-11-07

### Changed

- **Ten**: changed.

## 0.1.9

- **Nine**: no date.

## Unreleased notes

- Not a version.

## 0.1.8 · 2026-11-05
";

    fn versions(sections: &[ChangelogSection]) -> Vec<&str> {
        sections
            .iter()
            .map(|section| section.version.as_str())
            .collect()
    }

    #[test]
    fn reads_each_versions_section() {
        let sections = sections(CHANGELOG);
        assert_eq!(versions(&sections), ["0.2.0", "0.1.10", "0.1.9", "0.1.8"]);
        assert_eq!(
            sections[0],
            ChangelogSection {
                version: "0.2.0".to_owned(),
                date: Some("2026-12-01".to_owned()),
                body: "### Added\n\n- **Big thing**: it does it.\n- **Other thing**: it does it too.\n\n### Fixed\n\n- **A bug**: gone.".to_owned(),
            }
        );
        assert_eq!(sections[2].date, None);
        assert_eq!(
            sections[2].body, "- **Nine**: no date.",
            "a heading that isn't a version ends the section before it"
        );
        assert_eq!(sections[3].body, "");
    }

    #[test]
    fn reads_windows_line_endings() {
        let sections = sections(&CHANGELOG.replace('\n', "\r\n"));
        assert_eq!(sections[1].body, "### Changed\n\n- **Ten**: changed.");
    }

    #[test]
    fn shows_what_changed_after_the_last_run_up_to_this_version() {
        assert_eq!(
            versions(&changes_since(CHANGELOG, Some("0.1.8"), "0.2.0")),
            ["0.2.0", "0.1.10", "0.1.9"]
        );
        assert_eq!(
            versions(&changes_since(CHANGELOG, Some("0.1.9"), "0.1.10")),
            ["0.1.10"],
            "nothing newer than the running version"
        );
        assert!(changes_since(CHANGELOG, Some("0.2.0"), "0.2.0").is_empty());
        assert!(
            changes_since(CHANGELOG, Some("0.2.0"), "0.1.10").is_empty(),
            "an older version tells nothing"
        );
        assert_eq!(
            versions(&changes_since(CHANGELOG, None, "0.1.10")),
            ["0.1.10"],
            "without a last run, only this version's own"
        );
    }

    #[test]
    fn tells_once_and_never_on_a_first_install() {
        let folder = tempfile::tempdir().unwrap();
        let settings = SettingsStore::open(folder.path().join("settings.json")).unwrap();

        note_start(&settings, "0.1.9", true).unwrap();
        assert!(whats_new(&settings, CHANGELOG, "0.1.9").is_empty());
        assert_eq!(settings.get().last_run_version.as_deref(), Some("0.1.9"));

        assert_eq!(
            versions(&whats_new(&settings, CHANGELOG, "0.2.0")),
            ["0.2.0", "0.1.10"]
        );
        assert_eq!(
            versions(&whats_new(&settings, CHANGELOG, "0.2.0")),
            ["0.2.0", "0.1.10"],
            "it stays until it's dismissed"
        );
        dismiss(&settings, "0.2.0").unwrap();
        assert!(whats_new(&settings, CHANGELOG, "0.2.0").is_empty());
        assert_eq!(
            SettingsStore::open(folder.path().join("settings.json"))
                .unwrap()
                .get()
                .last_run_version
                .as_deref(),
            Some("0.2.0")
        );
    }

    #[test]
    fn an_install_from_before_hears_about_this_version() {
        let folder = tempfile::tempdir().unwrap();
        let settings = SettingsStore::open(folder.path().join("settings.json")).unwrap();
        note_start(&settings, "0.1.10", false).unwrap();
        assert_eq!(
            versions(&whats_new(&settings, CHANGELOG, "0.1.10")),
            ["0.1.10"]
        );
    }
}
