// SPDX-License-Identifier: Apache-2.0
//! What the Export screen chooses for each format besides the pictures (spec 10.2 and 10.3): how the animated GIF is paced, the paper,
//! margins, cover, contents and lines of text of the PDF and the Word document, and the first and last slides of the PowerPoint. They are
//! kept in the settings (`exportOptions`) and in an export template (`export_templates.rs`), and turned into the export crate's own
//! option types only when an export is made, so a number that is out of range or a word this program does not know never reaches it.

use openstage_capture_render::Picture;
use openstage_express_export::docx::{self, DocxOptions};
use openstage_express_export::gif::{GifOptions, ALLOWED_SECONDS};
use openstage_express_export::pdf::{self, Orientation, PageSize, PdfOptions};
use openstage_express_export::picture::decode_png;
use openstage_express_export::pptx::{LogoPicture, PptxOptions};
use openstage_express_export::watermark::{Place, Watermark};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// The most letters a header or footer line may have. A longer one is cut, so one line cannot fill a page.
pub const MAX_LINE_CHARS: usize = 120;
/// The margins on offer, in millimetres.
pub const MARGIN_CHOICES: [u32; 6] = [10, 15, 20, 25, 30, 40];
/// The widths of the animated GIF on offer, in pixels.
pub const GIF_WIDTH_CHOICES: [u32; 5] = [480, 640, 800, 1024, 1280];

/// Read a value that, if it cannot be read, is the usual one: a settings file or an export template with something odd in one place
/// must never cost the person the rest of it.
pub fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let value = Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

/// The paper. A word this program does not know means A4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Paper {
    Letter,
    A3,
    A5,
    Legal,
    /// Last in the list, because serde takes any other word to be this one.
    #[default]
    #[serde(other)]
    A4,
}

impl Paper {
    pub const ALL: [Paper; 5] = [Paper::A4, Paper::Letter, Paper::A3, Paper::A5, Paper::Legal];

    pub fn name(self) -> &'static str {
        match self {
            Paper::A4 => "A4",
            Paper::Letter => "Letter",
            Paper::A3 => "A3",
            Paper::A5 => "A5",
            Paper::Legal => "Legal",
        }
    }

    fn size(self) -> PageSize {
        match self {
            Paper::A4 => PageSize::A4,
            Paper::Letter => PageSize::Letter,
            Paper::A3 => PageSize::A3,
            Paper::A5 => PageSize::A5,
            Paper::Legal => PageSize::Legal,
        }
    }
}

/// Which way the paper is turned. A word this program does not know means upright.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Turn {
    Landscape,
    /// Last in the list, because serde takes any other word to be this one.
    #[default]
    #[serde(other)]
    Portrait,
}

impl Turn {
    pub fn name(self) -> &'static str {
        match self {
            Turn::Portrait => "Upright (portrait)",
            Turn::Landscape => "On its side (landscape)",
        }
    }

    fn orientation(self) -> Orientation {
        match self {
            Turn::Portrait => Orientation::Portrait,
            Turn::Landscape => Orientation::Landscape,
        }
    }
}

/// The animated GIF (spec 10.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GifChoices {
    /// Pixels across, one of [`GIF_WIDTH_CHOICES`].
    pub width: u32,
    /// How long each step stays: 1, 2, 3, 5 or 8 seconds.
    pub seconds: u32,
    pub loop_forever: bool,
    pub title_card: bool,
    pub progress_bar: bool,
}

impl Default for GifChoices {
    fn default() -> GifChoices {
        let usual = GifOptions::default();
        GifChoices {
            width: usual.max_width,
            seconds: usual.seconds_per_step,
            loop_forever: usual.loop_forever,
            title_card: usual.title_card,
            progress_bar: usual.progress_bar,
        }
    }
}

/// The PDF (spec 10.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PdfChoices {
    pub paper: Paper,
    pub turn: Turn,
    pub margin_mm: u32,
    pub cover_page: bool,
    pub contents: bool,
    pub page_numbers: bool,
    pub header_text: String,
    pub footer_text: String,
    pub new_page_per_step: bool,
}

impl Default for PdfChoices {
    fn default() -> PdfChoices {
        let usual = PdfOptions::default();
        PdfChoices {
            paper: Paper::A4,
            turn: Turn::Portrait,
            margin_mm: usual.margin_mm,
            cover_page: usual.cover_page,
            contents: usual.contents,
            page_numbers: usual.page_numbers,
            header_text: String::new(),
            footer_text: String::new(),
            new_page_per_step: usual.new_page_per_step,
        }
    }
}

