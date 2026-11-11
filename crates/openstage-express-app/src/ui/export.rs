// SPDX-License-Identifier: Apache-2.0
//! The Export screen (spec section 4.2): which formats, which folder, and a button. It draws the choices and says what was pressed;
//! making the exports is in `export_run.rs`, and it happens on a thread of its own so the window never freezes while a PDF is made.

use std::path::PathBuf;

use eframe::egui::{self, RichText};

use super::widgets::{self, palette};
use crate::export_run::{Format, PictureChoices, Selection};
use crate::settings::DocumentPicturesChoice;
use openstage_express_export::images::{DEFAULT_JPEG_QUALITY, JPEG_QUALITY_CHOICES};
use openstage_express_export::plan::{DEFAULT_IMAGE_WIDTH, DEFAULT_PREVIEW_STEPS, IMAGE_WIDTH_CHOICES};

/// The most steps a preview can be asked for (a guide with more than this is exported whole anyway).
pub const MAX_PREVIEW_STEPS: usize = 9999;

/// Read a JPG quality typed in a box: a whole number from 1 to 100.
pub fn parse_quality(text: &str) -> Result<u8, String> {
    match text.trim().parse::<u8>() {
        Ok(n) if (1..=100).contains(&n) => Ok(n),
        _ => Err("Type a whole number from 1 to 100 for the quality, such as 80.".to_string()),
    }
}

/// Read the number of steps for a preview typed in a box: a whole number from 1 to 9999.
pub fn parse_steps(text: &str) -> Result<usize, String> {
    match text.trim().parse::<usize>() {
        Ok(n) if (1..=MAX_PREVIEW_STEPS).contains(&n) => Ok(n),
        _ => Err(format!("Type a whole number from 1 to {MAX_PREVIEW_STEPS} for the number of steps, such as 5.")),
    }
}

/// What the Export screen holds between frames.
pub struct ExportState {
    pub selection: Selection,
    /// The folder every export goes into (a folder for the guide, and in it a new folder for each run, are made inside it).
    pub folder: PathBuf,
    /// Open the new folder when the exports are done.
    pub open_after: bool,
    /// An export is being made now.
    pub working: bool,
    /// What the last export said.
    pub said: Option<String>,
    /// Whether this computer can make a PDF.
    pub can_pdf: bool,
    /// For a copy as one file: put the original pictures in it (off by default, so the marks are drawn in and a blur cannot be undone).
    pub copy_originals: bool,
    /// How many pixels across every picture is made; always one of `IMAGE_WIDTH_CHOICES`.
    pub width: u32,
    /// How the web page, Word, PowerPoint and PDF keep their pictures: PNG (every pixel as it is), JPG (smaller files), or whichever is
    /// smaller for each picture.
    pub documents: DocumentPicturesChoice,
    /// The JPG quality last chosen or typed, from 1 to 100. It is what is kept in the settings.
    pub jpg_quality: u8,
    /// "Another number" is the choice for the JPG quality, and the number is what is in `jpg_text`.
    pub jpg_other: bool,
    pub jpg_text: String,
    /// Make only the first steps, to look at the result quickly. This is not kept between runs of the program: a preview must never be
    /// made by accident.
    pub preview: bool,
    /// The number of steps in a preview, last chosen or typed (kept in the settings), and what is typed in its box.
    pub preview_steps: usize,
    pub preview_text: String,
}

impl ExportState {
    pub fn new(folder: PathBuf, can_pdf: bool) -> ExportState {
        let mut selection = Selection::starting();
        if !can_pdf {
            selection.set(Format::Pdf, false);
            selection.set(Format::Word, true);
        }
        let mut state = ExportState {
            selection,
            folder,
            open_after: true,
            working: false,
            said: None,
            can_pdf,
            copy_originals: false,
            width: DEFAULT_IMAGE_WIDTH,
            documents: DocumentPicturesChoice::Png,
            jpg_quality: DEFAULT_JPEG_QUALITY,
            jpg_other: false,
            jpg_text: String::new(),
            preview: false,
            preview_steps: DEFAULT_PREVIEW_STEPS,
            preview_text: String::new(),
        };
        state.set_pictures(DEFAULT_IMAGE_WIDTH, u32::from(DEFAULT_JPEG_QUALITY), DEFAULT_PREVIEW_STEPS);
        state
    }

