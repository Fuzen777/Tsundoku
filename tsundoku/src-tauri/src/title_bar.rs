//! The main window's own title bar. Its native frame is off (tauri.conf.json: `decorations: false`) and the UI draws the
//! bar: the app's name and the views' tabs, whose empty space moves the window (`data-tauri-drag-region`), and
//! Minimize, Maximize and Close at the top right (`WindowControls`). Tauri keeps a frameless window's resize borders (a
//! child window of its own along the top edge, Windows' invisible borders on the other sides), shadow and rounded
//! corners.
//!
//! Windows 11 shows Snap Layouts over a maximize button only when the window under the pointer answers `WM_NCHITTEST`
//! with `HTMAXBUTTON`, which the WebView never does. So a window of the app's lies over the UI's maximize button, above
//! the WebView, drawing nothing and answering that, as Windows Terminal's title bar does: it maximizes or restores the
//! window when clicked, and tells the UI when the pointer comes onto it, presses it or leaves (`MAXIMIZE_BUTTON_EVENT`),
//! since the WebView doesn't see the pointer there.

#[cfg(windows)]
use tauri::Manager;
use tauri::WebviewWindow;
use tsundoku_core::title_bar::ButtonPlace;

/// Sent to the UI with a `MaximizeButtonState` when the pointer comes onto the maximize button, presses it or leaves.
#[cfg(windows)]
const MAXIMIZE_BUTTON_EVENT: &str = "maximize-button";

/// Lays the window that answers for the maximize button over the UI's button, making it the first time. The UI calls
/// it once its button is drawn, and again if the button moves; the window then follows the main window's size itself.
#[tauri::command]
pub fn place_maximize_button(window: WebviewWindow, place: ButtonPlace) {
    #[cfg(windows)]
    {
        let main = window.clone();
        // Windows have to be made on the thread that runs the main window's messages.
        let _ = window.run_on_main_thread(move || {
            if let Ok(parent) = main.hwnd() {
                snap_layouts::place(parent, place, main.app_handle());
            }
        });
    }
    #[cfg(not(windows))]
    let _ = (window, place);
}

#[cfg(windows)]
mod snap_layouts {
    use std::cell::Cell;

