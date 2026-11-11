// SPDX-License-Identifier: Apache-2.0
//! The menu bar and the menu buttons (Add, "..."), built the way OpenStage Author's are (the WAI-ARIA menu bar and menu button
//! patterns), so a person using the keyboard or a screen reader meets the same thing in every OpenStage program:
//!
//! * the menu bar is one stop for the Tab key; F10 goes to it from anywhere; Left and Right move along it;
//! * Down, Enter or Space opens a menu with the keyboard on its first item (Up: on its last); Up, Down, Home, End and a letter move
//!   in it; Enter or Space does the item; Escape closes it and gives the keyboard back to its title; Left and Right open the menu
//!   beside it;
//! * a command that cannot be used now stays in its menu, read as unavailable, and does nothing;
//! * after a command, the keyboard goes back to where it was before the menu was opened (or to the menu's title);
//! * a screen reader hears a menu bar, a title that has a menu and whether it is open, a menu named after its title, and items with
//!   their keys and, for one that can be ticked, whether it is.
//!
//! What a menu remembers between frames is kept in egui's memory under the menu's own id, so a menu needs no field in the screens.

use eframe::egui::{
    self, accesskit, Color32, CornerRadius, EventFilter, Id, Key, Modifiers, Rect, Sense, Stroke, StrokeKind, Ui,
};

use super::commands::ItemKind;
use super::icon::{self, UiIcon};
use super::widgets::{palette, ring};

/// One line of a menu.
#[derive(Debug, Clone)]
pub enum Line<C> {
    Item(Entry<C>),
    /// A line between groups.
    Separator,
}

/// One item of a menu.
#[derive(Debug, Clone)]
pub struct Entry<C> {
    pub command: C,
    pub words: String,
    pub icon: Option<UiIcon>,
    pub keys: Option<String>,
    pub enabled: bool,
    pub kind: ItemKind,
    pub ticked: bool,
}

/// One menu of the bar.
#[derive(Debug, Clone)]
pub struct Menu<C> {
    pub title: String,
    pub lines: Vec<Line<C>>,
}

/// What a menu remembers between frames.
#[derive(Debug, Clone, Default)]
struct State {
    /// The open menu (its place on the bar; a menu button has only 0).
    open: Option<usize>,
    /// The item that has the keyboard in the open menu, or is lit under the pointer.
    item: Option<usize>,
    /// The title that the Tab key stops at.
    current: usize,
    /// Give the keyboard to the current title in the next frame.
    focus_title: bool,
    /// Give the keyboard to `item` in the next frame.
    focus_item: bool,
    /// Where the keyboard was before the menu was opened, to give it back.
    came_from: Option<Id>,
}

fn load(ctx: &egui::Context, id: Id) -> State {
    ctx.data(|d| d.get_temp::<State>(id)).unwrap_or_default()
}

fn store(ctx: &egui::Context, id: Id, state: State) {
    ctx.data_mut(|d| d.insert_temp(id, state));
}

/// The id of the window's menu bar, so F10 can send the keyboard to it.
pub fn bar_id() -> Id {
    Id::unique("openstage-menu-bar")
}

/// Open one of the bar's menus before the window is first drawn (for a picture of it: `--menu`).
pub fn open_at_start(ctx: &egui::Context, menu: usize) {
    let state = State { open: Some(menu), current: menu, ..State::default() };
    store(ctx, bar_id(), state);
}

/// Send the keyboard to the menu bar (F10), remembering where it was.
pub fn focus_bar(ctx: &egui::Context) {
    let id = bar_id();
    let mut state = load(ctx, id);
    state.came_from = ctx.memory(|m| m.focused()).filter(|f| !is_ours(ctx, *f));
    state.focus_title = true;
    store(ctx, id, state);
    ctx.request_repaint();
}

/// Whether `focused` is a title or an item of the menu bar (so F10 pressed there does not remember the bar as where to go back to).
fn is_ours(ctx: &egui::Context, focused: Id) -> bool {
    ctx.data(|d| d.get_temp::<Vec<Id>>(bar_id().with("ids"))).is_some_and(|ids| ids.contains(&focused))
}

/// An item of an open menu keeps every key it uses, Tab too (Tab closes the menu).
const KEEP_KEYS: EventFilter = EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true };
/// A title of the bar keeps the arrows and Escape, and lets Tab move the keyboard on (the bar is one stop).
const TITLE_KEYS: EventFilter =
    EventFilter { tab: false, horizontal_arrows: true, vertical_arrows: true, escape: true };

