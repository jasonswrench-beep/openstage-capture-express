// SPDX-License-Identifier: Apache-2.0
//! The Editor screen (spec sections 4.2 and 9): the steps list on the left, the step's picture in the middle, its details on the
//! right. Every control has a name and a keyboard route, the picture has its words, and what happens is said in the status line.
//! The navy bar above (Back to guides, Undo, Redo, Export) and the menus are the window's (`shell.rs`); every command here is in a
//! menu too (`commands.rs`), and [`run`] does it the same way wherever it was chosen.
//!
//! Keys: Ctrl+Z and Ctrl+Y undo and redo (while a text box has the keyboard they belong to the box); Alt+Up and Alt+Down move
//! the step; Ctrl+Shift+Up and Ctrl+Shift+Down choose the step before or after; Ctrl+Shift+Delete deletes the step; F2 goes to
//! the title; Up and Down in the list choose a step. To choose several steps: Ctrl+click adds or takes out one, Shift+click or
//! Shift+Up and Shift+Down chooses a run, Ctrl+A or Choose all steps chooses every step; the buttons at the right then skip or
//! delete them all together (and move them up or down, with Alt+Up and Alt+Down or the buttons, or duplicate them), and one undo
//! takes the change back. Deleted steps are kept with the guide: the Deleted steps button, there once something is deleted, opens
//! the list where each can be put back.

use eframe::egui::{self, Key, Modifiers, RichText};

use openstage_express_core::marks::{self, Tool, COLOURS, WIDTHS};
use openstage_express_core::model::AnnotationKind;

use super::commands::{self, Command};
use super::editor_state::EditorState;
use super::icon::{self, UiIcon};
use super::menubar::{self, Entry, Line};
use super::shell;
use super::widgets::{self, palette, Status};

/// Whether the person left the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leave {
    Stay,
    /// To the list of deleted steps, where each can be put back.
    ToDeleted,
}

/// Whether one of the editor's commands can be used now, and whether it is ticked; `None` for a command that is not the editor's.
pub fn command_state(state: &EditorState, command: Command) -> Option<(bool, bool)> {
    let chosen = state.chosen_count();
    let one = chosen == 1 && state.selected_step().is_some();
    let rows = state.rows().len();
    let has_steps = !state.doc().guide.steps.is_empty();
    Some(match command {
        Command::Undo => (state.can_undo(), false),
        Command::Redo => (state.can_redo(), false),
        Command::ChooseAll => (rows > 1 && chosen < rows, false),
        Command::ChooseOne => (chosen > 1, false),
        Command::DeletedSteps => (state.deleted_count() > 0, false),
        Command::AddStep => (true, false),
        Command::DuplicateStep | Command::DeleteStep | Command::MoveUp | Command::MoveDown => (chosen >= 1, false),
        Command::PreviousStep | Command::NextStep => (rows > 1, false),
        Command::EditTitle => (one, false),
        Command::SkipInExport => (state.selected_step().is_some(), state.selected_step().is_some_and(|s| s.skipped)),
        Command::FocusedView => (one && state.can_focus(), state.is_focused()),
        Command::AddMark(_) => (one && state.picture_size().is_some(), false),
        Command::DeleteMark => (one && state.selected_mark().is_some(), false),
        Command::FocusedAllOn | Command::FocusedAllOff => (has_steps, false),
        _ => return None,
    })
}

