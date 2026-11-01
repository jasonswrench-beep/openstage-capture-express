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

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use openstage_express_core::builder::{FieldText, FocusedField, Input, Ms, Settings, Target};
use openstage_express_core::capture::{Area, CaptureEngine, Captured, FieldReader, RING_BYTES};
use windows::Win32::Foundation::POINT;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowThreadProcessId, WindowFromPoint};

use crate::hotkeys::{Hotkey, DEFAULTS};
use crate::input::{Hooks, Raw};
use crate::screen::{area_of, display_of, displays, dpi_percent, window_area, window_is_open, ScreenFeed};
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

/// What part of the screen a run captures (spec C3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureTarget {
    /// The whole of a display.
    Screen,
    /// One window, by Windows' number for it. The display it is on is captured whole and each step's picture is cut to the window's
    /// frame as it is at the click, so a menu or a tooltip that is still open stays in the picture when it lies inside the frame.
    Window { handle: isize },
    /// A rectangle of a display, in pixels from that display's top left corner.
    Region { x: u32, y: u32, width: u32, height: u32 },
}

pub struct RunOptions {
    pub settings: Settings,
    /// 1 is the main display. A window is captured on the display it is on, whatever this says.
    pub display_number: u32,
    pub target: CaptureTarget,
    /// Express's own hotkeys, which the keyboard hook must never count as typing.
    pub hotkeys: Vec<Hotkey>,
}

