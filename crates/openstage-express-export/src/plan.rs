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
use openstage_express_core::{AnnotationKind, Guide, StepKind};

use crate::picture::{encode_png, PictureSource};

/// How a guide is turned into pictures and words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOptions {
    /// Every picture is scaled to this many pixels across (the height follows).
    pub image_width: u32,
}

impl Default for ExportOptions {
    fn default() -> ExportOptions {
        ExportOptions { image_width: 1280 }
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
    pub title: String,
    pub description: String,
    pub language: String,
    pub steps: Vec<PlannedStep>,
    pub warnings: Vec<String>,
}

fn normalised(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Work out what an export contains. A step whose picture cannot be found is still exported, without a picture, and the
/// plan says so in its warnings.
pub fn plan(guide: &Guide, pictures: &dyn PictureSource, text: &dyn TextRenderer, options: &ExportOptions) -> Plan {
    // Which steps are in, and their numbers and depths.
    let mut skipped_ids: HashSet<&str> = HashSet::new();
    let mut counters: HashMap<Option<&str>, u32> = HashMap::new();
    let mut numbers: HashMap<&str, Option<String>> = HashMap::new();
    let mut depths: HashMap<&str, usize> = HashMap::new();
    let mut included: Vec<(&openstage_express_core::Step, usize, Option<String>)> = Vec::new();
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
                let scaled = scale_to_width(&rendered.canvas, options.image_width);
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
    Plan {
        title: guide.title.clone(),
        description: guide.description.clone(),
        language: guide.language.clone(),
        steps,
        warnings,
    }
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
}
