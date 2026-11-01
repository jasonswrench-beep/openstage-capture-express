// SPDX-License-Identifier: Apache-2.0
//! The marks of a step as shapes a person works with in the editor (spec E4): what each tool starts with, moving and resizing a mark,
//! finding the mark under a point, and saying a mark in words. No drawing happens here (that is `render.rs`), and nothing here knows
//! about windows or the mouse: the editor passes in picture pixels.
//!
//! Everything is done in whole picture pixels and never lets a mark leave the picture altogether.

use crate::model::{Annotation, AnnotationKind, Rect};

/// What the person can add.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Rectangle,
    Oval,
    Line,
    Arrow,
    Text,
    Number,
    Highlight,
    Blur,
    Spotlight,
}

impl Tool {
    /// All of them, in the order the editor lists them.
    pub const ALL: [Tool; 9] = [
        Tool::Rectangle,
        Tool::Oval,
        Tool::Line,
        Tool::Arrow,
        Tool::Text,
        Tool::Number,
        Tool::Highlight,
        Tool::Blur,
        Tool::Spotlight,
    ];

    /// The name on its button and in what is said when it is used: `rectangle`.
    pub fn name(self) -> &'static str {
        match self {
            Tool::Rectangle => "rectangle",
            Tool::Oval => "oval",
            Tool::Line => "line",
            Tool::Arrow => "arrow",
            Tool::Text => "text",
            Tool::Number => "step number",
            Tool::Highlight => "highlight",
            Tool::Blur => "blur",
            Tool::Spotlight => "spotlight",
        }
    }

    pub fn kind(self) -> AnnotationKind {
        match self {
            Tool::Rectangle => AnnotationKind::Rect,
            Tool::Oval => AnnotationKind::Oval,
            Tool::Line => AnnotationKind::Line,
            Tool::Arrow => AnnotationKind::Arrow,
            Tool::Text => AnnotationKind::Text,
            Tool::Number => AnnotationKind::Number,
            Tool::Highlight => AnnotationKind::Highlight,
            Tool::Blur => AnnotationKind::Blur,
            Tool::Spotlight => AnnotationKind::Spotlight,
        }
    }
}

/// A part of a mark that can be dragged to change its size or reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Left,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
    /// The start of a line or arrow.
    Start,
    /// The end of a line or arrow.
    End,
}

/// The colours on offer, by name, as `#RRGGBB`.
pub const COLOURS: [(&str, &str); 7] = [
    ("Indigo", "#7D87F2"),
    ("Red", "#E5484D"),
    ("Green", "#30A46C"),
    ("Orange", "#F76B15"),
    ("Yellow", "#FFE600"),
    ("Black", "#000000"),
    ("White", "#FFFFFF"),
];

/// The line widths on offer, in picture pixels.
pub const WIDTHS: [i32; 4] = [2, 4, 6, 8];

/// How close, in picture pixels, a point must be to a line to be on it, and how big a handle is to grab.
pub const GRAB: i32 = 10;

/// The smallest a box mark can be made, in either direction.
pub const MIN_SIDE: i32 = 6;

fn is_box(kind: AnnotationKind) -> bool {
    matches!(
        kind,
        AnnotationKind::Rect
            | AnnotationKind::Oval
            | AnnotationKind::Highlight
            | AnnotationKind::Blur
            | AnnotationKind::Spotlight
    )
}

fn is_line(kind: AnnotationKind) -> bool {
    matches!(kind, AnnotationKind::Line | AnnotationKind::Arrow)
}

/// Whether a mark's size is one number (a round thing: a step number, a click marker or a pointer).
fn is_round(kind: AnnotationKind) -> bool {
    matches!(kind, AnnotationKind::Number | AnnotationKind::ClickMark | AnnotationKind::Cursor)
}

