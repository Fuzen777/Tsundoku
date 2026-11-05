//! Reading a release's checksum file.

/// The SHA-256 of `asset`, in lowercase hex, from a release's checksum file. Reads both forms the tools publish:
/// `<hash>  <file name>` lines (yt-dlp's SHA2-256SUMS, FFmpeg-Builds' checksums.sha256), and the `Hash : <HASH>` of
/// PowerShell's Get-FileHash (Deno's .sha256sum, which covers a single file and doesn't name it usefully).
pub(crate) fn find(text: &str, asset: &str) -> Option<String> {
    let mut unnamed_hash = None;
    for line in text.lines() {
        let line = line.trim();
        let mut words = line.split_whitespace();
        if let (Some(first), Some(name)) = (words.next(), words.next())
            && is_sha256(first)
            && name.trim_start_matches('*') == asset
        {
            return Some(first.to_ascii_lowercase());
        }
        if let Some((key, value)) = line.split_once(':')
            && key.trim() == "Hash"
            && is_sha256(value.trim())
        {
            unnamed_hash = Some(value.trim().to_ascii_lowercase());
        }
    }
    unnamed_hash
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.chars().all(|character| character.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    const YT_DLP_SUMS: &str = "\
e51e69e89489f199530f13d32a4342da3ddb2cf34842f364603a066db05afbf3  yt-dlp
f5389d3698ecde4ef06f4e6b1ce0294fe9ac1396228c8990b8d3416d6d177a7d  yt-dlp.exe
0066dbac3c124028e2dcae2a197fccee33cd91f456313d24f7af89fd54590e98  yt-dlp_arm64.exe
";

    const DENO_SUM: &str = "\r
Algorithm : SHA256\r
Hash      : CBA84B26E0DF5AE7F74FC66B86A95B866872A86292EBD84FA1E959927EDD93BF\r
Path      : C:\\a\\deno\\deno\\target\\release\\deno-x86_64-pc-windows-msvc.zip\r
\r
";

    #[test]
    fn finds_the_asset_among_named_lines() {
        assert_eq!(
            find(YT_DLP_SUMS, "yt-dlp.exe").as_deref(),
            Some("f5389d3698ecde4ef06f4e6b1ce0294fe9ac1396228c8990b8d3416d6d177a7d")
        );
        assert_eq!(find(YT_DLP_SUMS, "yt-dlp_x86.exe"), None);
    }

    #[test]
    fn reads_binary_mode_names() {
        let line = "9bf14c31c12695e61aa406642dc7296ba402ef8c6d7b4cca414fd69e7a4287df *ffmpeg.zip";
        assert!(find(line, "ffmpeg.zip").is_some());
    }

    #[test]
    fn reads_powershell_hashes_in_lowercase() {
        assert_eq!(
            find(DENO_SUM, "deno-x86_64-pc-windows-msvc.zip").as_deref(),
            Some("cba84b26e0df5ae7f74fc66b86a95b866872a86292ebd84fa1e959927edd93bf")
        );
    }

    #[test]
    fn ignores_text_without_a_hash() {
        assert_eq!(find("Not Found", "yt-dlp.exe"), None);
        assert_eq!(find("Hash : tooshort", "yt-dlp.exe"), None);
    }
}
