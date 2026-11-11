// SPDX-License-Identifier: Apache-2.0
//! The window's look: the OpenStage suite's colours (the navy bar, a pale page, white panels, one product colour), with Capture
//! Express's own colour, the indigo of its icon. A light and a dark version follow Windows' own setting, or the person's choice.
//!
//! The light version uses the suite's colours as they are written in OpenStage Author's `design/openstage-ui.css` (version 1), and
//! Express's pair is measured the way the suite's design guide asks: the icon's own indigo `#9CA5FF` on the navy bar, and a deeper
//! indigo `#4E57BC` for filled buttons, the chosen row and links (white words on it 6.2 to 1). The dark version is Express's own.
//! The rules the colours must keep are checked by tests:
//!
//! * words against everything they sit on are at least 4.5 to 1;
//! * borders, the focus ring and other things that are not words are at least 3 to 1 against what they sit on;
//! * words on the navy bar are at least 4.5 to 1, and white is never put on the light indigo of the dark version (3.2 to 1).

use eframe::egui::{self, Color32, CornerRadius, FontId, Stroke, TextStyle, Theme, ThemePreference, Visuals};

use super::fonts;

/// Every colour the window uses, for one scheme.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// The window behind everything, and the area round the picture.
    pub page: Color32,
    /// Panels, cards, menus and dialogs.
    pub surface: Color32,
    /// A quiet group inside a panel.
    pub subtle: Color32,
    /// A text box.
    pub field: Color32,
    /// A button at rest.
    pub raised: Color32,
    /// A button under the pointer.
    pub raised_hover: Color32,
    pub text: Color32,
    /// Secondary words.
    pub muted: Color32,
    /// Express's indigo for links, icons in menus and marks on the page.
    pub accent: Color32,
    /// Express's indigo for fills (the main button, the chosen row).
    pub accent_fill: Color32,
    /// The main button under the pointer.
    pub accent_hover: Color32,
    /// Words on `accent_fill` and `accent_hover`.
    pub on_accent: Color32,
    /// The background of a menu item under the pointer or the keyboard, and of an open menu's title.
    pub tint: Color32,
    /// The edge of a control (a button, a box).
    pub border: Color32,
    /// A line between parts, which marks nothing on its own.
    pub divider: Color32,
    /// The ring round whatever has the keyboard.
    pub focus: Color32,
    pub error: Color32,
    /// The navy bar at the top of the window, the suite's own colour.
    pub shell: Color32,
    /// A button on the bar under the pointer.
    pub shell_hover: Color32,
    /// Words on the bar.
    pub on_shell: Color32,
    /// Quieter words on the bar.
    pub on_shell_muted: Color32,
    /// Express's own colour on the bar (its name).
    pub brand: Color32,
    /// The frame and handles drawn over a picture (the crop, the chosen mark, the area to capture): orange, to stand out on any
    /// screenshot.
    pub overlay: Color32,
}

pub const DARK: Palette = Palette {
    page: Color32::from_rgb(0x0B, 0x17, 0x2A),
    surface: Color32::from_rgb(0x10, 0x20, 0x37),
    subtle: Color32::from_rgb(0x14, 0x27, 0x4A),
    field: Color32::from_rgb(0x0B, 0x17, 0x2A),
    raised: Color32::from_rgb(0x1B, 0x2E, 0x4D),
    raised_hover: Color32::from_rgb(0x26, 0x3C, 0x62),
    text: Color32::from_rgb(0xE8, 0xEC, 0xFF),
    muted: Color32::from_rgb(0xB4, 0xBC, 0xDB),
    accent: Color32::from_rgb(0x9C, 0xA5, 0xFF),
    accent_fill: Color32::from_rgb(0x7D, 0x87, 0xF2),
    accent_hover: Color32::from_rgb(0x9C, 0xA5, 0xFF),
    on_accent: Color32::from_rgb(0x0B, 0x17, 0x2A),
    tint: Color32::from_rgb(0x1F, 0x2C, 0x5A),
    border: Color32::from_rgb(0x85, 0x90, 0xB8),
    divider: Color32::from_rgb(0x2A, 0x3B, 0x5C),
    focus: Color32::from_rgb(0xFF, 0xB4, 0x54),
    error: Color32::from_rgb(0xFF, 0xA3, 0xA3),
    shell: Color32::from_rgb(0x0D, 0x1B, 0x30),
    shell_hover: Color32::from_rgb(0x1C, 0x2D, 0x49),
    on_shell: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    on_shell_muted: Color32::from_rgb(0xB9, 0xC4, 0xD4),
    brand: Color32::from_rgb(0x9C, 0xA5, 0xFF),
    overlay: Color32::from_rgb(0xFF, 0xB4, 0x54),
};

