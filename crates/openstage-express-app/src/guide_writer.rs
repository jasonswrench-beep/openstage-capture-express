// SPDX-License-Identifier: Apache-2.0
//! Writing captured steps into a guide folder as they are made (spec C10 and section 8.8): the picture first, then `guide.json`
//! in one atomic rename. A crash loses at most the step in flight; every earlier step opens again.

use std::path::Path;

use openstage_express_core::capture::{step_draft, Captured, MarkOptions};
use openstage_express_core::fsutil::{new_guide_id, utc_now_iso};
use openstage_express_core::model::{Guide, StepImage};
use openstage_express_core::{Document, GuideFolder, Position, Session};

/// A picture as PNG, with fast compression (spec 8.5: under 150 ms for 1080p). `bgra` is blue, green, red, alpha for every
/// pixel; the alpha is dropped, because a screen has none.
pub fn encode_bgra_png(width: u32, height: u32, bgra: &[u8]) -> Result<Vec<u8>, String> {
    let pixels = (width as usize).checked_mul(height as usize).ok_or("The picture is too large.")?;
    if pixels == 0 || bgra.len() != pixels * 4 {
        return Err("The picture's bytes do not match its size.".to_string());
    }
    let mut rgb = Vec::with_capacity(pixels * 3);
    for px in bgra.as_chunks::<4>().0 {
        rgb.extend_from_slice(&[px[2], px[1], px[0]]);
    }
    let mut out = Vec::with_capacity(pixels / 2);
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        encoder.set_filter(png::FilterType::Sub);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&rgb).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// A guide being captured into.
pub struct GuideWriter {
    folder: GuideFolder,
    session: Session,
    marks: MarkOptions,
}

impl GuideWriter {
    /// Make a new guide folder inside `parent`. The folder is named from `title` and never replaces one that exists.
    pub fn create(parent: &Path, title: &str, language: &str, marks: MarkOptions) -> Result<GuideWriter, String> {
        let now = utc_now_iso();
        let guide = Guide::new(&new_guide_id(&now), title, language, &now);
        let doc = Document::new(guide);
        let folder = GuideFolder::create(parent, &doc).map_err(|e| e.to_string())?;
        Ok(GuideWriter { folder, session: Session::new(doc), marks })
    }

    pub fn path(&self) -> &Path {
        self.folder.path()
    }

    #[cfg(test)]
    pub fn doc(&self) -> &Document {
        self.session.doc()
    }

    pub fn steps(&self) -> usize {
        self.session.doc().guide.steps.len()
    }

    /// Write a captured step: its picture, then the guide. Returns the new step's id. If it fails the guide is as it was and
    /// the caller can say why and carry on.
    pub fn add(&mut self, captured: &Captured) -> Result<String, String> {
        let png = encode_bgra_png(captured.picture.width, captured.picture.height, &captured.picture.bgra)?;
        let number = self.session.doc().guide.next_step_number;
        let name = self.folder.add_picture(number, &png).map_err(|e| e.to_string())?;
        let image = StepImage::new(&name, captured.picture.width, captured.picture.height);
        let now = utc_now_iso();
        let draft = step_draft(captured, image, &self.marks, &now);
        let id = self.session.apply(|doc| doc.add_step(draft, Position::End, &now)).map_err(|e| e.to_string())?;
        if let Err(why) = self.folder.save(self.session.doc()) {
            // The picture is on disk and the step is in memory but not saved: say so, and the next save will include it.
            return Err(format!("The step was made but the guide could not be saved: {why}"));
        }
        Ok(id)
    }

