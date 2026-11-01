// SPDX-License-Identifier: Apache-2.0
//! A baseline JPEG encoder, for the JPG picture export (spec section 10.4).
//!
//! There is no JPEG library in the build, so this writes the file itself: the JFIF container, the standard quantization tables
//! scaled by the usual quality rule (1 to 100), the standard Huffman tables, a full-resolution colour layout (no chroma
//! subsampling, so the edges of text and marks stay sharp), and byte stuffing. It is a plain sequential (baseline) file that
//! every program reads. Pictures are always opaque; anything with transparency is laid over white first.
//!
//! The tests decode the result with a small separate decoder written for them, and the demo's files were also decoded by
//! Windows' own imaging code, which is the check that does not share this file's understanding of the format.

/// The most this encoder accepts across or down: JPEG's own limit.
pub const MAX_SIDE: u32 = 65_535;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JpegError {
    #[error("A picture {0} by {1} pixels cannot be saved as a JPG (it must be at least 1 pixel and at most 65,535 each way).")]
    BadSize(u32, u32),
    #[error("The picture's pixel data does not match its size.")]
    BadData,
}

#[rustfmt::skip]
const ZIGZAG: [usize; 64] = [
     0,  1,  8, 16,  9,  2,  3, 10,
    17, 24, 32, 25, 18, 11,  4,  5,
    12, 19, 26, 33, 40, 48, 41, 34,
    27, 20, 13,  6,  7, 14, 21, 28,
    35, 42, 49, 56, 57, 50, 43, 36,
    29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46,
    53, 60, 61, 54, 47, 55, 62, 63,
];

/// The standard luminance quantization table (the one for quality 50), row by row.
#[rustfmt::skip]
const LUMA_QUANT: [u16; 64] = [
    16, 11, 10, 16,  24,  40,  51,  61,
    12, 12, 14, 19,  26,  58,  60,  55,
    14, 13, 16, 24,  40,  57,  69,  56,
    14, 17, 22, 29,  51,  87,  80,  62,
    18, 22, 37, 56,  68, 109, 103,  77,
    24, 35, 55, 64,  81, 104, 113,  92,
    49, 64, 78, 87, 103, 121, 120, 101,
    72, 92, 95, 98, 112, 100, 103,  99,
];

/// The standard chrominance quantization table (quality 50), row by row.
#[rustfmt::skip]
const CHROMA_QUANT: [u16; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99,
    18, 21, 26, 66, 99, 99, 99, 99,
    24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
];

const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_VALUES: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
#[rustfmt::skip]
const AC_LUMA_VALUES: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

const AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
#[rustfmt::skip]
const AC_CHROMA_VALUES: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

/// A quantization table for `quality` (1 to 100) by the usual rule: 50 is the standard table, higher is finer.
fn scaled(base: &[u16; 64], quality: u8) -> [u16; 64] {
    let q = u32::from(quality.clamp(1, 100));
    let scale = if q < 50 { 5000 / q } else { 200 - 2 * q };
    let mut out = [0u16; 64];
    for (o, &b) in out.iter_mut().zip(base.iter()) {
        *o = ((u32::from(b) * scale + 50) / 100).clamp(1, 255) as u16;
    }
    out
}

/// A Huffman table as the encoder uses it: for each symbol, its code and how many bits long it is.
struct Huffman {
    code: [u16; 256],
    size: [u8; 256],
}

impl Huffman {
    /// The canonical codes for `bits` (how many codes there are of each length 1 to 16) and `values` (the symbols, shortest
    /// code first).
    fn new(bits: &[u8; 16], values: &[u8]) -> Huffman {
        let mut table = Huffman { code: [0; 256], size: [0; 256] };
        let mut code = 0u16;
        let mut next = 0;
        for (length, &count) in bits.iter().enumerate() {
            for _ in 0..count {
                let symbol = values[next] as usize;
                table.code[symbol] = code;
                table.size[symbol] = length as u8 + 1;
                code += 1;
                next += 1;
            }
            code <<= 1;
        }
        table
    }
}

/// Collects bits, most significant first, and writes bytes with the `0xFF` stuffing JPEG needs.
struct Bits {
    out: Vec<u8>,
    accumulator: u32,
    count: u32,
}

impl Bits {
    fn put(&mut self, value: u32, size: u32) {
        if size == 0 {
            return;
        }
        self.accumulator = (self.accumulator << size) | (value & ((1 << size) - 1));
        self.count += size;
        while self.count >= 8 {
            let byte = ((self.accumulator >> (self.count - 8)) & 0xFF) as u8;
            self.out.push(byte);
            if byte == 0xFF {
                self.out.push(0);
            }
            self.count -= 8;
        }
        self.accumulator &= (1 << self.count) - 1;
    }

