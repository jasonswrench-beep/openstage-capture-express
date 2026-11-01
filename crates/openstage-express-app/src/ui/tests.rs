// SPDX-License-Identifier: Apache-2.0
//! Whole screens run without a window, and read back what a screen reader would be told.
//!
//! A `Harness` runs the program's real drawing code through egui with AccessKit switched on, exactly as the window does, and
//! keeps the accessibility tree each pass produces: every node's role, name, value, state and place. It can then do what a
//! screen reader or a keyboard does (invoke a control by its name, press keys, type) and read the tree again. Nothing here
//! looks at pixels: what is checked is what an assistive technology is given.

use std::collections::HashMap;

use eframe::egui::{self, accesskit, Event, Key, Modifiers, Pos2, RawInput, Rect, Vec2};

use std::cell::RefCell;
use std::rc::Rc;

use super::app::{App, Screen, WindowMode};
use super::bar::{BarAction, BarPhase, BarView};
use super::capture::{CaptureControl, Finished, HotkeyAction, Outcome, Starter};
use super::theme;

use accesskit::{Action, ActionRequest, Live, Node, NodeId, Role, TreeId};

struct Harness {
    ctx: egui::Context,
    app: App,
    size: Vec2,
    time: f64,
    nodes: HashMap<NodeId, Node>,
    /// Every node in reading order: the tree's own parent-and-child order, which is what a screen reader walks.
    order: Vec<NodeId>,
    depth: HashMap<NodeId, usize>,
    focus: Option<NodeId>,
    events: Vec<Event>,
}

impl Harness {
    fn new(screen: Screen) -> Harness {
        Harness::sized(screen, egui::vec2(1280.0, 720.0), 1.0, true)
    }

    fn sized(screen: Screen, size: Vec2, pixels_per_point: f32, dark: bool) -> Harness {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        theme::install(&ctx);
        ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
        ctx.set_pixels_per_point(pixels_per_point);
        let mut h = Harness {
            ctx,
            app: App::new(screen),
            size,
            time: 0.0,
            nodes: HashMap::new(),
            order: Vec::new(),
            depth: HashMap::new(),
            focus: None,
            events: Vec::new(),
        };
        h.settle();
        h
    }

