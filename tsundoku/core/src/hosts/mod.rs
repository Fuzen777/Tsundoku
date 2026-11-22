//! File hosts Tsundoku reads itself, without a tool: MEGA, MediaFire, Google Drive, Dropbox and OneDrive. A host's
//! reader turns a share link into the files behind it (`Share`), once for the preview card and again when the
//! download starts, since the addresses files download from expire. `downloads::file_runner` then saves them. A host
//! may put a check only a person can pass in front of a file (`check`), or a countdown (`FileSource::countdown`).

pub mod archive;
pub mod boxcom;
pub mod check;
pub mod drive;
pub mod dropbox;
pub mod fanbox;
pub mod filemail;
pub mod filesfm;
pub mod icedrive;
pub mod mediafire;
pub mod mega;
pub mod onedrive;
pub mod onefichier;
pub mod pcloud;
pub mod swisstransfer;
#[cfg(test)]
pub(crate) mod test_server;
pub mod transfernow;
pub mod uploadee;
pub mod wetransfer;
pub mod window_sites;
pub mod windowed;
pub mod yandex;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{
    ACCEPT_LANGUAGE, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, HeaderMap, HeaderValue,
    LAST_MODIFIED, RANGE,
};
use reqwest::{ResponseBuilderExt, StatusCode};

use crate::cancel::CancelToken;
use crate::metadata::{FileContents, PreviewError, SharedFile, VideoInfo};
use crate::passwords::Password;
use crate::rules::safe_folder_name;
use crate::sites;

/// How long connecting may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// How long a request may wait for its answer, or a download for its next bytes.
const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// Sent with the requests to hosts that expect a browser (`HostFacts::browser_agent`; none since MediaFire was found to
/// answer Tsundoku's own identity without Cloudflare's check), and by `client`, for the history's thumbnails.
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:140.0) Gecko/20100101 Firefox/140.0";
/// Sent with the requests to hosts that answer Tsundoku as they answer a browser.
const HONEST_AGENT: &str = concat!("Tsundoku/", env!("CARGO_PKG_VERSION"));
/// Pages in English, for the error messages read from them (Google Drive's follow the address otherwise).
const LANGUAGE: &str = "en-US,en;q=0.9";
/// The most of a page read to explain why a file didn't come.
const PAGE_LIMIT: u64 = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    Mega,
    MediaFire,
    GoogleDrive,
    Dropbox,
    OneDrive,
    PCloud,
    WeTransfer,
    ArchiveOrg,
    YandexDisk,
    Box,
    Icedrive,
    OneFichier,
    SwissTransfer,
    FilesFm,
    UploadEe,
    TransferNow,
    Filemail,
    /// Fanbox's posts, read through Tsundoku's window (`fanbox`).
    Fanbox,
    /// A site read only through the check window (`window_sites`).
    Window(window_sites::WindowSite),
}

/// What Tsundoku knows about a host, kept in the host's own module (its `FACTS`). Adding a host is a module with its
/// `FACTS`, `read` and file source, and a line in `Host::facts`.
pub(crate) struct HostFacts {
    /// The name people know it by, shown in Supported Sites and used for its folder.
    pub name: &'static str,
    /// The site in the history, in lowercase as the tools' names are.
    pub site: &'static str,
    /// The domains its links are on, the one shown on the preview card first.
    pub domains: &'static [&'static str],
    /// Whether it marks every file it sends as an attachment, so that any other answer is a page saying why the file
    /// didn't come.
    pub sends_attachments: bool,
    /// Whether it's asked as Firefox (`USER_AGENT`), as the hosts Tsundoku read first were. A host that answers the
    /// same to Tsundoku saying who it is (`HONEST_AGENT`) is asked that way.
    pub browser_agent: bool,
    /// Lists the files behind a link.
    pub read: fn(&Client, &str, &CancelToken) -> Result<Share, PreviewError>,
    /// Why it answered with a page instead of a file, when its pages say more than their status.
    pub page_error: Option<fn(&Page) -> PreviewError>,
    /// For a site read only through the check window (`window_sites`): the shape of its file links.
    pub window_link: Option<&'static window_sites::LinkShape>,
    /// The file a download address of the host's own names, by the host's ID: for a download link a person copied
    /// from their browser, which gets a waiting download past its check (MediaFire's `download<n>.mediafire.com/…`).
    pub download_file_id: Option<fn(&str) -> Option<String>>,
    /// Lists the files behind a link that needs a password, with the one the person typed. Set only once a real
    /// link that needs one was downloaded through it: without it, such a link says Tsundoku can't enter its password
    /// yet (`PreviewError::PasswordProtected`). A file whose address needs the password too carries it in its
    /// `FileSource`, in memory only.
    pub unlock: Option<Unlock>,
}

/// A host's reader for a link that needs a password, given the one the person typed (`HostFacts::unlock`).
pub(crate) type Unlock = fn(&Client, &str, &Password, &CancelToken) -> Result<Share, PreviewError>;

impl Host {
    /// The hosts with a reader of their own.
    pub const BUILT_IN: [Self; 18] = [
        Self::Mega,
        Self::MediaFire,
        Self::GoogleDrive,
        Self::Dropbox,
        Self::OneDrive,
        Self::PCloud,
        Self::WeTransfer,
        Self::ArchiveOrg,
        Self::YandexDisk,
        Self::Box,
        Self::Icedrive,
        Self::OneFichier,
        Self::SwissTransfer,
        Self::FilesFm,
        Self::UploadEe,
        Self::TransferNow,
        Self::Filemail,
        Self::Fanbox,
    ];

