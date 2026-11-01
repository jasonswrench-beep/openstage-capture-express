// SPDX-License-Identifier: Apache-2.0
//! The capture engine (spec sections 6.4, 8.1 and 8.5), with no Windows code in it. The Windows side hands it what it saw (clicks
//! and keys, already placed in the picture's own coordinates, and the frames of the screen) and answers its one question (what
//! is in the field that has the keyboard); the engine runs the step builder and hands back steps that are ready to be written,
//! each with its picture.
//!
//! * [`FrameRing`] keeps the last few pictures of the screen, bounded by memory, so that a click's picture is the one the person
//!   saw just before the click, with an open menu or a tooltip still in it (rule R11).
//! * [`CaptureEngine`] is the step builder's host. It gives the builder its pictures, holds back a step whose picture is still to
//!   be taken (the screenshot delay), and sends steps out in the order the actions happened (rule R12).
//! * [`step_draft`] turns a finished step into what the guide stores: the marker and the cursor as ordinary marks, and the
//!   automatic focused view.
//!
//! Time is whatever clock the caller likes, in milliseconds, as long as hooks, frames and ticks all use the same one.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use crate::builder::{Draft, FocusedField, Host, Input, Ms, Settings, ShotId, StepBuilder};
use crate::focus::{auto_focus, DEFAULT_ZOOM_PERCENT};
use crate::model::{Annotation, AnnotationKind, Click, StepImage};
use crate::ops::StepDraft;

/// How much memory the frame ring may use at the most (spec 8.5).
pub const RING_BYTES: usize = 64 * 1024 * 1024;

/// How long a step waits for a picture taken after a delay before it settles for the newest picture it has. A screen that is
/// not changing sends no new pictures, so waiting for one forever would hold the step up forever.
pub const DELAY_GRACE_MS: Ms = 120;

/// The size of the circle round a click, and of the pointer drawn at it, at 100 percent display scaling (spec 8.6).
pub const CLICK_MARK_SIZE: i32 = 56;
pub const CURSOR_SIZE: i32 = 32;

/// One picture of the screen: blue, green, red, alpha for every pixel, rows top to bottom, no padding. Cheap to copy: the pixels
/// are shared.
#[derive(Debug, Clone)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub bgra: Arc<Vec<u8>>,
}

impl Picture {
    /// `None` if the bytes are not exactly `width * height * 4`.
    pub fn new(width: u32, height: u32, bgra: Vec<u8>) -> Option<Picture> {
        let wanted = (width as usize).checked_mul(height as usize)?.checked_mul(4)?;
        (width > 0 && height > 0 && bgra.len() == wanted).then(|| Picture { width, height, bgra: Arc::new(bgra) })
    }

    pub fn bytes(&self) -> usize {
        self.bgra.len()
    }

    /// The part of this picture inside the rectangle x, y, width, height (measured from the picture's top left corner), as a
    /// picture of its own. The part of the rectangle that is outside the picture is left out. None if nothing of it is inside.
    pub fn crop(&self, x: i32, y: i32, width: u32, height: u32) -> Option<Picture> {
        let (left, top) = (i64::from(x).max(0), i64::from(y).max(0));
        let right = (i64::from(x) + i64::from(width)).min(i64::from(self.width));
        let bottom = (i64::from(y) + i64::from(height)).min(i64::from(self.height));
        if right <= left || bottom <= top {
            return None;
        }
        let (w, h) = ((right - left) as usize, (bottom - top) as usize);
        let stride = self.width as usize * 4;
        let mut out = Vec::with_capacity(w * h * 4);
        for row in 0..h {
            let from = (top as usize + row) * stride + left as usize * 4;
            out.extend_from_slice(&self.bgra[from..from + w * 4]);
        }
        Picture::new(w as u32, h as u32, out)
    }
}

