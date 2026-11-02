// SPDX-License-Identifier: Apache-2.0
//! What the Word and PowerPoint exports share: XML escaping, the relationships files, language tags, picture sizes, and the
//! standard parts of an Office Open XML package.

use crate::zip::ZipError;

pub const REL_BASE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const REL_PACKAGE: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

/// Why a Word or PowerPoint file could not be made. The text is written for the person.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OfficeError {
    #[error("{0}")]
    Zip(#[from] ZipError),
}

/// Escape text for an XML element or a quoted attribute. Characters XML 1.0 cannot hold at all (most control characters) are
/// dropped, so one stray character in an author's text can never make the file unreadable.
pub fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

/// A language tag (`en-US`) for a guide's language code (`en`). Codes the exports have words for get their usual region; any
/// other code is used as the author wrote it, cleaned up.
pub fn lang_tag(code: &str) -> String {
    let cleaned: String = code.trim().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    match cleaned.to_ascii_lowercase().as_str() {
        "" => "en-US".to_string(),
        "en" => "en-US".to_string(),
        "de" => "de-DE".to_string(),
        "fr" => "fr-FR".to_string(),
        _ => cleaned,
    }
}

/// The relationships of one part, in the order they were added. Ids are `rId1`, `rId2` and so on.
#[derive(Debug, Default)]
pub struct Rels {
    items: Vec<Rel>,
}

#[derive(Debug)]
struct Rel {
    id: String,
    kind: String,
    target: String,
    external: bool,
}

impl Rels {
    /// Add a relationship of type `REL_BASE/<kind>` to `target` and return its id. An existing one to the same place with the
    /// same type is reused.
    pub fn add(&mut self, kind: &str, target: &str, external: bool) -> String {
        let full = format!("{REL_BASE}/{kind}");
        if let Some(existing) =
            self.items.iter().find(|r| r.kind == full && r.target == target && r.external == external)
        {
            return existing.id.clone();
        }
        let id = format!("rId{}", self.items.len() + 1);
        self.items.push(Rel { id: id.clone(), kind: full, target: target.to_string(), external });
        id
    }

    pub fn xml(&self) -> String {
        let mut out = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
        );
        for r in &self.items {
            out.push_str(&format!(
                "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"{}/>",
                r.id,
                xml_escape(&r.kind),
                xml_escape(&r.target),
                if r.external { " TargetMode=\"External\"" } else { "" }
            ));
        }
        out.push_str("</Relationships>");
        out
    }
}

/// The package-level relationships (`_rels/.rels`): the main document, the core properties and the extended properties.
pub fn package_rels(main_part: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{REL_PACKAGE}\">\
<Relationship Id=\"rId1\" Type=\"{REL_BASE}/officeDocument\" Target=\"{main_part}\"/>\
<Relationship Id=\"rId2\" Type=\"{REL_PACKAGE}/metadata/core-properties\" Target=\"docProps/core.xml\"/>\
<Relationship Id=\"rId3\" Type=\"{REL_BASE}/extended-properties\" Target=\"docProps/app.xml\"/>\
</Relationships>"
    )
}

/// `docProps/core.xml`: the document's title, its language and who made it. No dates, so the same guide gives the same file.
pub fn core_properties(title: &str, language: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<dc:title>{}</dc:title><dc:creator>OpenStage Capture Express</dc:creator><dc:language>{}</dc:language></cp:coreProperties>",
        xml_escape(title),
        xml_escape(language)
    )
}

/// `docProps/app.xml`.
pub fn app_properties() -> String {
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\"><Application>OpenStage Capture Express</Application></Properties>".to_string()
}

/// `[Content_Types].xml` for the given overrides (part name, content type); `png`, `jpg` and the two XML kinds are defaults.
pub fn content_types(overrides: &[(&str, &str)]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Default Extension=\"png\" ContentType=\"image/png\"/>\
<Default Extension=\"jpg\" ContentType=\"image/jpeg\"/>",
    );
    for (part, kind) in overrides {
        out.push_str(&format!("<Override PartName=\"{}\" ContentType=\"{}\"/>", xml_escape(part), xml_escape(kind)));
    }
    out.push_str("</Types>");
    out
}

pub const EMU_PER_INCH: i64 = 914_400;
pub const EMU_PER_PIXEL: i64 = 9_525;

