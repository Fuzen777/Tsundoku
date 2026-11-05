//! Which tool reads a link first. gallery-dl goes first for sites that are mostly images, where yt-dlp finds no
//! video in a photo post; yt-dlp goes first everywhere else. Either way, the preview falls back to the other tool
//! when the first can't handle the link, so this list only decides the order.

/// Sites whose posts are mostly images, by domain. A link to one of these or a subdomain of it goes to gallery-dl
/// first.
const IMAGE_SITES: [&str; 15] = [
    "instagram.com",
    "x.com",
    "twitter.com",
    "bsky.app",
    "pixiv.net",
    "deviantart.com",
    "artstation.com",
    "tumblr.com",
    "imgur.com",
    "flickr.com",
    "pinterest.com",
    "danbooru.donmai.us",
    "gelbooru.com",
    "e621.net",
    "wallhaven.cc",
];

/// Whether gallery-dl should read `link` first. `link` must come from `metadata::normalize_link`.
pub fn prefers_images(link: &str) -> bool {
    let Some(host) = host(link) else {
        return false;
    };
    IMAGE_SITES.iter().any(|site| {
        host == *site
            || host
                .strip_suffix(site)
                .is_some_and(|prefix| prefix.ends_with('.'))
    })
}

/// The link's host in lowercase, without a port.
fn host(link: &str) -> Option<String> {
    let after_scheme = link.split_once("://")?.1;
    let authority = after_scheme.split(['/', '?', '#']).next()?;
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host.split(':').next()?;
    Some(host.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sends_image_sites_to_gallery_dl_first() {
        for link in [
            "https://www.instagram.com/p/DfPQchkG1tg/",
            "https://instagram.com/reel/DfH-Y3JugI4/",
            "https://x.com/NASA/status/2105698284354367897",
            "https://mobile.twitter.com/NASA/status/1",
            "https://WWW.PIXIV.NET/en/artworks/1",
            "https://someone.tumblr.com/post/1",
            "https://i.imgur.com/abc.jpg",
        ] {
            assert!(prefers_images(link), "{link}");
        }
    }

    #[test]
    fn sends_everything_else_to_yt_dlp_first() {
        for link in [
            "https://www.youtube.com/watch?v=jNQXAC9IVRw",
            "https://soundcloud.com/a/b",
            "https://notinstagram.com/p/1",
            "https://instagram.com.example.org/p/1",
            "https://example.com/?next=https://instagram.com/p/1",
        ] {
            assert!(!prefers_images(link), "{link}");
        }
    }
}