/// Do one of the editor's commands, from a menu, the command search or a button, and say what was done. Returns where to go,
/// for the commands that leave the editor.
pub fn run(state: &mut EditorState, status: &mut Status, command: Command) -> Leave {
    if command_state(state, command).is_some_and(|(usable, _)| !usable) {
        return Leave::Stay;
    }
    match command {
        Command::Undo => status.say_opt(state.undo()),
        Command::Redo => status.say_opt(state.redo()),
        Command::ChooseAll => {
            status.say_opt(state.choose_all());
            state.focus_row_next = true;
        }
        Command::ChooseOne => {
            status.say_opt(state.choose_only_selected());
            state.focus_row_next = true;
        }
        Command::DeletedSteps => {
            state.commit_pending();
            return Leave::ToDeleted;
        }
        Command::AddStep => status.say_opt(state.add_step()),
        Command::DuplicateStep => status.say_opt(state.duplicate_selected()),
        Command::DeleteStep => {
            status.say_opt(if state.chosen_count() > 1 { state.delete_chosen() } else { state.delete_selected() });
            state.focus_row_next = true;
        }
        Command::MoveUp => status.say_opt(state.move_selected(-1)),
        Command::MoveDown => status.say_opt(state.move_selected(1)),
        Command::PreviousStep | Command::NextStep => {
            if let Some(id) = state.neighbour(if command == Command::PreviousStep { -1 } else { 1 }) {
                state.select(&id);
                state.focus_row_next = true;
                status.say_opt(state.selected_row().map(|r| r.label()));
            }
        }
        Command::EditTitle => state.focus_title_next = true,
        Command::SkipInExport => {
            if state.chosen_count() > 1 {
                let rows = state.rows();
                let ids = state.chosen_ids();
                let all_skipped = rows.iter().filter(|r| ids.contains(&r.id)).all(|r| r.skipped);
                status.say_opt(state.skip_chosen(!all_skipped));
            } else {
                let skipped = state.selected_step().is_some_and(|s| s.skipped);
                status.say_opt(state.set_skipped(!skipped));
            }
        }
        Command::FocusedView => {
            let on = state.is_focused();
            status.say_opt(state.set_focused(!on));
        }
        Command::AddMark(tool) => status.say_opt(state.add_mark(tool)),
        Command::DeleteMark => status.say_opt(state.delete_mark()),
        Command::FocusedAllOn => status.say_opt(state.set_focused_for_all(true)),
        Command::FocusedAllOff => status.say_opt(state.set_focused_for_all(false)),
        _ => {}
    }
    Leave::Stay
}

/// A menu of the editor's commands, as they stand now.
fn menu_lines(state: &EditorState, plan: &[Option<Command>]) -> Vec<Line<Command>> {
    plan.iter()
        .map(|item| match item {
            None => Line::Separator,
            Some(command) => {
                let (enabled, ticked) = command_state(state, *command).unwrap_or((true, false));
                Line::Item(Entry {
                    command: *command,
                    words: command.words(),
                    icon: Some(command.icon()),
                    keys: command.keys().map(str::to_string),
                    enabled,
                    kind: command.kind(),
                    ticked,
                })
            }
        })
        .collect()
}

pub fn show(ui: &mut egui::Ui, state: &mut EditorState, status: &mut Status) -> Leave {
    let mut leave = Leave::Stay;
    keys(ui.ctx(), state, status);
    let p = palette(ui);

    // The panels are a fixed share of the window, not draggable: a drag handle is a control with no keyboard route. The steps
    // come first, then the picture, then the details, which is the order a screen reader reads them in.
    let left = (ui.available_width() * 0.26).clamp(240.0, 340.0);
    let mut to_deleted = false;
    let mut chosen: Option<Command> = None;
    let steps_frame =
        egui::Frame::new().fill(p.surface).inner_margin(egui::Margin { left: 12, right: 10, top: 8, bottom: 8 });
    egui::Panel::left("editor-steps").resizable(false).exact_size(left).frame(steps_frame).show(ui, |ui| {
        steps_list(ui, state, status, &mut to_deleted, &mut chosen);
    });

    egui::CentralPanel::default().frame(egui::Frame::new().fill(p.page)).show(ui, |ui| {
        let area = ui.available_rect_before_wrap();
        ui.painter().vline(area.min.x, area.y_range(), egui::Stroke::new(1.0, p.divider));
        let details_width = (area.width() * 0.42).clamp(280.0, 400.0).min(area.width() * 0.6);
        let picture_area =
            egui::Rect::from_min_max(area.min, egui::pos2(area.max.x - details_width, area.max.y)).shrink(12.0);
        let details_area = egui::Rect::from_min_max(egui::pos2(area.max.x - details_width, area.min.y), area.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(picture_area), |ui| picture(ui, state, status));
        ui.painter().rect_filled(details_area, 0.0, p.surface);
        ui.painter().vline(details_area.min.x, area.y_range(), egui::Stroke::new(1.0, p.divider));
        ui.scope_builder(egui::UiBuilder::new().max_rect(details_area.shrink2(egui::vec2(12.0, 8.0))), |ui| {
            details(ui, state, status, &mut chosen)
        });
    });
    if let Some(command) = chosen {
        let went = run(state, status, command);
        if went != Leave::Stay {
            leave = went;
        }
    }
    if to_deleted {
        state.commit_pending();
        leave = Leave::ToDeleted;
    }
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
        status.say_opt(state.delete_chosen());
    }
    if rename {
        state.focus_title_next = true;
    }
}