/// The mark a tool starts with, in the middle of `around` (a point of the picture, such as where the click was) or of the picture.
/// `taken` is how many step numbers are already on the step, so the new one is the next.
pub fn default_mark(tool: Tool, picture_w: u32, picture_h: u32, around: Option<(i32, i32)>, taken: u32) -> Annotation {
    let (w, h) = (picture_w as i32, picture_h as i32);
    let (cx, cy) = around.unwrap_or((w / 2, h / 2));
    let mut mark = Annotation::at("", tool.kind(), cx, cy);
    let box_w = (w / 6).max(40);
    let box_h = (h / 9).max(30);
    match tool {
        Tool::Rectangle | Tool::Oval | Tool::Highlight | Tool::Blur | Tool::Spotlight => {
            mark.x = cx - box_w / 2;
            mark.y = cy - box_h / 2;
            mark.width = Some(box_w);
            mark.height = Some(box_h);
        }
        Tool::Line | Tool::Arrow => {
            let half = (w / 10).max(30);
            mark.x = cx - half;
            mark.y = cy;
            mark.x2 = Some(cx + half);
            mark.y2 = Some(cy);
        }
        Tool::Text => {
            mark.text = Some("Text".to_string());
        }
        Tool::Number => {
            mark.number = Some(taken + 1);
            mark.size = Some((h / 22).max(28));
        }
    }
    keep_on_picture(&mut mark, picture_w, picture_h);
    mark
}

/// The smallest box that holds a mark, in picture pixels (a line's box holds both ends; text is guessed from its words).
pub fn bounds(mark: &Annotation, picture_h: u32) -> Rect {
    let kind = mark.kind;
    if is_line(kind) {
        let (x2, y2) = (mark.x2.unwrap_or(mark.x), mark.y2.unwrap_or(mark.y));
        let (left, right) = (mark.x.min(x2), mark.x.max(x2));
        let (top, bottom) = (mark.y.min(y2), mark.y.max(y2));
        return Rect::new(left, top, (right - left).max(1), (bottom - top).max(1));
    }
    if is_round(kind) {
        let size = mark.size.unwrap_or(32).max(1);
        return Rect::new(mark.x - size / 2, mark.y - size / 2, size, size);
    }
    if kind == AnnotationKind::Text {
        let font = mark.font_size.filter(|f| *f > 0).unwrap_or((picture_h as i32 / 40).max(16));
        let words = mark.text.as_deref().unwrap_or("").chars().count().max(1) as i32;
        let width = mark.width.filter(|w| *w > 0).unwrap_or(words * font * 6 / 10 + font);
        return Rect::new(mark.x, mark.y, width, font * 3 / 2);
    }
    Rect::new(mark.x, mark.y, mark.width.unwrap_or(1).max(1), mark.height.unwrap_or(1).max(1))
}

fn distance_to_segment(px: i32, py: i32, (x1, y1): (i32, i32), (x2, y2): (i32, i32)) -> f64 {
    let (px, py, x1, y1, x2, y2) = (px as f64, py as f64, x1 as f64, y1 as f64, x2 as f64, y2 as f64);
    let (dx, dy) = (x2 - x1, y2 - y1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((px - x1) * dx + (py - y1) * dy) / len2).clamp(0.0, 1.0) };
    let (cx, cy) = (x1 + t * dx, y1 + t * dy);
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

/// Whether a point of the picture is on a mark. A line is on when the point is within a few pixels of it; a box when the point is
/// inside it; a round or text mark when the point is inside its box.
pub fn hit(mark: &Annotation, x: i32, y: i32, picture_h: u32) -> bool {
    if is_line(mark.kind) {
        let end = (mark.x2.unwrap_or(mark.x), mark.y2.unwrap_or(mark.y));
        let reach = (mark.stroke_width.unwrap_or(4) / 2).max(GRAB / 2) as f64 + 2.0;
        return distance_to_segment(x, y, (mark.x, mark.y), end) <= reach;
    }
    let b = bounds(mark, picture_h);
    x >= b.x && y >= b.y && x < b.x + b.width && y < b.y + b.height
}

/// The mark under a point: the last one in the list (the one drawn on top). `None` if there is none.
pub fn top_mark_at(marks: &[Annotation], x: i32, y: i32, picture_h: u32) -> Option<usize> {
    marks.iter().rposition(|m| hit(m, x, y, picture_h))
}

