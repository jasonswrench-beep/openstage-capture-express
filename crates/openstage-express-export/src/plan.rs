// SPDX-License-Identifier: Apache-2.0
//! What every export agrees on (spec section 10): which steps are in it, how they are numbered, what each picture looks like
//! once the marks are burned in and it is cropped and scaled, and what words go with it.
//!
//! Skipped steps are left out, and so are their substeps (skipping a step skips everything under it). The steps that stay
//! are numbered `1`, `2`, `2.1`, `2.2`, `3` and so on; a content block is not numbered. Pictures are all scaled to the same
//! width, so the steps of a guide line up. The marks are burned into the pixels: a blurred area can never be undone, and
//! the original stays in the guide folder.

use std::collections::{HashMap, HashSet};

use openstage_capture_render::TextRenderer;
use openstage_express_core::render::{render_step, scale_to_width};
use openstage_express_core::{AnnotationKind, Guide, Step, StepKind};

use crate::picture::{encode_png, PictureSource};

/// The picture widths the Export screen offers, in pixels. The usual one is `DEFAULT_IMAGE_WIDTH`.
pub const IMAGE_WIDTH_CHOICES: [u32; 6] = [640, 800, 1024, 1280, 1600, 1920];
pub const DEFAULT_IMAGE_WIDTH: u32 = 1280;
/// Whatever width is asked for is kept between these, so a wild number cannot ask for a picture too small to read or too big to make.
pub const MIN_IMAGE_WIDTH: u32 = 64;
pub const MAX_IMAGE_WIDTH: u32 = 4096;
/// How many steps a preview has when nothing else is chosen (spec 10.1).
pub const DEFAULT_PREVIEW_STEPS: usize = 5;

/// How a guide is turned into pictures and words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOptions {
    /// Every picture is scaled to this many pixels across (the height follows).
    pub image_width: u32,
    /// Make only the first this many numbered steps, to see the look before waiting for all of it (a preview); `None` is every step.
    /// Substeps count as steps, and a note between two steps stays with the step before it. 0 is taken as 1.
    pub first_steps: Option<usize>,
}

impl Default for ExportOptions {
    fn default() -> ExportOptions {
        ExportOptions { image_width: DEFAULT_IMAGE_WIDTH, first_steps: None }
    }
}

/// What a preview holds. It is there whenever a preview was asked for, even when the guide is short enough to be in it whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preview {
    /// How many numbered steps are in the export.
    pub shown: usize,
    /// How many numbered steps the whole guide has (skipped steps are not counted).
    pub total: usize,
}

impl Preview {
    /// Whether steps were left out.
    pub fn is_cut(&self) -> bool {
        self.shown < self.total
    }

    /// For the front of the guide's title: `Preview, first 5 of 12 steps`.
    pub fn words(&self) -> String {
        if self.shown == 1 {
            format!("Preview, first step of {}", self.total)
        } else {
            format!("Preview, first {} of {} steps", self.shown, self.total)
        }
    }
}

/// A step's picture with its marks, cropped to its focused view and scaled, as a PNG file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPicture {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// The name it gets in exports that have a folder of pictures, such as `images/step-03.png`.
    pub file: String,
}

/// One step of an export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedStep {
    pub id: String,
    /// 0 for a top-level step, 1 for a substep, and so on.
    pub depth: usize,
    /// `1`, `2.1` and so on. A content block has none.
    pub number: Option<String>,
    pub kind: StepKind,
    pub title: String,
    pub description: String,
    /// The words that describe the picture: the author's alt text, or the title if there is none.
    pub alt: String,
    pub picture: Option<RenderedPicture>,
}

/// Everything an export is made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The guide's title; for a preview that leaves steps out it starts with `Preview, first 5 of 12 steps - `, so a file made from
    /// it can never be taken for the whole guide. It is at the front because file names keep only the first 80 characters of a title.
    pub title: String,
    pub description: String,
    pub language: String,
    pub steps: Vec<PlannedStep>,
    pub warnings: Vec<String>,
    /// Set when a preview was asked for.
    pub preview: Option<Preview>,
}

