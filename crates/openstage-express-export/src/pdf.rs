// SPDX-License-Identifier: Apache-2.0
//! The PDF export (spec section 10.2).
//!
//! The PDF is made by the Microsoft WebView2 engine that Windows ships: this module writes a *print page* (a web page made
//! for paper, from the same [`Plan`] as every other export) and hands it to a [`PdfRenderer`]. The renderer for Windows
//! lives in `openstage-express-windows`, so everything here runs and is tested on any computer, with a stand-in renderer.
//!
//! What the print page controls: the paper (A4, Letter, A3, A5 or Legal; portrait or landscape; margins), an optional cover
//! page, a contents list whose entries are links, a header and footer line, page numbers, and whether each step starts a new
//! page. A picture is never split across two pages, a heading is never left alone at the foot of one, and a step's text goes
//! with its picture. That is done with `break-after: avoid` chains, never with `break-inside: avoid` around a step:
//! WebView2 writes a heading's bookmark twice when that moves the block (see `style`). The fonts are Roboto, put into the
//! page as data.
//!
//! Accessibility is a goal, not a promise: the engine writes headings, lists, links, a document language, bookmarks and the
//! pictures' alt text into the PDF as tags. [`audit_pdf`] reads the finished file and [`PdfAudit::warnings`] says plainly what
//! is missing. The page numbers and the header and footer text are drawn in the page margins, and the engine does not tag
//! them, so a strict checker will report them; they can be turned off.

use openstage_express_core::fsutil::sanitise_folder_name;
use openstage_express_core::{Language, StepKind};

use crate::encoding::{base64_encode, html_escape, markdown_to_html};
use crate::html::{toc_list, LIGHT_LINK, LIGHT_MUTED, LIGHT_TEXT};
use crate::plan::Plan;
use crate::{Export, OutFile};

pub const MIN_MARGIN_MM: u32 = 10;
pub const MAX_MARGIN_MM: u32 = 40;

// Roboto 2.137 (Apache License 2.0, Copyright 2011 Google Inc.), put into every PDF so that it does not depend on the fonts
// installed on the computer that makes it, and so that no Windows font is copied into the file. It has no Chinese, Japanese,
// Korean or Arabic letters: those fall back to a Windows font. The licence is named in each file and in `NOTICE`.
const ROBOTO_REGULAR: &[u8] = include_bytes!("../fonts/Roboto-Regular.ttf");
const ROBOTO_BOLD: &[u8] = include_bytes!("../fonts/Roboto-Bold.ttf");
const ROBOTO_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-Italic.ttf");
const FONT_STACK: &str = "Roboto,'Segoe UI',Arial,Helvetica,sans-serif";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSize {
    A4,
    Letter,
    A3,
    A5,
    Legal,
}