pub const LIGHT: Palette = Palette {
    page: Color32::from_rgb(0xEE, 0xF1, 0xF5),
    surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    subtle: Color32::from_rgb(0xF7, 0xF9, 0xFC),
    field: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    raised: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    raised_hover: Color32::from_rgb(0xEE, 0xF1, 0xF5),
    text: Color32::from_rgb(0x19, 0x2A, 0x43),
    muted: Color32::from_rgb(0x5A, 0x69, 0x80),
    accent: Color32::from_rgb(0x4E, 0x57, 0xBC),
    accent_fill: Color32::from_rgb(0x4E, 0x57, 0xBC),
    accent_hover: Color32::from_rgb(0x3E, 0x46, 0xA3),
    on_accent: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    tint: Color32::from_rgb(0xEC, 0xEE, 0xFC),
    border: Color32::from_rgb(0x7A, 0x86, 0x99),
    divider: Color32::from_rgb(0xDC, 0xE3, 0xEB),
    focus: Color32::from_rgb(0x0B, 0x3F, 0xB3),
    error: Color32::from_rgb(0xB4, 0x42, 0x48),
    shell: Color32::from_rgb(0x0D, 0x1B, 0x30),
    shell_hover: Color32::from_rgb(0x1C, 0x2D, 0x49),
    on_shell: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    on_shell_muted: Color32::from_rgb(0xB9, 0xC4, 0xD4),
    brand: Color32::from_rgb(0x9C, 0xA5, 0xFF),
    overlay: Color32::from_rgb(0xC2, 0x41, 0x0C),
};

/// The palette for a scheme.
pub fn palette(theme: Theme) -> Palette {
    match theme {
        Theme::Dark => DARK,
        Theme::Light => LIGHT,
    }
}

/// The colours as egui's own style wants them.
pub fn visuals(p: &Palette, theme: Theme) -> Visuals {
    let mut v = match theme {
        Theme::Dark => Visuals::dark(),
        Theme::Light => Visuals::light(),
    };
    let radius = CornerRadius::same(6);
    v.hyperlink_color = p.accent;
    v.panel_fill = p.page;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.divider);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(8);
    v.extreme_bg_color = p.field;
    v.faint_bg_color = p.subtle;
    v.text_edit_bg_color = Some(p.field);
    v.error_fg_color = p.error;
    v.warn_fg_color = p.error;
    v.selection.bg_fill = p.accent_fill;
    v.selection.stroke = Stroke::new(1.5, p.on_accent);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.divider);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = p.raised;
    w.inactive.weak_bg_fill = p.raised;
    w.inactive.bg_stroke = Stroke::new(1.0, p.border);
    w.inactive.fg_stroke = Stroke::new(1.5, p.text);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = p.raised_hover;
    w.hovered.weak_bg_fill = p.raised_hover;
    w.hovered.bg_stroke = Stroke::new(1.5, p.accent);
    w.hovered.fg_stroke = Stroke::new(1.5, p.text);
    w.hovered.corner_radius = radius;

    // egui draws "pressed" and "has the keyboard" the same way, so both get the indigo fill with white (or navy) words.
    w.active.bg_fill = p.accent_fill;
    w.active.weak_bg_fill = p.accent_fill;
    w.active.bg_stroke = Stroke::new(1.5, p.accent);
    w.active.fg_stroke = Stroke::new(1.5, p.on_accent);
    w.active.corner_radius = radius;

    w.open = w.hovered;
    v
}

