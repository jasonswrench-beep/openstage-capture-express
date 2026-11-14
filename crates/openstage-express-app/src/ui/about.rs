// SPDX-License-Identifier: Apache-2.0
//! The About screen: what the program is, its licence, and the licences of what it is built from. The texts are the project's own
//! `NOTICE` and `THIRD-PARTY.md`, compiled in, so they are always the ones that match this program.

use eframe::egui::{self, RichText};
use openstage_express_core::updates::{is_release_page, release, spoken, UpdateAnswer};

use super::widgets::{self, palette};

const NOTICE: &str = include_str!("../../../../NOTICE");
const THIRD_PARTY: &str = include_str!("../../../../THIRD-PARTY.md");

/// The program's version, such as `0.1.0`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The third-party list as plain lines, `name version: licence`, from the rows of the table in `THIRD-PARTY.md`.
pub fn third_party_lines() -> Vec<String> {
    THIRD_PARTY
        .lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.trim().trim_matches('|').split('|').map(str::trim).collect();
            let looks_like_a_row = line.trim_start().starts_with('|') && cells.len() == 3;
            let is_header = cells.first().is_some_and(|c| c.eq_ignore_ascii_case("crate"))
                || cells.iter().all(|c| c.starts_with('-'));
            (looks_like_a_row && !is_header).then(|| format!("{} {}: {}", cells[0], cells[1], cells[2]))
        })
        .collect()
}

/// The notice paragraphs: the `NOTICE` file split at its blank lines, each paragraph on one line.
pub fn notice_paragraphs() -> Vec<String> {
    NOTICE
        .replace("\r\n", "\n")
        .split("\n\n")
        .map(|p| p.lines().map(str::trim).collect::<Vec<_>>().join(" "))
        .filter(|p| !p.is_empty() && p != "---")
        .collect()
}

/// What the About screen shows of Check for updates.
pub struct AboutView<'a> {
    /// A check is under way.
    pub checking: bool,
    /// The answer of the last check, if one was made.
    pub answer: Option<&'a UpdateAnswer>,
}

/// What the person asked for on the About screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AboutAction {
    None,
    Back,
    /// Check for updates now.
    Check,
    /// Open this page of the releases in the browser (only ever a release page).
    OpenPage(String),
}

/// The version line: the program's version, and the release it was built as when it was built for one.
pub fn version_words() -> String {
    if release().is_empty() {
        format!("Version {}.", version())
    } else {
        format!("Version {}, release {}.", version(), spoken(release()))
    }
}

/// Draw the screen.
pub fn show(ui: &mut egui::Ui, view: &AboutView<'_>) -> AboutAction {
    let mut action = AboutAction::None;
    if super::shell::screen_header(ui, "about-top", "Back to guides", "About OpenStage Capture Express", false) {
        action = AboutAction::Back;
    }
    egui::CentralPanel::default().frame(super::shell::page_frame(ui)).show(ui, |ui| {
        let p = palette(ui);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.label(version_words());
            ui.label("A free program that turns what you do on screen into a step-by-step guide. It has no account and sends nothing about you or your guides anywhere. The one network request it makes is Check for updates, and only when you choose it.");
            ui.label("A typing step says which field was typed in. If you choose, in Settings, to have it say what was typed, the text is read from the field when you finish typing, never recorded key by key, and never for a password field.");
            ui.add_space(8.0);
            widgets::subheading(ui, "Updates");
            if view.checking {
                ui.label(RichText::new("Checking for updates. Please wait.").color(p.muted));
            } else if let Some(answer) = view.answer {
                ui.label(RichText::new(&answer.message).color(p.text));
            } else {
                ui.label(RichText::new("Check for updates asks GitHub, once, whether a newer Express is published. It installs nothing.").color(p.muted));
            }
            ui.horizontal_wrapped(|ui| {
                if widgets::button_enabled(ui, !view.checking, "Check for updates").clicked() {
                    action = AboutAction::Check;
                }
                if let Some(url) = view.answer.and_then(|a| a.url.as_deref()).filter(|u| is_release_page(u)) {
                    if widgets::button(ui, "Open the release page").clicked() {
                        action = AboutAction::OpenPage(url.to_string());
                    }
                }
            });
            ui.add_space(8.0);
            widgets::subheading(ui, "Licence and notices");
            for paragraph in notice_paragraphs() {
                ui.label(RichText::new(paragraph).color(p.text));
            }
            ui.add_space(8.0);
            widgets::subheading(ui, "Programs and libraries this one is built from");
            ui.label(RichText::new("Each is used under the licence shown.").color(p.muted));
            for line in third_party_lines() {
                ui.label(RichText::new(line).size(13.5).color(p.muted));
            }
        });
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notice_is_split_into_readable_paragraphs_and_names_the_licence() {
        let paragraphs = notice_paragraphs();
        assert!(paragraphs.len() >= 5, "{paragraphs:?}");
        assert!(paragraphs.iter().any(|p| p.contains("Apache License, Version 2.0")));
        assert!(paragraphs.iter().any(|p| p.starts_with("Roboto")), "the bundled font is credited");
        assert!(paragraphs.iter().all(|p| !p.contains('\n')));
    }

    #[test]
    fn the_third_party_list_has_one_line_per_crate_with_its_licence() {
        let lines = third_party_lines();
        assert!(lines.len() > 100, "{}", lines.len());
        assert!(lines.iter().any(|l| l.starts_with("egui ") && l.contains("MIT")), "egui is listed");
        assert!(lines.iter().all(|l| l.contains(": ") && !l.contains('|')));
        assert!(!lines.iter().any(|l| l.starts_with("Crate ") || l.starts_with("---")), "no table header");
    }

    #[test]
    fn the_version_is_the_programs() {
        assert!(!version().is_empty());
        assert!(version().chars().next().is_some_and(|c| c.is_ascii_digit()));
    }
}
