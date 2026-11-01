// SPDX-License-Identifier: Apache-2.0
//! Drawing a step's marks on its picture (spec sections 8.6, 9 and 10): the pixels an export shows.
//!
//! Everything is drawn on OpenStage Capture's `Canvas`, which uses only whole numbers and arithmetic every computer does
//! the same way, so the same step always gives the same bytes and every mark can be tested pixel by pixel on any
//! computer. The marks are drawn on the whole original picture in list order, and only then is the picture cropped to the
//! step's focused view, so a mark that crosses the edge of the crop stays where it belongs.
//!
//! The picture itself is never touched: [`render_step`] draws on a copy.
//!
//! Blur is a mosaic of flat blocks. Each block becomes the average colour of what was in it, so the detail is gone and
//! cannot be calculated back; a soft blur could be partly undone. A blacked-out area is the way to hide a secret.
//!
//! Text is drawn by a [`TextRenderer`] (Windows' own text drawing on Windows; blocks in tests). Marks drawn here use
//! their own artwork (the pointers, the hand), not any other program's.

use openstage_capture_project::EditAlign;
use openstage_capture_render::{
    padding, wrap_lines, Canvas, Picture, PixelRect, Rgb, SourceRect, TextRenderer, TextRequest, LINE_SPACING,
};

use crate::focus::clamp_inside;
use crate::model::{Annotation, AnnotationKind, Rect, Step};

/// The brand's indigo, used for marks that have no colour of their own.
pub const ACCENT: Rgb = Rgb(0x7D, 0x87, 0xF2);
/// The brand's navy.
pub const NAVY: Rgb = Rgb(0x0B, 0x17, 0x2A);
const WHITE: Rgb = Rgb(255, 255, 255);
const BLACK: Rgb = Rgb(0, 0, 0);

/// A colour with how solid it is (255 is solid, 0 is clear).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colour {
    pub rgb: Rgb,
    pub alpha: u8,
}

impl Colour {
    fn solid(rgb: Rgb) -> Colour {
        Colour { rgb, alpha: 255 }
    }

    fn with_alpha(rgb: Rgb, alpha: u8) -> Colour {
        Colour { rgb, alpha }
    }

    /// Read `#RRGGBB` or `#RRGGBBAA`.
    pub fn parse(text: &str) -> Option<Colour> {
        let hex = text.strip_prefix('#')?;
        if (hex.len() != 6 && hex.len() != 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let part = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        let alpha = if hex.len() == 8 { part(6)? } else { 255 };
        Some(Colour { rgb: Rgb(part(0)?, part(2)?, part(4)?), alpha })
    }
}

/// What drawing the marks found that the person may want to fix.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct DrawReport {
    /// Marks whose text did not all fit in its box, so some words were cut off.
    pub text_cut_off: Vec<String>,
    /// Marks whose text could not be drawn at all, with the reason.
    pub text_errors: Vec<String>,
}

/// A step's picture with its marks, cropped to its focused view.
#[derive(Debug)]
pub struct RenderedStep {
    pub canvas: Canvas,
    pub report: DrawReport,
}

/// Draw every mark of a step on a copy of its picture and crop it to the step's focused view (the whole picture if none).
pub fn render_step(picture: &Picture, step: &Step, text: &dyn TextRenderer) -> RenderedStep {
    let mut full = Canvas::new(picture.width, picture.height);
    full.draw_picture(
        picture,
        SourceRect { x: 0.0, y: 0.0, w: picture.width as f64, h: picture.height as f64 },
        PixelRect::new(0, 0, picture.width as i32, picture.height as i32),
    );
    let report = draw_annotations(&mut full, &step.annotations, text);
    let canvas = match &step.focus {
        Some(focus) => crop_canvas(&full, focus),
        None => full,
    };
    RenderedStep { canvas, report }
}

/// A copy of the part of a canvas inside `rect` (moved inside the canvas first if it hangs over an edge).
pub fn crop_canvas(canvas: &Canvas, rect: &Rect) -> Canvas {
    if canvas.width == 0 || canvas.height == 0 {
        return canvas.clone();
    }
    let r = clamp_inside(rect, canvas.width, canvas.height);
    let mut out = Canvas::new(r.width as u32, r.height as u32);
    let row_bytes = r.width as usize * 4;
    for row in 0..r.height as usize {
        let from = ((r.y as usize + row) * canvas.width as usize + r.x as usize) * 4;
        let to = row * out.width as usize * 4;
        out.bgra[to..to + row_bytes].copy_from_slice(&canvas.bgra[from..from + row_bytes]);
    }
    out
}

/// A copy of a canvas scaled to `width` pixels across, the height following (rounded, never zero). A canvas made smaller is
/// averaged so thin lines stay readable.
pub fn scale_to_width(canvas: &Canvas, width: u32) -> Canvas {
    if width == 0 || canvas.width == 0 || canvas.height == 0 || width == canvas.width {
        return canvas.clone();
    }
    let height = ((canvas.height as u64 * width as u64 + canvas.width as u64 / 2) / canvas.width as u64).max(1) as u32;
    let picture = Picture::solid_pixels(canvas.width, canvas.height, canvas.bgra.clone());
    let mut out = Canvas::new(width, height);
    out.draw_picture(
        &picture,
        SourceRect { x: 0.0, y: 0.0, w: canvas.width as f64, h: canvas.height as f64 },
        PixelRect::new(0, 0, width as i32, height as i32),
    );
    out
}

/// Draw marks on a canvas, in list order (a later mark is drawn over an earlier one).
pub fn draw_annotations(canvas: &mut Canvas, annotations: &[Annotation], text: &dyn TextRenderer) -> DrawReport {
    let mut report = DrawReport::default();
    for mark in annotations {
        match mark.kind {
            AnnotationKind::Rect => draw_rect(canvas, mark),
            AnnotationKind::Oval => draw_oval(canvas, mark),
            AnnotationKind::Line => draw_line(canvas, mark, false),
            AnnotationKind::Arrow => draw_line(canvas, mark, true),
            AnnotationKind::Highlight => draw_highlight(canvas, mark),
            AnnotationKind::Blur => {
                if let Some(rect) = rect_of(mark) {
                    mosaic(canvas, rect);
                }
            }
            AnnotationKind::Spotlight => draw_spotlight(canvas, mark),
            AnnotationKind::Text => draw_text(canvas, mark, text, &mut report),
            AnnotationKind::Number => draw_number(canvas, mark, text, &mut report),
            AnnotationKind::Cursor => draw_cursor(canvas, mark),
            AnnotationKind::ClickMark => draw_click_mark(canvas, mark),
        }
    }
    report
}

// ---- small helpers