    /// Every host: those with a reader of their own, then the sites read only through the check window.
    pub fn all() -> impl Iterator<Item = Self> {
        Self::BUILT_IN.into_iter().chain(
            (0..window_sites::LIST.len())
                .map(|index| Self::Window(window_sites::WindowSite(index))),
        )
    }

    pub(crate) fn facts(self) -> &'static HostFacts {
        match self {
            Self::Mega => &mega::FACTS,
            Self::MediaFire => &mediafire::FACTS,
            Self::GoogleDrive => &drive::FACTS,
            Self::Dropbox => &dropbox::FACTS,
            Self::OneDrive => &onedrive::FACTS,
            Self::Icedrive => &icedrive::FACTS,
            Self::OneFichier => &onefichier::FACTS,
            Self::SwissTransfer => &swisstransfer::FACTS,
            Self::FilesFm => &filesfm::FACTS,
            Self::UploadEe => &uploadee::FACTS,
            Self::TransferNow => &transfernow::FACTS,
            Self::Filemail => &filemail::FACTS,
            Self::Fanbox => &fanbox::FACTS,
            Self::Window(site) => &window_sites::LIST[site.0],
            Self::Box => &boxcom::FACTS,
            Self::YandexDisk => &yandex::FACTS,
            Self::ArchiveOrg => &archive::FACTS,
            Self::WeTransfer => &wetransfer::FACTS,
            Self::PCloud => &pcloud::FACTS,
        }
    }

    /// The host a link belongs to, by its domain. `link` must come from `metadata::normalize_link`.
    pub fn of(link: &str) -> Option<Self> {
        let host = sites::host(link)?;
        Self::all().find(|candidate| candidate.owns(&host))
    }

    /// Whether `host`, a link's host name, is on one of this host's domains.
    pub(crate) fn owns(self, host: &str) -> bool {
        self.domains()
            .iter()
            .any(|domain| sites::on_domain(host, domain))
    }

    /// The name people know it by, shown in Supported Sites and used for its folder.
    pub fn name(self) -> &'static str {
        self.facts().name
    }

    /// The file a link names, for a site read only through the check window (`window_sites`).
    pub(crate) fn window_link(self, link: &str) -> Option<windowed::WindowLink> {
        window_sites::link_of(self.facts().window_link?, link)
    }

    /// The file a download address of the host's own names, by the host's ID (`HostFacts::download_file_id`).
    pub(crate) fn download_file_id(self, link: &str) -> Option<String> {
        (self.facts().download_file_id?)(link)
    }

    /// The error for a link on this host that isn't to a file or folder Tsundoku can read.
    pub(crate) fn link_not_recognized(self) -> PreviewError {
        PreviewError::LinkNotRecognized {
            site: self.name().to_owned(),
        }
    }

    /// The site in the history, in lowercase as the tools' names are. Google Drive and Dropbox match yt-dlp's names
    /// for them, so what it downloaded from them before still counts.
    pub fn site(self) -> &'static str {
        self.facts().site
    }

    fn domains(self) -> &'static [&'static str] {
        self.facts().domains
    }

    /// What its requests say they come from: Firefox, or Tsundoku and its version.
    fn user_agent(self) -> &'static str {
        if self.facts().browser_agent {
            USER_AGENT
        } else {
            HONEST_AGENT
        }
    }

    /// Whether the host marks every file it sends as an attachment, so that any other answer is a page saying why
    /// the file didn't come.
    pub(crate) fn sends_attachments(self) -> bool {
        self.facts().sends_attachments
    }

    /// Why the host answered with a page instead of a file. Cloudflare's check for a browser is the same on every host.
    pub(crate) fn page_error(self, page: &Page) -> PreviewError {
        if check::is_browser_check(page) {
            return PreviewError::BrowserCheck {
                site: self.name().to_owned(),
            };
        }
        match self.facts().page_error {
            Some(page_error) => page_error(page),
            None => code_error(self, page.status).unwrap_or_else(|| PreviewError::Failed {
                detail: format!("{} answered with a page instead of the file", self.name()),
            }),
        }
    }

    /// Whether the person can enter a password for this host's links that need one (`HostFacts::unlock`).
    pub fn takes_passwords(self) -> bool {
        self.facts().unlock.is_some()
    }

    /// Lists the files behind `link`, opened with `password` when it needs one and the host takes it (`unlock`). A
    /// link that needs a password the host takes asks for it (`PreviewError::PasswordNeeded`), `wrong` when one was
    /// given. An empty shared folder has nothing to download.
    pub fn read(
        self,
        client: &Client,
        link: &str,
        password: Option<&Password>,
        cancel: &CancelToken,
    ) -> Result<Share, PreviewError> {
        let unlock = self.facts().unlock;
        let read = match (password, unlock) {
            (Some(password), Some(unlock)) => unlock(client, link, password, cancel),
            _ => (self.facts().read)(client, link, cancel),
        };
        let read = read.map_err(|error| match error {
            PreviewError::PasswordProtected { .. } if unlock.is_some() => {
                PreviewError::password_needed(password.is_some())
            }
            other => other,
        });
        let share = match read {
            // The link's own page is behind a check: one file the check window gives, named after the link, until
            // the person starts its download there.
            Err(PreviewError::BrowserCheck { .. }) => windowed::share(self, link),
            read => read?,
        };
        if share.files.is_empty() {
            return Err(PreviewError::NoVideo {
                detail: format!("the {} folder is empty", self.name()),
            });
        }
        Ok(share)
    }
}

