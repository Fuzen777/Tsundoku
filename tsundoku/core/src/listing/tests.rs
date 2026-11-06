use super::*;
use crate::metadata::PostContents;

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/listing/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn listing_of(link: &str) -> Option<(ListingKind, String)> {
    listing_link(link).map(|listing| (listing.kind, listing.link))
}

#[test]
fn lists_a_youtube_channels_videos_whatever_tab_the_link_opens() {
    let videos = |path: &str| {
        Some((
            ListingKind::Channel,
            format!("https://www.youtube.com/{path}/videos"),
        ))
    };
    assert_eq!(listing_of("https://www.youtube.com/@NASA"), videos("@NASA"));
    assert_eq!(
        listing_of("https://youtube.com/@NASA/shorts?si=abc"),
        videos("@NASA")
    );
    assert_eq!(
        listing_of("https://m.youtube.com/@NASA/streams"),
        videos("@NASA")
    );
    assert_eq!(
        listing_of("https://www.youtube.com/channel/UCLA_DiR1FfKNvjuUpBHmylQ/featured"),
        videos("channel/UCLA_DiR1FfKNvjuUpBHmylQ")
    );
    assert_eq!(
        listing_of("https://www.youtube.com/c/NASA"),
        videos("c/NASA")
    );
    assert_eq!(
        listing_of("https://www.youtube.com/user/NASAtelevision/videos"),
        videos("user/NASAtelevision")
    );
}

#[test]
fn lists_a_youtube_playlist() {
    assert_eq!(
        listing_of("https://www.youtube.com/playlist?list=PLwP_SiAcdui0KVebT0mU9Apz359a4ubsC&si=x"),
        Some((
            ListingKind::Playlist,
            "https://www.youtube.com/playlist?list=PLwP_SiAcdui0KVebT0mU9Apz359a4ubsC".to_owned()
        ))
    );
    assert_eq!(listing_of("https://www.youtube.com/playlist"), None);
}

#[test]
fn a_video_stays_a_video() {
    for link in [
        "https://www.youtube.com/watch?v=jNQXAC9IVRw&list=PL4lCao7KL_QFVb7Iudeipvc2BCavECqzc",
        "https://www.youtube.com/watch?v=jNQXAC9IVRw",
        "https://www.youtube.com/shorts/jNQXAC9IVRw",
        "https://youtu.be/jNQXAC9IVRw",
        "https://www.youtube.com/@",
        "https://www.youtube.com/results?search_query=nasa",
        "https://www.instagram.com/p/DfPQchkG1tg/",
        "https://www.instagram.com/reel/DfH-Y3JugI4/",
        "https://www.instagram.com/stories/instagram/",
        "https://www.instagram.com/instagram/tagged/",
        "https://www.instagram.com/explore/",
        "https://x.com/NASA",
        "https://soundcloud.com/the-concept-band/sets/the-royal-concept-ep",
    ] {
        assert_eq!(listing_of(link), None, "{link}");
    }
}

#[test]
fn lists_an_instagram_accounts_posts() {
    let posts = Some((
        ListingKind::Account,
        "https://www.instagram.com/instagram/posts/".to_owned(),
    ));
    assert_eq!(listing_of("https://www.instagram.com/instagram/"), posts);
    assert_eq!(
        listing_of("https://instagram.com/instagram?igsh=abc"),
        posts
    );
    assert_eq!(
        listing_of("https://www.instagram.com/instagram/reels/"),
        posts
    );
    assert_eq!(
        listing_of("https://www.instagram.com/instagram/posts"),
        posts
    );
    assert_eq!(
        listing_of("https://www.instagram.com/nat.geo_/"),
        Some((
            ListingKind::Account,
            "https://www.instagram.com/nat.geo_/posts/".to_owned()
        ))
    );
}

#[test]
fn tells_the_kind_of_pasted_text() {
    assert_eq!(kind_of("  youtube.com/@NASA "), Some(ListingKind::Channel));
    assert_eq!(kind_of("youtu.be/jNQXAC9IVRw"), None);
    assert_eq!(kind_of("not a link"), None);
}