fn normalised(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// A step that is in an export, with its depth and its number (a content block has none).
type Included<'a> = Vec<(&'a Step, usize, Option<String>)>;

/// Which steps are in an export, in order, with their numbers and depths. Skipped steps and everything under them are left out.
fn included_steps(guide: &Guide) -> Included<'_> {
    let mut skipped_ids: HashSet<&str> = HashSet::new();
    let mut counters: HashMap<Option<&str>, u32> = HashMap::new();
    let mut numbers: HashMap<&str, Option<String>> = HashMap::new();
    let mut depths: HashMap<&str, usize> = HashMap::new();
    let mut included: Included<'_> = Vec::new();
    for step in &guide.steps {
        let parent = step.parent.as_deref();
        if step.skipped || parent.is_some_and(|p| skipped_ids.contains(p) || !depths.contains_key(p)) {
            skipped_ids.insert(&step.id);
            continue;
        }
        let depth = parent.map(|p| depths[p] + 1).unwrap_or(0);
        let number = if step.kind == StepKind::Block {
            None
        } else {
            let counter = counters.entry(parent).or_insert(0);
            *counter += 1;
            Some(match parent.and_then(|p| numbers.get(p).cloned().flatten()) {
                Some(prefix) => format!("{prefix}.{counter}"),
                None => counter.to_string(),
            })
        };
        depths.insert(&step.id, depth);
        numbers.insert(&step.id, number.clone());
        included.push((step, depth, number));
    }
    included
}

/// How many numbered steps an export of `guide` has (skipped steps and notes are not counted). This is what a preview counts from.
pub fn exported_step_count(guide: &Guide) -> usize {
    included_steps(guide).iter().filter(|(_, _, number)| number.is_some()).count()
}

