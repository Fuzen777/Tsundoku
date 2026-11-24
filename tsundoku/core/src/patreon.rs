//! Patreon's posts (`patreon.com/posts/<title>-<id>`). Patreon answers its API only to a browser (gallery-dl got 403
//! on Fuzen's PC, signed in or not, even posing as Chrome), so a post is read through Tsundoku's own hidden window
//! (`hosts::patreon`), as Fanbox's are: the window asks `/api/posts/<id>` as Patreon's own pages do, and
//! `post_from_api` turns its JSON:API answer into one post, with its images, attachments, campaign and creator in it. A
//! post whose point is a video or an audio (`PostFacts::media`: Patreon's own player, an embedded YouTube or Vimeo
//! video, an audio file) is downloaded by yt-dlp, posing as Chrome (`yt_dlp_args`), any other by Tsundoku itself: its
//! images, attachments and the pictures in its text (`files`). Either way the links inside it go to Pending once it's
//! downloaded (`post_links`), as a Fanbox post's do, or at once when it has nothing else to download.
//!
//! A post for paying members says so (`PreviewError::PatreonLocked`, with the least a month that opens it): Tsundoku
//! can't sign in to Patreon yet. gallery-dl's Patreon code (GPL) was read for the shape of Patreon's answer only.

use reqwest::Url;
use serde_json::Value;

use crate::metadata::{self, PreviewError};
use crate::pending::files::{anchor_links, decode_entities};
use crate::sites;

/// The extensions of video and audio files, which make a post's own file (`post_file`) one for yt-dlp.
const MEDIA_EXTENSIONS: [&str; 15] = [
    "mp4", "m4v", "mov", "webm", "mkv", "avi", "m3u8", "mp3", "m4a", "aac", "wav", "flac", "ogg",
    "opus", "wma",
];

/// Patreon's kinds of post whose point is a video or an audio, by how their `post_type` starts ("video_embed",
/// "video_external_file", "audio_file", "audio_embed", "livestream_youtube", "podcast").
const MEDIA_POST_TYPES: [&str; 4] = ["video", "audio", "livestream", "podcast"];

/// The longest title made from a post's text.
const TITLE_LENGTH: usize = 120;

/// The ID of the post `link` (from `normalize_link`) names: `patreon.com/posts/<title>-<id>` or `/posts/<id>`, the same
/// after a creator's name (`/<creator>/posts/…`), or `/creation?hid=<id>`. `None` for any other link, such as a
/// creator's page.
pub fn post_id(link: &str) -> Option<String> {
    let url = Url::parse(link).ok()?;
    if !sites::on_domain(url.host_str()?, "patreon.com") {
        return None;
    }
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    let id = match segments.as_slice() {
        ["posts", slug] | [_, "posts", slug] => slug.rsplit('-').next()?.to_owned(),
        ["creation"] => url
            .query_pairs()
            .find(|(key, _)| key == "hid")
            .map(|(_, id)| id.into_owned())?,
        _ => return None,
    };
    (!id.is_empty() && id.chars().all(|character| character.is_ascii_digit())).then_some(id)
}

/// What yt-dlp is given for a Patreon link (a post or a creator's page): Chrome's connection fingerprint
/// (`--impersonate chrome`, curl_cffi's), as Kotodama does. Patreon answers its API only to a browser: yt-dlp poses as
/// one by itself only without a `session_id` cookie, and with one sends Patreon's Android app's user agent, which
/// Patreon answered "Patreon said: Forbidden" on Fuzen's PC (2026-11-24), so a browser signed in to Patreon broke its
/// videos and audio. As Chrome, it read both with Zen's cookies. Nothing for any other link.
pub(crate) fn yt_dlp_args(link: &str) -> &'static [&'static str] {
    let on_patreon = Url::parse(link)
        .ok()
        .and_then(|url| {
            url.host_str()
                .map(|host| sites::on_domain(host, "patreon.com"))
        })
        .unwrap_or(false);
    if on_patreon {
        &["--impersonate", "chrome"]
    } else {
        &[]
    }
}

/// What a post says of itself besides its files.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PostFacts {
    /// Its point is a video or an audio, which yt-dlp downloads.
    pub media: bool,
    /// The account can't open it: why (`PreviewError::PatreonLocked`).
    pub locked: Option<PreviewError>,
    /// The links inside it that go to Pending (`post_links`), with the page it embeds left out when that's the post's
    /// video or audio.
    pub links: Vec<String>,
}