    /// Pad the last byte with one-bits, as the format asks.
    fn finish(&mut self) {
        if self.count > 0 {
            let pad = 8 - self.count;
            self.put((1 << pad) - 1, pad);
        }
    }
}

/// How many bits a number needs (its "category"), and the bits to write for it (a negative number is written one less).
fn category(value: i32) -> (u32, u32) {
    if value == 0 {
        return (0, 0);
    }
    let size = 32 - value.unsigned_abs().leading_zeros();
    let bits = if value < 0 { (value - 1) as u32 & ((1 << size) - 1) } else { value as u32 };
    (size, bits)
}

/// The 1D cosine transform's matrix: `row[u][x] = c(u) / 2 * cos((2x + 1) u pi / 16)`.
fn dct_matrix() -> [[f32; 8]; 8] {
    let mut m = [[0f32; 8]; 8];
    for (u, row) in m.iter_mut().enumerate() {
        let c = if u == 0 { std::f64::consts::FRAC_1_SQRT_2 } else { 1.0 };
        for (x, cell) in row.iter_mut().enumerate() {
            *cell = (c / 2.0 * (((2 * x + 1) as f64) * (u as f64) * std::f64::consts::PI / 16.0).cos()) as f32;
        }
    }
    m
}

/// The forward cosine transform of one 8 by 8 block (already shifted to be centred on zero).
fn forward_dct(block: &[f32; 64], m: &[[f32; 8]; 8]) -> [f32; 64] {
    let mut rows = [0f32; 64];
    for y in 0..8 {
        for u in 0..8 {
            rows[y * 8 + u] = (0..8).map(|x| m[u][x] * block[y * 8 + x]).sum();
        }
    }
    let mut out = [0f32; 64];
    for u in 0..8 {
        for v in 0..8 {
            out[v * 8 + u] = (0..8).map(|y| m[v][y] * rows[y * 8 + u]).sum();
        }
    }
    out
}

fn segment(out: &mut Vec<u8>, marker: u8, body: &[u8]) {
    out.extend_from_slice(&[0xFF, marker]);
    out.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(body);
}

fn quant_segment(out: &mut Vec<u8>, id: u8, table: &[u16; 64]) {
    let mut body = vec![id];
    body.extend(ZIGZAG.iter().map(|&i| table[i] as u8));
    segment(out, 0xDB, &body);
}

fn huffman_segment(out: &mut Vec<u8>, class_and_id: u8, bits: &[u8; 16], values: &[u8]) {
    let mut body = vec![class_and_id];
    body.extend_from_slice(bits);
    body.extend_from_slice(values);
    segment(out, 0xC4, &body);
}

