// SPDX-License-Identifier: Apache-2.0
//! The Word export (spec section 10.2): a `.docx` file written as Office Open XML by hand.
//!
//! * The guide's title in the Title style; every step a heading in the Heading 1 style (a substep Heading 2, then Heading 3), so
//!   Word's navigation pane, its accessibility checker and screen readers find the structure. Step numbers are real text.
//! * A contents list as a real Word `TOC` field, already filled in with a link to every step (without page numbers, which only
//!   Word can work out), so it is complete when opened and can be updated in Word with a right click.
//! * Every picture is an inline picture with alt text, scaled to fit the page. A content block is a shaded note.
//! * Descriptions keep their bold, italic, links and lists. The document says what language it is in, has a title in its
//!   properties and, if asked, "Page 3 of 12" in a footer.
//! * Fonts are Calibri (Word's own), not embedded: a Word file cannot carry Windows fonts, and Roboto, which the PDF carries, is
//!   not embedded here yet.

use openstage_express_core::fsutil::sanitise_folder_name;
use openstage_express_core::{Language, StepKind};

use crate::encoding::{parse_markdown, Block, Inline};
use crate::office::{
    app_properties, content_types, core_properties, fit_picture, lang_tag, mm_to_emu, package_rels, xml_escape,
    OfficeError, Rels,
};
use crate::pdf::{words, Orientation, PageSize};
use crate::plan::Plan;
use crate::zip::{self, ZipEntry};
use crate::{Export, OutFile};

pub const MIN_MARGIN_MM: u32 = 10;
pub const MAX_MARGIN_MM: u32 = 40;

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const MAIN_TYPE: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const BULLETS: u32 = 1;

/// What the person chose in the Word export dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocxOptions {
    pub page_size: PageSize,
    pub orientation: Orientation,
    /// Margin on all four sides, kept between [`MIN_MARGIN_MM`] and [`MAX_MARGIN_MM`].
    pub margin_mm: u32,
    /// The contents list (a Word `TOC` field).
    pub contents: bool,
    /// "Page 3 of 12" centred in the footer.
    pub page_numbers: bool,
    /// Every top-level step starts on a new page.
    pub new_page_per_step: bool,
}

impl Default for DocxOptions {
    fn default() -> Self {
        DocxOptions {
            page_size: PageSize::A4,
            orientation: Orientation::Portrait,
            margin_mm: 25,
            contents: true,
            page_numbers: true,
            new_page_per_step: false,
        }
    }
}

impl DocxOptions {
    fn margin(&self) -> u32 {
        self.margin_mm.clamp(MIN_MARGIN_MM, MAX_MARGIN_MM)
    }

    fn paper_mm(&self) -> (f64, f64) {
        let (w, h) = self.page_size.portrait_mm();
        match self.orientation {
            Orientation::Portrait => (w, h),
            Orientation::Landscape => (h, w),
        }
    }
}

fn twips(mm: f64) -> i64 {
    (mm / 25.4 * 1440.0).round() as i64
}

/// Builds the body of `word/document.xml` and remembers what it needs from the other parts.
struct Body {
    xml: String,
    rels: Rels,
    images: Vec<ZipEntry>,
    next_drawing: u32,
    numbered_lists: u32,
    max_width: i64,
    max_height: i64,
}

impl Body {
    fn run(&self, text: &str, bold: bool, italic: bool, link: bool) -> String {
        let mut props = String::new();
        if link {
            props.push_str("<w:rStyle w:val=\"Hyperlink\"/>");
        }
        if bold {
            props.push_str("<w:b/>");
        }
        if italic {
            props.push_str("<w:i/>");
        }
        let props = if props.is_empty() { String::new() } else { format!("<w:rPr>{props}</w:rPr>") };
        format!("<w:r>{props}<w:t xml:space=\"preserve\">{}</w:t></w:r>", xml_escape(text))
    }

    fn inline_runs(&mut self, inlines: &[Inline], bold: bool, italic: bool, link: bool) -> String {
        let mut out = String::new();
        for inline in inlines {
            match inline {
                Inline::Text(text) => out.push_str(&self.run(text, bold, italic, link)),
                Inline::Strong(inner) => out.push_str(&self.inline_runs(inner, true, italic, link)),
                Inline::Emphasis(inner) => out.push_str(&self.inline_runs(inner, bold, true, link)),
                Inline::Link { url, label } => {
                    let id = self.rels.add("hyperlink", url, true);
                    let inner = self.inline_runs(label, bold, italic, true);
                    out.push_str(&format!("<w:hyperlink r:id=\"{id}\" w:history=\"1\">{inner}</w:hyperlink>"));
                }
            }
        }
        out
    }

    fn paragraph_properties(style: Option<&str>, keep_next: bool, extra: &str) -> String {
        let mut props = String::new();
        if let Some(style) = style {
            props.push_str(&format!("<w:pStyle w:val=\"{style}\"/>"));
        }
        if keep_next {
            props.push_str("<w:keepNext/>");
        }
        props.push_str(extra);
        if props.is_empty() {
            String::new()
        } else {
            format!("<w:pPr>{props}</w:pPr>")
        }
    }

