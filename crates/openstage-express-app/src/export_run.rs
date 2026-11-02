// SPDX-License-Identifier: Apache-2.0
//! Making the exports of a guide in a new folder. Used by the Export dialog (a real guide) and by `EXPORT DEMO.bat` (the made-up
//! guide). Nothing here replaces a file that is already there: every run goes into a new, time-named folder, and a file that
//! would clash stops the run.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

use openstage_capture_render::TextRenderer;
use openstage_express_core::Guide;
use openstage_express_export::docx::{export_docx, DocxOptions};
use openstage_express_export::gif::{export_gif, GifOptions};
use openstage_express_export::html::export_html;
use openstage_express_export::images::{export_images, export_images_as, ImageFormat, DEFAULT_JPEG_QUALITY};
use openstage_express_export::json::export_json;
use openstage_express_export::markdown::export_markdown;
use openstage_express_export::pdf::{export_pdf, PdfOptions, PdfRenderer};
use openstage_express_export::picture::PictureSource;
use openstage_express_export::plan::{plan as make_plan, ExportOptions, Plan, Preview, DEFAULT_IMAGE_WIDTH};
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

/// What the Export screen chooses about the pictures (spec 10.4 and 10.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PictureChoices {
    /// How many pixels across every picture is made.
    pub width: u32,
    /// The quality of the pictures made as JPG files, and of the JPG pictures inside documents, from 1 to 100.
    pub jpg_quality: u8,
    /// Only the first this many steps (a preview), or all of them.
    pub first_steps: Option<usize>,
    /// Whether the web page, Word, PowerPoint and PDF keep their pictures as JPG (smaller files) instead of PNG (every pixel as it is).
    pub documents_jpg: bool,
}

impl Default for PictureChoices {
    fn default() -> PictureChoices {
        PictureChoices {
            width: DEFAULT_IMAGE_WIDTH,
            jpg_quality: DEFAULT_JPEG_QUALITY,
            first_steps: None,
            documents_jpg: false,
        }
    }
}

