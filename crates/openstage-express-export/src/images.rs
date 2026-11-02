// SPDX-License-Identifier: Apache-2.0
//! The pictures export (spec X4 and section 10.4): every exported step as a flattened picture in a folder, named in step order
//! (`step-01.png` or `step-01.jpg`). The marks are burned in and the focused view is applied, exactly as in the other exports.
//! Pictures are encoded from the same lossless rendering, so the marks stay crisp. JPG takes a quality from 1 to 100.

use crate::jpeg;
use crate::picture::decode_png;
use crate::plan::Plan;
use crate::{Export, OutFile};

/// The qualities the export dialog offers, as spec 10.4 lists them; any number from 1 to 100 also works.
pub const JPEG_QUALITY_CHOICES: [u8; 4] = [100, 90, 70, 50];
pub const DEFAULT_JPEG_QUALITY: u8 = 90;

/// What kind of picture file to make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    /// JPG with a quality from 1 (smallest file) to 100 (best picture); a number outside that is moved to the nearest end.
    Jpeg {
        quality: u8,
    },
}

/// Every step's picture as a PNG, and nothing else.
pub fn export_images(plan: &Plan) -> Export {
    export_images_as(plan, ImageFormat::Png)
}

/// Every step's picture in the chosen format, and nothing else. A picture that cannot be turned into a JPG is left out with a
/// warning, and the others are still made.
pub fn export_images_as(plan: &Plan, format: ImageFormat) -> Export {
    let mut files = Vec::new();
    let mut warnings = plan.warnings.clone();
    for step in &plan.steps {
        let Some(picture) = &step.picture else { continue };
        let png_name = picture.file.rsplit('/').next().unwrap_or(&picture.file);
        match format {
            ImageFormat::Png => files.push(OutFile { path: png_name.to_string(), bytes: picture.png.clone() }),
            ImageFormat::Jpeg { quality } => {
                let name = format!("{}.jpg", png_name.strip_suffix(".png").unwrap_or(png_name));
                let encoded = decode_png(&picture.png).and_then(|p| {
                    jpeg::encode_bgra(&p.bgra, p.width, p.height, quality.clamp(1, 100)).map_err(|e| e.to_string())
                });
                match encoded {
                    Ok(bytes) => files.push(OutFile { path: name, bytes }),
                    Err(why) => warnings
                        .push(format!("The picture for \"{}\" was left out of the JPG export: {why}", step.title)),
                }
            }
        }
    }
    Export { files, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo;
    use crate::jpeg::decoder::{decode, psnr};
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
    fn jpg_gives_the_same_pictures_in_the_same_order_as_jpg_files_that_are_close_to_the_png() {
        let d = demo();
        let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let pngs = export_images(&p);
        let jpgs = export_images_as(&p, ImageFormat::Jpeg { quality: 90 });
        let names: Vec<&str> = jpgs.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(names, ["step-01.jpg", "step-02.jpg", "step-03.jpg", "step-04.jpg", "step-06.jpg"]);
        assert!(jpgs.warnings.is_empty(), "{:?}", jpgs.warnings);
        for (png, jpg) in pngs.files.iter().zip(&jpgs.files) {
            assert_eq!(&jpg.bytes[..2], &[0xFF, 0xD8], "{}", jpg.path);
            let original = decode_png(&png.bytes).unwrap();
            let back = decode(&jpg.bytes);
            assert_eq!((back.width as u32, back.height as u32), (original.width, original.height));
            let rgb: Vec<u8> = original.bgra.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0]]).collect();
            let db = psnr(&rgb, &back.rgb);
            assert!(db > 30.0, "{}: {db:.1} dB", jpg.path);
        }
    }

    #[test]
    fn a_lower_quality_makes_a_smaller_file_and_the_choices_are_the_specs() {
        let d = demo();
        let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let total = |quality: u8| -> usize {
            export_images_as(&p, ImageFormat::Jpeg { quality }).files.iter().map(|f| f.bytes.len()).sum()
        };
        let sizes: Vec<usize> = JPEG_QUALITY_CHOICES.iter().map(|&q| total(q)).collect();
        assert!(sizes.windows(2).all(|w| w[0] > w[1]), "100, 90, 70 and 50 get smaller each time: {sizes:?}");
        assert_eq!(JPEG_QUALITY_CHOICES, [100, 90, 70, 50]);
        assert_eq!(DEFAULT_JPEG_QUALITY, 90);
        assert!(
            total(90) < export_images(&p).files.iter().map(|f| f.bytes.len()).sum::<usize>() * 3,
            "not absurdly larger than the PNGs"
        );
    }

    #[test]
    fn a_quality_outside_one_to_a_hundred_is_moved_to_the_nearest_end() {
        let d = demo();
        let p = plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default());
        let at = |quality: u8| export_images_as(&p, ImageFormat::Jpeg { quality });
        assert_eq!(at(0), at(1));
        assert_eq!(at(150), at(100));
        assert_ne!(at(1), at(100));
    }

    #[test]
    fn the_blur_is_burned_into_the_exported_picture_as_flat_blocks_the_original_is_not() {
        let d = demo();
        // The Submit step is cropped to (0, 240) 853 by 480 round the click, and exporting at 853 across leaves it unscaled,
        // so the picture's pixels line up with the original's. The blurred name box is (280, 380) 500 by 40 in the original,
        // which is (280, 140) in the crop. Its blocks are a third of the shorter side, 13 pixels.
        let p = plan(
            &d.document.guide,
            &d.pictures,
            &BlockText,
            &ExportOptions { image_width: 853, ..ExportOptions::default() },
        );
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
    fn the_blur_survives_a_jpg_too() {
        let d = demo();
        let p = plan(
            &d.document.guide,
            &d.pictures,
            &BlockText,
            &ExportOptions { image_width: 853, ..ExportOptions::default() },
        );
        let jpg = decode(&export_images_as(&p, ImageFormat::Jpeg { quality: 90 }).files.last().unwrap().bytes);
        // Inside the first blurred block the colour is flat, and a JPG at this quality keeps it flat to within a few levels.
        let at = |x: usize, y: usize| &jpg.rgb[(y * jpg.width + x) * 3..(y * jpg.width + x) * 3 + 3];
        let first = at(284, 144).to_vec();
        for dy in 4..9 {
            for dx in 4..9 {
                for (value, expected) in at(280 + dx, 140 + dy).iter().zip(&first) {
                    assert!((i32::from(*value) - i32::from(*expected)).abs() <= 6, "({dx}, {dy})");
                }
            }
        }
    }

    #[test]
    fn a_missing_picture_is_left_out_with_a_warning() {
        let d = demo();
        let mut pictures = d.pictures;
        pictures.0.remove("images/step-0002.png");
        let p = plan(&d.document.guide, &pictures, &BlockText, &ExportOptions::default());
        for export in [export_images(&p), export_images_as(&p, ImageFormat::Jpeg { quality: 80 })] {
            assert_eq!(export.files.len(), 4);
            assert_eq!(export.warnings.len(), 1);
            assert!(export.warnings[0].contains("images/step-0002.png"), "{:?}", export.warnings);
        }
    }
}
