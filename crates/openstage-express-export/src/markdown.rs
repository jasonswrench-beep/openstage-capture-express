// SPDX-License-Identifier: Apache-2.0
//! The Markdown export (spec section 10): one `.md` file and a folder of pictures beside it.
//!
//! Each step is a heading (`## 1. Title`, one level deeper for a substep), its description as the author wrote it (the
//! small Markdown subset), and its picture as `![alt text](images/step-01.png)`. A content block is a quotation. The
//! pictures have the marks burned in, exactly as in the other exports.

use openstage_express_core::fsutil::sanitise_folder_name;
use openstage_express_core::StepKind;

use crate::encoding::{markdown_escape, markdown_to_markdown};
use crate::plan::Plan;
use crate::{Export, OutFile};

/// The text of the Markdown file.
pub fn markdown_text(plan: &Plan) -> String {
    let mut blocks: Vec<String> = vec![format!("# {}", markdown_escape(&plan.title))];
    // Descriptions are written from the subset they were read as, escaped (the security review of 2026-11-13): written as typed,
    // a guide from someone else could carry HTML, a picture fetched from a server, or a script link to a viewer that shows them.
    blocks.extend(markdown_to_markdown(&plan.description));
    for step in &plan.steps {
        if step.kind == StepKind::Block {
            let mut quote = vec![format!("> **{}**", markdown_escape(&step.title))];
            let description = markdown_to_markdown(&step.description).join("\n\n");
            if !description.is_empty() {
                quote.push(">".to_string());
                quote.extend(description.lines().map(|l| {
                    if l.is_empty() {
                        ">".to_string()
                    } else {
                        format!("> {l}")
                    }
                }));
            }
            blocks.push(quote.join("\n"));
            continue;
        }
        let level = (step.depth + 2).min(6);
        blocks.push(format!(
            "{} {}. {}",
            "#".repeat(level),
            step.number.as_deref().unwrap_or(""),
            markdown_escape(&step.title)
        ));
        blocks.extend(markdown_to_markdown(&step.description));
        if let Some(picture) = &step.picture {
            blocks.push(format!("![{}]({})", markdown_escape(&step.alt), picture.file));
        }
    }
    let mut text = blocks.join("\n\n");
    text.push('\n');
    text
}

