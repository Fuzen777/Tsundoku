//! pCloud: public links to a file or a folder ("u.pcloud.link/publink/show?code=…" for its US data centre,
//! "e.pcloud.link/…" for its European one), read through pCloud's public API without signing in. `showpublink` gives
//! the whole tree at once, with each file's name, size and date; `getpublinkdownload` gives the address a file
//! downloads from, which expires, so it's asked when the download starts. Every request names the link's code, so
//! nothing outside it is reached.

use std::path::PathBuf;

use reqwest::Url;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::{
    Address, FileCheck, FileSource, Host, HostFacts, RemoteFile, Share, check_cancel, http_date,
    network_error, safe_name, status_error,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "pCloud",
    site: "pcloud",
    domains: &["pcloud.link", "pcloud.com"],
    sends_attachments: false,
    read,
    page_error: None,
};

/// The API of each data centre. A link is on one of them; the other answers that its code is invalid.
const US_API: &str = "https://api.pcloud.com";
const EU_API: &str = "https://eapi.pcloud.com";

/// pCloud's answers to a link it doesn't know (on this data centre), one with a password, one the owner deleted or
/// that was taken down, one only for premium accounts.
const INVALID_LINK: i64 = 7001;
const DELETED: i64 = 7002;
const TAKEN_DOWN: i64 = 7003;
const PASSWORD_REQUIRED: i64 = 2258;
const PASSWORD_WRONG: i64 = 1125;
const PREMIUM_ONLY: i64 = 7005;

/// A file to download from pCloud.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PCloudFile {
    /// The public link's code.
    pub code: String,
    /// The API of the link's data centre.
    pub api: &'static str,
    pub file_id: u64,
}

/// Reads a pCloud link: the file's name, size and date, or every file in the folder and its subfolders.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let (code, first) = parse_link(link).ok_or_else(|| Host::PCloud.link_not_recognized())?;
    let (api, answer) = show(client, &code, first, cancel)?;
    let root = answer.metadata;
    let mut files = Vec::new();
    collect(&root, PathBuf::new(), &code, api, &mut files);
    files.sort_by(|first, second| first.path.cmp(&second.path));
    Ok(Share {
        host: Host::PCloud,
        id: code,
        url: link.to_owned(),
        upload_date: root.created.as_deref().and_then(http_date),
        title: root.name,
        files,
    })
}

/// Asks pCloud where a file downloads from now.
pub(super) fn address(
    client: &Client,
    file: &PCloudFile,
    cancel: &CancelToken,
) -> Result<Address, PreviewError> {
    let file_id = file.file_id.to_string();
    let answer: DownloadAnswer = api(
        client,
        file.api,
        "getpublinkdownload",
        &[
            ("code", &file.code),
            ("fileid", &file_id),
            ("forcedownload", "1"),
        ],
        cancel,
    )?;
    let host = answer
        .hosts
        .first()
        .filter(|host| host.ends_with(".pcloud.com"))
        .ok_or_else(|| PreviewError::Failed {
            detail: "pCloud gave no server to download from".to_owned(),
        })?;
    Ok(Address {
        url: format!("https://{host}{}", answer.path),
        check: FileCheck::None,
    })
}

#[derive(Deserialize)]
struct ShowAnswer {
    metadata: Item,
}

#[derive(Debug, Deserialize)]
struct Item {
    name: String,
    #[serde(default)]
    isfolder: bool,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    fileid: Option<u64>,
    /// "Sat, 07 Sep 2024 14:08:27 +0000".
    #[serde(default)]
    created: Option<String>,
    #[serde(default)]
    contents: Vec<Item>,
}

#[derive(Deserialize)]
struct DownloadAnswer {
    path: String,
    #[serde(default)]
    hosts: Vec<String>,
}

/// The link's tree, from its data centre's API, or from the other one when the first doesn't know the code (a link
/// whose address doesn't say which it's on).
fn show(
    client: &Client,
    code: &str,
    first: &'static str,
    cancel: &CancelToken,
) -> Result<(&'static str, ShowAnswer), PreviewError> {
    match api(client, first, "showpublink", &[("code", code)], cancel) {
        Err(PreviewError::Unavailable { detail })
            if detail.starts_with(&format!("pCloud error {INVALID_LINK}:")) =>
        {
            let other = if first == US_API { EU_API } else { US_API };
            let answer = api(client, other, "showpublink", &[("code", code)], cancel)?;
            Ok((other, answer))
        }
        answer => answer.map(|answer| (first, answer)),
    }
}

