//! yt-dlp's progress, as lines Tsundoku asks it to print with `--progress-template`: a marker, then JSON.

use serde::Deserialize;

use super::{DownloadProgress, DownloadStage};

const MARKER: &str = "@@tsundoku ";

/// The `--progress-template` values: one for downloads, with the format's codecs to tell video from audio, one for
/// postprocessors (merging, converting, embedding). Only the fields Tsundoku reads are printed.
pub(crate) fn templates() -> [String; 2] {
    [
        format!(
            "download:{MARKER}{{\"stage\":\"download\",\
             \"progress\":%(progress.{{status,downloaded_bytes,total_bytes,total_bytes_estimate,speed,eta}})j,\
             \"vcodec\":%(info.vcodec)j,\"acodec\":%(info.acodec)j}}"
        ),
        format!(
            "postprocess:{MARKER}{{\"stage\":\"postprocess\",\"progress\":%(progress.{{status,postprocessor}})j}}"
        ),
    ]
}

#[derive(Deserialize)]
struct ProgressLine {
    stage: String,
    progress: ProgressFields,
    vcodec: Option<String>,
    acodec: Option<String>,
}

#[derive(Deserialize)]
struct ProgressFields {
    status: Option<String>,
    downloaded_bytes: Option<f64>,
    total_bytes: Option<f64>,
    total_bytes_estimate: Option<f64>,
    speed: Option<f64>,
    eta: Option<f64>,
    postprocessor: Option<String>,
}

/// Reads one of Tsundoku's progress lines; `None` for any other line yt-dlp prints.
pub(crate) fn parse_line(line: &str) -> Option<DownloadProgress> {
    let parsed: ProgressLine = serde_json::from_str(line.strip_prefix(MARKER)?).ok()?;
    let fields = parsed.progress;
    match parsed.stage.as_str() {
        "download" => {
            let total = fields
                .total_bytes
                .or(fields.total_bytes_estimate)
                .filter(|total| *total > 0.0);
            let finished = fields.status.as_deref() == Some("finished");
            Some(DownloadProgress {
                stage: download_stage(parsed.vcodec.as_deref(), parsed.acodec.as_deref()),
                fraction: if finished {
                    Some(1.0)
                } else {
                    fields
                        .downloaded_bytes
                        .zip(total)
                        .map(|(downloaded, total)| (downloaded / total).clamp(0.0, 1.0))
                },
                downloaded_bytes: fields.downloaded_bytes.map(to_whole),
                total_bytes: total.map(to_whole),
                speed: fields.speed,
                eta: fields.eta.map(to_whole),
            })
        }
        "postprocess" => Some(DownloadProgress {
            stage: postprocessor_stage(fields.postprocessor.as_deref().unwrap_or_default()),
            fraction: None,
            downloaded_bytes: None,
            total_bytes: None,
            speed: None,
            eta: None,
        }),
        _ => None,
    }
}

/// yt-dlp downloads a video's best video and audio formats one after the other, each with its own codecs; a format
/// with both is a single file. An unknown codec ("NA" or missing, as some sites give) can't tell which.
fn download_stage(vcodec: Option<&str>, acodec: Option<&str>) -> DownloadStage {
    match (known_codec(vcodec), known_codec(acodec)) {
        (Some("none"), _) => DownloadStage::DownloadingAudio,
        (Some(_), Some("none")) => DownloadStage::DownloadingVideo,
        _ => DownloadStage::Downloading,
    }
}

/// yt-dlp's postprocessor names, as its progress reports them.
fn postprocessor_stage(name: &str) -> DownloadStage {
    match name {
        "Merger" => DownloadStage::Merging,
        "ExtractAudio" | "VideoConvertor" | "VideoRemuxer" => DownloadStage::Converting,
        "Metadata" | "EmbedThumbnail" | "EmbedSubtitle" => DownloadStage::Embedding,
        "MoveFiles" => DownloadStage::Finishing,
        _ => DownloadStage::Processing,
    }
}

fn known_codec(codec: Option<&str>) -> Option<&str> {
    codec.filter(|codec| *codec != "NA")
}

