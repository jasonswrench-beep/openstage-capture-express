// SPDX-License-Identifier: Apache-2.0
//! The HTML export (spec section 10.1): one self-contained, accessible page.
//!
//! * One file. The pictures are inside it as data, there are no web fonts, no outside scripts and no requests of any kind.
//!   A Content-Security-Policy in the page forbids everything except its own style, its pictures, and one script named by
//!   its hash.
//! * `lang` is the guide's language; one `h1` (the title); each step an `h2` (or deeper for a substep) inside an ordered
//!   list; a `nav` that lists the steps; a "skip to the steps" link; one `main`.
//! * Every picture has real alt text (the author's, or the step title). Step numbers are real text.
//! * Text is at least 4.5 to 1 against its background in the light and the dark scheme, focus is a 3 pixel ring, the page
//!   reflows down to 320 pixels wide, honours reduced motion, and has a print style that keeps a picture on one page.
//! * A picture opens larger in a `dialog` (Escape closes it and focus goes back to the picture). Without the script the
//!   pictures are simply there at full width.
//!
//! Words drawn into a picture are not seen by a screen reader, so the plan warns when a text mark's words are missing from
//! the step's text.

use openstage_express_core::fsutil::sanitise_folder_name;
use openstage_express_core::{Language, StepKind};

use crate::encoding::{base64_encode, csp_script_hash, html_escape, markdown_to_html};
use crate::plan::{Plan, PlannedStep};
use crate::{Export, OutFile};

// The page's colours. Each pair is checked against the 4.5 to 1 rule in the tests.
pub const LIGHT_BACKGROUND: &str = "#FFFFFF";
pub const LIGHT_TEXT: &str = "#1B1F33";
pub const LIGHT_MUTED: &str = "#4A5068";
pub const LIGHT_LINK: &str = "#2B3AB8";
pub const LIGHT_FOCUS: &str = "#C2410C";
pub const LIGHT_PANEL: &str = "#F1F3FA";
pub const DARK_BACKGROUND: &str = "#0B172A";
pub const DARK_TEXT: &str = "#E8ECFF";
pub const DARK_MUTED: &str = "#B4BCDB";
pub const DARK_LINK: &str = "#9CA5FF";
pub const DARK_FOCUS: &str = "#FFB454";
pub const DARK_PANEL: &str = "#102037";

/// The few words the page says itself, in the guide's language.
struct Words {
    skip: &'static str,
    steps: &'static str,
    close: &'static str,
    opens_larger: &'static str,
    viewer: &'static str,
}

fn words(language: Language) -> Words {
    match language {
        Language::English => Words {
            skip: "Skip to the steps",
            steps: "Steps",
            close: "Close",
            opens_larger: "Opens a larger view of the picture",
            viewer: "Larger view of the picture",
        },
        Language::German => Words {
            skip: "Zu den Schritten springen",
            steps: "Schritte",
            close: "Schließen",
            opens_larger: "Öffnet eine größere Ansicht des Bildes",
            viewer: "Größere Ansicht des Bildes",
        },
        Language::French => Words {
            skip: "Passer aux étapes",
            steps: "Étapes",
            close: "Fermer",
            opens_larger: "Ouvre une vue agrandie de l'image",
            viewer: "Vue agrandie de l'image",
        },
    }
}

/// The page's one script: opens a picture in the dialog and puts focus back where it was when the dialog closes.
pub const SCRIPT: &str = "(function(){var d=document.getElementById('viewer');if(!d||!d.showModal){return;}\
var img=document.getElementById('viewer-img'),last=null;\
document.addEventListener('click',function(e){var b=e.target.closest('button.zoom');if(!b){return;}\
last=b;var s=b.querySelector('img');img.src=s.src;img.alt=s.alt;d.showModal();});\
d.addEventListener('close',function(){if(last){last.focus();}});\
d.addEventListener('click',function(e){if(e.target===d){d.close();}});})();";

