// SPDX-License-Identifier: Apache-2.0
//! The Deleted steps screen (spec E8): the steps deleted from the open guide, which are kept with it and can be put back where they
//! were. Restore puts one back; Restore all puts every one back as a single change. "Empty the trash" is the one thing here that
//! cannot be undone: it asks first, with "Keep them" ready, and then deletes the steps and their pictures for good.

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
    /// Open the question "Empty the trash for good?".
    AskToEmpty,
    /// Answer no to that question.
    KeepThem,
    /// Answer yes: delete every deleted step, and the pictures nothing else uses, for good.
    EmptyForGood,
}

/// What the screen remembers between passes.
#[derive(Debug, Default)]
pub struct DeletedView {
    /// Where the keyboard goes on the next pass: at the Restore button now at this place in the list (or on Back if the list is
    /// empty). Set after a step is put back, because the button that was pressed is gone.
    pub keyboard: Option<usize>,
    /// The question "Empty the trash for good?" is open.
    pub asking: bool,
    /// The keyboard goes to "Keep them" the first time the question is drawn, so that pressing Enter by accident keeps the steps.
    focus_keep: bool,
    /// The keyboard goes to the Empty the trash button on the next pass (after the answer was no).
    focus_empty: bool,
}

impl DeletedView {
    /// Open the question.
    pub fn ask(&mut self) {
        self.asking = true;
        self.focus_keep = true;
        self.focus_empty = false;
    }

    /// Close the question: the answer was no, or the person left, or the trash is empty. `to_button` puts the keyboard back on the
    /// button that opened it.
    pub fn stop_asking(&mut self, to_button: bool) {
        self.asking = false;
        self.focus_keep = false;
        self.focus_empty = to_button;
    }
}

pub fn show(ui: &mut egui::Ui, rows: &[DeletedRow], view: &mut DeletedView) -> DeletedAction {
    let mut action = DeletedAction::None;
    let go_to = view.keyboard.take();
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
        if view.asking && !rows.is_empty() {
            widgets::subheading(ui, "Empty the trash for good?");
            ui.label(
                RichText::new(format!(
                    "This deletes {} for good, and their pictures from the guide's folder. It cannot be undone, and Undo will not \
                     go back past it.",
                    steps_words(rows.len())
                ))
                .color(p.muted),
            );
            ui.horizontal_wrapped(|ui| {
                let keep = widgets::button(ui, "Keep them");
                if std::mem::take(&mut view.focus_keep) {
                    keep.request_focus();
                }
                if keep.clicked() {
                    action = DeletedAction::KeepThem;
                }
                if widgets::button(ui, "Empty the trash for good").clicked() {
                    action = DeletedAction::EmptyForGood;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    action = DeletedAction::KeepThem;
                }
            });
            return;
        }
        ui.label(
            RichText::new(
                "Steps you delete are kept with the guide, not erased, until you empty the trash. Restore puts a step back \
                 where it was. The step deleted last is first.",
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
        // The button comes first so that a long title can never push it out of the window. Room is kept below the list for the
        // button that empties the trash.
        egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(ui.available_height() - 52.0).show(ui, |ui| {
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
        ui.add_space(8.0);
        let empty = widgets::button(ui, "Empty the trash");
        if std::mem::take(&mut view.focus_empty) {
            empty.request_focus();
        }
        if empty.clicked() {
            action = DeletedAction::AskToEmpty;
        }
    });
    action
}
