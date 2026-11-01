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
            .filter(|words| !words.starts_with("Mark "))
            .collect()
    }

    fn rows(&self) -> Vec<String> {
        self.order
            .iter()
            .filter(|id| self.nodes[id].role() == Role::ListBoxOption)
            .filter_map(|id| self.name(*id))
            .filter(|words| !words.starts_with("Mark "))
            .collect()
    }

    /// The rows of the marks list (not the steps list).
    fn mark_rows(&self) -> Vec<String> {
        self.order
            .iter()
            .filter(|id| self.nodes[id].role() == Role::ListBoxOption)
            .filter_map(|id| self.name(*id))
            .filter(|words| words.starts_with("Mark "))
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
    for screen in [Screen::Home, Screen::Editor, Screen::Bar, Screen::Export, Screen::Settings, Screen::About] {
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
            let wanted = if screen == Screen::About { 1 } else { 2 }; // the About screen is something to read, with one way back
            assert!(inputs >= wanted, "{screen:?}: only {inputs} controls");
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
    assert_eq!(seen[2], "Settings");
    assert_eq!(seen[3], "About and licences");
    assert_eq!(seen[4], "Trash");
    assert_eq!(seen[5], "How to submit an assignment, 5 steps, example guide");
    assert!(seen[6..].contains(&"Start capturing".to_string()), "Tab comes back round to the start: {seen:?}");
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
                    Screen::Settings | Screen::About | Screen::Trash => &[(Role::Button, "Back to guides")],
                    Screen::ChooseWindow => &[(Role::Button, "Back")],
                    Screen::ChooseArea => &[(Role::Button, "Back")],
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
    /// The step count the capture reaches the next time it is polled (a step is counted when the capture is looked at, as the
    /// real one does when the saving thread answers).
    step_to: Option<usize>,
    /// Something went wrong since the capture was last asked.
    trouble: bool,
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
            target: crate::target::TargetKind::Screen,
            can_start: false,
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
        let mut s = self.0.borrow_mut();
        if let Some(n) = s.step_to.take() {
            s.steps = n;
        }
        std::mem::take(&mut s.said)
    }

    fn finished(&mut self) -> Option<Outcome> {
        self.0.borrow_mut().outcome.take()
    }

    fn trouble(&mut self) -> bool {
        std::mem::take(&mut self.0.borrow_mut().trouble)
    }
}

fn fake_starter(state: &Rc<RefCell<FakeState>>) -> Starter {
    let state = Rc::clone(state);
    Box::new(move |_, _, _| Ok(Box::new(FakeCapture(Rc::clone(&state))) as Box<dyn CaptureControl>))
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
        Box::new(|_, _, _| Err("Windows would not let the program watch the mouse.".to_string())),
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

// ---- the Settings screen

use crate::settings::{AppSettings, Appearance, KeyChoice, ModeChoice};

/// A window that saves its settings in `dir`, with the folder dialog answering `chosen`.
fn settings_harness(dir: &std::path::Path, chosen: Option<PathBuf>) -> Harness {
    let mut h = Harness::new(Screen::Home);
    let app = std::mem::replace(&mut h.app, App::new(Screen::Home));
    h.app = app
        .with_system(dir.join("exports"), false, Box::new(move |_| chosen.clone()), Box::new(|_| {}))
        .with_settings(AppSettings::default(), Some(dir.join("settings.json")), None);
    h.settle();
    h
}

#[test]
fn settings_open_from_home_and_every_choice_has_a_name() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = settings_harness(dir.path(), None);
    h.invoke(Role::Button, "Settings");
    assert_eq!(h.app.screen, Screen::Settings);
    h.find(Role::Heading, "Settings");
    for heading in ["Appearance", "Capturing", "Folders", "Hotkeys"] {
        h.find(Role::Heading, heading);
    }
    for radio in ["Follow Windows", "Light", "Dark", "every time", "English", "German", "French"] {
        h.find(Role::RadioButton, radio);
    }
    for check in [
        "Draw a pointer at each click",
        "Draw a circle round each click",
        "Make a step for what I type (never for a password)",
        "Start new steps with a close-up of the click",
        "Open the folder when an export is done",
    ] {
        h.find(Role::CheckBox, check);
    }
    h.find(Role::Button, "Choose guides folder...");
    h.find(Role::Button, "Choose exports folder...");
    h.invoke(Role::Button, "Back to guides");
    assert_eq!(h.app.screen, Screen::Home);
}

#[test]
fn a_change_is_saved_at_once_and_said_and_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = settings_harness(dir.path(), None);
    h.invoke(Role::Button, "Settings");
    h.invoke(Role::RadioButton, "Dark");
    assert_eq!(h.app.appearance(), Appearance::Dark);
    assert_eq!(h.status(), "Saved.");
    let (back, note) = crate::settings::load(&dir.path().join("settings.json"));
    assert_eq!(back.appearance, Appearance::Dark);
    assert!(note.is_none());
    h.invoke(Role::CheckBox, "Draw a pointer at each click");
    let (back, _) = crate::settings::load(&dir.path().join("settings.json"));
    assert!(!back.show_cursor && back.appearance == Appearance::Dark, "both changes are in the file");
}

#[test]
fn the_key_choices_are_off_until_a_mode_that_uses_a_key_is_chosen() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = settings_harness(dir.path(), None);
    h.invoke(Role::Button, "Settings");
    assert!(h.node(h.find(Role::RadioButton, "Shift")).is_disabled());
    h.invoke(Role::RadioButton, "only when I hold a key while I click");
    assert!(!h.node(h.find(Role::RadioButton, "Shift")).is_disabled());
    h.invoke(Role::RadioButton, "Shift");
    assert_eq!(h.app.settings.capture_mode, ModeChoice::WithKey);
    assert_eq!(h.app.settings.capture_key, KeyChoice::Shift);
}

#[test]
fn what_is_chosen_reaches_the_next_capture() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = settings_harness(dir.path(), None);
    let seen: Rc<RefCell<Option<AppSettings>>> = Rc::default();
    let state = Rc::new(RefCell::new(FakeState::default()));
    let (record, fake) = (Rc::clone(&seen), Rc::clone(&state));
    let app = std::mem::replace(&mut h.app, App::new(Screen::Home));
    h.app = app.with_capture(
        PathBuf::new(),
        Box::new(move |_, settings, _| {
            *record.borrow_mut() = Some(settings.clone());
            Ok(Box::new(FakeCapture(Rc::clone(&fake))) as Box<dyn CaptureControl>)
        }),
        "the whole screen",
        Box::new(|_| false),
    );
    h.settle();
    h.invoke(Role::Button, "Settings");
    h.invoke(Role::RadioButton, "never. Only the capture now hotkey makes a step");
    h.invoke(Role::CheckBox, "Draw a circle round each click");
    h.invoke(Role::RadioButton, "French");
    h.invoke(Role::Button, "Back to guides");
    h.invoke(Role::Button, "Start capturing");
    h.invoke(Role::Button, "Start");
    let got = seen.borrow().clone().expect("the starter was given the settings");
    assert_eq!(got.capture_mode, ModeChoice::HotkeyOnly);
    assert!(!got.mark_clicked_area);
    assert_eq!(got.title_language.code(), "fr");
    assert_eq!(got.builder_settings().mode, openstage_express_core::CaptureMode::HotkeyOnly);
}

#[test]
fn choosing_a_guides_folder_lists_the_guides_there_and_remembers_it() {
    let finished = one_captured_step();
    let guide_folder = finished.folder.path().to_path_buf();
    let library = guide_folder.parent().unwrap().to_path_buf();
    drop(finished);
    let dir = tempfile::tempdir().unwrap();
    let mut h = settings_harness(dir.path(), Some(library.clone()));
    h.find(Role::Button, "How to submit an assignment, 5 steps, example guide");
    h.invoke(Role::Button, "Settings");
    h.invoke(Role::Button, "Choose guides folder...");
    assert!(h.status().starts_with("Guides are now kept in "), "{}", h.status());
    h.find(Role::Label, &format!("Guides folder: {}", library.display()));
    h.invoke(Role::Button, "Back to guides");
    h.find(Role::Button, "Captured guide, 1 step");
    let (back, _) = crate::settings::load(&dir.path().join("settings.json"));
    assert_eq!(back.guides_folder.as_deref(), Some(library.to_str().unwrap()));
}

#[test]
fn choosing_an_exports_folder_is_remembered_and_cancelling_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = dir.path().join("my exports");
    let mut h = settings_harness(dir.path(), Some(elsewhere.clone()));
    h.invoke(Role::Button, "Settings");
    h.invoke(Role::Button, "Choose exports folder...");
    assert_eq!(h.app.export.folder, elsewhere);
    let (back, _) = crate::settings::load(&dir.path().join("settings.json"));
    assert_eq!(back.exports_folder.as_deref(), Some(elsewhere.to_str().unwrap()));
    let mut h2 = settings_harness(dir.path(), None);
    h2.invoke(Role::Button, "Settings");
    let before = h2.app.export.folder.clone();
    h2.invoke(Role::Button, "Choose exports folder...");
    assert_eq!(h2.app.export.folder, before);
}

#[test]
fn the_export_screens_choices_are_remembered_in_the_settings() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = settings_harness(dir.path(), None);
    h.app.screen = Screen::Editor;
    h.app.editor = Some(super::editor_state::EditorState::new(openstage_express_export::demo::demo()));
    h.settle();
    h.invoke(Role::Button, "Export");
    h.invoke(Role::CheckBox, "Markdown");
    h.invoke(Role::CheckBox, "Open the folder when it is done");
    let (back, _) = crate::settings::load(&dir.path().join("settings.json"));
    assert!(!back.open_folder_after_export);
    assert!(back.export_formats.iter().any(|f| f == "markdown"), "{:?}", back.export_formats);
}

