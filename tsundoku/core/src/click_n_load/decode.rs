//! Click'n'Load 2's links: `crypted` is base64 of the links, one per line, encrypted with AES-128-CBC whose key and IV
//! are both the 16 bytes a JavaScript function (`jk`) returns as 32 hex characters. Tsundoku never runs a page's
//! script: the key is read from the function's text, as the quoted value it returns, and a function that computes it
//! any other way is refused (`DecodeError::KeyUnreadable`).

use std::sync::LazyLock;

use base64::Engine;
use base64::alphabet;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use regex::Regex;

use crate::cbc;
use crate::metadata;
use crate::pending::watcher::has_web_scheme;

/// The longest `crypted` taken, as text.
pub(super) const MOST_CRYPTED: usize = 1 << 20;

/// Why a post's links couldn't be read. Each is a sentence the page's post gets back as its answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("Tsundoku can't read this page's key without running its script, which it never does.")]
    KeyUnreadable,
    #[error("The links this page sent aren't valid base64.")]
    BadBase64,
    #[error("The links this page sent aren't a whole number of AES blocks.")]
    WrongLength,
    #[error("The links this page sent are too long.")]
    TooLong,
    #[error("This page sent no links.")]
    NoLinks,
}

/// Standard base64, with or without its padding.
const BASE64: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// The value a `jk` function returns, when it returns 32 hex characters in quotes and nothing more: `function f(){
/// return '…'; }`, with either quotes, any spaces and line breaks, and the value in parentheses or not.
static RETURNED_KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\breturn\s*\(?\s*['"]([0-9A-Fa-f]{32})['"]\s*\)?\s*(?:;|\})"#)
        .expect("a valid pattern")
});

/// The links in a Click'n'Load 2 post: `crypted` and `jk` as the form gave them.
pub fn decode(crypted: &str, jk: &str) -> Result<Vec<String>, DecodeError> {
    if crypted.len() > MOST_CRYPTED {
        return Err(DecodeError::TooLong);
    }
    let key = key_of(jk)?;
    // A "+" sent unencoded in a form arrives as a space; line breaks are only wrapping.
    let text: String = crypted
        .trim()
        .chars()
        .filter(|character| !matches!(character, '\r' | '\n'))
        .map(|character| if character == ' ' { '+' } else { character })
        .collect();
    let encrypted = BASE64.decode(text).map_err(|_| DecodeError::BadBase64)?;
    if encrypted.is_empty() || encrypted.len() % 16 != 0 {
        return Err(DecodeError::WrongLength);
    }
    let links = web_links_in(&without_padding(cbc::decrypt(&key, key, &encrypted)));
    if !links.is_empty() {
        return Ok(links);
    }
    // Some senders put the IV ahead of the links, as their first block.
    let (iv, rest) = encrypted.split_at(16);
    let iv: [u8; 16] = iv.try_into().expect("16 bytes");
    let links = web_links_in(&without_padding(cbc::decrypt(&key, iv, rest)));
    if links.is_empty() {
        Err(DecodeError::NoLinks)
    } else {
        Ok(links)
    }
}

/// The 16 bytes of the key a `jk` function returns (`RETURNED_KEY`).
pub(super) fn key_of(jk: &str) -> Result<[u8; 16], DecodeError> {
    let hex = RETURNED_KEY
        .captures(jk)
        .and_then(|found| found.get(1))
        .ok_or(DecodeError::KeyUnreadable)?
        .as_str();
    let mut key = [0; 16];
    for (byte, pair) in key.iter_mut().zip(hex.as_bytes().chunks(2)) {
        let pair = std::str::from_utf8(pair).map_err(|_| DecodeError::KeyUnreadable)?;
        *byte = u8::from_str_radix(pair, 16).map_err(|_| DecodeError::KeyUnreadable)?;
    }
    Ok(key)
}

/// The text without its padding: trailing zero bytes, else a valid PKCS #7 pad.
fn without_padding(mut plain: Vec<u8>) -> String {
    if plain.last() == Some(&0) {
        while plain.last() == Some(&0) {
            plain.pop();
        }
    } else if let Some(&pad) = plain.last()
        && (1..=16).contains(&pad)
        && plain.len() >= usize::from(pad)
        && plain[plain.len() - usize::from(pad)..]
            .iter()
            .all(|byte| *byte == pad)
    {
        plain.truncate(plain.len() - usize::from(pad));
    }
    String::from_utf8_lossy(&plain).into_owned()
}

