// SPDX-License-Identifier: Apache-2.0
//! What the window needs from capturing, as a small interface, so the screens can be run and tested without taking pictures of
//! anyone's screen. The real thing (hooks, UI Automation, Windows Graphics Capture) is in `capture_win.rs`; the tests use a stand-in.

use std::path::Path;

use openstage_express_core::capture::Picture;
use openstage_express_core::cues::Cue;
use openstage_express_core::{Document, GuideFolder};

use super::bar::{BarAction, BarView};
use crate::settings::AppSettings;
use crate::target::Target;

/// A key pressed anywhere in Windows that Express listens for.
pub use openstage_express_core::hotkey::HotkeyAction;

/// Plays the cue sounds. The real one is in `capture_win.rs`; the tests use one that only writes down what it was asked to play, so
/// that nothing is ever heard from a test.
pub trait CuePlayer {
    fn play(&self, cue: Cue);
}

/// A player for a system that cannot play sounds, or a run that must stay silent.
pub struct NoCues;

impl CuePlayer for NoCues {
    fn play(&self, _: Cue) {}
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
    /// Whether something went wrong or needs a look since the last call (a step that could not be saved, a window that closed). It
    /// is what the problem sound is for. Most stand-ins never have trouble.
    fn trouble(&mut self) -> bool {
        false
    }
}

/// Starts a capture into the guides folder, of what the person chose to capture.
pub type Starter = Box<dyn FnMut(&Path, &AppSettings, &Target) -> Result<Box<dyn CaptureControl>, String>>;

/// The starter for a system that cannot capture.
pub fn cannot_capture() -> Starter {
    Box::new(|_, _, _| Err("Capturing needs Windows.".to_string()))
}

/// One open window in the list the person chooses from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowRow {
    pub handle: isize,
    pub title: String,
    pub process: String,
    /// What the list shows and a screen reader says.
    pub words: String,
}

/// One display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayRow {
    /// 1 is the main display.
    pub number: u32,
    pub width: u32,
    pub height: u32,
    pub words: String,
}

/// What the window needs to know about the desktop to let the person choose what to capture.
pub trait Desktop {
    /// The open windows, the one in front first, without this program's own.
    fn windows(&self) -> Result<Vec<WindowRow>, String>;
    fn displays(&self) -> Result<Vec<DisplayRow>, String>;
    /// A picture of one display as it is now (this program's own windows are not in it).
    fn snapshot(&self, display: u32) -> Result<Picture, String>;
}

/// A made-up desktop (a few windows, one display, a drawn picture), for looking at the choosing screens without showing anyone's real
/// windows. Used by `--screen choose-window` and `--screen choose-area`.
pub struct DemoDesktop;

impl Desktop for DemoDesktop {
    fn windows(&self) -> Result<Vec<WindowRow>, String> {
        let row = |handle: isize, title: &str, process: &str, size: &str| WindowRow {
            handle,
            title: title.to_string(),
            process: process.to_string(),
            words: format!("{process}: {title} ({size})"),
        };
        Ok(vec![
            row(1, "Course site: Assignments", "browser.exe", "1600 by 900"),
            row(2, "Untitled", "notepad.exe", "800 by 600"),
            row(3, "Grades", "spreadsheet.exe", "1280 by 720"),
        ])
    }

    fn displays(&self) -> Result<Vec<DisplayRow>, String> {
        Ok(vec![DisplayRow {
            number: 1,
            width: 1920,
            height: 1080,
            words: "Display 1, 1920 by 1080, main display".to_string(),
        }])
    }

    fn snapshot(&self, _: u32) -> Result<Picture, String> {
        let (w, h) = (1920u32, 1080u32);
        let mut bgra = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let (title_bar, panel) = (y < 60, (300..1500).contains(&x) && (200..900).contains(&y));
                let shade = if title_bar {
                    60
                } else if panel {
                    235
                } else {
                    150 + (y / 12) as u8
                };
                bgra.extend_from_slice(&[shade, shade.saturating_sub(10), shade.saturating_sub(30), 255]);
            }
        }
        Picture::new(w, h, bgra).ok_or_else(|| "no picture".to_string())
    }
}

/// The desktop of a system that cannot capture.
pub struct NoDesktop;

impl Desktop for NoDesktop {
    fn windows(&self) -> Result<Vec<WindowRow>, String> {
        Err("Choosing a window needs Windows.".to_string())
    }

    fn displays(&self) -> Result<Vec<DisplayRow>, String> {
        Err("Choosing an area needs Windows.".to_string())
    }

    fn snapshot(&self, _: u32) -> Result<Picture, String> {
        Err("A picture of the screen needs Windows.".to_string())
    }
}
