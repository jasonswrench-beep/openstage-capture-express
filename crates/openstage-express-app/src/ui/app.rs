// SPDX-License-Identifier: Apache-2.0
//! The program's screens and how they connect: Home, the capture bar, and the Editor. All of it draws into a `Ui` it is given, so
//! the tests can run the whole window without opening one.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;

use eframe::egui;
use openstage_express_core::cues::Cue;
use openstage_express_core::fsutil::{sanitise_folder_name, utc_now_iso};
use openstage_express_core::hotkey::{self, HotkeySet, KeyCombo};
use openstage_express_core::updates::UpdateAnswer;
use openstage_express_core::{Access, GuideFolder};
use openstage_express_export::demo::demo;
use openstage_express_export::plan::exported_step_count;
use openstage_express_export::share::CopyOptions;

use super::about;
use super::bar::{self, BarAction, BarView};
use super::capture::{
    cannot_capture, CaptureControl, CuePlayer, Desktop, HotkeyAction, NoCues, NoDesktop, Outcome, Starter,
};
use super::choose_area::{self, AreaAction, AreaScreen};
use super::choose_window::{self, WindowList, WindowListAction};
use super::commands::{Command, MENUS};
use super::deleted::{self, DeletedAction, DeletedView};
use super::editor::{self, Leave};
use super::editor_state::EditorState;
use super::export::{self, ExportAction, ExportState};
use super::home::{self, Action, HomeState};
use super::menubar::{self, Entry, Line, Menu};
use super::open_copy::{self, OpenCopyAction, OpenCopyState};
use super::rows::{step_count, steps_words};
use super::settings::{self as settings_screen, HotkeyEdit, SettingsAction, SettingsInfo};
use super::shell::{self, BarButton, Saved, SearchState, TopBarView};
use super::trash::{self, TrashAction, TrashState};
use super::widgets::{self, Status};
use crate::export_run::{self, Job, Report};
use crate::library;
use crate::settings::{self, AppSettings, Appearance};
use crate::target::{AreaChoice, Target, TargetKind, WindowChoice};
use openstage_express_core::area::AreaRect;

/// Asks the person for a folder (Windows' own dialog), starting at the one given; None when they cancel.
pub type FolderChooser = Box<dyn FnMut(&Path) -> Option<PathBuf>>;

/// Asks the person for a file (Windows' own dialog), starting in the folder given; None when they cancel.
pub type FileChooser = Box<dyn FnMut(&Path) -> Option<PathBuf>>;

/// The height of the line at the foot of the window.
const STATUS_HEIGHT: f32 = 34.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Editor,
    /// Choosing formats and a folder, and exporting the open guide.
    Export,
    /// The settings, on one page.
    Settings,
    /// What the program is, its licence and the licences of what it is built from.
    About,
    /// The guides deleted from Home, which can be brought back.
    Trash,
    /// The steps deleted from the open guide, which can be put back.
    DeletedSteps,
    /// A guide saved as one file (a copy), to be opened as a new guide.
    OpenCopy,
    /// The capture bar: the window is a small strip that stays on top while the person works in other programs.
    Bar,
    /// The list of open windows, to choose the one to capture (still on top, taller than the bar).
    ChooseWindow,
    /// A picture of a display with the area to capture drawn on it.
    ChooseArea,
}

/// How the window itself should look: the whole program, or the strip of the capture bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMode {
    Normal,
    Bar,
    /// The bar made taller, for the list of windows.
    Tall,
}

pub struct App {
    pub screen: Screen,
    pub home: HomeState,
    /// The guides in the Trash, as they were when the Trash was last opened or changed.
    trash: TrashState,
    /// What the Deleted steps screen remembers: where the keyboard goes after a step is put back, and whether it is asking about
    /// emptying the trash.
    deleted: DeletedView,
    pub editor: Option<EditorState>,
    pub status: Status,
    library: PathBuf,
    starter: Starter,
    capture: Option<Box<dyn CaptureControl>>,
    /// What the bar says it will capture, before Start.
    what: String,
    /// The hotkeys in a sentence, for the bar.
    hotkey_words: String,
    /// New hotkeys the person chose, which whoever listens for the keys has not been told yet.
    hotkey_change: Option<HotkeySet>,
    /// The three boxes on the Settings screen where hotkeys are typed.
    hotkey_edit: HotkeyEdit,
    /// For a picture of the lower part of the Settings page (`--scroll`).
    settings_scroll: Option<f32>,
    /// Plays the cue sounds (silent unless the real player was given).
    cues: Box<dyn CuePlayer>,
    is_running: Box<dyn Fn(u32) -> bool>,
    pub export: ExportState,
    pub settings: AppSettings,
    /// Where the settings are saved (nowhere, in the tests).
    settings_path: Option<PathBuf>,
    /// Something to say about the settings file (damaged, newer), shown on the Settings screen.
    settings_note: Option<String>,
    export_job: Option<Receiver<Result<Report, String>>>,
    /// A copy as one file being made on its own thread.
    copy_job: Option<Receiver<Result<export_run::CopyReport, String>>>,
    /// Check for updates: what asks (GitHub, on Windows; a stand-in in the tests), what opens a release page, the check under way,
    /// and its answer, shown on the About screen.
    update_checker: Arc<dyn Fn() -> UpdateAnswer + Send + Sync>,
    page_opener: Box<dyn Fn(&str)>,
    update_job: Option<Receiver<UpdateAnswer>>,
    update_answer: Option<UpdateAnswer>,
    /// What to capture, as chosen so far.
    pub target: Target,
    desktop: Box<dyn Desktop>,
    window_list: WindowList,
    area_screen: Option<AreaScreen>,
    /// Asks the person for a folder (Windows' own dialog), starting at the one given.
    chooser: FolderChooser,
    /// Asks the person for a copy saved as one file (Windows' own dialog).
    file_chooser: FileChooser,
    /// The Open a copy screen: the path typed or chosen, and what the last try said.
    open_copy: OpenCopyState,
    /// Shows a folder to the person (opens it in Explorer).
    opener: Box<dyn Fn(&Path)>,
    /// Opens Windows' notification settings (Do not disturb). Nothing in the tests.
    notifications: Box<dyn Fn()>,
    /// The command search (Ctrl+K), while it is open.
    pub search: Option<SearchState>,
    /// Whether the list of keyboard keys (Ctrl+/) is open.
    pub keys_open: bool,
    /// Where the keyboard was when the command search or the keys list opened, to give it back when it closes (`None` until
    /// it is noted, in the first frame the window is open).
    dialog_return: Option<Option<egui::Id>>,
    /// The keyboard to give back once a closed dialog no longer holds it: egui keeps the keyboard inside a dialog's layer for one
    /// frame after it was last drawn, so the place is given back in the frame after that (the count is the frames to wait).
    focus_next_frame: Option<(egui::Id, u8)>,
}

