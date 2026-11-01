// SPDX-License-Identifier: Apache-2.0
//! The pictures export (spec X4): every exported step as a flattened PNG in a folder, named in step order
//! (`step-01.png`). The marks are burned in and the focused view is applied, exactly as in the other exports.
//! JPG with a quality setting is not built yet.

use crate::plan::Plan;
use crate::{Export, OutFile};

/// Every step's picture, and nothing else.
pub fn export_images(plan: &Plan) -> Export {
    let files = plan
        .steps
        .iter()
        .filter_map(|s| s.picture.as_ref())
        .map(|p| OutFile { path: p.file.rsplit('/').next().unwrap_or(&p.file).to_string(), bytes: p.png.clone() })
        .collect();
    Export { files, warnings: plan.warnings.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo;
    use crate::picture::decode_png;
    use crate::plan::{plan, ExportOptions};
    use openstage_capture_render::BlockText;

    #[test]
    fn every_picture_step_gives_one_png_in_order() {
        let d = demo();
        let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let export = export_images(&p);
        let names: Vec<&str> = export.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(names, ["step-01.png", "step-02.png", "step-03.png", "step-04.png", "step-06.png"]);
        for file in &export.files {
            assert_eq!(decode_png(&file.bytes).unwrap().width, 1280);
        }
    }

    #[test]
    fn the_blur_is_burned_into_the_exported_picture_as_flat_blocks_the_original_is_not() {
        let d = demo();
        // The Submit step is cropped to (0, 240) 853 by 480 round the click, and exporting at 853 across leaves it unscaled,
        // so the picture's pixels line up with the original's. The blurred name box is (280, 380) 500 by 40 in the original,
        // which is (280, 140) in the crop. Its blocks are a third of the shorter side, 13 pixels.
        let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions { image_width: 853 });
        let export = export_images(&p);
        let exported = decode_png(&export.files.last().unwrap().bytes).unwrap();
        assert_eq!((exported.width, exported.height), (853, 480));
        let original = &d.pictures.0["images/step-0005.png"];
        let pixel = |picture: &openstage_capture_render::Picture, x: u32, y: u32| {
            let i = ((y * picture.width + x) * 4) as usize;
            [picture.bgra[i], picture.bgra[i + 1], picture.bgra[i + 2]]
        };
        let (mut flat_exported, mut flat_original) = (true, true);
        let first_exported = pixel(&exported, 280, 140);
        let first_original = pixel(original, 280, 380);
        for dy in 0..13 {
            for dx in 0..13 {
                flat_exported &= pixel(&exported, 280 + dx, 140 + dy) == first_exported;
                flat_original &= pixel(original, 280 + dx, 380 + dy) == first_original;
            }
        }
        assert!(flat_exported, "the first block of the blurred box is one flat colour in the export");
        assert!(!flat_original, "but it holds letters in the original, which the export never changes");
    }

    #[test]
    fn a_missing_picture_is_left_out_with_a_warning() {
        let d = demo();
        let mut pictures = d.pictures;
        pictures.0.remove("images/step-0002.png");
        let p = plan(&d.document.guide, &pictures, &BlockText, &ExportOptions::default());
        let export = export_images(&p);
        assert_eq!(export.files.len(), 4);
        assert_eq!(export.warnings.len(), 1);
        assert!(export.warnings[0].contains("images/step-0002.png"), "{:?}", export.warnings);
    }
}
