// SPDX-License-Identifier: Apache-2.0
//! The program people run: `openstage-capture-express.exe`. On Windows it is a window program, so no black console box ever appears
//! behind its window. See `lib.rs` for what it does and for the console program the scripts use.

#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    openstage_express_app::run(true)
}