/// A rectangle of the desktop in physical pixels. Its corner can be negative (a display to the left of the main one).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Area {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        let (x, y) = (x as i64, y as i64);
        x >= self.x as i64
            && y >= self.y as i64
            && x < self.x as i64 + self.width as i64
            && y < self.y as i64 + self.height as i64
    }

    /// The part this area shares with another, or None if they do not overlap.
    pub fn intersect(&self, other: &Area) -> Option<Area> {
        let left = i64::from(self.x).max(i64::from(other.x));
        let top = i64::from(self.y).max(i64::from(other.y));
        let right = (i64::from(self.x) + i64::from(self.width)).min(i64::from(other.x) + i64::from(other.width));
        let bottom = (i64::from(self.y) + i64::from(self.height)).min(i64::from(other.y) + i64::from(other.height));
        (right > left && bottom > top).then(|| Area {
            x: left as i32,
            y: top as i32,
            width: (right - left) as u32,
            height: (bottom - top) as u32,
        })
    }

    /// A point of the desktop in the coordinates of a picture of this area.
    pub fn to_picture(&self, x: i32, y: i32) -> (i32, i32) {
        (x.saturating_sub(self.x), y.saturating_sub(self.y))
    }
}

/// The last few pictures of the screen, newest last, bounded by memory.
#[derive(Debug)]
pub struct FrameRing {
    frames: VecDeque<(Ms, Picture)>,
    bytes: usize,
    max_bytes: usize,
}

impl FrameRing {
    pub fn new(max_bytes: usize) -> FrameRing {
        FrameRing { frames: VecDeque::new(), bytes: 0, max_bytes }
    }

    /// Add a picture taken at `at`. The oldest pictures go when the ring is over its memory, but the newest two always stay.
    pub fn push(&mut self, at: Ms, picture: Picture) {
        self.bytes += picture.bytes();
        self.frames.push_back((at, picture));
        while self.bytes > self.max_bytes && self.frames.len() > 2 {
            if let Some((_, old)) = self.frames.pop_front() {
                self.bytes -= old.bytes();
            }
        }
    }

    /// The picture for an action at `at`: the newest one taken at or before it, or, if every held picture is later, the oldest
    /// of them. `None` only when the ring is empty.
    pub fn before(&self, at: Ms) -> Option<Picture> {
        self.frames
            .iter()
            .rev()
            .find(|(t, _)| *t <= at)
            .or_else(|| self.frames.front())
            .map(|(_, picture)| picture.clone())
    }

    pub fn latest(&self) -> Option<Picture> {
        self.frames.back().map(|(_, picture)| picture.clone())
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

/// Asked, on the noting thread, for the field that has the keyboard.
pub trait FieldReader {
    fn focused_field(&mut self) -> FocusedField;
}

enum Shot {
    Ready(Picture),
    /// The picture is to be taken at `due` or later.
    Waiting {
        due: Ms,
    },
}

/// A step that is ready: what the builder made, and its picture.
#[derive(Debug, Clone)]
pub struct Captured {
    pub draft: Draft,
    pub picture: Picture,
}

struct EngineHost<'a> {
    ring: &'a FrameRing,
    shots: &'a mut HashMap<ShotId, Shot>,
    next: &'a mut u64,
    now: Ms,
    fields: &'a mut dyn FieldReader,
}

impl Host for EngineHost<'_> {
    fn screenshot(&mut self, at: Ms) -> ShotId {
        let id = ShotId(*self.next);
        *self.next += 1;
        let shot = if at > self.now {
            Shot::Waiting { due: at }
        } else {
            match self.ring.before(at) {
                Some(picture) => Shot::Ready(picture),
                None => Shot::Waiting { due: at },
            }
        };
        self.shots.insert(id, shot);
        id
    }

    fn focused_field(&mut self) -> FocusedField {
        self.fields.focused_field()
    }
}

/// The step builder with its pictures.
pub struct CaptureEngine {
    builder: StepBuilder,
    ring: FrameRing,
    shots: HashMap<ShotId, Shot>,
    next_shot: u64,
    queue: VecDeque<Draft>,
    lost: u32,
    last_now: Ms,
}

impl CaptureEngine {
    pub fn new(settings: Settings, ring_bytes: usize) -> CaptureEngine {
        CaptureEngine {
            builder: StepBuilder::new(settings),
            ring: FrameRing::new(ring_bytes),
            shots: HashMap::new(),
            next_shot: 1,
            queue: VecDeque::new(),
            lost: 0,
            last_now: 0,
        }
    }

    pub fn settings(&self) -> &Settings {
        self.builder.settings()
    }

    pub fn set_settings(&mut self, settings: Settings) {
        self.builder.set_settings(settings);
    }