/// The facts of a post (`post_from_api`).
pub(crate) fn facts(post: &Value) -> PostFacts {
    let media = is_media(post);
    PostFacts {
        media,
        locked: locked(post),
        links: post_links(post, media),
    }
}

/// The API's address for a post, asking for what `post_from_api` and `files` read: its campaign, creator, images,
/// attachments and tags, with the fields of each that are used.
pub(crate) fn api_address(id: &str) -> String {
    format!(
        "https://www.patreon.com/api/posts/{id}?include=campaign,attachments,attachments_media,images,user,\
         user_defined_tags&fields[post]=content,content_json_string,current_user_can_view,embed,image,\
         min_cents_pledged_to_view,post_file,post_type,published_at,title,url&fields[campaign]=currency,name,url\
         &fields[user]=full_name,url,vanity&fields[media]=download_url,file_name,image_urls\
         &fields[post_tag]=tag_type,value&json-api-version=1.0"
    )
}

/// One post from the API's JSON:API answer (`{"data": {...}, "included": [...]}`): the post's own fields, with its
/// `id`, and in place of its relationships the things they name, from `included`: `images`, `attachments`
/// (older posts' files) and `attachments_media` as lists, `campaign` and `creator` (its `user`) as objects, `tags` as
/// names. A post written in Patreon's newer editor keeps its text as a document (`content_json_string`), which becomes
/// HTML in `content`, as an older post's is.
pub(crate) fn post_from_api(answer: &Value) -> Result<Value, PreviewError> {
    let failed = |detail: &str| PreviewError::Failed {
        detail: detail.to_owned(),
    };
    let data = object(answer, "data").ok_or_else(|| failed("Patreon's answer has no post"))?;
    let mut post = object(data, "attributes")
        .cloned()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    post["id"] = data.get("id").cloned().unwrap_or(Value::Null);
    let included: Vec<&Value> = answer
        .get("included")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default();
    let related = |key: &str| -> Vec<Value> {
        let named = data
            .get("relationships")
            .and_then(|relationships| relationships.get(key))
            .and_then(|relationship| relationship.get("data"));
        let names: Vec<&Value> = match named {
            Some(Value::Array(names)) => names.iter().collect(),
            Some(name @ Value::Object(_)) => vec![name],
            _ => Vec::new(),
        };
        names
            .into_iter()
            .filter_map(|name| {
                let (kind, id) = (text(name, "type")?, text(name, "id")?);
                let item = included.iter().find(|item| {
                    text(item, "type").as_deref() == Some(kind.as_str())
                        && text(item, "id").as_deref() == Some(id.as_str())
                })?;
                let mut attributes = object(item, "attributes")
                    .cloned()
                    .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
                attributes["id"] = Value::String(id);
                Some(attributes)
            })
            .collect()
    };
    for key in ["images", "attachments", "attachments_media"] {
        post[key] = Value::Array(related(key));
    }
    post["campaign"] = related("campaign")
        .into_iter()
        .next()
        .unwrap_or(Value::Null);
    post["creator"] = related("user").into_iter().next().unwrap_or(Value::Null);
    post["tags"] = Value::Array(
        related("user_defined_tags")
            .iter()
            .filter_map(|tag| text(tag, "value"))
            .map(Value::String)
            .collect(),
    );
    if text(&post, "content").is_none()
        && let Some(document) = text(&post, "content_json_string")
    {
        post["content"] = Value::String(document_html(&document));
    }
    Ok(post)
}

/// A post's text written in Patreon's newer editor (a ProseMirror document, `{"type": "doc", "content": [...]}`) as
/// HTML: its paragraphs and headings, its links (marks on text, link blocks, buttons) and its pictures, which is what
/// `post_links`, `title` and `files` read. Anything else is left out, its text kept.
fn document_html(document: &str) -> String {
    let mut html = String::new();
    if let Ok(document) = serde_json::from_str::<Value>(document) {
        node_html(&document, &mut html);
    }
    html
}

