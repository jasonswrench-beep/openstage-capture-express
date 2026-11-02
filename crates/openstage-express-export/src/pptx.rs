// SPDX-License-Identifier: Apache-2.0
//! The PowerPoint export (spec section 10.2): a `.pptx` file written as Office Open XML by hand.
//!
//! * One 16:9 slide for each step: the step's number and title as the slide's title (a real title placeholder, so the outline,
//!   the accessibility checker and screen readers see it), the picture below it with its alt text, and the first lines of the
//!   description under the picture. The *full* description goes in the speaker notes.
//! * Optionally a cover slide (the guide's title and description) and agenda slides (the top-level steps, at most
//!   [`AGENDA_ROWS`] to a slide), a footer line, and slide numbers.
//! * The reading order is the order things are written in: title, picture, description, footer, slide number.
//! * Fonts are Calibri (PowerPoint's own), not embedded; see `docx` for why.

use openstage_express_core::fsutil::sanitise_folder_name;
use openstage_express_core::{Language, StepKind};

use crate::encoding::markdown_to_plain_lines;
use crate::office::{
    app_properties, content_types, core_properties, fit_picture, lang_tag, package_rels, xml_escape, OfficeError, Rels,
};
use crate::pdf::words;
use crate::plan::{Plan, PlannedStep};
use crate::zip::{self, ZipEntry};
use crate::{Export, OutFile};

/// The most steps on one agenda slide; a longer agenda continues on the next slide.
pub const AGENDA_ROWS: usize = 10;
/// The most lines of description shown on a slide that has a picture; the notes hold all of it.
const SLIDE_LINES: usize = 2;
const SLIDE_CHARS: usize = 240;

const NS: &str = "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"";
const GROUP: &str = "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>";
const CLR_MAP: &str = "<p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/>";
const XML_HEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

const SLIDE_W: i64 = 12_192_000;
const SLIDE_H: i64 = 6_858_000;
const MARGIN: i64 = 548_640; // 0.6 inch
const CONTENT_W: i64 = SLIDE_W - 2 * MARGIN;
const PICTURE_Y: i64 = 1_280_160; // 1.4 inch
const PICTURE_H_WITH_TEXT: i64 = 4_297_680; // 4.7 inch
const PICTURE_H_ALONE: i64 = 4_937_760; // 5.4 inch
const TEXT_Y: i64 = 5_669_280; // 6.2 inch
const TEXT_H: i64 = 685_800; // 0.75 inch

/// What the person chose in the PowerPoint export dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PptxOptions {
    /// A first slide with the guide's title and description.
    pub cover_slide: bool,
    /// A slide (or more) listing the top-level steps.
    pub agenda_slide: bool,
    /// Shown at the foot of every slide but the cover; empty for none.
    pub footer_text: String,
    pub slide_numbers: bool,
    /// The full description of each step in its speaker notes.
    pub speaker_notes: bool,
}

impl Default for PptxOptions {
    fn default() -> Self {
        PptxOptions {
            cover_slide: true,
            agenda_slide: false,
            footer_text: String::new(),
            slide_numbers: true,
            speaker_notes: true,
        }
    }
}

fn text_run(text: &str, lang: &str, size: Option<u32>) -> String {
    let size = size.map(|s| format!(" sz=\"{s}\"")).unwrap_or_default();
    format!("<a:r><a:rPr lang=\"{lang}\"{size} dirty=\"0\"/><a:t>{}</a:t></a:r>", xml_escape(text))
}

fn paragraphs(lines: &[String], lang: &str, size: Option<u32>, plain: bool) -> String {
    if lines.is_empty() {
        return format!("<a:p><a:endParaRPr lang=\"{lang}\"/></a:p>");
    }
    let props = if plain { "<a:pPr marL=\"0\" indent=\"0\"><a:buNone/></a:pPr>" } else { "" };
    lines.iter().map(|line| format!("<a:p>{props}{}</a:p>", text_run(line, lang, size))).collect()
}

fn placeholder(id: u32, name: &str, ph: &str, lines: &[String], lang: &str, plain: bool) -> String {
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph {ph}/></p:nvPr></p:nvSpPr><p:spPr/>\
<p:txBody><a:bodyPr/><a:lstStyle/>{}</p:txBody></p:sp>",
        paragraphs(lines, lang, None, plain)
    )
}

/// Where a shape sits on a slide, in EMU.
#[derive(Clone, Copy)]
struct Frame {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
}

fn text_box(id: u32, name: &str, frame: Frame, lines: &[String], lang: &str, size: u32) -> String {
    let Frame { x, y, cx, cy } = frame;
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
<p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/></p:spPr>\
<p:txBody><a:bodyPr wrap=\"square\" rtlCol=\"0\" anchor=\"t\"><a:normAutofit/></a:bodyPr><a:lstStyle/>{}</p:txBody></p:sp>",
        paragraphs(lines, lang, Some(size), false)
    )
}

fn slide_number(id: u32, number: usize, lang: &str) -> String {
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"Slide Number Placeholder {id}\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"sldNum\" sz=\"quarter\" idx=\"12\"/></p:nvPr></p:nvSpPr><p:spPr/>\
<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:fld id=\"{{B6F15528-21DE-4FAA-801E-634DDDAF4B2B}}\" type=\"slidenum\"><a:rPr lang=\"{lang}\"/><a:t>{number}</a:t></a:fld><a:endParaRPr lang=\"{lang}\"/></a:p></p:txBody></p:sp>"
    )
}

/// The description as it is shown on a slide: its first lines, cut short with an ellipsis if there is more.
fn slide_lines(description: &str) -> Vec<String> {
    let all = markdown_to_plain_lines(description);
    let mut lines: Vec<String> = all.iter().take(SLIDE_LINES).cloned().collect();
    let mut cut = all.len() > SLIDE_LINES;
    let mut used = 0usize;
    for line in lines.iter_mut() {
        let room = SLIDE_CHARS.saturating_sub(used);
        if line.chars().count() > room {
            *line = line.chars().take(room.saturating_sub(1)).collect::<String>().trim_end().to_string() + "\u{2026}";
            cut = false;
            used = SLIDE_CHARS;
        } else {
            used += line.chars().count();
        }
    }
    if cut {
        if let Some(last) = lines.last_mut() {
            last.push('\u{2026}');
        }
    }
    lines
}

fn theme_xml() -> String {
    let line = |w: u32| {
        format!("<a:ln w=\"{w}\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/></a:ln>")
    };
    format!(
        "{XML_HEAD}<a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" name=\"Express\"><a:themeElements>\
<a:clrScheme name=\"Express\"><a:dk1><a:srgbClr val=\"1B1F33\"/></a:dk1><a:lt1><a:srgbClr val=\"FFFFFF\"/></a:lt1><a:dk2><a:srgbClr val=\"4A5068\"/></a:dk2><a:lt2><a:srgbClr val=\"F1F3FA\"/></a:lt2>\
<a:accent1><a:srgbClr val=\"2B3AB8\"/></a:accent1><a:accent2><a:srgbClr val=\"C2410C\"/></a:accent2><a:accent3><a:srgbClr val=\"0F766E\"/></a:accent3><a:accent4><a:srgbClr val=\"7C3AED\"/></a:accent4>\
<a:accent5><a:srgbClr val=\"B45309\"/></a:accent5><a:accent6><a:srgbClr val=\"BE185D\"/></a:accent6><a:hlink><a:srgbClr val=\"2B3AB8\"/></a:hlink><a:folHlink><a:srgbClr val=\"6B21A8\"/></a:folHlink></a:clrScheme>\
<a:fontScheme name=\"Express\"><a:majorFont><a:latin typeface=\"Calibri\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:majorFont><a:minorFont><a:latin typeface=\"Calibri\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:minorFont></a:fontScheme>\
<a:fmtScheme name=\"Express\"><a:fillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:fillStyleLst>\
<a:lnStyleLst>{}{}{}</a:lnStyleLst>\
<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>\
<a:bgFillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme>\
</a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>",
        line(6350),
        line(12700),
        line(19050)
    )
}

