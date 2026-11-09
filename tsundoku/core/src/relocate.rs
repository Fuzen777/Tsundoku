//! Moving files the history recorded to another folder (History's Move Files…): each download's files keep their
//! places relative to the folder that holds them all, so a shared folder's subfolders come along, and a name already
//! taken gets " (2)". A move to another drive copies, then deletes the original.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use crate::downloads::free_name;

/// What Move Files… did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MovedFiles {
    pub moved: u32,
    /// Files the history names that weren't there anymore, left as they were.
    pub missing: u32,
    /// Files that couldn't be moved, with why.
    pub failed: Vec<FailedMove>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FailedMove {
    pub path: String,
    pub reason: String,
}

/// The deepest folder holding every one of `files`: a video's or a post's own folder, or a shared folder's top.
pub fn common_folder(files: &[PathBuf]) -> Option<PathBuf> {
    let mut folders = files.iter().filter_map(|file| file.parent());
    let first: Vec<Component> = folders.next()?.components().collect();
    let shared = folders.fold(first.len(), |shared, folder| {
        first
            .iter()
            .zip(folder.components())
            .take(shared)
            .take_while(|(a, b)| same_component(a, b))
            .count()
    });
    Some(first[..shared].iter().collect())
}

/// Windows doesn't tell letter case apart in paths.
fn same_component(first: &Component, second: &Component) -> bool {
    first.as_os_str().eq_ignore_ascii_case(second.as_os_str())
}

/// Where `file` goes when what's in `from` moves to `to`.
pub fn destination_of(file: &Path, from: &Path, to: &Path) -> PathBuf {
    match file.strip_prefix(from) {
        Ok(relative) => to.join(relative),
        Err(_) => to.join(file.file_name().unwrap_or(file.as_os_str())),
    }
}

/// Moves a file to `to`, or to "Name (2).ext" beside it when that's taken, making its folder, and answers where it
/// went. Another drive's folder gets a copy, then the original is deleted.
pub fn move_file(from: &Path, to: &Path) -> io::Result<PathBuf> {
    if let Some(folder) = to.parent() {
        fs::create_dir_all(folder)?;
    }
    let to = free_name(to);
    if fs::rename(from, &to).is_ok() {
        return Ok(to);
    }
    fs::copy(from, &to)?;
    if let Err(error) = fs::remove_file(from) {
        // Keep one copy only: the original stays where the history says it is.
        let _ = fs::remove_file(&to);
        return Err(error);
    }
    Ok(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_folder_holding_every_file() {
        let files = [
            PathBuf::from(r"D:\Shares\Pack\a.jpg"),
            PathBuf::from(r"D:\Shares\Pack\Sub\b.jpg"),
        ];
        assert_eq!(
            common_folder(&files),
            Some(PathBuf::from(r"D:\Shares\Pack"))
        );
        assert_eq!(
            common_folder(&[PathBuf::from(r"D:\Videos\clip.mkv")]),
            Some(PathBuf::from(r"D:\Videos"))
        );
        assert_eq!(
            common_folder(&[
                PathBuf::from(r"D:\videos\a.mkv"),
                PathBuf::from(r"D:\Videos\b.mkv")
            ]),
            Some(PathBuf::from(r"D:\videos")),
            "ignoring case"
        );
        assert_eq!(common_folder(&[]), None);
    }

    #[test]
    fn keeps_each_file_where_it_was_inside_the_folder() {
        assert_eq!(
            destination_of(
                Path::new(r"D:\Shares\Pack\Sub\b.jpg"),
                Path::new(r"D:\Shares\Pack"),
                Path::new(r"E:\Archive")
            ),
            PathBuf::from(r"E:\Archive\Sub\b.jpg")
        );
    }

    #[test]
    fn moves_a_file_without_replacing_one_already_there() {
        let folder = tempfile::tempdir().unwrap();
        let from = folder.path().join("clip.mkv");
        fs::write(&from, b"new").unwrap();
        let target = folder.path().join("Archive").join("clip.mkv");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"old").unwrap();

        let moved = move_file(&from, &target).unwrap();
        assert_eq!(moved, folder.path().join("Archive").join("clip (2).mkv"));
        assert_eq!(fs::read(&moved).unwrap(), b"new");
        assert_eq!(fs::read(&target).unwrap(), b"old");
        assert!(!from.exists());
    }
}