/// The window's menu bar. Returns the command chosen, if one was.
pub fn menu_bar<C: Clone>(ui: &mut Ui, menus: &[Menu<C>]) -> Option<C> {
    let id = bar_id();
    let ctx = ui.ctx().clone();
    let mut state = load(&ctx, id);
    if menus.is_empty() {
        return None;
    }
    state.current = state.current.min(menus.len() - 1);
    let p = palette(ui);
    let mut chosen = None;
    let mut title_rects = Vec::new();
    let mut ids = Vec::new();
    let opened_by_pointer_before = state.open;
    let builder = egui::UiBuilder::new()
        .layout(egui::Layout::left_to_right(egui::Align::Center))
        .accessibility_role(accesskit::Role::MenuBar)
        .accessibility_label("Menus");
    ui.scope_builder(builder, |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (i, menu) in menus.iter().enumerate() {
            let title_id = id.with(("title", i));
            ids.push(title_id);
            let font = egui::FontId::new(15.0, super::fonts::bold_family());
            let galley = ui.painter().layout_no_wrap(menu.title.clone(), font, p.text);
            let size = egui::vec2(galley.size().x + 20.0 + 18.0, 34.0);
            let sense = if i == state.current { Sense::click() } else { Sense::CLICK };
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let response = ui.interact(rect, title_id, sense);
            title_rects.push(rect);
            let is_open = state.open == Some(i);
            if i == state.current && std::mem::take(&mut state.focus_title) {
                response.request_focus();
            }
            if ui.is_rect_visible(rect) {
                let lit = is_open || response.hovered() || response.has_focus();
                if lit {
                    ui.painter().rect_filled(rect, CornerRadius::same(6), p.tint);
                }
                let words = if lit { p.accent } else { p.text };
                let galley = ui.painter().layout_no_wrap(
                    menu.title.clone(),
                    egui::FontId::new(15.0, super::fonts::bold_family()),
                    words,
                );
                let text_at = egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0);
                ui.painter().galley(text_at, galley.clone(), words);
                let chevron = Rect::from_center_size(
                    egui::pos2(text_at.x + galley.size().x + 11.0, rect.center().y),
                    egui::vec2(13.0, 13.0),
                );
                icon::paint(ui.painter(), chevron, UiIcon::ChevronDown, if lit { p.accent } else { p.muted });
            }
            let title = menu.title.clone();
            ctx.accesskit_node_builder(title_id, |node| {
                node.set_role(accesskit::Role::MenuItem);
                node.set_label(title);
                // Only the title the Tab key stops at can have the keyboard, and egui places only those by itself.
                node.set_bounds(bounds_of(rect));
                node.set_has_popup(accesskit::HasPopup::Menu);
                node.set_expanded(is_open);
                node.add_action(accesskit::Action::Click);
                node.add_action(accesskit::Action::Focus);
            });
            ring(ui, &response);
            // A pointer on another title while a menu is open opens that one instead.
            if response.hovered()
                && state.open.is_some()
                && !is_open
                && ui.input(|inp| inp.pointer.delta() != egui::Vec2::ZERO)
            {
                state.open = Some(i);
                state.item = None;
                state.current = i;
            }
            if response.clicked() {
                let by_pointer = ui.input(|inp| inp.pointer.any_released());
                state.current = i;
                if is_open {
                    state.open = None;
                    state.focus_title = true;
                } else {
                    if state.came_from.is_none() {
                        state.came_from = ctx.memory(|m| m.focused()).filter(|f| !ids.contains(f) && *f != title_id);
                    }
                    state.open = Some(i);
                    state.item = if by_pointer { None } else { first_item(&menu.lines, 1) };
                    state.focus_item = !by_pointer;
                    if by_pointer {
                        state.focus_title = true;
                    }
                }
            }
            if response.has_focus() {
                ui.memory_mut(|m| m.set_focus_lock_filter(title_id, TITLE_KEYS));
                let (left, right, down, up, escape, tab) = ui.input_mut(|inp| {
                    (
                        inp.consume_key(Modifiers::NONE, Key::ArrowLeft),
                        inp.consume_key(Modifiers::NONE, Key::ArrowRight),
                        inp.consume_key(Modifiers::NONE, Key::ArrowDown),
                        inp.consume_key(Modifiers::NONE, Key::ArrowUp),
                        inp.consume_key(Modifiers::NONE, Key::Escape),
                        inp.key_pressed(Key::Tab),
                    )
                });
                if left || right {
                    let n = menus.len();
                    let next = if left { (i + n - 1) % n } else { (i + 1) % n };
                    state.current = next;
                    state.focus_title = true;
                    if state.open.is_some() {
                        state.open = Some(next);
                        state.item = first_item(&menus[next].lines, 1);
                        state.focus_item = true;
                        state.focus_title = false;
                    }
                } else if down || up {
                    state.open = Some(i);
                    state.item = first_item(&menu.lines, if up { -1 } else { 1 });
                    state.focus_item = true;
                } else if escape {
                    if state.open.is_some() {
                        state.open = None;
                        state.focus_title = true;
                    } else if let Some(back) = state.came_from.take() {
                        ui.memory_mut(|m| m.request_focus(back));
                    }
                } else if tab {
                    // The bar is one stop: Tab leaves it (egui moves the keyboard on) and closes what is open.
                    state.open = None;
                    state.came_from = None;
                }
            }
        }
    });
    if let Some(open) = state.open.filter(|o| *o < menus.len()) {
        let anchor = title_rects[open];
        let outcome = popup(ui, id.with(("menu", open)), anchor, &menus[open].title, &menus[open].lines, &mut state);
        ids.extend(outcome.item_ids.iter().copied());
        match outcome.event {
            PopupEvent::None => {}
            PopupEvent::Chosen(command) => {
                chosen = Some(command);
                state.open = None;
                match state.came_from.take() {
                    Some(back) => ui.memory_mut(|m| m.request_focus(back)),
                    None => state.focus_title = true,
                }
            }
            PopupEvent::Closed { give_title_keyboard } => {
                state.open = None;
                state.focus_title = give_title_keyboard;
            }
            PopupEvent::Sideways(step) => {
                let n = menus.len() as i32;
                let next = ((open as i32 + step).rem_euclid(n)) as usize;
                state.open = Some(next);
                state.current = next;
                state.item = first_item(&menus[next].lines, 1);
                state.focus_item = true;
            }
        }
        // A click anywhere else closes the menu, without moving the keyboard.
        if state.open.is_some() && opened_by_pointer_before.is_some() {
            let clicked_elsewhere = ui.input(|inp| {
                inp.pointer.any_pressed()
                    && inp
                        .pointer
                        .interact_pos()
                        .is_some_and(|at| !outcome.rect.contains(at) && !title_rects.iter().any(|r| r.contains(at)))
            });
            if clicked_elsewhere {
                state.open = None;
            }
        }
    }
    ctx.data_mut(|d| d.insert_temp(id.with("ids"), ids));
    store(&ctx, id, state);
    chosen
}