    /// One pass of the whole window, with whatever has been queued as input.
    fn frame(&mut self) {
        self.time += 1.0 / 60.0;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.time),
            events: std::mem::take(&mut self.events),
            ..Default::default()
        };
        let app = &mut self.app;
        let mut output = self.ctx.run_ui(input, |ui| app.draw(ui));
        // There is no screen to draw on, so the changes to the pictures are dropped on purpose.
        output.textures_delta.clear();
        if let Some(update) = output.platform_output.accesskit_update {
            self.nodes = update.nodes.into_iter().collect();
            self.focus = Some(update.focus);
            self.order.clear();
            self.depth.clear();
            if let Some(tree) = update.tree {
                let mut stack = vec![(tree.root, 0usize)];
                while let Some((id, depth)) = stack.pop() {
                    let Some(node) = self.nodes.get(&id) else { continue };
                    self.order.push(id);
                    self.depth.insert(id, depth);
                    for child in node.children().iter().rev() {
                        stack.push((*child, depth + 1));
                    }
                }
            }
        }
    }

    /// Enough passes for the layout to settle and for what a key or a click did to show.
    fn settle(&mut self) {
        for _ in 0..4 {
            self.frame();
        }
    }

    /// The name a screen reader says for a node: its own label, or else the words of what labels it, or (for plain text) its
    /// text. A box's own contents are not its name.
    fn name(&self, id: NodeId) -> Option<String> {
        let node = self.nodes.get(&id)?;
        if let Some(label) = node.label().filter(|l| !l.is_empty()) {
            return Some(label.to_string());
        }
        let by: Vec<String> = node.labelled_by().iter().filter_map(|l| self.name(*l)).collect();
        if !by.is_empty() {
            return Some(by.join(" "));
        }
        if matches!(node.role(), Role::Label | Role::TextRun | Role::Heading | Role::Status) {
            return node.value().map(str::to_string);
        }
        None
    }

    fn find_all(&self, role: Role, words: &str) -> Vec<NodeId> {
        self.order
            .iter()
            .copied()
            .filter(|id| self.nodes[id].role() == role && self.name(*id).is_some_and(|w| w.trim_end() == words))
            .collect()
    }

    fn find(&self, role: Role, words: &str) -> NodeId {
        let found = self.find_all(role, words);
        assert_eq!(
            found.len(),
            1,
            "expected one {role:?} named {words:?}, found {}. The tree has:\n{}",
            found.len(),
            self.dump()
        );
        found[0]
    }

    fn has(&self, role: Role, words: &str) -> bool {
        !self.find_all(role, words).is_empty()
    }

    fn node(&self, id: NodeId) -> &Node {
        &self.nodes[&id]
    }

    fn dump(&self) -> String {
        self.order
            .iter()
            .map(|id| {
                let n = &self.nodes[id];
                format!(
                    "{}{:?} {:?}{}{}{}",
                    "  ".repeat(self.depth[id] + 1),
                    n.role(),
                    self.name(*id),
                    n.value()
                        .filter(|_| !matches!(n.role(), Role::Label | Role::TextRun | Role::Heading))
                        .map(|v| format!(" value={v:?}"))
                        .unwrap_or_default(),
                    if n.is_disabled() { " [disabled]" } else { "" },
                    if n.is_selected() == Some(true) { " [selected]" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// What a screen reader's "press" does.
    fn invoke(&mut self, role: Role, words: &str) {
        let id = self.find(role, words);
        self.events.push(Event::AccessKitActionRequest(ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: id,
            data: None,
        }));
        self.settle();
    }

    /// What a screen reader's "move the keyboard here" does.
    fn focus_on(&mut self, role: Role, words: &str) {
        let id = self.find(role, words);
        self.events.push(Event::AccessKitActionRequest(ActionRequest {
            action: Action::Focus,
            target_tree: TreeId::ROOT,
            target_node: id,
            data: None,
        }));
        self.settle();
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        for pressed in [true, false] {
            self.events.push(Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers });
        }
        self.settle();
    }

    fn type_text(&mut self, text: &str) {
        self.events.push(Event::Text(text.to_string()));
        self.settle();
    }

    fn focused_words(&self) -> Option<String> {
        self.name(self.focus?)
    }

    fn status_node(&self) -> NodeId {
        self.order.iter().copied().find(|id| self.nodes[id].role() == Role::Status).expect("a status line")
    }

    fn status(&self) -> String {
        self.name(self.status_node()).unwrap_or_default().trim_end().to_string()
    }

    fn selected_rows(&self) -> Vec<String> {
        self.order
            .iter()
            .filter(|id| self.nodes[id].role() == Role::ListBoxOption && self.nodes[id].is_selected() == Some(true))
            .filter_map(|id| self.name(*id))
            .collect()
    }

    fn rows(&self) -> Vec<String> {
        self.order
            .iter()
            .filter(|id| self.nodes[id].role() == Role::ListBoxOption)
            .filter_map(|id| self.name(*id))
            .collect()
    }
}
const NO_MODS: Modifiers = Modifiers::NONE;

fn ctrl() -> Modifiers {
    Modifiers::COMMAND
}

#[test]
fn home_is_a_heading_a_button_a_search_box_and_the_guide_all_with_names() {
    let h = Harness::new(Screen::Home);
    let title = h.find(Role::Heading, "OpenStage Capture Express");
    assert_eq!(h.node(title).level(), Some(1));
    h.find(Role::Button, "Start capturing");
    let search = h.find(Role::TextInput, "Search");
    assert_eq!(h.node(search).placeholder(), Some("Search your guides"));
    assert_eq!(h.node(h.find(Role::Heading, "Your guides")).level(), Some(2));
    h.find(Role::Button, "How to submit an assignment, 5 steps, example guide");
    assert_eq!(h.status(), "Ready.");
}

#[test]
fn every_control_on_every_screen_has_a_name_and_can_be_reached_by_the_keyboard() {
    for screen in [Screen::Home, Screen::Editor, Screen::Bar] {
        for dark in [true, false] {
            let h = Harness::sized(screen, egui::vec2(1280.0, 720.0), 1.0, dark);
            let mut inputs = 0;
            for id in &h.order {
                let node = h.node(*id);
                if egui::accessibility::is_input(node.role()) {
                    inputs += 1;
                    let named = h.name(*id).is_some_and(|w| !w.trim().is_empty());
                    assert!(named, "{screen:?}: an unnamed {:?}. The tree has:\n{}", node.role(), h.dump());
                    assert!(
                        node.supports_action(Action::Focus) || node.is_disabled(),
                        "{screen:?}: {:?} {:?} cannot be given the keyboard",
                        node.role(),
                        h.name(*id)
                    );
                }
            }
            assert!(inputs >= 2, "{screen:?}: only {inputs} controls");
        }
    }
}

#[test]
fn invoking_the_guide_opens_the_editor_and_says_so() {
    let mut h = Harness::new(Screen::Home);
    h.invoke(Role::Button, "How to submit an assignment, 5 steps, example guide");
    assert_eq!(h.app.screen, Screen::Editor);
    assert_eq!(h.status(), "Opened the example guide, 5 steps. It is made up, for trying the editor.");
    h.find(Role::Button, "Back to guides");
    h.find(Role::Heading, "How to submit an assignment");
}

#[test]
fn start_capturing_opens_the_capture_bar_ready_to_start() {
    let mut h = Harness::new(Screen::Home);
    assert_eq!(h.app.window_mode(), WindowMode::Normal);
    h.invoke(Role::Button, "Start capturing");
    assert_eq!(h.app.screen, Screen::Bar);
    assert_eq!(h.app.window_mode(), WindowMode::Bar, "the window becomes the strip");
    h.find(Role::Heading, "Ready to capture");
    h.find(Role::Button, "Start");
    h.find(Role::Button, "Cancel");
    assert!(h.status().starts_with("The capture bar is open."), "{}", h.status());
    assert_eq!(h.focused_words().as_deref(), Some("Start"), "the keyboard is on Start, so Enter starts");
}

#[test]
fn the_editor_lists_every_step_as_a_choice_and_marks_the_chosen_one() {
    let h = Harness::new(Screen::Editor);
    assert_eq!(
        h.rows(),
        [
            "1. Click \"Assignments\"",
            "2. Click \"New assignment\"",
            "2.1. Type \"Week 3 reflection\"",
            "3. Select \"Course\"",
            "Click \"Debug\" (skipped)",
            "Note: Tip",
            "4. Click \"Submit\""
        ]
    );
    assert_eq!(h.selected_rows(), ["1. Click \"Assignments\""]);
}

#[test]
fn the_details_are_named_boxes_with_the_steps_words_in_them() {
    let h = Harness::new(Screen::Editor);
    let title = h.find(Role::TextInput, "Title");
    assert_eq!(h.node(title).value(), Some("Click \"Assignments\""));
    let description = h.find(Role::MultilineTextInput, "Description");
    assert!(h.node(description).value().unwrap().starts_with("Open the **Assignments** page"));
    let words = h.find(Role::MultilineTextInput, "Picture description, for people who cannot see the picture");
    assert_eq!(h.node(words).value(), Some("The course site with the Assignments item in the left menu circled"));
    let focused = h.find(Role::CheckBox, "Focused view");
    assert_eq!(h.node(focused).toggled(), Some(accesskit::Toggled::True));
    let skip = h.find(Role::CheckBox, "Skip in export");
    assert_eq!(h.node(skip).toggled(), Some(accesskit::Toggled::False));
}

#[test]
fn the_picture_is_an_image_with_the_authors_words() {
    let h = Harness::new(Screen::Editor);
    let image = h.find(Role::Image, "The course site with the Assignments item in the left menu circled");
    assert!(h.node(image).bounds().is_some_and(|b| b.x1 > b.x0 && b.y1 > b.y0));
}

#[test]
fn undo_and_redo_are_unavailable_until_there_is_something_to_undo() {
    let h = Harness::new(Screen::Editor);
    assert!(h.node(h.find(Role::Button, "Undo")).is_disabled());
    assert!(h.node(h.find(Role::Button, "Redo")).is_disabled());
}

#[test]
fn down_arrow_in_the_list_chooses_the_next_step_and_the_keyboard_goes_with_it() {
    let mut h = Harness::new(Screen::Editor);
    h.focus_on(Role::ListBoxOption, "1. Click \"Assignments\"");
    h.key(Key::ArrowDown, NO_MODS);
    assert_eq!(h.selected_rows(), ["2. Click \"New assignment\""]);
    assert_eq!(
        h.focused_words().as_deref(),
        Some("2. Click \"New assignment\""),
        "the keyboard stays with the chosen row"
    );
    assert_eq!(
        h.node(h.find(Role::TextInput, "Title")).value(),
        Some("Click \"New assignment\""),
        "the details follow"
    );
    h.key(Key::ArrowUp, NO_MODS);
    assert_eq!(h.selected_rows(), ["1. Click \"Assignments\""]);
    assert_eq!(h.focused_words().as_deref(), Some("1. Click \"Assignments\""));
    h.key(Key::ArrowUp, NO_MODS);
    assert_eq!(h.selected_rows(), ["1. Click \"Assignments\""], "going up past the top does not choose another step");
}

#[test]
fn invoking_a_row_chooses_that_step() {
    let mut h = Harness::new(Screen::Editor);
    h.invoke(Role::ListBoxOption, "3. Select \"Course\"");
    assert_eq!(h.selected_rows(), ["3. Select \"Course\""]);
    assert_eq!(h.node(h.find(Role::TextInput, "Title")).value(), Some("Select \"Course\""));
}

#[test]
fn alt_down_moves_the_step_and_the_status_line_says_where_it_went() {
    let mut h = Harness::new(Screen::Editor);
    h.key(Key::ArrowDown, Modifiers::ALT);
    assert_eq!(h.status(), "Moved down. Now 2. Click \"Assignments\".");
    assert!(h.rows()[0].starts_with("1. Click \"New assignment\""), "{:?}", h.rows());
    assert!(!h.node(h.find(Role::Button, "Undo")).is_disabled());
    h.key(Key::ArrowUp, Modifiers::ALT);
    assert_eq!(h.rows()[0], "1. Click \"Assignments\"");
}

#[test]
fn control_z_undoes_and_control_y_redoes() {
    let mut h = Harness::new(Screen::Editor);
    h.key(Key::ArrowDown, Modifiers::ALT);
    h.key(Key::Z, ctrl());
    assert_eq!(h.status(), "Undone.");
    assert!(h.rows()[0].starts_with("1. Click \"Assignments\""));
    h.key(Key::Y, ctrl());
    assert_eq!(h.status(), "Redone.");
    assert!(h.rows()[0].starts_with("1. Click \"New assignment\""));
}

#[test]
fn control_shift_delete_deletes_the_step_and_undo_brings_it_back() {
    let mut h = Harness::new(Screen::Editor);
    let before = h.rows();
    h.key(Key::Delete, Modifiers::COMMAND | Modifiers::SHIFT);
    assert!(h.status().starts_with("Deleted 1. Click \"Assignments\". Undo brings it back"), "{}", h.status());
    assert_eq!(h.rows().len(), before.len() - 1);
    h.invoke(Role::Button, "Undo");
    assert_eq!(h.rows(), before);
}

#[test]
fn f2_puts_the_keyboard_in_the_title_and_typing_then_leaving_keeps_the_words_as_one_undo() {
    let mut h = Harness::new(Screen::Editor);
    h.key(Key::F2, NO_MODS);
    assert_eq!(h.focused_words().as_deref(), Some("Title"));
    h.key(Key::A, ctrl());
    h.type_text("Open the Assignments page");
    h.key(Key::Enter, NO_MODS);
    assert!(h.has(Role::ListBoxOption, "1. Open the Assignments page"), "{:?}", h.rows());
    h.invoke(Role::Button, "Undo");
    assert!(h.has(Role::ListBoxOption, "1. Click \"Assignments\""), "{:?}", h.rows());
    assert!(h.node(h.find(Role::Button, "Undo")).is_disabled(), "the whole edit was one change");
}

#[test]
fn control_z_in_a_text_box_belongs_to_the_box_and_does_not_undo_the_guide() {
    let mut h = Harness::new(Screen::Editor);
    h.key(Key::ArrowDown, Modifiers::ALT);
    let before = h.rows();
    h.focus_on(Role::TextInput, "Title");
    h.key(Key::Z, ctrl());
    assert_eq!(h.rows(), before, "the guide was not undone from inside a text box");
}

#[test]
fn the_check_boxes_change_the_step_and_say_so() {
    let mut h = Harness::new(Screen::Editor);
    h.invoke(Role::CheckBox, "Skip in export");
    assert!(h.status().contains("left out of exports"), "{}", h.status());
    assert!(h.rows()[0].ends_with("(skipped)"), "{:?}", h.rows());
    h.invoke(Role::CheckBox, "Focused view");
    assert_eq!(h.status(), "Focused view is off for this step.");
    assert_eq!(h.node(h.find(Role::CheckBox, "Focused view")).toggled(), Some(accesskit::Toggled::False));
}

#[test]
fn a_note_has_no_picture_and_the_focused_view_box_is_unavailable() {
    let mut h = Harness::new(Screen::Editor);
    h.invoke(Role::ListBoxOption, "Note: Tip");
    assert!(h.node(h.find(Role::CheckBox, "Focused view")).is_disabled());
    assert!(
        !h.has(Role::MultilineTextInput, "Picture description, for people who cannot see the picture"),
        "no picture, no description of it"
    );
    assert!(h.order.iter().all(|id| h.nodes[id].role() != Role::Image));
}

#[test]
fn adding_a_step_puts_the_keyboard_in_its_title() {
    let mut h = Harness::new(Screen::Editor);
    h.invoke(Role::Button, "Add a step");
    assert_eq!(h.status(), "Added an empty step. Type its title.");
    assert_eq!(h.focused_words().as_deref(), Some("Title"));
    assert!(
        h.has(Role::ListBoxOption, "8. New step") || h.rows().iter().any(|r| r.contains("New step")),
        "{:?}",
        h.rows()
    );
}

#[test]
fn back_to_guides_returns_home_and_keeps_the_guide_for_next_time() {
    let mut h = Harness::new(Screen::Editor);
    h.key(Key::ArrowDown, Modifiers::ALT);
    h.invoke(Role::Button, "Back to guides");
    assert_eq!(h.app.screen, Screen::Home);
    assert_eq!(h.status(), "Back at your guides.");
    h.invoke(Role::Button, "How to submit an assignment, 5 steps, example guide");
    assert!(h.rows()[0].starts_with("1. Click \"New assignment\""), "the change is still there: {:?}", h.rows());
}

#[test]
fn the_status_line_is_a_polite_live_region_and_a_repeated_message_is_heard_again() {
    let mut h = Harness::new(Screen::Home);
    let status = h.status_node();
    assert_eq!(h.node(status).live(), Some(Live::Polite));
    h.app.status.say("Saved.");
    h.settle();
    let first = h.node(h.status_node()).label().unwrap().to_string();
    h.app.status.say("Saved.");
    h.settle();
    let second = h.node(h.status_node()).label().unwrap().to_string();
    assert_ne!(first, second, "the same words said twice must still change the node, or the second is not announced");
    assert_eq!(first.trim_end(), second.trim_end());
}

#[test]
fn tab_visits_every_control_on_home_in_order_and_does_not_get_stuck() {
    let mut h = Harness::new(Screen::Home);
    let mut seen = Vec::new();
    for _ in 0..8 {
        h.key(Key::Tab, NO_MODS);
        seen.push(h.focused_words().unwrap_or_default());
    }
    assert_eq!(seen[0], "Start capturing");
    assert_eq!(seen[1], "Search");
    assert_eq!(seen[2], "How to submit an assignment, 5 steps, example guide");
    assert!(seen[3..].contains(&"Start capturing".to_string()), "Tab comes back round to the start: {seen:?}");
}

#[test]
fn tab_visits_every_control_in_the_editor_and_comes_back_round() {
    let mut h = Harness::new(Screen::Editor);
    let mut seen = Vec::new();
    for _ in 0..24 {
        h.key(Key::Tab, NO_MODS);
        seen.push(h.focused_words().unwrap_or_default());
    }
    for must in ["Back to guides", "Title", "Description", "Focused view", "Skip in export", "Add a step"] {
        assert!(seen.iter().any(|w| w == must), "Tab never reached {must:?}: {seen:?}");
    }
    assert!(seen.iter().any(|w| w.starts_with("1. Click")), "the steps list is reachable: {seen:?}");
    let first = seen[0].clone();
    assert!(seen[1..].contains(&first), "Tab goes all the way round: {seen:?}");
}

#[test]
fn the_window_works_at_the_smallest_size_and_at_200_percent_in_both_colour_schemes() {
    for (size, scale) in
        [(egui::vec2(900.0, 560.0), 1.0), (egui::vec2(1280.0, 720.0), 2.0), (egui::vec2(900.0, 560.0), 2.0)]
    {
        for dark in [true, false] {
            for screen in [Screen::Home, Screen::Editor] {
                let h = Harness::sized(screen, size, scale, dark);
                let window = Rect::from_min_size(Pos2::ZERO, size / scale * scale);
                let _ = window;
                // Every control that is not scrolled out of a list is inside the window.
                let area = Rect::from_min_size(Pos2::ZERO, size);
                let fixed: &[(Role, &str)] = match screen {
                    Screen::Home => &[
                        (Role::Button, "Start capturing"),
                        (Role::TextInput, "Search"),
                        (Role::Button, "How to submit an assignment, 5 steps, example guide"),
                    ],
                    Screen::Bar => &[(Role::Button, "Start"), (Role::Button, "Cancel")],
                    Screen::Export => &[(Role::Button, "Back to the guide")],
                    Screen::Editor => &[
                        (Role::Button, "Back to guides"),
                        (Role::Button, "Undo"),
                        (Role::Button, "Redo"),
                        (Role::Button, "Add a step"),
                        (Role::ListBoxOption, "1. Click \"Assignments\""),
                    ],
                };
                for (role, words) in fixed {
                    let id = h.find(*role, words);
                    let b = h.node(id).bounds().expect("a control has a place");
                    let inside = b.x0 >= -1.0
                        && b.y0 >= -1.0
                        && b.x1 <= f64::from(area.max.x) + 1.0
                        && b.y1 <= f64::from(area.max.y) + 1.0;
                    assert!(
                        inside,
                        "{screen:?} {size:?} x{scale} dark={dark}: {words:?} is at {b:?}, outside the window"
                    );
                }
                // The details are in a scrolling panel, so a control can be below the edge, but each is there and has a place.
                if screen == Screen::Editor {
                    for (role, words) in [
                        (Role::TextInput, "Title"),
                        (Role::MultilineTextInput, "Description"),
                        (Role::CheckBox, "Focused view"),
                        (Role::CheckBox, "Skip in export"),
                    ] {
                        assert!(h.node(h.find(role, words)).bounds().is_some());
                    }
                }
            }
        }
    }
}

#[test]
fn the_dev_switches_are_read_and_bad_ones_are_refused_in_plain_words() {
    use super::DevOptions;
    let words = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let ok = DevOptions::parse(&words(&[
        "--screen",
        "editor",
        "--theme",
        "light",
        "--screenshot",
        "a.png",
        "--close-after",
        "2.5",
    ]))
    .unwrap();
    assert_eq!(ok.screen, Some(Screen::Editor));
    assert_eq!(ok.theme, Some(egui::Theme::Light));
    assert_eq!(ok.screenshot.as_deref(), Some(std::path::Path::new("a.png")));
    assert_eq!(ok.close_after, Some(std::time::Duration::from_millis(2500)));
    for bad in [
        &["--screen", "nowhere"][..],
        &["--theme", "purple"],
        &["--close-after", "soon"],
        &["--close-after", "-1"],
        &["--screenshot"],
        &["--bogus"],
    ] {
        let error = DevOptions::parse(&words(bad)).unwrap_err();
        assert!(error.ends_with('.'), "{error}");
    }
}

#[test]
fn the_editor_is_read_in_a_natural_order_title_bar_steps_picture_details_then_the_status_line() {
    let h = Harness::new(Screen::Editor);
    let at = |role: Role, words: &str| h.order.iter().position(|id| *id == h.find(role, words)).unwrap();
    let back = at(Role::Button, "Back to guides");
    let undo = at(Role::Button, "Undo");
    let redo = at(Role::Button, "Redo");
    let title = at(Role::Heading, "How to submit an assignment");
    let steps = at(Role::Heading, "Steps");
    let first_row = at(Role::ListBoxOption, "1. Click \"Assignments\"");
    let add = at(Role::Button, "Add a step");
    let picture = at(Role::Image, "The course site with the Assignments item in the left menu circled");
    let details = at(Role::Heading, "Step details");
    let title_box = at(Role::TextInput, "Title");
    let status = h.order.iter().position(|id| *id == h.status_node()).unwrap();
    let order = [back, undo, redo, title, steps, first_row, add, picture, details, title_box, status];
    assert!(order.windows(2).all(|w| w[0] < w[1]), "the reading order is wrong: {order:?}\n{}", h.dump());
}

#[test]
fn nothing_the_keyboard_reaches_is_a_control_that_does_nothing() {
    for screen in [Screen::Home, Screen::Editor] {
        let h = Harness::new(screen);
        assert!(
            h.order.iter().all(|id| h.nodes[id].role() != Role::Splitter),
            "{screen:?}: a drag handle has no keyboard use\n{}",
            h.dump()
        );
    }
}

#[test]
fn the_steps_list_is_one_stop_for_the_tab_key_however_long_it_is() {
    let mut h = Harness::new(Screen::Editor);
    let mut stops = Vec::new();
    for _ in 0..6 {
        h.key(Key::Tab, NO_MODS);
        stops.push(h.focused_words().unwrap_or_default());
    }
    let in_the_list = stops
        .iter()
        .filter(|w| {
            w.starts_with("1. ")
                || w.starts_with("2. ")
                || w.starts_with("3. ")
                || w.starts_with("4. ")
                || w.starts_with("Note")
        })
        .count();
    assert_eq!(in_the_list, 1, "the list should be a single stop: {stops:?}");
    assert_eq!(
        stops.iter().position(|w| w == "Add a step"),
        stops.iter().position(|w| w.starts_with("1. ")).map(|p| p + 1),
        "straight from the list to the next control: {stops:?}"
    );
}

// ---- the capture bar, and capturing through a stand-in that takes no pictures

/// What the stand-in capture has been asked, and what it will say next. Shared so the test can steer it.
#[derive(Default)]
struct FakeState {
    phase: Option<BarPhase>,
    steps: usize,
    acts: Vec<BarAction>,
    said: Vec<String>,
    outcome: Option<Outcome>,
}

struct FakeCapture(Rc<RefCell<FakeState>>);

impl CaptureControl for FakeCapture {
    fn view(&self) -> BarView {
        let s = self.0.borrow();
        BarView {
            phase: s.phase.unwrap_or(BarPhase::Capturing),
            steps: s.steps,
            last: None,
            what: "the whole screen".to_string(),
            hotkeys: String::new(),
        }
    }

    fn act(&mut self, action: BarAction) {
        let mut s = self.0.borrow_mut();
        s.acts.push(action);
        match action {
            BarAction::Pause => s.phase = Some(BarPhase::Paused),
            BarAction::Resume => s.phase = Some(BarPhase::Capturing),
            BarAction::Finish => s.phase = Some(BarPhase::Saving),
            _ => {}
        }
    }

    fn poll(&mut self) -> Vec<String> {
        std::mem::take(&mut self.0.borrow_mut().said)
    }

    fn finished(&mut self) -> Option<Outcome> {
        self.0.borrow_mut().outcome.take()
    }
}

fn fake_starter(state: &Rc<RefCell<FakeState>>) -> Starter {
    let state = Rc::clone(state);
    Box::new(move |_| Ok(Box::new(FakeCapture(Rc::clone(&state))) as Box<dyn CaptureControl>))
}

impl Harness {
    /// Give the window a guides folder and a stand-in for capturing.
    fn with_capture(&mut self, library: std::path::PathBuf, starter: Starter) {
        let app = std::mem::replace(&mut self.app, App::new(Screen::Home));
        self.app = app.with_capture(library, starter, "the whole screen", Box::new(|_| false));
        self.settle();
    }
}

fn started_harness() -> (Harness, Rc<RefCell<FakeState>>) {
    let state = Rc::new(RefCell::new(FakeState::default()));
    let mut h = Harness::new(Screen::Home);
    h.with_capture(std::path::PathBuf::new(), fake_starter(&state));
    h.invoke(Role::Button, "Start capturing");
    h.invoke(Role::Button, "Start");
    (h, state)
}

#[test]
fn cancel_on_the_ready_bar_goes_back_to_home_and_captures_nothing() {
    let state = Rc::new(RefCell::new(FakeState::default()));
    let mut h = Harness::new(Screen::Home);
    h.with_capture(std::path::PathBuf::new(), fake_starter(&state));
    h.invoke(Role::Button, "Start capturing");
    h.invoke(Role::Button, "Cancel");
    assert_eq!(h.app.screen, Screen::Home);
    assert!(!h.app.is_capturing());
    assert_eq!(h.status(), "Cancelled. Nothing was captured.");
}

#[test]
fn start_begins_capturing_and_the_bar_shows_pause_capture_now_and_finish() {
    let (h, _state) = started_harness();
    assert!(h.app.is_capturing());
    h.find(Role::Heading, "Capturing, step 1");
    h.find(Role::Button, "Pause");
    h.find(Role::Button, "Capture now");
    h.find(Role::Button, "Finish");
    assert!(!h.has(Role::Button, "Start"), "Start is gone once capturing");
    assert!(h.status().starts_with("Capturing."), "{}", h.status());
    assert_eq!(h.focused_words().as_deref(), Some("Pause"), "the keyboard moves to the first button of the new state");
}

#[test]
fn the_bar_buttons_reach_the_capture() {
    let (mut h, state) = started_harness();
    h.invoke(Role::Button, "Pause");
    h.find(Role::Heading, "Paused, 0 steps");
    h.find(Role::Button, "Resume");
    assert_eq!(h.focused_words().as_deref(), Some("Resume"));
    h.invoke(Role::Button, "Resume");
    h.invoke(Role::Button, "Capture now");
    h.invoke(Role::Button, "Finish");
    assert_eq!(state.borrow().acts, [BarAction::Pause, BarAction::Resume, BarAction::CaptureNow, BarAction::Finish]);
    h.find(Role::Heading, "Saving your guide, 0 steps");
}

#[test]
fn each_step_the_capture_makes_is_announced_in_the_status_line() {
    let (mut h, state) = started_harness();
    state.borrow_mut().said = vec!["Step 1 captured: Click \"Save\".".to_string()];
    h.settle();
    assert_eq!(h.status(), "Step 1 captured: Click \"Save\".");
    state.borrow_mut().said =
        vec!["Step 2 captured: Type \"Ann\".".to_string(), "Step 3 captured: Click \"OK\".".to_string()];
    h.settle();
    assert_eq!(h.status(), "Step 2 captured: Type \"Ann\". Step 3 captured: Click \"OK\".", "all of them, none lost");
}

#[test]
fn the_hotkeys_start_pause_resume_and_finish() {
    let state = Rc::new(RefCell::new(FakeState::default()));
    let mut h = Harness::new(Screen::Home);
    h.with_capture(std::path::PathBuf::new(), fake_starter(&state));
    h.app.hotkey(HotkeyAction::PauseOrResume);
    h.app.hotkey(HotkeyAction::CaptureNow);
    assert!(
        state.borrow().acts.is_empty() && !h.app.is_capturing(),
        "pause and capture-now do nothing before a capture"
    );
    h.app.hotkey(HotkeyAction::StartOrFinish);
    h.settle();
    assert!(h.app.is_capturing());
    assert_eq!(h.app.screen, Screen::Bar);
    h.app.hotkey(HotkeyAction::PauseOrResume);
    h.app.hotkey(HotkeyAction::PauseOrResume);
    h.app.hotkey(HotkeyAction::CaptureNow);
    h.app.hotkey(HotkeyAction::StartOrFinish);
    assert_eq!(state.borrow().acts, [BarAction::Pause, BarAction::Resume, BarAction::CaptureNow, BarAction::Finish]);
}

#[test]
fn a_start_hotkey_in_the_editor_does_not_throw_the_guide_away() {
    let state = Rc::new(RefCell::new(FakeState::default()));
    let mut h = Harness::new(Screen::Editor);
    h.with_capture(std::path::PathBuf::new(), fake_starter(&state));
    h.app.hotkey(HotkeyAction::StartOrFinish);
    h.settle();
    assert_eq!(h.app.screen, Screen::Editor);
    assert!(!h.app.is_capturing());
    assert!(h.status().starts_with("Go back to your guides first"), "{}", h.status());
}

#[test]
fn capture_that_cannot_start_says_why_and_stays_on_the_bar() {
    let mut h = Harness::new(Screen::Home);
    h.with_capture(
        std::path::PathBuf::new(),
        Box::new(|_| Err("Windows would not let the program watch the mouse.".to_string())),
    );
    h.invoke(Role::Button, "Start capturing");
    h.invoke(Role::Button, "Start");
    assert_eq!(h.app.screen, Screen::Bar);
    assert!(!h.app.is_capturing());
    assert_eq!(h.status(), "Capturing could not start. Windows would not let the program watch the mouse.");
    h.find(Role::Button, "Start");
}

fn one_captured_step() -> Finished {
    use openstage_express_core::builder::{Draft, DraftKind, ShotId};
    use openstage_express_core::capture::{Captured, MarkOptions, Picture};
    use openstage_express_core::model::{Button, Click, Control};
    let captured = Captured {
        draft: Draft {
            kind: DraftKind::Click,
            title: "Click \"Save\"".to_string(),
            shot: ShotId(1),
            click: Some(Click { x: 8, y: 4, button: Button::Left, extra: Default::default() }),
            control: Some(Control { name: "Save".into(), role: "button".into(), extra: Default::default() }),
            typed: None,
            at: 10,
        },
        picture: Picture::new(16, 9, vec![200; 16 * 9 * 4]).unwrap(),
    };
    let dir = tempfile::tempdir().unwrap();
    let mut writer =
        crate::guide_writer::GuideWriter::create(dir.path(), "Captured guide", "en", MarkOptions::default()).unwrap();
    writer.add(&captured).unwrap();
    let (folder, doc) = writer.into_parts();
    // The folder must outlive the test body: leak the directory handle (the operating system's temp cleanup removes it).
    std::mem::forget(dir);
    Finished { folder, doc }
}

#[test]
fn finishing_opens_the_editor_over_the_new_guide() {
    let (mut h, state) = started_harness();
    state.borrow_mut().outcome = Some(Outcome::Guide(Box::new(one_captured_step())));
    h.settle();
    assert_eq!(h.app.screen, Screen::Editor);
    assert!(!h.app.is_capturing());
    assert_eq!(h.window_mode(), WindowMode::Normal, "the window is the whole program again");
    assert_eq!(h.status(), "Finished. 1 step captured. Your guide is open for editing.");
    h.find(Role::Heading, "Captured guide");
    assert_eq!(h.rows(), ["1. Click \"Save\""]);
}

#[test]
fn finishing_with_nothing_captured_makes_no_guide() {
    let (mut h, state) = started_harness();
    state.borrow_mut().outcome = Some(Outcome::Nothing);
    h.settle();
    assert_eq!(h.app.screen, Screen::Home);
    assert_eq!(h.status(), "Finished. No steps were captured, so no guide was made.");
}

#[test]
fn a_capture_that_fails_goes_home_and_says_why() {
    let (mut h, state) = started_harness();
    state.borrow_mut().outcome = Some(Outcome::Failed("The disk is full.".to_string()));
    h.settle();
    assert_eq!(h.app.screen, Screen::Home);
    assert_eq!(h.status(), "Capturing stopped. The disk is full.");
}

#[test]
fn a_real_guide_in_the_folder_is_listed_opened_and_every_change_is_saved() {
    let finished = one_captured_step();
    let folder_path = finished.folder.path().to_path_buf();
    let library = folder_path.parent().unwrap().to_path_buf();
    drop(finished); // lets go of the guide's lock, as closing it would
    let mut h = Harness::new(Screen::Home);
    h.with_capture(library, super::capture::cannot_capture());
    h.find(Role::Button, "Captured guide, 1 step");
    assert!(
        !h.has(Role::Button, "How to submit an assignment, 5 steps, example guide"),
        "the example gives way to real guides"
    );
    h.invoke(Role::Button, "Captured guide, 1 step");
    assert_eq!(h.app.screen, Screen::Editor);
    assert_eq!(h.rows(), ["1. Click \"Save\""]);
    h.invoke(Role::CheckBox, "Skip in export");
    h.invoke(Role::Button, "Back to guides");
    assert_eq!(h.app.screen, Screen::Home);
    assert!(h.app.editor.is_none(), "the guide's folder is let go of when the person leaves it");
    let saved = std::fs::read_to_string(folder_path.join("guide.json")).unwrap();
    assert!(saved.contains("\"skipped\": true"), "the change reached the disk: {saved}");
    assert!(!folder_path.join("guide.lock").exists(), "and the lock is gone");
}

impl Harness {
    fn window_mode(&self) -> WindowMode {
        self.app.window_mode()
    }
}

// ---- the Export screen

use std::path::PathBuf;
use std::time::Duration;

/// Let the export thread finish, frame by frame, as the window would.
fn wait_for_export(h: &mut Harness) {
    for _ in 0..400 {
        h.frame();
        if !h.app.is_exporting() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    h.settle();
}

type Opened = Rc<RefCell<Vec<PathBuf>>>;

/// An editor over the example guide with an exports folder in a temporary place, a stand-in for the folder dialog (it answers
/// with `chosen`, or is cancelled when `None`) and a stand-in for opening a folder that records what it was asked to open.
fn export_harness(exports: &std::path::Path, chosen: Option<PathBuf>) -> (Harness, Opened) {
    let opened: Opened = Rc::default();
    let mut h = Harness::new(Screen::Editor);
    let record = Rc::clone(&opened);
    let app = std::mem::replace(&mut h.app, App::new(Screen::Home));
    h.app = app.with_system(
        exports.to_path_buf(),
        false,
        Box::new(move |_| chosen.clone()),
        Box::new(move |folder| record.borrow_mut().push(folder.to_path_buf())),
    );
    h.app.screen = Screen::Editor;
    h.settle();
    (h, opened)
}

#[test]
fn the_editor_has_an_export_button_that_opens_the_export_screen() {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, _) = export_harness(dir.path(), None);
    h.invoke(Role::Button, "Export");
    assert_eq!(h.app.screen, Screen::Export);
    h.find(Role::Heading, "Export");
    h.find(Role::Button, "Back to the guide");
    for format in crate::export_run::Format::ALL {
        h.find(Role::CheckBox, format.name());
    }
    h.find(Role::Button, "Choose folder...");
    h.find(Role::CheckBox, "Open the folder when it is done");
    h.find(Role::Button, "Export 2 formats");
    assert!(h.status().starts_with("Export. Choose the formats"), "{}", h.status());
    // This computer is pretending to have no PDF engine, so PDF cannot be chosen and says why.
    assert!(h.node(h.find(Role::CheckBox, "PDF")).is_disabled());
    h.invoke(Role::Button, "Back to the guide");
    assert_eq!(h.app.screen, Screen::Editor);
}

#[test]
fn choosing_formats_changes_the_button_and_nothing_chosen_cannot_be_exported() {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, _) = export_harness(dir.path(), None);
    h.invoke(Role::Button, "Export");
    h.invoke(Role::CheckBox, "Markdown");
    h.find(Role::Button, "Export 3 formats");
    for name in ["Web page", "Markdown", "Word document"] {
        h.invoke(Role::CheckBox, name);
    }
    assert!(h.node(h.find(Role::Button, "Export")).is_disabled(), "nothing chosen: no export");
    h.find(Role::Label, "Choose at least one format.");
}

#[test]
fn exporting_makes_the_chosen_formats_in_a_new_folder_says_so_and_opens_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, opened) = export_harness(dir.path(), None);
    h.invoke(Role::Button, "Export");
    for name in ["Web page", "Word document"] {
        h.invoke(Role::CheckBox, name);
    }
    h.invoke(Role::CheckBox, "Markdown");
    h.invoke(Role::CheckBox, "JSON data");
    h.invoke(Role::Button, "Export 2 formats");
    assert!(h.status().starts_with("Exporting 2 formats."), "{}", h.status());
    wait_for_export(&mut h);
    assert!(h.status().starts_with("Exported 2 formats to "), "{}", h.status());
    let runs: Vec<_> = std::fs::read_dir(dir.path().join("How to submit an assignment")).unwrap().flatten().collect();
    assert_eq!(runs.len(), 1, "one new folder for this run");
    let run = runs[0].path();
    assert!(run.join("markdown/How to submit an assignment.md").is_file());
    assert!(run.join("json/How to submit an assignment.json").is_file());
    assert!(!run.join("html").exists());
    assert_eq!(*opened.borrow(), [run], "the new folder was shown to the person");
}

