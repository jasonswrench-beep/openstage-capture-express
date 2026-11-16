// SPDX-License-Identifier: Apache-2.0
//! The watermark (spec 10.2): a logo or any other picture laid over every exported picture, in one of nine places, with an opacity, a
//! size and some room from the edge. It is drawn into the pixels like the marks and the blur, so it is in every format that holds the
//! step pictures (the web page, Markdown, JSON, the pictures, the GIF, the PDF, Word, PowerPoint and the books for OpenExplainer and
//! Author) and cannot be taken off afterwards. It is not put on a one-file copy of a guide, which is the guide itself, not an export.
//!
//! A picture with see-through parts (a logo on a clear background) is laid over the step picture by how solid each pixel is; the
//! opacity makes every pixel less solid by the same share. Every step is the same arithmetic on whole numbers, so the same guide gives
//! the same pixels on every computer.

use openstage_capture_render::{Canvas, Picture, PixelRect, SourceRect};

/// The shortest width a watermark is drawn at, in pixels: a smaller one is a smudge, not a mark.
pub const MIN_WIDTH_PX: u32 = 16;

/// Where on the picture the watermark sits: three rows by three columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Place {
    TopLeft,
    TopCentre,
    TopRight,
    MiddleLeft,
    Centre,
    MiddleRight,
    BottomLeft,
    BottomCentre,
    #[default]
    BottomRight,
}

impl Place {
    /// All nine, in reading order (row by row from the top left).
    pub const ALL: [Place; 9] = [
        Place::TopLeft,
        Place::TopCentre,
        Place::TopRight,
        Place::MiddleLeft,
        Place::Centre,
        Place::MiddleRight,
        Place::BottomLeft,
        Place::BottomCentre,
        Place::BottomRight,
    ];

    /// 0 for the left column, 1 for the middle and 2 for the right.
    fn column(self) -> i64 {
        Place::ALL.iter().position(|p| *p == self).unwrap_or(8) as i64 % 3
    }

    /// 0 for the top row, 1 for the middle and 2 for the bottom.
    fn row(self) -> i64 {
        Place::ALL.iter().position(|p| *p == self).unwrap_or(8) as i64 / 3
    }
}

/// A watermark: the picture and how it is laid over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watermark {
    pub picture: Picture,
    pub place: Place,
    /// How solid the picture is made, from 1 to 100 percent; a number outside that is moved to the nearest end.
    pub opacity_percent: u32,
    /// How wide the watermark is, as a share of the width of the picture it goes on: 1 to 100 percent.
    pub size_percent: u32,
    /// The room between the watermark and the edge of the picture, as a share of the picture's width: 0 to 25 percent.
    pub padding_percent: u32,
}

impl Watermark {
    /// The watermark's own copy at `opacity_percent`: every pixel's solidity scaled, nothing else changed.
    fn faded(&self) -> Picture {
        let opacity = self.opacity_percent.clamp(1, 100);
        if opacity == 100 {
            return self.picture.clone();
        }
        let mut bgra = self.picture.bgra.clone();
        for pixel in bgra.as_chunks_mut::<4>().0 {
            pixel[3] = ((pixel[3] as u32 * opacity + 50) / 100) as u8;
        }
        Picture::new(self.picture.width, self.picture.height, bgra)
    }

    /// The rectangle the watermark takes on a picture of this size, or `None` when there is no room for it or no picture.
    pub fn rectangle(&self, width: u32, height: u32) -> Option<PixelRect> {
        let (pw, ph) = (self.picture.width as i64, self.picture.height as i64);
        if pw == 0 || ph == 0 || width == 0 || height == 0 {
            return None;
        }
        let (cw, ch) = (width as i64, height as i64);
        let padding = cw * self.padding_percent.min(25) as i64 / 100;
        let room_w = cw - 2 * padding;
        let room_h = ch - 2 * padding;
        if room_w < 1 || room_h < 1 {
            return None;
        }
        // The wanted width, then the height that keeps the shape; if that is too tall for the room, the room's height decides.
        let wanted = (cw * self.size_percent.clamp(1, 100) as i64 / 100).max(MIN_WIDTH_PX as i64);
        let mut w = wanted.min(room_w);
        let mut h = ((ph * w + pw / 2) / pw).max(1);
        if h > room_h {
            h = room_h;
            w = ((pw * h + ph / 2) / ph).max(1);
        }
        let x = padding + (room_w - w) * self.place.column() / 2;
        let y = padding + (room_h - h) * self.place.row() / 2;
        Some(PixelRect::new(x as i32, y as i32, w as i32, h as i32))
    }