fn style() -> String {
    format!(
        "*{{box-sizing:border-box}}\
:root{{color-scheme:light dark;--bg:{LIGHT_BACKGROUND};--text:{LIGHT_TEXT};--muted:{LIGHT_MUTED};--link:{LIGHT_LINK};--focus:{LIGHT_FOCUS};--panel:{LIGHT_PANEL}}}\
@media (prefers-color-scheme:dark){{:root{{--bg:{DARK_BACKGROUND};--text:{DARK_TEXT};--muted:{DARK_MUTED};--link:{DARK_LINK};--focus:{DARK_FOCUS};--panel:{DARK_PANEL}}}}}\
html{{font-size:100%}}\
body{{margin:0;background:var(--bg);color:var(--text);font-family:system-ui,-apple-system,'Segoe UI',Roboto,Arial,sans-serif;line-height:1.55}}\
main{{max-width:60rem;margin:0 auto;padding:1rem}}\
h1{{font-size:2rem;line-height:1.2;margin:1rem 0}}\
h2,h3,h4,h5,h6{{line-height:1.3;margin:0 0 .5rem}}\
h2{{font-size:1.4rem}}h3{{font-size:1.2rem}}h4,h5,h6{{font-size:1.05rem}}\
a{{color:var(--link)}}\
a:focus-visible,button:focus-visible{{outline:3px solid var(--focus);outline-offset:3px}}\
.skip{{position:absolute;left:-999px;top:0;background:var(--panel);padding:.5rem 1rem;z-index:2}}\
.skip:focus{{left:0}}\
nav.toc{{background:var(--panel);border-radius:.5rem;padding:.5rem 1rem;margin:1rem 0 2rem}}\
ol{{padding-left:1.25rem}}\
nav.toc ol{{margin:.25rem 0;list-style:none;padding-left:0}}nav.toc ol ol{{padding-left:1.25rem}}\
ol.steps,ol.steps ol{{list-style:none;padding-left:0}}\
ol.steps ol{{padding-left:1.25rem}}\
li.step{{margin:0 0 2rem}}\
.num{{color:var(--muted);margin-right:.35rem}}\
figure{{margin:1rem 0}}\
button.zoom{{display:block;width:100%;padding:0;border:1px solid var(--muted);border-radius:.4rem;background:none;cursor:zoom-in}}\
button.zoom img{{display:block;width:100%;height:auto;border-radius:.35rem}}\
.sr{{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}}\
aside.block{{background:var(--panel);border-left:.3rem solid var(--link);padding:.5rem 1rem;border-radius:.3rem}}\
.block-title{{font-weight:700;margin:.25rem 0}}\
dialog{{max-width:95vw;max-height:95vh;padding:1rem;border:1px solid var(--muted);background:var(--bg);color:var(--text)}}\
dialog img{{max-width:100%;max-height:80vh;height:auto}}\
dialog button{{font:inherit;padding:.4rem 1rem;margin-bottom:.5rem}}\
@media (prefers-reduced-motion:reduce){{*{{scroll-behavior:auto;transition:none;animation:none}}}}\
@media print{{.skip,nav.toc,dialog,.sr{{display:none}}li.step,figure{{break-inside:avoid}}button.zoom{{border:0}}}}"
    )
}

/// The nested ordered list of links to the steps (content blocks are not in it), shared with the PDF's print page. Empty
/// when the plan has no steps to list.
pub(crate) fn toc_list(plan: &Plan) -> String {
    let listed: Vec<&PlannedStep> = plan.steps.iter().filter(|s| s.kind != StepKind::Block).collect();
    let mut out = String::new();
    if listed.is_empty() {
        return out;
    }
    let mut level: Option<usize> = None;
    for step in &listed {
        let depth = match level {
            None => 0,
            Some(l) => step.depth.min(l + 1),
        };
        match level {
            None => out.push_str("<ol>\n"),
            Some(l) if depth > l => out.push_str("\n<ol>\n"),
            Some(mut l) => {
                out.push_str("</li>\n");
                while l > depth {
                    out.push_str("</ol>\n</li>\n");
                    l -= 1;
                }
            }
        }
        level = Some(depth);
        let number = step.number.as_deref().unwrap_or("");
        out.push_str(&format!(
            "<li><a href=\"#{}\">{} {}</a>",
            html_escape(&step.id),
            html_escape(number),
            html_escape(&step.title)
        ));
    }
    out.push_str("</li>\n");
    for _ in 0..level.unwrap_or(0) {
        out.push_str("</ol>\n</li>\n");
    }
    out.push_str("</ol>\n");
    out
}

