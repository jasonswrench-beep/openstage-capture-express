// SPDX-License-Identifier: Apache-2.0
//! The program's screens and how they connect: Home, and the Editor opened from it. All of it draws into a `Ui` it is given, so
//! the tests can run the whole window without opening one.

use eframe::egui;
use openstage_express_export::demo::demo;

use super::editor::{self, Leave};
use super::editor_state::EditorState;
use super::home::{self, Action, HomeState};
use super::rows::step_count;
use super::widgets::{self, Status};

/// The height of the line at the foot of the window.
const STATUS_HEIGHT: f32 = 34.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Editor,
}

pub struct App {
    pub screen: Screen,
    pub home: HomeState,
    pub editor: Option<EditorState>,
    pub status: Status,
}

impl App {
    /// The program on its first screen (or, for a quick look, already in the editor).
    pub fn new(screen: Screen) -> App {
        let mut app = App { screen: Screen::Home, home: HomeState::example(), editor: None, status: Status::default() };
        if screen == Screen::Editor {
            app.open_example();
        }
        app
    }

    fn open_example(&mut self) {
        if self.editor.is_none() {
            self.editor = Some(EditorState::new(demo()));
        }
        self.screen = Screen::Editor;
        let steps = self.editor.as_ref().map(|e| step_count(&e.doc().guide)).unwrap_or(0);
        self.status.say(format!(
            "Opened the example guide, {}. It is made up, for trying the editor.",
            super::rows::steps_words(steps)
        ));
    }

    /// Draw the window: the screen, then the status line along the foot. The foot is drawn last so that a screen reader reaches it
    /// last, after everything the screen shows.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        let full = ui.available_rect_before_wrap();
        let split = (full.max.y - STATUS_HEIGHT).max(full.min.y);
        let main = egui::Rect::from_min_max(full.min, egui::pos2(full.max.x, split));
        let foot = egui::Rect::from_min_max(egui::pos2(full.min.x, split), full.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(main), |ui| self.draw_screen(ui));
        ui.scope_builder(egui::UiBuilder::new().max_rect(foot), |ui| {
            let border = widgets::palette(ui).border;
            ui.painter().hline(foot.x_range(), foot.min.y, egui::Stroke::new(1.0, border));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                widgets::status_line(ui, &self.status);
            });
        });
    }

    fn draw_screen(&mut self, ui: &mut egui::Ui) {
        match self.screen {
            Screen::Home => match home::show(ui, &mut self.home) {
                Action::None => {}
                Action::StartCapturing => {
                    self.status.say("Capturing is not built yet. You can try the editor on the example guide below.");
                }
                Action::Open(_) => self.open_example(),
            },
            Screen::Editor => {
                let Some(editor_state) = self.editor.as_mut() else {
                    self.screen = Screen::Home;
                    return;
                };
                if editor::show(ui, editor_state, &mut self.status) == Leave::ToHome {
                    if let Some(card) = self.home.guides.first_mut() {
                        card.steps = step_count(&editor_state.doc().guide);
                        card.title = editor_state.doc().guide.title.clone();
                    }
                    self.screen = Screen::Home;
                    self.status.say("Back at your guides.");
                }
            }
        }
    }
}