    /// Use the picture choices kept in the settings. A width that is not on offer becomes the nearest one that is, a quality is kept
    /// between 1 and 100, and a number of steps between 1 and the most.
    pub fn set_pictures(&mut self, width: u32, jpg_quality: u32, preview_steps: usize) {
        self.width =
            IMAGE_WIDTH_CHOICES.into_iter().min_by_key(|choice| choice.abs_diff(width)).unwrap_or(DEFAULT_IMAGE_WIDTH);
        self.jpg_quality = jpg_quality.clamp(1, 100) as u8;
        self.jpg_other = !JPEG_QUALITY_CHOICES.contains(&self.jpg_quality);
        self.jpg_text = self.jpg_quality.to_string();
        self.preview_steps = preview_steps.clamp(1, MAX_PREVIEW_STEPS);
        self.preview_text = self.preview_steps.to_string();
    }

    /// The JPG quality as chosen now: a preset, or what is typed in the box (which must be a number from 1 to 100).
    pub fn jpg_quality_now(&self) -> Result<u8, String> {
        if self.jpg_other {
            parse_quality(&self.jpg_text)
        } else {
            Ok(self.jpg_quality)
        }
    }

    /// The number of steps for a preview as typed now, when a preview is wanted.
    pub fn first_steps_now(&self) -> Result<Option<usize>, String> {
        if self.preview {
            parse_steps(&self.preview_text).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Everything chosen about the pictures, or why it cannot be used yet (a box with something in it that is not a number).
    pub fn picture_choices(&self) -> Result<PictureChoices, String> {
        Ok(PictureChoices {
            width: self.width,
            jpg_quality: self.jpg_quality_now()?,
            first_steps: self.first_steps_now()?,
            documents: self.documents,
        })
    }
}

/// A radio button whose words on the screen are short and whose name for a screen reader says what it is a choice of, because a
/// screen reader says the name of the button the keyboard is on without the heading above it.
fn choice(ui: &mut egui::Ui, selected: bool, shown: &str, name: &str) -> egui::Response {
    let response = ui.add(egui::RadioButton::new(selected, shown));
    widgets::ring(ui, &response);
    let name = name.to_string();
    ui.ctx().accesskit_node_builder(response.id, move |node| node.set_label(name));
    response
}

/// What a quality number means in words.
fn quality_words(quality: u8) -> &'static str {
    match quality {
        100 => "best picture",
        90 => "the usual",
        70 => "smaller file",
        50 => "smallest file",
        _ => "",
    }
}

/// What the person asked for on this screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportAction {
    None,
    Back,
    ChooseFolder,
    Export,
    /// Save the guide as one file, a copy to send or keep.
    SaveCopy,
}