/// A button that opens a menu (Add, "..."): `shown` are the words on it (with its icon), `name` what a screen reader says.
/// Returns the command chosen, if one was.
pub fn menu_button<C: Clone>(
    ui: &mut Ui,
    id_salt: &str,
    icon_name: UiIcon,
    shown: &str,
    name: &str,
    lines: &[Line<C>],
) -> Option<C> {
    let id = ui.make_persistent_id(("menu-button", id_salt));
    let ctx = ui.ctx().clone();
    let mut state = load(&ctx, id);
    let is_open = state.open.is_some();
    let p = palette(ui);
    let button = if shown.is_empty() {
        egui::Button::new(icon::atom(icon_name, 18.0)).min_size(egui::vec2(36.0, 32.0))
    } else {
        egui::Button::new((icon::atom(icon_name, 16.0), shown.to_string(), icon::atom(UiIcon::ChevronDown, 13.0)))
    };
    let button = if is_open { button.fill(p.tint) } else { button };
    let response = ui.add(button);
    if std::mem::take(&mut state.focus_title) {
        response.request_focus();
    }
    let spoken = name.to_string();
    ctx.accesskit_node_builder(response.id, |node| {
        node.set_label(spoken);
        node.set_has_popup(accesskit::HasPopup::Menu);
        node.set_expanded(is_open);
    });
    if shown.is_empty() || shown != name {
        response.clone().on_hover_text(name);
    }
    ring(ui, &response);
    let mut chosen = None;
    if response.clicked() {
        if is_open {
            state.open = None;
        } else {
            let by_pointer = ui.input(|inp| inp.pointer.any_released());
            state.open = Some(0);
            state.item = if by_pointer { None } else { first_item(lines, 1) };
            state.focus_item = !by_pointer;
        }
    }
    if response.has_focus() && !is_open {
        let (down, up) = ui.input_mut(|inp| {
            (inp.consume_key(Modifiers::NONE, Key::ArrowDown), inp.consume_key(Modifiers::NONE, Key::ArrowUp))
        });
        if down || up {
            state.open = Some(0);
            state.item = first_item(lines, if up { -1 } else { 1 });
            state.focus_item = true;
        }
    }
    if state.open.is_some() {
        let outcome = popup(ui, id.with("menu"), response.rect, name, lines, &mut state);
        match outcome.event {
            PopupEvent::None => {}
            PopupEvent::Chosen(command) => {
                chosen = Some(command);
                state.open = None;
                state.focus_title = true;
            }
            PopupEvent::Closed { give_title_keyboard } => {
                state.open = None;
                state.focus_title = give_title_keyboard;
            }
            // A menu button has no menu beside it: Left and Right do nothing.
            PopupEvent::Sideways(_) => {}
        }
        if state.open.is_some() && is_open {
            let clicked_elsewhere = ui.input(|inp| {
                inp.pointer.any_pressed()
                    && inp
                        .pointer
                        .interact_pos()
                        .is_some_and(|at| !outcome.rect.contains(at) && !response.rect.contains(at))
            });
            if clicked_elsewhere {
                state.open = None;
            }
        }
    }
    store(&ctx, id, state);
    chosen
}

