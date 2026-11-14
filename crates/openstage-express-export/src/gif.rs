// SPDX-License-Identifier: Apache-2.0
//! The animated GIF export (spec section 10.3): one frame for each step, written by this program's own encoder (no GIF
//! library, so nothing to licence and the same bytes on every computer).
//!
//! * Every frame is the step's flattened picture scaled to the same width. Frames of different heights are centred on a
//!   background colour so the animation does not jump.
//! * A step with no picture (an empty step or a content block) is a card with its title.
//! * Each frame stays for 1, 2, 3, 5 or 8 seconds (3 by default); the animation loops unless told not to; a title card and a
//!   progress bar are options.
//! * A GIF cannot be made accessible: it has no text a screen reader can read. The HTML export is the one to share.
//!
//! Each frame has its own colour table of up to 256 colours. A frame with 256 colours or fewer keeps them exactly;
//! otherwise the colours are reduced by median cut on a 5 bits per channel histogram, which is deterministic.

use std::collections::hash_map::Entry;
use std::collections::HashMap;

use openstage_capture_project::EditAlign;
use openstage_capture_render::{Canvas, PixelRect, Rgb, TextRenderer, TextRequest};
use openstage_express_core::fsutil::sanitise_folder_name;
use openstage_express_core::render::scale_to_width;

use crate::picture::decode_png;
use crate::plan::Plan;
use crate::{Export, OutFile};

/// The least and the most a GIF may be across, in pixels.
pub const MIN_WIDTH: u32 = 320;
pub const MAX_WIDTH: u32 = 1920;
/// The seconds a frame may stay (spec: 1, 2, 3, 5 or 8).
pub const ALLOWED_SECONDS: [u32; 5] = [1, 2, 3, 5, 8];
/// A GIF bigger than this gets a warning.
pub const SIZE_WARNING_BYTES: usize = 10 * 1024 * 1024;

/// How the GIF is made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GifOptions {
    /// Every frame is this many pixels across (kept between 320 and 1920).
    pub max_width: u32,
    /// How long each frame stays: the nearest of 1, 2, 3, 5 and 8.
    pub seconds_per_step: u32,
    pub loop_forever: bool,
    /// What shows round a picture that is shorter than the tallest.
    pub background: Rgb,
    /// A thin bar along the bottom showing how far through the guide the frame is.
    pub progress_bar: bool,
    /// A first frame with the guide's title.
    pub title_card: bool,
}

impl Default for GifOptions {
    fn default() -> GifOptions {
        GifOptions {
            max_width: 800,
            seconds_per_step: 3,
            loop_forever: true,
            background: Rgb(255, 255, 255),
            progress_bar: false,
            title_card: false,
        }
    }
}

impl GifOptions {
    fn width(&self) -> u32 {
        self.max_width.clamp(MIN_WIDTH, MAX_WIDTH)
    }

    /// The delay in hundredths of a second, using the nearest allowed number of seconds.
    fn delay_cs(&self) -> u16 {
        let nearest = ALLOWED_SECONDS.iter().min_by_key(|s| s.abs_diff(self.seconds_per_step)).copied().unwrap_or(3);
        (nearest * 100) as u16
    }
}

const NAVY: Rgb = Rgb(0x0B, 0x17, 0x2A);
const ACCENT: Rgb = Rgb(0x7D, 0x87, 0xF2);
const TRACK: Rgb = Rgb(0xD0, 0xD4, 0xE4);

// ---- colours

