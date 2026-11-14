// SPDX-License-Identifier: Apache-2.0
//! The Windows-only parts of OpenStage Capture Express.
//!
//! Built so far: [`pdf`], the PDF engine, which drives the Microsoft WebView2 runtime that Windows ships, and [`console`],
//! which lets the window close the console box a double-clicked program starts with.
//!
//! Planned contents (spec sections 6.2, 6.4 and 8, milestone M6): mouse and keyboard hooks, UI Automation lookups,
//! the frame ring, hotkeys and the capture bar's window handling, reusing OpenStage Capture's crates (from the
//! openstage-author repository at one fixed commit, named in the workspace's Cargo.toml). On other systems this crate stays empty so that
//! `cargo test --workspace` still runs everywhere.

#[cfg(windows)]
pub mod affinity;
#[cfg(windows)]
pub mod console;
#[cfg(windows)]
pub mod dialogs;
#[cfg(windows)]
pub mod hotkeys;
#[cfg(windows)]
pub mod input;
#[cfg(windows)]
pub mod pdf;
#[cfg(windows)]
pub mod run;
#[cfg(windows)]
pub mod screen;
#[cfg(windows)]
pub mod sound;
#[cfg(windows)]
pub mod synthetic;
#[cfg(windows)]
pub mod system;
#[cfg(windows)]
pub mod testwin;
#[cfg(windows)]
pub mod uia;
#[cfg(windows)]
pub mod updates;

/// Windows' own text drawing (from Capture's code): the words on text marks and the numbers on step badges, in the computer's fonts.
#[cfg(windows)]
pub use openstage_capture_windows::WindowsText;

/// Windows' own text drawing, tried by itself (the program's tests use the plain renderer so that they give the same pictures on every
/// computer; this checks that on a Windows computer the real one makes real letters).
#[cfg(all(test, windows))]
mod text_tests {
    use super::WindowsText;
    use openstage_capture_project::EditAlign;
    use openstage_capture_render::{BlockText, Canvas, PixelRect, Rgb, TextRenderer, TextRequest};
    use std::collections::BTreeSet;

    const AREA: PixelRect = PixelRect { x: 10, y: 10, w: 400, h: 70 };

    fn white() -> Canvas {
        let mut canvas = Canvas::new(420, 90);
        canvas.fill_rect(PixelRect::new(0, 0, 420, 90), Rgb(255, 255, 255));
        canvas
    }

    fn request(words: &str, area: PixelRect, bold: bool, background: Option<Rgb>) -> TextRequest {
        TextRequest {
            text: words.to_string(),
            area,
            font_px: 30.0,
            bold,
            color: Rgb(0, 0, 0),
            background,
            align: EditAlign::Left,
        }
    }

    /// How many different shades of the blue channel a picture has.
    fn shades(canvas: &Canvas) -> usize {
        canvas.bgra.as_chunks::<4>().0.iter().map(|p| p[0]).collect::<BTreeSet<u8>>().len()
    }

    /// How many pixels are not white.
    fn ink(canvas: &Canvas) -> usize {
        canvas.bgra.as_chunks::<4>().0.iter().filter(|p| p[..3] != [255, 255, 255]).count()
    }

    #[test]
    fn windows_draws_smoothed_letters_where_the_plain_renderer_draws_blocks() {
        let (mut real, mut blocks) = (white(), white());
        let fit = WindowsText.draw(&mut real, &request("Hamburgefonts", AREA, false, None)).unwrap();
        BlockText.draw(&mut blocks, &request("Hamburgefonts", AREA, false, None)).unwrap();
        assert!(fit.complete);
        assert_eq!(shades(&blocks), 2, "blocks are only ink and paper");
        assert!(shades(&real) > 8, "letters have smoothed edges: {} shades", shades(&real));
        assert_ne!(real.bgra, blocks.bgra);
        assert!(ink(&real) > 200, "something was drawn: {}", ink(&real));
        // Nothing is drawn outside the box.
        for y in 0..90 {
            for x in 0..420 {
                let inside = (AREA.x..AREA.x + AREA.w).contains(&x) && (AREA.y..AREA.y + AREA.h).contains(&y);
                let at = (y as usize * 420 + x as usize) * 4;
                if !inside {
                    assert_eq!(real.bgra[at..at + 3], [255, 255, 255], "ink outside the box at {x},{y}");
                }
            }
        }
    }

    #[test]
    fn bold_letters_take_more_ink_a_background_fills_the_box_and_empty_words_draw_nothing() {
        let (mut plain, mut bold) = (white(), white());
        WindowsText.draw(&mut plain, &request("Save the guide", AREA, false, None)).unwrap();
        WindowsText.draw(&mut bold, &request("Save the guide", AREA, true, None)).unwrap();
        assert!(ink(&bold) > ink(&plain), "bold {} against plain {}", ink(&bold), ink(&plain));
        let mut filled = white();
        WindowsText.draw(&mut filled, &request("Hi", AREA, false, Some(Rgb(200, 0, 0)))).unwrap();
        let corner = ((AREA.y + AREA.h - 2) as usize * 420 + (AREA.x + AREA.w - 2) as usize) * 4;
        assert_eq!(
            filled.bgra[corner..corner + 3],
            [0, 0, 200],
            "the box has its background colour (blue, green, red order)"
        );
        let mut nothing = white();
        let fit = WindowsText.draw(&mut nothing, &request("   ", AREA, false, None)).unwrap();
        assert!(fit.complete && ink(&nothing) == 0);
    }

    #[test]
    fn words_that_do_not_fit_say_so_and_other_alphabets_are_drawn() {
        let mut small = white();
        let tiny = PixelRect::new(10, 10, 90, 36);
        let fit = WindowsText
            .draw(&mut small, &request("This is far too much to say in such a small box", tiny, false, None))
            .unwrap();
        assert!(!fit.complete, "cut off, and said so");
        let mut other = white();
        let fit = WindowsText
            .draw(
                &mut other,
                &request("Caf\u{e9} \u{65e5}\u{672c}\u{8a9e} \u{442}\u{435}\u{43a}\u{441}\u{442}", AREA, false, None),
            )
            .unwrap();
        assert!(ink(&other) > 300, "accents and other alphabets are drawn with a Windows font: {}", ink(&other));
        let _ = fit;
    }
}
