// SPDX-License-Identifier: Apache-2.0
//! The program's screens and how they connect: Home, the capture bar, and the Editor. All of it draws into a `Ui` it is given, so
//! the tests can run the whole window without opening one.

use std::path::{Path, PathBuf};

use eframe::egui;
use openstage_express_core::{Access, GuideFolder};
use openstage_express_export::demo::demo;

use super::bar::{self, BarAction, BarView};
use super::capture::{cannot_capture, CaptureControl, HotkeyAction, Outcome, Starter};
use super::editor::{self, Leave};
use super::editor_state::EditorState;
use super::home::{self, Action, HomeState};
use super::rows::{step_count, steps_words};
use super::widgets::{self, Status};
use crate::library;

/// The height of the line at the foot of the window.
const STATUS_HEIGHT: f32 = 34.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Editor,
    /// The capture bar: the window is a small strip that stays on top while the person works in other programs.
    Bar,
}

/// How the window itself should look: the whole program, or the strip of the capture bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMode {
    Normal,
    Bar,
}

pub struct App {
    pub screen: Screen,
    pub home: HomeState,
    pub editor: Option<EditorState>,
    pub status: Status,
    library: PathBuf,
    starter: Starter,
    capture: Option<Box<dyn CaptureControl>>,
    /// What the bar says it will capture, before Start.
    what: String,
    /// The hotkeys in a sentence, for the bar.
    hotkey_words: String,
    is_running: Box<dyn Fn(u32) -> bool>,
}

impl App {
    /// The program on its first screen (or, for a quick look, already in the editor or on the bar). It cannot capture and has no
    /// guides folder until [`App::with_capture`] is called.
    pub fn new(screen: Screen) -> App {
        let mut app = App {
            screen: Screen::Home,
            home: HomeState::example(),
            editor: None,
            status: Status::default(),
            library: PathBuf::new(),
            starter: cannot_capture(),
            capture: None,
            what: "the whole screen".to_string(),
            hotkey_words: String::new(),
            is_running: Box::new(|_| false),
        };
        match screen {
            Screen::Editor => app.open_example(),
            Screen::Bar => app.screen = Screen::Bar,
            Screen::Home => {}
        }
        app
    }

    /// Give the program a guides folder to list and to capture into, and a way to capture.
    pub fn with_capture(
        mut self,
        library: PathBuf,
        starter: Starter,
        what: &str,
        is_running: Box<dyn Fn(u32) -> bool>,
    ) -> App {
        self.library = library;
        self.starter = starter;
        self.what = what.to_string();
        self.is_running = is_running;
        self.refresh_home();
        self
    }

    pub fn set_hotkey_words(&mut self, words: &str) {
        self.hotkey_words = words.to_string();
    }

    /// Look at the guides folder again.
    pub fn refresh_home(&mut self) {
        if !self.library.as_os_str().is_empty() {
            let found = library::scan(&self.library);
            self.home.with_guides(&found);
        }
    }

    pub fn window_mode(&self) -> WindowMode {
        if self.screen == Screen::Bar {
            WindowMode::Bar
        } else {
            WindowMode::Normal
        }
    }

    /// Whether a capture is running (or paused), so the window must keep checking for what it has to say.
    pub fn is_capturing(&self) -> bool {
        self.capture.is_some()
    }

    fn open_example(&mut self) {
        if self.editor.as_ref().is_none_or(|e| e.is_saved_to_disk()) {
            self.editor = Some(EditorState::new(demo()));
        }
        self.screen = Screen::Editor;
        let steps = self.editor.as_ref().map(|e| step_count(&e.doc().guide)).unwrap_or(0);
        self.status
            .say(format!("Opened the example guide, {}. It is made up, for trying the editor.", steps_words(steps)));
    }

    fn open_guide(&mut self, path: &Path) {
        match GuideFolder::open(path, &*self.is_running) {
            Ok(opened) => match opened.access {
                Access::ReadOnly { held_by_pid } => {
                    self.status.say(format!(
                        "That guide is open in another copy of Express (process {held_by_pid}), so it cannot be opened here."
                    ));
                }
                Access::ReadWrite => {
                    let steps = step_count(&opened.doc.guide);
                    let title = opened.doc.guide.title.clone();
                    self.editor = Some(EditorState::open(opened.folder, opened.doc));
                    self.screen = Screen::Editor;
                    let crash = if opened.recovered_after_crash {
                        " Express closed unexpectedly last time. Your steps up to the last one are safe."
                    } else {
                        ""
                    };
                    self.status.say(format!("Opened {title}, {}.{crash}", steps_words(steps)));
                }
            },
            Err(why) => self.status.say(format!("The guide could not be opened: {why}")),
        }
    }

