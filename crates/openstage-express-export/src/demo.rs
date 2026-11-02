// SPDX-License-Identifier: Apache-2.0
//! A made-up guide with made-up screenshots, so every export can be tried, tested and looked at without capturing anything
//! (`EXPORT DEMO.bat` uses it, and so do the tests).
//!
//! The screenshots are drawn here: a small imaginary course site with a menu, a form and some buttons. The words on them
//! are drawn as plain blocks, because the screenshots are made up (the words on the marks put over them are drawn by whatever text
//! renderer an export is given, which is Windows' own in the program).

use openstage_capture_render::{BlockText, Canvas, Picture, PixelRect, Rgb, TextRenderer, TextRequest};
use openstage_express_core::focus::{auto_focus, DEFAULT_ZOOM_PERCENT};
use openstage_express_core::{
    Annotation, AnnotationKind, Button, Click, Control, Document, Guide, Position, Rect, StepDraft, StepImage, StepKind,
};

use crate::picture::MemoryPictures;

const NOW: &str = "2026-11-01T10:00:00Z";
const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

/// A guide and the pictures its steps name.
pub struct Demo {
    pub document: Document,
    pub pictures: MemoryPictures,
}

fn label(canvas: &mut Canvas, text: &str, area: PixelRect, font_px: f32, colour: Rgb, background: Option<Rgb>) {
    let request = TextRequest {
        text: text.to_string(),
        area,
        font_px,
        bold: false,
        color: colour,
        background,
        align: openstage_capture_project::EditAlign::Left,
    };
    let _ = BlockText.draw(canvas, &request);
}

/// The imaginary page. `typed` fills the name field, `course` fills the course box.
fn screen(typed: &str, course: &str, name_shown: bool) -> Canvas {
    let mut c = Canvas::new(WIDTH, HEIGHT);
    c.clear(Rgb(0xEE, 0xF0, 0xF5));
    c.fill_rect(PixelRect::new(0, 0, WIDTH as i32, 48), Rgb(0x1F, 0x2A, 0x44));
    label(&mut c, "Course site", PixelRect::new(16, 8, 300, 32), 22.0, Rgb(255, 255, 255), None);
    c.fill_rect(PixelRect::new(0, 48, 220, 672), Rgb(0xDD, 0xE1, 0xEE));
    for (i, name) in ["Home", "Assignments", "Grades", "Messages"].iter().enumerate() {
        let top = 100 + i as i32 * 50;
        label(&mut c, name, PixelRect::new(20, top, 180, 40), 20.0, Rgb(0x1F, 0x2A, 0x44), Some(Rgb(0xC9, 0xCF, 0xE4)));
    }
    c.fill_rect(PixelRect::new(260, 90, 960, 590), Rgb(255, 255, 255));
    c.stroke_rect(PixelRect::new(260, 90, 960, 590), 2, Rgb(0xC0, 0xC6, 0xD8));
    label(
        &mut c,
        "New assignment",
        PixelRect::new(280, 110, 200, 44),
        20.0,
        Rgb(255, 255, 255),
        Some(Rgb(0x3F, 0x4B, 0xD4)),
    );
    label(&mut c, "Name", PixelRect::new(280, 190, 200, 28), 18.0, Rgb(0x33, 0x33, 0x44), None);
    c.stroke_rect(PixelRect::new(280, 220, 500, 40), 2, Rgb(0x80, 0x88, 0xA8));
    if !typed.is_empty() {
        label(&mut c, typed, PixelRect::new(286, 224, 480, 32), 20.0, Rgb(0x11, 0x11, 0x22), None);
    }
    label(&mut c, "Course", PixelRect::new(280, 270, 200, 28), 18.0, Rgb(0x33, 0x33, 0x44), None);
    c.stroke_rect(PixelRect::new(280, 300, 500, 40), 2, Rgb(0x80, 0x88, 0xA8));
    if !course.is_empty() {
        label(&mut c, course, PixelRect::new(286, 304, 480, 32), 20.0, Rgb(0x11, 0x11, 0x22), None);
    }
    label(&mut c, "Student", PixelRect::new(280, 350, 200, 28), 18.0, Rgb(0x33, 0x33, 0x44), None);
    c.stroke_rect(PixelRect::new(280, 380, 500, 40), 2, Rgb(0x80, 0x88, 0xA8));
    if name_shown {
        label(&mut c, "Jordan Rivera 4417", PixelRect::new(286, 384, 480, 32), 20.0, Rgb(0x11, 0x11, 0x22), None);
    }
    label(&mut c, "Submit", PixelRect::new(280, 520, 160, 44), 20.0, Rgb(255, 255, 255), Some(Rgb(0x1E, 0x7B, 0x4A)));
    c
}

fn mark(kind: AnnotationKind, x: i32, y: i32) -> Annotation {
    Annotation::at("", kind, x, y)
}

