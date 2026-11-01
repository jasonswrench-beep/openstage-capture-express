// SPDX-License-Identifier: Apache-2.0
//! Drawing the area to capture (spec C3). The window shows a picture of the display, with the rectangle on it: drag inside the
//! rectangle to move it, drag a square handle to change its size, drag on the picture outside it to draw a new one. Everything the
//! mouse does can also be done with the four number boxes and the buttons, so nothing needs the mouse or sight.

use eframe::egui::{self, RichText};
use openstage_express_core::area::{AreaEditor, AreaRect};
use openstage_express_core::capture::Picture;
use openstage_express_core::marks;

use super::capture::DisplayRow;
use super::widgets::{self, palette};

/// What the screen holds between frames.
pub struct AreaScreen {
    pub displays: Vec<DisplayRow>,
    /// The display the picture is of (1 is the main display).
    pub display: u32,
    pub editor: AreaEditor,
    picture: Option<Picture>,
    texture: Option<(u32, egui::TextureHandle)>,
    /// What is typed in the four number boxes: left, top, width, height.
    pub fields: [String; 4],
    /// The rectangle the boxes were last filled from, to fill them again when it has changed by other means.
    fields_for: Option<AreaRect>,
    /// Why the typed numbers were not used, in words.
    pub problem: Option<String>,
    /// A press on the picture is being dragged.
    dragging: bool,
}

impl AreaScreen {
    /// The screen for `display`, whose picture is `picture`, starting from `initial`.
    pub fn new(displays: Vec<DisplayRow>, display: u32, picture: Picture, initial: Option<AreaRect>) -> AreaScreen {
        let editor = AreaEditor::new(picture.width, picture.height, initial);
        AreaScreen {
            displays,
            display,
            editor,
            picture: Some(picture),
            texture: None,
            fields: Default::default(),
            fields_for: None,
            problem: None,
            dragging: false,
        }
    }

    /// Show another display's picture, keeping the rectangle if it fits.
    pub fn show_display(&mut self, display: u32, picture: Picture) {
        let keep = self.editor.rect();
        self.display = display;
        self.editor = AreaEditor::new(picture.width, picture.height, Some(keep));
        self.picture = Some(picture);
        self.texture = None;
        self.fields_for = None;
    }

    fn texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if let Some((n, t)) = &self.texture {
            if *n == self.display {
                return Some(t.clone());
            }
        }
        let picture = self.picture.as_ref()?;
        let rgba: Vec<u8> = picture.bgra.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], 255]).collect();
        let size = [picture.width as usize, picture.height as usize];
        let t = ctx.load_texture(
            "area-display",
            egui::ColorImage::from_rgba_unmultiplied(size, &rgba),
            egui::TextureOptions::LINEAR,
        );
        self.texture = Some((self.display, t.clone()));
        Some(t)
    }

    fn refill_fields(&mut self) {
        let r = self.editor.rect();
        self.fields = [r.x.to_string(), r.y.to_string(), r.width.to_string(), r.height.to_string()];
        self.fields_for = Some(r);
    }
}

/// What the person asked for on this screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaAction {
    None,
    Back,
    /// Use the rectangle as it is.
    Use,
    /// Show another display (by its number).
    ChangeDisplay(u32),
}