/// Reduce a canvas to a colour table of at most 256 colours and one index for each pixel.
pub fn quantize(canvas: &Canvas) -> (Vec<[u8; 3]>, Vec<u8>) {
    let (pixels, _) = canvas.bgra.as_chunks::<4>();
    // The exact way: if the canvas has 256 colours or fewer, keep every one of them.
    let mut index_of: HashMap<u32, u8> = HashMap::new();
    let mut table: Vec<[u8; 3]> = Vec::new();
    let mut exact = true;
    for p in pixels {
        let key = (p[2] as u32) << 16 | (p[1] as u32) << 8 | p[0] as u32;
        if let Entry::Vacant(slot) = index_of.entry(key) {
            if table.len() == 256 {
                exact = false;
                break;
            }
            slot.insert(table.len() as u8);
            table.push([p[2], p[1], p[0]]);
        }
    }
    if exact {
        let indices =
            pixels.iter().map(|p| index_of[&((p[2] as u32) << 16 | (p[1] as u32) << 8 | p[0] as u32)]).collect();
        return (table, indices);
    }

    // Median cut on a histogram of 5 bits per channel.
    let key15 = |p: &[u8]| ((p[2] as usize >> 3) << 10) | ((p[1] as usize >> 3) << 5) | (p[0] as usize >> 3);
    let mut histogram = vec![0u32; 32768];
    let mut sums = vec![[0u64; 3]; 32768];
    for p in pixels {
        let key = key15(p);
        histogram[key] += 1;
        sums[key][0] += p[2] as u64;
        sums[key][1] += p[1] as u64;
        sums[key][2] += p[0] as u64;
    }
    let colours: Vec<(u16, u32)> =
        histogram.iter().enumerate().filter(|(_, c)| **c > 0).map(|(k, c)| (k as u16, *c)).collect();
    let channel = |key: u16, which: usize| -> u8 {
        match which {
            0 => (key >> 10) as u8,
            1 => ((key >> 5) & 31) as u8,
            _ => (key & 31) as u8,
        }
    };
    let mut boxes: Vec<Vec<(u16, u32)>> = vec![colours];
    while boxes.len() < 256 {
        let Some((at, _)) = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .max_by_key(|(i, b)| (b.iter().map(|c| c.1 as u64).sum::<u64>(), std::cmp::Reverse(*i)))
        else {
            break;
        };
        let mut current = boxes.swap_remove(at);
        // swap_remove moved the last box into `at`; put things back in a stable order afterwards.
        let spans: Vec<u8> = (0..3)
            .map(|w| {
                let (lo, hi) =
                    current.iter().fold((31u8, 0u8), |(lo, hi), c| (lo.min(channel(c.0, w)), hi.max(channel(c.0, w))));
                hi - lo
            })
            .collect();
        let widest = (0..3).max_by_key(|&w| (spans[w], std::cmp::Reverse(w))).unwrap_or(0);
        current.sort_by_key(|c| (channel(c.0, widest), c.0));
        let total: u64 = current.iter().map(|c| c.1 as u64).sum();
        let mut running = 0u64;
        let mut cut = current.len() - 1;
        for (i, c) in current.iter().enumerate() {
            running += c.1 as u64;
            if running * 2 >= total {
                cut = (i + 1).clamp(1, current.len() - 1);
                break;
            }
        }
        let second = current.split_off(cut);
        boxes.push(current);
        boxes.push(second);
        // Keep the order of boxes the same whatever swap_remove did, so the palette is the same every time.
        boxes.sort_by_key(|b| b.first().map(|c| c.0).unwrap_or(0));
    }
    let mut index_of_key = vec![0u8; 32768];
    let mut table: Vec<[u8; 3]> = Vec::with_capacity(boxes.len());
    for (i, b) in boxes.iter().enumerate() {
        let total: u64 = b.iter().map(|c| c.1 as u64).sum::<u64>().max(1);
        let mut sum = [0u64; 3];
        for &(key, _) in b {
            index_of_key[key as usize] = i as u8;
            for (w, s) in sum.iter_mut().enumerate() {
                *s += sums[key as usize][w];
            }
        }
        table.push([
            ((sum[0] + total / 2) / total) as u8,
            ((sum[1] + total / 2) / total) as u8,
            ((sum[2] + total / 2) / total) as u8,
        ]);
    }
    let indices = pixels.iter().map(|p| index_of_key[key15(p)]).collect();
    (table, indices)
}

// ---- the compression GIF uses

/// A bit packer that fills each byte from its low end, as GIF wants.
struct BitWriter {
    bytes: Vec<u8>,
    current: u32,
    used: u32,
}

impl BitWriter {
    fn new() -> BitWriter {
        BitWriter { bytes: Vec::new(), current: 0, used: 0 }
    }