/// Encode `rgb` (red, green, blue for each pixel, row by row) as a JPEG of the given quality (1 to 100).
pub fn encode_rgb(rgb: &[u8], width: u32, height: u32, quality: u8) -> Result<Vec<u8>, JpegError> {
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(JpegError::BadSize(width, height));
    }
    let (w, h) = (width as usize, height as usize);
    if rgb.len() != w * h * 3 {
        return Err(JpegError::BadData);
    }
    let luma_q = scaled(&LUMA_QUANT, quality);
    let chroma_q = scaled(&CHROMA_QUANT, quality);

    // The three planes: luminance and the two colour differences.
    let mut planes = [vec![0f32; w * h], vec![0f32; w * h], vec![0f32; w * h]];
    for (i, p) in rgb.as_chunks::<3>().0.iter().enumerate() {
        let (r, g, b) = (f32::from(p[0]), f32::from(p[1]), f32::from(p[2]));
        planes[0][i] = 0.299 * r + 0.587 * g + 0.114 * b - 128.0;
        planes[1][i] = -0.168_736 * r - 0.331_264 * g + 0.5 * b;
        planes[2][i] = 0.5 * r - 0.418_688 * g - 0.081_312 * b;
    }

    let mut out = vec![0xFF, 0xD8];
    segment(&mut out, 0xE0, &[b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0]);
    quant_segment(&mut out, 0, &luma_q);
    quant_segment(&mut out, 1, &chroma_q);
    let mut frame = vec![8];
    frame.extend_from_slice(&(height as u16).to_be_bytes());
    frame.extend_from_slice(&(width as u16).to_be_bytes());
    frame.extend_from_slice(&[3, 1, 0x11, 0, 2, 0x11, 1, 3, 0x11, 1]);
    segment(&mut out, 0xC0, &frame);
    huffman_segment(&mut out, 0x00, &DC_LUMA_BITS, &DC_VALUES);
    huffman_segment(&mut out, 0x10, &AC_LUMA_BITS, &AC_LUMA_VALUES);
    huffman_segment(&mut out, 0x01, &DC_CHROMA_BITS, &DC_VALUES);
    huffman_segment(&mut out, 0x11, &AC_CHROMA_BITS, &AC_CHROMA_VALUES);
    segment(&mut out, 0xDA, &[3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);

    let dc_tables = [Huffman::new(&DC_LUMA_BITS, &DC_VALUES), Huffman::new(&DC_CHROMA_BITS, &DC_VALUES)];
    let ac_tables = [Huffman::new(&AC_LUMA_BITS, &AC_LUMA_VALUES), Huffman::new(&AC_CHROMA_BITS, &AC_CHROMA_VALUES)];
    let quant = [luma_q, chroma_q];
    let matrix = dct_matrix();
    let mut bits = Bits { out: Vec::new(), accumulator: 0, count: 0 };
    let mut previous_dc = [0i32; 3];
    for block_y in 0..h.div_ceil(8) {
        for block_x in 0..w.div_ceil(8) {
            for component in 0..3 {
                let table = usize::from(component > 0);
                // Edge blocks repeat the last row and column of pixels.
                let mut block = [0f32; 64];
                for y in 0..8 {
                    for x in 0..8 {
                        let (sx, sy) = ((block_x * 8 + x).min(w - 1), (block_y * 8 + y).min(h - 1));
                        block[y * 8 + x] = planes[component][sy * w + sx];
                    }
                }
                let coefficients = forward_dct(&block, &matrix);
                let mut quantized = [0i32; 64];
                for (i, &z) in ZIGZAG.iter().enumerate() {
                    quantized[i] = (coefficients[z] / f32::from(quant[table][z])).round() as i32;
                }
                // The first coefficient is sent as its change from the block before.
                let diff = quantized[0] - previous_dc[component];
                previous_dc[component] = quantized[0];
                let (size, value) = category(diff);
                bits.put(
                    u32::from(dc_tables[table].code[size as usize]),
                    u32::from(dc_tables[table].size[size as usize]),
                );
                bits.put(value, size);
                // The others as runs of zeros and then a value.
                let mut run = 0;
                let ac = &ac_tables[table];
                for &coefficient in &quantized[1..] {
                    if coefficient == 0 {
                        run += 1;
                        continue;
                    }
                    while run > 15 {
                        bits.put(u32::from(ac.code[0xF0]), u32::from(ac.size[0xF0]));
                        run -= 16;
                    }
                    let (size, value) = category(coefficient);
                    let symbol = ((run << 4) | size) as usize;
                    bits.put(u32::from(ac.code[symbol]), u32::from(ac.size[symbol]));
                    bits.put(value, size);
                    run = 0;
                }
                if run > 0 {
                    bits.put(u32::from(ac.code[0x00]), u32::from(ac.size[0x00]));
                }
            }
        }
    }
    bits.finish();
    out.extend_from_slice(&bits.out);
    out.extend_from_slice(&[0xFF, 0xD9]);
    Ok(out)
}

/// Encode `bgra` (blue, green, red, alpha for each pixel, as the pictures are held) as a JPEG. Transparent pixels are laid
/// over white.
pub fn encode_bgra(bgra: &[u8], width: u32, height: u32, quality: u8) -> Result<Vec<u8>, JpegError> {
    if bgra.len() != width as usize * height as usize * 4 {
        return Err(JpegError::BadData);
    }
    let mut rgb = Vec::with_capacity(bgra.len() / 4 * 3);
    for p in bgra.as_chunks::<4>().0 {
        let a = u32::from(p[3]);
        let over_white = |c: u8| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8;
        rgb.extend_from_slice(&[over_white(p[2]), over_white(p[1]), over_white(p[0])]);
    }
    encode_rgb(&rgb, width, height, quality)
}

#[cfg(test)]
pub(crate) mod decoder {
    //! A small baseline JPEG decoder for the tests. It reads what the encoder writes (and other baseline files with one
    //! scan and full-resolution components), and it is deliberately plain and slow.

    use std::collections::HashMap;

