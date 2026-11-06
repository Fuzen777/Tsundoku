//! The keys, names and MACs here were made with async-mega-py, an independent implementation of MEGA's format, so
//! these tests check Tsundoku against it rather than against itself.

use sha2::{Digest, Sha256};

use super::*;

/// A 1,300,123-byte file, spanning five MAC chunks, with its key.
const FILE_KEY: &str = "mj-l3w2rEIdP8F-BjmoocDwuN08B9HWCWW3W9qxYqcs";
const FILE_SIZE: usize = 1_300_123;
/// The SHA-256 of the file encrypted, and its first 32 bytes.
const CIPHER_SHA256: &str = "a02a902e3459188c2cf1b6487b5d34df383d53bf27f4327f34c04eb4b193c3ae";
const CIPHER_HEAD: &str = "0bd9cfcb53388a7081dc0aa3b2e216225bebb8302b3171fe7b5c604bf03761c3";
/// The file's attributes, {"n": "Test file.txt"}.
const FILE_ATTRIBUTES: &str = "1jwzXoHeRVXb2nVvJMKZTJRLSc0ssT5PiVHAjJpbFvs";
/// A file of exactly 128 KiB, one MAC chunk.
const FILE_KEY_128K: &str = "HgHWmpERoCBpbx5ZlcQdRBz5Xws3Dt8Zbw9i_XJ_luk";

/// A shared folder "Shared" holding a folder "Sub: one", which holds the file above, each key encrypted with the
/// folder's.
const FOLDER_KEY: &str = "nhaj9NmFYHVTN8r8Z-VS3g";
const ROOT_KEY: &str = "g9FSSQ7y2QFTP-T6E8LxWQ";
const ROOT_ATTRIBUTES: &str = "0gi2culfO6AM4MXNuIJTMEHuJj2KFLz9LVacgQl6IK0";
const SUB_KEY: &str = "N9wNJ_H2IoiBrXu0DbOV1w";
const SUB_ATTRIBUTES: &str = "A2p-QB8q7P179m018VgO0Fk9eLfr91f4C8IOJCFzsd4";
const NODE_FILE_KEY: &str = "ziAE50ykZpR0Jtf-hrAhykafEJp_rFcp2ckiLc9W_Zk";

fn plain(size: usize, step: usize, add: usize) -> Vec<u8> {
    (0..size)
        .map(|index| ((index * step + add) % 256) as u8)
        .collect()
}

