//! Rules: how a download goes, picked from what it is ("when the site is Instagram and the uploader is natgeo, save in
//! D:\Hydrus\natgeo"; "when the site is SoundCloud, start as Audio MP3"). A rule does up to three things: picks the
//! folder, the preset a video starts with, and whether a copied or sent link downloads right away. For each, the last
//! rule that's on, does it and matches decides (the bottom-most in the Rules view), so one rule can pick the preset and
//! an earlier one the folder. With no rule picking the folder, the preset's own folder, then the catch-all (the first
//! rule, which takes everything else), then the download folder; with none picking the preset, the one picked by hand or the default; with none picking
//! Download Right Away, Settings. Kept in `settings.json`, edited in the Rules view.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::hosts::Host;
use crate::metadata::VideoInfo;
use crate::presets::Preset;
use crate::settings::{SettingsError, SettingsStore};
use crate::sites;

/// The ID of the catch-all, the rule every install starts with: every download the other rules don't take goes in a
/// folder for its site. It's always the first rule, so every other rule overrides it, has no conditions, and comes
/// after a preset's own folder. It can be edited or turned off but not deleted.
pub const CATCH_ALL_RULE_ID: &str = "catch-all";

/// The catch-all's ID from 0.1.11 to 0.1.29, when it was the posts rule and took only posts of images.
const POSTS_RULE_ID: &str = "posts";

/// Replaced in a rule's folder by the download's site, such as "Instagram". The others are in `placeholder_value`.
const SITE_PLACEHOLDER: &str = "site";

/// Characters Windows refuses in a file name, besides control characters; `\` and `/` would also make folders.
const FILE_NAME_FORBIDDEN: &str = r#"<>:"/\|?*"#;

/// A post's file's place in it, in a rule's file name: kept as it is by `file_name_on`, for the runner to fill in.
pub const NUMBER_PLACEHOLDER: &str = "{number}";

/// The most characters a placeholder fills in, so a long title doesn't make a path longer than Windows takes.
const LONGEST_PLACEHOLDER: usize = 80;

/// When every condition matches a download, it goes in `folder`, starts with `preset_id` and downloads right away or
/// not, each when set. A rule sets at least one; the catch-all only its folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderRule {
    /// Stays the same when the rule is edited or moved: "catch-all" for the rule every install starts with,
    /// "rule-<n>" for the others. Empty for a rule not saved yet; saving gives it one.
    pub id: String,
    /// What the UI calls it instead of "Rule 3", or empty for its number. Trimmed when saved; missing in rules saved
    /// before 0.1.91.
    #[serde(default)]
    pub name: String,
    /// A rule that's off is kept but skipped.
    pub enabled: bool,
    /// Each joined to the one before by its own `join`: conditions joined by OR are alternatives, and AND joins those
    /// groups, so "A OR B AND C" is (A or B) and C. A rule without any matches every download.
    pub conditions: Vec<RuleCondition>,
    /// How 0.1.89 and 0.1.90 joined every condition of the rule, read only: `with_catch_all_first` moves an OR onto
    /// each condition, and it's never written again.
    #[serde(default, skip_serializing)]
    #[ts(skip)]
    pub join: ConditionJoin,
    /// Where matching downloads go, or empty to leave the folder to the rules before it. `{site}` and `{uploader}`
    /// become the download's, and a folder that isn't a full path is inside the download folder. Empty rather than
    /// missing, so a version from before 0.1.35 still reads the rule.
    pub folder: String,
    /// How matching downloads' files are named, with the folder's placeholders (and `{number}`, a post's file's place
    /// in it), or empty to leave it to the rules before it, then the preset (a video) or Settings (a post's files).
    /// See `file_name_on`. Missing in rules saved before 0.1.96.
    #[serde(default)]
    pub file_name: String,
    /// The preset matching videos start with, by its ID, or `None` to leave it to the rules before it. A rule that
    /// checks the kind or the preset can't pick one (`can_pick_preset`), since both depend on the preset.
    #[serde(default)]
    pub preset_id: Option<String>,
    /// Whether matching links that Download Right Away covers (copied, or sent with the clipboard shortcut or from a
    /// browser) start once read, or `None` to leave it to the rules before it, then Settings.
    #[serde(default)]
    pub download_right_away: Option<bool>,
}

/// How a condition joins the one before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ConditionJoin {
    /// AND: both must match, "when the site is X AND the uploader is natgeo".
    #[default]
    All,
    /// OR: either is enough, "when the site is X OR the site is Bluesky".
    Any,
}

/// One test of a download: its `field` compared to `value`, ignoring case unless `case_sensitive`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuleCondition {
    pub field: RuleField,
    pub operator: RuleOperator,
    /// For `Kind`, a `DownloadKind` ("post"); for `Preset`, a preset's ID.
    pub value: String,
    /// Whether the text, or the pattern, must match letter case too. Off by default, and for conditions saved
    /// before 0.1.90. The kind and the preset ignore it.
    #[serde(default)]
    pub case_sensitive: bool,
    /// How it joins the condition before it (AND unless set, and for conditions saved before 0.1.92); the first
    /// condition's is ignored.
    #[serde(default)]
    pub join: ConditionJoin,
}

/// What a condition looks at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RuleField {
    /// The site, by any of its names: "YouTube", "youtube" or "youtube.com".
    Site,
    /// The channel or account, by its name or its handle ("National Geographic" or "natgeo").
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
    /// The value is a pattern (a regular expression), found anywhere in the text, ignoring case. The groups it
    /// captures can go in the rule's folder as `{1}` to `{9}`.
    Matches,
    DoesNotMatch,
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
    /// As `{kind}` puts it in a folder: "Videos", "Audio", "Posts" or "Files".
    pub fn folder_name(self) -> &'static str {
        match self {
            Self::Video => "Videos",
            Self::Audio => "Audio",
            Self::Post => "Posts",
            Self::File => "Files",
        }
    }

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

