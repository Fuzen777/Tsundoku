//! Installing the app's own copy of a tool from its official release.

use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::Path;

use zip::ZipArchive;

use super::catalog::{Package, executable_name, spec};
use super::github::{GitHub, Release};
use super::locate::ffprobe_beside;
use super::{ToolError, ToolId, ToolStage, checksum};
use crate::cancel::CancelToken;

/// Downloads the tool from `release`, checks it against the release's checksum and puts it in its folder under
/// `tools_folder`, in place of any older copy. Returns the release file's checksum.
pub(crate) fn install_release(
    id: ToolId,
    release: &Release,
    github: &GitHub,
    tools_folder: &Path,
    cancel: &CancelToken,
    report: &dyn Fn(ToolStage),
) -> Result<String, ToolError> {
    let tool = spec(id);
    let asset = release.asset(tool.release.asset)?;
    let expected = release_checksum(id, release, github)?;
    // One working folder per tool, since the first-run setup installs all three at once.
    let download_folder = tools_folder.join(format!(".download-{}", tool.folder));
    let new_folder = tools_folder.join(format!(".new-{}", tool.folder));

    let installed = (|| {
        recreate_folder(&download_folder)?;
        let download_path = download_folder.join(&asset.name);
        let actual = github.download(
            &asset.browser_download_url,
            &download_path,
            cancel,
            |received, total| report(ToolStage::Downloading { received, total }),
        )?;

        report(ToolStage::Verifying);
        if actual != expected {
            return Err(ToolError::ChecksumMismatch);
        }

        report(ToolStage::Installing);
        recreate_folder(&new_folder)?;
        match tool.release.package {
            Package::Program => fs::rename(
                &download_path,
                new_folder.join(executable_name(tool.command)),
            )?,
            Package::Zip { keep } => extract_zip(&download_path, &new_folder, keep)?,
        }
        require_executables(id, &new_folder)?;
        replace_folder(&new_folder, &tools_folder.join(tool.folder))?;
        Ok(expected)
    })();

    let _ = fs::remove_dir_all(&download_folder);
    let _ = fs::remove_dir_all(&new_folder);
    installed
}

/// The SHA-256 the release lists for the tool's file, in lowercase hex.
pub(crate) fn release_checksum(
    id: ToolId,
    release: &Release,
    github: &GitHub,
) -> Result<String, ToolError> {
    let source = &spec(id).release;
    let checksums = release.asset(source.checksums)?;
    let text = github.text(&checksums.browser_download_url)?;
    checksum::find(&text, source.asset).ok_or_else(|| ToolError::ChecksumMissing {
        file: source.asset.to_owned(),
    })
}

fn recreate_folder(folder: &Path) -> io::Result<()> {
    match fs::remove_dir_all(folder) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    fs::create_dir_all(folder)
}

/// Extracts the files `keep` accepts into `destination`, side by side whatever their folder in the zip.
fn extract_zip(
    archive_path: &Path,
    destination: &Path,
    keep: fn(&Path) -> bool,
) -> Result<(), ToolError> {
    let damaged = |error: zip::result::ZipError| ToolError::Storage {
        detail: format!("the downloaded zip can't be read: {error}"),
    };
    let mut archive =
        ZipArchive::new(BufReader::new(File::open(archive_path)?)).map_err(damaged)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(damaged)?;
        // `enclosed_name` refuses paths that would escape the destination, such as "../file".
        let Some(entry_path) = entry.enclosed_name() else {
            continue;
        };
        if entry.is_dir() || !keep(&entry_path) {
            continue;
        }
        let Some(file_name) = entry_path.file_name() else {
            continue;
        };
        let mut output = File::create(destination.join(file_name))?;
        io::copy(&mut entry, &mut output)?;
    }
    Ok(())
}

