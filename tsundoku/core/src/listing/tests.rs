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
        "https://x.com/NASA/status/2105698284354367897",
        "https://x.com/NASA/status/2105698284354367897/photo/1",
        "https://x.com/i/status/2105698284354367897",
        "https://x.com/NASA/likes",
        "https://x.com/NASA/followers",
        "https://x.com/home",
        "https://x.com/Explore",
        "https://x.com/search?q=nasa",
        "https://x.com/i/lists/1",
        "https://x.com/a_name_too_long_for_x",
        "https://x.com/not-a-handle",
        "https://fxtwitter.com/NASA",
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
fn lists_an_x_accounts_media_whatever_tab_the_link_opens() {
    let media = |name: &str| Some((ListingKind::Account, format!("https://x.com/{name}/media")));
    assert_eq!(listing_of("https://x.com/NASA"), media("NASA"));
    assert_eq!(listing_of("https://x.com/NASA/"), media("NASA"));
    assert_eq!(listing_of("https://x.com/NASA/media"), media("NASA"));
    assert_eq!(
        listing_of("https://twitter.com/NASA_Webb?s=20&t=abc"),
        media("NASA_Webb")
    );
    assert_eq!(
        listing_of("https://mobile.twitter.com/nasa/with_replies"),
        media("nasa")
    );
    assert_eq!(
        listing_of("https://www.x.com/NASA/highlights"),
        media("NASA")
    );
    assert_eq!(listing_of("https://x.com/a"), media("a"));
}

#[test]
fn lists_pixiv_artists_the_following_feed_and_tags() {
    let account = |link: &str| Some((ListingKind::Account, link.to_owned()));
    let artist = account("https://www.pixiv.net/users/11/artworks");
    assert_eq!(listing_of("https://www.pixiv.net/users/11"), artist);
    assert_eq!(
        listing_of("https://www.pixiv.net/en/users/11/illustrations"),
        artist
    );
    assert_eq!(listing_of("https://pixiv.net/users/11/manga"), artist);
    assert_eq!(
        listing_of("https://www.pixiv.net/users/11/bookmarks/artworks"),
        None,
        "not their works"
    );
    assert_eq!(listing_of("https://www.pixiv.net/users/abc"), None);
    assert_eq!(
        listing_of("https://www.pixiv.net/bookmark_new_illust.php"),
        account("https://www.pixiv.net/bookmark_new_illust.php")
    );
    assert_eq!(
        listing_of("https://www.pixiv.net/en/tags/%E5%8E%9F%E7%A5%9E/artworks?s_mode=s_tag&p=2"),
        account("https://www.pixiv.net/tags/%E5%8E%9F%E7%A5%9E/artworks?s_mode=s_tag")
    );
    assert_eq!(
        listing_of("https://www.pixiv.net/tags/genshin"),
        account("https://www.pixiv.net/tags/genshin/artworks")
    );
    assert_eq!(
        listing_of("https://www.pixiv.net/en/artworks/66806629"),
        None,
        "one work"
    );
    assert_eq!(
        pixiv_list_title("https://www.pixiv.net/tags/%E5%8E%9F%E7%A5%9E/artworks?s_mode=s_tag")
            .as_deref(),
        Some("#原神")
    );
    assert_eq!(
        pixiv_list_title(PIXIV_FOLLOWING).as_deref(),
        Some("Pixiv Following")
    );
    assert_eq!(
        pixiv_list_title("https://www.pixiv.net/users/11/artworks"),
        None
    );
}