fn rect_of(mark: &Annotation) -> Option<PixelRect> {
    let (w, h) = (mark.width?, mark.height?);
    (w > 0 && h > 0).then(|| PixelRect::new(mark.x, mark.y, w, h))
}

fn colour_or_none(text: &Option<String>) -> Option<Colour> {
    text.as_deref().and_then(Colour::parse)
}

fn stroke_width(canvas: &Canvas, mark: &Annotation) -> i32 {
    mark.stroke_width.filter(|w| *w > 0).unwrap_or((canvas.height as i32 / 270).max(3))
}

/// Cover a rectangle with a colour, as much of it as its alpha says.
fn fill_alpha(canvas: &mut Canvas, rect: PixelRect, colour: Colour) {
    if colour.alpha == 0 {
        return;
    }
    let r = rect.clipped(canvas.width, canvas.height);
    for y in r.y..r.y + r.h {
        for x in r.x..r.x + r.w {
            if colour.alpha == 255 {
                canvas.set_pixel(x as u32, y as u32, colour.rgb);
            } else {
                canvas.blend_pixel(x as u32, y as u32, colour.rgb, colour.alpha as u32);
            }
        }
    }
}

/// The edge of a rectangle, `line` pixels thick and inside the rectangle, with alpha.
fn stroke_alpha(canvas: &mut Canvas, rect: PixelRect, line: i32, colour: Colour) {
    let line = line.max(1).min(rect.w / 2 + 1).min(rect.h / 2 + 1);
    fill_alpha(canvas, PixelRect::new(rect.x, rect.y, rect.w, line), colour);
    fill_alpha(canvas, PixelRect::new(rect.x, rect.y + rect.h - line, rect.w, line), colour);
    fill_alpha(canvas, PixelRect::new(rect.x, rect.y + line, line, rect.h - 2 * line), colour);
    fill_alpha(canvas, PixelRect::new(rect.x + rect.w - line, rect.y + line, line, rect.h - 2 * line), colour);
}

/// Draw something solid on a copy of the canvas and lay it over the real one by `alpha`. Smoothed edges and everything
/// else work as if the colour had been translucent from the start.
fn lay_alpha(canvas: &mut Canvas, area: PixelRect, alpha: u8, draw: impl FnOnce(&mut Canvas)) {
    match alpha {
        0 => {}
        255 => draw(canvas),
        _ => {
            let mut top = canvas.clone();
            draw(&mut top);
            canvas.lay_over(&top, area, alpha as u32, 0);
        }
    }
}

/// White or navy, whichever reads better on this colour. Brightness is worked out with whole numbers (the colours squared,
/// weighted as the eye sees them), which is close to the real thing and the same on every computer. The turning point is
/// where white and navy give the same contrast ratio, so white text goes only on genuinely dark colours: white on the
/// brand's indigo would be only about 3 to 1, and navy on it is about 5.6 to 1.
fn contrast_text(background: Rgb) -> Rgb {
    let (r, g, b) = (background.0 as u64, background.1 as u64, background.2 as u64);
    let brightness = (2126 * r * r + 7152 * g * g + 722 * b * b) / 10_000;
    if brightness < 12_700 {
        WHITE
    } else {
        NAVY
    }
}

// ---- shapes

fn draw_rect(canvas: &mut Canvas, mark: &Annotation) {
    let Some(rect) = rect_of(mark) else { return };
    let fill = colour_or_none(&mark.fill);
    let mut stroke = colour_or_none(&mark.stroke);
    if fill.is_none() && stroke.is_none() {
        stroke = Some(Colour::solid(ACCENT));
    }
    if let Some(fill) = fill {
        fill_alpha(canvas, rect, fill);
    }
    if let Some(stroke) = stroke {
        stroke_alpha(canvas, rect, stroke_width(canvas, mark), stroke);
    }
}

fn draw_oval(canvas: &mut Canvas, mark: &Annotation) {
    let Some(rect) = rect_of(mark) else { return };
    let fill = colour_or_none(&mark.fill);
    let mut stroke = colour_or_none(&mark.stroke);
    if fill.is_none() && stroke.is_none() {
        stroke = Some(Colour::solid(ACCENT));
    }
    ellipse_with_alpha(canvas, rect, fill, stroke, stroke_width(canvas, mark));
}

fn ellipse_with_alpha(canvas: &mut Canvas, rect: PixelRect, fill: Option<Colour>, stroke: Option<Colour>, line: i32) {
    let area = PixelRect::new(rect.x - 2, rect.y - 2, rect.w + 4, rect.h + 4);
    if let Some(fill) = fill {
        lay_alpha(canvas, area, fill.alpha, |c| c.ellipse(rect, 1.0, fill.rgb, Some(fill.rgb)));
    }
    if let Some(stroke) = stroke {
        lay_alpha(canvas, area, stroke.alpha, |c| c.ellipse(rect, line as f32, stroke.rgb, None));
    }
}

fn draw_line(canvas: &mut Canvas, mark: &Annotation, arrow: bool) {
    let (Some(x2), Some(y2)) = (mark.x2, mark.y2) else { return };
    let stroke = colour_or_none(&mark.stroke).unwrap_or(Colour::solid(ACCENT));
    let width = stroke_width(canvas, mark);
    let (from, to) = ((mark.x as f32, mark.y as f32), (x2 as f32, y2 as f32));
    let pad = width * 5 + 12;
    let area = PixelRect::new(
        mark.x.min(x2) - pad,
        mark.y.min(y2) - pad,
        (mark.x - x2).abs() + 2 * pad,
        (mark.y - y2).abs() + 2 * pad,
    );
    lay_alpha(canvas, area, stroke.alpha, |c| {
        if arrow {
            c.arrow(from, to, width as f32, stroke.rgb);
        } else {
            c.line(from, to, width as f32, stroke.rgb);
        }
    });
}

fn draw_highlight(canvas: &mut Canvas, mark: &Annotation) {
    let Some(rect) = rect_of(mark) else { return };
    let fill = colour_or_none(&mark.fill).unwrap_or(Colour::with_alpha(Rgb(0xFF, 0xE6, 0x00), 0x66));
    fill_alpha(canvas, rect, fill);
    if let Some(stroke) = colour_or_none(&mark.stroke) {
        stroke_alpha(canvas, rect, stroke_width(canvas, mark), stroke);
    }
}

