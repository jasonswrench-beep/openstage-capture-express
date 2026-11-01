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

/// Why a ZIP file could not be read. The words are for the person.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ZipReadError {
    /// It is not a ZIP file, or it is damaged or cut short.
    #[error("This is not a ZIP file, or it is damaged ({0}).")]
    Damaged(String),
    /// It is a ZIP file Express will not open: something in it is unsafe or beyond what is supported.
    #[error("Express will not open this ZIP file: {0}.")]
    Refused(String),
}

/// How much a ZIP file may hold before it is refused, so that a damaged or hostile file cannot use up the computer's memory or disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadLimits {
    /// The most files.
    pub files: usize,
    /// The most bytes one file may have once unpacked.
    pub file_bytes: u64,
    /// The most bytes all the files may have once unpacked.
    pub total_bytes: u64,
}

impl Default for ReadLimits {
    /// Plenty for a guide with hundreds of pictures, and no more.
    fn default() -> ReadLimits {
        ReadLimits { files: 5_000, file_bytes: 256 << 20, total_bytes: 1 << 30 }
    }
}

fn damaged(why: &str) -> ZipReadError {
    ZipReadError::Damaged(why.to_string())
}

fn refused(why: impl Into<String>) -> ZipReadError {
    ZipReadError::Refused(why.into())
}

/// Bytes at `at`, or an error if the file is too short to have them.
fn slice(data: &[u8], at: usize, len: usize) -> Result<&[u8], ZipReadError> {
    at.checked_add(len).and_then(|end| data.get(at..end)).ok_or_else(|| damaged("it is cut short"))
}

fn read16(data: &[u8], at: usize) -> Result<u16, ZipReadError> {
    Ok(u16::from_le_bytes(slice(data, at, 2)?.try_into().expect("two bytes")))
}

fn read32(data: &[u8], at: usize) -> Result<u32, ZipReadError> {
    Ok(u32::from_le_bytes(slice(data, at, 4)?.try_into().expect("four bytes")))
}

/// Whether a name inside a ZIP file is one Express will unpack: a plain relative path with no `..`, no drive, no backslash, no
/// control character and no empty part.
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 512
        && !name.starts_with('/')
        && !name.contains(['\\', ':', '\0'])
        && !name.chars().any(char::is_control)
        && name.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

