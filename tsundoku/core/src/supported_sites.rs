//! The list of supported sites in Settings: every site yt-dlp or gallery-dl can read, as one list, since which tool
//! reads a link is Tsundoku's business, not the user's. Sites that need an account show whether a browser profile
//! is signed in to them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use ts_rs::TS;

use crate::browsers::{self, BrowserProfile, BrowserRoots, CookieUse};
use crate::hosts::Host;
use crate::process::{self, ProgramCommand};
use crate::settings::CookiesBrowser;
use crate::sites::SIGN_IN_SITES;

/// How long a tool may take to list its sites.
const LIST_TIMEOUT: Duration = Duration::from_secs(30);

/// Parts of a site that yt-dlp lists as extractors of their own, named after the site with one or more of these
/// added ("ArteTVPlaylist" beside "ArteTV"). Such a name is left out when the site itself is listed. Words that often
/// make a name of their own ("TV", "News", "Play") aren't here.
const SITE_PARTS: [&str; 36] = [
    "album",
    "albums",
    "article",
    "articles",
    "category",
    "categories",
    "channel",
    "channels",
    "collection",
    "collections",
    "course",
    "courses",
    "embed",
    "episode",
    "episodes",
    "live",
    "playlist",
    "playlists",
    "podcast",
    "podcasts",
    "program",
    "programs",
    "recording",
    "recordings",
    "search",
    "season",
    "seasons",
    "series",
    "show",
    "shows",
    "tab",
    "title",
    "user",
    "users",
    "video",
    "videos",
];

/// Everything Settings → Supported Sites shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SupportedSites {
    /// Every site, sorted by name.
    pub sites: Vec<SupportedSite>,
    /// Image sites are only listed once gallery-dl is set up.
    pub images_set_up: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SupportedSite {
    pub name: String,
    /// For a site where an account matters (`sites::SIGN_IN_SITES`), the browser profiles signed in to it, most
    /// recently used first; empty when none is. `None` for other sites.
    pub signed_in: Option<Vec<SignedInProfile>>,
}

/// A browser profile signed in to a site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SignedInProfile {
    pub browser: CookiesBrowser,
    /// The profile's name in the browser.
    pub profile: String,
}

/// Lists the sites the set-up tools support and the file hosts Tsundoku reads itself, with this PC's browser sign-ins.
pub fn supported_sites(yt_dlp: Option<&Path>, gallery_dl: Option<&Path>) -> SupportedSites {
    let yt_dlp_names = yt_dlp
        .and_then(|program| list(program, &["--ignore-config", "--list-extractors"]))
        .map(|lines| yt_dlp_sites(&lines))
        .unwrap_or_default();
    let gallery_dl_names = gallery_dl
        .and_then(|program| list(program, &["--config-ignore", "--list-extractors"]))
        .map(|lines| gallery_dl_sites(&lines))
        .unwrap_or_default();
    let profiles = BrowserRoots::of_this_user()
        .map(|roots| browsers::find_profiles(&roots))
        .unwrap_or_default();
    SupportedSites {
        sites: merge(
            yt_dlp_names
                .iter()
                .chain(&gallery_dl_names)
                .map(String::as_str)
                .chain(Host::ALL.map(Host::name)),
            &sign_ins(&profiles, browsers::cookie_uses),
        ),
        images_set_up: gallery_dl.is_some(),
    }
}

/// What a tool wrote listing its sites, or `None` if it couldn't run.
fn list(program: &Path, args: &[&str]) -> Option<Vec<String>> {
    let command = ProgramCommand::new(program).args(args);
    let output = process::run_collecting(&command, LIST_TIMEOUT).ok()?;
    output
        .status
        .success()
        .then(|| output.stdout().map(str::to_owned).collect())
}

/// yt-dlp lists one extractor per line, such as "youtube:tab" or "Instagram"; the part before ":" is the site.
/// Broken extractors, the catch-all "generic" and descriptions rather than names ("Bilibili category extractor")
/// are left out.
fn yt_dlp_sites(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.contains("(CURRENTLY BROKEN)"))
        .map(|line| line.split(':').next().unwrap_or(line).trim().to_owned())
        .filter(|name| !name.eq_ignore_ascii_case("generic") && !name.contains(char::is_whitespace))
        .collect()
}

