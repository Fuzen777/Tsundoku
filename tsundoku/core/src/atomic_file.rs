//! Replacing a file's contents without ever leaving it half-written.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

/// Writes `contents` to a temporary file beside `path`, then renames it over `path`, so a crash mid-write leaves the
/// old file whole. Creates the folder if needed.
pub fn write_atomically(path: &Path, contents: &[u8]) -> io::Result<()> {
    let folder = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the path has no folder"))?;
    fs::create_dir_all(folder)?;

    let mut temporary_name = path.file_name().unwrap_or_default().to_os_string();
    temporary_name.push(".tmp");
    let temporary_path = folder.join(temporary_name);

    let written = File::create(&temporary_path).and_then(|mut file| {
        file.write_all(contents)?;
        file.sync_all()
    });
    // On Windows, rename replaces an existing file (MoveFileExW with MOVEFILE_REPLACE_EXISTING).
    let result = written.and_then(|()| fs::rename(&temporary_path, path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_file_and_leaves_no_temporary_file() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("nested").join("file.txt");

        write_atomically(&path, b"first").unwrap();
        write_atomically(&path, b"second").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"second");
        let names: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["file.txt"]);
    }
}
