//! MultiUp's mirror pages: "multiup.io/download/<hash>/<name>", or "/<language>/mirror/<hash>/<name>", on multiup.io or
//! multiup.org, read at "multiup.io/en/mirror/<hash>/<name>", whose host buttons carry each mirror's `link`, `nameHost`
//! and `validity`.

use reqwest::StatusCode;
use reqwest::Url;
use reqwest::blocking::Client;

use super::{Mirror, attribute, web_link};
use crate::cancel::CancelToken;
use crate::hosts::{Page, check_cancel, html_text, network_error};
use crate::metadata::PreviewError;

const SITE: &str = "https://multiup.io";

/// The page's mirrors, or `None` for a MultiUp link that isn't a mirror page.
pub(super) fn mirrors(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Option<Vec<Mirror>>, PreviewError> {
    let Some(page_link) = mirror_page(link) else {
        return Ok(None);
    };
    check_cancel(cancel)?;
    let page = Page::read(client.get(&page_link).send().map_err(network_error)?);
    if page.status == StatusCode::NOT_FOUND {
        return Err(PreviewError::Unavailable {
            detail: "MultiUp doesn't know this file".to_owned(),
        });
    }
    if !page.status.is_success() {
        return Err(PreviewError::Network {
            detail: format!("MultiUp answered {}", page.status),
        });
    }
    Ok(Some(listed(&page.html)))
}

/// The mirrors on the page: its "host" buttons, each with `link`, `nameHost` and `validity` attributes.
fn listed(html: &str) -> Vec<Mirror> {
    html.match_indices("<a ")
        .filter_map(|(start, _)| {
            let tag = &html[start..start + html[start..].find('>')?];
            let link = html_text(attribute(tag, "link")?);
            let url = web_link(&link)?;
            let host_name = attribute(tag, "nameHost")
                .map(html_text)
                .or_else(|| url.host_str().map(str::to_owned))?;
            Some(Mirror {
                link,
                host_name,
                dead: attribute(tag, "validity").is_some_and(|validity| validity == "invalid"),
            })
        })
        .collect()
}

/// The mirror list's address for a MultiUp link: "/en/mirror/<hash>/<name>".
fn mirror_page(link: &str) -> Option<String> {
    let url = Url::parse(link).ok()?;
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    let rest = match segments.as_slice() {
        ["download" | "mirror", rest @ ..] => rest,
        [language, "download" | "mirror", rest @ ..] if language.len() == 2 => rest,
        _ => return None,
    };
    let (hash, name) = match rest {
        [hash, name, ..] => (*hash, *name),
        [hash] => (*hash, "file"),
        _ => return None,
    };
    let is_hash = hash.len() == 32 && hash.chars().all(|character| character.is_ascii_hexdigit());
    is_hash.then(|| format!("{SITE}/en/mirror/{}/{name}", hash.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mirror on MultiUp's page on 2026-11-11, with two made up after it.
    const PAGE: &str = r#"<footer class="panel-footer">
        <a href="https://clicknupload.click/8qa3vg6x38zq" target="_blank"
           nameHost="clicknupload.click"
           link="https://clicknupload.click/8qa3vg6x38zq"
           validity="valid"
           dateLastChecked="6/17/18, 4:33 AM"
           id="88215855"
           class="host btn btn-md btn-default btn-block btn-3d hvr-bounce-to-right">Download (0)</a>
        <a href="https://mega.nz/file/AbCdEf12#key" nameHost="mega.nz" link="https://mega.nz/file/AbCdEf12#key" validity="invalid" class="host btn">Download</a>
        <a href="https://www.mediafire.com/file/abc123" nameHost="mediafire.com" link="https://www.mediafire.com/file/abc123" validity="unknown" class="host btn">Download</a>
        <a href="/en/upload/from-links">Upload</a>
    </footer>"#;

    #[test]
    fn reads_its_links() {
        for (link, page) in [
            (
                "https://multiup.io/download/080e3fb87debea6e9818d55aa1c68b43/test.txt",
                Some("https://multiup.io/en/mirror/080e3fb87debea6e9818d55aa1c68b43/test.txt"),
            ),
            (
                "https://multiup.org/fr/mirror/080E3FB87DEBEA6E9818D55AA1C68B43/test.txt",
                Some("https://multiup.io/en/mirror/080e3fb87debea6e9818d55aa1c68b43/test.txt"),
            ),
            (
                "https://multiup.io/en/download/080e3fb87debea6e9818d55aa1c68b43",
                Some("https://multiup.io/en/mirror/080e3fb87debea6e9818d55aa1c68b43/file"),
            ),
            ("https://multiup.io/en/upload/from-links", None),
            ("https://multiup.io/download/nothex/test.txt", None),
        ] {
            assert_eq!(mirror_page(link).as_deref(), page, "{link}");
        }
    }

    #[test]
    fn reads_the_host_buttons() {
        let found = listed(PAGE);
        assert_eq!(found.len(), 3, "the host buttons only");
        assert_eq!(found[0].host_name, "clicknupload.click");
        assert!(found[1].dead && !found[2].dead);
        assert_eq!(found[2].link, "https://www.mediafire.com/file/abc123");
    }
}
