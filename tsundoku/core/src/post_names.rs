//! How the files of a post are named (Settings → Posts of Images): a name made of text and placeholders such as
//! `{account}` and `{number}`, turned into gallery-dl's `-f` format for each post. The user never sees gallery-dl's
//! own fields: each placeholder is the post's value when Tsundoku knows it from the preview (its ID, date and site),
//! and otherwise the field each site gives it under, with fallbacks so a name never says "None".

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::metadata::VideoInfo;
use crate::rules;

/// The account, the post's date, its ID and the file's place in it: "natgeo 2026-11-04 DfPQchkG1tg 1".
pub const DEFAULT_POST_FILE_NAME: &str = "{account} {date} {post} {number}";

/// Characters Windows refuses in a file name, besides control characters. `\` and `/` would also make folders.
const FORBIDDEN_CHARACTERS: &str = r#"<>:"/\|?*"#;

/// What a post file's name can hold besides text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PostNamePlaceholder {
    /// The account that posted: its handle, such as "natgeo".
    Account,
    /// The day it was posted, as YYYY-MM-DD.
    Date,
    /// The post's ID on its site.
    Post,
    /// The file's place in the post, from 1.
    Number,
    /// The site, as rules name it ("Instagram", "X").
    Site,
}

impl PostNamePlaceholder {
    pub const ALL: [Self; 5] = [
        Self::Account,
        Self::Date,
        Self::Post,
        Self::Number,
        Self::Site,
    ];

    /// Its name between braces.
    pub fn name(self) -> &'static str {
        match self {
            Self::Account => "account",
            Self::Date => "date",
            Self::Post => "post",
            Self::Number => "number",
            Self::Site => "site",
        }
    }

    fn named(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|placeholder| placeholder.name().eq_ignore_ascii_case(name))
    }
}

/// Why a post file name can't be used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum PostNameError {
    /// Without the post and the file's place in it, two files could get the same name and one would be lost.
    #[error("the name needs {{{}}}", placeholder.name())]
    MissingPlaceholder { placeholder: PostNamePlaceholder },
    /// Text in braces that isn't a placeholder, such as a misspelled one.
    #[error("{{{name}}} isn't a placeholder")]
    UnknownPlaceholder { name: String },
    /// A character Windows refuses in a file name, or a separator, which would put the file in another folder.
    #[error("{character} can't be in a file name")]
    ForbiddenCharacter { character: String },
}

/// A piece of a name: text as it is, or a placeholder.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Text(String),
    Placeholder(PostNamePlaceholder),
}

/// Checks a post file name and returns it trimmed, or the default when it's blank. It must hold `{post}` and
/// `{number}`, so each file of each post gets its own name, and nothing Windows refuses in a name.
pub fn check(template: &str) -> Result<String, PostNameError> {
    let template = template.trim();
    if template.is_empty() {
        return Ok(DEFAULT_POST_FILE_NAME.to_owned());
    }
    if let Some(character) = template
        .chars()
        .find(|character| character.is_control() || FORBIDDEN_CHARACTERS.contains(*character))
    {
        return Err(PostNameError::ForbiddenCharacter {
            character: if character.is_control() {
                format!("U+{:04X}", u32::from(character))
            } else {
                character.to_string()
            },
        });
    }
    let parts = parse(template)?;
    for required in [PostNamePlaceholder::Post, PostNamePlaceholder::Number] {
        if !parts.contains(&Part::Placeholder(required)) {
            return Err(PostNameError::MissingPlaceholder {
                placeholder: required,
            });
        }
    }
    Ok(template.to_owned())
}

/// The name's text and placeholders. `{word}` must be a placeholder; other braces are text.
fn parse(template: &str) -> Result<Vec<Part>, PostNameError> {
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        text.push_str(&rest[..start]);
        rest = &rest[start..];
        let word_length = rest[1..]
            .find(|character: char| !character.is_ascii_alphabetic())
            .unwrap_or(rest.len() - 1);
        let word = &rest[1..=word_length];
        if word.is_empty() || !rest[1 + word_length..].starts_with('}') {
            text.push('{');
            rest = &rest[1..];
            continue;
        }
        let placeholder =
            PostNamePlaceholder::named(word).ok_or_else(|| PostNameError::UnknownPlaceholder {
                name: word.to_owned(),
            })?;
        if !text.is_empty() {
            parts.push(Part::Text(std::mem::take(&mut text)));
        }
        parts.push(Part::Placeholder(placeholder));
        rest = &rest[word_length + 2..];
    }
    text.push_str(rest);
    if !text.is_empty() {
        parts.push(Part::Text(text));
    }
    Ok(parts)
}

