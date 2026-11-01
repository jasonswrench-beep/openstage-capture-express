// SPDX-License-Identifier: Apache-2.0
//! The Home screen (spec section 4.2): the title, the Start button, a search box and the list of guides. The list holds the guides
//! found in the guides folder, newest first; when there are none it holds one example guide, made up for trying the editor, and
//! the screen says so.

use std::path::PathBuf;

use eframe::egui::{self, RichText};
use openstage_express_export::demo::demo;

use crate::library::Entry;

use super::rows::{step_count, steps_words};
use super::widgets::{self, palette};

/// One guide in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuideCard {
    pub title: String,
    pub steps: usize,
    /// A guide made up for trying the program, not one a person made.
    pub example: bool,
    /// Where the guide's folder is (the example has none).
    pub path: Option<PathBuf>,
}

impl GuideCard {
    /// The words on the card, which are also its name for a screen reader.
    pub fn label(&self) -> String {
        let kind = if self.example { ", example guide" } else { "" };
        format!("{}, {}{kind}", self.title, steps_words(self.steps))
    }
}

/// A guide being renamed: which one, and the name being typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renaming {
    pub path: PathBuf,
    /// The name it has now.
    pub title: String,
    /// What is typed in the box (it starts as the name it has now).
    pub text: String,
    /// The keyboard goes to the box the first time it is drawn.
    focus_next: bool,
}

/// A guide the person has asked to delete, which waits for them to say yes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deleting {
    pub path: PathBuf,
    pub title: String,
    /// The keyboard goes to "Keep it" the first time it is drawn, so that pressing Enter by accident keeps the guide.
    focus_next: bool,
}

pub struct HomeState {
    pub search: String,
    pub guides: Vec<GuideCard>,
    pub renaming: Option<Renaming>,
    pub deleting: Option<Deleting>,
}

impl HomeState {
    /// Home with the one example guide.
    pub fn example() -> HomeState {
        let guide = demo().document.guide;
        HomeState {
            search: String::new(),
            guides: vec![GuideCard {
                title: guide.title.clone(),
                steps: step_count(&guide),
                example: true,
                path: None,
            }],
            renaming: None,
            deleting: None,
        }
    }

    /// Begin renaming the guide at this place in the list. Nothing happens for the example, which is not a guide on disk.
    pub fn start_renaming(&mut self, index: usize) -> bool {
        let Some(card) = self.guides.get(index) else { return false };
        let Some(path) = card.path.clone() else { return false };
        self.deleting = None;
        self.renaming = Some(Renaming { path, title: card.title.clone(), text: card.title.clone(), focus_next: true });
        true
    }

    /// Ask whether to delete the guide at this place in the list.
    pub fn start_deleting(&mut self, index: usize) -> bool {
        let Some(card) = self.guides.get(index) else { return false };
        let Some(path) = card.path.clone() else { return false };
        self.renaming = None;
        self.deleting = Some(Deleting { path, title: card.title.clone(), focus_next: true });
        true
    }

    /// Home over the guides found in the guides folder, or over the example guide if there are none. Whatever was typed in the
    /// search box is kept.
    pub fn with_guides(&mut self, entries: &[Entry]) {
        // A rename or a delete waiting for an answer is dropped when the guide it was about is no longer in the list.
        let still_there = |path: &PathBuf| entries.iter().any(|e| &e.path == path);
        if self.renaming.as_ref().is_some_and(|r| !still_there(&r.path)) {
            self.renaming = None;
        }
        if self.deleting.as_ref().is_some_and(|d| !still_there(&d.path)) {
            self.deleting = None;
        }
        if entries.is_empty() {
            let search = std::mem::take(&mut self.search);
            *self = HomeState::example();
            self.search = search;
        } else {
            self.guides = entries
                .iter()
                .map(|e| GuideCard {
                    title: e.title.clone(),
                    steps: e.steps,
                    example: false,
                    path: Some(e.path.clone()),
                })
                .collect();
        }
    }

    /// Whether the list is the example guide.
    pub fn shows_example(&self) -> bool {
        self.guides.iter().any(|g| g.example)
    }