/// Give the window its fonts, its colours (dark and light, following Windows) and roomy, easy-to-hit controls.
pub fn install(ctx: &egui::Context) -> fonts::FontChoice {
    let (definitions, choice) = fonts::definitions();
    ctx.set_fonts(definitions);
    for theme in [Theme::Dark, Theme::Light] {
        let p = palette(theme);
        ctx.set_visuals_of(theme, visuals(&p, theme));
        ctx.style_mut_of(theme, |style| {
            style.text_styles.insert(TextStyle::Heading, FontId::new(24.0, fonts::bold_family()));
            style.text_styles.insert(TextStyle::Body, FontId::new(16.0, egui::FontFamily::Proportional));
            style.text_styles.insert(TextStyle::Button, FontId::new(16.0, egui::FontFamily::Proportional));
            style.text_styles.insert(TextStyle::Small, FontId::new(13.5, egui::FontFamily::Proportional));
            style.text_styles.insert(TextStyle::Monospace, FontId::new(15.0, egui::FontFamily::Monospace));
            style.spacing.item_spacing = egui::vec2(10.0, 9.0);
            style.spacing.button_padding = egui::vec2(14.0, 7.0);
            style.spacing.interact_size.y = 36.0;
            // Tick boxes and round choices a little bigger than egui's own, easier to see and to hit.
            style.spacing.icon_width = 19.0;
            style.spacing.icon_width_inner = 10.0;
            style.spacing.icon_spacing = 8.0;
            style.spacing.window_margin = egui::Margin::same(16);
            style.animation_time = 0.0;
        });
    }
    ctx.set_theme(ThemePreference::System);
    choice
}

/// The relative luminance of a colour as the accessibility rules define it.
#[cfg(test)]
fn luminance(c: Color32) -> f64 {
    let channel = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}