#[test]
fn reads_a_youtube_channels_newest_videos() {
    let listing = from_yt_dlp(
        &fixture("youtube-channel.json"),
        Some(ListingKind::Channel),
        "https://www.youtube.com/@NASA/videos",
    )
    .unwrap();
    assert_eq!(listing.kind, ListingKind::Channel);
    assert_eq!(
        listing.title, "NASA",
        "the channel's name, not \"NASA - Videos\""
    );
    assert_eq!(listing.uploader, None);
    assert_eq!(listing.site_name, "youtube.com");
    assert_eq!(listing.url, "https://www.youtube.com/@NASA/videos");
    assert!(listing.newest_only);
    assert_eq!(listing.total, None);
    assert_eq!(listing.items.len(), 50);
    assert!(
        listing
            .thumbnail_url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://yt3.googleusercontent.com/")),
        "the channel's avatar"
    );

    let first = &listing.items[0];
    assert_eq!(
        (first.video.site.as_str(), first.video.video_id.as_str()),
        ("youtube", "Zezt6EsG9WE"),
        "keyed as reading the video's own link would key it"
    );
    assert_eq!(
        first.video.url,
        "https://www.youtube.com/watch?v=Zezt6EsG9WE"
    );
    assert_eq!(first.video.site_name, "youtube.com");
    assert_eq!(
        first.video.uploader.as_deref(),
        Some("NASA"),
        "the channel's videos are by the channel"
    );
    assert_eq!(first.video.duration_seconds, Some(558.0));
    assert_eq!(
        first.video.upload_date, None,
        "YouTube's date is only rough"
    );
    assert_eq!(first.approximate_date.as_deref(), Some("2026-10-24"));
    assert!(
        first
            .video
            .thumbnail_url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://i.ytimg.com/vi/Zezt6EsG9WE/"))
    );
    assert_eq!(first.video.post, None);
    assert_eq!(first.last_download, None);
    assert!(!first.in_queue);
}

#[test]
fn reads_a_youtube_playlist_whole() {
    let link = "https://www.youtube.com/playlist?list=PLwP_SiAcdui0KVebT0mU9Apz359a4ubsC";
    let listing = from_yt_dlp(
        &fixture("youtube-playlist.json"),
        Some(ListingKind::Playlist),
        link,
    )
    .unwrap();
    assert_eq!(listing.kind, ListingKind::Playlist);
    assert_eq!(listing.title, "29C3: Not my department");
    assert_eq!(listing.uploader.as_deref(), Some("Christiaan008"));
    assert!(!listing.newest_only);
    assert_eq!(listing.total, Some(96));
    assert_eq!(
        listing.items.len(),
        95,
        "a video the playlist holds twice is listed once"
    );
    assert_eq!(listing.items[0].video.video_id, "6ueeeG0TBjs");
    assert_eq!(
        listing.items[0].video.uploader.as_deref(),
        Some("Christiaan008")
    );
    assert!(listing.thumbnail_url.is_some());
}

#[test]
fn reads_other_sites_playlists() {
    let album = from_yt_dlp(
        &fixture("bandcamp-album.json"),
        None,
        "https://nightbringer.bandcamp.com/album/hierophany-of-the-open-grave",
    )
    .unwrap();
    assert_eq!(album.kind, ListingKind::Playlist);
    assert_eq!(album.title, "Hierophany of the Open Grave");
    assert_eq!(album.items.len(), 9);
    let track = &album.items[0].video;
    // Reading the track's own link gives this site and ID too.
    assert_eq!(
        (track.site.as_str(), track.video_id.as_str()),
        ("bandcamp", "1399739925")
    );
    assert_eq!(track.title, "Rite of the Slaying Tongue");
    assert_eq!(track.site_name, "nightbringer.bandcamp.com");

    let set = from_yt_dlp(
        &fixture("soundcloud-set.json"),
        None,
        "https://soundcloud.com/the-concept-band/sets/the-royal-concept-ep",
    )
    .unwrap();
    assert_eq!(set.uploader.as_deref(), Some("The Royal Concept"));
    assert_eq!(set.items.len(), 6);
    let first = &set.items[0].video;
    assert_eq!(
        (first.site.as_str(), first.video_id.as_str()),
        ("soundcloud", "75206121")
    );
    assert_eq!(
        first.title, "world-on-fire-1",
        "named by its link when the listing gives no title"
    );
}

#[test]
fn refuses_a_listing_of_listings() {
    // A channel's home page read as any link: its entries are its tabs.
    assert_eq!(
        from_yt_dlp(
            &fixture("youtube-channel-tabs.json"),
            None,
            "https://www.youtube.com/NASA"
        ),
        Err(PreviewError::NotListable {
            site: "YouTube".to_owned()
        })
    );
}