/// Every file under `item`, its path inside the download's folder starting with the shared folder's own name.
fn collect(
    item: &Item,
    folder: PathBuf,
    code: &str,
    api: &'static str,
    files: &mut Vec<RemoteFile>,
) {
    let path = folder.join(safe_name(&item.name));
    if item.isfolder {
        for child in &item.contents {
            collect(child, path.clone(), code, api, files);
        }
    } else if let Some(file_id) = item.fileid {
        files.push(RemoteFile {
            path,
            size: item.size,
            source: FileSource::PCloud(PCloudFile {
                code: code.to_owned(),
                api,
                file_id,
            }),
        });
    }
}

/// Calls pCloud's API and returns its answer, or its error.
fn api<T: DeserializeOwned>(
    client: &Client,
    api: &str,
    method: &str,
    parameters: &[(&str, &str)],
    cancel: &CancelToken,
) -> Result<T, PreviewError> {
    check_cancel(cancel)?;
    let url = Url::parse_with_params(&format!("{api}/{method}"), parameters).map_err(|error| {
        PreviewError::Failed {
            detail: error.to_string(),
        }
    })?;
    let response = client.get(url).send().map_err(network_error)?;
    let status = status_error(Host::PCloud, &response);
    let body: Value = match response.json() {
        Ok(body) => body,
        Err(error) => {
            return Err(status.unwrap_or_else(|| PreviewError::Failed {
                detail: format!("pCloud's answer wasn't as expected: {error}"),
            }));
        }
    };
    let result = body.get("result").and_then(Value::as_i64).unwrap_or(-1);
    if result != 0 {
        return Err(api_error(
            result,
            body.get("error")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        ));
    }
    serde_json::from_value(body).map_err(|error| PreviewError::Failed {
        detail: format!("pCloud's answer wasn't as expected: {error}"),
    })
}

/// What pCloud's error codes mean, sorted as a preview's errors are.
fn api_error(result: i64, message: &str) -> PreviewError {
    let detail = format!("pCloud error {result}: {message}");
    let site = || Host::PCloud.name().to_owned();
    match result {
        INVALID_LINK | DELETED | TAKEN_DOWN => PreviewError::Unavailable { detail },
        PASSWORD_REQUIRED | PASSWORD_WRONG => PreviewError::PasswordProtected { site: site() },
        PREMIUM_ONLY => PreviewError::Unavailable {
            detail: format!("{detail} (for premium accounts only)"),
        },
        _ if message.to_ascii_lowercase().contains("traffic") => PreviewError::LimitReached {
            site: site(),
            detail,
        },
        _ => PreviewError::Failed { detail },
    }
}

