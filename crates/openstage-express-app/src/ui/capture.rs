// SPDX-License-Identifier: Apache-2.0
//! What the window needs from capturing, as a small interface, so the screens can be run and tested without taking pictures of
//! anyone's screen. The real thing (hooks, UI Automation, Windows Graphics Capture) is in `capture_win.rs`; the tests use a stand-in.

use std::path::Path;

use openstage_express_core::{Document, GuideFolder};

use super::bar::{BarAction, BarView};
use crate::settings::AppSettings;

/// A key pressed anywhere in Windows that Express listens for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    StartOrFinish,
    PauseOrResume,
    CaptureNow,
}

/// A guide that has been captured and written.
pub struct Finished {
    pub folder: GuideFolder,
    pub doc: Document,
}

/// How a capture ended.
pub enum Outcome {
    Guide(Box<Finished>),
    /// Nothing was captured, so no guide was made.
    Nothing,
    Failed(String),
}

/// A capture in progress.
pub trait CaptureControl {
    /// What the bar shows now.
    fn view(&self) -> BarView;
    /// Pause, Resume, Finish or Capture now. Other actions are ignored.
    fn act(&mut self, action: BarAction);
    /// Take what has happened since the last call: the words to put in the status line (a screen reader says them).
    fn poll(&mut self) -> Vec<String>;
    /// `Some` once the capture has ended and the guide is written (or has failed to be).
    fn finished(&mut self) -> Option<Outcome>;
}

/// Starts a capture into the guides folder.
pub type Starter = Box<dyn FnMut(&Path, &AppSettings) -> Result<Box<dyn CaptureControl>, String>>;

/// The starter for a system that cannot capture.
pub fn cannot_capture() -> Starter {
    Box::new(|_, _| Err("Capturing needs Windows.".to_string()))
}
