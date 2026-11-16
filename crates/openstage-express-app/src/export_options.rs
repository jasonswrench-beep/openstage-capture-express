// SPDX-License-Identifier: Apache-2.0
//! What the Export screen chooses for each format besides the pictures (spec 10.2 and 10.3): how the animated GIF is paced, the paper,
//! margins, cover, contents and lines of text of the PDF and the Word document, and the first and last slides of the PowerPoint. They are
//! kept in the settings (`exportOptions`) and in an export template (`export_templates.rs`), and turned into the export crate's own
//! option types only when an export is made, so a number that is out of range or a word this program does not know never reaches it.

use openstage_express_export::docx::{self, DocxOptions};
use openstage_express_export::gif::{GifOptions, ALLOWED_SECONDS};
use openstage_express_export::pdf::{self, Orientation, PageSize, PdfOptions};
use openstage_express_export::pptx::PptxOptions;
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

    pub fn pptx_options(&self) -> PptxOptions {
        let c = self.normalised().powerpoint;
        PptxOptions {
            cover_slide: c.cover_slide,
            agenda_slide: c.agenda_slide,
            footer_text: c.footer_text,
            slide_numbers: c.slide_numbers,
            speaker_notes: c.speaker_notes,
        }
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
        assert_eq!(c.pptx_options(), PptxOptions::default());
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
        let pptx = c.pptx_options();
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
}