impl PictureChoices {
    /// What the plan of the export is made with.
    pub fn export_options(&self) -> ExportOptions {
        ExportOptions {
            image_width: self.width,
            first_steps: self.first_steps,
            document_pictures: if self.documents_jpg {
                ImageFormat::Jpeg { quality: self.jpg_quality }
            } else {
                ImageFormat::Png
            },
        }
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
    /// Set when a preview was asked for.
    pub preview: Option<Preview>,
}

impl Report {
    /// One sentence for the status line.
    pub fn sentence(&self) -> String {
        let made = self.made.len();
        let mut words =
            format!("Exported {} format{} to {}.", made, if made == 1 { "" } else { "s" }, self.folder.display());
        if let Some(preview) = &self.preview {
            if preview.is_cut() {
                let first = if preview.shown == 1 {
                    "the first step".to_string()
                } else {
                    format!("the first {}", preview.shown)
                };
                words.push_str(&format!(
                    " It is a preview, not the whole guide: {first} of the guide's {}.",
                    crate::ui::rows::steps_words(preview.total)
                ));
            } else {
                words.push_str(&format!(
                    " You asked for a preview, but the guide has only {}, so all of it is in.",
                    crate::ui::rows::steps_words(preview.total)
                ));
            }
        }
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

/// The folder name of a run that is a preview of the first `shown` steps, such as `2026-11-01 10-30-05 (preview of 5 steps)`, so a
/// folder of a preview is never taken for the whole guide.
pub fn preview_run_folder_name(now_iso: &str, shown: usize) -> String {
    format!("{} (preview of {})", run_folder_name(now_iso), crate::ui::rows::steps_words(shown.max(1)))
}

/// Why a PDF was not made when this computer has no PDF engine.
pub const NO_PDF_ENGINE: &str =
    "this computer has no Microsoft Edge WebView2 Runtime (it comes with Windows 11), so no PDF can be made.";

/// Make the chosen exports of `plan` in `folder` (made if needed; nothing already there is replaced). `pdf` is the engine that
/// makes PDF files, or `None` when this computer has none, in which case a chosen PDF is reported as skipped and the rest are made.
/// `jpg_quality` (1 to 100) is for the pictures made as JPG files.
pub fn make(
    plan: &Plan,
    selection: &Selection,
    gif: &GifOptions,
    jpg_quality: u8,
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
            Format::Jpg => export_images_as(plan, ImageFormat::Jpeg { quality: jpg_quality }),
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
    Ok(Report { folder: folder.to_path_buf(), made, warnings, skipped, preview: plan.preview })
}

/// Everything an export needs, owned, so that it can be made on a thread of its own.
pub struct Job {
    pub guide: Guide,
    pub pictures: Arc<dyn PictureSource + Send + Sync>,
    pub selection: Selection,
    /// How wide the pictures are, how good the JPGs are, and whether this is a preview.
    pub choices: PictureChoices,
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
        let text = crate::text::renderer();
        let the_plan = make_plan(&job.guide, &*job.pictures, text, &job.choices.export_options());
        let pdf = if job.selection.contains(Format::Pdf) { system_pdf() } else { None };
        let result = make(
            &the_plan,
            &job.selection,
            &GifOptions::default(),
            job.choices.jpg_quality,
            text,
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
        let _ =
            tx.send(make_one_file_copy(&job.guide, &*job.pictures, crate::text::renderer(), &job.options, &job.folder));
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
    use openstage_capture_render::BlockText;
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

    /// The program itself (not a test that passes a renderer) draws the words on marks with Windows' text, in a copy as in an export.
    #[cfg(windows)]
    #[test]
    fn a_copy_made_by_the_program_has_the_words_on_its_marks_in_windows_text_not_in_blocks() {
        use openstage_express_core::render::render_marked;
        use openstage_express_export::picture::decode_png;
        use openstage_express_export::share::read_copy;
        let d = demo();
        let (guide, pictures) = (d.document.guide.clone(), d.pictures);
        let dir = tempfile::tempdir().unwrap();
        let job = CopyJob {
            guide,
            pictures: Arc::new(pictures),
            options: CopyOptions::default(),
            folder: dir.path().to_path_buf(),
        };
        let report = spawn_copy(job).recv_timeout(std::time::Duration::from_secs(60)).unwrap().unwrap();
        let copy = read_copy(&std::fs::read(&report.file).unwrap()).unwrap();
        // The step with a text mark ("Start here").
        let again = demo();
        let step = again.document.guide.steps.iter().find(|s| s.title == "Click \"New assignment\"").unwrap();
        let original = again.pictures.picture(&step.image.as_ref().unwrap().file).unwrap();
        let in_copy = copy.guide.steps.iter().find(|s| s.title == step.title).unwrap();
        let (_, bytes) = copy.pictures.iter().find(|(n, _)| *n == in_copy.image.as_ref().unwrap().file).unwrap();
        let got = decode_png(bytes).unwrap();
        let windows = render_marked(&original, step, &openstage_express_windows::WindowsText).canvas;
        let blocks = render_marked(&original, step, &BlockText).canvas;
        assert_eq!(got.bgra, windows.bgra, "the words are in Windows' text");
        assert_ne!(got.bgra, blocks.bgra, "and are not blocks");
    }

    /// An export made by the program (on its thread) draws the words on marks with Windows' text, like a copy does.
    #[cfg(windows)]
    #[test]
    fn an_export_made_by_the_program_has_the_words_on_its_marks_in_windows_text_not_in_blocks() {
        use openstage_express_windows::WindowsText;
        let d = demo();
        let guide = d.document.guide.clone();
        let dir = tempfile::tempdir().unwrap();
        let job = Job {
            guide: guide.clone(),
            pictures: Arc::new(d.pictures),
            selection: Selection::none_then(&[Format::Images]),
            choices: PictureChoices::default(),
            folder: dir.path().to_path_buf(),
        };
        spawn(job).recv_timeout(std::time::Duration::from_secs(60)).unwrap().unwrap();
        let pictures = demo().pictures;
        let expected =
            |text: &dyn TextRenderer| export_images(&make_plan(&guide, &pictures, text, &ExportOptions::default()));
        let (windows, blocks) = (expected(&WindowsText), expected(&BlockText));
        assert_ne!(windows.files, blocks.files, "the two renderers make different pictures");
        for file in &windows.files {
            let written = std::fs::read(dir.path().join("images").join(&file.path)).unwrap();
            assert_eq!(written, file.bytes, "{} is made with Windows' text", file.path);
        }
    }

    #[test]
    fn only_the_chosen_formats_are_made_and_each_has_its_own_folder() {
        let dir = tempfile::tempdir().unwrap();
        let mut selection = Selection::none();
        selection.set(Format::Markdown, true);
        selection.set(Format::Word, true);
        let report =
            make(&the_plan(), &selection, &GifOptions::default(), DEFAULT_JPEG_QUALITY, &BlockText, None, dir.path())
                .unwrap();
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
        let report =
            make(&the_plan(), &selection, &GifOptions::default(), DEFAULT_JPEG_QUALITY, &BlockText, None, dir.path())
                .unwrap();
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
        let report = make(
            &the_plan(),
            &Selection::all(),
            &GifOptions::default(),
            DEFAULT_JPEG_QUALITY,
            &BlockText,
            Some(&StandIn),
            dir.path(),
        )
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
        make(&the_plan(), &selection, &GifOptions::default(), DEFAULT_JPEG_QUALITY, &BlockText, None, dir.path())
            .unwrap();
        let again =
            make(&the_plan(), &selection, &GifOptions::default(), DEFAULT_JPEG_QUALITY, &BlockText, None, dir.path());
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

    fn file_names(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(folder)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn the_picture_choices_reach_the_files_and_the_title_and_the_report_says_it_is_a_preview() {
        use openstage_express_export::picture::decode_png;
        use openstage_express_export::plan::exported_step_count;
        let d = demo();
        let total = exported_step_count(&d.document.guide);
        assert!(total > 3);
        let choices = PictureChoices { width: 640, jpg_quality: 50, first_steps: Some(3), ..Default::default() };
        let the_plan = plan(&d.document.guide, &d.pictures, &BlockText, &choices.export_options());
        let dir = tempfile::tempdir().unwrap();
        let selection = Selection::none_then(&[Format::Images, Format::Jpg, Format::Json]);
        let report =
            make(&the_plan, &selection, &GifOptions::default(), choices.jpg_quality, &BlockText, None, dir.path())
                .unwrap();
        assert_eq!(file_names(&dir.path().join("images")), ["step-01.png", "step-02.png", "step-03.png"]);
        for name in file_names(&dir.path().join("images")) {
            let picture = decode_png(&std::fs::read(dir.path().join("images").join(name)).unwrap()).unwrap();
            assert_eq!(picture.width, 640);
        }
        let jpgs = file_names(&dir.path().join("jpg"));
        assert_eq!(jpgs, ["step-01.jpg", "step-02.jpg", "step-03.jpg"]);
        assert!(std::fs::read(dir.path().join("jpg").join(&jpgs[0])).unwrap().starts_with(&[0xFF, 0xD8]));
        let json_files = file_names(&dir.path().join("json"));
        let json_file = json_files.iter().find(|name| name.ends_with(".json")).expect("a .json file");
        let json = std::fs::read_to_string(dir.path().join("json").join(json_file)).unwrap();
        assert!(json.contains(&format!("Preview, first 3 of {total} steps - ")), "{json}");
        assert_eq!(report.preview, Some(Preview { shown: 3, total }));
        let said = report.sentence();
        assert!(
            said.contains(&format!("It is a preview, not the whole guide: the first 3 of the guide's {total} steps.")),
            "{said}"
        );
    }

    #[test]
    fn a_higher_jpg_quality_makes_bigger_files() {
        let size_at = |quality: u8| -> u64 {
            let dir = tempfile::tempdir().unwrap();
            let selection = Selection::none_then(&[Format::Jpg]);
            make(&the_plan(), &selection, &GifOptions::default(), quality, &BlockText, None, dir.path()).unwrap();
            std::fs::read_dir(dir.path().join("jpg")).unwrap().flatten().map(|e| e.metadata().unwrap().len()).sum()
        };
        let (best, usual, smallest) = (size_at(100), size_at(DEFAULT_JPEG_QUALITY), size_at(50));
        assert!(best > usual && usual > smallest, "{best} {usual} {smallest}");
    }

    #[test]
    fn a_job_is_made_with_the_choices_it_was_given() {
        use openstage_express_export::picture::decode_png;
        let d = demo();
        let dir = tempfile::tempdir().unwrap();
        let job = Job {
            guide: d.document.guide.clone(),
            pictures: Arc::new(d.pictures),
            selection: Selection::none_then(&[Format::Images]),
            choices: PictureChoices { width: 800, jpg_quality: 90, first_steps: Some(2), ..Default::default() },
            folder: dir.path().to_path_buf(),
        };
        let report = spawn(job).recv_timeout(std::time::Duration::from_secs(60)).unwrap().unwrap();
        assert_eq!(report.made, [(Format::Images, 2)]);
        let names = file_names(&dir.path().join("images"));
        assert_eq!(names, ["step-01.png", "step-02.png"]);
        for name in names {
            let picture = decode_png(&std::fs::read(dir.path().join("images").join(name)).unwrap()).unwrap();
            assert_eq!(picture.width, 800);
        }
    }

    /// The bytes of the one file with this ending in the folder of a format in a run.
    fn the_file(run: &Path, format: Format, ending: &str) -> Vec<u8> {
        let folder = run.join(format.folder());
        let name = file_names(&folder)
            .into_iter()
            .find(|n| n.ends_with(ending))
            .unwrap_or_else(|| panic!("no file ending {ending} in {folder:?}: {:?}", file_names(&folder)));
        std::fs::read(folder.join(name)).unwrap()
    }

    fn holds(bytes: &[u8], words: &str) -> bool {
        bytes.windows(words.len()).any(|w| w == words.as_bytes())
    }

    fn run_with(choices: PictureChoices, formats: &[Format]) -> (tempfile::TempDir, Report) {
        let d = demo();
        let dir = tempfile::tempdir().unwrap();
        let job = Job {
            guide: d.document.guide.clone(),
            pictures: Arc::new(d.pictures),
            selection: Selection::none_then(formats),
            choices,
            folder: dir.path().to_path_buf(),
        };
        let report = spawn(job).recv_timeout(std::time::Duration::from_secs(120)).unwrap().unwrap();
        (dir, report)
    }

    #[test]
    fn jpg_pictures_inside_documents_reach_the_web_page_word_and_powerpoint_and_not_the_other_formats() {
        let formats = [Format::Html, Format::Word, Format::PowerPoint, Format::Images, Format::Markdown];
        let (png_run, png_report) = run_with(PictureChoices::default(), &formats);
        let jpg_choices = PictureChoices { documents_jpg: true, jpg_quality: 70, ..Default::default() };
        let (jpg_run, jpg_report) = run_with(jpg_choices, &formats);
        assert!(png_report.warnings.is_empty() && jpg_report.warnings.is_empty(), "{:?}", jpg_report.warnings);
        // The usual run keeps PNG inside, as it always has.
        let page = the_file(png_run.path(), Format::Html, ".html");
        assert!(holds(&page, "data:image/png;base64,") && !holds(&page, "data:image/jpeg"));
        assert!(holds(&the_file(png_run.path(), Format::Word, ".docx"), "word/media/image1.png"));
        assert!(holds(&the_file(png_run.path(), Format::PowerPoint, ".pptx"), "ppt/media/image1.png"));
        // With JPG chosen, all three hold JPG pictures.
        let page = the_file(jpg_run.path(), Format::Html, ".html");
        assert!(holds(&page, "data:image/jpeg;base64,") && !holds(&page, "data:image/png"));
        let word = the_file(jpg_run.path(), Format::Word, ".docx");
        assert!(holds(&word, "word/media/image1.jpg") && !holds(&word, "word/media/image1.png"));
        let slides = the_file(jpg_run.path(), Format::PowerPoint, ".pptx");
        assert!(holds(&slides, "ppt/media/image1.jpg") && !holds(&slides, "ppt/media/image1.png"));
        // The folder of pictures and the Markdown folder are the same either way.
        assert_eq!(file_names(&jpg_run.path().join("images")), file_names(&png_run.path().join("images")));
        for name in file_names(&png_run.path().join("images")) {
            assert_eq!(
                std::fs::read(png_run.path().join("images").join(&name)).unwrap(),
                std::fs::read(jpg_run.path().join("images").join(&name)).unwrap(),
                "{name}"
            );
        }
        assert_eq!(
            the_file(png_run.path(), Format::Markdown, ".md"),
            the_file(jpg_run.path(), Format::Markdown, ".md")
        );
    }

    #[test]
    fn the_jpg_quality_chosen_decides_the_size_of_the_pictures_inside_documents() {
        let page_size = |quality: u8| -> usize {
            let choices = PictureChoices { documents_jpg: true, jpg_quality: quality, ..Default::default() };
            let (run, _) = run_with(choices, &[Format::Html]);
            the_file(run.path(), Format::Html, ".html").len()
        };
        let (best, usual, smallest) = (page_size(100), page_size(DEFAULT_JPEG_QUALITY), page_size(50));
        assert!(best > usual && usual > smallest, "{best} {usual} {smallest}");
    }

    #[test]
    fn a_preview_with_jpg_pictures_inside_documents_holds_only_its_own_steps() {
        let choices = PictureChoices { documents_jpg: true, first_steps: Some(2), ..Default::default() };
        let (run, report) = run_with(choices, &[Format::Html, Format::PowerPoint]);
        assert!(report.preview.is_some_and(|p| p.is_cut()));
        let page = the_file(run.path(), Format::Html, ".html");
        let text = String::from_utf8_lossy(&page);
        assert_eq!(text.matches("<img src=\"data:image/jpeg;base64,").count(), 2);
        let slides = the_file(run.path(), Format::PowerPoint, ".pptx");
        assert!(holds(&slides, "ppt/media/image2.jpg") && !holds(&slides, "ppt/media/image3.jpg"));
    }

    #[test]
    fn the_usual_picture_choices_are_the_usual_width_and_quality_and_every_step() {
        let usual = PictureChoices::default();
        assert_eq!((usual.width, usual.jpg_quality, usual.first_steps), (1280, 90, None));
        assert_eq!(usual.export_options(), ExportOptions::default());
        assert!(!usual.documents_jpg, "documents keep PNG pictures unless JPG is asked for");
        let asked = PictureChoices { width: 800, jpg_quality: 40, first_steps: Some(7), ..Default::default() };
        assert_eq!(
            asked.export_options(),
            ExportOptions { image_width: 800, first_steps: Some(7), ..ExportOptions::default() },
            "the JPG quality alone does not turn JPG on inside documents"
        );
        let jpg = PictureChoices { jpg_quality: 40, documents_jpg: true, ..Default::default() };
        assert_eq!(
            jpg.export_options(),
            ExportOptions { document_pictures: ImageFormat::Jpeg { quality: 40 }, ..ExportOptions::default() },
            "one quality for the JPG files and the JPG pictures inside documents"
        );
    }

    fn report_with(preview: Option<Preview>) -> Report {
        Report {
            folder: PathBuf::from("X"),
            made: vec![(Format::Json, 1)],
            warnings: Vec::new(),
            skipped: Vec::new(),
            preview,
        }
    }

    #[test]
    fn the_report_says_when_it_is_a_preview_and_when_the_preview_was_the_whole_guide() {
        assert_eq!(report_with(None).sentence(), "Exported 1 format to X.");
        assert_eq!(
            report_with(Some(Preview { shown: 5, total: 12 })).sentence(),
            "Exported 1 format to X. It is a preview, not the whole guide: the first 5 of the guide's 12 steps."
        );
        assert_eq!(
            report_with(Some(Preview { shown: 1, total: 12 })).sentence(),
            "Exported 1 format to X. It is a preview, not the whole guide: the first step of the guide's 12 steps."
        );
        assert_eq!(
            report_with(Some(Preview { shown: 4, total: 4 })).sentence(),
            "Exported 1 format to X. You asked for a preview, but the guide has only 4 steps, so all of it is in."
        );
        assert_eq!(
            report_with(Some(Preview { shown: 1, total: 1 })).sentence(),
            "Exported 1 format to X. You asked for a preview, but the guide has only 1 step, so all of it is in."
        );
    }

    #[test]
    fn the_folder_of_a_preview_says_so_and_has_no_characters_windows_forbids() {
        let name = preview_run_folder_name("2026-11-01T10:30:05Z", 5);
        assert_eq!(name, "2026-11-01 10-30-05 (preview of 5 steps)");
        assert_eq!(preview_run_folder_name("2026-11-01T10:30:05Z", 1), "2026-11-01 10-30-05 (preview of 1 step)");
        assert_eq!(preview_run_folder_name("2026-11-01T10:30:05Z", 0), "2026-11-01 10-30-05 (preview of 1 step)");
        assert!(!name.contains([':', '/', '\\', '*', '?', '"', '<', '>', '|']));
        assert!(name.starts_with(&run_folder_name("2026-11-01T10:30:05Z")), "it sorts with the other runs");
    }
}