fn node_html(node: &Value, html: &mut String) {
    let attribute = |name: &str| node.get("attrs").and_then(|attrs| text(attrs, name));
    let children = |html: &mut String| {
        for child in node
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            node_html(child, html);
        }
    };
    match text(node, "type").as_deref() {
        Some("text") => {
            let words = escape_html(&text(node, "text").unwrap_or_default());
            let href = node
                .get("marks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|mark| text(mark, "type").as_deref() == Some("link"))
                .find_map(|mark| mark.get("attrs").and_then(|attrs| text(attrs, "href")));
            match href {
                Some(href) => {
                    html.push_str(&format!("<a href=\"{}\">{words}</a>", escape_html(&href)))
                }
                None => html.push_str(&words),
            }
        }
        Some("link") => {
            html.push_str(&format!(
                "<a href=\"{}\">",
                escape_html(&attribute("href").unwrap_or_default())
            ));
            children(html);
            html.push_str("</a>");
        }
        Some("cta") => {
            if let Some(link) = attribute("button_link") {
                html.push_str(&format!(
                    "<p><a href=\"{}\">{}</a></p>",
                    escape_html(&link),
                    escape_html(&attribute("button_text").unwrap_or_default())
                ));
            }
        }
        Some("image") => {
            if let Some(source) = attribute("src") {
                html.push_str(&format!(
                    "<figure><img src=\"{}\"/></figure>",
                    escape_html(&source)
                ));
            }
        }
        Some("hardBreak") => html.push_str("<br/>"),
        Some("doc") => children(html),
        Some(_) | None => {
            html.push_str("<p>");
            children(html);
            html.push_str("</p>");
        }
    }
}

/// Text as HTML writes it: its `&`, `<`, `>` and `"` as entities, which `plain_text` and `anchor_links` decode.
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A file of a post that Tsundoku downloads itself, in the post's order: where it is and what it's called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PostFile {
    pub name: String,
    pub url: String,
}