/// What a click or a key did to the choice of steps.
enum Choice {
    /// Only this step is chosen.
    Only(String),
    /// Ctrl+click: add the step to the chosen ones, or take it out.
    Toggle(String),
    /// Shift+click or Shift+Up and Down: the run from where it started to this step.
    RunTo(String),
    /// Ctrl+A or the button: every step.
    All,
}

fn steps_list(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    status: &mut Status,
    to_deleted: &mut bool,
    chosen_command: &mut Option<Command>,
) {
    // The pane's heading, with its Add button and its "..." menu (the suite's panes all have the two).
    // Laid out left to right after a space, so the Tab key meets Add before "..." as the eye does.
    ui.horizontal(|ui| {
        widgets::subheading(ui, "Steps");
        let room = ui.available_width() - 136.0;
        ui.add_space(room.max(4.0));
        let add = ui.add(egui::Button::new((icon::atom(UiIcon::Plus, 16.0), "Add".to_string())));
        ui.ctx().accesskit_node_builder(add.id, |node| node.set_label("Add a step".to_string()));
        let add = add.on_hover_text("Add a step");
        widgets::ring(ui, &add);
        if add.clicked() {
            status.say_opt(state.add_step());
        }
        let more = menu_lines(state, &commands::STEPS_MORE);
        if let Some(command) = menubar::menu_button(ui, "steps-more", UiIcon::Ellipsis, "", "More for the steps", &more)
        {
            *chosen_command = Some(command);
        }
    });
    let rows = state.rows();
    let selected = state.selected_id().map(str::to_string);
    let take_keyboard = std::mem::take(&mut state.focus_row_next);
    let mut choice: Option<Choice> = None;
    let mut keyboard_on_selected = false;
    // Room for the hint under the list, and for the Deleted steps button when there are deleted steps to open.
    let below = if state.deleted_count() > 0 { 92.0 } else { 40.0 };
    egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(ui.available_height() - below).show(ui, |ui| {
        for row in &rows {
            let is_selected = selected.as_deref() == Some(row.id.as_str());
            // A steps list is a list of choices: each row is named for what it says, and each chosen one is marked. Only the
            // selected row takes the keyboard.
            let response = widgets::list_row_of_many(
                ui,
                &row.label(),
                state.is_chosen(&row.id),
                is_selected,
                row.skipped,
                row.depth as f32 * 22.0,
                take_keyboard,
            );
            if response.clicked() {
                let (command, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
                if command {
                    choice = Some(Choice::Toggle(row.id.clone()));
                } else if shift {
                    choice = Some(Choice::RunTo(row.id.clone()));
                } else if !is_selected || state.chosen_count() > 1 {
                    choice = Some(Choice::Only(row.id.clone()));
                }
                state.focus_row_next = true;
            }
            keyboard_on_selected |= is_selected && response.has_focus();
            if response.has_focus() {
                response.scroll_to_me(None);
            }
        }
    });
    // Up and Down choose the step above or below while the chosen row has the keyboard, and the keyboard goes with the choice.
    // Shift with them lengthens or shortens a run of chosen steps, and Ctrl+A chooses every step. A screen reader says the row it
    // lands on, so nothing is added to the status line for a single step.
    if keyboard_on_selected {
        let (up, down, shift_up, shift_down, all) = ui.ctx().input_mut(|i| {
            // The keys with Shift are taken first, so that Shift+Up is not also a plain Up.
            let shift_up = i.consume_key(Modifiers::SHIFT, Key::ArrowUp);
            let shift_down = i.consume_key(Modifiers::SHIFT, Key::ArrowDown);
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                shift_up,
                shift_down,
                i.consume_key(Modifiers::COMMAND, Key::A),
            )
        });
        let target = if shift_up {
            state.neighbour(-1).map(Choice::RunTo)
        } else if shift_down {
            state.neighbour(1).map(Choice::RunTo)
        } else if up {
            state.neighbour(-1).map(Choice::Only)
        } else if down {
            state.neighbour(1).map(Choice::Only)
        } else if all {
            Some(Choice::All)
        } else {
            None
        };
        if target.is_some() {
            choice = target;
            state.focus_row_next = true;
        }
    }
    if let Some(choice) = choice {
        match choice {
            Choice::Only(id) => state.select(&id),
            Choice::Toggle(id) => status.say_opt(state.toggle_chosen(&id)),
            Choice::RunTo(id) => status.say_opt(state.choose_run_to(&id)),
            Choice::All => status.say_opt(state.choose_all()),
        }
    }
    ui.add_space(4.0);
    // Only when there is something deleted, so a guide that has lost nothing has no extra control to stop at.
    let deleted = state.deleted_count();
    if deleted > 0 && widgets::icon_button(ui, UiIcon::ArchiveRestore, &format!("Deleted steps ({deleted})")).clicked()
    {
        *to_deleted = true;
    }
    if rows.len() > 1 {
        ui.label(RichText::new("Ctrl+click or Shift+click chooses several.").size(13.5).color(palette(ui).muted));
    }
}

