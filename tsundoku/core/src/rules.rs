//! Rules: how a download goes, picked from what it is ("when the site is Instagram and the uploader is natgeo, save in
//! D:\Hydrus\natgeo"; "when the site is SoundCloud, start as Audio MP3"). A rule does up to three things: picks the
//! folder, the preset a video starts with, and whether a copied or sent link downloads right away. For each, the first
//! rule that's on, does it and matches decides, so one rule can pick the preset and a later one the folder. With no
//! rule picking the folder, the preset's own folder, then the catch-all (the last rule, which takes everything else),
//! then the download folder; with none picking the preset, the one picked by hand or the default; with none picking
//! Download Right Away, Settings. Kept in `settings.json`, edited in the Rules view.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::hosts::Host;
use crate::metadata::VideoInfo;
use crate::presets::Preset;
use crate::settings::{SettingsError, SettingsStore};
use crate::sites;

/// The ID of the catch-all, the rule every install starts with: every download the other rules don't take goes in a
/// folder for its site. It's always the last rule, has no conditions, and comes after a preset's own folder. It can be
/// edited or turned off but not deleted.
pub const CATCH_ALL_RULE_ID: &str = "catch-all";

/// The catch-all's ID from 0.1.11 to 0.1.29, when it was the posts rule and took only posts of images.
const POSTS_RULE_ID: &str = "posts";

/// Replaced in a rule's folder by the download's site, such as "Instagram".
const SITE_PLACEHOLDER: &str = "site";
/// Replaced in a rule's folder by the download's uploader or account.
const UPLOADER_PLACEHOLDER: &str = "uploader";

/// When every condition matches a download, it goes in `folder`, starts with `preset_id` and downloads right away or
/// not, each when set. A rule sets at least one; the catch-all only its folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderRule {
    /// Stays the same when the rule is edited or moved: "catch-all" for the rule every install starts with,
    /// "rule-<n>" for the others. Empty for a rule not saved yet; saving gives it one.
    pub id: String,
    /// A rule that's off is kept but skipped.
    pub enabled: bool,
    /// All must match. A rule without any matches every download.
    pub conditions: Vec<RuleCondition>,
    /// Where matching downloads go, or empty to leave the folder to the rules after it. `{site}` and `{uploader}`
    /// become the download's, and a folder that isn't a full path is inside the download folder. Empty rather than
    /// missing, so a version from before 0.1.35 still reads the rule.
    pub folder: String,
    /// The preset matching videos start with, by its ID, or `None` to leave it to the rules after it. A rule that
    /// checks the kind or the preset can't pick one (`can_pick_preset`), since both depend on the preset.
    #[serde(default)]
    pub preset_id: Option<String>,
    /// Whether matching links that Download Right Away covers (copied, or sent with the clipboard shortcut or from a
    /// browser) start once read, or `None` to leave it to the rules after it, then Settings.
    #[serde(default)]
    pub download_right_away: Option<bool>,
}

/// One test of a download: its `field` compared to `value`, ignoring case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuleCondition {
    pub field: RuleField,
    pub operator: RuleOperator,
    /// For `Kind`, a `DownloadKind` ("post"); for `Preset`, a preset's ID.
    pub value: String,
}

/// What a condition looks at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RuleField {
    /// The site, by any of its names: "YouTube", "youtube" or "youtube.com".
    Site,
    /// The channel or account.
    Uploader,
    Title,
    /// The link to the video or post.
    Link,
    /// Video, audio, a post of images or a file host's files (`DownloadKind`).
    Kind,
    /// The preset, by its ID or its name. A post has none.
    Preset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RuleOperator {
    Is,
    IsNot,
    Contains,
    DoesNotContain,
}

/// What a download is: a video or audio by its preset, a post of images, or a file host's files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DownloadKind {
    Video,
    Audio,
    Post,
    File,
}

impl DownloadKind {
    /// As a condition's value holds it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Post => "post",
            Self::File => "file",
        }
    }
}

/// The preset a rule picks for a video, and which rule, for the preview card and Pending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RulePreset {
    pub preset_id: String,
    /// The rule, numbered from 1 as the Rules view lists them.
    pub rule: u32,
}

/// Where a download goes, and what decided it, for the preview card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Destination {
    pub folder: String,
    pub source: DestinationSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum DestinationSource {
    /// A rule, numbered from 1 as the Rules view lists them.
    Rule { number: u32 },
    /// The preset's own folder.
    Preset,
    /// The download folder (Settings → Downloads).
    DownloadFolder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum RuleError {
    /// The catch-all without a folder. Rules are numbered from 1.
    #[error("rule {rule} has no folder")]
    EmptyFolder { rule: u32 },
    /// A rule that picks neither a folder, a preset nor whether to download right away.
    #[error("rule {rule} does nothing")]
    NoAction { rule: u32 },
    /// A rule that picks the preset but checks the kind or the preset, which both depend on it.
    #[error("rule {rule} picks the preset but checks the kind or the preset")]
    CircularPreset { rule: u32 },
    /// A condition with nothing to compare to.
    #[error("a condition of rule {rule} is empty")]
    EmptyValue { rule: u32 },
    #[error("couldn't save: {detail}")]
    Storage { detail: String },
}

impl From<SettingsError> for RuleError {
    fn from(error: SettingsError) -> Self {
        Self::Storage {
            detail: error.to_string(),
        }
    }
}

/// The rules in `settings.json`, in the order they're tried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RulesSettings {
    pub list: Vec<FolderRule>,
}