    fn put(&mut self, code: u16, bits: u32) {
        self.current |= (code as u32) << self.used;
        self.used += bits;
        while self.used >= 8 {
            self.bytes.push((self.current & 0xFF) as u8);
            self.current >>= 8;
            self.used -= 8;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.used > 0 {
            self.bytes.push(self.current as u8);
        }
        self.bytes
    }
}

/// GIF's variant of LZW over colour indices. `min_code_size` is 2 to 8.
pub fn lzw_encode(indices: &[u8], min_code_size: u8) -> Vec<u8> {
    let clear = 1u16 << min_code_size;
    let eoi = clear + 1;
    let mut writer = BitWriter::new();
    let mut code_size = min_code_size as u32 + 1;
    let mut next = eoi + 1;
    let mut table: HashMap<(u16, u8), u16> = HashMap::new();
    writer.put(clear, code_size);
    let Some((&first, rest)) = indices.split_first() else {
        writer.put(eoi, code_size);
        return writer.finish();
    };
    let mut prefix = first as u16;
    for &k in rest {
        if let Some(&code) = table.get(&(prefix, k)) {
            prefix = code;
            continue;
        }
        writer.put(prefix, code_size);
        if next == 4096 {
            writer.put(clear, code_size);
            table.clear();
            next = eoi + 1;
            code_size = min_code_size as u32 + 1;
        } else {
            if next as u32 >= (1 << code_size) {
                code_size += 1;
            }
            table.insert((prefix, k), next);
            next += 1;
        }
        prefix = k as u16;
    }
    writer.put(prefix, code_size);
    writer.put(eoi, code_size);
    writer.finish()
}

// ---- the file

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Write frames (all the same size) as an animated GIF. `delays_cs` is how long each stays, in hundredths of a second.
pub fn encode_gif(frames: &[Canvas], delays_cs: &[u16], loop_forever: bool) -> Vec<u8> {
    assert_eq!(frames.len(), delays_cs.len(), "one delay for each frame");
    let (width, height) = frames.first().map(|f| (f.width as u16, f.height as u16)).unwrap_or((1, 1));
    let mut out = Vec::new();
    out.extend_from_slice(b"GIF89a");
    put_u16(&mut out, width);
    put_u16(&mut out, height);
    out.extend_from_slice(&[0x70, 0, 0]);
    if loop_forever {
        out.extend_from_slice(&[0x21, 0xFF, 0x0B]);
        out.extend_from_slice(b"NETSCAPE2.0");
        out.extend_from_slice(&[0x03, 0x01, 0x00, 0x00, 0x00]);
    }
    for (frame, delay) in frames.iter().zip(delays_cs) {
        let (mut table, indices) = quantize(frame);
        let size = table.len().next_power_of_two().max(2);
        table.resize(size, [0, 0, 0]);
        let bits = size.trailing_zeros() as u8;
        out.extend_from_slice(&[0x21, 0xF9, 0x04, 0x04]);
        put_u16(&mut out, *delay);
        out.extend_from_slice(&[0x00, 0x00]);
        out.push(0x2C);
        put_u16(&mut out, 0);
        put_u16(&mut out, 0);
        put_u16(&mut out, frame.width as u16);
        put_u16(&mut out, frame.height as u16);
        out.push(0x80 | (bits - 1));
        for colour in &table {
            out.extend_from_slice(colour);
        }
        let min_code_size = bits.max(2);
        out.push(min_code_size);
        for block in lzw_encode(&indices, min_code_size).chunks(255) {
            out.push(block.len() as u8);
            out.extend_from_slice(block);
        }
        out.push(0);
    }
    out.push(0x3B);
    out
}

// ---- the export

/// A card with some words on it, for a step with no picture and for the title.
fn card(width: u32, height: u32, words: &str, font_px: f32, text: &dyn TextRenderer) -> Canvas {
    let mut canvas = Canvas::new(width, height);
    canvas.clear(NAVY);
    let margin = (width / 12) as i32;
    let area = PixelRect::new(margin, (height / 3) as i32, width as i32 - 2 * margin, (height / 2) as i32);
    let request = TextRequest {
        text: words.to_string(),
        area,
        font_px,
        bold: true,
        color: Rgb(255, 255, 255),
        background: None,
        align: EditAlign::Center,
    };
    let _ = text.draw(&mut canvas, &request);
    canvas
}

/// Put a frame in the middle of a canvas of the given size, on a background colour.
fn pad_to(frame: &Canvas, width: u32, height: u32, background: Rgb) -> Canvas {
    let mut out = Canvas::new(width, height);
    out.clear(background);
    let top = ((height - frame.height) / 2) as usize;
    for row in 0..frame.height as usize {
        let to = ((top + row) * width as usize) * 4;
        let from = row * frame.width as usize * 4;
        out.bgra[to..to + frame.width as usize * 4].copy_from_slice(&frame.bgra[from..from + frame.width as usize * 4]);
    }
    out
}

fn draw_progress(canvas: &mut Canvas, done: usize, of: usize) {
    let bar = (canvas.height / 60).clamp(4, 10) as i32;
    let (w, h) = (canvas.width as i32, canvas.height as i32);
    canvas.fill_rect(PixelRect::new(0, h - bar, w, bar), TRACK);
    let filled = (w as i64 * done as i64 / of.max(1) as i64) as i32;
    canvas.fill_rect(PixelRect::new(0, h - bar, filled, bar), ACCENT);
}

/// The tallest frame a GIF is made with. The format allows 65,535 pixels; a frame of a step is never sensibly taller than this,
/// and every frame is padded to the tallest, so one very tall picture made the whole GIF enormous (the security review of
/// 2026-11-13). A taller picture is shown as a card with its title instead.
pub const MAX_FRAME_HEIGHT: u32 = 8192;

/// The most pixels all of a GIF's frames together may have, once each is padded to the tallest: 256 million, a gigabyte of
/// colour. A GIF that would be bigger is not made, and the export says what to change.
pub const MAX_TOTAL_PIXELS: u64 = 256 * 1024 * 1024;

/// Make the GIF for a plan.
pub fn export_gif(plan: &Plan, options: &GifOptions, text: &dyn TextRenderer) -> Export {
    let width = options.width();
    let mut warnings = plan.warnings.clone();
    let mut frames: Vec<Canvas> = Vec::new();
    let card_height = width * 9 / 16;
    if options.title_card {
        frames.push(card(width, card_height, &plan.title, width as f32 / 14.0, text));
    }
    for step in &plan.steps {
        let frame = match step.picture.as_ref().and_then(|p| decode_png(&p.png).ok()) {
            Some(picture) => {
                let canvas = {
                    let mut c = Canvas::new(picture.width, picture.height);
                    c.bgra = picture.bgra.clone();
                    c
                };
                let scaled = scale_to_width(&canvas, width);
                if scaled.height > MAX_FRAME_HEIGHT {
                    warnings.push(format!("The picture of \"{}\" is too tall for a GIF, so its frame shows the step's title.", step.title));
                    card(width, card_height, &step.title, width as f32 / 18.0, text)
                } else {
                    scaled
                }
            }
            None => card(width, card_height, &step.title, width as f32 / 18.0, text),
        };
        frames.push(frame);
    }
    if frames.is_empty() {
        frames.push(card(width, card_height, &plan.title, width as f32 / 14.0, text));
        warnings.push("The guide has no steps to show, so the GIF is only its title.".to_string());
    }
    let height = frames.iter().map(|f| f.height).max().unwrap_or(card_height).min(MAX_FRAME_HEIGHT);
    if frames.len() as u64 * width as u64 * height as u64 > MAX_TOTAL_PIXELS {
        warnings.push("The GIF would be too big to make: choose a smaller width, or make it from fewer steps.".to_string());
        return Export { files: Vec::new(), warnings };
    }
    let steps_total = frames.len() - usize::from(options.title_card && !plan.steps.is_empty());
    let mut padded: Vec<Canvas> = Vec::with_capacity(frames.len());
    for (i, frame) in frames.iter().enumerate() {
        let mut canvas = pad_to(frame, width, height, options.background);
        if options.progress_bar {
            let step_number = i + 1 - usize::from(options.title_card && !plan.steps.is_empty());
            draw_progress(
                &mut canvas,
                if options.title_card && i == 0 && !plan.steps.is_empty() { 0 } else { step_number },
                steps_total,
            );
        }
        padded.push(canvas);
    }
    let delays = vec![options.delay_cs(); padded.len()];
    let bytes = encode_gif(&padded, &delays, options.loop_forever);
    if bytes.len() > SIZE_WARNING_BYTES {
        warnings.push(format!(
            "The GIF is {:.1} MB, which is large. A smaller width or fewer steps makes a smaller file.",
            bytes.len() as f64 / (1024.0 * 1024.0)
        ));
    }
    Export { files: vec![OutFile { path: format!("{}.gif", sanitise_folder_name(&plan.title)), bytes }], warnings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo;
    use crate::plan::{plan, ExportOptions};
    use openstage_capture_render::BlockText;

    // ---- a small GIF reader, only for the tests, so the encoder is checked by something that is not itself

    struct Decoded {
        width: u16,
        height: u16,
        looped: bool,
        frames: Vec<DecodedFrame>,
    }

    struct DecodedFrame {
        delay_cs: u16,
        table: Vec<[u8; 3]>,
        indices: Vec<u8>,
    }

    fn lzw_decode(data: &[u8], min_code_size: u8, expected: usize) -> Vec<u8> {
        let clear = 1u16 << min_code_size;
        let eoi = clear + 1;
        let mut out: Vec<u8> = Vec::with_capacity(expected);
        let mut dictionary: Vec<Vec<u8>> = Vec::new();
        let reset = |dictionary: &mut Vec<Vec<u8>>| {
            dictionary.clear();
            for i in 0..clear {
                dictionary.push(vec![i as u8]);
            }
            dictionary.push(vec![]);
            dictionary.push(vec![]);
        };
        reset(&mut dictionary);
        let mut code_size = min_code_size as u32 + 1;
        let (mut bit_buffer, mut bit_count, mut position) = (0u32, 0u32, 0usize);
        let mut previous: Option<u16> = None;
        loop {
            while bit_count < code_size {
                if position >= data.len() {
                    return out;
                }
                bit_buffer |= (data[position] as u32) << bit_count;
                bit_count += 8;
                position += 1;
            }
            let code = (bit_buffer & ((1 << code_size) - 1)) as u16;
            bit_buffer >>= code_size;
            bit_count -= code_size;
            if code == clear {
                reset(&mut dictionary);
                code_size = min_code_size as u32 + 1;
                previous = None;
                continue;
            }
            if code == eoi {
                return out;
            }
            let entry: Vec<u8> = if (code as usize) < dictionary.len() {
                dictionary[code as usize].clone()
            } else if code as usize == dictionary.len() && previous.is_some() {
                let mut e = dictionary[previous.unwrap() as usize].clone();
                e.push(e[0]);
                e
            } else {
                panic!("bad code {code} with {} entries", dictionary.len());
            };
            out.extend_from_slice(&entry);
            if let Some(p) = previous {
                let mut new = dictionary[p as usize].clone();
                new.push(entry[0]);
                dictionary.push(new);
                if dictionary.len() == (1 << code_size) && code_size < 12 {
                    code_size += 1;
                }
            }
            previous = Some(code);
        }
    }

    fn decode(bytes: &[u8]) -> Decoded {
        assert_eq!(&bytes[..6], b"GIF89a");
        let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        let (width, height) = (u16_at(6), u16_at(8));
        assert_eq!(bytes[10] & 0x80, 0, "no global colour table");
        let mut i = 13;
        let (mut looped, mut delay) = (false, 0u16);
        let mut frames = Vec::new();
        loop {
            match bytes[i] {
                0x3B => break,
                0x21 => {
                    let label = bytes[i + 1];
                    if label == 0xF9 {
                        assert_eq!(bytes[i + 2], 4);
                        delay = u16_at(i + 4);
                    }
                    if label == 0xFF {
                        assert_eq!(&bytes[i + 3..i + 14], b"NETSCAPE2.0");
                        looped = true;
                    }
                    i += 2;
                    while bytes[i] != 0 {
                        i += bytes[i] as usize + 1;
                    }
                    i += 1;
                }
                0x2C => {
                    let (w, h) = (u16_at(i + 5) as usize, u16_at(i + 7) as usize);
                    let packed = bytes[i + 9];
                    assert_eq!(packed & 0x80, 0x80, "a local colour table");
                    let size = 2usize << (packed & 7);
                    i += 10;
                    let table: Vec<[u8; 3]> =
                        (0..size).map(|k| [bytes[i + k * 3], bytes[i + k * 3 + 1], bytes[i + k * 3 + 2]]).collect();
                    i += size * 3;
                    let min_code_size = bytes[i];
                    i += 1;
                    let mut data = Vec::new();
                    while bytes[i] != 0 {
                        let len = bytes[i] as usize;
                        data.extend_from_slice(&bytes[i + 1..i + 1 + len]);
                        i += len + 1;
                    }
                    i += 1;
                    let indices = lzw_decode(&data, min_code_size, w * h);
                    assert_eq!(indices.len(), w * h, "every pixel comes back");
                    frames.push(DecodedFrame { delay_cs: delay, table, indices });
                }
                other => panic!("unexpected block {other:#x} at {i}"),
            }
        }
        Decoded { width, height, looped, frames }
    }

    fn colour_at(frame: &DecodedFrame, width: usize, x: usize, y: usize) -> [u8; 3] {
        frame.table[frame.indices[y * width + x] as usize]
    }

    /// True if every channel of `a` is within `tolerance` of `b`.
    fn near(a: [u8; 3], b: [u8; 3], tolerance: i32) -> bool {
        a.iter().zip(b.iter()).all(|(x, y)| (*x as i32 - *y as i32).abs() <= tolerance)
    }

    fn solid(w: u32, h: u32, colour: Rgb) -> Canvas {
        let mut c = Canvas::new(w, h);
        c.clear(colour);
        c
    }

    // ---- LZW

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    #[test]
    fn lzw_round_trips_every_kind_of_data() {
        let mut rng = Rng(99);
        let mut cases: Vec<(Vec<u8>, u8)> = vec![
            (vec![], 2),
            (vec![0], 2),
            (vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1], 2),
            (vec![0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3], 2),
            (vec![7; 100_000], 3),
            ((0..=255).collect(), 8),
        ];
        cases.push(((0..50_000).map(|_| (rng.next() % 4) as u8).collect(), 2));
        cases.push(((0..80_000).map(|_| (rng.next() % 256) as u8).collect(), 8));
        cases.push(((0..30_000).map(|i| ((i / 7) % 13) as u8).collect(), 4));
        for (data, bits) in cases {
            let packed = lzw_encode(&data, bits.max(2));
            assert_eq!(lzw_decode(&packed, bits.max(2), data.len()), data, "{} values at {bits} bits", data.len());
        }
    }

    #[test]
    fn lzw_compresses_flat_pictures_a_great_deal() {
        let flat = vec![3u8; 640 * 360];
        assert!(lzw_encode(&flat, 2).len() < 2_000);
    }

    // ---- colours

    #[test]
    fn a_picture_with_few_colours_keeps_every_one_exactly() {
        let mut c = solid(50, 40, Rgb(255, 255, 255));
        c.fill_rect(PixelRect::new(5, 5, 20, 20), Rgb(200, 30, 60));
        c.fill_rect(PixelRect::new(30, 10, 15, 25), Rgb(10, 120, 240));
        let (table, indices) = quantize(&c);
        assert_eq!(table.len(), 3);
        for y in 0..40 {
            for x in 0..50 {
                let Rgb(r, g, b) = c.pixel(x, y);
                assert_eq!(table[indices[(y * 50 + x) as usize] as usize], [r, g, b]);
            }
        }
    }

    #[test]
    fn a_picture_with_many_colours_is_reduced_to_256_close_ones() {
        let mut c = Canvas::new(256, 128);
        for y in 0..128 {
            for x in 0..256 {
                c.set_pixel(x, y, Rgb(x as u8, (y * 2) as u8, ((x + y) / 2) as u8));
            }
        }
        let (table, indices) = quantize(&c);
        assert!(table.len() <= 256 && table.len() > 128, "{}", table.len());
        let mut worst = 0i32;
        let mut total = 0i64;
        for y in 0..128u32 {
            for x in 0..256u32 {
                let Rgb(r, g, b) = c.pixel(x, y);
                let [pr, pg, pb] = table[indices[(y * 256 + x) as usize] as usize];
                let error =
                    (r as i32 - pr as i32).abs().max((g as i32 - pg as i32).abs()).max((b as i32 - pb as i32).abs());
                worst = worst.max(error);
                total += error as i64;
            }
        }
        assert!(worst <= 16, "the worst colour is {worst} out of 255 away");
        assert!(total as f64 / (256.0 * 128.0) < 6.0, "average error {}", total as f64 / (256.0 * 128.0));
    }

    #[test]
    fn colour_reduction_is_the_same_every_time() {
        let mut c = Canvas::new(120, 120);
        for y in 0..120 {
            for x in 0..120 {
                c.set_pixel(x, y, Rgb((x * 2) as u8, (y * 2) as u8, ((x * y) % 256) as u8));
            }
        }
        assert_eq!(quantize(&c), quantize(&c));
    }

    // ---- the file

    #[test]
    fn a_gif_reads_back_with_its_size_frames_delays_and_loop() {
        let frames = vec![solid(64, 36, Rgb(255, 0, 0)), solid(64, 36, Rgb(0, 255, 0)), solid(64, 36, Rgb(0, 0, 255))];
        let bytes = encode_gif(&frames, &[100, 200, 300], true);
        let d = decode(&bytes);
        assert_eq!((d.width, d.height), (64, 36));
        assert!(d.looped);
        assert_eq!(d.frames.len(), 3);
        assert_eq!(d.frames.iter().map(|f| f.delay_cs).collect::<Vec<_>>(), [100, 200, 300]);
        assert_eq!(colour_at(&d.frames[0], 64, 10, 10), [255, 0, 0]);
        assert_eq!(colour_at(&d.frames[1], 64, 63, 35), [0, 255, 0]);
        assert_eq!(colour_at(&d.frames[2], 64, 0, 0), [0, 0, 255]);
        assert_eq!(*bytes.last().unwrap(), 0x3B);
    }

    #[test]
    fn a_gif_that_does_not_loop_has_no_loop_block() {
        let bytes = encode_gif(&[solid(8, 8, Rgb(1, 2, 3))], &[100], false);
        assert!(!decode(&bytes).looped);
        assert!(!bytes.windows(11).any(|w| w == b"NETSCAPE2.0"));
    }

    #[test]
    fn a_one_pixel_gif_works() {
        let bytes = encode_gif(&[solid(1, 1, Rgb(9, 8, 7))], &[50], true);
        let d = decode(&bytes);
        assert_eq!((d.width, d.height, d.frames.len()), (1, 1, 1));
        assert_eq!(colour_at(&d.frames[0], 1, 0, 0), [9, 8, 7]);
    }

    #[test]
    fn a_busy_frame_survives_the_trip() {
        let mut c = Canvas::new(97, 53);
        for y in 0..53 {
            for x in 0..97 {
                c.set_pixel(
                    x,
                    y,
                    Rgb(((x * 37 + y * 11) % 256) as u8, ((x * 5 + y * 91) % 256) as u8, ((x * y) % 256) as u8),
                );
            }
        }
        let (table, indices) = quantize(&c);
        let bytes = encode_gif(std::slice::from_ref(&c), &[10], false);
        let d = decode(&bytes);
        assert_eq!(d.frames[0].indices, indices, "the picture's indices come back exactly");
        assert_eq!(&d.frames[0].table[..table.len()], &table[..], "and so does the colour table");
    }

    // ---- the export

    fn demo_plan() -> Plan {
        let d = demo();
        plan(&d.document.guide, &d.pictures, &BlockText, &ExportOptions::default())
    }

    #[test]
    fn the_export_has_one_frame_for_each_step_all_the_same_size() {
        let p = demo_plan();
        let export = export_gif(&p, &GifOptions::default(), &BlockText);
        assert_eq!(export.files.len(), 1);
        assert_eq!(export.files[0].path, "How to submit an assignment.gif");
        let d = decode(&export.files[0].bytes);
        assert_eq!(d.frames.len(), p.steps.len());
        assert_eq!(d.width, 800);
        assert!(d.looped);
        assert!(d.frames.iter().all(|f| f.delay_cs == 300));
        assert!(d.frames.iter().all(|f| f.indices.len() == d.width as usize * d.height as usize));
        assert!(export.warnings.is_empty(), "{:?}", export.warnings);
    }

    #[test]
    fn options_set_the_width_the_delay_the_loop_and_the_title_card() {
        let p = demo_plan();
        let options = GifOptions {
            max_width: 100,
            seconds_per_step: 4,
            loop_forever: false,
            title_card: true,
            ..GifOptions::default()
        };
        let d = decode(&export_gif(&p, &options, &BlockText).files[0].bytes);
        assert_eq!(d.width, 320, "never narrower than 320");
        assert_eq!(d.frames.len(), p.steps.len() + 1);
        assert!(!d.looped);
        assert!(
            d.frames.iter().all(|f| f.delay_cs == 300 || f.delay_cs == 500),
            "4 seconds is not allowed, so the nearest is used"
        );
        assert_eq!(colour_at(&d.frames[0], 320, 2, 2), [0x0B, 0x17, 0x2A], "the title card is navy");
        let wide = GifOptions { max_width: 5000, ..GifOptions::default() };
        assert_eq!(decode(&export_gif(&p, &wide, &BlockText).files[0].bytes).width, 1920);
    }

    #[test]
    fn the_allowed_delays() {
        for (asked, wanted) in
            [(1, 100), (2, 200), (3, 300), (5, 500), (8, 800), (0, 100), (4, 300), (7, 800), (100, 800)]
        {
            assert_eq!(GifOptions { seconds_per_step: asked, ..GifOptions::default() }.delay_cs(), wanted, "{asked}");
        }
    }

    #[test]
    fn shorter_pictures_are_centred_on_the_background_colour() {
        let p = demo_plan();
        let options = GifOptions { background: Rgb(10, 200, 30), ..GifOptions::default() };
        let d = decode(&export_gif(&p, &options, &BlockText).files[0].bytes);
        let height = d.height as usize;
        // The demo's cards (blocks and empty steps) are 16:9 and its pictures are 16:9 too, so make a taller guide.
        assert!(height >= 450);
        let mut tall = demo();
        tall.document.guide.steps[0].focus = Some(openstage_express_core::Rect::new(0, 0, 400, 600));
        let tall_plan = plan(&tall.document.guide, &tall.pictures, &BlockText, &ExportOptions::default());
        let d = decode(&export_gif(&tall_plan, &options, &BlockText).files[0].bytes);
        assert!(d.height > 450, "the tall picture makes the animation taller: {}", d.height);
        let last = &d.frames[d.frames.len() - 1];
        // A frame with many colours has them reduced, so a flat colour may be a few levels out.
        assert!(
            near(colour_at(last, d.width as usize, 0, 0), [10, 200, 30], 4),
            "a shorter frame has the background above it"
        );
        assert!(near(colour_at(last, d.width as usize, d.width as usize - 1, d.height as usize - 1), [10, 200, 30], 4));
    }

    #[test]
    fn the_progress_bar_grows_along_the_bottom() {
        let p = demo_plan();
        let options = GifOptions { progress_bar: true, ..GifOptions::default() };
        let d = decode(&export_gif(&p, &options, &BlockText).files[0].bytes);
        let (w, h) = (d.width as usize, d.height as usize);
        let filled = |frame: &DecodedFrame| {
            (0..w).filter(|&x| near(colour_at(frame, w, x, h - 1), [0x7D, 0x87, 0xF2], 4)).count()
        };
        let counts: Vec<usize> = d.frames.iter().map(filled).collect();
        assert!(counts.windows(2).all(|c| c[0] < c[1]), "{counts:?}");
        assert_eq!(*counts.last().unwrap(), w, "the last frame is full");
        assert!(near(colour_at(&d.frames[0], w, w - 1, h - 1), [0xD0, 0xD4, 0xE4], 4), "the unfilled part is a track");
    }

    #[test]
    fn a_title_card_frame_starts_the_progress_bar_empty() {
        let p = demo_plan();
        let options = GifOptions { progress_bar: true, title_card: true, ..GifOptions::default() };
        let d = decode(&export_gif(&p, &options, &BlockText).files[0].bytes);
        let (w, h) = (d.width as usize, d.height as usize);
        assert!(near(colour_at(&d.frames[0], w, 0, h - 1), [0xD0, 0xD4, 0xE4], 4));
        assert!(near(colour_at(&d.frames[d.frames.len() - 1], w, w - 1, h - 1), [0x7D, 0x87, 0xF2], 4));
    }

    #[test]
    fn a_guide_with_no_steps_still_makes_a_gif_and_says_so() {
        let guide = openstage_express_core::Guide::new("g", "Nothing yet", "en", "t");
        let p = plan(&guide, &crate::picture::MemoryPictures::default(), &BlockText, &ExportOptions::default());
        let export = export_gif(&p, &GifOptions::default(), &BlockText);
        assert_eq!(decode(&export.files[0].bytes).frames.len(), 1);
        assert!(export.warnings.iter().any(|w| w.contains("no steps")));
    }

    #[test]
    fn steps_without_pictures_become_title_cards() {
        let d = demo();
        let p =
            plan(&d.document.guide, &crate::picture::MemoryPictures::default(), &BlockText, &ExportOptions::default());
        let g = decode(&export_gif(&p, &GifOptions::default(), &BlockText).files[0].bytes);
        assert_eq!(g.frames.len(), p.steps.len());
        assert_eq!(colour_at(&g.frames[0], 800, 3, 3), [0x0B, 0x17, 0x2A]);
    }

    #[test]
    fn the_export_is_the_same_every_time() {
        let p = demo_plan();
        assert_eq!(
            export_gif(&p, &GifOptions::default(), &BlockText),
            export_gif(&p, &GifOptions::default(), &BlockText)
        );
    }
}
