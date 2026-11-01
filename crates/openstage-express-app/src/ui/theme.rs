// SPDX-License-Identifier: Apache-2.0
//! The window's look (spec section 4.4): the brand's navy and indigo, in a dark and a light version that follow Windows' own
//! setting. The rules the colours must keep are checked by tests:
//!
//! * text against the page, a panel and a button is at least 4.5 to 1;
//! * borders, the focus ring and other things that are not text are at least 3 to 1 against what they sit on;
//! * navy text on an indigo fill, never white text on the light indigo (3.2 to 1, which fails).

use eframe::egui::{self, Color32, CornerRadius, FontId, Stroke, TextStyle, Theme, ThemePreference, Visuals};

use super::fonts;

/// Every colour the window uses, for one scheme.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// The window and the main area.
    pub background: Color32,
    /// Side panels and text boxes.
    pub panel: Color32,
    /// A button at rest.
    pub raised: Color32,
    /// A button under the pointer.
    pub raised_hover: Color32,
    pub text: Color32,
    /// Secondary text.
    pub muted: Color32,
    /// Indigo for text links, borders and marks on the page.
    pub accent: Color32,
    /// Indigo for fills (the main button, the selected row).
    pub accent_fill: Color32,
    /// Text on `accent_fill`.
    pub on_accent: Color32,
    pub border: Color32,
    /// The 3 point ring round whatever has the keyboard.
    pub focus: Color32,
    pub error: Color32,
}

pub const DARK: Palette = Palette {
    background: Color32::from_rgb(0x0B, 0x17, 0x2A),
    panel: Color32::from_rgb(0x10, 0x20, 0x37),
    raised: Color32::from_rgb(0x1B, 0x2E, 0x4D),
    raised_hover: Color32::from_rgb(0x26, 0x3C, 0x62),
    text: Color32::from_rgb(0xE8, 0xEC, 0xFF),
    muted: Color32::from_rgb(0xB4, 0xBC, 0xDB),
    accent: Color32::from_rgb(0x9C, 0xA5, 0xFF),
    accent_fill: Color32::from_rgb(0x7D, 0x87, 0xF2),
    on_accent: Color32::from_rgb(0x0B, 0x17, 0x2A),
    border: Color32::from_rgb(0x85, 0x90, 0xB8),
    focus: Color32::from_rgb(0xFF, 0xB4, 0x54),
    error: Color32::from_rgb(0xFF, 0xA3, 0xA3),
};