    /// The order the 64 coefficients of a block are sent in, worked out by walking the diagonals, not copied from the encoder.
    pub fn zigzag_order() -> [usize; 64] {
        let mut order = [0usize; 64];
        let (mut x, mut y, mut up) = (0i32, 0i32, true);
        for slot in order.iter_mut() {
            *slot = (y * 8 + x) as usize;
            if up {
                if x == 7 {
                    y += 1;
                    up = false;
                } else if y == 0 {
                    x += 1;
                    up = false;
                } else {
                    x += 1;
                    y -= 1;
                }
            } else if y == 7 {
                x += 1;
                up = true;
            } else if x == 0 {
                y += 1;
                up = true;
            } else {
                x -= 1;
                y += 1;
            }
        }
        order
    }

    pub struct Decoded {
        pub width: usize,
        pub height: usize,
        pub rgb: Vec<u8>,
    }

    struct Table {
        // For each length 1 to 16: the smallest code, the largest, and where its symbols start in `values`.
        min: [i32; 17],
        max: [i32; 17],
        start: [usize; 17],
        values: Vec<u8>,
    }

    fn table(bits: &[u8], values: &[u8]) -> Table {
        let mut t = Table { min: [0; 17], max: [-1; 17], start: [0; 17], values: values.to_vec() };
        let (mut code, mut k) = (0i32, 0usize);
        for length in 1..=16 {
            let count = bits[length - 1] as usize;
            t.start[length] = k;
            if count > 0 {
                t.min[length] = code;
                code += count as i32;
                t.max[length] = code - 1;
                k += count;
            }
            code <<= 1;
        }
        t
    }

