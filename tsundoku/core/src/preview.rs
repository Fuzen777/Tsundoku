//! The preview card's content: what a link is (metadata.rs) and when it was last downloaded (history.rs).

use std::sync::Mutex;

use serde::Serialize;
use ts_rs::TS;

use crate::cancel::CancelToken;
use crate::downloads::DownloadQueue;
use crate::history::{History, PastDownload};
use crate::metadata::{self, FetchTools, PreviewError, VideoInfo};
use crate::sync::lock;
use crate::tools::{ToolId, ToolManager};

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VideoPreview {
    pub video: VideoInfo,
    /// The latest download of this video, when it was downloaded before.
    pub last_download: Option<PastDownload>,
    /// The video is in the download queue and not done yet.
    pub in_queue: bool,
    /// yt-dlp ran without Deno, so YouTube may have offered fewer formats.
    pub missing_js_runtime: bool,
}

/// Fetches previews one at a time: starting one cancels the one before, so only the latest link is ever shown.
#[derive(Debug, Default)]
pub struct PreviewService {
    current: Mutex<Option<CancelToken>>,
}

impl PreviewService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks yt-dlp what `text` links to, the history whether it was downloaded before, and the queue whether it's
    /// in it.
    pub fn fetch(
        &self,
        tools: &ToolManager,
        history: &History,
        queue: &DownloadQueue,
        text: &str,
    ) -> Result<VideoPreview, PreviewError> {
        let link = metadata::normalize_link(text).ok_or(PreviewError::InvalidLink)?;
        let cancel = CancelToken::new();
        if let Some(previous) = lock(&self.current).replace(cancel.clone()) {
            previous.cancel();
        }

        let yt_dlp = tools.executable(ToolId::YtDlp);
        let deno = tools.executable(ToolId::Deno);
        let ffmpeg = tools.executable(ToolId::Ffmpeg);
        let fetch_tools = FetchTools {
            yt_dlp: yt_dlp.as_deref(),
            deno: deno.as_deref(),
            ffmpeg: ffmpeg.as_deref(),
            cookies_browser: queue.options().cookies_browser,
        };
        let fetched = metadata::fetch(&fetch_tools, &link, &cancel);
        {
            let mut current = lock(&self.current);
            if current.as_ref().is_some_and(|token| token.is_same(&cancel)) {
                *current = None;
            }
        }

        let fetched = fetched?;
        let last_download = history
            .last_download(&fetched.video.site, &fetched.video.video_id)
            .map_err(|error| PreviewError::Failed {
                detail: error.to_string(),
            })?;
        Ok(VideoPreview {
            in_queue: queue.contains(&fetched.video.site, &fetched.video.video_id),
            video: fetched.video,
            last_download,
            missing_js_runtime: fetched.missing_js_runtime,
        })
    }

    /// Stops the preview being fetched, if any.
    pub fn cancel(&self) {
        if let Some(current) = lock(&self.current).take() {
            current.cancel();
        }
    }
}
