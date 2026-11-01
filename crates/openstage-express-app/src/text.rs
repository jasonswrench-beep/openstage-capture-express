// SPDX-License-Identifier: Apache-2.0
//! The drawing of words on marks: a text mark's words and the number in a step badge, in the editor's picture and in every export.
//!
//! On Windows these are drawn with Windows' own text drawing, so the letters are real letters in the computer's fonts. On any other
//! computer the plain renderer is used, which draws each letter as a block. (Tests of the export crate and of the drawing itself pass
//! the plain renderer on purpose, so that their pictures are the same everywhere; the program itself never does.)

use openstage_capture_render::TextRenderer;

/// The renderer the program draws marks with.
#[cfg(windows)]
pub fn renderer() -> &'static dyn TextRenderer {
    &openstage_express_windows::WindowsText
}

/// The renderer the program draws marks with.
#[cfg(not(windows))]
pub fn renderer() -> &'static dyn TextRenderer {
    &openstage_capture_render::BlockText
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use openstage_capture_project::EditAlign;
    use openstage_capture_render::{BlockText, Canvas, PixelRect, Rgb, TextRequest};

    fn drawn(renderer: &dyn TextRenderer) -> Vec<u8> {
        let mut canvas = Canvas::new(300, 60);
        canvas.fill_rect(PixelRect::new(0, 0, 300, 60), Rgb(255, 255, 255));
        let request = TextRequest {
            text: "Start here".to_string(),
            area: PixelRect::new(5, 5, 290, 50),
            font_px: 26.0,
            bold: false,
            color: Rgb(0, 0, 0),
            background: None,
            align: EditAlign::Left,
        };
        renderer.draw(&mut canvas, &request).unwrap();
        canvas.bgra
    }

    #[test]
    fn the_program_draws_the_words_on_marks_with_windows_text_and_not_with_blocks() {
        let program = drawn(renderer());
        assert_eq!(program, drawn(&openstage_express_windows::WindowsText), "it is Windows' text drawing");
        assert_ne!(program, drawn(&BlockText), "it is not the block letters");
    }
}
