//! Rar archives (RAR 4 and 5, split into parts either way, names encrypted or not), through UnRAR's own library
//! (`unrar_sys`, built from its C++ source, whose licence lets any software read rar archives with it). Tsundoku has it
//! test each file and writes the bytes it hands over itself, so names are made safe as for the other archives, progress
//! is counted, and a stop is heard at once. The library asks for the password and for each next part through a
//! callback: the part it can't find is the one the archive waits for.

use std::cell::{Cell, RefCell};
use std::ffi::c_int;
use std::path::Path;
use std::sync::Mutex;

use unrar_sys as native;

use super::files::{EntryFile, Output, from_file_time};
use super::{Failure, Listing};
use crate::sync::lock;

/// The library keeps some state of its own between calls (its error handler), so one archive is read at a time.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// The longest name the library hands the callback, in characters.
const NAME_LENGTH: usize = 2048;

/// An open archive, closed when dropped.
struct Handle(*const native::Handle);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from `RAROpenArchiveEx` and is closed once, here.
        unsafe {
            native::RARCloseArchive(self.0);
        }
    }
}

/// What the library's callback reaches while an archive is open.
struct Session<'s, 'a> {
    /// NUL-terminated, or `None` to give none.
    password: Option<Vec<native::WCHAR>>,
    /// The part the library looked for and didn't find.
    missing: RefCell<Option<String>>,
    asked_password: Cell<bool>,
    /// Where the bytes of the file being tested go.
    writing: RefCell<Writing<'s, 'a>>,
}

struct Writing<'s, 'a> {
    output: Option<&'s mut Output<'a>>,
    entry: Option<EntryFile>,
    /// Why writing stopped, when it did: a write that failed, or a stop.
    stopped: Option<Failure>,
}

impl<'s, 'a> Session<'s, 'a> {
    fn new(password: Option<&str>, output: Option<&'s mut Output<'a>>) -> Self {
        Self {
            password: password.map(|password| password.chars().map(wide_char).chain([0]).collect()),
            missing: RefCell::new(None),
            asked_password: Cell::new(false),
            writing: RefCell::new(Writing {
                output,
                entry: None,
                stopped: None,
            }),
        }
    }

    fn address(&self) -> native::LPARAM {
        self as *const Self as native::LPARAM
    }

    /// Hands data the library unpacked to the file being written. False stops the library.
    fn write(&self, data: &[u8]) -> bool {
        let mut writing = self.writing.borrow_mut();
        let Writing {
            output,
            entry,
            stopped,
        } = &mut *writing;
        let (Some(output), Some(entry)) = (output.as_deref_mut(), entry.as_mut()) else {
            return true;
        };
        match entry.write(data).and_then(|()| output.wrote(data.len())) {
            Ok(()) => true,
            Err(failure) => {
                *stopped = Some(failure);
                false
            }
        }
    }

    /// Runs `act` with the output, when there's one.
    fn with_output<T>(&self, act: impl FnOnce(&mut Output<'a>) -> T) -> Option<T> {
        self.writing.borrow_mut().output.as_deref_mut().map(act)
    }

    /// What a code the library answered means, knowing what the callback heard.
    fn failure(&self, code: c_int) -> Failure {
        if let Some(stopped) = self.writing.borrow_mut().stopped.take() {
            return stopped;
        }
        if let Some(missing) = self.missing.borrow_mut().take() {
            return Failure::Missing(missing);
        }
        match code {
            native::ERAR_MISSING_PASSWORD => Failure::NeedsPassword,
            native::ERAR_BAD_PASSWORD => Failure::WrongPassword,
            // RAR 4 has no way to check a password: a wrong one reads as damage.
            native::ERAR_BAD_DATA if self.asked_password.get() && self.password.is_some() => {
                Failure::WrongPassword
            }
            native::ERAR_BAD_DATA => Failure::Damaged("a file's data is damaged".to_owned()),
            native::ERAR_BAD_ARCHIVE | native::ERAR_UNKNOWN_FORMAT => {
                Failure::Damaged("not a rar archive, or a damaged one".to_owned())
            }
            native::ERAR_EOPEN => Failure::Damaged("a part couldn't be opened".to_owned()),
            native::ERAR_EREAD => Failure::Damaged("a part couldn't be read".to_owned()),
            native::ERAR_NO_MEMORY => Failure::Damaged("not enough memory".to_owned()),
            native::ERAR_SMALL_BUF | native::ERAR_EREFERENCE => {
                Failure::Unsupported(format!("unrar's error {code}"))
            }
            code => Failure::Damaged(format!("unrar's error {code}")),
        }
    }
}

/// The library's callback: the password when it asks, each next part (`RAR_VOL_ASK` means it isn't there), and the
/// unpacked bytes. -1 stops it.
extern "C" fn callback(
    message: native::UINT,
    user_data: native::LPARAM,
    first: native::LPARAM,
    second: native::LPARAM,
) -> c_int {
    // SAFETY: `user_data` is the address of the `Session` the archive was opened with, which outlives its handle.
    let session = unsafe { &*(user_data as *const Session) };
    match message {
        native::UCM_CHANGEVOLUMEW if second == native::RAR_VOL_ASK => {
            // SAFETY: the library passes the next part's NUL-terminated name, at most `NAME_LENGTH` characters.
            let name = unsafe { wide_string(first as *const native::WCHAR, NAME_LENGTH) };
            let name = Path::new(&name)
                .file_name()
                .map_or(name.clone(), |name| name.to_string_lossy().into_owned());
            *session.missing.borrow_mut() = Some(name);
            -1
        }
        native::UCM_NEEDPASSWORDW => {
            session.asked_password.set(true);
            match &session.password {
                Some(password)
                    if usize::try_from(second).is_ok_and(|room| password.len() <= room) =>
                {
                    // SAFETY: the library's buffer holds `second` characters, and the password with its NUL fits.
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            password.as_ptr(),
                            first as *mut native::WCHAR,
                            password.len(),
                        );
                    }
                    1
                }
                _ => -1,
            }
        }
        native::UCM_NEEDPASSWORD => -1,
        native::UCM_PROCESSDATA => {
            let Ok(length) = usize::try_from(second) else {
                return -1;
            };
            // SAFETY: the library passes `second` bytes it unpacked, valid during the call.
            let data = unsafe { std::slice::from_raw_parts(first as *const u8, length) };
            if session.write(data) { 0 } else { -1 }
        }
        _ => 0,
    }
}

