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

/// Sites that limit how fast an account reads them: a post's files can be downloaded with a pause between them, and
/// their links in Pending read with a pause between them too (`DownloadSettings::pause_for`). Instagram always is,
/// the others when Settings → Pacing gives them a pause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PacedSite {
    Instagram,
    X,
    Pixiv,
    Tumblr,
    Fanbox,
}

impl PacedSite {
    const ALL: [Self; 5] = [
        Self::Instagram,
        Self::X,
        Self::Pixiv,
        Self::Tumblr,
        Self::Fanbox,
    ];

    /// The site's domains: its links, and their subdomains.
    fn domains(self) -> &'static [&'static str] {
        match self {
            Self::Instagram => &["instagram.com"],
            Self::X => &["x.com", "twitter.com"],
            Self::Pixiv => &["pixiv.net"],
            Self::Tumblr => &["tumblr.com"],
            Self::Fanbox => &["fanbox.cc"],
        }
    }
}

/// A site where an account changes what can be read, and the cookies that show a browser is signed in to it.
#[derive(Debug, PartialEq, Eq)]
pub struct SignInSite {
    /// The name shown to the user.
    pub name: &'static str,
    /// The site's domains: its links, and the hosts its cookies are set for.
    pub domains: &'static [&'static str],
    /// The site's name in yt-dlp's and gallery-dl's lists of supported sites, in lowercase.
    pub extractor: &'static str,
    /// Cookies that only exist while signed in. Only their names are ever read, never their values.
    pub cookies: &'static [&'static str],
}

/// Sites whose sign-in Tsundoku can tell from a browser's cookies.
pub const SIGN_IN_SITES: [SignInSite; 9] = [
    SignInSite {
        name: "Instagram",
        domains: &["instagram.com"],
        extractor: "instagram",
        cookies: &["sessionid"],
    },
    SignInSite {
        name: "X",
        domains: &["x.com", "twitter.com"],
        extractor: "twitter",
        cookies: &["auth_token"],
    },
    SignInSite {
        name: "YouTube",
        domains: &["youtube.com", "youtu.be"],
        extractor: "youtube",
        cookies: &["LOGIN_INFO"],
    },
    SignInSite {
        name: "TikTok",
        domains: &["tiktok.com"],
        extractor: "tiktok",
        cookies: &["sessionid"],
    },
    SignInSite {
        name: "Reddit",
        domains: &["reddit.com"],
        extractor: "reddit",
        cookies: &["reddit_session"],
    },
    SignInSite {
        name: "Facebook",
        domains: &["facebook.com"],
        extractor: "facebook",
        cookies: &["c_user"],
    },
    SignInSite {
        name: "Twitch",
        domains: &["twitch.tv"],
        extractor: "twitch",
        cookies: &["auth-token"],
    },
    SignInSite {
        name: "Niconico",
        domains: &["nicovideo.jp"],
        extractor: "niconico",
        cookies: &["user_session"],
    },
    SignInSite {
        name: "Bilibili",
        domains: &["bilibili.com"],
        extractor: "bilibili",
        cookies: &["SESSDATA"],
    },
];

/// Whether gallery-dl should read `link` first. `link` must come from `metadata::normalize_link`.
pub fn prefers_images(link: &str) -> bool {
    let Some(host) = host(link) else {
        return false;
    };
    IMAGE_SITES.iter().any(|site| on_domain(&host, site))
}

/// The site `link` is on, if it's one that may be paced (`PacedSite`).
pub fn paced_site(link: &str) -> Option<PacedSite> {
    let host = host(link)?;
    PacedSite::ALL
        .into_iter()
        .find(|site| site.domains().iter().any(|domain| on_domain(&host, domain)))
}

/// The sign-in site `link` belongs to, if any.
pub fn sign_in_site(link: &str) -> Option<&'static SignInSite> {
    let host = host(link)?;
    SIGN_IN_SITES
        .iter()
        .find(|site| site.domains.iter().any(|domain| on_domain(&host, domain)))
}

/// A site as far as its limits go (`site_limits`): what a "too many requests" from one of its links holds back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    /// The same for every domain the site goes by: "x.com" for a twitter.com link too.
    pub key: String,
    /// What the user knows it by: "Instagram", "MEGA", or its domain ("vimeo.com").
    pub name: String,
}

/// The site `link` belongs to: a sign-in site or a file host by its name, any other by its domain's last two labels
/// ("player.vimeo.com" → "vimeo.com"). `link` must come from `metadata::normalize_link`.
pub fn site_of(link: &str) -> Option<Site> {
    if let Some(site) = sign_in_site(link) {
        return Some(Site {
            key: site.domains[0].to_owned(),
            name: site.name.to_owned(),
        });
    }
    if let Some(host) = crate::hosts::Host::of(link) {
        return Some(Site {
            key: host.site().to_owned(),
            name: host.name().to_owned(),
        });
    }
    let domain = cookie_domains(link).pop()?;
    (!domain.is_empty()).then(|| Site {
        key: domain.clone(),
        name: domain,
    })
}

