// SPDX-License-Identifier: Apache-2.0
//! The OpenStage suite's frame round every screen of the whole window (not the capture bar): the navy bar along the top (the icon
//! and the name, the open guide and whether it is saved, the command search, and the screen's main buttons), the menu bar under it,
//! and the two windows the suite shares: the command search (Ctrl+K) and the list of keyboard keys (Ctrl+/). Sections that open
//! and close (the editor's details) are here too.

use eframe::egui::{
    self, accesskit, Color32, CornerRadius, Id, Key, Modifiers, Rect, RichText, Sense, Stroke, StrokeKind, Ui,
};

use super::commands::{self, Command};
use super::fonts::bold_family;
use super::icon::{self, UiIcon};
use super::widgets::{self, palette, ring};

/// The height of the navy bar and of the menu bar under it.
pub const BAR_HEIGHT: f32 = 52.0;
pub const MENU_HEIGHT: f32 = 42.0;

/// A button on the navy bar.
#[derive(Debug, Clone)]
pub struct BarButton {
    pub command: Command,
    /// The words on it (shorter than the menu's, without "...").
    pub words: &'static str,
    /// The screen's one main button, filled in Express's indigo.
    pub primary: bool,
    pub enabled: bool,
}

/// Whether the open guide is saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Saved {
    /// Every change is in the guide's folder.
    Saved,
    /// The example guide, which is never saved.
    Example,
}

impl Saved {
    pub fn words(self) -> &'static str {
        match self {
            Saved::Saved => "Saved",
            Saved::Example => "Example, not saved",
        }
    }
}

/// What the navy bar shows.
#[derive(Debug, Clone, Default)]
pub struct TopBarView {
    /// A way back on the left (Back to guides, in the editor).
    pub back: Option<BarButton>,
    /// The open guide's title and whether it is saved; `heading` makes the title the screen's first heading (in the editor).
    pub guide: Option<(String, Saved, bool)>,
    /// The main buttons on the right.
    pub buttons: Vec<BarButton>,
}

/// The program's icon, made into a picture once.
fn brand_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = Id::unique("express-brand-icon");
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(texture);
    }
    const ICON: &[u8] = include_bytes!("../../../../assets/icon/PNG/OpenCapture_Express_64.png");
    let mut decoder = png::Decoder::new(std::io::Cursor::new(ICON));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).ok()?;
    if info.color_type != png::ColorType::Rgba {
        return None;
    }
    buffer.truncate(info.buffer_size());
    let image = egui::ColorImage::from_rgba_unmultiplied([info.width as usize, info.height as usize], &buffer);
    let texture = ctx.load_texture("express-brand-icon", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    Some(texture)
}

/// The ring round a control on the navy bar that has the keyboard: white, as the suite's blue would not show on navy.
fn shell_ring(ui: &Ui, response: &egui::Response) {
    if response.has_focus() {
        let p = palette(ui);
        ui.painter().rect_stroke(
            response.rect.expand(3.0),
            CornerRadius::same(9),
            Stroke::new(2.5, p.on_shell),
            StrokeKind::Outside,
        );
    }
}

/// Make the buttons drawn in `ui` look right on the navy bar.
fn shell_style(ui: &mut Ui) {
    let p = palette(ui);
    let v = ui.visuals_mut();
    let edge = Stroke::new(1.0, p.on_shell_muted);
    v.widgets.inactive.weak_bg_fill = p.shell;
    v.widgets.inactive.bg_fill = p.shell;
    v.widgets.inactive.bg_stroke = edge;
    v.widgets.inactive.fg_stroke = Stroke::new(1.5, p.on_shell);
    v.widgets.hovered.weak_bg_fill = p.shell_hover;
    v.widgets.hovered.bg_fill = p.shell_hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.on_shell);
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, p.on_shell);
    v.widgets.active = v.widgets.hovered;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.on_shell);
    // A switched-off button keeps its words readable (egui fades them by itself otherwise).
    v.disabled_alpha = 0.6;
}

