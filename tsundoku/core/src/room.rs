//! Room on a drive for a download, so one that won't fit waits instead of failing partway: what it's expected to take
//! (a file host's files by their size, a video by yt-dlp's estimate for the formats its preset picks) against what its
//! drive has free, less what the downloads running there still need, with a margin.
//!
//! A video's estimate is kept from when its link was read (`remember_sizes`, from yt-dlp's answer), for as long as the
//! app runs: a video added before a restart, a post (gallery-dl gives no sizes) or a video whose formats have none
//! isn't checked.

use std::collections::HashMap;
use std::path::{Component, Path, Prefix};
use std::sync::{Mutex, OnceLock};

use crate::metadata::VideoInfo;
use crate::presets::{Preset, PresetOutput};
use crate::sync::lock;

/// Kept free besides what a download needs, as sizes are estimates: 100 MB, and 5% of the download.
const MARGIN_BYTES: u64 = 100 * 1024 * 1024;
const MARGIN_PERCENT: u64 = 5;

/// The most videos whose sizes are kept; past it, they're forgotten and kept again as links are read.
const KEPT_VIDEOS: usize = 5000;

/// One of a video's formats, as far as its size goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormatSize {
    /// `None` for audio only.
    pub height: Option<u32>,
    /// Whether it holds audio: audio only, or video with its sound.
    pub audio: bool,
    /// yt-dlp's `filesize`, else its `filesize_approx`; `None` when it gives neither.
    pub bytes: Option<u64>,
}

/// What a video's formats weigh, by yt-dlp's estimate.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormatSizes {
    pub formats: Vec<FormatSize>,
}

impl FormatSizes {
    /// Roughly what a download with `preset` takes, picking formats as the preset's `-f` does: for video, the tallest
    /// format up to its height limit (the shortest above it when none is) plus the best audio; for audio, the best
    /// audio, else the best format with sound. The biggest of the formats it could pick, so it errs on the safe side.
    /// `None` when the formats it would pick have no size.
    pub fn estimate(&self, preset: &Preset) -> Option<u64> {
        let audio_only = |format: &&FormatSize| format.height.is_none() && format.audio;
        let best_audio = self
            .formats
            .iter()
            .filter(audio_only)
            .filter_map(|format| format.bytes)
            .max();
        match preset.output {
            PresetOutput::Audio { .. } => best_audio.or_else(|| {
                self.formats
                    .iter()
                    .filter(|format| format.audio)
                    .filter_map(|format| format.bytes)
                    .max()
            }),
            PresetOutput::Video { max_height, .. } => {
                let heights = self.formats.iter().filter_map(|format| format.height);
                let height = match max_height {
                    Some(limit) => heights
                        .clone()
                        .filter(|height| *height <= limit)
                        .max()
                        .or_else(|| heights.min()),
                    None => heights.max(),
                }?;
                let video = self
                    .formats
                    .iter()
                    .filter(|format| format.height == Some(height))
                    .filter_map(|format| format.bytes)
                    .max()?;
                Some(video + best_audio.unwrap_or(0))
            }
        }
    }
}

/// The sizes of every video read since the app started, by site and ID.
fn kept() -> &'static Mutex<HashMap<(String, String), FormatSizes>> {
    static KEPT: OnceLock<Mutex<HashMap<(String, String), FormatSizes>>> = OnceLock::new();
    KEPT.get_or_init(Mutex::default)
}

/// Keeps what a video's formats weigh, read from yt-dlp's answer, for when it's downloaded. Formats without any size
/// aren't worth keeping.
pub fn remember_sizes(video: &VideoInfo, sizes: FormatSizes) {
    if sizes.formats.iter().all(|format| format.bytes.is_none()) {
        return;
    }
    let mut kept = lock(kept());
    if kept.len() >= KEPT_VIDEOS {
        kept.clear();
    }
    kept.insert((video.site.clone(), video.video_id.clone()), sizes);
}

/// Roughly what downloading `video` with `preset` takes: a file host's files by their size, a video by yt-dlp's
/// estimate when its link was read since the app started. `None` when it isn't known, such as for a post.
pub fn expected_bytes(video: &VideoInfo, preset: Option<&Preset>) -> Option<u64> {
    if let Some(files) = video.files {
        return Some(files.bytes);
    }
    if video.post.is_some() {
        return None;
    }
    let kept = lock(kept());
    kept.get(&(video.site.clone(), video.video_id.clone()))?
        .estimate(preset?)
}

