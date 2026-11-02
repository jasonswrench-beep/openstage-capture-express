// SPDX-License-Identifier: Apache-2.0
//! The export demo: every export of the made-up guide, each in its own folder, so a person can open them and see what
//! Express makes without capturing anything. Used by `EXPORT DEMO.bat`.

use std::fmt::Write as _;
use std::path::Path;

use openstage_express_core::fsutil::utc_now_iso;
use openstage_express_export::demo::demo;
use openstage_express_export::gif::GifOptions;
use openstage_express_export::images::DEFAULT_JPEG_QUALITY;
use openstage_express_export::pdf::PdfRenderer;
use openstage_express_export::plan::{plan, ExportOptions};

use crate::export_run::{make, run_folder_name, Selection};

/// Make every export in a new folder inside `out`. Returns what to tell the person.
pub fn run(out: &Path) -> Result<String, String> {
    #[cfg(windows)]
    {
        use openstage_express_windows::pdf::WebView2Pdf;
        if WebView2Pdf::runtime_version().is_some() {
            return run_at(out, &utc_now_iso(), Some(&WebView2Pdf));
        }
    }
    run_at(out, &utc_now_iso(), None)
}

/// `pdf` is the engine that makes PDF files, or `None` when this computer has none (the demo then says so and goes on).
fn run_at(out: &Path, now_iso: &str, pdf: Option<&dyn PdfRenderer>) -> Result<String, String> {
    let guide = demo();
    let text = crate::text::renderer();
    let the_plan = plan(&guide.document.guide, &guide.pictures, text, &ExportOptions::default());
    let folder = out.join(run_folder_name(now_iso));
    // The demo turns on the title card and the progress bar, to show them.
    let gif = GifOptions { progress_bar: true, title_card: true, ..GifOptions::default() };
    let made = make(&the_plan, &Selection::all(), &gif, DEFAULT_JPEG_QUALITY, text, pdf, &folder)?;
    let mut report = String::new();
    writeln!(
        report,
        "Made a made-up guide called \"{}\" with {} steps, and exported it:",
        the_plan.title,
        the_plan.steps.len()
    )
    .ok();
    for (format, why) in &made.skipped {
        writeln!(report, "  {}: skipped, because {why}", format.folder()).ok();
    }
    for (format, files) in &made.made {
        writeln!(report, "  {}: {files} file(s)", format.folder()).ok();
    }
    for warning in &made.warnings {
        writeln!(report, "    warning: {warning}").ok();
    }
    writeln!(report, "\nEverything is in:\n  {}", folder.display()).ok();
    writeln!(
        report,
        "\nThe words inside the made-up screenshots are drawn as plain blocks, because the screenshots are made up. The words on \
         marks and the numbers on step badges are drawn with Windows' own fonts."
    )
    .ok();
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;
    use openstage_express_export::pdf::{PrintPage, RenderError};

    /// Stands in for the PDF engine, so this test runs the same everywhere.
    struct StandIn;

    impl PdfRenderer for StandIn {
        fn render(&self, _page: &PrintPage<'_>) -> Result<Vec<u8>, RenderError> {
            Ok(b"%PDF-1.4\n/StructTreeRoot /Marked true /Lang (en) /Outlines\n".to_vec())
        }
    }

    #[test]
    fn a_run_makes_every_export_in_a_new_folder_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let report = run_at(dir.path(), "2026-11-01T10:30:05Z", Some(&StandIn)).unwrap();
        let folder = dir.path().join("2026-11-01 10-30-05");
        for expected in [
            "html/How to submit an assignment.html",
            "markdown/How to submit an assignment.md",
            "markdown/images/step-01.png",
            "json/How to submit an assignment.json",
            "images/step-06.png",
            "jpg/step-06.jpg",
            "gif/How to submit an assignment.gif",
            "pdf/How to submit an assignment.pdf",
            "word/How to submit an assignment.docx",
            "powerpoint/How to submit an assignment.pptx",
        ] {
            assert!(folder.join(expected).is_file(), "{expected}");
        }
        assert!(
            report.contains("html: 1 file(s)")
                && report.contains("gif: 1 file(s)")
                && report.contains("pdf: 1 file(s)")
                && report.contains("2026-11-01 10-30-05")
        );
        let again = run_at(dir.path(), "2026-11-01T10:30:05Z", Some(&StandIn));
        assert!(again.unwrap_err().contains("without replacing it"), "a second run at the same moment stops");
        assert!(run_at(dir.path(), "2026-11-01T10:30:06Z", Some(&StandIn)).is_ok());
    }

    #[test]
    fn without_a_pdf_engine_the_demo_says_so_and_still_makes_everything_else() {
        let dir = tempfile::tempdir().unwrap();
        let report = run_at(dir.path(), "2026-11-01T10:30:05Z", None).unwrap();
        assert!(report.contains("pdf: skipped") && report.contains("WebView2"), "{report}");
        let folder = dir.path().join("2026-11-01 10-30-05");
        assert!(folder.join("html/How to submit an assignment.html").is_file());
        assert!(!folder.join("pdf").exists());
    }

    #[test]
    fn the_folder_name_has_no_characters_windows_forbids() {
        let name = run_folder_name("2026-11-01T10:30:05Z");
        assert_eq!(name, "2026-11-01 10-30-05");
        assert!(!name.contains([':', '/', '\\']));
    }
}
