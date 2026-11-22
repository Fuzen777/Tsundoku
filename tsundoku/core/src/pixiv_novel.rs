//! Pixiv's novels (`www.pixiv.net/novel/show.php?id=<id>`, or `/n/<id>`), saved as EPUB books (Fuzen's choice). gallery-dl
//! reads a novel with Pixiv's sign-in (its `pixiv-novel` extractor, `pixiv_account::gallery_dl_config`): its details,
//! then its text as a `text:` address, in Pixiv's own markup. The preview card shows it as a post of one file, its work
//! type Novel; its download reads it again and writes the book (`book`, `epub::write`), the novel's cover first, then
//! a page for each of its pages (`[newpage]`), its chapters (`[chapter:…]`) in the table of contents, its ruby
//! (`[[rb:漢字 > かんじ]]`) as ruby, its links (`[[jumpuri:…]]`, `[jump:<page>]`) as links. Pictures it embeds
//! (`[pixivimage:…]`, `[uploadedimage:…]`) aren't in the book yet: a line says one was there.

use std::sync::OnceLock;

use regex::Regex;
use reqwest::Url;
use serde_json::Value;

use crate::epub::{self, Book, Cover, Page};
use crate::metadata::{
    PixivDetails, PixivWorkType, PostContents, PreviewError, VideoInfo, VideoTag, tags_from,
};

/// The novel a link names, by its ID.
pub fn novel_id(link: &str) -> Option<String> {
    let url = Url::parse(link).ok()?;
    if url.host_str() != Some("www.pixiv.net") {
        return None;
    }
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect();
    // A language first ("/en/novel/…") changes nothing.
    let segments = match segments.as_slice() {
        [language, rest @ ..] if language.len() == 2 => rest,
        all => all,
    };
    let id = match segments {
        ["novel", "show.php"] => url
            .query_pairs()
            .find(|(key, _)| key == "id")
            .map(|(_, id)| id.into_owned())?,
        ["n", id] => (*id).to_owned(),
        _ => return None,
    };
    (!id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit())).then_some(id)
}

/// A novel's page, as Pixiv writes it.
pub fn novel_page(id: &str) -> String {
    format!("https://www.pixiv.net/novel/show.php?id={id}")
}