#[test]
fn a_note_about_the_settings_file_is_shown_on_the_screen() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(Screen::Home);
    let app = std::mem::replace(&mut h.app, App::new(Screen::Home));
    h.app = app.with_settings(
        AppSettings::default(),
        None,
        Some("The settings file is damaged, so the usual settings are used.".to_string()),
    );
    h.settle();
    h.invoke(Role::Button, "Settings");
    h.find(Role::Label, "The settings file is damaged, so the usual settings are used.");
    drop(dir);
}

// ---- the About screen and the icon

#[test]
fn about_opens_from_home_and_shows_the_licence_and_the_credits() {
    let mut h = Harness::new(Screen::Home);
    h.invoke(Role::Button, "About and licences");
    assert_eq!(h.app.screen, Screen::About);
    h.find(Role::Heading, "About OpenStage Capture Express");
    h.find(Role::Heading, "Licence and notices");
    h.find(Role::Heading, "Programs and libraries this one is built from");
    assert!(h.has(Role::Label, &format!("Version {}.", super::about::version())));
    let text: Vec<String> = h.order.iter().filter_map(|id| h.name(*id)).collect();
    assert!(text.iter().any(|t| t.contains("Apache License, Version 2.0")), "the licence is on the screen");
    assert!(text.iter().any(|t| t.starts_with("Roboto")), "the bundled font is credited");
    assert!(text.iter().any(|t| t.starts_with("egui ")), "the libraries are listed");
    h.invoke(Role::Button, "Back to guides");
    assert_eq!(h.app.screen, Screen::Home);
}

#[test]
fn the_window_has_an_icon_from_the_icon_pack() {
    let icon = super::window_icon().expect("the icon decodes");
    assert_eq!((icon.width, icon.height), (256, 256));
    assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    assert!(icon.rgba.chunks(4).any(|p| p[3] == 255), "it is not blank");
}

// ---- marks

fn the_editor(h: &Harness) -> &super::editor_state::EditorState {
    h.app.editor.as_ref().expect("the editor is open")
}

fn the_selected_mark(h: &Harness) -> openstage_express_core::model::Annotation {
    the_editor(h).selected_mark().expect("a mark is chosen")
}

#[test]
fn the_marks_section_offers_every_tool_and_lists_the_marks_the_step_has() {
    let h = Harness::new(Screen::Editor);
    h.find(Role::Heading, "Marks");
    for tool in openstage_express_core::marks::Tool::ALL {
        h.find(Role::Button, &format!("Add {}", tool.name()));
    }
    assert_eq!(h.mark_rows(), ["Mark 1: Click marker, at 110, 170", "Mark 2: Pointer, at 110, 170"]);
}

#[test]
fn adding_a_mark_chooses_it_says_so_and_is_one_undo() {
    let mut h = Harness::new(Screen::Editor);
    h.invoke(Role::Button, "Add rectangle");
    assert!(h.status().starts_with("Added a rectangle: Rectangle"), "{}", h.status());
    assert_eq!(h.mark_rows().len(), 3);
    let row = h.mark_rows()[2].clone();
    assert_eq!(h.node(h.find(Role::ListBoxOption, &row)).is_selected(), Some(true));
    assert_eq!(the_selected_mark(&h).kind, openstage_express_core::model::AnnotationKind::Rect);
    h.invoke(Role::Button, "Undo");
    assert_eq!(h.mark_rows().len(), 2, "one undo takes the whole mark away");
    assert!(the_editor(&h).selected_mark().is_none(), "and nothing is chosen that is not there");
}

#[test]
fn the_buttons_move_and_resize_the_chosen_mark_and_each_is_one_undo() {
    let mut h = Harness::new(Screen::Editor);
    h.invoke(Role::Button, "Add rectangle");
    let start = the_selected_mark(&h);
    h.invoke(Role::Button, "Move right");
    h.invoke(Role::Button, "Move down");
    let moved = the_selected_mark(&h);
    assert_eq!((moved.x, moved.y), (start.x + 10, start.y + 10));
    assert!(h.status().starts_with("Moved. Rectangle"), "{}", h.status());
    h.invoke(Role::Button, "Wider");
    h.invoke(Role::Button, "Taller");
    let grown = the_selected_mark(&h);
    assert_eq!((grown.width, grown.height), (Some(start.width.unwrap() + 10), Some(start.height.unwrap() + 10)));
    assert!(h.status().starts_with("Resized. Rectangle"), "{}", h.status());
    h.invoke(Role::Button, "Undo");
    assert_eq!(the_selected_mark(&h).height, start.height, "undo takes back one change at a time");
}

#[test]
fn colour_and_line_width_are_chosen_from_lists_and_applied() {
    let mut h = Harness::new(Screen::Editor);
    h.invoke(Role::Button, "Add arrow");
    h.invoke(Role::RadioButton, "Red");
    h.invoke(Role::RadioButton, "8 pixels");
    let mark = the_selected_mark(&h);
    assert_eq!(mark.stroke.as_deref(), Some("#E5484D"));
    assert_eq!(mark.stroke_width, Some(8));
    assert!(h.status().contains("8 pixels wide"), "{}", h.status());
}

#[test]
fn a_blur_has_no_colour_to_choose_and_a_text_mark_has_a_box_for_its_words() {
    let mut h = Harness::new(Screen::Editor);
    h.invoke(Role::Button, "Add blur");
    assert!(!h.has(Role::RadioButton, "Red"), "a blur has no colour");
    h.invoke(Role::Button, "Add text");
    h.find(Role::TextInput, "Words in the text mark");
    h.invoke(Role::Button, "Add step number");
    h.find(Role::Button, "Number up");
    assert!(h.node(h.find(Role::Button, "Number down")).is_disabled(), "step numbers start at 1");
    h.invoke(Role::Button, "Number up");
    assert_eq!(the_selected_mark(&h).number, Some(2));
}

#[test]
fn deleting_a_mark_by_its_button_or_by_the_delete_key_and_undo_brings_it_back() {
    let mut h = Harness::new(Screen::Editor);
    h.invoke(Role::Button, "Add oval");
    h.invoke(Role::Button, "Delete mark");
    assert!(h.status().starts_with("Deleted: Oval"), "{}", h.status());
    assert_eq!(h.mark_rows().len(), 2);
    h.invoke(Role::Button, "Undo");
    assert_eq!(h.mark_rows().len(), 3);
    // The Delete key, with the keyboard on a row of the marks list.
    let row = h.mark_rows()[2].clone();
    h.invoke(Role::ListBoxOption, &row);
    h.key(Key::Delete, NO_MODS);
    assert_eq!(h.mark_rows().len(), 2, "{:?}", h.mark_rows());
}

#[test]
fn up_and_down_in_the_marks_list_choose_the_mark_above_or_below() {
    let mut h = Harness::new(Screen::Editor);
    let first = h.mark_rows()[0].clone();
    h.invoke(Role::ListBoxOption, &first);
    assert_eq!(the_editor(&h).selected_mark_id(), Some("a1"));
    h.key(Key::ArrowDown, NO_MODS);
    assert_eq!(the_editor(&h).selected_mark_id(), Some("a2"));
    h.key(Key::ArrowUp, NO_MODS);
    assert_eq!(the_editor(&h).selected_mark_id(), Some("a1"));
    assert_eq!(h.focused_words().as_deref(), Some(first.as_str()), "the keyboard goes with the choice");
}

#[test]
fn marks_are_available_only_when_the_step_has_a_picture() {
    let mut h = Harness::new(Screen::Editor);
    h.invoke(Role::Button, "Add a step");
    h.settle();
    h.find(Role::Label, "Marks need a picture.");
    assert!(!h.has(Role::Button, "Add rectangle"));
}

#[test]
fn every_control_the_marks_section_shows_has_a_name_and_a_keyboard_route() {
    let mut h = Harness::new(Screen::Editor);
    h.invoke(Role::Button, "Add arrow");
    for id in &h.order {
        let node = h.node(*id);
        if egui::accessibility::is_input(node.role()) {
            assert!(h.name(*id).is_some_and(|w| !w.trim().is_empty()), "unnamed {:?}\n{}", node.role(), h.dump());
            assert!(node.supports_action(Action::Focus) || node.is_disabled(), "{:?} {:?}", node.role(), h.name(*id));
        }
    }
}

#[test]
fn dragging_a_mark_with_the_mouse_moves_it_by_what_the_pointer_moved_and_is_one_undo() {
    let mut h = Harness::new(Screen::Editor);
    h.invoke(Role::Button, "Add rectangle");
    let before = the_selected_mark(&h);
    let image =
        h.order.iter().copied().find(|id| h.node(*id).role() == Role::Image).expect("the picture is in the tree");
    let b = h.node(image).bounds().expect("the picture has a place");
    let (picture_w, _) = the_editor(&h).picture_size().unwrap();
    let scale = (b.x1 - b.x0) as f32 / picture_w as f32;
    let at = |x: i32, y: i32| Pos2::new(b.x0 as f32 + x as f32 * scale, b.y0 as f32 + y as f32 * scale);
    let (cx, cy) = (before.x + before.width.unwrap() / 2, before.y + before.height.unwrap() / 2);
    let button = |pos: Pos2, pressed: bool| Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: NO_MODS,
    };
    h.events.push(Event::PointerMoved(at(cx, cy)));
    h.frame();
    h.events.push(button(at(cx, cy), true));
    h.frame();
    h.events.push(Event::PointerMoved(at(cx + 80, cy + 40)));
    h.frame();
    h.frame();
    assert!(the_editor(&h).is_dragging());
    let shown = the_selected_mark(&h);
    assert!(shown.x > before.x + 60, "the mark follows the pointer while it is down: {} to {}", before.x, shown.x);
    h.events.push(button(at(cx + 80, cy + 40), false));
    h.frame();
    h.settle();
    assert!(!the_editor(&h).is_dragging());
    let after = the_selected_mark(&h);
    assert!(
        (after.x - (before.x + 80)).abs() <= 3 && (after.y - (before.y + 40)).abs() <= 3,
        "{before:?} then {after:?}"
    );
    assert!(h.status().starts_with("Moved. Rectangle"), "{}", h.status());
    h.invoke(Role::Button, "Undo");
    let undone = the_selected_mark(&h);
    assert_eq!((undone.x, undone.y), (before.x, before.y), "the whole drag is one undo");
}

