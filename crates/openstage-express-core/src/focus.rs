// SPDX-License-Identifier: Apache-2.0
//! The focused view (spec section 9, E5): a step's picture is shown cropped and enlarged around the click, so every step
//! of a guide exports at the same width and the thing clicked is easy to see.
//!
//! A focused view is a stored rectangle (`focus` on the step), not an edited picture: the original is never changed and
//! the marks are drawn on the whole picture before it is cropped, so they stay in place. This file is the arithmetic:
//! working out the automatic crop around a click, moving and zooming a crop, and keeping every crop inside the picture
//! whatever a person (or a damaged file) asks for.

use crate::model::Rect;

/// How close the automatic crop looks, in percent: 150 means the crop is two thirds of the picture across.
pub const DEFAULT_ZOOM_PERCENT: u32 = 150;
/// The closest a crop may look (eight times).
pub const MAX_ZOOM_PERCENT: u32 = 800;

/// The smallest a crop may be, in pixels.
const MIN_SIDE: i32 = 8;

/// Make `rect` fit inside a picture `image_w` by `image_h`: no smaller than a few pixels, no larger than the picture,
/// and moved (not squashed) if it hangs over an edge. A picture too small for the minimum is taken whole.
pub fn clamp_inside(rect: &Rect, image_w: u32, image_h: u32) -> Rect {
    let (w, h) = (image_w as i32, image_h as i32);
    let width = rect.width.clamp(MIN_SIDE.min(w), w.max(1));
    let height = rect.height.clamp(MIN_SIDE.min(h), h.max(1));
    let x = rect.x.clamp(0, (w - width).max(0));
    let y = rect.y.clamp(0, (h - height).max(0));
    Rect::new(x, y, width, height)
}

/// The automatic crop for a step clicked at `click`: the picture's own shape, `zoom_percent` closer, centred on the click
/// and moved to stay inside the picture. None means show the whole picture (no click, or a zoom of 100 or less).
/// A click outside the picture is treated as being on its nearest edge.
pub fn auto_focus(image_w: u32, image_h: u32, click: Option<(i32, i32)>, zoom_percent: u32) -> Option<Rect> {
    let (cx, cy) = click?;
    if zoom_percent <= 100 || image_w == 0 || image_h == 0 {
        return None;
    }
    let zoom = zoom_percent.min(MAX_ZOOM_PERCENT) as i64;
    let width = ((image_w as i64 * 100 + zoom / 2) / zoom) as i32;
    let height = ((image_h as i64 * 100 + zoom / 2) / zoom) as i32;
    let wanted = Rect::new(cx.saturating_sub(width / 2), cy.saturating_sub(height / 2), width, height);
    Some(clamp_inside(&wanted, image_w, image_h))
}

/// Move a crop by `dx`, `dy` pixels (dragging with Space held), stopping at the edges of the picture.
pub fn pan(rect: &Rect, dx: i32, dy: i32, image_w: u32, image_h: u32) -> Rect {
    let moved = Rect::new(rect.x.saturating_add(dx), rect.y.saturating_add(dy), rect.width, rect.height);
    clamp_inside(&moved, image_w, image_h)
}

/// Make a crop look closer or further away, keeping its centre and its shape. `percent` of 200 shows half as much (twice
/// as close), 50 shows twice as much. The result is never larger than the picture and never closer than eight times.
pub fn zoom(rect: &Rect, percent: u32, image_w: u32, image_h: u32) -> Rect {
    let percent = percent.max(1) as i64;
    let (centre_x, centre_y) = (rect.x as i64 + rect.width as i64 / 2, rect.y as i64 + rect.height as i64 / 2);
    let mut width = (rect.width as i64 * 100 / percent).clamp(1, i32::MAX as i64);
    let mut height = (rect.height as i64 * 100 / percent).clamp(1, i32::MAX as i64);
    // No closer than eight times, in the shape of the picture's own sides.
    width = width.max(image_w as i64 * 100 / MAX_ZOOM_PERCENT as i64);
    height = height.max(image_h as i64 * 100 / MAX_ZOOM_PERCENT as i64);
    let wanted = Rect::new(
        (centre_x - width / 2).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        (centre_y - height / 2).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        width as i32,
        height as i32,
    );
    clamp_inside(&wanted, image_w, image_h)
}

