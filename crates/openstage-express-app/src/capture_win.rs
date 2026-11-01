// SPDX-License-Identifier: Apache-2.0
//! The real capture behind the capture bar (Windows only): a [`CaptureRun`] (hooks, UI Automation, pictures of the screen) and a
//! thread that writes each step to the guide folder as it is made, so a slow disk never slows the noting of clicks and a crash
//! loses at most the step in flight (spec C10).
//!
//! The guide folder is made when the first step arrives, so a capture of nothing leaves nothing behind.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

use openstage_express_core::capture::{Captured, MarkOptions};
use openstage_express_windows::hotkeys::{Action, Hotkey, DEFAULTS};
use openstage_express_windows::run::{CaptureRun, Command, RunEvent, RunOptions, RunState, RunSummary};
use openstage_express_windows::screen;

use crate::guide_writer::GuideWriter;
use crate::ui::bar::{BarAction, BarPhase, BarView};
use crate::ui::capture::{CaptureControl, Finished, HotkeyAction, Outcome, Starter};

/// What the saving thread tells back.
enum Saved {
    Step { number: usize, title: String },
    Failed(String),
}

/// The thread that writes steps to disk.
struct Saver {
    to: Option<Sender<Captured>>,
    back: Receiver<Saved>,
    thread: Option<JoinHandle<Option<GuideWriter>>>,
}

fn spawn_saver(parent: PathBuf, title: String, marks: MarkOptions) -> Saver {
    let (to, incoming) = mpsc::channel::<Captured>();
    let (reply, back) = mpsc::channel::<Saved>();
    let thread = std::thread::Builder::new()
        .name("express-saving".into())
        .spawn(move || {
            let mut writer: Option<GuideWriter> = None;
            for captured in incoming {
                if writer.is_none() {
                    match GuideWriter::create(&parent, &title, "en", marks) {
                        Ok(w) => writer = Some(w),
                        Err(why) => {
                            let _ = reply.send(Saved::Failed(format!("The guide folder could not be made: {why}")));
                            continue;
                        }
                    }
                }
                if let Some(w) = writer.as_mut() {
                    let title = captured.draft.title.clone();
                    match w.add(&captured) {
                        Ok(_) => {
                            let _ = reply.send(Saved::Step { number: w.steps(), title });
                        }
                        Err(why) => {
                            let _ = reply.send(Saved::Failed(why));
                        }
                    }
                }
            }
            writer
        })
        .ok();
    Saver { to: Some(to), back, thread }
}

pub struct WindowsCapture {
    run: Option<CaptureRun>,
    saver: Saver,
    phase: BarPhase,
    steps: usize,
    last: Option<String>,
    what: String,
    hotkeys: String,
    save_failures: Vec<String>,
    summary: Option<RunSummary>,
    outcome_given: bool,
    /// Whether the run has said it is capturing once (the first time is the start, not a resume).
    started: bool,
}

impl WindowsCapture {
    fn new(run: CaptureRun, parent: &Path, hotkeys: &str) -> WindowsCapture {
        let what = format!("the whole screen ({})", run.display);
        let marks = MarkOptions { dpi_percent: run.dpi_percent, ..MarkOptions::default() };
        WindowsCapture {
            run: Some(run),
            saver: spawn_saver(
                parent.to_path_buf(),
                format!("Guide {}", openstage_express_windows::system::local_time_stamp()),
                marks,
            ),
            phase: BarPhase::Capturing,
            steps: 0,
            last: None,
            what,
            hotkeys: hotkeys.to_string(),
            save_failures: Vec::new(),
            summary: None,
            outcome_given: false,
            started: false,
        }
    }

    fn send(&self, command: Command) {
        if let Some(run) = &self.run {
            run.send(command);
        }
    }
}

impl CaptureControl for WindowsCapture {
    fn view(&self) -> BarView {
        BarView {
            phase: self.phase,
            steps: self.steps,
            last: self.last.clone(),
            what: self.what.clone(),
            hotkeys: self.hotkeys.clone(),
        }
    }

    fn act(&mut self, action: BarAction) {
        match action {
            BarAction::Pause if self.phase == BarPhase::Capturing => self.send(Command::Pause),
            BarAction::Resume if self.phase == BarPhase::Paused => self.send(Command::Resume),
            BarAction::CaptureNow if self.phase == BarPhase::Capturing => self.send(Command::CaptureNow),
            BarAction::Finish if matches!(self.phase, BarPhase::Capturing | BarPhase::Paused) => {
                self.phase = BarPhase::Saving;
                self.send(Command::Finish);
            }
            _ => {}
        }
    }

