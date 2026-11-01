// SPDX-License-Identifier: Apache-2.0
//! A capture run: the pieces of this crate put together (spec sections 6.4 and 8.1).
//!
//! ```text
//! hook thread (mouse, keyboard)   stamps each event with the time and queues it; never waits
//! screen thread                   Windows Graphics Capture pictures of the display, a few a second
//!   -> noting thread (this file)  asks UI Automation what was clicked, runs the capture engine (the step builder with its
//!                                 frame ring), and hands finished steps to whoever started the run
//! ```
//!
//! The run does not write anything. It sends [`RunEvent::Step`] with the step's picture; the program that started it encodes and
//! saves the picture and the guide, so a slow disk never slows the noting of clicks.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use openstage_express_core::builder::{FieldText, FocusedField, Input, Ms, Settings, Target};
use openstage_express_core::capture::{Area, CaptureEngine, Captured, FieldReader, RING_BYTES};
use windows::Win32::Foundation::POINT;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowThreadProcessId, WindowFromPoint};

use crate::input::{Hooks, Raw};
use crate::screen::{area_of, displays, dpi_percent, ScreenFeed};
use crate::uia::Uia;

/// How often the engine is told that time has passed, so that waits can end.
const TICK: Duration = Duration::from_millis(50);
/// The closest two pictures of the screen are kept to each other. With the 64 megabyte ring this is about a third of a second
/// of pictures at 1080p.
const PICTURE_GAP: Duration = Duration::from_millis(40);

/// What the person (or a hotkey) can ask of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Pause,
    Resume,
    /// Settle everything waiting and stop.
    Finish,
    /// Stop now. Whatever was waiting to become a step is given up.
    Cancel,
    /// Make a step from the screen as it is now, without a click.
    CaptureNow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Capturing,
    Paused,
    Finished,
}

/// What a run tells the program that started it.
#[derive(Debug, Clone)]
pub enum RunEvent {
    State(RunState),
    /// A step is ready, with its picture.
    Step(Captured),
    /// Something to tell the person, in words (the run keeps going).
    Notice(String),
}

/// How a run went, for the bar and for the tests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunSummary {
    pub steps: u32,
    /// Steps that could not be made because no picture of the screen ever arrived for them.
    pub lost: u32,
    /// Mouse and keyboard happenings thrown away because the noting thread fell behind.
    pub dropped_events: u64,
    /// Pictures of the screen thrown away because the program was too slow to take them.
    pub skipped_pictures: u64,
    pub cancelled: bool,
}

pub struct RunOptions {
    pub settings: Settings,
    /// 1 is the main display.
    pub display_number: u32,
}

impl Default for RunOptions {
    fn default() -> RunOptions {
        RunOptions { settings: Settings::default(), display_number: 1 }
    }
}

/// A run in progress. Dropping it cancels the run and takes the hooks out.
pub struct CaptureRun {
    /// The part of the desktop being captured, in physical pixels.
    pub area: Area,
    /// The display's scaling in percent.
    pub dpi_percent: u32,
    /// For example "Display 1, 1920 by 1080, main display".
    pub display: String,
    commands: Sender<Command>,
    events: Receiver<RunEvent>,
    thread: Option<JoinHandle<RunSummary>>,
}

impl CaptureRun {
    /// Start capturing. Fails, in words, if Windows will not give the pictures or let the hooks in.
    pub fn start(options: RunOptions) -> Result<CaptureRun, String> {
        // Asking for the displays also makes this program aware of display scaling, which the click positions rely on.
        displays()?;
        let feed = ScreenFeed::start(options.display_number, PICTURE_GAP)?;
        let hooks = Hooks::start()?;
        let (area, percent, display) = {
            let d = feed.display();
            (area_of(d), dpi_percent(d), d.describe())
        };
        let (commands, command_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let settings = options.settings;
        let thread = std::thread::Builder::new()
            .name("express-noting".into())
            .spawn(move || noting(settings, area, hooks, feed, command_rx, event_tx))
            .map_err(|e| format!("The program could not start its capture thread: {e}"))?;
        Ok(CaptureRun { area, dpi_percent: percent, display, commands, events, thread: Some(thread) })
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    /// The next thing the run has to say, if it has already said it.
    pub fn try_event(&self) -> Option<RunEvent> {
        self.events.try_recv().ok()
    }

    /// The next thing the run says, waiting up to `wait` for it.
    pub fn event(&self, wait: Duration) -> Option<RunEvent> {
        self.events.recv_timeout(wait).ok()
    }

    /// Wait for the run to end (after a `Finish` or `Cancel`) and say how it went. Events not yet read are lost to the caller
    /// only if it has stopped reading them; read until [`RunState::Finished`] first.
    pub fn join(mut self) -> RunSummary {
        self.thread.take().map(|t| t.join().unwrap_or_default()).unwrap_or_default()
    }
}

impl Drop for CaptureRun {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = self.commands.send(Command::Cancel);
            let _ = thread.join();
        }
    }
}

/// Reads the field that has the keyboard, on the noting thread.
struct UiaFields<'a>(Option<&'a Uia>);

impl FieldReader for UiaFields<'_> {
    fn focused_field(&mut self) -> FocusedField {
        match self.0 {
            Some(uia) => uia.focused_field(),
            None => FocusedField { name: String::new(), role: String::new(), text: FieldText::Unreadable },
        }
    }
}

/// Whether the window at this point belongs to this program (used only when UI Automation cannot say).
fn window_at_is_ours(x: i32, y: i32) -> bool {
    unsafe {
        let hwnd = WindowFromPoint(POINT { x, y });
        if hwnd.0.is_null() {
            return false;
        }
        let mut owner = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut owner));
        owner == GetCurrentProcessId()
    }
}