/// Whether copied and sent links start once read, and the rule that says so, for Rules' Test a Link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RightAwayRule {
    pub download_right_away: bool,
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
    /// A folder picked for this one download, which the rules don't change.
    Chosen,
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
    /// A file name with a character Windows refuses in one, or a separator, which would make a folder.
    #[error("rule {rule}'s file name can't have {character}")]
    FileNameCharacter { rule: u32, character: String },
    /// A condition's pattern that isn't a valid regular expression, and what's wrong with it.
    #[error("a pattern of rule {rule} isn't valid: {detail}")]
    InvalidPattern { rule: u32, detail: String },
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

/// The rules in `settings.json`, in the Rules view's order: the last one that matches decides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RulesSettings {
    pub list: Vec<FolderRule>,
    /// Whether `list` is in the order of 0.1.86 and later, where the last matching rule decides and the catch-all is
    /// first. Missing (false) in a file from before, whose first matching rule decided with the catch-all last: reading
    /// it reverses the list (`Settings::from_json`), so every rule keeps deciding what it did.
    #[serde(default)]
    pub last_wins: bool,
}

impl Default for RulesSettings {
    fn default() -> Self {
        Self {
            list: vec![catch_all_rule()],
            last_wins: true,
        }
    }
}

/// The rules with the catch-all first, where it always is, without conditions, since it takes everything, and picking
/// only a folder. The posts rule of 0.1.11 to 0.1.29 ("kind is post → {site}") becomes it, keeping its folder and
/// whether it's on. A list without either (the posts rule could be deleted in 0.1.11 to 0.1.18) gets it back turned
/// off, so where downloads go doesn't change.
pub fn with_catch_all_first(rules: Vec<FolderRule>) -> Vec<FolderRule> {
    let (found, mut rules): (Vec<_>, Vec<_>) = rules
        .into_iter()
        .map(FolderRule::with_joins_on_conditions)
        .partition(|rule| rule.id == CATCH_ALL_RULE_ID || rule.id == POSTS_RULE_ID);
    let catch_all = match found.into_iter().next() {
        Some(rule) => FolderRule {
            id: CATCH_ALL_RULE_ID.to_owned(),
            conditions: Vec::new(),
            file_name: String::new(),
            preset_id: None,
            download_right_away: None,
            join: ConditionJoin::All,
            ..rule
        },
        None => FolderRule {
            enabled: false,
            ..catch_all_rule()
        },
    };
    rules.insert(0, catch_all);
    rules
}

/// The rule every install starts with: every download no other rule takes, and whose preset has no folder of its
/// own, goes in a folder for its site ("Instagram", "YouTube") inside the download folder.
pub fn catch_all_rule() -> FolderRule {
    FolderRule {
        id: CATCH_ALL_RULE_ID.to_owned(),
        name: String::new(),
        enabled: true,
        conditions: Vec::new(),
        folder: format!("{{{SITE_PLACEHOLDER}}}"),
        file_name: String::new(),
        preset_id: None,
        download_right_away: None,
        join: ConditionJoin::All,
    }
}

/// The posts folder of 0.1.8 to 0.1.10 as a rule: posts of images go in a folder for their site inside `folder`.
pub fn posts_folder_rule(folder: &str) -> FolderRule {
    FolderRule {
        id: "rule-1".to_owned(),
        name: String::new(),
        enabled: true,
        conditions: vec![RuleCondition {
            field: RuleField::Kind,
            operator: RuleOperator::Is,
            value: DownloadKind::Post.as_str().to_owned(),
            case_sensitive: false,
            join: ConditionJoin::All,
        }],
        folder: Path::new(folder)
            .join(format!("{{{SITE_PLACEHOLDER}}}"))
            .display()
            .to_string(),
        file_name: String::new(),
        preset_id: None,
        download_right_away: None,
        join: ConditionJoin::All,
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
            RuleField::Uploader => video
                .uploader
                .iter()
                .chain(&video.account)
                .map(String::as_str)
                .collect(),
            RuleField::Title => vec![video.title.as_str()],
            RuleField::Link => vec![video.url.as_str()],
            RuleField::Kind => vec![self.kind().as_str()],
            RuleField::Preset => self
                .preset
                .map(|preset| vec![preset.id.as_str(), preset.name.as_str()])
                .unwrap_or_default(),
        }
    }
}

impl FolderRule {
    /// Whether it's the catch-all, which comes after a preset's own folder.
    pub fn is_catch_all(&self) -> bool {
        self.id == CATCH_ALL_RULE_ID
    }

    /// Whether the conditions match the download: OR binds first, so the conditions joined by OR form groups of which
    /// one must match, and every group joined by AND must. A rule without conditions matches every download.
    pub fn matches(&self, subject: &RuleSubject<'_>) -> bool {
        let mut group_matched = true;
        for (index, condition) in self.conditions.iter().enumerate() {
            let starts_group = index == 0 || condition.join == ConditionJoin::All;
            if starts_group {
                if !group_matched {
                    return false;
                }
                group_matched = false;
            }
            group_matched = group_matched || condition.matches(subject);
        }
        group_matched
    }

    /// The rule with 0.1.89's rule-wide OR moved onto each condition after the first, as it reads since.
    fn with_joins_on_conditions(mut self) -> Self {
        if self.join == ConditionJoin::Any {
            for condition in self.conditions.iter_mut().skip(1) {
                condition.join = ConditionJoin::Any;
            }
        }
        self.join = ConditionJoin::All;
        self
    }