/// The contrast ratio between two colours: 1 to 21.
#[cfg(test)]
pub(crate) fn contrast(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn each() -> [(&'static str, Palette); 2] {
        [("dark", DARK), ("light", LIGHT)]
    }

    #[test]
    fn words_have_at_least_four_and_a_half_to_one_on_everything_they_sit_on() {
        for (name, p) in each() {
            let backs = [
                ("the page", p.page),
                ("a panel", p.surface),
                ("a quiet group", p.subtle),
                ("a text box", p.field),
                ("a button", p.raised),
                ("a hovered button", p.raised_hover),
                ("a menu item under the pointer", p.tint),
            ];
            for (what, words) in
                [("words", p.text), ("muted words", p.muted), ("a link", p.accent), ("an error", p.error)]
            {
                for (on, back) in backs {
                    let ratio = contrast(words, back);
                    assert!(ratio >= 4.5, "{name}: {what} on {on}: {ratio:.2}");
                }
            }
            for (on, back) in [("indigo", p.accent_fill), ("indigo under the pointer", p.accent_hover)] {
                let ratio = contrast(p.on_accent, back);
                assert!(ratio >= 4.5, "{name}: words on {on}: {ratio:.2}");
            }
        }
    }

    #[test]
    fn words_on_the_navy_bar_have_at_least_four_and_a_half_to_one() {
        for (name, p) in each() {
            for (what, words) in [("words", p.on_shell), ("quiet words", p.on_shell_muted), ("Express's name", p.brand)]
            {
                for (on, back) in [("the bar", p.shell), ("a button on the bar under the pointer", p.shell_hover)] {
                    let ratio = contrast(words, back);
                    assert!(ratio >= 4.5, "{name}: {what} on {on}: {ratio:.2}");
                }
            }
        }
    }

    #[test]
    fn borders_the_focus_ring_and_the_chosen_row_have_at_least_three_to_one_on_what_they_touch() {
        for (name, p) in each() {
            for (what, colour) in [
                ("a control's edge", p.border),
                ("the focus ring", p.focus),
                ("indigo", p.accent),
                ("a chosen row", p.accent_fill),
            ] {
                for (on, back) in
                    [("the page", p.page), ("a panel", p.surface), ("a quiet group", p.subtle), ("a button", p.raised)]
                {
                    let ratio = contrast(colour, back);
                    assert!(ratio >= 3.0, "{name}: {what} on {on}: {ratio:.2}");
                }
            }
            // The ring round a focused button on the bar touches the bar.
            assert!(contrast(p.on_shell, p.shell) >= 3.0, "{name}: the ring on the bar");
        }
    }

    #[test]
    fn white_words_on_the_light_indigo_would_fail_so_the_dark_scheme_uses_navy() {
        let light_indigo = Color32::from_rgb(0x7D, 0x87, 0xF2);
        assert!(
            contrast(Color32::WHITE, light_indigo) < 4.5,
            "white on #7D87F2 is {:.2}",
            contrast(Color32::WHITE, light_indigo)
        );
        assert_eq!(DARK.accent_fill, light_indigo);
        assert_eq!(DARK.on_accent, DARK.page, "navy on indigo");
        assert!(contrast(DARK.on_accent, DARK.accent_fill) > 5.0);
    }

    #[test]
    fn the_light_colours_are_the_suites_and_expresss_pair_is_the_indigo_of_its_icon() {
        // From OpenStage Author's design/openstage-ui.css, version 1.
        assert_eq!(LIGHT.shell, Color32::from_rgb(0x0D, 0x1B, 0x30));
        assert_eq!(LIGHT.page, Color32::from_rgb(0xEE, 0xF1, 0xF5));
        assert_eq!(LIGHT.surface, Color32::WHITE);
        assert_eq!(LIGHT.text, Color32::from_rgb(0x19, 0x2A, 0x43));
        assert_eq!(LIGHT.muted, Color32::from_rgb(0x5A, 0x69, 0x80));
        assert_eq!(LIGHT.border, Color32::from_rgb(0x7A, 0x86, 0x99));
        assert_eq!(LIGHT.focus, Color32::from_rgb(0x0B, 0x3F, 0xB3));
        assert_eq!(DARK.shell, LIGHT.shell, "the bar is the same navy in both");
        // The icon's gradient runs from #9CA5FF to #7D87F2 (assets/icon/README.txt).
        assert_eq!(LIGHT.brand, Color32::from_rgb(0x9C, 0xA5, 0xFF));
        assert_eq!(DARK.accent, LIGHT.brand);
        let ratio = contrast(Color32::WHITE, LIGHT.accent_fill);
        assert!(
            (6.0..6.5).contains(&ratio),
            "white on Express's deeper indigo is about 6.2 to 1, like the other products: {ratio:.2}"
        );
        assert!((contrast(LIGHT.brand, LIGHT.shell) - 7.6).abs() < 0.1, "the icon's indigo on the bar");
    }

    #[test]
    fn contrast_is_computed_the_standard_way() {
        assert!((contrast(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 1e-9);
        assert!((contrast(Color32::WHITE, Color32::WHITE) - 1.0).abs() < 1e-9);
        assert!((contrast(Color32::from_rgb(0x77, 0x77, 0x77), Color32::WHITE) - 4.48).abs() < 0.02);
    }

    #[test]
    fn the_visuals_use_the_palette_for_words_buttons_and_the_chosen_row() {
        for (theme, p) in [(Theme::Dark, DARK), (Theme::Light, LIGHT)] {
            let v = visuals(&p, theme);
            assert_eq!(v.widgets.noninteractive.fg_stroke.color, p.text);
            assert_eq!(v.widgets.inactive.fg_stroke.color, p.text);
            assert_eq!(
                v.override_text_color, None,
                "one colour forced over all words would also be forced onto a chosen row"
            );
            assert_eq!(v.panel_fill, p.page);
            assert_eq!(v.window_fill, p.surface);
            assert_eq!(v.widgets.inactive.weak_bg_fill, p.raised);
            assert_eq!(v.widgets.inactive.bg_stroke.color, p.border, "a button has an edge that can be seen (3 to 1)");
            assert_eq!(v.widgets.hovered.weak_bg_fill, p.raised_hover);
            assert_eq!(v.widgets.active.weak_bg_fill, p.accent_fill);
            assert_eq!(
                v.widgets.active.fg_stroke.color, p.on_accent,
                "words on a pressed or focused button can be read"
            );
            assert_eq!(v.selection.bg_fill, p.accent_fill);
            assert_eq!(v.selection.stroke.color, p.on_accent);
            assert_eq!(v.dark_mode, theme == Theme::Dark);
        }
    }
}