impl Default for RulesSettings {
    fn default() -> Self {
        Self {
            list: vec![catch_all_rule()],
        }
    }
}

/// The rules with the catch-all last, where it always is, without conditions, since it takes everything, and picking
/// only a folder. The posts rule of 0.1.11 to 0.1.29 ("kind is post → {site}") becomes it, keeping its folder and
/// whether it's on. A list without either (the posts rule could be deleted in 0.1.11 to 0.1.18) gets it back turned
/// off, so where downloads go doesn't change.
pub fn with_catch_all_last(rules: Vec<FolderRule>) -> Vec<FolderRule> {
    let (found, mut rules): (Vec<_>, Vec<_>) = rules
        .into_iter()
        .partition(|rule| rule.id == CATCH_ALL_RULE_ID || rule.id == POSTS_RULE_ID);
    let catch_all = match found.into_iter().next() {
        Some(rule) => FolderRule {
            id: CATCH_ALL_RULE_ID.to_owned(),
            conditions: Vec::new(),
            preset_id: None,
            download_right_away: None,
            ..rule
        },
        None => FolderRule {
            enabled: false,
            ..catch_all_rule()
        },
    };
    rules.push(catch_all);
    rules
}

/// The rule every install starts with: every download no other rule takes, and whose preset has no folder of its
/// own, goes in a folder for its site ("Instagram", "YouTube") inside the download folder.
pub fn catch_all_rule() -> FolderRule {
    FolderRule {
        id: CATCH_ALL_RULE_ID.to_owned(),
        enabled: true,
        conditions: Vec::new(),
        folder: format!("{{{SITE_PLACEHOLDER}}}"),
        preset_id: None,
        download_right_away: None,
    }
}

/// The posts folder of 0.1.8 to 0.1.10 as a rule: posts of images go in a folder for their site inside `folder`.
pub fn posts_folder_rule(folder: &str) -> FolderRule {
    FolderRule {
        id: "rule-1".to_owned(),
        enabled: true,
        conditions: vec![RuleCondition {
            field: RuleField::Kind,
            operator: RuleOperator::Is,
            value: DownloadKind::Post.as_str().to_owned(),
        }],
        folder: Path::new(folder)
            .join(format!("{{{SITE_PLACEHOLDER}}}"))
            .display()
            .to_string(),
        preset_id: None,
        download_right_away: None,
    }
}

/// The site a download is from, as `{site}` gives it: its sign-in name ("X"), else the file host's ("MEGA"), else
/// the tool's name for it ("tumblr"), made safe as a folder's or file's name.
pub fn site_name(video: &VideoInfo) -> String {
    let site = sites::sign_in_site(&video.url)
        .map(|site| site.name)
        .or_else(|| Host::of(&video.url).map(Host::name))
        .unwrap_or(video.site.as_str());
    safe_folder_name(site, "Other")
}

/// A download about to be queued, as rules see it.
pub struct RuleSubject<'a> {
    pub video: &'a VideoInfo,
    /// `None` for a post, which has no preset.
    pub preset: Option<&'a Preset>,
}

impl RuleSubject<'_> {
    pub fn kind(&self) -> DownloadKind {
        if self.video.post.is_some() {
            DownloadKind::Post
        } else if self.video.files.is_some() {
            DownloadKind::File
        } else if self.preset.is_some_and(Preset::is_audio) {
            DownloadKind::Audio
        } else {
            DownloadKind::Video
        }
    }

    /// What a condition on `field` compares to: every name the download's value goes by.
    fn values(&self, field: RuleField) -> Vec<&str> {
        let video = self.video;
        match field {
            RuleField::Site => {
                let mut names = vec![video.site.as_str(), video.site_name.as_str()];
                names.extend(sites::sign_in_site(&video.url).map(|site| site.name));
                names.extend(Host::of(&video.url).map(Host::name));
                names
            }
            RuleField::Uploader => video.uploader.as_deref().into_iter().collect(),
            RuleField::Title => vec![video.title.as_str()],
            RuleField::Link => vec![video.url.as_str()],
            RuleField::Kind => vec![self.kind().as_str()],
            RuleField::Preset => self
                .preset
                .map(|preset| vec![preset.id.as_str(), preset.name.as_str()])
                .unwrap_or_default(),
        }
    }

    /// The site as a folder's name: the name people know for sites that need an account ("Instagram", "X") and
    /// for file hosts ("MEGA"), otherwise the tools' own ("danbooru").
    fn site_folder_name(&self) -> String {
        site_name(self.video)
    }

    fn uploader_folder_name(&self) -> String {
        safe_folder_name(
            self.video.uploader.as_deref().unwrap_or_default(),
            "Unknown",
        )
    }
}

impl FolderRule {
    /// Whether it's the catch-all, which comes after a preset's own folder.
    pub fn is_catch_all(&self) -> bool {
        self.id == CATCH_ALL_RULE_ID
    }

    /// Whether every condition matches the download.
    pub fn matches(&self, subject: &RuleSubject<'_>) -> bool {
        self.conditions
            .iter()
            .all(|condition| condition.matches(subject))
    }