/// The Word document (spec 10.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WordChoices {
    pub paper: Paper,
    pub turn: Turn,
    pub margin_mm: u32,
    pub contents: bool,
    pub page_numbers: bool,
    pub new_page_per_step: bool,
}

impl Default for WordChoices {
    fn default() -> WordChoices {
        let usual = DocxOptions::default();
        WordChoices {
            paper: Paper::A4,
            turn: Turn::Portrait,
            margin_mm: usual.margin_mm,
            contents: usual.contents,
            page_numbers: usual.page_numbers,
            new_page_per_step: usual.new_page_per_step,
        }
    }
}

/// The PowerPoint presentation (spec 10.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PowerPointChoices {
    pub cover_slide: bool,
    pub agenda_slide: bool,
    pub footer_text: String,
    pub slide_numbers: bool,
    pub speaker_notes: bool,
    /// The chosen picture in the corner of every slide.
    pub logo: bool,
    /// What a screen reader says for the logo.
    pub logo_description: String,
}

impl Default for PowerPointChoices {
    fn default() -> PowerPointChoices {
        let usual = PptxOptions::default();
        PowerPointChoices {
            cover_slide: usual.cover_slide,
            agenda_slide: usual.agenda_slide,
            footer_text: String::new(),
            slide_numbers: usual.slide_numbers,
            speaker_notes: usual.speaker_notes,
            logo: false,
            logo_description: "Logo".to_string(),
        }
    }
}

/// Everything the Export screen chooses for the four formats that have options of their own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ExportChoices {
    pub gif: GifChoices,
    pub pdf: PdfChoices,
    pub word: WordChoices,
    pub powerpoint: PowerPointChoices,
    /// The picture file used for the watermark and the PowerPoint logo (a PNG). Empty when none is chosen.
    pub logo_file: String,
    pub watermark: WatermarkChoices,
}

/// The watermark's opacities on offer, in percent.
pub const OPACITY_CHOICES: [u32; 5] = [15, 30, 50, 75, 100];
/// The watermark's widths on offer, in percent of the picture's width.
pub const SIZE_CHOICES: [u32; 5] = [10, 15, 20, 30, 40];
/// The room round a watermark on offer, in percent of the picture's width.
pub const PADDING_CHOICES: [u32; 4] = [0, 2, 4, 8];
/// The biggest picture file that is read for a watermark or a logo.
pub const MAX_LOGO_BYTES: u64 = 8 * 1024 * 1024;

/// Where the watermark goes. A word this program does not know means the bottom right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Spot {
    TopLeft,
    TopCentre,
    TopRight,
    MiddleLeft,
    Centre,
    MiddleRight,
    BottomLeft,
    BottomCentre,
    /// Last in the list, because serde takes any other word to be this one.
    #[default]
    #[serde(other)]
    BottomRight,
}

impl Spot {
    pub const ALL: [Spot; 9] = [
        Spot::TopLeft,
        Spot::TopCentre,
        Spot::TopRight,
        Spot::MiddleLeft,
        Spot::Centre,
        Spot::MiddleRight,
        Spot::BottomLeft,
        Spot::BottomCentre,
        Spot::BottomRight,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Spot::TopLeft => "Top left",
            Spot::TopCentre => "Top centre",
            Spot::TopRight => "Top right",
            Spot::MiddleLeft => "Middle left",
            Spot::Centre => "Centre",
            Spot::MiddleRight => "Middle right",
            Spot::BottomLeft => "Bottom left",
            Spot::BottomCentre => "Bottom centre",
            Spot::BottomRight => "Bottom right",
        }
    }

    fn place(self) -> Place {
        Place::ALL[Spot::ALL.iter().position(|s| *s == self).unwrap_or(8)]
    }
}

/// The watermark: the same picture on every exported picture (spec 10.2). The picture itself is [`ExportChoices::logo_file`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WatermarkChoices {
    pub on: bool,
    pub place: Spot,
    /// How solid it is, in percent.
    pub opacity: u32,
    /// How wide it is, in percent of the picture's width.
    pub size: u32,
    /// The room between it and the edge, in percent of the picture's width.
    pub padding: u32,
}

impl Default for WatermarkChoices {
    fn default() -> WatermarkChoices {
        WatermarkChoices { on: false, place: Spot::BottomRight, opacity: 30, size: 20, padding: 2 }
    }
}