/// A button on the navy bar: an icon and words. The screen's main one is filled in Express's indigo.
fn bar_button(ui: &mut Ui, button: &BarButton, compact: bool) -> egui::Response {
    let p = palette(ui);
    let icon = icon::atom(button.command.icon(), 17.0);
    let widget = if compact {
        egui::Button::new(icon).min_size(egui::vec2(36.0, 34.0))
    } else {
        egui::Button::new((icon, RichText::new(button.words).strong())).min_size(egui::vec2(0.0, 34.0))
    };
    let widget =
        if button.primary { widget.fill(p.accent_fill).stroke(Stroke::new(1.0, p.accent_fill)) } else { widget };
    let response = ui.scope(|ui| {
        if button.primary {
            let v = ui.visuals_mut();
            for state in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
                state.fg_stroke = Stroke::new(1.5, p.on_accent);
            }
            v.widgets.hovered.weak_bg_fill = p.accent_hover;
            v.widgets.hovered.bg_fill = p.accent_hover;
        }
        ui.add_enabled(button.enabled, widget)
    });
    let response = response.inner;
    let words = button.words.to_string();
    if compact {
        ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(words));
    }
    let tip = match button.command.keys() {
        Some(keys) => format!("{} ({keys})", button.words),
        None => button.words.to_string(),
    };
    let response = response.on_hover_text(tip);
    shell_ring(ui, &response);
    response
}

/// The navy bar. Returns the command of a button that was pressed.
pub fn top_bar(ui: &mut Ui, view: &TopBarView) -> Option<Command> {
    let p = palette(ui);
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, p.shell);
    if ui.visuals().dark_mode {
        ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0, p.divider));
    }
    let mut pressed = None;
    // Narrow windows first drop "OpenStage" from the name, then the words on the bar's buttons (their names and tooltips stay).
    let width = rect.width();
    let compact_buttons = width < 1000.0;
    let short_name = width < 1180.0;
    let inner = rect.shrink2(egui::vec2(16.0, 0.0));
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            shell_style(ui);
            ui.spacing_mut().item_spacing.x = 10.0;
            // The icon is decoration (the name beside it says the same), so it is painted, not a picture a screen reader stops at.
            let (at, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), Sense::hover());
            if let Some(texture) = brand_texture(ui.ctx()) {
                let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter().image(texture.id(), at, uv, Color32::WHITE);
            }
            let mut name = egui::text::LayoutJob::default();
            if !short_name {
                name.append(
                    "OpenStage ",
                    0.0,
                    egui::TextFormat::simple(egui::FontId::new(18.0, bold_family()), p.on_shell),
                );
            }
            name.append(
                "Capture Express",
                0.0,
                egui::TextFormat::simple(egui::FontId::new(18.0, egui::FontFamily::Proportional), p.brand),
            );
            let label = ui.add(egui::Label::new(name).selectable(false));
            let full = "OpenStage Capture Express".to_string();
            ui.ctx().accesskit_node_builder(label.id, |node| node.set_label(full));
            ui.add_space(8.0);
            if let Some(back) = &view.back {
                if bar_button(ui, back, false).clicked() {
                    pressed = Some(back.command);
                }
            }
            // The buttons on the right are laid out left to right, so the Tab key goes through them in the order they are seen: a
            // space is left before them as wide as they were last time (measured as they are drawn), and the guide's title takes what
            // is left between.
            let measured_id = Id::unique(("express-bar-right", compact_buttons, view.buttons.len()));
            let right_width = ui.ctx().data(|d| d.get_temp::<f32>(measured_id)).unwrap_or_else(|| {
                let words: f32 = view
                    .buttons
                    .iter()
                    .map(|b| {
                        let text = ui.painter().layout_no_wrap(
                            b.words.into(),
                            egui::TextStyle::Button.resolve(ui.style()),
                            p.on_shell,
                        );
                        if compact_buttons {
                            46.0
                        } else {
                            60.0 + text.size().x
                        }
                    })
                    .sum();
                words + if compact_buttons { 46.0 } else { 120.0 }
            });
            if let Some((title, saved, heading)) = &view.guide {
                let room = (ui.available_width() - right_width - 16.0).max(80.0);
                let saved_text = ui.painter().layout_no_wrap(
                    saved.words().into(),
                    egui::TextStyle::Small.resolve(ui.style()),
                    p.on_shell_muted,
                );
                let title_room = (room - saved_text.size().x - 36.0).max(40.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(title_room, 34.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_max_width(title_room);
                        let shown = ui
                            .add(
                                egui::Label::new(
                                    RichText::new(title).color(p.on_shell).family(bold_family()).size(16.0),
                                )
                                .truncate()
                                .selectable(false),
                            )
                            .on_hover_text(title);
                        let words = title.clone();
                        let as_heading = *heading;
                        ui.ctx().accesskit_node_builder(shown.id, |node| {
                            if as_heading {
                                node.set_role(accesskit::Role::Heading);
                                node.set_level(1);
                            }
                            node.set_label(words);
                        });
                    },
                );
                let (at, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), Sense::hover());
                let mark = if *saved == Saved::Saved { UiIcon::CircleCheck } else { UiIcon::CircleDot };
                icon::paint(ui.painter(), at, mark, p.on_shell_muted);
                ui.label(RichText::new(saved.words()).size(13.5).color(p.on_shell_muted));
            }
            let gap = (ui.available_width() - right_width - ui.spacing().item_spacing.x - 2.0).max(0.0);
            ui.add_space(gap);
            let search = BarButton { command: Command::SearchCommands, words: "Search", primary: false, enabled: true };
            let response = bar_button(ui, &search, compact_buttons);
            let first = response.rect.min.x;
            let mut last = response.rect.max.x;
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.set_label("Search for a command".to_string());
                node.set_keyboard_shortcut("Ctrl+K".to_string());
                node.set_has_popup(accesskit::HasPopup::Dialog);
            });
            if response.clicked() {
                pressed = Some(Command::SearchCommands);
            }
            for button in &view.buttons {
                let response = bar_button(ui, button, compact_buttons);
                last = response.rect.max.x;
                if response.clicked() {
                    pressed = Some(button.command);
                }
            }
            let used = last - first + 2.0;
            if (used - right_width).abs() > 0.5 {
                ui.ctx().data_mut(|d| d.insert_temp(measured_id, used));
                ui.ctx().request_repaint();
            }
        },
    );
    pressed
}