/// Darken everything outside the rectangle so the eye goes to what is inside it.
fn draw_spotlight(canvas: &mut Canvas, mark: &Annotation) {
    let Some(rect) = rect_of(mark) else { return };
    let dim = colour_or_none(&mark.fill).unwrap_or(Colour::with_alpha(BLACK, 0xA0));
    let (w, h) = (canvas.width as i32, canvas.height as i32);
    let hole = rect.clipped(canvas.width, canvas.height);
    // The four bands round the hole, which together are every pixel outside it.
    fill_alpha(canvas, PixelRect::new(0, 0, w, hole.y), dim);
    fill_alpha(canvas, PixelRect::new(0, hole.y + hole.h, w, h - hole.y - hole.h), dim);
    fill_alpha(canvas, PixelRect::new(0, hole.y, hole.x, hole.h), dim);
    fill_alpha(canvas, PixelRect::new(hole.x + hole.w, hole.y, w - hole.x - hole.w, hole.h), dim);
    if let Some(stroke) = colour_or_none(&mark.stroke) {
        stroke_alpha(canvas, rect, stroke_width(canvas, mark), stroke);
    }
}

/// Replace each block of a rectangle by its average colour. The blocks are a third of the shorter side (at least 12 and
/// at most 48 pixels), so small print is far smaller than one block. What was in a block cannot be worked out again.
pub fn mosaic(canvas: &mut Canvas, rect: PixelRect) {
    let r = rect.clipped(canvas.width, canvas.height);
    if r.is_empty() {
        return;
    }
    let block = (r.w.min(r.h) / 3).clamp(12, 48);
    let mut top = r.y;
    while top < r.y + r.h {
        let height = block.min(r.y + r.h - top);
        let mut left = r.x;
        while left < r.x + r.w {
            let width = block.min(r.x + r.w - left);
            let mut sum = [0u64; 3];
            for y in top..top + height {
                for x in left..left + width {
                    let Rgb(red, green, blue) = canvas.pixel(x as u32, y as u32);
                    sum[0] += red as u64;
                    sum[1] += green as u64;
                    sum[2] += blue as u64;
                }
            }
            let count = (width * height) as u64;
            let average = Rgb(
                ((sum[0] + count / 2) / count) as u8,
                ((sum[1] + count / 2) / count) as u8,
                ((sum[2] + count / 2) / count) as u8,
            );
            canvas.fill_rect(PixelRect::new(left, top, width, height), average);
            left += width;
        }
        top += height;
    }
}

// ---- marks with words

fn draw_text(canvas: &mut Canvas, mark: &Annotation, renderer: &dyn TextRenderer, report: &mut DrawReport) {
    let content = mark.text.as_deref().unwrap_or("");
    let font_px = mark.font_size.filter(|f| *f > 0).unwrap_or((canvas.height as i32 / 40).max(16)) as f32;
    let pad = padding(font_px);
    let letter = font_px * 0.6;
    let width_of = |s: &str| s.chars().count() as f32 * letter;
    let longest = content.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let narrowest = 2 * pad + letter.ceil() as i32;
    let width = match mark.width.filter(|w| *w > 0) {
        Some(w) => w,
        None => ((longest as f32 * letter).ceil() as i32 + 2 * pad).min((canvas.width as i32 - mark.x).max(narrowest)),
    }
    .max(narrowest);
    let lines = wrap_lines(content, (width - 2 * pad) as f32, &width_of);
    let height = (pad as f32 + (lines.len() - 1) as f32 * font_px * LINE_SPACING + font_px).ceil() as i32 + pad;
    let area = PixelRect::new(mark.x, mark.y, width, height);
    let fill = colour_or_none(&mark.fill).unwrap_or(Colour::solid(NAVY));
    fill_alpha(canvas, area, fill);
    let colour = colour_or_none(&mark.stroke).map(|c| c.rgb).unwrap_or_else(|| contrast_text(fill.rgb));
    let request = TextRequest {
        text: content.to_string(),
        area,
        font_px,
        bold: false,
        color: colour,
        background: None,
        align: EditAlign::Left,
    };
    match renderer.draw(canvas, &request) {
        Ok(fit) if !fit.complete => report.text_cut_off.push(mark.id.clone()),
        Ok(_) => {}
        Err(reason) => report.text_errors.push(format!("{}: {reason}", mark.id)),
    }
}

fn draw_number(canvas: &mut Canvas, mark: &Annotation, renderer: &dyn TextRenderer, report: &mut DrawReport) {
    let Some(number) = mark.number else { return };
    let size = mark.size.filter(|s| *s > 0).unwrap_or((canvas.height as i32 / 25).max(28));
    let rect = PixelRect::new(mark.x - size / 2, mark.y - size / 2, size, size);
    let fill = colour_or_none(&mark.fill).unwrap_or(Colour::solid(ACCENT));
    ellipse_with_alpha(canvas, rect, Some(fill), colour_or_none(&mark.stroke), stroke_width(canvas, mark));
    let font_px = size as f32 * 0.55;
    let pad = padding(font_px);
    let line = (font_px * LINE_SPACING).round() as i32;
    let area = PixelRect::new(rect.x, mark.y - (pad as f32 + font_px / 2.0).round() as i32, size, line + 2 * pad);
    let request = TextRequest {
        text: number.to_string(),
        area,
        font_px,
        bold: true,
        color: contrast_text(fill.rgb),
        background: None,
        align: EditAlign::Center,
    };
    match renderer.draw(canvas, &request) {
        Ok(fit) if !fit.complete => report.text_cut_off.push(mark.id.clone()),
        Ok(_) => {}
        Err(reason) => report.text_errors.push(format!("{}: {reason}", mark.id)),
    }
}

// ---- the click mark and the pointer

fn draw_click_mark(canvas: &mut Canvas, mark: &Annotation) {
    let Some(size) = mark.size.filter(|s| *s > 0) else { return };
    let rect = PixelRect::new(mark.x - size / 2, mark.y - size / 2, size, size);
    let fill = colour_or_none(&mark.fill).unwrap_or(Colour::with_alpha(ACCENT, 0x33));
    let stroke = colour_or_none(&mark.stroke).unwrap_or(Colour::solid(ACCENT));
    ellipse_with_alpha(canvas, rect, Some(fill), Some(stroke), stroke_width(canvas, mark));
}

