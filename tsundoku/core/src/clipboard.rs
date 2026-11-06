//! Windows' clipboard, as the clipboard watcher (`pending::watcher`) and the clipboard shortcut (`copied_text`) read
//! it. For the watcher, only the change count is read until something is copied; then the text, unless Tsundoku
//! copied it (its window, or WebView2's processes, which it starts) or the program that copied it marks it private,
//! as password managers do.

use std::thread;
use std::time::Duration;
use std::{mem, ptr, slice};

use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, GetClipboardOwner, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

use crate::pending::watcher::Clipboard;

/// Formats a program adds to a copy to ask clipboard watchers to leave it alone, such as a copied password.
const PRIVATE_FORMATS: [&str; 2] = [
    "ExcludeClipboardContentFromMonitorProcessing",
    "Clipboard Viewer Ignore",
];

/// A copy longer than this isn't read: a list of links is far shorter.
const LONGEST_TEXT_BYTES: usize = 1024 * 1024;

/// How many times opening the clipboard is tried while another program has it open, and how long apart.
const OPEN_TRIES: u32 = 10;
const OPEN_RETRY_WAIT: Duration = Duration::from_millis(20);

/// How far up from the program that copied to look for Tsundoku: WebView2's processes are its children.
const ANCESTORS_CHECKED: usize = 3;

pub struct SystemClipboard {
    private_formats: Vec<u32>,
}

impl SystemClipboard {
    pub fn new() -> Self {
        let private_formats = PRIVATE_FORMATS
            .iter()
            .map(|name| {
                let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
                // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call.
                unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
            })
            .filter(|&format| format != 0)
            .collect();
        Self { private_formats }
    }
}

impl Default for SystemClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clipboard for SystemClipboard {
    fn change_count(&mut self) -> Option<u32> {
        // SAFETY: takes nothing; 0 means the count can't be read.
        let count = unsafe { GetClipboardSequenceNumber() };
        (count != 0).then_some(count)
    }

    fn text(&mut self) -> Option<String> {
        let has_format =
            // SAFETY: only asks which formats the clipboard holds.
            |format: u32| unsafe { IsClipboardFormatAvailable(format) } != 0;
        if !has_format(u32::from(CF_UNICODETEXT))
            || self
                .private_formats
                .iter()
                .any(|&format| has_format(format))
            || copied_by_tsundoku()
        {
            return None;
        }
        copied_text()
    }
}

/// The text on the clipboard, whoever copied it, for the clipboard shortcut: pressing it asks for what's there.
/// `None` when what's copied isn't text or is too long to be a list of links.
pub fn copied_text() -> Option<String> {
    let _open = OpenedClipboard::open()?;
    // SAFETY: the clipboard is open, and the handle it gives stays valid until it's closed.
    unsafe {
        let handle = GetClipboardData(u32::from(CF_UNICODETEXT));
        if handle.is_null() {
            return None;
        }
        let size = GlobalSize(handle);
        if size == 0 || size > LONGEST_TEXT_BYTES {
            return None;
        }
        let data = GlobalLock(handle).cast::<u16>();
        if data.is_null() {
            return None;
        }
        let units = slice::from_raw_parts(data, size / mem::size_of::<u16>());
        let end = units
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(units.len());
        let text = String::from_utf16_lossy(&units[..end]);
        GlobalUnlock(handle);
        Some(text)
    }
}

/// The clipboard, open until dropped.
struct OpenedClipboard;

impl OpenedClipboard {
    /// Opens the clipboard, waiting a little while another program has it open.
    fn open() -> Option<Self> {
        for _ in 0..OPEN_TRIES {
            // SAFETY: a null window opens the clipboard for this thread; it's closed on drop.
            if unsafe { OpenClipboard(ptr::null_mut()) } != 0 {
                return Some(Self);
            }
            thread::sleep(OPEN_RETRY_WAIT);
        }
        None
    }
}

impl Drop for OpenedClipboard {
    fn drop(&mut self) {
        // SAFETY: this thread opened the clipboard.
        unsafe { CloseClipboard() };
    }
}

/// Whether Tsundoku copied what's on the clipboard: the window that owns it belongs to this process or to one it
/// started, as WebView2's processes are.
fn copied_by_tsundoku() -> bool {
    // SAFETY: takes nothing; a null window means no program owns the clipboard.
    let owner = unsafe { GetClipboardOwner() };
    if owner.is_null() {
        return false;
    }
    let mut process = 0;
    // SAFETY: `owner` is a window handle and `process` a valid place to write its process's ID.
    unsafe { GetWindowThreadProcessId(owner, &mut process) };
    // SAFETY: takes nothing.
    let tsundoku = unsafe { GetCurrentProcessId() };
    if process == 0 {
        return false;
    }
    let parents = parent_processes();
    let mut current = process;
    for _ in 0..=ANCESTORS_CHECKED {
        if current == tsundoku {
            return true;
        }
        match parents.iter().find(|(child, _)| *child == current) {
            Some(&(_, parent)) if parent != 0 && parent != current => current = parent,
            _ => return false,
        }
    }
    false
}

/// Every running process's ID and its parent's.
fn parent_processes() -> Vec<(u32, u32)> {
    let mut pairs = Vec::new();
    // SAFETY: a snapshot of the processes, closed below; the entry is sized as the calls require.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
            return pairs;
        }
        let mut entry: PROCESSENTRY32W = mem::zeroed();
        entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = Process32FirstW(snapshot, &mut entry) != 0;
        while found {
            pairs.push((entry.th32ProcessID, entry.th32ParentProcessID));
            found = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    pairs
}
