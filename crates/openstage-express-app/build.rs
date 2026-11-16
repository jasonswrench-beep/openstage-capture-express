// SPDX-License-Identifier: Apache-2.0
//! Puts the product icon and the program's name and version into `openstage-capture-express.exe` itself, so that Explorer, the
//! taskbar, the Start menu and the file's Properties show them for the file `OPEN EXPRESS.bat` runs from `target\`, not only for the
//! copies in the zip and the setup file (`tools/set-exe-icon.ps1` still does the same for those, with the same resource numbers).
//!
//! It writes a Windows resource file (`.res`) by hand, from `assets/icon/OpenCapture_Express.ico`, so nothing has to be installed
//! or downloaded: no resource compiler, no extra library. The Microsoft linker takes a `.res` file as it takes any other input.
//! Other targets (and the GNU toolchain, which cannot link a `.res` file) get no resources, and the program still builds.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const RT_VERSION: u16 = 16;
const ENGLISH_US: u16 = 0x0409;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let root = manifest_dir.join("..").join("..");
    let icon = root.join("assets").join("icon").join("OpenCapture_Express.ico");
    let release = root.join("installer").join("release.txt");
    println!("cargo:rerun-if-changed={}", icon.display());
    println!("cargo:rerun-if-changed={}", release.display());
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }
    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("express.res");
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string());
    let label = fs::read_to_string(&release).ok().map(|text| text.trim().to_string()).filter(|t| !t.is_empty());
    let bytes = match build_resources(&icon, &version, label.as_deref().unwrap_or(&version)) {
        Ok(bytes) => bytes,
        Err(why) => {
            // A program without its icon is still a working program, so say so and carry on.
            println!("cargo:warning=The icon and version details were not put into the program: {why}");
            return;
        }
    };
    fs::write(&out, bytes).expect("the resource file can be written into cargo's own output folder");
    println!("cargo:rustc-link-arg-bins={}", out.display());
}

/// The whole `.res` file: its empty first entry, the icon pictures, the icon group and the version details.
fn build_resources(icon: &Path, version: &str, label: &str) -> Result<Vec<u8>, String> {
    let ico = fs::read(icon).map_err(|e| format!("{} could not be read ({e})", icon.display()))?;
    let pictures = read_ico(&ico)?;
    let mut res = Vec::new();
    // A `.res` file starts with an entry that holds nothing, so that a reader can tell it from other kinds of file.
    entry(&mut res, 0, 0, 0, &[]);
    let mut group = vec![0, 0, 1, 0];
    group.extend_from_slice(&(pictures.len() as u16).to_le_bytes());
    for (index, picture) in pictures.iter().enumerate() {
        let id = (index + 1) as u16;
        entry(&mut res, RT_ICON, id, ENGLISH_US, &picture.data);
        group.extend_from_slice(&picture.header);
        group.extend_from_slice(&id.to_le_bytes());
    }
    entry(&mut res, RT_GROUP_ICON, 1, ENGLISH_US, &group);
    entry(&mut res, RT_VERSION, 1, ENGLISH_US, &version_info(version, label));
    Ok(res)
}

struct Picture {
    /// The first 12 bytes of the picture's entry in the icon file (size, colours, planes, bits, byte count), which the group repeats.
    header: [u8; 12],
    data: Vec<u8>,
}

fn read_ico(ico: &[u8]) -> Result<Vec<Picture>, String> {
    if ico.len() < 6 || ico[0..4] != [0, 0, 1, 0] {
        return Err("the icon file is not a Windows icon file".to_string());
    }
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    if count == 0 {
        return Err("the icon file holds no pictures".to_string());
    }
    let mut pictures = Vec::new();
    for index in 0..count {
        let at = 6 + 16 * index;
        let entry = ico.get(at..at + 16).ok_or("the icon file is cut short")?;
        let size = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
        let offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;
        let data =
            ico.get(offset..offset.saturating_add(size)).ok_or("a picture in the icon file runs past its end")?;
        let mut header = [0u8; 12];
        header.copy_from_slice(&entry[0..12]);
        pictures.push(Picture { header, data: data.to_vec() });
    }
    Ok(pictures)
}