fn master_placeholder(id: u32, name: &str, ph: &str, frame: Frame, anchor: &str, sample: &str, algn: &str) -> String {
    let Frame { x, y, cx, cy } = frame;
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph {ph}/></p:nvPr></p:nvSpPr>\
<p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
<p:txBody><a:bodyPr vert=\"horz\" lIns=\"91440\" tIns=\"45720\" rIns=\"91440\" bIns=\"45720\" rtlCol=\"0\" anchor=\"{anchor}\"><a:normAutofit/></a:bodyPr>\
<a:lstStyle><a:lvl1pPr algn=\"{algn}\"><a:buNone/><a:defRPr sz=\"1200\"><a:solidFill><a:schemeClr val=\"tx2\"/></a:solidFill></a:defRPr></a:lvl1pPr></a:lstStyle>\
<a:p><a:r><a:rPr lang=\"en-US\"/><a:t>{sample}</a:t></a:r></a:p></p:txBody></p:sp>"
    )
}

fn master_xml() -> String {
    let title = format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Title Placeholder 1\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"title\"/></p:nvPr></p:nvSpPr>\
<p:spPr><a:xfrm><a:off x=\"{MARGIN}\" y=\"320040\"/><a:ext cx=\"{CONTENT_W}\" cy=\"868680\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
<p:txBody><a:bodyPr vert=\"horz\" lIns=\"91440\" tIns=\"45720\" rIns=\"91440\" bIns=\"45720\" rtlCol=\"0\" anchor=\"ctr\"><a:normAutofit/></a:bodyPr><a:lstStyle/>\
<a:p><a:r><a:rPr lang=\"en-US\"/><a:t>Click to edit Master title style</a:t></a:r></a:p></p:txBody></p:sp>"
    );
    let body = format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"3\" name=\"Text Placeholder 2\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"body\" idx=\"1\"/></p:nvPr></p:nvSpPr>\
<p:spPr><a:xfrm><a:off x=\"{MARGIN}\" y=\"1371600\"/><a:ext cx=\"{CONTENT_W}\" cy=\"4754880\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
<p:txBody><a:bodyPr vert=\"horz\" lIns=\"91440\" tIns=\"45720\" rIns=\"91440\" bIns=\"45720\" rtlCol=\"0\"><a:normAutofit/></a:bodyPr><a:lstStyle/>\
<a:p><a:pPr lvl=\"0\"/><a:r><a:rPr lang=\"en-US\"/><a:t>Click to edit Master text styles</a:t></a:r></a:p></p:txBody></p:sp>"
    );
    let footer = master_placeholder(
        4,
        "Footer Placeholder 3",
        "type=\"ftr\" sz=\"quarter\" idx=\"3\"",
        Frame { x: MARGIN, y: 6_400_800, cx: 7_315_200, cy: 365_760 },
        "ctr",
        "Footer",
        "l",
    );
    let number = String::from(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"5\" name=\"Slide Number Placeholder 4\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"sldNum\" sz=\"quarter\" idx=\"4\"/></p:nvPr></p:nvSpPr>\
<p:spPr><a:xfrm><a:off x=\"9326880\" y=\"6400800\"/><a:ext cx=\"2316480\" cy=\"365760\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
<p:txBody><a:bodyPr vert=\"horz\" lIns=\"91440\" tIns=\"45720\" rIns=\"91440\" bIns=\"45720\" rtlCol=\"0\" anchor=\"ctr\"/>\
<a:lstStyle><a:lvl1pPr algn=\"r\"><a:defRPr sz=\"1200\"><a:solidFill><a:schemeClr val=\"tx2\"/></a:solidFill></a:defRPr></a:lvl1pPr></a:lstStyle>\
<a:p><a:fld id=\"{B6F15528-21DE-4FAA-801E-634DDDAF4B2B}\" type=\"slidenum\"><a:rPr lang=\"en-US\"/><a:t>\u{2039}#\u{203a}</a:t></a:fld><a:endParaRPr lang=\"en-US\"/></a:p></p:txBody></p:sp>"
    );
    let level = |n: u32, marl: u32, size: u32| {
        format!(
            "<a:lvl{n}pPr marL=\"{marl}\" indent=\"-228600\" algn=\"l\" defTabSz=\"914400\" rtl=\"0\" eaLnBrk=\"1\" latinLnBrk=\"0\" hangingPunct=\"1\"><a:lnSpc><a:spcPct val=\"100000\"/></a:lnSpc><a:spcBef><a:spcPts val=\"600\"/></a:spcBef>\
<a:buFont typeface=\"Arial\"/><a:buChar char=\"&#8226;\"/><a:defRPr sz=\"{size}\" kern=\"1200\"><a:solidFill><a:schemeClr val=\"tx1\"/></a:solidFill><a:latin typeface=\"+mn-lt\"/><a:ea typeface=\"+mn-ea\"/><a:cs typeface=\"+mn-cs\"/></a:defRPr></a:lvl{n}pPr>"
        )
    };
    format!(
        "{XML_HEAD}<p:sldMaster {NS}><p:cSld><p:bg><p:bgRef idx=\"1001\"><a:schemeClr val=\"bg1\"/></p:bgRef></p:bg><p:spTree>{GROUP}{title}{body}{footer}{number}</p:spTree></p:cSld>{CLR_MAP}\
<p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/><p:sldLayoutId id=\"2147483650\" r:id=\"rId2\"/><p:sldLayoutId id=\"2147483651\" r:id=\"rId3\"/></p:sldLayoutIdLst>\
<p:txStyles><p:titleStyle><a:lvl1pPr algn=\"l\" defTabSz=\"914400\" rtl=\"0\" eaLnBrk=\"1\" latinLnBrk=\"0\" hangingPunct=\"1\"><a:lnSpc><a:spcPct val=\"90000\"/></a:lnSpc><a:spcBef><a:spcPct val=\"0\"/></a:spcBef><a:buNone/>\
<a:defRPr sz=\"3200\" b=\"1\" kern=\"1200\"><a:solidFill><a:schemeClr val=\"tx1\"/></a:solidFill><a:latin typeface=\"+mj-lt\"/><a:ea typeface=\"+mj-ea\"/><a:cs typeface=\"+mj-cs\"/></a:defRPr></a:lvl1pPr></p:titleStyle>\
<p:bodyStyle>{}{}{}</p:bodyStyle>\
<p:otherStyle><a:defPPr><a:defRPr lang=\"en-US\"/></a:defPPr><a:lvl1pPr marL=\"0\" algn=\"l\" defTabSz=\"914400\" rtl=\"0\" eaLnBrk=\"1\" latinLnBrk=\"0\" hangingPunct=\"1\"><a:defRPr sz=\"1800\" kern=\"1200\"><a:solidFill><a:schemeClr val=\"tx1\"/></a:solidFill><a:latin typeface=\"+mn-lt\"/><a:ea typeface=\"+mn-ea\"/><a:cs typeface=\"+mn-cs\"/></a:defRPr></a:lvl1pPr></p:otherStyle>\
</p:txStyles></p:sldMaster>",
        level(1, 228_600, 2400),
        level(2, 685_800, 2000),
        level(3, 1_143_000, 1800)
    )
}