#[test]
fn tells_the_kind_of_pasted_text() {
    assert_eq!(kind_of("  youtube.com/@NASA "), Some(ListingKind::Channel));
    assert_eq!(kind_of("x.com/NASA"), Some(ListingKind::Account));
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
    assert_eq!(
        listing.more,
        Some(ListingMore {
            link: "https://www.youtube.com/@NASA/videos".to_owned(),
            title: "NASA".to_owned(),
            read: 50,
            cursor: None,
            channel_id: Some("UCLA_DiR1FfKNvjuUpBHmylQ".to_owned()),
            media: None,
        }),
        "50 answered of the 50 asked: there may be more, which the channel's ID can count"
    );
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
fn names_what_its_items_were_picked_from() {
    let mut answer: Value = serde_json::from_str(&fixture("youtube-channel.json")).unwrap();
    answer["uploader_id"] = "@NASA".into();
    let channel = from_yt_dlp(
        &answer.to_string(),
        Some(ListingKind::Channel),
        "https://www.youtube.com/@NASA/videos",
    )
    .unwrap();
    let first = &channel.items[0].video;
    assert_eq!(first.playlist.as_deref(), Some("NASA"));
    assert_eq!(
        first.account.as_deref(),
        Some("NASA"),
        "the channel's handle"
    );

    let playlist = from_yt_dlp(
        &fixture("youtube-playlist.json"),
        Some(ListingKind::Playlist),
        "https://www.youtube.com/playlist?list=PLwP_SiAcdui0KVebT0mU9Apz359a4ubsC",
    )
    .unwrap();
    assert!(
        playlist
            .items
            .iter()
            .all(|item| item.video.playlist.as_deref() == Some("29C3: Not my department"))
    );

    let account = from_gallery_dl(
        &fixture("instagram-account.json"),
        "https://www.instagram.com/instagram/posts/",
        NEWEST_COUNT,
    )
    .unwrap();
    let post = &account.items[0].video;
    assert_eq!(post.playlist.as_deref(), Some("Instagram"));
    assert_eq!(post.account.as_deref(), Some("instagram"));
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
    assert_eq!(listing.media_total, None, "only an X account has one");
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
fn a_later_window_of_a_channel_says_where_the_next_one_starts() {
    let link = "https://www.youtube.com/@NASA/videos";
    let read = |start, count| {
        from_yt_dlp_window(
            &fixture("youtube-channel.json"),
            Some(ListingKind::Channel),
            link,
            Some(Window { start, count }),
        )
        .unwrap()
        .more
    };
    assert_eq!(read(50, 50).map(|more| more.read), Some(100));
    assert_eq!(read(50, 60), None, "fewer than asked: the channel's end");

    let playlist = from_yt_dlp(
        &fixture("youtube-playlist.json"),
        Some(ListingKind::Playlist),
        "https://www.youtube.com/playlist?list=PLx",
    )
    .unwrap();
    assert_eq!(playlist.more, None, "a playlist is read whole");
}

#[test]
fn reads_a_channels_video_from_its_own_line() {
    // As `--lazy-playlist --dump-json` writes each of a channel's videos (`run_entries`), trimmed.
    let line = r#"{"_type": "url", "ie_key": "Youtube", "id": "Zezt6EsG9WE",
        "url": "https://www.youtube.com/watch?v=Zezt6EsG9WE", "webpage_url": "https://www.youtube.com/watch?v=Zezt6EsG9WE",
        "title": "A Video", "duration": 558.0, "timestamp": 1792800000, "upload_date": "20261023",
        "channel": null, "uploader": null, "playlist_channel": "NASA", "playlist_uploader": "NASA",
        "playlist_uploader_id": "@NASA", "playlist": "NASA - Videos"}"#;
    let item = channel_entry(serde_json::from_str(line).unwrap(), "NASA").unwrap();
    assert_eq!(
        (item.video.site.as_str(), item.video.video_id.as_str()),
        ("youtube", "Zezt6EsG9WE")
    );
    assert_eq!(
        item.video.upload_date, None,
        "yt-dlp's day of the rough date isn't the upload's"
    );
    assert_eq!(item.approximate_date.as_deref(), Some("2026-10-24"));
    assert_eq!(item.video.uploader.as_deref(), Some("NASA"));
    assert_eq!(item.video.account.as_deref(), Some("NASA"));
    assert_eq!(item.video.playlist.as_deref(), Some("NASA"));

    let live: Value = serde_json::from_str(
        r#"{"_type": "url", "ie_key": "Youtube", "id": "b", "url": "https://www.youtube.com/watch?v=b", "live_status": "is_live"}"#,
    )
    .unwrap();
    assert!(channel_entry(live, "NASA").is_none());
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
        NEWEST_COUNT,
    )
    .unwrap();
    assert_eq!(listing.kind, ListingKind::Account);
    assert_eq!(listing.title, "Instagram");
    assert_eq!(listing.site_name, "instagram.com");
    assert_eq!(listing.url, "https://www.instagram.com/instagram/");
    assert_eq!(listing.total, Some(8634));
    assert_eq!(
        listing.media_total, None,
        "Instagram's count is of its posts"
    );
    assert!(listing.newest_only);
    assert!(listing.thumbnail_url.is_some(), "the account's picture");
    assert_eq!(listing.items.len(), 12);
    assert_eq!(listing.more, None, "fewer than the 50 asked");

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
            videos: 1,
            others: 0,
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
            videos: 4,
            others: 0,
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
fn reads_an_x_accounts_newest_posts() {
    let listing = from_gallery_dl(
        &fixture("x-account.json"),
        "https://x.com/NASA/media",
        NEWEST_COUNT,
    )
    .unwrap();
    assert_eq!(listing.kind, ListingKind::Account);
    assert_eq!(listing.title, "NASA");
    assert_eq!(listing.uploader, None);
    assert_eq!(listing.site_name, "x.com");
    assert_eq!(listing.url, "https://x.com/NASA");
    assert_eq!(
        listing_of(&listing.url),
        Some((ListingKind::Account, "https://x.com/NASA/media".to_owned())),
        "its own page lists it again, as a watched source's check does"
    );
    assert_eq!(
        listing.total, None,
        "X counts photos and videos, never posts"
    );
    assert_eq!(
        listing.media_total,
        Some(28249),
        "its photos and videos, as its Media tab says"
    );
    assert!(listing.newest_only);
    assert_eq!(
        listing.thumbnail_url.as_deref(),
        Some("https://pbs.twimg.com/profile_images/1321163587679784960/0ZxKlEKB.jpg"),
        "the account's picture"
    );
    assert_eq!(listing.items.len(), 12);

    let first = &listing.items[0].video;
    // Keyed as reading the post's own link would key it.
    assert_eq!(
        (first.site.as_str(), first.video_id.as_str()),
        ("twitter", "2121280846183137564")
    );
    assert_eq!(
        first.url, "https://x.com/NASA/status/2121280846183137564",
        "each post downloads from its own page, not the account's"
    );
    assert_eq!(first.site_name, "x.com");
    assert_eq!(first.upload_date.as_deref(), Some("2026-11-13"));
    assert_eq!(first.title, "Let's talk about Voyager 1 and 2!");
    assert_eq!(first.uploader.as_deref(), Some("NASA"));
    assert_eq!(first.account.as_deref(), Some("NASA"));
    assert_eq!(first.playlist.as_deref(), Some("NASA"));

    let urls: std::collections::HashSet<&str> = listing
        .items
        .iter()
        .map(|item| item.video.url.as_str())
        .collect();
    assert_eq!(urls.len(), 12, "every post has a page of its own");

    let images = listing
        .items
        .iter()
        .find(|item| item.video.video_id == "2119846862286930036")
        .unwrap();
    assert_eq!(
        images.video.post,
        Some(PostContents {
            images: 4,
            videos: 0,
            others: 0,
        })
    );
    assert_eq!(images.video.post_files.len(), 4);
    let video = listing
        .items
        .iter()
        .find(|item| item.video.video_id == "2119833829389041896")
        .unwrap();
    assert_eq!(
        video.video.post,
        Some(PostContents {
            images: 0,
            videos: 1,
            others: 0,
        })
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
fn an_x_account_says_how_many_photos_and_videos_it_holds_never_how_many_posts() {
    // As `x_media` hands the account over, from whichever list gave it.
    let account = AccountInfo {
        username: Some("iruri_rapulras".to_owned()),
        full_name: Some("IRURI".to_owned()),
        media_count: Some(372),
        ..AccountInfo::default()
    };
    let listing = account_listing(
        Some(account),
        "https://x.com/iruri_rapulras/media",
        Vec::new(),
        |_, total| {
            assert_eq!(total, None, "no count of posts to stop Load More at");
            None
        },
    );
    assert_eq!(listing.media_total, Some(372));
    assert_eq!(listing.total, None);

    // The UI reads it under its own name, beside an empty total.
    let json = serde_json::to_value(&listing).unwrap();
    assert_eq!(json["mediaTotal"], 372);
    assert_eq!(json["total"], Value::Null);

    // An account that says nothing has neither.
    let listing = account_listing(None, "https://x.com/someone/media", Vec::new(), |_, _| None);
    assert_eq!((listing.total, listing.media_total), (None, None));
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
    let listing = from_gallery_dl(
        &json,
        "https://www.instagram.com/someone/posts/",
        NEWEST_COUNT,
    )
    .unwrap();
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
        from_gallery_dl(
            json,
            "https://www.instagram.com/someone/posts/",
            NEWEST_COUNT
        ),
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
    let listing = from_gallery_dl(
        "[]",
        "https://www.instagram.com/someone/posts/",
        NEWEST_COUNT,
    )
    .unwrap();
    assert!(listing.items.is_empty());
    assert_eq!(listing.title, "someone");

    let listing = from_gallery_dl("[]", "https://x.com/someone/media", NEWEST_COUNT).unwrap();
    assert!(listing.items.is_empty());
    assert_eq!(listing.title, "someone");
    assert_eq!(listing.site_name, "x.com");
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
fn a_read_that_answers_every_post_asked_may_have_more() {
    let more = |asked: u32| {
        from_gallery_dl(
            &fixture("instagram-account.json"),
            "https://www.instagram.com/instagram/posts/",
            asked,
        )
        .unwrap()
        .more
        .map(|more| more.read)
    };
    // The answer holds 12 posts.
    assert_eq!(more(INSTAGRAM_PAGING.check_first), Some(12));
    assert_eq!(more(NEWEST_COUNT), None);
}

#[test]
fn a_check_of_an_account_reads_its_first_answer_then_the_newest_only_when_it_must() {
    assert_eq!(
        INSTAGRAM_PAGING.check_first, 11,
        "one less than Instagram's page of 12"
    );
    assert_eq!(
        paging_of("https://www.instagram.com/someone/posts/"),
        INSTAGRAM_PAGING
    );
    assert_eq!(
        X_PAGING.check_first, 19,
        "less than the smallest first answer X gave (21)"
    );
    assert_eq!(paging_of("https://x.com/someone/media"), X_PAGING);
    // Each answer says how many it was asked for in `total`.
    let answer = |count: u32, more: bool| Listing {
        kind: ListingKind::Account,
        title: "someone".to_owned(),
        uploader: None,
        site_name: "instagram.com".to_owned(),
        url: "https://www.instagram.com/someone/".to_owned(),
        thumbnail_url: None,
        total: Some(count),
        media_total: None,
        pace: None,
        newest_only: true,
        items: Vec::new(),
        more: more.then(|| ListingMore {
            link: "https://www.instagram.com/someone/posts/".to_owned(),
            title: "someone".to_owned(),
            read: count,
            cursor: None,
            channel_id: None,
            media: None,
        }),
        also_read: Vec::new(),
    };
    // What a check asked for, how often it paused, and which answer it kept, for whether the first answer reached
    // what was seen before and whether the account had more.
    let check = |first_reached: bool, has_more: bool, pause_works: bool| {
        let mut asked = Vec::new();
        let mut pauses = 0;
        let found = read_in_steps(
            11,
            &mut |count| {
                asked.push(count);
                Ok(answer(count, has_more))
            },
            &mut || {
                pauses += 1;
                if pause_works {
                    Ok(())
                } else {
                    Err(PreviewError::Cancelled)
                }
            },
            &|found| first_reached && found.total == Some(11),
        );
        (asked, pauses, found.map(|found| found.total))
    };
    // Most checks: the first page reaches what was seen, and that's all.
    assert_eq!(check(true, true, true), (vec![11], 0, Ok(Some(11))));
    // An account with fewer posts than a page ends there too.
    assert_eq!(check(false, false, true), (vec![11], 0, Ok(Some(11))));
    // Many new posts: after a pause, the newest 50, as the preview card reads them.
    assert_eq!(check(false, true, true), (vec![11, 50], 1, Ok(Some(50))));
    // A check stopped during the pause reads no more.
    assert_eq!(
        check(false, true, false),
        (vec![11], 1, Err(PreviewError::Cancelled))
    );
}

#[test]
fn turns_timestamps_into_days() {
    assert_eq!(date_of_timestamp(0), "1970-01-01");
    assert_eq!(date_of_timestamp(1_792_800_000), "2026-10-24");
    assert_eq!(date_of_timestamp(951_782_400), "2000-02-29");
    assert_eq!(date_of_timestamp(1_383_782_399), "2013-11-06");
    assert_eq!(date_of_timestamp(-86_400), "1969-12-31");
}

#[test]
fn counts_a_channels_videos_with_its_playlist_of_videos() {
    assert_eq!(
        videos_playlist("UCLA_DiR1FfKNvjuUpBHmylQ").as_deref(),
        Some("https://www.youtube.com/playlist?list=UULFLA_DiR1FfKNvjuUpBHmylQ")
    );
    assert_eq!(
        videos_playlist("UULFLA_DiR1FfKNvjuUpBHmylQ"),
        None,
        "not a channel's ID"
    );
    assert_eq!(
        videos_playlist("UCLA_DiR1FfKNvjuUpBHmyl"),
        None,
        "one short"
    );
    assert_eq!(videos_playlist("UCLA_DiR1FfKNvjuUpBHmyl&"), None);
    assert_eq!(videos_playlist("@NASA"), None);
}

#[test]
fn reads_whole_pages() {
    assert_eq!(whole_pages(1, 12), 12);
    assert_eq!(whole_pages(12, 12), 12);
    assert_eq!(whole_pages(13, 12), 24);
    assert_eq!(whole_pages(0, 12), 12, "at least a page");
}

/// Reads an X account as its picker does, with the network and a signed-in browser's cookies: its newest 50, then
/// Load More, printing what each read hands over and where the next one starts. With `X_LIVE_ALL=<posts>`, it then
/// loads on as Load All does until that many are listed, saying when X's limit makes it wait, which a big account
/// meets after about 1,000 posts (and then waits up to 15 minutes).
///
/// `X_LIVE=<handle> X_LIVE_GALLERY_DL=<gallery-dl.exe> X_LIVE_COOKIES=<--cookies-from-browser value>
/// cargo test -p tsundoku-core --lib x_live -- --ignored --nocapture`
#[test]
#[ignore = "reads x.com with a signed-in browser's cookies"]
fn x_live_lists_an_account() {
    let variable = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    let Some(handle) = variable("X_LIVE") else {
        println!("X_LIVE isn't set");
        return;
    };
    let gallery_dl = variable("X_LIVE_GALLERY_DL").map(std::path::PathBuf::from);
    let images = GalleryTools {
        gallery_dl: gallery_dl.as_deref(),
        cookies: variable("X_LIVE_COOKIES"),
        caches: None,
    };
    let videos = FetchTools::default();
    let cancel = CancelToken::new();
    let listing = listing_link(&format!("https://x.com/{handle}")).expect("an X account");
    let started = Instant::now();
    let seconds = |started: Instant| started.elapsed().as_secs_f32();
    let describe = |items: &[ListedItem]| {
        let id = |item: Option<&ListedItem>| item.map(|item| item.video.video_id.clone());
        format!(
            "{} posts, {:?} to {:?}",
            items.len(),
            id(items.first()),
            id(items.last())
        )
    };

    let first = read(&videos, &images, &listing, &cancel).unwrap();
    println!(
        "{:.1} s: {} ({:?} photos & videos): {}
  next: {:?}
  set aside: {}",
        seconds(started),
        first.title,
        first.media_total,
        describe(&first.items),
        first.more.as_ref().and_then(|more| more.media.as_ref()),
        first.also_read.len(),
    );
    let Some(mut more) = first.more else {
        return;
    };
    let mut listed: HashSet<String> = first
        .items
        .iter()
        .map(|item| item.video.video_id.clone())
        .collect();
    let all = variable("X_LIVE_ALL").and_then(|count| count.parse::<usize>().ok());
    for kind in [Some(MoreRead::Next), all.map(|_| MoreRead::All)]
        .into_iter()
        .flatten()
    {
        let mut next = None;
        let result = read_more(
            &videos,
            &images,
            &more,
            kind,
            &cancel,
            &mut |batch| {
                let new = batch
                    .items
                    .iter()
                    .filter(|item| listed.insert(item.video.video_id.clone()))
                    .count();
                println!(
                    "{:.1} s: {kind:?}: {}, {new} of them new, {} listed
  next: {:?}",
                    seconds(started),
                    describe(&batch.items),
                    listed.len(),
                    batch.more.as_ref().and_then(|more| more.media.as_ref()),
                );
                next = batch.more;
                if all.is_some_and(|all| listed.len() >= all) {
                    cancel.cancel();
                }
                Ok(new as u32)
            },
            &|until| println!("{:.1} s: waits until {until:?}", seconds(started)),
        );
        println!("{:.1} s: {kind:?} ended: {result:?}", seconds(started));
        match next {
            Some(next) => more = next,
            None => return,
        }
    }
}
