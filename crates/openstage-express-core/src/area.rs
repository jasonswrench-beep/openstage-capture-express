// SPDX-License-Identifier: Apache-2.0
//! Choosing a rectangle of a display (spec C3, "a drawn area"): the geometry only. The window shows a picture of the display and
//! lets the person draw, move and resize the rectangle on it with the mouse, or move it with buttons or type its four numbers; all of
//! those end up here, in whole pixels measured from the display's top left corner, always inside the display.
//!
//! The shape work is shared with the marks of a step (`marks.rs`): the handles, the hit test and the resizing are the same.

use crate::marks::{self, Handle};
use crate::model::{Annotation, AnnotationKind};

/// The smallest an area can be, in either direction. A guide picture smaller than this could not show a click in any useful way.
pub const MIN_AREA_SIDE: u32 = 32;

/// A rectangle of a display, in pixels from the display's top left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// What a press on the picture started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// A handle of the rectangle: dragging changes its size.
    Handle(Handle),
    /// The inside of the rectangle: dragging moves it.
    Inside,
    /// Empty picture: dragging draws a new rectangle from the point.
    New,
}

#[derive(Debug, Clone, Copy)]
struct Drag {
    pick: Pick,
    from: (i32, i32),
    original: AreaRect,
}

/// The rectangle being chosen on a display of a given size.
#[derive(Debug, Clone)]
pub struct AreaEditor {
    display: (u32, u32),
    rect: AreaRect,
    drag: Option<Drag>,
}

fn as_mark(r: &AreaRect) -> Annotation {
    let mut m = Annotation::at("area", AnnotationKind::Rect, r.x as i32, r.y as i32);
    m.width = Some(r.width as i32);
    m.height = Some(r.height as i32);
    m
}

fn from_mark(m: &Annotation) -> AreaRect {
    AreaRect {
        x: m.x.max(0) as u32,
        y: m.y.max(0) as u32,
        width: m.width.unwrap_or(0).max(0) as u32,
        height: m.height.unwrap_or(0).max(0) as u32,
    }
}

impl AreaEditor {
    /// An editor over a display of `width` by `height`, starting at `initial` (made to fit) or at the whole display.
    pub fn new(width: u32, height: u32, initial: Option<AreaRect>) -> AreaEditor {
        let mut editor = AreaEditor {
            display: (width.max(1), height.max(1)),
            rect: AreaRect { x: 0, y: 0, width, height },
            drag: None,
        };
        if let Some(r) = initial {
            editor.rect = editor.fitted(r);
        }
        editor
    }

    /// Where the eight square handles are, for drawing them.
    pub fn handle_points(&self) -> Vec<(i32, i32)> {
        marks::handles(&as_mark(&self.rect), self.display.1).into_iter().map(|(_, at)| at).collect()
    }

    pub fn display_size(&self) -> (u32, u32) {
        self.display
    }

    pub fn rect(&self) -> AreaRect {
        self.rect
    }

    pub fn is_whole_display(&self) -> bool {
        self.rect == AreaRect { x: 0, y: 0, width: self.display.0, height: self.display.1 }
    }

    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// A rectangle made to fit the display: at least [`MIN_AREA_SIDE`] each way (or the whole display if that is smaller), moved
    /// back inside it if it hangs over an edge.
    fn fitted(&self, r: AreaRect) -> AreaRect {
        let (dw, dh) = self.display;
        let width = r.width.clamp(MIN_AREA_SIDE.min(dw), dw);
        let height = r.height.clamp(MIN_AREA_SIDE.min(dh), dh);
        AreaRect { x: r.x.min(dw - width), y: r.y.min(dh - height), width, height }
    }

    pub fn use_whole_display(&mut self) {
        self.drag = None;
        self.rect = AreaRect { x: 0, y: 0, width: self.display.0, height: self.display.1 };
    }