    /// Show the capture bar, ready to start.
    fn show_bar(&mut self) {
        self.screen = Screen::Bar;
        self.status.say("The capture bar is open. Press Start, then do the task you want to explain.");
    }

    fn begin_capture(&mut self) {
        match (self.starter)(&self.library) {
            Ok(control) => {
                self.capture = Some(control);
                self.screen = Screen::Bar;
                let finish = if self.hotkey_words.is_empty() {
                    String::new()
                } else {
                    " Use the capture bar or the hotkeys to pause or finish.".to_string()
                };
                self.status.say(format!("Capturing. Do the task you want to explain.{finish}"));
            }
            Err(why) => {
                self.screen = Screen::Bar;
                self.status.say(format!("Capturing could not start. {why}"));
            }
        }
    }

    /// A key pressed anywhere in Windows that Express listens for.
    pub fn hotkey(&mut self, action: HotkeyAction) {
        match (action, self.capture.is_some()) {
            (HotkeyAction::StartOrFinish, false) => match self.screen {
                Screen::Home | Screen::Bar => self.begin_capture(),
                Screen::Editor => {
                    self.status.say("Go back to your guides first, then press the hotkey again to start capturing.")
                }
            },
            (HotkeyAction::StartOrFinish, true) => self.bar_action(BarAction::Finish),
            (HotkeyAction::PauseOrResume, true) => {
                let paused = self.capture.as_ref().is_some_and(|c| c.view().phase == bar::BarPhase::Paused);
                self.bar_action(if paused { BarAction::Resume } else { BarAction::Pause });
            }
            (HotkeyAction::CaptureNow, true) => self.bar_action(BarAction::CaptureNow),
            (HotkeyAction::PauseOrResume | HotkeyAction::CaptureNow, false) => {}
        }
    }

    fn bar_action(&mut self, action: BarAction) {
        match action {
            BarAction::None => {}
            BarAction::Start => self.begin_capture(),
            BarAction::Cancel => {
                self.screen = Screen::Home;
                self.status.say("Cancelled. Nothing was captured.");
            }
            other => {
                if let Some(capture) = self.capture.as_mut() {
                    capture.act(other);
                }
            }
        }
    }

    /// Take what the capture has to say, and open the guide when it has finished. Call it every frame.
    pub fn poll(&mut self) {
        let Some(capture) = self.capture.as_mut() else { return };
        let said = capture.poll();
        if !said.is_empty() {
            self.status.say(said.join(" "));
        }
        let Some(outcome) = capture.finished() else { return };
        self.capture = None;
        match outcome {
            Outcome::Guide(finished) => {
                let steps = step_count(&finished.doc.guide);
                self.editor = Some(EditorState::open(finished.folder, finished.doc));
                self.screen = Screen::Editor;
                self.refresh_home();
                self.status.say(format!("Finished. {} captured. Your guide is open for editing.", steps_words(steps)));
            }
            Outcome::Nothing => {
                self.screen = Screen::Home;
                self.status.say("Finished. No steps were captured, so no guide was made.");
            }
            Outcome::Failed(why) => {
                self.screen = Screen::Home;
                self.refresh_home();
                self.status.say(format!("Capturing stopped. {why}"));
            }
        }
    }

    /// Draw the window: the screen, then the status line along the foot. The foot is drawn last so that a screen reader reaches it
    /// last, after everything the screen shows.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        self.poll();
        if let Some(editor) = self.editor.as_mut() {
            if let Some(why) = editor.save_if_changed() {
                self.status.say(why);
            }
        }
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
                Action::StartCapturing => self.show_bar(),
                Action::Open(index) => match self.home.guides.get(index).and_then(|g| g.path.clone()) {
                    Some(path) => self.open_guide(&path),
                    None => self.open_example(),
                },
            },
            Screen::Bar => {
                let view = match &self.capture {
                    Some(capture) => capture.view(),
                    None => BarView::ready(&self.what, &self.hotkey_words),
                };
                let action = bar::show(ui, &view);
                self.bar_action(action);
            }
            Screen::Editor => {
                let Some(editor_state) = self.editor.as_mut() else {
                    self.screen = Screen::Home;
                    return;
                };
                if editor::show(ui, editor_state, &mut self.status) == Leave::ToHome {
                    let saved_to_disk = editor_state.is_saved_to_disk();
                    if let Some(why) = editor_state.save_if_changed() {
                        self.status.say(why);
                    }
                    if saved_to_disk {
                        // Let go of the guide's folder, and its lock, now that the person has left it.
                        self.editor = None;
                        self.refresh_home();
                    } else if let Some(card) = self.home.guides.iter_mut().find(|g| g.example) {
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
