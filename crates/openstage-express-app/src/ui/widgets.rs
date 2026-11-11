// SPDX-License-Identifier: Apache-2.0
//! The few widgets the screens share: buttons and boxes that always show the 3 point focus ring, the status line that a screen
//! reader announces, and the colours of the scheme in use.

use eframe::egui::{self, accesskit, CornerRadius, Response, RichText, Stroke, StrokeKind, Ui};

use super::fonts::bold_family;
use super::icon::{self, UiIcon};
use super::theme::{Palette, DARK, LIGHT};

/// The palette of the scheme the window is using now.
pub fn palette(ui: &Ui) -> Palette {
    if ui.visuals().dark_mode {
        DARK
    } else {
        LIGHT
    }
}

/// Draw the focus ring round `response` if it has the keyboard. Everything a person can reach calls this.
pub fn ring(ui: &Ui, response: &Response) {
    if response.has_focus() {
        let colour = palette(ui).focus;
        ui.painter().rect_stroke(
            response.rect.expand(3.0),
            CornerRadius::same(9),
            Stroke::new(3.0, colour),
            StrokeKind::Outside,
        );
    }
}

/// An ordinary button.
pub fn button(ui: &mut Ui, text: &str) -> Response {
    let response = ui.button(text);
    ring(ui, &response);
    response
}

/// A button with short words on it (`Rename`) whose name for a screen reader says what it is for (`Rename How to sign in`), so that
/// a list of them is not a list of buttons all called Rename.
pub fn button_named(ui: &mut Ui, text: &str, name: &str) -> Response {
    let response = ui.button(text);
    let name = name.to_string();
    ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(name));
    ring(ui, &response);
    response
}

/// An ordinary button that may be switched off. A switched-off button is still read by a screen reader, as unavailable.
pub fn button_enabled(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    let response = ui.add_enabled(enabled, egui::Button::new(text));
    ring(ui, &response);
    response
}

/// The one main button of a screen: indigo, with text that keeps its contrast.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    let p = palette(ui);
    let label = RichText::new(text).color(p.on_accent).family(bold_family());
    let response = ui.add(
        egui::Button::new(label)
            .fill(p.accent_fill)
            .stroke(Stroke::new(1.0, p.accent_fill))
            .min_size(egui::vec2(160.0, 40.0)),
    );
    ring(ui, &response);
    response
}

/// The one main button of a screen, with an icon before its words.
pub fn primary_icon_button(ui: &mut Ui, icon_name: UiIcon, text: &str) -> Response {
    let p = palette(ui);
    let label = RichText::new(text).color(p.on_accent).family(bold_family());
    let button = egui::Button::new((icon::atom(icon_name, 18.0), label))
        .fill(p.accent_fill)
        .stroke(Stroke::new(1.0, p.accent_fill))
        .min_size(egui::vec2(140.0, 40.0));
    let response = ui
        .scope(|ui| {
            // The icon takes the words' colour, so the words' colour on a filled button is set for every state.
            let v = ui.visuals_mut();
            v.widgets.inactive.fg_stroke = Stroke::new(1.5, p.on_accent);
            v.widgets.hovered.fg_stroke = Stroke::new(1.5, p.on_accent);
            v.widgets.active.fg_stroke = Stroke::new(1.5, p.on_accent);
            v.widgets.hovered.weak_bg_fill = p.accent_hover;
            ui.add(button)
        })
        .inner;
    ring(ui, &response);
    response
}

/// A first-level heading in a colour of its own (the capture bar's, on navy).
pub fn heading_in(ui: &mut Ui, text: &str, colour: egui::Color32) -> Response {
    let response = ui.label(RichText::new(text).heading().color(colour));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(accesskit::Role::Heading);
        node.set_level(1);
        node.set_label(text.to_string());
    });
    response
}

/// An ordinary button with an icon before its words (the icon takes the words' colour).
pub fn icon_button(ui: &mut Ui, icon_name: UiIcon, text: &str) -> Response {
    let response = ui.add(egui::Button::new((icon::atom(icon_name, 16.0), text.to_string())));
    ring(ui, &response);
    response
}

/// A button with an icon and short words (`Rename`) whose name for a screen reader says what it is for (`Rename How to sign in`).
pub fn icon_button_named(ui: &mut Ui, icon_name: UiIcon, text: &str, name: &str) -> Response {
    let response = ui.add(egui::Button::new((icon::atom(icon_name, 16.0), text.to_string())));
    let name = name.to_string();
    ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(name));
    ring(ui, &response);
    response
}

/// A white panel with a thin edge and rounded corners, for a group of things on the page (the suite's card).
pub fn card_frame(ui: &Ui) -> egui::Frame {
    let p = palette(ui);
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.divider))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(16))
}

/// One row of a list of choices (the steps list): left-aligned, indented by `indent`, filled in indigo when chosen. It is a
/// list box option with a name and a "selected" state for a screen reader, and it shows the focus ring.
///
/// Like a list box, the list is one stop for the Tab key: only the chosen row can have the keyboard, and while it does it keeps
/// the Up and Down keys for itself (the screen then chooses the row above or below and passes the keyboard to it).
/// `take_keyboard` gives the keyboard to this row now (used right after Up or Down chose it).
pub fn list_row(ui: &mut Ui, label: &str, selected: bool, muted: bool, indent: f32, take_keyboard: bool) -> Response {
    list_row_of_many(ui, label, selected, selected, muted, indent, take_keyboard)
}