impl PageSize {
    /// The name CSS uses for the paper.
    fn css(self) -> &'static str {
        match self {
            PageSize::A4 => "A4",
            PageSize::Letter => "letter",
            PageSize::A3 => "A3",
            PageSize::A5 => "A5",
            PageSize::Legal => "legal",
        }
    }

    /// Width and height in millimetres, portrait.
    fn portrait_mm(self) -> (f64, f64) {
        match self {
            PageSize::A4 => (210.0, 297.0),
            PageSize::Letter => (215.9, 279.4),
            PageSize::A3 => (297.0, 420.0),
            PageSize::A5 => (148.0, 210.0),
            PageSize::Legal => (215.9, 355.6),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// What the person chose in the PDF export dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfOptions {
    pub page_size: PageSize,
    pub orientation: Orientation,
    /// Margin on all four sides, kept between [`MIN_MARGIN_MM`] and [`MAX_MARGIN_MM`].
    pub margin_mm: u32,
    /// A first page holding only the title and the description.
    pub cover_page: bool,
    /// The list of steps, each a link to the step.
    pub contents: bool,
    /// "Page 3 of 12" at the bottom right of every page.
    pub page_numbers: bool,
    /// A line at the top of every page; empty for none.
    pub header_text: String,
    /// A line at the bottom left of every page; empty for none.
    pub footer_text: String,
    /// Every top-level step starts on a new page.
    pub new_page_per_step: bool,
}

impl Default for PdfOptions {
    fn default() -> Self {
        PdfOptions {
            page_size: PageSize::A4,
            orientation: Orientation::Portrait,
            margin_mm: 20,
            cover_page: false,
            contents: true,
            page_numbers: true,
            header_text: String::new(),
            footer_text: String::new(),
            new_page_per_step: false,
        }
    }
}

impl PdfOptions {
    fn margin(&self) -> u32 {
        self.margin_mm.clamp(MIN_MARGIN_MM, MAX_MARGIN_MM)
    }

    /// Paper width and height in millimetres after turning it to the chosen orientation.
    fn paper_mm(&self) -> (f64, f64) {
        let (w, h) = self.page_size.portrait_mm();
        match self.orientation {
            Orientation::Portrait => (w, h),
            Orientation::Landscape => (h, w),
        }
    }

    /// The tallest a picture may be: the page less its margins and room for a step's heading.
    fn picture_max_height_mm(&self) -> u32 {
        let (_, h) = self.paper_mm();
        ((h - 2.0 * f64::from(self.margin()) - 40.0).max(40.0)).floor() as u32
    }
}

/// The few words the print page says itself, in the guide's language.
struct Words {
    steps: &'static str,
    contents: &'static str,
    page: &'static str,
    of: &'static str,
}

fn words(language: Language) -> Words {
    match language {
        Language::English => Words { steps: "Steps", contents: "Contents", page: "Page", of: "of" },
        Language::German => Words { steps: "Schritte", contents: "Inhalt", page: "Seite", of: "von" },
        Language::French => Words { steps: "Étapes", contents: "Sommaire", page: "Page", of: "sur" },
    }
}

/// `text` as a quoted CSS string that cannot end the string, the rule or the `<style>` element early.
fn css_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\22 "),
            '\\' => out.push_str("\\5C "),
            '<' => out.push_str("\\3C "),
            '>' => out.push_str("\\3E "),
            '&' => out.push_str("\\26 "),
            '\n' | '\r' | '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The three Roboto faces, inside the page as data so that nothing is looked up or installed.
fn font_faces() -> String {
    let face = |weight: u32, style: &str, bytes: &[u8]| {
        format!(
            "@font-face{{font-family:Roboto;font-weight:{weight};font-style:{style};src:url(data:font/ttf;base64,{}) format(\"truetype\")}}",
            base64_encode(bytes)
        )
    };
    format!(
        "{}{}{}",
        face(400, "normal", ROBOTO_REGULAR),
        face(700, "normal", ROBOTO_BOLD),
        face(400, "italic", ROBOTO_ITALIC)
    )
}

fn style(options: &PdfOptions, w: &Words) -> String {
    let orientation = match options.orientation {
        Orientation::Portrait => "portrait",
        Orientation::Landscape => "landscape",
    };
    let margin_box = format!("font:9pt {FONT_STACK};color:{LIGHT_MUTED}");
    let mut css = format!("@page{{size:{} {orientation};margin:{}mm;", options.page_size.css(), options.margin());
    if !options.header_text.trim().is_empty() {
        css.push_str(&format!("@top-center{{content:{};{margin_box}}}", css_string(options.header_text.trim())));
    }
    if !options.footer_text.trim().is_empty() {
        css.push_str(&format!("@bottom-left{{content:{};{margin_box}}}", css_string(options.footer_text.trim())));
    }
    if options.page_numbers {
        let content = format!(
            "{} counter(page) {} counter(pages)",
            css_string(&format!("{} ", w.page)),
            css_string(&format!(" {} ", w.of))
        );
        css.push_str(&format!("@bottom-right{{content:{content};{margin_box}}}"));
    }
    css.push('}');
    css.push_str(&font_faces());
    css.push_str(&format!(
        "*{{box-sizing:border-box}}\
html{{font-size:11pt}}\
body{{margin:0;background:#FFFFFF;color:{LIGHT_TEXT};font-family:{FONT_STACK};line-height:1.5}}\
h1{{font-size:24pt;line-height:1.2;margin:0 0 .5em}}\
h2,h3,h4,h5,h6{{line-height:1.3;margin:0 0 .4em;break-after:avoid}}\
h2{{font-size:15pt}}h3{{font-size:13pt}}h4,h5,h6{{font-size:11.5pt}}\
a{{color:{LIGHT_LINK}}}\
p{{margin:0 0 .6em;orphans:3;widows:3}}\
ol{{margin:0;padding-left:0;list-style:none}}\
ol ol{{padding-left:1.2em}}\
nav.toc{{margin:0 0 1.5em}}\
.num{{color:{LIGHT_MUTED};margin-right:.3em}}\
li.step{{margin:0 0 1.2em}}\
*:has(+figure){{break-after:avoid}}\
figure{{margin:.6em 0;break-inside:avoid}}\
figure img{{display:block;width:auto;height:auto;max-width:100%;max-height:{}mm;border:1px solid {LIGHT_MUTED}}}\
aside.block{{border-left:3pt solid {LIGHT_LINK};padding:.3em .8em;break-inside:avoid}}\
.block-title{{font-weight:700;margin:0 0 .3em}}",
        options.picture_max_height_mm()
    ));
    if options.cover_page {
        css.push_str(".cover{break-after:page;padding-top:30mm}.cover h1{font-size:32pt}");
    }
    // Careful with page-break rules around headings. On 2026-11-01 WebView2 156 wrote some headings' bookmarks twice
    // ("2.2. Click Save Click Save") from a page that used Segoe UI and put `break-inside: avoid` on a block holding each
    // heading, and the Edge command-line printer did the same after any forced break. The cause is not understood: the same
    // page with Roboto does not do it, and WebView2 handles forced breaks correctly. So no block that holds a heading may
    // avoid breaking, steps are kept with their pictures by `break-after: avoid` chains instead, and
    // `openstage-express-windows` prints every layout option with the real engine and checks the bookmarks, so a runtime
    // that brings the fault back is noticed.
    if options.new_page_per_step {
        css.push_str("ol.steps>li.step{break-before:page}");
    }
    css
}

/// The web page the engine prints: the guide made for paper. No script, no outside request of any kind.
pub fn print_page(plan: &Plan, options: &PdfOptions) -> String {
    let language = Language::from_code(&plan.language);
    let w = words(language);
    let fallback_lang =
        if matches!(plan.language.to_ascii_lowercase().split(['-', '_']).next(), Some("en" | "de" | "fr")) {
            String::new()
        } else {
            format!(" lang=\"{}\"", language.code())
        };
    let csp = "default-src 'none'; img-src data:; font-src data:; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'";
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n");
    out.push_str(&format!("<html lang=\"{}\">\n<head>\n<meta charset=\"utf-8\">\n", html_escape(&plan.language)));
    out.push_str(&format!("<meta http-equiv=\"Content-Security-Policy\" content=\"{}\">\n", html_escape(csp)));
    out.push_str(&format!(
        "<title>{}</title>\n<style>{}</style>\n</head>\n<body>\n<main>\n",
        html_escape(&plan.title),
        style(options, &w)
    ));

    // The title and description are the cover page, or the top of the first page. The contents list follows.
    let title_block = format!("<h1>{}</h1>\n{}", html_escape(&plan.title), markdown_to_html(&plan.description));
    if options.cover_page {
        out.push_str(&format!("<section class=\"cover\">\n{title_block}</section>\n"));
    } else {
        out.push_str(&title_block);
    }
    if options.contents {
        let toc = toc_list(plan);
        if !toc.is_empty() {
            out.push_str(&format!(
                "<nav class=\"toc\" aria-label=\"{}\"{fallback_lang}>\n<h2>{}</h2>\n{toc}</nav>\n",
                html_escape(w.steps),
                html_escape(w.contents)
            ));
        }
    }

    out.push_str("<ol class=\"steps\">\n");
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
                "<figure><img src=\"data:image/png;base64,{}\" alt=\"{}\" width=\"{}\" height=\"{}\"></figure>\n",
                base64_encode(&picture.png),
                html_escape(&step.alt),
                picture.width,
                picture.height
            ));
        }
    }
    if level.is_some() {
        out.push_str("</li>\n");
        for _ in 0..level.unwrap_or(0) {
            out.push_str("</ol>\n</li>\n");
        }
    }
    out.push_str("</ol>\n</main>\n</body>\n</html>\n");
    out
}

