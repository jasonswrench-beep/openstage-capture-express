// SPDX-License-Identifier: Apache-2.0
//! The capture bar (spec section 4.2): a small strip that stays on top while the person works in other programs. Its words are
//! what a screen reader says, and what the status line announces after each step ("Step 4 captured: Click Save").
//!
//! This file draws the bar and says what was pressed. What capturing does is in [`super::capture`].

use eframe::egui::{self, RichText};

use super::widgets::{self, palette};

/// What the bar is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarPhase {
    /// Waiting for Start.
    Ready,
    Capturing,
    Paused,
    /// Capturing has finished and the guide is being written.
    Saving,
}

/// What the bar shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarView {
    pub phase: BarPhase,
    pub steps: usize,
    /// The words of the step made last.
    pub last: Option<String>,
    /// What is being captured, for example "the whole screen (Display 1, 1920 by 1080, main display)".
    pub what: String,
    /// The hotkeys, in one sentence (empty if none could be set).
    pub hotkeys: String,
}

impl BarView {
    pub fn ready(what: &str, hotkeys: &str) -> BarView {
        BarView { phase: BarPhase::Ready, steps: 0, last: None, what: what.to_string(), hotkeys: hotkeys.to_string() }
    }

    /// The bar's main words, which are also read out.
    pub fn headline(&self) -> String {
        match self.phase {
            BarPhase::Ready => "Ready to capture".to_string(),
            BarPhase::Capturing => format!("Capturing, step {}", self.steps + 1),
            BarPhase::Paused => format!("Paused, {}", super::rows::steps_words(self.steps)),
            BarPhase::Saving => format!("Saving your guide, {}", super::rows::steps_words(self.steps)),
        }
    }
}

/// What the person pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarAction {
    None,
    Start,
    Cancel,
    Pause,
    Resume,
    Finish,
    CaptureNow,
}

/// Draw the bar. The first button gets the keyboard whenever the phase changes, so a person who starts it with the keyboard
/// can pause or finish it with the keyboard too.
pub fn show(ui: &mut egui::Ui, view: &BarView) -> BarAction {
    let mut action = BarAction::None;
    let p = palette(ui);
    let last_phase_id = ui.make_persistent_id("bar-phase");
    let phase_changed = ui.ctx().data_mut(|d| {
        let before = d.get_temp::<BarPhase>(last_phase_id);
        d.insert_temp(last_phase_id, view.phase);
        before != Some(view.phase)
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let headline = widgets::heading(ui, &view.headline());
        let _ = headline;
        if view.phase == BarPhase::Ready {
            ui.label(RichText::new(format!("Captures {}.", view.what)).color(p.muted));
        }
        if let Some(last) = &view.last {
            ui.label(RichText::new(format!("Last step: {last}")).color(p.muted));
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| match view.phase {
            BarPhase::Ready => {
                let start = widgets::primary_button(ui, "Start");
                if phase_changed {
                    start.request_focus();
                }
                if start.clicked() {
                    action = BarAction::Start;
                }
                if widgets::button(ui, "Cancel").clicked() {
                    action = BarAction::Cancel;
                }
            }
            BarPhase::Capturing => {
                let pause = widgets::button(ui, "Pause");
                if phase_changed {
                    pause.request_focus();
                }
                if pause.clicked() {
                    action = BarAction::Pause;
                }
                if widgets::button(ui, "Capture now").clicked() {
                    action = BarAction::CaptureNow;
                }
                if widgets::primary_button(ui, "Finish").clicked() {
                    action = BarAction::Finish;
                }
            }
            BarPhase::Paused => {
                let resume = widgets::primary_button(ui, "Resume");
                if phase_changed {
                    resume.request_focus();
                }
                if resume.clicked() {
                    action = BarAction::Resume;
                }
                if widgets::button(ui, "Finish").clicked() {
                    action = BarAction::Finish;
                }
            }
            BarPhase::Saving => {
                widgets::button_enabled(ui, false, "Saving...");
            }
        });
        if !view.hotkeys.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new(&view.hotkeys).color(p.muted));
        }
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_headline_says_the_state_in_words() {
        let mut v = BarView::ready("the whole screen", "");
        assert_eq!(v.headline(), "Ready to capture");
        v.phase = BarPhase::Capturing;
        v.steps = 3;
        assert_eq!(v.headline(), "Capturing, step 4");
        v.phase = BarPhase::Paused;
        assert_eq!(v.headline(), "Paused, 3 steps");
        v.phase = BarPhase::Saving;
        v.steps = 1;
        assert_eq!(v.headline(), "Saving your guide, 1 step");
    }
}