/// What a share link holds: one file, or the files of a shared folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Share {
    pub host: Host,
    /// The host's ID for the file or folder, the history's key with the host's site.
    pub id: String,
    /// The link to download it again, with anything it needs to be read (MEGA's key).
    pub url: String,
    /// The file's name, or the folder's.
    pub title: String,
    /// As YYYY-MM-DD, when the host says.
    pub upload_date: Option<String>,
    pub files: Vec<RemoteFile>,
}

impl Share {
    pub fn total_bytes(&self) -> u64 {
        self.files.iter().map(|file| file.size).sum()
    }

    /// The share as the preview card and the queue know it.
    pub fn video_info(&self) -> VideoInfo {
        VideoInfo {
            site: self.host.site().to_owned(),
            site_name: self.host.domains()[0].to_owned(),
            video_id: self.id.clone(),
            url: self.url.clone(),
            title: self.title.clone(),
            uploader: None,
            account: None,
            duration_seconds: None,
            upload_date: self.upload_date.clone(),
            thumbnail_url: None,
            best_height: None,
            post: None,
            files: Some(FileContents {
                count: u32::try_from(self.files.len()).unwrap_or(u32::MAX),
                bytes: self.total_bytes(),
            }),
            post_files: Vec::new(),
            // A share of one file has nothing to pick.
            shared_files: if self.files.len() > 1 {
                self.files
                    .iter()
                    .map(|file| SharedFile {
                        path: shared_path(&file.path),
                        size: file.size,
                        picked: true,
                    })
                    .collect()
            } else {
                Vec::new()
            },
            playlist: None,
            tags: Vec::new(),
            pixiv: None,
            fanbox: None,
        }
    }
}

/// A share's file's path (`RemoteFile::path`) as the preview, the download and the history spell it: its folders,
/// then its name, joined with "/".
pub fn shared_path(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// One file of a share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    /// Where it goes inside the download's folder: a shared folder's own folder and subfolders, then the file's
    /// name, each made safe for Windows.
    pub path: PathBuf,
    pub size: u64,
    pub source: FileSource,
}

/// How to fetch a file's download address once its download starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSource {
    Mega(mega::MegaFile),
    MediaFire(mediafire::MediaFireFile),
    GoogleDrive(drive::DriveFile),
    Dropbox(dropbox::DropboxFile),
    OneDrive(onedrive::OneDriveFile),
    Icedrive(icedrive::IcedriveFile),
    Box(boxcom::BoxFile),
    YandexDisk(yandex::YandexFile),
    ArchiveOrg(archive::ArchiveFile),
    WeTransfer(wetransfer::WeTransferFile),
    PCloud(pcloud::PCloudFile),
    OneFichier(onefichier::OneFichierFile),
    SwissTransfer(swisstransfer::SwissTransferFile),
    FilesFm(filesfm::FilesFmFile),
    UploadEe(uploadee::UploadEeFile),
    TransferNow(transfernow::TransferNowFile),
    Filemail(filemail::FilemailFile),
    /// A file only the check window can give (`windowed`).
    Windowed(windowed::WindowedFile),
    Fanbox(fanbox::FanboxFile),
}

/// Where a file downloads from, and how to check what arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub url: String,
    pub check: FileCheck,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileCheck {
    /// MEGA's files arrive encrypted: they're decrypted with the key as they're saved, then checked with its MAC.
    Mega(mega::FileKey),
    /// The SHA-256 MediaFire gives, in lowercase hex.
    Sha256(String),
    /// The SHA-1 OneDrive gives, in lowercase hex.
    Sha1(String),
    /// The MD5 Filemail gives, in lowercase hex.
    Md5(String),
    /// Google Drive and Dropbox give nothing to check by beyond the size.
    None,
}

impl FileSource {
    /// Asks the host where the file downloads from now. These addresses expire, so it's asked once per run.
    pub fn address(&self, client: &Client, cancel: &CancelToken) -> Result<Address, PreviewError> {
        match self {
            Self::Mega(file) => mega::address(client, file, cancel),
            Self::MediaFire(file) => mediafire::address(client, file, cancel),
            // These links lead to the file whenever they're asked.
            Self::GoogleDrive(file) => Ok(Address {
                url: drive::download_url(file),
                check: FileCheck::None,
            }),
            Self::Dropbox(file) => Ok(Address {
                url: file.url.clone(),
                check: FileCheck::None,
            }),
            Self::OneDrive(file) => onedrive::address(client, file, cancel),
            Self::Icedrive(file) => icedrive::address(client, file, cancel),
            Self::Box(file) => Ok(Address {
                url: boxcom::download_url(file),
                check: FileCheck::None,
            }),
            Self::YandexDisk(file) => yandex::address(client, file, cancel),
            Self::ArchiveOrg(file) => archive::address(client, file, cancel),
            Self::WeTransfer(file) => wetransfer::address(client, file, cancel),
            Self::PCloud(file) => pcloud::address(client, file, cancel),
            Self::OneFichier(file) => onefichier::address(client, file, cancel),
            Self::SwissTransfer(file) => swisstransfer::address(client, file, cancel),
            Self::FilesFm(file) => filesfm::address(client, file, cancel),
            Self::UploadEe(file) => uploadee::address(client, file, cancel),
            Self::TransferNow(file) => transfernow::address(client, file, cancel),
            Self::Filemail(file) => filemail::address(client, file, cancel),
            Self::Windowed(file) => windowed::address(file),
            Self::Fanbox(file) => Ok(fanbox::address(file)),
        }
    }

    /// Whether its size may be unknown (0) until it downloads: a file the check window gives, or a Fanbox image whose
    /// server says no size.
    pub fn size_may_be_unknown(&self) -> bool {
        matches!(self, Self::Windowed(_) | Self::Fanbox(_))
    }