fn cursor() -> Option<(i32, i32)> {
    let mut at = POINT::default();
    unsafe { GetCursorPos(&mut at) }.ok().map(|_| (at.x, at.y))
}

fn noting(
    settings: Settings,
    area: Area,
    hooks: Hooks,
    feed: ScreenFeed,
    commands: Receiver<Command>,
    events: Sender<RunEvent>,
) -> RunSummary {
    let uia = Uia::new();
    let mut fields = UiaFields(uia.as_ref());
    let mut engine = CaptureEngine::new(settings, RING_BYTES);
    let started = Instant::now();
    let ms = |at: Instant| -> Ms { at.saturating_duration_since(started).as_millis() as Ms };
    let mut summary = RunSummary::default();
    let mut state = RunState::Capturing;
    let mut last_tick = started;
    let mut told_dropped = 0u64;
    let mut told_lost = 0u32;
    let receiver_gone = std::cell::Cell::new(false);

    let say = |events: &Sender<RunEvent>, event: RunEvent| {
        if events.send(event).is_err() {
            receiver_gone.set(true);
        }
    };
    if uia.is_none() {
        say(
            &events,
            RunEvent::Notice(
                "Windows' accessibility information is not available, so the names of the things you click are not noted and \
                 typed text cannot be read. Steps will say \"Click here\"."
                    .to_string(),
            ),
        );
    }
    say(&events, RunEvent::State(RunState::Capturing));

    macro_rules! emit {
        ($steps:expr) => {
            for captured in $steps {
                summary.steps += 1;
                say(&events, RunEvent::Step(captured));
            }
        };
    }

    'run: loop {
        if receiver_gone.get() {
            summary.cancelled = true;
            break;
        }
        // What the person or a hotkey asked for.
        loop {
            match commands.try_recv() {
                Ok(Command::Pause) if state == RunState::Capturing => {
                    hooks.set_active(false);
                    let now = ms(Instant::now());
                    emit!(engine.settle(now, false, &mut fields));
                    state = RunState::Paused;
                    say(&events, RunEvent::State(RunState::Paused));
                }
                Ok(Command::Resume) if state == RunState::Paused => {
                    hooks.set_active(true);
                    state = RunState::Capturing;
                    say(&events, RunEvent::State(RunState::Capturing));
                }
                Ok(Command::CaptureNow) if state == RunState::Capturing => {
                    let now = ms(Instant::now());
                    let at = cursor().map(|(x, y)| area.to_picture(x, y));
                    emit!(engine.input(Input::CaptureNow { at: now, cursor: at }, &mut fields));
                }
                Ok(Command::Finish) => {
                    let now = ms(Instant::now());
                    emit!(engine.settle(now, true, &mut fields));
                    break 'run;
                }
                Ok(Command::Cancel) | Err(TryRecvError::Disconnected) => {
                    summary.cancelled = true;
                    break 'run;
                }
                Ok(_) => {}
                Err(TryRecvError::Empty) => break,
            }
        }
        // Pictures of the screen that have arrived.
        while let Ok((at, picture)) = feed.next(Duration::ZERO) {
            emit!(engine.push_frame(ms(at), picture));
        }
        // One thing the hooks saw.
        match hooks.next(Duration::from_millis(15)) {
            Ok(stamped) if state == RunState::Capturing => {
                let at = ms(stamped.at);
                let input = match stamped.raw {
                    Raw::Click { x, y, button, mods } => {
                        let element = uia.as_ref().and_then(|u| u.element_at(x, y));
                        let own_window = match &element {
                            Some(e) => e.process_id == unsafe { GetCurrentProcessId() },
                            None => window_at_is_ours(x, y),
                        };
                        let (px, py) = area.to_picture(x, y);
                        Input::Click {
                            at,
                            button,
                            x: px,
                            y: py,
                            target: element.map(|e| Target { name: e.name, role: e.role, editable: e.editable }),
                            mods,
                            inside_area: area.contains(x, y),
                            own_window,
                        }
                    }
                    Raw::Typing => Input::Typing { at },
                    Raw::Special(key) => Input::Key { at, key },
                };
                emit!(engine.input(input, &mut fields));
            }
            Ok(_) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                summary.cancelled = true;
                break;
            }
        }
        // Time passing, and the things worth telling the person.
        let now = Instant::now();
        if now.duration_since(last_tick) >= TICK {
            last_tick = now;
            if state == RunState::Capturing {
                emit!(engine.tick(ms(now), &mut fields));
            }
            let dropped = hooks.dropped();
            if dropped > told_dropped {
                told_dropped = dropped;
                say(
                    &events,
                    RunEvent::Notice(format!(
                        "Express could not keep up, so {dropped} mouse or keyboard actions were not noted. Steps may be missing."
                    )),
                );
            }
            if engine.lost() > told_lost {
                told_lost = engine.lost();
                say(
                    &events,
                    RunEvent::Notice(
                        "A step could not be made because Windows gave no picture of the screen for it.".to_string(),
                    ),
                );
            }
            if let Some(why) = feed.ended() {
                say(&events, RunEvent::Notice(format!("The screen capture stopped: {why} The steps so far are kept.")));
                let now = ms(Instant::now());
                emit!(engine.settle(now, true, &mut fields));
                break;
            }
        }
    }

    summary.lost = engine.lost();
    summary.dropped_events = hooks.dropped();
    summary.skipped_pictures = feed.skipped();
    say(&events, RunEvent::State(RunState::Finished));
    summary
}