fn click_marks(x: i32, y: i32) -> Vec<Annotation> {
    let mut ring = mark(AnnotationKind::ClickMark, x, y);
    ring.size = Some(56);
    let mut cursor = mark(AnnotationKind::Cursor, x, y);
    cursor.style = Some("pointer".to_string());
    cursor.size = Some(32);
    vec![ring, cursor]
}

fn boxed(kind: AnnotationKind, rect: (i32, i32, i32, i32)) -> Annotation {
    let mut a = mark(kind, rect.0, rect.1);
    a.width = Some(rect.2);
    a.height = Some(rect.3);
    a
}

struct Shot {
    title: &'static str,
    description: &'static str,
    alt: &'static str,
    click: (i32, i32),
    control: (&'static str, &'static str),
    typed: Option<&'static str>,
    canvas: Canvas,
    marks: Vec<Annotation>,
}

/// The imaginary guide: seven steps (one skipped, one substep, one content block) and the six pictures they use.
pub fn demo() -> Demo {
    let mut doc = Document::new(Guide::new("g-demo", "How to submit an assignment", "en", NOW));
    doc.set_guide_description("Hand in a piece of work on the course site, step by step.", NOW);
    let mut pictures = MemoryPictures::default();
    let mut next_picture = 1;
    let mut add_shot = |doc: &mut Document, shot: Shot, position: Position<'_>, skipped: bool| -> String {
        let file = format!("images/step-{next_picture:04}.png");
        next_picture += 1;
        pictures.0.insert(file.clone(), Picture::solid_pixels(WIDTH, HEIGHT, shot.canvas.bgra.clone()));
        let mut draft = StepDraft::with_picture(shot.title, StepImage::new(&file, WIDTH, HEIGHT));
        draft.description = shot.description.to_string();
        draft.image_description = shot.alt.to_string();
        draft.click = Some(Click { x: shot.click.0, y: shot.click.1, button: Button::Left, extra: Default::default() });
        draft.control = Some(Control {
            name: shot.control.0.to_string(),
            role: shot.control.1.to_string(),
            extra: Default::default(),
        });
        draft.typed = shot.typed.map(str::to_string);
        draft.program = Some("browser.exe".to_string());
        draft.captured_at = Some(NOW.to_string());
        draft.focus = auto_focus(WIDTH, HEIGHT, Some(shot.click), DEFAULT_ZOOM_PERCENT);
        draft.annotations = shot
            .marks
            .into_iter()
            .enumerate()
            .map(|(i, mut m)| {
                m.id = format!("a{}", i + 1);
                m
            })
            .collect();
        let id = doc.add_step(draft, position, NOW).expect("the demo steps are well formed");
        if skipped {
            doc.set_skipped(&id, true, NOW).expect("the step was just added");
        }
        id
    };

    add_shot(
        &mut doc,
        Shot {
            title: "Click \"Assignments\"",
            description: "Open the **Assignments** page from the menu on the left.",
            alt: "The course site with the Assignments item in the left menu circled",
            click: (110, 170),
            control: ("Assignments", "link"),
            typed: None,
            canvas: screen("", "", true),
            marks: click_marks(110, 170),
        },
        Position::End,
        false,
    );

    let mut arrow = mark(AnnotationKind::Arrow, 720, 250);
    arrow.x2 = Some(490);
    arrow.y2 = Some(150);
    arrow.stroke = Some("#D6336C".to_string());
    arrow.stroke_width = Some(5);
    let mut note = mark(AnnotationKind::Text, 560, 262);
    note.text = Some("Start here".to_string());
    note.font_size = Some(26);
    let mut marks = click_marks(380, 132);
    marks.push(arrow);
    marks.push(note);
    let second = add_shot(
        &mut doc,
        Shot {
            title: "Click \"New assignment\"",
            description: "Start here: this opens the form.",
            alt: "The New assignment button with an arrow pointing at it",
            click: (380, 132),
            control: ("New assignment", "button"),
            typed: None,
            canvas: screen("", "", true),
            marks,
        },
        Position::End,
        false,
    );

    let mut oval = boxed(AnnotationKind::Oval, (270, 212, 520, 56));
    oval.stroke = Some("#D6336C".to_string());
    oval.stroke_width = Some(4);
    add_shot(
        &mut doc,
        Shot {
            title: "Type \"Week 3 reflection\"",
            description:
                "Give the work a short name.\n\n- Use words your teacher will recognise\n- You can change it later",
            alt: "The Name box filled in with Week 3 reflection",
            click: (530, 240),
            control: ("Name", "edit"),
            typed: Some("Week 3 reflection"),
            canvas: screen("Week 3 reflection", "", true),
            marks: vec![oval],
        },
        Position::ChildOf(&second),
        false,
    );

    let mut highlight = boxed(AnnotationKind::Highlight, (280, 300, 500, 40));
    highlight.fill = Some("#FFE60066".to_string());
    add_shot(
        &mut doc,
        Shot {
            title: "Select \"Course\"",
            description: "Choose the course the work belongs to.",
            alt: "The Course list with Communication 360 chosen",
            click: (530, 320),
            control: ("Course", "combo box"),
            typed: None,
            canvas: screen("Week 3 reflection", "Communication 360", true),
            marks: vec![highlight],
        },
        Position::End,
        false,
    );

    add_shot(
        &mut doc,
        Shot {
            title: "Click \"Debug\"",
            description: "An accidental click that should not be in the guide.",
            alt: "",
            click: (900, 600),
            control: ("Debug", "button"),
            typed: None,
            canvas: screen("Week 3 reflection", "Communication 360", true),
            marks: click_marks(900, 600),
        },
        Position::End,
        true,
    );

    let mut tip = StepDraft::empty("Tip");
    tip.kind = StepKind::Block;
    tip.description =
        "**Tip:** you can [read the help](https://example.com/help) at any time.\n\nSaved work is kept for a year."
            .to_string();
    doc.add_step(tip, Position::End, NOW).expect("the tip is well formed");

    let blur = boxed(AnnotationKind::Blur, (280, 380, 500, 40));
    add_shot(
        &mut doc,
        Shot {
            title: "Click \"Submit\"",
            description: "Check your details, then submit. The student's name is hidden in this guide.",
            alt: "The Submit button, with the student's name blurred",
            click: (360, 542),
            control: ("Submit", "button"),
            typed: None,
            canvas: screen("Week 3 reflection", "Communication 360", true),
            marks: {
                let mut marks = vec![blur];
                marks.extend(click_marks(360, 542));
                marks
            },
        },
        Position::End,
        false,
    );

    // Everything the demo says about focus must stay inside its pictures.
    for step in &doc.guide.steps {
        if let Some(focus) = &step.focus {
            let inside = Rect::new(0, 0, WIDTH as i32, HEIGHT as i32);
            debug_assert!(focus.x >= inside.x && focus.y >= inside.y && focus.x + focus.width <= inside.width);
        }
    }
    Demo { document: doc, pictures }
}