fn file_key(text: &str) -> FileKey {
    FileKey(decode(text).unwrap().try_into().unwrap())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn reads_todays_links_and_the_old_ones() {
    let key = file_key(FILE_KEY);
    for link in [
        format!("https://mega.nz/file/AbCdEf12#{FILE_KEY}"),
        format!("https://mega.nz/embed/AbCdEf12#{FILE_KEY}"),
        format!("https://mega.nz/#!AbCdEf12!{FILE_KEY}"),
        format!("https://mega.co.nz/#!AbCdEf12!{FILE_KEY}="),
    ] {
        assert_eq!(
            parse_link(&link),
            Ok(MegaLink::File {
                handle: "AbCdEf12".to_owned(),
                key: key.clone()
            }),
            "{link}"
        );
    }

    let folder_key: [u8; 16] = decode(FOLDER_KEY).unwrap().try_into().unwrap();
    for (link, within) in [
        (
            format!("https://mega.nz/folder/Fold3r12#{FOLDER_KEY}"),
            None,
        ),
        (
            format!("https://mega.nz/folder/Fold3r12#{FOLDER_KEY}/file/N0de1234"),
            Some("N0de1234"),
        ),
        (
            format!("https://mega.nz/folder/Fold3r12#{FOLDER_KEY}/folder/Sub12345"),
            Some("Sub12345"),
        ),
        (format!("https://mega.nz/#F!Fold3r12!{FOLDER_KEY}"), None),
        (
            format!("https://mega.nz/#F!Fold3r12!{FOLDER_KEY}!Sub12345"),
            Some("Sub12345"),
        ),
    ] {
        assert_eq!(
            parse_link(&link),
            Ok(MegaLink::Folder {
                handle: "Fold3r12".to_owned(),
                key: folder_key,
                within: within.map(str::to_owned),
            }),
            "{link}"
        );
    }
}

#[test]
fn says_when_a_link_is_missing_its_key_or_a_password() {
    let incomplete = Err(PreviewError::LinkIncomplete {
        site: "MEGA".to_owned(),
    });
    for link in [
        "https://mega.nz/file/AbCdEf12",
        "https://mega.nz/file/AbCdEf12#",
        "https://mega.nz/file/AbCdEf12#tooShort",
        "https://mega.nz/folder/Fold3r12",
        "https://mega.nz/#!AbCdEf12",
    ] {
        assert_eq!(parse_link(link), incomplete, "{link}");
    }
    assert_eq!(
        parse_link("https://mega.nz/#P!AgBkzXyZ"),
        Err(PreviewError::PasswordProtected {
            site: "MEGA".to_owned()
        })
    );
    assert_eq!(
        parse_link("https://mega.nz/pro"),
        Err(PreviewError::UnsupportedSite)
    );
}

#[test]
fn decrypts_a_name() {
    let key = file_key(FILE_KEY);
    assert_eq!(
        decrypt_name(FILE_ATTRIBUTES, &key.aes_key()).as_deref(),
        Ok("Test file.txt")
    );
    // Another file's key gives garbage, which means the link's key is wrong.
    assert_eq!(
        decrypt_name(FILE_ATTRIBUTES, &file_key(FILE_KEY_128K).aes_key()),
        Err(PreviewError::LinkIncomplete {
            site: "MEGA".to_owned()
        })
    );
}

#[test]
fn decrypts_a_file_from_any_point() {
    let key = file_key(FILE_KEY);
    let plain = plain(FILE_SIZE, 31, 7);
    // Counter mode encrypts and decrypts alike.
    let mut encrypted = plain.clone();
    key.decryptor(0).apply(&mut encrypted);
    assert_eq!(hex(&encrypted[..32]), CIPHER_HEAD);
    assert_eq!(hex(&Sha256::digest(&encrypted)), CIPHER_SHA256);

    // Resuming partway, even mid-block, gives the same bytes.
    let offset = 300_007;
    let mut rest = encrypted[offset..].to_vec();
    key.decryptor(offset as u64).apply(&mut rest);
    assert_eq!(rest, plain[offset..]);
}

#[test]
fn checks_a_files_mac_however_it_arrives() {
    let key = file_key(FILE_KEY);
    let plain = plain(FILE_SIZE, 31, 7);
    for piece in [1, 16, 1000, 65_536, FILE_SIZE] {
        let mut checker = key.mac_checker();
        for part in plain.chunks(piece) {
            checker.update(part);
        }
        assert!(checker.matches(), "in pieces of {piece}");
    }

    let mut damaged = plain.clone();
    damaged[900_000] ^= 1;
    let mut checker = key.mac_checker();
    checker.update(&damaged);
    assert!(!checker.matches());

    let mut short = key.mac_checker();
    short.update(&plain[..FILE_SIZE - 1]);
    assert!(!short.matches());
}

#[test]
fn checks_a_file_of_exactly_one_chunk() {
    let mut checker = file_key(FILE_KEY_128K).mac_checker();
    checker.update(&plain(128 * 1024, 13, 1));
    assert!(checker.matches());
}

#[test]
fn lists_a_shared_folder_with_its_layout() {
    let folder_key: [u8; 16] = decode(FOLDER_KEY).unwrap().try_into().unwrap();
    let node =
        |handle: &str, parent: &str, kind: u8, attributes: &str, key: &str, size: u64| NodeAnswer {
            handle: handle.to_owned(),
            parent: parent.to_owned(),
            kind,
            attributes: attributes.to_owned(),
            key: format!("{handle}:{key}"),
            size,
        };
    let answer = [
        node("Root1234", "Owner123", 1, ROOT_ATTRIBUTES, ROOT_KEY, 0),
        node("Sub12345", "Root1234", 1, SUB_ATTRIBUTES, SUB_KEY, 0),
        node(
            "File1234",
            "Sub12345",
            0,
            FILE_ATTRIBUTES,
            NODE_FILE_KEY,
            FILE_SIZE as u64,
        ),
    ];
    let nodes = decrypt_nodes(&answer, &folder_key).unwrap();
    assert_eq!(
        nodes
            .iter()
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>(),
        ["Shared", "Sub: one", "Test file.txt"]
    );
    assert_eq!(nodes[2].key, Some(file_key(FILE_KEY)));

    let (title, files) = folder_files(&nodes, "Fold3r12", None).unwrap();
    assert_eq!(title, "Shared");
    assert_eq!(files.len(), 1);
    assert_eq!(
        files[0].path,
        PathBuf::from("Shared")
            .join("Sub_ one")
            .join("Test file.txt")
    );
    assert_eq!(files[0].size, FILE_SIZE as u64);
    assert_eq!(
        files[0].source,
        FileSource::Mega(MegaFile {
            folder: Some("Fold3r12".to_owned()),
            handle: "File1234".to_owned(),
            key: file_key(FILE_KEY),
        })
    );

    let (title, files) = folder_files(&nodes, "Fold3r12", Some("Sub12345")).unwrap();
    assert_eq!(title, "Sub: one");
    assert_eq!(
        files[0].path,
        PathBuf::from("Sub_ one").join("Test file.txt")
    );

    let (title, files) = folder_files(&nodes, "Fold3r12", Some("File1234")).unwrap();
    assert_eq!(title, "Test file.txt");
    assert_eq!(files[0].path, PathBuf::from("Test file.txt"));

    assert!(matches!(
        folder_files(&nodes, "Fold3r12", Some("Missing1")),
        Err(PreviewError::Unavailable { .. })
    ));
}

#[test]
fn sorts_megas_errors() {
    assert!(matches!(error_for(-9), PreviewError::Unavailable { .. }));
    assert!(
        matches!(error_for(-16), PreviewError::Unavailable { detail } if detail.contains("taken it down"))
    );
    assert!(matches!(error_for(-17), PreviewError::LimitReached { site, .. } if site == "MEGA"));
    assert!(matches!(error_for(-3), PreviewError::Network { .. }));
    assert!(matches!(error_for(-999), PreviewError::Failed { .. }));
}

#[test]
fn never_prints_a_key() {
    assert_eq!(format!("{:?}", file_key(FILE_KEY)), "FileKey(…)");
}