/// Whether a command can be used now: on which screen the window is, whether a guide is open, whether a capture is running.
fn self_usable(editor: &Option<EditorState>, screen: Screen, capturing: bool, command: Command) -> bool {
    let in_editor = screen == Screen::Editor;
    let guide_open = editor.is_some() && matches!(screen, Screen::Editor | Screen::Export | Screen::DeletedSteps);
    match command {
        Command::SearchCommands
        | Command::KeyboardKeys
        | Command::CheckForUpdates
        | Command::About
        | Command::Colours(_)
        | Command::Settings => true,
        Command::StartCapturing => !capturing,
        Command::OpenCopy | Command::Trash => true,
        Command::BackToGuides => screen != Screen::Home,
        Command::ExportScreen => guide_open && editor.as_ref().is_some_and(|e| !e.doc().guide.steps.is_empty()),
        Command::DeletedSteps => guide_open && editor.as_ref().is_some_and(|e| e.deleted_count() > 0),
        other => {
            in_editor && editor.as_ref().and_then(|e| editor::command_state(e, other)).is_some_and(|(usable, _)| usable)
        }
    }
}

impl App {
    /// The program on its first screen (or, for a quick look, already in the editor or on the bar). It cannot capture and has no
    /// guides folder until [`App::with_capture`] is called.
    pub fn new(screen: Screen) -> App {
        let mut app = App {
            screen: Screen::Home,
            home: HomeState::example(),
            trash: TrashState::default(),
            deleted: DeletedView::default(),
            editor: None,
            status: Status::default(),
            library: PathBuf::new(),
            starter: cannot_capture(),
            capture: None,
            what: "the whole screen".to_string(),
            hotkey_words: String::new(),
            hotkey_change: None,
            hotkey_edit: HotkeyEdit::default(),
            settings_scroll: None,
            cues: Box::new(NoCues),
            is_running: Box::new(|_| false),
            export: ExportState::new(PathBuf::new(), false),
            settings: AppSettings::default(),
            settings_path: None,
            settings_note: None,
            export_job: None,
            copy_job: None,
            update_checker: Arc::new(|| UpdateAnswer::error("Checking for updates works only in the Windows program.")),
            page_opener: Box::new(|_| {}),
            update_job: None,
            update_answer: None,
            target: Target::default(),
            desktop: Box::new(NoDesktop),
            window_list: WindowList::default(),
            area_screen: None,
            chooser: Box::new(|_| None),
            file_chooser: Box::new(|_| None),
            open_copy: OpenCopyState::new(),
            opener: Box::new(|_| {}),
            notifications: Box::new(|| {}),
            search: None,
            keys_open: false,
            dialog_return: None,
            focus_next_frame: None,
        };
        match screen {
            Screen::Editor => app.open_example(),
            Screen::Bar | Screen::ChooseWindow | Screen::ChooseArea => app.screen = Screen::Bar,
            Screen::Export => {
                app.open_example();
                app.screen = Screen::Export;
            }
            Screen::Settings => app.screen = Screen::Settings,
            Screen::About => app.screen = Screen::About,
            Screen::Trash => app.screen = Screen::Trash,
            Screen::DeletedSteps => {
                app.open_example();
                app.delete_demo_steps();
                app.screen = Screen::DeletedSteps;
            }
            Screen::OpenCopy => app.screen = Screen::OpenCopy,
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

    /// Give the program what Export needs from the computer: the folder exports go into, whether a PDF can be made, a way to ask
    /// for another folder, and a way to show a folder.
    pub fn with_system(
        mut self,
        exports: PathBuf,
        can_pdf: bool,
        chooser: FolderChooser,
        opener: Box<dyn Fn(&Path)>,
    ) -> App {
        self.export = ExportState::new(exports, can_pdf);
        self.chooser = chooser;
        self.opener = opener;
        self
    }

    /// What is in the Open a copy screen's box (for the tests).
    #[cfg(test)]
    pub fn open_copy_path(&self) -> &str {
        &self.open_copy.path
    }

    /// Give the program a way to ask for a copy saved as one file.
    /// What the bar's Do not disturb button does (opening Windows' notification settings, on Windows).
    pub fn with_notifications(mut self, open: Box<dyn Fn()>) -> App {
        self.notifications = open;
        self
    }

    /// What Check for updates asks, and what opens a page of the releases (on Windows, GitHub and the person's browser).
    pub fn with_updates(mut self, checker: Arc<dyn Fn() -> UpdateAnswer + Send + Sync>, page_opener: Box<dyn Fn(&str)>) -> App {
        self.update_checker = checker;
        self.page_opener = page_opener;
        self
    }

    /// The same as `with_updates`, for a test that already has the app.
    #[cfg(test)]
    pub fn use_updates(&mut self, checker: Arc<dyn Fn() -> UpdateAnswer + Send + Sync>, page_opener: Box<dyn Fn(&str)>) {
        self.update_checker = checker;
        self.page_opener = page_opener;
    }

    /// Ask, once, on a thread of its own, whether a newer Express is published. The answer is shown on the About screen.
    fn start_update_check(&mut self) {
        if self.update_job.is_some() {
            return;
        }
        let checker = self.update_checker.clone();
        let (tell, heard) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tell.send(checker());
        });
        self.update_job = Some(heard);
        self.update_answer = None;
        self.status.say("Checking for updates. Please wait.");
    }

