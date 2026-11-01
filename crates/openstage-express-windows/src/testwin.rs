// SPDX-License-Identifier: Apache-2.0
//! The windows the self-test (`TEST EXPRESS CLICKS.bat`) needs:
//!
//! * [`run_controls_window`] is a small window of ordinary Windows controls with known names: a button "Save", a check box
//!   "Check me", a drop-down list "Country", an edit field "Email" and a **password** field "Password". It runs in a process of
//!   its own (the program starts itself with `test-window`), because a window of Express would be left out of the pictures and
//!   its clicks ignored, exactly as the program is meant to do.
//! * [`Beacon`] is a plain magenta rectangle. The self-test shows one, first as any program's window and then marked as
//!   excluded from capture, and looks for magenta in the pictures: that proves the capture bar and the editor stay out of them.

use std::ffi::c_void;
use std::mem::size_of;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, GetSysColorBrush, COLOR_WINDOW, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, GetWindowRect, LoadCursorW,
    PostMessageW, PostQuitMessage, RegisterClassExW, SendMessageW, SetForegroundWindow, ShowWindow, TranslateMessage,
    BS_AUTOCHECKBOX, CBS_DROPDOWNLIST, CB_ADDSTRING, CW_USEDEFAULT, ES_AUTOHSCROLL, ES_PASSWORD, HMENU, IDC_ARROW, MSG,
    SW_SHOW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_DESTROY, WNDCLASSEXW, WS_BORDER, WS_CAPTION, WS_CHILD,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_OVERLAPPED, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};

/// Where the controls are, in screen pixels (their centres), as the test window reports them on its first line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub save: (i32, i32),
    pub check: (i32, i32),
    pub country: (i32, i32),
    pub email: (i32, i32),
    pub password: (i32, i32),
    /// An empty part of the test window's own background.
    pub background: (i32, i32),
    /// The window itself, so that the test can tell whether it has the keyboard.
    pub window: isize,
}

/// The first line the test window prints.
pub const READY: &str = "READY ";

unsafe extern "system" fn controls_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_CLOSE => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn centre(hwnd: HWND) -> (i32, i32) {
    let mut r = RECT::default();
    unsafe {
        let _ = GetWindowRect(hwnd, &mut r);
    }
    ((r.left + r.right) / 2, (r.top + r.bottom) / 2)
}

fn style(bits: u32) -> WINDOW_STYLE {
    WINDOW_STYLE(bits)
}