/// Whether `host` is `domain` or one of its subdomains. A cookie host's leading dot is ignored.
pub(crate) fn on_domain(host: &str, domain: &str) -> bool {
    let host = host.strip_prefix('.').unwrap_or(host);
    host.eq_ignore_ascii_case(domain)
        || host.len() > domain.len()
            && host.as_bytes()[host.len() - domain.len() - 1] == b'.'
            && host[host.len() - domain.len()..].eq_ignore_ascii_case(domain)
}

/// The domains whose cookies count for `link`: the sign-in site's domains, or else the host's last two labels
/// ("www.example.com" → "example.com").
pub(crate) fn cookie_domains(link: &str) -> Vec<String> {
    if let Some(site) = sign_in_site(link) {
        return site
            .domains
            .iter()
            .map(|domain| (*domain).to_owned())
            .collect();
    }
    let Some(host) = host(link) else {
        return Vec::new();
    };
    let labels: Vec<&str> = host.split('.').collect();
    let start = labels.len().saturating_sub(2);
    vec![labels[start..].join(".")]
}

/// The link's host in lowercase, without a port.
pub(crate) fn host(link: &str) -> Option<String> {
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
    fn knows_the_sites_that_may_be_paced() {
        assert_eq!(
            paced_site("https://www.instagram.com/p/DfPQchkG1tg/"),
            Some(PacedSite::Instagram)
        );
        assert_eq!(
            paced_site("https://x.com/NASA/status/2105698284354367897"),
            Some(PacedSite::X)
        );
        assert_eq!(
            paced_site("https://twitter.com/NASA/status/1"),
            Some(PacedSite::X)
        );
        assert_eq!(
            paced_site("https://www.pixiv.net/en/artworks/1"),
            Some(PacedSite::Pixiv)
        );
        assert_eq!(
            paced_site("https://someone.tumblr.com/post/1"),
            Some(PacedSite::Tumblr)
        );
        assert_eq!(
            paced_site("https://someone.fanbox.cc/posts/1"),
            Some(PacedSite::Fanbox)
        );
        assert_eq!(paced_site("https://notinstagram.com/p/1"), None);
        assert_eq!(paced_site("https://www.youtube.com/watch?v=1"), None);
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

    #[test]
    fn finds_the_sign_in_site_of_a_link() {
        let name = |link| sign_in_site(link).map(|site| site.name);
        assert_eq!(
            name("https://www.instagram.com/p/DfPQchkG1tg/"),
            Some("Instagram")
        );
        assert_eq!(name("https://mobile.twitter.com/NASA/status/1"), Some("X"));
        assert_eq!(name("https://youtu.be/jNQXAC9IVRw"), Some("YouTube"));
        assert_eq!(name("https://soundcloud.com/a/b"), None);
        assert_eq!(name("https://notinstagram.com/p/1"), None);
    }

    #[test]
    fn names_the_site_a_limit_holds_back() {
        let site = |link| site_of(link).map(|site| (site.key, site.name));
        let pair = |key: &str, name: &str| Some((key.to_owned(), name.to_owned()));
        assert_eq!(
            site("https://www.instagram.com/p/DfPQchkG1tg/"),
            pair("instagram.com", "Instagram")
        );
        assert_eq!(
            site("https://mobile.twitter.com/NASA/status/1"),
            site("https://x.com/NASA/status/2"),
            "one site under both its domains"
        );
        assert_eq!(
            site("https://mega.nz/file/AbCdEf12#key"),
            pair("mega", "MEGA")
        );
        assert_eq!(
            site("https://player.vimeo.com/video/1"),
            pair("vimeo.com", "vimeo.com")
        );
        assert_eq!(site("not a link"), None);
    }

    #[test]
    fn matches_cookie_hosts_to_domains() {
        assert!(on_domain(".instagram.com", "instagram.com"));
        assert!(on_domain("www.Instagram.com", "instagram.com"));
        assert!(!on_domain("notinstagram.com", "instagram.com"));
        assert!(!on_domain("instagram.com.evil.org", "instagram.com"));
    }

    #[test]
    fn reads_cookies_of_the_whole_site() {
        assert_eq!(
            cookie_domains("https://x.com/NASA/status/1"),
            ["x.com", "twitter.com"]
        );
        assert_eq!(cookie_domains("https://media.example.co/a"), ["example.co"]);
    }
}
