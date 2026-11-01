// SPDX-License-Identifier: Apache-2.0
//! The guide model: what `guide.json` holds (spec section 7.2, written down in `docs/guide-format.md`).
//!
//! Every struct keeps the fields it does not know in `extra`, so a guide written by a slightly newer Express
//! (same major version) survives being opened and saved by this one.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The version of the guide format this code reads and writes.
pub const SCHEMA_VERSION: &str = "0.1.0";

/// Fields this program does not know, kept and written back unchanged.
pub type Extra = Map<String, Value>;

fn yes() -> bool {
    true
}

fn english() -> String {
    "en".to_string()
}

/// A whole guide: its title and its steps in reading order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Guide {
    pub schema_version: String,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "english")]
    pub language: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default = "yes")]
    pub focused_view: bool,
    /// The number the next new step gets. It only ever goes up, so a step id is never used twice.
    pub next_step_number: u32,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Guide {
    /// A new, empty guide. `now` is an ISO 8601 time such as `2026-10-31T14:32:05Z`.
    pub fn new(id: &str, title: &str, language: &str, now: &str) -> Guide {
        Guide {
            schema_version: SCHEMA_VERSION.to_string(),
            id: id.to_string(),
            title: title.to_string(),
            description: String::new(),
            language: language.to_string(),
            created_at: now.to_string(),
            updated_at: now.to_string(),
            focused_view: true,
            next_step_number: 1,
            steps: Vec::new(),
            extra: Extra::new(),
        }
    }

    /// The step with this id, if there is one.
    pub fn step(&self, id: &str) -> Option<&Step> {
        self.steps.iter().find(|s| s.id == id)
    }

    /// Every way this guide breaks the rules of the format. An empty list means it is sound. Opening a guide with
    /// problems still works: the list is for the program to tell the person about.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen: Vec<&str> = Vec::new();
        let mut highest = 0u32;
        for (index, step) in self.steps.iter().enumerate() {
            match step_number(&step.id) {
                Some(n) => highest = highest.max(n),
                None => out.push(format!("Step {} has an id that is not of the form step-<number>.", step.id)),
            }
            if seen.contains(&step.id.as_str()) {
                out.push(format!("Two steps have the id {}.", step.id));
            }
            seen.push(&step.id);

            if let Some(parent) = &step.parent {
                if parent == &step.id {
                    out.push(format!("Step {} is its own parent.", step.id));
                } else {
                    match self.steps.iter().position(|s| &s.id == parent) {
                        None => out.push(format!("Step {} has a parent, {}, that does not exist.", step.id, parent)),
                        Some(p) if p > index => {
                            out.push(format!("Step {} comes before its parent {}.", step.id, parent))
                        }
                        Some(_) => {}
                    }
                }
            }

            match (step.kind, &step.image) {
                (StepKind::Image, None) => out.push(format!("Step {} should have a picture but has none.", step.id)),
                (StepKind::Empty | StepKind::Block, Some(_)) => {
                    out.push(format!("Step {} has a picture but is not a picture step.", step.id))
                }
                _ => {}
            }
            if let Some(image) = &step.image {
                if !is_safe_picture_path(&image.file) {
                    out.push(format!("Step {} names a picture file that is not allowed: {}.", step.id, image.file));
                }
                if image.width == 0 || image.height == 0 {
                    out.push(format!("Step {} has a picture with no size.", step.id));
                }
            }
            if let Some(focus) = &step.focus {
                if focus.width <= 0 || focus.height <= 0 {
                    out.push(format!("Step {} has a focused view with no size.", step.id));
                }
            }

            let mut annotation_ids: Vec<&str> = Vec::new();
            for annotation in &step.annotations {
                if annotation_ids.contains(&annotation.id.as_str()) {
                    out.push(format!("Step {} has two marks with the id {}.", step.id, annotation.id));
                }
                annotation_ids.push(&annotation.id);
                for problem in annotation.problems() {
                    out.push(format!("Step {}, mark {}: {}", step.id, annotation.id, problem));
                }
            }
        }
        if self.next_step_number <= highest {
            out.push(format!(
                "The next step number is {} but a step numbered {} already exists.",
                self.next_step_number, highest
            ));
        }
        out
    }
}

/// The number in an id such as `step-12`.
pub fn step_number(id: &str) -> Option<u32> {
    let digits = id.strip_prefix("step-")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().filter(|n| *n >= 1)
}

/// True if `path` is a picture path the format allows: `images/` and one plain file name, nothing that climbs out of
/// the guide folder.
pub fn is_safe_picture_path(path: &str) -> bool {
    let Some(name) = path.strip_prefix("images/") else {
        return false;
    };
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':', '\0'])
        && !name.starts_with('.')
        && name.chars().all(|c| !c.is_control())
}