/// The strip under the navy bar that holds the menu bar.
pub fn menu_row(ui: &mut Ui, menus: &[super::menubar::Menu<Command>]) -> Option<Command> {
    let p = palette(ui);
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, p.surface);
    ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0, p.divider));
    let mut chosen = None;
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(10.0, 4.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| chosen = super::menubar::menu_bar(ui, menus),
    );
    chosen
}

/// The widest a page of settings or choices gets, so a wide window does not stretch its lines across it.
pub const PAGE_WIDTH: f32 = 960.0;

/// The pale page a screen's content sits on, with room round it.
pub fn page_frame(ui: &Ui) -> egui::Frame {
    egui::Frame::new().fill(palette(ui).page).inner_margin(egui::Margin { left: 24, right: 16, top: 14, bottom: 8 })
}

/// The top of a screen under the menus (Settings, Export, the Trash...): a way back and the screen's first heading, on a white strip.
pub fn screen_header(ui: &mut Ui, id: &str, back: &str, title: &str, back_first_focus: bool) -> bool {
    let p = palette(ui);
    let mut went_back = false;
    egui::Panel::top(id.to_string())
        .frame(
            egui::Frame::new()
                .fill(p.surface)
                .inner_margin(egui::Margin::symmetric(16, 8))
                .stroke(Stroke::new(1.0, p.divider)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let response = ui.add(egui::Button::new((icon::atom(UiIcon::ArrowLeft, 16.0), back.to_string())));
                if back_first_focus {
                    response.request_focus();
                }
                ring(ui, &response);
                if response.clicked() {
                    went_back = true;
                }
                ui.add_space(6.0);
                widgets::heading(ui, title);
            });
        });
    went_back
}