    /// How to check the file once it arrives, as far as the host said when it was read: for an address the check
    /// window gave, which `address` isn't asked for.
    pub fn known_check(&self) -> FileCheck {
        match self {
            Self::MediaFire(mediafire::MediaFireFile {
                sha256: Some(sha256),
                ..
            })
            | Self::YandexDisk(yandex::YandexFile {
                sha256: Some(sha256),
                ..
            }) => FileCheck::Sha256(sha256.clone()),
            _ => FileCheck::None,
        }
    }

    /// How long the host makes a free download wait before it gives the file's address, asked just before
    /// `address` (1fichier's countdown). The host may instead say to wait between downloads
    /// (`PreviewError::WaitBetweenDownloads`).
    pub fn countdown(
        &self,
        client: &Client,
        cancel: &CancelToken,
    ) -> Result<Option<Duration>, PreviewError> {
        match self {
            Self::OneFichier(file) => onefichier::countdown(client, file, cancel).map(Some),
            _ => Ok(None),
        }
    }
}

/// The client every host request goes through.
pub fn client() -> Result<Client, PreviewError> {
    built(Client::builder(), USER_AGENT)
}

/// The client a host's requests go through: saying it's Tsundoku, unless the host expects a browser.
pub fn client_for(host: Host) -> Result<Client, PreviewError> {
    let mut builder = Client::builder();
    // Fanbox's file server gives a supported post's files only to the signed-in session.
    if host == Host::Fanbox
        && let Some(cookie) = fanbox::session_header()
        && let Ok(mut value) = HeaderValue::from_str(&cookie)
    {
        value.set_sensitive(true);
        builder = builder.default_headers(HeaderMap::from_iter([(reqwest::header::COOKIE, value)]));
    }
    built(builder, host.user_agent())
}

/// A client saying it's Tsundoku, for a site that isn't one of the hosts (a mirror page).
pub(crate) fn tsundoku_client() -> Result<Client, PreviewError> {
    built(Client::builder(), HONEST_AGENT)
}

/// A host's client, answering redirects instead of following them: for a short link, whose redirect names what it
/// leads to.
pub(crate) fn client_without_redirects(host: Host) -> Result<Client, PreviewError> {
    built(
        Client::builder().redirect(reqwest::redirect::Policy::none()),
        host.user_agent(),
    )
}

/// A client saying it's Tsundoku, answering redirects instead of following them: for a short-link site's link
/// (`short_links`), whose redirect names where it leads.
pub(crate) fn tsundoku_client_without_redirects() -> Result<Client, PreviewError> {
    built(
        Client::builder().redirect(reqwest::redirect::Policy::none()),
        HONEST_AGENT,
    )
}

fn built(builder: reqwest::blocking::ClientBuilder, agent: &str) -> Result<Client, PreviewError> {
    builder
        .user_agent(agent)
        .default_headers(HeaderMap::from_iter([(
            ACCEPT_LANGUAGE,
            HeaderValue::from_static(LANGUAGE),
        )]))
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(READ_TIMEOUT)
        .build()
        .map_err(|error| PreviewError::Failed {
            detail: error.to_string(),
        })
}

/// Lists the files behind a host's link, for the preview card, opened with `password` when it needs one.
pub fn read(
    link: &str,
    password: Option<&Password>,
    cancel: &CancelToken,
) -> Result<Share, PreviewError> {
    let host = Host::of(link).ok_or(PreviewError::UnsupportedSite)?;
    host.read(&client_for(host)?, link, password, cancel)
}

/// A link's files as the preview card shows them: the share, with what a Fanbox post says of itself (its creator,
/// cover and tags).
pub fn read_preview(
    link: &str,
    password: Option<&Password>,
    cancel: &CancelToken,
) -> Result<VideoInfo, PreviewError> {
    let host = Host::of(link).ok_or(PreviewError::UnsupportedSite)?;
    let client = client_for(host)?;
    if host == Host::Fanbox {
        return fanbox::read_for_preview(&client, link, cancel).map(|post| post.video_info());
    }
    host.read(&client, link, password, cancel)
        .map(|share| share.video_info())
}

/// Form fields as a request's body (`application/x-www-form-urlencoded`).
pub(crate) fn form(fields: &[(&str, &str)]) -> String {
    let mut url = reqwest::Url::parse("https://form.invalid/").expect("an address");
    url.query_pairs_mut().extend_pairs(fields);
    url.query().unwrap_or_default().to_owned()
}

/// A file or folder name from a host, safe to use on Windows.
pub(crate) fn safe_name(name: &str) -> String {
    safe_folder_name(name, "file")
}