    /// What `{1}` to `{9}` become in its folder: the groups of its first pattern that captures any, whole match
    /// first. `None` when it has no such pattern, or when that pattern didn't match (one Or left out).
    fn captured(&self, subject: &RuleSubject<'_>) -> Option<Vec<Option<String>>> {
        self.conditions
            .iter()
            .filter_map(|condition| condition.captured(subject))
            .find(|captured| captured.len() > 1)
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
    /// checking it, every condition has a value, and every pattern is valid. `number` counts from 1.
    fn validate(&self, number: u32) -> Result<(), RuleError> {
        if self.is_catch_all() && !self.picks_folder() {
            return Err(RuleError::EmptyFolder { rule: number });
        }
        if !self.picks_folder()
            && self.file_name.trim().is_empty()
            && self.preset_id.is_none()
            && self.download_right_away.is_none()
        {
            return Err(RuleError::NoAction { rule: number });
        }
        if let Some(character) = self
            .file_name
            .chars()
            .find(|character| FILE_NAME_FORBIDDEN.contains(*character) || character.is_control())
        {
            return Err(RuleError::FileNameCharacter {
                rule: number,
                character: character.to_string(),
            });
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
        if let Some(error) = self
            .conditions
            .iter()
            .filter(|condition| condition.is_pattern())
            .find_map(|condition| condition.pattern().err())
        {
            return Err(RuleError::InvalidPattern {
                rule: number,
                detail: pattern_problem(&error),
            });
        }
        Ok(())
    }
}

impl RuleCondition {
    /// Whether any of the field's values passes the comparison. A field without a value (no uploader) passes only
    /// the negative ones. A pattern that isn't valid, which only a settings.json edited by hand can hold, matches
    /// nothing either way.
    pub fn matches(&self, subject: &RuleSubject<'_>) -> bool {
        let found = |test: &dyn Fn(&str) -> bool| subject.values(self.field).into_iter().any(test);
        let case_sensitive = self.minds_case();
        let folded = |text: &str| {
            if case_sensitive {
                text.to_owned()
            } else {
                text.to_lowercase()
            }
        };
        let wanted = folded(self.value.trim());
        let is = |value: &str| folded(value) == wanted;
        let contains = |value: &str| folded(value).contains(&wanted);
        match self.operator {
            RuleOperator::Is => found(&is),
            RuleOperator::IsNot => !found(&is),
            RuleOperator::Contains => found(&contains),
            RuleOperator::DoesNotContain => !found(&contains),
            RuleOperator::Matches => self
                .pattern()
                .is_ok_and(|pattern| found(&|value: &str| pattern.is_match(value))),
            RuleOperator::DoesNotMatch => self
                .pattern()
                .is_ok_and(|pattern| !found(&|value: &str| pattern.is_match(value))),
        }
    }

    /// Whether its value is a pattern rather than text.
    pub fn is_pattern(&self) -> bool {
        matches!(
            self.operator,
            RuleOperator::Matches | RuleOperator::DoesNotMatch
        )
    }

    /// Whether it compares letter case: when it asks to, on a field of text (not the kind or the preset, whose values
    /// are picked from a list).
    fn minds_case(&self) -> bool {
        self.case_sensitive && !matches!(self.field, RuleField::Kind | RuleField::Preset)
    }

    /// Its value as a pattern, ignoring case unless it minds it.
    fn pattern(&self) -> Result<Regex, regex::Error> {
        RegexBuilder::new(self.value.trim())
            .case_insensitive(!self.minds_case())
            .build()
    }

    /// For a Matches condition, the groups its pattern captured in the first of the field's values it's found in,
    /// whole match first (a group that took no part is `None`). `None` otherwise.
    fn captured(&self, subject: &RuleSubject<'_>) -> Option<Vec<Option<String>>> {
        if self.operator != RuleOperator::Matches {
            return None;
        }
        let pattern = self.pattern().ok()?;
        subject.values(self.field).into_iter().find_map(|value| {
            pattern.captures(value).map(|captures| {
                captures
                    .iter()
                    .map(|group| group.map(|group| group.as_str().to_owned()))
                    .collect()
            })
        })
    }
}

/// What's wrong with a pattern, in a line ("unclosed group"), without the copy of the pattern and the caret under it
/// that the regex crate's message starts with.
fn pattern_problem(error: &regex::Error) -> String {
    let message = error.to_string();
    message
        .lines()
        .find_map(|line| line.trim().strip_prefix("error: "))
        .map_or_else(|| message.clone(), str::to_owned)
}

/// Where a download goes: the folder of the last rule that's on, picks one and matches, else the preset's own folder,
/// else the catch-all's when it's on, else the download folder.
pub fn destination(
    rules: &[FolderRule],
    subject: &RuleSubject<'_>,
    download_folder: &Path,
) -> Destination {
    destination_on(rules, subject, download_folder, &local_date())
}

/// `destination` for a download added on `today` (YYYY-MM-DD), which `{added}` becomes.
fn destination_on(
    rules: &[FolderRule],
    subject: &RuleSubject<'_>,
    download_folder: &Path,
    today: &str,
) -> Destination {
    let rule_folder_of = |catch_all: bool| {
        rules
            .iter()
            .enumerate()
            .rev()
            .find(|(_, rule)| {
                rule.enabled
                    && rule.is_catch_all() == catch_all
                    && rule.picks_folder()
                    && rule.matches(subject)
            })
            .map(|(index, rule)| {
                (
                    rule_folder(rule, subject, download_folder, today),
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

/// The preset a video starts with by the rules: that of the last rule that's on, picks a preset that still exists
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
    rules.iter().enumerate().rev().find_map(|(index, rule)| {
        let preset_id = rule.preset_id.as_deref()?;
        (rule.enabled && rule.can_pick_preset() && exists(preset_id) && rule.matches(&subject))
            .then(|| RulePreset {
                preset_id: preset_id.to_owned(),
                rule: rule_number(index),
            })
    })
}

/// Whether a link that Download Right Away covers starts once read, by the last rule that's on, says so and matches.
/// `None` leaves it to Settings.
pub fn download_right_away(rules: &[FolderRule], subject: &RuleSubject<'_>) -> Option<bool> {
    right_away_rule(rules, subject).map(|decided| decided.download_right_away)
}

/// `download_right_away`, with the rule that decides it.
pub fn right_away_rule(rules: &[FolderRule], subject: &RuleSubject<'_>) -> Option<RightAwayRule> {
    rules.iter().enumerate().rev().find_map(|(index, rule)| {
        let download_right_away = rule.download_right_away?;
        (rule.enabled && rule.matches(subject)).then(|| RightAwayRule {
            download_right_away,
            rule: rule_number(index),
        })
    })
}

/// A rule's number, from 1, as the Rules view lists them.
fn rule_number(index: usize) -> u32 {
    u32::try_from(index + 1).unwrap_or(u32::MAX)
}

/// How a download's files are named by the rules: the file name of the last rule that's on, names files and matches,
/// filled in for a download added today, or `None` to leave it to the preset or Settings. A file host's files keep
/// their own names, so they get `None`. See `file_name_on`.
pub fn file_name(rules: &[FolderRule], subject: &RuleSubject<'_>) -> Option<String> {
    file_name_on(rules, subject, &local_date())
}

/// `file_name` for a download added on `today`. The name is its placeholders filled in, with ` {id}` added when it
/// has none and, for a post, ` {number}` too, so two files never share a name. The answer is in gallery-dl's way of
/// writing text: braces from the filled-in values or the rule's own text are doubled, and `{number}` (a post's only)
/// stays as it is; `yt_dlp_output` and `post_names::rule_format` turn it into each tool's.
fn file_name_on(rules: &[FolderRule], subject: &RuleSubject<'_>, today: &str) -> Option<String> {
    if subject.video.files.is_some() {
        return None;
    }
    let rule = rules.iter().rev().find(|rule| {
        rule.enabled
            && !rule.is_catch_all()
            && !rule.file_name.trim().is_empty()
            && rule.matches(subject)
    })?;
    let is_post = subject.video.post.is_some();
    let mut template = rule.file_name.trim().to_owned();
    if split_at_placeholder(&template, "id").len() < 2 {
        template.push_str(" {id}");
    }
    if is_post && split_at_placeholder(&template, "number").len() < 2 {
        template.push(' ');
        template.push_str(NUMBER_PLACEHOLDER);
    }
    let fill_in = FillIn {
        captured: rule.captured(subject),
        today,
    };
    let pieces: Vec<String> = split_at_placeholder(&template, "number")
        .into_iter()
        .map(|piece| {
            fill_placeholders(piece, subject, &fill_in)
                .replace('{', "{{")
                .replace('}', "}}")
        })
        .collect();
    Some(pieces.join(if is_post { NUMBER_PLACEHOLDER } else { "" }))
}

/// `text` cut at each `{name}`, in any case: one piece when it has none.
fn split_at_placeholder<'a>(text: &'a str, name: &str) -> Vec<&'a str> {
    let wanted = format!("{{{name}}}");
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while let Some(found) = text[at..].find('{') {
        let begin = at + found;
        let end = begin + wanted.len();
        if text
            .get(begin..end)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&wanted))
        {
            pieces.push(&text[start..begin]);
            start = end;
            at = end;
        } else {
            at = begin + 1;
        }
    }
    pieces.push(&text[start..]);
    pieces
}

/// A rule's file name (`file_name`) as yt-dlp's output template for a video: its braces back to single, `%` doubled so
/// yt-dlp keeps it as it is, and the file's extension after it.
pub fn yt_dlp_output(name: &str) -> String {
    let text = name
        .replace(NUMBER_PLACEHOLDER, "")
        .replace("{{", "{")
        .replace("}}", "}")
        .replace('%', "%%");
    format!("{}.%(ext)s", text.trim())
}

/// A rule's folder for one download added on `today`: its placeholders filled in, inside the download folder unless
/// it's a full path.
fn rule_folder(
    rule: &FolderRule,
    subject: &RuleSubject<'_>,
    download_folder: &Path,
    today: &str,
) -> PathBuf {
    let fill_in = FillIn {
        captured: rule.captured(subject),
        today,
    };
    let folder = PathBuf::from(fill_placeholders(rule.folder.trim(), subject, &fill_in));
    if folder.is_absolute() {
        folder
    } else {
        download_folder.join(folder)
    }
}

/// A folder with the rules' placeholders filled in for one download added today, inside the download folder unless it's
/// a full path: a watched source's Save In. `{1}` to `{9}` stay as they are, having no pattern.
pub fn filled_folder(template: &str, subject: &RuleSubject<'_>, download_folder: &Path) -> PathBuf {
    let today = local_date();
    let fill_in = FillIn {
        captured: None,
        today: &today,
    };
    let folder = PathBuf::from(fill_placeholders(template.trim(), subject, &fill_in));
    if folder.is_absolute() {
        folder
    } else {
        download_folder.join(folder)
    }
}

/// What a rule's folder takes for one download besides the download itself.
struct FillIn<'a> {
    /// The groups of the rule's first pattern that captures any (`FolderRule::captured`).
    captured: Option<Vec<Option<String>>>,
    /// The local date it's added, as YYYY-MM-DD.
    today: &'a str,
}

/// Every placeholder (`placeholder_value`), in any case, replaced by the download's value, made safe as a folder's
/// name. Other text in braces stays as it is.
fn fill_placeholders(folder: &str, subject: &RuleSubject<'_>, fill_in: &FillIn<'_>) -> String {
    let mut filled = String::with_capacity(folder.len());
    let mut rest = folder;
    while let Some(start) = rest.find('{') {
        filled.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find('}') else {
            break;
        };
        match placeholder_value(&rest[1..end], subject, fill_in) {
            Some(value) => filled.push_str(&value),
            None => filled.push_str(&rest[..=end]),
        }
        rest = &rest[end + 1..];
    }
    filled.push_str(rest);
    filled
}

/// What a placeholder in a rule's folder becomes, made safe as a folder's name, with a word for a value the download
/// doesn't have. `None` for a name that isn't a placeholder, and for `{1}` to `{9}` in a rule without a pattern that
/// captures, which then stay as they are.
fn placeholder_value(
    name: &str,
    subject: &RuleSubject<'_>,
    fill_in: &FillIn<'_>,
) -> Option<String> {
    let video = subject.video;
    let uploaded = video.upload_date.as_deref();
    let kind = subject.kind().folder_name();
    let (value, fallback): (Option<&str>, &str) = match name.to_ascii_lowercase().as_str() {
        SITE_PLACEHOLDER => return Some(site_name(video)),
        "uploader" => (video.uploader.as_deref(), "Unknown"),
        // The name stands in on a site without handles.
        "account" => (
            video.account.as_deref().or(video.uploader.as_deref()),
            "Unknown",
        ),
        // Only a download picked from a playlist, channel or account has one.
        "playlist" => (video.playlist.as_deref(), "No Playlist"),
        "title" => (Some(video.title.as_str()), "Untitled"),
        "id" => (Some(video.video_id.as_str()), "Unknown"),
        "kind" => (Some(kind), kind),
        // A post or a file host's files have no preset.
        "preset" => (subject.preset.map(|preset| preset.name.as_str()), kind),
        "year" => (uploaded.and_then(|date| date.get(..4)), "Unknown"),
        "month" => (uploaded.and_then(|date| date.get(5..7)), "Unknown"),
        "date" => (uploaded, "Unknown"),
        "added" => (Some(fill_in.today), "Unknown"),
        other => {
            let [digit @ b'1'..=b'9'] = other.as_bytes() else {
                return None;
            };
            let captured = fill_in.captured.as_ref()?;
            let group = captured.get(usize::from(digit - b'0')).cloned().flatten();
            return Some(folder_part(group.as_deref().unwrap_or_default(), "Unknown"));
        }
    };
    Some(folder_part(value.unwrap_or_default(), fallback))
}

/// `value` made safe as one folder's name (`safe_folder_name`), cut to `LONGEST_PLACEHOLDER` characters.
fn folder_part(value: &str, fallback: &str) -> String {
    let safe = safe_folder_name(value, fallback);
    if safe.chars().count() <= LONGEST_PLACEHOLDER {
        return safe;
    }
    let cut: String = safe.chars().take(LONGEST_PLACEHOLDER).collect();
    safe_folder_name(&cut, fallback)
}

/// Today's local date, as YYYY-MM-DD, for `{added}`.
#[cfg(windows)]
fn local_date() -> String {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;

    // SAFETY: GetLocalTime only writes the SYSTEMTIME it's given, which lives through the call.
    let time = unsafe {
        let mut time: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut time);
        time
    };
    format!("{:04}-{:02}-{:02}", time.wYear, time.wMonth, time.wDay)
}