fn layout_xml(kind: &str, name: &str, shapes: &str) -> String {
    format!(
        "{XML_HEAD}<p:sldLayout {NS} type=\"{kind}\" preserve=\"1\"><p:cSld name=\"{name}\"><p:spTree>{GROUP}{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"
    )
}

fn layout_title_placeholder() -> String {
    "<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Title 1\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"title\"/></p:nvPr></p:nvSpPr><p:spPr/>\
<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\"/><a:t>Click to edit Master title style</a:t></a:r></a:p></p:txBody></p:sp>"
        .to_string()
}

fn layout_footers(first_id: u32) -> String {
    let a = first_id;
    let b = first_id + 1;
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{a}\" name=\"Footer Placeholder {a}\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"ftr\" sz=\"quarter\" idx=\"11\"/></p:nvPr></p:nvSpPr><p:spPr/>\
<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang=\"en-US\"/></a:p></p:txBody></p:sp>\
<p:sp><p:nvSpPr><p:cNvPr id=\"{b}\" name=\"Slide Number Placeholder {b}\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"sldNum\" sz=\"quarter\" idx=\"12\"/></p:nvPr></p:nvSpPr><p:spPr/>\
<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:fld id=\"{{B6F15528-21DE-4FAA-801E-634DDDAF4B2B}}\" type=\"slidenum\"><a:rPr lang=\"en-US\"/><a:t>\u{2039}#\u{203a}</a:t></a:fld><a:endParaRPr lang=\"en-US\"/></a:p></p:txBody></p:sp>"
    )
}

fn layouts() -> [String; 3] {
    let cover = "<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Title 1\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"ctrTitle\"/></p:nvPr></p:nvSpPr>\
<p:spPr><a:xfrm><a:off x=\"914400\" y=\"2130425\"/><a:ext cx=\"10363200\" cy=\"1470025\"/></a:xfrm></p:spPr>\
<p:txBody><a:bodyPr anchor=\"b\"/><a:lstStyle><a:lvl1pPr algn=\"l\"><a:defRPr sz=\"4400\"/></a:lvl1pPr></a:lstStyle><a:p><a:r><a:rPr lang=\"en-US\"/><a:t>Click to edit Master title style</a:t></a:r></a:p></p:txBody></p:sp>\
<p:sp><p:nvSpPr><p:cNvPr id=\"3\" name=\"Subtitle 2\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"subTitle\" idx=\"1\"/></p:nvPr></p:nvSpPr>\
<p:spPr><a:xfrm><a:off x=\"914400\" y=\"3886200\"/><a:ext cx=\"10363200\" cy=\"1752600\"/></a:xfrm></p:spPr>\
<p:txBody><a:bodyPr/><a:lstStyle><a:lvl1pPr marL=\"0\" indent=\"0\" algn=\"l\"><a:buNone/><a:defRPr sz=\"2400\"><a:solidFill><a:schemeClr val=\"tx2\"/></a:solidFill></a:defRPr></a:lvl1pPr></a:lstStyle>\
<a:p><a:r><a:rPr lang=\"en-US\"/><a:t>Click to edit Master subtitle style</a:t></a:r></a:p></p:txBody></p:sp>";
    let title_only = format!("{}{}", layout_title_placeholder(), layout_footers(3));
    let body = "<p:sp><p:nvSpPr><p:cNvPr id=\"3\" name=\"Content Placeholder 2\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph idx=\"1\"/></p:nvPr></p:nvSpPr><p:spPr/>\
<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:pPr lvl=\"0\"/><a:r><a:rPr lang=\"en-US\"/><a:t>Click to edit Master text styles</a:t></a:r></a:p></p:txBody></p:sp>";
    let content = format!("{}{}{}", layout_title_placeholder(), body, layout_footers(4));
    [
        layout_xml("title", "Title Slide", cover),
        layout_xml("titleOnly", "Title Only", &title_only),
        layout_xml("obj", "Title and Content", &content),
    ]
}

fn notes_master_xml() -> String {
    format!(
        "{XML_HEAD}<p:notesMaster {NS}><p:cSld><p:bg><p:bgRef idx=\"1001\"><a:schemeClr val=\"bg1\"/></p:bgRef></p:bg><p:spTree>{GROUP}\
<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Slide Image Placeholder 1\"/><p:cNvSpPr><a:spLocks noGrp=\"1\" noRot=\"1\" noChangeAspect=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"sldImg\" idx=\"2\"/></p:nvPr></p:nvSpPr>\
<p:spPr><a:xfrm><a:off x=\"685800\" y=\"1143000\"/><a:ext cx=\"5486400\" cy=\"3086100\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/><a:ln w=\"12700\"><a:solidFill><a:prstClr val=\"black\"/></a:solidFill></a:ln></p:spPr></p:sp>\
<p:sp><p:nvSpPr><p:cNvPr id=\"3\" name=\"Notes Placeholder 2\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"body\" sz=\"quarter\" idx=\"3\"/></p:nvPr></p:nvSpPr>\
<p:spPr><a:xfrm><a:off x=\"685800\" y=\"4400550\"/><a:ext cx=\"5486400\" cy=\"3600450\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
<p:txBody><a:bodyPr vert=\"horz\" lIns=\"91440\" tIns=\"45720\" rIns=\"91440\" bIns=\"45720\" rtlCol=\"0\"/><a:lstStyle/><a:p><a:pPr lvl=\"0\"/><a:r><a:rPr lang=\"en-US\"/><a:t>Click to edit Master text styles</a:t></a:r></a:p></p:txBody></p:sp>\
</p:spTree></p:cSld>{CLR_MAP}<p:notesStyle><a:lvl1pPr marL=\"0\" algn=\"l\" defTabSz=\"914400\" rtl=\"0\" eaLnBrk=\"1\" latinLnBrk=\"0\" hangingPunct=\"1\"><a:defRPr sz=\"1200\" kern=\"1200\"><a:solidFill><a:schemeClr val=\"tx1\"/></a:solidFill><a:latin typeface=\"+mn-lt\"/><a:ea typeface=\"+mn-ea\"/><a:cs typeface=\"+mn-cs\"/></a:defRPr></a:lvl1pPr></p:notesStyle></p:notesMaster>"
    )
}

fn notes_slide_xml(lines: &[String], lang: &str) -> String {
    format!(
        "{XML_HEAD}<p:notes {NS}><p:cSld><p:spTree>{GROUP}\
<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Slide Image Placeholder 1\"/><p:cNvSpPr><a:spLocks noGrp=\"1\" noRot=\"1\" noChangeAspect=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"sldImg\"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp>\
<p:sp><p:nvSpPr><p:cNvPr id=\"3\" name=\"Notes Placeholder 2\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"body\" idx=\"1\"/></p:nvPr></p:nvSpPr><p:spPr/>\
<p:txBody><a:bodyPr/><a:lstStyle/>{}</p:txBody></p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>",
        paragraphs(lines, lang, None, false)
    )
}