    /// Whether it picks where matching downloads go.
    pub fn picks_folder(&self) -> bool {
        !self.folder.trim().is_empty()
    }

    /// Whether it may pick the preset: not when it checks the kind (video or audio) or the preset, which both depend
    /// on the preset it would pick.
    pub fn can_pick_preset(&self) -> bool {
        !self
            .conditions
            .iter()
            .any(|condition| matches!(condition.field, RuleField::Kind | RuleField::Preset))
    }

    /// Checks the rule can be saved: it does something (the catch-all picks a folder), doesn't pick the preset while
    /// checking it, and every condition has a value. `number` counts from 1.
    fn validate(&self, number: u32) -> Result<(), RuleError> {
        if self.is_catch_all() && !self.picks_folder() {
            return Err(RuleError::EmptyFolder { rule: number });
        }
        if !self.picks_folder() && self.preset_id.is_none() && self.download_right_away.is_none() {
            return Err(RuleError::NoAction { rule: number });
        }
        if self.preset_id.is_some() && !self.can_pick_preset() {
            return Err(RuleError::CircularPreset { rule: number });
        }
        if self
            .conditions
            .iter()
            .any(|condition| condition.value.trim().is_empty())
        {
            return Err(RuleError::EmptyValue { rule: number });
        }
        Ok(())
    }
}

impl RuleCondition {
    pub fn matches(&self, subject: &RuleSubject<'_>) -> bool {
        let wanted = self.value.trim().to_lowercase();
        let values: Vec<String> = subject
            .values(self.field)
            .into_iter()
            .map(str::to_lowercase)
            .collect();
        let is = values.contains(&wanted);
        let contains = values.iter().any(|value| value.contains(&wanted));
        match self.operator {
            RuleOperator::Is => is,
            RuleOperator::IsNot => !is,
            RuleOperator::Contains => contains,
            RuleOperator::DoesNotContain => !contains,
        }
    }
}

/// Where a download goes: the folder of the first rule that's on, picks one and matches, else the preset's own folder,
/// else the catch-all's when it's on, else the download folder.
pub fn destination(
    rules: &[FolderRule],
    subject: &RuleSubject<'_>,
    download_folder: &Path,
) -> Destination {
    let rule_folder_of = |catch_all: bool| {
        rules
            .iter()
            .enumerate()
            .find(|(_, rule)| {
                rule.enabled
                    && rule.is_catch_all() == catch_all
                    && rule.picks_folder()
                    && rule.matches(subject)
            })
            .map(|(index, rule)| {
                (
                    rule_folder(&rule.folder, subject, download_folder),
                    DestinationSource::Rule {
                        number: rule_number(index),
                    },
                )
            })
    };
    let preset_folder = || {
        subject
            .preset
            .and_then(Preset::own_folder)
            .map(|folder| (PathBuf::from(folder), DestinationSource::Preset))
    };
    let (folder, source) = rule_folder_of(false)
        .or_else(preset_folder)
        .or_else(|| rule_folder_of(true))
        .unwrap_or_else(|| {
            (
                download_folder.to_path_buf(),
                DestinationSource::DownloadFolder,
            )
        });
    Destination {
        folder: folder.display().to_string(),
        source,
    }
}

/// The preset a video starts with by the rules: that of the first rule that's on, picks a preset that still exists
/// (`exists`), may pick one and matches. `None` when no rule picks one, and for a post or a file host's files, which
/// have no preset.
pub fn start_preset(
    rules: &[FolderRule],
    video: &VideoInfo,
    exists: impl Fn(&str) -> bool,
) -> Option<RulePreset> {
    if video.post.is_some() || video.files.is_some() {
        return None;
    }
    let subject = RuleSubject {
        video,
        preset: None,
    };
    rules.iter().enumerate().find_map(|(index, rule)| {
        let preset_id = rule.preset_id.as_deref()?;
        (rule.enabled && rule.can_pick_preset() && exists(preset_id) && rule.matches(&subject))
            .then(|| RulePreset {
                preset_id: preset_id.to_owned(),
                rule: rule_number(index),
            })
    })
}

/// Whether a link that Download Right Away covers starts once read, by the first rule that's on, says so and matches.
/// `None` leaves it to Settings.
pub fn download_right_away(rules: &[FolderRule], subject: &RuleSubject<'_>) -> Option<bool> {
    rules
        .iter()
        .filter(|rule| rule.enabled)
        .find_map(|rule| rule.download_right_away.filter(|_| rule.matches(subject)))
}

/// A rule's number, from 1, as the Rules view lists them.
fn rule_number(index: usize) -> u32 {
    u32::try_from(index + 1).unwrap_or(u32::MAX)
}

/// A rule's folder for one download: its placeholders filled in, inside the download folder unless it's a full
/// path.
fn rule_folder(folder: &str, subject: &RuleSubject<'_>, download_folder: &Path) -> PathBuf {
    let folder = PathBuf::from(fill_placeholders(folder.trim(), subject));
    if folder.is_absolute() {
        folder
    } else {
        download_folder.join(folder)
    }
}

