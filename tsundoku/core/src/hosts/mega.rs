//! MEGA: links to a file or a shared folder, read through MEGA's JSON API (`g.api.mega.co.nz/cs`). Files and their
//! names are encrypted. The key is the part of the link after "#", which never leaves the PC: names are decrypted
//! here, and a file's content as it's saved (AES-128 in counter mode), then checked against the MAC in its key.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aes::Aes128;
use aes::cipher::{
    BlockCipherDecrypt, BlockCipherEncrypt, KeyInit, KeyIvInit, StreamCipher, StreamCipherSeek,
};
use base64::Engine;
use base64::alphabet;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Address, FileCheck, FileSource, Host, HostFacts, RemoteFile, SendChecked, Share, check_cancel,
    date_of, safe_name, status_error,
};
use crate::cancel::CancelToken;
use crate::metadata::PreviewError;

pub(super) const FACTS: HostFacts = HostFacts {
    name: "MEGA",
    site: "mega",
    domains: &["mega.nz", "mega.co.nz"],
    sends_attachments: false,
    browser_agent: false,
    read,
    page_error: None,
    window_link: None,
    download_file_id: None,
};

const API: &str = "https://g.api.mega.co.nz/cs";
/// How many times a request MEGA answers "busy" (-3) is tried again, waiting twice as long each time.
const RETRIES: u32 = 4;
/// MEGA's base64: the URL-safe alphabet without padding. Old links may still carry some.
const BASE64: GeneralPurpose = GeneralPurpose::new(
    &alphabet::URL_SAFE,
    GeneralPurposeConfig::new()
        .with_encode_padding(false)
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true),
);
/// MAC chunks start at 128 KiB and grow by 128 KiB up to 1 MiB, then stay at 1 MiB.
const MAC_CHUNK_STEP: u64 = 128 * 1024;
const MAC_CHUNK_MAX: u64 = 1024 * 1024;

type Ctr = ctr::Ctr128BE<Aes128>;

/// A file's key: 32 bytes, from its link or, in a folder, from the folder's key. Never printed.
#[derive(Clone, PartialEq, Eq)]
pub struct FileKey([u8; 32]);

impl fmt::Debug for FileKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("FileKey(…)")
    }
}

impl FileKey {
    /// The AES key: the two halves of the key XOR-ed together.
    fn aes_key(&self) -> [u8; 16] {
        std::array::from_fn(|index| self.0[index] ^ self.0[index + 16])
    }

    /// The counter's first half, also the start of every MAC chunk.
    fn nonce(&self) -> [u8; 8] {
        std::array::from_fn(|index| self.0[16 + index])
    }

    /// The MAC the whole file must have, condensed to 8 bytes.
    fn meta_mac(&self) -> [u8; 8] {
        std::array::from_fn(|index| self.0[24 + index])
    }

    /// Decrypts a file's content from `offset` on, so a download can resume.
    pub fn decryptor(&self, offset: u64) -> Decryptor {
        let mut iv = [0; 16];
        iv[..8].copy_from_slice(&self.nonce());
        let mut cipher = Ctr::new(&self.aes_key().into(), &iv.into());
        cipher.seek(offset);
        Decryptor(cipher)
    }

    /// Checks a decrypted file, fed in order from its first byte.
    pub fn mac_checker(&self) -> MacChecker {
        let nonce = self.nonce();
        let start: [u8; 16] = std::array::from_fn(|index| nonce[index % 8]);
        MacChecker {
            cipher: Aes128::new(&self.aes_key().into()),
            expected: self.meta_mac(),
            start,
            chunk_mac: start,
            file_mac: [0; 16],
            chunk_start: 0,
            chunk_length: MAC_CHUNK_STEP,
            position: 0,
            block: [0; 16],
            block_length: 0,
        }
    }
}

pub struct Decryptor(Ctr);

impl Decryptor {
    /// Decrypts `data` in place: the next bytes of the file.
    pub fn apply(&mut self, data: &mut [u8]) {
        self.0.apply_keystream(data);
    }
}