    /// A description (Markdown subset) as paragraphs. `style` is the paragraph style of its ordinary paragraphs.
    fn description(&mut self, text: &str, style: Option<&str>, keep_next: bool) {
        for block in parse_markdown(text) {
            match block {
                Block::Paragraph(lines) => {
                    let mut runs = String::new();
                    for (n, line) in lines.iter().enumerate() {
                        if n > 0 {
                            runs.push_str("<w:r><w:br/></w:r>");
                        }
                        runs.push_str(&self.inline_runs(line, false, false, false));
                    }
                    let props = Self::paragraph_properties(style, keep_next, "");
                    self.xml.push_str(&format!("<w:p>{props}{runs}</w:p>"));
                }
                Block::List { numbered, items } => {
                    let num_id = if numbered {
                        self.numbered_lists += 1;
                        BULLETS + self.numbered_lists
                    } else {
                        BULLETS
                    };
                    for item in items {
                        let runs = self.inline_runs(&item, false, false, false);
                        let numbering =
                            format!("<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"{num_id}\"/></w:numPr>");
                        let props = Self::paragraph_properties(Some("ListParagraph"), keep_next, &numbering);
                        self.xml.push_str(&format!("<w:p>{props}{runs}</w:p>"));
                    }
                }
            }
        }
    }

    fn picture(&mut self, png: &[u8], width: u32, height: u32, alt: &str) {
        let n = self.images.len() as u32 + 1;
        let path = format!("word/media/image{n}.png");
        let id = self.rels.add("image", &format!("media/image{n}.png"), false);
        self.images.push(ZipEntry::stored(path, png.to_vec()));
        self.next_drawing += 1;
        let drawing = self.next_drawing;
        let (cx, cy) = fit_picture(width, height, self.max_width, self.max_height);
        let alt = xml_escape(alt);
        self.xml.push_str(&format!(
            "<w:p><w:pPr><w:keepLines/></w:pPr><w:r><w:drawing>\
<wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/>\
<wp:docPr id=\"{drawing}\" name=\"Picture {drawing}\" descr=\"{alt}\"/>\
<wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect=\"1\"/></wp:cNvGraphicFramePr>\
<a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\
<pic:pic><pic:nvPicPr><pic:cNvPr id=\"{drawing}\" name=\"image{n}.png\" descr=\"{alt}\"/><pic:cNvPicPr/></pic:nvPicPr>\
<pic:blipFill><a:blip r:embed=\"{id}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>\
<pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr>\
</pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"
        ));
    }
}

fn bookmark_name(n: usize) -> String {
    format!("_Toc{}", 100_000_000 + n)
}

/// The heading level for a step `depth` levels down: Heading 1 for a step, 2 for a substep, 3 for anything deeper.
fn heading_level(depth: usize) -> usize {
    (depth + 1).min(3)
}

fn heading_text(number: Option<&str>, title: &str) -> String {
    format!("{}. {title}", number.unwrap_or(""))
}