/// `{site}` and `{uploader}`, in any case, replaced by the download's, each made safe as a folder's name. Other text
/// in braces stays as it is.
fn fill_placeholders(folder: &str, subject: &RuleSubject<'_>) -> String {
    let mut filled = String::with_capacity(folder.len());
    let mut rest = folder;
    while let Some(start) = rest.find('{') {
        filled.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find('}') else {
            break;
        };
        let name = &rest[1..end];
        if name.eq_ignore_ascii_case(SITE_PLACEHOLDER) {
            filled.push_str(&subject.site_folder_name());
        } else if name.eq_ignore_ascii_case(UPLOADER_PLACEHOLDER) {
            filled.push_str(&subject.uploader_folder_name());
        } else {
            filled.push_str(&rest[..=end]);
        }
        rest = &rest[end + 1..];
    }
    filled.push_str(rest);
    filled
}

/// `name` made safe as one folder's name: characters Windows refuses in names, separators included, become "_", and
/// dots and spaces at its ends go. `fallback` when nothing is left.
pub fn safe_folder_name(name: &str, fallback: &str) -> String {
    let name: String = name
        .chars()
        .map(|character| {
            if character.is_control() || r#"<>:"/\|?*"#.contains(character) {
                '_'
            } else {
                character
            }
        })
        .collect();
    let name = name.trim_matches(['.', ' ']);
    if name.is_empty() {
        fallback.to_owned()
    } else {
        name.to_owned()
    }
}

/// The rules, saved to `settings.json` as they change.
pub struct RuleManager {
    settings: Arc<SettingsStore>,
}

impl RuleManager {
    pub fn new(settings: Arc<SettingsStore>) -> Self {
        Self { settings }
    }

    pub fn list(&self) -> Vec<FolderRule> {
        self.settings.get().rules.list
    }

    /// The preset a rule picks for `video`, if any, among those for which `exists` holds (`start_preset`).
    pub fn start_preset(
        &self,
        video: &VideoInfo,
        exists: impl Fn(&str) -> bool,
    ) -> Option<RulePreset> {
        start_preset(&self.list(), video, exists)
    }