    /// Set the four numbers. Says in words what was wrong if they cannot be used; the rectangle is then left as it was.
    pub fn set_numbers(&mut self, x: u32, y: u32, width: u32, height: u32) -> Result<(), String> {
        let (dw, dh) = self.display;
        if width < MIN_AREA_SIDE.min(dw) || height < MIN_AREA_SIDE.min(dh) {
            return Err(format!(
                "The area must be at least {} by {} pixels.",
                MIN_AREA_SIDE.min(dw),
                MIN_AREA_SIDE.min(dh)
            ));
        }
        if u64::from(x) + u64::from(width) > u64::from(dw) || u64::from(y) + u64::from(height) > u64::from(dh) {
            return Err(format!(
                "The area would reach past the edge of the display, which is {dw} by {dh} pixels: it starts {x} from the left and {y} from the top and is {width} by {height}."
            ));
        }
        self.rect = AreaRect { x, y, width, height };
        Ok(())
    }

    /// Set how far the rectangle is from the left. If it would no longer fit, it is made narrower to make room.
    pub fn set_left(&mut self, x: u32) -> Result<(), String> {
        let (dw, _) = self.display;
        let min = MIN_AREA_SIDE.min(dw);
        if x > dw - min {
            return Err(format!(
                "The left edge can be at most {} pixels from the left, to leave {min} pixels for the area.",
                dw - min
            ));
        }
        self.rect.x = x;
        self.rect.width = self.rect.width.min(dw - x);
        Ok(())
    }

    /// Set how far the rectangle is from the top. If it would no longer fit, it is made shorter to make room.
    pub fn set_top(&mut self, y: u32) -> Result<(), String> {
        let (_, dh) = self.display;
        let min = MIN_AREA_SIDE.min(dh);
        if y > dh - min {
            return Err(format!(
                "The top edge can be at most {} pixels from the top, to leave {min} pixels for the area.",
                dh - min
            ));
        }
        self.rect.y = y;
        self.rect.height = self.rect.height.min(dh - y);
        Ok(())
    }

    /// Set the width.
    pub fn set_width(&mut self, width: u32) -> Result<(), String> {
        let room = self.display.0 - self.rect.x;
        if width < MIN_AREA_SIDE.min(self.display.0) {
            return Err(format!("The width must be at least {} pixels.", MIN_AREA_SIDE.min(self.display.0)));
        }
        if width > room {
            return Err(format!("The width can be at most {room} pixels, because the area starts {} pixels from the left and the display is {} pixels wide.", self.rect.x, self.display.0));
        }
        self.rect.width = width;
        Ok(())
    }

    /// Set the height.
    pub fn set_height(&mut self, height: u32) -> Result<(), String> {
        let room = self.display.1 - self.rect.y;
        if height < MIN_AREA_SIDE.min(self.display.1) {
            return Err(format!("The height must be at least {} pixels.", MIN_AREA_SIDE.min(self.display.1)));
        }
        if height > room {
            return Err(format!("The height can be at most {room} pixels, because the area starts {} pixels from the top and the display is {} pixels tall.", self.rect.y, self.display.1));
        }
        self.rect.height = height;
        Ok(())
    }

    /// Move the whole rectangle by `dx`, `dy` pixels, stopping at the edges.
    pub fn nudge(&mut self, dx: i32, dy: i32) {
        let mut m = as_mark(&self.rect);
        marks::nudge(&mut m, dx, dy, self.display.0, self.display.1);
        self.rect = from_mark(&m);
    }

    /// Make the rectangle wider or narrower (`dw`) and taller or shorter (`dh`) by moving its bottom right corner.
    pub fn grow(&mut self, dw: i32, dh: i32) {
        let mut m = as_mark(&self.rect);
        marks::resize(&mut m, Handle::BottomRight, dw, dh, self.display.0, self.display.1);
        self.rect = self.fitted(from_mark(&m));
    }

    /// A press at a point of the display's picture: what it picked, and the drag begins.
    pub fn press(&mut self, x: i32, y: i32) -> Pick {
        let m = as_mark(&self.rect);
        let pick = if let Some(h) = marks::handle_at(&m, x, y, self.display.1) {
            Pick::Handle(h)
        } else if marks::hit(&m, x, y, self.display.1) {
            Pick::Inside
        } else {
            Pick::New
        };
        self.drag = Some(Drag { pick, from: (x, y), original: self.rect });
        pick
    }

