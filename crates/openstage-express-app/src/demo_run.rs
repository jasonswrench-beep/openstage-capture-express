// SPDX-License-Identifier: Apache-2.0
//! The export demo: every export of the made-up guide, each in its own folder, so a person can open them and see what
//! Express makes without capturing anything. Used by `EXPORT DEMO.bat`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use openstage_capture_render::BlockText;
use openstage_express_core::fsutil::utc_now_iso;
use openstage_express_export::demo::demo;
use openstage_express_export::gif::{export_gif, GifOptions};
use openstage_express_export::html::export_html;
use openstage_express_export::images::export_images;
use openstage_express_export::json::export_json;
use openstage_express_export::markdown::export_markdown;
use openstage_express_export::plan::{plan, ExportOptions};
use openstage_express_export::Export;

/// A folder name for this run, from the time: `2026-11-01 10-30-05`.
fn run_folder_name(now_iso: &str) -> String {
    now_iso.trim_end_matches('Z').replace('T', " ").replace(':', "-")
}

/// Make every export in a new folder inside `out`. Returns what to tell the person.
pub fn run(out: &Path) -> Result<String, String> {
    run_at(out, &utc_now_iso())
}

fn run_at(out: &Path, now_iso: &str) -> Result<String, String> {
    let guide = demo();
    let the_plan = plan(&guide.document.guide, &guide.pictures, &BlockText, &ExportOptions::default());
    let folder: PathBuf = out.join(run_folder_name(now_iso));
    let exports: Vec<(&str, Export)> = vec![
        ("html", export_html(&the_plan)),
        ("markdown", export_markdown(&the_plan)),
        ("json", export_json(&the_plan)),
        ("images", export_images(&the_plan)),
        (
            "gif",
            export_gif(
                &the_plan,
                &GifOptions { progress_bar: true, title_card: true, ..GifOptions::default() },
                &BlockText,
            ),
        ),
    ];
    let mut report = String::new();
    writeln!(
        report,
        "Made a made-up guide called \"{}\" with {} steps, and exported it:",
        the_plan.title,
        the_plan.steps.len()
    )
    .ok();
    for (name, export) in &exports {
        let written = export.write_to(&folder.join(name)).map_err(|e| e.to_string())?;
        writeln!(report, "  {name}: {} file(s)", written.len()).ok();
        for warning in &export.warnings {
            writeln!(report, "    warning: {warning}").ok();
        }
    }
    writeln!(report, "\nEverything is in:\n  {}", folder.display()).ok();
    writeln!(
        report,
        "\nThe words on the made-up screenshots are drawn as plain blocks. Real text drawing comes with capture."
    )
    .ok();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_makes_every_export_in_a_new_folder_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let report = run_at(dir.path(), "2026-11-01T10:30:05Z").unwrap();
        let folder = dir.path().join("2026-11-01 10-30-05");
        for expected in [
            "html/How to submit an assignment.html",
            "markdown/How to submit an assignment.md",
            "markdown/images/step-01.png",
            "json/How to submit an assignment.json",
            "images/step-06.png",
            "gif/How to submit an assignment.gif",
        ] {
            assert!(folder.join(expected).is_file(), "{expected}");
        }
        assert!(
            report.contains("html: 1 file(s)")
                && report.contains("gif: 1 file(s)")
                && report.contains("2026-11-01 10-30-05")
        );
        let again = run_at(dir.path(), "2026-11-01T10:30:05Z");
        assert!(again.unwrap_err().contains("without replacing it"), "a second run at the same moment stops");
        assert!(run_at(dir.path(), "2026-11-01T10:30:06Z").is_ok());
    }

    #[test]
    fn the_folder_name_has_no_characters_windows_forbids() {
        let name = run_folder_name("2026-11-01T10:30:05Z");
        assert_eq!(name, "2026-11-01 10-30-05");
        assert!(!name.contains([':', '/', '\\']));
    }
}