/// What a step is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StepKind {
    /// A picture, a title and a description.
    Image,
    /// A step with no picture.
    Empty,
    /// A plain text content block (not in the table of contents).
    Block,
}

/// One step of a guide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub id: String,
    pub kind: StepKind,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub image: Option<StepImage>,
    /// Alt text the author wrote. If empty, exports use the title.
    #[serde(default)]
    pub image_description: String,
    /// The id of the parent step, or none for a top-level step.
    pub parent: Option<String>,
    /// Left out of every export.
    #[serde(default)]
    pub skipped: bool,
    pub click: Option<Click>,
    pub control: Option<Control>,
    /// The text of a `Type` step. Private: exports show it only as the step title.
    pub typed: Option<String>,
    /// The program's file name only. Window titles are never stored.
    pub program: Option<String>,
    /// The focused view crop, in picture pixels. None means the whole picture.
    pub focus: Option<Rect>,
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    pub captured_at: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Step {
    /// A step with only a title and nothing else. The id comes from the document, not from here.
    pub fn bare(id: &str, kind: StepKind, title: &str) -> Step {
        Step {
            id: id.to_string(),
            kind,
            title: title.to_string(),
            description: String::new(),
            image: None,
            image_description: String::new(),
            parent: None,
            skipped: false,
            click: None,
            control: None,
            typed: None,
            program: None,
            focus: None,
            annotations: Vec::new(),
            captured_at: None,
            extra: Extra::new(),
        }
    }
}

/// A step's picture: an original that is never modified. Everything else is drawn on top of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepImage {
    /// `images/step-0001.png`, relative to the guide folder.
    pub file: String,
    pub width: u32,
    pub height: u32,
    #[serde(flatten)]
    pub extra: Extra,
}

impl StepImage {
    pub fn new(file: &str, width: u32, height: u32) -> StepImage {
        StepImage { file: file.to_string(), width, height, extra: Extra::new() }
    }
}

/// Which mouse button a click used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Button {
    #[default]
    Left,
    Right,
    Middle,
}

/// Where a click happened, in picture pixels, top left origin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Click {
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub button: Button,
    #[serde(flatten)]
    pub extra: Extra,
}

/// The control that was clicked, as UI Automation named it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Control {
    pub name: String,
    /// Capture's English role names: `button`, `link`, `edit`, and so on.
    pub role: String,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A rectangle in picture pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect { x, y, width, height, extra: Extra::new() }
    }
}

/// The kinds of mark that can be drawn on a step's picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnnotationKind {
    Rect,
    Oval,
    Line,
    Arrow,
    Text,
    Number,
    Highlight,
    Blur,
    Spotlight,
    Cursor,
    ClickMark,
}

/// One mark drawn on a picture, such as a circle round a click. Which fields a kind uses is in `docs/guide-format.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: AnnotationKind,
    pub x: i32,
    pub y: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x2: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y2: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_width: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Annotation {
    /// A mark of this kind at a point with every optional field empty. Fill in what the kind needs.
    pub fn at(id: &str, kind: AnnotationKind, x: i32, y: i32) -> Annotation {
        Annotation {
            id: id.to_string(),
            kind,
            x,
            y,
            width: None,
            height: None,
            x2: None,
            y2: None,
            size: None,
            fill: None,
            stroke: None,
            stroke_width: None,
            text: None,
            font_size: None,
            style: None,
            number: None,
            extra: Extra::new(),
        }
    }

    /// What is wrong with this mark, in words. Empty if it is sound.
    pub fn problems(&self) -> Vec<String> {
        use AnnotationKind::*;
        let mut out = Vec::new();
        let positive = |value: Option<i32>| value.is_some_and(|v| v > 0);
        match self.kind {
            Rect | Oval | Highlight | Blur | Spotlight => {
                if !positive(self.width) || !positive(self.height) {
                    out.push("needs a width and a height above zero.".to_string());
                }
            }
            Line | Arrow => {
                if self.x2.is_none() || self.y2.is_none() {
                    out.push("needs an end point.".to_string());
                }
            }
            Text => {
                if self.text.is_none() {
                    out.push("needs its text.".to_string());
                }
            }
            Number => {
                if self.number.is_none() {
                    out.push("needs its number.".to_string());
                }
            }
            Cursor => {
                if self.style.is_none() {
                    out.push("needs a cursor style.".to_string());
                }
            }
            ClickMark => {
                if !positive(self.size) {
                    out.push("needs a size above zero.".to_string());
                }
            }
        }
        for (what, value) in [("fill", &self.fill), ("stroke", &self.stroke)] {
            if let Some(colour) = value {
                if !is_colour(colour) {
                    out.push(format!("the {what} colour {colour} is not #RRGGBB or #RRGGBBAA."));
                }
            }
        }
        out
    }
}