/// MEGA's file MAC: a CBC-MAC of each chunk, those MACs chained into one, folded to 8 bytes.
pub struct MacChecker {
    cipher: Aes128,
    expected: [u8; 8],
    /// What each chunk's MAC starts from: the nonce twice.
    start: [u8; 16],
    chunk_mac: [u8; 16],
    file_mac: [u8; 16],
    chunk_start: u64,
    chunk_length: u64,
    position: u64,
    /// The part of a 16-byte block not yet added, at the very end of the file.
    block: [u8; 16],
    block_length: usize,
}

impl MacChecker {
    /// Adds the next bytes of the decrypted file.
    pub fn update(&mut self, mut data: &[u8]) {
        while !data.is_empty() {
            let left_in_chunk = self.chunk_start + self.chunk_length - self.position;
            let count = data
                .len()
                .min(usize::try_from(left_in_chunk).unwrap_or(usize::MAX));
            let (now, rest) = data.split_at(count);
            for &byte in now {
                self.block[self.block_length] = byte;
                self.block_length += 1;
                if self.block_length == 16 {
                    self.add_block();
                }
            }
            self.position += count as u64;
            data = rest;
            if self.position == self.chunk_start + self.chunk_length {
                self.end_chunk();
            }
        }
    }

    /// Whether the file's MAC is the one its key carries. An empty file has nothing to check.
    pub fn matches(mut self) -> bool {
        if self.position == 0 {
            return true;
        }
        if self.position > self.chunk_start {
            self.end_chunk();
        }
        let mac = self.file_mac;
        let condensed: [u8; 8] = std::array::from_fn(|index| {
            let word = index / 4;
            mac[word * 8 + index % 4] ^ mac[word * 8 + 4 + index % 4]
        });
        condensed == self.expected
    }

    fn add_block(&mut self) {
        let mut block = self.chunk_mac;
        for (byte, data) in block.iter_mut().zip(self.block) {
            *byte ^= data;
        }
        self.chunk_mac = self.encrypt(block);
        self.block_length = 0;
    }

    fn end_chunk(&mut self) {
        if self.block_length > 0 {
            self.block[self.block_length..].fill(0);
            self.add_block();
        }
        let mut block = self.file_mac;
        for (byte, chunk) in block.iter_mut().zip(self.chunk_mac) {
            *byte ^= chunk;
        }
        self.file_mac = self.encrypt(block);
        self.chunk_mac = self.start;
        self.chunk_start = self.position;
        if self.chunk_length < MAC_CHUNK_MAX {
            self.chunk_length += MAC_CHUNK_STEP;
        }
    }

    fn encrypt(&self, block: [u8; 16]) -> [u8; 16] {
        let mut block = block.into();
        self.cipher.encrypt_block(&mut block);
        block.into()
    }
}

/// A file to download from MEGA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MegaFile {
    /// The shared folder's handle for a file inside one, which every request about it names.
    pub folder: Option<String>,
    /// The file's own handle: its public one for a file link, its node in a folder.
    pub handle: String,
    pub key: FileKey,
}

/// What a MEGA link points to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum MegaLink {
    File {
        handle: String,
        key: FileKey,
    },
    Folder {
        handle: String,
        key: [u8; 16],
        /// A file or subfolder inside it, for a link that points to one ("…/folder/<handle>#<key>/file/<node>").
        within: Option<String>,
    },
}