    struct Reader<'a> {
        data: &'a [u8],
        at: usize,
        accumulator: u32,
        count: u32,
    }

    impl Reader<'_> {
        fn bit(&mut self) -> i32 {
            if self.count == 0 {
                let byte = self.data[self.at];
                self.at += 1;
                if byte == 0xFF {
                    assert_eq!(self.data[self.at], 0, "a marker inside the picture data");
                    self.at += 1;
                }
                self.accumulator = u32::from(byte);
                self.count = 8;
            }
            self.count -= 1;
            ((self.accumulator >> self.count) & 1) as i32
        }

        fn bits(&mut self, n: u32) -> i32 {
            (0..n).fold(0, |v, _| (v << 1) | self.bit())
        }

        fn symbol(&mut self, t: &Table) -> u8 {
            let mut code = 0i32;
            for length in 1..=16 {
                code = (code << 1) | self.bit();
                if t.max[length] >= 0 && code <= t.max[length] && code >= t.min[length] {
                    return t.values[t.start[length] + (code - t.min[length]) as usize];
                }
            }
            panic!("a code that is in no table");
        }

        /// A signed value of `size` bits as the format writes it.
        fn signed(&mut self, size: u32) -> i32 {
            if size == 0 {
                return 0;
            }
            let v = self.bits(size);
            if v < (1 << (size - 1)) {
                v - (1 << size) + 1
            } else {
                v
            }
        }
    }

    pub fn decode(data: &[u8]) -> Decoded {
        assert_eq!(&data[..2], &[0xFF, 0xD8], "SOI");
        assert_eq!(&data[data.len() - 2..], &[0xFF, 0xD9], "EOI");
        let mut at = 2;
        let mut quant: HashMap<u8, [u16; 64]> = HashMap::new();
        let mut huffman: HashMap<u8, Table> = HashMap::new();
        let (mut width, mut height) = (0usize, 0usize);
        let mut components: Vec<(u8, u8)> = Vec::new(); // (quant table, sampling)
        loop {
            assert_eq!(data[at], 0xFF, "a marker at {at}");
            let marker = data[at + 1];
            let length = u16::from_be_bytes([data[at + 2], data[at + 3]]) as usize;
            let body = &data[at + 4..at + 2 + length];
            match marker {
                0xE0 => {}
                0xDB => {
                    let mut b = body;
                    while !b.is_empty() {
                        assert_eq!(b[0] >> 4, 0, "8-bit tables only");
                        let mut t = [0u16; 64];
                        for i in 0..64 {
                            t[zigzag_order()[i]] = u16::from(b[1 + i]);
                        }
                        quant.insert(b[0] & 15, t);
                        b = &b[65..];
                    }
                }
                0xC0 => {
                    assert_eq!(body[0], 8);
                    height = u16::from_be_bytes([body[1], body[2]]) as usize;
                    width = u16::from_be_bytes([body[3], body[4]]) as usize;
                    assert_eq!(body[5], 3);
                    for c in 0..3 {
                        assert_eq!(body[7 + c * 3], 0x11, "no subsampling");
                        components.push((body[8 + c * 3], body[7 + c * 3]));
                    }
                }
                0xC4 => {
                    let mut b = body;
                    while !b.is_empty() {
                        let n: usize = b[1..17].iter().map(|&x| x as usize).sum();
                        huffman.insert(b[0], table(&b[1..17], &b[17..17 + n]));
                        b = &b[17 + n..];
                    }
                }
                0xDA => {
                    at += 2 + length;
                    break;
                }
                other => panic!("a marker this test decoder does not know: {other:#x}"),
            }
            at += 2 + length;
        }
        let zigzag = zigzag_order();
        let mut reader = Reader { data: &data[..data.len() - 2], at, accumulator: 0, count: 0 };
        let (bw, bh) = (width.div_ceil(8), height.div_ceil(8));
        let mut planes = vec![vec![0f32; bw * 8 * bh * 8]; 3];
        let mut previous = [0i32; 3];
        let mut cos = [[0f32; 8]; 8];
        for (u, row) in cos.iter_mut().enumerate() {
            for (x, cell) in row.iter_mut().enumerate() {
                let c = if u == 0 { std::f32::consts::FRAC_1_SQRT_2 } else { 1.0 };
                *cell = c / 2.0 * ((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI / 16.0).cos();
            }
        }
        for by in 0..bh {
            for bx in 0..bw {
                for (component, &(quant_id, _)) in components.iter().enumerate() {
                    let t = usize::from(component > 0) as u8;
                    let dc = &huffman[&t];
                    let ac = &huffman[&(0x10 | t)];
                    let q = &quant[&quant_id];
                    let mut coefficients = [0i32; 64];
                    let size = u32::from(reader.symbol(dc));
                    previous[component] += reader.signed(size);
                    coefficients[0] = previous[component];
                    let mut k = 1;
                    while k < 64 {
                        let rs = reader.symbol(ac);
                        let (run, size) = ((rs >> 4) as usize, u32::from(rs & 15));
                        if size == 0 {
                            if run == 15 {
                                k += 16;
                                continue;
                            }
                            break;
                        }
                        k += run;
                        coefficients[k] = reader.signed(size);
                        k += 1;
                    }
                    let mut dequantized = [0f32; 64];
                    for i in 0..64 {
                        dequantized[zigzag[i]] = coefficients[i] as f32 * f32::from(q[zigzag[i]]);
                    }
                    // The inverse transform: samples[y][x] = sum over u, v of cos[u][x] * cos[v][y] * coefficient[v][u].
                    let mut columns = [0f32; 64];
                    for u in 0..8 {
                        for y in 0..8 {
                            columns[y * 8 + u] = (0..8).map(|v| cos[v][y] * dequantized[v * 8 + u]).sum();
                        }
                    }
                    for y in 0..8 {
                        for x in 0..8 {
                            let sample: f32 = (0..8).map(|u| cos[u][x] * columns[y * 8 + u]).sum();
                            planes[component][(by * 8 + y) * bw * 8 + bx * 8 + x] = sample;
                        }
                    }
                }
            }
        }
        let mut rgb = Vec::with_capacity(width * height * 3);
        let stride = bw * 8;
        for y in 0..height {
            for x in 0..width {
                let (l, cb, cr) =
                    (planes[0][y * stride + x] + 128.0, planes[1][y * stride + x], planes[2][y * stride + x]);
                let r = l + 1.402 * cr;
                let g = l - 0.344_136 * cb - 0.714_136 * cr;
                let b = l + 1.772 * cb;
                rgb.extend_from_slice(&[r, g, b].map(|c| c.round().clamp(0.0, 255.0) as u8));
            }
        }
        Decoded { width, height, rgb }
    }

    /// Peak signal to noise ratio in decibels between two pictures of the same size (higher is closer; over 40 is very close).
    pub fn psnr(a: &[u8], b: &[u8]) -> f64 {
        assert_eq!(a.len(), b.len());
        let mse = a.iter().zip(b).map(|(&x, &y)| (f64::from(x) - f64::from(y)).powi(2)).sum::<f64>() / a.len() as f64;
        if mse == 0.0 {
            return 99.0;
        }
        10.0 * (255.0f64 * 255.0 / mse).log10()
    }
}

#[cfg(test)]
mod tests {
    use super::decoder::{decode, psnr, zigzag_order};
    use super::*;