#[test]
fn a_second_export_goes_into_another_new_folder_and_does_not_open_it_when_that_is_off() {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, opened) = export_harness(dir.path(), None);
    h.invoke(Role::Button, "Export");
    h.invoke(Role::CheckBox, "Open the folder when it is done");
    for name in ["Word document", "Web page"] {
        h.invoke(Role::CheckBox, name);
    }
    h.invoke(Role::CheckBox, "JSON data");
    h.invoke(Role::Button, "Export 1 format");
    wait_for_export(&mut h);
    std::thread::sleep(Duration::from_millis(1100)); // run folders are named by the second
    h.invoke(Role::Button, "Export 1 format");
    wait_for_export(&mut h);
    let runs = std::fs::read_dir(dir.path().join("How to submit an assignment")).unwrap().count();
    assert_eq!(runs, 2, "each export has a folder of its own");
    assert!(opened.borrow().is_empty());
}

#[test]
fn choosing_another_folder_changes_where_exports_go_and_cancelling_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = dir.path().join("elsewhere");
    let (mut h, _) = export_harness(dir.path(), Some(elsewhere.clone()));
    h.invoke(Role::Button, "Export");
    h.invoke(Role::Button, "Choose folder...");
    assert_eq!(h.app.export.folder, elsewhere);
    assert!(h.status().starts_with("Exports will go into "), "{}", h.status());
    h.find(Role::Label, &format!("Export folder: {}", elsewhere.display()));
    let (mut h2, _) = export_harness(dir.path(), None);
    h2.invoke(Role::Button, "Export");
    h2.invoke(Role::Button, "Choose folder...");
    assert_eq!(h2.app.export.folder, dir.path());
}

