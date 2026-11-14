//! The preview card's content: what a link is (metadata.rs for yt-dlp, gallery.rs for gallery-dl, hosts for the file
//! hosts Tsundoku reads itself, listing.rs for a channel, playlist or account) and when it was last downloaded
//! (history.rs).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use ts_rs::TS;

use crate::browsers;
use crate::cancel::CancelToken;
use crate::downloads::DownloadQueue;
use crate::gallery::{self, GalleryTools};
use crate::history::{History, PastDownload};
use crate::hosts::{self, Host};
use crate::listing::{self, Found, Listing, ListingMore, MoreItems};
use crate::metadata::{self, FetchTools, Fetched, PreviewError, VideoInfo};
use crate::mirrors;
use crate::passwords::Password;
use crate::pending::LinkReader;
use crate::settings::{CookiesBrowser, SettingsStore};
use crate::short_links::{self, ShortLink};
use crate::sites;
use crate::sync::lock;
use crate::tools::{ToolCacheFolder, ToolId, ToolManager};

/// What the preview card shows for a link: one video or post, or what a channel, playlist or account holds.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum LinkPreview {
    Video(Box<VideoPreview>),
    /// Its items marked with what the history and the queue have (`listing::mark`). Boxed, as `Video`, since a
    /// listing is much bigger than the enum's other variant.
    Listing {
        listing: Box<Listing>,
    },
}

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

/// Fetches previews one at a time: starting one cancels the one before, so only the latest link is ever shown. A
/// listing's next items (`fetch_more`) are read one read at a time too, and a new preview cancels them.
#[derive(Debug, Default)]
pub struct PreviewService {
    current: Mutex<Option<CancelToken>>,
    /// The listing's next items being read, if any.
    more: Mutex<Option<CancelToken>>,
}

impl PreviewService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads what `text` links to (see `read_with_tools`), asks the history whether it was downloaded before, and the
    /// queue whether it's in it. A site that answered "too many requests" isn't read until its wait ends
    /// (`DownloadQueue::site_wait`), and an answer of that kind starts one. A link that needs a password is read with
    /// `password`, the one the person typed, or else the one the history keeps for the link, so Download Again never
    /// asks twice.
    pub fn fetch(
        &self,
        tools: &ToolManager,
        history: &History,
        queue: &DownloadQueue,
        text: &str,
        password: Option<Password>,
    ) -> Result<LinkPreview, PreviewError> {
        let link = metadata::normalize_link(text).ok_or(PreviewError::InvalidLink)?;
        let typed = password.is_some();
        let password = password.or_else(|| history.password_for_link(&link).ok().flatten());
        let cancel = CancelToken::new();
        if let Some(previous) = lock(&self.current).replace(cancel.clone()) {
            previous.cancel();
        }
        self.cancel_more();
        let fetched = match queue.site_wait(&link) {
            Some(_) => Err(PreviewError::too_many_requests(String::new())),
            None => read_for_preview(
                tools,
                queue.options().cookies_browser,
                &link,
                password.as_ref(),
                &cancel,
            )
            .map_err(|error| error.after_password(typed)),
        };
        {
            let mut current = lock(&self.current);
            if current.as_ref().is_some_and(|token| token.is_same(&cancel)) {
                *current = None;
            }
        }
        let fetched = fetched.map_err(|error| with_site_wait(error, queue, &link))?;
        queue.site_worked(&link);
        preview_of(fetched, history, queue)
    }

    /// Reads the items after those a listing on the preview card has (`more`): the next ones for Load More, or all the
    /// rest for Load All (`all`), handing each batch to `on_batch` with its items marked as `fetch` marks a listing's.
    /// Starting it stops the read before; a site that asked to slow down isn't read until its wait ends, and Load All
    /// stops once a download from the site starts such a wait (`listing::read_more`).
    pub fn fetch_more(
        &self,
        tools: &ToolManager,
        history: &History,
        queue: &DownloadQueue,
        more: &ListingMore,
        all: bool,
        on_batch: &mut dyn FnMut(MoreItems),
    ) -> Result<(), PreviewError> {
        let link = more.link.as_str();
        let cancel = CancelToken::new();
        if let Some(previous) = lock(&self.more).replace(cancel.clone()) {
            previous.cancel();
        }
        let cookies_choice = queue.options().cookies_browser;
        let read = match queue.site_wait(link) {
            Some(_) => Err(PreviewError::too_many_requests(String::new())),
            None => {
                let set = LinkTools::new(tools, cookies_choice, link);
                listing::read_more(
                    &set.videos(true, None),
                    &set.images(),
                    more,
                    all,
                    &cancel,
                    &mut |mut batch| {
                        listing::mark_items(
                            &mut batch.items,
                            |site, id| history.last_download(site, id),
                            |site, id| queue.contains(site, id),
                        )
                        .map_err(|error| PreviewError::Failed {
                            detail: error.to_string(),
                        })?;
                        let more_to_come = batch.more.is_some();
                        on_batch(batch);
                        match queue.site_wait(link) {
                            Some(_) if more_to_come => {
                                Err(PreviewError::too_many_requests(String::new()))
                            }
                            _ => Ok(()),
                        }
                    },
                )
                .map_err(|error| with_sign_in_state(error, cookies_choice, link))
            }
        };
        {
            let mut current = lock(&self.more);
            if current.as_ref().is_some_and(|token| token.is_same(&cancel)) {
                *current = None;
            }
        }
        read.map_err(|error| with_site_wait(error, queue, link))?;
        queue.site_worked(link);
        Ok(())
    }

    /// Stops the preview being fetched, if any, and the listing's next items being read.
    pub fn cancel(&self) {
        if let Some(current) = lock(&self.current).take() {
            current.cancel();
        }
        self.cancel_more();
    }

    /// Stops the listing's next items being read, if any (`fetch_more`), keeping what was read.
    pub fn cancel_more(&self) {
        if let Some(more) = lock(&self.more).take() {
            more.cancel();
        }
    }
}

