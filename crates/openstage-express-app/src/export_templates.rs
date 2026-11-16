// SPDX-License-Identifier: Apache-2.0
//! Export templates (spec 10.2, X9): everything the Export screen has chosen, kept in a small file that can be used again and sent to
//! someone else. A template remembers the formats, the width and quality of the pictures, the options of each format, the watermark
//! and, when it is small enough, the logo picture itself (inside the file, so that it works on another computer where the picture is not
//! at the same place). It does not remember the folder the exports go into, whether to open it afterwards, or a preview: those belong to
//! the computer and to the moment.
//!
//! The files are `*.osexport` in `Export templates` beside the settings, and are plain JSON. Express never replaces or erases one: a new
//! template with a name that is taken gets a free one (`Handout (2).osexport`), and a person who wants one gone deletes it in Explorer.

use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use openstage_express_core::fsutil::{is_link, is_network_path, sanitise_folder_name, write_atomic};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::export_options::{lenient, ExportChoices};
use crate::settings::DocumentPicturesChoice;

/// The file name's ending.
pub const EXTENSION: &str = "osexport";
/// What the file says it is, so that another JSON file is not taken for a template.
pub const FORMAT_NAME: &str = "OpenStage Capture Express export template";
/// The layout of the file. A newer one is refused in words rather than read wrongly.
pub const VERSION: u32 = 1;
/// The biggest template file that is read.
pub const MAX_TEMPLATE_BYTES: u64 = 4 * 1024 * 1024;
/// The biggest logo picture that is put inside a template.
pub const MAX_EMBEDDED_LOGO_BYTES: usize = 2 * 1024 * 1024;
/// The most templates the screen lists; the rest are in the folder.
pub const MAX_LISTED: usize = 50;
/// The most letters in a template's name.
pub const MAX_NAME_CHARS: usize = 60;

/// One template as it is written in its file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Template {
    pub format: String,
    pub version: u32,
    pub name: String,
    /// The formats ticked, by folder name (`html`, `pdf`, ...).
    pub formats: Vec<String>,
    pub picture_width: u32,
    pub jpg_quality: u32,
    pub document_pictures: DocumentPicturesChoice,
    /// The options of each format and the watermark. `logoFile` inside it is always empty: the picture is in `logoPng`.
    #[serde(deserialize_with = "lenient")]
    pub options: ExportChoices,
    /// The logo picture (a PNG file), written in base64, when there is one and it is small enough.
    pub logo_png: Option<String>,
}

impl Default for Template {
    fn default() -> Template {
        Template {
            format: FORMAT_NAME.to_string(),
            version: VERSION,
            name: String::new(),
            formats: Vec::new(),
            picture_width: 1280,
            jpg_quality: 90,
            document_pictures: DocumentPicturesChoice::Png,
            options: ExportChoices::default(),
            logo_png: None,
        }
    }
}

/// A template's name as it is kept: white space squeezed, controls dropped, cut to [`MAX_NAME_CHARS`].
pub fn tidy_name(name: &str) -> String {
    let kept: String = name.chars().filter(|c| !c.is_control() || c.is_whitespace()).collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_NAME_CHARS).collect()
}

/// A template in the folder, for the list on the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The file's name without its ending.
    pub name: String,
    pub path: PathBuf,
}

/// Where templates are kept, beside the settings file.
pub fn folder_beside(settings_file: &Path) -> Option<PathBuf> {
    settings_file.parent().map(|dir| dir.join("Export templates"))
}

/// The templates in `folder`, by name (capitals do not matter), at most [`MAX_LISTED`] and how many more there are. A folder that is not
/// there, or is a link, has none.
pub fn list(folder: &Path) -> (Vec<Entry>, usize) {
    if is_link(folder) {
        return (Vec::new(), 0);
    }
    let Ok(read) = std::fs::read_dir(folder) else { return (Vec::new(), 0) };
    let mut found: Vec<Entry> = read
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let path = e.path();
            let name = path.file_name()?.to_str()?;
            let stem = name.strip_suffix(&format!(".{EXTENSION}"))?;
            (!stem.is_empty()).then(|| Entry { name: stem.to_string(), path: path.clone() })
        })
        .collect();
    found.sort_by_key(|e| (e.name.to_lowercase(), e.name.clone()));
    let more = found.len().saturating_sub(MAX_LISTED);
    found.truncate(MAX_LISTED);
    (found, more)
}