/// A busy picture: a smooth sweep of colour with a little noise, as a photograph or a crowded screen has. A PNG cannot squeeze it much,
/// and a JPG makes it a good deal smaller. The same every time.
pub fn busy_picture(width: u32, height: u32) -> Picture {
    let mut seed = 12_345_u32;
    let mut bgra = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = ((seed >> 24) % 13) as i32 - 6;
            let level = |base: u32| (base as i32 + noise).clamp(0, 255) as u8;
            bgra.extend_from_slice(&[level(x * 255 / width), level(y * 255 / height), level(128), 255]);
        }
    }
    Picture::new(width, height, bgra)
}

/// A guide of two steps with pictures 320 by 240 pixels: "Busy", whose picture is smaller as a JPG, and "Plain", one flat colour,
/// whose picture is smaller as a PNG. For trying what happens when a guide holds both kinds.
pub fn busy_then_plain() -> Demo {
    let mut doc = Document::new(Guide::new("g-mixed", "Busy then plain", "en", NOW));
    let mut pictures = MemoryPictures::default();
    pictures.0.insert("images/busy.png".to_string(), busy_picture(320, 240));
    pictures.0.insert("images/plain.png".to_string(), Picture::solid(320, 240, Rgb(30, 90, 160)));
    for (title, file) in [("Busy", "images/busy.png"), ("Plain", "images/plain.png")] {
        let draft = StepDraft::with_picture(title, StepImage::new(file, 320, 240));
        doc.add_step(draft, Position::End, NOW).expect("the steps are well formed");
    }
    Demo { document: doc, pictures }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_busy_then_plain_guide_is_sound_and_the_same_every_time() {
        let (a, b) = (busy_then_plain(), busy_then_plain());
        assert!(a.document.guide.problems().is_empty(), "{:?}", a.document.guide.problems());
        assert_eq!(a.document.guide.steps.len(), 2);
        assert!(a
            .document
            .guide
            .steps
            .iter()
            .all(|s| s.image.as_ref().is_some_and(|i| a.pictures.0.contains_key(&i.file))));
        assert_eq!(a.document, b.document);
        assert_eq!(a.pictures.0, b.pictures.0);
    }

    #[test]
    fn the_demo_guide_is_sound_and_has_what_the_exports_need_to_show_off() {
        let d = demo();
        assert!(d.document.guide.problems().is_empty(), "{:?}", d.document.guide.problems());
        let steps = &d.document.guide.steps;
        assert_eq!(steps.len(), 7);
        assert!(steps.iter().any(|s| s.skipped));
        assert!(steps.iter().any(|s| s.parent.is_some()));
        assert!(steps.iter().any(|s| s.kind == StepKind::Block));
        assert!(steps.iter().any(|s| s.typed.is_some()));
        assert!(steps.iter().all(|s| s.image.as_ref().is_none_or(|i| d.pictures.0.contains_key(&i.file))));
    }

    #[test]
    fn the_demo_is_the_same_every_time() {
        let (a, b) = (demo(), demo());
        assert_eq!(a.document, b.document);
        assert_eq!(a.pictures.0, b.pictures.0);
    }
}
