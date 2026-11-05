//! What the app knows about each tool: its name on PATH, how it prints its version, and where its official Windows
//! build is published.

use std::ffi::OsStr;
use std::path::Path;

use super::ToolId;

pub(crate) struct ToolSpec {
    /// The program's name, looked up on PATH.
    pub command: &'static str,
    /// The folder under `tools\` holding the app's copy.
    pub folder: &'static str,
    pub version_args: &'static [&'static str],
    pub release: ReleaseSource,
}

/// Where the official build is published: a GitHub repository's latest release.
pub(crate) struct ReleaseSource {
    pub repository: &'static str,
    pub asset: &'static str,
    /// The release file listing the asset's SHA-256.
    pub checksums: &'static str,
    pub package: Package,
}

pub(crate) enum Package {
    /// The asset is the program itself.
    Program,
    /// A zip, of which the files `keep` accepts are extracted, side by side, into the tool's folder.
    Zip { keep: fn(&Path) -> bool },
}

pub(crate) fn spec(id: ToolId) -> &'static ToolSpec {
    match id {
        ToolId::YtDlp => &YT_DLP,
        ToolId::Ffmpeg => &FFMPEG,
        ToolId::Deno => &DENO,
    }
}

/// The file name of a program: `yt-dlp.exe` on Windows.
pub(crate) fn executable_name(program: &str) -> String {
    format!("{program}{}", std::env::consts::EXE_SUFFIX)
}

static YT_DLP: ToolSpec = ToolSpec {
    command: "yt-dlp",
    folder: "yt-dlp",
    // --ignore-config, so a broken yt-dlp config file can't stop it from answering.
    version_args: &["--ignore-config", "--version"],
    release: ReleaseSource {
        repository: "yt-dlp/yt-dlp",
        asset: "yt-dlp.exe",
        checksums: "SHA2-256SUMS",
        package: Package::Program,
    },
};

/// yt-dlp's own FFmpeg builds, which it recommends. The shared build is half the size of the static one (91 MB to
/// download instead of 202) since ffmpeg and ffprobe share their DLLs instead of each carrying a copy.
static FFMPEG: ToolSpec = ToolSpec {
    command: "ffmpeg",
    folder: "ffmpeg",
    version_args: &["-version"],
    release: ReleaseSource {
        repository: "yt-dlp/FFmpeg-Builds",
        asset: "ffmpeg-master-latest-win64-gpl-shared.zip",
        checksums: "checksums.sha256",
        package: Package::Zip {
            keep: keep_ffmpeg_file,
        },
    },
};

static DENO: ToolSpec = ToolSpec {
    command: "deno",
    folder: "deno",
    version_args: &["--version"],
    release: ReleaseSource {
        repository: "denoland/deno",
        asset: "deno-x86_64-pc-windows-msvc.zip",
        checksums: "deno-x86_64-pc-windows-msvc.zip.sha256sum",
        package: Package::Zip {
            keep: keep_deno_file,
        },
    },
};

/// Everything in the build's `bin` folder (ffmpeg, ffprobe and their DLLs) except ffplay, which yt-dlp never uses.
fn keep_ffmpeg_file(path: &Path) -> bool {
    path.parent().and_then(Path::file_name) == Some(OsStr::new("bin"))
        && path.file_name() != Some(OsStr::new("ffplay.exe"))
}

fn keep_deno_file(path: &Path) -> bool {
    path.file_name() == Some(OsStr::new("deno.exe"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_ffmpeg_and_its_libraries_but_not_ffplay() {
        let keep = |path: &str| keep_ffmpeg_file(Path::new(path));
        assert!(keep("ffmpeg-master-latest-win64-gpl-shared/bin/ffmpeg.exe"));
        assert!(keep(
            "ffmpeg-master-latest-win64-gpl-shared/bin/ffprobe.exe"
        ));
        assert!(keep(
            "ffmpeg-master-latest-win64-gpl-shared/bin/avcodec-63.dll"
        ));
        assert!(!keep(
            "ffmpeg-master-latest-win64-gpl-shared/bin/ffplay.exe"
        ));
        assert!(!keep("ffmpeg-master-latest-win64-gpl-shared/LICENSE.txt"));
        assert!(!keep(
            "ffmpeg-master-latest-win64-gpl-shared/include/libavcodec/avcodec.h"
        ));
    }

    #[test]
    fn keeps_only_deno() {
        assert!(keep_deno_file(Path::new("deno.exe")));
        assert!(!keep_deno_file(Path::new("README.md")));
    }
}