/// True for `#RRGGBB` and `#RRGGBBAA`.
pub fn is_colour(text: &str) -> bool {
    let Some(hex) = text.strip_prefix('#') else {
        return false;
    };
    (hex.len() == 6 || hex.len() == 8) && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    /// A guide that sets every field the format has, including some unknown ones, so a round trip can be exact.
    pub(crate) fn full_fixture() -> Value {
        json!({
            "schemaVersion": "0.1.0",
            "id": "g-2026-10-31-1432-a1b2",
            "title": "How to submit an assignment",
            "description": "",
            "language": "en",
            "createdAt": "2026-10-31T14:32:05Z",
            "updatedAt": "2026-10-31T14:40:11Z",
            "focusedView": true,
            "nextStepNumber": 4,
            "futureGuideField": {"kept": [1, 2, 3]},
            "steps": [
                {
                    "id": "step-1",
                    "kind": "image",
                    "title": "Click \"Assignments\"",
                    "description": "",
                    "image": {"file": "images/step-0001.png", "width": 1920, "height": 1080, "dpi": 144},
                    "imageDescription": "",
                    "parent": null,
                    "skipped": false,
                    "click": {"x": 312, "y": 188, "button": "left", "pressure": 0.5},
                    "control": {"name": "Assignments", "role": "link", "automationId": "nav-3"},
                    "typed": null,
                    "program": "chrome.exe",
                    "focus": {"x": 0, "y": 0, "width": 1280, "height": 720},
                    "annotations": [
                        {"id": "a1", "type": "clickMark", "x": 312, "y": 188, "size": 40,
                         "fill": "#7D87F233", "stroke": "#7D87F2", "strokeWidth": 4, "futureMarkField": true},
                        {"id": "a2", "type": "cursor", "x": 312, "y": 188, "style": "pointer", "size": 28}
                    ],
                    "capturedAt": "2026-10-31T14:32:41Z",
                    "futureStepField": "kept"
                },
                {
                    "id": "step-3",
                    "kind": "empty",
                    "title": "Do the next part",
                    "description": "A note.",
                    "image": null,
                    "imageDescription": "",
                    "parent": "step-1",
                    "skipped": true,
                    "click": null,
                    "control": null,
                    "typed": null,
                    "program": null,
                    "focus": null,
                    "annotations": [],
                    "capturedAt": null
                }
            ]
        })
    }

    #[test]
    fn a_full_guide_round_trips_exactly_including_unknown_fields() {
        let fixture = full_fixture();
        let guide: Guide = serde_json::from_value(fixture.clone()).unwrap();
        assert_eq!(serde_json::to_value(&guide).unwrap(), fixture);
        assert_eq!(guide.extra["futureGuideField"], json!({"kept": [1, 2, 3]}));
        assert_eq!(guide.steps[0].extra["futureStepField"], json!("kept"));
        assert_eq!(guide.steps[0].image.as_ref().unwrap().extra["dpi"], json!(144));
        assert_eq!(guide.steps[0].annotations[0].extra["futureMarkField"], json!(true));
    }

    #[test]
    fn the_fixture_is_sound() {
        let guide: Guide = serde_json::from_value(full_fixture()).unwrap();
        assert_eq!(guide.problems(), Vec::<String>::new());
    }

    #[test]
    fn missing_optional_fields_read_as_defaults() {
        let guide: Guide = serde_json::from_value(json!({
            "schemaVersion": "0.1.0", "id": "g", "title": "T", "createdAt": "x", "updatedAt": "x",
            "nextStepNumber": 2,
            "steps": [{"id": "step-1", "kind": "empty"}]
        }))
        .unwrap();
        assert_eq!(guide.language, "en");
        assert!(guide.focused_view);
        let step = &guide.steps[0];
        assert_eq!((step.title.as_str(), step.skipped, step.parent.is_none()), ("", false, true));
        assert!(step.annotations.is_empty());
    }

    #[test]
    fn a_missing_required_field_is_an_error() {
        let result: Result<Guide, _> = serde_json::from_value(json!({"schemaVersion": "0.1.0", "title": "T"}));
        assert!(result.is_err());
    }

    fn with_steps(steps: Vec<Step>) -> Guide {
        let mut guide = Guide::new("g", "T", "en", "2026-10-31T00:00:00Z");
        guide.next_step_number = 100;
        guide.steps = steps;
        guide
    }

    fn picture_step(id: &str) -> Step {
        let mut step = Step::bare(id, StepKind::Image, "Click");
        step.image = Some(StepImage::new("images/step-0001.png", 100, 50));
        step
    }

    #[test]
    fn problems_catch_duplicate_ids_and_bad_ids() {
        let guide = with_steps(vec![picture_step("step-1"), picture_step("step-1"), picture_step("oops")]);
        let text = guide.problems().join("\n");
        assert!(text.contains("Two steps have the id step-1"), "{text}");
        assert!(text.contains("oops"), "{text}");
    }

    #[test]
    fn problems_catch_a_child_before_its_parent_and_missing_parents() {
        let mut child = picture_step("step-1");
        child.parent = Some("step-2".into());
        let mut orphan = picture_step("step-3");
        orphan.parent = Some("step-99".into());
        let mut selfish = picture_step("step-4");
        selfish.parent = Some("step-4".into());
        let guide = with_steps(vec![child, picture_step("step-2"), orphan, selfish]);
        let text = guide.problems().join("\n");
        assert!(text.contains("Step step-1 comes before its parent step-2"), "{text}");
        assert!(text.contains("parent, step-99, that does not exist"), "{text}");
        assert!(text.contains("its own parent"), "{text}");
    }

    #[test]
    fn problems_catch_picture_mismatches_and_unsafe_paths() {
        let mut no_picture = Step::bare("step-1", StepKind::Image, "x");
        no_picture.image = None;
        let mut extra_picture = Step::bare("step-2", StepKind::Empty, "x");
        extra_picture.image = Some(StepImage::new("images/a.png", 1, 1));
        let mut climbing = picture_step("step-3");
        climbing.image = Some(StepImage::new("images/../secret.png", 1, 1));
        let mut absolute = picture_step("step-4");
        absolute.image = Some(StepImage::new("C:/Windows/x.png", 1, 1));
        let text = with_steps(vec![no_picture, extra_picture, climbing, absolute]).problems().join("\n");
        assert!(text.contains("should have a picture"), "{text}");
        assert!(text.contains("is not a picture step"), "{text}");
        assert!(text.contains("images/../secret.png"), "{text}");
        assert!(text.contains("C:/Windows/x.png"), "{text}");
    }

    #[test]
    fn safe_picture_paths() {
        assert!(is_safe_picture_path("images/step-0001.png"));
        assert!(is_safe_picture_path("images/step-0001-2.png"));
        for bad in [
            "",
            "images/",
            "images/..",
            "images/../x.png",
            "images/a/b.png",
            "images\\a.png",
            "/images/a.png",
            "trash/a.png",
            "images/C:evil.png",
            "images/.hidden.png",
            "images/a\u{0}.png",
        ] {
            assert!(!is_safe_picture_path(bad), "{bad:?} should be refused");
        }
    }

    #[test]
    fn problems_catch_a_next_step_number_that_is_too_low() {
        let mut guide = with_steps(vec![picture_step("step-7")]);
        guide.next_step_number = 7;
        assert!(guide.problems().join("\n").contains("next step number is 7"));
        guide.next_step_number = 8;
        assert!(guide.problems().is_empty());
    }

    #[test]
    fn annotation_problems_by_kind() {
        use AnnotationKind::*;
        let bare = |kind| Annotation::at("a1", kind, 0, 0);
        for kind in [Rect, Oval, Highlight, Blur, Spotlight, Line, Arrow, Text, Number, Cursor, ClickMark] {
            assert_eq!(bare(kind).problems().len(), 1, "{kind:?} with nothing filled in should have one problem");
        }
        let mut rect = bare(Rect);
        rect.width = Some(10);
        rect.height = Some(0);
        assert_eq!(rect.problems().len(), 1);
        rect.height = Some(5);
        assert!(rect.problems().is_empty());
        let mut mark = bare(ClickMark);
        mark.size = Some(40);
        mark.fill = Some("red".into());
        mark.stroke = Some("#7D87F2".into());
        assert_eq!(mark.problems().len(), 1);
        mark.fill = Some("#7D87F233".into());
        assert!(mark.problems().is_empty());
    }

    #[test]
    fn step_numbers() {
        assert_eq!(step_number("step-1"), Some(1));
        assert_eq!(step_number("step-120"), Some(120));
        for bad in ["step-0", "step-", "step-1a", "Step-1", "step--1", "x", "step-+3"] {
            assert_eq!(step_number(bad), None, "{bad}");
        }
    }
}