#[test]
fn a_click_on_empty_picture_with_the_mouse_chooses_no_mark() {
    let mut h = Harness::new(Screen::Editor);
    h.invoke(Role::Button, "Add rectangle");
    let image = h.order.iter().copied().find(|id| h.node(*id).role() == Role::Image).unwrap();
    let b = h.node(image).bounds().unwrap();
    let corner = Pos2::new(b.x1 as f32 - 3.0, b.y1 as f32 - 3.0);
    h.events.push(Event::PointerMoved(corner));
    h.frame();
    h.events.push(Event::PointerButton {
        pos: corner,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: NO_MODS,
    });
    h.frame();
    h.events.push(Event::PointerButton {
        pos: corner,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: NO_MODS,
    });
    h.frame();
    h.settle();
    assert!(the_editor(&h).selected_mark().is_none());
}

#[test]
fn the_guide_is_renamed_in_the_editor_and_home_lists_it_by_the_new_name() {
    let finished = one_captured_step();
    let folder_path = finished.folder.path().to_path_buf();
    let library = folder_path.parent().unwrap().to_path_buf();
    drop(finished);
    let mut h = Harness::new(Screen::Home);
    h.with_capture(library, super::capture::cannot_capture());
    h.invoke(Role::Button, "Captured guide, 1 step");
    h.focus_on(Role::TextInput, "Guide title");
    h.key(Key::A, ctrl());
    h.type_text("Saving a file");
    h.key(Key::Tab, NO_MODS);
    assert!(h.status().starts_with("The guide is now called Saving a file."), "{}", h.status());
    h.find(Role::Heading, "Saving a file");
    h.invoke(Role::Button, "Back to guides");
    h.find(Role::Button, "Saving a file, 1 step");
    let saved = std::fs::read_to_string(folder_path.join("guide.json")).unwrap();
    assert!(saved.contains("\"title\": \"Saving a file\""), "{saved}");
}

#[test]
fn duplicate_step_makes_a_copy_and_says_so() {
    let mut h = Harness::new(Screen::Editor);
    let before = h.rows().len();
    h.invoke(Role::Button, "Duplicate step");
    assert_eq!(h.rows().len(), before + 1);
    assert!(h.status().starts_with("Duplicated. The copy is"), "{}", h.status());
}

// ---- choosing what to capture: the screen, one window, or an area

use super::capture::{Desktop, DisplayRow, WindowRow};
use crate::target::TargetKind;
use openstage_express_core::capture::Picture;

/// A desktop of made-up windows and displays that takes no pictures. `snapshots` records which displays were looked at.
struct FakeDesktop {
    windows: Vec<WindowRow>,
    displays: Vec<DisplayRow>,
    snapshots: Rc<RefCell<Vec<u32>>>,
}

fn window_row(handle: isize, title: &str, process: &str) -> WindowRow {
    WindowRow {
        handle,
        title: title.into(),
        process: process.into(),
        words: format!("{process}: {title} (800 by 600)"),
    }
}

fn display_row(number: u32) -> DisplayRow {
    DisplayRow { number, width: 400, height: 300, words: format!("Display {number}, 400 by 300") }
}

impl Desktop for FakeDesktop {
    fn windows(&self) -> Result<Vec<WindowRow>, String> {
        Ok(self.windows.clone())
    }

    fn displays(&self) -> Result<Vec<DisplayRow>, String> {
        Ok(self.displays.clone())
    }

    fn snapshot(&self, display: u32) -> Result<Picture, String> {
        self.snapshots.borrow_mut().push(display);
        let shade = display as u8 * 40;
        Picture::new(400, 300, vec![shade; 400 * 300 * 4]).ok_or_else(|| "no picture".to_string())
    }
}

type Asked = Rc<RefCell<Vec<crate::target::Target>>>;
type Looked = Rc<RefCell<Vec<u32>>>;

/// A window on the capture bar with a made-up desktop, settings saved in `dir`, and a starter that records what it was asked to
/// capture.
fn choosing_harness(dir: &std::path::Path, windows: Vec<WindowRow>, displays: u32) -> (Harness, Asked, Looked) {
    let asked: Asked = Rc::default();
    let looked: Looked = Rc::default();
    let state = Rc::new(RefCell::new(FakeState::default()));
    let (record, fake) = (Rc::clone(&asked), Rc::clone(&state));
    let mut h = Harness::new(Screen::Home);
    let app = std::mem::replace(&mut h.app, App::new(Screen::Home));
    h.app = app
        .with_capture(
            PathBuf::new(),
            Box::new(move |_, _, target| {
                record.borrow_mut().push(target.clone());
                Ok(Box::new(FakeCapture(Rc::clone(&fake))) as Box<dyn CaptureControl>)
            }),
            "the whole screen",
            Box::new(|_| false),
        )
        .with_desktop(Box::new(FakeDesktop {
            windows,
            displays: (1..=displays).map(display_row).collect(),
            snapshots: Rc::clone(&looked),
        }))
        .with_settings(AppSettings::default(), Some(dir.join("settings.json")), None);
    h.settle();
    h.invoke(Role::Button, "Start capturing");
    (h, asked, looked)
}

#[test]
fn the_ready_bar_offers_the_screen_a_window_or_an_area() {
    let dir = tempfile::tempdir().unwrap();
    let (h, _, _) = choosing_harness(dir.path(), vec![], 1);
    for kind in TargetKind::ALL {
        h.find(Role::RadioButton, kind.name());
    }
    assert_eq!(h.node(h.find(Role::RadioButton, "The whole screen")).toggled(), Some(accesskit::Toggled::True));
    assert!(!h.has(Role::Button, "Choose window..."), "nothing to choose for the whole screen");
    assert!(!h.node(h.find(Role::Button, "Start")).is_disabled());
}

#[test]
fn choosing_one_window_needs_a_window_chosen_before_start_and_remembers_the_kind() {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, asked, _) = choosing_harness(dir.path(), vec![window_row(7, "Untitled", "notepad.exe")], 1);
    h.invoke(Role::RadioButton, "One window");
    assert_eq!(h.app.target.kind, TargetKind::Window);
    h.find(Role::Button, "Choose window...");
    assert!(h.node(h.find(Role::Button, "Start")).is_disabled(), "no window yet");
    h.find(Role::Label, "Captures no window yet. Choose one first.");
    h.app.hotkey(super::capture::HotkeyAction::StartOrFinish);
    h.settle();
    assert_eq!(h.status(), "Choose the window to capture first.");
    assert!(asked.borrow().is_empty(), "nothing was started");
    let (saved, _) = crate::settings::load(&dir.path().join("settings.json"));
    assert_eq!(saved.capture_target, TargetKind::Window, "the kind is remembered");
}

#[test]
fn the_list_of_windows_is_read_out_and_choosing_one_returns_to_the_bar_ready_to_start() {
    let dir = tempfile::tempdir().unwrap();
    let rows = vec![window_row(7, "Untitled", "notepad.exe"), window_row(9, "Inbox", "outlook.exe")];
    let (mut h, asked, _) = choosing_harness(dir.path(), rows, 1);
    h.invoke(Role::RadioButton, "One window");
    h.invoke(Role::Button, "Choose window...");
    assert_eq!(h.app.screen, Screen::ChooseWindow);
    assert_eq!(h.app.window_mode(), WindowMode::Tall, "the strip is made taller for the list");
    h.find(Role::Heading, "Choose a window");
    assert_eq!(h.mark_rows().len(), 0);
    h.find(Role::ListBoxOption, "notepad.exe: Untitled (800 by 600)");
    h.find(Role::ListBoxOption, "outlook.exe: Inbox (800 by 600)");
    assert!(h.status().starts_with("Choose a window. 2 windows are open."), "{}", h.status());
    h.key(Key::ArrowDown, NO_MODS);
    h.invoke(Role::Button, "Use this window");
    assert_eq!(h.app.screen, Screen::Bar);
    assert_eq!(h.app.window_mode(), WindowMode::Bar);
    h.find(Role::Label, "Captures the window \"Inbox\" of outlook.exe.");
    assert!(!h.node(h.find(Role::Button, "Start")).is_disabled());
    h.invoke(Role::Button, "Start");
    let target = asked.borrow()[0].clone();
    assert_eq!(target.kind, TargetKind::Window);
    assert_eq!(target.window.map(|w| w.handle), Some(9), "the capture was asked for the window that was chosen");
}

#[test]
fn an_empty_list_of_windows_says_so_and_back_returns_to_the_bar() {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, _, _) = choosing_harness(dir.path(), vec![], 1);
    h.invoke(Role::RadioButton, "One window");
    h.invoke(Role::Button, "Choose window...");
    h.find(
        Role::Label,
        "No window is open that can be captured. Open the program you want to explain, then refresh the list.",
    );
    assert!(h.node(h.find(Role::Button, "Use this window")).is_disabled());
    h.invoke(Role::Button, "Refresh the list");
    h.invoke(Role::Button, "Back");
    assert_eq!(h.app.screen, Screen::Bar);
}