/// Reads a pCloud link into its code and the API of its data centre: "…pcloud.link/publink/show?code=<code>" (or
/// on pcloud.com, or "#page=publink&code=<code>"), the European one when the host starts with "e" ("e.pcloud.link",
/// "e1.pcloud.link").
fn parse_link(link: &str) -> Option<(String, &'static str)> {
    let url = Url::parse(link).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let in_query = url
        .query_pairs()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value.into_owned());
    let in_fragment = url.fragment().and_then(|fragment| {
        fragment
            .split('&')
            .find_map(|pair| pair.strip_prefix("code="))
            .map(str::to_owned)
    });
    let code = in_query.or(in_fragment).filter(|code| {
        !code.is_empty()
            && code
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
    })?;
    let first_label = host.split('.').next().unwrap_or_default();
    let european = first_label
        .strip_prefix('e')
        .is_some_and(|rest| rest.chars().all(|character| character.is_ascii_digit()));
    Some((code, if european { EU_API } else { US_API }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_its_links() {
        for (link, expected) in [
            (
                "https://u.pcloud.link/publink/show?code=XZdjuv0ZtabS8BN58thUiE8FGjznajoMc6Qy",
                Some(("XZdjuv0ZtabS8BN58thUiE8FGjznajoMc6Qy", US_API)),
            ),
            (
                "https://e.pcloud.link/publink/show?code=XZAAk0Z8P6jmN12mTQic0hDRPqhhu6yQg0y",
                Some(("XZAAk0Z8P6jmN12mTQic0hDRPqhhu6yQg0y", EU_API)),
            ),
            (
                "https://e1.pcloud.link/publink/show?code=kZabc",
                Some(("kZabc", EU_API)),
            ),
            (
                "https://my.pcloud.com/#page=publink&code=kZabc",
                Some(("kZabc", US_API)),
            ),
            ("https://www.pcloud.com/pricing.html", None),
            ("https://u.pcloud.link/publink/show?code=", None),
            ("https://u.pcloud.link/publink/show?code=a%20b", None),
        ] {
            assert_eq!(
                parse_link(link),
                expected.map(|(code, api)| (code.to_owned(), api)),
                "{link}"
            );
        }
        assert_eq!(
            Host::of("https://e.pcloud.link/publink/show?code=x"),
            Some(Host::PCloud)
        );
    }

    #[test]
    fn reads_a_folder_with_its_subfolders() {
        // pCloud's answer for a folder, trimmed, with a subfolder added.
        let answer: ShowAnswer = serde_json::from_str(
            r#"{"result": 0, "metadata": {"name": "debs", "isfolder": true, "folderid": 23028052587,
            "created": "Sat, 07 Sep 2024 14:08:27 +0000", "contents": [
                {"name": "linux-dtb-next-sun50iw9_1.0.2_arm64.deb", "isfolder": false, "size": 15596,
                 "fileid": 65653956472, "created": "Sat, 07 Sep 2024 14:04:07 +0000"},
                {"name": "old", "isfolder": true, "folderid": 5, "contents": [
                    {"name": "a:b.deb", "isfolder": false, "size": 3, "fileid": 7}]}]}}"#,
        )
        .unwrap();
        let mut files = Vec::new();
        collect(&answer.metadata, PathBuf::new(), "kZB", US_API, &mut files);
        let paths: Vec<(PathBuf, u64)> = files
            .iter()
            .map(|file| (file.path.clone(), file.size))
            .collect();
        assert_eq!(
            paths,
            [
                (
                    PathBuf::from("debs").join("linux-dtb-next-sun50iw9_1.0.2_arm64.deb"),
                    15596
                ),
                (PathBuf::from("debs").join("old").join("a_b.deb"), 3),
            ]
        );
        assert_eq!(
            files[1].source,
            FileSource::PCloud(PCloudFile {
                code: "kZB".to_owned(),
                api: US_API,
                file_id: 7
            })
        );
        assert_eq!(
            answer
                .metadata
                .created
                .as_deref()
                .and_then(http_date)
                .as_deref(),
            Some("2024-09-07")
        );
    }

    #[test]
    fn a_single_file_is_its_own_name() {
        let answer: ShowAnswer = serde_json::from_str(
            r#"{"result": 0, "metadata": {"name": "autoblooger_api_faq.html", "isfolder": false, "size": 9943,
            "fileid": 66825795794, "created": "Fri, 11 Oct 2024 19:58:17 +0000"}}"#,
        )
        .unwrap();
        let mut files = Vec::new();
        collect(&answer.metadata, PathBuf::new(), "XZd", US_API, &mut files);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, PathBuf::from("autoblooger_api_faq.html"));
        let download: DownloadAnswer = serde_json::from_str(
            r#"{"result": 0, "path": "\/D4Zn\/autoblooger_api_faq.html", "hosts": ["p-lux2.pcloud.com", "def1.pcloud.com"]}"#,
        )
        .unwrap();
        assert_eq!(download.path, "/D4Zn/autoblooger_api_faq.html");
        assert_eq!(download.hosts[0], "p-lux2.pcloud.com");
    }

    #[test]
    fn explains_its_errors() {
        assert!(matches!(
            api_error(7002, "This link is deleted by the owner."),
            PreviewError::Unavailable { detail } if detail.contains("deleted")
        ));
        assert!(matches!(
            api_error(7001, "Invalid link 'code'."),
            PreviewError::Unavailable { .. }
        ));
        assert_eq!(
            api_error(2258, "Password required."),
            PreviewError::PasswordProtected {
                site: "pCloud".to_owned()
            }
        );
        assert!(matches!(
            api_error(7010, "This link has exceeded its traffic limit."),
            PreviewError::LimitReached { site, .. } if site == "pCloud"
        ));
        assert!(matches!(
            api_error(5000, "Internal error."),
            PreviewError::Failed { .. }
        ));
    }
}