/// One slide, before it is written: what is on it and which layout it uses.
struct SlideSpec {
    layout: u32,
    shapes: String,
    notes: Vec<String>,
    image: Option<(String, Vec<u8>)>,
}

fn slide_xml(shapes: &str) -> String {
    format!("{XML_HEAD}<p:sld {NS}><p:cSld><p:spTree>{GROUP}{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>")
}

fn step_title(step: &PlannedStep) -> String {
    match (&step.number, step.kind) {
        (Some(number), k) if k != StepKind::Block => format!("{number}. {}", step.title),
        _ => step.title.clone(),
    }
}

/// The parts of a PowerPoint file for `plan`.
pub fn pptx_parts(plan: &Plan, options: &PptxOptions) -> Vec<ZipEntry> {
    let language = Language::from_code(&plan.language);
    let lang = lang_tag(&plan.language);
    let w = words(language);
    let footer_lines: Vec<String> =
        if options.footer_text.trim().is_empty() { vec![] } else { vec![options.footer_text.trim().to_string()] };
    let mut specs: Vec<SlideSpec> = Vec::new();

    // Slide numbers are the position in the file, so every slide knows its own.
    let furniture = |id: &mut u32, number: usize| -> String {
        let mut out = String::new();
        if !footer_lines.is_empty() {
            out.push_str(&placeholder(
                *id,
                &format!("Footer Placeholder {id}"),
                "type=\"ftr\" sz=\"quarter\" idx=\"11\"",
                &footer_lines,
                &lang,
                false,
            ));
            *id += 1;
        }
        if options.slide_numbers {
            out.push_str(&slide_number(*id, number, &lang));
            *id += 1;
        }
        out
    };

    if options.cover_slide {
        let description = markdown_to_plain_lines(&plan.description);
        let shapes = format!(
            "{}{}",
            placeholder(2, "Title 1", "type=\"ctrTitle\"", std::slice::from_ref(&plan.title), &lang, false),
            placeholder(3, "Subtitle 2", "type=\"subTitle\" idx=\"1\"", &description, &lang, true)
        );
        specs.push(SlideSpec { layout: 1, shapes, notes: vec![], image: None });
    }

    let top_level: Vec<&PlannedStep> =
        plan.steps.iter().filter(|s| s.depth == 0 && s.kind != StepKind::Block).collect();
    if options.agenda_slide && !top_level.is_empty() {
        let pages: Vec<&[&PlannedStep]> = top_level.chunks(AGENDA_ROWS).collect();
        for (n, chunk) in pages.iter().enumerate() {
            let title = if n == 0 { w.contents.to_string() } else { format!("{} ({})", w.contents, n + 1) };
            let rows: Vec<String> = chunk.iter().map(|s| step_title(s)).collect();
            let number = specs.len() + 1;
            let mut id = 4;
            let shapes = format!(
                "{}{}{}",
                placeholder(2, "Title 1", "type=\"title\"", &[title], &lang, false),
                placeholder(3, "Content Placeholder 2", "idx=\"1\"", &rows, &lang, true),
                furniture(&mut id, number)
            );
            specs.push(SlideSpec { layout: 3, shapes, notes: vec![], image: None });
        }
    }

    let mut image_count = 0;
    for step in &plan.steps {
        let number = specs.len() + 1;
        let title = step_title(step);
        let all_lines = markdown_to_plain_lines(&step.description);
        let mut id = 2;
        let mut shapes = placeholder(id, "Title 1", "type=\"title\"", &[title], &lang, false);
        id += 1;
        let mut image = None;
        match &step.picture {
            Some(picture) if step.kind != StepKind::Block => {
                let lines = slide_lines(&step.description);
                let has_text = !lines.is_empty();
                let area_h = if has_text { PICTURE_H_WITH_TEXT } else { PICTURE_H_ALONE };
                let (cx, cy) = fit_picture(picture.width, picture.height, CONTENT_W, area_h);
                let x = MARGIN + (CONTENT_W - cx) / 2;
                image_count += 1;
                let alt = xml_escape(&step.alt);
                shapes.push_str(&format!(
                    "<p:pic><p:nvPicPr><p:cNvPr id=\"{id}\" name=\"Picture {image_count}\" descr=\"{alt}\"/><p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>\
<p:blipFill><a:blip r:embed=\"@IMAGE@\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>\
<p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{PICTURE_Y}\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>"
                ));
                id += 1;
                if has_text {
                    let frame = Frame { x: MARGIN, y: TEXT_Y, cx: CONTENT_W, cy: TEXT_H };
                    shapes.push_str(&text_box(id, "Description", frame, &lines, &lang, 1800));
                    id += 1;
                }
                let embedded = picture.embedded();
                image = Some((format!("ppt/media/image{image_count}.{}", embedded.extension), embedded.bytes.to_vec()));
            }
            _ => {
                if !all_lines.is_empty() {
                    let frame = Frame { x: MARGIN, y: PICTURE_Y, cx: CONTENT_W, cy: PICTURE_H_ALONE };
                    shapes.push_str(&text_box(id, "Description", frame, &all_lines, &lang, 2400));
                    id += 1;
                }
            }
        }
        shapes.push_str(&furniture(&mut id, number));
        let notes = if options.speaker_notes { all_lines } else { vec![] };
        specs.push(SlideSpec { layout: 2, shapes, notes, image });
    }

    // Write the parts. Relationship ids follow the order things are added, and the XML above leaves `@IMAGE@` where a slide
    // names its picture.
    let mut overrides: Vec<(String, &str)> = vec![
        (
            "/ppt/presentation.xml".into(),
            "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
        ),
        (
            "/ppt/slideMasters/slideMaster1.xml".into(),
            "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml",
        ),
        (
            "/ppt/notesMasters/notesMaster1.xml".into(),
            "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml",
        ),
        ("/ppt/theme/theme1.xml".into(), "application/vnd.openxmlformats-officedocument.theme+xml"),
        ("/ppt/theme/theme2.xml".into(), "application/vnd.openxmlformats-officedocument.theme+xml"),
        ("/docProps/core.xml".into(), "application/vnd.openxmlformats-package.core-properties+xml"),
        ("/docProps/app.xml".into(), "application/vnd.openxmlformats-officedocument.extended-properties+xml"),
    ];
    for n in 1..=3 {
        overrides.push((
            format!("/ppt/slideLayouts/slideLayout{n}.xml"),
            "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml",
        ));
    }

    let mut presentation_rels = Rels::default();
    presentation_rels.add("slideMaster", "slideMasters/slideMaster1.xml", false);
    let notes_master_rel = presentation_rels.add("notesMaster", "notesMasters/notesMaster1.xml", false);
    presentation_rels.add("theme", "theme/theme1.xml", false);

    let mut slide_parts: Vec<ZipEntry> = Vec::new();
    let mut slide_ids = String::new();
    for (n, spec) in specs.iter().enumerate() {
        let n = n + 1;
        let slide_rel = presentation_rels.add("slide", &format!("slides/slide{n}.xml"), false);
        slide_ids.push_str(&format!("<p:sldId id=\"{}\" r:id=\"{slide_rel}\"/>", 255 + n));
        overrides.push((
            format!("/ppt/slides/slide{n}.xml"),
            "application/vnd.openxmlformats-officedocument.presentationml.slide+xml",
        ));
        let mut rels = Rels::default();
        rels.add("slideLayout", &format!("../slideLayouts/slideLayout{}.xml", spec.layout), false);
        let mut shapes = spec.shapes.clone();
        if let Some((path, bytes)) = &spec.image {
            let id = rels.add("image", &format!("../media/{}", path.trim_start_matches("ppt/media/")), false);
            shapes = shapes.replace("@IMAGE@", &id);
            slide_parts.push(ZipEntry::stored(path.clone(), bytes.clone()));
        }
        if !spec.notes.is_empty() {
            rels.add("notesSlide", &format!("../notesSlides/notesSlide{n}.xml"), false);
            overrides.push((
                format!("/ppt/notesSlides/notesSlide{n}.xml"),
                "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml",
            ));
            let mut notes_rels = Rels::default();
            notes_rels.add("notesMaster", "../notesMasters/notesMaster1.xml", false);
            notes_rels.add("slide", &format!("../slides/slide{n}.xml"), false);
            slide_parts.push(ZipEntry::text(
                format!("ppt/notesSlides/notesSlide{n}.xml"),
                notes_slide_xml(&spec.notes, &lang),
            ));
            slide_parts.push(ZipEntry::text(format!("ppt/notesSlides/_rels/notesSlide{n}.xml.rels"), notes_rels.xml()));
        }
        slide_parts.push(ZipEntry::text(format!("ppt/slides/slide{n}.xml"), slide_xml(&shapes)));
        slide_parts.push(ZipEntry::text(format!("ppt/slides/_rels/slide{n}.xml.rels"), rels.xml()));
    }

    let presentation = format!(
        "{XML_HEAD}<p:presentation {NS} saveSubsetFonts=\"1\"><p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst>\
<p:notesMasterIdLst><p:notesMasterId r:id=\"{notes_master_rel}\"/></p:notesMasterIdLst><p:sldIdLst>{slide_ids}</p:sldIdLst>\
<p:sldSz cx=\"{SLIDE_W}\" cy=\"{SLIDE_H}\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/></p:presentation>"
    );

    let mut master_rels = Rels::default();
    for n in 1..=3 {
        master_rels.add("slideLayout", &format!("../slideLayouts/slideLayout{n}.xml"), false);
    }
    master_rels.add("theme", "../theme/theme1.xml", false);
    let mut layout_rels = Rels::default();
    layout_rels.add("slideMaster", "../slideMasters/slideMaster1.xml", false);
    let mut notes_master_rels = Rels::default();
    notes_master_rels.add("theme", "../theme/theme2.xml", false);

    let override_refs: Vec<(&str, &str)> = overrides.iter().map(|(p, t)| (p.as_str(), *t)).collect();
    let mut parts = vec![
        ZipEntry::text("[Content_Types].xml", content_types(&override_refs)),
        ZipEntry::text("_rels/.rels", package_rels("ppt/presentation.xml")),
        ZipEntry::text("docProps/core.xml", core_properties(&plan.title, &lang)),
        ZipEntry::text("docProps/app.xml", app_properties()),
        ZipEntry::text("ppt/presentation.xml", presentation),
        ZipEntry::text("ppt/_rels/presentation.xml.rels", presentation_rels.xml()),
        ZipEntry::text("ppt/theme/theme1.xml", theme_xml()),
        ZipEntry::text("ppt/theme/theme2.xml", theme_xml()),
        ZipEntry::text("ppt/slideMasters/slideMaster1.xml", master_xml()),
        ZipEntry::text("ppt/slideMasters/_rels/slideMaster1.xml.rels", master_rels.xml()),
        ZipEntry::text("ppt/notesMasters/notesMaster1.xml", notes_master_xml()),
        ZipEntry::text("ppt/notesMasters/_rels/notesMaster1.xml.rels", notes_master_rels.xml()),
    ];
    for (n, layout) in layouts().into_iter().enumerate() {
        parts.push(ZipEntry::text(format!("ppt/slideLayouts/slideLayout{}.xml", n + 1), layout));
        parts.push(ZipEntry::text(format!("ppt/slideLayouts/_rels/slideLayout{}.xml.rels", n + 1), layout_rels.xml()));
    }
    parts.extend(slide_parts);
    parts
}