/// gallery-dl's `-f` format for the files of `post`, named by `template` (from `check`; one that doesn't pass falls
/// back to the default), with the file's extension after it. A placeholder whose value isn't known (a post without
/// a date) is left out with the text that separates it from the one before, so no name has "None" or two spaces.
pub fn gallery_format(template: &str, post: &VideoInfo) -> String {
    let parts = check(template)
        .ok()
        .and_then(|template| parse(&template).ok())
        .unwrap_or_else(|| parse(DEFAULT_POST_FILE_NAME).expect("the default name parses"));
    let pieces: Vec<Option<String>> = parts
        .iter()
        .map(|part| match part {
            Part::Text(text) => Some(literal(text)),
            Part::Placeholder(placeholder) => value(*placeholder, post),
        })
        .collect();

    // Which pieces stay: a missing value goes, with the text before it, or after it when it's the first placeholder.
    let mut kept = vec![true; parts.len()];
    for (index, piece) in pieces.iter().enumerate() {
        if piece.is_some() {
            continue;
        }
        kept[index] = false;
        let earlier_placeholder = parts[..index]
            .iter()
            .enumerate()
            .any(|(at, part)| kept[at] && matches!(part, Part::Placeholder(_)));
        let separator = if earlier_placeholder {
            index.checked_sub(1)
        } else {
            Some(index + 1).filter(|next| *next + 1 < parts.len())
        };
        if let Some(separator) = separator
            && matches!(parts.get(separator), Some(Part::Text(_)))
        {
            kept[separator] = false;
        }
    }
    let mut format: String = pieces
        .into_iter()
        .zip(kept)
        .filter_map(|(piece, kept)| piece.filter(|_| kept))
        .collect();
    format.push_str(".{extension}");
    format
}

/// What a placeholder becomes in the format: the post's own value as text when the preview knows it, else the
/// fields gallery-dl gives it under on the post's site, ending with a value that's always there.
fn value(placeholder: PostNamePlaceholder, post: &VideoInfo) -> Option<String> {
    match placeholder {
        PostNamePlaceholder::Post => Some(literal(&post.video_id)),
        PostNamePlaceholder::Date => post.upload_date.as_deref().map(literal),
        PostNamePlaceholder::Site => Some(literal(&rules::site_name(post))),
        // A post's file without a number of its own (on a site that gives none) keeps the name it has on the site,
        // so no two files of the post share a name.
        PostNamePlaceholder::Number => Some("{num|filename}".to_owned()),
        PostNamePlaceholder::Account => {
            let fields: &[&str] = match post.site.as_str() {
                "instagram" => &["username"],
                "twitter" => &["author[name]", "user[name]"],
                _ => &[
                    "username",
                    "author[name]",
                    "author[username]",
                    "author[handle]",
                    "user[account]",
                    "user[name]",
                    "user[username]",
                    "blog[name]",
                ],
            };
            let fallback = post
                .uploader
                .as_deref()
                .map(quotable)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| quotable(&rules::site_name(post)));
            Some(format!("{{{}|'{fallback}'}}", fields.join("|")))
        }
    }
}

/// Text that stays as it is in a gallery-dl format: its braces doubled.
fn literal(text: &str) -> String {
    text.replace('{', "{{").replace('}', "}}")
}