/// A drive and what it has free.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveSpace {
    /// As people name it: "D:", or "\\server\share" for a network folder.
    pub drive: String,
    pub free: u64,
}

/// Tells what a folder's drive has free. Tests use a fake.
pub trait DiskSpace: Send + Sync {
    /// The drive of `folder`, which may not exist yet, and its free space; `None` if Windows won't say.
    fn free(&self, folder: &Path) -> Option<DriveSpace>;
}

/// Windows' own answer, for the user this app runs as (quotas included).
pub struct SystemDisks;

impl DiskSpace for SystemDisks {
    fn free(&self, folder: &Path) -> Option<DriveSpace> {
        // The folder may not exist until the download makes it: ask about the nearest one that does.
        let existing = folder.ancestors().find(|path| path.is_dir())?;
        Some(DriveSpace {
            drive: drive_name(folder),
            free: free_bytes(existing)?,
        })
    }
}

#[cfg(windows)]
fn free_bytes(folder: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide: Vec<u16> = folder
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut available = 0_u64;
    // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the call, and the one pointer passed out is to a
    // local; the other two may be null.
    let succeeded = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (succeeded != 0).then_some(available)
}

#[cfg(not(windows))]
fn free_bytes(_folder: &Path) -> Option<u64> {
    None
}

/// The drive a folder is on, as people name it: "D:", or "\\server\share". A folder without one is named whole.
pub fn drive_name(folder: &Path) -> String {
    match folder.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                format!("{}:", char::from(letter).to_ascii_uppercase())
            }
            Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => format!(
                r"\\{}\{}",
                server.to_string_lossy(),
                share.to_string_lossy()
            ),
            _ => prefix.as_os_str().to_string_lossy().into_owned(),
        },
        _ => folder.display().to_string(),
    }
}

/// A download that won't fit: the drive, what it has free, what the download needs with the margin, and what the
/// downloads already running there still need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortfall {
    pub drive: String,
    pub free: u64,
    pub needed: u64,
    pub others: u64,
}

