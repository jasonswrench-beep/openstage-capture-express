// SPDX-License-Identifier: Apache-2.0
//! The Open a copy screen (spec A8): a guide that someone sent as one file (`Title.osguide.zip`), or that was saved with "Save a copy as
//! one file", is opened by naming the file. It becomes a new guide of the person's own in the guides folder; the file is only read.
//! The file can be chosen with Windows' own dialog (Browse) or its path typed or pasted.

use eframe::egui::{self, RichText};

use super::widgets::{self, palette};

/// What the screen holds between frames.
#[derive(Debug, Default)]
pub struct OpenCopyState {
    /// The path as typed or chosen.
    pub path: String,
    /// What the last try said, if it did not work.
    pub said: Option<String>,
    /// The keyboard goes to the box the first time the screen is drawn.
    focus_next: bool,
}

impl OpenCopyState {
    /// A fresh screen: nothing typed, the keyboard on the box.
    pub fn new() -> OpenCopyState {
        OpenCopyState { path: String::new(), said: None, focus_next: true }
    }

    /// The path as it should be used: Explorer's "Copy as path" puts quotation marks round it, and spaces at either end are slips.
    pub fn clean_path(&self) -> String {
        self.path.trim().trim_matches('"').trim().to_string()
    }
}

/// What the person asked for on this screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenCopyAction {
    None,
    Back,
    Browse,
    Open,
}

pub fn show(ui: &mut egui::Ui, state: &mut OpenCopyState) -> OpenCopyAction {
    let mut action = OpenCopyAction::None;
    egui::Panel::top("open-copy-top").show(ui, |ui| {
        ui.horizontal(|ui| {
            if widgets::button(ui, "Back to guides").clicked() {
                action = OpenCopyAction::Back;
            }
            widgets::heading(ui, "Open a copy");
        });
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let p = palette(ui);
        ui.label(
            RichText::new(
                "A copy is one file ending .osguide.zip that someone sent you, or that you saved with Save a copy as one file. Opening it \
                 makes a new guide of your own in your guides folder. The file itself is not changed.",
            )
            .color(p.muted),
        );
        ui.add_space(8.0);
        let label = ui.label("File to open");
        let field = ui
            .add(
                egui::TextEdit::singleline(&mut state.path)
                    .hint_text("For example C:\\Users\\You\\Downloads\\Guide.osguide.zip")
                    .desired_width(f32::INFINITY),
            )
            .labelled_by(label.id);
        widgets::ring(ui, &field);
        if std::mem::take(&mut state.focus_next) {
            field.request_focus();
        }
        // Enter in the box opens the copy.
        if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !state.clean_path().is_empty() {
            action = OpenCopyAction::Open;
        }
        ui.horizontal(|ui| {
            if widgets::button(ui, "Browse...").clicked() {
                action = OpenCopyAction::Browse;
            }
            let ready = !state.clean_path().is_empty();
            let go = ui.add_enabled_ui(ready, |ui| widgets::primary_button(ui, "Open the copy"));
            if go.inner.clicked() {
                action = OpenCopyAction::Open;
            }
        });
        if state.clean_path().is_empty() {
            ui.label(RichText::new("Type or paste the file's place, or choose it with Browse.").size(13.5).color(p.muted));
        }
        if let Some(said) = &state.said {
            ui.add_space(8.0);
            ui.label(RichText::new(said).color(p.text));
        }
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_copied_from_explorer_is_cleaned_of_its_quotation_marks_and_spaces() {
        let mut s = OpenCopyState::new();
        s.path = "  \"C:\\Users\\Me\\My guide.osguide.zip\" ".to_string();
        assert_eq!(s.clean_path(), "C:\\Users\\Me\\My guide.osguide.zip");
        s.path = "   ".to_string();
        assert_eq!(s.clean_path(), "");
    }
}