#[test]
fn drawing_an_area_shows_a_picture_of_the_display_and_the_numbers_can_be_typed() {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, asked, looked) = choosing_harness(dir.path(), vec![], 1);
    h.invoke(Role::RadioButton, "An area I draw");
    assert!(h.node(h.find(Role::Button, "Start")).is_disabled());
    h.invoke(Role::Button, "Draw the area...");
    assert_eq!(h.app.screen, Screen::ChooseArea);
    assert_eq!(h.app.window_mode(), WindowMode::Normal, "the whole window, to see the picture");
    assert_eq!(*looked.borrow(), [1]);
    h.find(Role::Heading, "Draw the area to capture");
    h.find(Role::Label, "The area is the whole display, 400 by 300.");
    h.find(Role::Image, "A picture of the display. The area is the whole display, 400 by 300.");
    // Type the four numbers, as a keyboard user would.
    for (name, value) in
        [("Left, in pixels", "50"), ("Top, in pixels", "40"), ("Width, in pixels", "200"), ("Height, in pixels", "150")]
    {
        h.focus_on(Role::TextInput, name);
        h.key(Key::A, ctrl());
        h.type_text(value);
        h.key(Key::Tab, NO_MODS);
    }
    h.find(Role::Label, "The area is 200 by 150, 50 pixels from the left and 40 from the top.");
    h.invoke(Role::Button, "Use this area");
    assert_eq!(h.app.screen, Screen::Bar);
    h.find(Role::Label, "Captures an area of Display 1: 200 by 150, 50 pixels from the left and 40 from the top.");
    h.invoke(Role::Button, "Start");
    let target = asked.borrow()[0].clone();
    let area = target.area.expect("the area went to the capture");
    assert_eq!((area.display, area.x, area.y, area.width, area.height), (1, 50, 40, 200, 150));
    let (saved, _) = crate::settings::load(&dir.path().join("settings.json"));
    assert_eq!(saved.last_area, Some(area), "the last area is remembered");
}

#[test]
fn numbers_that_cannot_be_used_are_refused_in_words_and_change_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, _, _) = choosing_harness(dir.path(), vec![], 1);
    h.invoke(Role::RadioButton, "An area I draw");
    h.invoke(Role::Button, "Draw the area...");
    h.focus_on(Role::TextInput, "Width, in pixels");
    h.key(Key::A, ctrl());
    h.type_text("900");
    h.key(Key::Tab, NO_MODS);
    h.find(Role::Label, "The width can be at most 400 pixels, because the area starts 0 pixels from the left and the display is 400 pixels wide.");
    h.find(Role::Label, "The area is the whole display, 400 by 300.");
    h.focus_on(Role::TextInput, "Height, in pixels");
    h.key(Key::A, ctrl());
    h.type_text("tall");
    h.key(Key::Tab, NO_MODS);
    h.find(Role::Label, "Each box needs a whole number of pixels, such as 640.");
}

#[test]
fn the_buttons_move_and_resize_the_area_and_the_whole_display_button_resets_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, _, _) = choosing_harness(dir.path(), vec![], 1);
    h.invoke(Role::RadioButton, "An area I draw");
    h.invoke(Role::Button, "Draw the area...");
    h.invoke(Role::Button, "Narrower");
    h.invoke(Role::Button, "Shorter");
    h.find(Role::Label, "The area is 390 by 290, 0 pixels from the left and 0 from the top.");
    h.invoke(Role::Button, "Move right");
    h.invoke(Role::Button, "Move down");
    h.find(Role::Label, "The area is 390 by 290, 10 pixels from the left and 10 from the top.");
    h.invoke(Role::Button, "Use the whole display");
    h.find(Role::Label, "The area is the whole display, 400 by 300.");
}

#[test]
fn a_second_display_can_be_chosen_and_its_picture_is_taken() {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, _, looked) = choosing_harness(dir.path(), vec![], 2);
    h.invoke(Role::RadioButton, "An area I draw");
    h.invoke(Role::Button, "Draw the area...");
    h.find(Role::RadioButton, "Display 1, 400 by 300");
    h.invoke(Role::RadioButton, "Display 2, 400 by 300");
    assert_eq!(*looked.borrow(), [1, 2]);
    h.invoke(Role::Button, "Use this area");
    assert_eq!(h.app.target.area.map(|a| a.display), Some(2));
}

#[test]
fn dragging_on_the_picture_with_the_mouse_draws_a_new_area() {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, _, _) = choosing_harness(dir.path(), vec![], 1);
    h.invoke(Role::RadioButton, "An area I draw");
    h.invoke(Role::Button, "Draw the area...");
    for _ in 0..10 {
        h.invoke(Role::Button, "Narrower"); // the area no longer covers the right of the picture: it is now 300 wide
    }
    let image = h.order.iter().copied().find(|id| h.node(*id).role() == Role::Image).expect("the picture");
    let b = h.node(image).bounds().unwrap();
    let scale = (b.x1 - b.x0) as f32 / 400.0;
    let at = |x: f32, y: f32| Pos2::new(b.x0 as f32 + x * scale, b.y0 as f32 + y * scale);
    let press = |pos: Pos2, down: bool| Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: down,
        modifiers: NO_MODS,
    };
    // Press on empty picture (the strip the area no longer covers), drag, let go.
    h.events.push(Event::PointerMoved(at(360.0, 60.0)));
    h.frame();
    h.events.push(press(at(360.0, 60.0), true));
    h.frame();
    h.events.push(Event::PointerMoved(at(160.0, 200.0)));
    h.frame();
    h.frame();
    h.events.push(press(at(160.0, 200.0), false));
    h.frame();
    h.settle();
    let area = h.app.target.clone();
    let _ = area;
    h.invoke(Role::Button, "Use this area");
    let chosen = h.app.target.area.expect("an area was chosen");
    assert!((chosen.x as i32 - 160).abs() <= 3 && (chosen.y as i32 - 60).abs() <= 3, "{chosen:?}");
    assert!((chosen.width as i32 - 200).abs() <= 3 && (chosen.height as i32 - 140).abs() <= 3, "{chosen:?}");
}

#[test]
fn back_from_drawing_the_area_returns_to_the_bar_without_choosing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut h, _, _) = choosing_harness(dir.path(), vec![], 1);
    h.invoke(Role::RadioButton, "An area I draw");
    h.invoke(Role::Button, "Draw the area...");
    h.invoke(Role::Button, "Back");
    assert_eq!(h.app.screen, Screen::Bar);
    assert!(h.app.target.area.is_none());
}

// ---- changing the hotkeys, and the cue sounds

use openstage_express_core::cues::Cue;
use openstage_express_core::hotkey::{HotkeySet, KeyCombo};

/// Writes down every sound it is asked to play. Nothing is ever played from a test.
struct RecordedCues(Rc<RefCell<Vec<Cue>>>);

impl super::capture::CuePlayer for RecordedCues {
    fn play(&self, cue: Cue) {
        self.0.borrow_mut().push(cue);
    }
}

/// The window, the stand-in capture that steers it, and what the sounds were asked to play.
type SoundHarness = (Harness, Rc<RefCell<FakeState>>, Rc<RefCell<Vec<Cue>>>);

/// A window with a stand-in capture and a recorder for the sounds, with the given sounds turned on.
fn sound_harness(sounds: crate::settings::SoundSettings) -> SoundHarness {
    let state = Rc::new(RefCell::new(FakeState::default()));
    let played: Rc<RefCell<Vec<Cue>>> = Rc::default();
    let mut h = Harness::new(Screen::Home);
    h.with_capture(std::path::PathBuf::new(), fake_starter(&state));
    let app = std::mem::replace(&mut h.app, App::new(Screen::Home));
    h.app = app.with_cues(Box::new(RecordedCues(Rc::clone(&played))));
    h.app.settings.sounds = sounds;
    h.settle();
    (h, state, played)
}

fn all_sounds() -> crate::settings::SoundSettings {
    crate::settings::SoundSettings { start_and_stop: true, step: true, problem: true }
}

/// Type `text` into the box named `name` and leave it with Tab, as a keyboard user would.
fn type_into(h: &mut Harness, name: &str, text: &str) {
    h.focus_on(Role::TextInput, name);
    h.key(Key::A, ctrl());
    h.type_text(text);
    h.key(Key::Tab, NO_MODS);
}

#[test]
fn every_hotkey_has_a_named_box_that_shows_its_keys_and_the_usual_ones_button() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = settings_harness(dir.path(), None);
    h.invoke(Role::Button, "Settings");
    for (name, keys) in
        [("Start or finish", "Ctrl+Shift+F7"), ("Pause or resume", "Ctrl+Shift+F8"), ("Capture now", "Ctrl+Shift+F9")]
    {
        let id = h.find(Role::TextInput, name);
        assert_eq!(h.node(id).value(), Some(keys), "{name}");
    }
    h.find(Role::Button, "Use the usual hotkeys");
    h.find(Role::Heading, "Sounds");
}

#[test]
fn a_hotkey_is_changed_by_typing_it_and_the_change_is_saved_said_and_handed_on() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = settings_harness(dir.path(), None);
    h.invoke(Role::Button, "Settings");
    type_into(&mut h, "Capture now", "alt + shift + k");
    assert_eq!(h.app.settings.hotkeys.capture_now.describe(), "Alt+Shift+K");
    assert_eq!(
        h.app.settings.hotkeys.start_or_finish,
        HotkeySet::default().start_or_finish,
        "the others are left alone"
    );
    assert_eq!(
        h.status(),
        "Saved. Hotkeys: Ctrl+Shift+F7 start or finish, Ctrl+Shift+F8 pause or resume, Alt+Shift+K capture now."
    );
    let id = h.find(Role::TextInput, "Capture now");
    assert_eq!(h.node(id).value(), Some("Alt+Shift+K"), "what was typed is tidied into the usual way of writing it");
    // Whoever listens for the keys is told once, and the file has the change.
    let told = h.app.take_hotkey_change().expect("the listener is told");
    assert_eq!(told.capture_now.describe(), "Alt+Shift+K");
    assert!(h.app.take_hotkey_change().is_none(), "only once");
    let (back, _) = crate::settings::load(&dir.path().join("settings.json"));
    assert_eq!(back.hotkeys, told);
    h.find(
        Role::Label,
        "Hotkeys: Ctrl+Shift+F7 start or finish, Ctrl+Shift+F8 pause or resume, Alt+Shift+K capture now.",
    );
}