/// The Markdown export: `<title>.md` and `images/step-NN.png`.
pub fn export_markdown(plan: &Plan) -> Export {
    let mut files = vec![OutFile {
        path: format!("{}.md", sanitise_folder_name(&plan.title)),
        bytes: markdown_text(plan).into_bytes(),
    }];
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

    fn demo_plan() -> Plan {
        let d = demo();
        plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default())
    }

    #[test]
    fn the_demo_guide_comes_out_exactly_like_this() {
        let expected = "\
# How to submit an assignment

Hand in a piece of work on the course site, step by step.

## 1. Click \"Assignments\"

Open the **Assignments** page from the menu on the left.

![The course site with the Assignments item in the left menu circled](images/step-01.png)

## 2. Click \"New assignment\"

Start here: this opens the form.

![The New assignment button with an arrow pointing at it](images/step-02.png)

### 2.1. Type \"Week 3 reflection\"

Give the work a short name.

- Use words your teacher will recognise
- You can change it later

![The Name box filled in with Week 3 reflection](images/step-03.png)

## 3. Select \"Course\"

Choose the course the work belongs to.

![The Course list with Communication 360 chosen](images/step-04.png)

> **Tip**
>
> **Tip:** you can [read the help](https://example.com/help) at any time.
>
> Saved work is kept for a year.

## 4. Click \"Submit\"

Check your details, then submit. The student's name is hidden in this guide.

![The Submit button, with the student's name blurred](images/step-06.png)
";
        assert_eq!(markdown_text(&demo_plan()), expected);
    }

    #[test]
    fn the_export_has_the_text_and_every_picture_and_the_links_point_at_real_files() {
        let p = demo_plan();
        let export = export_markdown(&p);
        assert_eq!(export.files[0].path, "How to submit an assignment.md");
        let text = String::from_utf8(export.files[0].bytes.clone()).unwrap();
        let pictures: Vec<&OutFile> = export.files.iter().skip(1).collect();
        assert_eq!(pictures.len(), 5);
        for file in &pictures {
            assert!(file.bytes.starts_with(&[0x89, b'P', b'N', b'G']));
            assert!(text.contains(&format!("]({})", file.path)), "{}", file.path);
        }
        assert_eq!(text.matches("](images/").count(), pictures.len());
    }

    #[test]
    fn special_characters_in_titles_and_alt_text_cannot_break_the_page() {
        let mut d = demo();
        d.document.guide.title = "Use *stars* & [brackets]".to_string();
        d.document.guide.steps[0].title = "Click \"A_B\" #1 <now>".to_string();
        d.document.guide.steps[0].image_description = "A ] picture [with] odd `marks`".to_string();
        let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let text = markdown_text(&p);
        assert!(text.starts_with("# Use \\*stars\\* & \\[brackets\\]\n"), "{text}");
        assert!(text.contains("## 1. Click \"A\\_B\" \\#1 \\<now\\>\n"));
        assert!(text.contains("![A \\] picture \\[with\\] odd \\`marks\\`](images/step-01.png)"));
    }

    /// The security review of 2026-11-13: descriptions went into the file as typed, so a guide from someone else could carry HTML, a
    /// picture fetched from a server, or a script link to a viewer that shows them. Now only the subset passes, and the rest is text.
    #[test]
    fn html_a_remote_picture_and_a_script_link_in_a_description_come_out_as_plain_escaped_text() {
        let mut p = demo_plan();
        p.description = "<script>alert(1)</script> intro".to_string();
        p.steps[0].description = "<img src=https://evil.example/t.png onerror=alert(1)>\n\
             ![seen](https://evil.example/track.png)\n\
             [click](javascript:alert(1))\n\n\
             **Bold** and [the help page](https://example.org/help)\n\n\
             - one\n- two"
            .to_string();
        let text = markdown_text(&p);
        // No tag gets through: every angle bracket has a backslash in front of it, except the `>` that starts a line of a quotation.
        for (at, _) in text.match_indices(['<', '>']) {
            let quote_marker = text[at..].starts_with('>') && (at == 0 || text[..at].ends_with('\n'));
            assert!(quote_marker || text[..at].ends_with('\\'), "an angle bracket got through at {at}: {text}");
        }
        assert!(text.contains(r"\<img src=https://evil.example/t.png onerror=alert(1)\>"), "{text}");
        assert!(text.contains(r"\<script\>alert(1)\</script\> intro"), "{text}");
        // The only pictures are the guide's own: every other `![` has a backslash in front of it.
        for (at, _) in text.match_indices("![") {
            let escaped = text[..at].ends_with('\\');
            let line = text[at..].lines().next().unwrap_or("");
            assert!(escaped || line.contains("](images/"), "a picture link got through: {line}");
        }
        assert!(text.contains(r"\![seen](https://evil.example/track.png)"), "a link to the page, never a picture: {text}");
        assert!(text.contains(r"\[click\](javascript:alert(1))"), "{text}");
        for (at, _) in text.match_indices("](javascript") {
            assert!(text[..at].ends_with('\\'), "a script link got through: {text}");
        }
        assert!(text.contains("**Bold** and [the help page](https://example.org/help)"), "{text}");
        assert!(text.contains("- one\n- two"), "{text}");
    }

    #[test]
    fn an_empty_guide_is_just_its_title() {
        let guide = openstage_express_core::Guide::new("g", "Nothing yet", "en", "t");
        let p = plan(&guide, &crate::picture::MemoryPictures::default(), &BlockText, &ExportOptions::default());
        assert_eq!(markdown_text(&p), "# Nothing yet\n");
        assert_eq!(export_markdown(&p).files.len(), 1);
    }

    #[test]
    fn a_step_with_no_picture_has_no_image_line() {
        let d = demo();
        let p =
            plan(&d.document.guide, &crate::picture::MemoryPictures::default(), &BlockText, &ExportOptions::default());
        let text = markdown_text(&p);
        assert!(!text.contains("!["));
        assert!(text.contains("## 1. Click \"Assignments\""));
        assert!(!export_markdown(&p).warnings.is_empty());
    }
}