/// A row of a list in which several rows can be chosen: every chosen row is marked, but `has_keyboard_stop` is true for the one row
/// the Tab key can reach and the keyboard goes to (the list is still one stop however many rows are chosen).
pub fn list_row_of_many(
    ui: &mut Ui,
    label: &str,
    selected: bool,
    has_keyboard_stop: bool,
    muted: bool,
    indent: f32,
    take_keyboard: bool,
) -> Response {
    let p = palette(ui);
    let sense = if has_keyboard_stop { egui::Sense::click() } else { egui::Sense::CLICK };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), sense);
    if has_keyboard_stop && take_keyboard {
        response.request_focus();
    }
    if response.has_focus() {
        let keep = egui::EventFilter { vertical_arrows: true, ..Default::default() };
        ui.memory_mut(|m| m.set_focus_lock_filter(response.id, keep));
    }
    if ui.is_rect_visible(rect) {
        let fill = if selected {
            p.accent_fill
        } else if response.hovered() {
            p.raised_hover
        } else {
            egui::Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
        let colour = if selected {
            p.on_accent
        } else if muted {
            p.muted
        } else {
            p.text
        };
        let font = egui::TextStyle::Body.resolve(ui.style());
        let clip = rect.shrink2(egui::vec2(8.0, 0.0));
        ui.painter().with_clip_rect(clip).text(
            egui::pos2(rect.left() + 12.0 + indent, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            font,
            colour,
        );
    }
    let name = label.to_string();
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(accesskit::Role::ListBoxOption);
        node.set_label(name);
        node.set_selected(selected);
        node.add_action(accesskit::Action::Click);
        node.add_action(accesskit::Action::Focus);
    });
    ring(ui, &response);
    response
}

/// A card that opens something (a guide on Home): the title on the left, a few words on the right. A button for a screen reader,
/// named `name`. It is as wide as the room allows, but no wider than `most` (to leave room for buttons beside it).
pub fn card(ui: &mut Ui, title: &str, trailing: &str, name: &str, most: f32) -> Response {
    let p = palette(ui);
    let width = ui.available_width().min(most);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 56.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let (fill, edge) = if response.hovered() { (p.subtle, p.accent) } else { (p.surface, p.border) };
        ui.painter().rect(rect, CornerRadius::same(10), fill, Stroke::new(1.0, edge), StrokeKind::Inside);
        let at = egui::Rect::from_center_size(egui::pos2(rect.left() + 26.0, rect.center().y), egui::vec2(20.0, 20.0));
        icon::paint(ui.painter(), at, UiIcon::BookOpen, p.accent);
        let painter = ui.painter().with_clip_rect(rect.shrink2(egui::vec2(6.0, 0.0)));
        painter.text(
            egui::pos2(rect.left() + 48.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            title,
            egui::FontId::new(18.0, bold_family()),
            p.text,
        );
        painter.text(
            egui::pos2(rect.right() - 16.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            trailing,
            egui::TextStyle::Body.resolve(ui.style()),
            p.muted,
        );
    }
    let name = name.to_string();
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(accesskit::Role::Button);
        node.set_label(name);
        node.add_action(accesskit::Action::Click);
        node.add_action(accesskit::Action::Focus);
    });
    ring(ui, &response);
    response
}

/// A second-level heading (the screen's own title uses `ui.heading`).
pub fn subheading(ui: &mut Ui, text: &str) -> Response {
    let response = ui.label(RichText::new(text).size(19.0).family(bold_family()));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(accesskit::Role::Heading);
        node.set_level(2);
        node.set_label(text.to_string());
    });
    response
}

/// The first-level heading of a screen.
pub fn heading(ui: &mut Ui, text: &str) -> Response {
    let response = ui.heading(text);
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(accesskit::Role::Heading);
        node.set_level(1);
        node.set_label(text.to_string());
    });
    response
}

/// What the status line says, and how many times it has been changed (so the same words said twice are heard twice).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Status {
    pub text: String,
    pub serial: u64,
}

impl Status {
    pub fn say(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.serial += 1;
    }

    pub fn say_opt(&mut self, text: Option<String>) {
        if let Some(text) = text {
            self.say(text);
        }
    }
}

/// The line at the foot of the window. It is a polite live region: a screen reader says each new message when it appears,
/// without taking the keyboard away from where it is.
pub fn status_line(ui: &mut Ui, status: &Status) {
    let p = palette(ui);
    let words = if status.text.is_empty() { "Ready." } else { status.text.as_str() };
    let response = ui.label(RichText::new(words).color(p.muted));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(accesskit::Role::Status);
        node.set_live(accesskit::Live::Polite);
        // A message said twice in a row must still change the node, or the second is not announced: every other message ends in a space.
        node.set_label(if status.serial.is_multiple_of(2) { words.to_string() } else { format!("{words} ") });
    });
}