    /// Steps that were dropped because no picture of the screen ever arrived for them.
    pub fn lost(&self) -> u32 {
        self.lost
    }

    pub fn ring(&self) -> &FrameRing {
        &self.ring
    }

    /// Whether the builder, or the queue of finished drafts, is holding something back.
    pub fn is_waiting(&self) -> bool {
        self.builder.is_waiting() || !self.queue.is_empty()
    }

    /// A new picture of the screen, taken at `at`. Steps that were waiting for a picture from this moment on are completed.
    pub fn push_frame(&mut self, at: Ms, picture: Picture) -> Vec<Captured> {
        self.last_now = self.last_now.max(at);
        self.ring.push(at, picture.clone());
        for shot in self.shots.values_mut() {
            if matches!(shot, Shot::Waiting { due } if *due <= at) {
                *shot = Shot::Ready(picture.clone());
            }
        }
        self.drain()
    }

    /// Something happened (a click, a key, the capture-now hotkey, time passing). Returns the steps that are now ready.
    pub fn input(&mut self, input: Input, fields: &mut dyn FieldReader) -> Vec<Captured> {
        let now = input.at().max(self.last_now);
        self.last_now = now;
        let drafts = {
            let mut host =
                EngineHost { ring: &self.ring, shots: &mut self.shots, next: &mut self.next_shot, now, fields };
            self.builder.push(&mut host, input)
        };
        self.queue.extend(drafts);
        for unused in self.builder.take_discarded() {
            self.shots.remove(&unused);
        }
        self.settle_late(now);
        self.drain()
    }

    /// Time passing. Call it a few times a second so that waits end.
    pub fn tick(&mut self, at: Ms, fields: &mut dyn FieldReader) -> Vec<Captured> {
        self.input(Input::Tick { at }, fields)
    }

    /// Capturing is finished or paused: settle everything waiting now, taking the newest picture for a step that wanted a
    /// later one. A step that never had a picture to take is counted in [`CaptureEngine::lost`].
    pub fn settle(&mut self, at: Ms, finish: bool, fields: &mut dyn FieldReader) -> Vec<Captured> {
        let input = if finish { Input::Finish { at } } else { Input::Pause { at } };
        let mut out = self.input(input, fields);
        if let Some(latest) = self.ring.latest() {
            for shot in self.shots.values_mut() {
                if matches!(shot, Shot::Waiting { .. }) {
                    *shot = Shot::Ready(latest.clone());
                }
            }
        }
        out.extend(self.drain());
        let stuck = self.queue.len() as u32;
        if stuck > 0 {
            self.lost += stuck;
            self.queue.clear();
            self.shots.clear();
        }
        out
    }

    /// A step waiting for a later picture takes the newest one once the grace time has passed (a still screen sends none).
    fn settle_late(&mut self, now: Ms) {
        let Some(latest) = self.ring.latest() else { return };
        for shot in self.shots.values_mut() {
            if matches!(shot, Shot::Waiting { due } if due.saturating_add(DELAY_GRACE_MS) <= now) {
                *shot = Shot::Ready(latest.clone());
            }
        }
    }

    /// Send out the drafts at the front of the queue whose pictures are ready, in order (R12).
    fn drain(&mut self) -> Vec<Captured> {
        let mut out = Vec::new();
        while let Some(front) = self.queue.front() {
            match self.shots.get(&front.shot) {
                Some(Shot::Ready(_)) => {
                    let draft = self.queue.pop_front().expect("the front was just seen");
                    if let Some(Shot::Ready(picture)) = self.shots.remove(&draft.shot) {
                        out.push(Captured { draft, picture });
                    }
                }
                Some(Shot::Waiting { .. }) => break,
                None => {
                    self.queue.pop_front();
                    self.lost += 1;
                }
            }
        }
        out
    }
}

/// How the marker, the cursor and the focused view are made for new steps (spec 8.6, 8.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkOptions {
    pub click_mark: bool,
    pub cursor: bool,
    /// The display's scaling in percent (100, 125, 150, ...), so the marks look the same size on any display.
    pub dpi_percent: u32,
    pub focused_view: bool,
    pub zoom_percent: u32,
}

