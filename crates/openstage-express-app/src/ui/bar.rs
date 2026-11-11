// SPDX-License-Identifier: Apache-2.0
//! The capture bar (spec section 4.2): a small strip that stays on top while the person works in other programs. Its words are
//! what a screen reader says, and what the status line announces after each step ("Step 4 captured: Click Save").
//!
//! This file draws the bar and says what was pressed. What capturing does is in [`super::capture`].

use eframe::egui::{self, RichText};

use super::icon::UiIcon;
use super::widgets::{self, palette};
use crate::target::TargetKind;

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
    /// Which of the three kinds of thing to capture is chosen (the choice is open only before Start).
    pub target: TargetKind,
    /// Whether Start can be pressed: a window or an area must have been chosen.
    pub can_start: bool,
    /// The hotkeys, in one sentence (empty if none could be set).
    pub hotkeys: String,
}

impl BarView {
    pub fn ready(what: &str, hotkeys: &str) -> BarView {
        BarView {
            phase: BarPhase::Ready,
            steps: 0,
            last: None,
            what: what.to_string(),
            target: TargetKind::Screen,
            can_start: true,
            hotkeys: hotkeys.to_string(),
        }
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
    /// Capture the whole screen, one window or an area.
    SetTarget(TargetKind),
    ChooseWindow,
    ChooseArea,
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
    // The suite's navy strip along the top, with what the capture is doing: a red dot while it captures, the pause sign while it is
    // paused (each with its words, never the colour alone).
    let strip = egui::Frame::new().fill(p.shell).inner_margin(egui::Margin { left: 16, right: 16, top: 6, bottom: 6 });
    egui::Panel::top("bar-strip").frame(strip).show(ui, |ui| {
        ui.horizontal(|ui| {
            let (at, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
            match view.phase {
                BarPhase::Capturing => {
                    ui.painter().circle_filled(at.center(), 7.0, egui::Color32::from_rgb(0xE5, 0x48, 0x4D));
                }
                BarPhase::Paused => super::icon::paint(ui.painter(), at, UiIcon::Pause, p.on_shell),
                BarPhase::Ready | BarPhase::Saving => super::icon::paint(ui.painter(), at, UiIcon::Camera, p.brand),
            }
            widgets::heading_in(ui, &view.headline(), p.on_shell);
        });
    });
    let page = egui::Frame::new().fill(p.page).inner_margin(egui::Margin { left: 16, right: 16, top: 10, bottom: 8 });
    egui::CentralPanel::default().frame(page).show(ui, |ui| {
        if view.phase == BarPhase::Ready {
            ui.label(RichText::new(format!("Captures {}.", view.what)).color(p.muted));
        }
        if let Some(last) = &view.last {
            ui.label(RichText::new(format!("Last step: {last}")).color(p.muted));
        }
        if view.phase == BarPhase::Ready {
            ui.label(RichText::new("What to capture:").color(p.muted));
            ui.horizontal_wrapped(|ui| {
                for kind in TargetKind::ALL {
                    let r = ui.add(egui::RadioButton::new(view.target == kind, kind.name()));
                    widgets::ring(ui, &r);
                    if r.clicked() && view.target != kind {
                        action = BarAction::SetTarget(kind);
                    }
                }
            });
            let choose = match view.target {
                TargetKind::Screen => None,
                TargetKind::Window => Some(("Choose window...", BarAction::ChooseWindow)),
                TargetKind::Area => Some(("Draw the area...", BarAction::ChooseArea)),
            };
            if let Some((words, choice)) = choose {
                if widgets::icon_button(ui, target_icon(view.target), words).clicked() {
                    action = choice;
                }
            }
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| match view.phase {
            BarPhase::Ready => {
                let start = if view.can_start {
                    widgets::primary_icon_button(ui, UiIcon::CircleDot, "Start")
                } else {
                    widgets::button_enabled(ui, false, "Start")
                };
                if phase_changed {
                    start.request_focus();
                }
                if start.clicked() {
                    action = BarAction::Start;
                }
                if widgets::icon_button(ui, UiIcon::X, "Cancel").clicked() {
                    action = BarAction::Cancel;
                }
            }
            BarPhase::Capturing => {
                let pause = widgets::icon_button(ui, UiIcon::Pause, "Pause");
                if phase_changed {
                    pause.request_focus();
                }
                if pause.clicked() {
                    action = BarAction::Pause;
                }
                if widgets::icon_button(ui, UiIcon::Camera, "Capture now").clicked() {
                    action = BarAction::CaptureNow;
                }
                if widgets::primary_icon_button(ui, UiIcon::CircleStop, "Finish").clicked() {
                    action = BarAction::Finish;
                }
            }
            BarPhase::Paused => {
                let resume = widgets::primary_icon_button(ui, UiIcon::Play, "Resume");
                if phase_changed {
                    resume.request_focus();
                }
                if resume.clicked() {
                    action = BarAction::Resume;
                }
                if widgets::icon_button(ui, UiIcon::CircleStop, "Finish").clicked() {
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

/// The icon of each thing that can be captured.
fn target_icon(kind: TargetKind) -> UiIcon {
    match kind {
        TargetKind::Screen => UiIcon::Monitor,
        TargetKind::Window => UiIcon::AppWindow,
        TargetKind::Area => UiIcon::Scan,
    }
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
