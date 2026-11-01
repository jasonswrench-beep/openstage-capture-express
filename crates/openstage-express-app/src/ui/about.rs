// SPDX-License-Identifier: Apache-2.0
//! The About screen: what the program is, its licence, and the licences of what it is built from. The texts are the project's own
//! `NOTICE` and `THIRD-PARTY.md`, compiled in, so they are always the ones that match this program.

use eframe::egui::{self, RichText};

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

/// Draw the screen. Returns true when the person asked to go back.
pub fn show(ui: &mut egui::Ui) -> bool {
    let mut back = false;
    egui::Panel::top("about-top").show(ui, |ui| {
        ui.horizontal(|ui| {
            if widgets::button(ui, "Back to guides").clicked() {
                back = true;
            }
            widgets::heading(ui, "About OpenStage Capture Express");
        });
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let p = palette(ui);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.label(format!("Version {}.", version()));
            ui.label("A free program that turns what you do on screen into a step-by-step guide. It has no account, makes no network connection, and sends nothing anywhere.");
            ui.label("What you type is read from the field when you finish typing, never recorded key by key, and never for a password field.");
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
    back
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