pub fn show(ui: &mut egui::Ui, state: &mut ExportState) -> ExportAction {
    let mut action = ExportAction::None;
    if super::shell::screen_header(ui, "export-top", "Back to the guide", "Export", false) {
        action = ExportAction::Back;
    }
    egui::CentralPanel::default().frame(super::shell::page_frame(ui)).show(ui, |ui| {
        let p = palette(ui);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(super::shell::PAGE_WIDTH));
widgets::card_frame(ui).show(ui, |ui| {
ui.set_width(ui.available_width());
            widgets::subheading(ui, "Formats");
            ui.label(RichText::new("Each one you choose goes into a folder of its own.").color(p.muted));
            ui.add_space(4.0);
            ui.spacing_mut().item_spacing.y = 5.0;
            for format in Format::ALL {
                let mut on = state.selection.contains(format);
                let enabled = format != Format::Pdf || state.can_pdf;
                let about = if enabled {
                    format.about().to_string()
                } else {
                    "needs the Microsoft Edge WebView2 Runtime, which this computer does not have".to_string()
                };
                ui.horizontal(|ui| {
                    let response = ui.add_enabled(enabled, egui::Checkbox::new(&mut on, format.name()));
                    widgets::ring(ui, &response);
                    if response.changed() {
                        state.selection.set(format, on);
                    }
                    ui.label(RichText::new(about).size(13.5).color(p.muted));
                });
            }
});
ui.add_space(12.0);
widgets::card_frame(ui).show(ui, |ui| {
ui.set_width(ui.available_width());
            widgets::subheading(ui, "How wide the pictures are");
            ui.label(
                RichText::new(
                    "Every picture in the exports is made this many pixels across, and its height follows. Wider is sharper and makes \
                     bigger files.",
                )
                .color(p.muted),
            );
            ui.horizontal_wrapped(|ui| {
                for width in IMAGE_WIDTH_CHOICES {
                    let shown =
                        if width == DEFAULT_IMAGE_WIDTH { format!("{width} (the usual)") } else { width.to_string() };
                    let r = choice(ui, state.width == width, &shown, &format!("Picture width {width} pixels"));
                    if r.clicked() {
                        state.width = width;
                    }
                }
            });
            ui.add_space(6.0);
            widgets::subheading(ui, "Pictures inside the web page, Word, PowerPoint and PDF");
            ui.label(
                RichText::new(
                    "These four hold their pictures inside themselves. PNG keeps every pixel as it is. JPG usually makes the file a good \
                     deal smaller for ordinary screenshots and loses a little, as set by the quality below; a plain screen with big \
                     flat areas can be smaller as PNG. Whichever is smaller looks at each picture and keeps the JPG only when it is \
                     smaller than the PNG, so a plain screen stays an exact PNG and a busy one becomes a JPG. The folders of pictures, \
                     Markdown, JSON and the GIF are not changed by this.",
                )
                .color(p.muted),
            );
            ui.horizontal_wrapped(|ui| {
                for (which, shown, name) in [
                    (
                        DocumentPicturesChoice::Png,
                        "PNG (every pixel kept)",
                        "Pictures inside the web page, Word, PowerPoint and PDF: PNG, every pixel kept",
                    ),
                    (
                        DocumentPicturesChoice::Jpg,
                        "JPG (smaller files)",
                        "Pictures inside the web page, Word, PowerPoint and PDF: JPG, smaller files",
                    ),
                    (
                        DocumentPicturesChoice::Smaller,
                        "Whichever is smaller for each picture",
                        "Pictures inside the web page, Word, PowerPoint and PDF: whichever is smaller for each picture, PNG or JPG",
                    ),
                ] {
                    if choice(ui, state.documents == which, shown, name).clicked() {
                        state.documents = which;
                    }
                }
            });
            ui.add_space(6.0);
            widgets::subheading(ui, "Quality of the JPG pictures");
            ui.label(
                RichText::new(
                    "For Pictures (JPG), and for the pictures inside the web page, Word, PowerPoint and PDF when JPG or whichever is \
                     smaller is chosen above. A lower number makes a smaller file and a rougher picture.",
                )
                .color(p.muted),
            );
            ui.horizontal_wrapped(|ui| {
                for quality in JPEG_QUALITY_CHOICES {
                    let shown = format!("{quality} ({})", quality_words(quality));
                    let selected = !state.jpg_other && state.jpg_quality == quality;
                    let r = choice(ui, selected, &shown, &format!("JPG quality {quality}, {}", quality_words(quality)));
                    if r.clicked() {
                        state.jpg_other = false;
                        state.jpg_quality = quality;
                    }
                }
                let r = choice(ui, state.jpg_other, "Another number", "JPG quality, another number typed in the box");
                if r.clicked() && !state.jpg_other {
                    state.jpg_other = true;
                    state.jpg_text = state.jpg_quality.to_string();
                }
            });
            ui.horizontal(|ui| {
                let label = ui.label("Quality, 1 to 100");
                let field =
                    ui.add(egui::TextEdit::singleline(&mut state.jpg_text).desired_width(60.0)).labelled_by(label.id);
                widgets::ring(ui, &field);
                if field.changed() {
                    state.jpg_other = true;
                    if let Ok(quality) = parse_quality(&state.jpg_text) {
                        state.jpg_quality = quality;
                    }
                }
            });
            if state.jpg_other {
                if let Err(problem) = parse_quality(&state.jpg_text) {
                    ui.label(RichText::new(problem).color(p.text));
                }
            }
});
ui.add_space(12.0);
widgets::card_frame(ui).show(ui, |ui| {
ui.set_width(ui.available_width());
            widgets::subheading(ui, "Try it on a few steps first");
            ui.label(
                RichText::new(
                    "A preview makes only the first steps, so a long guide shows you how it will look in a moment. Substeps count as \
                     steps. Its folder and the guide's title say it is a preview, so it is not taken for the whole guide.",
                )
                .color(p.muted),
            );
            let preview = ui.add(egui::Checkbox::new(&mut state.preview, "Make a preview of the first steps only"));
            widgets::ring(ui, &preview);
            ui.horizontal(|ui| {
                let label = ui.label("Steps in the preview");
                let field = ui
                    .add(egui::TextEdit::singleline(&mut state.preview_text).desired_width(60.0))
                    .labelled_by(label.id);
                widgets::ring(ui, &field);
                if field.changed() {
                    state.preview = true;
                    if let Ok(steps) = parse_steps(&state.preview_text) {
                        state.preview_steps = steps;
                    }
                }
            });
            if state.preview {
                if let Err(problem) = parse_steps(&state.preview_text) {
                    ui.label(RichText::new(problem).color(p.text));
                }
            }
});
ui.add_space(12.0);
widgets::card_frame(ui).show(ui, |ui| {
ui.set_width(ui.available_width());
            widgets::subheading(ui, "Where to put them");
            let folder_words = state.folder.display().to_string();
            let shown = ui.label(RichText::new(&folder_words).color(p.text));
            let name = format!("Export folder: {folder_words}");
            ui.ctx().accesskit_node_builder(shown.id, |node| node.set_label(name));
            ui.horizontal(|ui| {
                if widgets::icon_button(ui, super::icon::UiIcon::FolderOpen, "Choose folder...").clicked() {
                    action = ExportAction::ChooseFolder;
                }
            });
            let open = ui.add(egui::Checkbox::new(&mut state.open_after, "Open the folder when it is done"));
            widgets::ring(ui, &open);
            ui.add_space(12.0);
            let words = if state.working {
                "Exporting...".to_string()
            } else if state.selection.is_empty() {
                "Export".to_string()
            } else {
                format!("Export {}", format_count(state.selection.count()))
            };
            let go = ui.add_enabled_ui(!state.working && !state.selection.is_empty(), |ui| {
                widgets::primary_button(ui, &words)
            });
            if go.inner.clicked() {
                action = ExportAction::Export;
            }
            if state.selection.is_empty() {
                ui.label(RichText::new("Choose at least one format.").color(p.muted));
            }
});
ui.add_space(12.0);
widgets::card_frame(ui).show(ui, |ui| {
ui.set_width(ui.available_width());
            widgets::subheading(ui, "A copy as one file");
            ui.label(
                RichText::new(
                    "Saves the whole guide as one file ending .osguide.zip, to send to someone or keep. It goes in the same folder as the \
                     exports. Someone opens it with Open a copy on Home.",
                )
                .color(p.muted),
            );
            let originals = ui.add(egui::Checkbox::new(&mut state.copy_originals, "Include the original pictures"));
            widgets::ring(ui, &originals);
            let kind = if state.copy_originals {
                "The original pictures are in the file, and the marks stay marks. A blur can be taken away by whoever opens it, and \
                 then the picture under it can be seen. Steps you skipped are included."
            } else {
                "Each picture has its marks drawn into it, blur included, and the original pictures are not in the file, so a blur \
                 cannot be undone. Steps you skipped are left out."
            };
            ui.label(RichText::new(format!("{kind} Deleted steps are never included.")).size(13.5).color(p.muted));
            let copy = ui.add_enabled_ui(!state.working, |ui| {
                widgets::icon_button(ui, super::icon::UiIcon::Package, if state.working { "Working..." } else { "Save a copy as one file" })
            });
            if copy.inner.clicked() {
                action = ExportAction::SaveCopy;
            }
            if let Some(said) = &state.said {
                ui.add_space(8.0);
                ui.label(RichText::new(said).color(p.text));
            }
});
ui.add_space(12.0);
        });
    });
    action
}