fn open(first: &Path, mode: std::ffi::c_uint, session: &Session) -> Result<Handle, Failure> {
    let name = ArchiveName::of(first)?;
    let mut data = name.open_data(mode);
    data.callback = Some(callback);
    data.user_data = session.address();
    // SAFETY: `data` and the name it points to outlive the call; the session outlives the handle. The library writes its result into `data`, though the binding takes a `*const`.
    let handle = unsafe { native::RAROpenArchiveEx((&raw mut data).cast_const()) };
    if handle.is_null() || data.open_result != 0 {
        if !handle.is_null() {
            drop(Handle(handle));
        }
        return Err(
            session.failure(c_int::try_from(data.open_result).unwrap_or(native::ERAR_UNKNOWN))
        );
    }
    let handle = Handle(handle);
    // SAFETY: the handle is open, and the session outlives it.
    unsafe { native::RARSetCallback(handle.0, Some(callback), session.address()) };
    Ok(handle)
}

/// The next file's header, or `None` at the end.
fn next_header(
    handle: &Handle,
    session: &Session,
) -> Result<Option<Box<native::HeaderDataEx>>, Failure> {
    let mut header = Box::<native::HeaderDataEx>::default();
    // SAFETY: the handle is open and `header` is a whole `HeaderDataEx` the library fills in.
    let code = unsafe { native::RARReadHeaderEx(handle.0, (&raw mut *header).cast_const()) };
    match code {
        native::ERAR_SUCCESS => Ok(Some(header)),
        native::ERAR_END_ARCHIVE => Ok(None),
        code => Err(session.failure(code)),
    }
}

/// Skips, tests or extracts the file whose header was just read; testing hands its bytes to the callback.
fn process(handle: &Handle, session: &Session, operation: c_int) -> Result<(), Failure> {
    // SAFETY: the handle is open, and null paths are allowed for skipping and testing.
    let code =
        unsafe { native::RARProcessFileW(handle.0, operation, std::ptr::null(), std::ptr::null()) };
    if code == native::ERAR_SUCCESS {
        Ok(())
    } else {
        Err(session.failure(code))
    }
}

fn size_of(header: &native::HeaderDataEx) -> u64 {
    u64::from(header.unp_size_high) << 32 | u64::from(header.unp_size)
}

fn is_directory(header: &native::HeaderDataEx) -> bool {
    header.flags & native::RHDF_DIRECTORY != 0
}

/// What it holds. Its names may be encrypted, so this may need the password; it also finds a missing part.
pub(super) fn list(first: &Path, password: Option<&str>) -> Result<Listing, Failure> {
    let _one = lock(&ONE_AT_A_TIME);
    let session = Session::new(password, None);
    let handle = open(first, native::RAR_OM_LIST, &session)?;
    let mut listing = Listing::default();
    while let Some(header) = next_header(&handle, &session)? {
        if !is_directory(&header) && header.redir_type == 0 {
            listing.bytes = listing.bytes.saturating_add(size_of(&header));
        }
        process(&handle, &session, native::RAR_SKIP)?;
    }
    Ok(listing)
}

