// SPDX-License-Identifier: Apache-2.0
//! The Trash (spec A1): the guides that were deleted from Home, which Express only moved to a folder called `Trash` inside the guides
//! folder. Each can be brought back. Nothing is erased here: to get rid of one for good, delete its folder in Explorer, which the
//! button "Show the Trash folder" opens.

use eframe::egui::{self, RichText};

use crate::library::Entry;

use super::rows::steps_words;
use super::widgets::{self, palette};

/// The guides in the Trash.
#[derive(Debug, Default)]
pub struct TrashState {
    pub entries: Vec<Entry>,
}

/// What the person asked for on this screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrashAction {
    None,
    Back,
    /// Bring back the guide at this place in the list.
    Restore(usize),
    /// Show the Trash folder in Explorer.
    ShowFolder,
}

pub fn show(ui: &mut egui::Ui, state: &TrashState) -> TrashAction {
    let mut action = TrashAction::None;
    egui::Panel::top("trash-top").show(ui, |ui| {
        ui.horizontal(|ui| {
            if widgets::button(ui, "Back to guides").clicked() {
                action = TrashAction::Back;
            }
            widgets::heading(ui, "Trash");
        });
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let p = palette(ui);
        ui.label(
            RichText::new(
                "Guides you delete are moved here, not erased. They stay until you delete them yourself in Explorer.",
            )
            .color(p.muted),
        );
        if widgets::button(ui, "Show the Trash folder").clicked() {
            action = TrashAction::ShowFolder;
        }
        ui.add_space(10.0);
        if state.entries.is_empty() {
            ui.label(RichText::new("The Trash is empty.").color(p.text));
        }
        for (index, entry) in state.entries.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&entry.title).strong().color(p.text));
                ui.label(RichText::new(steps_words(entry.steps)).color(p.muted));
                if widgets::button_named(ui, "Restore", &format!("Restore {}", entry.title)).clicked() {
                    action = TrashAction::Restore(index);
                }
            });
        }
    });
    action
}