/// A picture read from a file for a watermark or a logo: its pixels, and the PNG bytes it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogoFile {
    pub picture: Picture,
    pub png: Vec<u8>,
}

/// Read the picture the person chose for a watermark or a logo: a PNG file on this computer (not one on another computer, which Windows
/// would connect to with the person's sign-in), of a size that can be used. The words of an error are for the person.
pub fn load_logo(path: &str) -> Result<LogoFile, String> {
    let named = std::path::Path::new(path.trim());
    if path.trim().is_empty() {
        return Err("No picture is chosen.".to_string());
    }
    if openstage_express_core::fsutil::is_network_path(named) {
        return Err(
            "The picture is on another computer. Express keeps to this computer, so copy it here first.".to_string()
        );
    }
    let meta = std::fs::metadata(named).map_err(|e| format!("The picture could not be found ({e})."))?;
    if !meta.is_file() {
        return Err("The place chosen is not a picture file.".to_string());
    }
    if meta.len() > MAX_LOGO_BYTES {
        return Err(format!(
            "The picture file is bigger than {} MB, which is more than a logo needs.",
            MAX_LOGO_BYTES >> 20
        ));
    }
    let png = std::fs::read(named).map_err(|e| format!("The picture could not be read ({e})."))?;
    if png.get(..8) != Some(b"\x89PNG\r\n\x1a\n") {
        return Err("The picture is not a PNG file. Save it as a PNG picture and choose that.".to_string());
    }
    let picture = decode_png(&png)?;
    Ok(LogoFile { picture, png })
}

/// The nearest of `choices` to `value`; the smaller one when two are as near.
fn nearest(choices: &[u32], value: u32) -> u32 {
    choices.iter().copied().min_by_key(|c| (c.abs_diff(value), *c)).unwrap_or(value)
}

/// A line of text for a header or a footer: the edges trimmed, white space inside squeezed to single spaces, controls dropped and cut to
/// [`MAX_LINE_CHARS`].
pub fn tidy_line(text: &str) -> String {
    let squeezed: String = text.chars().filter(|c| !c.is_control() || c.is_whitespace()).collect();
    let squeezed = squeezed.split_whitespace().collect::<Vec<_>>().join(" ");
    squeezed.chars().take(MAX_LINE_CHARS).collect()
}

impl ExportChoices {
    /// The same choices with every number moved to one that is on offer and every line of text tidied. Used when choices come from a
    /// file, so that a hand-edited number is shown as the nearest choice and never costs the other settings.
    pub fn normalised(&self) -> ExportChoices {
        let mut out = self.clone();
        out.gif.width = nearest(&GIF_WIDTH_CHOICES, out.gif.width);
        out.gif.seconds = nearest(&ALLOWED_SECONDS, out.gif.seconds);
        out.pdf.margin_mm = nearest(&MARGIN_CHOICES, out.pdf.margin_mm);
        out.pdf.header_text = tidy_line(&out.pdf.header_text);
        out.pdf.footer_text = tidy_line(&out.pdf.footer_text);
        out.word.margin_mm = nearest(&MARGIN_CHOICES, out.word.margin_mm);
        out.powerpoint.footer_text = tidy_line(&out.powerpoint.footer_text);
        out.powerpoint.logo_description = tidy_line(&out.powerpoint.logo_description);
        out.watermark.opacity = nearest(&OPACITY_CHOICES, out.watermark.opacity);
        out.watermark.size = nearest(&SIZE_CHOICES, out.watermark.size);
        out.watermark.padding = nearest(&PADDING_CHOICES, out.watermark.padding);
        out.logo_file = out.logo_file.trim().to_string();
        out
    }

    pub fn gif_options(&self) -> GifOptions {
        let c = self.normalised().gif;
        GifOptions {
            max_width: c.width,
            seconds_per_step: c.seconds,
            loop_forever: c.loop_forever,
            title_card: c.title_card,
            progress_bar: c.progress_bar,
            ..GifOptions::default()
        }
    }

    pub fn pdf_options(&self) -> PdfOptions {
        let c = self.normalised().pdf;
        PdfOptions {
            page_size: c.paper.size(),
            orientation: c.turn.orientation(),
            margin_mm: c.margin_mm.clamp(pdf::MIN_MARGIN_MM, pdf::MAX_MARGIN_MM),
            cover_page: c.cover_page,
            contents: c.contents,
            page_numbers: c.page_numbers,
            header_text: c.header_text,
            footer_text: c.footer_text,
            new_page_per_step: c.new_page_per_step,
        }
    }