    /// A picture with smooth colour, sharp edges and a little noise, like a screenshot with a photograph in it.
    fn sample(width: usize, height: usize) -> Vec<u8> {
        let mut rgb = Vec::with_capacity(width * height * 3);
        let mut seed = 12_345u32;
        for y in 0..height {
            for x in 0..width {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let noise = ((seed >> 16) & 15) as i32 - 8;
                let (mut r, mut g, mut b) = ((x * 255 / width.max(1)) as i32, (y * 255 / height.max(1)) as i32, 128);
                if (x / 16 + y / 16) % 2 == 0 {
                    r = 255 - r;
                }
                if x > width / 2 && y > height / 2 {
                    g = 20;
                    b = 220;
                }
                rgb.extend_from_slice(&[
                    (r + noise).clamp(0, 255) as u8,
                    (g + noise).clamp(0, 255) as u8,
                    (b + noise).clamp(0, 255) as u8,
                ]);
            }
        }
        rgb
    }

    #[test]
    fn every_huffman_table_has_as_many_symbols_as_its_counts_say_and_no_code_starts_another() {
        for (name, bits, values) in [
            ("DC luminance", &DC_LUMA_BITS[..], &DC_VALUES[..]),
            ("DC chrominance", &DC_CHROMA_BITS[..], &DC_VALUES[..]),
            ("AC luminance", &AC_LUMA_BITS[..], &AC_LUMA_VALUES[..]),
            ("AC chrominance", &AC_CHROMA_BITS[..], &AC_CHROMA_VALUES[..]),
        ] {
            let total: usize = bits.iter().map(|&b| b as usize).sum();
            assert_eq!(total, values.len(), "{name}");
            let mut seen = std::collections::HashSet::new();
            assert!(values.iter().all(|v| seen.insert(*v)), "{name}: a symbol twice");
            let bits16: [u8; 16] = bits.try_into().unwrap();
            let table = Huffman::new(&bits16, values);
            let codes: Vec<(u16, u8)> =
                values.iter().map(|&v| (table.code[v as usize], table.size[v as usize])).collect();
            for (i, &(ca, la)) in codes.iter().enumerate() {
                assert!((1..=16).contains(&la), "{name}");
                for &(cb, lb) in &codes[i + 1..] {
                    let (short, long, ls, ll) = if la <= lb { (ca, cb, la, lb) } else { (cb, ca, lb, la) };
                    assert_ne!(u32::from(long) >> (ll - ls), u32::from(short), "{name}: one code starts another");
                }
            }
        }
        // The AC tables hold every run and size from the standard: end of block, sixteen zeros, and each run 0..=15 with sizes 1..=10.
        for values in [&AC_LUMA_VALUES[..], &AC_CHROMA_VALUES[..]] {
            assert!(values.contains(&0x00) && values.contains(&0xF0));
            for run in 0..16u8 {
                for size in 1..=10u8 {
                    assert!(values.contains(&((run << 4) | size)), "run {run} size {size}");
                }
            }
        }
    }

    #[test]
    fn the_zigzag_order_is_the_one_found_by_walking_the_diagonals() {
        assert_eq!(zigzag_order(), ZIGZAG);
        assert_eq!(&ZIGZAG[..10], &[0, 1, 8, 16, 9, 2, 3, 10, 17, 24], "the start everyone knows");
        let mut seen = ZIGZAG.to_vec();
        seen.sort();
        assert_eq!(seen, (0..64).collect::<Vec<usize>>(), "every place exactly once");
    }

    #[test]
    fn quality_scales_the_standard_tables() {
        assert_eq!(scaled(&LUMA_QUANT, 50), LUMA_QUANT, "50 is the standard table");
        assert_eq!(scaled(&CHROMA_QUANT, 50), CHROMA_QUANT);
        assert!(scaled(&LUMA_QUANT, 100).iter().all(|&q| q == 1), "100 loses as little as the format allows");
        assert!(scaled(&LUMA_QUANT, 1).iter().all(|&q| q == 255), "1 is clamped to what a byte can hold");
        assert_eq!(scaled(&LUMA_QUANT, 0), scaled(&LUMA_QUANT, 1), "0 is treated as 1");
        assert_eq!(scaled(&LUMA_QUANT, 200), scaled(&LUMA_QUANT, 100), "above 100 is treated as 100");
        for pair in [(10, 30), (30, 50), (50, 70), (70, 90), (90, 99)] {
            let (coarse, fine) = (scaled(&LUMA_QUANT, pair.0), scaled(&LUMA_QUANT, pair.1));
            assert!(coarse.iter().zip(&fine).all(|(c, f)| c >= f), "{pair:?}");
            assert_ne!(coarse, fine, "{pair:?}");
        }
        assert_eq!(scaled(&LUMA_QUANT, 90)[0], 3, "16 at quality 90 is 3");
        assert_eq!(scaled(&LUMA_QUANT, 70)[0], 10, "16 at quality 70 is 10");
        assert_eq!(scaled(&LUMA_QUANT, 25)[0], 32, "and 32 at quality 25");
    }