/// Tsundoku only runs on Windows; elsewhere, where only its tests build, `{added}` doesn't know the day.
#[cfg(not(windows))]
fn local_date() -> String {
    String::new()
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

    /// Replaces every rule, in this order but with the catch-all first, after checking each. Folders, presets and
    /// values are trimmed (a blank preset is none), and a rule not saved before (an empty ID) gets one.
    pub fn save(&self, rules: Vec<FolderRule>) -> Result<Vec<FolderRule>, RuleError> {
        let mut rules = with_catch_all_first(rules);
        for (index, rule) in rules.iter_mut().enumerate() {
            rule.name = rule.name.trim().to_owned();
            rule.folder = rule.folder.trim().to_owned();
            rule.file_name = rule.file_name.trim().to_owned();
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
        self.settings.update(|settings| {
            settings.rules.list = rules.clone();
            settings.rules.last_wins = true;
        })?;
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
            account: None,
            duration_seconds: Some(19.0),
            upload_date: None,
            thumbnail_url: None,
            best_height: Some(240),
            post: None,
            files: None,
            post_files: Vec::new(),
            playlist: None,
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
            case_sensitive: false,
            join: ConditionJoin::All,
        }
    }

    /// A condition joined to the one before it by OR.
    fn or(condition: RuleCondition) -> RuleCondition {
        RuleCondition {
            join: ConditionJoin::Any,
            ..condition
        }
    }

    fn rule(conditions: Vec<RuleCondition>, folder: &str) -> FolderRule {
        FolderRule {
            id: String::new(),
            name: String::new(),
            enabled: true,
            conditions,
            folder: folder.to_owned(),
            file_name: String::new(),
            preset_id: None,
            download_right_away: None,
            join: ConditionJoin::All,
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
    fn the_last_rule_that_is_on_and_matches_decides() {
        let video = video();
        let youtube = || vec![condition(RuleField::Site, RuleOperator::Is, "YouTube")];
        let mut off = rule(youtube(), r"D:\Off");
        off.enabled = false;
        let rules = vec![
            rule(youtube(), r"D:\Earlier"),
            rule(youtube(), r"D:\YouTube"),
            rule(
                vec![condition(RuleField::Title, RuleOperator::Contains, "cat")],
                r"D:\Cats",
            ),
            off,
        ];
        assert_eq!(
            folder_of(&rules, &video, None),
            Destination {
                folder: r"D:\YouTube".to_owned(),
                source: DestinationSource::Rule { number: 2 },
            }
        );
    }

    #[test]
    fn minds_case_when_asked() {
        let video = video();
        let subject = RuleSubject {
            video: &video,
            preset: None,
        };
        let title = video.title.clone();
        let exact = |operator: RuleOperator, value: &str| RuleCondition {
            case_sensitive: true,
            ..condition(RuleField::Title, operator, value)
        };
        assert!(exact(RuleOperator::Is, &title).matches(&subject));
        assert!(!exact(RuleOperator::Is, &title.to_uppercase()).matches(&subject));
        assert!(
            condition(RuleField::Title, RuleOperator::Is, &title.to_uppercase()).matches(&subject)
        );
        assert!(!exact(RuleOperator::Contains, &title.to_uppercase()).matches(&subject));
        assert!(exact(RuleOperator::DoesNotContain, &title.to_uppercase()).matches(&subject));
        assert!(
            !exact(
                RuleOperator::Matches,
                &format!("^{}$", title.to_uppercase())
            )
            .matches(&subject)
        );
        assert!(exact(RuleOperator::Matches, &format!("^{title}$")).matches(&subject));
        let kind = RuleCondition {
            case_sensitive: true,
            ..condition(RuleField::Kind, RuleOperator::Is, "VIDEO")
        };
        assert!(kind.matches(&subject), "the kind ignores it");

        let saved = r#"{ "field": "title", "operator": "is", "value": "x" }"#;
        assert!(
            !serde_json::from_str::<RuleCondition>(saved)
                .unwrap()
                .case_sensitive,
            "conditions from before ignore case"
        );
    }

    #[test]
    fn names_files_by_the_last_rule_that_does() {
        let video = video();
        let subject = RuleSubject {
            video: &video,
            preset: None,
        };
        let named = |name: &str| FolderRule {
            file_name: name.to_owned(),
            ..rule(Vec::new(), "")
        };
        let today = "2026-11-10";
        assert_eq!(
            file_name_on(&[catch_all_rule()], &subject, today),
            None,
            "no rule names it"
        );
        let rules = vec![
            named("{uploader} - {title}"),
            named("{title} [{id}]"),
            rule(Vec::new(), r"D:\x"),
        ];
        assert_eq!(
            file_name_on(&rules, &subject, today).as_deref(),
            Some("Me at the zoo [jNQXAC9IVRw]"),
            "the last one that names files"
        );
        assert_eq!(
            file_name_on(&rules[..1], &subject, today).as_deref(),
            Some("jawed - Me at the zoo jNQXAC9IVRw"),
            "the ID is added when the name has none"
        );
        assert_eq!(
            file_name_on(&[named("{added} {{x}} 100%")], &subject, today).as_deref(),
            Some("2026-11-10 {{{{x}}}} 100% jNQXAC9IVRw"),
            "braces doubled"
        );
        assert_eq!(
            yt_dlp_output("2026-11-10 {{x}} 100% jNQXAC9IVRw"),
            "2026-11-10 {x} 100%% jNQXAC9IVRw.%(ext)s"
        );
        assert_eq!(
            file_name_on(
                &[named("{title}")],
                &RuleSubject {
                    video: &mega_file(),
                    preset: None
                },
                today
            ),
            None,
            "a file host's files keep their names"
        );
    }

    #[test]
    fn numbers_a_posts_files() {
        let post = tweet();
        let subject = RuleSubject {
            video: &post,
            preset: None,
        };
        let named = |name: &str| FolderRule {
            file_name: name.to_owned(),
            ..rule(Vec::new(), "")
        };
        let today = "2026-11-10";
        let id = &post.video_id;
        assert_eq!(
            file_name_on(&[named("{id} #{NUMBER}")], &subject, today),
            Some(format!("{id} #{{number}}"))
        );
        assert_eq!(
            file_name_on(&[named("{site}")], &subject, today),
            Some(format!("X {id} {{number}}")),
            "the ID and the number are added"
        );
    }

    #[test]
    fn refuses_a_file_name_windows_refuses() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        let named = FolderRule {
            file_name: "{title}: {id}".to_owned(),
            ..rule(Vec::new(), "")
        };
        assert_eq!(
            manager.save(vec![named]),
            Err(RuleError::FileNameCharacter {
                rule: 2,
                character: ":".to_owned()
            })
        );
        let only_named = FolderRule {
            file_name: " {title} ".to_owned(),
            ..rule(Vec::new(), "")
        };
        let saved = manager.save(vec![only_named]).unwrap();
        assert_eq!(
            saved[1].file_name, "{title}",
            "naming files is enough, trimmed"
        );
    }

    #[test]
    fn needs_one_condition_with_or() {
        let video = video();
        let either = rule(
            vec![
                condition(RuleField::Site, RuleOperator::Is, "SoundCloud"),
                or(condition(RuleField::Site, RuleOperator::Is, "YouTube")),
            ],
            r"D:\Either",
        );
        let subject = RuleSubject {
            video: &video,
            preset: None,
        };
        assert!(either.matches(&subject));
        let neither = FolderRule {
            conditions: vec![
                condition(RuleField::Site, RuleOperator::Is, "X"),
                or(condition(RuleField::Site, RuleOperator::Is, "Bluesky")),
            ],
            ..either.clone()
        };
        assert!(!neither.matches(&subject));
        let without = FolderRule {
            conditions: Vec::new(),
            ..either
        };
        assert!(
            without.matches(&subject),
            "no conditions still match every download"
        );

        let saved = r#"{ "field": "title", "operator": "is", "value": "x" }"#;
        assert_eq!(
            serde_json::from_str::<RuleCondition>(saved).unwrap().join,
            ConditionJoin::All,
            "conditions from before read as AND"
        );
    }

    #[test]
    fn or_groups_bind_before_and() {
        let video = video();
        let subject = RuleSubject {
            video: &video,
            preset: None,
        };
        let site = |name: &str| condition(RuleField::Site, RuleOperator::Is, name);
        let title = |text: &str| condition(RuleField::Title, RuleOperator::Contains, text);
        let matches = |conditions: Vec<RuleCondition>| rule(conditions, r"D:\x").matches(&subject);
        assert!(
            matches(vec![site("SoundCloud"), or(site("YouTube")), title("zoo")]),
            "(SoundCloud OR YouTube) AND zoo"
        );
        assert!(!matches(vec![
            site("SoundCloud"),
            or(site("YouTube")),
            title("cats")
        ]));
        assert!(
            !matches(vec![site("YouTube"), title("cats"), or(site("X"))]),
            "YouTube AND (cats OR X), not (YouTube AND cats) OR X"
        );
        assert!(!matches(vec![site("X"), title("zoo"), or(site("YouTube"))]));
        assert!(matches(vec![
            site("YouTube"),
            title("cats"),
            or(title("zoo"))
        ]));
    }

    #[test]
    fn moves_a_rule_wide_or_onto_its_conditions() {
        let saved = r#"{ "id": "rule-1", "enabled": true, "folder": "D:\\x", "join": "any", "conditions": [
            { "field": "site", "operator": "is", "value": "X" },
            { "field": "site", "operator": "is", "value": "Bluesky" }
        ] }"#;
        let read: FolderRule = serde_json::from_str(saved).unwrap();
        let rules = with_catch_all_first(vec![read]);
        let joins: Vec<ConditionJoin> = rules[1]
            .conditions
            .iter()
            .map(|condition| condition.join)
            .collect();
        assert_eq!(joins, [ConditionJoin::All, ConditionJoin::Any]);
        let written = serde_json::to_value(&rules[1]).unwrap();
        assert!(written.get("join").is_none(), "never written again");
        assert_eq!(written["conditions"][1]["join"], "any");
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

    fn folder_on(rules: &[FolderRule], video: &VideoInfo, preset: Option<&Preset>) -> String {
        destination_on(
            rules,
            &RuleSubject { video, preset },
            Path::new(DOWNLOADS),
            "2026-11-06",
        )
        .folder
    }

    #[test]
    fn fills_in_what_the_download_is_and_when() {
        let mut video = video();
        video.upload_date = Some("2005-04-23".to_owned());
        let best_video = preset("best-video");
        let rules = [rule(
            Vec::new(),
            r"D:\{title}\{ID}\{kind}\{preset}\{year}\{month}\{date}\{added}",
        )];
        assert_eq!(
            folder_on(&rules, &video, Some(&best_video)),
            format!(
                r"D:\Me at the zoo\jNQXAC9IVRw\Videos\{}\2005\04\2005-04-23\2026-11-06",
                best_video.name
            )
        );
        assert!(
            folder_on(&rules, &video, Some(&preset("best-audio"))).contains(r"\Audio\"),
            "audio by its preset"
        );

        let mut tweet = tweet();
        tweet.title = "a/b".repeat(40);
        let expected_title = "a_b".repeat(40).chars().take(80).collect::<String>();
        assert_eq!(
            folder_on(&rules, &tweet, None),
            format!(r"D:\{expected_title}\1\Posts\Posts\Unknown\Unknown\Unknown\2026-11-06"),
            "a post has no preset or, here, upload date, and a long title is cut"
        );
        assert!(folder_on(&rules, &mega_file(), None).contains(r"\Files\Files\"));
    }

    #[test]
    fn knows_the_uploader_by_their_handle_too() {
        let mut post = tweet();
        post.uploader = Some("National Geographic".to_owned());
        post.account = Some("natgeo".to_owned());
        let subject = RuleSubject {
            video: &post,
            preset: None,
        };
        assert!(condition(RuleField::Uploader, RuleOperator::Is, "NatGeo").matches(&subject));
        assert!(
            condition(RuleField::Uploader, RuleOperator::Is, "national geographic")
                .matches(&subject)
        );
        assert!(
            !condition(RuleField::Uploader, RuleOperator::IsNot, "natgeo").matches(&subject),
            "not the uploader by either name"
        );
        assert!(condition(RuleField::Uploader, RuleOperator::Matches, "^nat").matches(&subject));

        let rules = [rule(Vec::new(), r"D:\{uploader}\{Account}\{playlist}")];
        assert_eq!(
            folder_on(&rules, &post, None),
            r"D:\National Geographic\natgeo\No Playlist"
        );
        post.account = None;
        post.playlist = Some("Best of 2026".to_owned());
        assert_eq!(
            folder_on(&rules, &post, None),
            r"D:\National Geographic\National Geographic\Best of 2026",
            "the name stands in for a site without handles"
        );
    }

    #[test]
    fn matches_a_pattern_ignoring_case() {
        let video = video();
        let subject = RuleSubject {
            video: &video,
            preset: None,
        };
        let matches = |field, pattern| condition(field, RuleOperator::Matches, pattern);
        assert!(matches(RuleField::Title, r"^me AT\s").matches(&subject));
        assert!(!matches(RuleField::Title, "^zoo").matches(&subject));
        assert!(condition(RuleField::Title, RuleOperator::DoesNotMatch, "^zoo").matches(&subject));
        assert!(matches(RuleField::Link, r"watch\?v=[\w-]{11}$").matches(&subject));
        assert!(
            matches(RuleField::Site, "^x$").matches(&RuleSubject {
                video: &tweet(),
                preset: None
            }),
            "any of the site's names"
        );

        let mut nobody = video.clone();
        nobody.uploader = None;
        let nobody = RuleSubject {
            video: &nobody,
            preset: None,
        };
        assert!(!matches(RuleField::Uploader, ".*").matches(&nobody));
        assert!(condition(RuleField::Uploader, RuleOperator::DoesNotMatch, "a").matches(&nobody));

        let broken = condition(RuleField::Title, RuleOperator::DoesNotMatch, "(zoo");
        assert!(
            !broken.matches(&subject),
            "a broken pattern matches nothing"
        );
    }

    #[test]
    fn puts_what_a_pattern_captured_in_the_folder() {
        let mut episode = video();
        episode.title = "[Show: Night] Episode 12".to_owned();
        let captures = rule(
            vec![
                condition(RuleField::Title, RuleOperator::Matches, "episode"),
                condition(
                    RuleField::Title,
                    RuleOperator::Matches,
                    r"^\[(.+?)\] episode (\d+)(x)?",
                ),
            ],
            r"D:\Series\{1}\{2}\{3}\{4}",
        );
        assert_eq!(
            folder_on(&[captures], &episode, None),
            r"D:\Series\Show_ Night\12\Unknown\Unknown",
            "the first pattern with groups; a group that took no part, or isn't there, is Unknown"
        );

        let without = rule(
            vec![condition(
                RuleField::Title,
                RuleOperator::Contains,
                "episode",
            )],
            r"D:\Series\{1}",
        );
        assert_eq!(folder_on(&[without], &episode, None), r"D:\Series\{1}");
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
            catch_all_rule(),
            rule(
                vec![condition(RuleField::Kind, RuleOperator::Is, "post")],
                r"D:\Posts",
            ),
        ];
        assert_eq!(
            folder_of(&rules, &video, Some(&mine)),
            Destination {
                folder: r"D:\Tsundoku\YouTube".to_owned(),
                source: DestinationSource::Rule { number: 1 },
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
    fn a_rule_without_a_folder_leaves_it_to_the_one_before() {
        let track = soundcloud_track();
        let audio = preset("audio-mp3");
        let soundcloud = || vec![condition(RuleField::Site, RuleOperator::Is, "SoundCloud")];
        let rules = vec![
            catch_all_rule(),
            rule(soundcloud(), r"D:\Music"),
            preset_rule(soundcloud(), "audio-mp3"),
        ];
        assert_eq!(
            folder_of(&rules, &track, Some(&audio)),
            Destination {
                folder: r"D:\Music".to_owned(),
                source: DestinationSource::Rule { number: 2 },
            }
        );
        let only_preset = vec![catch_all_rule(), preset_rule(soundcloud(), "audio-mp3")];
        assert_eq!(
            folder_of(&only_preset, &track, Some(&audio)).folder,
            r"D:\Tsundoku\soundcloud",
            "the catch-all, as with no rule"
        );
    }

    #[test]
    fn the_last_rule_that_picks_a_preset_and_matches_decides() {
        let track = soundcloud_track();
        let mut off = preset_rule(Vec::new(), "best-video");
        off.enabled = false;
        let rules = vec![
            catch_all_rule(),
            preset_rule(Vec::new(), "best-audio"),
            preset_rule(
                vec![condition(
                    RuleField::Site,
                    RuleOperator::Is,
                    "soundcloud.com",
                )],
                "audio-mp3",
            ),
            preset_rule(
                vec![condition(RuleField::Site, RuleOperator::Is, "YouTube")],
                "1080p-mp4",
            ),
            rule(Vec::new(), r"D:\All"),
            off,
        ];
        assert_eq!(
            start_preset(&rules, &track, any_preset),
            Some(RulePreset {
                preset_id: "audio-mp3".to_owned(),
                rule: 3,
            })
        );
        assert_eq!(
            start_preset(&rules, &video(), any_preset).map(|pick| pick.rule),
            Some(4)
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
    fn the_last_rule_that_says_decides_download_right_away() {
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
            right_away(false, Vec::new()),
            right_away(
                true,
                vec![condition(RuleField::Kind, RuleOperator::Is, "audio")],
            ),
            rule(Vec::new(), r"D:\All"),
            off,
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
        assert_eq!(download_right_away(&rules[2..], &subject), None, "Settings");
        assert_eq!(
            right_away_rule(&rules, &subject),
            Some(RightAwayRule {
                download_right_away: true,
                rule: 2
            }),
            "numbered as listed, the rule turned off included"
        );
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
        rules.push(FolderRule {
            name: " Nat Geo ".to_owned(),
            ..rule(
                vec![condition(RuleField::Uploader, RuleOperator::Is, " natgeo ")],
                r" D:\NatGeo ",
            )
        });
        rules.insert(0, rule(Vec::new(), r"D:\Rest"));
        let saved = manager.save(rules).unwrap();
        let ids: Vec<&str> = saved.iter().map(|rule| rule.id.as_str()).collect();
        assert_eq!(
            ids,
            ["catch-all", "rule-1", "rule-2"],
            "the catch-all stays first"
        );
        assert_eq!(saved[2].folder, r"D:\NatGeo");
        assert_eq!(saved[2].name, "Nat Geo");
        assert_eq!(saved[1].name, "", "no name: the UI calls it by its number");
        assert_eq!(saved[2].conditions[0].value, "natgeo");
        assert_eq!(manager_in(&folder).list(), saved, "kept in settings.json");

        let mut next = saved.clone();
        next.remove(1);
        next.push(rule(Vec::new(), r"D:\More"));
        assert_eq!(manager.save(next).unwrap()[2].id, "rule-3");
    }

    #[test]
    fn keeps_the_catch_all_first_and_never_loses_it() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        let mut first = rule(Vec::new(), r"D:\First");
        first.id = "rule-1".to_owned();
        let moved = manager.save(vec![first.clone(), catch_all_rule()]).unwrap();
        assert_eq!(moved, vec![catch_all_rule(), first.clone()]);

        let without = manager.save(vec![first.clone()]).unwrap();
        assert_eq!(
            without,
            vec![
                FolderRule {
                    enabled: false,
                    ..catch_all_rule()
                },
                first.clone(),
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
            with_catch_all_first(vec![first.clone(), posts]),
            vec![
                FolderRule {
                    id: CATCH_ALL_RULE_ID.to_owned(),
                    enabled: false,
                    ..rule(Vec::new(), r"D:\Hydrus\{site}")
                },
                first,
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
            Err(RuleError::NoAction { rule: 2 }),
            "numbered as saved, with the catch-all first"
        );
        let blank_preset = vec![FolderRule {
            preset_id: Some(" ".to_owned()),
            ..rule(Vec::new(), "")
        }];
        assert_eq!(
            manager.save(blank_preset),
            Err(RuleError::NoAction { rule: 2 })
        );
        let catch_all = vec![FolderRule {
            folder: " ".to_owned(),
            ..catch_all_rule()
        }];
        assert_eq!(
            manager.save(catch_all),
            Err(RuleError::EmptyFolder { rule: 1 })
        );
        let broken = vec![rule(
            vec![condition(RuleField::Title, RuleOperator::Matches, "(zoo")],
            r"D:\x",
        )];
        assert_eq!(
            manager.save(broken),
            Err(RuleError::InvalidPattern {
                rule: 2,
                detail: "unclosed group".to_owned()
            })
        );
        let empty = vec![rule(
            vec![condition(RuleField::Title, RuleOperator::Contains, " ")],
            r"D:\x",
        )];
        assert_eq!(manager.save(empty), Err(RuleError::EmptyValue { rule: 2 }));
        let circular = vec![preset_rule(
            vec![condition(RuleField::Kind, RuleOperator::Is, "audio")],
            "audio-mp3",
        )];
        assert_eq!(
            manager.save(circular),
            Err(RuleError::CircularPreset { rule: 2 })
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
                catch_all_rule(),
                preset_rule(soundcloud(), " audio-mp3 "),
                FolderRule {
                    download_right_away: Some(false),
                    ..rule(soundcloud(), " ")
                },
            ])
            .unwrap();
        assert_eq!(saved[1].preset_id.as_deref(), Some("audio-mp3"));
        assert_eq!(saved[2].folder, "");
        assert_eq!(saved[2].download_right_away, Some(false));
        assert_eq!(manager_in(&folder).list(), saved, "kept in settings.json");
        assert_eq!(
            manager
                .start_preset(&soundcloud_track(), any_preset)
                .map(|pick| pick.rule),
            Some(2)
        );
    }
}