fn styles_xml(lang: &str) -> String {
    let lang = xml_escape(lang);
    let heading = |level: u32, size: u32, before: u32| {
        format!(
            "<w:style w:type=\"paragraph\" w:styleId=\"Heading{level}\"><w:name w:val=\"heading {level}\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:uiPriority w:val=\"9\"/><w:qFormat/>\
<w:pPr><w:keepNext/><w:keepLines/><w:spacing w:before=\"{before}\" w:after=\"120\"/><w:outlineLvl w:val=\"{}\"/></w:pPr>\
<w:rPr><w:b/><w:color w:val=\"1B1F33\"/><w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/></w:rPr></w:style>",
            level - 1
        )
    };
    let toc = |level: u32, indent: u32| {
        format!(
            "<w:style w:type=\"paragraph\" w:styleId=\"TOC{level}\"><w:name w:val=\"toc {level}\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:autoRedefine/><w:uiPriority w:val=\"39\"/><w:unhideWhenUsed/>\
<w:pPr><w:spacing w:after=\"60\"/><w:ind w:left=\"{indent}\"/></w:pPr></w:style>"
        )
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:styles xmlns:w=\"{W}\">\
<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\" w:eastAsia=\"Calibri\" w:cs=\"Calibri\"/><w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/><w:lang w:val=\"{lang}\" w:eastAsia=\"{lang}\" w:bidi=\"ar-SA\"/></w:rPr></w:rPrDefault>\
<w:pPrDefault><w:pPr><w:spacing w:after=\"120\" w:line=\"276\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault></w:docDefaults>\
<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:qFormat/></w:style>\
<w:style w:type=\"character\" w:default=\"1\" w:styleId=\"DefaultParagraphFont\"><w:name w:val=\"Default Paragraph Font\"/><w:uiPriority w:val=\"1\"/><w:semiHidden/><w:unhideWhenUsed/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Title\"><w:name w:val=\"Title\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:uiPriority w:val=\"10\"/><w:qFormat/>\
<w:pPr><w:spacing w:after=\"240\"/></w:pPr><w:rPr><w:b/><w:color w:val=\"1B1F33\"/><w:sz w:val=\"56\"/><w:szCs w:val=\"56\"/></w:rPr></w:style>\
{}{}{}\
<w:style w:type=\"paragraph\" w:styleId=\"TOCHeading\"><w:name w:val=\"TOC Heading\"/><w:basedOn w:val=\"Heading1\"/><w:next w:val=\"Normal\"/><w:uiPriority w:val=\"39\"/><w:unhideWhenUsed/><w:qFormat/>\
<w:pPr><w:outlineLvl w:val=\"9\"/></w:pPr></w:style>\
{}{}{}\
<w:style w:type=\"character\" w:styleId=\"Hyperlink\"><w:name w:val=\"Hyperlink\"/><w:basedOn w:val=\"DefaultParagraphFont\"/><w:uiPriority w:val=\"99\"/><w:unhideWhenUsed/>\
<w:rPr><w:color w:val=\"2B3AB8\"/><w:u w:val=\"single\"/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"ListParagraph\"><w:name w:val=\"List Paragraph\"/><w:basedOn w:val=\"Normal\"/><w:uiPriority w:val=\"34\"/><w:qFormat/>\
<w:pPr><w:ind w:left=\"720\"/><w:contextualSpacing/></w:pPr></w:style>\
<w:style w:type=\"paragraph\" w:customStyle=\"1\" w:styleId=\"ExpressNote\"><w:name w:val=\"Express Note\"/><w:basedOn w:val=\"Normal\"/><w:uiPriority w:val=\"35\"/><w:qFormat/>\
<w:pPr><w:keepLines/><w:pBdr><w:left w:val=\"single\" w:sz=\"24\" w:space=\"8\" w:color=\"2B3AB8\"/></w:pBdr><w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"F1F3FA\"/><w:ind w:left=\"200\"/></w:pPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Footer\"><w:name w:val=\"footer\"/><w:basedOn w:val=\"Normal\"/><w:uiPriority w:val=\"99\"/><w:unhideWhenUsed/>\
<w:pPr><w:spacing w:after=\"0\"/><w:jc w:val=\"center\"/></w:pPr><w:rPr><w:color w:val=\"4A5068\"/><w:sz w:val=\"18\"/><w:szCs w:val=\"18\"/></w:rPr></w:style>\
</w:styles>",
        heading(1, 32, 360),
        heading(2, 26, 240),
        heading(3, 24, 200),
        toc(1, 0),
        toc(2, 220),
        toc(3, 440)
    )
}

fn settings_xml() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:settings xmlns:w=\"{W}\"><w:zoom w:percent=\"100\"/><w:defaultTabStop w:val=\"720\"/>\
<w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/></w:compat></w:settings>"
    )
}

fn numbering_xml(numbered_lists: u32) -> String {
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:numbering xmlns:w=\"{W}\">\
<w:abstractNum w:abstractNumId=\"0\"><w:multiLevelType w:val=\"hybridMultilevel\"/>\
<w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/><w:numFmt w:val=\"bullet\"/><w:lvlText w:val=\"&#8226;\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"720\" w:hanging=\"360\"/></w:pPr></w:lvl></w:abstractNum>\
<w:abstractNum w:abstractNumId=\"1\"><w:multiLevelType w:val=\"hybridMultilevel\"/>\
<w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/><w:numFmt w:val=\"decimal\"/><w:lvlText w:val=\"%1.\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"720\" w:hanging=\"360\"/></w:pPr></w:lvl></w:abstractNum>\
<w:num w:numId=\"{BULLETS}\"><w:abstractNumId w:val=\"0\"/></w:num>"
    );
    // Each numbered list is its own list that starts again at 1.
    for n in 1..=numbered_lists {
        out.push_str(&format!(
            "<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"1\"/><w:lvlOverride w:ilvl=\"0\"><w:startOverride w:val=\"1\"/></w:lvlOverride></w:num>",
            BULLETS + n
        ));
    }
    out.push_str("</w:numbering>");
    out
}

fn field(instruction: &str, shown: &str) -> String {
    format!(
        "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\"> {instruction} </w:instrText></w:r>\
<w:r><w:fldChar w:fldCharType=\"separate\"/></w:r><w:r><w:t>{shown}</w:t></w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r>"
    )
}

fn footer_xml(language: Language) -> String {
    let w = words(language);
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:ftr xmlns:w=\"{W}\"><w:p><w:pPr><w:pStyle w:val=\"Footer\"/></w:pPr>\
<w:r><w:t xml:space=\"preserve\">{} </w:t></w:r>{}<w:r><w:t xml:space=\"preserve\"> {} </w:t></w:r>{}</w:p></w:ftr>",
        xml_escape(w.page),
        field("PAGE", "1"),
        xml_escape(w.of),
        field("NUMPAGES", "1")
    )
}