/// A post's files (`post_from_api`), in the post's order and each once: its images (named by the post and their place
/// in it, "153639294 01.png", as a Fanbox post's are), its own picture when it isn't one of them (`image.large_url`),
/// its own file (`post_file`) unless that's the video or audio yt-dlp downloads (`media`), and the pictures in its
/// text, then its attachments under their own names. A file Patreon gives twice (the same picture as an image and in
/// the text) is known by the 32-character hash in its address.
pub(crate) fn files(post: &Value, media: bool) -> Vec<PostFile> {
    let list = |key: &str| -> Vec<Value> {
        post.get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let mut pictures: Vec<(String, Option<String>)> = Vec::new();
    for image in list("images") {
        let url = text(&image, "download_url").or_else(|| {
            object(&image, "image_urls")
                .and_then(|urls| text(urls, "original").or_else(|| text(urls, "default")))
        });
        pictures.extend(url.map(|url| (url, text(&image, "file_name"))));
    }
    if let Some(image) = object(post, "image")
        && let Some(url) = text(image, "large_url")
    {
        pictures.push((url, None));
    }
    if let Some(file) = object(post, "post_file")
        && !media
        && let Some(url) = text(file, "url")
        && !is_media_name(&url)
    {
        pictures.push((url, text(file, "name")));
    }
    let content = text(post, "content").unwrap_or_default();
    pictures.extend(
        content_pictures(&content)
            .into_iter()
            .map(|url| (url, None)),
    );

    let id = text(post, "id").unwrap_or_default();
    let mut seen: Vec<String> = Vec::new();
    let mut fresh = |url: &str| {
        let key = file_key(url);
        let new = !seen.contains(&key);
        if new {
            seen.push(key);
        }
        new
    };
    let pictures: Vec<(String, Option<String>)> =
        pictures.into_iter().filter(|(url, _)| fresh(url)).collect();
    let digits = pictures.len().to_string().len().max(2);
    let mut found: Vec<PostFile> = pictures
        .into_iter()
        .enumerate()
        .map(|(index, (url, name))| {
            let extension = name
                .as_deref()
                .and_then(extension_of)
                .or_else(|| extension_of(url.split(['?', '#']).next().unwrap_or(&url)))
                .unwrap_or_else(|| "jpg".to_owned());
            PostFile {
                name: format!("{id} {:0digits$}.{extension}", index + 1),
                url,
            }
        })
        .collect();
    for attachment in list("attachments_media")
        .into_iter()
        .chain(list("attachments"))
    {
        let Some(url) = text(&attachment, "download_url").or_else(|| text(&attachment, "url"))
        else {
            continue;
        };
        if !fresh(&url) {
            continue;
        }
        let name = text(&attachment, "file_name")
            .or_else(|| text(&attachment, "name"))
            .unwrap_or_else(|| "attachment".to_owned());
        found.push(PostFile { name, url });
    }
    found
}

/// The pictures in a post's text (`<img src="…">`), with their entities decoded.
fn content_pictures(html: &str) -> Vec<String> {
    let mut pictures = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(start) = lower[from..].find("<img") {
        let tag_start = from + start;
        let tag_end = lower[tag_start..]
            .find('>')
            .map_or(lower.len(), |end| tag_start + end);
        let tag = &html[tag_start..tag_end];
        if let Some(at) = tag.to_ascii_lowercase().find("src=\"") {
            let value = &tag[at + 5..];
            if let Some(end) = value.find('"') {
                let url = decode_entities(&value[..end]);
                if url.starts_with("https://") || url.starts_with("http://") {
                    pictures.push(url);
                }
            }
        }
        from = tag_end;
    }
    pictures
}

/// What tells two of a post's addresses for one file apart: the 32-character hash in its path that Patreon's file
/// server names files by, else the address without its query.
fn file_key(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.split('/')
        .rev()
        .find(|part| part.len() == 32 && part.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        .unwrap_or(path)
        .to_owned()
}

/// A file name's extension in lowercase, if it has a short one.
fn extension_of(name: &str) -> Option<String> {
    let (_, extension) = name.rsplit_once('.')?;
    let extension = extension.to_ascii_lowercase();
    (!extension.is_empty()
        && extension.len() <= 5
        && extension.bytes().all(|byte| byte.is_ascii_alphanumeric()))
    .then_some(extension)
}

/// A post with no images or files of its own, saying where its links went: the preview sends them to Pending.
pub(crate) fn nothing_of_its_own(links: usize) -> PreviewError {
    let detail = match links {
        0 => "this Patreon post has no images, files or links to download".to_owned(),
        1 => "this Patreon post has no images or files of its own; its link went to Pending"
            .to_owned(),
        count => format!(
            "this Patreon post has no images or files of its own; its {count} links went to Pending"
        ),
    };
    PreviewError::NoVideo { detail }
}

/// Whether a post's point is a video or an audio: its kind says so, its text written in the newer editor holds one
/// (a "text_only" post with Patreon's own player in it), or its own file (`post_file`) is one, such as a video Patreon
/// plays from Mux ("stream.mux.com", which gallery-dl names "video").
fn is_media(post: &Value) -> bool {
    let kind = text(post, "post_type").unwrap_or_default();
    if MEDIA_POST_TYPES.iter().any(|start| kind.starts_with(start)) {
        return true;
    }
    if text(post, "content_json_string")
        .and_then(|document| serde_json::from_str::<Value>(&document).ok())
        .is_some_and(|document| holds_media(&document))
    {
        return true;
    }
    let Some(file) = post.get("post_file").filter(|file| file.is_object()) else {
        return false;
    };
    [text(file, "name"), text(file, "url")]
        .into_iter()
        .flatten()
        .any(|name| is_media_name(&name))
}

/// Whether a document of the newer editor (`document_html`) holds a video or an audio of Patreon's own player.
fn holds_media(node: &Value) -> bool {
    matches!(text(node, "type").as_deref(), Some("video" | "audio"))
        || node
            .get("content")
            .and_then(Value::as_array)
            .is_some_and(|children| children.iter().any(holds_media))
}

fn is_media_name(name: &str) -> bool {
    if name == "video" || name.contains("stream.mux.com") {
        return true;
    }
    let path = name.split(['?', '#']).next().unwrap_or(name);
    path.rsplit_once('.').is_some_and(|(_, extension)| {
        MEDIA_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
    })
}

/// A post the account can't open (`current_user_can_view` false): for members of a tier of `min_cents_pledged_to_view`
/// a month or more, in the campaign's currency.
fn locked(post: &Value) -> Option<PreviewError> {
    let can_view = post.get("current_user_can_view").and_then(Value::as_bool);
    (can_view == Some(false)).then(|| PreviewError::PatreonLocked {
        cents: post
            .get("min_cents_pledged_to_view")
            .and_then(Value::as_u64)
            .and_then(|cents| u32::try_from(cents).ok())
            .unwrap_or(0),
        currency: post
            .get("campaign")
            .filter(|campaign| campaign.is_object())
            .and_then(|campaign| text(campaign, "currency")),
    })
}

/// The links inside a post that something may download, in its order and each once: those its text links to or
/// writes out, then the page it embeds, unless that's the video or audio yt-dlp downloads as the post itself
/// (`media`). Links to the post itself, to Patreon's pages other than posts (the creator's page, a checkout), and to a
/// channel's, an account's or a creator's page elsewhere are left out.
pub(crate) fn post_links(post: &Value, media: bool) -> Vec<String> {
    let content = text(post, "content").unwrap_or_default();
    let embed = post
        .get("embed")
        .filter(|embed| embed.is_object())
        .and_then(|embed| text(embed, "url"))
        .and_then(|link| metadata::normalize_link(&link));
    let mut found = anchor_links(&content);
    found.extend(written_links(&content));
    found.extend(embed.clone());
    let this_post = text(post, "id");
    let mut links: Vec<String> = Vec::new();
    for link in found {
        let Some(link) = metadata::normalize_link(&link) else {
            continue;
        };
        let on_patreon =
            sites::host(&link).is_some_and(|host| sites::on_domain(&host, "patreon.com"));
        let keep = if on_patreon {
            post_id(&link).is_some_and(|id| Some(&id) != this_post.as_ref())
        } else {
            crate::listing::listing_link(&link).is_none()
                && !(media && embed.as_ref() == Some(&link))
        };
        if keep && !links.contains(&link) {
            links.push(link);
        }
    }
    links
}

/// The links written out in a post's text (its HTML), as a copy of it would hold them. A link's own words are left
/// out, since `anchor_links` reads where it leads, and they may be shortened.
fn written_links(html: &str) -> Vec<String> {
    crate::pending::links_in_copy(&plain_text(html, false))
        .into_iter()
        // A link ending a sentence leaves its full stop or colon out.
        .map(|link| link.trim_end_matches(['.', ':']).to_owned())
        .collect()
}

/// A post's HTML as text, each tag a line break, its entities decoded, with or without its links' own words.
fn plain_text(html: &str, with_links: bool) -> String {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    let mut in_link = false;
    while let Some(start) = rest.find('<') {
        if with_links || !in_link {
            text.push_str(&rest[..start]);
        }
        let Some(length) = rest[start..].find('>') else {
            rest = "";
            break;
        };
        let tag = rest[start + 1..start + length].trim().to_ascii_lowercase();
        if tag == "a" || tag.starts_with("a ") {
            in_link = true;
        } else if tag == "/a" {
            in_link = false;
        }
        text.push('\n');
        rest = &rest[start + length + 1..];
    }
    if with_links || !in_link {
        text.push_str(rest);
    }
    decode_entities(&text.replace("&nbsp;", " "))
}

/// The post's title, else its text's first line, shortened.
pub(crate) fn title(post: &Value) -> Option<String> {
    let title = text(post, "title").map(|title| title.trim().to_owned());
    let title = title.filter(|title| !title.is_empty()).or_else(|| {
        let content = text(post, "content")?;
        let text = plain_text(&content, true);
        text.lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(str::to_owned)
    })?;
    Some(if title.chars().count() > TITLE_LENGTH {
        let shortened: String = title.chars().take(TITLE_LENGTH - 1).collect();
        format!("{}…", shortened.trim_end())
    } else {
        title
    })
}

/// The creator's name as Patreon shows it: the campaign's, else its owner's.
pub(crate) fn creator_name(post: &Value) -> Option<String> {
    object(post, "campaign")
        .and_then(|campaign| text(campaign, "name"))
        .or_else(|| object(post, "creator").and_then(|creator| text(creator, "full_name")))
}

/// The creator's handle, as in their page's address (`patreon.com/<handle>`): its owner's `vanity`, else the last part
/// of the campaign's or the owner's page, unless that's a number's page (`/user?u=…`).
pub(crate) fn creator_handle(post: &Value) -> Option<String> {
    let creator = object(post, "creator");
    creator
        .and_then(|creator| text(creator, "vanity"))
        .or_else(|| {
            [object(post, "campaign"), creator]
                .into_iter()
                .flatten()
                .filter_map(|owner| text(owner, "url"))
                .find_map(|page| handle_in(&page))
        })
}

fn handle_in(page: &str) -> Option<String> {
    let url = Url::parse(page).ok()?;
    let last = url.path_segments()?.rfind(|segment| !segment.is_empty())?;
    let reserved = ["user", "c", "cw", "posts", "profile", "home"];
    (!reserved.contains(&last) && url.query().is_none()).then(|| last.to_owned())
}

fn object<'a>(value: &'a Value, field: &str) -> Option<&'a Value> {
    value.get(field).filter(|inner| inner.is_object())
}