#[test]
fn a_hotkey_that_is_not_allowed_is_refused_in_words_and_nothing_changes() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = settings_harness(dir.path(), None);
    h.invoke(Role::Button, "Settings");
    type_into(&mut h, "Capture now", "Ctrl+Alt+K");
    assert!(h.status().starts_with("Capture now was not changed."), "{}", h.status());
    assert!(h.status().contains("AltGr"), "{}", h.status());
    assert_eq!(h.app.settings.hotkeys, HotkeySet::default());
    assert!(h.app.take_hotkey_change().is_none(), "the listener is not troubled");
    assert!(!dir.path().join("settings.json").exists(), "nothing was saved");
    // What was typed stays in the box to be fixed, and the reason is on the screen next to it.
    let id = h.find(Role::TextInput, "Capture now");
    assert_eq!(h.node(id).value(), Some("Ctrl+Alt+K"));
    let reason = h.status().replace("Capture now was not changed. ", "");
    assert!(h.has(Role::Label, &reason), "the reason is shown beside the box");
    // A key that cannot be read is also said.
    type_into(&mut h, "Pause or resume", "banana");
    assert!(h.status().starts_with("Pause or resume was not changed."), "{}", h.status());
    assert!(h.status().contains("not a key Express can use"), "{}", h.status());
    // And one that another hotkey already has.
    type_into(&mut h, "Pause or resume", "ctrl+shift+f7");
    assert!(h.status().contains("already the hotkey to start or finish"), "{}", h.status());
    assert_eq!(h.app.settings.hotkeys, HotkeySet::default());
}

#[test]
fn a_good_key_after_a_refused_one_works_and_the_refusal_goes() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = settings_harness(dir.path(), None);
    h.invoke(Role::Button, "Settings");
    type_into(&mut h, "Capture now", "F7");
    let refusal = h.status().replace("Capture now was not changed. ", "");
    assert!(h.has(Role::Label, &refusal));
    type_into(&mut h, "Capture now", "Ctrl+Shift+Q");
    assert_eq!(h.app.settings.hotkeys.capture_now.describe(), "Ctrl+Shift+Q");
    assert!(!h.has(Role::Label, &refusal), "the old refusal is gone");
}

#[test]
fn leaving_a_box_without_changing_it_changes_nothing_and_says_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = settings_harness(dir.path(), None);
    h.invoke(Role::Button, "Settings");
    let before = h.status();
    h.focus_on(Role::TextInput, "Start or finish");
    h.key(Key::Tab, NO_MODS);
    assert!(h.app.take_hotkey_change().is_none());
    assert_eq!(h.status(), before);
    assert!(!dir.path().join("settings.json").exists());
    // Writing the same key in other words is tidied but is not a change.
    type_into(&mut h, "Start or finish", "SHIFT+ctrl+f7");
    assert!(h.app.take_hotkey_change().is_none());
    let id = h.find(Role::TextInput, "Start or finish");
    assert_eq!(h.node(id).value(), Some("Ctrl+Shift+F7"));
}

#[test]
fn the_usual_hotkeys_button_puts_every_box_and_key_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = settings_harness(dir.path(), None);
    h.invoke(Role::Button, "Settings");
    type_into(&mut h, "Capture now", "Alt+Shift+K");
    type_into(&mut h, "Start or finish", "Ctrl+Shift+F1");
    let _ = h.app.take_hotkey_change();
    // A refused try is left in a box too.
    type_into(&mut h, "Pause or resume", "q");
    h.invoke(Role::Button, "Use the usual hotkeys");
    assert_eq!(h.app.settings.hotkeys, HotkeySet::default());
    assert_eq!(h.app.take_hotkey_change(), Some(HotkeySet::default()));
    for (name, keys) in
        [("Start or finish", "Ctrl+Shift+F7"), ("Pause or resume", "Ctrl+Shift+F8"), ("Capture now", "Ctrl+Shift+F9")]
    {
        h.settle();
        let id = h.find(Role::TextInput, name);
        assert_eq!(h.node(id).value(), Some(keys), "{name}");
    }
    assert!(h.status().starts_with("Saved. Hotkeys: Ctrl+Shift+F7"), "{}", h.status());
    // With the usual keys already in use the button still clears a refused try, without troubling the listener.
    type_into(&mut h, "Capture now", "x");
    h.invoke(Role::Button, "Use the usual hotkeys");
    h.settle();
    assert!(h.app.take_hotkey_change().is_none());
    let id = h.find(Role::TextInput, "Capture now");
    assert_eq!(h.node(id).value(), Some("Ctrl+Shift+F9"));
}

#[test]
fn the_bar_names_the_hotkeys_that_are_set_now_and_leaves_out_one_that_could_not_be_set() {
    let (mut h, _state, _) = sound_harness(Default::default());
    h.invoke(Role::Button, "Start capturing");
    h.invoke(Role::Button, "Start");
    let chosen = KeyCombo::parse("Alt+Shift+K").unwrap();
    let usual = HotkeySet::default().entries();
    h.app.set_hotkeys_working(&[usual[0], (HotkeyAction::CaptureNow, chosen)]);
    h.settle();
    h.find(Role::Label, "Hotkeys: Ctrl+Shift+F7 start or finish, Alt+Shift+K capture now.");
    h.app.set_hotkeys_working(&[]);
    h.settle();
    assert!(!h.has(Role::Label, "Hotkeys: Ctrl+Shift+F7 start or finish, Alt+Shift+K capture now."));
}

#[test]
fn every_sound_is_off_until_turned_on_so_nothing_is_played() {
    let (mut h, state, played) = sound_harness(Default::default());
    h.invoke(Role::Button, "Start capturing");
    h.invoke(Role::Button, "Start");
    state.borrow_mut().step_to = Some(1);
    h.settle();
    state.borrow_mut().trouble = true;
    h.settle();
    h.invoke(Role::Button, "Pause");
    h.invoke(Role::Button, "Resume");
    h.invoke(Role::Button, "Finish");
    assert!(played.borrow().is_empty(), "{:?}", played.borrow());
}

#[test]
fn with_the_sounds_on_each_event_plays_its_own() {
    let (mut h, state, played) = sound_harness(all_sounds());
    h.invoke(Role::Button, "Start capturing");
    h.invoke(Role::Button, "Start");
    assert_eq!(*played.borrow(), [Cue::Start], "starting");
    state.borrow_mut().step_to = Some(1);
    h.settle();
    assert_eq!(*played.borrow(), [Cue::Start, Cue::Step], "a step was captured");
    h.settle();
    assert_eq!(played.borrow().len(), 2, "a step is heard once, not every frame");
    state.borrow_mut().trouble = true;
    h.settle();
    assert_eq!(played.borrow().last(), Some(&Cue::Error), "something went wrong");
    h.settle();
    assert_eq!(played.borrow().iter().filter(|c| **c == Cue::Error).count(), 1, "and is heard once");
    h.invoke(Role::Button, "Pause");
    h.invoke(Role::Button, "Resume");
    h.invoke(Role::Button, "Finish");
    assert_eq!(
        *played.borrow(),
        [Cue::Start, Cue::Step, Cue::Error, Cue::Stop, Cue::Start, Cue::Stop],
        "pausing and finishing stop, resuming starts"
    );
}

#[test]
fn the_hotkeys_make_the_same_sounds_as_the_buttons_and_a_key_that_does_nothing_makes_none() {
    let (mut h, _state, played) = sound_harness(all_sounds());
    h.app.hotkey(HotkeyAction::PauseOrResume);
    h.app.hotkey(HotkeyAction::CaptureNow);
    assert!(played.borrow().is_empty(), "before a capture the pause key does nothing, so it is silent");
    h.app.hotkey(HotkeyAction::StartOrFinish);
    h.app.hotkey(HotkeyAction::PauseOrResume);
    h.app.hotkey(HotkeyAction::PauseOrResume);
    h.app.hotkey(HotkeyAction::StartOrFinish);
    assert_eq!(*played.borrow(), [Cue::Start, Cue::Stop, Cue::Start, Cue::Stop]);
}

#[test]
fn each_sound_setting_turns_on_only_its_own_cues() {
    for (sounds, expect) in [
        (crate::settings::SoundSettings { start_and_stop: true, ..Default::default() }, vec![Cue::Start, Cue::Stop]),
        (crate::settings::SoundSettings { step: true, ..Default::default() }, vec![Cue::Step]),
        (crate::settings::SoundSettings { problem: true, ..Default::default() }, vec![Cue::Error]),
    ] {
        let (mut h, state, played) = sound_harness(sounds);
        h.invoke(Role::Button, "Start capturing");
        h.invoke(Role::Button, "Start");
        state.borrow_mut().step_to = Some(1);
        h.settle();
        state.borrow_mut().trouble = true;
        h.settle();
        h.invoke(Role::Button, "Finish");
        let mut got = played.borrow().clone();
        got.dedup();
        assert_eq!(got, expect, "{sounds:?}");
    }
}

