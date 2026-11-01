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
use openstage_express_core::cues::Cue;
use openstage_express_windows::run::{CaptureRun, CaptureTarget, Command, RunEvent, RunOptions, RunState, RunSummary};
use openstage_express_windows::screen;

use crate::guide_writer::GuideWriter;
use crate::settings::AppSettings;
use crate::target::{Target, TargetKind};
use crate::ui::bar::{BarAction, BarPhase, BarView};
use crate::ui::capture::{CaptureControl, CuePlayer, Desktop, DisplayRow, Finished, Outcome, Starter, WindowRow};
use openstage_express_core::capture::Picture;

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

fn spawn_saver(parent: PathBuf, title: String, language: String, marks: MarkOptions) -> Saver {
    let (to, incoming) = mpsc::channel::<Captured>();
    let (reply, back) = mpsc::channel::<Saved>();
    let thread = std::thread::Builder::new()
        .name("express-saving".into())
        .spawn(move || {
            let mut writer: Option<GuideWriter> = None;
            for captured in incoming {
                if writer.is_none() {
                    match GuideWriter::create(&parent, &title, &language, marks) {
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
    kind: TargetKind,
    save_failures: Vec<String>,
    summary: Option<RunSummary>,
    outcome_given: bool,
    /// Whether the run has said it is capturing once (the first time is the start, not a resume).
    started: bool,
    /// Something went wrong or needs a look since the problem sound was last asked about.
    trouble: bool,
}

impl WindowsCapture {
    fn new(run: CaptureRun, parent: &Path, settings: &AppSettings, target: &Target) -> WindowsCapture {
        let what = target.words(&format!("the whole screen ({})", run.display));
        let marks = settings.mark_options(run.dpi_percent);
        WindowsCapture {
            run: Some(run),
            saver: spawn_saver(
                parent.to_path_buf(),
                format!("Guide {}", openstage_express_windows::system::local_time_stamp()),
                settings.title_language.code().to_string(),
                marks,
            ),
            phase: BarPhase::Capturing,
            steps: 0,
            last: None,
            what,
            kind: target.kind,
            save_failures: Vec::new(),
            summary: None,
            outcome_given: false,
            started: false,
            trouble: false,
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
            target: self.kind,
            can_start: false,
            // The window fills in the hotkeys, which the person can change while the program runs.
            hotkeys: String::new(),
        }
    }

    fn trouble(&mut self) -> bool {
        std::mem::take(&mut self.trouble)
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
                    RunEvent::Notice(words) => {
                        self.trouble = true;
                        said.push(words);
                    }
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
                    self.trouble = true;
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
                    self.trouble = true;
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
pub fn starter() -> Starter {
    Box::new(move |library: &Path, settings: &AppSettings, target: &Target| {
        let (what, display_number) = capture_target(target)?;
        let run = CaptureRun::start(RunOptions {
            settings: settings.builder_settings(),
            display_number,
            target: what,
            hotkeys: run_hotkeys(settings),
        })?;
        Ok(Box::new(WindowsCapture::new(run, library, settings, target)) as Box<dyn CaptureControl>)
    })
}

/// The keys the run must not count as typing: the person's own hotkeys.
pub fn run_hotkeys(settings: &AppSettings) -> Vec<openstage_express_windows::hotkeys::Hotkey> {
    settings.hotkeys.entries().iter().map(|(_, key)| *key).collect()
}

/// Plays the cue sounds through Windows. A person turns a sound on in Settings, and presses a button there to hear it.
pub struct WindowsCues;

impl CuePlayer for WindowsCues {
    fn play(&self, cue: Cue) {
        let _ = openstage_express_windows::sound::play(cue);
    }
}

/// What the person chose, as what a run captures, and the display to capture (a window is captured on the display it is on).
fn capture_target(target: &Target) -> Result<(CaptureTarget, u32), String> {
    match target.kind {
        TargetKind::Screen => Ok((CaptureTarget::Screen, 1)),
        TargetKind::Window => target
            .window
            .as_ref()
            .map(|w| (CaptureTarget::Window { handle: w.handle }, 1))
            .ok_or_else(|| "Choose the window to capture first.".to_string()),
        TargetKind::Area => target
            .area
            .map(|a| (CaptureTarget::Region { x: a.x, y: a.y, width: a.width, height: a.height }, a.display))
            .ok_or_else(|| "Draw the area to capture first.".to_string()),
    }
}

/// The real desktop: the open windows, the displays, and a picture of a display.
pub struct WindowsDesktop;

impl Desktop for WindowsDesktop {
    fn windows(&self) -> Result<Vec<WindowRow>, String> {
        Ok(screen::windows()?
            .into_iter()
            .map(|w| WindowRow { words: w.describe(), handle: w.handle, title: w.title, process: w.process })
            .collect())
    }

    fn displays(&self) -> Result<Vec<DisplayRow>, String> {
        Ok(screen::displays()?
            .into_iter()
            .map(|d| DisplayRow { number: d.number, width: d.width, height: d.height, words: d.describe() })
            .collect())
    }

    fn snapshot(&self, display: u32) -> Result<Picture, String> {
        let feed = screen::ScreenFeed::start(display, std::time::Duration::from_millis(40))?;
        feed.next(std::time::Duration::from_secs(5))
            .map(|(_, picture)| picture)
            .map_err(|_| "Windows gave no picture of the display in five seconds.".to_string())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_run_is_told_the_persons_own_hotkeys_not_the_usual_ones() {
        use openstage_express_core::hotkey::HotkeyAction;
        let mut settings = AppSettings::default();
        let usual: Vec<String> = run_hotkeys(&settings).iter().map(|k| k.describe()).collect();
        assert_eq!(usual, ["Ctrl+Shift+F7", "Ctrl+Shift+F8", "Ctrl+Shift+F9"]);
        settings.hotkeys.try_set(HotkeyAction::CaptureNow, "Alt+Shift+K").unwrap();
        let changed: Vec<String> = run_hotkeys(&settings).iter().map(|k| k.describe()).collect();
        assert_eq!(changed, ["Ctrl+Shift+F7", "Ctrl+Shift+F8", "Alt+Shift+K"]);
    }

    #[test]
    fn what_the_person_chose_becomes_what_a_run_captures() {
        use crate::target::{AreaChoice, WindowChoice};
        assert_eq!(capture_target(&Target::default()), Ok((CaptureTarget::Screen, 1)));
        let window = Target {
            kind: TargetKind::Window,
            window: Some(WindowChoice { handle: 77, title: "t".into(), process: "p.exe".into() }),
            area: None,
        };
        assert_eq!(capture_target(&window), Ok((CaptureTarget::Window { handle: 77 }, 1)));
        let area = Target {
            kind: TargetKind::Area,
            window: None,
            area: Some(AreaChoice { display: 2, x: 10, y: 20, width: 300, height: 200 }),
        };
        assert_eq!(capture_target(&area), Ok((CaptureTarget::Region { x: 10, y: 20, width: 300, height: 200 }, 2)));
        assert!(capture_target(&Target { kind: TargetKind::Window, ..Target::default() })
            .unwrap_err()
            .contains("Choose the window"));
        assert!(capture_target(&Target { kind: TargetKind::Area, ..Target::default() })
            .unwrap_err()
            .contains("Draw the area"));
        // Only the kind chosen counts: a window picked earlier does not make the area ready.
        assert!(capture_target(&Target { kind: TargetKind::Area, ..window }).is_err());
    }

    #[test]
    fn a_capture_of_nothing_leaves_no_guide_folder_behind() {
        let dir = tempfile::tempdir().unwrap();
        let mut saver =
            spawn_saver(dir.path().to_path_buf(), "Guide".to_string(), "en".to_string(), MarkOptions::default());
        saver.to = None; // nothing was ever sent
        let writer = saver.thread.take().unwrap().join().unwrap();
        assert!(writer.is_none());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