/// Fails when the release didn't contain the tool's program (and, for ffmpeg, ffprobe).
fn require_executables(id: ToolId, folder: &Path) -> Result<(), ToolError> {
    let executable = folder.join(executable_name(spec(id).command));
    let mut required = vec![executable.clone()];
    if id == ToolId::Ffmpeg {
        required.push(ffprobe_beside(&executable));
    }
    match required.into_iter().find(|path| !path.is_file()) {
        Some(missing) => Err(ToolError::MissingFromRelease {
            file: missing
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        }),
        None => Ok(()),
    }
}

/// Puts `new_folder` in place of `target`. The old copy is moved aside first and deleted after, or put back if the
/// new one can't take its place. Fails if a file in the old copy is in use.
fn replace_folder(new_folder: &Path, target: &Path) -> io::Result<()> {
    let mut old_name = std::ffi::OsString::from(".old-");
    old_name.push(target.file_name().unwrap_or_default());
    let old_folder = target.with_file_name(old_name);
    let _ = fs::remove_dir_all(&old_folder);

    let had_old_copy = target.exists();
    if had_old_copy {
        fs::rename(target, &old_folder)?;
    }
    if let Err(error) = fs::rename(new_folder, target) {
        if had_old_copy {
            let _ = fs::rename(&old_folder, target);
        }
        return Err(error);
    }
    let _ = fs::remove_dir_all(&old_folder);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    use super::*;

    fn write_zip(path: &Path, entries: &[&str]) {
        let mut writer = ZipWriter::new(File::create(path).unwrap());
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for name in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(name.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }

    fn names_in(folder: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn extracts_ffmpeg_flat_without_ffplay() {
        let folder = tempfile::tempdir().unwrap();
        let archive = folder.path().join("ffmpeg.zip");
        write_zip(
            &archive,
            &[
                "build/LICENSE.txt",
                "build/bin/avcodec-63.dll",
                "build/bin/ffmpeg.exe",
                "build/bin/ffplay.exe",
                "build/bin/ffprobe.exe",
                "build/doc/ffmpeg.html",
            ],
        );
        let destination = folder.path().join("ffmpeg");
        fs::create_dir(&destination).unwrap();

        let Package::Zip { keep } = &spec(ToolId::Ffmpeg).release.package else {
            panic!("ffmpeg comes as a zip");
        };
        extract_zip(&archive, &destination, *keep).unwrap();

        assert_eq!(
            names_in(&destination),
            ["avcodec-63.dll", "ffmpeg.exe", "ffprobe.exe"]
        );
        assert_eq!(
            fs::read(destination.join("ffmpeg.exe")).unwrap(),
            b"build/bin/ffmpeg.exe"
        );
        require_executables(ToolId::Ffmpeg, &destination).unwrap();
    }

    #[test]
    fn reports_a_missing_program() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("ffmpeg.exe"), b"").unwrap();
        let error = require_executables(ToolId::Ffmpeg, folder.path()).unwrap_err();
        assert_eq!(
            error,
            ToolError::MissingFromRelease {
                file: executable_name("ffprobe")
            }
        );
    }

    #[test]
    fn replaces_an_old_copy() {
        let folder = tempfile::tempdir().unwrap();
        let target = folder.path().join("deno");
        let new_folder = folder.path().join(".new-deno");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("old.txt"), b"old").unwrap();
        fs::create_dir(&new_folder).unwrap();
        fs::write(new_folder.join("new.txt"), b"new").unwrap();

        replace_folder(&new_folder, &target).unwrap();

        assert_eq!(names_in(&target), ["new.txt"]);
        assert_eq!(names_in(folder.path()), ["deno"]);
    }

    #[test]
    fn installs_where_there_was_no_copy() {
        let folder = tempfile::tempdir().unwrap();
        let new_folder = folder.path().join(".new-deno");
        fs::create_dir(&new_folder).unwrap();
        replace_folder(&new_folder, &folder.path().join("deno")).unwrap();
        assert_eq!(names_in(folder.path()), ["deno"]);
    }
}
