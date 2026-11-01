// SPDX-License-Identifier: Apache-2.0
//! A small ZIP writer, for the Word and PowerPoint files (which are ZIP files holding XML and pictures).
//!
//! Only what those files need: entries with UTF-8 names, either stored as they are (pictures, which are already compressed) or
//! deflated (the XML), no encryption, no ZIP64 (so a file or the whole archive must stay under 4 GB and 65,535 entries; larger
//! is refused with an error rather than written wrongly). Every entry carries the same fixed date, so the same input always
//! gives the same bytes.

use std::io::Write;

use flate2::write::DeflateEncoder;
use flate2::Compression;

/// The fixed time every entry is given: 2026-01-01 00:00:00 in MS-DOS form.
const DOS_DATE: u16 = (((2026 - 1980) as u16) << 9) | (1 << 5) | 1;
const DOS_TIME: u16 = 0;

const LOCAL_HEADER: u32 = 0x0403_4b50;
const CENTRAL_HEADER: u32 = 0x0201_4b50;
const END_OF_DIRECTORY: u32 = 0x0605_4b50;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ZipError {
    #[error("The file name {0:?} cannot go in a ZIP file.")]
    BadName(String),
    #[error("There is more in this export than a ZIP file without ZIP64 can hold ({0}).")]
    TooBig(&'static str),
    #[error("The file {0:?} is in the package twice.")]
    Duplicate(String),
    #[error("The data could not be compressed: {0}")]
    Compress(String),
}

/// One file to put in the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    pub name: String,
    pub bytes: Vec<u8>,
    /// Deflate the bytes (right for XML); otherwise store them as they are (right for PNG).
    pub compress: bool,
}

impl ZipEntry {
    pub fn text(name: impl Into<String>, text: impl Into<String>) -> ZipEntry {
        ZipEntry { name: name.into(), bytes: text.into().into_bytes(), compress: true }
    }

    pub fn stored(name: impl Into<String>, bytes: Vec<u8>) -> ZipEntry {
        ZipEntry { name: name.into(), bytes, compress: false }
    }
}

fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 0xFFFF
        && !name.starts_with('/')
        && !name.contains(['\\', '\0'])
        && !name.chars().any(char::is_control)
        && name.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

fn le16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn le32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

struct Written {
    name: String,
    method: u16,
    crc: u32,
    compressed: u32,
    size: u32,
    offset: u32,
}

/// The bytes of a ZIP archive holding `entries`, in order.
pub fn build(entries: &[ZipEntry]) -> Result<Vec<u8>, ZipError> {
    if entries.len() > 0xFFFF {
        return Err(ZipError::TooBig("more than 65,535 files"));
    }
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<u8> = Vec::new();
    let mut written: Vec<Written> = Vec::with_capacity(entries.len());
    for entry in entries {
        if !plain_name(&entry.name) {
            return Err(ZipError::BadName(entry.name.clone()));
        }
        if !seen.insert(entry.name.as_str()) {
            return Err(ZipError::Duplicate(entry.name.clone()));
        }
        let size = u32::try_from(entry.bytes.len()).map_err(|_| ZipError::TooBig("a file over 4 GB"))?;
        let crc = crc32fast::hash(&entry.bytes);
        let (method, body): (u16, Vec<u8>) = if entry.compress && !entry.bytes.is_empty() {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::new(6));
            encoder.write_all(&entry.bytes).map_err(|e| ZipError::Compress(e.to_string()))?;
            let deflated = encoder.finish().map_err(|e| ZipError::Compress(e.to_string()))?;
            // Keep the stored form if deflating did not help.
            if deflated.len() < entry.bytes.len() {
                (8, deflated)
            } else {
                (0, entry.bytes.clone())
            }
        } else {
            (0, entry.bytes.clone())
        };
        let compressed = u32::try_from(body.len()).map_err(|_| ZipError::TooBig("a file over 4 GB"))?;
        let offset = u32::try_from(out.len()).map_err(|_| ZipError::TooBig("an archive over 4 GB"))?;
        le32(&mut out, LOCAL_HEADER);
        le16(&mut out, 20); // version needed
        le16(&mut out, 0x0800); // flags: the name is UTF-8
        le16(&mut out, method);
        le16(&mut out, DOS_TIME);
        le16(&mut out, DOS_DATE);
        le32(&mut out, crc);
        le32(&mut out, compressed);
        le32(&mut out, size);
        le16(&mut out, entry.name.len() as u16);
        le16(&mut out, 0); // extra field length
        out.extend_from_slice(entry.name.as_bytes());
        out.extend_from_slice(&body);
        written.push(Written { name: entry.name.clone(), method, crc, compressed, size, offset });
    }
    let directory_start = u32::try_from(out.len()).map_err(|_| ZipError::TooBig("an archive over 4 GB"))?;
    for w in &written {
        le32(&mut out, CENTRAL_HEADER);
        le16(&mut out, 20); // version made by
        le16(&mut out, 20); // version needed
        le16(&mut out, 0x0800);
        le16(&mut out, w.method);
        le16(&mut out, DOS_TIME);
        le16(&mut out, DOS_DATE);
        le32(&mut out, w.crc);
        le32(&mut out, w.compressed);
        le32(&mut out, w.size);
        le16(&mut out, w.name.len() as u16);
        le16(&mut out, 0); // extra
        le16(&mut out, 0); // comment
        le16(&mut out, 0); // disk number
        le16(&mut out, 0); // internal attributes
        le32(&mut out, 0); // external attributes
        le32(&mut out, w.offset);
        out.extend_from_slice(w.name.as_bytes());
    }
    let directory_size =
        u32::try_from(out.len() - directory_start as usize).map_err(|_| ZipError::TooBig("an archive over 4 GB"))?;
    le32(&mut out, END_OF_DIRECTORY);
    le16(&mut out, 0); // this disk
    le16(&mut out, 0); // disk with the directory
    le16(&mut out, written.len() as u16);
    le16(&mut out, written.len() as u16);
    le32(&mut out, directory_size);
    le32(&mut out, directory_start);
    le16(&mut out, 0); // comment length
    u32::try_from(out.len()).map_err(|_| ZipError::TooBig("an archive over 4 GB"))?;
    Ok(out)
}