/// A site's "too many requests" with when its wait ends, starting one when none is running (`DownloadQueue::slow_down`),
/// for the preview card to say when to try again.
fn with_site_wait(error: PreviewError, queue: &DownloadQueue, link: &str) -> PreviewError {
    match error {
        PreviewError::TooManyRequests { detail, .. } => {
            let wait = queue.site_wait(link).or_else(|| queue.slow_down(link));
            PreviewError::TooManyRequests {
                detail,
                site: wait.as_ref().map(|wait| wait.site.clone()),
                until: wait.map(|wait| wait.until),
            }
        }
        other => other,
    }
}

/// Reads links for the pending links as the preview card does, with the tools set up and the cookies Settings picks.
pub struct ToolReader {
    pub tools: Arc<ToolManager>,
    pub settings: Arc<SettingsStore>,
}

impl LinkReader for ToolReader {
    fn read(
        &self,
        link: &str,
        any_page: bool,
        password: Option<&Password>,
        cancel: &CancelToken,
    ) -> Result<Fetched, PreviewError> {
        let cookies_choice = self.settings.get().downloads.cookies_browser;
        read_with_tools(
            &self.tools,
            cookies_choice,
            link,
            any_page,
            password,
            cancel,
        )
    }
}

impl crate::watched::ListingReader for ToolReader {
    /// A watched source's listing, read as the preview card reads it; anything else is refused as not listable.
    fn read(&self, url: &str, cancel: &CancelToken) -> Result<Listing, PreviewError> {
        let cookies_choice = self.settings.get().downloads.cookies_browser;
        match read_for_preview(&self.tools, cookies_choice, url, None, cancel)? {
            Found::Listing(listing) => Ok(listing),
            Found::One(_) => Err(listing::not_listable(url)),
        }
    }
}