/// A novel as gallery-dl answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Novel {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub account: Option<String>,
    /// As YYYY-MM-DD.
    pub date: Option<String>,
    pub caption: Option<String>,
    pub tags: Vec<VideoTag>,
    pub series: Option<String>,
    pub rating: Option<String>,
    pub ai: bool,
    /// Its cover, at the size Pixiv's lists show it.
    pub cover: Option<String>,
    pub characters: u32,
    /// In Pixiv's markup.
    pub text: String,
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// The novel in gallery-dl's answer (`--dump-json`): its `[2, details]` and `[3, "text:…", details]` messages.
pub fn parse(json: &str) -> Result<Novel, PreviewError> {
    let messages: Vec<Value> =
        serde_json::from_str(json).map_err(|error| PreviewError::Failed {
            detail: format!("gallery-dl's answer can't be read: {error}"),
        })?;
    let mut details = None;
    let mut body = None;
    for message in &messages {
        let Some(parts) = message.as_array() else {
            continue;
        };
        match parts.first().and_then(Value::as_i64) {
            Some(-1) => {
                return Err(crate::gallery::classify_error(
                    parts.get(1).unwrap_or(&Value::Null),
                ));
            }
            Some(2) => details = details.or_else(|| parts.last().cloned()),
            Some(3) => {
                if let Some(content) = parts
                    .get(1)
                    .and_then(Value::as_str)
                    .and_then(|url| url.strip_prefix("text:"))
                {
                    body = Some(content.to_owned());
                    details = details.or_else(|| parts.last().cloned());
                }
            }
            _ => {}
        }
    }
    let details = details.ok_or_else(|| PreviewError::Unavailable {
        detail: "gallery-dl found no novel".to_owned(),
    })?;
    let body = body.ok_or_else(|| PreviewError::Unavailable {
        detail: "Pixiv gave the novel without its text".to_owned(),
    })?;
    let id = details
        .get("id")
        .and_then(|id| {
            id.as_u64()
                .map(|id| id.to_string())
                .or_else(|| id.as_str().map(str::to_owned))
        })
        .ok_or_else(|| PreviewError::Failed {
            detail: "the novel has no ID".to_owned(),
        })?;
    let user = details.get("user");
    let tags = details
        .get("tags")
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(|tag| match tag {
                    Value::String(name) => Some(VideoTag::named(name)),
                    Value::Object(_) => text(tag, "name").map(|name| VideoTag {
                        name,
                        english: text(tag, "translated_name"),
                    }),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let date =
        text(&details, "create_date").and_then(|date| crate::metadata::day_of_timestamp(&date));
    Ok(Novel {
        title: text(&details, "title").unwrap_or_else(|| format!("Novel {id}")),
        author: user.and_then(|user| text(user, "name")),
        account: user.and_then(|user| text(user, "account")),
        date,
        caption: text(&details, "caption"),
        tags: tags_from(tags),
        series: details
            .get("series")
            .filter(|series| series.is_object())
            .and_then(|series| text(series, "title")),
        rating: text(&details, "rating"),
        ai: details.get("novel_ai_type").and_then(Value::as_i64) == Some(2),
        cover: details
            .get("image_urls")
            .and_then(|urls| text(urls, "large").or_else(|| text(urls, "medium"))),
        characters: details
            .get("text_length")
            .and_then(Value::as_u64)
            .and_then(|length| u32::try_from(length).ok())
            .unwrap_or(0),
        text: body,
        id,
    })
}

impl Novel {
    /// The novel as the preview card and the queue know it: a post of one file, its work type Novel.
    pub fn video_info(&self) -> VideoInfo {
        VideoInfo {
            site: "pixiv-novel".to_owned(),
            site_name: "pixiv.net".to_owned(),
            video_id: self.id.clone(),
            url: novel_page(&self.id),
            title: self.title.clone(),
            uploader: self.author.clone(),
            account: self.account.clone(),
            duration_seconds: None,
            upload_date: self.date.clone(),
            thumbnail_url: self.cover.clone(),
            best_height: None,
            post: Some(PostContents {
                images: 0,
                videos: 0,
                others: 1,
            }),
            files: None,
            post_files: Vec::new(),
            shared_files: Vec::new(),
            playlist: None,
            tags: self.tags.clone(),
            pixiv: Some(PixivDetails {
                rating: self.rating.clone(),
                work_type: Some(PixivWorkType::Novel),
                ai: self.ai,
                series: self.series.clone(),
            }),
            fanbox: None,
        }
    }

    /// The novel as a book, with its cover when it could be read.
    pub fn book(&self, cover: Option<Cover>) -> Book {
        Book {
            identifier: novel_page(&self.id),
            title: self.title.clone(),
            author: self.author.clone(),
            language: "ja".to_owned(),
            date: self.date.clone(),
            description: self.caption.as_deref().map(strip_tags),
            subjects: self.tags.iter().map(|tag| tag.name.clone()).collect(),
            cover,
            pages: pages(&self.text),
        }
    }
}

/// Pixiv's caption is HTML: its text, its line breaks kept.
fn strip_tags(html: &str) -> String {
    static TAG: OnceLock<Regex> = OnceLock::new();
    let tag = TAG.get_or_init(|| Regex::new(r"(?i)<br\s*/?>|<[^>]*>").expect("a pattern"));
    let text = tag.replace_all(html, |found: &regex::Captures| {
        if found[0].to_lowercase().starts_with("<br") {
            "\n"
        } else {
            ""
        }
        .to_owned()
    });
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// The novel's text as the book's pages: one for each of its pages (`[newpage]`), a line a paragraph.
pub fn pages(markup: &str) -> Vec<Page> {
    static CHAPTER: OnceLock<Regex> = OnceLock::new();
    let chapter =
        CHAPTER.get_or_init(|| Regex::new(r"^\s*\[chapter:(.*)\]\s*$").expect("a pattern"));
    markup
        .split("[newpage]")
        .map(|page| {
            let mut title = None;
            let mut body = String::new();
            for line in page.trim_matches(['\r', '\n']).lines() {
                if let Some(found) = chapter.captures(line) {
                    let heading = inline(&found[1]);
                    title.get_or_insert_with(|| plain(&found[1]));
                    body.push_str(&format!("<h2>{heading}</h2>\n"));
                } else if line.trim().is_empty() {
                    body.push_str("<p class=\"blank\"><br/></p>\n");
                } else {
                    body.push_str(&format!("<p>{}</p>\n", inline(line)));
                }
            }
            Page { title, body }
        })
        .collect()
}

fn inline_pattern() -> &'static Regex {
    static INLINE: OnceLock<Regex> = OnceLock::new();
    INLINE.get_or_init(|| {
        Regex::new(
            r"\[\[rb:(?P<base>.+?) ?> ?(?P<reading>.+?)\]\]|\[\[jumpuri:(?P<label>.+?) ?> ?(?P<url>.+?)\]\]|\[jump:(?P<page>\d+)\]|\[(?:pixivimage|uploadedimage):[^\]]*\]",
        )
        .expect("a pattern")
    })
}

/// A line's text as XHTML: ruby, links and embedded pictures in Pixiv's markup become their own, the rest escaped.
fn inline(line: &str) -> String {
    let mut html = String::new();
    let mut last = 0;
    for found in inline_pattern().captures_iter(line) {
        let whole = found.get(0).expect("a match");
        html.push_str(&epub::escape(&line[last..whole.start()]));
        last = whole.end();
        if let (Some(base), Some(reading)) = (found.name("base"), found.name("reading")) {
            html.push_str(&format!(
                "<ruby>{}<rt>{}</rt></ruby>",
                epub::escape(base.as_str().trim()),
                epub::escape(reading.as_str().trim())
            ));
        } else if let (Some(label), Some(url)) = (found.name("label"), found.name("url")) {
            let url = url.as_str().trim();
            if url.starts_with("https://") || url.starts_with("http://") {
                html.push_str(&format!(
                    "<a href=\"{}\">{}</a>",
                    epub::escape(url),
                    epub::escape(label.as_str().trim())
                ));
            } else {
                html.push_str(&epub::escape(label.as_str().trim()));
            }
        } else if let Some(page) = found.name("page") {
            let number: usize = page.as_str().parse().unwrap_or(1).max(1);
            html.push_str(&format!(
                "<a href=\"page-{number:04}.xhtml\">→ {number}</a>"
            ));
        } else {
            html.push_str("<span class=\"picture\">[picture]</span>");
        }
    }
    html.push_str(&epub::escape(&line[last..]));
    html
}

/// A chapter's title without its markup, for the table of contents: ruby's base text, a link's label.
fn plain(text: &str) -> String {
    inline_pattern()
        .replace_all(text, |found: &regex::Captures| {
            found
                .name("base")
                .or_else(|| found.name("label"))
                .map_or_else(String::new, |part| part.as_str().trim().to_owned())
        })
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_novels_id_from_its_links() {
        for link in [
            "https://www.pixiv.net/novel/show.php?id=29689492",
            "https://www.pixiv.net/en/novel/show.php?id=29689492&view=reading",
            "https://www.pixiv.net/n/29689492",
        ] {
            assert_eq!(novel_id(link).as_deref(), Some("29689492"), "{link}");
        }
        for link in [
            "https://www.pixiv.net/artworks/1",
            "https://www.pixiv.net/novel/series/1",
            "https://www.pixiv.net/novel/show.php?id=abc",
            "https://example.com/novel/show.php?id=1",
        ] {
            assert_eq!(novel_id(link), None, "{link}");
        }
    }

    #[test]
    fn turns_pixivs_markup_into_pages() {
        let markup = "[chapter:第一章 [[rb:始 > はじ]]まり]\nある日、<b>彼</b>は[[rb:東京 > とうきょう]]へ。\n\n\
            [[jumpuri:作者のページ > https://www.pixiv.net/users/1]]\n[pixivimage:123-1]\n[newpage]\n続き[jump:1]";
        let pages = pages(markup);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].title.as_deref(), Some("第一章 始まり"));
        assert_eq!(
            pages[0].body,
            "<h2>第一章 <ruby>始<rt>はじ</rt></ruby>まり</h2>\n\
             <p>ある日、&lt;b&gt;彼&lt;/b&gt;は<ruby>東京<rt>とうきょう</rt></ruby>へ。</p>\n\
             <p class=\"blank\"><br/></p>\n\
             <p><a href=\"https://www.pixiv.net/users/1\">作者のページ</a></p>\n\
             <p><span class=\"picture\">[picture]</span></p>\n"
        );
        assert_eq!(pages[1].title, None);
        assert_eq!(
            pages[1].body,
            "<p>続き<a href=\"page-0001.xhtml\">→ 1</a></p>\n"
        );
    }

    #[test]
    fn reads_gallery_dls_answer_for_a_novel() {
        let json = serde_json::json!([
            [2, {"category": "pixiv-novel", "id": 7, "title": "A Novel", "caption": "One<br />Two",
                 "create_date": "2026-11-20T10:00:00+09:00", "text_length": 3584, "novel_ai_type": 1,
                 "rating": "General", "series": {"id": 1, "title": "Series"},
                 "image_urls": {"large": "https://i.pximg.net/c/240x480_80/novel-cover-master/img/x.jpg"},
                 "user": {"name": "Someone", "account": "someone"}, "tags": [{"name": "原神", "translated_name": "Genshin Impact"}]}],
            [3, "text:Hello\n[newpage]\nWorld", {"category": "pixiv-novel", "extension": "txt"}]
        ])
        .to_string();
        let novel = parse(&json).unwrap();
        assert_eq!(novel.id, "7");
        assert_eq!(novel.text, "Hello\n[newpage]\nWorld");
        assert_eq!(novel.characters, 3584);
        let video = novel.video_info();
        assert_eq!(video.url, "https://www.pixiv.net/novel/show.php?id=7");
        assert_eq!(video.upload_date.as_deref(), Some("2026-11-20"));
        assert_eq!(video.pixiv.unwrap().work_type, Some(PixivWorkType::Novel));
        let book = novel.book(None);
        assert_eq!(book.date.as_deref(), Some("2026-11-20"));
        assert_eq!(book.description.as_deref(), Some("One\nTwo"));
        assert_eq!(book.pages.len(), 2);
    }
}