/// Write `template` into `folder` (made if needed) under a name that is free. Returns where it went.
pub fn save(folder: &Path, template: &Template) -> Result<PathBuf, String> {
    let name = tidy_name(&template.name);
    if name.is_empty() {
        return Err("Type a name for the template first.".to_string());
    }
    if is_link(folder) {
        return Err("The folder for templates is a shortcut, so nothing was saved there.".to_string());
    }
    std::fs::create_dir_all(folder).map_err(|e| format!("The folder for templates could not be made ({e})."))?;
    let base = sanitise_folder_name(&name);
    let mut path = folder.join(format!("{base}.{EXTENSION}"));
    let mut n = 2;
    while path.exists() {
        path = folder.join(format!("{base} ({n}).{EXTENSION}"));
        n += 1;
        if n > 999 {
            return Err("There are too many templates with that name. Choose another name.".to_string());
        }
    }
    let kept = Template { name, ..template.clone() };
    let mut bytes = serde_json::to_vec_pretty(&kept).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    write_atomic(&path, &bytes).map_err(|e| format!("The template could not be saved ({e})."))?;
    Ok(path)
}

/// Read a template file. A file that is too big, on another computer, not JSON, not a template, or made by a newer Express is refused in
/// words, and nothing is changed by it.
pub fn read(path: &Path) -> Result<Template, String> {
    if is_network_path(path) {
        return Err(
            "The template is on another computer. Express keeps to this computer, so copy it here first.".to_string()
        );
    }
    let meta = std::fs::metadata(path).map_err(|e| format!("The template could not be found ({e})."))?;
    if !meta.is_file() {
        return Err("The place chosen is not a template file.".to_string());
    }
    if meta.len() > MAX_TEMPLATE_BYTES {
        return Err("That file is far bigger than a template can be, so it was not read.".to_string());
    }
    let bytes = std::fs::read(path).map_err(|e| format!("The template could not be read ({e})."))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| "That file is not a template: it is not in the form a template is written in.".to_string())?;
    if value.get("format").and_then(|v| v.as_str()) != Some(FORMAT_NAME) {
        return Err("That file is not an export template from Express.".to_string());
    }
    let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
    if version > u64::from(VERSION) {
        return Err("That template was made by a newer Express than this one, so it cannot be used here. Update Express to use it.".to_string());
    }
    let mut template: Template = serde_json::from_value(value)
        .map_err(|_| "That template has something in it that this Express cannot read.".to_string())?;
    template.name = tidy_name(&template.name);
    template.options = template.options.normalised();
    Ok(template)
}