/// Work out what an export contains. A step whose picture cannot be found is still exported, without a picture, and the
/// plan says so in its warnings.
pub fn plan(guide: &Guide, pictures: &dyn PictureSource, text: &dyn TextRenderer, options: &ExportOptions) -> Plan {
    let mut included = included_steps(guide);
    let total = included.iter().filter(|(_, _, number)| number.is_some()).count();
    let preview = options.first_steps.map(|asked| {
        let keep = asked.max(1);
        let mut seen = 0;
        let cut = included.iter().position(|(_, _, number)| {
            seen += usize::from(number.is_some());
            seen > keep
        });
        if let Some(cut) = cut {
            included.truncate(cut);
        }
        Preview { shown: keep.min(total), total }
    });
    let image_width = options.image_width.clamp(MIN_IMAGE_WIDTH, MAX_IMAGE_WIDTH);

    let digits = included.len().to_string().len().max(2);
    let mut warnings = Vec::new();
    let mut steps = Vec::with_capacity(included.len());
    for (position, (step, depth, number)) in included.into_iter().enumerate() {
        let label = number.clone().unwrap_or_else(|| step.title.clone());
        let picture = step.image.as_ref().and_then(|image| match pictures.picture(&image.file) {
            None => {
                warnings.push(format!(
                    "Step {label}: its picture ({}) could not be found or read, so it has no picture.",
                    image.file
                ));
                None
            }
            Some(original) => {
                let rendered = render_step(&original, step, text);
                for id in &rendered.report.text_cut_off {
                    warnings.push(format!(
                        "Step {label}: the words of mark {id} do not all fit in their box. Make the box bigger."
                    ));
                }
                for problem in &rendered.report.text_errors {
                    warnings.push(format!("Step {label}: a text mark could not be drawn ({problem})."));
                }
                let scaled = scale_to_width(&rendered.canvas, image_width);
                Some(RenderedPicture {
                    png: encode_png(&scaled),
                    width: scaled.width,
                    height: scaled.height,
                    file: format!("images/step-{:0digits$}.png", position + 1),
                })
            }
        });
        // Words drawn into a picture cannot be read by a screen reader, so they must also be in the step's text.
        let spoken = normalised(&format!("{} {} {}", step.title, step.description, step.image_description));
        for mark in step.annotations.iter().filter(|m| m.kind == AnnotationKind::Text) {
            let words = normalised(mark.text.as_deref().unwrap_or(""));
            if !words.is_empty() && !spoken.contains(&words) {
                warnings.push(format!(
                    "Step {label}: the words \"{}\" are drawn on the picture but are not in the step's text. Add them to the description so a screen reader can read them.",
                    mark.text.as_deref().unwrap_or("").trim()
                ));
            }
        }
        let alt =
            if step.image_description.trim().is_empty() { step.title.clone() } else { step.image_description.clone() };
        steps.push(PlannedStep {
            id: step.id.clone(),
            depth,
            number,
            kind: step.kind,
            title: step.title.clone(),
            description: step.description.clone(),
            alt,
            picture,
        });
    }
    let title = match preview {
        Some(p) if p.is_cut() => format!("{} - {}", p.words(), guide.title),
        _ => guide.title.clone(),
    };
    Plan { title, description: guide.description.clone(), language: guide.language.clone(), steps, warnings, preview }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo;
    use crate::picture::{decode_png, MemoryPictures};
    use openstage_capture_render::BlockText;
    use openstage_express_core::{Annotation, Guide, Step, StepDraft, StepImage};

    fn plan_of(guide: &Guide, pictures: &MemoryPictures) -> Plan {
        plan(guide, pictures, &BlockText, &ExportOptions::default())
    }

    #[test]
    fn the_demo_plan_has_the_right_steps_numbers_and_depths() {
        let d = demo();
        let p = plan_of(&d.document.guide, &d.pictures);
        let summary: Vec<(String, usize, Option<String>)> =
            p.steps.iter().map(|s| (s.title.clone(), s.depth, s.number.clone())).collect();
        assert_eq!(summary[0], ("Click \"Assignments\"".to_string(), 0, Some("1".to_string())));
        assert_eq!(summary[1].2.as_deref(), Some("2"));
        assert_eq!(summary[2].1, 1, "a substep");
        assert_eq!(summary[2].2.as_deref(), Some("2.1"));
        assert!(!p.steps.iter().any(|s| s.title.contains("Debug")), "the skipped step is left out");
        let tip = p.steps.iter().find(|s| s.kind == StepKind::Block).unwrap();
        assert_eq!(tip.number, None, "a content block has no number");
        assert!(p.steps.iter().filter(|s| s.number.is_some()).count() >= 5);
    }

    #[test]
    fn every_picture_is_the_same_width_and_a_real_png() {
        let d = demo();
        let p = plan_of(&d.document.guide, &d.pictures);
        let with_pictures: Vec<&PlannedStep> = p.steps.iter().filter(|s| s.picture.is_some()).collect();
        assert!(with_pictures.len() >= 5);
        for step in with_pictures {
            let picture = step.picture.as_ref().unwrap();
            assert_eq!(picture.width, 1280, "{}", step.title);
            let decoded = decode_png(&picture.png).unwrap();
            assert_eq!((decoded.width, decoded.height), (picture.width, picture.height));
        }
    }

    #[test]
    fn picture_file_names_follow_the_export_order() {
        let d = demo();
        let p = plan_of(&d.document.guide, &d.pictures);
        let files: Vec<String> = p.steps.iter().filter_map(|s| s.picture.as_ref().map(|x| x.file.clone())).collect();
        assert!(files[0].starts_with("images/step-0") && files[0].ends_with(".png"));
        let mut sorted = files.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), files.len(), "no two steps share a file name");
        assert_eq!(files[0], format!("images/step-{:02}.png", 1));
    }

    #[test]
    fn skipping_a_step_skips_its_substeps_and_renumbers_the_rest() {
        let mut doc = openstage_express_core::Document::new(Guide::new("g", "T", "en", "2026-11-01T00:00:00Z"));
        doc.add_step(StepDraft::empty("A"), openstage_express_core::Position::End, "t").unwrap();
        let b = doc.add_step(StepDraft::empty("B"), openstage_express_core::Position::End, "t").unwrap();
        doc.add_step(StepDraft::empty("B1"), openstage_express_core::Position::ChildOf(&b), "t").unwrap();
        doc.add_step(StepDraft::empty("C"), openstage_express_core::Position::End, "t").unwrap();
        doc.set_skipped(&b, true, "t").unwrap();
        let p = plan_of(&doc.guide, &MemoryPictures::default());
        let numbered: Vec<(&str, Option<&str>)> =
            p.steps.iter().map(|s| (s.title.as_str(), s.number.as_deref())).collect();
        assert_eq!(numbered, [("A", Some("1")), ("C", Some("2"))]);
    }

    #[test]
    fn a_missing_picture_is_a_warning_and_the_step_stays() {
        let d = demo();
        let p = plan_of(&d.document.guide, &MemoryPictures::default());
        assert!(p.steps.iter().all(|s| s.picture.is_none()));
        assert!(p.warnings.iter().any(|w| w.contains("could not be found")), "{:?}", p.warnings);
        assert!(p.steps.len() >= 6);
    }

    #[test]
    fn alt_text_is_the_authors_words_or_the_title() {
        let mut doc = openstage_express_core::Document::new(Guide::new("g", "T", "en", "t"));
        let image = StepImage::new("images/step-0001.png", 64, 48);
        let a = doc
            .add_step(StepDraft::with_picture("Click Save", image.clone()), openstage_express_core::Position::End, "t")
            .unwrap();
        let b = doc
            .add_step(StepDraft::with_picture("Click Close", image), openstage_express_core::Position::End, "t")
            .unwrap();
        doc.set_image_description(&a, "The Save button at the top right", "t").unwrap();
        doc.set_image_description(&b, "   ", "t").unwrap();
        let mut pictures = MemoryPictures::default();
        pictures.0.insert(
            "images/step-0001.png".into(),
            openstage_capture_render::Picture::solid(64, 48, openstage_capture_render::Rgb(9, 9, 9)),
        );
        let p = plan_of(&doc.guide, &pictures);
        assert_eq!(p.steps[0].alt, "The Save button at the top right");
        assert_eq!(p.steps[1].alt, "Click Close");
    }

    #[test]
    fn words_drawn_on_a_picture_must_also_be_in_the_text() {
        let mut doc = openstage_express_core::Document::new(Guide::new("g", "T", "en", "t"));
        let image = StepImage::new("images/step-0001.png", 200, 100);
        let a = doc
            .add_step(StepDraft::with_picture("Open the menu", image), openstage_express_core::Position::End, "t")
            .unwrap();
        let mut said = Annotation::at("", AnnotationKind::Text, 5, 5);
        said.text = Some("open  THE menu".into());
        doc.add_annotation(&a, said, "t").unwrap();
        let mut unsaid = Annotation::at("", AnnotationKind::Text, 5, 40);
        unsaid.text = Some("Careful: this deletes data".into());
        doc.add_annotation(&a, unsaid, "t").unwrap();
        let mut pictures = MemoryPictures::default();
        pictures.0.insert(
            "images/step-0001.png".into(),
            openstage_capture_render::Picture::solid(200, 100, openstage_capture_render::Rgb(255, 255, 255)),
        );
        let p = plan_of(&doc.guide, &pictures);
        assert_eq!(p.warnings.len(), 1, "{:?}", p.warnings);
        assert!(p.warnings[0].contains("Careful: this deletes data") && p.warnings[0].contains("screen reader"));
        doc.set_description(&a, "Careful: this deletes data.", "t").unwrap();
        assert!(plan_of(&doc.guide, &pictures).warnings.is_empty());
    }

    #[test]
    fn text_that_does_not_fit_is_a_warning() {
        let mut doc = openstage_express_core::Document::new(Guide::new("g", "T", "en", "t"));
        let a = doc
            .add_step(
                StepDraft::with_picture("x", StepImage::new("images/step-0001.png", 200, 100)),
                openstage_express_core::Position::End,
                "t",
            )
            .unwrap();
        let mut mark = Annotation::at("a1", AnnotationKind::Text, 5, 5);
        mark.text = Some("x".repeat(60));
        mark.width = Some(40);
        mark.font_size = Some(16);
        doc.add_annotation(&a, mark, "t").unwrap();
        doc.set_description(&a, &"x".repeat(60), "t").unwrap();
        let mut pictures = MemoryPictures::default();
        pictures.0.insert(
            "images/step-0001.png".into(),
            openstage_capture_render::Picture::solid(200, 100, openstage_capture_render::Rgb(255, 255, 255)),
        );
        let p = plan_of(&doc.guide, &pictures);
        assert!(p.warnings.iter().any(|w| w.contains("do not all fit")), "{:?}", p.warnings);
    }

    #[test]
    fn a_guide_with_a_damaged_parent_still_exports_every_reachable_step() {
        let mut g = Guide::new("g", "T", "en", "t");
        let mut a = Step::bare("step-1", StepKind::Empty, "A");
        a.parent = None;
        let mut b = Step::bare("step-2", StepKind::Empty, "B");
        b.parent = Some("step-9".into());
        g.steps = vec![a, b];
        g.next_step_number = 3;
        let p = plan_of(&g, &MemoryPictures::default());
        assert_eq!(p.steps.len(), 1, "a step whose parent is missing is not exported");
    }

    #[test]
    fn the_same_guide_always_plans_to_the_same_bytes() {
        let d = demo();
        assert_eq!(plan_of(&d.document.guide, &d.pictures), plan_of(&d.document.guide, &d.pictures));
    }

    use openstage_capture_render::{Picture, Rgb};
    use openstage_express_core::{Document, Position};

    fn add(
        doc: &mut Document,
        pictures: &mut MemoryPictures,
        title: &str,
        position: Position<'_>,
        block: bool,
    ) -> String {
        let draft = if block {
            let mut draft = StepDraft::empty(title);
            draft.kind = StepKind::Block;
            draft
        } else {
            let file = format!("images/step-{:04}.png", pictures.0.len() + 1);
            pictures.0.insert(file.clone(), Picture::solid(64, 48, Rgb(40, 80, 120)));
            StepDraft::with_picture(title, StepImage::new(&file, 64, 48))
        };
        doc.add_step(draft, position, "t").unwrap()
    }

    /// A, B with two substeps, a tip, then C, D, E and F: eight numbered steps (substeps count) and one note, in this order:
    /// 1 A, 2 B, 2.1 B1, 2.2 B2, the tip, 3 C, 4 D, 5 E, 6 F.
    fn eight_steps() -> (Document, MemoryPictures) {
        let mut doc = Document::new(Guide::new("g", "Guide", "en", "t"));
        let mut pictures = MemoryPictures::default();
        add(&mut doc, &mut pictures, "A", Position::End, false);
        let b = add(&mut doc, &mut pictures, "B", Position::End, false);
        add(&mut doc, &mut pictures, "B1", Position::ChildOf(&b), false);
        add(&mut doc, &mut pictures, "B2", Position::ChildOf(&b), false);
        add(&mut doc, &mut pictures, "Tip", Position::End, true);
        for title in ["C", "D", "E", "F"] {
            add(&mut doc, &mut pictures, title, Position::End, false);
        }
        (doc, pictures)
    }

    fn preview_of(doc: &Document, pictures: &MemoryPictures, first: Option<usize>) -> Plan {
        plan(&doc.guide, pictures, &BlockText, &ExportOptions { first_steps: first, ..ExportOptions::default() })
    }

    fn titles(p: &Plan) -> Vec<&str> {
        p.steps.iter().map(|s| s.title.as_str()).collect()
    }

    #[test]
    fn a_preview_keeps_the_first_numbered_steps_and_substeps_count() {
        let (doc, pictures) = eight_steps();
        let whole = preview_of(&doc, &pictures, None);
        assert_eq!(titles(&whole), ["A", "B", "B1", "B2", "Tip", "C", "D", "E", "F"]);
        assert_eq!(whole.preview, None, "nothing was asked for");
        assert_eq!(whole.title, "Guide");
        let three = preview_of(&doc, &pictures, Some(3));
        assert_eq!(titles(&three), ["A", "B", "B1"]);
        assert_eq!(three.steps[2].number.as_deref(), Some("2.1"), "a substep is a step, and keeps its number");
        assert_eq!(three.preview, Some(Preview { shown: 3, total: 8 }));
        assert_eq!(three.title, "Preview, first 3 of 8 steps - Guide");
        assert_eq!(three.description, whole.description, "only the title says it is a preview");
        let one = preview_of(&doc, &pictures, Some(1));
        assert_eq!(titles(&one), ["A"]);
        assert_eq!(one.title, "Preview, first step of 8 - Guide");
    }

    #[test]
    fn a_note_after_the_last_step_of_a_preview_stays_and_a_note_after_the_cut_goes() {
        let (doc, pictures) = eight_steps();
        let four = preview_of(&doc, &pictures, Some(4));
        assert_eq!(titles(&four), ["A", "B", "B1", "B2", "Tip"], "the tip follows B2 and is not under C");
        let five = preview_of(&doc, &pictures, Some(5));
        assert_eq!(titles(&five), ["A", "B", "B1", "B2", "Tip", "C"]);
        assert_eq!(five.preview.map(|p| p.shown), Some(5));
    }

    #[test]
    fn a_preview_as_long_as_the_guide_is_the_whole_guide_and_does_not_rename_it() {
        let (doc, pictures) = eight_steps();
        let whole = preview_of(&doc, &pictures, None);
        for asked in [8, 9, 5000] {
            let p = preview_of(&doc, &pictures, Some(asked));
            assert_eq!(p.steps, whole.steps, "{asked}");
            assert_eq!(p.title, "Guide", "{asked}");
            let preview = p.preview.unwrap();
            assert_eq!((preview.shown, preview.total, preview.is_cut()), (8, 8, false), "{asked}");
        }
        assert!(preview_of(&doc, &pictures, Some(7)).preview.unwrap().is_cut());
    }

    #[test]
    fn asking_for_no_steps_gives_one_step() {
        let (doc, pictures) = eight_steps();
        let p = preview_of(&doc, &pictures, Some(0));
        assert_eq!(titles(&p), ["A"]);
        assert_eq!(p.preview, Some(Preview { shown: 1, total: 8 }));
    }

    #[test]
    fn skipped_steps_are_not_counted_in_a_preview() {
        let (mut doc, pictures) = eight_steps();
        let b = doc.guide.steps.iter().find(|s| s.title == "B").unwrap().id.clone();
        doc.set_skipped(&b, true, "t").unwrap();
        assert_eq!(exported_step_count(&doc.guide), 5, "A, C, D, E and F");
        let two = preview_of(&doc, &pictures, Some(2));
        assert_eq!(titles(&two), ["A", "Tip", "C"]);
        assert_eq!(two.preview, Some(Preview { shown: 2, total: 5 }));
        assert_eq!(two.title, "Preview, first 2 of 5 steps - Guide");
    }

    #[test]
    fn the_count_of_steps_in_an_export_leaves_out_notes_and_counts_substeps() {
        let (doc, _) = eight_steps();
        assert_eq!(exported_step_count(&doc.guide), 8);
        assert_eq!(exported_step_count(&Guide::new("g", "T", "en", "t")), 0);
    }

    #[test]
    fn the_pictures_of_steps_left_out_of_a_preview_are_never_read() {
        struct Asked<'a>(&'a MemoryPictures, std::cell::RefCell<Vec<String>>);
        impl PictureSource for Asked<'_> {
            fn picture(&self, logical: &str) -> Option<Picture> {
                self.1.borrow_mut().push(logical.to_string());
                self.0.picture(logical)
            }
        }
        let (doc, pictures) = eight_steps();
        let asked = Asked(&pictures, Default::default());
        let options = ExportOptions { first_steps: Some(2), ..ExportOptions::default() };
        let p = plan(&doc.guide, &asked, &BlockText, &options);
        assert_eq!(p.steps.iter().filter(|s| s.picture.is_some()).count(), 2);
        assert_eq!(
            asked.1.borrow().len(),
            2,
            "only the two steps in the preview had their pictures read: {:?}",
            asked.1
        );
    }

    #[test]
    fn the_files_of_a_preview_are_named_from_one() {
        let (doc, pictures) = eight_steps();
        let p = preview_of(&doc, &pictures, Some(3));
        let files: Vec<&str> = p.steps.iter().filter_map(|s| s.picture.as_ref().map(|x| x.file.as_str())).collect();
        assert_eq!(files, ["images/step-01.png", "images/step-02.png", "images/step-03.png"]);
    }

    #[test]
    fn the_words_for_a_preview_are_singular_or_plural() {
        assert_eq!(Preview { shown: 1, total: 4 }.words(), "Preview, first step of 4");
        assert_eq!(Preview { shown: 5, total: 12 }.words(), "Preview, first 5 of 12 steps");
    }

    #[test]
    fn the_mark_of_a_preview_is_still_in_the_file_name_when_the_title_is_long() {
        let (mut doc, pictures) = eight_steps();
        doc.guide.title =
            "A very long title for a guide about handing in work on the course site, step by step, for new students"
                .to_string();
        let p = preview_of(&doc, &pictures, Some(3));
        let name = openstage_express_core::fsutil::sanitise_folder_name(&p.title);
        assert!(name.starts_with("Preview, first 3 of 8 steps - A very long title"), "{name}");
        assert!(name.chars().count() <= 80, "file names keep 80 characters of a title, so the mark has to be first");
    }

    #[test]
    fn every_width_on_offer_is_the_width_of_the_pictures_and_the_usual_one_is_on_offer() {
        assert_eq!(IMAGE_WIDTH_CHOICES, [640, 800, 1024, 1280, 1600, 1920]);
        assert_eq!((MIN_IMAGE_WIDTH, MAX_IMAGE_WIDTH, DEFAULT_PREVIEW_STEPS), (64, 4096, 5));
        assert!(IMAGE_WIDTH_CHOICES.contains(&DEFAULT_IMAGE_WIDTH));
        assert_eq!(ExportOptions::default().image_width, DEFAULT_IMAGE_WIDTH);
        assert!(IMAGE_WIDTH_CHOICES.windows(2).all(|w| w[0] < w[1]), "smallest first");
        let (doc, pictures) = eight_steps();
        for width in IMAGE_WIDTH_CHOICES {
            let options = ExportOptions { image_width: width, first_steps: Some(1) };
            let p = plan(&doc.guide, &pictures, &BlockText, &options);
            let picture = p.steps[0].picture.as_ref().unwrap();
            assert_eq!(picture.width, width);
            assert_eq!(picture.height, width * 48 / 64, "the height follows");
            let decoded = decode_png(&picture.png).unwrap();
            assert_eq!(decoded.width, width);
        }
    }

    #[test]
    fn a_wild_width_is_kept_between_the_smallest_and_the_largest() {
        let (doc, pictures) = eight_steps();
        let width_for = |asked: u32| {
            let options = ExportOptions { image_width: asked, first_steps: Some(1) };
            plan(&doc.guide, &pictures, &BlockText, &options).steps[0].picture.as_ref().unwrap().width
        };
        assert_eq!(width_for(0), MIN_IMAGE_WIDTH);
        assert_eq!(width_for(1), MIN_IMAGE_WIDTH);
        assert_eq!(width_for(u32::MAX), MAX_IMAGE_WIDTH);
        assert_eq!(width_for(MAX_IMAGE_WIDTH + 1), MAX_IMAGE_WIDTH);
        assert_eq!(width_for(MAX_IMAGE_WIDTH), MAX_IMAGE_WIDTH);
        assert_eq!(width_for(MIN_IMAGE_WIDTH), MIN_IMAGE_WIDTH);
    }
}
