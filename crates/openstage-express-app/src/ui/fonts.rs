// SPDX-License-Identifier: Apache-2.0
//! The window's fonts (spec section 6.3). The library's own bundled fonts are switched off, so the program names its own:
//!
//! * Segoe UI, read from the computer's own fonts folder when the program starts. It is never copied or shipped.
//! * Tahoma, then Arial, if there is no Segoe UI.
//! * Roboto (Apache-2.0, bundled in the export crate) as the last choice and as the fallback for letters the first font lacks,
//!   so the window still draws on a computer with none of the others.
//!
//! None of these has Chinese, Japanese, Korean or Arabic letters yet; the interface is English first (spec A4).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{FontData, FontDefinitions, FontFamily};
use openstage_express_export::pdf::{ROBOTO_BOLD, ROBOTO_REGULAR};

/// The family for bold text: `RichText::new(..).family(bold_family())`.
pub fn bold_family() -> FontFamily {
    FontFamily::Name("bold".into())
}

/// Font files to look for, best first, in a Windows fonts folder.
const REGULAR_FILES: [&str; 3] = ["segoeui.ttf", "tahoma.ttf", "arial.ttf"];
const BOLD_FILES: [&str; 3] = ["segoeuib.ttf", "tahomabd.ttf", "arialbd.ttf"];

/// Which fonts were found, for the About box and for the tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontChoice {
    /// The regular font's file name, or `None` when the bundled Roboto is the main font.
    pub regular: Option<String>,
    pub bold: Option<String>,
}

/// The computer's own fonts folder: `%WINDIR%\Fonts`, or `C:\Windows\Fonts` if that is not set.
pub fn system_fonts_folder() -> PathBuf {
    let windows = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    windows.join("Fonts")
}

fn first_readable(folder: &Path, names: &[&str]) -> Option<(String, Vec<u8>)> {
    names.iter().find_map(|name| {
        let bytes = std::fs::read(folder.join(name)).ok()?;
        // A TrueType or OpenType file starts with one of these; anything else is not a font this can use.
        let looks_like_a_font = matches!(bytes.get(..4), Some([0, 1, 0, 0] | b"OTTO" | b"true"));
        looks_like_a_font.then(|| (name.to_string(), bytes))
    })
}

/// The font set for the window, looking in `folder` for the computer's fonts.
pub fn definitions_from(folder: &Path) -> (FontDefinitions, FontChoice) {
    let regular = first_readable(folder, &REGULAR_FILES);
    let bold = first_readable(folder, &BOLD_FILES);
    let choice =
        FontChoice { regular: regular.as_ref().map(|(n, _)| n.clone()), bold: bold.as_ref().map(|(n, _)| n.clone()) };

    let mut fonts = FontDefinitions::empty();
    fonts.font_data.insert("roboto".into(), Arc::new(FontData::from_static(ROBOTO_REGULAR)));
    fonts.font_data.insert("roboto-bold".into(), Arc::new(FontData::from_static(ROBOTO_BOLD)));
    let mut proportional = Vec::new();
    let mut bold_list = Vec::new();
    if let Some((_, bytes)) = regular {
        fonts.font_data.insert("regular".into(), Arc::new(FontData::from_owned(bytes)));
        proportional.push("regular".to_string());
    }
    proportional.push("roboto".to_string());
    if let Some((_, bytes)) = bold {
        fonts.font_data.insert("bold".into(), Arc::new(FontData::from_owned(bytes)));
        bold_list.push("bold".to_string());
    }
    bold_list.push("roboto-bold".to_string());
    fonts.families.insert(FontFamily::Proportional, proportional.clone());
    fonts.families.insert(FontFamily::Monospace, proportional);
    fonts.families.insert(bold_family(), bold_list);
    (fonts, choice)
}

/// The font set for the window, from the computer's fonts folder.
pub fn definitions() -> (FontDefinitions, FontChoice) {
    definitions_from(&system_fonts_folder())
}
