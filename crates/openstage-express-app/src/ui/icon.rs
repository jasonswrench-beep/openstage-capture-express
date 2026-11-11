// SPDX-License-Identifier: Apache-2.0
//! Drawing the suite's icons (Lucide, in `uiicons_gen.rs`): each is a few lines on a 24 by 24 grid, drawn 2 grid squares thick
//! with round ends, in the colour of the words beside it. An icon is never the only thing that says what a control does: a button
//! with an icon has words, or a name a screen reader says and a tooltip.

use eframe::egui::{self, epaint::PathShape, Atom, Color32, Pos2, Rect, Stroke};

pub use super::uiicons_gen::UiIcon;

/// Draw `icon` filling the square in the middle of `rect`, in `colour`.
pub fn paint(painter: &egui::Painter, rect: Rect, icon: UiIcon, colour: Color32) {
    let side = rect.width().min(rect.height());
    let scale = side / 24.0;
    let origin = rect.center() - egui::vec2(12.0 * scale, 12.0 * scale);
    let width = (2.0 * scale).max(1.25);
    let stroke = Stroke::new(width, colour);
    for line in icon.lines() {
        let points: Vec<Pos2> = line.points.iter().map(|[x, y]| origin + egui::vec2(x * scale, y * scale)).collect();
        let (Some(&first), Some(&last)) = (points.first(), points.last()) else { continue };
        if points.len() == 2 && first == last {
            painter.circle_filled(first, width / 2.0, colour);
            continue;
        }
        if line.closed {
            painter.add(PathShape::closed_line(points, stroke));
        } else {
            painter.add(PathShape::line(points, stroke));
            // Lucide's lines have round ends.
            painter.circle_filled(first, width / 2.0, colour);
            painter.circle_filled(last, width / 2.0, colour);
        }
    }
}

/// An icon as part of a button (`egui::Button::new((icon::atom(UiIcon::Undo2, 18.0), "Undo"))`): it takes the colour of the
/// button's words, so it follows the hovered, pressed and switched-off looks with them.
pub fn atom(icon: UiIcon, size: f32) -> Atom<'static> {
    Atom::paint(egui::Vec2::splat(size), move |ui, args| paint(ui.painter(), args.rect, icon, args.fallback_text_color))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_has_lines_on_its_grid() {
        for icon in UiIcon::ALL {
            assert!(!icon.lines().is_empty(), "{} has no lines", icon.name());
            for line in icon.lines() {
                assert!(line.points.len() >= 2, "{}: a line needs two points", icon.name());
                for [x, y] in line.points {
                    assert!(
                        (-1.0..=25.0).contains(x) && (-1.0..=25.0).contains(y),
                        "{} has ({x}, {y}) off its grid",
                        icon.name()
                    );
                }
            }
        }
    }

    #[test]
    fn the_icons_the_window_needs_are_there_by_their_lucide_names() {
        let names: Vec<&str> = UiIcon::ALL.iter().map(|i| i.name()).collect();
        for name in ["camera", "undo-2", "redo-2", "file-output", "search", "keyboard", "ellipsis", "chevron-down", "x"]
        {
            assert!(names.contains(&name), "{name} is missing");
        }
    }
}