/// Reads a MEGA link: the file's name and size, or every file in the folder with its place in it.
pub(super) fn read(
    client: &Client,
    link: &str,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    match parse_link(link)? {
        MegaLink::File { handle, key } => {
            let answer: FileAnswer = api(client, None, json!({"a": "g", "p": handle}), cancel)?;
            let name = decrypt_name(&answer.attributes, &key.aes_key())?;
            Ok(Share {
                host: Host::Mega,
                id: handle.clone(),
                url: link.to_owned(),
                title: name.clone(),
                upload_date: None,
                files: vec![RemoteFile {
                    path: PathBuf::from(safe_name(&name)),
                    size: answer.size,
                    source: FileSource::Mega(MegaFile {
                        folder: None,
                        handle,
                        key,
                    }),
                }],
            })
        }
        MegaLink::Folder {
            handle,
            key,
            within,
        } => {
            let answer: FolderAnswer = api(
                client,
                Some(&handle),
                json!({"a": "f", "c": 1, "ca": 1, "r": 1}),
                cancel,
            )?;
            let root = share_root(&answer.nodes).ok_or_else(|| PreviewError::Unavailable {
                detail: "MEGA listed no shared folder".to_owned(),
            })?;
            let nodes = decrypt_nodes(&answer.nodes, &root.handle, &key);
            let (top, files) = folder_files(&nodes, &handle, &root.handle, within.as_deref())?;
            let (title, upload_date) = (top.name.clone(), top.timestamp.and_then(date_of));
            let id = match &within {
                Some(node) => format!("{handle}/{node}"),
                None => handle,
            };
            Ok(Share {
                host: Host::Mega,
                id,
                url: link.to_owned(),
                title,
                upload_date,
                files,
            })
        }
    }
}

/// Asks MEGA where a file downloads from now.
pub(super) fn address(
    client: &Client,
    file: &MegaFile,
    cancel: &CancelToken,
) -> Result<Address, PreviewError> {
    let command = match file.folder {
        None => json!({"a": "g", "g": 1, "ssl": 2, "p": file.handle}),
        Some(_) => json!({"a": "g", "g": 1, "ssl": 2, "n": file.handle}),
    };
    let answer: AddressAnswer = api(client, file.folder.as_deref(), command, cancel)?;
    if let Some(code) = answer.error {
        return Err(error_for(code));
    }
    match answer.url {
        Some(url) if !url.is_empty() => Ok(Address {
            url,
            check: FileCheck::Mega(file.key.clone()),
        }),
        // MEGA leaves the address out while this connection's transfer quota is used up.
        _ => Err(PreviewError::LimitReached {
            site: Host::Mega.name().to_owned(),
            detail: "MEGA gave no download address".to_owned(),
        }),
    }
}

#[derive(Deserialize)]
struct FileAnswer {
    #[serde(rename = "s")]
    size: u64,
    #[serde(rename = "at")]
    attributes: String,
}

#[derive(Deserialize)]
struct AddressAnswer {
    #[serde(rename = "g")]
    url: Option<String>,
    #[serde(rename = "e")]
    error: Option<i64>,
}

#[derive(Deserialize)]
struct FolderAnswer {
    #[serde(rename = "f")]
    nodes: Vec<NodeAnswer>,
}

/// A file or folder in a shared folder, as MEGA lists it.
#[derive(Deserialize)]
struct NodeAnswer {
    #[serde(rename = "h")]
    handle: String,
    #[serde(rename = "p", default)]
    parent: String,
    /// 0 for a file, 1 for a folder.
    #[serde(rename = "t")]
    kind: u8,
    #[serde(rename = "a", default)]
    attributes: String,
    /// "<handle>:<key encrypted with that folder's key>", sometimes several separated by "/".
    #[serde(rename = "k", default)]
    key: String,
    #[serde(rename = "s", default)]
    size: u64,
    /// When it was added, in seconds since 1970.
    #[serde(rename = "ts", default)]
    timestamp: Option<i64>,
}

/// A decrypted node.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Node {
    handle: String,
    parent: String,
    name: String,
    size: u64,
    /// Set for a file.
    key: Option<FileKey>,
    timestamp: Option<i64>,
}

