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
pub mod console;
#[cfg(windows)]
pub mod pdf;