    pub fn docx_options(&self) -> DocxOptions {
        let c = self.normalised().word;
        DocxOptions {
            page_size: c.paper.size(),
            orientation: c.turn.orientation(),
            margin_mm: c.margin_mm.clamp(docx::MIN_MARGIN_MM, docx::MAX_MARGIN_MM),
            contents: c.contents,
            page_numbers: c.page_numbers,
            new_page_per_step: c.new_page_per_step,
        }
    }

    /// The options of the PowerPoint file, with `logo` in the corner of every slide when that is chosen and a picture was read.
    pub fn pptx_options(&self, logo: Option<&LogoFile>) -> PptxOptions {
        let c = self.normalised().powerpoint;
        let words = if c.logo_description.trim().is_empty() { "Logo".to_string() } else { c.logo_description.clone() };
        PptxOptions {
            cover_slide: c.cover_slide,
            agenda_slide: c.agenda_slide,
            footer_text: c.footer_text,
            slide_numbers: c.slide_numbers,
            speaker_notes: c.speaker_notes,
            logo: logo.filter(|_| c.logo).map(|file| LogoPicture {
                png: file.png.clone(),
                width: file.picture.width,
                height: file.picture.height,
                description: words,
            }),
        }
    }

    /// Whether the watermark or the PowerPoint logo needs the picture file read.
    pub fn needs_logo_file(&self, powerpoint_chosen: bool) -> bool {
        self.watermark.on || (self.powerpoint.logo && powerpoint_chosen)
    }