#[test]
fn a_capture_that_cannot_start_or_that_fails_plays_the_problem_sound() {
    let played: Rc<RefCell<Vec<Cue>>> = Rc::default();
    let mut h = Harness::new(Screen::Home);
    h.with_capture(std::path::PathBuf::new(), Box::new(|_, _, _| Err("No.".to_string())));
    let app = std::mem::replace(&mut h.app, App::new(Screen::Home));
    h.app = app.with_cues(Box::new(RecordedCues(Rc::clone(&played))));
    h.app.settings.sounds.problem = true;
    h.settle();
    h.invoke(Role::Button, "Start capturing");
    h.invoke(Role::Button, "Start");
    assert_eq!(*played.borrow(), [Cue::Error]);

    let (mut h, state, played) = sound_harness(crate::settings::SoundSettings { problem: true, ..Default::default() });
    h.invoke(Role::Button, "Start capturing");
    h.invoke(Role::Button, "Start");
    state.borrow_mut().outcome = Some(Outcome::Failed("The disk is full.".to_string()));
    h.settle();
    assert_eq!(*played.borrow(), [Cue::Error]);
    assert!(h.status().starts_with("Capturing stopped."), "{}", h.status());
}

#[test]
fn the_sound_check_boxes_are_in_settings_and_the_hear_buttons_play_even_when_a_sound_is_off() {
    let dir = tempfile::tempdir().unwrap();
    let played: Rc<RefCell<Vec<Cue>>> = Rc::default();
    let mut h = settings_harness(dir.path(), None);
    let app = std::mem::replace(&mut h.app, App::new(Screen::Home));
    h.app = app.with_cues(Box::new(RecordedCues(Rc::clone(&played))));
    h.invoke(Role::Button, "Settings");
    for check in [
        "When capturing starts or resumes, and when it pauses or finishes",
        "When a step is captured",
        "When something goes wrong or needs a look",
    ] {
        let id = h.find(Role::CheckBox, check);
        assert_eq!(h.node(id).toggled(), Some(accesskit::Toggled::False), "{check} starts off");
    }
    for (cue, words) in [
        (Cue::Start, "Hear the start sound"),
        (Cue::Stop, "Hear the stop sound"),
        (Cue::Step, "Hear the step sound"),
        (Cue::Error, "Hear the problem sound"),
    ] {
        h.invoke(Role::Button, words);
        assert_eq!(played.borrow().last(), Some(&cue), "{words}");
        assert_eq!(h.status(), format!("Played the {} sound.", cue.name()));
    }
    assert_eq!(played.borrow().len(), 4);
    // Turning one on is saved like any setting.
    h.invoke(Role::CheckBox, "When a step is captured");
    assert!(h.app.settings.sounds.step);
    let (back, _) = crate::settings::load(&dir.path().join("settings.json"));
    assert!(back.sounds.step && !back.sounds.start_and_stop && !back.sounds.problem);
}

// ---- drawing a new mark by dragging on the picture

use openstage_express_core::marks::Tool;
use openstage_express_core::model::AnnotationKind;

const SHAPES_HEADING: &str = "Draw a shape with the mouse";

impl Harness {
    /// Every node that is named `words` and can be pressed.
    fn pressable_named(&self, words: &str) -> Vec<NodeId> {
        self.order
            .iter()
            .copied()
            .filter(|id| {
                self.name(*id).is_some_and(|w| w.trim_end() == words) && self.nodes[id].supports_action(Action::Click)
            })
            .collect()
    }

    /// Press the one control named `words`, whatever its role (a heading that folds open, for example).
    fn invoke_named(&mut self, words: &str) {
        let found = self.pressable_named(words);
        assert_eq!(found.len(), 1, "expected one pressable {words:?}, found {}.\n{}", found.len(), self.dump());
        self.events.push(Event::AccessKitActionRequest(ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: found[0],
            data: None,
        }));
        self.settle();
    }
}

/// Where a point of the picture is on the screen, for the mouse.
fn picture_mapper(h: &Harness) -> impl Fn(i32, i32) -> Pos2 {
    let image =
        h.order.iter().copied().find(|id| h.node(*id).role() == Role::Image).expect("the picture is in the tree");
    let b = h.node(image).bounds().expect("the picture has a place");
    let (picture_w, _) = the_editor(h).picture_size().unwrap();
    let scale = (b.x1 - b.x0) as f32 / picture_w as f32;
    move |x, y| Pos2::new(b.x0 as f32 + x as f32 * scale, b.y0 as f32 + y as f32 * scale)
}

fn primary(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: NO_MODS }
}

/// Press at `from` and move to `to`, leaving the button down.
fn press_and_move(h: &mut Harness, from: Pos2, to: Pos2) {
    h.events.push(Event::PointerMoved(from));
    h.frame();
    h.events.push(primary(from, true));
    h.frame();
    h.events.push(Event::PointerMoved(to));
    h.frame();
    h.frame();
}

fn let_go(h: &mut Harness, at: Pos2) {
    h.events.push(primary(at, false));
    h.frame();
    h.settle();
}

/// A window on the editor with the folded choices opened and `tool` chosen.
fn drawing_harness(tool_choice: &str) -> Harness {
    let mut h = Harness::new(Screen::Editor);
    h.invoke_named(SHAPES_HEADING);
    h.invoke(Role::RadioButton, tool_choice);
    h
}

#[test]
fn the_shapes_to_draw_are_folded_behind_one_heading_and_unfold_to_ten_choices() {
    let mut h = Harness::new(Screen::Editor);
    assert!(
        !h.has(Role::RadioButton, "Draw a rectangle"),
        "folded away to begin with, so the keyboard passes one stop"
    );
    h.invoke_named(SHAPES_HEADING);
    let none = h.find(Role::RadioButton, "Do not draw (pick and move marks)");
    assert_eq!(h.node(none).toggled(), Some(accesskit::Toggled::True), "no shape is chosen to begin with");
    for name in [
        "Draw a rectangle",
        "Draw an oval",
        "Draw a line",
        "Draw an arrow",
        "Draw a text",
        "Draw a step number",
        "Draw a highlight",
        "Draw a blur",
        "Draw a spotlight",
    ] {
        let id = h.find(Role::RadioButton, name);
        assert_eq!(h.node(id).toggled(), Some(accesskit::Toggled::False), "{name}");
    }
}

#[test]
fn a_step_without_a_picture_offers_no_drawing() {
    let mut h = Harness::new(Screen::Editor);
    h.invoke(Role::Button, "Add a step");
    h.find(Role::Label, "Marks need a picture.");
    assert!(h.pressable_named(SHAPES_HEADING).is_empty());
}

#[test]
fn choosing_a_shape_says_how_to_use_it_and_choosing_none_turns_it_off() {
    let mut h = drawing_harness("Draw an oval");
    assert_eq!(the_editor(&h).drawing_tool(), Some(Tool::Oval));
    assert_eq!(h.status(), "Drag on the picture to draw an oval. Press Escape to stop.");
    let oval = h.find(Role::RadioButton, "Draw an oval");
    assert_eq!(h.node(oval).toggled(), Some(accesskit::Toggled::True));
    h.invoke(Role::RadioButton, "Draw an arrow");
    assert_eq!(the_editor(&h).drawing_tool(), Some(Tool::Arrow), "choosing another replaces it");
    h.invoke(Role::RadioButton, "Do not draw (pick and move marks)");
    assert_eq!(the_editor(&h).drawing_tool(), None);
    assert_eq!(h.status(), "Drawing with the mouse is off.");
}

#[test]
fn dragging_with_a_shape_chosen_draws_it_is_shown_while_dragging_and_is_one_change_and_one_undo() {
    let mut h = drawing_harness("Draw an oval");
    let before = h.mark_rows().len();
    let at = picture_mapper(&h);
    press_and_move(&mut h, at(200, 150), at(500, 400));
    assert!(the_editor(&h).is_drawing());
    let preview = the_editor(&h).mark_being_drawn().expect("the shape is shown as it will be");
    assert_eq!(preview.kind, AnnotationKind::Oval);
    assert_eq!(h.mark_rows().len(), before, "it is not in the guide until the mouse is let go");
    let_go(&mut h, at(500, 400));
    assert!(!the_editor(&h).is_drawing());
    assert_eq!(h.mark_rows().len(), before + 1);
    let mark = the_selected_mark(&h);
    assert_eq!(mark.kind, AnnotationKind::Oval);
    let near = |a: i32, b: i32| (a - b).abs() <= 3;
    assert!(
        near(mark.x, 200) && near(mark.y, 150) && near(mark.width.unwrap(), 300) && near(mark.height.unwrap(), 250),
        "{mark:?}"
    );
    assert!(h.status().starts_with("Drew an oval: Oval, "), "{}", h.status());
    assert_eq!(the_editor(&h).drawing_tool(), None, "a shape is chosen for one drawing");
    let none = h.find(Role::RadioButton, "Do not draw (pick and move marks)");
    assert_eq!(h.node(none).toggled(), Some(accesskit::Toggled::True));
    h.invoke(Role::Button, "Undo");
    assert_eq!(h.mark_rows().len(), before, "the whole drawing is one undo");
}

#[test]
fn a_press_without_a_drag_places_the_usual_shape_where_it_was_pressed() {
    let mut h = drawing_harness("Draw a rectangle");
    let before = h.mark_rows().len();
    let at = picture_mapper(&h);
    press_and_move(&mut h, at(800, 500), at(800, 500));
    let_go(&mut h, at(800, 500));
    assert_eq!(h.mark_rows().len(), before + 1);
    let mark = the_selected_mark(&h);
    let (cx, cy) = (mark.x + mark.width.unwrap() / 2, mark.y + mark.height.unwrap() / 2);
    assert!((cx - 800).abs() <= 3 && (cy - 500).abs() <= 3, "centred where it was pressed: {mark:?}");
    assert!(h.status().starts_with("Placed a rectangle: Rectangle"), "{}", h.status());
}