    /// The guides the search lets through (all of them when it is empty), as places in `guides`.
    pub fn visible(&self) -> Vec<usize> {
        let words: Vec<String> = self.search.split_whitespace().map(str::to_lowercase).collect();
        self.guides
            .iter()
            .enumerate()
            .filter(|(_, g)| {
                let title = g.title.to_lowercase();
                words.iter().all(|w| title.contains(w))
            })
            .map(|(i, _)| i)
            .collect()
    }
}

/// What the person asked for on this screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Open(usize),
    StartCapturing,
    Settings,
    About,
    /// Look at the Trash.
    Trash,
    /// Begin renaming the guide at this place in the list.
    Rename(usize),
    /// Use the name that was typed.
    SaveName,
    CancelRename,
    /// Ask whether to move the guide at this place in the list to the Trash.
    Delete(usize),
    /// Yes, move it.
    ConfirmDelete,
    /// No, keep it.
    KeepGuide,
}

/// What a card takes of the width and what is left for the Rename and Delete buttons beside it.
const BUTTONS_WIDTH: f32 = 220.0;

pub fn show(ui: &mut egui::Ui, state: &mut HomeState) -> Action {
    let mut action = Action::None;
    egui::CentralPanel::default().show(ui, |ui| {
        let p = palette(ui);
        widgets::heading(ui, "OpenStage Capture Express");
        ui.label(RichText::new("Step-by-step guides from what you do on screen.").color(p.muted));
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            if widgets::primary_button(ui, "Start capturing").clicked() {
                action = Action::StartCapturing;
            }
            ui.add_space(24.0);
            let label = ui.label("Search");
            let search = ui
                .add(egui::TextEdit::singleline(&mut state.search).hint_text("Search your guides").desired_width(280.0))
                .labelled_by(label.id);
            widgets::ring(ui, &search);
            ui.add_space(24.0);
            if widgets::button(ui, "Settings").clicked() {
                action = Action::Settings;
            }
            if widgets::button(ui, "About and licences").clicked() {
                action = Action::About;
            }
            if widgets::button(ui, "Trash").clicked() {
                action = Action::Trash;
            }
        });
        ui.add_space(14.0);
        widgets::subheading(ui, "Your guides");
        if let Some(rename) = state.renaming.as_mut() {
            let shown = format!("Rename \"{}\"", rename.title);
            widgets::subheading(ui, &shown);
            ui.horizontal_wrapped(|ui| {
                let label = ui.label(format!("New name for {}", rename.title));
                let field = ui
                    .add(egui::TextEdit::singleline(&mut rename.text).desired_width(360.0))
                    .labelled_by(label.id);
                widgets::ring(ui, &field);
                if std::mem::take(&mut rename.focus_next) {
                    field.request_focus();
                }
                // Enter in the box saves the name, and Escape in it cancels.
                let (enter, escape) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
                if field.lost_focus() && enter {
                    action = Action::SaveName;
                } else if field.lost_focus() && escape {
                    action = Action::CancelRename;
                }
                if widgets::primary_button(ui, "Save name").clicked() {
                    action = Action::SaveName;
                }
                if widgets::button(ui, "Cancel").clicked() {
                    action = Action::CancelRename;
                }
            });
            ui.add_space(8.0);
        }
        if let Some(delete) = state.deleting.as_mut() {
            let shown = format!("Move \"{}\" to the Trash?", delete.title);
            widgets::subheading(ui, &shown);
            ui.label(
                RichText::new("Nothing is erased. You can bring it back from the Trash, which is a folder next to your guides.")
                    .color(p.muted),
            );
            ui.horizontal_wrapped(|ui| {
                let keep = widgets::button(ui, "Keep it");
                if std::mem::take(&mut delete.focus_next) {
                    keep.request_focus();
                }
                if keep.clicked() {
                    action = Action::KeepGuide;
                }
                if widgets::button(ui, "Move to the Trash").clicked() {
                    action = Action::ConfirmDelete;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    action = Action::KeepGuide;
                }
            });
            ui.add_space(8.0);
        }
        let visible = state.visible();
        if visible.is_empty() {
            ui.label(RichText::new("No guide matches what you typed.").color(p.muted));
        }
        for index in visible {
            let card = &state.guides[index];
            ui.horizontal(|ui| {
                let most = if card.path.is_some() { (ui.available_width() - BUTTONS_WIDTH).clamp(240.0, 760.0) } else { 760.0 };
                let response = widgets::card(ui, &card.title, &steps_words(card.steps), &card.label(), most);
                if response.clicked() {
                    action = Action::Open(index);
                }
                // A guide on disk can be renamed or deleted; the example cannot. Each button says whose it is.
                if card.path.is_some() {
                    if widgets::button_named(ui, "Rename", &format!("Rename {}", card.title)).clicked() {
                        action = Action::Rename(index);
                    }
                    if widgets::button_named(ui, "Delete", &format!("Delete {}", card.title)).clicked() {
                        action = Action::Delete(index);
                    }
                }
            });
        }
        ui.add_space(14.0);
        if state.shows_example() {
            ui.label(
                RichText::new(
                    "This guide is an example made up for trying the editor. Click Start capturing, then do the task you want to explain.",
                )
                .color(p.muted),
            );
        }
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_starts_with_the_one_example_guide_and_says_what_it_is() {
        let home = HomeState::example();
        assert_eq!(home.guides.len(), 1);
        assert_eq!(home.guides[0].title, "How to submit an assignment");
        assert_eq!(home.guides[0].steps, 5);
        assert!(home.guides[0].example);
        assert_eq!(home.guides[0].label(), "How to submit an assignment, 5 steps, example guide");
        assert_eq!(GuideCard { title: "One".into(), steps: 1, example: false, path: None }.label(), "One, 1 step");
    }

    fn with_two_real_guides() -> HomeState {
        let mut home = HomeState::example();
        home.with_guides(&[
            Entry { path: PathBuf::from("a.osguide"), title: "Alpha".into(), steps: 2, updated_at: "2".into() },
            Entry { path: PathBuf::from("b.osguide"), title: "Beta".into(), steps: 3, updated_at: "1".into() },
        ]);
        home
    }

    #[test]
    fn a_rename_or_a_delete_is_about_one_guide_on_disk_and_never_the_example() {
        let mut example = HomeState::example();
        assert!(!example.start_renaming(0) && !example.start_deleting(0), "the example is not a guide on disk");
        assert!(example.renaming.is_none() && example.deleting.is_none());
        let mut home = with_two_real_guides();
        assert!(!home.start_renaming(7) && !home.start_deleting(7), "no such guide");
        assert!(home.start_renaming(1));
        let rename = home.renaming.clone().unwrap();
        assert_eq!(
            (rename.path, rename.title.as_str(), rename.text.as_str()),
            (PathBuf::from("b.osguide"), "Beta", "Beta")
        );
        assert!(home.start_deleting(0));
        assert!(home.renaming.is_none(), "asking to delete drops the rename: one question at a time");
        assert_eq!(home.deleting.as_ref().unwrap().path, PathBuf::from("a.osguide"));
        assert!(home.start_renaming(0));
        assert!(home.deleting.is_none(), "and the other way round");
    }

    #[test]
    fn a_question_about_a_guide_that_has_gone_from_the_list_is_dropped() {
        let mut home = with_two_real_guides();
        home.start_deleting(1);
        home.with_guides(&[Entry {
            path: PathBuf::from("a.osguide"),
            title: "Alpha".into(),
            steps: 2,
            updated_at: "2".into(),
        }]);
        assert!(home.deleting.is_none(), "Beta was moved away, so nothing is left to ask about");
        home.start_renaming(0);
        home.with_guides(&[Entry {
            path: PathBuf::from("a.osguide"),
            title: "Alpha 2".into(),
            steps: 2,
            updated_at: "3".into(),
        }]);
        assert!(home.renaming.is_some(), "the guide is still there, so the rename goes on");
        home.with_guides(&[]);
        assert!(home.renaming.is_none() && home.shows_example());
    }

    #[test]
    fn the_search_lets_through_guides_that_have_all_the_words_in_any_case() {
        let mut home = HomeState::example();
        home.guides.push(GuideCard { title: "Set up two-step sign-in".into(), steps: 7, example: false, path: None });
        assert_eq!(home.visible(), [0, 1]);
        home.search = "  SUBMIT ".into();
        assert_eq!(home.visible(), [0]);
        home.search = "set sign".into();
        assert_eq!(home.visible(), [1]);
        home.search = "submit two-step".into();
        assert!(home.visible().is_empty(), "every word must be in the title");
        home.search = "nothing like it".into();
        assert!(home.visible().is_empty());
        home.search = "   ".into();
        assert_eq!(home.visible(), [0, 1], "only spaces is the same as no search");
    }
}
