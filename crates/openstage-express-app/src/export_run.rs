// SPDX-License-Identifier: Apache-2.0
//! Making the exports of a guide in a new folder. Used by the Export dialog (a real guide) and by `EXPORT DEMO.bat` (the made-up
//! guide). Nothing here replaces a file that is already there: every run goes into a new, time-named folder, and a file that
//! would clash stops the run.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

use openstage_capture_render::{BlockText, TextRenderer};
use openstage_express_core::Guide;
use openstage_express_export::docx::{export_docx, DocxOptions};
use openstage_express_export::gif::{export_gif, GifOptions};
use openstage_express_export::html::export_html;
use openstage_express_export::images::{export_images, export_images_as, ImageFormat, DEFAULT_JPEG_QUALITY};
use openstage_express_export::json::export_json;
use openstage_express_export::markdown::export_markdown;
use openstage_express_export::pdf::{export_pdf, PdfOptions, PdfRenderer};
use openstage_express_export::picture::PictureSource;
use openstage_express_export::plan::{plan as make_plan, ExportOptions, Plan};
use openstage_express_export::pptx::{export_pptx, PptxOptions};
use openstage_express_export::share::{make_copy, CopyOptions};
use openstage_express_export::{Export, OutFile};

/// One kind of export the person can choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    Html,
    Markdown,
    Json,
    Images,
    Jpg,
    Gif,
    Pdf,
    Word,
    PowerPoint,
}

impl Format {
    /// All of them, in the order the dialog lists them.
    pub const ALL: [Format; 9] = [
        Format::Html,
        Format::Markdown,
        Format::Json,
        Format::Images,
        Format::Jpg,
        Format::Gif,
        Format::Pdf,
        Format::Word,
        Format::PowerPoint,
    ];

    /// The name on the checkbox.
    pub fn name(self) -> &'static str {
        match self {
            Format::Html => "Web page",
            Format::Markdown => "Markdown",
            Format::Json => "JSON data",
            Format::Images => "Pictures (PNG)",
            Format::Jpg => "Pictures (JPG)",
            Format::Gif => "Animated GIF",
            Format::Pdf => "PDF",
            Format::Word => "Word document",
            Format::PowerPoint => "PowerPoint presentation",
        }
    }

    /// A few words on what it is for, read after the name.
    pub fn about(self) -> &'static str {
        match self {
            Format::Html => "one page that opens in any browser, with the pictures inside",
            Format::Markdown => "text for a wiki or a help system, with a folder of pictures",
            Format::Json => "the steps as data for other programs",
            Format::Images => "one picture file for each step",
            Format::Jpg => "one smaller picture file for each step",
            Format::Gif => "a short animation that steps through the guide",
            Format::Pdf => "a tagged document with contents and bookmarks, made by Microsoft Edge's engine",
            Format::Word => "an editable document with headings, a contents list and the pictures",
            Format::PowerPoint => "one slide for each step, with the text in the notes",
        }
    }

    /// The folder this format goes into inside a run's folder.
    pub fn folder(self) -> &'static str {
        match self {
            Format::Html => "html",
            Format::Markdown => "markdown",
            Format::Json => "json",
            Format::Images => "images",
            Format::Jpg => "jpg",
            Format::Gif => "gif",
            Format::Pdf => "pdf",
            Format::Word => "word",
            Format::PowerPoint => "powerpoint",
        }
    }
}

/// The formats chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    chosen: Vec<Format>,
}

impl Selection {
    #[cfg(test)]
    pub fn none() -> Selection {
        Selection { chosen: Vec::new() }
    }

    pub fn all() -> Selection {
        Selection { chosen: Format::ALL.to_vec() }
    }

    /// Exactly these formats, in the dialog's order.
    pub fn none_then(formats: &[Format]) -> Selection {
        let mut selection = Selection::starting();
        for f in Format::ALL {
            selection.set(f, formats.contains(&f));
        }
        selection
    }

    /// What is chosen when a person has chosen nothing yet: a web page and a PDF.
    pub fn starting() -> Selection {
        Selection { chosen: vec![Format::Html, Format::Pdf] }
    }

    pub fn contains(&self, format: Format) -> bool {
        self.chosen.contains(&format)
    }