/// Whether a download of `bytes` fits on `space`'s drive once `others` (what the downloads running there still need)
/// is set aside, with a margin: `None` when it does.
pub fn shortfall(space: &DriveSpace, bytes: u64, others: u64) -> Option<Shortfall> {
    let needed = bytes
        .saturating_add(MARGIN_BYTES)
        .saturating_add(bytes / 100 * MARGIN_PERCENT);
    (space.free < needed.saturating_add(others)).then(|| Shortfall {
        drive: space.drive.clone(),
        free: space.free,
        needed,
        others,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{FileContents, PostContents};
    use crate::presets::built_in_presets;

    const MB: u64 = 1024 * 1024;

    fn preset(id: &str) -> Preset {
        built_in_presets()
            .into_iter()
            .find(|preset| preset.id == id)
            .unwrap()
    }

    fn format(height: Option<u32>, audio: bool, megabytes: Option<u64>) -> FormatSize {
        FormatSize {
            height,
            audio,
            bytes: megabytes.map(|size| size * MB),
        }
    }

    /// A video at 2160p, 1080p and 720p without sound, two audio formats, and a 360p one with its sound.
    fn sizes() -> FormatSizes {
        FormatSizes {
            formats: vec![
                format(None, true, Some(3)),
                format(None, true, Some(5)),
                format(Some(360), true, Some(20)),
                format(Some(720), false, Some(40)),
                format(Some(1080), false, Some(80)),
                format(Some(1080), false, Some(90)),
                format(Some(2160), false, Some(400)),
            ],
        }
    }

    #[test]
    fn estimates_what_each_preset_picks() {
        let sizes = sizes();
        assert_eq!(
            sizes.estimate(&preset("best-video")),
            Some(405 * MB),
            "the tallest, with the best audio"
        );
        assert_eq!(
            sizes.estimate(&preset("1080p-mp4")),
            Some(95 * MB),
            "the biggest at 1080p, with the best audio"
        );
        assert_eq!(sizes.estimate(&preset("best-audio")), Some(5 * MB));
        assert_eq!(sizes.estimate(&preset("audio-mp3")), Some(5 * MB));
    }

    #[test]
    fn falls_back_as_yt_dlp_does() {
        let mut low = preset("1080p-mp4");
        low.output = PresetOutput::Video {
            max_height: Some(240),
            container: crate::presets::VideoContainer::Mp4,
        };
        assert_eq!(
            sizes().estimate(&low),
            Some(25 * MB),
            "nothing at 240p or less: the shortest above"
        );
        let combined = FormatSizes {
            formats: vec![format(Some(480), true, Some(30))],
        };
        assert_eq!(
            combined.estimate(&preset("audio-mp3")),
            Some(30 * MB),
            "no audio alone: the format with sound"
        );
        assert_eq!(combined.estimate(&preset("best-video")), Some(30 * MB));
    }

    #[test]
    fn doesnt_guess_without_sizes() {
        let without_sizes = FormatSizes {
            formats: vec![format(Some(1080), false, None), format(None, true, Some(5))],
        };
        assert_eq!(without_sizes.estimate(&preset("best-video")), None);
        assert_eq!(without_sizes.estimate(&preset("best-audio")), Some(5 * MB));
    }

    fn video(id: &str) -> VideoInfo {
        VideoInfo {
            site: "youtube".to_owned(),
            site_name: "youtube.com".to_owned(),
            video_id: id.to_owned(),
            url: format!("https://www.youtube.com/watch?v={id}"),
            title: id.to_owned(),
            uploader: None,
            account: None,
            duration_seconds: None,
            upload_date: None,
            thumbnail_url: None,
            best_height: Some(2160),
            post: None,
            files: None,
            post_files: Vec::new(),
            playlist: None,
        }
    }

    #[test]
    fn knows_a_video_read_before_and_a_file_hosts_files() {
        let read = video("room-read");
        remember_sizes(&read, sizes());
        assert_eq!(
            expected_bytes(&read, Some(&preset("best-audio"))),
            Some(5 * MB)
        );
        assert_eq!(expected_bytes(&read, None), None, "no preset");
        assert_eq!(
            expected_bytes(&video("room-never-read"), Some(&preset("best-audio"))),
            None
        );

        let shared = VideoInfo {
            files: Some(FileContents {
                count: 3,
                bytes: 7 * MB,
            }),
            ..video("room-shared")
        };
        assert_eq!(expected_bytes(&shared, None), Some(7 * MB));
        let post = VideoInfo {
            post: Some(PostContents {
                images: 2,
                videos: 0,
            }),
            ..video("room-post")
        };
        assert_eq!(expected_bytes(&post, None), None);
    }

    #[test]
    fn needs_room_for_the_download_and_the_ones_running() {
        let space = |free_megabytes: u64| DriveSpace {
            drive: "D:".to_owned(),
            free: free_megabytes * MB,
        };
        assert_eq!(shortfall(&space(2000), 1000 * MB, 0), None);
        assert_eq!(
            shortfall(&space(1100), 1000 * MB, 0),
            Some(Shortfall {
                drive: "D:".to_owned(),
                free: 1100 * MB,
                needed: 1000 * MB + 100 * MB + 50 * MB,
                others: 0,
            }),
            "the margin counts"
        );
        assert!(
            shortfall(&space(2000), 1000 * MB, 900 * MB).is_some(),
            "the running downloads' share is set aside"
        );
    }

    #[test]
    fn names_the_drive_as_people_do() {
        assert_eq!(drive_name(Path::new(r"d:\Downloads\Tsundoku")), "D:");
        assert_eq!(drive_name(Path::new(r"\\?\D:\Downloads")), "D:");
        assert_eq!(drive_name(Path::new(r"\\nas\media\Videos")), r"\\nas\media");
    }

    #[cfg(windows)]
    #[test]
    fn asks_windows_about_a_folder_not_made_yet() {
        let folder = tempfile::tempdir().unwrap();
        let space = SystemDisks
            .free(&folder.path().join("not").join("made"))
            .unwrap();
        assert!(space.free > 0);
        assert_eq!(space.drive, drive_name(folder.path()));
    }
}