/// A finished print page, handed to whatever turns web pages into PDF.
#[derive(Debug, Clone, Copy)]
pub struct PrintPage<'a> {
    pub html: &'a str,
    pub title: &'a str,
    pub language: &'a str,
}

/// Why a PDF could not be made. The text is written for the person.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    /// The engine that makes PDFs is not on this computer.
    #[error("{0}")]
    Unavailable(String),
    /// The engine was there but did not give back a PDF.
    #[error("{0}")]
    Failed(String),
}

/// Anything that can turn a print page into the bytes of a PDF file. On Windows it is the WebView2 engine; tests use a
/// stand-in.
pub trait PdfRenderer {
    fn render(&self, page: &PrintPage<'_>) -> Result<Vec<u8>, RenderError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PdfError {
    #[error("{0}")]
    Render(#[from] RenderError),
    #[error("The PDF engine did not give back a PDF file, so nothing was exported.")]
    NotAPdf,
}

/// What a finished PDF holds, found by reading its bytes. It looks only for the engine's plain (uncompressed) markers, so a
/// PDF written in another way reads as "not tagged"; that is why [`PdfAudit::warnings`] words it as "could not find".
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PdfAudit {
    pub is_pdf: bool,
    pub pages: usize,
    /// A structure tree and the "this PDF is tagged" mark.
    pub tagged: bool,
    pub language: bool,
    pub bookmarks: bool,
    pub title_shown: bool,
    pub headings: usize,
    pub figures: usize,
    pub alt_texts: usize,
}

fn count(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || haystack.len() < needle.len() {
        return 0;
    }
    haystack.windows(needle.len()).filter(|w| *w == needle).count()
}

pub fn audit_pdf(bytes: &[u8]) -> PdfAudit {
    if !bytes.starts_with(b"%PDF-") {
        return PdfAudit::default();
    }
    // Whitespace between tokens is free in a PDF, so compare with it taken out.
    let compact: Vec<u8> = bytes.iter().copied().filter(|b| !b.is_ascii_whitespace()).collect();
    let has = |needle: &[u8]| count(&compact, needle) > 0;
    PdfAudit {
        is_pdf: true,
        pages: count(&compact, b"/Type/Page").saturating_sub(count(&compact, b"/Type/Pages")),
        tagged: has(b"/StructTreeRoot") && has(b"/Markedtrue"),
        language: has(b"/Lang(") || has(b"/Lang<"),
        bookmarks: has(b"/Outlines"),
        title_shown: has(b"/DisplayDocTitletrue"),
        headings: (1..=6).map(|n| count(&compact, format!("/S/H{n}").as_bytes())).sum(),
        figures: count(&compact, b"/S/Figure"),
        alt_texts: count(&compact, b"/Alt(") + count(&compact, b"/Alt<"),
    }
}

impl PdfAudit {
    /// What the person should be told about the PDF, for a guide with `pictures` pictures made with `options`.
    pub fn warnings(&self, pictures: usize, options: &PdfOptions) -> Vec<String> {
        let mut warnings = Vec::new();
        if !self.tagged {
            warnings.push(
                "The PDF has no tags that could be found, so a screen reader cannot read its headings, lists or picture \
descriptions in order. Export the web page as well: it is the accessible copy."
                    .to_string(),
            );
            return warnings;
        }
        if !self.language {
            warnings.push(
                "The PDF does not say which language it is in, so a screen reader may read it in the wrong voice."
                    .to_string(),
            );
        }
        if self.alt_texts < pictures {
            warnings.push(format!(
                "Only {} of the {pictures} pictures in the PDF carry a description for a screen reader.",
                self.alt_texts.min(pictures)
            ));
        }
        if !self.bookmarks {
            warnings.push("The PDF has no bookmarks, so it has no outline to jump around by.".to_string());
        }
        if options.page_numbers || !options.header_text.trim().is_empty() || !options.footer_text.trim().is_empty() {
            warnings.push(
                "The page numbers and the header and footer lines are printed in the page margins and are not tagged, \
so a strict accessibility checker will report them. Turn them off if this PDF must pass such a check."
                    .to_string(),
            );
        }
        warnings
    }
}

/// The PDF export: one file named after the guide, made by `renderer`.
pub fn export_pdf(plan: &Plan, options: &PdfOptions, renderer: &dyn PdfRenderer) -> Result<Export, PdfError> {
    let html = print_page(plan, options);
    let bytes = renderer.render(&PrintPage { html: &html, title: &plan.title, language: &plan.language })?;
    let audit = audit_pdf(&bytes);
    if !audit.is_pdf {
        return Err(PdfError::NotAPdf);
    }
    let pictures = plan.steps.iter().filter(|s| s.picture.is_some()).count();
    let mut warnings = plan.warnings.clone();
    warnings.extend(audit.warnings(pictures, options));
    Ok(Export { files: vec![OutFile { path: format!("{}.pdf", sanitise_folder_name(&plan.title)), bytes }], warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo;
    use crate::plan::{plan, ExportOptions};
    use openstage_capture_render::BlockText;
    use std::cell::RefCell;

    fn demo_plan() -> Plan {
        let d = demo();
        plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default())
    }

    fn page(options: &PdfOptions) -> String {
        print_page(&demo_plan(), options)
    }

    const TAGGED_SAMPLE: &str =
        "%PDF-1.4\n1 0 obj\n<</Type /Catalog /Lang (en) /MarkInfo <</Type /MarkInfo /Marked true>>\n\
/StructTreeRoot 5 0 R /Outlines 6 0 R /ViewerPreferences <</DisplayDocTitle true>> /Pages 2 0 R>>\nendobj\n\
2 0 obj\n<</Type /Pages /Count 2>>\nendobj\n3 0 obj\n<</Type /Page>>\nendobj\n4 0 obj\n<</Type /Page>>\nendobj\n\
7 0 obj\n<</S /H1>>\nendobj\n8 0 obj\n<</S /H2>>\nendobj\n9 0 obj\n<</S /H3>>\nendobj\n\
10 0 obj\n<</S /Figure /Alt (A picture)>>\nendobj\n11 0 obj\n<</S/Figure/Alt<FEFF0041>>>\nendobj\n%%EOF\n";

    struct Fake {
        seen: RefCell<Option<(String, String, String)>>,
        reply: Result<Vec<u8>, RenderError>,
    }

    impl Fake {
        fn replying(reply: Result<Vec<u8>, RenderError>) -> Fake {
            Fake { seen: RefCell::new(None), reply }
        }
    }

    impl PdfRenderer for Fake {
        fn render(&self, page: &PrintPage<'_>) -> Result<Vec<u8>, RenderError> {
            *self.seen.borrow_mut() = Some((page.html.to_string(), page.title.to_string(), page.language.to_string()));
            self.reply.clone()
        }
    }

    #[test]
    fn the_print_page_has_the_structure_a_screen_reader_needs() {
        let p = page(&PdfOptions::default());
        assert!(p.starts_with("<!DOCTYPE html>\n<html lang=\"en\">"));
        assert_eq!(p.matches("<h1>").count(), 1);
        assert_eq!(p.matches("<main>").count(), 1);
        assert!(p.contains("<title>How to submit an assignment</title>"));
        assert!(p.contains("<nav class=\"toc\" aria-label=\"Steps\">\n<h2>Contents</h2>\n<ol>"));
        assert!(p.contains("<ol class=\"steps\">"));
        assert!(p.contains("<span class=\"num\">1.</span> Click &quot;Assignments&quot;"));
        assert!(p.contains("<span class=\"num\">2.1.</span> Type &quot;Week 3 reflection&quot;"));
    }

    #[test]
    fn headings_never_skip_a_level_and_the_substep_is_a_level_deeper() {
        let p = page(&PdfOptions::default());
        let levels: Vec<u32> =
            p.match_indices("<h").filter_map(|(i, _)| p[i + 2..].chars().next().and_then(|c| c.to_digit(10))).collect();
        assert_eq!(levels[0], 1);
        assert!(levels.windows(2).all(|w| w[1] <= w[0] + 1), "{levels:?}");
        assert!(levels.contains(&3), "{levels:?}");
    }

    #[test]
    fn every_picture_has_alt_text_and_the_lists_are_nested_properly() {
        let plan = demo_plan();
        let p = print_page(&plan, &PdfOptions::default());
        let pictures = plan.steps.iter().filter(|s| s.picture.is_some()).count();
        assert_eq!(p.matches("<img ").count(), pictures);
        assert_eq!(p.matches(" alt=\"").count(), pictures);
        assert!(p.contains("alt=\"The Submit button, with the student&#39;s name blurred\""));
        assert_eq!(p.matches("<ol").count(), p.matches("</ol>").count());
        assert_eq!(p.matches("<li").count(), p.matches("</li>").count());
        assert_eq!(p.matches("<ul>").count(), p.matches("</ul>").count());
        assert_eq!(p.matches("<figure>").count(), p.matches("</figure>").count());
        assert_eq!(p.matches("<div").count(), p.matches("</div>").count());
        assert!(p.contains("<aside class=\"block\">"));
    }

    #[test]
    fn the_print_page_has_no_script_and_makes_no_outside_request() {
        let p = page(&PdfOptions::default());
        for forbidden in [
            "<script",
            "<link ",
            "@import",
            "src=\"http",
            "<iframe",
            "<object",
            "<embed",
            "srcset",
            "<form",
            "<button",
            "<dialog",
            " onclick=",
            " onload=",
            " onerror=",
            "javascript:",
            "http://",
        ] {
            assert!(!p.contains(forbidden), "{forbidden}");
        }
        // The author's own links stay links in the PDF; nothing else may name an address.
        for (at, _) in p.match_indices("https://") {
            assert!(
                p[..at].ends_with("<a href=\""),
                "an address that is not a link: {}",
                &p[at.saturating_sub(30)..at + 30]
            );
        }
        assert_eq!(p.matches("url(").count(), p.matches("url(data:").count(), "the only url() is the fonts' own data");
        assert!(
            p.contains("default-src &#39;none&#39;; img-src data:; font-src data:; style-src &#39;unsafe-inline&#39;;")
        );
        assert!(!p.contains("script-src"), "scripts fall under default-src none");
    }

    #[test]
    fn roboto_is_inside_the_page_and_carries_its_licence() {
        for (name, bytes) in [("Regular", ROBOTO_REGULAR), ("Bold", ROBOTO_BOLD), ("Italic", ROBOTO_ITALIC)] {
            assert_eq!(&bytes[..4], &[0, 1, 0, 0], "{name} is a TrueType file");
            assert!((100_000..400_000).contains(&bytes.len()), "{name}: {} bytes", bytes.len());
            // The name table holds text as Mac Roman (plain bytes) or as Windows UTF-16 (a zero byte before each letter).
            let licence = "Apache License, Version 2.0";
            let plain = licence.as_bytes().to_vec();
            let utf16: Vec<u8> = licence.encode_utf16().flat_map(u16::to_be_bytes).collect();
            let named = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
            assert!(named(&plain) || named(&utf16), "{name} names its licence in its own name table");
        }
        let p = page(&PdfOptions::default());
        assert_eq!(p.matches("@font-face{font-family:Roboto;").count(), 3);
        assert_eq!(p.matches("url(data:font/ttf;base64,").count(), 3);
        assert!(p.contains("font-weight:700;font-style:normal") && p.contains("font-weight:400;font-style:italic"));
        assert!(
            p.contains(&format!("font-family:{FONT_STACK};line-height")),
            "Roboto first, a Windows font only as a fallback"
        );
        assert!(p.contains(&format!(
            "@bottom-right{{content:\"Page \" counter(page) \" of \" counter(pages);font:9pt {FONT_STACK}"
        )));
    }

    #[test]
    fn no_block_that_holds_a_heading_is_kept_together() {
        // WebView2 156 writes a heading's bookmark twice when `break-inside: avoid` on a block holding the heading moves it.
        // Only pictures and the aside (which hold no heading) may avoid breaking.
        for options in [
            PdfOptions::default(),
            PdfOptions { cover_page: true, ..PdfOptions::default() },
            PdfOptions { new_page_per_step: true, ..PdfOptions::default() },
            PdfOptions { new_page_per_step: true, cover_page: true, contents: false, ..PdfOptions::default() },
        ] {
            let p = page(&options);
            let css = &p[p.find("<style>").unwrap()..p.find("</style>").unwrap()];
            for (at, _) in css.match_indices("break-inside:avoid") {
                let before = &css[..at];
                let open = before.rfind('{').unwrap();
                let close = before[..open].rfind('}').map_or(0, |i| i + 1);
                let selector = &before[close..open];
                assert!(selector == "figure" || selector == "aside.block", "break-inside:avoid on {selector}");
            }
            assert!(
                css.contains("h2,h3,h4,h5,h6{line-height:1.3;margin:0 0 .4em;break-after:avoid}"),
                "no stranded heading"
            );
        }
    }

    #[test]
    fn the_authors_words_cannot_break_out_of_the_page_or_the_style() {
        let mut d = demo();
        d.document.guide.title = "A </style><script>alert(1)</script> title".to_string();
        d.document.guide.steps[0].title = "<img src=x onerror=alert(2)>".to_string();
        d.document.guide.steps[1].image_description = "x\" onerror=\"alert(3)".to_string();
        let plan = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let options = PdfOptions {
            header_text: "Hello </style><script>alert(4)</script> \"quoted\" \\ back".to_string(),
            footer_text: "x'};body{display:none}/*".to_string(),
            ..PdfOptions::default()
        };
        let p = print_page(&plan, &options);
        assert!(!p.contains("<script"), "{p}");
        assert_eq!(p.matches("</style>").count(), 1);
        assert!(!p.contains("<img src=x"));
        assert!(!p.contains("\" onerror=\"alert(3)"));
        assert!(p.contains("<title>A &lt;/style&gt;&lt;script&gt;alert(1)&lt;/script&gt; title</title>"));
        assert!(p.contains("\\3C /style\\3E \\3C script\\3E alert(4)"), "the header text is escaped for CSS");
        assert!(p.contains("\\22 quoted\\22 "));
        assert!(p.contains("\\5C  back"));
        let style_start = p.find("<style>").unwrap();
        let style_end = p.find("</style>").unwrap();
        let css = &p[style_start..style_end];
        assert!(
            css.contains("@bottom-left{content:\"x'};body{display:none}/*\""),
            "the footer stays inside its string"
        );
    }

    #[test]
    fn css_strings_are_quoted_and_cannot_end_early() {
        assert_eq!(css_string("Plain text"), "\"Plain text\"");
        assert_eq!(css_string("a\"b"), "\"a\\22 b\"");
        assert_eq!(css_string("a\\b"), "\"a\\5C b\"");
        assert_eq!(css_string("</style>"), "\"\\3C /style\\3E \"");
        assert_eq!(css_string("a&b"), "\"a\\26 b\"");
        assert_eq!(css_string("line\none\r\ttab"), "\"line one  tab\"");
        assert_eq!(css_string("bell\u{7}"), "\"bell\"");
        assert_eq!(css_string("Zürich – naïve"), "\"Zürich – naïve\"");
    }

    #[test]
    fn the_paper_orientation_and_margins_are_in_the_page_rule() {
        let p = page(&PdfOptions::default());
        assert!(p.contains("@page{size:A4 portrait;margin:20mm;"));
        let p = page(&PdfOptions {
            page_size: PageSize::Letter,
            orientation: Orientation::Landscape,
            margin_mm: 15,
            ..PdfOptions::default()
        });
        assert!(p.contains("@page{size:letter landscape;margin:15mm;"));
        for (size, css) in
            [(PageSize::A3, "A3"), (PageSize::A5, "A5"), (PageSize::Legal, "legal"), (PageSize::A4, "A4")]
        {
            assert!(page(&PdfOptions { page_size: size, ..PdfOptions::default() })
                .contains(&format!("size:{css} portrait")));
        }
    }

    #[test]
    fn margins_are_kept_sensible() {
        assert!(page(&PdfOptions { margin_mm: 0, ..PdfOptions::default() }).contains("margin:10mm;"));
        assert!(page(&PdfOptions { margin_mm: 500, ..PdfOptions::default() }).contains("margin:40mm;"));
    }

    #[test]
    fn a_picture_can_never_be_taller_than_the_page() {
        let tallest = |options: PdfOptions| {
            let p = page(&options);
            let start = p.find("max-height:").unwrap() + "max-height:".len();
            p[start..start + p[start..].find("mm").unwrap()].parse::<u32>().unwrap()
        };
        assert_eq!(tallest(PdfOptions::default()), 217, "297 less 2 by 20 less 40");
        assert_eq!(
            tallest(PdfOptions { orientation: Orientation::Landscape, ..PdfOptions::default() }),
            130,
            "210 less 2 by 20 less 40"
        );
        let small = tallest(PdfOptions {
            page_size: PageSize::A5,
            orientation: Orientation::Landscape,
            margin_mm: 40,
            ..PdfOptions::default()
        });
        assert_eq!(small, 40, "never below a usable height");
    }

    #[test]
    fn page_numbers_header_and_footer_are_margin_boxes_in_the_guides_language() {
        let p = page(&PdfOptions::default());
        assert!(p.contains("@bottom-right{content:\"Page \" counter(page) \" of \" counter(pages);"));
        assert!(!p.contains("@top-center") && !p.contains("@bottom-left"), "no header or footer unless asked for");
        let p = page(&PdfOptions {
            header_text: "  Help desk guide  ".to_string(),
            footer_text: "Version 2".to_string(),
            ..PdfOptions::default()
        });
        assert!(p.contains("@top-center{content:\"Help desk guide\";"));
        assert!(p.contains("@bottom-left{content:\"Version 2\";"));
        let p = page(&PdfOptions { page_numbers: false, ..PdfOptions::default() });
        assert!(!p.contains("counter(page)"));
        let mut d = demo();
        for (code, text) in [("de", "\"Seite \" counter(page) \" von \""), ("fr", "\"Page \" counter(page) \" sur \"")]
        {
            d.document.guide.language = code.to_string();
            let plan = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
            let p = print_page(&plan, &PdfOptions::default());
            assert!(p.contains(text), "{code}");
            assert!(p.contains(&format!("<html lang=\"{code}\">")));
        }
    }

    #[test]
    fn the_contents_list_and_the_cover_are_optional() {
        let p = page(&PdfOptions { contents: false, ..PdfOptions::default() });
        assert!(!p.contains("<nav"));
        assert!(!p.contains("<h2>Contents</h2>"));
        assert!(!p.contains("class=\"cover\">"), "{p}");
        let p = page(&PdfOptions { cover_page: true, ..PdfOptions::default() });
        assert!(p.contains("<section class=\"cover\">\n<h1>How to submit an assignment</h1>"));
        assert_eq!(p.matches("<h1>").count(), 1);
        assert!(p.find("class=\"cover\">").unwrap() < p.find("<nav").unwrap(), "the cover comes first");
    }

    #[test]
    fn the_contents_entries_link_to_every_step() {
        let plan = demo_plan();
        let p = print_page(&plan, &PdfOptions::default());
        for step in plan.steps.iter().filter(|s| s.kind != StepKind::Block) {
            assert!(p.contains(&format!("<a href=\"#{}\">", step.id)), "{}", step.id);
            assert!(p.contains(&format!("<li class=\"step\" id=\"{}\">", step.id)), "{}", step.id);
        }
    }

    #[test]
    fn pages_are_broken_only_where_the_options_ask_for_it() {
        for forced in
            ["break-before:page", "break-after:page", "page-break-before", "page-break-after", "break-before:always"]
        {
            assert!(!page(&PdfOptions::default()).contains(forced), "{forced}");
        }
        let p = page(&PdfOptions { new_page_per_step: true, ..PdfOptions::default() });
        assert!(p.contains("ol.steps>li.step{break-before:page}"), "every top-level step starts a page");
        assert!(!p.contains("break-after:page"), "no cover, no break after it");
    }

    #[test]
    fn the_cover_is_its_own_page_and_holds_the_title() {
        let p = page(&PdfOptions { cover_page: true, ..PdfOptions::default() });
        assert!(p.contains(".cover{break-after:page;padding-top:30mm}"));
        let cover_end = p.find("</section>").unwrap();
        assert!(p[..cover_end].contains("<h1>How to submit an assignment</h1>"), "the title is on the cover");
        assert!(p[cover_end..].find("<h1>").is_none(), "and nowhere else");
        assert!(p[cover_end..].contains("<nav class=\"toc\""), "the contents list follows the cover");
        assert!(p.find("</section>").unwrap() < p.find("<ol class=\"steps\">").unwrap());
    }

    #[test]
    fn a_picture_never_splits_and_a_heading_never_ends_a_page() {
        let p = page(&PdfOptions::default());
        assert!(p.contains("*:has(+figure){break-after:avoid}"), "a step's text stays with its picture");
        assert!(p.contains("figure{margin:.6em 0;break-inside:avoid}"));
        assert!(p.contains("aside.block{border-left:3pt solid #2B3AB8;padding:.3em .8em;break-inside:avoid}"));
    }

    #[test]
    fn an_empty_guide_still_makes_a_valid_page() {
        let guide = openstage_express_core::Guide::new("g", "Nothing yet", "en", "t");
        let plan = plan(&guide, &crate::picture::MemoryPictures::default(), &BlockText, &ExportOptions::default());
        let p = print_page(&plan, &PdfOptions::default());
        assert!(p.contains("<h1>Nothing yet</h1>"));
        assert!(!p.contains("<nav"), "nothing to list");
        assert_eq!(p.matches("<ol").count(), p.matches("</ol>").count());
    }

    #[test]
    fn the_print_page_is_repeatable() {
        assert_eq!(page(&PdfOptions::default()), page(&PdfOptions::default()));
    }

    #[test]
    fn the_audit_finds_what_a_tagged_pdf_holds() {
        let audit = audit_pdf(TAGGED_SAMPLE.as_bytes());
        assert_eq!(
            audit,
            PdfAudit {
                is_pdf: true,
                pages: 2,
                tagged: true,
                language: true,
                bookmarks: true,
                title_shown: true,
                headings: 3,
                figures: 2,
                alt_texts: 2,
            }
        );
    }

    #[test]
    fn the_audit_reads_a_file_that_is_not_a_pdf_as_nothing() {
        assert_eq!(audit_pdf(b"<html></html>"), PdfAudit::default());
        assert_eq!(audit_pdf(b""), PdfAudit::default());
        let untagged = audit_pdf(b"%PDF-1.4\n1 0 obj\n<</Type /Catalog>>\nendobj\n");
        assert!(untagged.is_pdf && !untagged.tagged);
    }

    #[test]
    fn a_structure_tree_without_the_marked_flag_is_not_called_tagged() {
        let audit = audit_pdf(b"%PDF-1.4\n<</StructTreeRoot 5 0 R /Lang (en)>>\n");
        assert!(!audit.tagged);
    }

    #[test]
    fn the_audit_says_what_is_missing_in_plain_words() {
        let options = PdfOptions { page_numbers: false, ..PdfOptions::default() };
        let good = audit_pdf(TAGGED_SAMPLE.as_bytes());
        assert!(good.warnings(2, &options).is_empty(), "{:?}", good.warnings(2, &options));
        let fewer = good.warnings(5, &options);
        assert_eq!(fewer.len(), 1);
        assert!(fewer[0].contains("Only 2 of the 5 pictures"), "{fewer:?}");
        let untagged = audit_pdf(b"%PDF-1.4\n").warnings(2, &options);
        assert_eq!(untagged.len(), 1, "one clear message, not a list of consequences");
        assert!(untagged[0].contains("no tags") && untagged[0].contains("web page"), "{untagged:?}");
        let no_lang = PdfAudit { language: false, ..good.clone() }.warnings(0, &options);
        assert!(no_lang[0].contains("language"), "{no_lang:?}");
        let no_marks = PdfAudit { bookmarks: false, ..good.clone() }.warnings(0, &options);
        assert!(no_marks[0].contains("bookmarks"), "{no_marks:?}");
    }

    #[test]
    fn page_numbers_and_header_lines_bring_the_untagged_margin_warning() {
        let good = audit_pdf(TAGGED_SAMPLE.as_bytes());
        for options in [
            PdfOptions::default(),
            PdfOptions { page_numbers: false, header_text: "Top".to_string(), ..PdfOptions::default() },
            PdfOptions { page_numbers: false, footer_text: "Bottom".to_string(), ..PdfOptions::default() },
        ] {
            let warnings = good.warnings(2, &options);
            assert_eq!(warnings.len(), 1, "{options:?}");
            assert!(warnings[0].contains("not tagged"), "{warnings:?}");
        }
        let plain = PdfOptions { page_numbers: false, ..PdfOptions::default() };
        assert!(good.warnings(2, &plain).is_empty());
    }

    #[test]
    fn the_export_hands_the_print_page_to_the_renderer_and_names_the_file_after_the_guide() {
        let plan = demo_plan();
        let fake = Fake::replying(Ok(TAGGED_SAMPLE.as_bytes().to_vec()));
        let options = PdfOptions::default();
        let export = export_pdf(&plan, &options, &fake).unwrap();
        assert_eq!(export.files.len(), 1);
        assert_eq!(export.files[0].path, "How to submit an assignment.pdf");
        assert_eq!(export.files[0].bytes, TAGGED_SAMPLE.as_bytes());
        let (html, title, language) = fake.seen.borrow().clone().unwrap();
        assert_eq!(html, print_page(&plan, &options));
        assert_eq!(title, "How to submit an assignment");
        assert_eq!(language, "en");
        let pictures = plan.steps.iter().filter(|s| s.picture.is_some()).count();
        assert!(
            export.warnings.iter().any(|w| w.contains(&format!("Only 2 of the {pictures} pictures"))),
            "{:?}",
            export.warnings
        );
        assert!(export.warnings.iter().any(|w| w.contains("not tagged")), "{:?}", export.warnings);
    }

    #[test]
    fn the_plans_own_warnings_come_through() {
        let d = demo();
        let mut pictures = d.pictures;
        pictures.0.remove("images/step-0002.png");
        let plan = plan(&d.document.guide, &pictures, &BlockText, &ExportOptions::default());
        assert!(!plan.warnings.is_empty());
        let export =
            export_pdf(&plan, &PdfOptions::default(), &Fake::replying(Ok(TAGGED_SAMPLE.as_bytes().to_vec()))).unwrap();
        assert!(export.warnings.iter().any(|w| w.contains("images/step-0002.png")), "{:?}", export.warnings);
    }

    #[test]
    fn a_renderer_that_fails_stops_the_export_with_its_own_words() {
        let plan = demo_plan();
        let fake = Fake::replying(Err(RenderError::Unavailable("WebView2 is not installed.".to_string())));
        let error = export_pdf(&plan, &PdfOptions::default(), &fake).unwrap_err();
        assert_eq!(error.to_string(), "WebView2 is not installed.");
        assert!(matches!(error, PdfError::Render(RenderError::Unavailable(_))));
    }

    #[test]
    fn something_that_is_not_a_pdf_is_never_written_as_one() {
        let plan = demo_plan();
        let fake = Fake::replying(Ok(b"<html>an error page</html>".to_vec()));
        let error = export_pdf(&plan, &PdfOptions::default(), &fake).unwrap_err();
        assert_eq!(error, PdfError::NotAPdf);
        assert!(error.to_string().contains("nothing was exported"));
    }
}