#[test]
fn escape_stops_a_drawing_in_progress_and_then_turns_the_shape_off() {
    let mut h = drawing_harness("Draw a line");
    let before = h.mark_rows().len();
    let at = picture_mapper(&h);
    press_and_move(&mut h, at(100, 100), at(600, 300));
    assert!(the_editor(&h).is_drawing());
    h.key(Key::Escape, NO_MODS);
    assert!(!the_editor(&h).is_drawing());
    assert_eq!(the_editor(&h).drawing_tool(), Some(Tool::Line), "the shape is still chosen for another try");
    assert_eq!(h.status(), "Cancelled. Nothing was drawn.");
    let_go(&mut h, at(600, 300));
    assert_eq!(h.mark_rows().len(), before, "letting go after Escape adds nothing");
    h.key(Key::Escape, NO_MODS);
    assert_eq!(the_editor(&h).drawing_tool(), None);
    assert_eq!(h.status(), "Drawing with the mouse is off.");
}

#[test]
fn with_no_shape_chosen_a_drag_on_empty_picture_draws_nothing() {
    let mut h = Harness::new(Screen::Editor);
    let before = h.mark_rows().len();
    let at = picture_mapper(&h);
    press_and_move(&mut h, at(1700, 1000), at(1500, 900));
    assert!(!the_editor(&h).is_drawing());
    let_go(&mut h, at(1500, 900));
    assert_eq!(h.mark_rows().len(), before);
}

#[test]
fn a_dragged_step_number_takes_the_next_number_and_a_drag_outside_the_picture_stays_on_it() {
    let mut h = Harness::new(Screen::Editor);
    h.invoke(Role::Button, "Add step number");
    h.invoke(Role::Button, "Add step number");
    h.invoke_named(SHAPES_HEADING);
    h.invoke(Role::RadioButton, "Draw a step number");
    let at = picture_mapper(&h);
    press_and_move(&mut h, at(300, 300), at(360, 350));
    let_go(&mut h, at(360, 350));
    let mark = the_selected_mark(&h);
    assert_eq!(mark.kind, AnnotationKind::Number);
    assert_eq!(mark.number, Some(3), "after the two there already were");
    assert!(mark.size.is_some_and(|s| (57..=63).contains(&s)), "{mark:?}");
    assert!(h.status().starts_with("Drew a step number: Step number 3"), "{}", h.status());
}

#[test]
fn a_drawing_in_progress_is_dropped_when_another_step_is_chosen_or_a_change_is_undone() {
    let mut e = super::editor_state::EditorState::new(openstage_express_export::demo::demo());
    e.choose_drawing_tool(Some(Tool::Rectangle));
    assert!(e.begin_draw(50, 50));
    e.draw_to(400, 300);
    assert!(e.is_drawing() && e.mark_being_drawn().is_some());
    let marks_before = e.marks().len();
    e.select("step-2");
    assert!(!e.is_drawing(), "another step was chosen");
    assert!(e.mark_being_drawn().is_none());
    assert_eq!(e.drawing_tool(), Some(Tool::Rectangle), "the shape stays chosen");
    assert!(e.begin_draw(10, 10));
    e.undo();
    assert!(!e.is_drawing(), "an undo cannot happen under a drawing");
    e.select("step-1");
    assert_eq!(e.marks().len(), marks_before, "nothing was added by any of it");
    assert!(e.end_draw().is_none(), "nothing is being drawn");
}

#[test]
fn drawing_needs_a_chosen_shape_and_a_picture() {
    let mut e = super::editor_state::EditorState::new(openstage_express_export::demo::demo());
    assert!(!e.begin_draw(10, 10), "no shape is chosen");
    assert!(e.cancel_draw().is_none(), "and so there is nothing to stop");
    assert!(e.choose_drawing_tool(None).is_none(), "choosing none when none is chosen says nothing");
    e.choose_drawing_tool(Some(Tool::Blur));
    assert!(e.choose_drawing_tool(Some(Tool::Blur)).is_none(), "nor does choosing the same shape again");
    assert!(e.begin_draw(10, 10));
    assert_eq!(e.cancel_draw().as_deref(), Some("Cancelled. Nothing was drawn."));
}

#[test]
fn a_shape_being_drawn_is_in_the_picture_until_it_is_cancelled() {
    let mut e = super::editor_state::EditorState::new(openstage_express_export::demo::demo());
    let ctx = egui::Context::default();
    let plain = e.texture(&ctx).expect("the step has a picture");
    assert_eq!(e.texture(&ctx).unwrap().id(), plain.id(), "the picture is kept while nothing changes");
    e.choose_drawing_tool(Some(Tool::Rectangle));
    assert!(e.begin_draw(50, 50));
    e.draw_to(400, 300);
    let during = e.texture(&ctx).unwrap();
    assert_ne!(during.id(), plain.id(), "the shape is drawn into the picture while it is being drawn");
    e.draw_to(500, 350);
    assert_ne!(e.texture(&ctx).unwrap().id(), during.id(), "and follows the pointer");
    assert!(e.cancel_draw().is_some());
    let after = e.texture(&ctx).unwrap();
    assert_ne!(after.id(), during.id(), "the picture is made again without the shape once it is cancelled");
    assert_eq!(e.marks().len(), openstage_express_export::demo::demo().document.guide.steps[0].annotations.len());
}

// ---- renaming and deleting guides from Home, and the Trash

use openstage_express_core::GuideFolder;

/// A guides folder holding one real guide for each title (each the example guide under that title).
fn guides_dir(titles: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for title in titles {
        let mut doc = openstage_express_export::demo::demo().document;
        doc.guide.title = title.to_string();
        drop(GuideFolder::create(dir.path(), &doc).unwrap());
    }
    dir
}

/// Where the window was asked to show a folder.
type Shown = Rc<RefCell<Vec<PathBuf>>>;

/// Home over the guides in `dir`, where the process numbers `running` are other copies of Express that are running.
fn home_over(dir: &std::path::Path, running: &'static [u32]) -> (Harness, Shown) {
    let shown: Shown = Rc::default();
    let record = Rc::clone(&shown);
    let mut h = Harness::new(Screen::Home);
    let app = std::mem::replace(&mut h.app, App::new(Screen::Home));
    h.app = app
        .with_capture(
            dir.to_path_buf(),
            super::capture::cannot_capture(),
            "the whole screen",
            Box::new(move |pid| running.contains(&pid)),
        )
        .with_system(
            dir.join("exports"),
            false,
            Box::new(|_| None),
            Box::new(move |folder| record.borrow_mut().push(folder.to_path_buf())),
        );
    h.settle();
    (h, shown)
}

fn lock_held_by(guide: &std::path::Path, pid: u32) {
    std::fs::write(guide.join("guide.lock"), format!("{{\"pid\": {pid}, \"startedAt\": \"2026-11-01T09:00:00Z\"}}"))
        .unwrap();
}

#[test]
fn a_guide_on_disk_has_rename_and_delete_buttons_named_for_it_and_the_example_has_none() {
    let h = Harness::new(Screen::Home);
    assert!(!h.has(Role::Button, "Rename How to submit an assignment"));
    assert!(!h.has(Role::Button, "Delete How to submit an assignment"), "the example is not a guide on disk");
    let dir = guides_dir(&["Alpha", "Beta"]);
    let (h, _) = home_over(dir.path(), &[]);
    for title in ["Alpha", "Beta"] {
        h.find(Role::Button, &format!("{title}, 5 steps"));
        h.find(Role::Button, &format!("Rename {title}"));
        h.find(Role::Button, &format!("Delete {title}"));
    }
    h.find(Role::Button, "Trash");
}

#[test]
fn renaming_opens_a_box_with_the_name_and_enter_saves_it() {
    let dir = guides_dir(&["Alpha", "Beta"]);
    let (mut h, _) = home_over(dir.path(), &[]);
    h.invoke(Role::Button, "Rename Alpha");
    h.find(Role::Heading, "Rename \"Alpha\"");
    let id = h.find(Role::TextInput, "New name for Alpha");
    assert_eq!(h.node(id).value(), Some("Alpha"));
    assert_eq!(h.focused_words().as_deref(), Some("New name for Alpha"), "the keyboard is in the box");
    assert!(h.status().starts_with("Renaming Alpha."), "{}", h.status());
    h.key(Key::A, ctrl());
    h.type_text("Alpha two");
    h.key(Key::Enter, NO_MODS);
    assert_eq!(h.status(), "Renamed to Alpha two.");
    assert!(!h.has(Role::TextInput, "New name for Alpha"), "the box has gone");
    h.find(Role::Button, "Alpha two, 5 steps");
    assert!(!h.has(Role::Button, "Alpha, 5 steps"));
    h.find(Role::Button, "Rename Alpha two");
    assert!(dir.path().join("Alpha.osguide").is_dir(), "the folder keeps its name, as when the editor renames a guide");
    assert!(dir.path().join("Alpha.osguide").join("guide.json").is_file());
    assert!(!dir.path().join("Alpha.osguide").join("guide.lock").exists());
}

#[test]
fn the_save_button_saves_and_cancel_and_escape_leave_the_name_alone() {
    let dir = guides_dir(&["Alpha"]);
    let (mut h, _) = home_over(dir.path(), &[]);
    h.invoke(Role::Button, "Rename Alpha");
    h.key(Key::A, ctrl());
    h.type_text("Typed but not kept");
    h.invoke(Role::Button, "Cancel");
    assert_eq!(h.status(), "Cancelled. The name was not changed.");
    h.find(Role::Button, "Alpha, 5 steps");
    assert!(!h.has(Role::TextInput, "New name for Alpha"));
    h.invoke(Role::Button, "Rename Alpha");
    h.key(Key::A, ctrl());
    h.type_text("Typed but not kept either");
    h.key(Key::Escape, NO_MODS);
    assert_eq!(h.status(), "Cancelled. The name was not changed.");
    h.find(Role::Button, "Alpha, 5 steps");
    h.invoke(Role::Button, "Rename Alpha");
    h.key(Key::A, ctrl());
    h.type_text("  Saved by the button  ");
    h.invoke(Role::Button, "Save name");
    assert_eq!(h.status(), "Renamed to Saved by the button.");
    h.find(Role::Button, "Saved by the button, 5 steps");
}