fn format_count(n: usize) -> String {
    if n == 1 {
        "1 format".to_string()
    } else {
        format!("{n} formats")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_computer_without_a_pdf_engine_starts_with_word_instead() {
        let with = ExportState::new(PathBuf::from("x"), true);
        assert_eq!(with.selection.formats(), [Format::Html, Format::Pdf]);
        let without = ExportState::new(PathBuf::from("x"), false);
        assert_eq!(without.selection.formats(), [Format::Html, Format::Word]);
    }

    #[test]
    fn the_button_counts_in_words() {
        assert_eq!(format_count(1), "1 format");
        assert_eq!(format_count(4), "4 formats");
    }

    #[test]
    fn a_quality_is_a_whole_number_from_1_to_100() {
        for good in ["1", "100", " 80 ", "007", "55"] {
            assert!(parse_quality(good).is_ok(), "{good}");
        }
        assert_eq!(parse_quality("100"), Ok(100));
        assert_eq!(parse_quality(" 7 "), Ok(7));
        for bad in ["0", "101", "", "  ", "abc", "5.5", "-3", "256", "99999999999", "8 0", "1e1"] {
            let why = parse_quality(bad).unwrap_err();
            assert_eq!(why, "Type a whole number from 1 to 100 for the quality, such as 80.", "{bad}");
        }
    }

    #[test]
    fn a_number_of_steps_is_a_whole_number_from_1_to_9999() {
        assert_eq!(parse_steps("1"), Ok(1));
        assert_eq!(parse_steps(" 5 "), Ok(5));
        assert_eq!(parse_steps("9999"), Ok(9999));
        for bad in ["0", "10000", "", "x", "-1", "2.5", "99999999999999999999"] {
            assert!(parse_steps(bad).unwrap_err().contains("from 1 to 9999"), "{bad}");
        }
    }

    #[test]
    fn a_new_state_has_the_usual_picture_choices_and_no_preview() {
        let s = ExportState::new(PathBuf::from("x"), true);
        assert_eq!((s.width, s.jpg_quality, s.jpg_other, s.jpg_text.as_str()), (1280, 90, false, "90"));
        assert_eq!((s.preview, s.preview_steps, s.preview_text.as_str()), (false, 5, "5"));
        assert_eq!(s.picture_choices(), Ok(PictureChoices::default()));
    }

    #[test]
    fn jpg_inside_documents_is_off_to_begin_with_and_follows_the_choice_with_the_quality_in_use() {
        use openstage_express_export::plan::DocumentPictures;
        let mut s = ExportState::new(PathBuf::from("x"), true);
        assert_eq!(s.documents, DocumentPicturesChoice::Png);
        assert_eq!(s.picture_choices().unwrap().export_options().document_pictures, DocumentPictures::Png);
        s.documents = DocumentPicturesChoice::Jpg;
        let choices = s.picture_choices().unwrap();
        assert_eq!(choices.documents, DocumentPicturesChoice::Jpg);
        assert_eq!(choices.export_options().document_pictures, DocumentPictures::Jpg { quality: 90 });
        s.jpg_other = true;
        s.jpg_text = "40".to_string();
        assert_eq!(
            s.picture_choices().unwrap().export_options().document_pictures,
            DocumentPictures::Jpg { quality: 40 },
            "the typed quality is the one used inside documents too"
        );
        s.jpg_text = "abc".to_string();
        assert!(s.picture_choices().is_err(), "a quality that is not a number stops it, as it does for the JPG files");
        s.documents = DocumentPicturesChoice::Png;
        assert!(s.picture_choices().is_err(), "the box is in use while 'another number' is chosen");
        s.jpg_other = false;
        assert_eq!(s.picture_choices().unwrap().export_options().document_pictures, DocumentPictures::Png);
        s.documents = DocumentPicturesChoice::Jpg;
        s.set_pictures(800, 70, 3);
        assert_eq!(
            s.documents,
            DocumentPicturesChoice::Jpg,
            "using the saved width, quality and steps leaves the choice alone"
        );
    }

    #[test]
    fn whichever_is_smaller_follows_the_choice_with_the_quality_in_use() {
        use openstage_express_export::plan::DocumentPictures;
        let mut s = ExportState::new(PathBuf::from("x"), true);
        s.documents = DocumentPicturesChoice::Smaller;
        assert_eq!(
            s.picture_choices().unwrap().export_options().document_pictures,
            DocumentPictures::Smaller { quality: 90 }
        );
        s.jpg_other = true;
        s.jpg_text = "55".to_string();
        assert_eq!(
            s.picture_choices().unwrap().export_options().document_pictures,
            DocumentPictures::Smaller { quality: 55 },
            "the quality typed in the box is the one the JPG of each picture is made with"
        );
        s.set_pictures(800, 70, 3);
        assert_eq!(s.documents, DocumentPicturesChoice::Smaller);
    }

    #[test]
    fn a_width_that_is_not_on_offer_becomes_the_nearest_one_that_is() {
        let mut s = ExportState::new(PathBuf::from("x"), true);
        for (kept, shown) in
            [(640, 640), (1920, 1920), (1000, 1024), (1, 640), (0, 640), (99_999, 1920), (1400, 1280), (1500, 1600)]
        {
            s.set_pictures(kept, 90, 5);
            assert_eq!(s.width, shown, "{kept}");
            assert!(IMAGE_WIDTH_CHOICES.contains(&s.width));
        }
        s.set_pictures(720, 90, 5);
        assert_eq!(s.width, 640, "halfway between two: the smaller one");
    }

    #[test]
    fn a_kept_quality_is_a_preset_or_another_number_and_is_kept_between_1_and_100() {
        let mut s = ExportState::new(PathBuf::from("x"), true);
        for preset in [100, 90, 70, 50] {
            s.set_pictures(1280, preset, 5);
            assert!(!s.jpg_other, "{preset}");
            assert_eq!(s.jpg_quality_now(), Ok(preset as u8));
        }
        s.set_pictures(1280, 77, 5);
        assert!(s.jpg_other);
        assert_eq!((s.jpg_quality_now(), s.jpg_text.as_str()), (Ok(77), "77"));
        s.set_pictures(1280, 0, 5);
        assert_eq!(s.jpg_quality, 1);
        s.set_pictures(1280, 5000, 5);
        assert_eq!(s.jpg_quality, 100);
        assert!(!s.jpg_other, "100 is a preset");
    }

    #[test]
    fn a_kept_number_of_steps_is_kept_between_1_and_the_most() {
        let mut s = ExportState::new(PathBuf::from("x"), true);
        s.set_pictures(1280, 90, 0);
        assert_eq!((s.preview_steps, s.preview_text.as_str()), (1, "1"));
        s.set_pictures(1280, 90, 100_000);
        assert_eq!((s.preview_steps, s.preview_text.as_str()), (MAX_PREVIEW_STEPS, "9999"));
        s.set_pictures(1280, 90, 12);
        assert_eq!(s.preview_text, "12");
        assert!(!s.preview, "whether to make a preview is never carried over");
    }

    #[test]
    fn what_is_typed_counts_only_when_it_is_in_use() {
        let mut s = ExportState::new(PathBuf::from("x"), true);
        // Rubbish in a box nobody is using is no trouble.
        s.jpg_text = "abc".to_string();
        s.preview_text = "0".to_string();
        assert_eq!(s.picture_choices(), Ok(PictureChoices::default()));
        // Once "another number" is chosen, the box is read.
        s.jpg_other = true;
        assert!(s.picture_choices().unwrap_err().contains("quality"));
        s.jpg_text = "35".to_string();
        assert_eq!(s.picture_choices().unwrap().jpg_quality, 35);
        // And once a preview is wanted, so is its box.
        s.preview = true;
        assert!(s.picture_choices().unwrap_err().contains("number of steps"));
        s.preview_text = "8".to_string();
        let choices = s.picture_choices().unwrap();
        assert_eq!((choices.jpg_quality, choices.first_steps), (35, Some(8)));
        s.preview = false;
        assert_eq!(s.picture_choices().unwrap().first_steps, None, "unticked: every step, whatever the box says");
    }
}
