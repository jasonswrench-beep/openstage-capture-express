// SPDX-License-Identifier: Apache-2.0
//! Keeping Express's own windows out of its screenshots (spec section 8.5): every top-level window of this program is marked
//! `WDA_EXCLUDEFROMCAPTURE`, so the capture bar, the editor and every dialog are absent from the pictures of the screen. They
//! still show on the monitor as usual.

use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowDisplayAffinity, GetWindowThreadProcessId, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
    WINDOW_DISPLAY_AFFINITY,
};

struct Walk {
    pid: u32,
    changed: usize,
    all: usize,
}

unsafe extern "system" fn each(hwnd: HWND, data: LPARAM) -> BOOL {
    let walk = unsafe { &mut *(data.0 as *mut Walk) };
    let mut owner = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut owner)) };
    if owner == walk.pid {
        walk.all += 1;
        let mut current = 0u32;
        let _ = unsafe { GetWindowDisplayAffinity(hwnd, &mut current) };
        if current != WDA_EXCLUDEFROMCAPTURE.0
            && unsafe { SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(WDA_EXCLUDEFROMCAPTURE.0)) }.is_ok()
        {
            walk.changed += 1;
        }
    }
    BOOL(1)
}

/// Mark every top-level window of this program as excluded from capture. Windows made later are marked the next time this is
/// called. Returns how many windows were newly marked and how many this program has in all.
pub fn exclude_own_windows() -> (usize, usize) {
    let mut walk = Walk { pid: unsafe { GetCurrentProcessId() }, changed: 0, all: 0 };
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut walk as *mut Walk as isize));
    }
    (walk.changed, walk.all)
}

/// Whether one window is excluded from capture.
pub fn is_excluded(hwnd: HWND) -> bool {
    let mut current = 0u32;
    unsafe { GetWindowDisplayAffinity(hwnd, &mut current) }.is_ok() && current == WDA_EXCLUDEFROMCAPTURE.0
}