/// Show the window of known controls and handle its messages until it is closed. Prints one line, `READY` and the controls'
/// positions as JSON, when the window is up.
pub fn run_controls_window() -> Result<(), String> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let class = w!("ExpressClickTestWindow");
        let class_def = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(controls_proc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW).map_err(|e| e.to_string())?,
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            ..Default::default()
        };
        if RegisterClassExW(&class_def) == 0 {
            return Err("The test window class could not be registered.".to_string());
        }
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            class,
            w!("Express click test"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            160,
            140,
            600,
            520,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| e.to_string())?;
        let child = |kind: PCWSTR, text: PCWSTR, bits: u32, x: i32, y: i32, width: i32, height: i32, id: usize| {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                kind,
                text,
                WS_CHILD | WS_VISIBLE | style(bits),
                x,
                y,
                width,
                height,
                Some(hwnd),
                Some(HMENU(id as *mut c_void)),
                Some(instance.into()),
                None,
            )
            .map_err(|e| e.to_string())
        };
        // A label comes straight before the field it names: that is how Windows gives a plain field its accessible name.
        child(w!("STATIC"), w!("Email"), 0, 30, 30, 200, 24, 200)?;
        let email =
            child(w!("EDIT"), w!(""), WS_BORDER.0 | WS_TABSTOP.0 | ES_AUTOHSCROLL as u32, 30, 56, 360, 30, 101)?;
        child(w!("STATIC"), w!("Password"), 0, 30, 110, 200, 24, 201)?;
        let password = child(
            w!("EDIT"),
            w!(""),
            WS_BORDER.0 | WS_TABSTOP.0 | ES_AUTOHSCROLL as u32 | ES_PASSWORD as u32,
            30,
            136,
            360,
            30,
            102,
        )?;
        child(w!("STATIC"), w!("Country"), 0, 30, 190, 200, 24, 202)?;
        let country = child(w!("COMBOBOX"), w!(""), WS_TABSTOP.0 | CBS_DROPDOWNLIST as u32, 30, 216, 360, 160, 103)?;
        for name in [w!("Canada"), w!("Germany"), w!("France")] {
            SendMessageW(country, CB_ADDSTRING, Some(WPARAM(0)), Some(LPARAM(name.0 as isize)));
        }
        let check = child(w!("BUTTON"), w!("Check me"), WS_TABSTOP.0 | BS_AUTOCHECKBOX as u32, 30, 280, 200, 30, 104)?;
        let save = child(w!("BUTTON"), w!("Save"), WS_TABSTOP.0, 30, 340, 160, 44, 105)?;
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let layout = Layout {
            save: centre(save),
            check: centre(check),
            country: centre(country),
            email: centre(email),
            password: centre(password),
            background: {
                let (x, y) = centre(save);
                (x + 300, y + 60)
            },
            window: hwnd.0 as isize,
        };
        println!("{READY}{}", serde_json::to_string(&layout).map_err(|e| e.to_string())?);
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

unsafe extern "system" fn beacon_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_CLOSE => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// A magenta rectangle on the screen, in a window of this program. It does not take the keyboard.
pub struct Beacon {
    hwnd: isize,
    thread: Option<JoinHandle<()>>,
}

/// The colour in blue, green, red, alpha order, as it appears in a picture of the screen.
pub const BEACON_BGRA: [u8; 4] = [0xFF, 0x00, 0xFF, 0xFF];

impl Beacon {
    pub fn show(x: i32, y: i32, width: i32, height: i32) -> Result<Beacon, String> {
        let (tx, rx) = mpsc::channel::<Result<isize, String>>();
        let thread = std::thread::spawn(move || unsafe {
            let result = (|| -> Result<(), String> {
                let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
                let class = w!("ExpressBeaconWindow");
                let class_def = WNDCLASSEXW {
                    cbSize: size_of::<WNDCLASSEXW>() as u32,
                    lpfnWndProc: Some(beacon_proc),
                    hInstance: instance.into(),
                    lpszClassName: class,
                    hbrBackground: HBRUSH(CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00FF00FF)).0),
                    ..Default::default()
                };
                // Registered more than once in a run: the second time Windows says it exists, which is fine.
                RegisterClassExW(&class_def);
                let hwnd = CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    class,
                    w!("Express beacon"),
                    WS_POPUP | WS_VISIBLE,
                    x,
                    y,
                    width,
                    height,
                    None,
                    None,
                    Some(instance.into()),
                    None,
                )
                .map_err(|e| e.to_string())?;
                let _ = tx.send(Ok(hwnd.0 as isize));
                let mut message = MSG::default();
                while GetMessageW(&mut message, None, 0, 0).as_bool() {
                    DispatchMessageW(&message);
                }
                Ok(())
            })();
            if let Err(why) = result {
                let _ = tx.send(Err(why));
            }
        });
        let _ = CW_USEDEFAULT;
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(hwnd)) => Ok(Beacon { hwnd, thread: Some(thread) }),
            Ok(Err(why)) => Err(why),
            Err(_) => Err("The beacon window did not appear in time.".to_string()),
        }
    }

    pub fn hwnd(&self) -> HWND {
        HWND(self.hwnd as *mut c_void)
    }
}

impl Drop for Beacon {
    fn drop(&mut self) {
        unsafe {
            let _ = PostMessageW(Some(self.hwnd()), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