/// A time in seconds since 1970 as a date, YYYY-MM-DD in UTC, as upload dates are. `None` before 1970.
pub(crate) fn date_of(seconds: i64) -> Option<String> {
    if seconds < 0 {
        return None;
    }
    let days = seconds / 86_400;
    // Howard Hinnant's civil_from_days, for days since 1970-01-01.
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

/// Stops between requests once `cancel` is set.
pub(crate) fn check_cancel(cancel: &CancelToken) -> Result<(), PreviewError> {
    if cancel.is_cancelled() {
        Err(PreviewError::Cancelled)
    } else {
        Ok(())
    }
}

pub(crate) fn network_error(error: reqwest::Error) -> PreviewError {
    PreviewError::Network {
        detail: error.to_string(),
    }
}

/// The one way every reader's requests go to a site: what reaches it, and what doesn't, sorted the same for all.
pub(crate) trait SendChecked {
    /// Sends the request to `site` (its name, as people know it). An answer that is a check only a person can pass
    /// (`check::is_browser_check`) is `PreviewError::BrowserCheck` for `site`, whatever its status, before the reader
    /// makes anything of it; a request that doesn't reach the site is a network error.
    fn send_checked(self, site: &str) -> Result<Response, PreviewError>;
}

impl SendChecked for RequestBuilder {
    fn send_checked(self, site: &str) -> Result<Response, PreviewError> {
        checked(site, self.send().map_err(network_error)?)
    }
}

/// `response` from `site`, unless it's a check only a person can pass: then `PreviewError::BrowserCheck`. A check the
/// answer's headers don't mark is told by its page, which only an answer with a check's status is read for; the
/// reader then gets an answer with the same status, headers and page, to read as it would have.
pub(crate) fn checked(site: &str, response: Response) -> Result<Response, PreviewError> {
    if check::is_marked(response.headers()) {
        return Err(check::needs_person(site));
    }
    if !check::may_be_check_page(response.status()) {
        return Ok(response);
    }
    let url = response.url().clone();
    let status = response.status();
    let version = response.version();
    let mut headers = response.headers().clone();
    let mut page = Vec::new();
    // A page that doesn't arrive whole still has its status.
    let _ = response.take(PAGE_LIMIT).read_to_end(&mut page);
    if check::is_check_page(&String::from_utf8_lossy(&page)) {
        return Err(check::needs_person(site));
    }
    headers.insert(CONTENT_LENGTH, HeaderValue::from(page.len()));
    let mut again = http::Response::builder()
        .status(status)
        .version(version)
        .url(url);
    if let Some(all) = again.headers_mut() {
        *all = headers;
    }
    again
        .body(page)
        .map(Response::from)
        .map_err(|error| PreviewError::Failed {
            detail: error.to_string(),
        })
}

/// A host's answer that isn't a success, sorted as a preview's errors are.
pub(crate) fn status_error(host: Host, response: &Response) -> Option<PreviewError> {
    code_error(host, response.status())
}

/// What a status that isn't a success means, sorted as a preview's errors are.
pub(crate) fn code_error(host: Host, status: StatusCode) -> Option<PreviewError> {
    if status.is_success() {
        return None;
    }
    let detail = format!("{} answered {status}", host.name());
    Some(match status.as_u16() {
        // Fanbox's file server gives a supporter's file only to the signed-in session: its sign-in is missing, or out
        // of date, and signing in again fixes it.
        401 | 403 if host == Host::Fanbox => PreviewError::FanboxSignInNeeded { refused: true },
        404 | 410 => PreviewError::Unavailable { detail },
        429 | 509 => PreviewError::LimitReached {
            site: host.name().to_owned(),
            detail,
        },
        500..=599 => PreviewError::Network { detail },
        _ => PreviewError::Failed { detail },
    })
}

/// What a host says about a file when asked for its first byte.
pub(crate) enum Answer {
    File(Found),
    /// A page instead: an error, a sign-in, or a check for a browser.
    Page(Page),
}

/// A file's name and size, as its download answers.
pub(crate) struct Found {
    /// From Content-Disposition.
    pub name: Option<String>,
    pub size: u64,
    /// From Last-Modified, as YYYY-MM-DD.
    pub modified: Option<String>,
}

/// A page a host answered with instead of a file.
pub(crate) struct Page {
    /// Where it ended up, after redirects.
    pub url: String,
    pub status: StatusCode,
    pub html: String,
    /// The answer's headers mark it as a check for a browser (`check::is_marked`).
    pub challenged: bool,
}

impl Page {
    /// Reads the start of the answer, which is all that explains an error.
    pub(crate) fn read(response: Response) -> Self {
        let url = response.url().to_string();
        let status = response.status();
        let challenged = check::is_marked(response.headers());
        let mut bytes = Vec::new();
        // A page that doesn't arrive whole still has its status.
        let _ = response.take(PAGE_LIMIT).read_to_end(&mut bytes);
        Self {
            url,
            status,
            html: String::from_utf8_lossy(&bytes).into_owned(),
            challenged,
        }
    }
}

/// Asks `host` for the first byte of the file at `url`, for its name and size, or the page it answers instead.
pub(crate) fn probe(
    client: &Client,
    host: Host,
    url: &str,
    cancel: &CancelToken,
) -> Result<Answer, PreviewError> {
    check_cancel(cancel)?;
    let response = client
        .get(url)
        .header(RANGE, "bytes=0-0")
        .send_checked(host.name())?;
    if let Some(found) = found(response.status(), response.headers()) {
        return Ok(Answer::File(found));
    }
    // An empty file has no first byte. Google Drive answers 416 without its size then, but gives it for the whole
    // file, which a Google Doc doesn't have.
    if response.status() == StatusCode::RANGE_NOT_SATISFIABLE {
        check_cancel(cancel)?;
        let whole = client.head(url).send_checked(host.name())?;
        if let Some(found) = found(whole.status(), whole.headers()) {
            return Ok(Answer::File(found));
        }
    }
    Ok(Answer::Page(Page::read(response)))
}

/// The file's name and size, if the answer to a request for it is the file.
fn found(status: StatusCode, headers: &HeaderMap) -> Option<Found> {
    let header = |name| headers.get(name).and_then(|value| value.to_str().ok());
    let size = match status {
        // Part of the file, or none of an empty one ("bytes */0").
        StatusCode::PARTIAL_CONTENT | StatusCode::RANGE_NOT_SATISFIABLE => {
            header(CONTENT_RANGE).and_then(range_total)
        }
        // The whole file instead.
        StatusCode::OK if is_attachment(headers) => {
            header(CONTENT_LENGTH).and_then(|value| value.trim().parse().ok())
        }
        _ => None,
    }?;
    Some(Found {
        name: header(CONTENT_DISPOSITION).and_then(attachment_name),
        size,
        modified: header(LAST_MODIFIED).and_then(http_date),
    })
}

/// Whether an answer is a file to save rather than a page to show.
pub(crate) fn is_attachment(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim_start().get(..10))
        .is_some_and(|start| start.eq_ignore_ascii_case("attachment"))
}