/// A point is inside a polygon (the even-odd rule).
fn inside_polygon(px: f32, py: f32, polygon: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let ((xi, yi), (xj, yj)) = (polygon[i], polygon[j]);
        if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Fill a polygon with smoothed edges (sixteen sample points in each pixel).
fn fill_polygon(canvas: &mut Canvas, polygon: &[(f32, f32)], colour: Colour) {
    let min_x = polygon.iter().map(|p| p.0).fold(f32::MAX, f32::min).floor() as i32 - 1;
    let min_y = polygon.iter().map(|p| p.1).fold(f32::MAX, f32::min).floor() as i32 - 1;
    let max_x = polygon.iter().map(|p| p.0).fold(f32::MIN, f32::max).ceil() as i32 + 1;
    let max_y = polygon.iter().map(|p| p.1).fold(f32::MIN, f32::max).ceil() as i32 + 1;
    let area = PixelRect::new(min_x, min_y, max_x - min_x, max_y - min_y).clipped(canvas.width, canvas.height);
    for y in area.y..area.y + area.h {
        for x in area.x..area.x + area.w {
            let mut hits = 0u32;
            for sy in 0..4 {
                for sx in 0..4 {
                    let (px, py) = (x as f32 + (sx as f32 + 0.5) / 4.0, y as f32 + (sy as f32 + 0.5) / 4.0);
                    if inside_polygon(px, py, polygon) {
                        hits += 1;
                    }
                }
            }
            let share = hits * 255 / 16 * colour.alpha as u32 / 255;
            canvas.blend_pixel(x as u32, y as u32, colour.rgb, share);
        }
    }
}

fn outline_polygon(canvas: &mut Canvas, polygon: &[(f32, f32)], width: f32, colour: Rgb) {
    for i in 0..polygon.len() {
        canvas.line(polygon[i], polygon[(i + 1) % polygon.len()], width, colour);
    }
}

/// A mouse pointer drawn at the mark's position (its hot spot). `style` is `pointer` (the default), `hand` or `text`.
fn draw_cursor(canvas: &mut Canvas, mark: &Annotation) {
    let size = mark.size.filter(|s| *s > 0).unwrap_or((canvas.height as i32 / 38).max(24)).max(8) as f32;
    let body = colour_or_none(&mark.fill).unwrap_or(Colour::solid(WHITE));
    let edge = colour_or_none(&mark.stroke).unwrap_or(Colour::solid(BLACK));
    let (x, y) = (mark.x as f32, mark.y as f32);
    match mark.style.as_deref().unwrap_or("pointer") {
        "text" => {
            // An I-beam 19 units tall: a bar with a short bar across each end.
            let s = size / 19.0;
            let at = |ux: f32, uy: f32| (x + (ux - 6.0) * s, y + uy * s);
            for (width, colour) in [(2.6 * s, edge.rgb), (1.0 * s, body.rgb)] {
                canvas.line(at(6.0, 1.0), at(6.0, 18.0), width.max(1.0), colour);
                canvas.line(at(3.0, 1.0), at(9.0, 1.0), width.max(1.0), colour);
                canvas.line(at(3.0, 18.0), at(9.0, 18.0), width.max(1.0), colour);
            }
        }
        "hand" | "macHand" => {
            // A pointing hand 22 units tall: one raised finger and a palm. The hot spot is the fingertip.
            let s = size / 22.0;
            let at = |ux: f32, uy: f32| (x + (ux - 6.5) * s, y + (uy - 0.6) * s);
            let finger: Vec<(f32, f32)> = [(5.0, 2.0), (5.7, 0.6), (7.3, 0.6), (8.0, 2.0), (8.0, 10.0), (5.0, 10.0)]
                .iter()
                .map(|p| at(p.0, p.1))
                .collect();
            let palm: Vec<(f32, f32)> = [(3.0, 9.5), (14.5, 9.5), (14.5, 17.0), (12.0, 21.5), (5.5, 21.5), (3.0, 17.0)]
                .iter()
                .map(|p| at(p.0, p.1))
                .collect();
            for shape in [&palm, &finger] {
                fill_polygon(canvas, shape, body);
            }
            for shape in [&palm, &finger] {
                outline_polygon(canvas, shape, (s * 1.1).max(1.0), edge.rgb);
            }
            // Cover the line where the finger joins the palm, so they read as one hand.
            canvas.line(at(5.4, 9.6), at(7.6, 9.6), (s * 1.4).max(1.0), body.rgb);
        }
        _ => {
            // An arrow 19 units tall whose tip is the hot spot.
            let s = size / 19.0;
            let outline: Vec<(f32, f32)> =
                [(0.0, 0.0), (0.0, 16.0), (4.2, 12.4), (7.0, 19.0), (9.8, 17.8), (7.2, 11.4), (12.4, 11.4)]
                    .iter()
                    .map(|p| (x + p.0 * s, y + p.1 * s))
                    .collect();
            fill_polygon(canvas, &outline, body);
            outline_polygon(canvas, &outline, (s * 1.1).max(1.0), edge.rgb);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstage_capture_render::BlockText;

    fn white_canvas(w: u32, h: u32) -> Canvas {
        let mut canvas = Canvas::new(w, h);
        canvas.clear(WHITE);
        canvas
    }

    /// How the canvas mixes a colour over another by a share (0 to 255).
    fn mix(old: u8, new: u8, share: u32) -> u8 {
        ((old as u32 * (255 - share) + new as u32 * share + 127) / 255) as u8
    }

    fn mark(kind: AnnotationKind, x: i32, y: i32) -> Annotation {
        Annotation::at("a1", kind, x, y)
    }

    fn boxed(kind: AnnotationKind, x: i32, y: i32, w: i32, h: i32) -> Annotation {
        let mut a = mark(kind, x, y);
        a.width = Some(w);
        a.height = Some(h);
        a
    }

    fn draw(canvas: &mut Canvas, a: Annotation) -> DrawReport {
        draw_annotations(canvas, &[a], &BlockText)
    }

    fn count(canvas: &Canvas, colour: Rgb) -> usize {
        (0..canvas.height)
            .flat_map(|y| (0..canvas.width).map(move |x| (x, y)))
            .filter(|&(x, y)| canvas.pixel(x, y) == colour)
            .count()
    }

    fn changed(canvas: &Canvas, original: &Canvas) -> usize {
        (0..canvas.bgra.len() / 4).filter(|&i| canvas.bgra[i * 4..i * 4 + 4] != original.bgra[i * 4..i * 4 + 4]).count()
    }

    #[test]
    fn colours_parse_with_and_without_alpha() {
        assert_eq!(Colour::parse("#7D87F2"), Some(Colour { rgb: ACCENT, alpha: 255 }));
        assert_eq!(Colour::parse("#7d87f233"), Some(Colour { rgb: ACCENT, alpha: 0x33 }));
        for bad in ["", "7D87F2", "#7D87F", "#7D87F2F", "#GGGGGG", "red", "#7D87F2334"] {
            assert_eq!(Colour::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_rectangle_outline_is_drawn_inside_the_rectangle_and_nothing_else_changes() {
        let mut c = white_canvas(100, 80);
        let mut a = boxed(AnnotationKind::Rect, 10, 10, 40, 30);
        a.stroke = Some("#FF0000".into());
        a.stroke_width = Some(3);
        draw(&mut c, a);
        let red = Rgb(255, 0, 0);
        for (x, y) in [(10, 10), (49, 39), (12, 20), (30, 12), (30, 37), (47, 20)] {
            assert_eq!(c.pixel(x, y), red, "({x},{y}) should be on the outline");
        }
        for (x, y) in [(13, 20), (30, 13), (30, 36), (46, 20), (30, 25), (9, 10), (50, 40), (10, 40), (10, 9)] {
            assert_eq!(c.pixel(x, y), WHITE, "({x},{y}) should be untouched");
        }
        assert_eq!(count(&c, red), 40 * 30 - 34 * 24, "exactly the ring of three pixels");
    }

    #[test]
    fn a_translucent_fill_mixes_with_what_is_under_it_exactly() {
        let mut c = white_canvas(60, 40);
        let mut a = boxed(AnnotationKind::Rect, 10, 10, 20, 20);
        a.fill = Some("#0000FF80".into());
        draw(&mut c, a);
        let inside = c.pixel(20, 20);
        assert_eq!(inside, Rgb(mix(255, 0, 128), mix(255, 0, 128), mix(255, 255, 128)));
        assert_eq!(c.pixel(9, 20), WHITE);
        assert_eq!(c.pixel(10, 10), inside, "a fill with no stroke covers the whole rectangle");
        assert_eq!(c.pixel(29, 29), inside);
        assert_eq!(c.pixel(30, 29), WHITE);
    }

    #[test]
    fn a_rectangle_with_neither_fill_nor_stroke_gets_the_default_outline() {
        let mut c = white_canvas(60, 40);
        draw(&mut c, boxed(AnnotationKind::Rect, 5, 5, 30, 20));
        assert_eq!(c.pixel(5, 5), ACCENT);
        assert_eq!(c.pixel(20, 15), WHITE);
    }

    #[test]
    fn a_bad_colour_is_treated_as_missing() {
        let mut c = white_canvas(60, 40);
        let mut a = boxed(AnnotationKind::Rect, 5, 5, 30, 20);
        a.fill = Some("red".into());
        a.stroke = Some("#12".into());
        draw(&mut c, a);
        assert_eq!(c.pixel(5, 5), ACCENT, "it falls back to the default outline");
        assert_eq!(c.pixel(20, 15), WHITE);
    }

    #[test]
    fn an_oval_is_round_and_stays_inside_its_box() {
        let mut c = white_canvas(100, 80);
        let mut a = boxed(AnnotationKind::Oval, 10, 10, 60, 40);
        a.fill = Some("#00FF00".into());
        a.stroke = Some("#000000".into());
        a.stroke_width = Some(4);
        draw(&mut c, a);
        assert_eq!(c.pixel(40, 30), Rgb(0, 255, 0), "the middle is filled");
        assert_eq!(c.pixel(11, 30), Rgb(0, 0, 0), "the left end of the ring");
        assert_eq!(c.pixel(10, 10), WHITE, "the corner of the box is outside the oval");
        assert_eq!(c.pixel(69, 49), WHITE);
        assert_eq!(c.pixel(5, 30), WHITE);
        assert_eq!(c.pixel(40, 55), WHITE);
    }

    #[test]
    fn a_translucent_oval_fill_mixes_exactly_in_the_middle() {
        let mut c = white_canvas(60, 60);
        let mut a = boxed(AnnotationKind::Oval, 10, 10, 40, 40);
        a.fill = Some("#FF000040".into());
        draw(&mut c, a);
        assert_eq!(c.pixel(30, 30), Rgb(mix(255, 255, 0x40), mix(255, 0, 0x40), mix(255, 0, 0x40)));
        assert_eq!(c.pixel(11, 11), WHITE, "the box corner is outside the circle");
    }

    #[test]
    fn a_line_and_an_arrow_are_drawn_between_their_ends() {
        let mut c = white_canvas(100, 60);
        let mut a = mark(AnnotationKind::Line, 10, 30);
        a.x2 = Some(90);
        a.y2 = Some(30);
        a.stroke = Some("#000000".into());
        a.stroke_width = Some(4);
        draw(&mut c, a);
        assert_eq!(c.pixel(50, 30), Rgb(0, 0, 0));
        assert_eq!(c.pixel(50, 29), Rgb(0, 0, 0));
        assert_eq!(c.pixel(50, 40), WHITE);
        assert_eq!(c.pixel(50, 20), WHITE);

        let mut c = white_canvas(100, 60);
        let mut a = mark(AnnotationKind::Arrow, 10, 30);
        a.x2 = Some(90);
        a.y2 = Some(30);
        a.stroke = Some("#FF0000".into());
        a.stroke_width = Some(3);
        draw(&mut c, a);
        assert_eq!(c.pixel(40, 30), Rgb(255, 0, 0), "the shaft");
        assert_eq!(c.pixel(85, 30), Rgb(255, 0, 0), "the head, near the tip");
        assert_eq!(c.pixel(80, 32), Rgb(255, 0, 0), "the head is wider than the shaft");
        assert_eq!(c.pixel(40, 32), WHITE, "and the shaft is not");
    }

    #[test]
    fn a_translucent_line_mixes_with_what_is_under_it() {
        let mut c = white_canvas(60, 40);
        let mut a = mark(AnnotationKind::Line, 5, 20);
        a.x2 = Some(55);
        a.y2 = Some(20);
        a.stroke = Some("#00000080".into());
        a.stroke_width = Some(6);
        draw(&mut c, a);
        let grey = mix(255, 0, 128);
        assert_eq!(c.pixel(30, 20), Rgb(grey, grey, grey));
        assert_eq!(c.pixel(30, 5), WHITE);
    }

    #[test]
    fn a_line_with_no_end_point_draws_nothing() {
        let mut c = white_canvas(40, 40);
        let before = c.clone();
        draw(&mut c, mark(AnnotationKind::Line, 5, 5));
        draw(&mut c, mark(AnnotationKind::Arrow, 5, 5));
        assert_eq!(c, before);
    }

    #[test]
    fn a_highlight_is_a_translucent_yellow_by_default() {
        let mut c = white_canvas(60, 40);
        draw(&mut c, boxed(AnnotationKind::Highlight, 10, 10, 30, 10));
        assert_eq!(c.pixel(20, 15), Rgb(mix(255, 0xFF, 0x66), mix(255, 0xE6, 0x66), mix(255, 0, 0x66)));
        assert_eq!(c.pixel(20, 21), WHITE);
    }

    #[test]
    fn a_spotlight_darkens_everything_outside_its_box_and_nothing_inside() {
        let mut c = white_canvas(80, 60);
        draw(&mut c, boxed(AnnotationKind::Spotlight, 20, 15, 30, 20));
        let dark = mix(255, 0, 0xA0);
        let dimmed = Rgb(dark, dark, dark);
        for (x, y) in [(0, 0), (79, 59), (19, 25), (50, 25), (30, 14), (30, 35), (0, 59), (79, 0)] {
            assert_eq!(c.pixel(x, y), dimmed, "({x},{y}) is outside the box");
        }
        for (x, y) in [(20, 15), (49, 34), (30, 25)] {
            assert_eq!(c.pixel(x, y), WHITE, "({x},{y}) is inside the box");
        }
        assert_eq!(count(&c, WHITE), 30 * 20);
    }

    #[test]
    fn a_spotlight_that_hangs_over_the_edge_still_works() {
        let mut c = white_canvas(40, 40);
        draw(&mut c, boxed(AnnotationKind::Spotlight, -10, -10, 30, 30));
        assert_eq!(c.pixel(0, 0), WHITE);
        assert_eq!(c.pixel(19, 19), WHITE);
        assert_ne!(c.pixel(20, 20), WHITE);
        let mut c = white_canvas(40, 40);
        draw(&mut c, boxed(AnnotationKind::Spotlight, 500, 500, 30, 30));
        assert_ne!(c.pixel(0, 0), WHITE, "a box wholly outside leaves everything dimmed");
    }

    /// A picture whose every pixel is black or white by a rule.
    fn pattern(w: u32, h: u32, black: impl Fn(u32, u32) -> bool) -> Canvas {
        let mut c = white_canvas(w, h);
        for y in 0..h {
            for x in 0..w {
                if black(x, y) {
                    c.set_pixel(x, y, BLACK);
                }
            }
        }
        c
    }

    #[test]
    fn blur_makes_every_block_one_flat_colour() {
        let mut c = pattern(120, 90, |x, y| (x + y) % 2 == 0);
        draw(&mut c, boxed(AnnotationKind::Blur, 0, 0, 120, 90));
        // The shorter side is 90, so the blocks are 30 pixels across.
        for by in 0..3 {
            for bx in 0..4 {
                let first = c.pixel(bx * 30, by * 30);
                for y in by * 30..by * 30 + 30 {
                    for x in bx * 30..bx * 30 + 30 {
                        assert_eq!(c.pixel(x, y), first, "block ({bx},{by}) is not flat at ({x},{y})");
                    }
                }
                assert_eq!(first, Rgb(128, 128, 128), "half black, half white");
            }
        }
    }

    #[test]
    fn blurred_detail_cannot_be_got_back_because_different_pictures_give_the_same_result() {
        // Two different pictures with the same amount of black in every block: a checkerboard and stripes.
        let mut a = pattern(120, 90, |x, y| (x + y) % 2 == 0);
        let mut b = pattern(120, 90, |x, _| x % 2 == 0);
        assert_ne!(a, b);
        mosaic(&mut a, PixelRect::new(0, 0, 120, 90));
        mosaic(&mut b, PixelRect::new(0, 0, 120, 90));
        assert_eq!(a, b, "the output depends only on each block's average, so nothing else about the input survives");
    }

    #[test]
    fn small_text_sized_detail_is_wiped_by_blur() {
        // A 30 by 12 pixel strip of black-and-white "text" in a bigger picture, blurred as a rectangle round it.
        let mut c = pattern(200, 100, |x, y| (40..170).contains(&x) && (44..56).contains(&y) && (x / 2 + y) % 3 == 0);
        let letters =
            (0..100).flat_map(|y| (0..200).map(move |x| (x, y))).filter(|&(x, y)| c.pixel(x, y) == BLACK).count();
        assert!(letters > 100);
        mosaic(&mut c, PixelRect::new(30, 30, 150, 40));
        let still_black =
            (30..180).flat_map(|x| (30..70).map(move |y| (x, y))).filter(|&(x, y)| c.pixel(x, y) == BLACK).count();
        assert_eq!(still_black, 0, "no pure black pixel survives inside the blurred area");
        assert_eq!(c.pixel(29, 50), WHITE, "outside the area is untouched");
        assert_eq!(c.pixel(180, 50), WHITE);
    }

    #[test]
    fn blur_handles_small_and_off_picture_rectangles() {
        let mut c = pattern(40, 40, |x, y| (x + y) % 2 == 0);
        let before = c.clone();
        mosaic(&mut c, PixelRect::new(100, 100, 20, 20));
        mosaic(&mut c, PixelRect::new(0, 0, 0, 5));
        mosaic(&mut c, PixelRect::new(-5, -5, 0, 0));
        assert_eq!(c, before);
        mosaic(&mut c, PixelRect::new(30, 30, 50, 50));
        assert_eq!(c.pixel(35, 35), c.pixel(39, 39), "the part inside the picture is one block");
        assert_eq!(c.pixel(0, 0), before.pixel(0, 0));
    }

    #[test]
    fn text_gets_a_background_box_sized_to_its_words() {
        let mut c = white_canvas(300, 120);
        let mut a = mark(AnnotationKind::Text, 20, 20);
        a.text = Some("Click here".into());
        a.font_size = Some(20);
        let report = draw(&mut c, a);
        assert_eq!(report, DrawReport::default(), "everything fitted");
        assert_eq!(c.pixel(20, 20), NAVY, "the default box is navy");
        assert!(count(&c, WHITE) < 300 * 120, "the box covers part of the picture");
        assert_ne!(count(&c, NAVY), 0);
        assert_eq!(c.pixel(19, 20), WHITE, "and stops at its edge");
        assert_eq!(c.pixel(20, 19), WHITE);
        // 10 letters of 12 pixels each plus padding of 7 on both sides: 134 wide.
        assert_eq!(c.pixel(20 + 133, 25), NAVY);
        assert_eq!(c.pixel(20 + 134, 25), WHITE);
    }

    #[test]
    fn text_keeps_its_own_colours_and_a_stated_width_wraps_the_lines() {
        let mut c = white_canvas(300, 200);
        let mut a = mark(AnnotationKind::Text, 10, 10);
        a.text = Some("one two three four five six".into());
        a.font_size = Some(20);
        a.width = Some(100);
        a.fill = Some("#FFFF00".into());
        a.stroke = Some("#FF0000".into());
        let report = draw(&mut c, a);
        assert_eq!(report, DrawReport::default());
        assert_eq!(c.pixel(10, 10), Rgb(255, 255, 0));
        assert_ne!(count(&c, Rgb(255, 0, 0)), 0, "letters in the chosen colour");
        assert_eq!(c.pixel(110, 12), WHITE, "the box is 100 wide");
        let mut tall = 0;
        for y in 10..200 {
            if c.pixel(12, y) == Rgb(255, 255, 0) {
                tall += 1;
            }
        }
        assert!(tall > 48, "several lines, so the box is tall: {tall}");
    }

    #[test]
    fn text_that_does_not_fit_is_reported_by_mark() {
        let mut c = white_canvas(300, 100);
        let mut a = mark(AnnotationKind::Text, 10, 10);
        a.id = "a7".into();
        a.text = Some("supercalifragilisticexpialidocious".into());
        a.font_size = Some(20);
        a.width = Some(60);
        let report = draw(&mut c, a);
        assert_eq!(report.text_cut_off, ["a7"]);
        assert!(report.text_errors.is_empty());
    }

    #[test]
    fn an_automatic_text_box_never_runs_off_the_right_edge() {
        let mut c = white_canvas(100, 60);
        let mut a = mark(AnnotationKind::Text, 50, 10);
        a.text = Some("a very long piece of text that is wider than the picture".into());
        a.font_size = Some(10);
        draw(&mut c, a);
        assert_eq!(c.pixel(99, 12), NAVY);
        assert_eq!(c.pixel(49, 12), WHITE);
    }

    #[test]
    fn a_number_badge_is_a_filled_circle_with_its_number_in_a_contrasting_colour() {
        let mut c = white_canvas(100, 100);
        let mut a = mark(AnnotationKind::Number, 50, 50);
        a.number = Some(7);
        a.size = Some(40);
        draw(&mut c, a);
        assert_eq!(c.pixel(32, 50), ACCENT, "inside the circle, away from the number");
        assert_eq!(c.pixel(31, 31), WHITE, "the corner of the badge's box is outside the circle");
        assert_ne!(count(&c, NAVY), 0, "navy letters on indigo");
        assert_eq!(c.pixel(75, 50), WHITE);
        // a dark badge gets white letters
        let mut c = white_canvas(100, 100);
        let mut a = mark(AnnotationKind::Number, 50, 50);
        a.number = Some(12);
        a.size = Some(40);
        a.fill = Some("#000033".into());
        draw(&mut c, a);
        assert_eq!(c.pixel(32, 50), Rgb(0, 0, 0x33));
        let letters =
            (35..65).flat_map(|x| (35..65).map(move |y| (x, y))).filter(|&(x, y)| c.pixel(x, y) == WHITE).count();
        assert!(letters > 20, "white letters on the dark badge: {letters}");
    }

    #[test]
    fn a_number_badge_without_its_number_draws_nothing() {
        let mut c = white_canvas(40, 40);
        let before = c.clone();
        draw(&mut c, mark(AnnotationKind::Number, 20, 20));
        assert_eq!(c, before);
    }

    #[test]
    fn the_default_click_mark_is_a_translucent_disc_with_a_solid_ring() {
        let mut c = white_canvas(120, 120);
        let mut a = mark(AnnotationKind::ClickMark, 60, 60);
        a.size = Some(60);
        a.stroke_width = Some(4);
        draw(&mut c, a);
        assert_eq!(c.pixel(60, 60), Rgb(mix(255, 0x7D, 0x33), mix(255, 0x87, 0x33), mix(255, 0xF2, 0x33)));
        assert_eq!(c.pixel(31, 60), ACCENT, "on the ring");
        assert_eq!(c.pixel(60, 31), ACCENT);
        assert_eq!(c.pixel(28, 60), WHITE, "outside the ring");
        assert_eq!(c.pixel(32, 32), WHITE, "outside the circle in the box's corner");
    }

    #[test]
    fn a_click_mark_without_a_size_draws_nothing() {
        let mut c = white_canvas(40, 40);
        let before = c.clone();
        draw(&mut c, mark(AnnotationKind::ClickMark, 20, 20));
        assert_eq!(c, before);
    }

    #[test]
    fn the_pointer_has_its_tip_at_the_hot_spot_and_is_white_with_a_dark_edge() {
        let mut c = white_canvas(120, 120);
        c.clear(Rgb(0x30, 0x60, 0x90));
        let before = c.clone();
        let mut a = mark(AnnotationKind::Cursor, 40, 30);
        a.style = Some("pointer".into());
        a.size = Some(38);
        draw(&mut c, a);
        assert_eq!(c.pixel(40 + 4, 30 + 14), WHITE, "inside the arrow");
        assert_eq!(c.pixel(20, 20), before.pixel(20, 20), "nothing to the upper left of the tip");
        assert_eq!(c.pixel(40, 30 + 38), before.pixel(40, 30 + 38), "nothing below it");
        assert_ne!(count(&c, BLACK), 0, "a dark edge");
        let touched = changed(&c, &before);
        assert!(touched > 200 && touched < 1500, "{touched}");
        // the arrow is about 38 pixels tall from its tip, with a pixel or two of edge
        let rows: Vec<u32> = (0..120).filter(|&y| (0..120).any(|x| c.pixel(x, y) != before.pixel(x, y))).collect();
        let (first, last) = (rows[0], *rows.last().unwrap());
        assert!((27..=30).contains(&first) && (66..=70).contains(&last), "{first} to {last}");
    }

    #[test]
    fn the_hand_and_the_text_cursor_draw_something_and_stay_near_the_hot_spot() {
        for style in ["hand", "text", "macHand", "pointer", "no such style"] {
            let mut c = white_canvas(120, 120);
            c.clear(Rgb(0x30, 0x60, 0x90));
            let before = c.clone();
            let mut a = mark(AnnotationKind::Cursor, 60, 40);
            a.style = Some(style.into());
            a.size = Some(44);
            draw(&mut c, a);
            let touched = changed(&c, &before);
            assert!(touched > 100, "{style}: only {touched} pixels");
            let xs: Vec<u32> = (0..120).filter(|&x| (0..120).any(|y| c.pixel(x, y) != before.pixel(x, y))).collect();
            let ys: Vec<u32> = (0..120).filter(|&y| (0..120).any(|x| c.pixel(x, y) != before.pixel(x, y))).collect();
            assert!(*ys.last().unwrap() - ys[0] <= 48 && *xs.last().unwrap() - xs[0] <= 40, "{style}: too big");
            assert!(ys[0] >= 36, "{style}: it starts at the hot spot, not above it");
        }
    }

    #[test]
    fn later_marks_are_drawn_over_earlier_ones() {
        let mut c = white_canvas(60, 60);
        let mut red = boxed(AnnotationKind::Rect, 10, 10, 30, 30);
        red.id = "a1".into();
        red.fill = Some("#FF0000".into());
        let mut blue = boxed(AnnotationKind::Rect, 25, 25, 30, 30);
        blue.id = "a2".into();
        blue.fill = Some("#0000FF".into());
        draw_annotations(&mut c, &[red.clone(), blue.clone()], &BlockText);
        assert_eq!(c.pixel(30, 30), Rgb(0, 0, 255), "blue was last");
        let mut d = white_canvas(60, 60);
        draw_annotations(&mut d, &[blue, red], &BlockText);
        assert_eq!(d.pixel(30, 30), Rgb(255, 0, 0));
    }

    #[test]
    fn marks_partly_or_wholly_off_the_picture_are_cut_off_not_a_crash() {
        let mut c = white_canvas(50, 50);
        let before = c.clone();
        for kind in [
            AnnotationKind::Rect,
            AnnotationKind::Oval,
            AnnotationKind::Highlight,
            AnnotationKind::Blur,
            AnnotationKind::Spotlight,
            AnnotationKind::Text,
        ] {
            for (x, y) in [(-500, -500), (500, 500), (-20, 30), (30, -20), (45, 45), (i32::MAX / 4, 0)] {
                let mut a = boxed(kind, x, y, 40, 40);
                a.text = Some("hello".into());
                draw(&mut c, a);
            }
        }
        for (x, y) in [(-500, 10), (500, 500), (25, -9), (0, 0)] {
            let mut a = mark(AnnotationKind::Arrow, x, y);
            a.x2 = Some(10_000);
            a.y2 = Some(-10_000);
            draw(&mut c, a);
            let mut b = mark(AnnotationKind::ClickMark, x, y);
            b.size = Some(400);
            draw(&mut c, b);
            let mut d = mark(AnnotationKind::Cursor, x, y);
            d.size = Some(60);
            draw(&mut c, d);
            let mut e = mark(AnnotationKind::Number, x, y);
            e.number = Some(3);
            draw(&mut c, e);
        }
        assert_eq!((c.width, c.height), (50, 50));
        assert_ne!(c, before, "something was drawn where it overlapped");
    }

    #[test]
    fn marks_with_no_size_draw_nothing() {
        let mut c = white_canvas(40, 40);
        let before = c.clone();
        for kind in [
            AnnotationKind::Rect,
            AnnotationKind::Oval,
            AnnotationKind::Highlight,
            AnnotationKind::Blur,
            AnnotationKind::Spotlight,
        ] {
            draw(&mut c, mark(kind, 5, 5));
            draw(&mut c, boxed(kind, 5, 5, 0, 10));
            draw(&mut c, boxed(kind, 5, 5, 10, -3));
        }
        assert_eq!(c, before);
    }

    #[test]
    fn drawing_is_exactly_repeatable() {
        let build = || {
            let mut c = white_canvas(200, 120);
            let mut marks = Vec::new();
            let mut r = boxed(AnnotationKind::Rect, 10, 10, 80, 50);
            r.stroke = Some("#FF0000".into());
            r.fill = Some("#00FF0055".into());
            marks.push(r);
            let mut o = boxed(AnnotationKind::Oval, 60, 30, 100, 60);
            o.fill = Some("#0000FF66".into());
            marks.push(o);
            let mut ar = mark(AnnotationKind::Arrow, 5, 110);
            ar.x2 = Some(190);
            ar.y2 = Some(20);
            marks.push(ar);
            let mut cur = mark(AnnotationKind::Cursor, 100, 60);
            cur.size = Some(40);
            marks.push(cur);
            marks.push(boxed(AnnotationKind::Blur, 120, 70, 60, 40));
            draw_annotations(&mut c, &marks, &BlockText);
            c
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn the_picture_is_cropped_after_the_marks_are_drawn_so_a_mark_over_the_edge_stays_in_place() {
        let picture = Picture::solid(200, 100, Rgb(200, 200, 200));
        let mut step = Step::bare("step-1", crate::model::StepKind::Image, "x");
        let mut r = boxed(AnnotationKind::Rect, 30, 20, 100, 50);
        r.stroke = Some("#FF0000".into());
        r.stroke_width = Some(4);
        step.annotations.push(r);
        step.focus = Some(Rect::new(80, 10, 100, 60));
        let rendered = render_step(&picture, &step, &BlockText);
        assert_eq!((rendered.canvas.width, rendered.canvas.height), (100, 60));
        let red = Rgb(255, 0, 0);
        // the rectangle's top edge (y 20..23) crosses the crop; its right edge is at x 126..129, i.e. 46..49 in the crop
        assert_eq!(rendered.canvas.pixel(30, 10), red, "top edge, at x 110 in the picture");
        assert_eq!(rendered.canvas.pixel(47, 30), red, "right edge");
        assert_eq!(rendered.canvas.pixel(0, 10), red, "the top edge continues to the crop's left side");
        assert_eq!(rendered.canvas.pixel(30, 30), Rgb(200, 200, 200), "inside the rectangle");
        assert_eq!(rendered.canvas.pixel(80, 30), Rgb(200, 200, 200), "outside it");
        assert_eq!(picture.bgra[0..4], [200, 200, 200, 255], "the original picture is not touched");
    }

    #[test]
    fn a_step_with_no_focus_renders_the_whole_picture() {
        let picture = Picture::solid(64, 48, Rgb(10, 20, 30));
        let step = Step::bare("step-1", crate::model::StepKind::Image, "x");
        let rendered = render_step(&picture, &step, &BlockText);
        assert_eq!((rendered.canvas.width, rendered.canvas.height), (64, 48));
        assert_eq!(rendered.canvas.pixel(63, 47), Rgb(10, 20, 30));
        assert_eq!(rendered.report, DrawReport::default());
    }

    #[test]
    fn a_focus_that_hangs_over_the_edge_is_moved_inside() {
        let picture = Picture::solid(64, 48, Rgb(10, 20, 30));
        let mut step = Step::bare("step-1", crate::model::StepKind::Image, "x");
        step.focus = Some(Rect::new(50, 40, 30, 30));
        let rendered = render_step(&picture, &step, &BlockText);
        assert_eq!((rendered.canvas.width, rendered.canvas.height), (30, 30));
    }

    #[test]
    fn scaling_to_a_width_keeps_the_shape() {
        let mut c = white_canvas(200, 100);
        c.fill_rect(PixelRect::new(0, 0, 100, 100), Rgb(255, 0, 0));
        let small = scale_to_width(&c, 100);
        assert_eq!((small.width, small.height), (100, 50));
        assert_eq!(small.pixel(10, 10), Rgb(255, 0, 0));
        assert_eq!(small.pixel(90, 10), WHITE);
        let big = scale_to_width(&c, 400);
        assert_eq!((big.width, big.height), (400, 200));
        assert_eq!(scale_to_width(&c, 200), c);
        assert_eq!(scale_to_width(&c, 0), c);
        let thin = scale_to_width(&Canvas::new(1000, 1), 10);
        assert_eq!((thin.width, thin.height), (10, 1));
        assert_eq!(crate::focus::height_at_width(&Rect::new(0, 0, 200, 100), 100), small.height);
    }
}