/// A section of a panel that opens and closes: its heading is a button with the section's icon, its name and a chevron, read
/// as a heading with a button in it that says whether the section is open. What the person opens or closes stays so while the
/// program runs. Returns whether it is open (and draws `add` under the heading when it is).
pub fn section(
    ui: &mut Ui,
    id_salt: &str,
    icon_name: UiIcon,
    title: &str,
    starts_open: bool,
    add: impl FnOnce(&mut Ui),
) -> bool {
    let p = palette(ui);
    let id = Id::unique(("express-section", id_salt));
    let open = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(starts_open);
    let mut now_open = open;
    let heading =
        egui::UiBuilder::new().accessibility_role(accesskit::Role::Heading).accessibility_label(title.to_string());
    ui.scope_builder(heading, |ui| {
        let heading_id = ui.unique_id();
        ui.ctx().accesskit_node_builder(heading_id, |node| node.set_level(2));
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 38.0), Sense::click());
        if ui.is_rect_visible(rect) {
            if response.hovered() {
                ui.painter().rect_filled(rect, CornerRadius::same(6), p.subtle);
            }
            let at = Rect::from_center_size(egui::pos2(rect.left() + 15.0, rect.center().y), egui::vec2(18.0, 18.0));
            icon::paint(ui.painter(), at, icon_name, p.accent);
            ui.painter().text(
                egui::pos2(rect.left() + 34.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                title,
                egui::FontId::new(17.0, bold_family()),
                p.text,
            );
            let chevron =
                Rect::from_center_size(egui::pos2(rect.right() - 16.0, rect.center().y), egui::vec2(16.0, 16.0));
            icon::paint(ui.painter(), chevron, if open { UiIcon::ChevronUp } else { UiIcon::ChevronDown }, p.muted);
        }
        let words = title.to_string();
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(accesskit::Role::Button);
            node.set_label(words);
            node.set_expanded(open);
            node.add_action(accesskit::Action::Click);
            node.add_action(accesskit::Action::Focus);
        });
        ring(ui, &response);
        if response.clicked() {
            now_open = !open;
        }
    });
    if now_open != open {
        ui.ctx().data_mut(|d| d.insert_temp(id, now_open));
    }
    if open {
        egui::Frame::new().inner_margin(egui::Margin { left: 6, right: 2, top: 0, bottom: 4 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        });
    }
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top() - 2.0, Stroke::new(1.0, p.divider));
    ui.add_space(4.0);
    now_open
}

/// A dialog of the suite's: a white panel with rounded corners over a darker window, its title, a Close button in its corner that
/// does what Escape does, and its content. Returns what `add` returned, and whether it should close.
pub fn dialog<R>(ctx: &egui::Context, id: &str, title: &str, width: f32, add: impl FnOnce(&mut Ui) -> R) -> (R, bool) {
    let p = super::theme::palette(ctx.theme());
    let screen = ctx.content_rect();
    let width = width.min(screen.width() - 32.0).max(200.0);
    let height = (screen.height() - 48.0).max(160.0);
    let mut close = false;
    let modal = egui::Modal::new(Id::unique(("express-dialog", id)))
        .backdrop_color(Color32::from_rgba_unmultiplied(13, 27, 48, 140))
        .frame(
            egui::Frame::new()
                .fill(p.surface)
                .stroke(Stroke::new(1.0, p.divider))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(egui::Margin::same(18))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 8],
                    blur: 24,
                    spread: 0,
                    color: Color32::from_black_alpha(46),
                }),
        );
    let shown = modal.show(ctx, |ui| {
        let builder =
            egui::UiBuilder::new().accessibility_role(accesskit::Role::Dialog).accessibility_label(title.to_string());
        ui.scope_builder(builder, |ui| {
            ui.set_width(width - 36.0);
            ui.set_max_height(height - 36.0);
            let dialog_id = ui.unique_id();
            ui.ctx().accesskit_node_builder(dialog_id, |node| node.set_modal());
            ui.horizontal(|ui| {
                widgets::subheading(ui, title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let x = ui.add(
                        egui::Button::new(icon::atom(UiIcon::X, 18.0)).min_size(egui::vec2(34.0, 34.0)).frame(false),
                    );
                    ui.ctx().accesskit_node_builder(x.id, |node| node.set_label("Close".to_string()));
                    let x = x.on_hover_text("Close (Escape)");
                    ring(ui, &x);
                    if x.clicked() {
                        close = true;
                    }
                });
            });
            ui.add_space(4.0);
            add(ui)
        })
        .inner
    });
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        close = true;
    }
    (shown.inner, close)
}

/// What the command search remembers while it is open.
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    pub typed: String,
    /// The chosen result, by its place in the list.
    pub chosen: usize,
    /// Give the keyboard to the box when it is next drawn.
    pub focus_next: bool,
}

impl SearchState {
    pub fn new() -> SearchState {
        SearchState { typed: String::new(), chosen: 0, focus_next: true }
    }
}