pub(super) fn unpack(
    first: &Path,
    password: Option<&str>,
    output: &mut Output,
) -> Result<(), Failure> {
    let _one = lock(&ONE_AT_A_TIME);
    let session = Session::new(password, Some(output));
    let handle = open(first, native::RAR_OM_EXTRACT, &session)?;
    while let Some(header) = next_header(&handle, &session)? {
        // SAFETY: the name is the header's own array, NUL-terminated within it.
        let name = unsafe { wide_string(header.filename_w.as_ptr(), header.filename_w.len()) };
        let modified_ticks = u64::from(header.mtime_high) << 32 | u64::from(header.mtime_low);
        let modified = (modified_ticks != 0)
            .then(|| from_file_time(modified_ticks))
            .flatten();
        if header.redir_type != 0 {
            session.with_output(|output| output.leave_out(&name));
            process(&handle, &session, native::RAR_SKIP)?;
            continue;
        }
        if is_directory(&header) {
            session
                .with_output(|output| output.directory(&name))
                .transpose()?;
            process(&handle, &session, native::RAR_SKIP)?;
            continue;
        }
        let entry = session
            .with_output(|output| output.open(&name))
            .transpose()?
            .flatten();
        let Some(entry) = entry else {
            process(&handle, &session, native::RAR_SKIP)?;
            continue;
        };
        session.writing.borrow_mut().entry = Some(entry);
        let tested = process(&handle, &session, native::RAR_TEST);
        let entry = session.writing.borrow_mut().entry.take();
        match (tested, entry) {
            (Ok(()), Some(entry)) => entry.close(modified)?,
            (Err(failure), entry) => {
                if let Some(entry) = entry {
                    entry.discard();
                }
                return Err(failure);
            }
            (Ok(()), None) => {}
        }
    }
    Ok(())
}

/// A character as the library's wide characters hold it: UTF-16 on Windows (a character beyond it is two), UTF-32
/// elsewhere.
#[cfg(windows)]
fn wide_char(character: char) -> native::WCHAR {
    // Passwords outside the Basic Multilingual Plane are rare enough: their characters become U+FFFD.
    u16::try_from(u32::from(character)).unwrap_or(0xFFFD)
}

#[cfg(not(windows))]
fn wide_char(character: char) -> native::WCHAR {
    // UTF-32 fits wchar_t's 32 bits.
    native::WCHAR::try_from(u32::from(character)).unwrap_or(0xFFFD)
}

/// A NUL-terminated wide string of at most `length` characters.
///
/// # Safety
/// `pointer` must point to at least `length` readable wide characters, or to a NUL before them.
#[cfg(windows)]
unsafe fn wide_string(pointer: *const native::WCHAR, length: usize) -> String {
    // SAFETY: as the caller promises.
    let characters = unsafe { std::slice::from_raw_parts(pointer, length) };
    let end = characters
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(length);
    String::from_utf16_lossy(&characters[..end])
}

#[cfg(not(windows))]
unsafe fn wide_string(pointer: *const native::WCHAR, length: usize) -> String {
    // SAFETY: as the caller promises.
    let characters = unsafe { std::slice::from_raw_parts(pointer, length) };
    characters
        .iter()
        .take_while(|&&unit| unit != 0)
        .map(|&unit| {
            u32::try_from(unit)
                .ok()
                .and_then(char::from_u32)
                .unwrap_or(char::REPLACEMENT_CHARACTER)
        })
        .collect()
}

/// An archive's path as the library opens it: wide on Windows; elsewhere its bytes, since the library doesn't open
/// wide names there.
struct ArchiveName {
    #[cfg(windows)]
    wide: Vec<u16>,
    #[cfg(not(windows))]
    narrow: std::ffi::CString,
}

impl ArchiveName {
    #[cfg(windows)]
    fn of(path: &Path) -> Result<Self, Failure> {
        use std::os::windows::ffi::OsStrExt;
        Ok(Self {
            wide: path.as_os_str().encode_wide().chain([0]).collect(),
        })
    }

    #[cfg(not(windows))]
    fn of(path: &Path) -> Result<Self, Failure> {
        use std::os::unix::ffi::OsStrExt;
        std::ffi::CString::new(path.as_os_str().as_bytes())
            .map(|narrow| Self { narrow })
            .map_err(|_| Failure::Damaged("a NUL in its path".to_owned()))
    }

    #[cfg(windows)]
    fn open_data(&self, mode: std::ffi::c_uint) -> native::OpenArchiveDataEx {
        native::OpenArchiveDataEx::new(self.wide.as_ptr(), mode)
    }

    #[cfg(not(windows))]
    fn open_data(&self, mode: std::ffi::c_uint) -> native::OpenArchiveDataEx {
        native::OpenArchiveDataEx::new(self.narrow.as_ptr(), mode)
    }
}
