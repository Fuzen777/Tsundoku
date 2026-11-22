//! Site pictures the window can't show by itself: Pixiv's (on i.pximg.net), which Pixiv only serves to its own pages
//! (403 without its Referer). The app crate's `sitepicture` protocol fetches them here, with that Referer, for the
//! preview card, the pickers and Downloads (`api.ts`'s `sitePictureUrl` names them by their address in base64url).
//! Only Pixiv's picture servers are asked, so the protocol can't be made to fetch anything else.

use std::io::Read;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::header::{CONTENT_TYPE, REFERER};

use crate::hosts;

/// The hosts whose pictures need their site's Referer.
const PIXIV_PICTURE_HOSTS: [&str; 2] = ["i.pximg.net", "s.pximg.net"];
const PIXIV_REFERER: &str = "https://www.pixiv.net/";
/// The largest picture read: a preview's is far smaller.
const LARGEST: u64 = 15 * 1024 * 1024;

/// A picture as the protocol serves it.
pub struct SitePicture {
    pub content_type: String,
    pub bytes: Vec<u8>,
}

/// The picture whose address is `name` in base64url (no padding), when it's on one of Pixiv's picture servers and
/// answers with a picture. Why not, otherwise.
pub fn fetch(name: &str) -> Result<SitePicture, String> {
    let address = URL_SAFE_NO_PAD
        .decode(name.trim_start_matches('/'))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .ok_or("not a picture's address")?;
    let url = reqwest::Url::parse(&address).map_err(|error| error.to_string())?;
    let allowed = url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| PIXIV_PICTURE_HOSTS.contains(&host));
    if !allowed {
        return Err(format!("{address} isn't one of Pixiv's pictures"));
    }
    let client = hosts::client().map_err(|error| error.to_string())?;
    let response = client
        .get(url)
        .header(REFERER, PIXIV_REFERER)
        .send()
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Pixiv answered {status}"));
    }
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    if !content_type.starts_with("image/") {
        return Err(format!("Pixiv's answer is {content_type}, not a picture"));
    }
    let mut bytes = Vec::new();
    response
        .take(LARGEST + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > LARGEST {
        return Err("larger than 15 MB".to_owned());
    }
    Ok(SitePicture {
        content_type,
        bytes,
    })
}

/// A Pixiv picture at the size previews take (`c/540x540_70`, its proportions kept), for a page's original
/// (`img-original/img/…/<id>_p<n>.<ext>`) or an animation's zip of frames (`img-zip-ugoira/img/…/<id>_ugoira….zip`),
/// as Pixiv names its smaller copies: `img-master/img/…/<id>_p<n>_master1200.jpg`, or `<id>_master1200.jpg` for an
/// animation. `None` for any other address.
pub fn pixiv_preview(url: &str) -> Option<String> {
    const SERVER: &str = "https://i.pximg.net/";
    const SMALLER: &str = "https://i.pximg.net/c/540x540_70/img-master/img/";
    let rest = url.strip_prefix(SERVER)?;
    if let Some(path) = rest.strip_prefix("img-original/img/") {
        let (stem, _extension) = path.rsplit_once('.')?;
        return Some(format!("{SMALLER}{stem}_master1200.jpg"));
    }
    let path = rest.strip_prefix("img-zip-ugoira/img/")?;
    let (stem, _) = path.rsplit_once("_ugoira")?;
    Some(format!("{SMALLER}{stem}_master1200.jpg"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asks_pixiv_for_its_smaller_copies() {
        assert_eq!(
            pixiv_preview(
                "https://i.pximg.net/img-original/img/2016/04/15/11/53/44/56360615_p2.png"
            )
            .as_deref(),
            Some(
                "https://i.pximg.net/c/540x540_70/img-master/img/2016/04/15/11/53/44/56360615_p2_master1200.jpg"
            )
        );
        assert_eq!(
            pixiv_preview(
                "https://i.pximg.net/img-zip-ugoira/img/2018/01/15/13/24/48/66806629_ugoira1920x1080.zip"
            )
            .as_deref(),
            Some("https://i.pximg.net/c/540x540_70/img-master/img/2018/01/15/13/24/48/66806629_master1200.jpg")
        );
        assert_eq!(pixiv_preview("https://pbs.twimg.com/media/x.jpg"), None);
    }

    #[test]
    fn fetches_only_pixivs_pictures() {
        let named = |address: &str| URL_SAFE_NO_PAD.encode(address);
        assert!(fetch(&named("https://example.com/a.jpg")).is_err());
        assert!(
            fetch(&named("http://i.pximg.net/a.jpg")).is_err(),
            "only https"
        );
        assert!(fetch(&named("https://i.pximg.net.example.com/a.jpg")).is_err());
        assert!(fetch("not base64 !").is_err());
    }
}