enum PopupEvent<C> {
    None,
    Chosen(C),
    Closed { give_title_keyboard: bool },
    Sideways(i32),
}

struct PopupOutcome<C> {
    event: PopupEvent<C>,
    rect: Rect,
    item_ids: Vec<Id>,
}

/// Where a control is on the screen, for a screen reader (it draws its box there, and a touch or a magnifier finds it).
pub fn bounds_of(rect: Rect) -> accesskit::Rect {
    accesskit::Rect { x0: rect.min.x.into(), y0: rect.min.y.into(), x1: rect.max.x.into(), y1: rect.max.y.into() }
}

/// The first item that can have the keyboard, from the top (`step` 1) or the bottom (`step` -1).
fn first_item<C>(lines: &[Line<C>], step: i32) -> Option<usize> {
    let items: Vec<usize> =
        lines.iter().enumerate().filter(|(_, l)| matches!(l, Line::Item(_))).map(|(i, _)| i).collect();
    if step >= 0 {
        items.first().copied()
    } else {
        items.last().copied()
    }
}

/// The item after (or before) `from`, going round.
fn next_item<C>(lines: &[Line<C>], from: Option<usize>, step: i32) -> Option<usize> {
    let items: Vec<usize> =
        lines.iter().enumerate().filter(|(_, l)| matches!(l, Line::Item(_))).map(|(i, _)| i).collect();
    if items.is_empty() {
        return None;
    }
    let at = from.and_then(|f| items.iter().position(|i| *i == f));
    let n = items.len() as i32;
    let next = match at {
        Some(at) => (at as i32 + step).rem_euclid(n),
        None if step >= 0 => 0,
        None => n - 1,
    };
    Some(items[next as usize])
}

