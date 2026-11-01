// SPDX-License-Identifier: Apache-2.0
//! The console box. The program is built as a console program so that its command-line parts (`export-demo`) can print to the
//! window they were started from. Started by a double-click or by `OPEN EXPRESS.bat`, that gives it a black console box
//! nobody wants behind the real window, so the window lets it go as soon as it has opened.

use windows::Win32::System::Console::FreeConsole;

/// Let go of this program's console. If a console started with the program, it closes. If the program was started from a
/// console that is still in use (a command prompt), that console is left alone and keeps waiting for the program to end.
pub fn detach() {
    // SAFETY: no arguments; it only changes which console this process is attached to.
    unsafe {
        let _ = FreeConsole();
    }
}
