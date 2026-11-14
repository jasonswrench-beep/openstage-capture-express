// SPDX-License-Identifier: Apache-2.0
//! A guide as one file (spec A8): `Title.osguide.zip`, to send to someone or keep as a copy, and the reading of one back.
//!
//! A copy holds `guide.json`, a short `README.txt` and the pictures in `images/`. The deleted steps are never in it (they are not even
//! given to this code). There are two kinds, and the person chooses:
//!
//! * **Pictures with the marks drawn in** (the default, spec P5): each picture is drawn with its marks on it, blur included, and the
//!   guide has no marks left on those steps. The original pictures are not in the file, so nobody who gets it can undo a blur.
//!   Steps that were skipped, which the person has already kept out of every export, are left out too. Each step gets a picture file
//!   of its own, because two steps that share one picture may have different marks.
//! * **With the original pictures**: the guide exactly as it is (skipped steps and all, marks still marks), with the original picture
//!   files unchanged. Anyone who opens it can move or remove a blur mark and see what it hid.
//!
//! No Windows code here: the text on marks is drawn by the [`TextRenderer`] given.

use std::collections::HashSet;

use openstage_capture_render::TextRenderer;
use openstage_express_core::fsutil::sanitise_folder_name;
use openstage_express_core::model::{is_safe_picture_path, step_number};
use openstage_express_core::render::render_marked;
use openstage_express_core::{read_guide, Guide, StoreError};

use crate::picture::{encode_png, PictureSource};
use crate::zip::{self, ReadLimits, ZipEntry, ZipError, ZipReadError};

/// What a copy's file name ends with.
pub const FILE_ENDING: &str = ".osguide.zip";

const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// What the person chose for a copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CopyOptions {
    /// Put the original pictures in the copy (and leave the marks as marks), instead of pictures with the marks drawn in.
    pub include_originals: bool,
}