    /// The pointer is now at (`x`, `y`) with the button still down.
    pub fn drag_to(&mut self, x: i32, y: i32) {
        let Some(d) = self.drag else { return };
        let (dw, dh) = (self.display.0, self.display.1);
        match d.pick {
            Pick::Inside => {
                let mut m = as_mark(&d.original);
                marks::nudge(&mut m, x - d.from.0, y - d.from.1, dw, dh);
                self.rect = from_mark(&m);
            }
            Pick::Handle(handle) => {
                let mut m = as_mark(&d.original);
                marks::resize(&mut m, handle, x - d.from.0, y - d.from.1, dw, dh);
                self.rect = self.fitted(from_mark(&m));
            }
            Pick::New => {
                let (cx, cy) = (x.clamp(0, dw as i32), y.clamp(0, dh as i32));
                let (fx, fy) = (d.from.0.clamp(0, dw as i32), d.from.1.clamp(0, dh as i32));
                let (left, right) = (fx.min(cx), fx.max(cx));
                let (top, bottom) = (fy.min(cy), fy.max(cy));
                self.rect = AreaRect {
                    x: left as u32,
                    y: top as u32,
                    width: (right - left) as u32,
                    height: (bottom - top) as u32,
                };
            }
        }
    }

    /// The button was let go. A new rectangle that came out too small to use is given up and the one before comes back.
    pub fn release(&mut self) {
        let Some(d) = self.drag.take() else { return };
        let (dw, dh) = self.display;
        if d.pick == Pick::New && (self.rect.width < MIN_AREA_SIDE.min(dw) || self.rect.height < MIN_AREA_SIDE.min(dh))
        {
            self.rect = d.original;
        }
    }

