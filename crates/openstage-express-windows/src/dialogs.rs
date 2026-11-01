// SPDX-License-Identifier: Apache-2.0
//! Windows' own dialog for choosing a folder. It is the system's, so it works with every screen reader, with the keyboard, and with
//! the person's favourite places, and Express draws nothing of it.
//!
//! Written and type-checked; it needs a person to open it, so it is listed in `TESTING-TODO.md`.

use std::path::{Path, PathBuf};

use windows::core::HSTRING;
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
use windows::Win32::UI::Shell::{
    FileOpenDialog, IFileOpenDialog, IShellItem, SHCreateItemFromParsingName, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS,
    SIGDN_FILESYSPATH,
};

/// Ask the person to choose a folder, starting at `start` if it exists. `None` if they cancelled or Windows would not show the
/// dialog. Call it from the thread that owns the window (the one that handles its messages).
pub fn choose_folder(start: &Path, title: &str) -> Option<PathBuf> {
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let options = dialog.GetOptions().ok()?;
        dialog.SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM).ok()?;
        let _ = dialog.SetTitle(&HSTRING::from(title));
        let mut existing = start;
        while !existing.exists() {
            match existing.parent() {
                Some(parent) => existing = parent,
                None => break,
            }
        }
        if let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(existing.as_os_str()), None) {
            let _ = dialog.SetFolder(&item);
        }
        // Owned by the window that has the keyboard, so that it comes up in front of it.
        let owner = GetActiveWindow();
        dialog.Show(if owner.0.is_null() { None } else { Some(owner) }).ok()?;
        let item = dialog.GetResult().ok()?;
        let name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = name.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(name.0 as *const _));
        path
    }
}