/// Why a copy could not be made or opened. The words are for the person.
#[derive(Debug, thiserror::Error)]
pub enum ShareError {
    #[error("The copy could not be put in one file: {0}")]
    Write(#[from] ZipError),
    #[error("{0}")]
    Read(#[from] ZipReadError),
    #[error("This is not a copy of an Express guide: {0}")]
    NotACopy(String),
    #[error("{0}")]
    Guide(#[from] StoreError),
}

/// A copy that was made.
#[derive(Debug)]
pub struct CopyMade {
    /// The file.
    pub zip: Vec<u8>,
    /// What to call it: the guide's title, made safe for a file name, then `.osguide.zip`.
    pub file_name: String,
    /// How many steps are in it.
    pub steps: usize,
    /// How many picture files are in it.
    pub pictures: usize,
    /// How many pictures had their marks drawn in.
    pub flattened: usize,
    /// How many skipped steps were left out (only when the marks are drawn in).
    pub left_out: usize,
    pub warnings: Vec<String>,
}

fn guide_bytes(guide: &Guide) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(guide).expect("guide data always serialises");
    bytes.push(b'\n');
    bytes
}

fn readme(title: &str, options: &CopyOptions) -> String {
    let kind = if options.include_originals {
        "This copy has the ORIGINAL pictures in it, exactly as they were taken, and the marks are still marks you can move or remove.\r\n\
         That includes any blur: take a blur mark away and the picture under it can be seen.\r\n\
         Steps that were skipped are included. Deleted steps are never included."
    } else {
        "In this copy each picture has its marks (blur too) drawn into it, and the original pictures are not in the file, so a blur\r\n\
         cannot be undone. Steps that were skipped, which are left out of exports, are not in the copy. Deleted steps are never included."
    };
    format!(
        "A copy of the guide \"{title}\", made by OpenStage Capture Express.\r\n\r\n\
         To open it: in Express, on Home, choose Open a copy and pick this file. It becomes a guide of your own in your guides\r\n\
         folder, and the file itself is not changed.\r\n\r\n{kind}\r\n"
    )
}

/// Make a copy of `guide` as one file. `pictures` are the guide's pictures; `text` draws the words on text marks and the numbers on
/// step badges when marks are drawn into pictures. Nothing is changed in the guide or its folder.
pub fn make_copy(
    guide: &Guide,
    pictures: &dyn PictureSource,
    text: &dyn TextRenderer,
    options: &CopyOptions,
) -> Result<CopyMade, ShareError> {
    let mut copy = guide.clone();
    // What the JSON export leaves out is left out of a copy made for someone else too (the security review of 2026-11-13): where
    // the person clicked, the name of the control, and the program it was in. The title, the description and the marks say what
    // the step shows; these say more about the person's computer than the guide needs.
    for step in &mut copy.steps {
        step.click = None;
        step.control = None;
        step.program = None;
    }
    let mut warnings = Vec::new();
    let mut left_out = 0;
    if !options.include_originals {
        // A step that is skipped, or inside one that is, is left out of every export, so it is left out of a copy made to share.
        let mut skipped: HashSet<String> = HashSet::new();
        let before = copy.steps.len();
        copy.steps.retain(|step| {
            let out = step.skipped || step.parent.as_ref().is_some_and(|p| skipped.contains(p));
            if out {
                skipped.insert(step.id.clone());
            }
            !out
        });
        left_out = before - copy.steps.len();
    }

    let mut files: Vec<ZipEntry> = Vec::new();
    let mut named: HashSet<String> = HashSet::new();
    let mut flattened = 0;
    for (index, step) in copy.steps.iter_mut().enumerate() {
        let Some(image) = step.image.clone() else { continue };
        let words = if step.title.trim().is_empty() { step.id.clone() } else { step.title.trim().to_string() };
        if options.include_originals {
            if named.contains(&image.file) {
                continue;
            }
            match pictures.png_bytes(&image.file) {
                Some(bytes) if is_safe_picture_path(&image.file) => {
                    named.insert(image.file.clone());
                    files.push(ZipEntry::stored(image.file.clone(), bytes));
                }
                _ => warnings
                    .push(format!("The picture of \"{words}\" could not be found, so the copy has none for it.")),
            }
        } else {
            let Some(original) = pictures.picture(&image.file) else {
                warnings.push(format!("The picture of \"{words}\" could not be found, so the copy has none for it."));
                step.annotations.clear();
                continue;
            };
            let rendered = render_marked(&original, step, text);
            if !rendered.report.text_cut_off.is_empty() || !rendered.report.text_errors.is_empty() {
                warnings.push(format!("Some text on the marks of \"{words}\" did not fit or could not be drawn."));
            }
            // A picture of its own for each step: two steps that shared a picture may have had different marks.
            let number = step_number(&step.id).unwrap_or(index as u32 + 1);
            let mut file = format!("images/step-{number:04}.png");
            let mut again = 2;
            while !named.insert(file.clone()) {
                file = format!("images/step-{number:04}-{again}.png");
                again += 1;
            }
            files.push(ZipEntry::stored(file.clone(), encode_png(&rendered.canvas)));
            step.image.as_mut().expect("the step has a picture").file = file;
            step.annotations.clear();
            flattened += 1;
        }
    }

    let mut entries = vec![
        ZipEntry::text("guide.json", String::from_utf8(guide_bytes(&copy)).expect("JSON is UTF-8")),
        ZipEntry::text("README.txt", readme(&copy.title, options)),
    ];
    let pictures_in = files.len();
    entries.extend(files);
    let zip = zip::build(&entries)?;
    Ok(CopyMade {
        zip,
        file_name: format!("{}{FILE_ENDING}", sanitise_folder_name(&copy.title)),
        steps: copy.steps.len(),
        pictures: pictures_in,
        flattened,
        left_out,
        warnings,
    })
}

/// A copy that was opened: the guide and its picture files, ready to be made into a guide folder.
#[derive(Debug)]
pub struct OpenedCopy {
    pub guide: Guide,
    /// Each picture the guide uses and the copy has, as `images/step-0001.png` and the bytes of the PNG.
    pub pictures: Vec<(String, Vec<u8>)>,
    /// Things the person may want to know: a picture the copy lacks, or something that was not right in the guide.
    pub warnings: Vec<String>,
    /// The format version the guide had, if it was brought up to date.
    pub migrated_from: Option<String>,
}

/// Open a copy made by [`make_copy`] (or a guide folder that was zipped by hand, with or without the folder's own name in front).
/// Nothing is written anywhere. Pictures that are not named by a step, and everything else in the file, are ignored.
pub fn read_copy(bytes: &[u8]) -> Result<OpenedCopy, ShareError> {
    read_copy_with(bytes, &ReadLimits::default())
}

pub fn read_copy_with(bytes: &[u8], limits: &ReadLimits) -> Result<OpenedCopy, ShareError> {
    let files = zip::read(bytes, limits)?;
    // The guide is at the top of the file, or inside the one folder the whole guide was zipped in.
    let mut at_top = files.iter().filter(|(name, _)| name == "guide.json");
    let found = match at_top.next() {
        Some(file) => Some((String::new(), file)),
        None => {
            let inside: Vec<_> = files
                .iter()
                .filter_map(|file| {
                    let prefix = file.0.strip_suffix("guide.json")?;
                    (prefix.ends_with('/') && prefix.matches('/').count() == 1).then(|| (prefix.to_string(), file))
                })
                .collect();
            match inside.len() {
                0 => None,
                1 => inside.into_iter().next(),
                _ => return Err(ShareError::NotACopy("it has more than one guide in it".to_string())),
            }
        }
    };
    let Some((prefix, (_, guide_json))) = found else {
        return Err(ShareError::NotACopy("it has no guide.json file inside".to_string()));
    };
    let (guide, migrated_from) = read_guide(guide_json, "guide.json")?;
    let mut warnings: Vec<String> = guide.problems();
    let mut pictures: Vec<(String, Vec<u8>)> = Vec::new();
    let mut taken: HashSet<&str> = HashSet::new();
    for step in &guide.steps {
        let Some(image) = &step.image else { continue };
        if !taken.insert(image.file.as_str()) || !is_safe_picture_path(&image.file) {
            continue;
        }
        let words = if step.title.trim().is_empty() { step.id.clone() } else { step.title.trim().to_string() };
        match files.iter().find(|(name, _)| *name == format!("{prefix}{}", image.file)) {
            Some((_, png)) if png.len() >= PNG_MAGIC.len() && png[..PNG_MAGIC.len()] == PNG_MAGIC => {
                pictures.push((image.file.clone(), png.clone()));
            }
            Some(_) => warnings
                .push(format!("The picture of \"{words}\" in the copy is not a PNG picture, so it was left out.")),
            None => warnings.push(format!("The copy has no picture for \"{words}\".")),
        }
    }
    Ok(OpenedCopy { guide, pictures, warnings, migrated_from })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo;
    use crate::picture::decode_png;
    use crate::zip::reader::unzip;
    use openstage_capture_render::BlockText;
    use openstage_express_core::model::AnnotationKind;

    const NOW: &str = "2026-11-01T00:00:00Z";

    fn originals() -> CopyOptions {
        CopyOptions { include_originals: true }
    }

    #[test]
    fn a_copy_with_the_originals_holds_the_guide_exactly_and_each_original_picture_unchanged() {
        let d = demo();
        let made = make_copy(&d.document.guide, &d.pictures, &BlockText, &originals()).unwrap();
        assert_eq!(made.file_name, "How to submit an assignment.osguide.zip");
        assert_eq!((made.steps, made.pictures, made.flattened, made.left_out), (7, 6, 0, 0));
        assert!(made.warnings.is_empty(), "{:?}", made.warnings);
        let opened = read_copy(&made.zip).unwrap();
        // The guide exactly, except what a copy for someone else leaves out: where each click was, the control, the program.
        let mut want = d.document.guide.clone();
        for step in &mut want.steps {
            (step.click, step.control, step.program) = (None, None, None);
        }
        assert_eq!(opened.guide, want, "the marks and the skipped step are as they were");
        assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
        assert_eq!(opened.pictures.len(), 6);
        for (file, bytes) in &opened.pictures {
            let got = decode_png(bytes).unwrap();
            let want = d.pictures.picture(file).unwrap();
            assert_eq!((got.width, got.height, got.bgra), (want.width, want.height, want.bgra), "{file}");
        }
        let readme = unzip(&made.zip).text("README.txt");
        assert!(readme.contains("ORIGINAL pictures") && readme.contains("blur"), "{readme}");
    }

    #[test]
    fn a_copy_to_share_has_the_marks_drawn_in_no_marks_left_and_the_skipped_step_gone() {
        let d = demo();
        let made = make_copy(&d.document.guide, &d.pictures, &BlockText, &CopyOptions::default()).unwrap();
        assert_eq!((made.steps, made.pictures, made.flattened, made.left_out), (6, 5, 5, 1));
        let opened = read_copy(&made.zip).unwrap();
        assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
        assert!(opened.guide.steps.iter().all(|s| s.annotations.is_empty() && !s.skipped));
        assert!(opened.guide.steps.iter().all(|s| s.title != "Click \"Debug\""), "the skipped step is not in it");
        // The first step's picture is what the editor shows: the picture with its marks on it.
        let first = &d.document.guide.steps[0];
        let original = d.pictures.picture(&first.image.as_ref().unwrap().file).unwrap();
        let want = render_marked(&original, first, &BlockText).canvas;
        let file = &opened.guide.steps[0].image.as_ref().unwrap().file;
        let got = decode_png(&opened.pictures.iter().find(|(n, _)| n == file).unwrap().1).unwrap();
        assert_eq!(got.bgra, want.bgra);
        assert_ne!(got.bgra, original.bgra, "the marks are on it");
        // The focused crop is kept, and can still be moved by whoever opens it; where the click was is not in a copy for someone
        // else (the security review of 2026-11-13).
        assert_eq!(opened.guide.steps[0].focus, first.focus);
        assert!(first.click.is_some() && opened.guide.steps[0].click.is_none());
        let readme = unzip(&made.zip).text("README.txt");
        assert!(readme.contains("not in the file") && readme.contains("Deleted steps are never included"), "{readme}");
    }

    /// The security review of 2026-11-13: a Type step's text once stayed in the guide in a field of its own, and went out in a shared
    /// copy after the title had been changed. A guide made before then still has it; read, renamed and copied (both kinds), neither
    /// the text nor the program it was typed in is anywhere in the file.
    #[test]
    fn a_renamed_type_step_and_the_program_are_nowhere_in_either_kind_of_copy() {
        const SECRET: &str = "ann.lee@example.org pin 4417";
        let d = demo();
        let mut value = serde_json::to_value(&d.document.guide).unwrap();
        let typed_at = d.document.guide.steps.iter().position(|s| s.title.contains("Week 3 reflection")).expect("the demo's typing step");
        value["steps"][typed_at]["typed"] = serde_json::Value::String(SECRET.to_string());
        value["steps"][typed_at]["title"] = serde_json::Value::String(format!("Type \"{SECRET}\""));
        let old = serde_json::to_vec(&value).unwrap();
        let (guide, _) = read_guide(&old, "guide.json").unwrap();
        let mut doc = openstage_express_core::Document::new(guide);
        let id = doc.guide.steps[typed_at].id.clone();
        doc.set_title(&id, "Type the title of your work", "2026-11-13T10:00:00Z").unwrap();
        doc.guide.steps[typed_at].image_description.clear();
        assert!(doc.guide.steps.iter().any(|s| s.program.as_deref() == Some("browser.exe")), "the guide itself still names it");
        for options in [CopyOptions::default(), originals()] {
            let made = make_copy(&doc.guide, &d.pictures, &BlockText, &options).unwrap();
            let files = unzip(&made.zip);
            for name in &files.names {
                let bytes = String::from_utf8_lossy(files.get(name).unwrap()).to_string();
                for private in [SECRET, "ann.lee", "4417", "browser.exe", "\"typed\"", "\"program\": \"", "\"click\": {"] {
                    assert!(!bytes.contains(private), "{private} is in {name} (originals: {})", options.include_originals);
                }
            }
        }
    }

    #[test]
    fn a_blur_in_a_copy_to_share_cannot_be_undone_because_no_original_of_that_step_is_in_the_file() {
        let d = demo();
        let step = d
            .document
            .guide
            .steps
            .iter()
            .find(|s| s.annotations.iter().any(|a| a.kind == AnnotationKind::Blur))
            .expect("the example has a blur");
        let file = step.image.as_ref().unwrap().file.clone();
        let original = d.pictures.picture(&file).unwrap();
        let blur = step.annotations.iter().find(|a| a.kind == AnnotationKind::Blur).unwrap();
        let (bx, by, bw, bh) = (blur.x, blur.y, blur.width.unwrap(), blur.height.unwrap());
        let made = make_copy(&d.document.guide, &d.pictures, &BlockText, &CopyOptions::default()).unwrap();
        let everything = unzip(&made.zip);
        // No file in the copy is the original, and no picture in it has the original's pixels.
        let original_png = d.pictures.png_bytes(&file).unwrap();
        for name in &everything.names {
            assert_ne!(everything.get(name).unwrap(), original_png.as_slice(), "{name} is the original picture file");
            if name.starts_with("images/") {
                assert_ne!(decode_png(everything.get(name).unwrap()).unwrap().bgra, original.bgra, "{name}");
            }
        }
        // And inside the blurred rectangle the picture really is different.
        let opened = read_copy(&made.zip).unwrap();
        let new_file = &opened.guide.steps.iter().find(|s| s.title == step.title).unwrap().image.as_ref().unwrap().file;
        let flat = decode_png(&opened.pictures.iter().find(|(n, _)| n == new_file).unwrap().1).unwrap();
        let mut differing = 0;
        for y in by..by + bh {
            for x in bx..bx + bw {
                let at = (y as usize * original.width as usize + x as usize) * 4;
                if flat.bgra[at..at + 3] != original.bgra[at..at + 3] {
                    differing += 1;
                }
            }
        }
        assert!(differing > (bw * bh) as usize / 4, "the blurred area is changed: {differing} of {} pixels", bw * bh);
    }

    #[test]
    fn two_steps_that_share_one_picture_get_a_flattened_picture_each_because_their_marks_may_differ() {
        let mut d = demo();
        let copy = d.document.duplicate_step("step-1", NOW).unwrap();
        d.document.guide.steps.iter_mut().find(|s| s.id == copy).unwrap().annotations.clear();
        let made = make_copy(&d.document.guide, &d.pictures, &BlockText, &CopyOptions::default()).unwrap();
        let opened = read_copy(&made.zip).unwrap();
        let a = &opened.guide.steps[0].image.as_ref().unwrap().file;
        let b = &opened.guide.steps[1].image.as_ref().unwrap().file;
        assert_ne!(a, b, "one file each");
        let bytes = |n: &String| opened.pictures.iter().find(|(name, _)| name == n).unwrap().1.clone();
        assert_ne!(bytes(a), bytes(b), "one has marks drawn in and the other has none");
        // With the originals, the one shared file is kept once.
        let full = make_copy(&d.document.guide, &d.pictures, &BlockText, &originals()).unwrap();
        assert_eq!(full.pictures, 6);
    }

    #[test]
    fn a_picture_that_cannot_be_found_is_said_and_the_copy_is_made_without_it() {
        let d = demo();
        let none = crate::picture::MemoryPictures::default();
        for options in [CopyOptions::default(), originals()] {
            let made = make_copy(&d.document.guide, &none, &BlockText, &options).unwrap();
            assert_eq!(made.pictures, 0);
            assert!(made.warnings.iter().any(|w| w.contains("could not be found")), "{:?}", made.warnings);
            let opened = read_copy(&made.zip).unwrap();
            assert!(opened.pictures.is_empty());
            assert!(
                opened.warnings.iter().any(|w| w.starts_with("The copy has no picture for")),
                "{:?}",
                opened.warnings
            );
        }
    }

    #[test]
    fn the_same_guide_always_gives_the_same_copy_and_the_file_name_is_safe() {
        let d = demo();
        let one = make_copy(&d.document.guide, &d.pictures, &BlockText, &CopyOptions::default()).unwrap();
        let two = make_copy(&d.document.guide, &d.pictures, &BlockText, &CopyOptions::default()).unwrap();
        assert_eq!(one.zip, two.zip);
        let mut guide = d.document.guide.clone();
        guide.title = "Sales: Q3/Q4 <draft>?".to_string();
        let made = make_copy(&guide, &d.pictures, &BlockText, &CopyOptions::default()).unwrap();
        assert!(made.file_name.ends_with(FILE_ENDING), "{}", made.file_name);
        assert!(!made.file_name.contains(['/', '\\', ':', '?', '<', '>', '*', '"']), "{}", made.file_name);
    }

    #[test]
    fn a_guide_folder_zipped_by_hand_opens_with_or_without_its_own_folder_name_in_front() {
        let d = demo();
        let made = make_copy(&d.document.guide, &d.pictures, &BlockText, &originals()).unwrap();
        let files = zip::read(&made.zip, &ReadLimits::default()).unwrap();
        let inside = |prefix: &str| {
            let entries: Vec<ZipEntry> =
                files.iter().map(|(n, b)| ZipEntry::stored(format!("{prefix}{n}"), b.clone())).collect();
            zip::build(&entries).unwrap()
        };
        let plain = read_copy(&made.zip).unwrap();
        for prefix in ["Demo.osguide/", "Some folder/"] {
            let opened = read_copy(&inside(prefix)).unwrap();
            assert_eq!(opened.guide, plain.guide);
            assert_eq!(opened.pictures.len(), plain.pictures.len(), "{prefix}");
        }
        assert!(
            matches!(read_copy(&inside("a/b/")), Err(ShareError::NotACopy(_))),
            "two folders deep is not looked in"
        );
        let mut two = files.iter().map(|(n, b)| ZipEntry::stored(format!("one/{n}"), b.clone())).collect::<Vec<_>>();
        two.extend(files.iter().map(|(n, b)| ZipEntry::stored(format!("two/{n}"), b.clone())));
        assert!(matches!(read_copy(&zip::build(&two).unwrap()), Err(ShareError::NotACopy(_))), "which guide?");
    }

    #[test]
    fn what_is_not_a_copy_is_refused_in_words_and_a_newer_guide_is_not_read() {
        assert!(matches!(read_copy(b"this is not a zip file at all, it is only words"), Err(ShareError::Read(_))));
        let nothing = zip::build(&[ZipEntry::text("notes.txt", "hello")]).unwrap();
        assert!(matches!(read_copy(&nothing), Err(ShareError::NotACopy(_))));
        let broken = zip::build(&[ZipEntry::text("guide.json", "{ not json")]).unwrap();
        assert!(matches!(read_copy(&broken), Err(ShareError::Guide(_))));
        let d = demo();
        let made = make_copy(&d.document.guide, &d.pictures, &BlockText, &originals()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(unzip(&made.zip).get("guide.json").unwrap()).unwrap();
        value["schemaVersion"] = "99.0.0".into();
        let newer = zip::build(&[ZipEntry::text("guide.json", value.to_string())]).unwrap();
        let error = read_copy(&newer).unwrap_err();
        assert!(matches!(error, ShareError::Guide(StoreError::TooNew)), "{error}");
        assert!(error.to_string().contains("newer Express"), "{error}");
    }

    #[test]
    fn a_picture_that_is_not_a_png_or_has_a_hostile_name_is_left_out_with_a_warning() {
        let d = demo();
        let made = make_copy(&d.document.guide, &d.pictures, &BlockText, &originals()).unwrap();
        let files = zip::read(&made.zip, &ReadLimits::default()).unwrap();
        let mut entries: Vec<ZipEntry> = files
            .iter()
            .map(|(n, b)| {
                if n == "images/step-0001.png" {
                    ZipEntry::stored(n.clone(), b"GIF89a not a png".to_vec())
                } else {
                    ZipEntry::stored(n.clone(), b.clone())
                }
            })
            .collect();
        let opened = read_copy(&zip::build(&entries).unwrap()).unwrap();
        assert_eq!(opened.pictures.len(), 5);
        assert!(opened.warnings.iter().any(|w| w.contains("not a PNG")), "{:?}", opened.warnings);
        // A guide that names a picture outside its folder gets no picture, and the file is still read.
        let mut value: serde_json::Value = serde_json::from_slice(unzip(&made.zip).get("guide.json").unwrap()).unwrap();
        value["steps"][0]["image"]["file"] = "images/../../evil.png".into();
        entries.retain(|e| e.name != "guide.json");
        entries.push(ZipEntry::text("guide.json", value.to_string()));
        let opened = read_copy(&zip::build(&entries).unwrap()).unwrap();
        assert!(opened.pictures.iter().all(|(n, _)| !n.contains("..")));
    }
}