/// The preview card's content for what a link turned out to be: a video or post with whether the history or the queue
/// has it, or a listing with each of its items marked so.
fn preview_of(
    found: Found,
    history: &History,
    queue: &DownloadQueue,
) -> Result<LinkPreview, PreviewError> {
    let history_failed = |error: crate::history::HistoryError| PreviewError::Failed {
        detail: error.to_string(),
    };
    match found {
        Found::One(fetched) => {
            let video = fetched.video;
            Ok(LinkPreview::Video(Box::new(VideoPreview {
                last_download: history
                    .last_download(&video.site, &video.video_id)
                    .map_err(history_failed)?,
                in_queue: queue.contains_video(&video),
                video,
                missing_js_runtime: fetched.missing_js_runtime,
            })))
        }
        Found::Listing(mut listing) => {
            listing::mark(
                &mut listing,
                |site, id| history.last_download(site, id),
                |site, id| queue.contains(site, id),
            )
            .map_err(history_failed)?;
            Ok(LinkPreview::Listing {
                listing: Box::new(listing),
            })
        }
    }
}

/// The tools set up, and the cookies for one link, as the readers take them.
struct LinkTools {
    yt_dlp: Option<PathBuf>,
    deno: Option<PathBuf>,
    ffmpeg: Option<PathBuf>,
    gallery_dl: Option<PathBuf>,
    cookies: Option<String>,
    caches: Option<ToolCacheFolder>,
}

impl LinkTools {
    fn new(tools: &ToolManager, cookies_choice: Option<CookiesBrowser>, link: &str) -> Self {
        Self {
            yt_dlp: tools.executable(ToolId::YtDlp),
            deno: tools.executable(ToolId::Deno),
            ffmpeg: tools.executable(ToolId::Ffmpeg),
            gallery_dl: tools.executable(ToolId::GalleryDl),
            cookies: browsers::cookies_for(cookies_choice, link),
            caches: tools.cache_folder(),
        }
    }

    /// yt-dlp's, which only reads sites it knows without `any_page` (`FetchTools::any_page`), with the video's
    /// password when the link needs one.
    fn videos<'a>(&'a self, any_page: bool, password: Option<&'a Password>) -> FetchTools<'a> {
        FetchTools {
            yt_dlp: self.yt_dlp.as_deref(),
            deno: self.deno.as_deref(),
            ffmpeg: self.ffmpeg.as_deref(),
            cookies: self.cookies.clone(),
            any_page,
            caches: self.caches.as_ref(),
            password,
        }
    }

    /// gallery-dl's.
    fn images(&self) -> GalleryTools<'_> {
        GalleryTools {
            gallery_dl: self.gallery_dl.as_deref(),
            cookies: self.cookies.clone(),
            caches: self.caches.as_ref(),
        }
    }
}

/// Reads `link` (from `normalize_link`) for a pending link: a file host's itself (`hosts`), any other with the tools
/// that are set up (`read_link`). A channel, playlist or account is refused as `Playlist` without being read: the
/// preview card lists it instead. Without `any_page`, yt-dlp only reads sites it knows (`FetchTools::any_page`). When
/// it needs a signed-in account, the error says which site and whether a browser is signed in to it. `password` opens
/// a link that needs one, a file host's or a video's.
fn read_with_tools(
    tools: &ToolManager,
    cookies_choice: Option<CookiesBrowser>,
    link: &str,
    any_page: bool,
    password: Option<&Password>,
    cancel: &CancelToken,
) -> Result<Fetched, PreviewError> {
    let followed = follow(link, cancel)?;
    let link = followed.link.as_str();
    if listing::listing_link(link).is_some() {
        return Err(PreviewError::Playlist);
    }
    let set = LinkTools::new(tools, cookies_choice, link);
    // A file host Tsundoku reads itself goes there first, since its domains are exact, unless it leaves the link to
    // the tools.
    let fetched = match read_host(link, password, cancel) {
        Some(read) => read.map(|share| Fetched {
            video: share.video_info(),
            missing_js_runtime: false,
        }),
        None => read_link(
            link,
            cancel,
            |link, cancel| metadata::fetch(&set.videos(any_page, password), link, cancel),
            |link, cancel| gallery::fetch(&set.images(), link, cancel).map(from_gallery),
            set.gallery_dl.is_some(),
        ),
    };
    fetched.map_err(|error| followed.explain(with_sign_in_state(error, cookies_choice, link)))
}