/// Draw the open menu under `anchor`, and do what its keys and the pointer ask.
fn popup<C: Clone>(
    ui: &mut Ui,
    id: Id,
    anchor: Rect,
    title: &str,
    lines: &[Line<C>],
    state: &mut State,
) -> PopupOutcome<C> {
    let ctx = ui.ctx().clone();
    let p = palette(ui);
    let mut event = PopupEvent::None;
    let mut item_ids = Vec::new();
    let frame = egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.divider))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(6))
        .shadow(egui::epaint::Shadow { offset: [0, 6], blur: 20, spread: 0, color: Color32::from_black_alpha(46) });
    let area = egui::Area::new(id)
        .kind(egui::UiKind::GenericArea)
        .order(egui::Order::Foreground)
        .fixed_pos(anchor.left_bottom() + egui::vec2(0.0, 4.0))
        .constrain(true);
    let shown = area.show(&ctx, |ui| {
        frame.show(ui, |ui| {
            let builder = egui::UiBuilder::new()
                .layout(egui::Layout::top_down(egui::Align::LEFT))
                .accessibility_role(accesskit::Role::Menu)
                .accessibility_label(title.to_string());
            ui.scope_builder(builder, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let keys_width = lines
                    .iter()
                    .filter_map(|l| match l {
                        Line::Item(e) => e.keys.as_ref(),
                        Line::Separator => None,
                    })
                    .map(|k| {
                        ui.painter()
                            .layout_no_wrap(k.clone(), egui::TextStyle::Small.resolve(ui.style()), p.muted)
                            .size()
                            .x
                    })
                    .fold(0.0_f32, f32::max);
                let words_width = lines
                    .iter()
                    .filter_map(|l| match l {
                        Line::Item(e) => Some(&e.words),
                        Line::Separator => None,
                    })
                    .map(|w| {
                        ui.painter()
                            .layout_no_wrap(w.clone(), egui::TextStyle::Body.resolve(ui.style()), p.text)
                            .size()
                            .x
                    })
                    .fold(0.0_f32, f32::max);
                let width = (12.0
                    + 18.0
                    + 12.0
                    + words_width
                    + 24.0
                    + 18.0
                    + if keys_width > 0.0 { keys_width + 18.0 } else { 0.0 })
                .clamp(220.0, 520.0);
                for (index, line) in lines.iter().enumerate() {
                    let entry = match line {
                        Line::Separator => {
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 9.0), Sense::hover());
                            ui.painter().hline(
                                rect.x_range().shrink(6.0),
                                rect.center().y,
                                Stroke::new(1.0, p.divider),
                            );
                            continue;
                        }
                        Line::Item(entry) => entry,
                    };
                    let item_id = id.with(("item", index));
                    item_ids.push(item_id);
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 34.0), Sense::hover());
                    let response = ui.interact(rect, item_id, Sense::click());
                    if state.item == Some(index) && std::mem::take(&mut state.focus_item) {
                        response.request_focus();
                    }
                    if response.hovered() && ui.input(|inp| inp.pointer.delta() != egui::Vec2::ZERO) {
                        state.item = Some(index);
                    }
                    let lit = response.has_focus() || response.hovered();
                    if ui.is_rect_visible(rect) {
                        if lit {
                            ui.painter().rect_filled(rect, CornerRadius::same(6), p.tint);
                        }
                        if response.has_focus() {
                            ui.painter().rect_stroke(
                                rect.shrink(1.0),
                                CornerRadius::same(6),
                                Stroke::new(2.0, p.focus),
                                StrokeKind::Inside,
                            );
                        }
                        let words_colour = if entry.enabled { p.text } else { p.muted };
                        let icon_colour = if entry.enabled { p.accent } else { p.muted };
                        if let Some(name) = entry.icon {
                            let at = Rect::from_center_size(
                                egui::pos2(rect.left() + 21.0, rect.center().y),
                                egui::vec2(18.0, 18.0),
                            );
                            icon::paint(ui.painter(), at, name, icon_colour);
                        }
                        ui.painter().text(
                            egui::pos2(rect.left() + 42.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &entry.words,
                            egui::TextStyle::Body.resolve(ui.style()),
                            words_colour,
                        );
                        let mut right = rect.right() - 12.0;
                        if let Some(keys) = &entry.keys {
                            let galley = ui.painter().layout_no_wrap(
                                keys.clone(),
                                egui::TextStyle::Small.resolve(ui.style()),
                                p.muted,
                            );
                            right -= galley.size().x;
                            ui.painter().galley(
                                egui::pos2(right, rect.center().y - galley.size().y / 2.0),
                                galley,
                                p.muted,
                            );
                            right -= 14.0;
                        }
                        if entry.ticked {
                            let at = Rect::from_center_size(
                                egui::pos2(right - 8.0, rect.center().y),
                                egui::vec2(16.0, 16.0),
                            );
                            icon::paint(ui.painter(), at, UiIcon::Check, icon_colour);
                        }
                    }
                    let words = entry.words.clone();
                    let keys = entry.keys.clone();
                    let (enabled, kind, ticked) = (entry.enabled, entry.kind, entry.ticked);
                    ctx.accesskit_node_builder(item_id, |node| {
                        node.set_role(match kind {
                            ItemKind::Plain => accesskit::Role::MenuItem,
                            ItemKind::Tick => accesskit::Role::MenuItemCheckBox,
                            ItemKind::OneOf => accesskit::Role::MenuItemRadio,
                        });
                        node.set_label(words);
                        if let Some(keys) = keys {
                            node.set_keyboard_shortcut(keys);
                        }
                        if kind != ItemKind::Plain {
                            node.set_toggled(if ticked { accesskit::Toggled::True } else { accesskit::Toggled::False });
                        }
                        if !enabled {
                            node.set_disabled();
                        }
                        node.add_action(accesskit::Action::Click);
                        node.add_action(accesskit::Action::Focus);
                    });
                    if response.clicked() && entry.enabled {
                        event = PopupEvent::Chosen(entry.command.clone());
                    }
                    if response.has_focus() {
                        state.item = Some(index);
                        ui.memory_mut(|m| m.set_focus_lock_filter(item_id, KEEP_KEYS));
                        let (up, down, home, end, left, right_key, escape, tab, typed) = ui.input_mut(|inp| {
                            let typed = inp.events.iter().find_map(|e| match e {
                                egui::Event::Text(t) => t.chars().next(),
                                _ => None,
                            });
                            (
                                inp.consume_key(Modifiers::NONE, Key::ArrowUp),
                                inp.consume_key(Modifiers::NONE, Key::ArrowDown),
                                inp.consume_key(Modifiers::NONE, Key::Home),
                                inp.consume_key(Modifiers::NONE, Key::End),
                                inp.consume_key(Modifiers::NONE, Key::ArrowLeft),
                                inp.consume_key(Modifiers::NONE, Key::ArrowRight),
                                inp.consume_key(Modifiers::NONE, Key::Escape),
                                inp.consume_key(Modifiers::NONE, Key::Tab)
                                    || inp.consume_key(Modifiers::SHIFT, Key::Tab),
                                typed,
                            )
                        });
                        let target = if up {
                            next_item(lines, Some(index), -1)
                        } else if down {
                            next_item(lines, Some(index), 1)
                        } else if home {
                            first_item(lines, 1)
                        } else if end {
                            first_item(lines, -1)
                        } else if let Some(letter) = typed.filter(|c| c.is_alphanumeric()) {
                            jump_to_letter(lines, index, letter)
                        } else {
                            None
                        };
                        if let Some(target) = target {
                            state.item = Some(target);
                            state.focus_item = true;
                        }
                        if left || right_key {
                            event = PopupEvent::Sideways(if left { -1 } else { 1 });
                        } else if escape || tab {
                            event = PopupEvent::Closed { give_title_keyboard: true };
                        }
                    }
                }
            });
        });
    });
    PopupOutcome { event, rect: shown.response.rect, item_ids }
}