/// Millimetres to English metric units (914,400 to the inch).
pub fn mm_to_emu(mm: f64) -> i64 {
    (mm / 25.4 * EMU_PER_INCH as f64).round() as i64
}

/// The size a picture of `width` by `height` pixels is shown at: its natural size at 96 pixels to the inch, made smaller (never
/// larger) to fit inside `max_width` by `max_height` EMU, keeping its shape.
pub fn fit_picture(width: u32, height: u32, max_width: i64, max_height: i64) -> (i64, i64) {
    let natural_w = i64::from(width.max(1)) * EMU_PER_PIXEL;
    let natural_h = i64::from(height.max(1)) * EMU_PER_PIXEL;
    let scale = (max_width as f64 / natural_w as f64).min(max_height as f64 / natural_h as f64).min(1.0);
    (((natural_w as f64 * scale).round() as i64).max(1), ((natural_h as f64 * scale).round() as i64).max(1))
}

#[cfg(test)]
pub(crate) mod check {
    //! Test helpers: every XML part of a package must be well formed.

    use quick_xml::events::Event;
    use quick_xml::Reader;

    /// Panics, naming `what`, unless `xml` is one well-formed XML document with every element closed.
    pub fn well_formed(what: &str, xml: &str) {
        let mut reader = Reader::from_str(xml);
        let mut depth = 0i64;
        let mut roots = 0;
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => {
                    for a in e.attributes() {
                        a.unwrap_or_else(|err| panic!("{what}: a bad attribute: {err}"));
                    }
                    if depth == 0 {
                        roots += 1;
                    }
                    depth += 1;
                }
                Ok(Event::End(_)) => depth -= 1,
                Ok(Event::Empty(e)) => {
                    for a in e.attributes() {
                        a.unwrap_or_else(|err| panic!("{what}: a bad attribute: {err}"));
                    }
                    if depth == 0 {
                        roots += 1;
                    }
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(e) => panic!("{what} is not well-formed XML: {e} at byte {}", reader.buffer_position()),
            }
        }
        assert_eq!(depth, 0, "{what}: elements left open");
        assert_eq!(roots, 1, "{what}: exactly one root element");
    }

    /// The text of every `name` attribute value found for elements called `element` (for example `descr` of `wp:docPr`).
    pub fn attributes(xml: &str, element: &str, attribute: &str) -> Vec<String> {
        let mut reader = Reader::from_str(xml);
        let mut found = Vec::new();
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    if e.name().as_ref() == element {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == attribute {
                                found
                                    .push(a.normalized_value(quick_xml::XmlVersion::Implicit1_0).unwrap().into_owned());
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(e) => panic!("not well-formed XML: {e}"),
            }
        }
        found
    }

    /// All the text inside elements called `element`, one string for each element.
    pub fn texts(xml: &str, element: &str) -> Vec<String> {
        let mut reader = Reader::from_str(xml);
        let mut found = Vec::new();
        let mut current: Option<String> = None;
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) if e.name().as_ref() == element => current = Some(String::new()),
                Ok(Event::Text(t)) => {
                    if let Some(c) = current.as_mut() {
                        c.push_str(&t.xml10_content());
                    }
                }
                Ok(Event::GeneralRef(r)) => {
                    if let Some(c) = current.as_mut() {
                        let name = r.into_inner();
                        match name.as_ref() {
                            "amp" => c.push('&'),
                            "lt" => c.push('<'),
                            "gt" => c.push('>'),
                            "quot" => c.push('"'),
                            "apos" => c.push('\''),
                            other => panic!("unexpected entity &{other};"),
                        }
                    }
                }
                Ok(Event::End(e)) if e.name().as_ref() == element => {
                    if let Some(c) = current.take() {
                        found.push(c);
                    }
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(e) => panic!("not well-formed XML: {e}"),
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::check::*;
    use super::*;

    #[test]
    fn xml_text_is_escaped_and_impossible_characters_are_dropped() {
        assert_eq!(xml_escape("a<b>&\"c\"'d'"), "a&lt;b&gt;&amp;&quot;c&quot;&apos;d&apos;");
        assert_eq!(xml_escape("tab\there\nnew\rline"), "tab\there\nnew\rline");
        assert_eq!(xml_escape("bell\u{7} nul\u{0} esc\u{1b} ok\u{e9}\u{65e5}"), "bell nul esc ok\u{e9}\u{65e5}");
        assert_eq!(xml_escape("\u{FFFE}\u{FFFF}x"), "x");
        assert_eq!(xml_escape("\u{1F600}"), "\u{1F600}", "characters beyond the first plane are fine");
    }

    #[test]
    fn language_tags() {
        assert_eq!(lang_tag("en"), "en-US");
        assert_eq!(lang_tag("DE"), "de-DE");
        assert_eq!(lang_tag("fr"), "fr-FR");
        assert_eq!(lang_tag("ja"), "ja");
        assert_eq!(lang_tag("pt-BR"), "pt-BR");
        assert_eq!(lang_tag(""), "en-US");
        assert_eq!(lang_tag("e\"n<"), "en-US", "cleaned first, then looked up");
    }

    #[test]
    fn relationships_are_numbered_and_reused() {
        let mut rels = Rels::default();
        let a = rels.add("styles", "styles.xml", false);
        let b = rels.add("hyperlink", "https://example.com/?a=1&b=2", true);
        assert_eq!((a.as_str(), b.as_str()), ("rId1", "rId2"));
        assert_eq!(rels.add("styles", "styles.xml", false), "rId1");
        assert_eq!(rels.add("image", "media/image1.png", false), "rId3");
        let xml = rels.xml();
        well_formed("rels", &xml);
        assert!(xml.contains("Target=\"https://example.com/?a=1&amp;b=2\" TargetMode=\"External\""));
        assert!(xml.contains(&format!("Type=\"{REL_BASE}/image\" Target=\"media/image1.png\"/>")));
    }

    #[test]
    fn the_standard_parts_are_well_formed() {
        well_formed("package rels", &package_rels("word/document.xml"));
        well_formed("core", &core_properties("A <b> & \"title\"", "en-US"));
        well_formed("app", &app_properties());
        well_formed("types", &content_types(&[("/word/document.xml", "application/x")]));
        assert!(core_properties("T", "en-US").contains("<dc:title>T</dc:title>"));
        assert_eq!(texts(&core_properties("A <b> & \"t\"", "de-DE"), "dc:title"), ["A <b> & \"t\""]);
    }

    #[test]
    fn pictures_fit_the_space_and_are_never_enlarged() {
        // 1280 by 720 pixels is 12,192,000 by 6,858,000 EMU: too wide for 6 inches, so it is scaled down by width.
        let (w, h) = fit_picture(1280, 720, 6 * EMU_PER_INCH, 8 * EMU_PER_INCH);
        assert_eq!(w, 6 * EMU_PER_INCH);
        assert!((h as f64 / w as f64 - 0.5625).abs() < 1e-4);
        // A tall picture is limited by the height.
        let (w, h) = fit_picture(400, 1600, 6 * EMU_PER_INCH, 4 * EMU_PER_INCH);
        assert_eq!(h, 4 * EMU_PER_INCH);
        assert!((w as f64 / h as f64 - 0.25).abs() < 1e-4);
        // A small one stays its natural size.
        assert_eq!(fit_picture(96, 48, 6 * EMU_PER_INCH, 4 * EMU_PER_INCH), (EMU_PER_INCH, EMU_PER_INCH / 2));
        // Rubbish sizes still give a usable shape.
        let (w, h) = fit_picture(0, 0, 100, 100);
        assert!(w >= 1 && h >= 1);
    }

    #[test]
    fn millimetres_become_the_right_number_of_units() {
        assert_eq!(mm_to_emu(25.4), EMU_PER_INCH);
        assert_eq!(mm_to_emu(0.0), 0);
        assert_eq!(mm_to_emu(210.0), 7_560_000);
    }

    #[test]
    fn the_checking_helpers_notice_bad_xml() {
        for bad in ["<a><b></a>", "<a>", "<a/><b/>", "<a x=1/>", "text only", ""] {
            let result = std::panic::catch_unwind(|| well_formed("t", bad));
            assert!(result.is_err(), "{bad:?} should be refused");
        }
        well_formed("t", "<?xml version=\"1.0\"?><a x=\"1\"><b/>text</a>");
        assert_eq!(attributes("<a><b k=\"1\"/><b k=\"2\"/><c k=\"3\"/></a>", "b", "k"), ["1", "2"]);
    }
}
