// SPDX-License-Identifier: Apache-2.0
//! The Editor screen (spec sections 4.2 and 9): the steps list on the left, the step's picture in the middle, its details on the
//! right. Every control has a name and a keyboard route, the picture has its words, and what happens is said in the status line.
//!
//! Keys: Ctrl+Z and Ctrl+Y undo and redo (while a text box has the keyboard they belong to the box); Alt+Up and Alt+Down move
//! the step; Ctrl+Shift+Up and Ctrl+Shift+Down choose the step before or after; Ctrl+Shift+Delete deletes the step; F2 goes to
//! the title; Up and Down in the list choose a step.

use eframe::egui::{self, Key, Modifiers, RichText};

use super::editor_state::EditorState;
use super::widgets::{self, palette, Status};

/// Whether the person left the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leave {
    Stay,
    ToHome,
    ToExport,
}

pub fn show(ui: &mut egui::Ui, state: &mut EditorState, status: &mut Status) -> Leave {
    let mut leave = Leave::Stay;
    keys(ui.ctx(), state, status);

    egui::Panel::top("editor-top").show(ui, |ui| {
        ui.horizontal(|ui| {
            if widgets::button(ui, "Back to guides").clicked() {
                state.commit_pending();
                leave = Leave::ToHome;
            }
            if widgets::button_enabled(ui, state.can_undo(), "Undo").clicked() {
                status.say_opt(state.undo());
            }
            if widgets::button_enabled(ui, state.can_redo(), "Redo").clicked() {
                status.say_opt(state.redo());
            }
            let has_steps = !state.doc().guide.steps.is_empty();
            if widgets::button_enabled(ui, has_steps, "Export").clicked() {
                state.commit_pending();
                leave = Leave::ToExport;
            }
            widgets::heading(ui, &state.doc().guide.title.clone());
        });
    });

    // The panels are a fixed share of the window, not draggable: a drag handle is a control with no keyboard route. The steps
    // come first, then the picture, then the details, which is the order a screen reader reads them in.
    let left = (ui.available_width() * 0.26).clamp(240.0, 340.0);
    egui::Panel::left("editor-steps").resizable(false).exact_size(left).show(ui, |ui| {
        steps_list(ui, state, status);
    });

    egui::CentralPanel::default().show(ui, |ui| {
        let area = ui.available_rect_before_wrap();
        let details_width = (area.width() * 0.42).clamp(280.0, 400.0).min(area.width() * 0.6);
        let picture_area =
            egui::Rect::from_min_max(area.min, egui::pos2(area.max.x - details_width - 10.0, area.max.y));
        let details_area = egui::Rect::from_min_max(egui::pos2(area.max.x - details_width, area.min.y), area.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(picture_area), |ui| picture(ui, state));
        let border = palette(ui).border;
        ui.painter().vline(details_area.min.x - 6.0, area.y_range(), egui::Stroke::new(1.0, border));
        ui.scope_builder(egui::UiBuilder::new().max_rect(details_area), |ui| details(ui, state, status));
    });
    leave
}

fn keys(ctx: &egui::Context, state: &mut EditorState, status: &mut Status) {
    let typing = ctx.text_edit_focused();
    let (undo, redo, up, down, previous, next, delete, rename) = ctx.input_mut(|i| {
        let undo = !typing && i.consume_key(Modifiers::COMMAND, Key::Z);
        let redo = !typing
            && (i.consume_key(Modifiers::COMMAND, Key::Y)
                || i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z));
        (
            undo,
            redo,
            i.consume_key(Modifiers::ALT, Key::ArrowUp),
            i.consume_key(Modifiers::ALT, Key::ArrowDown),
            i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::ArrowUp),
            i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::ArrowDown),
            i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Delete),
            i.consume_key(Modifiers::NONE, Key::F2),
        )
    });
    if undo {
        status.say_opt(state.undo());
    }
    if redo {
        status.say_opt(state.redo());
    }
    if up {
        status.say_opt(state.move_selected(-1));
    }
    if down {
        status.say_opt(state.move_selected(1));
    }
    if previous || next {
        if let Some(id) = state.neighbour(if previous { -1 } else { 1 }) {
            state.select(&id);
            status.say_opt(state.selected_row().map(|r| r.label()));
        }
    }
    if delete {
        status.say_opt(state.delete_selected());
    }
    if rename {
        state.focus_title_next = true;
    }
}