/// Make the page for a plan.
pub fn html_page(plan: &Plan) -> String {
    let language = Language::from_code(&plan.language);
    let w = words(language);
    let fallback_lang =
        if matches!(plan.language.to_ascii_lowercase().split(['-', '_']).next(), Some("en" | "de" | "fr")) {
            String::new()
        } else {
            format!(" lang=\"{}\"", language.code())
        };
    let script_hash = csp_script_hash(SCRIPT);
    let csp = format!("default-src 'none'; img-src data:; style-src 'unsafe-inline'; script-src {script_hash}; base-uri 'none'; form-action 'none'");
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n");
    out.push_str(&format!("<html lang=\"{}\">\n<head>\n<meta charset=\"utf-8\">\n", html_escape(&plan.language)));
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str(&format!("<meta http-equiv=\"Content-Security-Policy\" content=\"{}\">\n", html_escape(&csp)));
    out.push_str(&format!(
        "<title>{}</title>\n<style>{}</style>\n</head>\n<body>\n",
        html_escape(&plan.title),
        style()
    ));
    out.push_str(&format!("<a class=\"skip\" href=\"#steps\"{fallback_lang}>{}</a>\n<main>\n", html_escape(w.skip)));
    out.push_str(&format!("<h1>{}</h1>\n", html_escape(&plan.title)));
    out.push_str(&markdown_to_html(&plan.description));

    // The list of steps (content blocks are not in it).
    let toc = toc_list(plan);
    if !toc.is_empty() {
        out.push_str(&format!("<nav class=\"toc\" aria-label=\"{}\"{fallback_lang}>\n", html_escape(w.steps)));
        out.push_str(&toc);
        out.push_str("</nav>\n");
    }

    // The steps.
    out.push_str("<ol class=\"steps\" id=\"steps\">\n");
    let mut level: Option<usize> = None;
    for step in &plan.steps {
        let depth = match level {
            None => 0,
            Some(l) => step.depth.min(l + 1),
        };
        match level {
            None => {}
            Some(l) if depth > l => out.push_str("<ol>\n"),
            Some(mut l) => {
                out.push_str("</li>\n");
                while l > depth {
                    out.push_str("</ol>\n</li>\n");
                    l -= 1;
                }
            }
        }
        level = Some(depth);
        out.push_str(&format!("<li class=\"step\" id=\"{}\">\n", html_escape(&step.id)));
        if step.kind == StepKind::Block {
            out.push_str(&format!(
                "<aside class=\"block\"><p class=\"block-title\">{}</p>\n{}</aside>\n",
                html_escape(&step.title),
                markdown_to_html(&step.description)
            ));
            continue;
        }
        let heading = (depth + 2).min(6);
        out.push_str(&format!(
            "<h{heading}><span class=\"num\">{}.</span> {}</h{heading}>\n",
            html_escape(step.number.as_deref().unwrap_or("")),
            html_escape(&step.title)
        ));
        out.push_str(&markdown_to_html(&step.description));
        if let Some(picture) = &step.picture {
            out.push_str(&format!(
                "<figure><button type=\"button\" class=\"zoom\" aria-haspopup=\"dialog\"><img src=\"data:image/png;base64,{}\" alt=\"{}\" width=\"{}\" height=\"{}\"><span class=\"sr\"{fallback_lang}>{}</span></button></figure>\n",
                base64_encode(&picture.png),
                html_escape(&step.alt),
                picture.width,
                picture.height,
                html_escape(w.opens_larger)
            ));
        }
    }
    if level.is_some() {
        out.push_str("</li>\n");
        for _ in 0..level.unwrap_or(0) {
            out.push_str("</ol>\n</li>\n");
        }
    }
    out.push_str("</ol>\n</main>\n");
    out.push_str(&format!(
        "<dialog id=\"viewer\" aria-label=\"{}\"{fallback_lang}><form method=\"dialog\"><button>{}</button></form><img id=\"viewer-img\" alt=\"\"></dialog>\n",
        html_escape(w.viewer),
        html_escape(w.close)
    ));
    out.push_str(&format!("<script>{SCRIPT}</script>\n</body>\n</html>\n"));
    out
}