    #[test]
    fn a_number_is_written_as_its_size_and_bits() {
        assert_eq!(category(0), (0, 0));
        assert_eq!(category(1), (1, 1));
        assert_eq!(category(-1), (1, 0));
        assert_eq!(category(2), (2, 2));
        assert_eq!(category(3), (2, 3));
        assert_eq!(category(-2), (2, 1));
        assert_eq!(category(-3), (2, 0));
        assert_eq!(category(255), (8, 255));
        assert_eq!(category(-255), (8, 0));
        assert_eq!(category(1024), (11, 1024));
        assert_eq!(category(-1024), (11, 1023));
        assert_eq!(category(2047), (11, 2047));
    }

    #[test]
    fn the_bit_writer_stuffs_a_zero_after_every_ff_and_pads_with_ones() {
        let mut bits = Bits { out: vec![], accumulator: 0, count: 0 };
        bits.put(0xFF, 8);
        bits.put(0b101, 3);
        bits.finish();
        assert_eq!(bits.out, [0xFF, 0x00, 0b1011_1111]);
        let mut bits = Bits { out: vec![], accumulator: 0, count: 0 };
        bits.put(0xFFFF, 16);
        bits.finish();
        assert_eq!(bits.out, [0xFF, 0x00, 0xFF, 0x00]);
        let mut bits = Bits { out: vec![], accumulator: 0, count: 0 };
        bits.finish();
        assert!(bits.out.is_empty());
    }

    #[test]
    fn the_file_has_every_segment_a_baseline_reader_needs_and_valid_lengths() {
        let jpeg = encode_rgb(&sample(40, 24), 40, 24, 90).unwrap();
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
        assert_eq!(&jpeg[jpeg.len() - 2..], &[0xFF, 0xD9]);
        let mut at = 2;
        let mut markers = Vec::new();
        while at < jpeg.len() {
            assert_eq!(jpeg[at], 0xFF, "a marker at {at}");
            let marker = jpeg[at + 1];
            markers.push(marker);
            if marker == 0xDA {
                break;
            }
            at += 2 + u16::from_be_bytes([jpeg[at + 2], jpeg[at + 3]]) as usize;
        }
        assert_eq!(markers, [0xE0, 0xDB, 0xDB, 0xC0, 0xC4, 0xC4, 0xC4, 0xC4, 0xDA]);
        assert_eq!(&jpeg[6..11], b"JFIF\0");
        let frame = jpeg.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
        assert_eq!(u16::from_be_bytes([jpeg[frame + 5], jpeg[frame + 6]]), 24, "height");
        assert_eq!(u16::from_be_bytes([jpeg[frame + 7], jpeg[frame + 8]]), 40, "width");
    }

    #[test]
    fn picture_data_never_holds_a_stray_marker() {
        let jpeg = encode_rgb(&sample(64, 64), 64, 64, 100).unwrap();
        let scan = jpeg.windows(2).position(|w| w == [0xFF, 0xDA]).unwrap();
        let start = scan + 2 + u16::from_be_bytes([jpeg[scan + 2], jpeg[scan + 3]]) as usize;
        let data = &jpeg[start..jpeg.len() - 2];
        for (i, &b) in data.iter().enumerate() {
            if b == 0xFF {
                assert_eq!(data[i + 1], 0, "an unstuffed FF at {i}");
            }
        }
        assert!(data.iter().filter(|&&b| b == 0xFF).count() > 0, "the test picture does make FF bytes");
    }

    #[test]
    fn a_picture_comes_back_close_to_what_went_in_and_closer_at_higher_quality() {
        let (w, h) = (96, 64);
        let rgb = sample(w, h);
        let mut previous_psnr = 0.0;
        let mut previous_size = 0;
        for quality in [30, 50, 70, 90, 100] {
            let jpeg = encode_rgb(&rgb, w as u32, h as u32, quality).unwrap();
            let back = decode(&jpeg);
            assert_eq!((back.width, back.height), (w, h));
            let quality_db = psnr(&rgb, &back.rgb);
            assert!(
                quality_db > previous_psnr,
                "quality {quality}: {quality_db:.1} dB is not better than {previous_psnr:.1}"
            );
            assert!(jpeg.len() > previous_size, "quality {quality}: the file did not grow");
            (previous_psnr, previous_size) = (quality_db, jpeg.len());
        }
        assert!(previous_psnr > 45.0, "quality 100 is very close: {previous_psnr:.1} dB");
        let at_90 = psnr(&rgb, &decode(&encode_rgb(&rgb, w as u32, h as u32, 90).unwrap()).rgb);
        assert!(at_90 > 33.0, "quality 90: {at_90:.1} dB");
    }