/// Keep a mark on the picture: a box is moved (not squashed) back inside it, a line is moved as a whole, a round or text mark has its
/// middle kept inside. A mark bigger than the picture is made to fit.
pub fn keep_on_picture(mark: &mut Annotation, picture_w: u32, picture_h: u32) {
    let (w, h) = (picture_w as i32, picture_h as i32);
    if is_box(mark.kind) {
        let bw = mark.width.unwrap_or(MIN_SIDE).clamp(MIN_SIDE.min(w), w.max(1));
        let bh = mark.height.unwrap_or(MIN_SIDE).clamp(MIN_SIDE.min(h), h.max(1));
        mark.width = Some(bw);
        mark.height = Some(bh);
        mark.x = mark.x.clamp(0, (w - bw).max(0));
        mark.y = mark.y.clamp(0, (h - bh).max(0));
    } else if is_line(mark.kind) {
        let (x2, y2) = (mark.x2.unwrap_or(mark.x), mark.y2.unwrap_or(mark.y));
        let dx = (-mark.x.min(x2)).max(0) - (mark.x.max(x2) - (w - 1)).max(0);
        let dy = (-mark.y.min(y2)).max(0) - (mark.y.max(y2) - (h - 1)).max(0);
        mark.x += dx;
        mark.y += dy;
        mark.x2 = Some(x2 + dx);
        mark.y2 = Some(y2 + dy);
    } else {
        mark.x = mark.x.clamp(0, (w - 1).max(0));
        mark.y = mark.y.clamp(0, (h - 1).max(0));
    }
}

/// Move a mark by `dx`, `dy` picture pixels, keeping it on the picture.
pub fn nudge(mark: &mut Annotation, dx: i32, dy: i32, picture_w: u32, picture_h: u32) {
    mark.x = mark.x.saturating_add(dx);
    mark.y = mark.y.saturating_add(dy);
    if is_line(mark.kind) {
        mark.x2 = Some(mark.x2.unwrap_or(mark.x).saturating_add(dx));
        mark.y2 = Some(mark.y2.unwrap_or(mark.y).saturating_add(dy));
    }
    keep_on_picture(mark, picture_w, picture_h);
}

/// Whether a mark has a size or reach that can be changed, and which handles it has.
pub fn handles(mark: &Annotation, picture_h: u32) -> Vec<(Handle, (i32, i32))> {
    if is_line(mark.kind) {
        return vec![
            (Handle::Start, (mark.x, mark.y)),
            (Handle::End, (mark.x2.unwrap_or(mark.x), mark.y2.unwrap_or(mark.y))),
        ];
    }
    let b = bounds(mark, picture_h);
    if is_round(mark.kind) {
        return vec![(Handle::BottomRight, (b.x + b.width, b.y + b.height))];
    }
    if mark.kind == AnnotationKind::Text {
        return vec![(Handle::Right, (b.x + b.width, b.y + b.height / 2))];
    }
    let (l, t, r, bt) = (b.x, b.y, b.x + b.width, b.y + b.height);
    let (mx, my) = ((l + r) / 2, (t + bt) / 2);
    vec![
        (Handle::TopLeft, (l, t)),
        (Handle::Top, (mx, t)),
        (Handle::TopRight, (r, t)),
        (Handle::Left, (l, my)),
        (Handle::Right, (r, my)),
        (Handle::BottomLeft, (l, bt)),
        (Handle::Bottom, (mx, bt)),
        (Handle::BottomRight, (r, bt)),
    ]
}

/// The handle of a mark near a point, if any.
pub fn handle_at(mark: &Annotation, x: i32, y: i32, picture_h: u32) -> Option<Handle> {
    handles(mark, picture_h)
        .into_iter()
        .find(|(_, (hx, hy))| (hx - x).abs() <= GRAB && (hy - y).abs() <= GRAB)
        .map(|(handle, _)| handle)
}