/// How tall a crop is when it is scaled to `width` pixels across (rounded, at least one pixel).
pub fn height_at_width(rect: &Rect, width: u32) -> u32 {
    if rect.width <= 0 {
        return 1;
    }
    ((rect.height as u64 * width as u64 + rect.width as u64 / 2) / rect.width as u64).max(1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside(r: &Rect, w: u32, h: u32) -> bool {
        r.x >= 0 && r.y >= 0 && r.width >= 1 && r.height >= 1 && r.x + r.width <= w as i32 && r.y + r.height <= h as i32
    }

    #[test]
    fn the_automatic_crop_is_centred_on_the_click_and_has_the_pictures_shape() {
        let r = auto_focus(1920, 1080, Some((960, 540)), 150).unwrap();
        assert_eq!((r.width, r.height), (1280, 720));
        assert_eq!((r.x, r.y), (320, 180));
        let r = auto_focus(1920, 1080, Some((960, 540)), 200).unwrap();
        assert_eq!((r.x, r.y, r.width, r.height), (480, 270, 960, 540));
    }

    #[test]
    fn a_click_near_an_edge_moves_the_crop_instead_of_leaving_the_picture() {
        let r = auto_focus(1920, 1080, Some((10, 20)), 150).unwrap();
        assert_eq!((r.x, r.y), (0, 0));
        let r = auto_focus(1920, 1080, Some((1915, 1075)), 150).unwrap();
        assert_eq!((r.x + r.width, r.y + r.height), (1920, 1080));
    }

    #[test]
    fn no_click_or_no_zoom_means_the_whole_picture() {
        assert_eq!(auto_focus(1920, 1080, None, 150), None);
        assert_eq!(auto_focus(1920, 1080, Some((5, 5)), 100), None);
        assert_eq!(auto_focus(1920, 1080, Some((5, 5)), 0), None);
        assert_eq!(auto_focus(0, 0, Some((5, 5)), 150), None);
    }

    #[test]
    fn every_crop_stays_inside_the_picture_and_holds_a_click_that_is_inside() {
        let sizes = [(1920u32, 1080u32), (800, 600), (50, 40), (9, 9), (3, 3), (1, 1), (4000, 300)];
        let zooms = [101, 125, 150, 200, 300, 400, 799, 800, 801, 5000, u32::MAX];
        for (w, h) in sizes {
            let xs =
                [-100_000, -1, 0, 1, w as i32 / 2, w as i32 - 1, w as i32, w as i32 + 7, 100_000, i32::MAX, i32::MIN];
            for &zoom_percent in &zooms {
                for &x in &xs {
                    for &y in &xs {
                        let Some(r) = auto_focus(w, h, Some((x, y)), zoom_percent) else { continue };
                        assert!(inside(&r, w, h), "{w}x{h} click ({x},{y}) zoom {zoom_percent}: {r:?}");
                        let click_in_picture = x >= 0 && y >= 0 && x < w as i32 && y < h as i32;
                        if click_in_picture && w >= 8 && h >= 8 {
                            assert!(
                                x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height,
                                "click lost: {r:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn clamping_moves_and_shrinks_but_never_leaves_the_picture() {
        let r = clamp_inside(&Rect::new(-50, -50, 100, 100), 200, 100);
        assert_eq!((r.x, r.y, r.width, r.height), (0, 0, 100, 100));
        let r = clamp_inside(&Rect::new(150, 60, 100, 100), 200, 100);
        assert_eq!((r.x, r.y, r.width, r.height), (100, 0, 100, 100));
        let r = clamp_inside(&Rect::new(0, 0, 5000, 5000), 200, 100);
        assert_eq!((r.x, r.y, r.width, r.height), (0, 0, 200, 100));
        let r = clamp_inside(&Rect::new(0, 0, -5, 0), 200, 100);
        assert!(inside(&r, 200, 100));
        let r = clamp_inside(&Rect::new(i32::MAX, i32::MIN, i32::MAX, i32::MAX), 200, 100);
        assert!(inside(&r, 200, 100));
    }

    #[test]
    fn panning_stops_at_the_edges() {
        let start = Rect::new(100, 100, 400, 300);
        let moved = pan(&start, 50, -20, 1000, 800);
        assert_eq!((moved.x, moved.y, moved.width, moved.height), (150, 80, 400, 300));
        let stuck = pan(&start, -10_000, 10_000, 1000, 800);
        assert_eq!((stuck.x, stuck.y), (0, 500));
        let huge = pan(&start, i32::MAX, i32::MIN, 1000, 800);
        assert!(inside(&huge, 1000, 800));
    }

    #[test]
    fn zooming_keeps_the_centre_and_the_shape() {
        // A crop with the shape of a 1600 by 1000 picture, centred on (800, 487).
        let start = Rect::new(500, 300, 600, 375);
        let closer = zoom(&start, 200, 1600, 1000);
        assert_eq!((closer.width, closer.height), (300, 187));
        assert_eq!((closer.x + closer.width / 2, closer.y + closer.height / 2), (800, 487));
        let further = zoom(&start, 50, 1600, 1000);
        assert_eq!((further.width, further.height), (1200, 750));
        assert_eq!((further.x, further.y), (200, 112));
    }

    #[test]
    fn zooming_out_stops_at_the_whole_picture_and_in_at_eight_times() {
        let start = Rect::new(500, 300, 600, 375);
        let out = zoom(&start, 1, 1600, 1000);
        assert_eq!((out.x, out.y, out.width, out.height), (0, 0, 1600, 1000));
        let mut r = start.clone();
        for _ in 0..40 {
            r = zoom(&r, 200, 1600, 1000);
            assert!(inside(&r, 1600, 1000));
        }
        assert_eq!((r.width, r.height), (200, 125), "no closer than eight times");
        assert!(inside(&zoom(&start, 0, 1600, 1000), 1600, 1000), "a zero percent is treated as the smallest");
        assert!(inside(&zoom(&start, u32::MAX, 1600, 1000), 1600, 1000));
    }

    #[test]
    fn the_exported_height_follows_the_width() {
        let r = Rect::new(0, 0, 1280, 720);
        assert_eq!(height_at_width(&r, 800), 450);
        assert_eq!(height_at_width(&r, 1920), 1080);
        assert_eq!(height_at_width(&Rect::new(0, 0, 3, 1), 10), 3);
        assert_eq!(height_at_width(&Rect::new(0, 0, 1000, 1), 10), 1, "never zero");
        assert_eq!(height_at_width(&Rect::new(0, 0, 0, 5), 10), 1);
    }

    #[test]
    fn all_the_steps_of_one_screen_export_at_the_same_size() {
        let sizes: Vec<(u32, u32)> = [(10, 10), (1900, 1000), (960, 540), (300, 700)]
            .iter()
            .map(|&click| {
                let r = auto_focus(1920, 1080, Some(click), DEFAULT_ZOOM_PERCENT).unwrap();
                (800, height_at_width(&r, 800))
            })
            .collect();
        assert!(sizes.windows(2).all(|w| w[0] == w[1]), "{sizes:?}");
    }
}