/// What can be done to several chosen steps at once, in place of the details of one.
fn several_chosen(ui: &mut egui::Ui, state: &mut EditorState, status: &mut Status) {
    let p = palette(ui);
    let ids = state.chosen_ids();
    ui.label(RichText::new(format!("{} steps are chosen:", ids.len())).color(p.text));
    let rows = state.rows();
    let shown = 8;
    for row in rows.iter().filter(|r| ids.contains(&r.id)).take(shown) {
        ui.label(RichText::new(row.label()).color(p.muted));
    }
    if ids.len() > shown {
        ui.label(RichText::new(format!("and {} more", ids.len() - shown)).color(p.muted));
    }
    ui.add_space(6.0);
    let any_skipped = rows.iter().any(|r| ids.contains(&r.id) && r.skipped);
    let any_in = rows.iter().any(|r| ids.contains(&r.id) && !r.skipped);
    if widgets::button_enabled(ui, any_in, "Skip these steps in export").clicked() {
        status.say_opt(state.skip_chosen(true));
    }
    if widgets::button_enabled(ui, any_skipped, "Put these steps back in export").clicked() {
        status.say_opt(state.skip_chosen(false));
    }
    // Always pressable, like Alt+Up and Alt+Down: at the end of the list they say so, and the keyboard is not left on a button
    // that has just been switched off.
    if widgets::button(ui, "Move these steps up").clicked() {
        status.say_opt(state.move_selected(-1));
    }
    if widgets::button(ui, "Move these steps down").clicked() {
        status.say_opt(state.move_selected(1));
    }
    if widgets::button(ui, "Duplicate these steps").clicked() {
        status.say_opt(state.duplicate_selected());
    }
    if widgets::button(ui, "Delete these steps").clicked() {
        status.say_opt(state.delete_chosen());
    }
    let only = state.selected_row().map(|r| r.label()).unwrap_or_default();
    if widgets::button_named(ui, "Choose just one", &format!("Choose just one step: {only}")).clicked() {
        status.say_opt(state.choose_only_selected());
        state.focus_row_next = true;
    }
    ui.label(
        RichText::new("Alt+Up and Alt+Down move them too. Undo takes back any of these, all the steps at once.")
            .size(13.5)
            .color(p.muted),
    );
}

fn details(ui: &mut egui::Ui, state: &mut EditorState, status: &mut Status, chosen: &mut Option<Command>) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        shell::section(ui, "guide", UiIcon::BookOpen, "Guide", true, |ui| guide_section(ui, state, status));
        if state.selected_step().is_none() {
            widgets::subheading(ui, "Step details");
            let p = palette(ui);
            ui.label(RichText::new("No step is chosen. Add a step, or choose one from the list.").color(p.muted));
            return;
        }
        if state.chosen_count() > 1 {
            shell::section(ui, "several", UiIcon::ListChecks, "Several steps chosen", true, |ui| {
                several_chosen(ui, state, status)
            });
            return;
        }
        shell::section(ui, "step", UiIcon::FileText, "Step details", true, |ui| step_section(ui, state, status));
        shell::section(ui, "marks", UiIcon::Shapes, "Marks", true, |ui| marks_panel(ui, state, status, chosen));
    });
}