#[test]
fn a_name_that_is_empty_is_refused_in_words_and_the_box_stays_to_be_fixed() {
    let dir = guides_dir(&["Alpha"]);
    let (mut h, _) = home_over(dir.path(), &[]);
    h.invoke(Role::Button, "Rename Alpha");
    h.key(Key::A, ctrl());
    h.key(Key::Backspace, NO_MODS);
    h.invoke(Role::Button, "Save name");
    assert!(h.status().starts_with("Alpha was not renamed. A guide needs a name."), "{}", h.status());
    h.find(Role::TextInput, "New name for Alpha");
    h.find(Role::Button, "Alpha, 5 steps");
}

#[test]
fn a_guide_another_copy_has_open_is_not_renamed_or_moved_and_the_reason_is_said() {
    let dir = guides_dir(&["Busy"]);
    lock_held_by(&dir.path().join("Busy.osguide"), 4242);
    let (mut h, _) = home_over(dir.path(), &[4242]);
    h.invoke(Role::Button, "Rename Busy");
    h.key(Key::A, ctrl());
    h.type_text("Other");
    h.invoke(Role::Button, "Save name");
    assert!(
        h.status().contains("Busy was not renamed. It is open in another copy of Express (process 4242)"),
        "{}",
        h.status()
    );
    h.invoke(Role::Button, "Cancel");
    h.invoke(Role::Button, "Delete Busy");
    h.invoke(Role::Button, "Move to the Trash");
    assert!(
        h.status().contains("Busy was not moved. It is open in another copy of Express (process 4242)"),
        "{}",
        h.status()
    );
    h.find(Role::Button, "Busy, 5 steps");
    assert!(dir.path().join("Busy.osguide").join("guide.json").is_file());
    assert!(dir.path().join("Busy.osguide").join("guide.lock").exists(), "the other copy's lock is left alone");
}

#[test]
fn delete_asks_first_with_keep_it_ready_and_then_moves_the_guide_to_the_trash() {
    let dir = guides_dir(&["Alpha", "Beta"]);
    let (mut h, _) = home_over(dir.path(), &[]);
    h.invoke(Role::Button, "Delete Alpha");
    h.find(Role::Heading, "Move \"Alpha\" to the Trash?");
    assert_eq!(h.focused_words().as_deref(), Some("Keep it"), "Enter by accident keeps the guide");
    assert!(h.status().starts_with("Move Alpha to the Trash?"), "{}", h.status());
    assert!(dir.path().join("Alpha.osguide").is_dir(), "nothing has moved yet");
    h.invoke(Role::Button, "Move to the Trash");
    assert_eq!(h.status(), "Moved Alpha to the Trash. Open the Trash to bring it back.");
    assert!(!h.has(Role::Button, "Alpha, 5 steps"));
    h.find(Role::Button, "Beta, 5 steps");
    assert!(!h.has(Role::Heading, "Move \"Alpha\" to the Trash?"), "the question has gone");
    assert!(!dir.path().join("Alpha.osguide").exists());
    assert!(dir.path().join("Trash").join("Alpha.osguide").join("guide.json").is_file(), "all of it is in the Trash");
    assert!(dir.path().join("Trash").join("Alpha.osguide").join("images").is_dir());
}

#[test]
fn keep_it_and_escape_both_leave_the_guide_where_it_is() {
    let dir = guides_dir(&["Alpha"]);
    let (mut h, _) = home_over(dir.path(), &[]);
    h.invoke(Role::Button, "Delete Alpha");
    h.invoke(Role::Button, "Keep it");
    assert_eq!(h.status(), "Kept Alpha.");
    assert!(!h.has(Role::Button, "Move to the Trash"));
    h.invoke(Role::Button, "Delete Alpha");
    h.key(Key::Escape, NO_MODS);
    assert_eq!(h.status(), "Kept Alpha.");
    assert!(dir.path().join("Alpha.osguide").is_dir() && !dir.path().join("Trash").exists());
    // One question at a time: asking to delete drops a rename that was open, and the other way round.
    h.invoke(Role::Button, "Rename Alpha");
    h.invoke(Role::Button, "Delete Alpha");
    assert!(!h.has(Role::TextInput, "New name for Alpha"));
    h.find(Role::Heading, "Move \"Alpha\" to the Trash?");
    h.invoke(Role::Button, "Rename Alpha");
    assert!(!h.has(Role::Button, "Move to the Trash"));
}

#[test]
fn the_trash_lists_what_was_deleted_and_restore_brings_it_back_whole() {
    let dir = guides_dir(&["Alpha", "Beta"]);
    let (mut h, _) = home_over(dir.path(), &[]);
    h.invoke(Role::Button, "Trash");
    assert_eq!(h.app.screen, Screen::Trash);
    h.find(Role::Heading, "Trash");
    h.find(Role::Label, "The Trash is empty.");
    assert_eq!(h.status(), "The Trash is empty.");
    h.invoke(Role::Button, "Back to guides");
    assert_eq!(h.app.screen, Screen::Home);
    h.invoke(Role::Button, "Delete Alpha");
    h.invoke(Role::Button, "Move to the Trash");
    h.invoke(Role::Button, "Trash");
    assert_eq!(h.status(), "The Trash has 1 guide. Restore brings one back.");
    h.find(Role::Label, "Alpha");
    h.find(Role::Button, "Restore Alpha");
    assert!(!h.has(Role::Label, "The Trash is empty."));
    h.invoke(Role::Button, "Restore Alpha");
    assert_eq!(h.status(), "Brought Alpha back to your guides.");
    h.find(Role::Label, "The Trash is empty.");
    assert!(dir.path().join("Alpha.osguide").join("guide.json").is_file());
    assert!(!dir.path().join("Trash").join("Alpha.osguide").exists());
    h.invoke(Role::Button, "Back to guides");
    h.find(Role::Button, "Alpha, 5 steps");
    h.invoke(Role::Button, "Alpha, 5 steps");
    assert_eq!(h.app.screen, Screen::Editor, "and it opens, with its steps");
}

#[test]
fn show_the_trash_folder_opens_it_or_the_guides_folder_until_something_has_been_deleted() {
    let dir = guides_dir(&["Alpha"]);
    let (mut h, shown) = home_over(dir.path(), &[]);
    h.invoke(Role::Button, "Trash");
    h.invoke(Role::Button, "Show the Trash folder");
    assert_eq!(*shown.borrow(), [dir.path().to_path_buf()], "there is no Trash folder yet");
    h.invoke(Role::Button, "Back to guides");
    h.invoke(Role::Button, "Delete Alpha");
    h.invoke(Role::Button, "Move to the Trash");
    h.invoke(Role::Button, "Trash");
    h.invoke(Role::Button, "Show the Trash folder");
    assert_eq!(shown.borrow().last(), Some(&dir.path().join("Trash")));
    assert_eq!(h.status(), "Showing the Trash folder in Explorer.");
}

#[test]
fn deleting_the_last_guide_brings_the_example_back_to_the_list() {
    let dir = guides_dir(&["Only one"]);
    let (mut h, _) = home_over(dir.path(), &[]);
    assert!(!h.app.home.shows_example());
    h.invoke(Role::Button, "Delete Only one");
    h.invoke(Role::Button, "Move to the Trash");
    assert!(h.app.home.shows_example());
    h.find(Role::Button, "How to submit an assignment, 5 steps, example guide");
    assert!(!h.has(Role::Button, "Delete How to submit an assignment"));
}

#[test]
fn the_rename_and_delete_buttons_follow_their_guide_in_the_tab_order() {
    let dir = guides_dir(&["Alpha"]);
    let (mut h, _) = home_over(dir.path(), &[]);
    let mut seen = Vec::new();
    for _ in 0..8 {
        h.key(Key::Tab, NO_MODS);
        seen.push(h.focused_words().unwrap_or_default());
    }
    let card = seen.iter().position(|w| w == "Alpha, 5 steps").expect("the card is reached");
    assert_eq!(seen[card + 1], "Rename Alpha");
    assert_eq!(seen[card + 2], "Delete Alpha");
}

#[test]
fn every_control_on_home_and_the_trash_has_a_name_and_a_keyboard_route() {
    let dir = guides_dir(&["Alpha"]);
    let (mut h, _) = home_over(dir.path(), &[]);
    h.invoke(Role::Button, "Rename Alpha");
    for stage in 0..3 {
        for id in &h.order {
            let node = h.node(*id);
            if egui::accessibility::is_input(node.role()) {
                assert!(h.name(*id).is_some_and(|w| !w.trim().is_empty()), "unnamed {:?}\n{}", node.role(), h.dump());
                assert!(
                    node.supports_action(Action::Focus) || node.is_disabled(),
                    "{:?} {:?}",
                    node.role(),
                    h.name(*id)
                );
            }
        }
        match stage {
            0 => {
                h.invoke(Role::Button, "Cancel");
                h.invoke(Role::Button, "Delete Alpha");
            }
            1 => {
                h.invoke(Role::Button, "Keep it");
                h.invoke(Role::Button, "Delete Alpha");
                h.invoke(Role::Button, "Move to the Trash");
                h.invoke(Role::Button, "Trash");
            }
            _ => {}
        }
    }
}
