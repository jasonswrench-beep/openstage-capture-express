// SPDX-License-Identifier: Apache-2.0
//! Two small questions for Windows that the rest of the program needs: where the person's Documents folder is (it may have been
//! moved, for example into OneDrive), and whether a process is still running (a guide folder whose lock names a process that is
//! gone was left by a crash).

use std::path::PathBuf;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::Shell::{FOLDERID_Documents, SHGetKnownFolderPath, KF_FLAG_DEFAULT};

/// The person's Documents folder as Windows has it, or `None` if Windows will not say.
/// Show a folder in File Explorer. Windows is asked to open it (ShellExecuteW), so no program is looked for by its bare name: one
/// named explorer.exe beside Express, in a folder others can write to, would otherwise run (the security review of 2026-11-13).
pub fn open_folder(folder: &std::path::Path) {
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    if !folder.is_dir() {
        return;
    }
    let path = HSTRING::from(folder.as_os_str());
    let _ = unsafe { ShellExecuteW(None, w!("open"), &path, PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
}

/// Open Windows' notification settings, where Do not disturb is turned on. Windows has no way for a program to turn it on itself.
pub fn open_notification_settings() {
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let _ = unsafe { ShellExecuteW(None, w!("open"), w!("ms-settings:notifications"), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
}

pub fn documents_folder() -> Option<PathBuf> {
    unsafe {
        let path = SHGetKnownFolderPath(&FOLDERID_Documents, KF_FLAG_DEFAULT, None).ok()?;
        let text = path.to_string().ok();
        CoTaskMemFree(Some(path.0 as *const _));
        text.map(PathBuf::from)
    }
}

/// The exit code Windows reports for a process that has not ended.
const STILL_ACTIVE: u32 = 259;

/// Whether the process with this id is running now. A process id that is zero, or one Windows will not let us look at, is
/// reported as not running only if it does not exist; a process we may not open but that exists counts as running.
pub fn process_is_running(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    unsafe {
        match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(handle) => {
                let mut code = 0u32;
                let alive = GetExitCodeProcess(handle, &mut code).is_ok() && code == STILL_ACTIVE;
                let _ = CloseHandle(handle);
                alive
            }
            // Access denied means the process exists; any other failure means there is no such process.
            Err(e) => e.code().0 & 0xFFFF == 5,
        }
    }
}

/// The local date and time as the person's clock shows it, for naming a guide: `2026-11-01 09.38` (no colon, which a folder name
/// cannot have).
pub fn local_time_stamp() -> String {
    let now = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:04}-{:02}-{:02} {:02}.{:02}", now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute)
}

/// How many bytes are free for this person on the drive that holds `folder` (the folder need not exist yet: its nearest
/// existing parent is asked). `None` if Windows will not say.
pub fn free_space(folder: &std::path::Path) -> Option<u64> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let mut existing = folder;
    while !existing.exists() {
        existing = existing.parent()?;
    }
    let wide = HSTRING::from(existing.as_os_str());
    let mut available = 0u64;
    unsafe { GetDiskFreeSpaceExW(&wide, Some(&mut available), None, None) }.ok()?;
    Some(available)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn some_space_is_free_where_the_temp_folder_is() {
        let free = free_space(&std::env::temp_dir()).expect("Windows says how much is free");
        assert!(free > 1024 * 1024, "{free}");
        let deeper = std::env::temp_dir().join("a folder that does not exist yet").join("nor this");
        assert!(free_space(&deeper).is_some(), "a folder that is not there yet is asked about by its nearest parent");
    }

    #[test]
    fn the_time_stamp_is_a_date_and_a_time_that_can_be_a_folder_name() {
        let stamp = local_time_stamp();
        assert_eq!(stamp.len(), 16, "{stamp}");
        let bytes = stamp.as_bytes();
        assert!(bytes[4] == b'-' && bytes[7] == b'-' && bytes[10] == b' ' && bytes[13] == b'.', "{stamp}");
        assert!(stamp.starts_with("20"), "{stamp}");
    }

    #[test]
    fn documents_is_a_folder_that_exists() {
        let docs = documents_folder().expect("Windows knows where Documents is");
        assert!(docs.is_dir(), "{docs:?}");
    }

    #[test]
    fn this_process_is_running_and_process_zero_is_not() {
        assert!(process_is_running(std::process::id()));
        assert!(!process_is_running(0));
    }

    #[test]
    fn a_finished_process_is_not_running() {
        let mut child = std::process::Command::new("cmd").args(["/c", "exit", "0"]).spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        assert!(!process_is_running(pid));
    }
}
