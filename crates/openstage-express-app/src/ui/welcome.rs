// SPDX-License-Identifier: Apache-2.0
//! The first run (milestone 8): a welcome page, shown once, before the first time Home is seen. It says in a few words what the program
//! does and what it never does, and asks the three things worth asking at the start: the language of the program's words (the language
//! Windows uses is chosen to begin with), the colours, and where guides are kept. Everything it asks is in Settings too. Nothing is
//! sent anywhere and no account is made.

use eframe::egui::{self, RichText};

use super::widgets::{self, palette};
use crate::i18n::{k, tr, tt, Lang};
use crate::settings::{AppSettings, Appearance};

/// What the person asked for on this screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WelcomeAction {
    None,
    /// Choose another folder for guides.
    ChooseFolder,
    /// Go to Home.
    Start,
    /// Go to the example guide.
    OpenExample,
}

/// What the screen shows besides the settings it changes.
pub struct WelcomeView {
    /// Where guides are kept.
    pub guides_folder: String,
    /// The capture hotkeys in a sentence (empty if none could be set).
    pub hotkeys: String,
}

pub fn show(ui: &mut egui::Ui, settings: &mut AppSettings, view: &WelcomeView) -> WelcomeAction {
    let mut action = WelcomeAction::None;
    egui::CentralPanel::default().frame(super::shell::page_frame(ui)).show(ui, |ui| {
        let p = palette(ui);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(super::shell::PAGE_WIDTH));
            ui.spacing_mut().item_spacing.y = 6.0;
            widgets::heading(ui, tr("Welcome to OpenStage Capture Express"));
            ui.label(
                RichText::new(tr(
                    "It turns what you do on screen into a step-by-step guide: a picture for each step with a marker where you clicked, \
                     and a title already written. You then change what you like and export it.",
                ))
                .color(p.text),
            );
            ui.add_space(8.0);
            widgets::card_frame(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::subheading(ui, tr("What it does and does not do"));
                for line in [
                    k("Everything stays on this computer. There is no account and no sign-in."),
                    k("It reaches the internet only when you choose Check for updates, and then only to ask GitHub for the list of releases."),
                    k("It never records which keys you press. A step for typing says which box you typed in, not what you typed, and never for a password."),
                    k("A light shows on the capture bar whenever it is capturing, and nothing is noted while it is paused."),
                ] {
                    ui.label(RichText::new(tr(line)).color(p.text));
                }
            });
            ui.add_space(8.0);
            widgets::card_frame(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::subheading(ui, tr("Language of the program"));
                ui.label(RichText::new(tr("The words on the buttons and in the menus. The language Windows uses is chosen to begin with.")).color(p.muted));
                ui.horizontal_wrapped(|ui| {
                    for lang in Lang::ALL {
                        let r = ui.radio_value(&mut settings.interface_language, lang, lang.own_name());
                    let name = tt(format!("Language of the program: {}", lang.own_name()));
                    ui.ctx().accesskit_node_builder(r.id, move |node| node.set_label(name));
                        widgets::ring(ui, &r);
                    }
                });
            });
            ui.add_space(8.0);
            widgets::card_frame(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::subheading(ui, tr("Colours"));
                ui.horizontal_wrapped(|ui| {
                    for (value, words) in [
                        (Appearance::System, k("Follow Windows")),
                        (Appearance::Light, k("Light")),
                        (Appearance::Dark, k("Dark")),
                    ] {
                        let r = ui.radio_value(&mut settings.appearance, value, tr(words));
                        widgets::ring(ui, &r);
                    }
                });
            });
            ui.add_space(8.0);
            widgets::card_frame(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::subheading(ui, tr("Where your guides are kept"));
                let folder = ui.label(RichText::new(&view.guides_folder).color(p.text));
                let name = tt(format!("Guides folder: {}", view.guides_folder));
                ui.ctx().accesskit_node_builder(folder.id, |node| node.set_label(name));
                if widgets::button(ui, tr("Choose another folder...")).clicked() {
                    action = WelcomeAction::ChooseFolder;
                }
            });
            ui.add_space(8.0);
            widgets::card_frame(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::subheading(ui, tr("Keys that work in any program"));
                if view.hotkeys.is_empty() {
                    ui.label(RichText::new(tr("No hotkeys are set.")).color(p.text));
                } else {
                    ui.label(RichText::new(&view.hotkeys).color(p.text));
                }
                ui.label(RichText::new(tr("You can change them in Tools > Settings. Ctrl+K finds any command by its name.")).color(p.muted));
            });
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                if widgets::primary_button(ui, tr("Start")).clicked() {
                    action = WelcomeAction::Start;
                }
                if widgets::button(ui, tr("Look at the example guide first")).clicked() {
                    action = WelcomeAction::OpenExample;
                }
            });
            ui.add_space(8.0);
        });
    });
    action
}