#[test]
fn refuses_items_without_a_link_of_their_own() {
    let refused = |entries: &str| {
        let json = format!(
            r#"{{"_type": "playlist", "title": "A post", "extractor_key": "Example", "entries": [{entries}]}}"#
        );
        from_yt_dlp(&json, None, "https://example.com/post/1")
    };
    let not_listable = Err(PreviewError::NotListable {
        site: "example.com".to_owned(),
    });
    // A file's address rather than a page.
    assert_eq!(
        refused(
            r#"{"_type": "video", "id": "1", "url": "https://cdn.example.com/1.mp4", "extractor_key": "Example"}"#
        ),
        not_listable
    );
    // No site to key it by.
    assert_eq!(
        refused(r#"{"_type": "url", "id": "1", "url": "https://example.com/v/1"}"#),
        not_listable
    );
    // Two videos of one post.
    assert_eq!(
        refused(
            r#"{"id": "1", "webpage_url": "https://example.com/post/1", "extractor_key": "Example"},
               {"id": "2", "webpage_url": "https://example.com/post/1", "extractor_key": "Example"}"#
        ),
        not_listable
    );
}

#[test]
fn leaves_out_premieres_and_live_streams() {
    let json = r#"{"_type": "playlist", "title": "Channel - Videos", "channel": "Channel", "extractor_key": "YoutubeTab",
        "entries": [
            {"_type": "url", "ie_key": "Youtube", "id": "a", "url": "https://www.youtube.com/watch?v=a", "live_status": "is_upcoming"},
            {"_type": "url", "ie_key": "Youtube", "id": "b", "url": "https://www.youtube.com/watch?v=b", "live_status": "is_live"},
            {"_type": "url", "ie_key": "Youtube", "id": "c", "url": "https://www.youtube.com/watch?v=c", "timestamp": 1792800000}
        ]}"#;
    let listing = from_yt_dlp(
        json,
        Some(ListingKind::Channel),
        "https://www.youtube.com/@Channel/videos",
    )
    .unwrap();
    let ids: Vec<&str> = listing
        .items
        .iter()
        .map(|item| item.video.video_id.as_str())
        .collect();
    assert_eq!(ids, ["c"]);
    assert_eq!(listing.title, "Channel");
}

#[test]
fn reads_an_instagram_accounts_newest_posts() {
    let listing = from_gallery_dl(
        &fixture("instagram-account.json"),
        "https://www.instagram.com/instagram/posts/",
    )
    .unwrap();
    assert_eq!(listing.kind, ListingKind::Account);
    assert_eq!(listing.title, "Instagram");
    assert_eq!(listing.site_name, "instagram.com");
    assert_eq!(listing.url, "https://www.instagram.com/instagram/");
    assert_eq!(listing.total, Some(8634));
    assert!(listing.newest_only);
    assert!(listing.thumbnail_url.is_some(), "the account's picture");
    assert_eq!(listing.items.len(), 12);

    let first = &listing.items[0].video;
    // Keyed as reading the reel's own link, with either tool, would key it.
    assert_eq!(
        (first.site.as_str(), first.video_id.as_str()),
        ("instagram", "DfXV5VqPw0P")
    );
    assert_eq!(first.url, "https://www.instagram.com/reel/DfXV5VqPw0P/");
    assert_eq!(first.upload_date.as_deref(), Some("2026-11-05"));
    assert_eq!(listing.items[0].approximate_date, None);
    assert_eq!(first.title, "let the (fashion) games begin 🎀🔍");
    assert_eq!(
        first.post,
        Some(PostContents {
            images: 0,
            videos: 1
        })
    );

    let carousel = listing
        .items
        .iter()
        .find(|item| item.video.video_id == "DfPQchkG1tg")
        .unwrap();
    assert_eq!(
        carousel.video.post,
        Some(PostContents {
            images: 2,
            videos: 4
        }),
        "the same as the carousel's own link gives"
    );

    let dates: Vec<_> = listing
        .items
        .iter()
        .map(|item| item.video.upload_date.clone().unwrap())
        .collect();
    let mut newest_first = dates.clone();
    newest_first.sort_by(|first, second| second.cmp(first));
    assert_eq!(dates, newest_first);
}

#[test]
fn puts_pinned_posts_where_their_date_goes() {
    let post = |id: &str, date: &str| {
        format!(
            r#"[2, {{"category": "instagram", "post_shortcode": "{id}", "date": "{date} 10:00:00",
                  "post_url": "https://www.instagram.com/p/{id}/", "user": {{"username": "someone", "media_count": 9}}}}],
               [3, "https://cdn.example/{id}.jpg", {{"category": "instagram", "post_shortcode": "{id}", "extension": "jpg"}}]"#
        )
    };
    let json = format!(
        "[{}, {}, {}]",
        post("pinned", "2020-01-01"),
        post("newest", "2026-11-05"),
        post("older", "2026-10-01")
    );
    let listing = from_gallery_dl(&json, "https://www.instagram.com/someone/posts/").unwrap();
    let ids: Vec<&str> = listing
        .items
        .iter()
        .map(|item| item.video.video_id.as_str())
        .collect();
    assert_eq!(ids, ["newest", "older", "pinned"]);
    assert_eq!(listing.title, "someone");
    assert_eq!(listing.url, "https://www.instagram.com/someone/");
}