    /// The watermark, when it is on and a picture was read.
    pub fn watermark_options(&self, logo: Option<&LogoFile>) -> Option<Watermark> {
        let c = self.normalised().watermark;
        let file = logo.filter(|_| c.on)?;
        Some(Watermark {
            picture: file.picture.clone(),
            place: c.place.place(),
            opacity_percent: c.opacity,
            size_percent: c.size,
            padding_percent: c.padding,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_usual_choices_are_the_export_crates_usual_options() {
        let c = ExportChoices::default();
        assert_eq!(c.gif_options(), GifOptions::default());
        assert_eq!(c.pdf_options(), PdfOptions::default());
        assert_eq!(c.docx_options(), DocxOptions::default());
        assert_eq!(c.pptx_options(None), PptxOptions::default());
        assert_eq!(c.normalised(), c, "the usual choices are all on offer");
    }

    #[test]
    fn every_choice_reaches_its_option() {
        let mut c = ExportChoices {
            gif: GifChoices { width: 1024, seconds: 5, loop_forever: false, title_card: true, progress_bar: true },
            ..ExportChoices::default()
        };
        c.pdf = PdfChoices {
            paper: Paper::Legal,
            turn: Turn::Landscape,
            margin_mm: 30,
            cover_page: true,
            contents: false,
            page_numbers: false,
            header_text: "  Draft   for  review ".to_string(),
            footer_text: "Acme".to_string(),
            new_page_per_step: true,
        };
        c.word = WordChoices {
            paper: Paper::Letter,
            turn: Turn::Landscape,
            margin_mm: 15,
            contents: false,
            page_numbers: false,
            new_page_per_step: true,
        };
        c.powerpoint = PowerPointChoices {
            cover_slide: false,
            agenda_slide: true,
            footer_text: "Team".to_string(),
            slide_numbers: false,
            speaker_notes: false,
            ..PowerPointChoices::default()
        };
        let gif = c.gif_options();
        assert_eq!((gif.max_width, gif.seconds_per_step, gif.loop_forever), (1024, 5, false));
        assert!(gif.title_card && gif.progress_bar);
        let pdf = c.pdf_options();
        assert_eq!((pdf.page_size, pdf.orientation, pdf.margin_mm), (PageSize::Legal, Orientation::Landscape, 30));
        assert!(pdf.cover_page && !pdf.contents && !pdf.page_numbers && pdf.new_page_per_step);
        assert_eq!((pdf.header_text.as_str(), pdf.footer_text.as_str()), ("Draft for review", "Acme"));
        let word = c.docx_options();
        assert_eq!((word.page_size, word.orientation, word.margin_mm), (PageSize::Letter, Orientation::Landscape, 15));
        assert!(!word.contents && !word.page_numbers && word.new_page_per_step);
        let pptx = c.pptx_options(None);
        assert!(!pptx.cover_slide && pptx.agenda_slide && !pptx.slide_numbers && !pptx.speaker_notes);
        assert_eq!(pptx.footer_text, "Team");
    }

    #[test]
    fn a_number_that_is_not_on_offer_becomes_the_nearest_one_that_is() {
        let mut c = ExportChoices::default();
        c.gif.width = 900;
        c.gif.seconds = 4;
        c.pdf.margin_mm = 22;
        c.word.margin_mm = 1000;
        let n = c.normalised();
        assert_eq!((n.gif.width, n.gif.seconds, n.pdf.margin_mm, n.word.margin_mm), (800, 3, 20, 40));
        c.gif.width = 0;
        c.gif.seconds = 0;
        c.pdf.margin_mm = 0;
        let n = c.normalised();
        assert_eq!((n.gif.width, n.gif.seconds, n.pdf.margin_mm), (480, 1, 10));
        c.gif.seconds = 6;
        assert_eq!(c.normalised().gif.seconds, 5, "halfway between two: the smaller one");
    }

    #[test]
    fn a_line_of_text_is_tidied_and_cut() {
        assert_eq!(tidy_line("  a \t b\n c  "), "a b c");
        assert_eq!(tidy_line("a\u{0}b\u{7}c"), "abc");
        assert_eq!(tidy_line(&"x".repeat(500)).chars().count(), MAX_LINE_CHARS);
        assert_eq!(tidy_line("   "), "");
    }

    #[test]
    fn words_this_program_does_not_know_mean_the_usual_ones_and_cost_nothing_else() {
        let c: ExportChoices = serde_json::from_str(
            r#"{"pdf": {"paper": "tabloid", "turn": "sideways", "marginMm": 30, "coverPage": true}, "gif": {"seconds": 8}}"#,
        )
        .unwrap();
        assert_eq!((c.pdf.paper, c.pdf.turn, c.pdf.margin_mm, c.pdf.cover_page), (Paper::A4, Turn::Portrait, 30, true));
        assert_eq!(c.gif.seconds, 8);
        assert_eq!(c.gif.width, 800, "what is not there is as usual");
        assert_eq!(c.word, WordChoices::default());
    }

    #[test]
    fn lenient_reading_falls_back_to_the_usual_choices() {
        #[derive(Deserialize)]
        struct Holder {
            #[serde(default, deserialize_with = "lenient")]
            options: ExportChoices,
            other: u32,
        }
        let h: Holder = serde_json::from_str(r#"{"options": {"gif": {"seconds": "three"}}, "other": 7}"#).unwrap();
        assert_eq!(h.options, ExportChoices::default());
        assert_eq!(h.other, 7, "the rest of the file is read");
        let h: Holder = serde_json::from_str(r#"{"options": 12, "other": 1}"#).unwrap();
        assert_eq!(h.options, ExportChoices::default());
        let h: Holder = serde_json::from_str(r#"{"other": 1}"#).unwrap();
        assert_eq!(h.options, ExportChoices::default());
    }

    #[test]
    fn the_choices_survive_being_written_and_read() {
        let mut c = ExportChoices::default();
        c.pdf.paper = Paper::A3;
        c.pdf.header_text = "Head".to_string();
        c.powerpoint.agenda_slide = true;
        let text = serde_json::to_string(&c).unwrap();
        assert!(text.contains("\"paper\":\"a3\"") && text.contains("\"agendaSlide\":true"), "{text}");
        assert_eq!(serde_json::from_str::<ExportChoices>(&text).unwrap(), c);
    }

    fn png_file(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        let mut canvas = openstage_capture_render::Canvas::new(8, 4);
        canvas.clear(openstage_capture_render::Rgb(1, 2, 3));
        let path = dir.join(name);
        std::fs::write(&path, openstage_express_export::picture::encode_png(&canvas)).unwrap();
        path
    }

    #[test]
    fn a_png_file_is_read_and_everything_else_is_refused_in_words() {
        let dir = tempfile::tempdir().unwrap();
        let good = png_file(dir.path(), "logo.png");
        let file = load_logo(&good.display().to_string()).unwrap();
        assert_eq!((file.picture.width, file.picture.height), (8, 4));
        assert_eq!(file.png, std::fs::read(&good).unwrap(), "the bytes are the file's own");
        assert!(
            load_logo(&format!("  {}  ", good.display())).is_ok(),
            "spaces round a pasted place are not part of it"
        );
        assert_eq!(load_logo("   ").unwrap_err(), "No picture is chosen.");
        assert!(load_logo(&dir.path().join("none.png").display().to_string())
            .unwrap_err()
            .contains("could not be found"));
        assert!(load_logo(&dir.path().display().to_string()).unwrap_err().contains("not a picture file"));
        let text = dir.path().join("words.png");
        std::fs::write(&text, b"hello").unwrap();
        assert!(load_logo(&text.display().to_string()).unwrap_err().contains("not a PNG file"));
        // A file that starts like a PNG and is not one that can be read.
        let broken = dir.path().join("broken.png");
        std::fs::write(&broken, b"\x89PNG\r\n\x1a\nthen nothing sensible").unwrap();
        assert!(load_logo(&broken.display().to_string()).unwrap_err().contains("PNG"));
        // Too big a file is refused before it is read.
        let big = dir.path().join("big.png");
        let handle = std::fs::File::create(&big).unwrap();
        handle.set_len(MAX_LOGO_BYTES + 1).unwrap();
        assert!(load_logo(&big.display().to_string()).unwrap_err().contains("bigger than 8 MB"));
        // A file on another computer is not reached for.
        assert!(load_logo(r"\\server\share\logo.png").unwrap_err().contains("another computer"));
    }

    #[test]
    fn the_watermark_and_the_logo_follow_the_choices_and_need_a_picture() {
        let dir = tempfile::tempdir().unwrap();
        let file = load_logo(&png_file(dir.path(), "logo.png").display().to_string()).unwrap();
        let mut c = ExportChoices::default();
        assert!(!c.needs_logo_file(true), "nothing asks for the picture to begin with");
        assert!(c.watermark_options(Some(&file)).is_none(), "off: no watermark, whatever picture there is");
        c.watermark = WatermarkChoices { on: true, place: Spot::TopCentre, opacity: 50, size: 30, padding: 4 };
        assert!(c.needs_logo_file(false));
        assert!(c.watermark_options(None).is_none(), "on but no picture read");
        let w = c.watermark_options(Some(&file)).unwrap();
        assert_eq!((w.place, w.opacity_percent, w.size_percent, w.padding_percent), (Place::TopCentre, 50, 30, 4));
        assert_eq!(w.picture, file.picture);
        // The PowerPoint logo is only for PowerPoint, and only when asked for.
        let mut d = ExportChoices::default();
        assert!(!d.needs_logo_file(true));
        d.powerpoint.logo = true;
        assert!(d.needs_logo_file(true) && !d.needs_logo_file(false), "not needed when PowerPoint is not chosen");
        assert!(d.pptx_options(None).logo.is_none());
        d.powerpoint.logo_description = "  The  Acme logo ".to_string();
        let logo = d.pptx_options(Some(&file)).logo.unwrap();
        assert_eq!((logo.width, logo.height, logo.description.as_str()), (8, 4, "The Acme logo"));
        assert_eq!(logo.png, file.png);
        d.powerpoint.logo_description = "   ".to_string();
        assert_eq!(d.pptx_options(Some(&file)).logo.unwrap().description, "Logo", "never left with no words");
        c.powerpoint.logo = false;
        assert!(c.pptx_options(Some(&file)).logo.is_none());
    }

    #[test]
    fn the_places_match_the_export_crates_nine_places_in_order() {
        let names: Vec<&str> = Spot::ALL.iter().map(|s| s.name()).collect();
        assert_eq!(
            names,
            [
                "Top left",
                "Top centre",
                "Top right",
                "Middle left",
                "Centre",
                "Middle right",
                "Bottom left",
                "Bottom centre",
                "Bottom right"
            ]
        );
        for (spot, place) in Spot::ALL.iter().zip(Place::ALL) {
            assert_eq!(spot.place(), place, "{}", spot.name());
        }
        let c: WatermarkChoices = serde_json::from_str(r#"{"place": "upstairs", "opacity": 77}"#).unwrap();
        assert_eq!((c.place, c.opacity, c.size), (Spot::BottomRight, 77, 20));
    }

    #[test]
    fn watermark_numbers_that_are_not_on_offer_become_the_nearest_that_are() {
        let mut c = ExportChoices::default();
        c.watermark.opacity = 0;
        c.watermark.size = 33;
        c.watermark.padding = 6;
        c.logo_file = "  C:\\logo.png ".to_string();
        let n = c.normalised();
        assert_eq!((n.watermark.opacity, n.watermark.size, n.watermark.padding), (15, 30, 4));
        assert_eq!(n.logo_file, "C:\\logo.png");
    }
}