/// Sends one command to MEGA's API and returns its answer, trying again while MEGA says it's busy.
fn api<T: for<'de> Deserialize<'de>>(
    client: &Client,
    folder: Option<&str>,
    command: Value,
    cancel: &CancelToken,
) -> Result<T, PreviewError> {
    let mut wait = Duration::from_secs(1);
    for attempt in 0..=RETRIES {
        check_cancel(cancel)?;
        let mut url = format!("{API}?id={}", next_request_id());
        if let Some(folder) = folder {
            url.push_str("&n=");
            url.push_str(folder);
        }
        let response = client
            .post(&url)
            .json(&[&command])
            .send_checked(FACTS.name)?;
        if response.status().as_u16() == 402 && response.headers().contains_key("x-hashcash") {
            return Err(PreviewError::Failed {
                detail: "MEGA asked for a proof of work, which Tsundoku doesn't solve yet"
                    .to_owned(),
            });
        }
        let busy = match status_error(Host::Mega, &response) {
            Some(PreviewError::Network { .. }) if attempt < RETRIES => true,
            Some(error) => return Err(error),
            None => {
                let answer: Value = response.json().map_err(|error| PreviewError::Failed {
                    detail: format!("MEGA's answer couldn't be read: {error}"),
                })?;
                // One answer per command, or a single error code for the whole request.
                let answer = match answer {
                    Value::Array(mut answers) if !answers.is_empty() => answers.swap_remove(0),
                    other => other,
                };
                match answer.as_i64() {
                    Some(-3) if attempt < RETRIES => true,
                    Some(code) => return Err(error_for(code)),
                    None => {
                        return serde_json::from_value(answer).map_err(|error| {
                            PreviewError::Failed {
                                detail: format!("MEGA's answer wasn't as expected: {error}"),
                            }
                        });
                    }
                }
            }
        };
        if busy {
            sleep(wait, cancel)?;
            wait *= 2;
        }
    }
    Err(PreviewError::Network {
        detail: "MEGA stayed busy".to_owned(),
    })
}

/// MEGA's error codes, sorted as a preview's errors are.
fn error_for(code: i64) -> PreviewError {
    let site = Host::Mega.name().to_owned();
    match code {
        -2 | -9 => PreviewError::Unavailable {
            detail: format!("MEGA has no such file or folder (error {code})"),
        },
        -11 => PreviewError::Unavailable {
            detail: format!("MEGA refused access (error {code})"),
        },
        -16 => PreviewError::Unavailable {
            detail: format!("MEGA has taken it down (error {code})"),
        },
        -4 | -6 | -17 => PreviewError::LimitReached {
            site,
            detail: format!("MEGA error {code}"),
        },
        -3 | -18 => PreviewError::Network {
            detail: format!("MEGA is busy (error {code})"),
        },
        _ => PreviewError::Failed {
            detail: format!("MEGA error {code}"),
        },
    }
}

/// Waits, waking now and then to stop if cancelled.
fn sleep(duration: Duration, cancel: &CancelToken) -> Result<(), PreviewError> {
    let step = Duration::from_millis(100);
    let mut waited = Duration::ZERO;
    while waited < duration {
        check_cancel(cancel)?;
        thread::sleep(step);
        waited += step;
    }
    Ok(())
}

/// MEGA wants a different number on each request.
fn next_request_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |elapsed| elapsed.as_secs());
    let _ = NEXT.compare_exchange(0, start, Ordering::SeqCst, Ordering::SeqCst);
    NEXT.fetch_add(1, Ordering::SeqCst)
}

/// Reads a MEGA link, in today's form ("mega.nz/file/<handle>#<key>", "mega.nz/folder/<handle>#<key>", optionally
/// followed by "/file/<node>" or "/folder/<node>") or the old one ("mega.nz/#!<handle>!<key>",
/// "mega.nz/#F!<handle>!<key>").
fn parse_link(link: &str) -> Result<MegaLink, PreviewError> {
    let incomplete = || PreviewError::LinkIncomplete {
        site: Host::Mega.name().to_owned(),
    };
    let after_scheme = link.split_once("://").map_or(link, |(_, rest)| rest);
    let (address, fragment) = after_scheme.split_once('#').unwrap_or((after_scheme, ""));
    let path = address.split_once('/').map_or("", |(_, path)| path);
    let path = path.split(['?']).next().unwrap_or_default();
    let segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    if fragment.starts_with("P!") {
        return Err(PreviewError::PasswordProtected {
            site: Host::Mega.name().to_owned(),
        });
    }
    // The old form keeps everything after "#".
    if segments.is_empty() {
        let parts: Vec<&str> = fragment.split('!').collect();
        return match parts.as_slice() {
            ["", handle, key] => file_link(handle, key),
            ["F", handle, key, rest @ ..] if rest.len() <= 1 => {
                folder_link(handle, key, rest.first().map(|node| (*node).to_owned()))
            }
            ["", _] | ["F", _] => Err(incomplete()),
            _ => Err(Host::Mega.link_not_recognized()),
        };
    }
    match segments.as_slice() {
        ["file" | "embed", handle, ..] => {
            if fragment.is_empty() {
                Err(incomplete())
            } else {
                file_link(handle, fragment)
            }
        }
        ["folder", handle, ..] => {
            let mut parts = fragment.split('/');
            let key = parts.next().unwrap_or_default();
            if key.is_empty() {
                return Err(incomplete());
            }
            let within = match (parts.next(), parts.next()) {
                (Some("file" | "folder"), Some(node)) if !node.is_empty() => Some(node.to_owned()),
                _ => None,
            };
            folder_link(handle, key, within)
        }
        _ => Err(Host::Mega.link_not_recognized()),
    }
}

