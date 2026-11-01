// SPDX-License-Identifier: Apache-2.0
//! Choosing the window to capture (spec C3): a list of the open windows, written out in words so it works with a screen reader and a
//! keyboard. Up and Down move through it, Enter or "Use this window" chooses.

use eframe::egui::{self, Key, Modifiers, RichText};

use super::capture::WindowRow;
use super::widgets::{self, palette};

/// What the list holds between frames.
#[derive(Debug, Default)]
pub struct WindowList {
    pub rows: Vec<WindowRow>,
    /// The row that is chosen (not yet used).
    pub selected: Option<usize>,
    /// Why the list could not be made, in words.
    pub problem: Option<String>,
    /// Set when Up, Down or a click chose a row: the keyboard goes to it.
    focus_next: bool,
}

impl WindowList {
    pub fn new(rows: Vec<WindowRow>, problem: Option<String>) -> WindowList {
        let selected = (!rows.is_empty()).then_some(0);
        WindowList { rows, selected, problem, focus_next: true }
    }
}

/// What the person asked for on this screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowListAction {
    None,
    Back,
    Refresh,
    /// Use the window at this place in the list.
    Use(usize),
}

pub fn show(ui: &mut egui::Ui, state: &mut WindowList) -> WindowListAction {
    let mut action = WindowListAction::None;
    egui::Panel::top("windows-top").show(ui, |ui| {
        ui.horizontal(|ui| {
            if widgets::button(ui, "Back").clicked() {
                action = WindowListAction::Back;
            }
            widgets::heading(ui, "Choose a window");
        });
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let p = palette(ui);
        ui.label(RichText::new("The windows that are open now. This program's own windows are not listed.").color(p.muted));
        if widgets::button(ui, "Refresh the list").clicked() {
            action = WindowListAction::Refresh;
        }
        if let Some(problem) = &state.problem {
            ui.label(RichText::new(problem).color(p.text));
        }
        if state.rows.is_empty() && state.problem.is_none() {
            ui.label(RichText::new("No window is open that can be captured. Open the program you want to explain, then refresh the list.").color(p.text));
        }
        let take_keyboard = std::mem::take(&mut state.focus_next);
        let mut chosen: Option<usize> = None;
        let mut keyboard_on_selected = false;
        egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(ui.available_height() - 60.0).show(ui, |ui| {
            for (i, row) in state.rows.iter().enumerate() {
                let is_selected = state.selected == Some(i);
                let response = widgets::list_row(ui, &row.words, is_selected, false, 0.0, take_keyboard);
                if response.clicked() && !is_selected {
                    chosen = Some(i);
                    state.focus_next = true;
                }
                if response.double_clicked() {
                    action = WindowListAction::Use(i);
                }
                keyboard_on_selected |= is_selected && response.has_focus();
                if response.has_focus() {
                    response.scroll_to_me(None);
                }
            }
        });
        if keyboard_on_selected {
            let (up, down, enter) = ui.ctx().input_mut(|i| {
                (
                    i.consume_key(Modifiers::NONE, Key::ArrowUp),
                    i.consume_key(Modifiers::NONE, Key::ArrowDown),
                    i.consume_key(Modifiers::NONE, Key::Enter),
                )
            });
            let at = state.selected.unwrap_or(0);
            if up && at > 0 {
                chosen = Some(at - 1);
                state.focus_next = true;
            } else if down && at + 1 < state.rows.len() {
                chosen = Some(at + 1);
                state.focus_next = true;
            }
            if enter {
                action = WindowListAction::Use(at);
            }
        }
        if let Some(i) = chosen {
            state.selected = Some(i);
        }
        ui.add_space(6.0);
        let go = ui.add_enabled_ui(state.selected.is_some(), |ui| widgets::primary_button(ui, "Use this window"));
        if go.inner.clicked() {
            if let Some(i) = state.selected {
                action = WindowListAction::Use(i);
            }
        }
    });
    action
}
