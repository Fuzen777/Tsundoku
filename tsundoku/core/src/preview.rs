//! The preview card's content: what a link is (metadata.rs for yt-dlp, gallery.rs for gallery-dl, hosts for the file
//! hosts Tsundoku reads itself) and when it was last downloaded (history.rs).

use std::sync::{Arc, Mutex};

use serde::Serialize;
use ts_rs::TS;

use crate::browsers;
use crate::cancel::CancelToken;
use crate::downloads::DownloadQueue;
use crate::gallery::{self, GalleryTools};
use crate::history::{History, PastDownload};
use crate::hosts::{self, Host};
use crate::metadata::{self, FetchTools, Fetched, PreviewError, VideoInfo};
use crate::pending::LinkReader;
use crate::settings::{CookiesBrowser, SettingsStore};
use crate::sites;
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

    /// Reads what `text` links to (see `read_with_tools`), asks the history whether it was downloaded before, and the
    /// queue whether it's in it.
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
        let fetched = read_with_tools(tools, queue.options().cookies_browser, &link, &cancel);
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

/// Reads links for the pending links as the preview card does, with the tools set up and the cookies Settings picks.
pub struct ToolReader {
    pub tools: Arc<ToolManager>,
    pub settings: Arc<SettingsStore>,
}

impl LinkReader for ToolReader {
    fn read(&self, link: &str, cancel: &CancelToken) -> Result<Fetched, PreviewError> {
        let cookies_choice = self.settings.get().downloads.cookies_browser;
        read_with_tools(&self.tools, cookies_choice, link, cancel)
    }
}

/// Reads `link` (from `normalize_link`): a file host's itself (`hosts`), any other with the tools that are set up
/// (`read_link`). When it needs a signed-in account, the error says which site and whether a browser is signed in
/// to it.
fn read_with_tools(
    tools: &ToolManager,
    cookies_choice: Option<CookiesBrowser>,
    link: &str,
    cancel: &CancelToken,
) -> Result<Fetched, PreviewError> {
    let yt_dlp = tools.executable(ToolId::YtDlp);
    let deno = tools.executable(ToolId::Deno);
    let ffmpeg = tools.executable(ToolId::Ffmpeg);
    let gallery_dl = tools.executable(ToolId::GalleryDl);
    let cookies = browsers::cookies_for(cookies_choice, link);
    let fetch_tools = FetchTools {
        yt_dlp: yt_dlp.as_deref(),
        deno: deno.as_deref(),
        ffmpeg: ffmpeg.as_deref(),
        cookies: cookies.clone(),
    };
    let gallery_tools = GalleryTools {
        gallery_dl: gallery_dl.as_deref(),
        cookies,
    };
    // A file host Tsundoku reads itself goes there first, since its domains are exact.
    let fetched = match Host::of(link) {
        Some(_) => hosts::read(link, cancel).map(|share| Fetched {
            video: share.video_info(),
            missing_js_runtime: false,
        }),
        None => read_link(
            link,
            cancel,
            |link, cancel| metadata::fetch(&fetch_tools, link, cancel),
            |link, cancel| gallery::fetch(&gallery_tools, link, cancel),
            gallery_dl.is_some(),
        ),
    };
    fetched.map_err(|error| match error {
        PreviewError::SignInRequired { detail, .. } => PreviewError::SignInRequired {
            detail,
            site: sites::sign_in_site(link).map(|site| site.name.to_owned()),
            cookies: browsers::sign_in_state(cookies_choice, link),
        },
        other => other,
    })
}

