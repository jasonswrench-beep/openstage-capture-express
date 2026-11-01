// SPDX-License-Identifier: Apache-2.0
//! The Deleted steps screen (spec E8): the steps deleted from the open guide, which are kept with it and can be put back where they
//! were. Nothing is erased here. Restore puts one back; Restore all puts every one back as a single change.

use eframe::egui::{self, RichText};

use super::rows::{steps_words, DeletedRow};
use super::widgets::{self, palette};

/// What the person asked for on this screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeletedAction {
    None,
    Back,
    /// Put this step back.
    Restore(String),
    RestoreAll,
}

/// Where the keyboard goes on the next pass: at the Restore button now at this place in the list (or on Back if the list is empty).
/// The screen sets it after a step is put back, because the button that was pressed is gone.
pub type KeyboardAt = Option<usize>;

pub fn show(ui: &mut egui::Ui, rows: &[DeletedRow], keyboard: &mut KeyboardAt) -> DeletedAction {
    let mut action = DeletedAction::None;
    let go_to = keyboard.take();
    egui::Panel::top("deleted-top").show(ui, |ui| {
        ui.horizontal(|ui| {
            let back = widgets::button(ui, "Back to the steps");
            if back.clicked() {
                action = DeletedAction::Back;
            }
            if go_to.is_some() && rows.is_empty() {
                back.request_focus();
            }
            widgets::heading(ui, "Deleted steps");
        });
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let p = palette(ui);
        ui.label(
            RichText::new(
                "Steps you delete are kept with the guide, not erased. Restore puts a step back where it was. \
                 The step deleted last is first.",
            )
            .color(p.muted),
        );
        ui.add_space(6.0);
        if rows.is_empty() {
            ui.label(RichText::new("No steps are deleted.").color(p.text));
            return;
        }
        if rows.len() > 1 {
            let words = format!("Restore all {}", steps_words(rows.len()));
            if widgets::button(ui, &words).clicked() {
                action = DeletedAction::RestoreAll;
            }
            ui.add_space(6.0);
        }
        // The button comes first so that a long title can never push it out of the window.
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (index, row) in rows.iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    let name = format!("Restore {}", row.label());
                    let restore = widgets::button_named(ui, "Restore", &name);
                    if restore.clicked() {
                        action = DeletedAction::Restore(row.id.clone());
                    }
                    if go_to.is_some_and(|at| at.min(rows.len() - 1) == index) {
                        restore.request_focus();
                    }
                    ui.label(RichText::new(row.label()).strong().color(p.text));
                    ui.label(RichText::new(row.kind_words()).color(p.muted));
                });
            }
        });
    });
    action
}
