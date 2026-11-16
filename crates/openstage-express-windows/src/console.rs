// SPDX-License-Identifier: Apache-2.0
//! The console. The program is a window program (Windows never gives it a black console box when it is started by a double-click, a
//! shortcut or `OPEN EXPRESS.bat`), but its command-line parts (`export-demo`, `check-computer`, `test-clicks`, `--help`) have to print
//! into the command prompt they were started from. [`attach_parent`] joins that prompt's console, and only does anything when there is
//! one, so a double-click still shows nothing but the window.

use windows::core::w;
use windows::Win32::Foundation::{GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_WRITE, OPEN_EXISTING};
use windows::Win32::System::Console::{
    AttachConsole, FreeConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE, STD_HANDLE,
    STD_OUTPUT_HANDLE,
};

/// Whether a standard handle is missing: a program built for windows has none of its own until it is given one.
fn is_missing(handle: windows::core::Result<HANDLE>) -> bool {
    match handle {
        Ok(h) => h.is_invalid() || h == INVALID_HANDLE_VALUE || h.0.is_null(),
        Err(_) => true,
    }
}

/// Join the console of the program that started this one (a command prompt, a `.bat` file), so that what the command-line parts print
/// shows there. Output that was sent to a file or a pipe (`> log.txt`) is left exactly as it is. With no console to join (Explorer started
/// the program), nothing happens.
pub fn attach_parent() {
    // SAFETY: only changes which console this process is attached to and the process's own standard handles.
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            return;
        }
        for which in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            give_console_if_missing(which);
        }
    }
}

/// # Safety
/// Calls Windows to open the console's output and to set the process's standard handle.
unsafe fn give_console_if_missing(which: STD_HANDLE) {
    if !is_missing(GetStdHandle(which)) {
        return;
    }
    if let Ok(console) =
        CreateFileW(w!("CONOUT$"), GENERIC_WRITE.0, FILE_SHARE_WRITE, None, OPEN_EXISTING, Default::default(), None)
    {
        let _ = SetStdHandle(which, console);
    }
}

/// Let go of this program's console. If a console started with the program, it closes. If the program was started from a
/// console that is still in use (a command prompt), that console is left alone and keeps waiting for the program to end.
pub fn detach() {
    // SAFETY: no arguments; it only changes which console this process is attached to.
    unsafe {
        let _ = FreeConsole();
    }
}