fn steps_list(ui: &mut egui::Ui, state: &mut EditorState, status: &mut Status) {
    widgets::subheading(ui, "Steps");
    let rows = state.rows();
    let selected = state.selected_id().map(str::to_string);
    let take_keyboard = std::mem::take(&mut state.focus_row_next);
    let mut chosen: Option<String> = None;
    let mut keyboard_on_selected = false;
    egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(ui.available_height() - 56.0).show(ui, |ui| {
        for row in &rows {
            let is_selected = selected.as_deref() == Some(row.id.as_str());
            // A steps list is a list of choices: each row is named for what it says, and the chosen one is marked.
            let response =
                widgets::list_row(ui, &row.label(), is_selected, row.skipped, row.depth as f32 * 22.0, take_keyboard);
            if response.clicked() && !is_selected {
                chosen = Some(row.id.clone());
                state.focus_row_next = true;
            }
            keyboard_on_selected |= is_selected && response.has_focus();
            if response.has_focus() {
                response.scroll_to_me(None);
            }
        }
    });
    // Up and Down choose the step above or below while the chosen row has the keyboard, and the keyboard goes with the choice.
    // A screen reader says the row it lands on, so nothing is added to the status line for this.
    if keyboard_on_selected {
        let (up, down) = ui.ctx().input_mut(|i| {
            (i.consume_key(Modifiers::NONE, Key::ArrowUp), i.consume_key(Modifiers::NONE, Key::ArrowDown))
        });
        let target = if up {
            state.neighbour(-1)
        } else if down {
            state.neighbour(1)
        } else {
            None
        };
        if let Some(id) = target {
            chosen = Some(id);
            state.focus_row_next = true;
        }
    }
    if let Some(id) = chosen {
        state.select(&id);
    }
    ui.add_space(4.0);
    if widgets::button(ui, "Add a step").clicked() {
        status.say_opt(state.add_step());
    }
}

fn details(ui: &mut egui::Ui, state: &mut EditorState, status: &mut Status) {
    widgets::subheading(ui, "Step details");
    let p = palette(ui);
    if state.selected_step().is_none() {
        ui.label(RichText::new("No step is chosen. Add a step, or choose one from the list.").color(p.muted));
        return;
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let label = ui.label("Title");
        let title = ui
            .add(egui::TextEdit::singleline(&mut state.buffer.title).desired_width(f32::INFINITY))
            .labelled_by(label.id);
        widgets::ring(ui, &title);
        if state.focus_title_next {
            title.request_focus();
            state.focus_title_next = false;
        }
        if title.lost_focus() {
            state.commit_pending();
        }

        let label = ui.label("Description");
        let description = ui
            .add(egui::TextEdit::multiline(&mut state.buffer.description).desired_rows(5).desired_width(f32::INFINITY))
            .labelled_by(label.id);
        widgets::ring(ui, &description);
        if description.lost_focus() {
            state.commit_pending();
        }
        ui.label(
            RichText::new("Marks you can use: **bold**, *italic*, [text](https://address) and - for a list item.")
                .size(13.5)
                .color(p.muted),
        );

        let has_picture = state.selected_step().is_some_and(|s| s.image.is_some());
        if has_picture {
            let label = ui.label("Picture description, for people who cannot see the picture");
            let words = ui
                .add(
                    egui::TextEdit::multiline(&mut state.buffer.picture_description)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                )
                .labelled_by(label.id);
            widgets::ring(ui, &words);
            if words.lost_focus() {
                state.commit_pending();
            }
        }

        ui.add_space(6.0);
        let mut focused = state.is_focused();
        let can_focus = state.can_focus();
        let focus_box = ui.add_enabled(can_focus, egui::Checkbox::new(&mut focused, "Focused view"));
        widgets::ring(ui, &focus_box);
        if focus_box.changed() {
            status.say_opt(state.set_focused(focused));
        }
        if !can_focus {
            ui.label(RichText::new("Needs a picture with a click in it.").size(13.5).color(p.muted));
        }

        let mut skipped = state.selected_step().is_some_and(|s| s.skipped);
        let skip_box = ui.add(egui::Checkbox::new(&mut skipped, "Skip in export"));
        widgets::ring(ui, &skip_box);
        if skip_box.changed() {
            status.say_opt(state.set_skipped(skipped));
        }

        ui.add_space(8.0);
        ui.label(RichText::new("Drawing tools for marks come next.").size(13.5).color(p.muted));
    });
}

fn picture(ui: &mut egui::Ui, state: &mut EditorState) {
    let p = palette(ui);
    let Some(step) = state.selected_step() else {
        ui.label(RichText::new("Nothing to show.").color(p.muted));
        return;
    };
    let has_picture = step.image.is_some();
    let words = state.picture_words();
    let Some(texture) = (if has_picture { state.texture(ui.ctx()) } else { None }) else {
        ui.label(RichText::new("This step has no picture.").color(p.muted));
        return;
    };
    let available = ui.available_size();
    let natural = texture.size_vec2();
    let scale = (available.x / natural.x).min(available.y / natural.y).clamp(0.05, 1.0);
    let size = natural * scale;
    ui.vertical_centered(|ui| {
        ui.add(egui::Image::from_texture((texture.id(), size)).alt_text(words));
    });
}