    /// Give the folder and the guide to the editor, which starts its own undo history.
    pub fn into_parts(self) -> (GuideFolder, Document) {
        let doc = self.session.doc().clone();
        (self.folder, doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstage_express_core::builder::{Draft, DraftKind, ShotId};
    use openstage_express_core::capture::Picture;
    use openstage_express_core::model::{Button, Click, Control};
    use openstage_express_core::GuideFolder;

    fn captured(title: &str, shade: u8) -> Captured {
        let (w, h) = (16u32, 9u32);
        let mut bgra = Vec::new();
        for y in 0..h {
            for x in 0..w {
                bgra.extend_from_slice(&[shade, (x * 10) as u8, (y * 20) as u8, 255]);
            }
        }
        Captured {
            draft: Draft {
                kind: DraftKind::Click,
                title: title.to_string(),
                shot: ShotId(1),
                click: Some(Click { x: 8, y: 4, button: Button::Left, extra: Default::default() }),
                control: Some(Control { name: "Save".into(), role: "button".into(), extra: Default::default() }),
                typed: None,
                at: 10,
            },
            picture: Picture::new(w, h, bgra).unwrap(),
        }
    }

    #[test]
    fn a_png_made_from_screen_pixels_decodes_to_the_same_colours() {
        let c = captured("x", 7);
        let bytes = encode_bgra_png(16, 9, &c.picture.bgra).unwrap();
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (16, 9));
        assert_eq!(info.color_type, png::ColorType::Rgb);
        // Pixel (3, 2): blue 7, green 30, red 40 in the source, so red, green, blue = 40, 30, 7 in the PNG.
        let at = (2 * 16 + 3) * 3;
        assert_eq!(&buf[at..at + 3], &[40, 30, 7]);
    }

    #[test]
    fn wrong_sized_pictures_are_refused() {
        assert!(encode_bgra_png(2, 2, &[0; 15]).is_err());
        assert!(encode_bgra_png(0, 2, &[]).is_err());
    }

    #[test]
    fn each_step_is_on_disk_the_moment_it_is_added() {
        let dir = tempfile::tempdir().unwrap();
        let mut writer = GuideWriter::create(dir.path(), "How to save", "en", MarkOptions::default()).unwrap();
        writer.add(&captured("Click \"Save\"", 1)).unwrap();
        writer.add(&captured("Click \"Open\"", 2)).unwrap();
        let path = writer.path().to_path_buf();
        assert_eq!(writer.steps(), 2);
        // A second look at the folder while the first is still open (as after a crash): both steps and both pictures are there.
        let text = std::fs::read_to_string(path.join("guide.json")).unwrap();
        assert!(text.contains("Click \\\"Save\\\"") && text.contains("Click \\\"Open\\\""), "{text}");
        assert!(path.join("images").join("step-0001.png").is_file());
        assert!(path.join("images").join("step-0002.png").is_file());
        drop(writer);
        let opened = GuideFolder::open(&path, &|_| false).unwrap();
        assert_eq!(opened.doc.guide.steps.len(), 2);
        assert!(opened.problems.is_empty(), "{:?}", opened.problems);
    }

    #[test]
    fn a_new_step_carries_its_marks_and_the_focused_view() {
        let dir = tempfile::tempdir().unwrap();
        let mut writer = GuideWriter::create(dir.path(), "Marks", "en", MarkOptions::default()).unwrap();
        let id = writer.add(&captured("Click \"Save\"", 1)).unwrap();
        let step = writer.doc().guide.step(&id).unwrap();
        assert_eq!(step.annotations.len(), 2);
        assert!(step.focus.is_some());
        assert_eq!(step.image.as_ref().map(|i| (i.width, i.height)), Some((16, 9)));
        assert_eq!(step.control.as_ref().map(|c| c.name.as_str()), Some("Save"));
    }

    #[test]
    fn two_guides_with_the_same_title_never_replace_each_other() {
        let dir = tempfile::tempdir().unwrap();
        let a = GuideWriter::create(dir.path(), "Same", "en", MarkOptions::default()).unwrap();
        let b = GuideWriter::create(dir.path(), "Same", "en", MarkOptions::default()).unwrap();
        assert_ne!(a.path(), b.path());
    }
}