/// Reads `link` (from `normalize_link`) for the preview card: as `read_with_tools` does, except that a channel,
/// playlist or account is listed (`listing::read`), and so is another site's playlist that yt-dlp answers with.
fn read_for_preview(
    tools: &ToolManager,
    cookies_choice: Option<CookiesBrowser>,
    link: &str,
    password: Option<&Password>,
    cancel: &CancelToken,
) -> Result<Found, PreviewError> {
    let followed = follow(link, cancel)?;
    let link = followed.link.as_str();
    let set = LinkTools::new(tools, cookies_choice, link);
    let host_read = if listing::listing_link(link).is_some() {
        None
    } else {
        read_host(link, password, cancel)
    };
    let found = if let Some(listing_link) = listing::listing_link(link) {
        listing::read(
            &set.videos(true, None),
            &set.images(),
            &listing_link,
            cancel,
        )
        .map(Found::Listing)
    } else if let Some(read) = host_read {
        read.map(|share| {
            Found::One(Fetched {
                video: share.video_info(),
                missing_js_runtime: false,
            })
        })
    } else {
        read_link(
            link,
            cancel,
            |link, cancel| listing::read_any(&set.videos(true, password), link, cancel),
            |link, cancel| {
                gallery::fetch(&set.images(), link, cancel)
                    .map(|video| Found::One(from_gallery(video)))
            },
            set.gallery_dl.is_some(),
        )
    };
    found.map_err(|error| followed.explain(with_sign_in_state(error, cookies_choice, link)))
}

/// The address a link is read at: where a short link leads, then the mirror a mirror page names.
#[derive(Debug, PartialEq, Eq)]
struct Followed {
    link: String,
    /// The short link it came through, if any.
    short_link: Option<ShortLink>,
}

impl Followed {
    /// The error for reading the address, naming the short link it came through when nothing downloads from where
    /// it leads.
    fn explain(&self, error: PreviewError) -> PreviewError {
        match (error, &self.short_link) {
            (PreviewError::UnsupportedSite, Some(short_link)) => {
                PreviewError::ShortLinkToUnsupportedSite {
                    site: short_link.site.to_owned(),
                    target: short_link.target_site(),
                }
            }
            (error, _) => error,
        }
    }
}

/// Where `link` (from `normalize_link`) is read: a short link's target (`short_links`), then, when that or the link
/// itself is a mirror page, its mirror's link (`mirrors`). Everything after sees only that address, as if it had
/// been pasted.
fn follow(link: &str, cancel: &CancelToken) -> Result<Followed, PreviewError> {
    let short_link = short_links::resolve(link, cancel).transpose()?;
    let link = short_link
        .as_ref()
        .map_or(link, |short_link| short_link.target.as_str());
    let mirror = mirrors::resolve(link, cancel).transpose()?;
    Ok(Followed {
        link: mirror.unwrap_or_else(|| link.to_owned()),
        short_link,
    })
}

/// Reads `link` as a file host's, or `None` when it isn't one's, or when its host leaves it to the tools: the
/// Internet Archive declines its video and audio items (`UnsupportedSite`), which yt-dlp reads with the presets.
fn read_host(
    link: &str,
    password: Option<&Password>,
    cancel: &CancelToken,
) -> Option<Result<hosts::Share, PreviewError>> {
    Host::of(link)?;
    match hosts::read(link, password, cancel) {
        Err(PreviewError::UnsupportedSite) => None,
        read => Some(read),
    }
}

/// A sign-in error says which site and whether a browser is signed in to it, for the hint on the card; Instagram's
/// "rate-limit reached or login required" is a slow-down instead while a browser is signed in to it.
fn with_sign_in_state(
    error: PreviewError,
    cookies_choice: Option<CookiesBrowser>,
    link: &str,
) -> PreviewError {
    match browsers::slow_down_if_signed_in(error, cookies_choice, link) {
        PreviewError::SignInRequired { detail, .. } => PreviewError::SignInRequired {
            detail,
            site: sites::sign_in_site(link).map(|site| site.name.to_owned()),
            cookies: browsers::sign_in_state(cookies_choice, link),
        },
        other => other,
    }
}