/// The parts of a Word file for `plan`.
pub fn docx_parts(plan: &Plan, options: &DocxOptions) -> Vec<ZipEntry> {
    let language = Language::from_code(&plan.language);
    let lang = lang_tag(&plan.language);
    let (paper_w, paper_h) = options.paper_mm();
    let margin = f64::from(options.margin());
    let max_width = mm_to_emu(paper_w - 2.0 * margin);
    let max_height = mm_to_emu(paper_h - 2.0 * margin - 40.0).max(mm_to_emu(40.0));

    let mut rels = Rels::default();
    rels.add("styles", "styles.xml", false);
    rels.add("settings", "settings.xml", false);
    rels.add("numbering", "numbering.xml", false);
    let footer_rel = options.page_numbers.then(|| rels.add("footer", "footer1.xml", false));
    let mut body = Body {
        xml: String::new(),
        rels,
        images: Vec::new(),
        next_drawing: 0,
        numbered_lists: 0,
        max_width,
        max_height,
    };

    // The title and the guide's own description.
    body.xml.push_str(&format!(
        "<w:p><w:pPr><w:pStyle w:val=\"Title\"/></w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        xml_escape(&plan.title)
    ));
    body.description(&plan.description, None, false);

    // Every step that is not a content block has a bookmark (for the contents list and for Word to find the heading by).
    let bookmark_of: Vec<Option<usize>> = {
        let mut n = 0;
        plan.steps
            .iter()
            .map(|s| {
                if s.kind == StepKind::Block {
                    None
                } else {
                    n += 1;
                    Some(n)
                }
            })
            .collect()
    };

    let listed: Vec<usize> = (0..plan.steps.len()).filter(|&i| bookmark_of[i].is_some()).collect();
    if options.contents && !listed.is_empty() {
        body.xml.push_str(&format!(
            "<w:p><w:pPr><w:pStyle w:val=\"TOCHeading\"/></w:pPr><w:r><w:t>{}</w:t></w:r></w:p>",
            xml_escape(words(language).contents)
        ));
        for (n, &i) in listed.iter().enumerate() {
            let step = &plan.steps[i];
            let level = heading_level(step.depth);
            let start = if n == 0 {
                "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\"> TOC \\o \"1-3\" \\h \\z \\n </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>"
            } else {
                ""
            };
            let text = xml_escape(&heading_text(step.number.as_deref(), &step.title));
            body.xml.push_str(&format!(
                "<w:p><w:pPr><w:pStyle w:val=\"TOC{level}\"/></w:pPr>{start}<w:hyperlink w:anchor=\"{}\" w:history=\"1\">\
<w:r><w:rPr><w:rStyle w:val=\"Hyperlink\"/></w:rPr><w:t xml:space=\"preserve\">{text}</w:t></w:r></w:hyperlink></w:p>",
                bookmark_name(bookmark_of[i].unwrap())
            ));
        }
        body.xml.push_str("<w:p><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>");
    }

    // The steps.
    for (i, step) in plan.steps.iter().enumerate() {
        if step.kind == StepKind::Block {
            let title = body.run(&step.title, true, false, false);
            body.xml
                .push_str(&format!("<w:p><w:pPr><w:pStyle w:val=\"ExpressNote\"/><w:keepNext/></w:pPr>{title}</w:p>"));
            body.description(&step.description, Some("ExpressNote"), false);
            continue;
        }
        let level = heading_level(step.depth);
        let page_break = if options.new_page_per_step && step.depth == 0 { "<w:pageBreakBefore/>" } else { "" };
        let n = bookmark_of[i].unwrap();
        body.xml.push_str(&format!(
            "<w:p><w:pPr><w:pStyle w:val=\"Heading{level}\"/>{page_break}</w:pPr><w:bookmarkStart w:id=\"{n}\" w:name=\"{}\"/>\
<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r><w:bookmarkEnd w:id=\"{n}\"/></w:p>",
            bookmark_name(n),
            xml_escape(&heading_text(step.number.as_deref(), &step.title))
        ));
        body.description(&step.description, None, step.picture.is_some());
        if let Some(picture) = &step.picture {
            body.picture(&picture.png, picture.width, picture.height, &step.alt);
        }
    }

    // The page.
    let (w, h) = (twips(paper_w), twips(paper_h));
    let orient = if options.orientation == Orientation::Landscape { " w:orient=\"landscape\"" } else { "" };
    let m = twips(margin);
    let footer_reference = footer_rel
        .as_ref()
        .map(|id| format!("<w:footerReference w:type=\"default\" r:id=\"{id}\"/>"))
        .unwrap_or_default();
    body.xml.push_str(&format!(
        "<w:sectPr>{footer_reference}<w:pgSz w:w=\"{w}\" w:h=\"{h}\"{orient}/>\
<w:pgMar w:top=\"{m}\" w:right=\"{m}\" w:bottom=\"{m}\" w:left=\"{m}\" w:header=\"{}\" w:footer=\"{}\" w:gutter=\"0\"/></w:sectPr>",
        m / 2,
        m / 2
    ));

    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document xmlns:w=\"{W}\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><w:body>{}</w:body></w:document>",
        body.xml
    );

    let mut overrides = vec![
        ("/word/document.xml", MAIN_TYPE),
        ("/word/styles.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"),
        ("/word/settings.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"),
        ("/word/numbering.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"),
        ("/docProps/core.xml", "application/vnd.openxmlformats-package.core-properties+xml"),
        ("/docProps/app.xml", "application/vnd.openxmlformats-officedocument.extended-properties+xml"),
    ];
    if footer_rel.is_some() {
        overrides
            .push(("/word/footer1.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"));
    }

    let mut parts = vec![
        ZipEntry::text("[Content_Types].xml", content_types(&overrides)),
        ZipEntry::text("_rels/.rels", package_rels("word/document.xml")),
        ZipEntry::text("docProps/core.xml", core_properties(&plan.title, &lang)),
        ZipEntry::text("docProps/app.xml", app_properties()),
        ZipEntry::text("word/document.xml", document),
        ZipEntry::text("word/_rels/document.xml.rels", body.rels.xml()),
        ZipEntry::text("word/styles.xml", styles_xml(&lang)),
        ZipEntry::text("word/settings.xml", settings_xml()),
        ZipEntry::text("word/numbering.xml", numbering_xml(body.numbered_lists)),
    ];
    if footer_rel.is_some() {
        parts.push(ZipEntry::text("word/footer1.xml", footer_xml(language)));
    }
    parts.extend(body.images);
    parts
}