/// Drag a handle by `dx`, `dy` picture pixels. A box keeps at least [`MIN_SIDE`] in each direction and stays on the picture; a line's
/// end moves freely on the picture; a round mark grows or shrinks by the larger of the two movements; a text mark's width changes.
pub fn resize(mark: &mut Annotation, handle: Handle, dx: i32, dy: i32, picture_w: u32, picture_h: u32) {
    let (w, h) = (picture_w as i32, picture_h as i32);
    if is_line(mark.kind) {
        match handle {
            Handle::Start => {
                mark.x = (mark.x + dx).clamp(0, w - 1);
                mark.y = (mark.y + dy).clamp(0, h - 1);
            }
            _ => {
                mark.x2 = Some((mark.x2.unwrap_or(mark.x) + dx).clamp(0, w - 1));
                mark.y2 = Some((mark.y2.unwrap_or(mark.y) + dy).clamp(0, h - 1));
            }
        }
        return;
    }
    if is_round(mark.kind) {
        let by = if dx.abs() >= dy.abs() { dx } else { dy };
        mark.size = Some((mark.size.unwrap_or(32) + 2 * by).clamp(MIN_SIDE, w.min(h).max(MIN_SIDE)));
        return;
    }
    if mark.kind == AnnotationKind::Text {
        let current = bounds(mark, picture_h).width;
        mark.width = Some((current + dx).clamp(MIN_SIDE * 4, (w - mark.x).max(MIN_SIDE * 4)));
        return;
    }
    let b = bounds(mark, picture_h);
    let (mut l, mut t, mut r, mut bt) = (b.x, b.y, b.x + b.width, b.y + b.height);
    let moves_left = matches!(handle, Handle::TopLeft | Handle::Left | Handle::BottomLeft);
    let moves_right = matches!(handle, Handle::TopRight | Handle::Right | Handle::BottomRight);
    let moves_top = matches!(handle, Handle::TopLeft | Handle::Top | Handle::TopRight);
    let moves_bottom = matches!(handle, Handle::BottomLeft | Handle::Bottom | Handle::BottomRight);
    if moves_left {
        l = (l + dx).clamp(0, r - MIN_SIDE);
    }
    if moves_right {
        r = (r + dx).clamp(l + MIN_SIDE, w);
    }
    if moves_top {
        t = (t + dy).clamp(0, bt - MIN_SIDE);
    }
    if moves_bottom {
        bt = (bt + dy).clamp(t + MIN_SIDE, h);
    }
    mark.x = l;
    mark.y = t;
    mark.width = Some(r - l);
    mark.height = Some(bt - t);
}

/// Which field of a mark the colour choice sets, if the kind has a colour to choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColourField {
    /// The outline or the line (`rect`, `oval`, `line`, `arrow`).
    Stroke,
    /// The fill (`highlight`, `text` box, `number` circle).
    Fill,
}

pub fn colour_field(kind: AnnotationKind) -> Option<ColourField> {
    match kind {
        AnnotationKind::Rect | AnnotationKind::Oval | AnnotationKind::Line | AnnotationKind::Arrow => {
            Some(ColourField::Stroke)
        }
        AnnotationKind::Highlight | AnnotationKind::Text | AnnotationKind::Number => Some(ColourField::Fill),
        _ => None,
    }
}

/// Give a mark a colour (`#RRGGBB`). A highlight keeps a see-through fill so the words under it can still be read.
pub fn set_colour(mark: &mut Annotation, hex: &str) {
    match colour_field(mark.kind) {
        Some(ColourField::Stroke) => mark.stroke = Some(hex.to_string()),
        Some(ColourField::Fill) if mark.kind == AnnotationKind::Highlight => mark.fill = Some(format!("{hex}66")),
        Some(ColourField::Fill) => mark.fill = Some(hex.to_string()),
        None => {}
    }
}

/// The colour a mark has, as `#RRGGBB` (without any see-through part), if it has one set.
pub fn colour_of(mark: &Annotation) -> Option<String> {
    let text = match colour_field(mark.kind)? {
        ColourField::Stroke => mark.stroke.as_deref(),
        ColourField::Fill => mark.fill.as_deref(),
    }?;
    text.get(..7).map(str::to_uppercase)
}

/// The name of one of the offered colours that a mark has.
pub fn colour_name(mark: &Annotation) -> Option<&'static str> {
    let have = colour_of(mark)?;
    COLOURS.iter().find(|(_, hex)| hex.eq_ignore_ascii_case(&have)).map(|(name, _)| *name)
}

/// Whether the line width applies to a mark.
pub fn has_width(kind: AnnotationKind) -> bool {
    matches!(kind, AnnotationKind::Rect | AnnotationKind::Oval | AnnotationKind::Line | AnnotationKind::Arrow)
}