impl Default for RunOptions {
    fn default() -> RunOptions {
        RunOptions {
            settings: Settings::default(),
            display_number: 1,
            target: CaptureTarget::Screen,
            hotkeys: DEFAULTS.iter().map(|(_, key)| *key).collect(),
        }
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
        let all = displays()?;
        let display_number = match options.target {
            CaptureTarget::Window { handle } => {
                let frame = window_area(handle)
                    .ok_or("That window is not open any more, or it is minimised. Choose it again.")?;
                display_of(&frame, &all).map(|d| d.number).unwrap_or(options.display_number)
            }
            _ => options.display_number,
        };
        let feed = ScreenFeed::start(display_number, PICTURE_GAP)?;
        let display_area = area_of(feed.display());
        let fixed = match options.target {
            CaptureTarget::Region { x, y, width, height } => {
                let wanted = Area { x: display_area.x + x as i32, y: display_area.y + y as i32, width, height };
                Some(wanted.intersect(&display_area).ok_or("The area is not on the display. Draw it again.")?)
            }
            _ => None,
        };
        let hooks = Hooks::start(&options.hotkeys)?;
        let (area, percent, display) = {
            let d = feed.display();
            let area = match options.target {
                CaptureTarget::Window { handle } => {
                    window_area(handle).and_then(|w| w.intersect(&display_area)).unwrap_or(display_area)
                }
                _ => fixed.unwrap_or(display_area),
            };
            (area, dpi_percent(d), d.describe())
        };
        let (commands, command_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let settings = options.settings;
        let target = options.target;
        let thread = std::thread::Builder::new()
            .name("express-noting".into())
            .spawn(move || {
                noting(settings, Where { display: display_area, target, fixed }, hooks, feed, command_rx, event_tx)
            })
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

/// Which part of the desktop the pictures are cut to, and how it is found.
struct Where {
    /// The whole display the pictures are taken of.
    display: Area,
    target: CaptureTarget,
    /// The rectangle of a region, which never moves.
    fixed: Option<Area>,
}

impl Where {
    /// The area now. A window's frame is asked for each time, since the window can move; last keeps the last answer. None when
    /// the window cannot be found (closed or minimised).
    fn now(&self, last: &mut Area) -> Option<Area> {
        match self.target {
            CaptureTarget::Screen => Some(self.display),
            CaptureTarget::Region { .. } => self.fixed,
            CaptureTarget::Window { handle } => {
                let frame = window_area(handle)?.intersect(&self.display)?;
                *last = frame;
                Some(frame)
            }
        }
    }

    /// Cut a finished step's picture to the area it was made in: the area stamped at its click, or the latest before it.
    fn cut(&self, captured: Captured, stamps: &VecDeque<(Ms, Area)>, last: Area) -> Option<Captured> {
        if self.target == CaptureTarget::Screen {
            return Some(captured);
        }
        let area = stamps.iter().rev().find(|(at, _)| *at <= captured.draft.at).map(|(_, a)| *a).unwrap_or(last);
        if area == self.display {
            return Some(captured);
        }
        let picture =
            captured.picture.crop(area.x - self.display.x, area.y - self.display.y, area.width, area.height)?;
        Some(Captured { draft: captured.draft, picture })
    }
}

fn noting(
    settings: Settings,
    place: Where,
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
    let mut outside = 0u32;
    let mut told_outside = 0u32;
    let mut last_area = place.fixed.unwrap_or(place.display);
    let mut stamps: VecDeque<(Ms, Area)> = VecDeque::new();
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
                match place.cut(captured, &stamps, last_area) {
                    Some(cut) => {
                        summary.steps += 1;
                        say(&events, RunEvent::Step(cut));
                    }
                    None => outside += 1,
                }
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
                    let area = place.now(&mut last_area).unwrap_or(last_area);
                    stamps.push_back((now, area));
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
                        let found = place.now(&mut last_area);
                        let area = found.unwrap_or(last_area);
                        stamps.push_back((at, area));
                        if stamps.len() > 64 {
                            stamps.pop_front();
                        }
                        let (px, py) = area.to_picture(x, y);
                        Input::Click {
                            at,
                            button,
                            x: px,
                            y: py,
                            target: element.map(|e| Target { name: e.name, role: e.role, editable: e.editable }),
                            mods,
                            inside_area: found.is_some() && area.contains(x, y),
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
            if outside > told_outside {
                told_outside = outside;
                say(
                    &events,
                    RunEvent::Notice(
                        "A step's picture was outside the part of the screen being captured, so the step was left out. Is the window on another display?"
                            .to_string(),
                    ),
                );
            }
            if let CaptureTarget::Window { handle } = place.target {
                if !window_is_open(handle) {
                    say(
                        &events,
                        RunEvent::Notice(
                            "The window was closed, so capturing has stopped. The steps so far are kept.".to_string(),
                        ),
                    );
                    let now = ms(Instant::now());
                    emit!(engine.settle(now, true, &mut fields));
                    break;
                }
            }
            if let Some(why) = feed.ended() {
                say(&events, RunEvent::Notice(format!("The screen capture stopped: {why} The steps so far are kept.")));
                let now = ms(Instant::now());
                emit!(engine.settle(now, true, &mut fields));
                break;
            }
        }
    }

    summary.lost = engine.lost() + outside;
    summary.dropped_events = hooks.dropped();
    summary.skipped_pictures = feed.skipped();
    say(&events, RunEvent::State(RunState::Finished));
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstage_express_core::builder::{Draft, DraftKind, ShotId};
    use openstage_express_core::capture::Picture;

    /// A display picture whose pixel at (x, y) is (x mod 256, y mod 256), so a crop shows where it came from.
    fn display_picture(width: u32, height: u32) -> Picture {
        let mut bgra = Vec::new();
        for y in 0..height {
            for x in 0..width {
                bgra.extend_from_slice(&[x as u8, y as u8, 7, 255]);
            }
        }
        Picture::new(width, height, bgra).unwrap()
    }

    fn captured(at: Ms, picture: Picture) -> Captured {
        Captured {
            draft: Draft {
                kind: DraftKind::Click,
                title: "Click here".into(),
                shot: ShotId(1),
                click: None,
                control: None,
                typed: None,
                at,
            },
            picture,
        }
    }

    fn place(target: CaptureTarget, fixed: Option<Area>) -> Where {
        Where { display: Area { x: -1000, y: 0, width: 400, height: 300 }, target, fixed }
    }

    #[test]
    fn the_whole_screen_is_left_as_it_is() {
        let w = place(CaptureTarget::Screen, None);
        let out = w.cut(captured(5, display_picture(400, 300)), &VecDeque::new(), w.display).unwrap();
        assert_eq!((out.picture.width, out.picture.height), (400, 300));
    }

    #[test]
    fn a_region_is_cut_from_the_display_with_a_negative_corner() {
        let fixed = Area { x: -900, y: 50, width: 120, height: 80 };
        let w = place(CaptureTarget::Region { x: 100, y: 50, width: 120, height: 80 }, Some(fixed));
        let out = w.cut(captured(5, display_picture(400, 300)), &VecDeque::new(), fixed).unwrap();
        assert_eq!((out.picture.width, out.picture.height), (120, 80));
        assert_eq!(&out.picture.bgra[0..2], &[100, 50], "its first pixel is the display's pixel 100, 50");
    }

    #[test]
    fn a_window_step_is_cut_to_the_frame_it_had_at_its_click_not_the_frame_it_has_now() {
        let w = place(CaptureTarget::Window { handle: 1 }, None);
        let before = Area { x: -950, y: 20, width: 100, height: 60 };
        let after = Area { x: -800, y: 120, width: 100, height: 60 };
        let stamps: VecDeque<(Ms, Area)> = [(100, before), (500, after)].into_iter().collect();
        let early = w.cut(captured(300, display_picture(400, 300)), &stamps, after).unwrap();
        assert_eq!(&early.picture.bgra[0..2], &[50, 20], "the frame at 100, which was still the latest at 300");
        let late = w.cut(captured(600, display_picture(400, 300)), &stamps, after).unwrap();
        assert_eq!(&late.picture.bgra[0..2], &[200, 120], "the frame at 500");
        let first = w.cut(captured(10, display_picture(400, 300)), &stamps, after).unwrap();
        assert_eq!(&first.picture.bgra[0..2], &[200, 120], "before any click is known, the last frame found");
        assert_eq!((early.picture.width, early.picture.height), (100, 60));
    }

    #[test]
    fn a_window_that_is_off_the_captured_display_gives_no_step_picture() {
        let w = place(CaptureTarget::Window { handle: 1 }, None);
        let elsewhere = Area { x: 5000, y: 5000, width: 100, height: 100 };
        let stamps: VecDeque<(Ms, Area)> = [(100, elsewhere)].into_iter().collect();
        assert!(w.cut(captured(300, display_picture(400, 300)), &stamps, elsewhere).is_none());
    }

    #[test]
    fn a_window_that_is_the_whole_display_needs_no_cutting() {
        let w = place(CaptureTarget::Window { handle: 1 }, None);
        let stamps: VecDeque<(Ms, Area)> = [(100, w.display)].into_iter().collect();
        let out = w.cut(captured(300, display_picture(400, 300)), &stamps, w.display).unwrap();
        assert_eq!((out.picture.width, out.picture.height), (400, 300));
    }

    #[test]
    fn the_area_now_for_a_region_is_fixed_and_for_the_screen_is_the_display() {
        let fixed = Area { x: -900, y: 50, width: 120, height: 80 };
        let mut last = fixed;
        assert_eq!(
            place(CaptureTarget::Region { x: 0, y: 0, width: 1, height: 1 }, Some(fixed)).now(&mut last),
            Some(fixed)
        );
        let screen = place(CaptureTarget::Screen, None);
        assert_eq!(screen.now(&mut last), Some(screen.display));
    }
}