    pub fn set(&mut self, format: Format, on: bool) {
        self.chosen.retain(|f| *f != format);
        if on {
            self.chosen.push(format);
        }
        self.chosen.sort_by_key(|f| Format::ALL.iter().position(|x| x == f));
    }

    pub fn is_empty(&self) -> bool {
        self.chosen.is_empty()
    }

    pub fn count(&self) -> usize {
        self.chosen.len()
    }

    pub fn formats(&self) -> &[Format] {
        &self.chosen
    }
}

/// What a run made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The new folder everything is in.
    pub folder: PathBuf,
    /// For each format made, how many files.
    pub made: Vec<(Format, usize)>,
    pub warnings: Vec<String>,
    /// Formats not made, and why.
    pub skipped: Vec<(Format, String)>,
}

impl Report {
    /// One sentence for the status line.
    pub fn sentence(&self) -> String {
        let made = self.made.len();
        let mut words =
            format!("Exported {} format{} to {}.", made, if made == 1 { "" } else { "s" }, self.folder.display());
        for (format, why) in &self.skipped {
            words.push_str(&format!(" {} was not made: {why}", format.name()));
        }
        if !self.warnings.is_empty() {
            let n = self.warnings.len();
            words.push_str(&format!(" {n} warning{}.", if n == 1 { "" } else { "s" }));
        }
        words
    }
}

/// A folder name for a run, from the time: `2026-11-01 10-30-05`.
pub fn run_folder_name(now_iso: &str) -> String {
    now_iso.trim_end_matches('Z').replace('T', " ").replace(':', "-")
}

/// Why a PDF was not made when this computer has no PDF engine.
pub const NO_PDF_ENGINE: &str =
    "this computer has no Microsoft Edge WebView2 Runtime (it comes with Windows 11), so no PDF can be made.";

/// Make the chosen exports of `plan` in `folder` (made if needed; nothing already there is replaced). `pdf` is the engine that
/// makes PDF files, or `None` when this computer has none, in which case a chosen PDF is reported as skipped and the rest are made.
pub fn make(
    plan: &Plan,
    selection: &Selection,
    gif: &GifOptions,
    text: &dyn TextRenderer,
    pdf: Option<&dyn PdfRenderer>,
    folder: &Path,
) -> Result<Report, String> {
    let mut skipped = Vec::new();
    let mut made = Vec::new();
    let mut warnings: Vec<String> = plan.warnings.clone();
    for &format in selection.formats() {
        let export: Export = match format {
            Format::Html => export_html(plan),
            Format::Markdown => export_markdown(plan),
            Format::Json => export_json(plan),
            Format::Images => export_images(plan),
            Format::Jpg => export_images_as(plan, ImageFormat::Jpeg { quality: DEFAULT_JPEG_QUALITY }),
            Format::Gif => export_gif(plan, gif, text),
            Format::Word => {
                export_docx(plan, &DocxOptions::default()).map_err(|e| format!("{}: {e}", format.name()))?
            }
            Format::PowerPoint => {
                export_pptx(plan, &PptxOptions::default()).map_err(|e| format!("{}: {e}", format.name()))?
            }
            Format::Pdf => match pdf {
                Some(renderer) => {
                    export_pdf(plan, &PdfOptions::default(), renderer).map_err(|e| format!("{}: {e}", format.name()))?
                }
                None => {
                    skipped.push((format, NO_PDF_ENGINE.to_string()));
                    continue;
                }
            },
        };
        let written = export.write_to(&folder.join(format.folder())).map_err(|e| e.to_string())?;
        made.push((format, written.len()));
        warnings.extend(export.warnings.iter().map(|w| format!("{}: {w}", format.name())));
    }
    Ok(Report { folder: folder.to_path_buf(), made, warnings, skipped })
}

/// Everything an export needs, owned, so that it can be made on a thread of its own.
pub struct Job {
    pub guide: Guide,
    pub pictures: Arc<dyn PictureSource + Send + Sync>,
    pub selection: Selection,
    /// The new folder everything goes into.
    pub folder: PathBuf,
}

/// The engine that makes PDF files on this computer, or None if there is none.
#[cfg(windows)]
pub fn system_pdf() -> Option<Box<dyn PdfRenderer + Send + Sync>> {
    use openstage_express_windows::pdf::WebView2Pdf;
    WebView2Pdf::runtime_version().is_some().then(|| Box::new(WebView2Pdf) as Box<dyn PdfRenderer + Send + Sync>)
}