pub const LIGHT: Palette = Palette {
    background: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    panel: Color32::from_rgb(0xF1, 0xF3, 0xFA),
    raised: Color32::from_rgb(0xE4, 0xE8, 0xF6),
    raised_hover: Color32::from_rgb(0xD3, 0xDA, 0xF1),
    text: Color32::from_rgb(0x1B, 0x1F, 0x33),
    muted: Color32::from_rgb(0x4A, 0x50, 0x68),
    accent: Color32::from_rgb(0x2B, 0x3A, 0xB8),
    accent_fill: Color32::from_rgb(0x2B, 0x3A, 0xB8),
    on_accent: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    border: Color32::from_rgb(0x6B, 0x73, 0x91),
    focus: Color32::from_rgb(0xC2, 0x41, 0x0C),
    error: Color32::from_rgb(0xB4, 0x23, 0x18),
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
    v.panel_fill = p.background;
    v.window_fill = p.panel;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.extreme_bg_color = p.panel;
    v.faint_bg_color = p.panel;
    v.text_edit_bg_color = Some(p.panel);
    v.error_fg_color = p.error;
    v.warn_fg_color = p.error;
    v.selection.bg_fill = p.accent_fill;
    v.selection.stroke = Stroke::new(1.5, p.on_accent);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.panel;
    w.noninteractive.weak_bg_fill = p.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
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

    // egui draws "pressed" and "has the keyboard" the same way, so both get the indigo fill with navy (or white) text.
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
            style.text_styles.insert(TextStyle::Heading, FontId::new(26.0, fonts::bold_family()));
            style.text_styles.insert(TextStyle::Body, FontId::new(16.0, egui::FontFamily::Proportional));
            style.text_styles.insert(TextStyle::Button, FontId::new(16.0, egui::FontFamily::Proportional));
            style.text_styles.insert(TextStyle::Small, FontId::new(13.5, egui::FontFamily::Proportional));
            style.text_styles.insert(TextStyle::Monospace, FontId::new(15.0, egui::FontFamily::Monospace));
            style.spacing.item_spacing = egui::vec2(10.0, 9.0);
            style.spacing.button_padding = egui::vec2(14.0, 8.0);
            style.spacing.interact_size.y = 36.0;
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
fn contrast(a: Color32, b: Color32) -> f64 {
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
    fn text_has_at_least_four_and_a_half_to_one_on_everything_it_sits_on() {
        for (name, p) in each() {
            for (what, text) in [("text", p.text), ("muted text", p.muted), ("link", p.accent)] {
                for (on, back) in [("the page", p.background), ("a panel", p.panel)] {
                    assert!(contrast(text, back) >= 4.5, "{name}: {what} on {on}: {:.2}", contrast(text, back));
                }
            }
            for (what, text) in [("text", p.text), ("muted text", p.muted)] {
                for (on, back) in [("a button", p.raised), ("a hovered button", p.raised_hover)] {
                    assert!(contrast(text, back) >= 4.5, "{name}: {what} on {on}: {:.2}", contrast(text, back));
                }
            }
            assert!(
                contrast(p.on_accent, p.accent_fill) >= 4.5,
                "{name}: text on indigo: {:.2}",
                contrast(p.on_accent, p.accent_fill)
            );
            assert!(
                contrast(p.error, p.background) >= 4.5,
                "{name}: error text: {:.2}",
                contrast(p.error, p.background)
            );
            assert!(contrast(p.error, p.panel) >= 4.5, "{name}: error text on a panel");
        }
    }

    #[test]
    fn borders_and_the_focus_ring_have_at_least_three_to_one_on_what_they_touch() {
        for (name, p) in each() {
            for (what, colour) in [("border", p.border), ("focus ring", p.focus), ("indigo", p.accent)] {
                for (on, back) in [("the page", p.background), ("a panel", p.panel), ("a button", p.raised)] {
                    assert!(contrast(colour, back) >= 3.0, "{name}: {what} on {on}: {:.2}", contrast(colour, back));
                }
            }
            // The ring is drawn outside the control with a gap, so what it touches is the page or the panel, never the control's own fill.
            assert!(contrast(p.accent_fill, p.background) >= 3.0, "{name}: an indigo fill against the page");
        }
    }

    #[test]
    fn white_text_on_the_light_indigo_would_fail_so_the_dark_scheme_uses_navy() {
        let light_indigo = Color32::from_rgb(0x7D, 0x87, 0xF2);
        assert!(
            contrast(Color32::WHITE, light_indigo) < 4.5,
            "white on #7D87F2 is {:.2}",
            contrast(Color32::WHITE, light_indigo)
        );
        assert_eq!(DARK.accent_fill, light_indigo);
        assert_eq!(DARK.on_accent, DARK.background, "navy on indigo");
        assert!(contrast(DARK.on_accent, DARK.accent_fill) > 5.0);
    }

    #[test]
    fn the_brand_colours_are_the_ones_the_spec_names() {
        assert_eq!(DARK.background, Color32::from_rgb(0x0B, 0x17, 0x2A));
        assert_eq!(DARK.panel, Color32::from_rgb(0x10, 0x20, 0x37));
        assert_eq!(DARK.accent, Color32::from_rgb(0x9C, 0xA5, 0xFF));
        assert!((contrast(DARK.accent, DARK.background) - 7.9).abs() < 0.3, "the spec says about 7.9 to 1");
        assert!((contrast(DARK.accent_fill, DARK.background) - 5.6).abs() < 0.3, "the spec says about 5.6 to 1");
    }

    #[test]
    fn contrast_is_computed_the_standard_way() {
        assert!((contrast(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 1e-9);
        assert!((contrast(Color32::WHITE, Color32::WHITE) - 1.0).abs() < 1e-9);
        assert!((contrast(Color32::from_rgb(0x77, 0x77, 0x77), Color32::WHITE) - 4.48).abs() < 0.02);
    }

    #[test]
    fn the_visuals_use_the_palette_for_text_buttons_and_the_selected_row() {
        for (theme, p) in [(Theme::Dark, DARK), (Theme::Light, LIGHT)] {
            let v = visuals(&p, theme);
            assert_eq!(v.widgets.noninteractive.fg_stroke.color, p.text);
            assert_eq!(v.widgets.inactive.fg_stroke.color, p.text);
            assert_eq!(
                v.override_text_color, None,
                "one text colour forced over everything would also be forced onto a selected row"
            );
            assert_eq!(v.panel_fill, p.background);
            assert_eq!(v.widgets.inactive.weak_bg_fill, p.raised);
            assert_eq!(v.widgets.hovered.weak_bg_fill, p.raised_hover);
            assert_eq!(v.widgets.active.weak_bg_fill, p.accent_fill);
            assert_eq!(
                v.widgets.active.fg_stroke.color, p.on_accent,
                "text on a pressed or focused button is readable"
            );
            assert_eq!(v.selection.bg_fill, p.accent_fill);
            assert_eq!(v.selection.stroke.color, p.on_accent);
            assert_eq!(v.dark_mode, theme == Theme::Dark);
        }
    }
}