/// The next item after `from` whose words start with `letter`, going round.
fn jump_to_letter<C>(lines: &[Line<C>], from: usize, letter: char) -> Option<usize> {
    let letter = letter.to_lowercase().next()?;
    let n = lines.len();
    (1..=n).map(|k| (from + k) % n).find(|i| match &lines[*i] {
        Line::Item(e) => e.words.to_lowercase().starts_with(letter),
        Line::Separator => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(words: &str) -> Line<u8> {
        Line::Item(Entry {
            command: 0,
            words: words.into(),
            icon: None,
            keys: None,
            enabled: true,
            kind: ItemKind::Plain,
            ticked: false,
        })
    }

    #[test]
    fn the_keyboard_moves_round_the_items_and_skips_the_lines_between_groups() {
        let lines = vec![line("Undo"), line("Redo"), Line::Separator, line("Choose all"), line("Copy")];
        assert_eq!(first_item(&lines, 1), Some(0));
        assert_eq!(first_item(&lines, -1), Some(4));
        assert_eq!(next_item(&lines, Some(1), 1), Some(3), "the line is skipped");
        assert_eq!(next_item(&lines, Some(4), 1), Some(0), "round to the top");
        assert_eq!(next_item(&lines, Some(0), -1), Some(4), "round to the bottom");
        assert_eq!(jump_to_letter(&lines, 0, 'c'), Some(3));
        assert_eq!(jump_to_letter(&lines, 3, 'C'), Some(4), "the next one with that letter, in any case");
        assert_eq!(jump_to_letter(&lines, 4, 'c'), Some(3), "round again");
        assert_eq!(jump_to_letter(&lines, 0, 'z'), None);
        let empty: Vec<Line<u8>> = vec![Line::Separator];
        assert_eq!(first_item(&empty, 1), None);
        assert_eq!(next_item(&empty, None, 1), None);
    }
}
