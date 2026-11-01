// SPDX-License-Identifier: Apache-2.0
//! What a capture takes pictures of (spec C3): the whole screen, one window, or an area drawn on a display. This is the person's
//! choice, in words and numbers; turning it into a capture is in `capture_win.rs`.

use serde::{Deserialize, Serialize};

/// Which of the three the person picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TargetKind {
    /// The whole of the main display.
    #[default]
    Screen,
    /// One window. The pictures are cut to its frame.
    Window,
    /// A rectangle drawn on a display.
    Area,
}

impl TargetKind {
    pub const ALL: [TargetKind; 3] = [TargetKind::Screen, TargetKind::Window, TargetKind::Area];

    /// The name on its radio button.
    pub fn name(self) -> &'static str {
        match self {
            TargetKind::Screen => "The whole screen",
            TargetKind::Window => "One window",
            TargetKind::Area => "An area I draw",
        }
    }
}

/// A window the person chose from the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowChoice {
    /// Windows' own number for it.
    pub handle: isize,
    pub title: String,
    /// The program, such as `notepad.exe`.
    pub process: String,
}

/// A rectangle of a display, in pixels from that display's top left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaChoice {
    /// 1 is the main display.
    pub display: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Everything the person has chosen about what to capture.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Target {
    pub kind: TargetKind,
    pub window: Option<WindowChoice>,
    pub area: Option<AreaChoice>,
}

impl Target {
    /// Whether a capture can start: a window or an area needs to have been chosen.
    pub fn is_ready(&self) -> bool {
        match self.kind {
            TargetKind::Screen => true,
            TargetKind::Window => self.window.is_some(),
            TargetKind::Area => self.area.is_some(),
        }
    }

    /// What will be captured, in words that finish "Captures ...". `screen` says what the whole screen is.
    pub fn words(&self, screen: &str) -> String {
        match self.kind {
            TargetKind::Screen => screen.to_string(),
            TargetKind::Window => match &self.window {
                Some(w) => format!("the window \"{}\" of {}", w.title, w.process),
                None => "no window yet. Choose one first".to_string(),
            },
            TargetKind::Area => match &self.area {
                Some(a) => format!(
                    "an area of Display {}: {} by {}, {} pixels from the left and {} from the top",
                    a.display, a.width, a.height, a.x, a.y
                ),
                None => "no area yet. Draw one first".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> WindowChoice {
        WindowChoice { handle: 77, title: "Untitled".into(), process: "notepad.exe".into() }
    }

    #[test]
    fn the_screen_is_always_ready_a_window_or_an_area_only_once_chosen() {
        let mut t = Target::default();
        assert!(t.is_ready());
        t.kind = TargetKind::Window;
        assert!(!t.is_ready());
        t.window = Some(window());
        assert!(t.is_ready());
        t.kind = TargetKind::Area;
        assert!(!t.is_ready(), "the window does not make the area ready");
        t.area = Some(AreaChoice { display: 1, x: 10, y: 20, width: 800, height: 600 });
        assert!(t.is_ready());
    }

    #[test]
    fn what_will_be_captured_is_said_in_words() {
        let screen = "the whole screen (Display 1)";
        let mut t = Target::default();
        assert_eq!(t.words(screen), screen);
        t.kind = TargetKind::Window;
        assert_eq!(t.words(screen), "no window yet. Choose one first");
        t.window = Some(window());
        assert_eq!(t.words(screen), "the window \"Untitled\" of notepad.exe");
        t.kind = TargetKind::Area;
        assert_eq!(t.words(screen), "no area yet. Draw one first");
        t.area = Some(AreaChoice { display: 2, x: 100, y: 50, width: 800, height: 600 });
        assert_eq!(t.words(screen), "an area of Display 2: 800 by 600, 100 pixels from the left and 50 from the top");
    }

    #[test]
    fn the_kinds_have_names_for_their_buttons() {
        let names: Vec<_> = TargetKind::ALL.iter().map(|k| k.name()).collect();
        assert_eq!(names, ["The whole screen", "One window", "An area I draw"]);
    }
}
