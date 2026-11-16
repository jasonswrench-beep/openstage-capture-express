// SPDX-License-Identifier: Apache-2.0
//! The OpenExplainer export: the guide as a book OpenExplainer can import (its "JSON with images" file, a ZIP holding `book.json` and a
//! folder of pictures, read back by OpenExplainer's *Import an export*). The guide becomes one chapter. Its description is a paragraph, its
//! steps are one numbered procedure with a picture on each, and a content block (a note that is not a step) is a heading and a
//! paragraph between two procedures.
//!
//! What goes over, and what does not, on purpose:
//!
//! * The pictures are the finished ones (the marks and the blur drawn in, cropped to the focused view), as in every other export, because
//!   that is all an export is made from. OpenExplainer therefore gets no click position and no control name, and draws no circle of its
//!   own on top of the one already there. The step's description is its result line.
//! * A picture has alt text only when the author wrote some. OpenExplainer shows a picture with none as "Needs a description", which is
//!   the right thing to say about a picture nobody has described; Express's other exports use the step's title instead, which is not a
//!   description, so it is not handed over as one here.
//! * Skipped steps are not in it, substeps are numbered steps one after the other (a procedure has no levels), and a step with no picture
//!   is a step with no picture.
//!
//! The layout is OpenExplainer's, written down in its `docs/format-spec.md` and `docs/exports.md`: `format` `openexplainer-export`,
//! `exportVersion` 1, `projectVersion` 21 (the project layout this was written for; OpenExplainer upgrades older layouts itself and
//! refuses newer ones in words, and it cleans every field on the way in, so a missing one is its usual).

use serde_json::{json, Value};

use openstage_express_core::fsutil::sanitise_folder_name;
use openstage_express_core::StepKind;

use crate::encoding::markdown_to_plain_lines;
use crate::plan::{Plan, PlannedStep};
use crate::zip::{self, ZipEntry, ZipError};
use crate::{Export, OutFile};

/// OpenExplainer's name for this kind of file.
pub const EXPORT_FORMAT: &str = "openexplainer-export";
/// The layout of `book.json` this is written to (OpenExplainer's `EXPORT_VERSION`).
pub const EXPORT_VERSION: u32 = 1;
/// The project layout the book is written in (OpenExplainer's `CURRENT_VERSION` when this was written).
pub const PROJECT_VERSION: u32 = 21;

/// A run of plain text, the way OpenExplainer keeps words.
fn runs(text: &str) -> Value {
    if text.trim().is_empty() {
        json!([])
    } else {
        json!([{ "text": text }])
    }
}

/// A description as one plain line: the paragraphs and list items it has, joined by spaces.
fn one_line(description: &str) -> String {
    markdown_to_plain_lines(description).join(" ")
}

/// The picture's alt text if the author wrote some (Express gives the title when there is none, and that is not a description).
fn author_alt(step: &PlannedStep) -> String {
    if step.alt.trim() == step.title.trim() {
        String::new()
    } else {
        step.alt.clone()
    }
}

/// The book as `book.json` and the pictures it names (`images/pic-0001.png`, in order).
pub fn book(plan: &Plan) -> (Value, Vec<(String, Vec<u8>)>) {
    let mut blocks: Vec<Value> = Vec::new();
    let mut pictures: Vec<Value> = Vec::new();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut procedure: Vec<Value> = Vec::new();
    let mut block_count = 0usize;
    let mut next_block = || {
        block_count += 1;
        format!("b{block_count}")
    };
    let flush = |procedure: &mut Vec<Value>, blocks: &mut Vec<Value>, id: String| {
        if !procedure.is_empty() {
            blocks.push(json!({ "id": id, "type": "procedure", "steps": std::mem::take(procedure) }));
        }
    };

    if !plan.description.trim().is_empty() {
        for line in markdown_to_plain_lines(&plan.description) {
            blocks.push(json!({ "id": next_block(), "type": "paragraph", "text": runs(&line) }));
        }
    }
    let mut step_count = 0usize;
    for step in &plan.steps {
        if step.kind == StepKind::Block {
            flush(&mut procedure, &mut blocks, next_block());
            if !step.title.trim().is_empty() {
                blocks.push(json!({ "id": next_block(), "type": "heading", "text": step.title.trim() }));
            }
            for line in markdown_to_plain_lines(&step.description) {
                blocks.push(json!({ "id": next_block(), "type": "paragraph", "text": runs(&line) }));
            }
            continue;
        }
        step_count += 1;
        let mut entry = json!({ "id": format!("s{step_count}"), "text": runs(&step.title) });
        let result = one_line(&step.description);
        if !result.is_empty() {
            entry["result"] = runs(&result);
        }
        if let Some(picture) = &step.picture {
            let id = format!("pic-{:04}", pictures.len() + 1);
            let file = format!("images/{id}.png");
            pictures.push(json!({
                "id": id,
                "file": file,
                "width": picture.width,
                "height": picture.height,
                "altText": author_alt(step),
                "source": { "kind": "import", "capturedAt": "" },
            }));
            files.push((file, picture.png.clone()));
            entry["pictureId"] = Value::String(id);
        }
        procedure.push(entry);
    }
    flush(&mut procedure, &mut blocks, next_block());

    let page = if blocks.is_empty() { Value::Null } else { json!({ "blocks": blocks }) };
    let mut chapter = json!({
        "id": "chapter-1",
        "number": "1",
        "title": plan.title,
        "status": "captured",
        "pictures": pictures,
        "children": [],
    });
    if !page.is_null() {
        chapter["page"] = page;
    }
    let book = json!({
        "format": EXPORT_FORMAT,
        "exportVersion": EXPORT_VERSION,
        "projectVersion": PROJECT_VERSION,
        "name": plan.title,
        "template": "empty",
        "language": plan.language,
        "variables": { "PRODUCT": "", "VERSION": "", "COMPANY": "" },
        "tree": [chapter],
    });
    (book, files)
}

