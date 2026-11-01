// SPDX-License-Identifier: Apache-2.0
//! The Settings screen (spec section 4.2): one plain page, with the less used choices folded under "Advanced". Every choice is a
//! radio button, a check box or a button, so it works the same by keyboard and with a screen reader. Each change is saved at once and
//! said in the status line.

use eframe::egui::{self, RichText};

use super::widgets::{self, palette};
use crate::settings::{
    AppSettings, Appearance, KeyChoice, ModeChoice, TitleLanguage, SCREENSHOT_DELAYS_MS, TYPING_DELAYS_MS,
};

/// What the screen shows besides the settings themselves.
pub struct SettingsInfo {
    pub guides_folder: String,
    pub exports_folder: String,
    /// The hotkeys in a sentence (empty if none could be set).
    pub hotkeys: String,
    /// Something to tell the person about the settings file (damaged, newer), if anything.
    pub note: Option<String>,
}

/// What the person asked for on this screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsAction {
    None,
    Back,
    ChooseGuidesFolder,
    ChooseExportsFolder,
    /// Something was changed (the caller saves).
    Changed,
}

fn seconds_words(ms: u64) -> String {
    if ms == 0 {
        "None".to_string()
    } else if ms.is_multiple_of(1000) {
        format!("{} second{}", ms / 1000, if ms == 1000 { "" } else { "s" })
    } else {
        format!("{ms} milliseconds")
    }
}

pub fn show(ui: &mut egui::Ui, settings: &mut AppSettings, info: &SettingsInfo) -> SettingsAction {
    let before = settings.clone();
    let mut action = SettingsAction::None;
    egui::Panel::top("settings-top").show(ui, |ui| {
        ui.horizontal(|ui| {
            if widgets::button(ui, "Back to guides").clicked() {
                action = SettingsAction::Back;
            }
            widgets::heading(ui, "Settings");
        });
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let p = palette(ui);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            if let Some(note) = &info.note {
                ui.label(RichText::new(note).color(p.text));
            }

            widgets::subheading(ui, "Appearance");
            for (value, words) in
                [(Appearance::System, "Follow Windows"), (Appearance::Light, "Light"), (Appearance::Dark, "Dark")]
            {
                let r = ui.radio_value(&mut settings.appearance, value, words);
                widgets::ring(ui, &r);
            }

            ui.add_space(8.0);
            widgets::subheading(ui, "Capturing");
            ui.label(RichText::new("A click makes a step:").color(p.muted));
            for (value, words) in [
                (ModeChoice::OnClick, "every time"),
                (ModeChoice::WithKey, "only when I hold a key while I click"),
                (ModeChoice::ExceptKey, "every time except when I hold a key while I click"),
                (ModeChoice::HotkeyOnly, "never. Only the capture now hotkey makes a step"),
            ] {
                let r = ui.radio_value(&mut settings.capture_mode, value, words);
                widgets::ring(ui, &r);
            }
            let uses_key = matches!(settings.capture_mode, ModeChoice::WithKey | ModeChoice::ExceptKey);
            ui.label(RichText::new("The key to hold:").color(p.muted));
            ui.horizontal(|ui| {
                for key in [KeyChoice::Ctrl, KeyChoice::Shift, KeyChoice::Alt] {
                    let r = ui.add_enabled(uses_key, egui::RadioButton::new(settings.capture_key == key, key.name()));
                    widgets::ring(ui, &r);
                    if r.clicked() {
                        settings.capture_key = key;
                    }
                }
            });
            for (value, words) in [
                (&mut settings.show_cursor, "Draw a pointer at each click"),
                (&mut settings.mark_clicked_area, "Draw a circle round each click"),
                (&mut settings.capture_typing, "Make a step for what I type (never for a password)"),
                (&mut settings.focused_view_for_new_steps, "Start new steps with a close-up of the click"),
            ] {
                let r = ui.checkbox(value, words);
                widgets::ring(ui, &r);
            }
            ui.label(RichText::new("The language of step titles:").color(p.muted));
            ui.horizontal(|ui| {
                for language in [TitleLanguage::En, TitleLanguage::De, TitleLanguage::Fr] {
                    let r = ui.radio_value(&mut settings.title_language, language, language.name());
                    widgets::ring(ui, &r);
                }
            });
            ui.label(
                RichText::new("Changes to capturing apply the next time you start capturing.")
                    .size(13.5)
                    .color(p.muted),
            );

            ui.add_space(8.0);
            widgets::subheading(ui, "Folders");
            let guides = ui.label(RichText::new(&info.guides_folder).color(p.text));
            let name = format!("Guides folder: {}", info.guides_folder);
            ui.ctx().accesskit_node_builder(guides.id, |node| node.set_label(name));
            if widgets::button(ui, "Choose guides folder...").clicked() {
                action = SettingsAction::ChooseGuidesFolder;
            }
            let exports = ui.label(RichText::new(&info.exports_folder).color(p.text));
            let name = format!("Exports folder: {}", info.exports_folder);
            ui.ctx().accesskit_node_builder(exports.id, |node| node.set_label(name));
            if widgets::button(ui, "Choose exports folder...").clicked() {
                action = SettingsAction::ChooseExportsFolder;
            }
            let r = ui.checkbox(&mut settings.open_folder_after_export, "Open the folder when an export is done");
            widgets::ring(ui, &r);

            ui.add_space(8.0);
            widgets::subheading(ui, "Hotkeys");
            let words = if info.hotkeys.is_empty() { "No hotkeys could be set." } else { info.hotkeys.as_str() };
            ui.label(RichText::new(words).color(p.text));
            ui.label(RichText::new("Changing the hotkeys is not built yet.").size(13.5).color(p.muted));

            ui.add_space(8.0);
            let header =
                egui::CollapsingHeader::new(RichText::new("Advanced").size(19.0).family(super::fonts::bold_family()))
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Wait before taking a picture after a click:").color(p.muted));
                        ui.horizontal_wrapped(|ui| {
                            for ms in SCREENSHOT_DELAYS_MS {
                                let r = ui.radio_value(&mut settings.screenshot_delay_ms, ms, seconds_words(ms));
                                widgets::ring(ui, &r);
                            }
                        });
                        ui.label(RichText::new("Typing ends after this long without a key:").color(p.muted));
                        ui.horizontal_wrapped(|ui| {
                            for ms in TYPING_DELAYS_MS {
                                let r = ui.radio_value(&mut settings.typing_delay_ms, ms, seconds_words(ms));
                                widgets::ring(ui, &r);
                            }
                        });
                        let r = ui.checkbox(
                            &mut settings.capture_outside_clicks,
                            "Make steps for clicks outside the captured screen",
                        );
                        widgets::ring(ui, &r);
                        let r = ui.checkbox(
                            &mut settings.hotkey_captures_cursor,
                            "The capture now hotkey also marks where the pointer is",
                        );
                        widgets::ring(ui, &r);
                    });
            widgets::ring(ui, &header.header_response);
        });
    });
    if action == SettingsAction::None && *settings != before {
        action = SettingsAction::Changed;
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_are_said_in_words() {
        assert_eq!(seconds_words(0), "None");
        assert_eq!(seconds_words(250), "250 milliseconds");
        assert_eq!(seconds_words(1000), "1 second");
        assert_eq!(seconds_words(2000), "2 seconds");
    }
}