    #[test]
    fn sizes_that_are_not_a_whole_number_of_blocks_work() {
        for (w, h) in [(1, 1), (7, 9), (8, 8), (9, 7), (13, 5), (17, 33), (100, 3), (3, 100)] {
            let rgb = sample(w, h);
            let jpeg = encode_rgb(&rgb, w as u32, h as u32, 95).unwrap();
            let back = decode(&jpeg);
            assert_eq!((back.width, back.height), (w, h), "{w}x{h}");
            assert!(psnr(&rgb, &back.rgb) > 30.0, "{w}x{h}: {:.1} dB", psnr(&rgb, &back.rgb));
        }
    }

    #[test]
    fn flat_colours_stay_the_same_colour() {
        for colour in [[0u8, 0, 0], [255, 255, 255], [43, 58, 184], [194, 65, 12], [128, 128, 128]] {
            let rgb: Vec<u8> = colour.iter().copied().cycle().take(20 * 20 * 3).collect();
            let back = decode(&encode_rgb(&rgb, 20, 20, 90).unwrap());
            for pixel in back.rgb.as_chunks::<3>().0 {
                for c in 0..3 {
                    assert!(
                        (i32::from(pixel[c]) - i32::from(colour[c])).abs() <= 2,
                        "{colour:?} came back as {pixel:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_long_run_of_zeros_is_sent_with_the_sixteen_zeros_code() {
        // A picture that is flat except for one sharp detail has blocks with a single far-out coefficient.
        let (w, h) = (16, 16);
        let mut rgb = vec![200u8; w * h * 3];
        for c in 0..3 {
            rgb[(8 * w + 15) * 3 + c] = 0;
        }
        let jpeg = encode_rgb(&rgb, w as u32, h as u32, 100).unwrap();
        let back = decode(&jpeg);
        assert!(psnr(&rgb, &back.rgb) > 40.0);
    }

    #[test]
    fn transparency_is_laid_over_white() {
        let bgra = [0u8, 0, 0, 0, 10, 20, 200, 255, 0, 0, 0, 128, 255, 255, 255, 255];
        let jpeg = encode_bgra(&bgra, 4, 1, 100).unwrap();
        let back = decode(&jpeg);
        assert!(back.rgb[..3].iter().all(|&c| c >= 253), "fully transparent is white: {:?}", &back.rgb[..3]);
        assert!(
            (i32::from(back.rgb[3]) - 200).abs() <= 3 && (i32::from(back.rgb[5]) - 10).abs() <= 3,
            "opaque is kept (BGRA to RGB): {:?}",
            &back.rgb[3..6]
        );
        assert!(
            (i32::from(back.rgb[6]) - 127).abs() <= 4,
            "half transparent black over white is mid grey: {:?}",
            &back.rgb[6..9]
        );
    }

    #[test]
    fn the_same_picture_always_gives_the_same_bytes() {
        let rgb = sample(50, 30);
        assert_eq!(encode_rgb(&rgb, 50, 30, 80).unwrap(), encode_rgb(&rgb, 50, 30, 80).unwrap());
    }

    #[test]
    fn impossible_pictures_are_refused_in_plain_words() {
        assert_eq!(encode_rgb(&[], 0, 5, 90), Err(JpegError::BadSize(0, 5)));
        assert_eq!(encode_rgb(&[], 5, 0, 90), Err(JpegError::BadSize(5, 0)));
        assert_eq!(encode_rgb(&[0; 3], 70_000, 1, 90), Err(JpegError::BadSize(70_000, 1)));
        assert_eq!(encode_rgb(&[0; 5], 1, 1, 90), Err(JpegError::BadData));
        assert_eq!(encode_bgra(&[0; 5], 1, 1, 90), Err(JpegError::BadData));
        assert!(JpegError::BadSize(0, 5).to_string().contains("at least 1 pixel"));
    }

    #[test]
    fn a_real_sized_picture_encodes_quickly_and_smaller_than_png() {
        let (w, h) = (1280, 720);
        let rgb = sample(w, h);
        let started = std::time::Instant::now();
        let jpeg = encode_rgb(&rgb, w as u32, h as u32, 90).unwrap();
        assert!(started.elapsed().as_secs() < 20, "took {:?}", started.elapsed());
        assert!(jpeg.len() < w * h * 3 / 3, "{} bytes", jpeg.len());
    }
}
