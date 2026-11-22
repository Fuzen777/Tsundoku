//! Downloading a Pixiv novel as an EPUB book (`pixiv_novel`): gallery-dl reads it again with Pixiv's sign-in, for its
//! text as it is now, its cover is read as previews read Pixiv's pictures (with Pixiv's Referer), and the book is
//! written into the item's folder, named after the novel, with a free name (" (2)") when one of its name is there.

use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use super::runner::{
    JobOutcome, JobSpec, PostJob, make_folders, open_log, seconds_since_epoch, write_log,
};
use super::{DownloadError, DownloadProgress, DownloadStage};
use crate::cancel::CancelToken;
use crate::epub::{self, Cover};
use crate::gallery::classify_download_failure;
use crate::metadata::PreviewError;
use crate::process::{self, OutputLine, ProgramCommand, RunError};
use crate::tools::{ToolId, start_error};
use crate::{hosts, pixiv_account, pixiv_novel, pixiv_warning, site_pictures};

pub(super) fn run(
    job: &JobSpec,
    post: &PostJob,
    cancel: &CancelToken,
    on_progress: &mut dyn FnMut(DownloadProgress),
) -> JobOutcome {
    let Some(gallery_dl) = &post.gallery_dl else {
        return JobOutcome::Failed(DownloadError::YtDlp {
            error: PreviewError::ImagesNotSetUp,
        });
    };
    if let Err(error) = make_folders(job) {
        return JobOutcome::Failed(error);
    }
    // Pixiv's sign-in, kept until the run ends, which deletes it.
    let config = pixiv_account::gallery_dl_config(&job.url, None);
    let mut args: Vec<String> = vec!["--config-ignore".to_owned()];
    if let Some(config) = &config {
        args.extend(config.args());
    }
    args.extend(["-j".to_owned(), "--".to_owned(), job.url.clone()]);
    let mut log = open_log(&job.log_path);
    write_log(
        &mut log,
        &format!(
            "=== {} gallery-dl (a novel's text) {}",
            seconds_since_epoch(),
            job.url
        ),
    );
    on_progress(DownloadProgress::at(DownloadStage::Starting));

    let mut json = String::new();
    let mut errors = Vec::new();
    let result = process::run(
        &ProgramCommand::new(gallery_dl).args(&args),
        cancel,
        None,
        |line| match line {
            OutputLine::Stdout(text) => json.push_str(&text),
            OutputLine::Stderr(text) => {
                write_log(&mut log, &text);
                errors.push(text);
            }
        },
    );
    pixiv_warning::note_download(&job.url, 1);
    match result {
        Err(RunError::Cancelled) => return JobOutcome::Cancelled,
        Err(RunError::Start { source, .. }) => {
            return JobOutcome::Failed(DownloadError::YtDlp {
                error: PreviewError::Tool {
                    tool: ToolId::GalleryDl,
                    error: start_error(source, gallery_dl),
                },
            });
        }
        Err(other) => {
            return JobOutcome::Failed(DownloadError::Unexpected {
                detail: other.to_string(),
            });
        }
        Ok(status) if !status.success() => {
            let lines: Vec<&str> = errors.iter().map(String::as_str).collect();
            return JobOutcome::Failed(DownloadError::YtDlp {
                error: classify_download_failure(status.code(), &lines),
            });
        }
        Ok(_) => {}
    }
    let novel = match pixiv_novel::parse(&json) {
        Ok(novel) => novel,
        Err(error) => return JobOutcome::Failed(DownloadError::YtDlp { error }),
    };
    on_progress(DownloadProgress::at(DownloadStage::Finishing));
    // The cover is a nicety: a book without it is written all the same.
    let cover = novel.cover.as_deref().and_then(|address| {
        let picture = site_pictures::fetch(&URL_SAFE_NO_PAD.encode(address));
        if let Err(error) = &picture {
            write_log(&mut log, &format!("the cover couldn't be read: {error}"));
        }
        let picture = picture.ok()?;
        let media_type = if picture.content_type.starts_with("image/png") {
            "image/png"
        } else {
            "image/jpeg"
        };
        Some(Cover {
            bytes: picture.bytes,
            media_type: media_type.to_owned(),
        })
    });
    let name = format!("{}.epub", hosts::safe_name(&novel.title));
    let wanted = job.folder.join(&name);
    let path: PathBuf = if job.overwrite {
        wanted
    } else {
        super::file_runner::free_name(&wanted)
    };
    match epub::write(&novel.book(cover), &path) {
        Ok(()) => {
            write_log(&mut log, &format!("wrote {}", path.display()));
            JobOutcome::Finished {
                files: vec![path],
                format: Some("EPUB".to_owned()),
                shared: Vec::new(),
            }
        }
        Err(error) => JobOutcome::Failed(DownloadError::Unexpected {
            detail: format!("the book couldn't be written: {error}"),
        }),
    }
}