    fn poll(&mut self) -> Vec<String> {
        let mut said = Vec::new();
        let mut run_ended = false;
        if let Some(run) = &self.run {
            while let Some(event) = run.try_event() {
                match event {
                    RunEvent::Step(captured) => {
                        if let Some(to) = &self.saver.to {
                            let _ = to.send(captured);
                        }
                    }
                    RunEvent::Notice(words) => said.push(words),
                    RunEvent::State(RunState::Capturing) if self.phase != BarPhase::Saving => {
                        self.phase = BarPhase::Capturing;
                        if self.started {
                            said.push("Capturing again.".to_string());
                        }
                        self.started = true;
                    }
                    RunEvent::State(RunState::Paused) if self.phase != BarPhase::Saving => {
                        self.phase = BarPhase::Paused;
                        said.push(format!("Paused, {}.", crate::ui::rows::steps_words(self.steps)));
                    }
                    RunEvent::State(RunState::Finished) => run_ended = true,
                    RunEvent::State(_) => {}
                }
            }
        }
        if run_ended {
            self.phase = BarPhase::Saving;
            if let Some(run) = self.run.take() {
                let summary = run.join();
                if summary.lost > 0 {
                    said.push(format!(
                        "{} could not be made because Windows gave no picture of the screen for {}.",
                        crate::ui::rows::steps_words(summary.lost as usize),
                        if summary.lost == 1 { "it" } else { "them" }
                    ));
                }
                self.summary = Some(summary);
            }
            // No more steps will come: let the saving thread finish what it has.
            self.saver.to = None;
        }
        while let Ok(saved) = self.saver.back.try_recv() {
            match saved {
                Saved::Step { number, title } => {
                    self.steps = number;
                    said.push(format!("Step {number} captured: {title}."));
                    self.last = Some(title);
                }
                Saved::Failed(why) => {
                    said.push(format!("A step could not be saved. {why}"));
                    self.save_failures.push(why);
                }
            }
        }
        said
    }

    fn finished(&mut self) -> Option<Outcome> {
        if self.outcome_given || self.run.is_some() || self.saver.to.is_some() {
            return None;
        }
        let thread = self.saver.thread.as_ref()?;
        if !thread.is_finished() {
            return None;
        }
        self.outcome_given = true;
        let writer = self.saver.thread.take().and_then(|t| t.join().ok()).flatten();
        // A last look for what the saving thread said before it ended.
        while let Ok(Saved::Failed(why)) = self.saver.back.try_recv() {
            self.save_failures.push(why);
        }
        Some(match writer {
            Some(writer) => {
                let (folder, doc) = writer.into_parts();
                Outcome::Guide(Box::new(Finished { folder, doc }))
            }
            None if !self.save_failures.is_empty() => Outcome::Failed(self.save_failures.remove(0)),
            None => Outcome::Nothing,
        })
    }
}

/// How to start a capture into the guides folder.
pub fn starter(hotkeys: String) -> Starter {
    Box::new(move |library: &Path| {
        let run = CaptureRun::start(RunOptions::default())?;
        Ok(Box::new(WindowsCapture::new(run, library, &hotkeys)) as Box<dyn CaptureControl>)
    })
}

/// The words for the ready bar: what will be captured.
pub fn ready_words() -> String {
    match screen::displays() {
        Ok(all) => match screen::pick_display(&all, 1) {
            Some(display) => format!("the whole screen ({})", display.describe()),
            None => "the whole screen".to_string(),
        },
        Err(_) => "the whole screen".to_string(),
    }
}

/// What Express's hotkeys are, in a sentence, leaving out any that could not be set (`problems` comes from the hotkeys).
pub fn hotkey_sentence(problems: &[String]) -> String {
    let working: Vec<String> = DEFAULTS
        .iter()
        .filter(|(_, key)| !problems.iter().any(|p| p.contains(&key.describe())))
        .map(|(action, key)| format!("{} {}", key.describe(), action.words()))
        .collect();
    if working.is_empty() {
        String::new()
    } else {
        format!("Hotkeys: {}.", working.join(", "))
    }
}

/// Map a hotkey of the Windows crate to the window's own kind.
pub fn hotkey_action(action: Action) -> HotkeyAction {
    match action {
        Action::StartOrFinish => HotkeyAction::StartOrFinish,
        Action::PauseOrResume => HotkeyAction::PauseOrResume,
        Action::CaptureNow => HotkeyAction::CaptureNow,
    }
}

/// The defaults, for the window to register.
pub fn default_hotkeys() -> Vec<(Action, Hotkey)> {
    DEFAULTS.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hotkey_sentence_lists_the_ones_that_work() {
        assert_eq!(
            hotkey_sentence(&[]),
            "Hotkeys: Ctrl+Shift+F7 start or finish, Ctrl+Shift+F8 pause or resume, Ctrl+Shift+F9 capture now."
        );
        let taken =
            vec!["Ctrl+Shift+F8 (pause or resume) is already used by another program, so it is not set.".to_string()];
        assert_eq!(hotkey_sentence(&taken), "Hotkeys: Ctrl+Shift+F7 start or finish, Ctrl+Shift+F9 capture now.");
        let all: Vec<String> = DEFAULTS.iter().map(|(_, k)| format!("{} is taken", k.describe())).collect();
        assert_eq!(hotkey_sentence(&all), "");
    }

    #[test]
    fn hotkeys_map_one_to_one() {
        assert_eq!(hotkey_action(Action::CaptureNow), HotkeyAction::CaptureNow);
        assert_eq!(hotkey_action(Action::StartOrFinish), HotkeyAction::StartOrFinish);
        assert_eq!(hotkey_action(Action::PauseOrResume), HotkeyAction::PauseOrResume);
    }

    #[test]
    fn a_capture_of_nothing_leaves_no_guide_folder_behind() {
        let dir = tempfile::tempdir().unwrap();
        let mut saver = spawn_saver(dir.path().to_path_buf(), "Guide".to_string(), MarkOptions::default());
        saver.to = None; // nothing was ever sent
        let writer = saver.thread.take().unwrap().join().unwrap();
        assert!(writer.is_none());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