/// gallery-dl lists each extractor in a block with a "Category: instagram - Subcategory: post" line; the category is
/// the site.
fn gallery_dl_sites(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| line.trim().strip_prefix("Category:"))
        .map(|rest| rest.split(" - ").next().unwrap_or(rest).trim().to_owned())
        .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("directlink"))
        .collect()
}

/// The profiles signed in to each sign-in site, by site name, most recently used first. Each profile's cookie
/// database is read once.
fn sign_ins(
    profiles: &[BrowserProfile],
    read: impl Fn(&BrowserProfile, &[String]) -> std::io::Result<Vec<CookieUse>>,
) -> BTreeMap<&'static str, Vec<SignedInProfile>> {
    let domains: Vec<String> = SIGN_IN_SITES
        .iter()
        .flat_map(|site| site.domains.iter().map(|domain| (*domain).to_owned()))
        .collect();
    let uses: Vec<(&BrowserProfile, Vec<CookieUse>)> = profiles
        .iter()
        .filter_map(|profile| Some((profile, read(profile, &domains).ok()?)))
        .collect();
    SIGN_IN_SITES
        .iter()
        .map(|site| {
            let mut signed_in: Vec<(i64, &BrowserProfile)> = uses
                .iter()
                .filter_map(|(profile, uses)| {
                    Some((browsers::signed_in_since(uses, site)?, *profile))
                })
                .collect();
            signed_in.sort_by_key(|(last_used, _)| std::cmp::Reverse(*last_used));
            let profiles = signed_in
                .into_iter()
                .map(|(_, profile)| SignedInProfile {
                    browser: profile.browser,
                    profile: profile.name.clone(),
                })
                .collect();
            (site.name, profiles)
        })
        .collect()
}

/// One entry per site whatever the case of its name or how many tools list it, sorted by name, without the parts of
/// a listed site (`SITE_PARTS`). Sign-in sites take their own name ("X" for "twitter") and are always listed.
fn merge<'a>(
    names: impl Iterator<Item = &'a str>,
    sign_ins: &BTreeMap<&'static str, Vec<SignedInProfile>>,
) -> Vec<SupportedSite> {
    let mut sites: BTreeMap<String, SupportedSite> = BTreeMap::new();
    for site in &SIGN_IN_SITES {
        sites.insert(
            site.extractor.to_owned(),
            SupportedSite {
                name: site.name.to_owned(),
                signed_in: Some(sign_ins.get(site.name).cloned().unwrap_or_default()),
            },
        );
    }
    for name in names {
        sites
            .entry(name.to_lowercase())
            .or_insert_with(|| SupportedSite {
                name: name.to_owned(),
                signed_in: None,
            });
    }
    let names: BTreeSet<String> = sites.keys().cloned().collect();
    sites.retain(|name, site| site.signed_in.is_some() || !is_part_of_listed_site(name, &names));
    let mut sites: Vec<SupportedSite> = sites.into_values().collect();
    sites.sort_by_key(|site| site.name.to_lowercase());
    sites
}

/// Whether `name` (in lowercase) is another listed site's name followed by `SITE_PARTS`, such as "artetvplaylist"
/// when "artetv" is listed.
fn is_part_of_listed_site(name: &str, names: &BTreeSet<String>) -> bool {
    name.char_indices()
        .skip(1)
        .any(|(index, _)| names.contains(&name[..index]) && is_site_parts(&name[index..]))
}