/// Text that can go between quotes in a gallery-dl field (`{username|'…'}`): without the characters that end a quote
/// or a field, with spaces collapsed.
fn quotable(text: &str) -> String {
    let kept: String = text
        .chars()
        .filter(|character| !character.is_control() && !r#"'"{}[]|:!\/"#.contains(*character))
        .collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn post(site: &str, url: &str) -> VideoInfo {
        VideoInfo {
            site: site.to_owned(),
            site_name: "instagram.com".to_owned(),
            video_id: "DfPQchkG1tg".to_owned(),
            url: url.to_owned(),
            title: "A carousel".to_owned(),
            uploader: Some("National Geographic".to_owned()),
            account: None,
            duration_seconds: None,
            upload_date: Some("2026-11-04".to_owned()),
            thumbnail_url: None,
            best_height: None,
            post: Some(crate::metadata::PostContents {
                images: 2,
                videos: 1,
            }),
            files: None,
            post_files: Vec::new(),
            playlist: None,
        }
    }

    fn instagram() -> VideoInfo {
        post("instagram", "https://www.instagram.com/p/DfPQchkG1tg/")
    }

    #[test]
    fn reads_placeholders_in_any_case_and_other_braces_as_text() {
        assert_eq!(
            parse("{Account} {date}-{post} #{NUMBER} {x").unwrap(),
            [
                Part::Placeholder(PostNamePlaceholder::Account),
                Part::Text(" ".to_owned()),
                Part::Placeholder(PostNamePlaceholder::Date),
                Part::Text("-".to_owned()),
                Part::Placeholder(PostNamePlaceholder::Post),
                Part::Text(" #".to_owned()),
                Part::Placeholder(PostNamePlaceholder::Number),
                Part::Text(" {x".to_owned()),
            ]
        );
        assert_eq!(
            parse("{} {1} {post}").unwrap(),
            [
                Part::Text("{} {1} ".to_owned()),
                Part::Placeholder(PostNamePlaceholder::Post)
            ]
        );
    }

    #[test]
    fn checks_names() {
        assert_eq!(
            check("  {site} {post} {number}  "),
            Ok("{site} {post} {number}".to_owned())
        );
        assert_eq!(check("   "), Ok(DEFAULT_POST_FILE_NAME.to_owned()));
        assert_eq!(
            check(DEFAULT_POST_FILE_NAME),
            Ok(DEFAULT_POST_FILE_NAME.to_owned())
        );
        assert_eq!(
            check("{account} {number}"),
            Err(PostNameError::MissingPlaceholder {
                placeholder: PostNamePlaceholder::Post
            })
        );
        assert_eq!(
            check("{account} {post}"),
            Err(PostNameError::MissingPlaceholder {
                placeholder: PostNamePlaceholder::Number
            })
        );
        assert_eq!(
            check("{post} {number} {title}"),
            Err(PostNameError::UnknownPlaceholder {
                name: "title".to_owned()
            })
        );
        for (name, character) in [
            (r"{account}\{post} {number}", r"\"),
            ("{account}/{post} {number}", "/"),
            ("{post} {number}?", "?"),
            ("{post}: {number}", ":"),
        ] {
            assert_eq!(
                check(name),
                Err(PostNameError::ForbiddenCharacter {
                    character: character.to_owned()
                }),
                "{name}"
            );
        }
        assert_eq!(
            check("{post}\t{number}"),
            Err(PostNameError::ForbiddenCharacter {
                character: "U+0009".to_owned()
            })
        );
    }

    #[test]
    fn names_instagram_files_by_the_account_date_post_and_number() {
        assert_eq!(
            gallery_format(DEFAULT_POST_FILE_NAME, &instagram()),
            "{username|'National Geographic'} 2026-11-04 DfPQchkG1tg {num|filename}.{extension}"
        );
    }

    #[test]
    fn names_x_files_by_the_authors_handle() {
        let mut tweet = post("twitter", "https://x.com/NASA/status/2105698284354367897");
        tweet.video_id = "2105698284354367897".to_owned();
        tweet.uploader = Some("NASA".to_owned());
        assert_eq!(
            gallery_format("{site} {account} {post} {number}", &tweet),
            "X {author[name]|user[name]|'NASA'} 2105698284354367897 {num|filename}.{extension}"
        );
    }

    #[test]
    fn falls_back_to_the_uploader_or_the_site_on_other_sites() {
        let mut other = post("tumblr", "https://example.tumblr.com/post/1");
        other.uploader = Some("it's {me}: [a|b]".to_owned());
        let format = gallery_format("{account} {post} {number}", &other);
        assert!(format.starts_with("{username|author[name]|"), "{format}");
        assert!(format.contains("|blog[name]|'its me ab'}"), "{format}");

        other.uploader = None;
        assert!(
            gallery_format("{account} {post} {number}", &other).contains("|'tumblr'}"),
            "the site's name when nothing else is known"
        );
    }

    #[test]
    fn leaves_out_an_unknown_date_with_its_separator() {
        let mut undated = instagram();
        undated.upload_date = None;
        assert_eq!(
            gallery_format("{account} - {date} - {post} {number}", &undated),
            "{username|'National Geographic'} - DfPQchkG1tg {num|filename}.{extension}"
        );
        assert_eq!(
            gallery_format("{date} {post} {number}", &undated),
            "DfPQchkG1tg {num|filename}.{extension}"
        );
        assert_eq!(
            gallery_format("Saved {date}", &undated).as_str(),
            // Not a name that passes `check`, so the default is used.
            "{username|'National Geographic'} DfPQchkG1tg {num|filename}.{extension}"
        );
    }

    #[test]
    fn keeps_text_braces_and_falls_back_to_the_default_for_a_bad_name() {
        assert_eq!(
            gallery_format("{post} {1} {number}", &instagram()),
            "DfPQchkG1tg {{1}} {num|filename}.{extension}"
        );
        assert_eq!(
            gallery_format("{post} {number} {title}", &instagram()),
            gallery_format(DEFAULT_POST_FILE_NAME, &instagram())
        );
    }
}