fn from_gallery(video: VideoInfo) -> Fetched {
    Fetched {
        video,
        missing_js_runtime: false,
    }
}

/// Reads a link with the tool that suits it, falling back to the other when the first can't handle it, so the user
/// never has to know which one did. Image sites (`sites::prefers_images`) go to gallery-dl first, the rest to
/// yt-dlp. gallery-dl is optional: without it, a link yt-dlp finds no video at may be images, which the error says.
fn read_link<T>(
    link: &str,
    cancel: &CancelToken,
    yt_dlp: impl Fn(&str, &CancelToken) -> Result<T, PreviewError>,
    gallery_dl: impl Fn(&str, &CancelToken) -> Result<T, PreviewError>,
    has_gallery_dl: bool,
) -> Result<T, PreviewError> {
    let prefers_images = sites::prefers_images(link);

    if prefers_images && has_gallery_dl {
        let gallery_error = match gallery_dl(link, cancel) {
            Ok(found) => return Ok(found),
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
                gallery_dl(link, cancel).map_err(|gallery_error| match gallery_error {
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

    #[test]
    fn the_internet_archive_leaves_video_and_audio_to_the_tools() {
        // These are declined before anything is fetched.
        for link in [
            "https://archive.org/download/night_of_the_living_dead/night_of_the_living_dead_512kb.mp4",
            "https://web.archive.org/web/2020/https://example.com/",
            "https://www.youtube.com/watch?v=jNQXAC9IVRw",
        ] {
            assert!(
                read_host(link, None, &CancelToken::new()).is_none(),
                "{link}"
            );
        }
    }
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
            account: None,
            duration_seconds: None,
            upload_date: None,
            thumbnail_url: None,
            best_height: None,
            post,
            files: None,
            post_files: Vec::new(),
            shared_files: Vec::new(),
            playlist: None,
        }
    }

    fn images() -> PostContents {
        PostContents {
            images: 2,
            videos: 0,
            others: 0,
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
                yt_dlp.clone()
            },
            |_, _| {
                asked.borrow_mut().push("gallery-dl");
                gallery_dl.clone().expect("gallery-dl isn't set up")
            },
            gallery_dl.is_some(),
        );
        (result, asked.into_inner())
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
    fn a_short_link_to_an_unknown_site_says_where_it_leads() {
        let followed = Followed {
            link: "https://www.example.com/page".to_owned(),
            short_link: Some(ShortLink {
                site: "Bitly",
                target: "https://www.example.com/page".to_owned(),
            }),
        };
        let (result, _) = read(
            &followed.link,
            Err(PreviewError::UnsupportedSite),
            Some(Err(PreviewError::UnsupportedSite)),
        );
        assert_eq!(
            followed.explain(result.unwrap_err()),
            PreviewError::ShortLinkToUnsupportedSite {
                site: "Bitly".to_owned(),
                target: "example.com".to_owned(),
            }
        );
        let unavailable = PreviewError::Unavailable {
            detail: "Video unavailable".to_owned(),
        };
        assert_eq!(
            followed.explain(unavailable.clone()),
            unavailable,
            "the target's other errors stay as they are"
        );

        let pasted = Followed {
            link: followed.link.clone(),
            short_link: None,
        };
        assert_eq!(
            pasted.explain(PreviewError::UnsupportedSite),
            PreviewError::UnsupportedSite
        );
    }

    #[test]
    fn a_link_that_is_neither_a_short_link_nor_a_mirror_page_is_read_as_it_is() {
        let link = "https://www.youtube.com/watch?v=jNQXAC9IVRw";
        assert_eq!(
            follow(link, &CancelToken::new()),
            Ok(Followed {
                link: link.to_owned(),
                short_link: None,
            })
        );
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
