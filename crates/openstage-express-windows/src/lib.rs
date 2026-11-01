// SPDX-License-Identifier: Apache-2.0
//! The Windows-only parts of OpenStage Capture Express.
//!
//! Planned contents (spec sections 6.2, 6.4 and 8, milestone M6): mouse and keyboard hooks, UI Automation lookups,
//! the frame ring, hotkeys and the capture bar's window handling, reusing OpenStage Capture's crates through a
//! path link to the sibling `openstage-author` folder. Nothing is built here yet, and on other systems this crate
//! will stay an empty stub so that `cargo test --workspace` still runs everywhere.