/// The PowerPoint export: one file named after the guide.
pub fn export_pptx(plan: &Plan, options: &PptxOptions) -> Result<Export, OfficeError> {
    let bytes = zip::build(&pptx_parts(plan, options))?;
    Ok(Export {
        files: vec![OutFile { path: format!("{}.pptx", sanitise_folder_name(&plan.title)), bytes }],
        warnings: plan.warnings.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo;
    use crate::office::check::{attributes, texts, well_formed};
    use crate::plan::DocumentPictures;
    use crate::plan::{plan, ExportOptions};
    use crate::zip::reader::{unzip, Unzipped};
    use openstage_capture_render::BlockText;

    fn demo_plan() -> Plan {
        let d = demo();
        plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default())
    }

    fn make(options: &PptxOptions) -> Unzipped {
        unzip(&export_pptx(&demo_plan(), options).unwrap().files[0].bytes)
    }

    fn jpg_plan() -> Plan {
        let d = demo();
        let options =
            ExportOptions { document_pictures: DocumentPictures::Jpg { quality: 80 }, ..ExportOptions::default() };
        plan(&d.document.guide, &d.pictures, &BlockText, &options)
    }

    /// The target of every picture relationship in the slides' relationship files, in slide order.
    fn picture_targets(z: &Unzipped) -> Vec<String> {
        let mut rel_files: Vec<&String> =
            z.names.iter().filter(|n| n.starts_with("ppt/slides/_rels/") && n.ends_with(".rels")).collect();
        rel_files.sort_by_key(|n| {
            n.trim_start_matches("ppt/slides/_rels/slide").trim_end_matches(".xml.rels").parse::<usize>().unwrap()
        });
        rel_files
            .into_iter()
            .flat_map(|f| attributes(&z.text(f), "Relationship", "Target"))
            .filter(|t| t.contains("media/"))
            .collect()
    }

    #[test]
    fn jpg_pictures_are_jpg_parts_with_a_content_type_and_each_slide_points_at_its_own() {
        let p = jpg_plan();
        let z = unzip(&export_pptx(&p, &PptxOptions::default()).unwrap().files[0].bytes);
        let media: Vec<&String> = z.names.iter().filter(|n| n.starts_with("ppt/media/")).collect();
        assert_eq!(
            media,
            [
                "ppt/media/image1.jpg",
                "ppt/media/image2.jpg",
                "ppt/media/image3.jpg",
                "ppt/media/image4.jpg",
                "ppt/media/image5.jpg"
            ]
        );
        for (n, picture) in p.steps.iter().filter_map(|s| s.picture.as_ref()).enumerate() {
            let part = z.get(&format!("ppt/media/image{}.jpg", n + 1)).unwrap();
            assert_eq!(part, picture.jpg.as_ref().unwrap().as_slice(), "image{} holds the step's JPG", n + 1);
            assert_eq!(&part[..2], &[0xFF, 0xD8]);
        }
        assert!(z.text("[Content_Types].xml").contains("<Default Extension=\"jpg\" ContentType=\"image/jpeg\"/>"));
        assert_eq!(
            picture_targets(&z),
            [
                "../media/image1.jpg",
                "../media/image2.jpg",
                "../media/image3.jpg",
                "../media/image4.jpg",
                "../media/image5.jpg"
            ]
        );
        for name in z.names.iter().filter(|n| n.ends_with(".xml") || n.ends_with(".rels")) {
            well_formed(name, &z.text(name));
        }
        let png = make(&PptxOptions::default());
        assert!(png.names.iter().any(|n| n == "ppt/media/image1.png"), "the PNG presentation is as it was");
    }

    #[test]
    fn a_picture_kept_as_png_beside_jpg_ones_has_its_own_extension_and_the_slide_points_at_it() {
        let mut p = jpg_plan();
        let picture = p.steps.iter_mut().filter_map(|s| s.picture.as_mut()).nth(1).unwrap();
        picture.jpg = None;
        let z = unzip(&export_pptx(&p, &PptxOptions::default()).unwrap().files[0].bytes);
        assert!(z.get("ppt/media/image2.png").is_some() && z.get("ppt/media/image2.jpg").is_none());
        assert!(z.get("ppt/media/image1.jpg").is_some() && z.get("ppt/media/image3.jpg").is_some());
        let targets = picture_targets(&z);
        assert_eq!(targets[1], "../media/image2.png");
        for target in &targets {
            assert!(z.get(&format!("ppt/media/{}", target.trim_start_matches("../media/"))).is_some(), "{target}");
        }
    }

    fn slide_names(z: &Unzipped) -> Vec<String> {
        let mut names: Vec<String> =
            z.names.iter().filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml")).cloned().collect();
        names.sort_by_key(|n| {
            n.trim_start_matches("ppt/slides/slide").trim_end_matches(".xml").parse::<usize>().unwrap()
        });
        names
    }

    #[test]
    fn the_package_has_every_part_with_the_content_types_file_first() {
        let z = make(&PptxOptions::default());
        assert_eq!(z.names[0], "[Content_Types].xml");
        for part in [
            "_rels/.rels",
            "docProps/core.xml",
            "ppt/presentation.xml",
            "ppt/_rels/presentation.xml.rels",
            "ppt/theme/theme1.xml",
            "ppt/theme/theme2.xml",
            "ppt/slideMasters/slideMaster1.xml",
            "ppt/slideMasters/_rels/slideMaster1.xml.rels",
            "ppt/notesMasters/notesMaster1.xml",
            "ppt/slideLayouts/slideLayout1.xml",
            "ppt/slideLayouts/slideLayout3.xml",
            "ppt/slides/slide1.xml",
            "ppt/media/image1.png",
        ] {
            assert!(z.names.iter().any(|n| n == part), "{part} in {:?}", z.names);
        }
        assert_eq!(slide_names(&z).len(), 7, "a cover and one slide for each of the six steps and notes");
    }

    #[test]
    fn every_xml_part_is_well_formed() {
        for options in [
            PptxOptions::default(),
            PptxOptions { cover_slide: false, slide_numbers: false, speaker_notes: false, ..PptxOptions::default() },
            PptxOptions {
                agenda_slide: true,
                footer_text: "Help desk <b>& co</b>".to_string(),
                ..PptxOptions::default()
            },
        ] {
            let z = make(&options);
            for name in z.names.iter().filter(|n| n.ends_with(".xml") || n.ends_with(".rels")) {
                well_formed(name, &z.text(name));
            }
        }
    }

    #[test]
    fn every_relationship_points_at_a_real_part_and_every_part_has_a_content_type() {
        let z = make(&PptxOptions { agenda_slide: true, ..PptxOptions::default() });
        let mut rel_files: Vec<&String> = z.names.iter().filter(|n| n.ends_with(".rels")).collect();
        rel_files.sort();
        assert!(rel_files.len() >= 10);
        for rel_file in rel_files {
            // `ppt/slides/_rels/slide1.xml.rels` belongs to the part `ppt/slides/slide1.xml`.
            let (dir, file) = rel_file.rsplit_once('/').unwrap();
            let base = dir.strip_suffix("_rels").unwrap_or("").trim_end_matches('/');
            let owner_dir = if rel_file == "_rels/.rels" { String::new() } else { base.to_string() };
            if rel_file != "_rels/.rels" {
                let owner = format!(
                    "{}{}",
                    if owner_dir.is_empty() { String::new() } else { format!("{owner_dir}/") },
                    file.trim_end_matches(".rels")
                );
                assert!(z.get(&owner).is_some(), "{rel_file} belongs to a part that is missing: {owner}");
            }
            let xml = z.text(rel_file);
            for target in attributes(&xml, "Relationship", "Target") {
                let mut parts: Vec<&str> = if owner_dir.is_empty() { vec![] } else { owner_dir.split('/').collect() };
                for piece in target.split('/') {
                    match piece {
                        ".." => {
                            parts.pop();
                        }
                        "." | "" => {}
                        p => parts.push(p),
                    }
                }
                let resolved = parts.join("/");
                assert!(
                    z.get(&resolved).is_some(),
                    "{rel_file}: target {target} resolves to {resolved}, which is not in the file"
                );
            }
        }
        let types = z.text("[Content_Types].xml");
        let overrides = attributes(&types, "Override", "PartName");
        for name in z.names.iter().filter(|n| *n != "[Content_Types].xml") {
            let has_default = name.ends_with(".rels") || name.ends_with(".png") || name.ends_with(".jpg");
            assert!(has_default || overrides.contains(&format!("/{name}")), "{name} has no content type");
        }
        for part in overrides {
            assert!(z.get(part.trim_start_matches('/')).is_some(), "content type for a part that is missing: {part}");
        }
    }

    #[test]
    fn every_id_a_slide_uses_is_one_its_relationships_define() {
        let z = make(&PptxOptions::default());
        for name in slide_names(&z) {
            let xml = z.text(&name);
            let rels = z.text(&name.replace("ppt/slides/", "ppt/slides/_rels/").replace(".xml", ".xml.rels"));
            let ids = attributes(&rels, "Relationship", "Id");
            for used in attributes(&xml, "a:blip", "r:embed") {
                assert!(ids.contains(&used), "{name}: {used} not in {ids:?}");
            }
            assert!(!xml.contains("@IMAGE@"));
        }
        let presentation = z.text("ppt/presentation.xml");
        let rels = z.text("ppt/_rels/presentation.xml.rels");
        let ids = attributes(&rels, "Relationship", "Id");
        for used in attributes(&presentation, "p:sldId", "r:id") {
            assert!(ids.contains(&used), "{used}");
        }
        assert_eq!(attributes(&presentation, "p:sldId", "id"), ["256", "257", "258", "259", "260", "261", "262"]);
        let master_rels = z.text("ppt/slideMasters/_rels/slideMaster1.xml.rels");
        let master = z.text("ppt/slideMasters/slideMaster1.xml");
        for used in attributes(&master, "p:sldLayoutId", "r:id") {
            assert!(attributes(&master_rels, "Relationship", "Id").contains(&used));
        }
    }

    #[test]
    fn each_step_is_a_slide_with_a_real_title_the_step_number_included() {
        let z = make(&PptxOptions::default());
        let slides = slide_names(&z);
        let title_of = |name: &str| texts(&z.text(name), "a:t")[0].clone();
        let titles: Vec<String> = slides.iter().map(|s| title_of(s)).collect();
        assert_eq!(
            titles,
            [
                "How to submit an assignment",
                "1. Click \"Assignments\"",
                "2. Click \"New assignment\"",
                "2.1. Type \"Week 3 reflection\"",
                "3. Select \"Course\"",
                "Tip",
                "4. Click \"Submit\""
            ]
        );
        assert!(z.text(&slides[0]).contains("<p:ph type=\"ctrTitle\"/>"));
        for name in &slides[1..] {
            assert!(z.text(name).contains("<p:ph type=\"title\"/>"), "{name} has a title placeholder");
        }
        let unique: std::collections::HashSet<&String> = titles.iter().collect();
        assert_eq!(unique.len(), titles.len(), "no two slides share a title");
    }

    #[test]
    fn the_reading_order_is_title_then_picture_then_description_then_the_foot() {
        let z = make(&PptxOptions { footer_text: "Help desk".to_string(), ..PptxOptions::default() });
        let xml = z.text("ppt/slides/slide3.xml");
        let at = |needle: &str| xml.find(needle).unwrap_or_else(|| panic!("{needle} in slide 3"));
        assert!(at("<p:ph type=\"title\"/>") < at("<p:pic>"));
        assert!(at("<p:pic>") < at("name=\"Description\""));
        assert!(at("name=\"Description\"") < at("type=\"ftr\""));
        assert!(at("type=\"ftr\"") < at("type=\"sldNum\""));
    }

    #[test]
    fn every_picture_has_alt_text_and_fits_inside_the_slide() {
        let p = demo_plan();
        let z = make(&PptxOptions::default());
        let mut alts = Vec::new();
        for name in slide_names(&z) {
            let xml = z.text(&name);
            let pics = attributes(&xml, "p:cNvPr", "descr");
            alts.extend(pics);
            let (xs, ys, cxs, cys) = (
                attributes(&xml, "a:off", "x"),
                attributes(&xml, "a:off", "y"),
                attributes(&xml, "a:ext", "cx"),
                attributes(&xml, "a:ext", "cy"),
            );
            // Every box, the picture's included, lies inside the 16:9 slide.
            for i in 0..xs.len().min(cxs.len()).saturating_sub(1) {
                let (x, y, cx, cy) = (
                    xs[i + 1].parse::<i64>().unwrap_or(0),
                    ys[i + 1].parse::<i64>().unwrap_or(0),
                    cxs[i + 1].parse::<i64>().unwrap_or(0),
                    cys[i + 1].parse::<i64>().unwrap_or(0),
                );
                if cx > 0 {
                    assert!(
                        x >= 0 && y >= 0 && x + cx <= SLIDE_W && y + cy <= SLIDE_H,
                        "{name}: box {i} at {x},{y} size {cx}x{cy}"
                    );
                }
            }
        }
        let expected: Vec<&str> = p.steps.iter().filter(|s| s.picture.is_some()).map(|s| s.alt.as_str()).collect();
        assert_eq!(alts, expected);
        let slide_width_in = SLIDE_W as f64 / 914_400.0;
        assert!(slide_width_in > 13.3 && slide_width_in < 13.4);
    }

    #[test]
    fn the_full_description_is_in_the_notes_and_the_first_lines_are_on_the_slide() {
        let z = make(&PptxOptions::default());
        let slide2 = z.text("ppt/slides/slide3.xml"); // step 2
        assert!(texts(&slide2, "a:t").contains(&"Start here: this opens the form.".to_string()));
        let notes = z.text("ppt/notesSlides/notesSlide4.xml"); // step 2.1 has a list
        let lines = texts(&notes, "a:t");
        assert_eq!(
            lines,
            [
                "Give the work a short name.",
                "\u{2022} Use words your teacher will recognise",
                "\u{2022} You can change it later"
            ]
        );
        let on_slide = texts(&z.text("ppt/slides/slide4.xml"), "a:t");
        assert!(on_slide.contains(&"Give the work a short name.".to_string()));
        assert!(on_slide.iter().any(|t| t.ends_with('\u{2026}')), "cut short, with an ellipsis: {on_slide:?}");
        assert!(!on_slide.iter().any(|t| t.contains("You can change it later")), "the third line is only in the notes");
        let rels = z.text("ppt/slides/_rels/slide4.xml.rels");
        assert!(rels.contains("notesSlides/notesSlide4.xml"));
        assert!(z.text("ppt/notesSlides/_rels/notesSlide4.xml.rels").contains("slides/slide4.xml"));
    }

    #[test]
    fn a_link_in_a_description_is_written_out_in_the_notes() {
        let z = make(&PptxOptions::default());
        let notes = texts(&z.text("ppt/notesSlides/notesSlide6.xml"), "a:t"); // the tip
        assert!(notes.iter().any(|l| l.contains("read the help (https://example.com/help)")), "{notes:?}");
    }

    #[test]
    fn speaker_notes_can_be_left_out() {
        let z = make(&PptxOptions { speaker_notes: false, ..PptxOptions::default() });
        assert!(!z.names.iter().any(|n| n.contains("notesSlide")));
        assert!(!z.text("[Content_Types].xml").contains("notesSlide"));
        assert!(z.get("ppt/notesMasters/notesMaster1.xml").is_some(), "the master stays so a person can add notes");
    }

    #[test]
    fn the_cover_slide_and_the_agenda_are_optional() {
        let no_cover = make(&PptxOptions { cover_slide: false, ..PptxOptions::default() });
        assert_eq!(slide_names(&no_cover).len(), 6);
        assert_eq!(texts(&no_cover.text("ppt/slides/slide1.xml"), "a:t")[0], "1. Click \"Assignments\"");
        let z = make(&PptxOptions { agenda_slide: true, ..PptxOptions::default() });
        let slides = slide_names(&z);
        assert_eq!(slides.len(), 8);
        let agenda = texts(&z.text("ppt/slides/slide2.xml"), "a:t");
        assert_eq!(
            &agenda[..5],
            [
                "Contents",
                "1. Click \"Assignments\"",
                "2. Click \"New assignment\"",
                "3. Select \"Course\"",
                "4. Click \"Submit\""
            ],
            "top-level steps only, no substeps and no tip"
        );
        assert!(
            z.text("ppt/slides/slide2.xml").contains("<a:buNone/>"),
            "no bullet in front of numbers that are already text"
        );
        assert!(z.text("ppt/slides/_rels/slide2.xml.rels").contains("slideLayout3.xml"));
    }

    #[test]
    fn a_long_agenda_continues_on_further_slides() {
        let mut d = demo();
        let mut steps = Vec::new();
        for i in 0..25 {
            let mut s = d.document.guide.steps[0].clone();
            s.id = format!("step-{}", 100 + i);
            s.title = format!("Step number {}", i + 1);
            s.parent = None;
            steps.push(s);
        }
        d.document.guide.steps = steps;
        let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let z = unzip(
            &export_pptx(&p, &PptxOptions { agenda_slide: true, ..PptxOptions::default() }).unwrap().files[0].bytes,
        );
        let titles: Vec<String> =
            (2..=4).map(|n| texts(&z.text(&format!("ppt/slides/slide{n}.xml")), "a:t")[0].clone()).collect();
        assert_eq!(titles, ["Contents", "Contents (2)", "Contents (3)"]);
        assert_eq!(
            texts(&z.text("ppt/slides/slide2.xml"), "a:t").len(),
            1 + AGENDA_ROWS + 1,
            "title, ten rows and the slide number"
        );
        assert_eq!(slide_names(&z).len(), 1 + 3 + 25);
    }

    #[test]
    fn the_footer_and_slide_numbers_are_placeholders_on_every_slide_but_the_cover() {
        let z = make(&PptxOptions { footer_text: "Help desk".to_string(), ..PptxOptions::default() });
        let slides = slide_names(&z);
        let cover = z.text(&slides[0]);
        assert!(!cover.contains("type=\"ftr\"") && !cover.contains("type=\"sldNum\""));
        for (n, name) in slides.iter().enumerate().skip(1) {
            let xml = z.text(name);
            assert!(xml.contains("<a:t>Help desk</a:t>"), "{name}");
            assert!(
                xml.contains(&format!("type=\"slidenum\"><a:rPr lang=\"en-US\"/><a:t>{}</a:t>", n + 1)),
                "{name} knows its own number"
            );
        }
        let none = make(&PptxOptions { slide_numbers: false, ..PptxOptions::default() });
        assert!(!none.text("ppt/slides/slide2.xml").contains("sldNum"));
        assert!(!none.text("ppt/slides/slide2.xml").contains("ftr"), "no footer text, no footer placeholder");
    }

    #[test]
    fn the_authors_words_cannot_break_the_file() {
        let mut d = demo();
        d.document.guide.title = "A <b>&</b> \"title\" \u{7}bell".to_string();
        d.document.guide.steps[0].title = "]]> <script>x</script>".to_string();
        d.document.guide.steps[0].description =
            "**<b>x</b>** & [y](https://example.com/?a=1&b=2)\n\n- item <i>".to_string();
        d.document.guide.steps[1].image_description = "x\" onerror=\"y <>".to_string();
        let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let options = PptxOptions { agenda_slide: true, footer_text: "a & <b>".to_string(), ..PptxOptions::default() };
        let z = unzip(&export_pptx(&p, &options).unwrap().files[0].bytes);
        for name in z.names.iter().filter(|n| n.ends_with(".xml") || n.ends_with(".rels")) {
            well_formed(name, &z.text(name));
        }
        let slide2 = z.text("ppt/slides/slide3.xml");
        assert!(texts(&slide2, "a:t").iter().any(|t| t == "1. ]]> <script>x</script>"));
        assert!(
            !z.names.iter().filter(|n| n.ends_with(".xml")).any(|n| z.text(n).contains("<script")),
            "no markup of the author's gets through"
        );
        let all_alts: Vec<String> =
            slide_names(&z).iter().flat_map(|n| attributes(&z.text(n), "p:cNvPr", "descr")).collect();
        assert!(all_alts.iter().any(|a| a == "x\" onerror=\"y <>"));
        assert_eq!(texts(&z.text("docProps/core.xml"), "dc:title"), ["A <b>&</b> \"title\" bell"]);
    }

    #[test]
    fn the_presentation_says_what_language_it_is_in() {
        let z = make(&PptxOptions::default());
        assert!(z.text("ppt/slides/slide2.xml").contains("lang=\"en-US\""));
        let mut d = demo();
        for (code, tag, contents) in [("de", "de-DE", "Inhalt"), ("fr", "fr-FR", "Sommaire")] {
            d.document.guide.language = code.to_string();
            let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
            let z = unzip(
                &export_pptx(&p, &PptxOptions { agenda_slide: true, ..PptxOptions::default() }).unwrap().files[0].bytes,
            );
            assert!(z.text("ppt/slides/slide3.xml").contains(&format!("lang=\"{tag}\"")), "{code}");
            assert_eq!(texts(&z.text("ppt/slides/slide2.xml"), "a:t")[0], contents);
            assert_eq!(texts(&z.text("docProps/core.xml"), "dc:language"), [tag]);
        }
    }

    #[test]
    fn the_slide_is_sixteen_by_nine() {
        let z = make(&PptxOptions::default());
        let presentation = z.text("ppt/presentation.xml");
        assert_eq!(attributes(&presentation, "p:sldSz", "cx"), ["12192000"]);
        assert_eq!(attributes(&presentation, "p:sldSz", "cy"), ["6858000"]);
        assert_eq!(12_192_000 * 9, 6_858_000 * 16);
    }

    #[test]
    fn description_lines_on_a_slide_are_cut_short_with_an_ellipsis() {
        assert_eq!(slide_lines(""), Vec::<String>::new());
        assert_eq!(slide_lines("One line."), ["One line."]);
        assert_eq!(slide_lines("a\nb"), ["a", "b"]);
        assert_eq!(slide_lines("a\n\nb\n\nc"), ["a", "b\u{2026}"], "a third paragraph is only in the notes");
        let long = "word ".repeat(100);
        let lines = slide_lines(&long);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].chars().count() <= SLIDE_CHARS && lines[0].ends_with('\u{2026}'), "{}", lines[0]);
    }

    #[test]
    fn a_step_with_no_picture_shows_all_its_text_in_a_big_box() {
        let z = make(&PptxOptions::default());
        let tip = z.text("ppt/slides/slide6.xml");
        assert!(!tip.contains("<p:pic>"));
        let lines = texts(&tip, "a:t");
        assert_eq!(lines[0], "Tip");
        assert!(
            lines.iter().any(|l| l.contains("Tip: you can read the help (https://example.com/help) at any time.")),
            "{lines:?}"
        );
        assert!(lines.iter().any(|l| l.contains("Saved work is kept for a year.")));
        assert!(tip.contains("sz=\"2400\""));
    }

    #[test]
    fn the_file_is_named_after_the_guide_and_is_the_same_every_time() {
        let p = demo_plan();
        let export = export_pptx(&p, &PptxOptions::default()).unwrap();
        assert_eq!(export.files.len(), 1);
        assert_eq!(export.files[0].path, "How to submit an assignment.pptx");
        assert_eq!(export, export_pptx(&p, &PptxOptions::default()).unwrap());
        assert!(export.files[0].bytes.len() > 20_000);
    }

    #[test]
    fn a_missing_picture_leaves_the_step_as_a_text_slide_with_a_warning() {
        let d = demo();
        let mut pictures = d.pictures;
        pictures.0.remove("images/step-0002.png");
        let p = plan(&d.document.guide, &pictures, &BlockText, &ExportOptions::default());
        let export = export_pptx(&p, &PptxOptions::default()).unwrap();
        assert!(export.warnings.iter().any(|w| w.contains("images/step-0002.png")));
        let z = unzip(&export.files[0].bytes);
        assert_eq!(z.names.iter().filter(|n| n.starts_with("ppt/media/")).count(), 4);
        assert_eq!(slide_names(&z).len(), 7);
    }

    #[test]
    fn an_empty_guide_is_a_valid_presentation() {
        let guide = openstage_express_core::Guide::new("g", "Nothing yet", "en", "t");
        let p = plan(&guide, &crate::picture::MemoryPictures::default(), &BlockText, &ExportOptions::default());
        let z = unzip(
            &export_pptx(&p, &PptxOptions { agenda_slide: true, ..PptxOptions::default() }).unwrap().files[0].bytes,
        );
        for name in z.names.iter().filter(|n| n.ends_with(".xml") || n.ends_with(".rels")) {
            well_formed(name, &z.text(name));
        }
        assert_eq!(slide_names(&z).len(), 1, "only the cover, and no empty agenda");
    }
}