#[cfg(not(windows))]
pub fn system_pdf() -> Option<Box<dyn PdfRenderer + Send + Sync>> {
    None
}

/// Make the exports on a thread of their own. The answer arrives on the returned channel.
pub fn spawn(job: Job) -> Receiver<Result<Report, String>> {
    let (tx, rx) = mpsc::channel();
    let started = std::thread::Builder::new().name("express-export".into()).spawn(move || {
        let the_plan = make_plan(&job.guide, &*job.pictures, &BlockText, &ExportOptions::default());
        let pdf = if job.selection.contains(Format::Pdf) { system_pdf() } else { None };
        let result = make(
            &the_plan,
            &job.selection,
            &GifOptions::default(),
            &BlockText,
            pdf.as_deref().map(|p| p as &dyn PdfRenderer),
            &job.folder,
        );
        let _ = tx.send(result);
    });
    if let Err(why) = started {
        // Nothing was started, so say so through the same channel.
        let (tx, rx2) = mpsc::channel();
        let _ = tx.send(Err(format!("The program could not start the export: {why}")));
        return rx2;
    }
    rx
}

/// What making a one-file copy of a guide needs, owned, so that it can be made on a thread of its own.
pub struct CopyJob {
    pub guide: Guide,
    pub pictures: Arc<dyn PictureSource + Send + Sync>,
    pub options: CopyOptions,
    /// The new folder the file goes into.
    pub folder: PathBuf,
}

/// What a copy made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyReport {
    /// The file that was made.
    pub file: PathBuf,
    /// The folder it is in.
    pub folder: PathBuf,
    pub size: usize,
    pub steps: usize,
    pub pictures: usize,
    pub left_out: usize,
    pub with_originals: bool,
    pub warnings: Vec<String>,
}

impl CopyReport {
    /// A few sentences for the status line.
    pub fn sentence(&self) -> String {
        let mut words = format!(
            "Saved a copy as one file: {} ({:.1} MB), with {} and {}.",
            self.file.display(),
            self.size as f64 / 1_048_576.0,
            crate::ui::rows::steps_words(self.steps),
            if self.pictures == 1 { "1 picture".to_string() } else { format!("{} pictures", self.pictures) },
        );
        if self.with_originals {
            words.push_str(" The original pictures are in it, so a blur can be taken away by whoever opens it.");
        } else {
            words.push_str(" The marks are drawn into the pictures and the originals are not in it.");
            if self.left_out > 0 {
                let n = self.left_out;
                words.push_str(&format!(" {n} skipped step{} left out.", if n == 1 { " was" } else { "s were" }));
            }
        }
        if !self.warnings.is_empty() {
            let n = self.warnings.len();
            words.push_str(&format!(" {n} warning{}: {}", if n == 1 { "" } else { "s" }, self.warnings.join(" ")));
        }
        words
    }
}

/// Make a one-file copy of a guide in `folder` (made if needed). The file is never put over one that is already there.
pub fn make_one_file_copy(
    guide: &Guide,
    pictures: &dyn PictureSource,
    text: &dyn TextRenderer,
    options: &CopyOptions,
    folder: &Path,
) -> Result<CopyReport, String> {
    let made = make_copy(guide, pictures, text, options).map_err(|e| e.to_string())?;
    let export =
        Export { files: vec![OutFile { path: made.file_name.clone(), bytes: made.zip.clone() }], warnings: Vec::new() };
    let written = export.write_to(folder).map_err(|e| e.to_string())?;
    Ok(CopyReport {
        file: written.into_iter().next().unwrap_or_else(|| folder.join(&made.file_name)),
        folder: folder.to_path_buf(),
        size: made.zip.len(),
        steps: made.steps,
        pictures: made.pictures,
        left_out: made.left_out,
        with_originals: options.include_originals,
        warnings: made.warnings,
    })
}