/// Whether a mark's box can be made wider, narrower, taller or shorter by buttons.
pub fn can_resize(kind: AnnotationKind) -> bool {
    is_box(kind) || is_line(kind) || is_round(kind) || kind == AnnotationKind::Text
}

/// What a mark is, in words a person reads and a screen reader says: `Rectangle, 200 by 100, at 120, 80`.
pub fn describe(mark: &Annotation, picture_h: u32) -> String {
    let b = bounds(mark, picture_h);
    match mark.kind {
        AnnotationKind::Rect => format!("Rectangle, {} by {}, at {}, {}", b.width, b.height, b.x, b.y),
        AnnotationKind::Oval => format!("Oval, {} by {}, at {}, {}", b.width, b.height, b.x, b.y),
        AnnotationKind::Highlight => format!("Highlight, {} by {}, at {}, {}", b.width, b.height, b.x, b.y),
        AnnotationKind::Blur => format!("Blur, {} by {}, at {}, {}", b.width, b.height, b.x, b.y),
        AnnotationKind::Spotlight => format!("Spotlight, {} by {}, at {}, {}", b.width, b.height, b.x, b.y),
        AnnotationKind::Line => {
            format!("Line, from {}, {} to {}, {}", mark.x, mark.y, mark.x2.unwrap_or(mark.x), mark.y2.unwrap_or(mark.y))
        }
        AnnotationKind::Arrow => {
            format!(
                "Arrow, from {}, {} to {}, {}",
                mark.x,
                mark.y,
                mark.x2.unwrap_or(mark.x),
                mark.y2.unwrap_or(mark.y)
            )
        }
        AnnotationKind::Text => format!("Text {:?}, at {}, {}", mark.text.as_deref().unwrap_or(""), mark.x, mark.y),
        AnnotationKind::Number => format!("Step number {}, at {}, {}", mark.number.unwrap_or(0), mark.x, mark.y),
        AnnotationKind::Cursor => format!("Pointer, at {}, {}", mark.x, mark.y),
        AnnotationKind::ClickMark => format!("Click marker, at {}, {}", mark.x, mark.y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u32 = 1920;
    const H: u32 = 1080;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Annotation {
        let mut a = Annotation::at("a1", AnnotationKind::Rect, x, y);
        a.width = Some(w);
        a.height = Some(h);
        a
    }

    fn line(x: i32, y: i32, x2: i32, y2: i32) -> Annotation {
        let mut a = Annotation::at("a1", AnnotationKind::Line, x, y);
        a.x2 = Some(x2);
        a.y2 = Some(y2);
        a
    }

    #[test]
    fn every_tool_starts_with_a_mark_the_format_accepts() {
        for tool in Tool::ALL {
            let mark = default_mark(tool, W, H, None, 0);
            assert_eq!(mark.kind, tool.kind());
            assert!(mark.problems().is_empty(), "{tool:?}: {:?}", mark.problems());
            let b = bounds(&mark, H);
            assert!(b.x >= 0 && b.y >= 0 && b.x < W as i32 && b.y < H as i32, "{tool:?} is on the picture: {b:?}");
        }
    }

    #[test]
    fn a_new_mark_is_centred_on_the_click_and_kept_on_the_picture() {
        let at_click = default_mark(Tool::Rectangle, W, H, Some((900, 500)), 0);
        let b = bounds(&at_click, H);
        assert_eq!((b.x + b.width / 2, b.y + b.height / 2), (900, 500));
        let in_the_corner = default_mark(Tool::Rectangle, W, H, Some((2, 2)), 0);
        assert!(in_the_corner.x >= 0 && in_the_corner.y >= 0, "moved back inside, not cut off");
        let far = default_mark(Tool::Oval, W, H, Some((5000, 5000)), 0);
        assert!(far.x + far.width.unwrap() <= W as i32 && far.y + far.height.unwrap() <= H as i32);
    }

    #[test]
    fn step_numbers_count_on_from_those_already_there() {
        assert_eq!(default_mark(Tool::Number, W, H, None, 0).number, Some(1));
        assert_eq!(default_mark(Tool::Number, W, H, None, 2).number, Some(3));
    }

    #[test]
    fn a_line_is_hit_near_it_and_not_far_from_it() {
        let l = line(100, 100, 300, 100);
        assert!(hit(&l, 200, 100, H));
        assert!(hit(&l, 200, 104, H), "a few pixels off still counts, so a thin line can be picked");
        assert!(!hit(&l, 200, 130, H));
        assert!(!hit(&l, 320, 100, H));
        assert!(hit(&l, 100, 100, H), "the ends count");
    }

    #[test]
    fn a_box_is_hit_inside_and_the_last_mark_in_the_list_wins() {
        let marks = vec![rect(100, 100, 200, 100), rect(150, 120, 50, 50), line(0, 500, 50, 500)];
        assert_eq!(top_mark_at(&marks, 160, 130, H), Some(1), "the one drawn on top");
        assert_eq!(top_mark_at(&marks, 110, 110, H), Some(0));
        assert_eq!(top_mark_at(&marks, 25, 502, H), Some(2));
        assert_eq!(top_mark_at(&marks, 900, 900, H), None);
        assert_eq!(top_mark_at(&[], 1, 1, H), None);
    }

    #[test]
    fn nudging_moves_the_whole_mark_and_stops_at_the_edges() {
        let mut r = rect(100, 100, 200, 100);
        nudge(&mut r, 10, -5, W, H);
        assert_eq!((r.x, r.y, r.width, r.height), (110, 95, Some(200), Some(100)));
        nudge(&mut r, -5000, -5000, W, H);
        assert_eq!((r.x, r.y), (0, 0), "stops at the corner, not squashed");
        nudge(&mut r, 9000, 9000, W, H);
        assert_eq!((r.x + 200, r.y + 100), (W as i32, H as i32));
        let mut l = line(10, 10, 110, 60);
        nudge(&mut l, 20, 30, W, H);
        assert_eq!((l.x, l.y, l.x2, l.y2), (30, 40, Some(130), Some(90)), "both ends move together");
        nudge(&mut l, -9000, 0, W, H);
        assert_eq!((l.x, l.x2), (0, Some(100)), "the whole line stops with its left end at the edge");
    }

    #[test]
    fn resizing_a_box_by_a_corner_moves_that_corner_and_keeps_a_minimum() {
        let mut r = rect(100, 100, 200, 100);
        resize(&mut r, Handle::BottomRight, 50, 20, W, H);
        assert_eq!((r.x, r.y, r.width, r.height), (100, 100, Some(250), Some(120)));
        resize(&mut r, Handle::TopLeft, -40, -10, W, H);
        assert_eq!((r.x, r.y, r.width, r.height), (60, 90, Some(290), Some(130)));
        resize(&mut r, Handle::Right, -9999, 0, W, H);
        assert_eq!(r.width, Some(MIN_SIDE), "never smaller than the minimum");
        resize(&mut r, Handle::Bottom, 99999, 99999, W, H);
        assert_eq!(r.y + r.height.unwrap(), H as i32, "never off the picture");
    }

    #[test]
    fn dragging_an_edge_handle_changes_only_one_direction() {
        let mut r = rect(100, 100, 200, 100);
        resize(&mut r, Handle::Right, 30, 99, W, H);
        assert_eq!((r.width, r.height, r.y), (Some(230), Some(100), 100));
        resize(&mut r, Handle::Top, 99, 20, W, H);
        assert_eq!((r.x, r.width, r.y, r.height), (100, Some(230), 120, Some(80)));
    }

    #[test]
    fn a_lines_ends_move_one_at_a_time() {
        let mut l = line(100, 100, 300, 100);
        resize(&mut l, Handle::End, 40, 60, W, H);
        assert_eq!((l.x, l.y, l.x2, l.y2), (100, 100, Some(340), Some(160)));
        resize(&mut l, Handle::Start, -20, -30, W, H);
        assert_eq!((l.x, l.y, l.x2, l.y2), (80, 70, Some(340), Some(160)));
        resize(&mut l, Handle::End, 99999, -99999, W, H);
        assert_eq!((l.x2, l.y2), (Some(W as i32 - 1), Some(0)));
    }

    #[test]
    fn round_marks_grow_and_shrink_about_their_centre() {
        let mut n = default_mark(Tool::Number, W, H, Some((500, 500)), 0);
        let before = n.size.unwrap();
        resize(&mut n, Handle::BottomRight, 5, 0, W, H);
        assert_eq!(n.size, Some(before + 10));
        assert_eq!((n.x, n.y), (500, 500), "the centre stays");
        resize(&mut n, Handle::BottomRight, -9999, 0, W, H);
        assert_eq!(n.size, Some(MIN_SIDE));
    }

    #[test]
    fn text_marks_change_width_by_their_right_handle() {
        let mut t = default_mark(Tool::Text, W, H, Some((400, 300)), 0);
        let before = bounds(&t, H).width;
        resize(&mut t, Handle::Right, 80, 0, W, H);
        assert_eq!(bounds(&t, H).width, before + 80);
        assert_eq!(handles(&t, H).len(), 1);
    }

    #[test]
    fn handles_are_found_near_their_places_and_each_kind_has_the_right_number() {
        let r = rect(100, 100, 200, 100);
        assert_eq!(handles(&r, H).len(), 8);
        assert_eq!(handle_at(&r, 103, 98, H), Some(Handle::TopLeft));
        assert_eq!(handle_at(&r, 301, 150, H), Some(Handle::Right));
        assert_eq!(handle_at(&r, 200, 150, H), None, "the middle is not a handle");
        assert_eq!(handles(&line(1, 1, 5, 5), H).len(), 2);
        assert_eq!(handles(&default_mark(Tool::Number, W, H, None, 0), H).len(), 1);
    }

    #[test]
    fn colours_set_the_right_field_and_a_highlight_stays_see_through() {
        let mut r = rect(0, 0, 10, 10);
        set_colour(&mut r, "#E5484D");
        assert_eq!(r.stroke.as_deref(), Some("#E5484D"));
        assert_eq!(colour_name(&r), Some("Red"));
        let mut h = Annotation::at("a2", AnnotationKind::Highlight, 0, 0);
        h.width = Some(10);
        h.height = Some(10);
        set_colour(&mut h, "#30A46C");
        assert_eq!(h.fill.as_deref(), Some("#30A46C66"));
        assert_eq!(colour_name(&h), Some("Green"));
        let mut b = Annotation::at("a3", AnnotationKind::Blur, 0, 0);
        set_colour(&mut b, "#E5484D");
        assert!(b.fill.is_none() && b.stroke.is_none(), "a blur has no colour to choose");
        assert_eq!(colour_field(AnnotationKind::Blur), None);
        assert_eq!(colour_name(&rect(0, 0, 5, 5)), None, "no colour set yet");
    }

    #[test]
    fn marks_are_said_in_words() {
        assert_eq!(describe(&rect(120, 80, 200, 100), H), "Rectangle, 200 by 100, at 120, 80");
        assert_eq!(describe(&line(1, 2, 3, 4), H), "Line, from 1, 2 to 3, 4");
        let mut t = Annotation::at("a", AnnotationKind::Text, 5, 6);
        t.text = Some("Click here".to_string());
        assert_eq!(describe(&t, H), "Text \"Click here\", at 5, 6");
        assert_eq!(describe(&default_mark(Tool::Number, W, H, Some((10, 20)), 3), H), "Step number 4, at 10, 20");
        let mut c = Annotation::at("a", AnnotationKind::ClickMark, 7, 8);
        c.size = Some(56);
        assert_eq!(describe(&c, H), "Click marker, at 7, 8");
    }

    #[test]
    fn a_mark_bigger_than_the_picture_is_made_to_fit() {
        let mut r = rect(-50, -50, 5000, 5000);
        keep_on_picture(&mut r, 400, 300);
        assert_eq!((r.x, r.y, r.width, r.height), (0, 0, Some(400), Some(300)));
    }

    #[test]
    fn what_can_be_resized_and_what_has_a_width() {
        assert!(
            can_resize(AnnotationKind::Rect) && can_resize(AnnotationKind::Arrow) && can_resize(AnnotationKind::Number)
        );
        assert!(can_resize(AnnotationKind::Highlight) && can_resize(AnnotationKind::ClickMark));
        assert!(
            has_width(AnnotationKind::Oval) && !has_width(AnnotationKind::Blur) && !has_width(AnnotationKind::Text)
        );
    }
}