/// The guide as a whole: its title, and the focused view for every step.
fn guide_section(ui: &mut egui::Ui, state: &mut EditorState, status: &mut Status) {
    let p = palette(ui);
    let label = ui.label("Guide title");
    let guide_title = ui
        .add(egui::TextEdit::singleline(&mut state.buffer.guide_title).desired_width(f32::INFINITY))
        .labelled_by(label.id);
    widgets::ring(ui, &guide_title);
    if guide_title.lost_focus() {
        let before = state.doc().guide.title.clone();
        state.commit_pending();
        if state.doc().guide.title != before {
            status.say(format!("The guide is now called {}.", state.doc().guide.title));
        }
    }
    // The focused view for the whole guide: how many steps have it, and one press to turn it on or off for all of them. Like the other
    // buttons that change many things, they are always pressable, and the status line says what was done.
    ui.add_space(4.0);
    ui.label(RichText::new(state.focus_summary()).size(13.5).color(p.muted));
    ui.horizontal_wrapped(|ui| {
        if widgets::icon_button(ui, UiIcon::ZoomIn, "Focused view on for every step").clicked() {
            status.say_opt(state.set_focused_for_all(true));
        }
        if widgets::icon_button(ui, UiIcon::ZoomOut, "Focused view off for every step").clicked() {
            status.say_opt(state.set_focused_for_all(false));
        }
    });
}

/// The chosen step: its words, its picture's description, the focused view, skipping it, and moving it.
fn step_section(ui: &mut egui::Ui, state: &mut EditorState, status: &mut Status) {
    let p = palette(ui);
    {
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
        // The crop's own buttons come right under the box that turns it on.
        if state.is_focused() {
            crop_panel(ui, state, status);
        }

        let mut skipped = state.selected_step().is_some_and(|s| s.skipped);
        let skip_box = ui.add(egui::Checkbox::new(&mut skipped, "Skip in export"));
        widgets::ring(ui, &skip_box);
        if skip_box.changed() {
            status.say_opt(state.set_skipped(skipped));
        }

        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if widgets::icon_button(ui, UiIcon::ArrowUp, "Move step up").clicked() {
                status.say_opt(state.move_selected(-1));
            }
            if widgets::icon_button(ui, UiIcon::ArrowDown, "Move step down").clicked() {
                status.say_opt(state.move_selected(1));
            }
        });
        ui.label(RichText::new("Alt+Up and Alt+Down do the same.").size(13.5).color(p.muted));
    }
}

