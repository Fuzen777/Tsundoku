//! Folder rules: where a download goes, picked from what it is ("when the site is Instagram and the uploader is
//! natgeo, save in D:\Hydrus\natgeo"). The first rule that's on and matches decides; with none, the preset's own
//! folder, then the download folder. Kept in `settings.json`, edited in the Rules view.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::hosts::Host;
use crate::metadata::VideoInfo;
use crate::presets::Preset;
use crate::settings::{SettingsError, SettingsStore};
use crate::sites;

/// The ID of the rule every install starts with: posts of images go in a folder for their site. It's always the last
/// rule, so every other rule comes before it, and it can be edited or turned off but not deleted.
pub const POSTS_RULE_ID: &str = "posts";

/// Replaced in a rule's folder by the download's site, such as "Instagram".
const SITE_PLACEHOLDER: &str = "site";
/// Replaced in a rule's folder by the download's uploader or account.
const UPLOADER_PLACEHOLDER: &str = "uploader";

/// When every condition matches a download, it goes in `folder`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderRule {
    /// Stays the same when the rule is edited or moved: "posts" for the rule every install starts with, "rule-<n>"
    /// for the others. Empty for a rule not saved yet; saving gives it one.
    pub id: String,
    /// A rule that's off is kept but skipped.
    pub enabled: bool,
    /// All must match. A rule without any matches every download.
    pub conditions: Vec<RuleCondition>,
    /// Where matching downloads go. `{site}` and `{uploader}` become the download's, and a folder that isn't a full
    /// path is inside the download folder.
    pub folder: String,
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
    /// A rule without a folder. Rules are numbered from 1.
    #[error("rule {rule} has no folder")]
    EmptyFolder { rule: u32 },
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
            list: vec![posts_rule(None)],
        }
    }
}

/// The rules with the posts rule last, where it always is. A list without it (it could be deleted in 0.1.11 to
/// 0.1.18) gets it back turned off, so where downloads go doesn't change.
pub fn with_posts_rule_last(mut rules: Vec<FolderRule>) -> Vec<FolderRule> {
    let posts = match rules.iter().position(|rule| rule.id == POSTS_RULE_ID) {
        Some(index) => rules.remove(index),
        None => FolderRule {
            enabled: false,
            ..posts_rule(None)
        },
    };
    rules.push(posts);
    rules
}

/// The rule every install starts with: posts of images go in a folder for their site ("Instagram", "X"), inside
/// `folder`, or inside the download folder when it's `None`.
pub fn posts_rule(folder: Option<&str>) -> FolderRule {
    let site = format!("{{{SITE_PLACEHOLDER}}}");
    FolderRule {
        id: POSTS_RULE_ID.to_owned(),
        enabled: true,
        conditions: vec![RuleCondition {
            field: RuleField::Kind,
            operator: RuleOperator::Is,
            value: DownloadKind::Post.as_str().to_owned(),
        }],
        folder: match folder {
            Some(folder) => Path::new(folder).join(site).display().to_string(),
            None => site,
        },
    }
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
        let site = sites::sign_in_site(&self.video.url)
            .map(|site| site.name)
            .or_else(|| Host::of(&self.video.url).map(Host::name))
            .unwrap_or(self.video.site.as_str());
        safe_folder_name(site, "Other")
    }

    fn uploader_folder_name(&self) -> String {
        safe_folder_name(
            self.video.uploader.as_deref().unwrap_or_default(),
            "Unknown",
        )
    }
}

impl FolderRule {
    /// Whether every condition matches the download.
    pub fn matches(&self, subject: &RuleSubject<'_>) -> bool {
        self.conditions
            .iter()
            .all(|condition| condition.matches(subject))
    }

    /// Checks the rule can be saved: a folder, and a value in every condition. `number` counts from 1.
    fn validate(&self, number: u32) -> Result<(), RuleError> {
        if self.folder.trim().is_empty() {
            return Err(RuleError::EmptyFolder { rule: number });
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

/// Where a download goes: the folder of the first rule that's on and matches, else the preset's own folder, else the
/// download folder.
pub fn destination(
    rules: &[FolderRule],
    subject: &RuleSubject<'_>,
    download_folder: &Path,
) -> Destination {
    let matched = rules
        .iter()
        .enumerate()
        .find(|(_, rule)| rule.enabled && rule.matches(subject));
    let (folder, source) = match (matched, subject.preset.and_then(Preset::own_folder)) {
        (Some((index, rule)), _) => (
            rule_folder(&rule.folder, subject, download_folder),
            DestinationSource::Rule {
                number: u32::try_from(index + 1).unwrap_or(u32::MAX),
            },
        ),
        (None, Some(folder)) => (PathBuf::from(folder), DestinationSource::Preset),
        (None, None) => (
            download_folder.to_path_buf(),
            DestinationSource::DownloadFolder,
        ),
    };
    Destination {
        folder: folder.display().to_string(),
        source,
    }
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

    /// Replaces every rule, in this order but with the posts rule last, after checking each. Folders and values are
    /// trimmed, and a rule not saved before (an empty ID) gets one.
    pub fn save(&self, rules: Vec<FolderRule>) -> Result<Vec<FolderRule>, RuleError> {
        let mut rules = with_posts_rule_last(rules);
        for (index, rule) in rules.iter_mut().enumerate() {
            rule.folder = rule.folder.trim().to_owned();
            for condition in &mut rule.conditions {
                condition.value = condition.value.trim().to_owned();
            }
            rule.validate(u32::try_from(index + 1).unwrap_or(u32::MAX))?;
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
        }
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
            "the posts rule"
        );
        let rules = [rule(Vec::new(), r"Saved\{site}")];
        assert_eq!(
            folder_of(&rules, &tweet, None).folder,
            r"D:\Tsundoku\Saved\X"
        );
    }

    #[test]
    fn the_posts_rule_can_start_in_a_folder() {
        assert_eq!(posts_rule(Some(r"D:\Posts")).folder, r"D:\Posts\{site}");
        assert_eq!(posts_rule(None).folder, "{site}");
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
            vec![posts_rule(None)],
            "every install starts with the posts rule"
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
            ["rule-1", "rule-2", "posts"],
            "the posts rule stays last"
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
    fn keeps_the_posts_rule_last_and_never_loses_it() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        let mut first = rule(Vec::new(), r"D:\First");
        first.id = "rule-1".to_owned();
        let moved = manager.save(vec![posts_rule(None), first.clone()]).unwrap();
        assert_eq!(moved, vec![first.clone(), posts_rule(None)]);

        let without = manager.save(vec![first.clone()]).unwrap();
        assert_eq!(
            without,
            vec![
                first,
                FolderRule {
                    enabled: false,
                    ..posts_rule(None)
                }
            ],
            "it comes back, turned off"
        );
    }

    #[test]
    fn refuses_a_rule_without_a_folder_or_with_an_empty_condition() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        let no_folder = vec![posts_rule(None), rule(Vec::new(), "  ")];
        assert_eq!(
            manager.save(no_folder),
            Err(RuleError::EmptyFolder { rule: 1 }),
            "numbered as saved, with the posts rule last"
        );
        let empty = vec![rule(
            vec![condition(RuleField::Title, RuleOperator::Contains, " ")],
            r"D:\x",
        )];
        assert_eq!(manager.save(empty), Err(RuleError::EmptyValue { rule: 1 }));
        assert_eq!(manager.list(), vec![posts_rule(None)], "nothing saved");
    }
}
