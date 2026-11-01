// SPDX-License-Identifier: Apache-2.0
//! The Windows-only parts of OpenStage Capture Express.
//!
//! Built so far: [`pdf`], the PDF engine, which drives the Microsoft WebView2 runtime that Windows ships, and [`console`],
//! which lets the window close the console box a double-clicked program starts with.
//!
//! Planned contents (spec sections 6.2, 6.4 and 8, milestone M6): mouse and keyboard hooks, UI Automation lookups,
//! the frame ring, hotkeys and the capture bar's window handling, reusing OpenStage Capture's crates through a
//! path link to the sibling `openstage-author` folder. On other systems this crate stays empty so that
//! `cargo test --workspace` still runs everywhere.

#[cfg(windows)]
pub mod affinity;
#[cfg(windows)]
pub mod console;
#[cfg(windows)]
pub mod dialogs;
#[cfg(windows)]
pub mod hotkeys;
#[cfg(windows)]
pub mod input;
#[cfg(windows)]
pub mod pdf;
#[cfg(windows)]
pub mod run;
#[cfg(windows)]
pub mod screen;
#[cfg(windows)]
pub mod sound;
#[cfg(windows)]
pub mod synthetic;
#[cfg(windows)]
pub mod system;
#[cfg(windows)]
pub mod testwin;
#[cfg(windows)]
pub mod uia;
