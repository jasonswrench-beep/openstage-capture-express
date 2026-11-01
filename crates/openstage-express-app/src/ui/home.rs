// SPDX-License-Identifier: Apache-2.0
//! The Home screen (spec section 4.2): the title, the Start button, a search box and the list of guides. Until capturing is
//! built the list holds one example guide, made up for trying the editor, and the screen says so.

use eframe::egui::{self, RichText};
use openstage_express_export::demo::demo;

use super::rows::{step_count, steps_words};
use super::widgets::{self, palette};

/// One guide in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuideCard {
    pub title: String,
    pub steps: usize,
    /// A guide made up for trying the program, not one a person made.
    pub example: bool,
}

impl GuideCard {
    /// The words on the card, which are also its name for a screen reader.
    pub fn label(&self) -> String {
        let kind = if self.example { ", example guide" } else { "" };
        format!("{}, {}{kind}", self.title, steps_words(self.steps))
    }
}

pub struct HomeState {
    pub search: String,
    pub guides: Vec<GuideCard>,
}

impl HomeState {
    /// Home with the one example guide.
    pub fn example() -> HomeState {
        let guide = demo().document.guide;
        HomeState {
            search: String::new(),
            guides: vec![GuideCard { title: guide.title.clone(), steps: step_count(&guide), example: true }],
        }
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
}

pub fn show(ui: &mut egui::Ui, state: &mut HomeState) -> Action {
    let mut action = Action::None;
    egui::CentralPanel::default().show(ui, |ui| {
        let p = palette(ui);
        widgets::heading(ui, "OpenStage Capture Express");
        ui.label(RichText::new("Step-by-step guides from what you do on screen.").color(p.muted));
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if widgets::primary_button(ui, "Start capturing").clicked() {
                action = Action::StartCapturing;
            }
            ui.add_space(24.0);
            let label = ui.label("Search");
            let search = ui
                .add(egui::TextEdit::singleline(&mut state.search).hint_text("Search your guides").desired_width(320.0))
                .labelled_by(label.id);
            widgets::ring(ui, &search);
        });
        ui.add_space(14.0);
        widgets::subheading(ui, "Your guides");
        let visible = state.visible();
        if visible.is_empty() {
            ui.label(RichText::new("No guide matches what you typed.").color(p.muted));
        }
        for index in visible {
            let card = &state.guides[index];
            let response = widgets::card(ui, &card.title, &steps_words(card.steps), &card.label());
            if response.clicked() {
                action = Action::Open(index);
            }
        }
        ui.add_space(14.0);
        ui.label(
            RichText::new(
                "This guide is an example made up for trying the editor. Capturing your own guides is the next thing to be built.",
            )
            .color(p.muted),
        );
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
        assert_eq!(GuideCard { title: "One".into(), steps: 1, example: false }.label(), "One, 1 step");
    }

    #[test]
    fn the_search_lets_through_guides_that_have_all_the_words_in_any_case() {
        let mut home = HomeState::example();
        home.guides.push(GuideCard { title: "Set up two-step sign-in".into(), steps: 7, example: false });
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
