// SPDX-License-Identifier: Apache-2.0
//! Getting a step's original picture in, and flattened pictures out, as PNG files.

use std::collections::HashMap;
use std::io::Cursor;

use openstage_capture_render::{Canvas, Picture};
use openstage_express_core::GuideFolder;

/// The biggest picture the exports will read, in pixels across or down. A bigger one is refused rather than risking all of
/// the computer's memory on a damaged or hostile file.
pub const MAX_SIDE: u32 = 16_384;

/// Where originals come from.
pub trait PictureSource {
    /// The original picture named `images/step-0001.png` (as a step names it), if there is one and it can be read.
    fn picture(&self, logical: &str) -> Option<Picture>;
}

/// Pictures from an open guide folder (looking in `images` and then `trash`).
pub struct FolderPictures<'a> {
    pub folder: &'a GuideFolder,
}

impl PictureSource for FolderPictures<'_> {
    fn picture(&self, logical: &str) -> Option<Picture> {
        let path = self.folder.picture_path(logical)?;
        decode_png(&std::fs::read(path).ok()?).ok()
    }
}

/// Pictures from a guide folder that is named by its path, so the owner does not have to borrow the open folder (looking in
/// `images` and then `trash`, as [`FolderPictures`] does). A name that is not an allowed picture path gives nothing.
#[derive(Debug, Clone)]
pub struct DiskPictures {
    pub root: std::path::PathBuf,
}

impl PictureSource for DiskPictures {
    fn picture(&self, logical: &str) -> Option<Picture> {
        if !openstage_express_core::model::is_safe_picture_path(logical) {
            return None;
        }
        let name = logical.strip_prefix("images/")?;
        let path = ["images", "trash"].iter().map(|dir| self.root.join(dir).join(name)).find(|p| p.is_file())?;
        decode_png(&std::fs::read(path).ok()?).ok()
    }
}

/// Pictures held in memory, for tests and the demo.
#[derive(Debug, Default)]
pub struct MemoryPictures(pub HashMap<String, Picture>);

impl PictureSource for MemoryPictures {
    fn picture(&self, logical: &str) -> Option<Picture> {
        self.0.get(logical).cloned()
    }
}

/// Read a PNG of any colour type or depth into pixels.
pub fn decode_png(bytes: &[u8]) -> Result<Picture, String> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| format!("It is not a PNG picture that can be read ({e})."))?;
    let (width, height) = {
        let info = reader.info();
        (info.width, info.height)
    };
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(format!("The picture is {width} by {height} pixels, which is not a size that can be used."));
    }
    let mut buffer = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buffer).map_err(|e| format!("The picture could not be read ({e})."))?;
    let data = &buffer[..frame.buffer_size()];
    let pixels = width as usize * height as usize;
    let mut bgra = Vec::with_capacity(pixels * 4);
    match frame.color_type {
        png::ColorType::Rgb => {
            data.as_chunks::<3>().0.iter().for_each(|p| bgra.extend_from_slice(&[p[2], p[1], p[0], 255]))
        }
        png::ColorType::Rgba => {
            data.as_chunks::<4>().0.iter().for_each(|p| bgra.extend_from_slice(&[p[2], p[1], p[0], p[3]]))
        }
        png::ColorType::Grayscale => data.iter().for_each(|&g| bgra.extend_from_slice(&[g, g, g, 255])),
        png::ColorType::GrayscaleAlpha => {
            data.as_chunks::<2>().0.iter().for_each(|p| bgra.extend_from_slice(&[p[0], p[0], p[0], p[1]]))
        }
        png::ColorType::Indexed => {
            return Err("The picture could not be read (its colours were not expanded).".to_string())
        }
    }
    if bgra.len() != pixels * 4 {
        return Err("The picture could not be read (its size does not match its data).".to_string());
    }
    Ok(Picture::new(width, height, bgra))
}