fn file_link(handle: &str, key: &str) -> Result<MegaLink, PreviewError> {
    let key: [u8; 32] = decode(key)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| PreviewError::LinkIncomplete {
            site: Host::Mega.name().to_owned(),
        })?;
    Ok(MegaLink::File {
        handle: handle.to_owned(),
        key: FileKey(key),
    })
}

fn folder_link(handle: &str, key: &str, within: Option<String>) -> Result<MegaLink, PreviewError> {
    let key: [u8; 16] = decode(key)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| PreviewError::LinkIncomplete {
            site: Host::Mega.name().to_owned(),
        })?;
    Ok(MegaLink::Folder {
        handle: handle.to_owned(),
        key,
        within,
    })
}

fn decode(text: &str) -> Option<Vec<u8>> {
    let text = text.replace('+', "-").replace('/', "_");
    BASE64.decode(text.trim_end_matches('=')).ok()
}

/// A file's or folder's name, from its attributes: "MEGA" and a JSON object, encrypted in CBC mode without an IV.
fn decrypt_name(attributes: &str, key: &[u8; 16]) -> Result<String, PreviewError> {
    // Garbage instead of "MEGA{" means the link's key isn't this file's.
    let wrong_key = || PreviewError::LinkIncomplete {
        site: Host::Mega.name().to_owned(),
    };
    let encrypted = decode(attributes).ok_or_else(wrong_key)?;
    if encrypted.is_empty() || encrypted.len() % 16 != 0 {
        return Err(wrong_key());
    }
    let cipher = Aes128::new(&(*key).into());
    let mut previous = [0; 16];
    let mut plain = Vec::with_capacity(encrypted.len());
    for block in encrypted.as_chunks::<16>().0 {
        let mut decrypted = (*block).into();
        cipher.decrypt_block(&mut decrypted);
        let decrypted: [u8; 16] = decrypted.into();
        plain.extend(
            decrypted
                .iter()
                .zip(previous)
                .map(|(byte, previous)| byte ^ previous),
        );
        previous = *block;
    }
    let text = String::from_utf8_lossy(&plain);
    let json = text
        .strip_prefix("MEGA")
        .ok_or_else(wrong_key)?
        .trim_end_matches('\0');
    #[derive(Deserialize)]
    struct Attributes {
        n: String,
    }
    serde_json::from_str::<Attributes>(json)
        .map(|attributes| attributes.n)
        .map_err(|_| wrong_key())
}

/// The shared folder's own node: the folder whose parent isn't in the listing.
fn share_root(nodes: &[NodeAnswer]) -> Option<&NodeAnswer> {
    let handles: HashSet<&str> = nodes.iter().map(|node| node.handle.as_str()).collect();
    nodes
        .iter()
        .find(|node| node.kind == 1 && !handles.contains(node.parent.as_str()))
}

