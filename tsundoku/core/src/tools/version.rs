//! Reading a tool's version from the first line it prints, which also tells whether a file is the tool expected.

use super::ToolId;

/// The version in a tool's first line of output, or `None` if the line isn't what that tool prints.
pub(crate) fn parse(id: ToolId, first_line: &str) -> Option<String> {
    let line = first_line.trim();
    let version = match id {
        // "2026.10.26", or "2026.10.30.232913" for a nightly build.
        ToolId::YtDlp => line
            .starts_with(|character: char| character.is_ascii_digit())
            .then_some(line)
            .filter(|version| {
                version.contains('.') && version.chars().all(|c| c.is_ascii_digit() || c == '.')
            }),
        // "ffmpeg version 9.0.2-full_build-www.gyan.dev Copyright (c) 2000-2026 the FFmpeg developers"
        ToolId::Ffmpeg => first_word_after(line, "ffmpeg version "),
        // "deno 2.9.8 (stable, release, x86_64-pc-windows-msvc)"
        ToolId::Deno => first_word_after(line, "deno "),
    };
    version.map(str::to_owned)
}

fn first_word_after<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    line.strip_prefix(prefix)?.split_whitespace().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_yt_dlp_versions() {
        assert_eq!(
            parse(ToolId::YtDlp, "2026.10.26\r").as_deref(),
            Some("2026.10.26")
        );
        assert_eq!(
            parse(ToolId::YtDlp, "2026.10.30.232913").as_deref(),
            Some("2026.10.30.232913")
        );
        assert_eq!(parse(ToolId::YtDlp, "Usage: yt-dlp [OPTIONS] URL"), None);
        assert_eq!(parse(ToolId::YtDlp, "2026"), None);
    }

    #[test]
    fn reads_ffmpeg_versions() {
        let gyan = "ffmpeg version 9.0.2-full_build-www.gyan.dev Copyright (c) 2000-2026 the FFmpeg developers";
        let nightly = "ffmpeg version N-127043-g5a54fcf75e-20260930 Copyright (c) 2000-2026 the FFmpeg developers";
        assert_eq!(
            parse(ToolId::Ffmpeg, gyan).as_deref(),
            Some("9.0.2-full_build-www.gyan.dev")
        );
        assert_eq!(
            parse(ToolId::Ffmpeg, nightly).as_deref(),
            Some("N-127043-g5a54fcf75e-20260930")
        );
        assert_eq!(
            parse(ToolId::Ffmpeg, "ffprobe version 9.0.2 Copyright"),
            None
        );
    }

    #[test]
    fn reads_deno_versions() {
        assert_eq!(
            parse(
                ToolId::Deno,
                "deno 2.9.8 (stable, release, x86_64-pc-windows-msvc)"
            )
            .as_deref(),
            Some("2.9.8")
        );
        assert_eq!(parse(ToolId::Deno, "v8 15.0.245.2-rusty"), None);
    }
}