fn to_whole(value: f64) -> u64 {
    value.max(0.0).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lines yt-dlp 2026.10.26 printed downloading jNQXAC9IVRw with Best Video, then converting it to MP3.
    const VIDEO_STARTED: &str = r#"@@tsundoku {"stage":"download","progress":{"status": "downloading", "downloaded_bytes": 1024, "total_bytes": 223779, "speed": null, "eta": null},"vcodec":"av01.0.00M.08.0.110.05.01.06.0","acodec":"none"}"#;
    const VIDEO_FINISHED: &str = r#"@@tsundoku {"stage":"download","progress":{"status": "finished", "downloaded_bytes": 223779, "total_bytes": 223779, "speed": 2758519.0910790176},"vcodec":"av01.0.00M.08.0.110.05.01.06.0","acodec":"none"}"#;
    const AUDIO: &str = r#"@@tsundoku {"stage":"download","progress":{"status": "downloading", "downloaded_bytes": 126091, "total_bytes": 252182, "speed": 655220.0299008391, "eta": 0.4},"vcodec":"none","acodec":"opus"}"#;
    const MERGER: &str = r#"@@tsundoku {"stage":"postprocess","progress":{"status": "started", "postprocessor": "Merger"}}"#;

    #[test]
    fn reads_download_progress() {
        let progress = parse_line(VIDEO_STARTED).unwrap();
        assert_eq!(progress.stage, DownloadStage::DownloadingVideo);
        assert_eq!(progress.downloaded_bytes, Some(1024));
        assert_eq!(progress.total_bytes, Some(223_779));
        assert!((progress.fraction.unwrap() - 1024.0 / 223_779.0).abs() < 1e-9);
        assert_eq!((progress.speed, progress.eta), (None, None));

        let finished = parse_line(VIDEO_FINISHED).unwrap();
        assert_eq!(finished.fraction, Some(1.0));

        let audio = parse_line(AUDIO).unwrap();
        assert_eq!(audio.stage, DownloadStage::DownloadingAudio);
        assert_eq!(audio.fraction, Some(0.5));
        assert_eq!(audio.eta, Some(0));
    }

    #[test]
    fn falls_back_on_the_estimated_size() {
        let line = r#"@@tsundoku {"stage":"download","progress":{"status":"downloading","downloaded_bytes":50,"total_bytes_estimate":200},"vcodec":"avc1","acodec":"mp4a"}"#;
        let progress = parse_line(line).unwrap();
        assert_eq!(
            progress.stage,
            DownloadStage::Downloading,
            "a format with both is one file"
        );
        assert_eq!(
            (progress.total_bytes, progress.fraction),
            (Some(200), Some(0.25))
        );
    }

    #[test]
    fn treats_unknown_codecs_as_a_plain_download() {
        let line = r#"@@tsundoku {"stage":"download","progress":{"status":"downloading","downloaded_bytes":5},"vcodec":"NA","acodec":"NA"}"#;
        let progress = parse_line(line).unwrap();
        assert_eq!(progress.stage, DownloadStage::Downloading);
        assert_eq!(progress.fraction, None, "no size, no fraction");
    }

    #[test]
    fn names_postprocessing_stages() {
        assert_eq!(parse_line(MERGER).unwrap().stage, DownloadStage::Merging);
        let stage = |name: &str| postprocessor_stage(name);
        assert_eq!(stage("ExtractAudio"), DownloadStage::Converting);
        assert_eq!(stage("EmbedThumbnail"), DownloadStage::Embedding);
        assert_eq!(stage("Metadata"), DownloadStage::Embedding);
        assert_eq!(stage("MoveFiles"), DownloadStage::Finishing);
        assert_eq!(stage("SponsorBlock"), DownloadStage::Processing);
    }

    #[test]
    fn ignores_other_lines() {
        assert!(parse_line("[download] Destination: video.mp4").is_none());
        assert!(parse_line("@@tsundoku not json").is_none());
    }

    #[test]
    fn templates_print_the_marker() {
        let [download, postprocess] = templates();
        assert!(download.starts_with("download:@@tsundoku {\"stage\":\"download\""));
        assert!(download.ends_with("\"acodec\":%(info.acodec)j}"));
        assert!(postprocess.starts_with("postprocess:@@tsundoku "));
    }
}