/// The total in a Content-Range: "bytes 0-0/1234" or "bytes */1234".
fn range_total(value: &str) -> Option<u64> {
    value.rsplit_once('/')?.1.trim().parse().ok()
}

/// The file name in a Content-Disposition: `filename*=UTF-8''<percent-encoded>` when there is one, since `filename`
/// may have lost its non-ASCII characters, or else `filename="<name>"`.
pub fn attachment_name(header: &str) -> Option<String> {
    let mut plain = None;
    for parameter in parameters(header) {
        let Some((key, value)) = parameter.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim().to_ascii_lowercase().as_str() {
            "filename*" => {
                let mut pieces = value.trim_matches('"').splitn(3, '\'');
                if let (Some(charset), Some(_language), Some(encoded)) =
                    (pieces.next(), pieces.next(), pieces.next())
                    && charset.eq_ignore_ascii_case("utf-8")
                {
                    let name = percent_decode(encoded);
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
            "filename" => plain = Some(unquote(value)),
            _ => {}
        }
    }
    plain.filter(|name| !name.is_empty())
}

/// A header's parameters: split at each ";" that isn't inside quotes.
fn parameters(header: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (index, character) in header.char_indices() {
        match character {
            _ if escaped => escaped = false,
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ';' if !quoted => {
                parts.push(&header[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&header[start..]);
    parts
}

/// A quoted header value without its quotes and backslashes.
fn unquote(value: &str) -> String {
    let Some(inner) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return value.to_owned();
    };
    let mut text = String::with_capacity(inner.len());
    let mut characters = inner.chars();
    while let Some(character) = characters.next() {
        match character {
            '\\' => text.extend(characters.next()),
            _ => text.push(character),
        }
    }
    text
}

/// Text with its "%xx" escapes turned back into the UTF-8 they stand for.
pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escaped = (bytes[index] == b'%')
            .then(|| bytes.get(index + 1..index + 3))
            .flatten()
            .filter(|hex| hex.iter().all(u8::is_ascii_hexdigit))
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        match escaped {
            Some(byte) => {
                decoded.push(byte);
                index += 3;
            }
            None => {
                decoded.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// A date as an HTTP header gives it ("Fri, 11 Oct 2019 03:51:58 GMT"), as YYYY-MM-DD.
pub(crate) fn http_date(value: &str) -> Option<String> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let parts: Vec<&str> = value.split_whitespace().collect();
    let [_, day, month, year, ..] = parts.as_slice() else {
        return None;
    };
    let day: u32 = day.parse().ok()?;
    let year: u32 = year.parse().ok()?;
    let month = MONTHS
        .iter()
        .position(|name| name.eq_ignore_ascii_case(month))?
        + 1;
    (1..=31)
        .contains(&day)
        .then(|| format!("{year:04}-{month:02}-{day:02}"))
}

/// HTML text with its entities ("&amp;", "&#39;", "&#x27;") turned back into characters.
pub(crate) fn html_text(text: &str) -> String {
    let mut decoded = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        decoded.push_str(&rest[..start]);
        rest = &rest[start..];
        let entity = rest
            .find(';')
            .filter(|&end| end <= 10)
            .and_then(|end| Some((entity(&rest[1..end])?, end)));
        match entity {
            Some((character, end)) => {
                decoded.push(character);
                rest = &rest[end + 1..];
            }
            None => {
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }
    decoded.push_str(rest);
    decoded
}

/// The character an HTML entity's name stands for: a few named ones, or any by number.
fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{a0}'),
        _ => {
            let number = name.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

/// The text between `start` and the next `end` after it.
pub(crate) fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = text.find(start)? + start.len();
    let length = text[from..].find(end)?;
    Some(&text[from..from + length])
}

/// A page's title, as text.
pub(crate) fn title_of(html: &str) -> Option<String> {
    let start = html.find("<title")?;
    let title = between(&html[start..], ">", "</title>")?;
    let title = html_text(title.trim());
    (!title.is_empty()).then_some(title)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share_of(paths: &[&str]) -> Share {
        Share {
            host: Host::Dropbox,
            id: "folder".to_owned(),
            url: "https://www.dropbox.com/scl/fo/folder".to_owned(),
            title: "Pack".to_owned(),
            upload_date: None,
            files: paths
                .iter()
                .map(|path| RemoteFile {
                    path: PathBuf::from(path),
                    size: 5,
                    source: FileSource::Dropbox(dropbox::DropboxFile {
                        url: format!("https://www.dropbox.com/{path}?dl=1"),
                    }),
                })
                .collect(),
        }
    }

    #[test]
    fn lists_a_shared_folders_files_to_pick_from() {
        let folder = share_of(&[r"Pack\a.zip", r"Pack\Sub\b.zip"]).video_info();
        assert_eq!(
            folder.shared_files,
            [
                SharedFile {
                    path: "Pack/a.zip".to_owned(),
                    size: 5,
                    picked: true,
                },
                SharedFile {
                    path: "Pack/Sub/b.zip".to_owned(),
                    size: 5,
                    picked: true,
                },
            ],
            "the folders joined with slashes, in the share's order"
        );
        assert_eq!(folder.picked_shared_files(), None, "every file");
        assert_eq!(folder.picked_contents(), folder.files);

        let one = share_of(&["a.zip"]).video_info();
        assert!(one.shared_files.is_empty(), "nothing to pick");
        assert_eq!(one.picked_shared_files(), None);
        assert_eq!(
            one.picked_contents(),
            Some(FileContents { count: 1, bytes: 5 })
        );
    }

    #[test]
    fn a_shared_folder_takes_the_files_ticked() {
        let mut folder = share_of(&[r"Pack\a.zip", r"Pack\b.zip", r"Pack\c.zip"]).video_info();
        folder.shared_files[1].picked = false;
        assert_eq!(
            folder.picked_shared_files(),
            Some(vec!["Pack/a.zip".to_owned(), "Pack/c.zip".to_owned()])
        );
        assert_eq!(
            folder.picked_contents(),
            Some(FileContents {
                count: 2,
                bytes: 10
            })
        );
        assert_eq!(
            folder.files,
            Some(FileContents {
                count: 3,
                bytes: 15
            }),
            "what the link holds stays the whole share"
        );
    }

    #[test]
    fn knows_its_hosts_by_domain() {
        for (link, host) in [
            ("https://mega.nz/file/AbCdEf12#key", Some(Host::Mega)),
            ("https://MEGA.co.nz/#!AbCdEf12!key", Some(Host::Mega)),
            (
                "https://www.mediafire.com/file/abc123/name.zip/file",
                Some(Host::MediaFire),
            ),
            ("https://mediafire.com/?abc123", Some(Host::MediaFire)),
            (
                "https://drive.google.com/file/d/1l_5RK28JRL19wpT22B-DY9We3TVXnnQQ/view",
                Some(Host::GoogleDrive),
            ),
            (
                "https://drive.usercontent.google.com/download?id=1l_5RK28JRL19wpT22B",
                Some(Host::GoogleDrive),
            ),
            ("https://docs.google.com/document/d/1TFYNzuZJTgN/edit", None),
            (
                "https://www.dropbox.com/scl/fi/r2kd2skcy5ylbbta5y1pz/a.mp4?rlkey=w",
                Some(Host::Dropbox),
            ),
            (
                "https://dl.dropboxusercontent.com/s/abc/a.mp4",
                Some(Host::Dropbox),
            ),
            (
                "https://1drv.ms/f/c/2d90e71fb9eb254f/EnMm8c2mP_FPv6lwt1jy01YB6bQhoPQ25vtzAhycYisERw",
                Some(Host::OneDrive),
            ),
            (
                "https://onedrive.live.com/redir?resid=36712431A95E7A25!476",
                Some(Host::OneDrive),
            ),
            ("https://notmega.nz/file/AbCdEf12", None),
            ("https://www.youtube.com/watch?v=jNQXAC9IVRw", None),
        ] {
            assert_eq!(Host::of(link), host, "{link}");
        }
    }

    #[test]
    fn names_the_host_of_a_link_it_cant_read() {
        // These fail before anything is fetched, so the test needs no network.
        for (link, site) in [
            ("https://mega.nz/pro", "MEGA"),
            ("https://www.mediafire.com/upgrade/", "MediaFire"),
            ("https://drive.google.com/drive/my-drive", "Google Drive"),
            ("https://www.dropbox.com/home", "Dropbox"),
            ("https://onedrive.live.com/about/en-us/", "OneDrive"),
            ("https://1drv.ms/", "OneDrive"),
        ] {
            assert_eq!(
                read(link, None, &CancelToken::new()).err(),
                Some(PreviewError::LinkNotRecognized {
                    site: site.to_owned()
                }),
                "{link}"
            );
        }
        assert_eq!(
            read("https://example.com/file.zip", None, &CancelToken::new()).err(),
            Some(PreviewError::UnsupportedSite)
        );
    }

    #[test]
    fn every_request_stops_at_a_check_before_its_reader_reads_it() {
        use test_server::{Reply, TestServer};
        let server = TestServer::start(|request| match request.path.as_str() {
            "/marked" => Reply::new(200, "<html>a page</html>").with("cf-mitigated", "challenge"),
            "/check" => Reply::new(
                403,
                "<title>Just a moment...</title><script>window._cf_chl_opt = {};</script>",
            ),
            "/busy" => Reply::new(503, "<script>window._cf_chl_opt={cvId: '3'}</script>"),
            "/forbidden" => Reply::new(403, r#"{"error": {"code": "accessDenied"}}"#)
                .with("Content-Type", "application/json"),
            "/agent" => Reply::new(200, request.header("user-agent").unwrap_or_default()),
            _ => Reply::new(200, "the file"),
        });
        let client = tsundoku_client().unwrap();
        let send = |path: &str| client.get(server.url(path)).send_checked("Example");
        let check = PreviewError::BrowserCheck {
            site: "Example".to_owned(),
        };
        for path in ["/marked", "/check", "/busy"] {
            assert_eq!(send(path).err(), Some(check.clone()), "{path}");
        }

        let forbidden = send("/forbidden").unwrap();
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
        assert_eq!(forbidden.url().as_str(), server.url("/forbidden"));
        assert_eq!(
            forbidden
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("application/json"),
            "the reader gets the answer whole"
        );
        let error: serde_json::Value = forbidden.json().unwrap();
        assert_eq!(error["error"]["code"], "accessDenied");

        let mut file = String::new();
        send("/file").unwrap().read_to_string(&mut file).unwrap();
        assert_eq!(file, "the file");
        let mut agent = String::new();
        send("/agent").unwrap().read_to_string(&mut agent).unwrap();
        assert_eq!(agent, HONEST_AGENT, "it says it's Tsundoku");
        assert_eq!(
            probe(
                &client,
                Host::Dropbox,
                &server.url("/check"),
                &CancelToken::new()
            )
            .err(),
            Some(PreviewError::BrowserCheck {
                site: "Dropbox".to_owned()
            }),
            "a probe too"
        );
    }

    #[test]
    fn reads_a_files_name_from_its_headers() {
        assert_eq!(
            attachment_name(
                "attachment; filename=\"youtube-dl test video '?BaW_jenozKc.mp4\"; filename*=UTF-8''youtube-dl%20test%20video%20%27%C3%A4BaW_jenozKc.mp4"
            )
            .as_deref(),
            Some("youtube-dl test video '\u{e4}BaW_jenozKc.mp4")
        );
        assert_eq!(
            attachment_name("attachment; filename=\"spam.txt\"").as_deref(),
            Some("spam.txt")
        );
        assert_eq!(
            attachment_name(r#"attachment; filename="a; \"b\".txt""#).as_deref(),
            Some(r#"a; "b".txt"#)
        );
        assert_eq!(
            attachment_name("attachment; filename=plain.zip").as_deref(),
            Some("plain.zip")
        );
        assert_eq!(attachment_name("attachment"), None);

        assert_eq!(range_total("bytes 0-0/498881336"), Some(498_881_336));
        assert_eq!(range_total("bytes */0"), Some(0));
        assert_eq!(range_total("bytes 0-0/*"), None);
        assert_eq!(
            http_date("Fri, 11 Oct 2019 03:51:58 GMT").as_deref(),
            Some("2019-10-11")
        );
        assert_eq!(http_date("yesterday"), None);

        let mut headers = HeaderMap::new();
        assert!(!is_attachment(&headers));
        assert!(found(StatusCode::OK, &headers).is_none(), "a page");
        headers.insert(
            CONTENT_DISPOSITION,
            HeaderValue::from_static("Attachment; filename=\"a.zip\""),
        );
        assert!(is_attachment(&headers));
        assert!(
            found(StatusCode::RANGE_NOT_SATISFIABLE, &headers).is_none(),
            "an empty file on Google Drive, or a Google Doc"
        );
        headers.insert(CONTENT_LENGTH, HeaderValue::from_static("0"));
        headers.insert(
            LAST_MODIFIED,
            HeaderValue::from_static("Fri, 11 Oct 2019 03:51:58 GMT"),
        );
        let empty = found(StatusCode::OK, &headers).unwrap();
        assert_eq!(
            (empty.name.as_deref(), empty.size, empty.modified.as_deref()),
            (Some("a.zip"), 0, Some("2019-10-11"))
        );
        headers.insert(CONTENT_RANGE, HeaderValue::from_static("bytes 0-0/1234"));
        assert_eq!(
            found(StatusCode::PARTIAL_CONTENT, &headers).map(|found| found.size),
            Some(1234)
        );
        headers.insert(CONTENT_RANGE, HeaderValue::from_static("bytes */0"));
        assert_eq!(
            found(StatusCode::RANGE_NOT_SATISFIABLE, &headers).map(|found| found.size),
            Some(0)
        );
    }

    #[test]
    fn reads_text_from_pages_and_links() {
        assert_eq!(
            html_text("Tom &amp; Jerry&#39;s &#x2014; &quot;x&quot; &lt;3 &bogus; & more"),
            "Tom & Jerry's \u{2014} \"x\" <3 &bogus; & more"
        );
        assert_eq!(
            percent_decode("4.5%20Food%20Drive/%C3%A9t%C3%A9%zz%2"),
            "4.5 Food Drive/\u{e9}t\u{e9}%zz%2"
        );
        assert_eq!(
            title_of("<html><head><title>\n Dropbox - Invalid Link - Simplify your life </title>")
                .as_deref(),
            Some("Dropbox - Invalid Link - Simplify your life")
        );
        assert_eq!(title_of("<title></title>"), None);
        assert_eq!(between("a [b] c", "[", "]"), Some("b"));
    }

    #[test]
    fn turns_seconds_into_a_date() {
        assert_eq!(date_of(0).as_deref(), Some("1970-01-01"));
        assert_eq!(date_of(951_782_400).as_deref(), Some("2000-02-29"));
        assert_eq!(date_of(1_746_575_999).as_deref(), Some("2025-05-06"));
        assert_eq!(date_of(1_793_926_645).as_deref(), Some("2026-11-06"));
        assert_eq!(date_of(-1), None);
    }

    #[test]
    fn a_share_becomes_a_preview_of_its_files() {
        let share = Share {
            host: Host::MediaFire,
            id: "abc123".to_owned(),
            url: "https://www.mediafire.com/file/abc123".to_owned(),
            title: "Pack".to_owned(),
            upload_date: Some("2024-05-01".to_owned()),
            files: [3, 4]
                .map(|size| RemoteFile {
                    path: PathBuf::from(format!("{size}.zip")),
                    size,
                    source: FileSource::MediaFire(mediafire::MediaFireFile {
                        quick_key: size.to_string(),
                        sha256: None,
                        direct: None,
                    }),
                })
                .to_vec(),
        };
        let video = share.video_info();
        assert_eq!(
            (video.site.as_str(), video.video_id.as_str()),
            ("mediafire", "abc123")
        );
        assert_eq!(video.site_name, "mediafire.com");
        assert_eq!(video.files, Some(FileContents { count: 2, bytes: 7 }));
        assert_eq!(video.post, None);
    }
}