/// The export: one ZIP file, named after the guide, with `book.json` first and then the pictures.
pub fn export_openexplainer(plan: &Plan) -> Result<Export, ZipError> {
    let (book, pictures) = book(plan);
    let mut text = serde_json::to_string_pretty(&book).unwrap_or_default();
    text.push('\n');
    // Stored, not squeezed: OpenExplainer writes its own exports that way, and PNG files do not shrink anyway.
    let mut entries = vec![ZipEntry::stored("book.json", text.into_bytes())];
    entries.extend(pictures.into_iter().map(|(name, bytes)| ZipEntry::stored(name, bytes)));
    let bytes = zip::build(&entries)?;
    let name = format!("{} (for OpenExplainer).zip", sanitise_folder_name(&plan.title));
    Ok(Export { files: vec![OutFile { path: name, bytes }], warnings: plan.warnings.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo;
    use crate::plan::{plan, ExportOptions};
    use openstage_capture_render::BlockText;

    fn demo_plan() -> Plan {
        let d = demo();
        plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default())
    }

    fn read_back(export: &Export) -> (Value, Vec<(String, Vec<u8>)>) {
        let parts = zip::read(&export.files[0].bytes, &Default::default()).unwrap();
        let book =
            parts.iter().find(|(n, _)| n == "book.json").map(|(_, b)| serde_json::from_slice(b).unwrap()).unwrap();
        (book, parts.into_iter().filter(|(n, _)| n.starts_with("images/")).collect())
    }

    #[test]
    fn the_file_is_a_zip_named_after_the_guide_with_book_json_and_the_pictures() {
        let p = demo_plan();
        let export = export_openexplainer(&p).unwrap();
        assert_eq!(export.files.len(), 1);
        assert_eq!(export.files[0].path, "How to submit an assignment (for OpenExplainer).zip");
        let (book, images) = read_back(&export);
        assert_eq!(book["format"], EXPORT_FORMAT);
        assert_eq!((book["exportVersion"].as_u64(), book["projectVersion"].as_u64()), (Some(1), Some(21)));
        assert_eq!(book["name"], "How to submit an assignment");
        let with_pictures = p.steps.iter().filter(|s| s.picture.is_some()).count();
        assert_eq!(images.len(), with_pictures);
        for (i, (name, bytes)) in images.iter().enumerate() {
            assert_eq!(name, &format!("images/pic-{:04}.png", i + 1));
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        }
    }

    #[test]
    fn the_guide_is_one_chapter_whose_page_is_its_description_and_one_numbered_procedure() {
        let p = demo_plan();
        let (book, _) = book(&p);
        let chapters = book["tree"].as_array().unwrap();
        assert_eq!(chapters.len(), 1);
        let chapter = &chapters[0];
        assert_eq!(
            (chapter["id"].as_str(), chapter["title"].as_str()),
            (Some("chapter-1"), Some("How to submit an assignment"))
        );
        let blocks = chapter["page"]["blocks"].as_array().unwrap();
        let kinds: Vec<&str> = blocks.iter().map(|b| b["type"].as_str().unwrap()).collect();
        assert!(kinds.contains(&"procedure"), "{kinds:?}");
        // Every step with a picture names one of the chapter's pictures, and every picture is used once.
        let pictures: Vec<&str> =
            chapter["pictures"].as_array().unwrap().iter().map(|x| x["id"].as_str().unwrap()).collect();
        let used: Vec<&str> = blocks
            .iter()
            .filter(|b| b["type"] == "procedure")
            .flat_map(|b| b["steps"].as_array().unwrap().iter())
            .filter_map(|s| s["pictureId"].as_str())
            .collect();
        assert_eq!(used, pictures);
        // The first step is the first step of the guide, in plain words.
        let first = blocks.iter().find(|b| b["type"] == "procedure").unwrap()["steps"][0].clone();
        assert_eq!(first["text"][0]["text"], "Click \"Assignments\"");
        assert_eq!(first["id"], "s1");
    }

    #[test]
    fn skipped_steps_are_left_out_substeps_go_on_in_order_and_a_content_block_splits_the_procedure() {
        let p = demo_plan();
        let (book, _) = book(&p);
        let blocks = book["tree"][0]["page"]["blocks"].as_array().unwrap().clone();
        let texts: Vec<String> = blocks
            .iter()
            .filter(|b| b["type"] == "procedure")
            .flat_map(|b| b["steps"].as_array().unwrap().iter())
            .map(|s| s["text"][0]["text"].as_str().unwrap().to_string())
            .collect();
        assert!(!texts.iter().any(|t| t.contains("Debug")), "the skipped step is not in it");
        let numbered = p.steps.iter().filter(|s| s.kind != StepKind::Block).count();
        assert_eq!(texts.len(), numbered, "a step each, substeps too");
        // The demo's tip is a content block: its heading sits between two procedures.
        let tip_at = blocks.iter().position(|b| b["type"] == "heading" && b["text"] == "Tip").unwrap();
        assert!(blocks[..tip_at].iter().any(|b| b["type"] == "procedure"));
        assert!(blocks[tip_at..].iter().any(|b| b["type"] == "procedure"));
    }

    #[test]
    fn a_description_is_the_result_line_and_alt_text_is_only_what_the_author_wrote() {
        let p = demo_plan();
        let (book, _) = book(&p);
        let chapter = &book["tree"][0];
        let procedure =
            chapter["page"]["blocks"].as_array().unwrap().iter().find(|b| b["type"] == "procedure").unwrap();
        let second = &procedure["steps"][1];
        assert_eq!(second["result"][0]["text"], "Start here: this opens the form.");
        let alts: Vec<&str> =
            chapter["pictures"].as_array().unwrap().iter().map(|x| x["altText"].as_str().unwrap()).collect();
        assert!(alts[0].starts_with("The course site"), "{alts:?}");
        // A step with no description of its picture has an empty alt text, not its title.
        let mut bare = demo_plan();
        for step in &mut bare.steps {
            step.alt = step.title.clone();
        }
        let (book, _) = super::book(&bare);
        assert!(book["tree"][0]["pictures"].as_array().unwrap().iter().all(|x| x["altText"] == ""));
    }

    #[test]
    fn ids_are_safe_pictures_have_sizes_and_nothing_about_the_click_goes_over() {
        let p = demo_plan();
        let (book, _) = book(&p);
        let safe = |id: &str| {
            !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        };
        for picture in book["tree"][0]["pictures"].as_array().unwrap() {
            assert!(safe(picture["id"].as_str().unwrap()));
            assert!(
                picture["file"].as_str().unwrap().starts_with("images/")
                    && picture["file"].as_str().unwrap().ends_with(".png")
            );
            assert!(picture["width"].as_u64().unwrap() > 0 && picture["height"].as_u64().unwrap() > 0);
            assert_eq!(picture["source"]["capturedAt"], "");
            assert!(picture.get("click").is_none(), "no click position: the marks are in the picture");
        }
        let text = serde_json::to_string(&book).unwrap();
        for word in ["control", "program", "browser.exe", "typed"] {
            assert!(!text.contains(word), "{word} is not for another program");
        }
    }

    #[test]
    fn a_guide_with_no_steps_is_still_a_book_with_a_chapter() {
        let mut p = demo_plan();
        p.steps.clear();
        p.description.clear();
        let (book, files) = book(&p);
        assert!(files.is_empty());
        assert!(book["tree"][0].get("page").is_none(), "no page when there is nothing to say");
        assert!(export_openexplainer(&p).is_ok());
    }

    #[test]
    fn the_same_guide_gives_the_same_bytes() {
        let p = demo_plan();
        assert_eq!(export_openexplainer(&p).unwrap().files, export_openexplainer(&p).unwrap().files);
    }
}