/// The http and https links in a text, one per word, each once: a word without its scheme isn't taken, since
/// decrypted garbage can look like a domain. NUL bytes, from zero padding, part words too.
pub(super) fn web_links_in(text: &str) -> Vec<String> {
    let mut links: Vec<String> = Vec::new();
    for word in text.split(|character: char| character.is_whitespace() || character == '\0') {
        let word = word
            .trim_start_matches(['<', '(', '[', '{', '"', '\''])
            .trim_end_matches(['>', ')', ']', '}', '"', '\'', ',', ';']);
        if has_web_scheme(word)
            && let Some(link) = metadata::normalize_link(word)
            && !links.contains(&link)
        {
            links.push(link);
        }
    }
    links
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Made with .NET's AES (PowerShell), not this code: key and IV "1234567890abcdef", zero padding, the links
    /// "https://www.youtube.com/watch?v=jNQXAC9IVRw\r\nhttps://archive.org/details/test-item\r\n".
    const ZERO_PADDED: &str = "Id4uAcAc/pk1oIGvGjZpAmjMkfZbMK0ma1yOkUFm0scHF39DcL4UWZcJFOnjbeeMCPFHcaGTMSd3SfDtIuX59RmiBWSZpu/a7qswYDJfJPpKrRpvqooZaRy63fV10+2R";
    /// The same key, PKCS #7 padding, "https://example.com/a\nhttps://example.com/b".
    const PKCS7_PADDED: &str = "53BH2S/ZS00dN7zYix1ozqVDzYS8PO6CwP7vhAAwSfFH+PNm95/gVGoQzH/56bLw";
    /// The same key with the IV 0, 7, 14…105 sent as the first block, zero padding, "https://example.com/first-block".
    const IV_FIRST: &str = "AAcOFRwjKjE4P0ZNVFtiaWOHkGwFXaVSDAht1Rme0VKqxSnaH3aJV+GJ92/+e71f";
    const JK: &str = "function f(){ return '31323334353637383930616263646566';}";

    #[test]
    fn decodes_known_links() {
        assert_eq!(
            decode(ZERO_PADDED, JK),
            Ok(vec![
                "https://www.youtube.com/watch?v=jNQXAC9IVRw".to_owned(),
                "https://archive.org/details/test-item".to_owned(),
            ])
        );
        assert_eq!(
            decode(PKCS7_PADDED, JK),
            Ok(vec![
                "https://example.com/a".to_owned(),
                "https://example.com/b".to_owned()
            ])
        );
        assert_eq!(
            decode(IV_FIRST, JK),
            Ok(vec!["https://example.com/first-block".to_owned()]),
            "the first block as the IV"
        );
    }

    #[test]
    fn a_space_stands_for_a_plus_sign() {
        let sent = ZERO_PADDED.replace('+', " ");
        assert_ne!(sent, ZERO_PADDED);
        assert_eq!(decode(&sent, JK), decode(ZERO_PADDED, JK));
        let wrapped = format!("{}\r\n{}", &ZERO_PADDED[..64], &ZERO_PADDED[64..]);
        assert_eq!(decode(&wrapped, JK), decode(ZERO_PADDED, JK));
    }

    #[test]
    fn reads_the_key_from_the_function_without_running_it() {
        let key = *b"1234567890abcdef";
        for jk in [
            JK,
            "function f(){ return \"31323334353637383930616263646566\";}",
            "function f ( ) {\n\treturn   '31323334353637383930616263646566'  ;\n}",
            "function f(){return('31323334353637383930616263646566')}",
            "function f(){ return '31323334353637383930616263646566' }",
        ] {
            assert_eq!(key_of(jk), Ok(key), "{jk}");
        }
        for jk in [
            "function f(){ return a + '3132333435363738'; }",
            "function f(){ return '3132333435363738' + '3930616263646566'; }",
            "function f(){ var k = '31323334353637383930616263646566'; return k; }",
            "function f(){ return '3132333435363738393061626364656'; }",
            "function f(){ return 'zz323334353637383930616263646566'; }",
            "",
        ] {
            assert_eq!(key_of(jk), Err(DecodeError::KeyUnreadable), "{jk}");
            assert_eq!(decode(ZERO_PADDED, jk), Err(DecodeError::KeyUnreadable));
        }
    }

    #[test]
    fn refuses_what_isnt_links() {
        assert_eq!(
            decode("not base64 at all!", JK),
            Err(DecodeError::BadBase64)
        );
        assert_eq!(decode("", JK), Err(DecodeError::WrongLength));
        assert_eq!(decode("AAAA", JK), Err(DecodeError::WrongLength));
        assert_eq!(
            decode(&"A".repeat(MOST_CRYPTED + 4), JK),
            Err(DecodeError::TooLong)
        );
        // Right length, wrong key: garbage, with no link either way.
        assert_eq!(
            decode(
                ZERO_PADDED,
                "function f(){ return '00000000000000000000000000000000';}"
            ),
            Err(DecodeError::NoLinks)
        );
    }

    #[test]
    fn takes_only_web_links() {
        assert_eq!(
            web_links_in(
                "https://a.example/1\r\nexample.com ftp://b.example/2 <https://c.example/3> https://a.example/1\0\0"
            ),
            ["https://a.example/1", "https://c.example/3"]
        );
        assert_eq!(without_padding(b"abc\x03\x03\x03".to_vec()), "abc");
        assert_eq!(without_padding(b"abc\x00\x00".to_vec()), "abc");
        assert_eq!(
            without_padding(b"abc\x05".to_vec()),
            "abc\x05",
            "not a valid pad"
        );
    }
}