    /// Take the answer of Check for updates, if it has come.
    fn poll_updates(&mut self) {
        let Some(job) = &self.update_job else { return };
        let answer = match job.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => UpdateAnswer::error("The check stopped without an answer. Try again."),
        };
        self.update_job = None;
        self.status.say(answer.message.clone());
        self.update_answer = Some(answer);
    }

    pub fn with_file_chooser(mut self, chooser: FileChooser) -> App {
        self.file_chooser = chooser;
        self
    }

    /// Use these settings, saved at `path` (`None` keeps them in memory only). `note` is something to tell the person about the
    /// settings file.
    pub fn with_settings(mut self, settings: AppSettings, path: Option<PathBuf>, note: Option<String>) -> App {
        self.export.open_after = settings.open_folder_after_export;
        self.target.kind = settings.capture_target;
        self.target.area = settings.last_area;
        let formats: Vec<_> = crate::export_run::Format::ALL
            .into_iter()
            .filter(|f| settings.export_formats.iter().any(|name| name == f.folder()))
            .filter(|f| *f != crate::export_run::Format::Pdf || self.export.can_pdf)
            .collect();
        if !formats.is_empty() {
            self.export.selection = crate::export_run::Selection::none_then(&formats);
        }
        self.export.set_pictures(
            settings.export_picture_width,
            settings.export_jpg_quality,
            settings.export_preview_steps,
        );
        self.export.documents = settings.export_document_pictures;
        self.home.set_sort(settings.home_sort);
        self.settings = settings;
        self.settings_path = path;
        self.settings_note = note;
        self
    }

    /// The colour scheme the person chose.
    pub fn appearance(&self) -> Appearance {
        self.settings.appearance
    }

    fn save_settings(&mut self) {
        if let Some(path) = &self.settings_path {
            if let Err(why) = settings::save(path, &self.settings) {
                self.status.say(why);
            }
        }
    }

    /// Keep the settings that the Export screen also changes in step with it.
    fn sync_export_settings(&mut self) {
        let chosen: Vec<String> = self.export.selection.formats().iter().map(|f| f.folder().to_string()).collect();
        // The quality and the number of steps kept are the last ones that were whole numbers in range, never half of what is being typed.
        let pictures = (self.export.width, u32::from(self.export.jpg_quality), self.export.preview_steps);
        let kept =
            (self.settings.export_picture_width, self.settings.export_jpg_quality, self.settings.export_preview_steps);
        if self.export.open_after != self.settings.open_folder_after_export
            || chosen != self.settings.export_formats
            || pictures != kept
            || self.export.documents != self.settings.export_document_pictures
        {
            self.settings.open_folder_after_export = self.export.open_after;
            self.settings.export_formats = chosen;
            self.settings.export_document_pictures = self.export.documents;
            (
                self.settings.export_picture_width,
                self.settings.export_jpg_quality,
                self.settings.export_preview_steps,
            ) = pictures;
            self.save_settings();
        }
    }

    /// For looking at the editor without a person: choose the first three steps of the open guide, so the screen for several
    /// chosen steps can be drawn.
    pub fn choose_demo_steps(&mut self) {
        let Some(editor) = self.editor.as_mut() else { return };
        let ids: Vec<String> = editor.rows().into_iter().map(|r| r.id).collect();
        if let (Some(first), Some(third)) = (ids.first(), ids.get(2)) {
            editor.select(first);
            let _ = editor.choose_run_to(third);
        }
    }

    /// For looking at the editor without a person: delete the second and the sixth step of the open guide, so the Deleted steps
    /// button and screen have something to show.
    pub fn delete_demo_steps(&mut self) {
        let Some(editor) = self.editor.as_mut() else { return };
        let ids: Vec<String> = editor.rows().into_iter().map(|r| r.id).collect();
        for at in [5, 1] {
            if let Some(id) = ids.get(at) {
                editor.select(id);
                let _ = editor.delete_selected();
            }
        }
        if let Some(first) = editor.rows().first() {
            let first = first.id.clone();
            editor.select(&first);
        }
    }

    /// For looking at the Deleted steps screen without a person: open its question "Empty the trash for good?".
    pub fn ask_to_empty_demo(&mut self) {
        self.deleted.ask();
    }

    /// For looking at the editor without a person: add one of each kind of mark to the open guide's chosen step.
    pub fn add_demo_marks(&mut self) {
        use openstage_express_core::marks::Tool;
        let Some(editor) = self.editor.as_mut() else { return };
        for tool in Tool::ALL {
            let _ = editor.add_mark(tool);
        }
        for (id, _) in editor.mark_rows().into_iter().skip(2).step_by(2) {
            editor.select_mark(Some(&id));
            let _ = editor.nudge_mark(60, 40);
        }
    }

    pub fn set_hotkey_words(&mut self, words: &str) {
        self.hotkey_words = words.to_string();
    }

    /// The hotkeys that are set now (some the person asked for may be missing, if another program holds them), said on the bar and
    /// on the Settings screen.
    pub fn set_hotkeys_working(&mut self, working: &[(HotkeyAction, KeyCombo)]) {
        self.hotkey_words = hotkey::sentence(working);
    }

    /// The hotkeys the person has chosen since this was last called, if they changed: the listener sets them again.
    pub fn take_hotkey_change(&mut self) -> Option<HotkeySet> {
        self.hotkey_change.take()
    }

    /// For a picture of the lower part of the Settings page: keep it scrolled this far down.
    pub fn with_settings_scroll(mut self, y: f32) -> App {
        self.settings_scroll = Some(y);
        self
    }

    /// Give the program a way to play the cue sounds.
    pub fn with_cues(mut self, cues: Box<dyn CuePlayer>) -> App {
        self.cues = cues;
        self
    }

    /// Play a cue if the person has turned that sound on.
    fn cue(&self, cue: Cue) {
        if self.settings.sounds.wants(cue) {
            self.cues.play(cue);
        }
    }

    /// Look at the guides folder again.
    pub fn refresh_home(&mut self) {
        if !self.library.as_os_str().is_empty() {
            let found = library::scan(&self.library);
            self.home.with_guides(&found);
        }
    }

    pub fn window_mode(&self) -> WindowMode {
        match self.screen {
            Screen::Bar => WindowMode::Bar,
            Screen::ChooseWindow => WindowMode::Tall,
            _ => WindowMode::Normal,
        }
    }

    /// Give the program a way to look at the desktop (the list of windows, the displays, a picture of a display).
    pub fn with_desktop(mut self, desktop: Box<dyn Desktop>) -> App {
        self.desktop = desktop;
        self
    }

    /// Whether a capture is running (or paused), so the window must keep checking for what it has to say.
    pub fn is_capturing(&self) -> bool {
        self.capture.is_some()
    }

    /// Whether an export is being made on its thread.
    pub fn is_exporting(&self) -> bool {
        self.export_job.is_some() || self.copy_job.is_some()
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
        if !self.target.is_ready() {
            self.screen = Screen::Bar;
            self.status.say(match self.target.kind {
                TargetKind::Window => "Choose the window to capture first.",
                _ => "Draw the area to capture first.",
            });
            return;
        }
        match (self.starter)(&self.library, &self.settings, &self.target) {
            Ok(control) => {
                self.capture = Some(control);
                self.screen = Screen::Bar;
                let finish = if self.hotkey_words.is_empty() {
                    String::new()
                } else {
                    " Use the capture bar or the hotkeys to pause or finish.".to_string()
                };
                self.status.say(format!("Capturing. Do the task you want to explain.{finish}"));
                self.cue(Cue::Start);
            }
            Err(why) => {
                self.screen = Screen::Bar;
                self.status.say(format!("Capturing could not start. {why}"));
                self.cue(Cue::Error);
            }
        }
    }

    /// A key pressed anywhere in Windows that Express listens for.
    pub fn hotkey(&mut self, action: HotkeyAction) {
        match (action, self.capture.is_some()) {
            (HotkeyAction::StartOrFinish, false) => match self.screen {
                Screen::Home | Screen::Bar => self.begin_capture(),
                Screen::ChooseWindow | Screen::ChooseArea => {
                    self.status.say("Finish choosing what to capture first, then press the hotkey again.")
                }
                Screen::Editor
                | Screen::Export
                | Screen::Settings
                | Screen::About
                | Screen::Trash
                | Screen::OpenCopy
                | Screen::DeletedSteps => {
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
            BarAction::SetTarget(kind) => {
                self.target.kind = kind;
                self.settings.capture_target = kind;
                self.save_settings();
                self.status.say(format!("Capturing {}.", self.target.words(&self.what)));
            }
            BarAction::ChooseWindow => self.open_window_list(),
            BarAction::ChooseArea => self.open_area_screen(1),
            BarAction::Cancel => {
                self.screen = Screen::Home;
                self.status.say("Cancelled. Nothing was captured.");
            }
            BarAction::DoNotDisturb => {
                (self.notifications)();
                self.status.say("Windows' notification settings are open: turn on Do not disturb there, then come back and press Start.");
            }
            other => {
                // Only an action the capture can take now makes a sound: Pause while capturing, Resume while paused, Finish either way.
                let phase = self.capture.as_ref().map(|c| c.view().phase);
                if let Some(capture) = self.capture.as_mut() {
                    capture.act(other);
                }
                match (other, phase) {
                    (BarAction::Pause, Some(bar::BarPhase::Capturing)) => self.cue(Cue::Stop),
                    (BarAction::Resume, Some(bar::BarPhase::Paused)) => self.cue(Cue::Start),
                    (BarAction::Finish, Some(bar::BarPhase::Capturing | bar::BarPhase::Paused)) => self.cue(Cue::Stop),
                    _ => {}
                }
            }
        }
    }

    /// For looking at the screen without a person: the window list, over whatever desktop the program was given.
    pub fn show_choose_window(&mut self) {
        self.target.kind = TargetKind::Window;
        self.open_window_list();
    }

    /// The same for drawing an area.
    pub fn show_choose_area(&mut self) {
        self.target.kind = TargetKind::Area;
        self.open_area_screen(1);
    }

    fn open_window_list(&mut self) {
        let (rows, problem) = match self.desktop.windows() {
            Ok(rows) => (rows, None),
            Err(why) => (Vec::new(), Some(why)),
        };
        self.window_list = WindowList::new(rows, problem);
        self.screen = Screen::ChooseWindow;
        let n = self.window_list.rows.len();
        self.status.say(format!("Choose a window. {n} {} open.", if n == 1 { "window is" } else { "windows are" }));
    }

    fn open_area_screen(&mut self, display: u32) {
        let displays = match self.desktop.displays() {
            Ok(d) if !d.is_empty() => d,
            Ok(_) => {
                self.status.say("Windows reports no display.");
                return;
            }
            Err(why) => {
                self.status.say(why);
                return;
            }
        };
        let display =
            self.target.area.map(|a| a.display).filter(|n| displays.iter().any(|d| d.number == *n)).unwrap_or(display);
        match self.desktop.snapshot(display) {
            Ok(picture) => {
                let initial = self.target.area.filter(|a| a.display == display).map(|a| AreaRect {
                    x: a.x,
                    y: a.y,
                    width: a.width,
                    height: a.height,
                });
                self.area_screen = Some(AreaScreen::new(displays, display, picture, initial));
                self.screen = Screen::ChooseArea;
                self.status.say("Draw the area to capture on the picture of the display, or type its numbers.");
            }
            Err(why) => self.status.say(format!("A picture of the display could not be taken. {why}")),
        }
    }

    /// Take what the copy as one file has said, if it has finished.
    fn poll_copy(&mut self) {
        let Some(job) = &self.copy_job else { return };
        let answer = match job.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("Making the copy stopped without an answer.".to_string()),
        };
        self.copy_job = None;
        self.export.working = false;
        match answer {
            Ok(report) => {
                let sentence = report.sentence();
                if self.export.open_after {
                    (self.opener)(&report.folder);
                }
                self.export.said = Some(sentence.clone());
                self.status.say(sentence);
            }
            Err(why) => {
                let words = format!("The copy was not made. {why}");
                self.export.said = Some(words.clone());
                self.status.say(words);
            }
        }
    }

    /// Start saving the open guide as one file, in a new folder beside the exports.
    fn start_copy(&mut self) {
        if self.export_job.is_some() || self.copy_job.is_some() {
            return;
        }
        let Some(editor) = self.editor.as_mut() else { return };
        editor.commit_pending();
        let guide = editor.doc().guide.clone();
        let folder = self
            .export
            .folder
            .join(sanitise_folder_name(&guide.title))
            .join(export_run::run_folder_name(&utc_now_iso()))
            .join("copy");
        let options = CopyOptions { include_originals: self.export.copy_originals };
        let job = export_run::CopyJob { guide, pictures: editor.pictures(), options, folder };
        self.copy_job = Some(export_run::spawn_copy(job));
        self.export.working = true;
        self.export.said = None;
        self.status.say("Saving a copy as one file. Please wait.");
    }

    /// Take what the export has said, if it has finished.
    fn poll_export(&mut self) {
        self.poll_copy();
        let Some(job) = &self.export_job else { return };
        let answer = match job.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("The export stopped without an answer.".to_string()),
        };
        self.export_job = None;
        self.export.working = false;
        match answer {
            Ok(report) => {
                let sentence = report.sentence();
                if self.export.open_after && !report.made.is_empty() {
                    (self.opener)(&report.folder);
                }
                self.export.said = Some(sentence.clone());
                self.status.say(sentence);
            }
            Err(why) => {
                let words = format!("The export did not work. {why}");
                self.export.said = Some(words.clone());
                self.status.say(words);
            }
        }
    }

    fn start_export(&mut self) {
        if self.export_job.is_some() || self.copy_job.is_some() {
            return;
        }
        if self.export.selection.is_empty() {
            self.status.say("Choose at least one format.");
            return;
        }
        // What is typed in the boxes is read now, so that it counts however the button was pressed.
        let choices = match self.export.picture_choices() {
            Ok(choices) => choices,
            Err(why) => {
                let words = format!("Nothing was exported. {why}");
                self.export.said = Some(words.clone());
                self.status.say(words);
                return;
            }
        };
        let Some(editor) = self.editor.as_ref() else { return };
        let guide = editor.doc().guide.clone();
        // A preview that leaves steps out says so in the name of its folder.
        let when = utc_now_iso();
        let run_name = match choices.first_steps.map(|n| n.max(1)).filter(|n| *n < exported_step_count(&guide)) {
            Some(shown) => export_run::preview_run_folder_name(&when, shown),
            None => export_run::run_folder_name(&when),
        };
        let folder = self.export.folder.join(sanitise_folder_name(&guide.title)).join(run_name);
        let job = Job { guide, pictures: editor.pictures(), selection: self.export.selection.clone(), choices, folder };
        let n = job.selection.count();
        self.export_job = Some(export_run::spawn(job));
        self.export.working = true;
        self.export.said = None;
        let formats = format!("{n} format{}", if n == 1 { "" } else { "s" });
        let words = if choices.first_steps.is_some() {
            format!("Exporting a preview in {formats}. Please wait.")
        } else {
            format!("Exporting {formats}. Please wait.")
        };
        self.status.say(words);
    }

    /// Take what the capture has to say, and open the guide when it has finished. Call it every frame.
    pub fn poll(&mut self) {
        self.poll_export();
        self.poll_updates();
        let Some(capture) = self.capture.as_mut() else { return };
        let steps_before = capture.view().steps;
        let said = capture.poll();
        let steps_after = capture.view().steps;
        let trouble = capture.trouble();
        let outcome = capture.finished();
        if steps_after > steps_before {
            self.cue(Cue::Step);
        }
        if trouble {
            self.cue(Cue::Error);
        }
        if !said.is_empty() {
            self.status.say(said.join(" "));
        }
        let Some(outcome) = outcome else { return };
        self.capture = None;
        if matches!(outcome, Outcome::Failed(_)) {
            self.cue(Cue::Error);
        }
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

    /// Whether this screen is shown in the suite's frame (the navy bar and the menus). The capture bar and the two screens that
    /// choose what to capture are a small window that stays on top, and keep to their own few controls.
    pub fn has_frame(&self) -> bool {
        !matches!(self.screen, Screen::Bar | Screen::ChooseWindow | Screen::ChooseArea)
    }

    /// Draw the window: the navy bar and the menus, the screen, then the status line along the foot. The foot is drawn last so that
    /// a screen reader reaches it last, after everything the screen shows. The command search and the keys list are drawn over it.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        self.poll();
        if let Some(editor) = self.editor.as_mut() {
            if let Some(why) = editor.save_if_changed() {
                self.status.say(why);
            }
        }
        match self.focus_next_frame.take() {
            Some((back, 0)) => ui.ctx().memory_mut(|m| m.request_focus(back)),
            Some((back, wait)) => {
                self.focus_next_frame = Some((back, wait - 1));
                ui.ctx().request_repaint();
            }
            None => {}
        }
        let mut chosen = if self.has_frame() { self.window_keys(ui.ctx()) } else { None };
        let full = ui.available_rect_before_wrap();
        let split = (full.max.y - STATUS_HEIGHT).max(full.min.y);
        let mut main = egui::Rect::from_min_max(full.min, egui::pos2(full.max.x, split));
        let foot = egui::Rect::from_min_max(egui::pos2(full.min.x, split), full.max);
        if self.has_frame() {
            let bar = egui::Rect::from_min_size(main.min, egui::vec2(main.width(), shell::BAR_HEIGHT));
            let menus = egui::Rect::from_min_size(bar.left_bottom(), egui::vec2(main.width(), shell::MENU_HEIGHT));
            main.min.y = menus.max.y;
            let view = self.bar_view();
            if let Some(command) =
                ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| shell::top_bar(ui, &view)).inner
            {
                chosen = Some(command);
            }
            let menus_now = self.menus();
            if let Some(command) =
                ui.scope_builder(egui::UiBuilder::new().max_rect(menus), |ui| shell::menu_row(ui, &menus_now)).inner
            {
                chosen = Some(command);
            }
        }
        ui.scope_builder(egui::UiBuilder::new().max_rect(main), |ui| self.draw_screen(ui));
        ui.scope_builder(egui::UiBuilder::new().max_rect(foot), |ui| {
            let p = widgets::palette(ui);
            if self.has_frame() {
                ui.painter().rect_filled(foot, 0.0, p.surface);
            }
            ui.painter().hline(
                foot.x_range(),
                foot.min.y,
                egui::Stroke::new(1.0, if self.has_frame() { p.divider } else { p.border }),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                widgets::status_line(ui, &self.status);
            });
        });
        if self.has_frame() {
            let a_dialog_was_open = self.search.is_some() || self.keys_open;
            if a_dialog_was_open && self.dialog_return.is_none() {
                self.dialog_return = Some(ui.ctx().memory(|m| m.focused()));
            }
            if let Some(search) = self.search.as_mut() {
                let usable = |c: Command| self_usable(&self.editor, self.screen, self.capture.is_some(), c);
                let (run, close) = shell::command_search(ui.ctx(), search, &usable);
                if close {
                    self.search = None;
                }
                if run.is_some() {
                    chosen = run;
                }
            } else if self.keys_open && shell::keyboard_keys(ui.ctx(), &self.hotkey_words) {
                self.keys_open = false;
            }
            // A dialog that closed gives the keyboard back to where it was (a command run from it may then move it on).
            if a_dialog_was_open && self.search.is_none() && !self.keys_open {
                if let Some(Some(back)) = self.dialog_return.take() {
                    self.focus_next_frame = Some((back, 1));
                    ui.ctx().request_repaint();
                }
                self.dialog_return = None;
            }
        }
        if let Some(command) = chosen {
            self.run_command(command);
        }
    }

    /// The keys that work anywhere in the window: Ctrl+K (search for a command), Ctrl+/ (the keys), F10 (the menus).
    fn window_keys(&mut self, ctx: &egui::Context) -> Option<Command> {
        let (search, keys, menus) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::K),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Slash),
                i.consume_key(egui::Modifiers::NONE, egui::Key::F10),
            )
        });
        if menus && self.search.is_none() && !self.keys_open {
            menubar::focus_bar(ctx);
        }
        if search {
            return Some(Command::SearchCommands);
        }
        if keys {
            return Some(Command::KeyboardKeys);
        }
        None
    }

    /// What the navy bar shows on this screen.
    fn bar_view(&self) -> TopBarView {
        let mut view = TopBarView::default();
        let guide = self.editor.as_ref().map(|e| {
            let saved = if e.is_saved_to_disk() { Saved::Saved } else { Saved::Example };
            (e.doc().guide.title.clone(), saved)
        });
        let state = |c: Command| self.command_state(c).0;
        match self.screen {
            Screen::Home => {
                view.buttons.push(BarButton {
                    command: Command::StartCapturing,
                    words: "Start capturing",
                    primary: true,
                    enabled: true,
                });
            }
            Screen::Editor => {
                view.back = Some(BarButton {
                    command: Command::BackToGuides,
                    words: "Back to guides",
                    primary: false,
                    enabled: true,
                });
                if let Some((title, saved)) = guide {
                    view.guide = Some((title, saved, true));
                }
                view.buttons.push(BarButton {
                    command: Command::Undo,
                    words: "Undo",
                    primary: false,
                    enabled: state(Command::Undo),
                });
                view.buttons.push(BarButton {
                    command: Command::Redo,
                    words: "Redo",
                    primary: false,
                    enabled: state(Command::Redo),
                });
                view.buttons.push(BarButton {
                    command: Command::ExportScreen,
                    words: "Export",
                    primary: true,
                    enabled: state(Command::ExportScreen),
                });
            }
            Screen::Export | Screen::DeletedSteps => {
                if let Some((title, saved)) = guide {
                    view.guide = Some((title, saved, false));
                }
            }
            _ => {}
        }
        view
    }

    /// The menu bar as it stands now: every command, with whether it can be used and whether it is ticked.
    fn menus(&self) -> Vec<Menu<Command>> {
        MENUS
            .iter()
            .map(|plan| Menu {
                title: plan.title.to_string(),
                lines: plan
                    .items
                    .iter()
                    .map(|item| match item {
                        None => Line::Separator,
                        Some(command) => {
                            let (enabled, ticked) = self.command_state(*command);
                            Line::Item(Entry {
                                command: *command,
                                words: command.words(),
                                icon: Some(command.icon()),
                                keys: command.keys().map(str::to_string),
                                enabled,
                                kind: command.kind(),
                                ticked,
                            })
                        }
                    })
                    .collect(),
            })
            .collect()
    }

    /// Whether a command can be used now, and whether it is ticked.
    pub fn command_state(&self, command: Command) -> (bool, bool) {
        if let Command::Colours(appearance) = command {
            return (true, self.settings.appearance == appearance);
        }
        (self_usable(&self.editor, self.screen, self.capture.is_some(), command), self.ticked(command))
    }

    fn ticked(&self, command: Command) -> bool {
        match (self.screen, self.editor.as_ref()) {
            (Screen::Editor, Some(editor)) => editor::command_state(editor, command).is_some_and(|(_, ticked)| ticked),
            _ => false,
        }
    }

    /// Leave the open guide, if there is one: save it, and let go of its folder (and its lock) when it is a guide on disk.
    fn leave_guide(&mut self) {
        let Some(editor_state) = self.editor.as_mut() else { return };
        editor_state.commit_pending();
        let saved_to_disk = editor_state.is_saved_to_disk();
        if let Some(why) = editor_state.save_if_changed() {
            self.status.say(why);
        }
        if saved_to_disk {
            self.editor = None;
            self.refresh_home();
        } else if let Some(card) = self.home.guides.iter_mut().find(|g| g.example) {
            card.steps = step_count(&editor_state.doc().guide);
            card.title = editor_state.doc().guide.title.clone();
        }
    }

    /// Do a command from the navy bar, a menu or the command search.
    pub fn run_command(&mut self, command: Command) {
        if !self.command_state(command).0 {
            return;
        }
        match command {
            Command::SearchCommands => {
                self.keys_open = false;
                self.search = Some(SearchState::new());
            }
            Command::KeyboardKeys => {
                self.search = None;
                self.keys_open = true;
            }
            Command::Colours(appearance) => {
                self.settings.appearance = appearance;
                self.save_settings();
                self.status.say(format!(
                    "Colours: {}.",
                    match appearance {
                        Appearance::System => "following Windows",
                        Appearance::Light => "light",
                        Appearance::Dark => "dark",
                    }
                ));
            }
            Command::StartCapturing => {
                self.leave_guide();
                self.home_action(Action::StartCapturing);
            }
            Command::CheckForUpdates => {
                self.leave_guide();
                self.screen = Screen::About;
                self.start_update_check();
            }
            Command::OpenCopy | Command::Trash | Command::Settings | Command::About => {
                self.leave_guide();
                self.home_action(match command {
                    Command::OpenCopy => Action::OpenCopy,
                    Command::Trash => Action::Trash,
                    Command::Settings => Action::Settings,
                    _ => Action::About,
                });
            }
            Command::BackToGuides => {
                self.leave_guide();
                self.screen = Screen::Home;
                self.status.say("Back at your guides.");
            }
            Command::ExportScreen => {
                if let Some(editor) = self.editor.as_mut() {
                    editor.commit_pending();
                }
                self.screen = Screen::Export;
                self.status.say("Export. Choose the formats and the folder, then press Export.");
            }
            Command::DeletedSteps => self.open_deleted_steps(),
            other => {
                let Some(editor_state) = self.editor.as_mut() else { return };
                match editor::run(editor_state, &mut self.status, other) {
                    Leave::ToDeleted => self.open_deleted_steps(),
                    Leave::Stay => {}
                }
            }
        }
    }

    fn open_deleted_steps(&mut self) {
        let Some(editor_state) = self.editor.as_mut() else { return };
        editor_state.commit_pending();
        self.screen = Screen::DeletedSteps;
        self.deleted = DeletedView::default();
        let n = editor_state.deleted_count();
        self.status.say(format!("Deleted steps. {n} can be put back. Restore puts a step back where it was."));
    }

    /// What the person asked for on Home.
    fn home_action(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::StartCapturing => self.show_bar(),
            Action::About => {
                self.screen = Screen::About;
                self.status.say("About OpenStage Capture Express, its licence and what it is built from.");
            }
            Action::Settings => {
                self.screen = Screen::Settings;
                self.status.say("Settings. Each change is saved as you make it.");
            }
            Action::Trash => {
                self.refresh_trash();
                self.screen = Screen::Trash;
                let n = self.trash.entries.len();
                self.status.say(if n == 0 {
                    "The Trash is empty.".to_string()
                } else {
                    format!("The Trash has {n} guide{}. Restore brings one back.", if n == 1 { "" } else { "s" })
                });
            }
            Action::OpenCopy => {
                self.open_copy = OpenCopyState::new();
                self.screen = Screen::OpenCopy;
                self.status.say("Open a copy. Type or paste the place of the file, or choose it with Browse.");
            }
            Action::Open(index) => match self.home.guides.get(index).and_then(|g| g.path.clone()) {
                Some(path) => self.open_guide(&path),
                None => self.open_example(),
            },
            Action::Rename(index) => {
                if self.home.start_renaming(index) {
                    let title = self.home.renaming.as_ref().map(|r| r.title.clone()).unwrap_or_default();
                    self.status
                        .say(format!("Renaming {title}. Type the new name and press Enter, or Escape to cancel."));
                }
            }
            Action::SaveName => self.save_name(),
            Action::CancelRename => {
                self.home.renaming = None;
                self.status.say("Cancelled. The name was not changed.");
            }
            Action::Delete(index) => {
                if self.home.start_deleting(index) {
                    let title = self.home.deleting.as_ref().map(|d| d.title.clone()).unwrap_or_default();
                    self.status.say(format!(
                        "Move {title} to the Trash? Nothing is erased. Press Move to the Trash, or Keep it."
                    ));
                }
            }
            Action::KeepGuide => {
                let title = self.home.deleting.take().map(|d| d.title).unwrap_or_default();
                self.status.say(format!("Kept {title}."));
            }
            Action::ConfirmDelete => self.move_to_trash(),
            Action::Sort => {
                let next = self.home.sort.next();
                self.settings.home_sort = next;
                self.home.set_sort(next);
                self.save_settings();
                self.status.say(format!("Sorted by {}.", next.name()));
            }
        }
    }

    /// Make a new guide of the person's own from the copy they named, and open it.
    fn open_a_copy(&mut self) {
        let path = PathBuf::from(self.open_copy.clean_path());
        match library::open_copy(&path, &self.library) {
            Ok(opened) => {
                self.refresh_home();
                self.open_guide(&opened.guide);
                let mut words = format!(
                    "Opened a copy of {} as a new guide of your own, with {}. The file was not changed.",
                    opened.title,
                    steps_words(opened.steps)
                );
                if !opened.warnings.is_empty() {
                    words.push_str(&format!(" To know: {}", opened.warnings.join(" ")));
                }
                // The guide's own opening said something; this is the more useful thing to hear.
                self.status.say(words);
                self.open_copy = OpenCopyState::new();
            }
            Err(why) => {
                self.open_copy.said = Some(why.clone());
                self.status.say(why);
            }
        }
    }

    /// Look at the Trash folder again.
    fn refresh_trash(&mut self) {
        self.trash.entries =
            if self.library.as_os_str().is_empty() { Vec::new() } else { library::scan_trash(&self.library) };
    }

    /// Use the name typed in the rename box.
    fn save_name(&mut self) {
        let Some(rename) = self.home.renaming.clone() else { return };
        match library::rename_guide(&rename.path, &rename.text, &*self.is_running) {
            Ok(kept) => {
                self.home.renaming = None;
                self.refresh_home();
                self.status.say(format!("Renamed to {kept}."));
            }
            // The box stays open, so the name can be fixed, or cancelled.
            Err(why) => self.status.say(format!("{} was not renamed. {why}", rename.title)),
        }
    }

    /// The person said yes to moving the guide to the Trash.
    fn move_to_trash(&mut self) {
        let Some(delete) = self.home.deleting.take() else { return };
        match library::move_to_trash(&delete.path, &self.library, &*self.is_running) {
            Ok(_) => {
                self.refresh_home();
                self.status.say(format!("Moved {} to the Trash. Open the Trash to bring it back.", delete.title));
            }
            Err(why) => self.status.say(format!("{} was not moved. {why}", delete.title)),
        }
    }

    /// Bring a guide back from the Trash.
    fn restore_from_trash(&mut self, index: usize) {
        let Some(entry) = self.trash.entries.get(index).cloned() else { return };
        match library::restore(&entry.path, &self.library) {
            Ok(_) => {
                self.refresh_trash();
                self.refresh_home();
                self.status.say(format!("Brought {} back to your guides.", entry.title));
            }
            Err(why) => self.status.say(format!("{} was not brought back. {why}", entry.title)),
        }
    }

    fn draw_screen(&mut self, ui: &mut egui::Ui) {
        match self.screen {
            Screen::Home => {
                let action = home::show(ui, &mut self.home);
                self.home_action(action);
            }
            Screen::Trash => match trash::show(ui, &self.trash) {
                TrashAction::None => {}
                TrashAction::Back => {
                    self.screen = Screen::Home;
                    self.status.say("Back at your guides.");
                }
                TrashAction::ShowFolder => {
                    let folder = library::trash_folder(&self.library);
                    // The folder is only made when something is first deleted; show the guides folder until then.
                    let shown = if folder.is_dir() { folder } else { self.library.clone() };
                    (self.opener)(&shown);
                    self.status.say("Showing the Trash folder in Explorer.");
                }
                TrashAction::Restore(index) => self.restore_from_trash(index),
            },
            Screen::OpenCopy => match open_copy::show(ui, &mut self.open_copy) {
                OpenCopyAction::None => {}
                OpenCopyAction::Back => {
                    self.screen = Screen::Home;
                    self.status.say("Back at your guides.");
                }
                OpenCopyAction::Browse => {
                    // Start where copies are saved, or in the folder of the file already named.
                    let named = PathBuf::from(self.open_copy.clean_path());
                    let start = named
                        .parent()
                        .filter(|p| p.is_dir())
                        .map(Path::to_path_buf)
                        .unwrap_or(self.export.folder.clone());
                    if let Some(chosen) = (self.file_chooser)(&start) {
                        self.open_copy.path = chosen.display().to_string();
                        self.open_copy.said = None;
                        self.status.say(format!("Chosen: {}. Press Open the copy.", chosen.display()));
                    }
                }
                OpenCopyAction::Open => self.open_a_copy(),
            },
            Screen::DeletedSteps => {
                let Some(editor) = self.editor.as_mut() else {
                    self.screen = Screen::Home;
                    return;
                };
                let rows = editor.deleted_rows();
                match deleted::show(ui, &rows, &mut self.deleted) {
                    DeletedAction::None => {}
                    DeletedAction::Back => {
                        self.screen = Screen::Editor;
                        self.deleted.stop_asking(false);
                        editor.focus_row_next = true;
                        self.status.say("Back at the guide.");
                    }
                    DeletedAction::Restore(id) => {
                        let at = rows.iter().position(|r| r.id == id).unwrap_or(0);
                        self.status.say_opt(editor.restore_deleted(&id));
                        // The button that was pressed is gone: the keyboard goes to the next one, or to Back if none is left.
                        self.deleted.keyboard = Some(at);
                    }
                    DeletedAction::RestoreAll => {
                        self.status.say_opt(editor.restore_all_deleted());
                        self.deleted.keyboard = Some(0);
                    }
                    DeletedAction::AskToEmpty => {
                        self.deleted.ask();
                        self.status.say(format!(
                            "Empty the trash for good? This deletes {} for good, and their pictures. It cannot be undone. \
                             Press Keep them, or Empty the trash for good.",
                            steps_words(rows.len())
                        ));
                    }
                    DeletedAction::KeepThem => {
                        self.deleted.stop_asking(true);
                        self.status.say("Kept the deleted steps. Nothing was deleted.");
                    }
                    DeletedAction::EmptyForGood => {
                        self.deleted.stop_asking(false);
                        self.status.say_opt(editor.empty_deleted());
                        // Nothing is left to press: the keyboard goes to Back (or stays on the list if the trash could not be emptied).
                        self.deleted.keyboard = Some(0);
                    }
                }
            }
            Screen::Bar => {
                let view = match &self.capture {
                    // The hotkeys are the ones set now, which the person can change while the program runs.
                    Some(capture) => BarView { hotkeys: self.hotkey_words.clone(), ..capture.view() },
                    None => BarView {
                        target: self.target.kind,
                        can_start: self.target.is_ready(),
                        ..BarView::ready(&self.target.words(&self.what), &self.hotkey_words)
                    },
                };
                let action = bar::show(ui, &view);
                self.bar_action(action);
            }
            Screen::ChooseWindow => match choose_window::show(ui, &mut self.window_list) {
                WindowListAction::None => {}
                WindowListAction::Back => {
                    self.screen = Screen::Bar;
                    self.status.say("Back at the capture bar.");
                }
                WindowListAction::Refresh => self.open_window_list(),
                WindowListAction::Use(i) => {
                    if let Some(row) = self.window_list.rows.get(i) {
                        self.target.window = Some(WindowChoice {
                            handle: row.handle,
                            title: row.title.clone(),
                            process: row.process.clone(),
                        });
                        self.target.kind = TargetKind::Window;
                        self.settings.capture_target = TargetKind::Window;
                        self.save_settings();
                        self.screen = Screen::Bar;
                        self.status.say(format!(
                            "Capturing {}. Press Start when you are ready.",
                            self.target.words(&self.what)
                        ));
                    }
                }
            },
            Screen::ChooseArea => {
                let Some(area) = self.area_screen.as_mut() else {
                    self.screen = Screen::Bar;
                    return;
                };
                match choose_area::show(ui, area) {
                    AreaAction::None => {}
                    AreaAction::Back => {
                        self.screen = Screen::Bar;
                        self.status.say("Back at the capture bar.");
                    }
                    AreaAction::ChangeDisplay(n) => match self.desktop.snapshot(n) {
                        Ok(picture) => area.show_display(n, picture),
                        Err(why) => self.status.say(format!("A picture of that display could not be taken. {why}")),
                    },
                    AreaAction::Use => {
                        let r = area.editor.rect();
                        let chosen =
                            AreaChoice { display: area.display, x: r.x, y: r.y, width: r.width, height: r.height };
                        self.target.area = Some(chosen);
                        self.target.kind = TargetKind::Area;
                        self.settings.capture_target = TargetKind::Area;
                        self.settings.last_area = Some(chosen);
                        self.save_settings();
                        self.screen = Screen::Bar;
                        self.status.say(format!(
                            "Capturing {}. Press Start when you are ready.",
                            self.target.words(&self.what)
                        ));
                    }
                }
            }
            Screen::About => {
                let view = about::AboutView { checking: self.update_job.is_some(), answer: self.update_answer.as_ref() };
                match about::show(ui, &view) {
                    about::AboutAction::None => {}
                    about::AboutAction::Back => {
                        self.screen = Screen::Home;
                        self.status.say("Back at your guides.");
                    }
                    about::AboutAction::Check => self.start_update_check(),
                    about::AboutAction::OpenPage(url) => {
                        (self.page_opener)(&url);
                        self.status.say("The release page is opening in your browser.");
                    }
                }
            }
            Screen::Settings => {
                let info = SettingsInfo {
                    guides_folder: self.library.display().to_string(),
                    exports_folder: self.export.folder.display().to_string(),
                    hotkeys: self.hotkey_words.clone(),
                    note: self.settings_note.clone(),
                    scroll: self.settings_scroll,
                };
                let shown = settings_screen::show(ui, &mut self.settings, &mut self.hotkey_edit, &info);
                if let Some(said) = self.hotkey_edit.take_said() {
                    self.status.say(said);
                }
                match shown {
                    SettingsAction::None => {}
                    SettingsAction::Hear(cue) => {
                        // The person pressed the button for it, so it plays whether or not that sound is turned on.
                        self.cues.play(cue);
                        self.status.say(format!("Played the {} sound.", cue.name()));
                    }
                    SettingsAction::HotkeysChanged => {
                        self.hotkey_change = Some(self.settings.hotkeys);
                        self.hotkey_words = hotkey::sentence(&self.settings.hotkeys.entries());
                        self.save_settings();
                        self.status.say(format!("Saved. {}", self.hotkey_words));
                    }
                    SettingsAction::Back => {
                        self.screen = Screen::Home;
                        self.status.say("Back at your guides.");
                    }
                    SettingsAction::ChooseGuidesFolder => {
                        if let Some(chosen) = (self.chooser)(&self.library).filter(|c| {
                            let network = openstage_express_core::fsutil::is_network_path(c);
                            if network {
                                self.status.say("Express keeps guides and exports on this computer, so a folder on another computer (\\\\server\\share) is not used. A network drive with a letter, such as Z:, can be chosen.");
                            }
                            !network
                        }) {
                            self.settings.guides_folder = Some(chosen.display().to_string());
                            self.library = chosen;
                            self.refresh_home();
                            self.save_settings();
                            self.status.say(format!("Guides are now kept in {}.", self.library.display()));
                        }
                    }
                    SettingsAction::ChooseExportsFolder => {
                        if let Some(chosen) = (self.chooser)(&self.export.folder).filter(|c| {
                            let network = openstage_express_core::fsutil::is_network_path(c);
                            if network {
                                self.status.say("Express keeps guides and exports on this computer, so a folder on another computer (\\\\server\\share) is not used. A network drive with a letter, such as Z:, can be chosen.");
                            }
                            !network
                        }) {
                            self.settings.exports_folder = Some(chosen.display().to_string());
                            self.export.folder = chosen;
                            self.save_settings();
                            self.status.say(format!("Exports will go into {}.", self.export.folder.display()));
                        }
                    }
                    SettingsAction::Changed => {
                        self.export.open_after = self.settings.open_folder_after_export;
                        self.save_settings();
                        self.status.say("Saved.");
                    }
                }
            }
            Screen::Export => {
                if self.editor.is_none() {
                    self.screen = Screen::Home;
                    return;
                }
                match export::show(ui, &mut self.export) {
                    ExportAction::None => {}
                    ExportAction::Back => {
                        self.screen = Screen::Editor;
                        self.status.say("Back at the guide.");
                    }
                    ExportAction::ChooseFolder => {
                        if let Some(chosen) = (self.chooser)(&self.export.folder) {
                            self.status.say(format!("Exports will go into {}.", chosen.display()));
                            self.export.folder = chosen;
                        }
                    }
                    ExportAction::Export => self.start_export(),
                    ExportAction::SaveCopy => self.start_copy(),
                }
                self.sync_export_settings();
            }
            Screen::Editor => {
                let Some(editor_state) = self.editor.as_mut() else {
                    self.screen = Screen::Home;
                    return;
                };
                match editor::show(ui, editor_state, &mut self.status) {
                    Leave::Stay => {}
                    Leave::ToDeleted => self.open_deleted_steps(),
                }
            }
        }
    }
}