    /// Replaces every rule, in this order but with the catch-all last, after checking each. Folders, presets and
    /// values are trimmed (a blank preset is none), and a rule not saved before (an empty ID) gets one.
    pub fn save(&self, rules: Vec<FolderRule>) -> Result<Vec<FolderRule>, RuleError> {
        let mut rules = with_catch_all_last(rules);
        for (index, rule) in rules.iter_mut().enumerate() {
            rule.folder = rule.folder.trim().to_owned();
            rule.preset_id = rule
                .preset_id
                .take()
                .map(|id| id.trim().to_owned())
                .filter(|id| !id.is_empty());
            for condition in &mut rule.conditions {
                condition.value = condition.value.trim().to_owned();
            }
            rule.validate(rule_number(index))?;
        }
        let mut last_number = rules
            .iter()
            .filter_map(|rule| rule.id.strip_prefix("rule-")?.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        for rule in &mut rules {
            if rule.id.trim().is_empty() {
                last_number += 1;
                rule.id = format!("rule-{last_number}");
            }
        }
        self.settings
            .update(|settings| settings.rules.list = rules.clone())?;
        Ok(rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{FileContents, PostContents};
    use crate::presets::built_in_presets;

    fn video() -> VideoInfo {
        VideoInfo {
            site: "youtube".to_owned(),
            site_name: "youtube.com".to_owned(),
            video_id: "jNQXAC9IVRw".to_owned(),
            url: "https://www.youtube.com/watch?v=jNQXAC9IVRw".to_owned(),
            title: "Me at the zoo".to_owned(),
            uploader: Some("jawed".to_owned()),
            duration_seconds: Some(19.0),
            upload_date: None,
            thumbnail_url: None,
            best_height: Some(240),
            post: None,
            files: None,
        }
    }

    fn tweet() -> VideoInfo {
        VideoInfo {
            site: "twitter".to_owned(),
            site_name: "x.com".to_owned(),
            video_id: "1".to_owned(),
            url: "https://x.com/NASA/status/1".to_owned(),
            title: "A photo".to_owned(),
            uploader: Some("NASA".to_owned()),
            post: Some(PostContents {
                images: 1,
                videos: 0,
            }),
            ..video()
        }
    }

    fn mega_file() -> VideoInfo {
        VideoInfo {
            site: "mega".to_owned(),
            site_name: "mega.nz".to_owned(),
            video_id: "AbCdEf12".to_owned(),
            url: "https://mega.nz/file/AbCdEf12#key".to_owned(),
            title: "Pack.zip".to_owned(),
            uploader: None,
            duration_seconds: None,
            best_height: None,
            files: Some(FileContents {
                count: 1,
                bytes: 2048,
            }),
            ..video()
        }
    }

    fn preset(id: &str) -> Preset {
        built_in_presets()
            .into_iter()
            .find(|preset| preset.id == id)
            .unwrap()
    }

    fn condition(field: RuleField, operator: RuleOperator, value: &str) -> RuleCondition {
        RuleCondition {
            field,
            operator,
            value: value.to_owned(),
        }
    }

    fn rule(conditions: Vec<RuleCondition>, folder: &str) -> FolderRule {
        FolderRule {
            id: String::new(),
            enabled: true,
            conditions,
            folder: folder.to_owned(),
            preset_id: None,
            download_right_away: None,
        }
    }

    /// A rule that starts matching videos with `preset_id` and picks no folder.
    fn preset_rule(conditions: Vec<RuleCondition>, preset_id: &str) -> FolderRule {
        FolderRule {
            preset_id: Some(preset_id.to_owned()),
            ..rule(conditions, "")
        }
    }

    fn soundcloud_track() -> VideoInfo {
        VideoInfo {
            site: "soundcloud".to_owned(),
            site_name: "soundcloud.com".to_owned(),
            video_id: "123".to_owned(),
            url: "https://soundcloud.com/artist/track".to_owned(),
            title: "A track".to_owned(),
            uploader: Some("artist".to_owned()),
            best_height: None,
            ..video()
        }
    }

    fn any_preset(_: &str) -> bool {
        true
    }

    const DOWNLOADS: &str = r"D:\Tsundoku";

    fn folder_of(rules: &[FolderRule], video: &VideoInfo, preset: Option<&Preset>) -> Destination {
        destination(rules, &RuleSubject { video, preset }, Path::new(DOWNLOADS))
    }

    #[test]
    fn matches_a_site_by_any_of_its_names() {
        let tweet = tweet();
        let subject = RuleSubject {
            video: &tweet,
            preset: None,
        };
        for name in ["X", "twitter", "x.com", "TWITTER"] {
            assert!(
                condition(RuleField::Site, RuleOperator::Is, name).matches(&subject),
                "{name}"
            );
        }
        assert!(!condition(RuleField::Site, RuleOperator::Is, "instagram").matches(&subject));
        assert!(condition(RuleField::Site, RuleOperator::IsNot, "instagram").matches(&subject));
        assert!(condition(RuleField::Site, RuleOperator::Contains, "twit").matches(&subject));
    }

    #[test]
    fn knows_a_file_host_by_the_name_people_use() {
        let file = mega_file();
        let subject = RuleSubject {
            video: &file,
            preset: None,
        };
        for name in ["MEGA", "mega", "mega.nz"] {
            assert!(
                condition(RuleField::Site, RuleOperator::Is, name).matches(&subject),
                "{name}"
            );
        }
        let rules = [rule(Vec::new(), r"D:\Hosts\{site}")];
        assert_eq!(folder_of(&rules, &file, None).folder, r"D:\Hosts\MEGA");
    }

    #[test]
    fn compares_text_ignoring_case_and_spaces_around() {
        let video = video();
        let subject = RuleSubject {
            video: &video,
            preset: None,
        };
        assert!(condition(RuleField::Title, RuleOperator::Contains, " ZOO ").matches(&subject));
        assert!(
            !condition(RuleField::Title, RuleOperator::DoesNotContain, "zoo").matches(&subject)
        );
        assert!(condition(RuleField::Title, RuleOperator::DoesNotContain, "cat").matches(&subject));
        assert!(condition(RuleField::Uploader, RuleOperator::Is, "Jawed").matches(&subject));
        assert!(condition(RuleField::Link, RuleOperator::Contains, "watch?v=").matches(&subject));
    }

    #[test]
    fn a_missing_uploader_is_never_equal_and_never_contains() {
        let mut video = video();
        video.uploader = None;
        let subject = RuleSubject {
            video: &video,
            preset: None,
        };
        assert!(!condition(RuleField::Uploader, RuleOperator::Contains, "a").matches(&subject));
        assert!(condition(RuleField::Uploader, RuleOperator::IsNot, "a").matches(&subject));
    }

    #[test]
    fn tells_videos_audio_and_posts_apart() {
        let video = video();
        let tweet = tweet();
        let best_audio = preset("best-audio");
        let best_video = preset("best-video");
        let kind =
            |video: &VideoInfo, preset: Option<&Preset>| RuleSubject { video, preset }.kind();
        assert_eq!(kind(&video, Some(&best_video)), DownloadKind::Video);
        assert_eq!(kind(&video, Some(&best_audio)), DownloadKind::Audio);
        assert_eq!(kind(&tweet, None), DownloadKind::Post);
        assert_eq!(kind(&mega_file(), None), DownloadKind::File);
        assert!(
            condition(RuleField::Kind, RuleOperator::Is, "file").matches(&RuleSubject {
                video: &mega_file(),
                preset: None
            })
        );

        let is_audio = condition(RuleField::Kind, RuleOperator::Is, "audio");
        assert!(is_audio.matches(&RuleSubject {
            video: &video,
            preset: Some(&best_audio)
        }));
        let by_name = condition(RuleField::Preset, RuleOperator::Is, "best audio");
        let by_id = condition(RuleField::Preset, RuleOperator::Is, "best-audio");
        for preset_condition in [by_name, by_id] {
            assert!(preset_condition.matches(&RuleSubject {
                video: &video,
                preset: Some(&best_audio)
            }));
            assert!(!preset_condition.matches(&RuleSubject {
                video: &tweet,
                preset: None
            }));
        }
    }

    #[test]
    fn the_first_rule_that_is_on_and_matches_decides() {
        let video = video();
        let youtube = || vec![condition(RuleField::Site, RuleOperator::Is, "YouTube")];
        let mut off = rule(youtube(), r"D:\Off");
        off.enabled = false;
        let rules = vec![
            off,
            rule(
                vec![condition(RuleField::Title, RuleOperator::Contains, "cat")],
                r"D:\Cats",
            ),
            rule(youtube(), r"D:\YouTube"),
            rule(youtube(), r"D:\Later"),
        ];
        assert_eq!(
            folder_of(&rules, &video, None),
            Destination {
                folder: r"D:\YouTube".to_owned(),
                source: DestinationSource::Rule { number: 3 },
            }
        );
    }

    #[test]
    fn needs_every_condition_and_matches_all_without_any() {
        let video = video();
        let both = rule(
            vec![
                condition(RuleField::Site, RuleOperator::Is, "youtube"),
                condition(RuleField::Uploader, RuleOperator::Is, "someone else"),
            ],
            r"D:\Both",
        );
        assert!(!both.matches(&RuleSubject {
            video: &video,
            preset: None
        }));
        assert!(rule(Vec::new(), r"D:\All").matches(&RuleSubject {
            video: &video,
            preset: None
        }));
    }

    #[test]
    fn falls_back_to_the_preset_folder_then_the_download_folder() {
        let video = video();
        let mut mine = preset("best-video");
        assert_eq!(
            folder_of(&[], &video, Some(&mine)),
            Destination {
                folder: DOWNLOADS.to_owned(),
                source: DestinationSource::DownloadFolder,
            }
        );
        mine.folder = Some(r"D:\Videos".to_owned());
        assert_eq!(
            folder_of(&[], &video, Some(&mine)),
            Destination {
                folder: r"D:\Videos".to_owned(),
                source: DestinationSource::Preset,
            }
        );
        let any = rule(Vec::new(), r"D:\Rule");
        assert_eq!(
            folder_of(&[any], &video, Some(&mine)).folder,
            r"D:\Rule",
            "a rule beats the preset"
        );
    }

    #[test]
    fn fills_in_the_site_and_uploader() {
        let tweet = tweet();
        let rules = [rule(Vec::new(), r"D:\Hydrus\{site}\{Uploader}\{other}")];
        assert_eq!(
            folder_of(&rules, &tweet, None).folder,
            r"D:\Hydrus\X\NASA\{other}"
        );

        let mut odd = video();
        odd.site = "a:b/c.".to_owned();
        odd.url = "https://example.org/1".to_owned();
        odd.uploader = Some("..".to_owned());
        assert_eq!(
            folder_of(&rules, &odd, None).folder,
            r"D:\Hydrus\a_b_c\Unknown\{other}"
        );
        odd.site = String::new();
        assert!(
            folder_of(&rules, &odd, None)
                .folder
                .starts_with(r"D:\Hydrus\Other\")
        );

        let unclosed = [rule(Vec::new(), r"D:\{site")];
        assert_eq!(folder_of(&unclosed, &tweet, None).folder, r"D:\{site");
    }

    #[test]
    fn a_folder_that_isnt_a_full_path_is_inside_the_download_folder() {
        let tweet = tweet();
        assert_eq!(
            folder_of(&RulesSettings::default().list, &tweet, None).folder,
            r"D:\Tsundoku\X",
            "the catch-all"
        );
        let rules = [rule(Vec::new(), r"Saved\{site}")];
        assert_eq!(
            folder_of(&rules, &tweet, None).folder,
            r"D:\Tsundoku\Saved\X"
        );
    }

    #[test]
    fn the_catch_all_comes_after_the_other_rules_and_the_presets_folder() {
        let video = video();
        let mut mine = preset("best-video");
        let rules = vec![
            rule(
                vec![condition(RuleField::Kind, RuleOperator::Is, "post")],
                r"D:\Posts",
            ),
            catch_all_rule(),
        ];
        assert_eq!(
            folder_of(&rules, &video, Some(&mine)),
            Destination {
                folder: r"D:\Tsundoku\YouTube".to_owned(),
                source: DestinationSource::Rule { number: 2 },
            },
            "a video no rule takes"
        );
        assert_eq!(folder_of(&rules, &tweet(), None).folder, r"D:\Posts");
        assert_eq!(
            folder_of(&rules, &mega_file(), None).folder,
            r"D:\Tsundoku\MEGA"
        );

        mine.folder = Some(r"D:\Videos".to_owned());
        assert_eq!(
            folder_of(&rules, &video, Some(&mine)).source,
            DestinationSource::Preset,
            "a preset's own folder beats the catch-all"
        );

        mine.folder = None;
        let off = vec![FolderRule {
            enabled: false,
            ..catch_all_rule()
        }];
        assert_eq!(
            folder_of(&off, &video, Some(&mine)).source,
            DestinationSource::DownloadFolder
        );
    }

    #[test]
    fn a_rule_without_a_folder_leaves_it_to_the_next() {
        let track = soundcloud_track();
        let audio = preset("audio-mp3");
        let soundcloud = || vec![condition(RuleField::Site, RuleOperator::Is, "SoundCloud")];
        let rules = vec![
            preset_rule(soundcloud(), "audio-mp3"),
            rule(soundcloud(), r"D:\Music"),
            catch_all_rule(),
        ];
        assert_eq!(
            folder_of(&rules, &track, Some(&audio)),
            Destination {
                folder: r"D:\Music".to_owned(),
                source: DestinationSource::Rule { number: 2 },
            }
        );
        let only_preset = vec![preset_rule(soundcloud(), "audio-mp3"), catch_all_rule()];
        assert_eq!(
            folder_of(&only_preset, &track, Some(&audio)).folder,
            r"D:\Tsundoku\soundcloud",
            "the catch-all, as with no rule"
        );
    }

    #[test]
    fn the_first_rule_that_picks_a_preset_and_matches_decides() {
        let track = soundcloud_track();
        let mut off = preset_rule(Vec::new(), "best-video");
        off.enabled = false;
        let rules = vec![
            off,
            rule(Vec::new(), r"D:\All"),
            preset_rule(
                vec![condition(RuleField::Site, RuleOperator::Is, "YouTube")],
                "1080p-mp4",
            ),
            preset_rule(
                vec![condition(
                    RuleField::Site,
                    RuleOperator::Is,
                    "soundcloud.com",
                )],
                "audio-mp3",
            ),
            preset_rule(Vec::new(), "best-audio"),
            catch_all_rule(),
        ];
        assert_eq!(
            start_preset(&rules, &track, any_preset),
            Some(RulePreset {
                preset_id: "audio-mp3".to_owned(),
                rule: 4,
            })
        );
        assert_eq!(
            start_preset(&rules, &video(), any_preset).map(|pick| pick.rule),
            Some(3)
        );
        assert_eq!(
            start_preset(&[rule(Vec::new(), r"D:\All")], &track, any_preset),
            None,
            "no rule picks one"
        );
    }

    #[test]
    fn posts_and_shared_files_get_no_preset() {
        let rules = vec![preset_rule(Vec::new(), "best-video")];
        assert_eq!(start_preset(&rules, &tweet(), any_preset), None);
        assert_eq!(start_preset(&rules, &mega_file(), any_preset), None);
    }

    #[test]
    fn a_deleted_preset_is_skipped() {
        let rules = vec![
            preset_rule(Vec::new(), "custom-1"),
            preset_rule(Vec::new(), "audio-mp3"),
        ];
        let exists = |id: &str| id != "custom-1";
        assert_eq!(
            start_preset(&rules, &soundcloud_track(), exists).map(|pick| pick.preset_id),
            Some("audio-mp3".to_owned())
        );
        assert_eq!(
            start_preset(&rules[..1], &soundcloud_track(), exists),
            None,
            "the default, then"
        );
    }

    #[test]
    fn a_rule_that_checks_the_kind_or_the_preset_cant_pick_the_preset() {
        let video = video();
        for field in [RuleField::Kind, RuleField::Preset] {
            let value = if field == RuleField::Kind {
                "video"
            } else {
                "best-video"
            };
            let circular = preset_rule(
                vec![condition(field, RuleOperator::IsNot, value)],
                "audio-mp3",
            );
            assert!(!circular.can_pick_preset());
            assert_eq!(start_preset(&[circular], &video, any_preset), None);
        }
        let site = preset_rule(
            vec![condition(RuleField::Site, RuleOperator::Is, "youtube")],
            "audio-mp3",
        );
        assert!(site.can_pick_preset());
    }

    #[test]
    fn the_first_rule_that_says_decides_download_right_away() {
        let track = soundcloud_track();
        let audio = preset("audio-mp3");
        let subject = RuleSubject {
            video: &track,
            preset: Some(&audio),
        };
        let right_away = |on: bool, conditions: Vec<RuleCondition>| FolderRule {
            download_right_away: Some(on),
            ..rule(conditions, "")
        };
        let mut off = right_away(false, Vec::new());
        off.enabled = false;
        let rules = vec![
            off,
            rule(Vec::new(), r"D:\All"),
            right_away(
                true,
                vec![condition(RuleField::Kind, RuleOperator::Is, "audio")],
            ),
            right_away(false, Vec::new()),
        ];
        assert_eq!(download_right_away(&rules, &subject), Some(true));
        assert_eq!(
            download_right_away(
                &rules,
                &RuleSubject {
                    video: &track,
                    preset: Some(&preset("best-video")),
                }
            ),
            Some(false)
        );
        assert_eq!(download_right_away(&rules[..2], &subject), None, "Settings");
    }

    #[test]
    fn rules_from_before_actions_read_as_folder_rules() {
        let saved = r#"{ "id": "rule-1", "enabled": true, "folder": "D:\\Music",
            "conditions": [{ "field": "site", "operator": "is", "value": "SoundCloud" }] }"#;
        let read: FolderRule = serde_json::from_str(saved).unwrap();
        assert_eq!(
            read,
            FolderRule {
                id: "rule-1".to_owned(),
                ..rule(
                    vec![condition(RuleField::Site, RuleOperator::Is, "SoundCloud")],
                    r"D:\Music"
                )
            }
        );
    }

    #[test]
    fn the_posts_folder_becomes_a_rule_for_posts() {
        let posts = posts_folder_rule(r"D:\Posts");
        assert_eq!(posts.folder, r"D:\Posts\{site}");
        assert!(posts.matches(&RuleSubject {
            video: &tweet(),
            preset: None
        }));
        assert!(!posts.matches(&RuleSubject {
            video: &mega_file(),
            preset: None
        }));
    }

    fn manager_in(folder: &tempfile::TempDir) -> RuleManager {
        RuleManager::new(Arc::new(
            SettingsStore::open(folder.path().join("settings.json")).unwrap(),
        ))
    }

    #[test]
    fn saves_rules_in_order_and_names_new_ones() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        assert_eq!(
            manager.list(),
            vec![catch_all_rule()],
            "every install starts with the catch-all"
        );

        let mut rules = manager.list();
        rules.insert(
            0,
            rule(
                vec![condition(RuleField::Uploader, RuleOperator::Is, " natgeo ")],
                r" D:\NatGeo ",
            ),
        );
        rules.push(rule(Vec::new(), r"D:\Rest"));
        let saved = manager.save(rules).unwrap();
        let ids: Vec<&str> = saved.iter().map(|rule| rule.id.as_str()).collect();
        assert_eq!(
            ids,
            ["rule-1", "rule-2", "catch-all"],
            "the catch-all stays last"
        );
        assert_eq!(saved[0].folder, r"D:\NatGeo");
        assert_eq!(saved[0].conditions[0].value, "natgeo");
        assert_eq!(manager_in(&folder).list(), saved, "kept in settings.json");

        let mut next = saved.clone();
        next.remove(0);
        next.push(rule(Vec::new(), r"D:\More"));
        assert_eq!(manager.save(next).unwrap()[1].id, "rule-3");
    }

    #[test]
    fn keeps_the_catch_all_last_and_never_loses_it() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        let mut first = rule(Vec::new(), r"D:\First");
        first.id = "rule-1".to_owned();
        let moved = manager.save(vec![catch_all_rule(), first.clone()]).unwrap();
        assert_eq!(moved, vec![first.clone(), catch_all_rule()]);

        let without = manager.save(vec![first.clone()]).unwrap();
        assert_eq!(
            without,
            vec![
                first.clone(),
                FolderRule {
                    enabled: false,
                    ..catch_all_rule()
                }
            ],
            "it comes back, turned off"
        );

        let mut with_conditions = catch_all_rule();
        with_conditions.conditions = vec![condition(RuleField::Site, RuleOperator::Is, "X")];
        assert_eq!(
            manager.save(vec![with_conditions]).unwrap(),
            vec![catch_all_rule()],
            "it takes everything"
        );

        let with_actions = FolderRule {
            preset_id: Some("audio-mp3".to_owned()),
            download_right_away: Some(true),
            ..catch_all_rule()
        };
        assert_eq!(
            manager.save(vec![with_actions]).unwrap(),
            vec![catch_all_rule()],
            "it only picks a folder"
        );
    }

    #[test]
    fn the_posts_rule_becomes_the_catch_all() {
        let posts = FolderRule {
            id: "posts".to_owned(),
            enabled: false,
            ..rule(
                vec![condition(RuleField::Kind, RuleOperator::Is, "post")],
                r"D:\Hydrus\{site}",
            )
        };
        let mut first = rule(Vec::new(), r"D:\First");
        first.id = "rule-1".to_owned();
        assert_eq!(
            with_catch_all_last(vec![posts, first.clone()]),
            vec![
                first,
                FolderRule {
                    id: CATCH_ALL_RULE_ID.to_owned(),
                    enabled: false,
                    ..rule(Vec::new(), r"D:\Hydrus\{site}")
                }
            ],
            "it keeps its folder and whether it's on"
        );
    }

    #[test]
    fn refuses_a_rule_that_does_nothing_or_has_an_empty_condition() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        let nothing = vec![catch_all_rule(), rule(Vec::new(), "  ")];
        assert_eq!(
            manager.save(nothing),
            Err(RuleError::NoAction { rule: 1 }),
            "numbered as saved, with the catch-all last"
        );
        let blank_preset = vec![FolderRule {
            preset_id: Some(" ".to_owned()),
            ..rule(Vec::new(), "")
        }];
        assert_eq!(
            manager.save(blank_preset),
            Err(RuleError::NoAction { rule: 1 })
        );
        let catch_all = vec![FolderRule {
            folder: " ".to_owned(),
            ..catch_all_rule()
        }];
        assert_eq!(
            manager.save(catch_all),
            Err(RuleError::EmptyFolder { rule: 1 })
        );
        let empty = vec![rule(
            vec![condition(RuleField::Title, RuleOperator::Contains, " ")],
            r"D:\x",
        )];
        assert_eq!(manager.save(empty), Err(RuleError::EmptyValue { rule: 1 }));
        let circular = vec![preset_rule(
            vec![condition(RuleField::Kind, RuleOperator::Is, "audio")],
            "audio-mp3",
        )];
        assert_eq!(
            manager.save(circular),
            Err(RuleError::CircularPreset { rule: 1 })
        );
        assert_eq!(manager.list(), vec![catch_all_rule()], "nothing saved");
    }

    #[test]
    fn saves_a_rule_that_only_picks_a_preset_or_download_right_away() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        let soundcloud = || vec![condition(RuleField::Site, RuleOperator::Is, "SoundCloud")];
        let saved = manager
            .save(vec![
                preset_rule(soundcloud(), " audio-mp3 "),
                FolderRule {
                    download_right_away: Some(false),
                    ..rule(soundcloud(), " ")
                },
                catch_all_rule(),
            ])
            .unwrap();
        assert_eq!(saved[0].preset_id.as_deref(), Some("audio-mp3"));
        assert_eq!(saved[1].folder, "");
        assert_eq!(saved[1].download_right_away, Some(false));
        assert_eq!(manager_in(&folder).list(), saved, "kept in settings.json");
        assert_eq!(
            manager
                .start_preset(&soundcloud_track(), any_preset)
                .map(|pick| pick.rule),
            Some(1)
        );
    }
}