/// The logo picture of `template` as a file in `folder` (made if needed), under a name made from the picture itself, so that using the same
/// template twice makes one file. `None` when the template has no logo, or it is not a PNG picture that can be read.
pub fn extract_logo(template: &Template, folder: &Path) -> Option<PathBuf> {
    let encoded = template.logo_png.as_deref()?;
    if encoded.len() > MAX_EMBEDDED_LOGO_BYTES * 4 / 3 + 16 {
        return None;
    }
    let bytes = STANDARD.decode(encoded.trim()).ok()?;
    if bytes.len() > MAX_EMBEDDED_LOGO_BYTES || bytes.get(..8) != Some(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    openstage_express_export::picture::decode_png(&bytes).ok()?;
    if is_link(folder) {
        return None;
    }
    std::fs::create_dir_all(folder).ok()?;
    let digest = Sha256::digest(&bytes);
    let name: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    let path = folder.join(format!("logo-{name}.png"));
    if !path.exists() {
        write_atomic(&path, &bytes).ok()?;
    }
    Some(path)
}

/// The base64 of the logo picture to put in a template: the picture file's bytes, if it is a small PNG that can be read.
pub fn embed_logo(logo_file: &str) -> Option<String> {
    let path = Path::new(logo_file.trim());
    if logo_file.trim().is_empty() || is_network_path(path) {
        return None;
    }
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() as usize > MAX_EMBEDDED_LOGO_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    (bytes.get(..8) == Some(b"\x89PNG\r\n\x1a\n")).then(|| STANDARD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export_options::Spot;

    fn template(name: &str) -> Template {
        let mut options = ExportChoices::default();
        options.pdf.header_text = "Help desk".to_string();
        options.watermark.on = true;
        options.watermark.place = Spot::TopRight;
        Template { name: name.to_string(), formats: vec!["html".into(), "pdf".into()], options, ..Template::default() }
    }

    fn logo_png(dir: &Path) -> PathBuf {
        let mut canvas = openstage_capture_render::Canvas::new(10, 5);
        canvas.clear(openstage_capture_render::Rgb(9, 9, 9));
        let path = dir.join("logo.png");
        std::fs::write(&path, openstage_express_export::picture::encode_png(&canvas)).unwrap();
        path
    }

    #[test]
    fn a_saved_template_reads_back_the_same() {
        let dir = tempfile::tempdir().unwrap();
        let t = template("Handout");
        let path = save(dir.path(), &t).unwrap();
        assert_eq!(path.file_name().unwrap(), "Handout.osexport");
        assert_eq!(read(&path).unwrap(), t);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("\"format\": \"OpenStage Capture Express export template\"")
                && text.contains("\"version\": 1"),
            "{text}"
        );
    }

    #[test]
    fn a_name_that_is_taken_gets_a_free_one_and_nothing_is_ever_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let first = save(dir.path(), &template("Handout")).unwrap();
        let before = std::fs::read(&first).unwrap();
        let mut other = template("Handout");
        other.picture_width = 640;
        let second = save(dir.path(), &other).unwrap();
        let third = save(dir.path(), &other).unwrap();
        assert_eq!(second.file_name().unwrap(), "Handout (2).osexport");
        assert_eq!(third.file_name().unwrap(), "Handout (3).osexport");
        assert_eq!(std::fs::read(&first).unwrap(), before, "the first is as it was");
        assert_eq!(read(&second).unwrap().picture_width, 640);
    }

    #[test]
    fn a_name_is_tidied_made_safe_for_a_file_and_must_not_be_empty() {
        assert_eq!(tidy_name("  Help \t desk\n guide "), "Help desk guide");
        assert_eq!(tidy_name(&"x".repeat(200)).chars().count(), MAX_NAME_CHARS);
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(save(dir.path(), &template("   ")).unwrap_err(), "Type a name for the template first.");
        let odd = save(dir.path(), &template("What? <for> \"all\"")).unwrap();
        assert!(
            odd.starts_with(dir.path()) && !odd.file_name().unwrap().to_str().unwrap().contains(['?', '<', '"']),
            "{odd:?}"
        );
        let con = save(dir.path(), &template("CON")).unwrap();
        assert_eq!(con.file_name().unwrap(), "CON_.osexport", "a name Windows keeps for itself");
        assert_eq!(read(&odd).unwrap().name, "What? <for> \"all\"", "the name inside the file is the one typed");
    }

    #[test]
    fn the_list_has_only_templates_in_name_order_and_says_how_many_more_there_are() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(list(&dir.path().join("none")), (vec![], 0));
        for name in ["beta", "Alpha", "gamma"] {
            save(dir.path(), &template(name)).unwrap();
        }
        std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
        std::fs::write(dir.path().join(".osexport"), "x").unwrap();
        std::fs::create_dir(dir.path().join("sub.osexport")).unwrap();
        let (found, more) = list(dir.path());
        assert_eq!(found.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Alpha", "beta", "gamma"]);
        assert_eq!(more, 0);
        for n in 0..MAX_LISTED {
            std::fs::write(dir.path().join(format!("t{n:03}.osexport")), "{}").unwrap();
        }
        let (found, more) = list(dir.path());
        assert_eq!((found.len(), more), (MAX_LISTED, 3));
    }

    #[test]
    fn what_is_not_a_template_or_is_from_a_newer_express_is_refused_in_words_and_nothing_is_read_wrongly() {
        let dir = tempfile::tempdir().unwrap();
        let file = |name: &str, text: &str| {
            let p = dir.path().join(name);
            std::fs::write(&p, text).unwrap();
            p
        };
        assert!(read(&dir.path().join("none.osexport")).unwrap_err().contains("could not be found"));
        assert!(read(dir.path()).unwrap_err().contains("not a template file"));
        assert!(read(&file("a.osexport", "{ not json")).unwrap_err().contains("not in the form"));
        assert!(read(&file("b.osexport", r#"{"hello": 1}"#)).unwrap_err().contains("not an export template"));
        assert!(read(&file("c.osexport", r#"{"format": "something else"}"#))
            .unwrap_err()
            .contains("not an export template"));
        let newer = format!(r#"{{"format": "{FORMAT_NAME}", "version": 2}}"#);
        assert!(read(&file("d.osexport", &newer)).unwrap_err().contains("newer Express"));
        let broken = format!(r#"{{"format": "{FORMAT_NAME}", "version": 1, "pictureWidth": "wide"}}"#);
        assert!(read(&file("e.osexport", &broken)).unwrap_err().contains("cannot read"));
        let big = dir.path().join("big.osexport");
        std::fs::File::create(&big).unwrap().set_len(MAX_TEMPLATE_BYTES + 1).unwrap();
        assert!(read(&big).unwrap_err().contains("far bigger"));
        assert!(read(Path::new(r"\\server\share\t.osexport")).unwrap_err().contains("another computer"));
    }

    #[test]
    fn a_template_with_odd_options_still_reads_and_takes_the_usual_ones() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("odd.osexport");
        let text = format!(
            r#"{{"format": "{FORMAT_NAME}", "version": 1, "name": " Odd  one ", "formats": ["html"], "options": {{"gif": {{"seconds": "x"}}}}}}"#
        );
        std::fs::write(&p, text).unwrap();
        let t = read(&p).unwrap();
        assert_eq!((t.name.as_str(), t.formats.as_slice()), ("Odd one", ["html".to_string()].as_slice()));
        assert_eq!(t.options, ExportChoices::default());
        // And a number that is not on offer is moved to one that is.
        let text = format!(r#"{{"format": "{FORMAT_NAME}", "version": 1, "options": {{"pdf": {{"marginMm": 22}}}}}}"#);
        std::fs::write(&p, text).unwrap();
        assert_eq!(read(&p).unwrap().options.pdf.margin_mm, 20);
    }

    #[test]
    fn the_logo_travels_inside_the_template_and_comes_out_as_one_file_whatever_the_computer() {
        let dir = tempfile::tempdir().unwrap();
        let logo = logo_png(dir.path());
        let embedded = embed_logo(&logo.display().to_string()).unwrap();
        let mut t = template("With logo");
        t.logo_png = Some(embedded);
        let saved = save(&dir.path().join("t"), &t).unwrap();
        let back = read(&saved).unwrap();
        let out = dir.path().join("elsewhere");
        let one = extract_logo(&back, &out).unwrap();
        let two = extract_logo(&back, &out).unwrap();
        assert_eq!(one, two, "the same picture is the same file");
        assert_eq!(std::fs::read(&one).unwrap(), std::fs::read(&logo).unwrap(), "byte for byte");
        assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1);
        assert!(one.file_name().unwrap().to_str().unwrap().starts_with("logo-"));
    }

    #[test]
    fn a_logo_that_is_not_a_small_readable_png_is_not_put_in_and_not_taken_out() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(embed_logo(""), None);
        assert_eq!(embed_logo(&dir.path().join("none.png").display().to_string()), None);
        let text = dir.path().join("words.png");
        std::fs::write(&text, b"not a picture").unwrap();
        assert_eq!(embed_logo(&text.display().to_string()), None, "not a PNG");
        let big = dir.path().join("big.png");
        std::fs::File::create(&big).unwrap().set_len(MAX_EMBEDDED_LOGO_BYTES as u64 + 1).unwrap();
        assert_eq!(embed_logo(&big.display().to_string()), None, "too big for a template");
        assert_eq!(embed_logo(r"\\server\share\logo.png"), None);
        let out = dir.path().join("out");
        let bad = |encoded: &str| Template { logo_png: Some(encoded.to_string()), ..Template::default() };
        assert_eq!(extract_logo(&bad("!!! not base64 !!!"), &out), None);
        assert_eq!(extract_logo(&bad(&STANDARD.encode(b"just words")), &out), None, "not a PNG");
        assert_eq!(
            extract_logo(&bad(&STANDARD.encode(b"\x89PNG\r\n\x1a\nbroken")), &out),
            None,
            "a PNG that cannot be read"
        );
        assert_eq!(extract_logo(&bad(&"A".repeat(MAX_EMBEDDED_LOGO_BYTES * 2)), &out), None, "too big");
        assert_eq!(extract_logo(&Template::default(), &out), None, "no logo");
        assert!(!out.exists(), "nothing is made for a picture that is refused");
    }

    #[test]
    fn the_folder_is_beside_the_settings() {
        assert_eq!(
            folder_beside(Path::new(r"C:\Users\x\AppData\Roaming\OpenStage Capture Express\settings.json")).unwrap(),
            Path::new(r"C:\Users\x\AppData\Roaming\OpenStage Capture Express\Export templates")
        );
        assert_eq!(folder_beside(Path::new("settings.json")).unwrap(), Path::new("Export templates"));
    }
}