    /// The rectangle in words: `800 by 600, 100 pixels from the left and 50 from the top`.
    pub fn words(&self) -> String {
        let r = self.rect;
        if self.is_whole_display() {
            return format!("the whole display, {} by {}", r.width, r.height);
        }
        format!("{} by {}, {} pixels from the left and {} from the top", r.width, r.height, r.x, r.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> AreaEditor {
        AreaEditor::new(1920, 1080, None)
    }

    #[test]
    fn it_starts_as_the_whole_display_or_where_it_is_told_made_to_fit() {
        assert!(editor().is_whole_display());
        let e = AreaEditor::new(1920, 1080, Some(AreaRect { x: 100, y: 50, width: 800, height: 600 }));
        assert_eq!(e.rect(), AreaRect { x: 100, y: 50, width: 800, height: 600 });
        let over = AreaEditor::new(1920, 1080, Some(AreaRect { x: 1800, y: 1000, width: 800, height: 600 }));
        assert_eq!(
            over.rect(),
            AreaRect { x: 1120, y: 480, width: 800, height: 600 },
            "moved back inside, not squashed"
        );
        let tiny = AreaEditor::new(1920, 1080, Some(AreaRect { x: 5, y: 5, width: 1, height: 1 }));
        assert_eq!((tiny.rect().width, tiny.rect().height), (MIN_AREA_SIDE, MIN_AREA_SIDE));
    }

    #[test]
    fn four_numbers_are_accepted_or_refused_in_words() {
        let mut e = editor();
        assert!(e.set_numbers(10, 20, 640, 480).is_ok());
        assert_eq!(e.rect(), AreaRect { x: 10, y: 20, width: 640, height: 480 });
        let past = e.set_numbers(1500, 0, 640, 480).unwrap_err();
        assert!(past.contains("past the edge") && past.contains("1920 by 1080"), "{past}");
        let small = e.set_numbers(0, 0, 5, 500).unwrap_err();
        assert!(small.contains("at least 32 by 32"), "{small}");
        assert_eq!(e.rect(), AreaRect { x: 10, y: 20, width: 640, height: 480 }, "a refusal changes nothing");
        assert!(e.set_numbers(u32::MAX, 0, u32::MAX, 100).is_err(), "no overflow");
    }

    #[test]
    fn each_number_can_be_set_alone_in_any_order_making_room_when_it_has_to() {
        let mut e = editor();
        e.set_left(50).unwrap();
        assert_eq!((e.rect().x, e.rect().width), (50, 1870), "the area was the whole display, so it is made narrower");
        e.set_top(40).unwrap();
        assert_eq!((e.rect().y, e.rect().height), (40, 1040));
        e.set_width(200).unwrap();
        e.set_height(150).unwrap();
        assert_eq!(e.rect(), AreaRect { x: 50, y: 40, width: 200, height: 150 });
        let wide = e.set_width(5000).unwrap_err();
        assert!(wide.contains("at most 1870 pixels") && wide.contains("starts 50 pixels from the left"), "{wide}");
        assert!(e.set_height(5).unwrap_err().contains("at least 32"));
        assert!(e.set_left(1919).unwrap_err().contains("at most 1888"));
        assert!(e.set_top(5000).is_err());
        assert_eq!(e.rect(), AreaRect { x: 50, y: 40, width: 200, height: 150 }, "a refusal changes nothing");
    }

    #[test]
    fn buttons_move_and_resize_and_stop_at_the_edges() {
        let mut e = editor();
        e.set_numbers(100, 100, 400, 300).unwrap();
        e.nudge(10, -20);
        assert_eq!((e.rect().x, e.rect().y), (110, 80));
        e.nudge(-9999, -9999);
        assert_eq!((e.rect().x, e.rect().y), (0, 0));
        e.nudge(99999, 99999);
        assert_eq!((e.rect().x + e.rect().width, e.rect().y + e.rect().height), (1920, 1080));
        e.set_numbers(100, 100, 400, 300).unwrap();
        e.grow(50, 20);
        assert_eq!((e.rect().width, e.rect().height), (450, 320));
        e.grow(-9999, -9999);
        assert_eq!((e.rect().width, e.rect().height), (MIN_AREA_SIDE, MIN_AREA_SIDE), "never smaller than the minimum");
    }

    #[test]
    fn dragging_inside_moves_a_handle_resizes_and_empty_picture_draws_a_new_one() {
        let mut e = editor();
        e.set_numbers(400, 300, 400, 300).unwrap();
        assert_eq!(e.press(600, 450), Pick::Inside);
        e.drag_to(650, 480);
        assert_eq!((e.rect().x, e.rect().y, e.rect().width, e.rect().height), (450, 330, 400, 300));
        e.release();
        assert_eq!(e.press(850, 630), Pick::Handle(Handle::BottomRight));
        e.drag_to(900, 650);
        assert_eq!((e.rect().width, e.rect().height), (450, 320));
        e.release();
        assert_eq!(e.press(10, 10), Pick::New);
        e.drag_to(210, 160);
        assert_eq!(e.rect(), AreaRect { x: 10, y: 10, width: 200, height: 150 });
        e.drag_to(5, 5);
        assert_eq!(
            e.rect(),
            AreaRect { x: 5, y: 5, width: 5, height: 5 },
            "dragging back over the start draws the other way"
        );
        e.release();
        assert_eq!(
            e.rect(),
            AreaRect { x: 450, y: 330, width: 450, height: 320 },
            "too small: the one before comes back"
        );
    }

    #[test]
    fn a_new_rectangle_is_kept_inside_the_display_whatever_the_pointer_does() {
        let mut e = editor();
        e.press(100, 100);
        e.drag_to(99999, 99999);
        assert_eq!((e.rect().x + e.rect().width, e.rect().y + e.rect().height), (1920, 1080));
        e.drag_to(-500, -500);
        assert_eq!((e.rect().x, e.rect().y), (0, 0));
        e.release();
    }

    #[test]
    fn the_whole_display_button_and_the_words() {
        let mut e = editor();
        e.set_numbers(100, 50, 800, 600).unwrap();
        assert_eq!(e.words(), "800 by 600, 100 pixels from the left and 50 from the top");
        e.use_whole_display();
        assert!(e.is_whole_display());
        assert_eq!(e.words(), "the whole display, 1920 by 1080");
    }

    #[test]
    fn a_display_smaller_than_the_minimum_still_works() {
        let mut e = AreaEditor::new(20, 20, None);
        assert!(e.is_whole_display());
        assert!(e.set_numbers(0, 0, 20, 20).is_ok());
        e.nudge(5, 5);
        assert_eq!(e.rect(), AreaRect { x: 0, y: 0, width: 20, height: 20 });
    }
}