/// Reads a link with the tool that suits it, falling back to the other when the first can't handle it, so the user
/// never has to know which one did. Image sites (`sites::prefers_images`) go to gallery-dl first, the rest to
/// yt-dlp. gallery-dl is optional: without it, a link yt-dlp finds no video at may be images, which the error says.
fn read_link(
    link: &str,
    cancel: &CancelToken,
    yt_dlp: impl Fn(&str, &CancelToken) -> Result<Fetched, PreviewError>,
    gallery_dl: impl Fn(&str, &CancelToken) -> Result<VideoInfo, PreviewError>,
    has_gallery_dl: bool,
) -> Result<Fetched, PreviewError> {
    let from_gallery = |video| Fetched {
        video,
        missing_js_runtime: false,
    };
    let prefers_images = sites::prefers_images(link);

    if prefers_images && has_gallery_dl {
        let gallery_error = match gallery_dl(link, cancel) {
            Ok(video) => return Ok(from_gallery(video)),
            Err(error) => error,
        };
        // gallery-dl knowing the site but failing to read it may be a post only yt-dlp reads; anything else, such as
        // a missing sign-in, would fail the same way in yt-dlp.
        if !matches!(
            gallery_error,
            PreviewError::UnsupportedSite
                | PreviewError::SiteError { .. }
                | PreviewError::Failed { .. }
        ) {
            return Err(gallery_error);
        }
        return yt_dlp(link, cancel).map_err(|yt_dlp_error| match gallery_error {
            PreviewError::UnsupportedSite => yt_dlp_error,
            _ => gallery_error,
        });
    }

    match yt_dlp(link, cancel) {
        Err(error @ (PreviewError::UnsupportedSite | PreviewError::NoVideo { .. })) => {
            if has_gallery_dl {
                gallery_dl(link, cancel)
                    .map(from_gallery)
                    .map_err(|gallery_error| match gallery_error {
                        // Neither tool knows the link: yt-dlp's answer says it best.
                        PreviewError::UnsupportedSite => error,
                        other => other,
                    })
            } else if prefers_images || matches!(error, PreviewError::NoVideo { .. }) {
                Err(PreviewError::ImagesNotSetUp)
            } else {
                Err(error)
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::metadata::PostContents;

    const PHOTO: &str = "https://www.instagram.com/p/DfPQchkG1tg/";
    const VIDEO: &str = "https://www.youtube.com/watch?v=jNQXAC9IVRw";

    fn info(id: &str, post: Option<PostContents>) -> VideoInfo {
        VideoInfo {
            site: "site".to_owned(),
            site_name: "site.com".to_owned(),
            video_id: id.to_owned(),
            url: String::new(),
            title: id.to_owned(),
            uploader: None,
            duration_seconds: None,
            upload_date: None,
            thumbnail_url: None,
            best_height: None,
            post,
            files: None,
        }
    }

    fn images() -> PostContents {
        PostContents {
            images: 2,
            videos: 0,
        }
    }

    /// Runs `read_link` with tools answering as given, and returns its answer and the tools asked, in order.
    fn read(
        link: &str,
        yt_dlp: Result<VideoInfo, PreviewError>,
        gallery_dl: Option<Result<VideoInfo, PreviewError>>,
    ) -> (Result<VideoInfo, PreviewError>, Vec<&'static str>) {
        let asked = RefCell::new(Vec::new());
        let result = read_link(
            link,
            &CancelToken::new(),
            |_, _| {
                asked.borrow_mut().push("yt-dlp");
                yt_dlp.clone().map(|video| Fetched {
                    video,
                    missing_js_runtime: false,
                })
            },
            |_, _| {
                asked.borrow_mut().push("gallery-dl");
                gallery_dl.clone().expect("gallery-dl isn't set up")
            },
            gallery_dl.is_some(),
        );
        (result.map(|fetched| fetched.video), asked.into_inner())
    }

    fn no_video() -> PreviewError {
        PreviewError::NoVideo {
            detail: "No video formats found!".to_owned(),
        }
    }

    #[test]
    fn image_sites_go_to_gallery_dl_first() {
        let (result, asked) = read(
            PHOTO,
            Err(no_video()),
            Some(Ok(info("post", Some(images())))),
        );
        assert_eq!(result.unwrap().video_id, "post");
        assert_eq!(asked, ["gallery-dl"]);
    }

    #[test]
    fn other_sites_go_to_yt_dlp_first() {
        let (result, asked) = read(VIDEO, Ok(info("video", None)), Some(Ok(info("post", None))));
        assert_eq!(result.unwrap().video_id, "video");
        assert_eq!(asked, ["yt-dlp"]);
    }

    #[test]
    fn falls_back_to_gallery_dl_when_yt_dlp_finds_no_video() {
        let link = "https://unlisted-image-site.example/post/1";
        let (result, asked) = read(
            link,
            Err(no_video()),
            Some(Ok(info("post", Some(images())))),
        );
        assert_eq!(result.unwrap().video_id, "post");
        assert_eq!(asked, ["yt-dlp", "gallery-dl"]);

        let (result, _) = read(
            link,
            Err(PreviewError::UnsupportedSite),
            Some(Ok(info("post", Some(images())))),
        );
        assert_eq!(result.unwrap().video_id, "post");
    }

    #[test]
    fn falls_back_to_yt_dlp_when_gallery_dl_cant_read_the_post() {
        let site_error = PreviewError::SiteError {
            tool: ToolId::GalleryDl,
            detail: "KeyError".to_owned(),
        };
        let (result, asked) = read(PHOTO, Ok(info("reel", None)), Some(Err(site_error.clone())));
        assert_eq!(result.unwrap().video_id, "reel");
        assert_eq!(asked, ["gallery-dl", "yt-dlp"]);

        // When both fail, gallery-dl's error is the one shown, since the site is one it knows.
        let (result, _) = read(PHOTO, Err(no_video()), Some(Err(site_error.clone())));
        assert_eq!(result.unwrap_err(), site_error);
    }

    #[test]
    fn a_missing_sign_in_isnt_retried_with_the_other_tool() {
        let sign_in = PreviewError::sign_in("login page".to_owned());
        let (result, asked) = read(PHOTO, Ok(info("reel", None)), Some(Err(sign_in.clone())));
        assert_eq!(result.unwrap_err(), sign_in);
        assert_eq!(asked, ["gallery-dl"]);
    }

    #[test]
    fn an_unknown_site_says_so() {
        let (result, asked) = read(
            "https://example.com/page",
            Err(PreviewError::UnsupportedSite),
            Some(Err(PreviewError::UnsupportedSite)),
        );
        assert_eq!(result.unwrap_err(), PreviewError::UnsupportedSite);
        assert_eq!(asked, ["yt-dlp", "gallery-dl"]);
    }

    #[test]
    fn without_gallery_dl_a_photo_asks_for_it() {
        let (result, asked) = read(PHOTO, Err(no_video()), None);
        assert_eq!(result.unwrap_err(), PreviewError::ImagesNotSetUp);
        assert_eq!(asked, ["yt-dlp"]);

        // A reel still works with yt-dlp alone.
        let (result, _) = read(PHOTO, Ok(info("reel", None)), None);
        assert_eq!(result.unwrap().video_id, "reel");

        // Elsewhere, an unknown site stays an unknown site.
        let (result, _) = read(
            "https://example.com/page",
            Err(PreviewError::UnsupportedSite),
            None,
        );
        assert_eq!(result.unwrap_err(), PreviewError::UnsupportedSite);
    }
}
