// SPDX-License-Identifier: Apache-2.0
//! The JSON export (spec section 7.4): a flat file for other programs, with the pictures in a folder beside it.
//!
//! It is not the guide's own file (`guide.json`), which is for Express. Field names are plain, descriptive ones
//! (`indexString`, `parentId`, `screenshotRelativePath`), so another program can read this with little work. That is a convenience,
//! not a promise of compatibility.
//!
//! What is left out on purpose: the text typed into a field (it appears only as the step's title), the click position,
//! the control and the program (they are for editing), and anything private.

use serde_json::{json, Value};

use openstage_express_core::fsutil::sanitise_folder_name;

use crate::plan::Plan;
use crate::{Export, OutFile};

/// The version of this export's layout. It changes when a field is renamed or removed.
pub const EXPORT_VERSION: u32 = 1;

/// The export as JSON.
pub fn json_value(plan: &Plan) -> Value {
    let mut steps = Vec::with_capacity(plan.steps.len());
    // The parent of a step is the nearest earlier step one level up.
    let mut stack: Vec<&str> = Vec::new();
    for (position, step) in plan.steps.iter().enumerate() {
        stack.truncate(step.depth);
        let parent_id = stack.last().copied();
        stack.push(&step.id);
        let picture = step.picture.as_ref();
        steps.push(json!({
            "id": step.id,
            "index": position + 1,
            "indexString": step.number,
            "parentId": parent_id,
            "kind": match step.kind {
                openstage_express_core::StepKind::Image => "image",
                openstage_express_core::StepKind::Empty => "empty",
                openstage_express_core::StepKind::Block => "block",
            },
            "title": step.title,
            "description": step.description,
            "altText": if picture.is_some() { Value::String(step.alt.clone()) } else { Value::Null },
            "screenshotFilename": picture.map(|p| p.file.rsplit('/').next().unwrap_or(&p.file).to_string()),
            "screenshotRelativePath": picture.map(|p| p.file.clone()),
            "width": picture.map(|p| p.width),
            "height": picture.map(|p| p.height),
        }));
    }
    json!({
        "exportFormat": "openstage-express-guide",
        "exportVersion": EXPORT_VERSION,
        "guide": {
            "title": plan.title,
            "description": plan.description,
            "language": plan.language,
        },
        "steps": steps,
    })
}

/// The JSON export: `<title>.json` and `images/step-NN.png`.
pub fn export_json(plan: &Plan) -> Export {
    let mut text = serde_json::to_string_pretty(&json_value(plan)).expect("plain data always serialises");
    text.push('\n');
    let mut files =
        vec![OutFile { path: format!("{}.json", sanitise_folder_name(&plan.title)), bytes: text.into_bytes() }];
    for step in &plan.steps {
        if let Some(picture) = &step.picture {
            files.push(OutFile { path: picture.file.clone(), bytes: picture.png.clone() });
        }
    }
    Export { files, warnings: plan.warnings.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo;
    use crate::plan::{plan, ExportOptions};
    use openstage_capture_render::BlockText;

    fn demo_json() -> Value {
        let d = demo();
        json_value(&plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default()))
    }

    #[test]
    fn the_guide_and_its_steps_are_described() {
        let v = demo_json();
        assert_eq!(v["exportFormat"], "openstage-express-guide");
        assert_eq!(v["exportVersion"], 1);
        assert_eq!(v["guide"]["title"], "How to submit an assignment");
        assert_eq!(v["guide"]["language"], "en");
        let steps = v["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 6);
        assert_eq!(
            steps[0],
            json!({
                "id": "step-1",
                "index": 1,
                "indexString": "1",
                "parentId": null,
                "kind": "image",
                "title": "Click \"Assignments\"",
                "description": "Open the **Assignments** page from the menu on the left.",
                "altText": "The course site with the Assignments item in the left menu circled",
                "screenshotFilename": "step-01.png",
                "screenshotRelativePath": "images/step-01.png",
                "width": 1280,
                "height": 720,
            })
        );
    }

    #[test]
    fn substeps_name_their_parent_and_blocks_have_no_number_or_picture() {
        let v = demo_json();
        let steps = v["steps"].as_array().unwrap();
        assert_eq!(steps[2]["indexString"], "2.1");
        assert_eq!(steps[2]["parentId"], "step-2");
        let tip = steps.iter().find(|s| s["kind"] == "block").unwrap();
        assert_eq!(tip["indexString"], Value::Null);
        assert_eq!(tip["screenshotRelativePath"], Value::Null);
        assert_eq!(tip["altText"], Value::Null);
        assert_eq!(tip["parentId"], Value::Null);
        let indexes: Vec<i64> = steps.iter().map(|s| s["index"].as_i64().unwrap()).collect();
        assert_eq!(indexes, [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn nothing_private_is_in_the_file() {
        let text = serde_json::to_string(&demo_json()).unwrap();
        // The text typed into the name field is only in the step's title, never in a field of its own.
        for private in ["typed", "click", "control", "program", "browser.exe", "capturedAt"] {
            assert!(!text.contains(private), "{private}");
        }
        assert_eq!(text.matches("Week 3 reflection").count(), 2, "the title, and the alt text");
    }

    #[test]
    fn the_file_is_the_same_every_time_and_comes_with_its_pictures() {
        let d = demo();
        let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let export = export_json(&p);
        assert_eq!(export, export_json(&p));
        assert_eq!(export.files[0].path, "How to submit an assignment.json");
        assert!(export.files[0].bytes.ends_with(b"}\n"));
        assert_eq!(export.files.len(), 1 + 5);
        let parsed: Value = serde_json::from_slice(&export.files[0].bytes).unwrap();
        assert_eq!(parsed, json_value(&p));
        for step in parsed["steps"].as_array().unwrap() {
            if let Some(path) = step["screenshotRelativePath"].as_str() {
                assert!(export.file(path).is_some(), "{path}");
            }
        }
    }
}