    use tauri::{AppHandle, Emitter};
    use tsundoku_core::title_bar::{ButtonPlace, MaximizeButtonState};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent,
    };
    use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        CREATESTRUCTW, CreateWindowExW, DefWindowProcW, FindWindowExW, GWLP_USERDATA,
        GetClientRect, GetParent, GetWindowLongPtrW, HTMAXBUTTON, HWND_TOP, IsIconic, IsZoomed,
        PostMessageW, RegisterClassExW, SC_MAXIMIZE, SC_RESTORE, SM_CYFRAME, SWP_NOACTIVATE,
        SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos, USER_DEFAULT_SCREEN_DPI, WM_DPICHANGED,
        WM_NCCREATE, WM_NCDESTROY, WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN,
        WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_NCRBUTTONDOWN, WM_NCRBUTTONUP, WM_SIZE,
        WM_SYSCOMMAND, WNDCLASSEXW, WS_CHILD, WS_EX_NOREDIRECTIONBITMAP,
    };
    use windows::core::{PCWSTR, w};

    use super::MAXIMIZE_BUTTON_EVENT;

    const CLASS_NAME: PCWSTR = w!("TSUNDOKU_MAXIMIZE_BUTTON");
    /// The id of the main window's subclass that keeps the button's window in place (`follow_main_window`).
    const SUBCLASS_ID: usize = 0x7473_6b64;

    /// What the button's window keeps, behind its `GWLP_USERDATA`. Only the main thread touches it, through the
    /// window's messages, so `Cell`s do.
    struct MaximizeButton {
        app: AppHandle,
        place: Cell<ButtonPlace>,
        state: Cell<MaximizeButtonState>,
        /// Whether Windows will say when the pointer leaves (`TrackMouseEvent`), which it says once per request.
        tracking: Cell<bool>,
    }

    impl MaximizeButton {
        /// Tells the UI what the pointer does now, when that changed.
        fn set_state(&self, state: MaximizeButtonState) {
            if self.state.replace(state) != state {
                let _ = self.app.emit(MAXIMIZE_BUTTON_EVENT, state);
            }
        }
    }

    /// Puts the button's window at `place` over the main window `parent`, making it the first time.
    pub fn place(parent: HWND, place: ButtonPlace, app: &AppHandle) {
        let button = match find(parent) {
            Some(button) => button,
            None => match create(parent, place, app) {
                Some(button) => button,
                // Without it, the UI's own button still maximizes, only without Snap Layouts.
                None => return,
            },
        };
        if let Some(data) = data_of(button) {
            data.place.set(place);
        }
        position(parent, button);
    }

    fn find(parent: HWND) -> Option<HWND> {
        // SAFETY: looks among `parent`'s children for the class registered here.
        unsafe { FindWindowExW(Some(parent), None, CLASS_NAME, PCWSTR::null()) }.ok()
    }

    /// The button's data, from its `GWLP_USERDATA`, for as long as the window lives.
    fn data_of<'window>(button: HWND) -> Option<&'window MaximizeButton> {
        // SAFETY: the pointer was set from a `Box` at WM_NCCREATE and is freed only at WM_NCDESTROY, the window's last
        // message; the main thread alone uses it.
        unsafe { (GetWindowLongPtrW(button, GWLP_USERDATA) as *const MaximizeButton).as_ref() }
    }

    /// Makes the button's window: without a surface of its own, so it draws nothing and the UI's button shows through,
    /// yet takes the pointer. (Windows Terminal's is also layered, which a child window can only be in an app whose
    /// manifest names Windows 8 or later; Tauri's names none, and it isn't needed.) A subclass of the main window then
    /// keeps it in place.
    fn create(parent: HWND, place: ButtonPlace, app: &AppHandle) -> Option<HWND> {
        // SAFETY: plain Win32 calls with valid arguments; the data's `Box` goes to the window, which frees it at
        // WM_NCDESTROY, or is freed here if no window was made.
        unsafe {
            let instance = GetModuleHandleW(None).ok()?;
            let class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(button_proc),
                hInstance: instance.into(),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            // Fails harmlessly once the class is registered.
            let _ = RegisterClassExW(&class);
            let data = Box::into_raw(Box::new(MaximizeButton {
                app: app.clone(),
                place: Cell::new(place),
                state: Cell::new(MaximizeButtonState::Rest),
                tracking: Cell::new(false),
            }));
            let Ok(button) = CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP,
                CLASS_NAME,
                PCWSTR::null(),
                WS_CHILD,
                0,
                0,
                0,
                0,
                Some(parent),
                None,
                Some(instance.into()),
                Some(data.cast_const().cast()),
            ) else {
                drop(Box::from_raw(data));
                return None;
            };
            let _ = SetWindowSubclass(parent, Some(follow_main_window), SUBCLASS_ID, 0);
            Some(button)
        }
    }

    /// Lays the button's window over the UI's button, above the WebView, leaving out the top resize border while the
    /// main window isn't maximized, as Tauri's resize border does.
    fn position(parent: HWND, button: HWND) {
        let Some(data) = data_of(button) else {
            return;
        };
        // SAFETY: plain Win32 calls on the main window and its child.
        unsafe {
            if IsIconic(parent).as_bool() {
                return;
            }
            let mut client = RECT::default();
            if GetClientRect(parent, &mut client).is_err() {
                return;
            }
            let dpi = match GetDpiForWindow(parent) {
                0 => USER_DEFAULT_SCREEN_DPI,
                dpi => dpi,
            };
            let top_border = if IsZoomed(parent).as_bool() {
                0
            } else {
                GetSystemMetricsForDpi(SM_CYFRAME, dpi)
            };
            let scale = f64::from(dpi) / f64::from(USER_DEFAULT_SCREEN_DPI);
            let rect = data
                .place
                .get()
                .in_pixels(client.right - client.left, scale, top_border);
            let _ = SetWindowPos(
                button,
                Some(HWND_TOP),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    /// The main window's subclass: once it has handled a change of size or DPI, the button's window follows.
    unsafe extern "system" fn follow_main_window(
        parent: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        // SAFETY: called by Windows with the main window's message, passed on unchanged.
        let result = unsafe { DefSubclassProc(parent, message, wparam, lparam) };
        match message {
            WM_SIZE | WM_DPICHANGED => {
                if let Some(button) = find(parent) {
                    position(parent, button);
                }
            }
            WM_NCDESTROY => {
                // SAFETY: removes this subclass from the window being destroyed.
                let _ =
                    unsafe { RemoveWindowSubclass(parent, Some(follow_main_window), SUBCLASS_ID) };
            }
            _ => {}
        }
        result
    }

    /// The button's window: it's the maximize button wherever the pointer is on it, and handles its own clicks, since
    /// Windows' own handling would draw an old-style button there.
    unsafe extern "system" fn button_proc(
        button: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_NCCREATE => {
                // SAFETY: at WM_NCCREATE, `lparam` is the CREATESTRUCTW whose parameter is the data's pointer.
                unsafe {
                    let created = &*(lparam.0 as *const CREATESTRUCTW);
                    SetWindowLongPtrW(button, GWLP_USERDATA, created.lpCreateParams as isize);
                }
            }
            WM_NCHITTEST => return LRESULT(HTMAXBUTTON as isize),
            WM_NCMOUSEMOVE => {
                if let Some(data) = data_of(button) {
                    if !data.tracking.get() {
                        let mut leave = TRACKMOUSEEVENT {
                            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE | TME_NONCLIENT,
                            hwndTrack: button,
                            dwHoverTime: 0,
                        };
                        // SAFETY: asks for WM_NCMOUSELEAVE on this window.
                        data.tracking
                            .set(unsafe { TrackMouseEvent(&mut leave) }.is_ok());
                    }
                    if data.state.get() == MaximizeButtonState::Rest {
                        data.set_state(MaximizeButtonState::Hovered);
                    }
                }
                return LRESULT(0);
            }
            WM_NCMOUSELEAVE => {
                if let Some(data) = data_of(button) {
                    data.tracking.set(false);
                    data.set_state(MaximizeButtonState::Rest);
                }
                return LRESULT(0);
            }
            WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK => {
                if let Some(data) = data_of(button) {
                    data.set_state(MaximizeButtonState::Pressed);
                }
                return LRESULT(0);
            }
            WM_NCLBUTTONUP => {
                if let Some(data) = data_of(button)
                    && data.state.get() == MaximizeButtonState::Pressed
                {
                    data.set_state(MaximizeButtonState::Hovered);
                    // SAFETY: asks the main window to maximize or restore, as Windows' own button does.
                    unsafe {
                        if let Ok(parent) = GetParent(button) {
                            let command = if IsZoomed(parent).as_bool() {
                                SC_RESTORE
                            } else {
                                SC_MAXIMIZE
                            };
                            let _ = PostMessageW(
                                Some(parent),
                                WM_SYSCOMMAND,
                                WPARAM(command as usize),
                                LPARAM(0),
                            );
                        }
                    }
                }
                return LRESULT(0);
            }
            // Windows' own handling would open the window's menu; the UI's button has none.
            WM_NCRBUTTONDOWN | WM_NCRBUTTONUP => return LRESULT(0),
            WM_NCDESTROY => {
                // SAFETY: the window's last message: takes back the `Box` given at WM_NCCREATE, once.
                unsafe {
                    let data = SetWindowLongPtrW(button, GWLP_USERDATA, 0) as *mut MaximizeButton;
                    if !data.is_null() {
                        drop(Box::from_raw(data));
                    }
                }
            }
            _ => {}
        }
        // SAFETY: Windows' default handling for everything else.
        unsafe { DefWindowProcW(button, message, wparam, lparam) }
    }
}