/// The command search (Ctrl+K): a box, and the commands that match what is typed, best first. Up and Down choose one while the
/// keyboard stays in the box, Enter does it, Escape closes. `usable` says whether a command can be used now (one that cannot is
/// listed, dimmed, and does nothing). Returns the command chosen, and whether the search should close.
pub fn command_search(
    ctx: &egui::Context,
    state: &mut SearchState,
    usable: &dyn Fn(Command) -> bool,
) -> (Option<Command>, bool) {
    let found = commands::search(&state.typed);
    if !found.is_empty() {
        state.chosen = state.chosen.min(found.len() - 1);
    }
    let mut run = None;
    let (_, close) = dialog(ctx, "command-search", "Search for a command", 560.0, |ui| {
        let p = palette(ui);
        let list_id = Id::unique("express-command-results");
        let (up, down, enter) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        if !found.is_empty() {
            if up {
                state.chosen = (state.chosen + found.len() - 1) % found.len();
            }
            if down {
                state.chosen = (state.chosen + 1) % found.len();
            }
        }
        let label = ui.label("Type what you want to do");
        let before = state.typed.clone();
        let response = ui
            .add(
                egui::TextEdit::singleline(&mut state.typed)
                    .hint_text("For example: export, undo, dark")
                    .desired_width(f32::INFINITY),
            )
            .labelled_by(label.id);
        if std::mem::take(&mut state.focus_next) {
            response.request_focus();
        }
        if state.typed != before {
            state.chosen = 0;
        }
        let chosen_id = list_id.with(("row", state.chosen));
        let count = found.len();
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_label("Search for a command".to_string());
            if count > 0 {
                node.set_active_descendant(chosen_id.accesskit_id());
            }
            node.set_controls(vec![list_id.accesskit_id()]);
        });
        ring(ui, &response);
        if enter {
            if let Some(command) = found.get(state.chosen).copied().filter(|c| usable(*c)) {
                run = Some(command);
            }
        }
        ui.label(
            RichText::new(match count {
                0 => "No command matches what you typed.".to_string(),
                1 => "1 command. Up and Down choose, Enter does it.".to_string(),
                n => format!("{n} commands. Up and Down choose, Enter does it."),
            })
            .size(13.5)
            .color(p.muted),
        );
        let list = egui::UiBuilder::new()
            .id_salt(list_id)
            .accessibility_role(accesskit::Role::ListBox)
            .accessibility_label("Commands");
        egui::ScrollArea::vertical().max_height((ui.available_height() - 8.0).clamp(120.0, 380.0)).show(ui, |ui| {
            ui.scope_builder(list, |ui| {
                let list_ui_id = ui.unique_id();
                let _ = list_ui_id;
                ui.spacing_mut().item_spacing.y = 2.0;
                for (place, command) in found.iter().enumerate() {
                    let row_id = list_id.with(("row", place));
                    let width = ui.available_width();
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 36.0), Sense::hover());
                    let response = ui.interact(rect, row_id, Sense::CLICK);
                    let chosen = place == state.chosen;
                    let can = usable(*command);
                    if chosen {
                        response.scroll_to_me(None);
                    }
                    if ui.is_rect_visible(rect) {
                        if chosen {
                            ui.painter().rect_filled(rect, CornerRadius::same(6), p.tint);
                            ui.painter().rect_stroke(
                                rect.shrink(1.0),
                                CornerRadius::same(6),
                                Stroke::new(2.0, p.accent),
                                StrokeKind::Inside,
                            );
                        } else if response.hovered() {
                            ui.painter().rect_filled(rect, CornerRadius::same(6), p.subtle);
                        }
                        let words = if can { p.text } else { p.muted };
                        let at = Rect::from_center_size(
                            egui::pos2(rect.left() + 20.0, rect.center().y),
                            egui::vec2(18.0, 18.0),
                        );
                        icon::paint(ui.painter(), at, command.icon(), if can { p.accent } else { p.muted });
                        let galley = ui.painter().layout_no_wrap(
                            command.words(),
                            egui::TextStyle::Body.resolve(ui.style()),
                            words,
                        );
                        let name_width = galley.size().x;
                        ui.painter().galley(
                            egui::pos2(rect.left() + 40.0, rect.center().y - galley.size().y / 2.0),
                            galley,
                            words,
                        );
                        let menu = format!("{} menu", command.menu());
                        ui.painter().text(
                            egui::pos2(rect.left() + 52.0 + name_width, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            menu,
                            egui::TextStyle::Small.resolve(ui.style()),
                            p.muted,
                        );
                        if let Some(keys) = command.keys() {
                            ui.painter().text(
                                egui::pos2(rect.right() - 12.0, rect.center().y),
                                egui::Align2::RIGHT_CENTER,
                                keys,
                                egui::TextStyle::Small.resolve(ui.style()),
                                p.muted,
                            );
                        }
                    }
                    let name = command.words();
                    let place_words = format!("{} menu", command.menu());
                    let keys = command.keys().map(str::to_string);
                    ui.ctx().accesskit_node_builder(row_id, |node| {
                        node.set_bounds(super::menubar::bounds_of(rect));
                        node.set_role(accesskit::Role::ListBoxOption);
                        node.set_label(name);
                        node.set_description(place_words);
                        node.set_selected(chosen);
                        if let Some(keys) = keys {
                            node.set_keyboard_shortcut(keys);
                        }
                        if !can {
                            node.set_disabled();
                        }
                        node.add_action(accesskit::Action::Click);
                    });
                    if response.clicked() {
                        state.chosen = place;
                        if can {
                            run = Some(*command);
                        }
                    }
                }
            });
        });
    });
    (run, close || run.is_some())
}

