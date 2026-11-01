// SPDX-License-Identifier: Apache-2.0
//! Whole screens run without a window, and read back what a screen reader would be told.
//!
//! A `Harness` runs the program's real drawing code through egui with AccessKit switched on, exactly as the window does, and
//! keeps the accessibility tree each pass produces: every node's role, name, value, state and place. It can then do what a
//! screen reader or a keyboard does (invoke a control by its name, press keys, type) and read the tree again. Nothing here
//! looks at pixels: what is checked is what an assistive technology is given.

use std::collections::HashMap;

use eframe::egui::{self, accesskit, Event, Key, Modifiers, Pos2, RawInput, Rect, Vec2};

use super::app::{App, Screen};
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
    for screen in [Screen::Home, Screen::Editor] {
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
            assert!(inputs >= 3, "{screen:?}: only {inputs} controls");
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
fn start_capturing_says_plainly_that_it_is_not_built_yet() {
    let mut h = Harness::new(Screen::Home);
    h.invoke(Role::Button, "Start capturing");
    assert_eq!(h.app.screen, Screen::Home);
    assert!(h.status().starts_with("Capturing is not built yet."), "{}", h.status());
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
    h.invoke(Role::Button, "Start capturing");
    let first = h.node(h.status_node()).label().unwrap().to_string();
    h.invoke(Role::Button, "Start capturing");
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