impl Default for MarkOptions {
    fn default() -> MarkOptions {
        MarkOptions {
            click_mark: true,
            cursor: true,
            dpi_percent: 100,
            focused_view: true,
            zoom_percent: DEFAULT_ZOOM_PERCENT,
        }
    }
}

fn scaled(size: i32, dpi_percent: u32) -> i32 {
    ((size as i64 * dpi_percent.clamp(50, 400) as i64 + 50) / 100) as i32
}

/// The marker and cursor for a click, as ordinary marks the author can remove or restyle.
pub fn marks(click: Option<&Click>, role: Option<&str>, options: &MarkOptions) -> Vec<Annotation> {
    let Some(click) = click else { return Vec::new() };
    let mut out = Vec::new();
    // Ids follow the scheme `Document::add_annotation` uses (a1, a2, ...), so marks added later by hand never clash.
    if options.click_mark {
        let mut ring = Annotation::at(&format!("a{}", out.len() + 1), AnnotationKind::ClickMark, click.x, click.y);
        ring.size = Some(scaled(CLICK_MARK_SIZE, options.dpi_percent));
        out.push(ring);
    }
    if options.cursor {
        let mut cursor = Annotation::at(&format!("a{}", out.len() + 1), AnnotationKind::Cursor, click.x, click.y);
        cursor.style = Some(if matches!(role, Some("edit" | "document")) { "text" } else { "pointer" }.to_string());
        cursor.size = Some(scaled(CURSOR_SIZE, options.dpi_percent));
        out.push(cursor);
    }
    out
}

