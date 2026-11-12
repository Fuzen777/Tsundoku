//! Mirrored.to's mirror pages: "mirrored.to/files/<id>/<name>_links" (or without a name), and the short "mir.cr/<id>",
//! read as its pages read them, without cookies. The file's page links its download page
//! ("/files/<id>/?hash=<hash>&dl=0"), whose script asks `/mirstats.php` for the table of mirrors: a row per host, its
//! name in the host picture's `alt` and a "/getlink/<id>/<n>/?hid=…" page that names the mirror's link
//! (`data-clipboard-text`). Each mirror's page is read, then the mirror to take is chosen from the links.

use reqwest::StatusCode;
use reqwest::blocking::Client;

use super::{Mirror, attribute, web_link};
use crate::cancel::CancelToken;
use crate::hosts::{Page, SendChecked, between, check_cancel, html_text};
use crate::metadata::PreviewError;

const SITE: &str = "https://www.mirrored.to";
/// The most mirrors read from one page.
const MOST_MIRRORS: usize = 30;

/// The page's mirrors, or `None` for a mirrored.to link that isn't a file's.
pub(super) fn mirrors(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Option<Vec<Mirror>>, PreviewError> {
    let Some(id) = file_id(link) else {
        return Ok(None);
    };
    let file_page = open(client, &format!("{SITE}/files/{id}/"), cancel)?;
    let download_page = between(&file_page, &format!("/files/{id}/?hash="), "\"")
        .map(|rest| format!("{SITE}/files/{id}/?hash={}", html_text(rest)))
        .ok_or_else(|| unexpected("its page had no link to the mirrors"))?;
    let download_page = open(client, &download_page, cancel)?;
    let table = between(&download_page, "\"/mirstats.php?", "\"")
        .map(|query| format!("{SITE}/mirstats.php?{}", html_text(query)))
        .ok_or_else(|| unexpected("its page didn't ask for the mirrors"))?;
    let table = open(client, &table, cancel)?;
    let mut mirrors = Vec::new();
    for (host_name, getlink) in rows(&table).into_iter().take(MOST_MIRRORS) {
        let page = open(client, &format!("{SITE}{getlink}"), cancel)?;
        if let Some(link) = mirror_link(&page) {
            mirrors.push(Mirror {
                link,
                host_name,
                dead: false,
            });
        }
    }
    Ok(Some(mirrors))
}

/// A mirrored.to page, as text.
fn open(client: &Client, url: &str, cancel: &CancelToken) -> Result<String, PreviewError> {
    check_cancel(cancel)?;
    let page = Page::read(
        client
            .get(url)
            .send_checked(super::MirrorSite::MirroredTo.name())?,
    );
    if page.status == StatusCode::NOT_FOUND {
        return Err(PreviewError::Unavailable {
            detail: "Mirrored.to doesn't know this file".to_owned(),
        });
    }
    if !page.status.is_success() {
        return Err(PreviewError::Network {
            detail: format!("Mirrored.to answered {}", page.status),
        });
    }
    Ok(page.html)
}

/// The table's rows: each host's name and the page that names its link.
fn rows(table: &str) -> Vec<(String, String)> {
    table
        .split("<tr")
        .skip(1)
        .filter_map(|row| {
            let host = between(row, "images/hosts/", "\"")?;
            let host_name = between(row, "alt=\"", "\"")
                .map(html_text)
                .unwrap_or_else(|| host.trim_end_matches(".png").to_owned());
            let getlink = html_text(between(row, "href=\"/getlink/", "\"")?);
            Some((host_name, format!("/getlink/{getlink}")))
        })
        .collect()
}

/// The mirror's link on its "getlink" page: the one its Copy button copies.
fn mirror_link(page: &str) -> Option<String> {
    page.match_indices("data-clipboard-text=\"")
        .find_map(|(start, _)| {
            let tag = &page[start.saturating_sub(1)..];
            let link = html_text(attribute(tag, "data-clipboard-text")?);
            web_link(&link).map(|_| link)
        })
}

/// The file's ID in "mirrored.to/files/<id>/…" or "mir.cr/<id>".
fn file_id(link: &str) -> Option<String> {
    let url = web_link(link)?;
    let host = url.host_str()?.to_ascii_lowercase();
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    let id = if host == "mir.cr" || host.ends_with(".mir.cr") {
        match segments.as_slice() {
            [id, ..] => *id,
            _ => return None,
        }
    } else {
        match segments.as_slice() {
            ["files", id, ..] => *id,
            _ => return None,
        }
    };
    (id.len() >= 6
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric()))
    .then(|| id.to_ascii_uppercase())
}

fn unexpected(what: &str) -> PreviewError {
    PreviewError::Failed {
        detail: format!("Mirrored.to wasn't as expected: {what}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_its_links() {
        for (link, id) in [
            (
                "https://www.mirrored.to/files/A6B0D72A/Sample.7z_links",
                Some("A6B0D72A"),
            ),
            ("https://mirrored.to/files/a6b0d72a/", Some("A6B0D72A")),
            ("https://mir.cr/A6B0D72A", Some("A6B0D72A")),
            ("https://www.mirrored.to/p/upload-tools", None),
        ] {
            assert_eq!(file_id(link).as_deref(), id, "{link}");
        }
    }

    #[test]
    fn reads_the_table_and_a_mirrors_page() {
        // Parts of mirrored.to's mirror table and a mirror's page on 2026-11-12.
        let table = r#"<table><tr> <th>Host</th> <th>File Link</th> </tr>
            <tr><td data-label="Host"><img src="https://www.mirrored.to/templates/mirrored/images/hosts/GoFileIo.png" alt="GoFileIo"/></td><td data-label="File Link"> <a href="/getlink/A6B0D72A/137/?hid=YWhO%2F10&amp;tid=9593" target=_blank> <button class="get_btn">Download</button></a></td></tr>
            <tr><td data-label="Host"><img src="https://www.mirrored.to/templates/mirrored/images/hosts/TurboBit.png" alt="TurboBit"/></td><td data-label="File Link"> <a href="/getlink/A6B0D72A/31/?hid=RVlt&amp;tid=9593" target=_blank>Download</a></td></tr>
            </table>"#;
        assert_eq!(
            rows(table),
            [
                (
                    "GoFileIo".to_owned(),
                    "/getlink/A6B0D72A/137/?hid=YWhO%2F10&tid=9593".to_owned()
                ),
                (
                    "TurboBit".to_owned(),
                    "/getlink/A6B0D72A/31/?hid=RVlt&tid=9593".to_owned()
                ),
            ]
        );
        let page = r#"<a href="https://gofile.io/d/YunkYlsN" target="_blank"><button class="get_btn">Download</button></a>
            <code>https://gofile.io/d/YunkYlsN</code>
            <button id="btn2" data-clipboard-text="https://gofile.io/d/YunkYlsN" title="Click to copy the link">"#;
        assert_eq!(
            mirror_link(page).as_deref(),
            Some("https://gofile.io/d/YunkYlsN")
        );
        assert_eq!(mirror_link("<html>Not found</html>"), None);
    }
}