#[test]
fn a_real_guide_is_exported_from_its_folder_with_its_pictures() {
    let finished = one_captured_step();
    let folder_path = finished.folder.path().to_path_buf();
    let library = folder_path.parent().unwrap().to_path_buf();
    drop(finished);
    let exports = tempfile::tempdir().unwrap();
    let mut h = Harness::new(Screen::Home);
    h.with_capture(library, super::capture::cannot_capture());
    let opened: Opened = Rc::default();
    let app = std::mem::replace(&mut h.app, App::new(Screen::Home));
    h.app = app.with_system(exports.path().to_path_buf(), false, Box::new(|_| None), Box::new(|_| {}));
    h.settle();
    h.invoke(Role::Button, "Captured guide, 1 step");
    h.invoke(Role::Button, "Export");
    h.invoke(Role::CheckBox, "Pictures (PNG)");
    h.invoke(Role::Button, "Export 3 formats");
    wait_for_export(&mut h);
    assert!(h.status().starts_with("Exported 3 formats to "), "{}", h.status());
    let run = std::fs::read_dir(exports.path().join("Captured guide")).unwrap().next().unwrap().unwrap().path();
    let png = std::fs::read(run.join("images/step-01.png")).expect("the picture was read from the guide's folder");
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    assert!(opened.borrow().is_empty());
}

#[test]
fn a_guide_with_no_steps_cannot_be_exported() {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, _) = export_harness(dir.path(), None);
    h.invoke(Role::Button, "Export");
    // Delete every step of the example guide in the editor.
    h.invoke(Role::Button, "Back to the guide");
    for _ in 0..8 {
        h.key(Key::Delete, Modifiers::COMMAND | Modifiers::SHIFT);
    }
    assert!(h.rows().is_empty(), "{:?}", h.rows());
    assert!(h.node(h.find(Role::Button, "Export")).is_disabled());
}