/// What the guide stores for a captured step, once its picture has been written as `image`.
pub fn step_draft(captured: &Captured, image: StepImage, options: &MarkOptions, captured_at: &str) -> StepDraft {
    let draft = &captured.draft;
    let role = draft.control.as_ref().map(|c| c.role.as_str());
    let mut step = StepDraft::with_picture(&draft.title, image);
    step.click = draft.click.clone();
    step.control = draft.control.clone();
    step.typed = draft.typed.clone();
    step.annotations = marks(draft.click.as_ref(), role, options);
    if options.focused_view {
        let click = draft.click.as_ref().map(|c| (c.x, c.y));
        step.focus = auto_focus(captured.picture.width, captured.picture.height, click, options.zoom_percent);
    }
    step.captured_at = Some(captured_at.to_string());
    step
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{FieldText, Key, ModKey, Mods, MouseButton, Target};

    fn picture(shade: u8) -> Picture {
        Picture::new(4, 2, vec![shade; 4 * 2 * 4]).expect("a 4 by 2 picture")
    }

    fn shade(p: &Picture) -> u8 {
        p.bgra[0]
    }

    struct Field(FocusedField);

    impl FieldReader for Field {
        fn focused_field(&mut self) -> FocusedField {
            self.0.clone()
        }
    }

    fn no_field() -> Field {
        Field(FocusedField { name: String::new(), role: String::new(), text: FieldText::Unreadable })
    }

    fn click(at: Ms, x: i32, y: i32) -> Input {
        Input::Click {
            at,
            button: MouseButton::Left,
            x,
            y,
            target: Some(Target::new("Save", "button")),
            mods: Mods::none(),
            inside_area: true,
            own_window: false,
        }
    }

    fn engine() -> CaptureEngine {
        CaptureEngine::new(Settings::default(), RING_BYTES)
    }

    // ---- the picture and the area

    #[test]
    fn a_picture_must_have_exactly_the_bytes_its_size_says() {
        assert!(Picture::new(4, 2, vec![0; 32]).is_some());
        assert!(Picture::new(4, 2, vec![0; 31]).is_none());
        assert!(Picture::new(0, 2, Vec::new()).is_none());
        assert!(Picture::new(u32::MAX, u32::MAX, vec![0; 4]).is_none());
    }

    #[test]
    fn an_area_with_a_negative_corner_converts_points_into_its_picture() {
        let area = Area { x: -1920, y: 0, width: 1920, height: 1080 };
        assert!(area.contains(-1920, 0));
        assert!(area.contains(-1, 1079));
        assert!(!area.contains(0, 0), "the display to the right starts at 0");
        assert!(!area.contains(-1921, 5));
        assert!(!area.contains(-5, 1080));
        assert_eq!(area.to_picture(-1900, 40), (20, 40));
        assert!(
            !Area { x: 0, y: 0, width: 10, height: 10 }.contains(i32::MAX, i32::MAX),
            "no overflow at the far edge"
        );
    }

    // ---- cropping

    fn numbered(width: u32, height: u32) -> Picture {
        let mut bgra = Vec::new();
        for y in 0..height {
            for x in 0..width {
                bgra.extend_from_slice(&[x as u8, y as u8, 0, 255]);
            }
        }
        Picture::new(width, height, bgra).unwrap()
    }

    #[test]
    fn a_crop_is_the_part_inside_the_rectangle_with_its_own_corner_at_zero() {
        let p = numbered(10, 8);
        let c = p.crop(2, 3, 4, 2).unwrap();
        assert_eq!((c.width, c.height), (4, 2));
        assert_eq!(&c.bgra[0..2], &[2, 3], "its first pixel is the picture's pixel at 2, 3");
        assert_eq!(
            &c.bgra[(4 + 3) * 4..(4 + 3) * 4 + 2],
            &[5, 4],
            "and the pixel at 3, 1 of the crop is 5, 4 of the picture"
        );
    }

    #[test]
    fn a_crop_over_the_edge_keeps_only_what_is_there_and_one_outside_is_nothing() {
        let p = numbered(10, 8);
        let c = p.crop(-3, -2, 6, 5).unwrap();
        assert_eq!((c.width, c.height), (3, 3));
        assert_eq!(&c.bgra[0..2], &[0, 0]);
        let c = p.crop(8, 6, 50, 50).unwrap();
        assert_eq!((c.width, c.height), (2, 2));
        assert!(p.crop(10, 0, 5, 5).is_none());
        assert!(p.crop(-5, 0, 5, 5).is_none());
        assert!(p.crop(0, 0, 0, 5).is_none());
        assert!(p.crop(i32::MAX, i32::MAX, u32::MAX, u32::MAX).is_none(), "no overflow");
    }

    #[test]
    fn areas_overlap_by_what_they_share() {
        let a = Area { x: 0, y: 0, width: 100, height: 100 };
        let b = Area { x: 60, y: -20, width: 100, height: 50 };
        assert_eq!(a.intersect(&b), Some(Area { x: 60, y: 0, width: 40, height: 30 }));
        assert_eq!(b.intersect(&a), a.intersect(&b));
        assert_eq!(a.intersect(&Area { x: 100, y: 0, width: 5, height: 5 }), None, "touching is not overlapping");
        assert_eq!(a.intersect(&a), Some(a));
    }

    // ---- the frame ring

    #[test]
    fn the_ring_gives_the_newest_picture_at_or_before_the_time() {
        let mut ring = FrameRing::new(RING_BYTES);
        ring.push(100, picture(1));
        ring.push(200, picture(2));
        ring.push(300, picture(3));
        assert_eq!(shade(&ring.before(250).unwrap()), 2);
        assert_eq!(shade(&ring.before(200).unwrap()), 2, "a picture taken at that very moment counts");
        assert_eq!(shade(&ring.before(999).unwrap()), 3);
        assert_eq!(shade(&ring.latest().unwrap()), 3);
    }

    #[test]
    fn when_every_held_picture_is_later_the_oldest_is_used() {
        let mut ring = FrameRing::new(RING_BYTES);
        ring.push(500, picture(5));
        ring.push(600, picture(6));
        assert_eq!(shade(&ring.before(100).unwrap()), 5);
        assert!(FrameRing::new(RING_BYTES).before(100).is_none());
    }

    #[test]
    fn the_ring_stays_inside_its_memory_but_always_keeps_the_newest_two() {
        let one = picture(0).bytes();
        let mut ring = FrameRing::new(one * 3);
        for n in 0..10u8 {
            ring.push(n as Ms * 10, picture(n));
            assert!(ring.bytes() <= one * 3, "at most three pictures at once");
        }
        assert_eq!(ring.len(), 3);
        assert_eq!(shade(&ring.latest().unwrap()), 9);
        let mut tiny = FrameRing::new(1);
        for n in 0..5u8 {
            tiny.push(n as Ms, picture(n));
        }
        assert_eq!(tiny.len(), 2, "a ring too small for one picture still keeps two");
        assert_eq!(shade(&tiny.before(0).unwrap()), 3);
    }

    #[test]
    fn a_picture_a_step_holds_outlives_its_place_in_the_ring() {
        let mut ring = FrameRing::new(1);
        ring.push(1, picture(7));
        let held = ring.before(1).unwrap();
        for n in 0..5u8 {
            ring.push(2 + n as Ms, picture(n));
        }
        assert_eq!(shade(&held), 7);
    }

    // ---- steps from clicks

    #[test]
    fn a_click_gets_the_picture_from_just_before_it() {
        let mut e = engine();
        let mut field = no_field();
        assert!(e.push_frame(100, picture(1)).is_empty());
        assert!(e.push_frame(180, picture(2)).is_empty(), "the menu was open");
        assert!(e.push_frame(260, picture(3)).is_empty(), "the click closed it");
        assert!(e.input(click(250, 2, 1), &mut field).is_empty(), "a plain click waits out the double-click time");
        let done = e.settle(900, true, &mut field);
        assert_eq!(done.len(), 1);
        assert_eq!(shade(&done[0].picture), 2, "the picture from before the click, not the one after");
        assert_eq!(done[0].draft.title, "Click \"Save\"");
    }

    #[test]
    fn steps_come_out_in_the_order_the_actions_happened() {
        let mut e = engine();
        let mut field = no_field();
        e.push_frame(10, picture(1));
        e.input(click(100, 1, 1), &mut field);
        let first = e.tick(700, &mut field);
        assert_eq!(first.len(), 1);
        e.push_frame(800, picture(2));
        e.input(click(900, 3, 1), &mut field);
        let second = e.settle(2000, true, &mut field);
        assert_eq!(second.len(), 1);
        assert!(first[0].draft.at < second[0].draft.at);
        assert_eq!(shade(&first[0].picture), 1);
        assert_eq!(shade(&second[0].picture), 2);
    }

    #[test]
    fn a_screenshot_delay_waits_for_a_later_picture() {
        let settings = Settings { screenshot_delay_ms: 500, ..Settings::default() };
        let mut e = CaptureEngine::new(settings, RING_BYTES);
        let mut field = no_field();
        e.push_frame(100, picture(1));
        e.input(click(200, 1, 1), &mut field);
        assert!(e.tick(800, &mut field).is_empty(), "the picture is to be taken at 700; none has come yet");
        assert!(e.push_frame(650, picture(2)).is_empty(), "too early: the animation is still running");
        let done = e.push_frame(720, picture(3));
        assert_eq!(done.len(), 1);
        assert_eq!(shade(&done[0].picture), 3, "the state after the delay");
    }

    #[test]
    fn a_still_screen_does_not_hold_a_delayed_step_up_forever() {
        let settings = Settings { screenshot_delay_ms: 500, ..Settings::default() };
        let mut e = CaptureEngine::new(settings, RING_BYTES);
        let mut field = no_field();
        e.push_frame(100, picture(1));
        e.input(click(200, 1, 1), &mut field);
        assert!(e.tick(700 + DELAY_GRACE_MS - 1, &mut field).is_empty());
        let done = e.tick(700 + DELAY_GRACE_MS, &mut field);
        assert_eq!(done.len(), 1, "no new picture came because nothing changed, so the newest is right");
        assert_eq!(shade(&done[0].picture), 1);
    }

    #[test]
    fn a_waiting_step_holds_back_the_later_ones_so_the_order_stays() {
        let settings = Settings { screenshot_delay_ms: 500, ..Settings::default() };
        let mut e = CaptureEngine::new(settings, RING_BYTES);
        let mut field = no_field();
        e.push_frame(100, picture(1));
        e.input(click(200, 1, 1), &mut field);
        e.input(Input::CaptureNow { at: 300, cursor: None }, &mut field);
        let done = e.settle(400, true, &mut field);
        assert_eq!(done.len(), 2, "finishing settles both");
        assert!(done[0].draft.at < done[1].draft.at);
        assert_eq!(done[0].draft.title, "Click \"Save\"");
        assert_eq!(done[1].draft.title, "Screenshot");
    }

    #[test]
    fn a_click_before_any_picture_arrives_takes_the_first_one_that_does() {
        let mut e = engine();
        let mut field = no_field();
        e.input(click(10, 1, 1), &mut field);
        assert!(e.tick(700, &mut field).is_empty(), "no picture yet");
        let done = e.push_frame(710, picture(9));
        assert_eq!(done.len(), 1);
        assert_eq!(shade(&done[0].picture), 9);
    }

    #[test]
    fn a_step_that_never_had_a_picture_is_counted_not_hidden() {
        let mut e = engine();
        let mut field = no_field();
        e.input(click(10, 1, 1), &mut field);
        let done = e.settle(900, true, &mut field);
        assert!(done.is_empty());
        assert_eq!(e.lost(), 1);
        assert!(!e.is_waiting());
    }

    #[test]
    fn the_capture_now_hotkey_makes_a_screenshot_step_with_the_current_picture() {
        let mut e = engine();
        let mut field = no_field();
        e.push_frame(50, picture(4));
        let done = e.input(Input::CaptureNow { at: 60, cursor: Some((1, 1)) }, &mut field);
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].draft.title, "Screenshot");
        assert!(done[0].draft.click.is_none(), "the cursor is only recorded if the setting says so");
    }

    #[test]
    fn typing_makes_one_step_with_a_new_picture_and_the_text_read_from_the_field() {
        let mut e = engine();
        let mut field = Field(FocusedField {
            name: "Email".into(),
            role: "edit".into(),
            text: FieldText::Text("ann@example.org".into()),
        });
        e.push_frame(100, picture(1));
        let field_click = Input::Click {
            at: 200,
            button: MouseButton::Left,
            x: 2,
            y: 1,
            target: Some(Target::editable("Email", "edit")),
            mods: Mods::none(),
            inside_area: true,
            own_window: false,
        };
        assert!(e.input(field_click, &mut field).is_empty());
        e.input(Input::Typing { at: 300 }, &mut field);
        e.push_frame(900, picture(2));
        e.input(Input::Typing { at: 950 }, &mut field);
        let early = e.input(Input::Key { at: 1000, key: Key::Enter }, &mut field);
        assert!(
            early.is_empty(),
            "the step waits for a picture taken after the screen has caught up with the last key"
        );
        let done = e.push_frame(950 + crate::builder::TYPE_SETTLE_MS, picture(3));
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].draft.typed.as_deref(), Some("ann@example.org"));
        assert_eq!(
            shade(&done[0].picture),
            3,
            "the picture shows the filled field, not the one from before the last letters"
        );
        assert!(e.shots.is_empty(), "the picture the field click took was let go, not kept");
    }

    #[test]
    fn a_password_field_never_makes_a_step_with_text() {
        let mut e = engine();
        let mut field = Field(FocusedField { name: "Password".into(), role: "edit".into(), text: FieldText::Password });
        e.push_frame(100, picture(1));
        let field_click = Input::Click {
            at: 200,
            button: MouseButton::Left,
            x: 2,
            y: 1,
            target: Some(Target::editable("Password", "edit")),
            mods: Mods::none(),
            inside_area: true,
            own_window: false,
        };
        e.input(field_click, &mut field);
        e.input(Input::Typing { at: 300 }, &mut field);
        let done = e.input(Input::Key { at: 1000, key: Key::Tab }, &mut field);
        for step in done.iter().chain(e.settle(5000, true, &mut field).iter()) {
            assert!(step.draft.typed.is_none());
            assert!(!step.draft.title.contains("hunter2"));
        }
    }

    #[test]
    fn the_capture_mode_still_applies() {
        let settings =
            Settings { mode: crate::builder::CaptureMode::OnClickWithKey(ModKey::Ctrl), ..Settings::default() };
        let mut e = CaptureEngine::new(settings, RING_BYTES);
        let mut field = no_field();
        e.push_frame(10, picture(1));
        e.input(click(100, 1, 1), &mut field);
        assert!(e.settle(2000, true, &mut field).is_empty(), "no Ctrl held, so no step");
        assert_eq!(e.lost(), 0, "and nothing was lost either");
    }

    #[test]
    fn a_field_click_with_nothing_typed_makes_a_click_step_and_keeps_no_picture_behind() {
        let mut e = engine();
        let mut field = no_field();
        e.push_frame(10, picture(1));
        let field_click = Input::Click {
            at: 100,
            button: MouseButton::Left,
            x: 2,
            y: 1,
            target: Some(Target::editable("Email", "edit")),
            mods: Mods::none(),
            inside_area: true,
            own_window: false,
        };
        e.input(field_click, &mut field);
        // A second click ends the wait: the field click becomes an ordinary Click step with its own picture.
        let done = e.input(click(400, 300, 200), &mut field);
        let rest = e.settle(2000, true, &mut field);
        assert_eq!(done.len() + rest.len(), 2);
        assert!(e.shots.is_empty(), "every picture was used");
    }

    // ---- marks and the stored step

    fn captured(title: &str, x: i32, y: i32, role: &str) -> Captured {
        Captured {
            draft: Draft {
                kind: crate::builder::DraftKind::Click,
                title: title.to_string(),
                shot: ShotId(1),
                click: Some(Click { x, y, button: crate::model::Button::Left, extra: Default::default() }),
                control: Some(crate::model::Control {
                    name: "Save".into(),
                    role: role.into(),
                    extra: Default::default(),
                }),
                typed: None,
                at: 5,
            },
            picture: Picture::new(1920, 1080, vec![0; 1920 * 1080 * 4]).unwrap(),
        }
    }

    #[test]
    fn a_step_gets_a_marker_a_cursor_and_the_focused_view_by_default() {
        let c = captured("Click \"Save\"", 960, 540, "button");
        let step =
            step_draft(&c, StepImage::new("images/x.png", 1920, 1080), &MarkOptions::default(), "2026-11-01T10:00:00Z");
        let kinds: Vec<_> = step.annotations.iter().map(|a| a.kind).collect();
        assert_eq!(kinds, vec![AnnotationKind::ClickMark, AnnotationKind::Cursor]);
        let ids: Vec<_> = step.annotations.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["a1", "a2"]);
        assert_eq!(step.annotations[0].size, Some(56));
        assert_eq!(step.annotations[1].size, Some(32));
        assert_eq!(step.annotations[1].style.as_deref(), Some("pointer"));
        let focus = step.focus.expect("the focused view is on");
        assert_eq!((focus.width, focus.height), (1280, 720));
        assert_eq!(step.captured_at.as_deref(), Some("2026-11-01T10:00:00Z"));
        assert_eq!(step.kind, crate::model::StepKind::Image);
    }

    #[test]
    fn the_marks_scale_with_the_display_and_can_be_switched_off() {
        let c = captured("Click \"Save\"", 100, 100, "edit");
        let big = MarkOptions { dpi_percent: 200, ..MarkOptions::default() };
        let step = step_draft(&c, StepImage::new("images/x.png", 1920, 1080), &big, "t");
        assert_eq!(step.annotations[0].size, Some(112));
        assert_eq!(step.annotations[1].size, Some(64));
        assert_eq!(step.annotations[1].style.as_deref(), Some("text"), "an edit field gets the I-beam");
        let none = MarkOptions { click_mark: false, cursor: false, focused_view: false, ..MarkOptions::default() };
        let plain = step_draft(&c, StepImage::new("images/x.png", 1920, 1080), &none, "t");
        assert!(plain.annotations.is_empty());
        assert!(plain.focus.is_none());
    }

    #[test]
    fn a_step_without_a_click_has_no_marks_and_no_focused_view() {
        let mut c = captured("Screenshot", 0, 0, "button");
        c.draft.click = None;
        c.draft.control = None;
        let step = step_draft(&c, StepImage::new("images/x.png", 1920, 1080), &MarkOptions::default(), "t");
        assert!(step.annotations.is_empty());
        assert!(step.focus.is_none());
    }

    #[test]
    fn the_stored_step_goes_into_a_guide() {
        use crate::model::Guide;
        use crate::ops::{Document, Position};
        let c = captured("Click \"Save\"", 960, 540, "button");
        let step = step_draft(&c, StepImage::new("images/step-0001.png", 1920, 1080), &MarkOptions::default(), "t");
        let mut doc = Document::new(Guide::new("g", "A guide", "en", "t"));
        let id = doc.add_step(step, Position::End, "t").expect("the step is added");
        assert_eq!(doc.guide.step(&id).unwrap().annotations.len(), 2);
        assert!(doc.guide.problems().is_empty(), "{:?}", doc.guide.problems());
    }
}
