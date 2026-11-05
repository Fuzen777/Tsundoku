//! Presets: how a video is downloaded (formats, container, file name, folder, what to embed), the yt-dlp arguments
//! each one gives, and the user's list of them, kept in `settings.json`.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::settings::{SettingsError, SettingsStore};

/// yt-dlp's file name: the title, cut to stay under Windows' path limit, then the video's ID.
pub const DEFAULT_FILE_NAME: &str = "%(title).150B [%(id)s].%(ext)s";
pub const DEFAULT_PRESET_ID: &str = "best-video";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Preset {
    /// Stays the same when the preset is renamed. Built-ins have fixed IDs ("best-video"); others are "custom-<n>".
    pub id: String,
    pub name: String,
    pub output: PresetOutput,
    /// yt-dlp's output template for the file name.
    pub file_name: String,
    /// Where files go, or `None` for the Videos folder (video) or the Music folder (audio).
    pub folder: Option<String>,
    pub embed_metadata: bool,
    pub embed_thumbnail: bool,
    pub embed_chapters: bool,
    /// Video only.
    pub embed_subtitles: bool,
    /// yt-dlp's `--sub-langs`, such as "en.*,ja".
    pub subtitle_languages: String,
    pub sponsor_block: SponsorBlock,
    /// More yt-dlp arguments, after Tsundoku's own, so they can override them.
    pub extra_arguments: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum PresetOutput {
    Video {
        /// The tallest video to pick, or `None` for the best there is.
        #[serde(rename = "maxHeight")]
        max_height: Option<u32>,
        container: VideoContainer,
    },
    Audio {
        format: AudioFormat,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum VideoContainer {
    /// Holds any codec, so nothing is re-encoded.
    Mkv,
    /// Plays everywhere: H.264 video and AAC audio are preferred, so it's rarely re-encoded.
    Mp4,
    /// Whatever yt-dlp merges into.
    Original,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AudioFormat {
    /// The best audio stream as it is, usually Opus or M4A.
    Original,
    Mp3,
    M4a,
    Opus,
    Flac,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SponsorBlock {
    #[default]
    Off,
    /// Chapters for every segment SponsorBlock knows.
    Mark,
    /// Cuts sponsor segments out.
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum PresetError {
    #[error("the name is empty")]
    EmptyName,
    #[error("the file name is empty")]
    EmptyFileName,
    /// An extra argument that would break progress, the history or the single-video rule.
    #[error("{argument} can't be used in a preset")]
    RefusedArgument { argument: String },
    #[error("a quote in the extra arguments isn't closed")]
    UnclosedQuote,
    #[error("no such preset")]
    NotFound,
    /// Built-ins can be reset, not deleted.
    #[error("built-in presets can't be deleted")]
    BuiltIn,
    #[error("couldn't save: {detail}")]
    Storage { detail: String },
}

impl From<SettingsError> for PresetError {
    fn from(error: SettingsError) -> Self {
        Self::Storage {
            detail: error.to_string(),
        }
    }
}

/// Options that would break what Tsundoku relies on: its progress lines, its file paths, the history and the
/// single-video rule. Long options are also refused when abbreviated, as yt-dlp would accept "--outp".
const REFUSED_LONG_OPTIONS: &[&str] = &[
    "--yes-playlist",
    "--download-archive",
    "--break-on-existing",
    "--print",
    "--print-to-file",
    "--progress-template",
    "--quiet",
    "--no-progress",
    "--simulate",
    "--skip-download",
    "--dump-json",
    "--dump-single-json",
    "--paths",
    "--output",
    "--batch-file",
    "--load-info-json",
    "--config-locations",
    "--update",
    "--update-to",
];
const REFUSED_SHORT_OPTIONS: &[&str] = &["-O", "-q", "-s", "-j", "-J", "-P", "-o", "-a", "-U"];

impl Preset {
    pub fn is_audio(&self) -> bool {
        matches!(self.output, PresetOutput::Audio { .. })
    }

    /// Checks the preset can be saved: a name, a file name, and extra arguments that parse and are allowed.
    pub fn validate(&self) -> Result<(), PresetError> {
        if self.name.trim().is_empty() {
            return Err(PresetError::EmptyName);
        }
        if self.file_name.trim().is_empty() {
            return Err(PresetError::EmptyFileName);
        }
        self.extra_argument_list().map(|_| ())
    }

    /// The preset's yt-dlp arguments: formats, conversion, file name, embedding, SponsorBlock, then the extra ones.
    pub fn to_args(&self) -> Result<Vec<String>, PresetError> {
        let mut args = Vec::new();
        let mut push =
            |values: &[&str]| args.extend(values.iter().map(|value| (*value).to_owned()));
        match self.output {
            PresetOutput::Video {
                max_height,
                container,
            } => {
                push(&["-f", "bv*+ba/b"]);
                let mut sort = Vec::new();
                if let Some(height) = max_height {
                    sort.push(format!("res:{height}"));
                }
                if container == VideoContainer::Mp4 {
                    // H.264 and AAC, which play everywhere and go into MP4 without re-encoding. AV1, also in MP4
                    // formats, doesn't play on older devices.
                    sort.push("vcodec:h264,acodec:aac".to_owned());
                }
                if !sort.is_empty() {
                    push(&["-S", &sort.join(",")]);
                }
                match container {
                    VideoContainer::Mkv => {
                        push(&["--merge-output-format", "mkv", "--remux-video", "mkv"])
                    }
                    VideoContainer::Mp4 => {
                        push(&["--merge-output-format", "mp4", "--remux-video", "mp4"])
                    }
                    VideoContainer::Original => {}
                }
            }
            PresetOutput::Audio { format } => {
                push(&["-f", "ba/b", "-x"]);
                match format {
                    AudioFormat::Original => {}
                    AudioFormat::Mp3 => push(&["--audio-format", "mp3", "--audio-quality", "0"]),
                    AudioFormat::M4a => push(&["-S", "ext:m4a", "--audio-format", "m4a"]),
                    AudioFormat::Opus => push(&["-S", "acodec:opus", "--audio-format", "opus"]),
                    AudioFormat::Flac => push(&["--audio-format", "flac"]),
                }
            }
        }
        push(&["-o", &self.file_name]);
        if self.embed_metadata {
            push(&["--embed-metadata"]);
        }
        if self.embed_thumbnail {
            push(&["--embed-thumbnail"]);
        }
        if self.embed_chapters {
            push(&["--embed-chapters"]);
        }
        if self.embed_subtitles && !self.is_audio() {
            push(&[
                "--write-subs",
                "--sub-langs",
                &self.subtitle_languages,
                "--embed-subs",
            ]);
        }
        match self.sponsor_block {
            SponsorBlock::Off => {}
            SponsorBlock::Mark => push(&["--sponsorblock-mark", "all"]),
            SponsorBlock::Remove => push(&["--sponsorblock-remove", "sponsor"]),
        }
        args.extend(self.extra_argument_list()?);
        Ok(args)
    }

    fn extra_argument_list(&self) -> Result<Vec<String>, PresetError> {
        let arguments = split_arguments(&self.extra_arguments)?;
        match arguments.iter().find(|argument| is_refused(argument)) {
            Some(refused) => Err(PresetError::RefusedArgument {
                argument: refused.clone(),
            }),
            None => Ok(arguments),
        }
    }

    /// Where the preset's files go: its folder, or else Videos or Music.
    pub fn resolved_folder(&self) -> PathBuf {
        match &self.folder {
            Some(folder) if !folder.trim().is_empty() => PathBuf::from(folder),
            _ => default_folder(self.is_audio()),
        }
    }
}

/// The Videos folder for video presets, the Music folder for audio ones, or the user's folder if Windows has
/// neither.
pub fn default_folder(audio: bool) -> PathBuf {
    let folder = if audio {
        dirs::audio_dir()
    } else {
        dirs::video_dir()
    };
    folder.or_else(dirs::home_dir).unwrap_or_default()
}

/// Splits extra arguments on spaces; double quotes keep spaces in an argument ("--sub-langs \"en, ja\"").
/// Backslashes are left alone, since they're Windows path separators.
fn split_arguments(text: &str) -> Result<Vec<String>, PresetError> {
    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut in_argument = false;
    let mut quoted = false;
    for character in text.chars() {
        match character {
            '"' => {
                quoted = !quoted;
                in_argument = true;
            }
            character if character.is_whitespace() && !quoted => {
                if in_argument {
                    arguments.push(std::mem::take(&mut current));
                    in_argument = false;
                }
            }
            character => {
                current.push(character);
                in_argument = true;
            }
        }
    }
    if quoted {
        return Err(PresetError::UnclosedQuote);
    }
    if in_argument {
        arguments.push(current);
    }
    Ok(arguments)
}

/// Allowed options whose name is also the start of a refused one, so they aren't taken for abbreviations of it.
const ALLOWED_OPTIONS_LIKE_REFUSED: &[&str] = &["progress"];

fn is_refused(argument: &str) -> bool {
    if let Some(name) = argument.strip_prefix("--") {
        let name = name.split('=').next().unwrap_or_default();
        // A refused option, or an abbreviation of one ("--outp"), which yt-dlp accepts; "--" alone ends the options.
        !name.is_empty()
            && !ALLOWED_OPTIONS_LIKE_REFUSED.contains(&name)
            && REFUSED_LONG_OPTIONS
                .iter()
                .any(|refused| refused[2..].starts_with(name))
    } else {
        argument.starts_with('-')
            && argument
                .get(..2)
                .is_some_and(|short| REFUSED_SHORT_OPTIONS.contains(&short))
    }
}

/// The four presets every install starts with.
pub fn built_in_presets() -> Vec<Preset> {
    let preset = |id: &str, name: &str, output: PresetOutput| Preset {
        id: id.to_owned(),
        name: name.to_owned(),
        output,
        file_name: DEFAULT_FILE_NAME.to_owned(),
        folder: None,
        embed_metadata: true,
        embed_thumbnail: true,
        embed_chapters: !matches!(output, PresetOutput::Audio { .. }),
        embed_subtitles: false,
        subtitle_languages: "en.*".to_owned(),
        sponsor_block: SponsorBlock::Off,
        extra_arguments: String::new(),
    };
    vec![
        preset(
            DEFAULT_PRESET_ID,
            "Best Video",
            PresetOutput::Video {
                max_height: None,
                container: VideoContainer::Mkv,
            },
        ),
        preset(
            "1080p-mp4",
            "1080p MP4",
            PresetOutput::Video {
                max_height: Some(1080),
                container: VideoContainer::Mp4,
            },
        ),
        preset(
            "best-audio",
            "Best Audio",
            PresetOutput::Audio {
                format: AudioFormat::Original,
            },
        ),
        preset(
            "audio-mp3",
            "Audio MP3",
            PresetOutput::Audio {
                format: AudioFormat::Mp3,
            },
        ),
    ]
}

/// The presets in `settings.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PresetsSettings {
    pub list: Vec<Preset>,
    pub default_id: String,
}

impl Default for PresetsSettings {
    fn default() -> Self {
        Self {
            list: built_in_presets(),
            default_id: DEFAULT_PRESET_ID.to_owned(),
        }
    }
}

/// The presets, for the presets view and the preview card's picker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PresetsOverview {
    pub presets: Vec<Preset>,
    pub default_id: String,
    /// IDs of the built-ins, which can be reset but not deleted.
    pub built_in_ids: Vec<String>,
    /// Where presets without a folder save: Videos and Music.
    pub default_video_folder: String,
    pub default_audio_folder: String,
}

/// Changes to the presets, saved to `settings.json` as they're made.
pub struct PresetManager {
    settings: Arc<SettingsStore>,
}

impl PresetManager {
    pub fn new(settings: Arc<SettingsStore>) -> Self {
        Self { settings }
    }

    pub fn overview(&self) -> PresetsOverview {
        let presets = self.settings.get().presets;
        PresetsOverview {
            presets: presets.list,
            default_id: presets.default_id,
            built_in_ids: built_in_presets()
                .into_iter()
                .map(|preset| preset.id)
                .collect(),
            default_video_folder: default_folder(false).display().to_string(),
            default_audio_folder: default_folder(true).display().to_string(),
        }
    }

    pub fn get(&self, id: &str) -> Option<Preset> {
        self.settings
            .get()
            .presets
            .list
            .into_iter()
            .find(|preset| preset.id == id)
    }

    /// Saves a preset, replacing the one with its ID or adding it at the end.
    pub fn save(&self, preset: Preset) -> Result<PresetsOverview, PresetError> {
        preset.validate()?;
        self.settings.update(|settings| {
            let list = &mut settings.presets.list;
            match list.iter_mut().find(|existing| existing.id == preset.id) {
                Some(existing) => *existing = preset,
                None => list.push(preset),
            }
        })?;
        Ok(self.overview())
    }

    /// Adds a copy of a preset right after it, under `name`. Returns the copy.
    pub fn duplicate(&self, id: &str, name: String) -> Result<Preset, PresetError> {
        let copy = self.settings.update(|settings| {
            let list = &mut settings.presets.list;
            let index = list.iter().position(|preset| preset.id == id)?;
            let next_number = list
                .iter()
                .filter_map(|preset| preset.id.strip_prefix("custom-")?.parse::<u32>().ok())
                .max()
                .unwrap_or(0)
                + 1;
            let copy = Preset {
                id: format!("custom-{next_number}"),
                name,
                ..list[index].clone()
            };
            list.insert(index + 1, copy.clone());
            Some(copy)
        })?;
        copy.ok_or(PresetError::NotFound)
    }

    /// Deletes a preset the user made. If it was the default, Best Video becomes the default.
    pub fn delete(&self, id: &str) -> Result<PresetsOverview, PresetError> {
        if is_built_in(id) {
            return Err(PresetError::BuiltIn);
        }
        let found = self.settings.update(|settings| {
            let presets = &mut settings.presets;
            let count = presets.list.len();
            presets.list.retain(|preset| preset.id != id);
            if presets.default_id == id {
                presets.default_id = DEFAULT_PRESET_ID.to_owned();
            }
            presets.list.len() < count
        })?;
        if found {
            Ok(self.overview())
        } else {
            Err(PresetError::NotFound)
        }
    }

    /// Puts a built-in back as it ships, or back in the list if it was missing.
    pub fn reset(&self, id: &str) -> Result<PresetsOverview, PresetError> {
        let original = built_in_presets()
            .into_iter()
            .find(|preset| preset.id == id)
            .ok_or(PresetError::NotFound)?;
        self.save(original)
    }

    pub fn set_default(&self, id: &str) -> Result<PresetsOverview, PresetError> {
        if self.get(id).is_none() {
            return Err(PresetError::NotFound);
        }
        self.settings
            .update(|settings| settings.presets.default_id = id.to_owned())?;
        Ok(self.overview())
    }
}

fn is_built_in(id: &str) -> bool {
    built_in_presets().iter().any(|preset| preset.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built_in(id: &str) -> Preset {
        built_in_presets()
            .into_iter()
            .find(|preset| preset.id == id)
            .unwrap()
    }

    fn args(preset: &Preset) -> String {
        preset.to_args().unwrap().join(" ")
    }

    #[test]
    fn best_video_merges_the_best_formats_into_mkv() {
        assert_eq!(
            args(&built_in("best-video")),
            "-f bv*+ba/b --merge-output-format mkv --remux-video mkv -o %(title).150B [%(id)s].%(ext)s \
             --embed-metadata --embed-thumbnail --embed-chapters"
        );
    }

    #[test]
    fn mp4_prefers_h264_and_aac_up_to_its_height() {
        let args = args(&built_in("1080p-mp4"));
        assert!(args.starts_with(
            "-f bv*+ba/b -S res:1080,vcodec:h264,acodec:aac --merge-output-format mp4 --remux-video mp4 "
        ));
    }

    #[test]
    fn audio_presets_extract_the_audio() {
        assert!(args(&built_in("best-audio")).starts_with("-f ba/b -x -o "));
        assert!(
            args(&built_in("audio-mp3"))
                .starts_with("-f ba/b -x --audio-format mp3 --audio-quality 0 -o ")
        );
        assert!(!args(&built_in("best-audio")).contains("--embed-chapters"));
    }

    #[test]
    fn adds_subtitles_sponsorblock_and_extra_arguments_last() {
        let mut preset = built_in("best-video");
        preset.embed_subtitles = true;
        preset.subtitle_languages = "en.*,ja".to_owned();
        preset.sponsor_block = SponsorBlock::Remove;
        preset.extra_arguments = r#"--limit-rate 2M --sub-langs "en, ja""#.to_owned();
        let args = preset.to_args().unwrap();
        let tail: Vec<&str> = args
            .iter()
            .rev()
            .take(10)
            .rev()
            .map(String::as_str)
            .collect();
        assert_eq!(
            tail,
            [
                "--write-subs",
                "--sub-langs",
                "en.*,ja",
                "--embed-subs",
                "--sponsorblock-remove",
                "sponsor",
                "--limit-rate",
                "2M",
                "--sub-langs",
                "en, ja"
            ]
        );
    }

    #[test]
    fn audio_presets_never_embed_subtitles() {
        let mut preset = built_in("audio-mp3");
        preset.embed_subtitles = true;
        assert!(!args(&preset).contains("--embed-subs"));
    }

    #[test]
    fn splits_extra_arguments_like_a_command_line() {
        assert_eq!(split_arguments("").unwrap(), Vec::<String>::new());
        assert_eq!(split_arguments("  -a   b ").unwrap(), ["-a", "b"]);
        assert_eq!(
            split_arguments(r##"--x "two words" """##).unwrap(),
            ["--x", "two words", ""]
        );
        assert_eq!(
            split_arguments(r"--cookies C:\cookies.txt").unwrap(),
            ["--cookies", r"C:\cookies.txt"]
        );
        assert_eq!(
            split_arguments(r#"--x "open"#),
            Err(PresetError::UnclosedQuote)
        );
    }

    #[test]
    fn refuses_arguments_that_would_break_downloads() {
        let refused = |extra: &str| {
            let mut preset = built_in("best-video");
            preset.extra_arguments = extra.to_owned();
            preset.to_args()
        };
        for argument in [
            "--yes-playlist",
            "--download-archive=a.txt",
            "--print",
            "--progress-template",
            "--outp",
            "-o",
            "-o%(id)s.%(ext)s",
            "-q",
            "-J",
            "-P",
            "--update",
        ] {
            assert!(
                matches!(refused(argument), Err(PresetError::RefusedArgument { .. })),
                "{argument} should be refused"
            );
        }
        for argument in [
            "-S",
            "res:720",
            "--limit-rate",
            "--split-chapters",
            "--output-na-placeholder",
            "--progress",
            "--",
            "-f",
        ] {
            assert!(refused(argument).is_ok(), "{argument} should be allowed");
        }
    }

    #[test]
    fn validates_names() {
        let mut preset = built_in("best-video");
        preset.name = "  ".to_owned();
        assert_eq!(preset.validate(), Err(PresetError::EmptyName));
        preset.name = "Mine".to_owned();
        preset.file_name = String::new();
        assert_eq!(preset.validate(), Err(PresetError::EmptyFileName));
    }

    #[test]
    fn resolves_the_folder() {
        let mut preset = built_in("best-audio");
        assert_eq!(preset.resolved_folder(), default_folder(true));
        preset.folder = Some(r"D:\Music\Rips".to_owned());
        assert_eq!(preset.resolved_folder(), PathBuf::from(r"D:\Music\Rips"));
    }

    fn manager_in(folder: &tempfile::TempDir) -> PresetManager {
        let settings = Arc::new(SettingsStore::open(folder.path().join("settings.json")).unwrap());
        PresetManager::new(settings)
    }

    #[test]
    fn starts_with_the_built_ins_and_best_video_as_default() {
        let folder = tempfile::tempdir().unwrap();
        let overview = manager_in(&folder).overview();
        let names: Vec<_> = overview
            .presets
            .iter()
            .map(|preset| preset.name.as_str())
            .collect();
        assert_eq!(
            names,
            ["Best Video", "1080p MP4", "Best Audio", "Audio MP3"]
        );
        assert_eq!(overview.default_id, "best-video");
        assert_eq!(overview.built_in_ids.len(), 4);
    }

    #[test]
    fn duplicates_edits_resets_and_deletes() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);

        let copy = manager
            .duplicate("audio-mp3", "Podcasts".to_owned())
            .unwrap();
        assert_eq!(copy.id, "custom-1");
        let second = manager
            .duplicate("custom-1", "Podcasts 2".to_owned())
            .unwrap();
        assert_eq!(second.id, "custom-2");
        let ids: Vec<_> = manager
            .overview()
            .presets
            .into_iter()
            .map(|preset| preset.id)
            .collect();
        assert_eq!(
            ids,
            [
                "best-video",
                "1080p-mp4",
                "best-audio",
                "audio-mp3",
                "custom-1",
                "custom-2"
            ]
        );

        let mut edited = manager.get("best-video").unwrap();
        edited.name = "Everything".to_owned();
        manager.save(edited).unwrap();
        assert_eq!(manager.get("best-video").unwrap().name, "Everything");
        manager.reset("best-video").unwrap();
        assert_eq!(manager.get("best-video").unwrap().name, "Best Video");

        manager.set_default("custom-1").unwrap();
        manager.delete("custom-1").unwrap();
        assert_eq!(
            manager.overview().default_id,
            "best-video",
            "the default falls back to Best Video"
        );
        assert_eq!(manager.delete("best-video"), Err(PresetError::BuiltIn));
        assert_eq!(manager.delete("custom-9"), Err(PresetError::NotFound));
        assert_eq!(manager.set_default("custom-9"), Err(PresetError::NotFound));
    }

    #[test]
    fn refuses_to_save_an_invalid_preset() {
        let folder = tempfile::tempdir().unwrap();
        let manager = manager_in(&folder);
        let mut preset = manager.get("best-video").unwrap();
        preset.extra_arguments = "--yes-playlist".to_owned();
        assert!(matches!(
            manager.save(preset),
            Err(PresetError::RefusedArgument { .. })
        ));
        assert_eq!(manager.get("best-video").unwrap().extra_arguments, "");
    }
}