#[cfg(test)]
pub(crate) mod reader {
    //! A separate, strict reader of the archives [`build`](super::build) writes, used by the tests of every format that is a
    //! ZIP file. It trusts nothing: it checks every signature, the sizes, the CRC of every file, and that the central directory
    //! and the local headers agree.

    use flate2::read::DeflateDecoder;
    use std::io::Read;

    pub struct Unzipped {
        pub names: Vec<String>,
        files: Vec<(String, Vec<u8>)>,
    }

    impl Unzipped {
        pub fn get(&self, name: &str) -> Option<&[u8]> {
            self.files.iter().find(|(n, _)| n == name).map(|(_, b)| b.as_slice())
        }

        pub fn text(&self, name: &str) -> String {
            String::from_utf8(self.get(name).unwrap_or_else(|| panic!("no {name} in {:?}", self.names)).to_vec())
                .unwrap_or_else(|_| panic!("{name} is not UTF-8"))
        }
    }

    fn u16_at(d: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([d[at], d[at + 1]])
    }

    fn u32_at(d: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
    }

    pub fn unzip(data: &[u8]) -> Unzipped {
        assert!(data.len() >= 22, "too short for a ZIP file");
        let end = data.len() - 22;
        assert_eq!(u32_at(data, end), 0x0605_4b50, "end-of-directory signature");
        assert_eq!(u16_at(data, end + 20), 0, "no comment");
        let count = u16_at(data, end + 10) as usize;
        assert_eq!(u16_at(data, end + 8) as usize, count);
        let dir_size = u32_at(data, end + 12) as usize;
        let dir_start = u32_at(data, end + 16) as usize;
        assert_eq!(dir_start + dir_size, end, "the directory ends where the end record begins");
        let mut at = dir_start;
        let mut names = Vec::new();
        let mut files = Vec::new();
        for _ in 0..count {
            assert_eq!(u32_at(data, at), 0x0201_4b50, "central header signature");
            let flags = u16_at(data, at + 8);
            assert_eq!(flags & 0x0800, 0x0800, "UTF-8 names");
            let method = u16_at(data, at + 10);
            let crc = u32_at(data, at + 16);
            let compressed = u32_at(data, at + 20) as usize;
            let size = u32_at(data, at + 24) as usize;
            let name_len = u16_at(data, at + 28) as usize;
            assert_eq!(u16_at(data, at + 30), 0);
            assert_eq!(u16_at(data, at + 32), 0);
            let local = u32_at(data, at + 42) as usize;
            let name = String::from_utf8(data[at + 46..at + 46 + name_len].to_vec()).unwrap();
            at += 46 + name_len;
            assert_eq!(u32_at(data, local), 0x0403_4b50, "local header signature for {name}");
            assert_eq!(u16_at(data, local + 26) as usize, name_len);
            assert_eq!(&data[local + 30..local + 30 + name_len], name.as_bytes(), "local name for {name}");
            assert_eq!(u32_at(data, local + 14), crc, "local crc for {name}");
            assert_eq!(u32_at(data, local + 18) as usize, compressed);
            assert_eq!(u32_at(data, local + 22) as usize, size);
            let body_start = local + 30 + name_len;
            let body = &data[body_start..body_start + compressed];
            let bytes = match method {
                0 => {
                    assert_eq!(compressed, size, "stored file {name}");
                    body.to_vec()
                }
                8 => {
                    let mut bytes = Vec::new();
                    DeflateDecoder::new(body).read_to_end(&mut bytes).unwrap();
                    bytes
                }
                other => panic!("method {other} for {name}"),
            };
            assert_eq!(bytes.len(), size, "size of {name}");
            assert_eq!(crc32fast::hash(&bytes), crc, "crc of {name}");
            names.push(name.clone());
            files.push((name, bytes));
        }
        assert_eq!(at, dir_start + dir_size, "every directory entry was read");
        Unzipped { names, files }
    }
}