/// A field as text: a string, or a number written out, since Patreon gives IDs as either.
fn text(value: &Value, field: &str) -> Option<String> {
    match value.get(field)? {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn yt_dlp_poses_as_chrome_on_patreon_only() {
        let chrome = ["--impersonate", "chrome"];
        assert_eq!(
            yt_dlp_args("https://www.patreon.com/posts/video-sketchbook-32452882"),
            chrome
        );
        assert_eq!(yt_dlp_args("https://patreon.com/kurzgesagt/posts"), chrome);
        assert!(yt_dlp_args("https://www.youtube.com/watch?v=jNQXAC9IVRw").is_empty());
        assert!(yt_dlp_args("https://notpatreon.com/posts/1").is_empty());
    }

    #[test]
    fn reads_a_posts_id_from_each_kind_of_link() {
        for link in [
            "https://www.patreon.com/posts/custom-firefox-153639294",
            "https://www.patreon.com/posts/153639294",
            "https://patreon.com/posts/custom-firefox-153639294?utm_source=x",
            "https://www.patreon.com/Insanimate/posts/custom-firefox-153639294",
            "https://www.patreon.com/creation?hid=153639294",
        ] {
            assert_eq!(post_id(link).as_deref(), Some("153639294"), "{link}");
        }
        for link in [
            "https://www.patreon.com/koveliana",
            "https://www.patreon.com/koveliana/posts",
            "https://www.patreon.com/c/koveliana",
            "https://www.patreon.com/checkout/PI_Art314?rid=26956166",
            "https://www.patreon.com/posts/not-a-number",
            "https://www.example.com/posts/153639294",
        ] {
            assert_eq!(post_id(link), None, "{link}");
        }
    }

    fn post(fields: Value) -> Value {
        let mut post = json!({
            "category": "patreon",
            "id": 153639294,
            "title": "Custom Firefox icons",
            "post_type": "image_file",
            "current_user_can_view": true,
            "min_cents_pledged_to_view": null,
            "campaign": {"name": "Icon Shop", "currency": "USD", "url": "https://www.patreon.com/iconshop"},
            "creator": {"full_name": "Sam", "vanity": "iconshop", "url": "https://www.patreon.com/iconshop"},
            "content": "",
            "embed": null,
            "post_file": null,
        });
        for (key, value) in fields.as_object().unwrap() {
            post[key] = value.clone();
        }
        post
    }

    #[test]
    fn finds_the_links_inside_a_post() {
        let content = "<p>All 12 are in the mega link: https://mega.nz/folder/abc#key.</p>\
            <p>Video on <a href=\"https://www.youtube.com/watch?v=jNQXAC9IVRw&amp;t=5\" target=\"_blank\">\
            https://www.youtube.com/watch?v=jNQXAC9IVRw&amp;t…</a></p>\
            <p>Join: <a href=\"https://www.patreon.com/checkout/iconshop?rid=1\">tiers</a>, \
            earlier: <a href=\"https://www.patreon.com/posts/older-icons-152000001\">here</a>, \
            this one: https://www.patreon.com/posts/custom-firefox-153639294 \
            and my channel https://www.youtube.com/@iconshop</p>\
            <figure><img src=\"https://c10.patreonusercontent.com/4/patreon-media/p/post/1/a/1.png\"/></figure>";
        let links = post_links(&post(json!({"content": content})), false);
        assert_eq!(
            links,
            [
                "https://www.youtube.com/watch?v=jNQXAC9IVRw&t=5",
                "https://www.patreon.com/posts/older-icons-152000001",
                "https://mega.nz/folder/abc#key",
            ]
        );
    }

    #[test]
    fn an_embedded_page_is_a_link_unless_its_the_posts_video() {
        let embed = json!({"embed": {"url": "https://vimeo.com/76979871", "subject": "A film"}});
        let link_post = post(json!({"post_type": "link", "embed": embed["embed"]}));
        let link_facts = facts(&link_post);
        assert!(!link_facts.media);
        assert_eq!(link_facts.links, ["https://vimeo.com/76979871"]);

        let video_post = post(json!({"post_type": "video_embed", "embed": embed["embed"]}));
        let video_facts = facts(&video_post);
        assert!(video_facts.media);
        assert!(video_facts.links.is_empty());
    }

    #[test]
    fn a_video_or_audio_is_yt_dlps() {
        let is = |fields: Value| is_media(&post(fields));
        assert!(!is(json!({})));
        assert!(!is(json!({"post_type": "text_only"})));
        assert!(is(json!({"post_type": "video_external_file"})));
        assert!(is(json!({"post_type": "audio_file"})));
        assert!(is(json!({"post_type": "podcast"})));
        // A post of another kind whose own file is a video or an audio.
        assert!(is(
            json!({"post_file": {"name": "video", "url": "https://stream.mux.com/abc.m3u8?token=x"}})
        ));
        assert!(is(
            json!({"post_file": {"name": "Episode 12.mp3", "url": "https://c10.patreonusercontent.com/x"}})
        ));
        assert!(!is(
            json!({"post_file": {"name": "outfits.png", "url": "https://c10.patreonusercontent.com/x.png"}})
        ));
        // A "text_only" post written in the newer editor, with Patreon's own player in its text (Insanimate's, 2025).
        let document = |node: Value| {
            json!({"type": "doc", "content": [node, {"type": "paragraph", "content": [{"type": "text", "text": "Animation of @ScottFalco"}]}]})
                .to_string()
        };
        assert!(is(json!({
            "post_type": "text_only",
            "content_json_string": document(json!({"type": "video", "attrs": {"fallback_strategy": "fallback", "media_id": "588447586"}}))
        })));
        assert!(is(
            json!({"content_json_string": document(json!({"type": "audio", "attrs": {"media_id": "1"}}))})
        ));
        assert!(!is(json!({
            "post_type": "text_only",
            "content_json_string": document(json!({"type": "image", "attrs": {"src": "https://c10.patreonusercontent.com/x.png"}}))
        })));
    }

    #[test]
    fn a_post_for_paying_members_says_what_opens_it() {
        let locked_post =
            post(json!({"current_user_can_view": false, "min_cents_pledged_to_view": 500}));
        assert_eq!(
            facts(&locked_post).locked,
            Some(PreviewError::PatreonLocked {
                cents: 500,
                currency: Some("USD".to_owned())
            })
        );
        assert_eq!(facts(&post(json!({}))).locked, None);
        assert_eq!(
            nothing_of_its_own(1),
            PreviewError::NoVideo {
                detail:
                    "this Patreon post has no images or files of its own; its link went to Pending"
                        .to_owned()
            }
        );
    }

    /// Patreon's answer for a post of two images, an attachment and a picture in its text, trimmed to what's read.
    fn api_answer() -> Value {
        json!({
            "data": {
                "id": "12497641",
                "type": "post",
                "attributes": {
                    "title": "Free post",
                    "post_type": "image_file",
                    "current_user_can_view": true,
                    "min_cents_pledged_to_view": null,
                    "published_at": "2017-07-30T20:16:37.000+00:00",
                    "url": "https://www.patreon.com/posts/free-post-12497641",
                    "content": "<p>Sketches</p><figure><img src=\"https://c10.patreonusercontent.com/4/patreon-media/p/post/12497641/0123456789abcdef0123456789abcdef/eyJ3Ijo2MjB9/1.png?token-hash=x\" /></figure>",
                    "embed": null,
                    "image": {"large_url": "https://c10.patreonusercontent.com/4/patreon-media/p/post/12497641/0123456789abcdef0123456789abcdef/eyJ3IjoxMDgwfQ%3D%3D/1.png?token-hash=y"},
                    "post_file": {"name": "sketch.png", "url": "https://c10.patreonusercontent.com/4/patreon-media/p/post/12497641/0123456789abcdef0123456789abcdef/eyJhIjoxfQ%3D%3D/1.png?token-hash=z"}
                },
                "relationships": {
                    "campaign": {"data": {"id": "70261", "type": "campaign"}},
                    "user": {"data": {"id": "2931440", "type": "user"}},
                    "images": {"data": [{"id": "1", "type": "media"}, {"id": "2", "type": "media"}]},
                    "attachments_media": {"data": [{"id": "3", "type": "media"}]},
                    "user_defined_tags": {"data": [{"id": "user_defined;sketch", "type": "post_tag"}]}
                }
            },
            "included": [
                {"id": "70261", "type": "campaign", "attributes": {"name": "Sam's Art", "currency": "EUR", "url": "https://www.patreon.com/samsart"}},
                {"id": "2931440", "type": "user", "attributes": {"full_name": "Sam", "url": "https://www.patreon.com/samsart", "vanity": "samsart"}},
                {"id": "1", "type": "media", "attributes": {"file_name": "sketch.png", "download_url": "https://c10.patreonusercontent.com/4/patreon-media/p/post/12497641/0123456789abcdef0123456789abcdef/eyJhIjoxfQ%3D%3D/1.png?token-hash=a"}},
                {"id": "2", "type": "media", "attributes": {"file_name": "colors.jpeg", "download_url": "https://c10.patreonusercontent.com/4/patreon-media/p/post/12497641/fedcba9876543210fedcba9876543210/eyJhIjoxfQ%3D%3D/1.jpeg?token-hash=b"}},
                {"id": "3", "type": "media", "attributes": {"file_name": "brushes.zip", "download_url": "https://www.patreon.com/media-u/abc"}},
                {"id": "user_defined;sketch", "type": "post_tag", "attributes": {"tag_type": "user_defined", "value": "sketch"}}
            ]
        })
    }

    #[test]
    fn reads_a_post_from_patreons_answer() {
        let read = post_from_api(&api_answer()).unwrap();
        assert_eq!(text(&read, "id").as_deref(), Some("12497641"));
        assert_eq!(title(&read).as_deref(), Some("Free post"));
        assert_eq!(creator_name(&read).as_deref(), Some("Sam's Art"));
        assert_eq!(creator_handle(&read).as_deref(), Some("samsart"));
        assert_eq!(read["tags"], json!(["sketch"]));
        assert_eq!(read["images"].as_array().map(Vec::len), Some(2));
        assert_eq!(
            text(&read["attachments_media"][0], "file_name").as_deref(),
            Some("brushes.zip")
        );
        assert!(!facts(&read).media);
        assert!(matches!(
            post_from_api(&json!({"errors": []})),
            Err(PreviewError::Failed { .. })
        ));
    }

    #[test]
    fn lists_a_posts_pictures_once_then_its_attachments() {
        let read = post_from_api(&api_answer()).unwrap();
        let listed = files(&read, false);
        let names: Vec<&str> = listed.iter().map(|file| file.name.as_str()).collect();
        // The first image is also the post's picture, its own file and the picture in its text.
        assert_eq!(
            names,
            ["12497641 01.png", "12497641 02.jpeg", "brushes.zip"]
        );
        assert!(listed[0].url.ends_with("token-hash=a"));
        assert_eq!(listed[2].url, "https://www.patreon.com/media-u/abc");
    }

    #[test]
    fn leaves_a_video_posts_own_file_to_yt_dlp() {
        let video = post(json!({
            "post_type": "video_external_file",
            "post_file": {"name": "video", "url": "https://stream.mux.com/abc.m3u8?token=x"},
            "image": {"large_url": "https://c10.patreonusercontent.com/4/patreon-media/p/post/1/0123456789abcdef0123456789abcdef/x/1.jpg"}
        }));
        let listed = files(&video, true);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "153639294 01.jpg");
    }

    #[test]
    fn reads_a_post_written_in_patreons_newer_editor() {
        let document = json!({"type": "doc", "content": [
            {"type": "paragraph", "content": [
                {"type": "text", "text": "Download: "},
                {"type": "text", "text": "here", "marks": [{"type": "link", "attrs": {"href": "https://mega.nz/file/abc#key"}}]},
                {"type": "text", "text": " & enjoy <3"}
            ]},
            {"type": "image", "attrs": {"src": "https://c10.patreonusercontent.com/4/patreon-media/p/post/1/0123456789abcdef0123456789abcdef/x/1.png?a=1&b=2", "media_id": 9}},
            {"type": "cta", "attrs": {"button_link": "https://www.youtube.com/watch?v=jNQXAC9IVRw", "button_text": "Watch"}}
        ]});
        let mut answer = api_answer();
        answer["data"]["attributes"]["content"] = Value::Null;
        answer["data"]["attributes"]["content_json_string"] = Value::String(document.to_string());
        let read = post_from_api(&answer).unwrap();
        assert_eq!(
            post_links(&read, false),
            [
                "https://mega.nz/file/abc#key",
                "https://www.youtube.com/watch?v=jNQXAC9IVRw"
            ]
        );
        assert_eq!(
            content_pictures(&text(&read, "content").unwrap()),
            [
                "https://c10.patreonusercontent.com/4/patreon-media/p/post/1/0123456789abcdef0123456789abcdef/x/1.png?a=1&b=2"
            ]
        );
        assert!(plain_text(&text(&read, "content").unwrap(), true).contains("& enjoy <3"));
    }

    #[test]
    fn names_the_post_and_its_creator() {
        let named = post(json!({}));
        assert_eq!(title(&named).as_deref(), Some("Custom Firefox icons"));
        assert_eq!(creator_name(&named).as_deref(), Some("Icon Shop"));
        assert_eq!(creator_handle(&named).as_deref(), Some("iconshop"));

        let untitled = post(json!({
            "title": "",
            "content": "<p></p><p>+3 outfits ^_^ &amp; more</p>",
            "creator": {"full_name": "Sam", "url": "https://www.patreon.com/user?u=2931440"},
            "campaign": {"name": "Sam's Art", "url": "https://www.patreon.com/c/samsart"},
        }));
        assert_eq!(title(&untitled).as_deref(), Some("+3 outfits ^_^ & more"));
        assert_eq!(creator_handle(&untitled).as_deref(), Some("samsart"));
    }
}