/// Make the copy on a thread of its own. The answer arrives on the returned channel.
pub fn spawn_copy(job: CopyJob) -> Receiver<Result<CopyReport, String>> {
    let (tx, rx) = mpsc::channel();
    let started = std::thread::Builder::new().name("express-copy".into()).spawn(move || {
        let _ = tx.send(make_one_file_copy(&job.guide, &*job.pictures, &BlockText, &job.options, &job.folder));
    });
    if let Err(why) = started {
        let (tx, rx2) = mpsc::channel();
        let _ = tx.send(Err(format!("The program could not start making the copy: {why}")));
        return rx2;
    }
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstage_express_export::demo::demo;
    use openstage_express_export::pdf::{PrintPage, RenderError};
    use openstage_express_export::plan::{plan, ExportOptions};

    struct StandIn;

    impl PdfRenderer for StandIn {
        fn render(&self, _page: &PrintPage<'_>) -> Result<Vec<u8>, RenderError> {
            Ok(b"%PDF-1.4\n/StructTreeRoot /Marked true /Lang (en) /Outlines\n".to_vec())
        }
    }

    fn the_plan() -> Plan {
        let d = demo();
        plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default())
    }

    #[test]
    fn only_the_chosen_formats_are_made_and_each_has_its_own_folder() {
        let dir = tempfile::tempdir().unwrap();
        let mut selection = Selection::none();
        selection.set(Format::Markdown, true);
        selection.set(Format::Word, true);
        let report = make(&the_plan(), &selection, &GifOptions::default(), &BlockText, None, dir.path()).unwrap();
        assert_eq!(report.made.iter().map(|(f, _)| *f).collect::<Vec<_>>(), [Format::Markdown, Format::Word]);
        assert!(dir.path().join("markdown/How to submit an assignment.md").is_file());
        assert!(dir.path().join("word/How to submit an assignment.docx").is_file());
        assert!(!dir.path().join("html").exists() && !dir.path().join("pdf").exists());
        assert!(report.skipped.is_empty());
    }

    #[test]
    fn a_pdf_without_an_engine_is_skipped_in_words_and_the_rest_are_made() {
        let dir = tempfile::tempdir().unwrap();
        let mut selection = Selection::starting();
        selection.set(Format::Json, true);
        let report = make(&the_plan(), &selection, &GifOptions::default(), &BlockText, None, dir.path()).unwrap();
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].0, Format::Pdf);
        assert!(report.skipped[0].1.contains("WebView2"));
        assert!(dir.path().join("html").is_dir() && dir.path().join("json").is_dir());
        let said = report.sentence();
        assert!(said.starts_with("Exported 2 formats to "), "{said}");
        assert!(said.contains("PDF was not made"), "{said}");
    }

    #[test]
    fn every_format_can_be_made_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let report =
            make(&the_plan(), &Selection::all(), &GifOptions::default(), &BlockText, Some(&StandIn), dir.path())
                .unwrap();
        assert_eq!(report.made.len(), 9);
        for format in Format::ALL {
            assert!(dir.path().join(format.folder()).is_dir(), "{}", format.folder());
        }
    }

    #[test]
    fn a_second_run_into_the_same_folder_stops_instead_of_replacing() {
        let dir = tempfile::tempdir().unwrap();
        let mut selection = Selection::none();
        selection.set(Format::Json, true);
        make(&the_plan(), &selection, &GifOptions::default(), &BlockText, None, dir.path()).unwrap();
        let again = make(&the_plan(), &selection, &GifOptions::default(), &BlockText, None, dir.path());
        assert!(again.unwrap_err().contains("without replacing it"));
    }

    #[test]
    fn the_selection_keeps_the_dialog_order_and_counts() {
        let mut s = Selection::none();
        assert!(s.is_empty());
        s.set(Format::PowerPoint, true);
        s.set(Format::Html, true);
        s.set(Format::Gif, true);
        assert_eq!(s.formats(), [Format::Html, Format::Gif, Format::PowerPoint]);
        s.set(Format::Gif, false);
        assert_eq!(s.count(), 2);
        assert!(!s.contains(Format::Gif));
        assert_eq!(Selection::starting().formats(), [Format::Html, Format::Pdf]);
    }

    #[test]
    fn every_format_has_words_and_a_folder_of_its_own() {
        let mut folders = std::collections::HashSet::new();
        for f in Format::ALL {
            assert!(!f.name().is_empty() && !f.about().is_empty());
            assert!(folders.insert(f.folder()), "{}", f.folder());
        }
    }

    #[test]
    fn the_run_folder_name_has_no_characters_windows_forbids() {
        let name = run_folder_name("2026-11-01T10:30:05Z");
        assert_eq!(name, "2026-11-01 10-30-05");
        assert!(!name.contains([':', '/', '\\']));
    }
}