/// One resource: its header (sizes, type, name, language) and its data, padded to a multiple of four bytes.
fn entry(out: &mut Vec<u8>, kind: u16, id: u16, language: u16, data: &[u8]) {
    // A type and a name written as numbers: 0xFFFF, then the number. The empty first entry uses the number 0 for both.
    let mut head = Vec::new();
    for number in [kind, id] {
        head.extend_from_slice(&0xFFFFu16.to_le_bytes());
        head.extend_from_slice(&number.to_le_bytes());
    }
    head.extend_from_slice(&0u32.to_le_bytes()); // data version
    let flags: u16 = if kind == 0 { 0 } else { 0x0030 }; // moveable and pure, for a real resource
    head.extend_from_slice(&flags.to_le_bytes());
    head.extend_from_slice(&language.to_le_bytes());
    head.extend_from_slice(&0u32.to_le_bytes()); // version
    head.extend_from_slice(&0u32.to_le_bytes()); // characteristics
    let header_size = 8 + head.len() as u32;
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&header_size.to_le_bytes());
    out.extend_from_slice(&head);
    out.extend_from_slice(data);
    pad4(out);
}

fn pad4(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).collect()
}

/// One block of a version resource: its length, the length of its value, whether the value is text, its key, then the value and the
/// blocks inside it. Every part starts on a four-byte boundary, counted from the start of the whole resource.
fn block(key: &str, value: &[u8], value_len: u16, text: bool, children: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&0u16.to_le_bytes()); // length, filled in below
    body.extend_from_slice(&value_len.to_le_bytes());
    body.extend_from_slice(&(text as u16).to_le_bytes());
    body.extend_from_slice(&utf16(key));
    pad4(&mut body);
    body.extend_from_slice(value);
    for child in children {
        pad4(&mut body);
        body.extend_from_slice(child);
    }
    let length = body.len() as u16;
    body[0..2].copy_from_slice(&length.to_le_bytes());
    body
}

fn text_value(key: &str, value: &str) -> Vec<u8> {
    let words = (value.encode_utf16().count() + 1) as u16;
    block(key, &utf16(value), words, true, &[])
}

/// The program's name, version and rights as Explorer's Properties window shows them.
fn version_info(version: &str, label: &str) -> Vec<u8> {
    let numbers: Vec<u16> = version.split(['.', '-']).take(4).map(|p| p.parse::<u16>().unwrap_or(0)).collect();
    let part = |i: usize| numbers.get(i).copied().unwrap_or(0) as u32;
    let (ms, ls) = (part(0) << 16 | part(1), part(2) << 16 | part(3));
    let mut fixed = Vec::new();
    for word in [0xFEEF_04BDu32, 0x0001_0000, ms, ls, ms, ls, 0x3F, 0, 0x0004_0004, 1, 0, 0, 0] {
        fixed.extend_from_slice(&word.to_le_bytes());
    }
    let strings: Vec<Vec<u8>> = [
        ("CompanyName", "OpenStage"),
        ("FileDescription", "OpenStage Capture Express"),
        ("FileVersion", label),
        ("InternalName", "openstage-capture-express"),
        ("LegalCopyright", "Apache License 2.0"),
        ("OriginalFilename", "openstage-capture-express.exe"),
        ("ProductName", "OpenStage Capture Express"),
        ("ProductVersion", label),
    ]
    .iter()
    .map(|(key, value)| text_value(key, value))
    .collect();
    let table = block("040904B0", &[], 0, true, &strings);
    let string_info = block("StringFileInfo", &[], 0, true, &[table]);
    let translation = block("Translation", &0x04B0_0409u32.to_le_bytes(), 4, false, &[]);
    let var_info = block("VarFileInfo", &[], 0, true, &[translation]);
    block("VS_VERSION_INFO", &fixed, fixed.len() as u16, false, &[string_info, var_info])
}