    /// Lay the watermark over `canvas`.
    pub fn apply(&self, canvas: &mut Canvas) {
        let Some(target) = self.rectangle(canvas.width, canvas.height) else { return };
        let picture = self.faded();
        let source = SourceRect { x: 0.0, y: 0.0, w: picture.width as f64, h: picture.height as f64 };
        canvas.draw_picture(&picture, source, target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstage_capture_render::Rgb;

    /// A canvas of one colour.
    fn canvas(w: u32, h: u32, colour: Rgb) -> Canvas {
        let mut c = Canvas::new(w, h);
        c.clear(colour);
        c
    }

    /// A solid picture of one colour.
    fn solid(w: u32, h: u32, colour: Rgb) -> Picture {
        Picture::solid(w, h, colour)
    }

    fn mark(picture: Picture, place: Place) -> Watermark {
        Watermark { picture, place, opacity_percent: 100, size_percent: 20, padding_percent: 2 }
    }

    #[test]
    fn the_nine_places_are_three_rows_of_three_in_reading_order() {
        let got: Vec<(i64, i64)> = Place::ALL.iter().map(|p| (p.row(), p.column())).collect();
        assert_eq!(got, [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2), (2, 0), (2, 1), (2, 2)]);
        assert_eq!(Place::default(), Place::BottomRight);
    }

    #[test]
    fn each_place_puts_the_mark_in_its_corner_edge_or_the_middle() {
        // 1000 by 500, a 100 by 50 mark (10 percent wide), room of 20 pixels (2 percent of the width) round the edge.
        let w = Watermark { size_percent: 10, ..mark(solid(200, 100, Rgb(0, 0, 0)), Place::TopLeft) };
        let at = |place: Place| Watermark { place, ..w.clone() }.rectangle(1000, 500).unwrap();
        assert_eq!(at(Place::TopLeft), PixelRect::new(20, 20, 100, 50));
        assert_eq!(at(Place::TopCentre), PixelRect::new(450, 20, 100, 50));
        assert_eq!(at(Place::TopRight), PixelRect::new(880, 20, 100, 50));
        assert_eq!(at(Place::MiddleLeft), PixelRect::new(20, 225, 100, 50));
        assert_eq!(at(Place::Centre), PixelRect::new(450, 225, 100, 50));
        assert_eq!(at(Place::MiddleRight), PixelRect::new(880, 225, 100, 50));
        assert_eq!(at(Place::BottomLeft), PixelRect::new(20, 430, 100, 50));
        assert_eq!(at(Place::BottomCentre), PixelRect::new(450, 430, 100, 50));
        assert_eq!(at(Place::BottomRight), PixelRect::new(880, 430, 100, 50));
    }

    #[test]
    fn the_size_is_a_share_of_the_pictures_width_and_keeps_the_marks_shape() {
        let w = mark(solid(300, 100, Rgb(0, 0, 0)), Place::TopLeft);
        let r = Watermark { size_percent: 30, ..w.clone() }.rectangle(1280, 720).unwrap();
        assert_eq!((r.w, r.h), (384, 128));
        let small = Watermark { size_percent: 1, ..w.clone() }.rectangle(640, 360).unwrap();
        assert_eq!(small.w, MIN_WIDTH_PX as i32, "never smaller than a mark can be read");
        // A tall mark that would not fit in the height is made to fit, keeping its shape.
        let tall =
            Watermark { size_percent: 100, padding_percent: 0, ..mark(solid(10, 100, Rgb(0, 0, 0)), Place::TopLeft) };
        let r = tall.rectangle(400, 200).unwrap();
        assert_eq!((r.w, r.h), (20, 200));
    }

    #[test]
    fn padding_is_a_share_of_the_width_and_is_kept_to_a_quarter() {
        let w =
            Watermark { padding_percent: 10, size_percent: 10, ..mark(solid(10, 10, Rgb(0, 0, 0)), Place::TopLeft) };
        assert_eq!(w.rectangle(1000, 500).unwrap().x, 100);
        let wild = Watermark { padding_percent: 90, ..w.clone() };
        assert_eq!(wild.rectangle(1000, 1000).unwrap().x, 250, "kept to 25 percent");
        assert!(wild.rectangle(1000, 500).is_none(), "and a picture too short for that much room has no room");
        let none = Watermark { padding_percent: 0, ..w };
        assert_eq!(none.rectangle(1000, 500).unwrap().x, 0);
    }

    #[test]
    fn a_picture_with_no_room_or_a_mark_with_nothing_in_it_draws_nothing() {
        let w = mark(solid(10, 10, Rgb(0, 0, 0)), Place::Centre);
        assert!(w.rectangle(0, 100).is_none());
        let crowded = Watermark { padding_percent: 25, ..w.clone() };
        assert!(crowded.rectangle(100, 1).is_none(), "padding takes all the height");
        let empty = Watermark { picture: Picture::new(0, 0, vec![]), ..w };
        let mut c = canvas(50, 50, Rgb(9, 9, 9));
        let before = c.clone();
        empty.apply(&mut c);
        assert_eq!(c, before);
    }

    #[test]
    fn a_solid_mark_replaces_the_pixels_under_it_and_leaves_the_rest_alone() {
        let w =
            Watermark { size_percent: 50, padding_percent: 0, ..mark(solid(10, 10, Rgb(200, 0, 0)), Place::TopLeft) };
        let mut c = canvas(100, 100, Rgb(255, 255, 255));
        w.apply(&mut c);
        assert_eq!(c.pixel(0, 0), Rgb(200, 0, 0));
        assert_eq!(c.pixel(49, 49), Rgb(200, 0, 0));
        assert_eq!(c.pixel(50, 50), Rgb(255, 255, 255), "outside the mark");
        assert_eq!(c.pixel(99, 99), Rgb(255, 255, 255));
    }

    #[test]
    fn opacity_lets_what_is_under_the_mark_show_through_by_that_share() {
        let base =
            Watermark { size_percent: 50, padding_percent: 0, ..mark(solid(10, 10, Rgb(0, 0, 0)), Place::TopLeft) };
        let at = |opacity: u32| {
            let mut c = canvas(100, 100, Rgb(200, 200, 200));
            Watermark { opacity_percent: opacity, ..base.clone() }.apply(&mut c);
            c.pixel(10, 10).0 as i32
        };
        assert_eq!(at(100), 0);
        let half = at(50);
        assert!((half - 100).abs() <= 2, "half way between black and 200: {half}");
        let quarter = at(25);
        assert!((quarter - 150).abs() <= 2, "a quarter of the way: {quarter}");
        assert!(at(1) > 190, "nearly clear");
        assert_eq!(at(0), at(1), "0 is taken as 1, so a mark is never invisible by accident");
        assert_eq!(at(500), at(100));
    }

    #[test]
    fn opacity_scales_each_pixels_solidity_exactly_and_changes_nothing_else() {
        let logo = Picture::new(1, 1, vec![10, 20, 30, 255]);
        let at = |opacity: u32| {
            let faded = Watermark { opacity_percent: opacity, ..mark(logo.clone(), Place::TopLeft) }.faded();
            faded.bgra
        };
        assert_eq!(at(100), [10, 20, 30, 255]);
        assert_eq!(at(50), [10, 20, 30, 128], "half of 255, rounded");
        assert_eq!(at(25), [10, 20, 30, 64]);
        assert_eq!(at(1), [10, 20, 30, 3]);
        let clear = Picture::new(1, 1, vec![1, 2, 3, 0]);
        assert_eq!(Watermark { opacity_percent: 50, ..mark(clear, Place::TopLeft) }.faded().bgra, [1, 2, 3, 0], "clear stays clear");
    }

    #[test]
    fn a_see_through_part_of_the_picture_lets_the_step_show_and_a_solid_part_covers_it() {
        // A 2 by 1 logo: the left pixel is clear, the right one solid blue (bgra).
        let logo = Picture::new(2, 1, vec![0, 0, 0, 0, 255, 0, 0, 255]);
        let w = Watermark { size_percent: 100, padding_percent: 0, ..mark(logo, Place::TopLeft) };
        let mut c = canvas(40, 20, Rgb(10, 200, 10));
        w.apply(&mut c);
        assert_eq!(c.pixel(2, 10), Rgb(10, 200, 10), "the clear half shows the picture");
        assert_eq!(c.pixel(38, 10), Rgb(0, 0, 255), "the solid half covers it");
    }

    #[test]
    fn the_same_mark_on_the_same_picture_is_the_same_pixels() {
        let w = mark(solid(30, 12, Rgb(1, 2, 3)), Place::BottomCentre);
        let (mut a, mut b) = (canvas(300, 150, Rgb(90, 90, 90)), canvas(300, 150, Rgb(90, 90, 90)));
        w.apply(&mut a);
        w.apply(&mut b);
        assert_eq!(a, b);
        assert_ne!(a, canvas(300, 150, Rgb(90, 90, 90)), "and it changed something");
    }
}