#[test]
fn an_account_needing_a_sign_in_says_so() {
    let json = r#"[[-1, {"error": "AbortExtraction", "message": "HTTP redirect to login page (https://www.instagram.com/accounts/login/)"}]]"#;
    assert!(matches!(
        from_gallery_dl(json, "https://www.instagram.com/someone/posts/"),
        Err(PreviewError::SignInRequired { .. })
    ));
}

#[test]
fn an_account_read_signed_out_asks_for_a_sign_in() {
    // What Instagram's account pages answer gallery-dl without cookies.
    let failure = PreviewError::Network {
        detail: "KeyError: 'PolarisProfilePostsTabContentQuery_connection'".to_owned(),
    };
    assert!(matches!(
        signed_out_failure(failure.clone(), false),
        PreviewError::SignInRequired { .. }
    ));
    assert_eq!(
        signed_out_failure(failure.clone(), true),
        failure,
        "with cookies, it's a failure of its own"
    );
    assert_eq!(
        signed_out_failure(PreviewError::ImagesNotSetUp, false),
        PreviewError::ImagesNotSetUp
    );
    assert_eq!(
        signed_out_failure(PreviewError::Cancelled, false),
        PreviewError::Cancelled
    );
}

#[test]
fn an_empty_account_is_an_empty_listing() {
    let listing = from_gallery_dl("[]", "https://www.instagram.com/someone/posts/").unwrap();
    assert!(listing.items.is_empty());
    assert_eq!(listing.title, "someone");
}

#[test]
fn marks_what_the_history_and_the_queue_have() {
    let mut listing = from_yt_dlp(
        &fixture("youtube-playlist.json"),
        Some(ListingKind::Playlist),
        "https://www.youtube.com/playlist?list=PLwP_SiAcdui0KVebT0mU9Apz359a4ubsC",
    )
    .unwrap();
    let downloaded = PastDownload {
        id: 7,
        preset: "Best Video".to_owned(),
        file_path: r"D:\Videos\a.mkv".to_owned(),
        file_size: None,
        format: None,
        downloaded_at: 0,
        file_exists: true,
        file_count: 1,
    };
    mark(
        &mut listing,
        |site, id| -> Result<_, ()> {
            Ok((site == "youtube" && id == "6ueeeG0TBjs").then(|| downloaded.clone()))
        },
        |site, id| site == "youtube" && id == "6RR00hxX7W0",
    )
    .unwrap();
    assert_eq!(listing.items[0].last_download, Some(downloaded));
    assert!(!listing.items[0].in_queue);
    assert!(listing.items[1].in_queue);
    assert_eq!(listing.items[1].last_download, None);
    assert!(
        listing.items[2..]
            .iter()
            .all(|item| item.last_download.is_none() && !item.in_queue)
    );
}

#[test]
fn a_check_stops_once_known_items_outnumber_the_pinned_ones() {
    // Uppercase is known. Each stops at the item that makes more than PINNED_AT_MOST known in a row.
    let check = |items: &str| {
        let found = before_known_ground(items.chars(), |item| item.is_ascii_uppercase());
        (
            found.new.into_iter().collect::<String>(),
            found.reached_known,
        )
    };
    assert_eq!(PINNED_AT_MOST, 3);
    // New items, then what was seen before; what follows isn't looked at.
    assert_eq!(check("abcDEFGhij"), ("abc".to_owned(), true));
    // Three known posts pinned above the newest, as Instagram allows, don't end it; nor does X's one.
    assert_eq!(check("XYZabDEFG"), ("ab".to_owned(), true));
    assert_eq!(check("XabcDEFG"), ("abc".to_owned(), true));
    // A known item among new ones is skipped.
    assert_eq!(check("aBcDEFeGHIJk"), ("ace".to_owned(), true));
    // Nothing known for long enough: every item is new, and more may lie beyond the listing.
    assert_eq!(check("abc"), ("abc".to_owned(), false));
    assert_eq!(check("abDEF"), ("ab".to_owned(), false));
    assert_eq!(check(""), ("".to_owned(), false));
    assert_eq!(check("WXYZ"), ("".to_owned(), true));
}

#[test]
fn turns_timestamps_into_days() {
    assert_eq!(date_of_timestamp(0), "1970-01-01");
    assert_eq!(date_of_timestamp(1_792_800_000), "2026-10-24");
    assert_eq!(date_of_timestamp(951_782_400), "2000-02-29");
    assert_eq!(date_of_timestamp(1_383_782_399), "2013-11-06");
    assert_eq!(date_of_timestamp(-86_400), "1969-12-31");
}