/// A canvas as a PNG file (no transparency: a canvas is always solid).
pub fn encode_png(canvas: &Canvas) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(canvas.width as usize * canvas.height as usize * 3);
    for p in canvas.bgra.as_chunks::<4>().0 {
        rgb.extend_from_slice(&[p[2], p[1], p[0]]);
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, canvas.width.max(1), canvas.height.max(1));
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Default);
        let mut writer = encoder.write_header().expect("an in-memory PNG header always writes");
        writer.write_image_data(&rgb).expect("an in-memory PNG always writes");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstage_capture_render::{PixelRect, Rgb};

    fn striped() -> Canvas {
        let mut c = Canvas::new(40, 30);
        c.clear(Rgb(250, 250, 250));
        c.fill_rect(PixelRect::new(5, 5, 10, 10), Rgb(200, 30, 60));
        c.fill_rect(PixelRect::new(20, 10, 15, 15), Rgb(10, 120, 240));
        c
    }

    #[test]
    fn disk_pictures_read_from_images_then_trash_and_refuse_unsafe_names() {
        let dir = tempfile::tempdir().unwrap();
        for sub in ["images", "trash"] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
        }
        let png = encode_png(&striped());
        std::fs::write(dir.path().join("images").join("step-0001.png"), &png).unwrap();
        std::fs::write(dir.path().join("trash").join("step-0002.png"), &png).unwrap();
        let pictures = DiskPictures { root: dir.path().to_path_buf() };
        assert_eq!(pictures.picture("images/step-0001.png").map(|p| (p.width, p.height)), Some((40, 30)));
        assert!(pictures.picture("images/step-0002.png").is_some(), "a deleted step's picture is found in the trash");
        assert!(pictures.picture("images/step-0003.png").is_none(), "a picture that is not there");
        assert!(pictures.picture("../guide.json").is_none());
        assert!(pictures.picture("images/../../x.png").is_none());
        assert!(pictures.picture("C:/Windows/x.png").is_none());
    }

    #[test]
    fn a_canvas_survives_a_trip_through_png_exactly() {
        let canvas = striped();
        let png = encode_png(&canvas);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        let back = decode_png(&png).unwrap();
        assert_eq!((back.width, back.height), (40, 30));
        assert_eq!(back.bgra, canvas.bgra);
    }

    #[test]
    fn a_picture_with_transparency_keeps_it() {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, 2, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[255, 0, 0, 255, 0, 255, 0, 64]).unwrap();
        }
        let picture = decode_png(&out).unwrap();
        assert_eq!(picture.bgra, [0, 0, 255, 255, 0, 255, 0, 64]);
    }

    #[test]
    fn grey_and_palette_pictures_are_read_too() {
        let mut grey = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut grey, 2, 1);
            encoder.set_color(png::ColorType::Grayscale);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header().unwrap().write_image_data(&[0, 200]).unwrap();
        }
        assert_eq!(decode_png(&grey).unwrap().bgra, [0, 0, 0, 255, 200, 200, 200, 255]);
        let mut indexed = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut indexed, 2, 1);
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_palette(vec![10, 20, 30, 40, 50, 60]);
            encoder.write_header().unwrap().write_image_data(&[1, 0]).unwrap();
        }
        assert_eq!(decode_png(&indexed).unwrap().bgra, [60, 50, 40, 255, 30, 20, 10, 255]);
    }

    #[test]
    fn sixteen_bit_pictures_are_read_as_eight() {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, 1, 1);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Sixteen);
            encoder.write_header().unwrap().write_image_data(&[0xFF, 0xFF, 0x80, 0x00, 0x00, 0x00]).unwrap();
        }
        assert_eq!(decode_png(&out).unwrap().bgra, [0, 0x80, 255, 255]);
    }

    #[test]
    fn rubbish_and_empty_files_are_refused_with_a_plain_message() {
        for bad in [&b""[..], b"not a picture", b"\x89PNG\r\n\x1a\nbroken"] {
            let error = decode_png(bad).unwrap_err();
            assert!(error.contains("PNG") || error.contains("picture"), "{error}");
        }
    }

    #[test]
    fn a_picture_of_an_unreasonable_size_is_refused() {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, MAX_SIDE + 1, 1);
            encoder.set_color(png::ColorType::Grayscale);
            encoder.set_depth(png::BitDepth::One);
            encoder.write_header().unwrap().write_image_data(&vec![0; (MAX_SIDE as usize + 1).div_ceil(8)]).unwrap();
        }
        assert!(decode_png(&out).unwrap_err().contains("not a size that can be used"));
    }

    #[test]
    fn memory_pictures_answer_by_name() {
        let mut pictures = MemoryPictures::default();
        pictures.0.insert("images/a.png".into(), Picture::solid(2, 2, Rgb(1, 2, 3)));
        assert!(pictures.picture("images/a.png").is_some());
        assert!(pictures.picture("images/b.png").is_none());
    }
}