/// Read every file in a ZIP file held in memory, in the order of its directory. Folders are skipped. It trusts nothing: every size
/// and offset is checked against the file, a file is unpacked only as far as the size it says it has, every CRC is checked, and a
/// file name that could reach outside the folder it is unpacked into (`..`, a drive, a leading slash, a backslash) refuses the
/// whole file. Encrypted files, ZIP64 and compression other than stored and deflate are refused, and so is anything over `limits`.
pub fn read(data: &[u8], limits: &ReadLimits) -> Result<Vec<(String, Vec<u8>)>, ZipReadError> {
    if data.len() < 22 {
        return Err(damaged("it is too short"));
    }
    // The end-of-directory record is the last 22 bytes unless the file has a comment, which is at most 65,535 bytes long.
    let latest = data.len() - 22;
    let earliest = latest.saturating_sub(0xFFFF);
    let end = (earliest..=latest)
        .rev()
        .find(|&at| {
            data[at..at + 4] == END_OF_DIRECTORY.to_le_bytes()
                && read16(data, at + 20).is_ok_and(|comment| at + 22 + comment as usize == data.len())
        })
        .ok_or_else(|| damaged("it has no directory of files"))?;
    let (this_disk, directory_disk) = (read16(data, end + 4)?, read16(data, end + 6)?);
    let (on_disk, count) = (read16(data, end + 8)?, read16(data, end + 10)?);
    let (directory_size, directory_start) = (read32(data, end + 12)?, read32(data, end + 16)?);
    if this_disk != 0 || directory_disk != 0 || on_disk != count {
        return Err(refused("it is in several pieces"));
    }
    if count == 0xFFFF || directory_size == u32::MAX || directory_start == u32::MAX {
        return Err(refused("it uses ZIP64, for very large files"));
    }
    if count as usize > limits.files {
        return Err(refused(format!("it holds more than {} files", limits.files)));
    }
    let (directory_start, directory_size) = (directory_start as usize, directory_size as usize);
    if directory_start.checked_add(directory_size).is_none_or(|stop| stop > end) {
        return Err(damaged("its directory is out of place"));
    }
    let mut at = directory_start;
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut total: u64 = 0;
    for _ in 0..count {
        if read32(data, at)? != CENTRAL_HEADER {
            return Err(damaged("its directory is damaged"));
        }
        let flags = read16(data, at + 8)?;
        let method = read16(data, at + 10)?;
        let crc = read32(data, at + 16)?;
        let compressed = read32(data, at + 20)?;
        let size = read32(data, at + 24)?;
        let (name_len, extra_len, comment_len) =
            (read16(data, at + 28)? as usize, read16(data, at + 30)? as usize, read16(data, at + 32)? as usize);
        let local = read32(data, at + 42)?;
        let name_bytes = slice(data, at + 46, name_len)?;
        at = at + 46 + name_len + extra_len + comment_len;
        let name = std::str::from_utf8(name_bytes).map_err(|_| refused("a file has a name that is not plain text"))?;
        if name.ends_with('/') && size == 0 {
            continue; // a folder
        }
        if !safe_name(name) {
            return Err(refused(format!(
                "it has a file called {name:?}, which could reach outside the folder it is unpacked into"
            )));
        }
        if flags & 0x0001 != 0 || flags & 0x0040 != 0 {
            return Err(refused("it has an encrypted file"));
        }
        if compressed == u32::MAX || size == u32::MAX || local == u32::MAX {
            return Err(refused("it uses ZIP64, for very large files"));
        }
        if method != 0 && method != 8 {
            return Err(refused("it uses a kind of compression Express does not read"));
        }
        if u64::from(size) > limits.file_bytes {
            return Err(refused(format!("{name} is larger than a guide's file should be")));
        }
        total += u64::from(size);
        if total > limits.total_bytes {
            return Err(refused("it unpacks to more than a guide should hold"));
        }
        if out.iter().any(|(n, _)| n == name) {
            return Err(refused(format!("it has {name} in it twice")));
        }
        // The file's own header must agree with the directory about the name, and its body must lie before the directory.
        let local = local as usize;
        if read32(data, local)? != LOCAL_HEADER {
            return Err(damaged("a file's header is damaged"));
        }
        let (local_name_len, local_extra_len) =
            (read16(data, local + 26)? as usize, read16(data, local + 28)? as usize);
        if slice(data, local + 30, local_name_len)? != name_bytes {
            return Err(damaged("a file's header does not match the directory"));
        }
        let body_start = local + 30 + local_name_len + local_extra_len;
        let body = slice(data, body_start, compressed as usize)?;
        if body_start + compressed as usize > directory_start {
            return Err(damaged("a file runs into the directory"));
        }
        let bytes = match method {
            0 => {
                if compressed != size {
                    return Err(damaged("a stored file's sizes do not match"));
                }
                body.to_vec()
            }
            _ => {
                use std::io::Read;
                // Read no more than the size the file says it has, plus one byte to notice if there is more.
                let mut bytes = Vec::with_capacity((size as usize).min(1 << 20));
                flate2::read::DeflateDecoder::new(body)
                    .take(u64::from(size) + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| damaged("a file could not be unpacked"))?;
                bytes
            }
        };
        if bytes.len() != size as usize {
            return Err(damaged("a file is not the size it says it is"));
        }
        if crc32fast::hash(&bytes) != crc {
            return Err(damaged("a file's check number does not match, so it was changed or damaged"));
        }
        out.push((name.to_string(), bytes));
    }
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

    // ---- the reader, which has to cope with files from anywhere

    /// One file of a hand-made ZIP, which can be written the way other programs write them (with the sizes after the data).
    struct Hand<'a> {
        name: &'a str,
        data: &'a [u8],
        descriptor: bool,
    }

    /// A ZIP file written by hand, in the style of other programs (a folder entry, sizes in a data descriptor, a comment), so the
    /// reader is tested on more than what this crate's own writer makes.
    fn hand_made(entries: &[Hand<'_>], comment: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for e in entries {
            let offset = out.len() as u32;
            let crc = crc32fast::hash(e.data);
            let len = e.data.len() as u32;
            let flags: u16 = if e.descriptor { 0x0008 } else { 0 };
            le32(&mut out, LOCAL_HEADER);
            le16(&mut out, 20);
            le16(&mut out, flags);
            le16(&mut out, 0);
            le16(&mut out, 0);
            le16(&mut out, 0x2c21);
            if e.descriptor {
                (0..3).for_each(|_| le32(&mut out, 0));
            } else {
                le32(&mut out, crc);
                le32(&mut out, len);
                le32(&mut out, len);
            }
            le16(&mut out, e.name.len() as u16);
            le16(&mut out, 0);
            out.extend_from_slice(e.name.as_bytes());
            out.extend_from_slice(e.data);
            if e.descriptor {
                le32(&mut out, 0x0807_4b50);
                le32(&mut out, crc);
                le32(&mut out, len);
                le32(&mut out, len);
            }
            le32(&mut central, CENTRAL_HEADER);
            le16(&mut central, 20);
            le16(&mut central, 20);
            le16(&mut central, flags);
            le16(&mut central, 0);
            le16(&mut central, 0);
            le16(&mut central, 0x2c21);
            le32(&mut central, crc);
            le32(&mut central, len);
            le32(&mut central, len);
            le16(&mut central, e.name.len() as u16);
            le16(&mut central, 0);
            le16(&mut central, 0);
            le16(&mut central, 0);
            le16(&mut central, 0);
            le32(&mut central, 0);
            le32(&mut central, offset);
            central.extend_from_slice(e.name.as_bytes());
        }
        let start = out.len() as u32;
        out.extend_from_slice(&central);
        le32(&mut out, END_OF_DIRECTORY);
        le16(&mut out, 0);
        le16(&mut out, 0);
        le16(&mut out, entries.len() as u16);
        le16(&mut out, entries.len() as u16);
        le32(&mut out, central.len() as u32);
        le32(&mut out, start);
        le16(&mut out, comment.len() as u16);
        out.extend_from_slice(comment);
        out
    }

    /// `data` with every `from` changed to `to` (the same length, so every offset stays right).
    fn swapped(data: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
        assert_eq!(from.len(), to.len());
        let mut out = data.to_vec();
        let mut at = 0;
        while at + from.len() <= out.len() {
            if &out[at..at + from.len()] == from {
                out[at..at + to.len()].copy_from_slice(to);
                at += from.len();
            } else {
                at += 1;
            }
        }
        out
    }

    fn sample() -> Vec<ZipEntry> {
        vec![
            ZipEntry::text("guide.json", "{\"a\": ".to_string() + &"1, ".repeat(400) + "0}"),
            ZipEntry::stored("images/step-0001.png", (0..=255u8).cycle().take(4000).collect()),
            ZipEntry::text("empty.txt", ""),
        ]
    }

    #[test]
    fn what_the_writer_made_is_read_back_exactly_in_the_same_order() {
        let entries = sample();
        let read = read(&build(&entries).unwrap(), &ReadLimits::default()).unwrap();
        let expected: Vec<(String, Vec<u8>)> = entries.iter().map(|e| (e.name.clone(), e.bytes.clone())).collect();
        assert_eq!(read, expected);
    }

    #[test]
    fn a_file_from_another_program_is_read_with_its_folder_entry_data_descriptors_and_comment() {
        let zip = hand_made(
            &[
                Hand { name: "Guide.osguide/", data: b"", descriptor: false },
                Hand { name: "Guide.osguide/guide.json", data: b"{\"x\":1}", descriptor: true },
                Hand { name: "Guide.osguide/images/a.png", data: &[1, 2, 3, 4], descriptor: false },
            ],
            b"made by another program",
        );
        let got = read(&zip, &ReadLimits::default()).unwrap();
        assert_eq!(got.len(), 2, "the folder is not a file: {:?}", got.iter().map(|g| &g.0).collect::<Vec<_>>());
        assert_eq!(got[0], ("Guide.osguide/guide.json".to_string(), b"{\"x\":1}".to_vec()));
        assert_eq!(got[1].1, [1, 2, 3, 4]);
    }

    #[test]
    fn a_name_that_could_reach_outside_the_folder_it_is_unpacked_into_refuses_the_whole_file() {
        let zip = build(&[ZipEntry::text("ab/cde", "x"), ZipEntry::text("abcdef", "y")]).unwrap();
        // Each is a name of the same length, put in place of one that is fine (in the file's header and in the directory).
        let bad_names: [(&[u8], &[u8]); 7] = [
            (b"ab/cde", b"../cde"),
            (b"abcdef", b"/abcde"),
            (b"abcdef", b"a\\bcde"),
            (b"abcdef", b"C:\\abc"),
            (b"abcdef", b"c:/abc"),
            (b"ab/cde", b"ab//de"),
            (b"ab/cde", b"./.cde"),
        ];
        for (from, to) in bad_names {
            let bad = swapped(&zip, from, to);
            let result = read(&bad, &ReadLimits::default());
            assert!(matches!(result, Err(ZipReadError::Refused(_))), "{:?}: {result:?}", String::from_utf8_lossy(to));
        }
        // A name that only looks odd is fine.
        let fine = swapped(&zip, b"ab/cde", b"ab/..e");
        assert!(read(&fine, &ReadLimits::default()).is_ok());
    }

    #[test]
    fn a_file_that_says_it_is_small_but_unpacks_big_is_stopped_not_unpacked() {
        let zip = build(&[ZipEntry::text("a.bin", "\0".repeat(5_000_000))]).unwrap();
        assert!(zip.len() < 20_000, "zeros pack small: {}", zip.len());
        // Both headers now say the file is 1000 bytes, though it unpacks to five million.
        let size_field = 5_000_000u32.to_le_bytes();
        let lie = swapped(&zip, &size_field, &1000u32.to_le_bytes());
        let result = read(&lie, &ReadLimits::default());
        assert!(matches!(result, Err(ZipReadError::Damaged(_))), "{result:?}");
    }

    #[test]
    fn the_limits_on_files_size_and_total_are_kept() {
        let zip = build(&[ZipEntry::stored("a", vec![1; 600]), ZipEntry::stored("b", vec![2; 600])]).unwrap();
        assert!(read(&zip, &ReadLimits { files: 2, file_bytes: 600, total_bytes: 1200 }).is_ok(), "exactly the limits");
        assert!(matches!(
            read(&zip, &ReadLimits { files: 1, file_bytes: 600, total_bytes: 1200 }),
            Err(ZipReadError::Refused(_))
        ));
        assert!(matches!(
            read(&zip, &ReadLimits { files: 2, file_bytes: 599, total_bytes: 1200 }),
            Err(ZipReadError::Refused(_))
        ));
        assert!(matches!(
            read(&zip, &ReadLimits { files: 2, file_bytes: 600, total_bytes: 1199 }),
            Err(ZipReadError::Refused(_))
        ));
    }

    #[test]
    fn encryption_zip64_other_compression_and_several_pieces_are_refused() {
        let zip = build(&[ZipEntry::stored("abc", vec![9; 50])]).unwrap();
        let central = zip.windows(4).position(|w| w == CENTRAL_HEADER.to_le_bytes()).unwrap();
        let end = zip.len() - 22;
        let patched = |at: usize, bytes: &[u8]| {
            let mut z = zip.clone();
            z[at..at + bytes.len()].copy_from_slice(bytes);
            z
        };
        let encrypted = patched(central + 8, &1u16.to_le_bytes());
        let method = patched(central + 10, &9u16.to_le_bytes());
        let sentinel = patched(central + 24, &u32::MAX.to_le_bytes());
        let many_disks = patched(end + 4, &1u16.to_le_bytes());
        let zip64 = patched(end + 10, &0xFFFFu16.to_le_bytes());
        for (what, z) in [
            ("encrypted", encrypted),
            ("method", method),
            ("sentinel", sentinel),
            ("disks", many_disks),
            ("zip64", zip64),
        ] {
            assert!(matches!(read(&z, &ReadLimits::default()), Err(ZipReadError::Refused(_))), "{what}");
        }
    }

    #[test]
    fn a_changed_file_is_noticed_and_so_is_a_name_that_disagrees_with_the_directory() {
        let zip = build(&[ZipEntry::stored("abc", vec![9; 50])]).unwrap();
        let mut changed = zip.clone();
        changed[40] ^= 0xFF; // inside the file's bytes
        assert!(matches!(read(&changed, &ReadLimits::default()), Err(ZipReadError::Damaged(_))));
        // Only the directory's copy of the name is changed.
        let directory_at = zip.windows(4).position(|w| w == CENTRAL_HEADER.to_le_bytes()).unwrap() + 46;
        let mut renamed = zip.clone();
        renamed[directory_at] = b'x';
        assert!(matches!(read(&renamed, &ReadLimits::default()), Err(ZipReadError::Damaged(_))));
    }

    #[test]
    fn two_files_with_the_same_name_are_refused_because_it_is_unclear_which_is_meant() {
        let zip = hand_made(
            &[
                Hand { name: "guide.json", data: b"{}", descriptor: false },
                Hand { name: "guide.json", data: b"[]", descriptor: false },
            ],
            b"",
        );
        assert!(matches!(read(&zip, &ReadLimits::default()), Err(ZipReadError::Refused(_))));
    }

    #[test]
    fn nothing_that_is_cut_short_or_damaged_ever_panics() {
        let zip = build(&sample()).unwrap();
        let short = ZipEntry::stored("a", vec![5; 30]);
        let small = build(&[short]).unwrap();
        for z in [&zip, &small] {
            for cut in 0..z.len() {
                assert!(read(&z[..cut], &ReadLimits::default()).is_err(), "cut at {cut}");
            }
        }
        // Every byte of a small file changed in turn, and a run of changes by a fixed pseudo-random sequence.
        for at in 0..small.len() {
            for change in [0x00u8, 0xFF, 0x55] {
                let mut z = small.clone();
                z[at] ^= change;
                let _ = read(&z, &ReadLimits::default());
            }
        }
        let mut seed = 12345u32;
        for _ in 0..2000 {
            let mut z = zip.clone();
            for _ in 0..4 {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let at = (seed >> 8) as usize % z.len();
                z[at] = (seed >> 24) as u8;
            }
            let _ = read(&z, &ReadLimits::default());
        }
        assert!(read(b"", &ReadLimits::default()).is_err() && read(b"PK", &ReadLimits::default()).is_err());
        assert!(read(&[0u8; 100], &ReadLimits::default()).is_err());
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