/// Decrypts a shared folder's nodes with the folder's key: each node's key, then its name. A node may list its key
/// several times, each encrypted for another folder ("<handle>:<key>/<handle>:<key>"); the link's key opens the one
/// under the shared folder (`root`), so that one is tried first, then the others. A node that none of them opens is
/// left out, with whatever is inside it, rather than failing the whole folder.
fn decrypt_nodes(nodes: &[NodeAnswer], root: &str, folder_key: &[u8; 16]) -> Vec<Node> {
    let cipher = Aes128::new(&(*folder_key).into());
    nodes
        .iter()
        .filter(|node| node.kind <= 1)
        .filter_map(|node| decrypt_node(node, root, &cipher))
        .collect()
}

fn decrypt_node(node: &NodeAnswer, root: &str, cipher: &Aes128) -> Option<Node> {
    let mut keys: Vec<(&str, &str)> = node
        .key
        .split('/')
        .filter_map(|part| part.split_once(':'))
        .collect();
    // Stable, so the others keep MEGA's order.
    keys.sort_by_key(|(handle, _)| *handle != root);
    keys.into_iter().find_map(|(_, encrypted)| {
        let encrypted = decode(encrypted)?;
        let mut key = Vec::with_capacity(encrypted.len());
        for block in encrypted.as_chunks::<16>().0 {
            let mut block = (*block).into();
            cipher.decrypt_block(&mut block);
            key.extend_from_slice(&<[u8; 16]>::from(block));
        }
        let (name_key, file_key) = match (node.kind, <[u8; 32]>::try_from(key.as_slice())) {
            (0, Ok(file_key)) => {
                let file_key = FileKey(file_key);
                (file_key.aes_key(), Some(file_key))
            }
            (1, _) => (<[u8; 16]>::try_from(key.get(..16)?).ok()?, None),
            _ => return None,
        };
        Some(Node {
            handle: node.handle.clone(),
            parent: node.parent.clone(),
            name: decrypt_name(&node.attributes, &name_key).ok()?,
            size: node.size,
            key: file_key,
            timestamp: node.timestamp,
        })
    })
}

/// The shared folder (`root_handle`), or with `within` the file or subfolder the link points to, and its files,
/// each with its path inside it, sorted by path. `folder_handle` is the link's, which downloads name.
fn folder_files<'a>(
    nodes: &'a [Node],
    folder_handle: &str,
    root_handle: &str,
    within: Option<&str>,
) -> Result<(&'a Node, Vec<RemoteFile>), PreviewError> {
    let by_handle: HashMap<&str, &Node> = nodes
        .iter()
        .map(|node| (node.handle.as_str(), node))
        .collect();
    let missing = || PreviewError::Unavailable {
        detail: "MEGA's folder doesn't hold what the link points to".to_owned(),
    };
    let root = match within {
        Some(handle) => *by_handle.get(handle).ok_or_else(missing)?,
        // The link's key didn't open the shared folder itself.
        None => *by_handle
            .get(root_handle)
            .ok_or_else(|| PreviewError::LinkIncomplete {
                site: Host::Mega.name().to_owned(),
            })?,
    };
    let to_file = |node: &Node, path: PathBuf| RemoteFile {
        path,
        size: node.size,
        source: FileSource::Mega(MegaFile {
            folder: Some(folder_handle.to_owned()),
            handle: node.handle.clone(),
            key: node.key.clone().unwrap_or(FileKey([0; 32])),
        }),
    };
    if root.key.is_some() {
        return Ok((
            root,
            vec![to_file(root, PathBuf::from(safe_name(&root.name)))],
        ));
    }

    let mut children: HashMap<&str, Vec<&Node>> = HashMap::new();
    for node in nodes {
        children.entry(node.parent.as_str()).or_default().push(node);
    }
    let mut files = Vec::new();
    let mut folders = vec![(root, PathBuf::from(safe_name(&root.name)))];
    while let Some((folder, path)) = folders.pop() {
        for child in children.get(folder.handle.as_str()).into_iter().flatten() {
            let child_path = path.join(safe_name(&child.name));
            if child.key.is_some() {
                files.push(to_file(child, child_path));
            } else {
                folders.push((child, child_path));
            }
        }
    }
    files.sort_by(|first, second| first.path.cmp(&second.path));
    Ok((root, files))
}

#[cfg(test)]
mod tests;