/// The HTML export: one file named after the guide.
pub fn export_html(plan: &Plan) -> Export {
    Export {
        files: vec![OutFile {
            path: format!("{}.html", sanitise_folder_name(&plan.title)),
            bytes: html_page(plan).into_bytes(),
        }],
        warnings: plan.warnings.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo;
    use crate::plan::{plan, ExportOptions};
    use openstage_capture_render::BlockText;

    fn demo_page() -> (Plan, String) {
        let d = demo();
        let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let page = html_page(&p);
        (p, page)
    }

    /// The relative luminance of `#RRGGBB`, as the accessibility rules define it.
    fn luminance(hex: &str) -> f64 {
        let channel = |i: usize| {
            let v = u8::from_str_radix(&hex[1 + i * 2..3 + i * 2], 16).unwrap() as f64 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(0) + 0.7152 * channel(1) + 0.0722 * channel(2)
    }

    fn contrast(a: &str, b: &str) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn every_text_colour_has_enough_contrast_in_both_schemes() {
        for (bg, panel, text, muted, link, focus) in [
            (LIGHT_BACKGROUND, LIGHT_PANEL, LIGHT_TEXT, LIGHT_MUTED, LIGHT_LINK, LIGHT_FOCUS),
            (DARK_BACKGROUND, DARK_PANEL, DARK_TEXT, DARK_MUTED, DARK_LINK, DARK_FOCUS),
        ] {
            for (name, colour) in [("text", text), ("muted", muted), ("link", link)] {
                for (backdrop_name, backdrop) in [("page", bg), ("panel", panel)] {
                    let ratio = contrast(colour, backdrop);
                    assert!(ratio >= 4.5, "{name} on the {backdrop_name}: {ratio:.2} to 1 ({colour} on {backdrop})");
                }
            }
            for (backdrop_name, backdrop) in [("page", bg), ("panel", panel)] {
                let ratio = contrast(focus, backdrop);
                assert!(ratio >= 3.0, "the focus ring on the {backdrop_name}: {ratio:.2} to 1");
            }
        }
    }

    #[test]
    fn the_page_has_the_structure_a_screen_reader_needs() {
        let (_, page) = demo_page();
        assert!(page.starts_with("<!DOCTYPE html>\n<html lang=\"en\">"));
        assert_eq!(page.matches("<h1>").count(), 1);
        assert_eq!(page.matches("<main>").count(), 1);
        assert!(page.contains("<a class=\"skip\" href=\"#steps\">Skip to the steps</a>"));
        assert!(page.contains("<nav class=\"toc\" aria-label=\"Steps\">"));
        assert!(page.contains("<ol class=\"steps\" id=\"steps\">"));
        assert!(page.contains("<title>How to submit an assignment</title>"));
        assert!(page.contains("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">"));
        assert!(page.contains("<dialog id=\"viewer\" aria-label=\"Larger view of the picture\">"));
    }

    #[test]
    fn headings_never_skip_a_level() {
        let (_, page) = demo_page();
        let levels: Vec<u32> = page
            .match_indices("<h")
            .filter_map(|(i, _)| page[i + 2..].chars().next().and_then(|c| c.to_digit(10)))
            .collect();
        assert_eq!(levels[0], 1);
        assert!(levels.windows(2).all(|w| w[1] <= w[0] + 1), "{levels:?}");
        assert!(levels.contains(&2) && levels.contains(&3), "the substep is a level deeper: {levels:?}");
    }

    #[test]
    fn every_picture_has_alt_text_and_the_page_contains_all_of_them() {
        let (p, page) = demo_page();
        let images = page.matches("<img ").count();
        let with_alt = page.matches("<img src=\"data:image/png;base64,").count();
        assert_eq!(images, with_alt + 1, "the steps' pictures, and the empty one in the dialog");
        assert_eq!(page.matches(" alt=\"").count(), images, "every img has an alt attribute");
        assert!(page.contains("alt=\"The Submit button, with the student&#39;s name blurred\""));
        assert_eq!(with_alt, p.steps.iter().filter(|s| s.picture.is_some()).count());
    }

    #[test]
    fn step_numbers_are_real_text_and_the_contents_list_links_to_every_step() {
        let (p, page) = demo_page();
        assert!(page.contains("<span class=\"num\">1.</span> Click &quot;Assignments&quot;"));
        assert!(page.contains("<span class=\"num\">2.1.</span> Type &quot;Week 3 reflection&quot;"));
        for step in p.steps.iter().filter(|s| s.kind != StepKind::Block) {
            assert!(page.contains(&format!("<a href=\"#{}\">", step.id)), "{}", step.id);
            assert!(page.contains(&format!("<li class=\"step\" id=\"{}\">", step.id)));
        }
        let block = p.steps.iter().find(|s| s.kind == StepKind::Block).unwrap();
        assert!(!page.contains(&format!("href=\"#{}\"", block.id)), "a content block is not in the contents list");
        assert!(page.contains("<aside class=\"block\">"));
    }

    #[test]
    fn the_lists_are_properly_nested() {
        let (_, page) = demo_page();
        assert_eq!(page.matches("<ol").count(), page.matches("</ol>").count());
        assert_eq!(page.matches("<li").count(), page.matches("</li>").count());
        assert_eq!(page.matches("<ul>").count(), page.matches("</ul>").count());
        assert_eq!(page.matches("<figure>").count(), page.matches("</figure>").count());
        assert_eq!(page.matches("<button").count(), page.matches("</button>").count());
    }

    #[test]
    fn the_page_makes_no_outside_requests_and_the_security_rule_matches_its_script() {
        let (_, page) = demo_page();
        for forbidden in [
            "src=\"http",
            "<link ",
            "@import",
            "url(",
            "<script src",
            "<iframe",
            "<object",
            "<embed",
            "srcset",
            "<form action",
        ] {
            assert!(!page.contains(forbidden), "{forbidden}");
        }
        let start = page.find("<script>").unwrap() + "<script>".len();
        let end = page.find("</script>").unwrap();
        assert_eq!(&page[start..end], SCRIPT);
        assert_eq!(page.matches("<script").count(), 1);
        assert!(page.contains(&csp_script_hash(SCRIPT).replace('\'', "&#39;")), "the policy names the script's hash");
        assert!(page.contains("default-src &#39;none&#39;; img-src data:;"));
        for handler in [" onclick=", " onload=", " onerror=", "javascript:"] {
            assert!(!page.contains(handler), "{handler}");
        }
    }

    #[test]
    fn the_authors_words_are_escaped_and_links_are_safe() {
        let mut d = demo();
        d.document.guide.title = "A <b>bold</b> & \"quoted\" title".to_string();
        d.document.guide.steps[0].title = "<img src=x onerror=alert(1)>".to_string();
        d.document.guide.steps[0].description =
            "[bad](javascript:alert(1)) and <script>alert(2)</script> and [good](https://example.com)".to_string();
        d.document.guide.description = "<i>intro</i> & more".to_string();
        d.document.guide.steps[1].image_description = "x\" onerror=\"alert(3)".to_string();
        let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let page = html_page(&p);
        assert!(page.contains("<title>A &lt;b&gt;bold&lt;/b&gt; &amp; &quot;quoted&quot; title</title>"));
        assert!(page.contains("<h1>A &lt;b&gt;bold&lt;/b&gt; &amp; &quot;quoted&quot; title</h1>"), "the heading too");
        assert!(!page.contains("<h1>A <b>") && !page.contains("<i>intro</i>"));
        assert!(page.contains("&lt;i&gt;intro&lt;/i&gt; &amp; more"), "the guide's description");
        assert!(!page.contains("\" onerror=\"alert(3)"), "alt text cannot break out of its attribute");
        assert!(page.contains("alt=\"x&quot; onerror=&quot;alert(3)\""));
        assert!(!page.contains("<img src=x"));
        assert!(page.contains("&lt;img src=x onerror=alert(1)&gt;"));
        assert!(!page.contains("<script>alert"));
        assert!(!page.contains("href=\"javascript:"));
        assert!(page.contains("<a href=\"https://example.com\" rel=\"noopener noreferrer\">good</a>"));
        assert_eq!(page.matches("<script").count(), 1);
    }

    #[test]
    fn descriptions_keep_their_lists_and_emphasis() {
        let (_, page) = demo_page();
        assert!(page
            .contains("<ul>\n<li>Use words your teacher will recognise</li>\n<li>You can change it later</li>\n</ul>"));
        assert!(page.contains("<strong>Assignments</strong>"));
        assert!(page.contains("<a href=\"https://example.com/help\" rel=\"noopener noreferrer\">read the help</a>"));
    }

    #[test]
    fn pictures_open_larger_in_a_dialog_that_closes_by_keyboard() {
        let (_, page) = demo_page();
        assert!(page.contains("<button type=\"button\" class=\"zoom\" aria-haspopup=\"dialog\">"));
        assert!(page.contains("<span class=\"sr\">Opens a larger view of the picture</span>"));
        assert!(SCRIPT.contains("showModal") && SCRIPT.contains("last.focus()"), "focus goes back");
        assert!(
            page.contains("<form method=\"dialog\"><button>Close</button></form>"),
            "a close button, and Escape works natively"
        );
        assert!(!SCRIPT.contains("keydown") && !SCRIPT.contains("trap"), "no hand-made keyboard handling to get wrong");
    }

    #[test]
    fn the_style_has_the_accessibility_rules() {
        let css = style();
        for needed in [
            "prefers-color-scheme:dark",
            "prefers-reduced-motion:reduce",
            "@media print",
            "break-inside:avoid",
            ":focus-visible",
            "outline:3px",
        ] {
            assert!(css.contains(needed), "{needed}");
        }
        assert!(css.contains("width:100%;height:auto"), "pictures shrink to fit a 320 pixel wide screen");
        assert!(css.contains("html{font-size:100%}"), "text size follows the person's settings");
        assert!(
            css.contains("nav.toc ol{margin:.25rem 0;list-style:none"),
            "the numbers in the contents list are the text, not also list markers"
        );
    }

    #[test]
    fn german_and_french_pages_speak_their_own_language() {
        let mut d = demo();
        for (code, skip) in [("de", "Zu den Schritten springen"), ("fr", "Passer aux étapes")] {
            d.document.guide.language = code.to_string();
            let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
            let page = html_page(&p);
            assert!(page.contains(&format!("<html lang=\"{code}\">")));
            assert!(page.contains(skip), "{code}");
            assert!(!page.contains("Skip to the steps"));
        }
    }

    #[test]
    fn a_language_the_page_cannot_speak_still_sets_lang_and_marks_the_english_words() {
        let mut d = demo();
        d.document.guide.language = "ja".to_string();
        let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let page = html_page(&p);
        assert!(page.contains("<html lang=\"ja\">"));
        assert!(page.contains("<a class=\"skip\" href=\"#steps\" lang=\"en\">Skip to the steps</a>"));
    }

    #[test]
    fn an_empty_guide_still_makes_a_valid_page() {
        let guide = openstage_express_core::Guide::new("g", "Nothing yet", "en", "t");
        let p = plan(&guide, &crate::picture::MemoryPictures::default(), &BlockText, &ExportOptions::default());
        let page = html_page(&p);
        assert!(page.contains("<h1>Nothing yet</h1>"));
        assert!(!page.contains("<nav"));
        assert_eq!(page.matches("<ol").count(), page.matches("</ol>").count());
    }

    #[test]
    fn the_export_is_one_file_named_after_the_guide_and_is_repeatable() {
        let d = demo();
        let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let export = export_html(&p);
        assert_eq!(export.files.len(), 1);
        assert_eq!(export.files[0].path, "How to submit an assignment.html");
        assert_eq!(export, export_html(&p));
        assert!(export.files[0].bytes.len() > 20_000, "the pictures are inside");
    }
}
