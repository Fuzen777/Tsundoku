//! Mirror pages: a page that lists the same file on several file hosts (MultiUp's, mirrored.to's). Before a link is
//! read, `resolve` turns a mirror page into the link of one of its mirrors, which is then read as if it had been
//! pasted: the preview, the queue and the history know the mirror's host and file, and any check or wait on it applies
//! as for that host's own link. A mirror on a host Tsundoku reads itself comes first, in `PREFERRED`'s order (hosts
//! that give a checksum and make no one wait first), then one on a site the tools read (`TOOL_HOSTS`); mirrors the
//! page marks dead are skipped. A page with none of those says which hosts it has (`PreviewError::NoSupportedMirror`).
//! Asked as Tsundoku, like the hosts.

mod mirrored;
mod multiup;

use reqwest::Url;

use crate::cancel::CancelToken;
use crate::hosts::{self, Host};
use crate::metadata::PreviewError;
use crate::sites;

/// The hosts Tsundoku reads itself, in the order a mirror is taken from them: those that give a checksum or answer at
/// once, then those with a check, a countdown or a wait between downloads.
const PREFERRED: [Host; 16] = [
    Host::GoogleDrive,
    Host::Dropbox,
    Host::OneDrive,
    Host::PCloud,
    Host::Box,
    Host::YandexDisk,
    Host::Mega,
    Host::Icedrive,
    Host::FilesFm,
    Host::SwissTransfer,
    Host::WeTransfer,
    Host::TransferNow,
    Host::Filemail,
    Host::UploadEe,
    Host::MediaFire,
    Host::OneFichier,
];

/// Sites the tools read a file from (gallery-dl), after Tsundoku's own hosts.
const TOOL_HOSTS: [&str; 2] = ["gofile.io", "pixeldrain.com"];

/// A mirror page's site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MirrorSite {
    MultiUp,
    MirroredTo,
}

impl MirrorSite {
    fn of(link: &str) -> Option<Self> {
        let host = sites::host(link)?;
        [Self::MultiUp, Self::MirroredTo].into_iter().find(|site| {
            site.domains()
                .iter()
                .any(|domain| sites::on_domain(&host, domain))
        })
    }

    fn name(self) -> &'static str {
        match self {
            Self::MultiUp => "MultiUp",
            Self::MirroredTo => "Mirrored.to",
        }
    }

    fn domains(self) -> &'static [&'static str] {
        match self {
            Self::MultiUp => &["multiup.io", "multiup.org"],
            Self::MirroredTo => &["mirrored.to", "mir.cr"],
        }
    }
}

/// A mirror on a page: the link to the file on its host, the host's name as the page gives it, and whether the page
/// found the file gone from it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Mirror {
    link: String,
    host_name: String,
    dead: bool,
}

/// The link of the mirror to read in place of `link`, when `link` is a mirror page; `None` for any other link.
pub fn resolve(link: &str, cancel: &CancelToken) -> Option<Result<String, PreviewError>> {
    let site = MirrorSite::of(link)?;
    Some(resolve_on(site, link, cancel))
}

fn resolve_on(site: MirrorSite, link: &str, cancel: &CancelToken) -> Result<String, PreviewError> {
    let client = hosts::tsundoku_client()?;
    let not_recognized = || PreviewError::LinkNotRecognized {
        site: site.name().to_owned(),
    };
    let mirrors = match site {
        MirrorSite::MultiUp => {
            multiup::mirrors(&client, link, cancel)?.ok_or_else(not_recognized)?
        }
        MirrorSite::MirroredTo => {
            mirrored::mirrors(&client, link, cancel)?.ok_or_else(not_recognized)?
        }
    };
    choose(&mirrors)
        .map(|mirror| mirror.link.clone())
        .ok_or_else(|| PreviewError::NoSupportedMirror {
            site: site.name().to_owned(),
            mirrors: host_names(&mirrors),
        })
}