/// The keyboard keys, in groups (Help > Keyboard keys, Ctrl+/). `hotkeys` is the sentence that names the capture hotkeys set now.
pub fn keyboard_keys(ctx: &egui::Context, hotkeys: &str) -> bool {
    let groups: [(&str, &[(&str, &str)]); 6] = [
        (
            "Everywhere",
            &[
                ("Ctrl+K", "Search for a command"),
                ("Ctrl+/", "This list of keys"),
                (
                    "F10",
                    "Go to the menus (then the arrows move along them and in them, Enter chooses, Escape goes back)",
                ),
                ("Tab, Shift+Tab", "Go to the next or the last control"),
            ],
        ),
        (
            "The steps list",
            &[
                ("Up, Down", "Choose the step above or below"),
                ("Shift+Up, Shift+Down", "Choose a run of steps"),
                ("Ctrl+click, Shift+click", "Add a step to the chosen ones, or choose a run"),
                ("Ctrl+A", "Choose every step"),
            ],
        ),
        (
            "Working with steps",
            &[
                ("Alt+Up, Alt+Down", "Move the chosen steps up or down"),
                ("Ctrl+Shift+Up, Ctrl+Shift+Down", "Choose the step before or after, from anywhere"),
                ("Ctrl+Shift+Delete", "Delete the chosen steps (they can be put back)"),
                ("F2", "Go to the step's title"),
                ("Ctrl+Z", "Undo (in a text box, undo the typing)"),
                ("Ctrl+Y, Ctrl+Shift+Z", "Redo"),
            ],
        ),
        (
            "Marks and the picture",
            &[
                ("Up, Down", "In the marks list, choose the mark above or below"),
                ("Delete", "In the marks list, delete the chosen mark"),
                ("Escape", "Stop drawing a shape, or put the drawing tool away"),
                ("Space while dragging", "Move the close-up even over a mark"),
            ],
        ),
        (
            "Rename and questions",
            &[("Enter", "Save the name typed"), ("Escape", "Cancel, or keep (say no to the question)")],
        ),
        ("Capturing (anywhere in Windows)", &[]),
    ];
    let (_, close) = dialog(ctx, "keyboard-keys", "Keyboard keys", 720.0, |ui| {
        let p = palette(ui);
        egui::ScrollArea::vertical().max_height((ui.available_height() - 4.0).max(160.0)).show(ui, |ui| {
            for (title, rows) in groups {
                widgets::subheading(ui, title);
                if rows.is_empty() {
                    ui.label(
                        RichText::new(if hotkeys.is_empty() { "No hotkeys are set." } else { hotkeys }).color(p.text),
                    );
                    ui.label(
                        RichText::new("Change them in Tools > Settings, under Hotkeys.").size(13.5).color(p.muted),
                    );
                    continue;
                }
                egui::Grid::new(("express-keys", title))
                    .num_columns(2)
                    .spacing(egui::vec2(18.0, 6.0))
                    .striped(false)
                    .show(ui, |ui| {
                        for (keys, what) in rows.iter() {
                            ui.label(RichText::new(*keys).family(bold_family()).color(p.text));
                            ui.label(RichText::new(*what).color(p.text));
                            ui.end_row();
                        }
                    });
                ui.add_space(8.0);
            }
        });
    });
    close
}
