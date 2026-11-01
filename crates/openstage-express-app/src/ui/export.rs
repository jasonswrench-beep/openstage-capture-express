// SPDX-License-Identifier: Apache-2.0
//! The Export screen (spec section 4.2): which formats, which folder, and a button. It draws the choices and says what was pressed;
//! making the exports is in `export_run.rs`, and it happens on a thread of its own so the window never freezes while a PDF is made.

use std::path::PathBuf;

use eframe::egui::{self, RichText};

use super::widgets::{self, palette};
use crate::export_run::{Format, Selection};

/// What the Export screen holds between frames.
pub struct ExportState {
    pub selection: Selection,
    /// The folder every export goes into (a folder for the guide, and in it a new folder for each run, are made inside it).
    pub folder: PathBuf,
    /// Open the new folder when the exports are done.
    pub open_after: bool,
    /// An export is being made now.
    pub working: bool,
    /// What the last export said.
    pub said: Option<String>,
    /// Whether this computer can make a PDF.
    pub can_pdf: bool,
    /// For a copy as one file: put the original pictures in it (off by default, so the marks are drawn in and a blur cannot be undone).
    pub copy_originals: bool,
}

impl ExportState {
    pub fn new(folder: PathBuf, can_pdf: bool) -> ExportState {
        let mut selection = Selection::starting();
        if !can_pdf {
            selection.set(Format::Pdf, false);
            selection.set(Format::Word, true);
        }
        ExportState { selection, folder, open_after: true, working: false, said: None, can_pdf, copy_originals: false }
    }
}

/// What the person asked for on this screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportAction {
    None,
    Back,
    ChooseFolder,
    Export,
    /// Save the guide as one file, a copy to send or keep.
    SaveCopy,
}

pub fn show(ui: &mut egui::Ui, state: &mut ExportState) -> ExportAction {
    let mut action = ExportAction::None;
    egui::Panel::top("export-top").show(ui, |ui| {
        ui.horizontal(|ui| {
            if widgets::button(ui, "Back to the guide").clicked() {
                action = ExportAction::Back;
            }
            widgets::heading(ui, "Export");
        });
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let p = palette(ui);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            widgets::subheading(ui, "Formats");
            ui.label(RichText::new("Each one you choose goes into a folder of its own.").color(p.muted));
            ui.add_space(4.0);
            ui.spacing_mut().item_spacing.y = 5.0;
            for format in Format::ALL {
                let mut on = state.selection.contains(format);
                let enabled = format != Format::Pdf || state.can_pdf;
                let about = if enabled {
                    format.about().to_string()
                } else {
                    "needs the Microsoft Edge WebView2 Runtime, which this computer does not have".to_string()
                };
                ui.horizontal(|ui| {
                    let response = ui.add_enabled(enabled, egui::Checkbox::new(&mut on, format.name()));
                    widgets::ring(ui, &response);
                    if response.changed() {
                        state.selection.set(format, on);
                    }
                    ui.label(RichText::new(about).size(13.5).color(p.muted));
                });
            }
            ui.add_space(10.0);
            widgets::subheading(ui, "Where to put them");
            let folder_words = state.folder.display().to_string();
            let shown = ui.label(RichText::new(&folder_words).color(p.text));
            let name = format!("Export folder: {folder_words}");
            ui.ctx().accesskit_node_builder(shown.id, |node| node.set_label(name));
            ui.horizontal(|ui| {
                if widgets::button(ui, "Choose folder...").clicked() {
                    action = ExportAction::ChooseFolder;
                }
            });
            let open = ui.add(egui::Checkbox::new(&mut state.open_after, "Open the folder when it is done"));
            widgets::ring(ui, &open);
            ui.add_space(12.0);
            let words = if state.working {
                "Exporting...".to_string()
            } else if state.selection.is_empty() {
                "Export".to_string()
            } else {
                format!("Export {}", format_count(state.selection.count()))
            };
            let go = ui.add_enabled_ui(!state.working && !state.selection.is_empty(), |ui| {
                widgets::primary_button(ui, &words)
            });
            if go.inner.clicked() {
                action = ExportAction::Export;
            }
            if state.selection.is_empty() {
                ui.label(RichText::new("Choose at least one format.").color(p.muted));
            }
            ui.add_space(14.0);
            widgets::subheading(ui, "A copy as one file");
            ui.label(
                RichText::new(
                    "Saves the whole guide as one file ending .osguide.zip, to send to someone or keep. It goes in the same folder as the \
                     exports. Someone opens it with Open a copy on Home.",
                )
                .color(p.muted),
            );
            let originals = ui.add(egui::Checkbox::new(&mut state.copy_originals, "Include the original pictures"));
            widgets::ring(ui, &originals);
            let kind = if state.copy_originals {
                "The original pictures are in the file, and the marks stay marks. A blur can be taken away by whoever opens it, and \
                 then the picture under it can be seen. Steps you skipped are included."
            } else {
                "Each picture has its marks drawn into it, blur included, and the original pictures are not in the file, so a blur \
                 cannot be undone. Steps you skipped are left out."
            };
            ui.label(RichText::new(format!("{kind} Deleted steps are never included.")).size(13.5).color(p.muted));
            let copy = ui.add_enabled_ui(!state.working, |ui| {
                widgets::button(ui, if state.working { "Working..." } else { "Save a copy as one file" })
            });
            if copy.inner.clicked() {
                action = ExportAction::SaveCopy;
            }
            if let Some(said) = &state.said {
                ui.add_space(8.0);
                ui.label(RichText::new(said).color(p.text));
            }
        });
    });
    action
}

fn format_count(n: usize) -> String {
    if n == 1 {
        "1 format".to_string()
    } else {
        format!("{n} formats")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_computer_without_a_pdf_engine_starts_with_word_instead() {
        let with = ExportState::new(PathBuf::from("x"), true);
        assert_eq!(with.selection.formats(), [Format::Html, Format::Pdf]);
        let without = ExportState::new(PathBuf::from("x"), false);
        assert_eq!(without.selection.formats(), [Format::Html, Format::Word]);
    }

    #[test]
    fn the_button_counts_in_words() {
        assert_eq!(format_count(1), "1 format");
        assert_eq!(format_count(4), "4 formats");
    }
}
