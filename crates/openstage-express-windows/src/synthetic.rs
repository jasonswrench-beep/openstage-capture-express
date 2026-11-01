// SPDX-License-Identifier: Apache-2.0
//! Mouse clicks and key presses made by the program itself with `SendInput`, for the self-test (`TEST EXPRESS CLICKS.bat`).
//! Windows treats them like a person's: the hooks see them, the program under the pointer receives them. Nothing else in
//! Express sends input.

use std::mem::size_of;
use std::time::Duration;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT, VIRTUAL_KEY, VK_CONTROL, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

fn send(inputs: &[INPUT]) -> bool {
    unsafe { SendInput(inputs, size_of::<INPUT>() as i32) as usize == inputs.len() }
}

fn mouse(flags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS, dx: i32, dy: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 { mi: MOUSEINPUT { dx, dy, mouseData: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

fn keyboard(vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: VIRTUAL_KEY(vk), wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 },
        },
    }
}

/// Move a window (of any program) to a new top left corner, keeping its size. `true` if Windows did it.
pub fn move_window(handle: isize, x: i32, y: i32) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER};
    unsafe {
        SetWindowPos(
            HWND(handle as *mut std::ffi::c_void),
            None,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    }
    .is_ok()
}

/// How long, in milliseconds, nobody has touched the mouse or keyboard. `None` if Windows will not say.
pub fn idle_ms() -> Option<u32> {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    if unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        Some(unsafe { GetTickCount() }.wrapping_sub(info.dwTime))
    } else {
        None
    }
}

/// The window that has the keyboard now (as the number Windows gives it), or 0 if none does.
pub fn foreground_window() -> isize {
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() }.0 as isize
}

/// Where the pointer is now.
pub fn pointer() -> Option<(i32, i32)> {
    let mut at = windows::Win32::Foundation::POINT::default();
    unsafe { GetCursorPos(&mut at) }.ok().map(|_| (at.x, at.y))
}

/// Move the pointer to a point of the desktop (physical pixels). `true` if Windows accepted the movement.
pub fn move_pointer(x: i32, y: i32) -> bool {
    let (vx, vy, vw, vh) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    if vw <= 1 || vh <= 1 {
        return false;
    }
    let dx = ((x - vx) as i64 * 65535 / (vw as i64 - 1)) as i32;
    let dy = ((y - vy) as i64 * 65535 / (vh as i64 - 1)) as i32;
    send(&[mouse(MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK, dx, dy)])
}

/// Move to a point and click the left button there.
pub fn left_click(x: i32, y: i32) -> bool {
    if !move_pointer(x, y) {
        return false;
    }
    std::thread::sleep(Duration::from_millis(60));
    send(&[mouse(MOUSEEVENTF_LEFTDOWN, 0, 0)]) && {
        std::thread::sleep(Duration::from_millis(30));
        send(&[mouse(MOUSEEVENTF_LEFTUP, 0, 0)])
    }
}

/// Type text the way an input method or dictation does: each letter is sent as a character, so it works whatever the keyboard
/// layout. The program under the pointer receives it as typing; the hooks see a key going down for each letter.
pub fn type_text(text: &str) -> bool {
    for unit in text.encode_utf16() {
        if !send(&[keyboard(0, unit, KEYEVENTF_UNICODE), keyboard(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)]) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    true
}

/// Press and release one key (a virtual-key code such as `0x0D` for Enter).
pub fn key(vk: u16) -> bool {
    send(&[keyboard(vk, 0, KEYBD_EVENT_FLAGS(0)), keyboard(vk, 0, KEYEVENTF_KEYUP)])
}

/// Press Ctrl and Shift together with another key, then release them all.
pub fn ctrl_shift(vk: u16) -> bool {
    send(&[
        keyboard(VK_CONTROL.0, 0, KEYBD_EVENT_FLAGS(0)),
        keyboard(VK_SHIFT.0, 0, KEYBD_EVENT_FLAGS(0)),
        keyboard(vk, 0, KEYBD_EVENT_FLAGS(0)),
        keyboard(vk, 0, KEYEVENTF_KEYUP),
        keyboard(VK_SHIFT.0, 0, KEYEVENTF_KEYUP),
        keyboard(VK_CONTROL.0, 0, KEYEVENTF_KEYUP),
    ])
}