/// Whether `rest` is made only of `SITE_PARTS`, such as "playlist" or "seasonplaylist".
fn is_site_parts(rest: &str) -> bool {
    rest.is_empty()
        || SITE_PARTS
            .iter()
            .any(|part| rest.strip_prefix(part).is_some_and(is_site_parts))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_owned).collect()
    }

    #[test]
    fn reads_yt_dlps_list() {
        let names = yt_dlp_sites(&lines(
            "Instagram\ninstagram:story\nyoutube\nyoutube:tab\nSomeSite (CURRENTLY BROKEN)\ngeneric\n\
             Bilibili category extractor\n",
        ));
        assert_eq!(names, ["Instagram", "instagram", "youtube", "youtube"]);
    }

    #[test]
    fn leaves_out_the_parts_of_a_listed_site() {
        let sites = merge(
            [
                "ArteTV",
                "ArteTVPlaylist",
                "ArteTVEmbed",
                "AmericasTestKitchen",
                "AmericasTestKitchenSeason",
                "BiliBiliBangumi",
                "BiliBiliBangumiSeason",
                "BilibiliAudio",
                "BilibiliAudioAlbum",
                "InstagramUser",
                // A part of a site that isn't listed stays.
                "SoloPlaylist",
                // Longer names that aren't parts stay too.
                "2ch",
                "2chan",
                "apa",
                "Aparat",
                "bbc",
                "bbc.co.uk",
            ]
            .into_iter(),
            &BTreeMap::new(),
        );
        let names: Vec<&str> = sites
            .iter()
            .filter(|site| site.signed_in.is_none())
            .map(|site| site.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "2ch",
                "2chan",
                "AmericasTestKitchen",
                "apa",
                "Aparat",
                "ArteTV",
                "bbc",
                "bbc.co.uk",
                "BilibiliAudio",
                "BiliBiliBangumi",
                "SoloPlaylist",
            ]
        );
    }

    #[test]
    fn reads_gallery_dls_list() {
        let names = gallery_dl_sites(&lines(
            "InstagramPostExtractor\nInstagram Posts\nCategory: instagram - Subcategory: post\n\
             Example : https://www.instagram.com/p/abcdefg/\n\n\
             DanbooruPostExtractor\nDanbooru Posts\nCategory: danbooru - Subcategory: post\n\
             Example : https://danbooru.donmai.us/posts/12345\n\n\
             DirectlinkExtractor\nDirect links\nCategory: directlink - Subcategory: \n",
        ));
        assert_eq!(names, ["instagram", "danbooru"]);
    }

    fn profile(browser: CookiesBrowser, name: &str) -> BrowserProfile {
        BrowserProfile {
            browser,
            name: name.to_owned(),
            path: PathBuf::from(name),
            default: true,
        }
    }

    fn cookie(host: &str, name: &str, last_used: i64) -> CookieUse {
        CookieUse {
            host: host.to_owned(),
            name: name.to_owned(),
            last_used,
        }
    }

    #[test]
    fn merges_the_tools_lists_with_sign_ins() {
        let profiles = [
            profile(CookiesBrowser::Firefox, "Old"),
            profile(CookiesBrowser::Zen, "Main"),
            profile(CookiesBrowser::Chrome, "Locked"),
        ];
        let read = |profile: &BrowserProfile, _: &[String]| match profile.name.as_str() {
            "Old" => Ok(vec![cookie(".instagram.com", "sessionid", 100)]),
            "Main" => Ok(vec![
                cookie(".instagram.com", "sessionid", 200),
                cookie(".x.com", "auth_token", 150),
                cookie(".youtube.com", "PREF", 300),
            ]),
            _ => Err(std::io::Error::other("locked")),
        };
        let sites = merge(
            ["Instagram", "twitter", "danbooru", "Danbooru", "abc.net.au"].into_iter(),
            &sign_ins(&profiles, read),
        );
        let find = |name: &str| sites.iter().find(|site| site.name == name).unwrap();

        let names: Vec<&str> = sites.iter().map(|site| site.name.as_str()).collect();
        assert!(names.starts_with(&[
            "abc.net.au",
            "Bilibili",
            "danbooru",
            "Facebook",
            "Instagram"
        ]));
        assert_eq!(
            names
                .iter()
                .filter(|name| name.eq_ignore_ascii_case("danbooru"))
                .count(),
            1
        );
        assert!(!names.contains(&"twitter"));
        assert_eq!(
            find("Instagram").signed_in.as_deref(),
            Some(
                &[
                    SignedInProfile {
                        browser: CookiesBrowser::Zen,
                        profile: "Main".to_owned()
                    },
                    SignedInProfile {
                        browser: CookiesBrowser::Firefox,
                        profile: "Old".to_owned()
                    },
                ][..]
            )
        );
        assert_eq!(find("X").signed_in.as_ref().map(Vec::len), Some(1));
        // Visited without signing in.
        assert_eq!(find("YouTube").signed_in.as_deref(), Some(&[][..]));
        assert_eq!(find("danbooru").signed_in, None);
    }
}