pub fn show(ui: &mut egui::Ui, state: &mut AreaScreen) -> AreaAction {
    let mut action = AreaAction::None;
    egui::Panel::top("area-top").show(ui, |ui| {
        ui.horizontal(|ui| {
            if widgets::button(ui, "Back").clicked() {
                action = AreaAction::Back;
            }
            widgets::heading(ui, "Draw the area to capture");
        });
    });
    egui::Panel::right("area-controls").resizable(false).exact_size(330.0).show(ui, |ui| {
        let p = palette(ui);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.label(RichText::new(format!("The area is {}.", state.editor.words())).color(p.text));
            if state.displays.len() > 1 {
                widgets::subheading(ui, "Display");
                for row in state.displays.clone() {
                    let r = ui.add(egui::RadioButton::new(state.display == row.number, row.words.clone()));
                    widgets::ring(ui, &r);
                    if r.clicked() && state.display != row.number {
                        action = AreaAction::ChangeDisplay(row.number);
                    }
                }
            }
            widgets::subheading(ui, "Numbers");
            if state.fields_for != Some(state.editor.rect()) && !ui.ctx().text_edit_focused() {
                state.refill_fields();
            }
            let mut typed: Option<usize> = None;
            for (i, name) in
                ["Left, in pixels", "Top, in pixels", "Width, in pixels", "Height, in pixels"].into_iter().enumerate()
            {
                let label = ui.label(name);
                let field =
                    ui.add(egui::TextEdit::singleline(&mut state.fields[i]).desired_width(120.0)).labelled_by(label.id);
                widgets::ring(ui, &field);
                if field.lost_focus() {
                    typed = Some(i);
                }
            }
            if let Some(i) = typed {
                // Only the box that was typed in is used, so the four can be filled in in any order.
                let r = state.editor.rect();
                let now = [r.x, r.y, r.width, r.height];
                match state.fields[i].trim().parse::<u32>() {
                    // The box still says what the area already is (the box was filled in again a moment ago): nothing to do, and a
                    // message about the last try stays.
                    Ok(n) if n == now[i] => {}
                    Ok(n) => {
                        let done = match i {
                            0 => state.editor.set_left(n),
                            1 => state.editor.set_top(n),
                            2 => state.editor.set_width(n),
                            _ => state.editor.set_height(n),
                        };
                        state.problem = done.err();
                    }
                    Err(_) => state.problem = Some("Each box needs a whole number of pixels, such as 640.".to_string()),
                }
                state.refill_fields();
            }
            if let Some(problem) = &state.problem {
                ui.label(RichText::new(problem).color(p.text));
            }
            widgets::subheading(ui, "Move it");
            ui.horizontal_wrapped(|ui| {
                for (words, dx, dy) in [("Left", -10, 0), ("Right", 10, 0), ("Up", 0, -10), ("Down", 0, 10)] {
                    if widgets::button(ui, &format!("Move {}", words.to_lowercase())).clicked() {
                        state.editor.nudge(dx, dy);
                    }
                }
            });
            widgets::subheading(ui, "Change its size");
            ui.horizontal_wrapped(|ui| {
                for (words, dw, dh) in [("Wider", 10, 0), ("Narrower", -10, 0), ("Taller", 0, 10), ("Shorter", 0, -10)]
                {
                    if widgets::button(ui, words).clicked() {
                        state.editor.grow(dw, dh);
                    }
                }
            });
            if widgets::button(ui, "Use the whole display").clicked() {
                state.editor.use_whole_display();
            }
            ui.add_space(8.0);
            if widgets::primary_button(ui, "Use this area").clicked() {
                action = AreaAction::Use;
            }
        });
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let p = palette(ui);
        let Some(texture) = state.texture(ui.ctx()) else {
            ui.label(RichText::new("There is no picture of the display.").color(p.muted));
            return;
        };
        let available = ui.available_size();
        let natural = texture.size_vec2();
        let scale = (available.x / natural.x).min(available.y / natural.y).clamp(0.05, 1.0);
        let image_rect = ui
            .vertical_centered(|ui| {
                ui.add(
                    egui::Image::from_texture((texture.id(), natural * scale))
                        .alt_text(format!("A picture of the display. The area is {}.", state.editor.words())),
                )
                .rect
            })
            .inner;
        let painter = ui.painter_at(image_rect);
        let r = state.editor.rect();
        let at = |x: i32, y: i32| image_rect.min + egui::vec2(x as f32 * scale, y as f32 * scale);
        let (tl, br) = (at(r.x as i32, r.y as i32), at((r.x + r.width) as i32, (r.y + r.height) as i32));
        let dim = egui::Color32::from_black_alpha(130);
        painter.rect_filled(egui::Rect::from_min_max(image_rect.min, egui::pos2(image_rect.max.x, tl.y)), 0.0, dim);
        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(image_rect.min.x, br.y), image_rect.max), 0.0, dim);
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(image_rect.min.x, tl.y), egui::pos2(tl.x, br.y)),
            0.0,
            dim,
        );
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(br.x, tl.y), egui::pos2(image_rect.max.x, br.y)),
            0.0,
            dim,
        );
        painter.rect_stroke(
            egui::Rect::from_min_max(tl, br),
            0.0,
            egui::Stroke::new(2.0, p.focus),
            egui::StrokeKind::Inside,
        );
        for (hx, hy) in state.editor.handle_points() {
            let square = egui::Rect::from_center_size(at(hx, hy), egui::vec2(10.0, 10.0));
            painter.rect_filled(square, 1.0, egui::Color32::WHITE);
            painter.rect_stroke(square, 1.0, egui::Stroke::new(2.0, p.focus), egui::StrokeKind::Inside);
        }
        // The mouse: press to pick the rectangle, a handle or empty picture, move to drag, let go to keep.
        let (pos, pressed, down, released) = ui.input(|i| {
            (
                i.pointer.interact_pos(),
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.primary_released(),
            )
        });
        let to_picture = |p: egui::Pos2| {
            let v = (p - image_rect.min) / scale;
            (v.x.round() as i32, v.y.round() as i32)
        };
        if let Some(pos) = pos {
            if pressed && image_rect.contains(pos) {
                let (x, y) = to_picture(pos);
                state.editor.press(x, y);
                state.dragging = true;
            } else if down && state.dragging {
                let (x, y) = to_picture(pos);
                state.editor.drag_to(x, y);
            }
        }
        if released && state.dragging {
            state.editor.release();
            state.dragging = false;
        }
        let _ = marks::GRAB;
    });
    action
}