/// The Word export: one file named after the guide.
pub fn export_docx(plan: &Plan, options: &DocxOptions) -> Result<Export, OfficeError> {
    let bytes = zip::build(&docx_parts(plan, options))?;
    Ok(Export {
        files: vec![OutFile { path: format!("{}.docx", sanitise_folder_name(&plan.title)), bytes }],
        warnings: plan.warnings.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo;
    use crate::office::check::{attributes, texts, well_formed};
    use crate::plan::{plan, ExportOptions};
    use crate::zip::reader::{unzip, Unzipped};
    use openstage_capture_render::BlockText;

    fn demo_plan() -> Plan {
        let d = demo();
        plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default())
    }

    fn make(options: &DocxOptions) -> Unzipped {
        unzip(&export_docx(&demo_plan(), options).unwrap().files[0].bytes)
    }

    fn document(options: &DocxOptions) -> String {
        make(options).text("word/document.xml")
    }

    #[test]
    fn the_package_has_every_part_with_the_content_types_file_first() {
        let z = make(&DocxOptions::default());
        assert_eq!(z.names[0], "[Content_Types].xml");
        for part in [
            "_rels/.rels",
            "docProps/core.xml",
            "docProps/app.xml",
            "word/document.xml",
            "word/_rels/document.xml.rels",
            "word/styles.xml",
            "word/settings.xml",
            "word/numbering.xml",
            "word/footer1.xml",
            "word/media/image1.png",
        ] {
            assert!(z.names.iter().any(|n| n == part), "{part} in {:?}", z.names);
        }
        assert_eq!(
            z.names.iter().filter(|n| n.starts_with("word/media/")).count(),
            5,
            "one picture for each step that has one"
        );
    }

    #[test]
    fn every_xml_part_is_well_formed() {
        for options in [
            DocxOptions::default(),
            DocxOptions { contents: false, page_numbers: false, ..DocxOptions::default() },
            DocxOptions { orientation: Orientation::Landscape, new_page_per_step: true, ..DocxOptions::default() },
        ] {
            let z = make(&options);
            for name in z.names.iter().filter(|n| n.ends_with(".xml") || n.ends_with(".rels")) {
                well_formed(name, &z.text(name));
            }
        }
    }

    #[test]
    fn every_relationship_points_at_a_real_part_and_every_part_has_a_content_type() {
        let z = make(&DocxOptions::default());
        let doc = z.text("word/document.xml");
        let rels = z.text("word/_rels/document.xml.rels");
        let ids = attributes(&rels, "Relationship", "Id");
        let targets = attributes(&rels, "Relationship", "Target");
        let modes = attributes(&rels, "Relationship", "TargetMode");
        assert_eq!(ids.len(), targets.len());
        for target in &targets {
            if target.starts_with("http") {
                continue;
            }
            assert!(z.get(&format!("word/{target}")).is_some(), "relationship target word/{target}");
        }
        assert_eq!(modes.len(), 1, "only the link in the demo's description is external");
        for used in attributes(&doc, "a:blip", "r:embed").into_iter().chain(attributes(&doc, "w:hyperlink", "r:id")) {
            assert!(ids.contains(&used), "{used} is used in the document but not defined in {ids:?}");
        }
        let package_rels = z.text("_rels/.rels");
        for target in attributes(&package_rels, "Relationship", "Target") {
            assert!(z.get(&target).is_some(), "package relationship target {target}");
        }
        let types = z.text("[Content_Types].xml");
        let overrides = attributes(&types, "Override", "PartName");
        for name in z.names.iter().filter(|n| *n != "[Content_Types].xml") {
            let has_default = name.ends_with(".rels") || name.ends_with(".png");
            assert!(has_default || overrides.contains(&format!("/{name}")), "{name} has no content type");
        }
        for part in overrides {
            assert!(z.get(part.trim_start_matches('/')).is_some(), "content type for a part that is missing: {part}");
        }
    }

    #[test]
    fn the_title_and_the_steps_are_headings_with_their_numbers_as_text() {
        let doc = document(&DocxOptions::default());
        assert_eq!(doc.matches("<w:pStyle w:val=\"Title\"/>").count(), 1);
        assert_eq!(doc.matches("<w:pStyle w:val=\"Heading1\"/>").count(), 4, "steps 1, 2, 3 and 4");
        assert_eq!(doc.matches("<w:pStyle w:val=\"Heading2\"/>").count(), 1, "the substep");
        assert_eq!(doc.matches("<w:pStyle w:val=\"Heading3\"/>").count(), 0);
        let all = texts(&doc, "w:t");
        for heading in [
            "How to submit an assignment",
            "1. Click \"Assignments\"",
            "2. Click \"New assignment\"",
            "2.1. Type \"Week 3 reflection\"",
            "3. Select \"Course\"",
            "4. Click \"Submit\"",
        ] {
            assert!(all.iter().any(|t| t == heading), "{heading} in {all:?}");
        }
        let styles = make(&DocxOptions::default()).text("word/styles.xml");
        for style in [
            "Title",
            "Heading1",
            "Heading2",
            "Heading3",
            "TOC1",
            "TOC2",
            "TOC3",
            "TOCHeading",
            "Hyperlink",
            "ListParagraph",
            "ExpressNote",
            "Footer",
            "Normal",
        ] {
            assert!(styles.contains(&format!("w:styleId=\"{style}\"")), "{style} is defined");
        }
        for used in attributes(&doc, "w:pStyle", "w:val") {
            assert!(styles.contains(&format!("w:styleId=\"{used}\"")), "{used} is used but not defined");
        }
    }

    #[test]
    fn every_picture_carries_its_alt_text_and_fits_the_page() {
        let p = demo_plan();
        let doc = document(&DocxOptions::default());
        let alts = attributes(&doc, "wp:docPr", "descr");
        let expected: Vec<&str> = p.steps.iter().filter(|s| s.picture.is_some()).map(|s| s.alt.as_str()).collect();
        assert_eq!(alts, expected);
        assert!(alts.iter().all(|a| !a.is_empty()));
        assert_eq!(attributes(&doc, "pic:cNvPr", "descr"), expected, "the picture's own description matches");
        let ids = attributes(&doc, "wp:docPr", "id");
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(ids.len(), unique.len(), "drawing ids are unique");
        // A4 less two 25 mm margins is 160 mm.
        let content_width = mm_to_emu(160.0);
        for cx in attributes(&doc, "wp:extent", "cx") {
            assert!(cx.parse::<i64>().unwrap() <= content_width, "{cx} is wider than the page");
        }
    }

    #[test]
    fn the_contents_list_is_a_word_field_already_filled_in_with_links_that_land_on_the_headings() {
        let doc = document(&DocxOptions::default());
        assert_eq!(doc.matches("w:fldCharType=\"begin\"").count(), 1);
        assert_eq!(doc.matches("w:fldCharType=\"separate\"").count(), 1);
        assert_eq!(doc.matches("w:fldCharType=\"end\"").count(), 1);
        assert!(doc.contains("TOC \\o \"1-3\" \\h \\z \\n"));
        let anchors = attributes(&doc, "w:hyperlink", "w:anchor");
        assert_eq!(anchors.len(), 5, "every step but the tip is listed");
        let bookmarks = attributes(&doc, "w:bookmarkStart", "w:name");
        assert_eq!(bookmarks, anchors, "each link lands on the bookmark round its step's heading, in order");
        let ids = attributes(&doc, "w:bookmarkStart", "w:id");
        assert_eq!(ids, ["1", "2", "3", "4", "5"]);
        assert_eq!(attributes(&doc, "w:bookmarkEnd", "w:id"), ids);
        assert!(doc.contains("<w:pStyle w:val=\"TOCHeading\"/></w:pPr><w:r><w:t>Contents</w:t>"));
        assert_eq!(doc.matches("<w:pStyle w:val=\"TOC1\"/>").count(), 4);
        assert_eq!(doc.matches("<w:pStyle w:val=\"TOC2\"/>").count(), 1);
    }

    #[test]
    fn the_contents_list_can_be_left_out() {
        let doc = document(&DocxOptions { contents: false, ..DocxOptions::default() });
        assert!(!doc.contains("fldCharType") && !doc.contains("TOCHeading") && !doc.contains("w:anchor"));
        assert_eq!(attributes(&doc, "w:bookmarkStart", "w:name").len(), 5, "the headings keep their bookmarks");
    }

    #[test]
    fn descriptions_keep_bold_italic_links_and_lists() {
        let z = make(&DocxOptions::default());
        let doc = z.text("word/document.xml");
        assert!(doc.contains("<w:r><w:rPr><w:b/></w:rPr><w:t xml:space=\"preserve\">Assignments</w:t></w:r>"));
        assert!(doc.contains("<w:hyperlink r:id=\""));
        let rels = z.text("word/_rels/document.xml.rels");
        assert!(rels.contains("Target=\"https://example.com/help\" TargetMode=\"External\""));
        assert!(doc.contains("<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"1\"/></w:numPr>"), "the bullet list");
        assert_eq!(doc.matches("<w:pStyle w:val=\"ListParagraph\"/>").count(), 2);
        assert!(z.text("word/numbering.xml").contains("w:numFmt w:val=\"bullet\""));
    }

    #[test]
    fn each_numbered_list_starts_again_at_one() {
        let mut d = demo();
        d.document.guide.steps[0].description =
            "1. one\n2. two\n\nbetween\n\n1. again\n2. more\n\n- bullet".to_string();
        let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let z = unzip(&export_docx(&p, &DocxOptions::default()).unwrap().files[0].bytes);
        let doc = z.text("word/document.xml");
        assert_eq!(doc.matches("<w:numId w:val=\"2\"/>").count(), 2);
        assert_eq!(doc.matches("<w:numId w:val=\"3\"/>").count(), 2);
        let numbering = z.text("word/numbering.xml");
        well_formed("numbering", &numbering);
        assert!(numbering.contains("<w:num w:numId=\"2\"><w:abstractNumId w:val=\"1\"/><w:lvlOverride w:ilvl=\"0\"><w:startOverride w:val=\"1\"/>"));
        assert!(numbering.contains("<w:num w:numId=\"3\">"));
        assert!(!numbering.contains("<w:num w:numId=\"4\">"));
    }

    #[test]
    fn a_content_block_is_a_shaded_note_and_is_not_listed() {
        let doc = document(&DocxOptions::default());
        assert!(doc.contains("<w:pStyle w:val=\"ExpressNote\"/><w:keepNext/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t xml:space=\"preserve\">Tip</w:t>"));
        assert!(!texts(&doc, "w:t").iter().any(|t| t.contains(". Tip")), "a note has no number");
    }

    #[test]
    fn the_authors_words_cannot_break_the_file() {
        let mut d = demo();
        d.document.guide.title = "A <b>&</b> \"title\" \u{7} with a bell".to_string();
        d.document.guide.steps[0].title = "]]> <script>alert(1)</script>".to_string();
        d.document.guide.steps[0].description =
            "**<b>bold</b>** & [x](https://example.com/?a=1&b=2) and [bad](javascript:alert(1))".to_string();
        d.document.guide.steps[1].image_description = "x\" onerror=\"alert(3) <>".to_string();
        let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let z = unzip(&export_docx(&p, &DocxOptions::default()).unwrap().files[0].bytes);
        for name in z.names.iter().filter(|n| n.ends_with(".xml") || n.ends_with(".rels")) {
            well_formed(name, &z.text(name));
        }
        let doc = z.text("word/document.xml");
        assert!(
            texts(&doc, "w:t").iter().any(|t| t == "A <b>&</b> \"title\"  with a bell"),
            "{:?}",
            texts(&doc, "w:t")
        );
        assert!(texts(&doc, "w:t").iter().any(|t| t == "1. ]]> <script>alert(1)</script>"));
        assert!(attributes(&doc, "wp:docPr", "descr").iter().any(|a| a == "x\" onerror=\"alert(3) <>"));
        assert!(!doc.contains("<script"));
        assert!(z.text("word/_rels/document.xml.rels").contains("Target=\"https://example.com/?a=1&amp;b=2\""));
        assert!(!z.text("word/_rels/document.xml.rels").contains("javascript"), "an unsafe address is never a link");
        assert!(texts(&doc, "w:t").iter().any(|t| t.contains("[bad](javascript:alert(1))")), "it stays as plain text");
        assert_eq!(texts(&z.text("docProps/core.xml"), "dc:title"), ["A <b>&</b> \"title\"  with a bell"]);
    }

    #[test]
    fn the_document_says_what_language_it_is_in() {
        let z = make(&DocxOptions::default());
        assert!(z.text("word/styles.xml").contains("<w:lang w:val=\"en-US\""));
        assert_eq!(texts(&z.text("docProps/core.xml"), "dc:language"), ["en-US"]);
        let mut d = demo();
        for (code, tag, page, of, contents) in
            [("de", "de-DE", "Seite", "von", "Inhalt"), ("fr", "fr-FR", "Page", "sur", "Sommaire")]
        {
            d.document.guide.language = code.to_string();
            let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
            let z = unzip(&export_docx(&p, &DocxOptions::default()).unwrap().files[0].bytes);
            assert!(z.text("word/styles.xml").contains(&format!("<w:lang w:val=\"{tag}\"")), "{code}");
            assert_eq!(texts(&z.text("word/footer1.xml"), "w:t")[0], format!("{page} "), "{code}");
            assert!(texts(&z.text("word/footer1.xml"), "w:t").iter().any(|t| t == &format!(" {of} ")), "{code}");
            assert!(texts(&z.text("word/document.xml"), "w:t").iter().any(|t| t == contents), "{code}");
        }
    }

    #[test]
    fn page_numbers_are_a_footer_with_page_fields() {
        let z = make(&DocxOptions::default());
        let footer = z.text("word/footer1.xml");
        assert!(footer.contains("> PAGE <") && footer.contains("> NUMPAGES <"));
        assert!(z.text("word/document.xml").contains("<w:footerReference w:type=\"default\" r:id=\""));
        let none = make(&DocxOptions { page_numbers: false, ..DocxOptions::default() });
        assert!(none.get("word/footer1.xml").is_none());
        assert!(!none.text("word/document.xml").contains("footerReference"));
        assert!(!none.text("[Content_Types].xml").contains("footer"));
    }

    #[test]
    fn paper_orientation_and_margins() {
        let a4 = document(&DocxOptions::default());
        assert!(a4.contains("<w:pgSz w:w=\"11906\" w:h=\"16838\"/>"));
        assert!(a4.contains("w:top=\"1417\" w:right=\"1417\" w:bottom=\"1417\" w:left=\"1417\""), "25 mm");
        let letter = document(&DocxOptions {
            page_size: PageSize::Letter,
            orientation: Orientation::Landscape,
            margin_mm: 15,
            ..DocxOptions::default()
        });
        assert!(letter.contains("<w:pgSz w:w=\"15840\" w:h=\"12240\" w:orient=\"landscape\"/>"));
        assert!(letter.contains("w:top=\"850\""));
        assert!(
            document(&DocxOptions { margin_mm: 0, ..DocxOptions::default() }).contains("w:top=\"567\""),
            "10 mm at least"
        );
        assert!(
            document(&DocxOptions { margin_mm: 900, ..DocxOptions::default() }).contains("w:top=\"2268\""),
            "40 mm at most"
        );
        for (size, w, h) in [(PageSize::A3, 16838, 23811), (PageSize::A5, 8391, 11906), (PageSize::Legal, 12240, 20160)]
        {
            let doc = document(&DocxOptions { page_size: size, ..DocxOptions::default() });
            assert!(doc.contains(&format!("<w:pgSz w:w=\"{w}\" w:h=\"{h}\"/>")), "{size:?}");
        }
    }

    #[test]
    fn a_new_page_for_every_top_level_step() {
        assert!(!document(&DocxOptions::default()).contains("pageBreakBefore"));
        let doc = document(&DocxOptions { new_page_per_step: true, ..DocxOptions::default() });
        assert_eq!(doc.matches("<w:pageBreakBefore/>").count(), 4, "steps 1 to 4, and not the substep");
    }

    #[test]
    fn a_step_stays_with_its_picture() {
        let doc = document(&DocxOptions::default());
        // The heading style keeps with the next paragraph, and each description paragraph that comes before a picture does too.
        assert!(doc.contains(
            "<w:pPr><w:keepNext/></w:pPr><w:r><w:t xml:space=\"preserve\">Start here: this opens the form.</w:t>"
        ));
    }

    #[test]
    fn the_file_is_named_after_the_guide_and_is_the_same_every_time() {
        let p = demo_plan();
        let export = export_docx(&p, &DocxOptions::default()).unwrap();
        assert_eq!(export.files.len(), 1);
        assert_eq!(export.files[0].path, "How to submit an assignment.docx");
        assert_eq!(export, export_docx(&p, &DocxOptions::default()).unwrap());
        assert!(export.files[0].bytes.len() > 20_000, "the pictures are inside");
        assert_eq!(&export.files[0].bytes[..2], b"PK");
    }

    #[test]
    fn the_plans_warnings_come_through_and_a_missing_picture_leaves_the_step() {
        let d = demo();
        let mut pictures = d.pictures;
        pictures.0.remove("images/step-0002.png");
        let p = plan(&d.document.guide, &pictures, &BlockText, &ExportOptions::default());
        let export = export_docx(&p, &DocxOptions::default()).unwrap();
        assert!(export.warnings.iter().any(|w| w.contains("images/step-0002.png")), "{:?}", export.warnings);
        let z = unzip(&export.files[0].bytes);
        assert_eq!(z.names.iter().filter(|n| n.starts_with("word/media/")).count(), 4);
        assert!(z.text("word/document.xml").contains("2. Click &quot;New assignment&quot;"));
    }

    #[test]
    fn an_empty_guide_is_still_a_valid_document() {
        let guide = openstage_express_core::Guide::new("g", "Nothing yet", "en", "t");
        let p = plan(&guide, &crate::picture::MemoryPictures::default(), &BlockText, &ExportOptions::default());
        let z = unzip(&export_docx(&p, &DocxOptions::default()).unwrap().files[0].bytes);
        for name in z.names.iter().filter(|n| n.ends_with(".xml") || n.ends_with(".rels")) {
            well_formed(name, &z.text(name));
        }
        let doc = z.text("word/document.xml");
        assert!(doc.contains("Nothing yet") && !doc.contains("fldChar") && doc.contains("<w:sectPr>"));
    }

    #[test]
    fn deeper_substeps_stay_at_heading_three() {
        assert_eq!((heading_level(0), heading_level(1), heading_level(2), heading_level(7)), (1, 2, 3, 3));
        assert_eq!(heading_text(Some("2.1"), "Save"), "2.1. Save");
        assert_eq!(bookmark_name(7), "_Toc100000007");
    }
}