/// The mirror to read: the first on one of `PREFERRED`'s hosts, in that order, else the first on one of `TOOL_HOSTS`,
/// leaving out the dead ones.
fn choose(mirrors: &[Mirror]) -> Option<&Mirror> {
    let alive = || mirrors.iter().filter(|mirror| !mirror.dead);
    PREFERRED
        .iter()
        .find_map(|preferred| alive().find(|mirror| Host::of(&mirror.link) == Some(*preferred)))
        .or_else(|| {
            TOOL_HOSTS.iter().find_map(|domain| {
                alive().find(|mirror| {
                    sites::host(&mirror.link).is_some_and(|host| sites::on_domain(&host, domain))
                })
            })
        })
}

/// The hosts a page's mirrors are on, each once.
fn host_names(mirrors: &[Mirror]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for mirror in mirrors {
        if !names.contains(&mirror.host_name) {
            names.push(mirror.host_name.clone());
        }
    }
    names
}

/// An http or https link, as a mirror's.
fn web_link(link: &str) -> Option<Url> {
    Url::parse(link)
        .ok()
        .filter(|url| matches!(url.scheme(), "https" | "http") && url.host_str().is_some())
}

/// An attribute's value in a tag (` name="value"`).
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!(" {name}=\"");
    let start = tag.find(&marker)? + marker.len();
    let length = tag[start..].find('"')?;
    Some(&tag[start..start + length])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mirror(link: &str, host_name: &str, dead: bool) -> Mirror {
        Mirror {
            link: link.to_owned(),
            host_name: host_name.to_owned(),
            dead,
        }
    }

    #[test]
    fn knows_mirror_pages() {
        assert_eq!(
            MirrorSite::of("https://multiup.io/download/080e3fb87debea6e9818d55aa1c68b43/test.txt"),
            Some(MirrorSite::MultiUp)
        );
        assert_eq!(
            MirrorSite::of("https://www.mirrored.to/files/A6B0D72A/Sample.7z_links"),
            Some(MirrorSite::MirroredTo)
        );
        assert_eq!(
            MirrorSite::of("https://mir.cr/A6B0D72A"),
            Some(MirrorSite::MirroredTo)
        );
        assert_eq!(MirrorSite::of("https://www.mediafire.com/file/abc"), None);
    }

    #[test]
    fn takes_its_own_hosts_first_then_the_tools() {
        let mirrors = [
            mirror(
                "https://clicknupload.click/8qa3vg6x38zq",
                "clicknupload.click",
                false,
            ),
            mirror("https://gofile.io/d/YunkYlsN", "GoFileIo", false),
            mirror("https://1fichier.com/?abcdefghij", "1fichier.com", false),
            mirror("https://mega.nz/file/AbCdEf12#key", "mega.nz", true),
            mirror(
                "https://www.mediafire.com/file/abc123",
                "mediafire.com",
                false,
            ),
        ];
        assert_eq!(
            choose(&mirrors).map(|mirror| mirror.link.as_str()),
            Some("https://www.mediafire.com/file/abc123"),
            "MEGA's is dead, and 1fichier comes last of Tsundoku's own"
        );
        assert_eq!(
            choose(&mirrors[..2]).map(|mirror| mirror.link.as_str()),
            Some("https://gofile.io/d/YunkYlsN"),
            "a site the tools read, when none is Tsundoku's own"
        );
        assert_eq!(choose(&mirrors[..1]), None);
        assert_eq!(
            host_names(&mirrors),
            [
                "clicknupload.click",
                "GoFileIo",
                "1fichier.com",
                "mega.nz",
                "mediafire.com"
            ]
        );
    }
}

/// A live check, run by hand when a mirror site changes: `MIRROR_LINKS="<link> <link>" cargo test -p tsundoku-core --lib
/// mirrors_live -- --ignored --nocapture`. It needs the network.
#[cfg(test)]
mod mirrors_live {
    use super::*;

    #[test]
    #[ignore = "needs the network"]
    fn mirrors_live_resolves_each_link() {
        let links = std::env::var("MIRROR_LINKS").unwrap_or_default();
        for link in links.split_whitespace() {
            println!(
                "{link}
  {:?}",
                resolve(link, &CancelToken::new())
            );
        }
    }
}