/// Where the focused crop is, in words, and buttons to move and zoom it. Like the buttons that move a step they are always pressable,
/// so the keyboard is never left on a button that has just switched itself off: at an edge the status line says so. Dragging the
/// picture does the same with the mouse, and is only a convenience on top.
fn crop_panel(ui: &mut egui::Ui, state: &mut EditorState, status: &mut Status) {
    let p = palette(ui);
    ui.label(RichText::new(state.crop_words().unwrap_or_default()).size(13.5).color(p.muted));
    ui.horizontal_wrapped(|ui| {
        for (words, across, down) in
            [("Move crop left", -1, 0), ("Move crop right", 1, 0), ("Move crop up", 0, -1), ("Move crop down", 0, 1)]
        {
            if widgets::button(ui, words).clicked() {
                status.say_opt(state.move_crop(across, down));
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        if widgets::button(ui, "Zoom crop in").clicked() {
            status.say_opt(state.zoom_crop(true));
        }
        if widgets::button(ui, "Zoom crop out").clicked() {
            status.say_opt(state.zoom_crop(false));
        }
        if widgets::button(ui, "Reset crop").clicked() {
            status.say_opt(state.reset_crop());
        }
    });
    ui.label(
        RichText::new("Or drag the picture to move the crop (hold Space to drag over a mark).")
            .size(13.5)
            .color(p.muted),
    );
    ui.add_space(4.0);
}

/// The marks of the selected step: buttons to add each kind, the list of marks, and what can be done to the chosen one. All of it
/// works by keyboard; dragging on the picture is a convenience on top.
fn marks_panel(ui: &mut egui::Ui, state: &mut EditorState, status: &mut Status, chosen: &mut Option<Command>) {
    let p = palette(ui);
    let Some((_, picture_h)) = state.picture_size() else {
        ui.label(RichText::new("Marks need a picture.").size(13.5).color(p.muted));
        return;
    };
    // The nine kinds of mark are one menu (and the Marks menu at the top), not nine buttons.
    let add = menu_lines(state, &commands::ADD_MARK);
    if let Some(command) = menubar::menu_button(ui, "add-mark", UiIcon::Plus, "Add a mark", "Add a mark", &add) {
        *chosen = Some(command);
    }
    // Drawing with the mouse is a convenience on top of the buttons, so its ten choices are folded away behind one heading: a person
    // using the keyboard passes one stop, not ten.
    let draw_header = egui::CollapsingHeader::new(RichText::new("Draw a shape with the mouse").size(15.0))
        .default_open(state.drawing_tool().is_some())
        .show(ui, |ui| {
            ui.label(
                RichText::new("Choose a shape, then drag on the picture. A click places a usual-sized one.")
                    .size(13.5)
                    .color(p.muted),
            );
            ui.horizontal_wrapped(|ui| {
                let off =
                    ui.add(egui::RadioButton::new(state.drawing_tool().is_none(), "Do not draw (pick and move marks)"));
                widgets::ring(ui, &off);
                if off.clicked() {
                    status.say_opt(state.choose_drawing_tool(None));
                }
                for tool in Tool::ALL {
                    let r = ui.add(egui::RadioButton::new(
                        state.drawing_tool() == Some(tool),
                        format!("Draw {}", tool.with_article()),
                    ));
                    widgets::ring(ui, &r);
                    if r.clicked() {
                        status.say_opt(state.choose_drawing_tool(Some(tool)));
                    }
                }
            });
        });
    widgets::ring(ui, &draw_header.header_response);
    let rows = state.mark_rows();
    if rows.is_empty() {
        ui.label(RichText::new("No marks yet. Add one with a button above.").size(13.5).color(p.muted));
        return;
    }
    ui.add_space(4.0);
    let selected = state.selected_mark_id().map(str::to_string);
    let take_keyboard = std::mem::take(&mut state.focus_mark_next);
    let mut chosen: Option<String> = None;
    let mut keyboard_on_selected = false;
    for (id, words) in &rows {
        let is_selected = selected.as_deref() == Some(id.as_str());
        let response = widgets::list_row(ui, words, is_selected, false, 0.0, take_keyboard);
        if response.clicked() && !is_selected {
            chosen = Some(id.clone());
            state.focus_mark_next = true;
        }
        keyboard_on_selected |= is_selected && response.has_focus();
    }
    if keyboard_on_selected {
        let (up, down, delete) = ui.ctx().input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::Delete),
            )
        });
        let target = if up {
            state.mark_neighbour(-1)
        } else if down {
            state.mark_neighbour(1)
        } else {
            None
        };
        if let Some(id) = target {
            chosen = Some(id);
            state.focus_mark_next = true;
        }
        if delete {
            status.say_opt(state.delete_mark());
        }
    }
    if let Some(id) = chosen {
        state.select_mark(Some(&id));
    }
    let Some(mark) = state.selected_mark() else { return };
    state.sync_mark_words();

    ui.add_space(6.0);
    ui.label(RichText::new("Move the chosen mark:").color(p.muted));
    ui.horizontal_wrapped(|ui| {
        for (words, dx, dy) in [("Move left", -10, 0), ("Move right", 10, 0), ("Move up", 0, -10), ("Move down", 0, 10)]
        {
            if widgets::button(ui, words).clicked() {
                status.say_opt(state.nudge_mark(dx, dy));
            }
        }
    });
    if marks::can_resize(mark.kind) {
        ui.label(RichText::new("Change its size:").color(p.muted));
        ui.horizontal_wrapped(|ui| {
            let round =
                matches!(mark.kind, AnnotationKind::Number | AnnotationKind::ClickMark | AnnotationKind::Cursor);
            let line = matches!(mark.kind, AnnotationKind::Line | AnnotationKind::Arrow);
            let mut choices: Vec<(&str, i32, i32)> = Vec::new();
            if round {
                choices.extend([("Bigger", 5, 0), ("Smaller", -5, 0)]);
            } else {
                choices.extend([("Wider", 10, 0), ("Narrower", -10, 0)]);
                if mark.kind != AnnotationKind::Text {
                    choices.extend([("Taller", 0, 10), ("Shorter", 0, -10)]);
                }
            }
            let _ = line;
            for (words, dx, dy) in choices {
                if widgets::button(ui, words).clicked() {
                    status.say_opt(state.size_mark(dx, dy));
                }
            }
        });
    }
    if marks::colour_field(mark.kind).is_some() {
        ui.label(RichText::new("Colour:").color(p.muted));
        ui.horizontal_wrapped(|ui| {
            let have = marks::colour_name(&mark);
            for (name, hex) in COLOURS {
                let r = ui.add(egui::RadioButton::new(have == Some(name), name));
                widgets::ring(ui, &r);
                if r.clicked() {
                    status.say_opt(state.set_mark_colour(name, hex));
                }
            }
        });
    }
    if marks::has_width(mark.kind) {
        ui.label(RichText::new("Line width:").color(p.muted));
        ui.horizontal_wrapped(|ui| {
            for width in WIDTHS {
                let r = ui.add(egui::RadioButton::new(mark.stroke_width == Some(width), format!("{width} pixels")));
                widgets::ring(ui, &r);
                if r.clicked() {
                    status.say_opt(state.set_mark_width(width));
                }
            }
        });
    }
    if mark.kind == AnnotationKind::Text {
        let label = ui.label("Words in the text mark");
        let words = ui
            .add(egui::TextEdit::singleline(&mut state.mark_words).desired_width(f32::INFINITY))
            .labelled_by(label.id);
        widgets::ring(ui, &words);
        if words.lost_focus() {
            let text = state.mark_words.clone();
            status.say_opt(state.set_mark_text(&text));
        }
    }
    if mark.kind == AnnotationKind::Number {
        let number = mark.number.unwrap_or(1);
        ui.label(format!("Step number {number}"));
        ui.horizontal(|ui| {
            if widgets::button(ui, "Number up").clicked() {
                status.say_opt(state.set_mark_number(number + 1));
            }
            if widgets::button_enabled(ui, number > 1, "Number down").clicked() {
                status.say_opt(state.set_mark_number(number - 1));
            }
        });
    }
    ui.add_space(4.0);
    if widgets::icon_button(ui, UiIcon::Trash, "Delete mark").clicked() {
        status.say_opt(state.delete_mark());
    }
    let _ = picture_h;
    let _ = Tool::ALL;
}