#[cfg(test)]
mod tests {
    use super::reader::unzip;
    use super::*;

    #[test]
    fn files_go_in_and_come_out_in_order_with_their_bytes() {
        let xml = "<?xml version=\"1.0\"?><a>".to_string() + &"hello ".repeat(500) + "</a>";
        let png: Vec<u8> = (0..=255u8).cycle().take(3000).collect();
        let zip = build(&[
            ZipEntry::text("[Content_Types].xml", xml.clone()),
            ZipEntry::stored("word/media/image1.png", png.clone()),
            ZipEntry::text("docProps/x\u{e9}.xml", "<b/>"),
        ])
        .unwrap();
        let read = unzip(&zip);
        assert_eq!(read.names, ["[Content_Types].xml", "word/media/image1.png", "docProps/x\u{e9}.xml"]);
        assert_eq!(read.text("[Content_Types].xml"), xml);
        assert_eq!(read.get("word/media/image1.png").unwrap(), png.as_slice());
        assert_eq!(read.text("docProps/x\u{e9}.xml"), "<b/>");
    }

    #[test]
    fn xml_is_deflated_and_pictures_and_tiny_files_are_stored() {
        let big = "<a>".to_string() + &"x".repeat(5000) + "</a>";
        let zip = build(&[
            ZipEntry::text("big.xml", big.clone()),
            ZipEntry::stored("p.png", vec![7; 100]),
            ZipEntry::text("tiny.xml", "<a/>"),
            ZipEntry::text("empty.xml", ""),
        ])
        .unwrap();
        assert!(zip.len() < big.len() / 4, "the XML shrank: {} bytes", zip.len());
        let read = unzip(&zip);
        assert_eq!(read.text("big.xml"), big);
        assert_eq!(read.text("tiny.xml"), "<a/>");
        assert_eq!(read.get("empty.xml").unwrap(), b"");
    }

    #[test]
    fn the_same_input_always_gives_the_same_bytes() {
        let entries =
            [ZipEntry::text("a.xml", "<a>".to_string() + &"y".repeat(900)), ZipEntry::stored("b.png", vec![1, 2, 3])];
        assert_eq!(build(&entries).unwrap(), build(&entries).unwrap());
    }

    #[test]
    fn an_empty_archive_is_valid() {
        let zip = build(&[]).unwrap();
        assert_eq!(zip.len(), 22);
        assert!(unzip(&zip).names.is_empty());
    }

    #[test]
    fn bad_and_repeated_names_are_refused() {
        for bad in ["", "/a", "../a", "a/../b", "a//b", "a\\b", "a\u{0}b", "a\nb", "./a", "a/"] {
            assert_eq!(build(&[ZipEntry::text(bad, "x")]), Err(ZipError::BadName(bad.to_string())), "{bad:?}");
        }
        let twice = build(&[ZipEntry::text("a.xml", "1"), ZipEntry::text("a.xml", "2")]);
        assert_eq!(twice, Err(ZipError::Duplicate("a.xml".to_string())));
    }

    #[test]
    fn the_date_is_fixed_and_valid() {
        let zip = build(&[ZipEntry::text("a.xml", "<a/>")]).unwrap();
        // 2026-01-01 as an MS-DOS date: years since 1980 in the top seven bits, then month, then day.
        assert_eq!(u16::from_le_bytes([zip[12], zip[13]]), (46 << 9) | (1 << 5) | 1);
        assert_eq!(u16::from_le_bytes([zip[10], zip[11]]), 0);
    }

    #[test]
    fn the_file_starts_and_ends_with_the_signatures_readers_look_for() {
        // Only the signatures: that other programs read the file is checked on the real Word and PowerPoint files.
        let zip = build(&[ZipEntry::text("a.xml", "<a/>"), ZipEntry::text("b.xml", "<b/>")]).unwrap();
        assert_eq!(&zip[..4], b"PK\x03\x04");
        assert_eq!(&zip[zip.len() - 22..zip.len() - 18], b"PK\x05\x06");
    }
}