/// The picture is drawn whole, with the focused crop (if the step has one) shown as a frame over it. Marks can be picked up and
/// dragged with the mouse, and the chosen mark shows handles to drag its size. None of this is the only way to do anything: the marks
/// list and its buttons do it all by keyboard.
fn picture(ui: &mut egui::Ui, state: &mut EditorState, status: &mut Status) {
    let p = palette(ui);
    let Some(step) = state.selected_step() else {
        ui.label(RichText::new("Nothing to show.").color(p.muted));
        return;
    };
    let has_picture = step.image.is_some();
    // The crop as it is now: one being dragged with the mouse is shown where it is.
    let focus = state.crop();
    let words = state.picture_words();
    let Some(texture) = (if has_picture { state.texture(ui.ctx()) } else { None }) else {
        ui.label(RichText::new("This step has no picture.").color(p.muted));
        return;
    };
    let available = ui.available_size();
    let natural = texture.size_vec2();
    let scale = (available.x / natural.x).min(available.y / natural.y).clamp(0.05, 1.0);
    let size = natural * scale;
    let image_rect =
        ui.vertical_centered(|ui| ui.add(egui::Image::from_texture((texture.id(), size)).alt_text(words)).rect).inner;
    let painter = ui.painter_at(image_rect);
    let to_screen = |x: i32, y: i32| image_rect.min + egui::vec2(x as f32 * scale, y as f32 * scale);
    if let Some(crop) = &focus {
        let dim = egui::Color32::from_black_alpha(120);
        let (tl, br) = (to_screen(crop.x, crop.y), to_screen(crop.x + crop.width, crop.y + crop.height));
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
            egui::Stroke::new(2.0, p.overlay),
            egui::StrokeKind::Inside,
        );
    }
    let picture_h = state.picture_size().map(|(_, h)| h).unwrap_or(1);
    if let Some(mark) = state.selected_mark() {
        let b = marks::bounds(&mark, picture_h);
        let outline =
            egui::Rect::from_min_max(to_screen(b.x, b.y), to_screen(b.x + b.width, b.y + b.height)).expand(2.0);
        painter.rect_stroke(outline, 0.0, egui::Stroke::new(2.0, p.overlay), egui::StrokeKind::Outside);
        for (_, (hx, hy)) in marks::handles(&mark, picture_h) {
            let at = to_screen(hx, hy);
            let square = egui::Rect::from_center_size(at, egui::vec2(9.0, 9.0));
            painter.rect_filled(square, 1.0, egui::Color32::WHITE);
            painter.rect_stroke(square, 1.0, egui::Stroke::new(2.0, p.overlay), egui::StrokeKind::Inside);
        }
    }
    // With several steps chosen the picture is only shown: the marks belong to one step.
    if state.chosen_count() > 1 {
        return;
    }
    // The mouse: press on a mark (or a handle of the chosen mark) to pick it up, move to drag it, let go to keep it.
    let origin_id = ui.make_persistent_id("mark-drag-origin");
    let (pos, pressed, down, released) = ui.input(|i| {
        (i.pointer.interact_pos(), i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.primary_released())
    });
    let to_picture = |at: egui::Pos2| -> (i32, i32) {
        let v = (at - image_rect.min) / scale;
        (v.x.round() as i32, v.y.round() as i32)
    };
    // With a drawing tool chosen the mouse draws instead: press where the shape starts, drag to where it ends, let go to keep it.
    // Escape stops a drawing in progress, or turns the tool off.
    if state.drawing_tool().is_some() {
        if ui.rect_contains_pointer(image_rect) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
        if ui.ctx().input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            status.say_opt(state.cancel_draw());
        }
        if let Some(pos) = pos {
            if pressed && image_rect.contains(pos) {
                let (x, y) = to_picture(pos);
                state.begin_draw(x, y);
            } else if down && state.is_drawing() {
                let (x, y) = to_picture(pos);
                state.draw_to(x, y);
            }
        }
        if released && state.is_drawing() {
            status.say_opt(state.end_draw());
            state.focus_mark_next = true;
        }
        return;
    }
    if let Some(pos) = pos {
        if pressed && image_rect.contains(pos) {
            let (x, y) = to_picture(pos);
            // A mark under the pointer is picked up first. Where there is none (or with Space held, which moves the crop even over
            // a mark) the press begins moving the focused crop, if the step has one.
            let space = ui.input(|i| i.key_down(Key::Space));
            if (!space && state.begin_drag(x, y)) || state.begin_crop_drag() {
                ui.ctx().data_mut(|d| d.insert_temp(origin_id, pos));
            }
        } else if down && (state.is_dragging() || state.is_moving_crop()) {
            if let Some(origin) = ui.ctx().data(|d| d.get_temp::<egui::Pos2>(origin_id)) {
                let delta = (pos - origin) / scale;
                let (dx, dy) = (delta.x.round() as i32, delta.y.round() as i32);
                if state.is_dragging() {
                    state.drag_by(dx, dy);
                } else {
                    state.crop_drag_by(dx, dy);
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
            }
        }
    }
    if released && state.is_dragging() {
        status.say_opt(state.end_drag());
        state.focus_mark_next = true;
    }
    if released && state.is_moving_crop() {
        status.say_opt(state.end_crop_drag());
    }
}
