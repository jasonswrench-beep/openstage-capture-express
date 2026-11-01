// SPDX-License-Identifier: Apache-2.0
//! A guide on disk: a folder named `<title>.osguide` (spec section 7.1, written down in `docs/guide-format.md`).
//!
//! ```text
//! How to submit an assignment.osguide/
//!   guide.json            the guide, always written atomically and last
//!   images/step-0001.png  the originals, never modified
//!   trash/                the pictures of deleted steps, and trash.json
//!   guide.lock            present while a copy of Express has the guide open
//! ```
//!
//! Crash safety: a picture is written before the JSON that names it, and the JSON is replaced in one rename, so a
//! crash leaves the last complete guide, plus at most one picture nothing uses yet. Nothing is deleted except by
//! [`GuideFolder::empty_trash`], which the person asks for.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::{io_error, StoreError};
use crate::fsutil::{unique_guide_folder, write_atomic};
use crate::migrate::{bring_up_to_date, MIGRATIONS};
use crate::model::{is_safe_picture_path, Guide, SCHEMA_VERSION};
use crate::ops::{Document, Trash};
use crate::session::Session;

const GUIDE_FILE: &str = "guide.json";
const LOCK_FILE: &str = "guide.lock";
const IMAGES_DIR: &str = "images";
const TRASH_DIR: &str = "trash";
const TRASH_FILE: &str = "trash.json";
const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// Whether this copy of Express may change the guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    ReadWrite,
    /// Another running copy of Express has the guide open.
    ReadOnly {
        held_by_pid: u32,
    },
}

/// What opening a guide found.
#[derive(Debug)]
pub struct Opened {
    pub folder: GuideFolder,
    pub doc: Document,
    /// Everything wrong with the guide, in words. The guide opens anyway.
    pub problems: Vec<String>,
    pub access: Access,
    /// True if the last copy of Express to have this guide open did not close it properly.
    pub recovered_after_crash: bool,
    /// The format version the file had, if it was brought up to date. The old file is kept as a copy on first save.
    pub migrated_from: Option<String>,
}

/// What a save found that the person may want to know.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SaveReport {
    /// Pictures the guide names that are not in the folder (neither in `images` nor in `trash`).
    pub missing_pictures: Vec<String>,
}

/// An open guide folder. Holds the lock file while it lives.
#[derive(Debug)]
pub struct GuideFolder {
    root: PathBuf,
    owns_lock: bool,
    read_only: bool,
    backup_before_first_save: Option<String>,
}

fn picture_name(logical: &str) -> Option<&str> {
    if is_safe_picture_path(logical) {
        logical.strip_prefix("images/")
    } else {
        None
    }
}

fn read_json(path: &Path, what: &str) -> Result<Value, StoreError> {
    let bytes = fs::read(path).map_err(io_error(format!("{what} could not be read")))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| StoreError::Damaged { what: what.to_string(), detail: format!("it is not valid JSON ({e})") })
}

fn to_json_bytes<T: serde::Serialize>(value: &T) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("guide data always serialises");
    bytes.push(b'\n');
    bytes
}

impl GuideFolder {
    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Make a new guide folder inside `parent`, named from the guide's title, write the guide into it, and take the lock.
    pub fn create(parent: &Path, doc: &Document) -> Result<GuideFolder, StoreError> {
        let root = unique_guide_folder(parent, &doc.guide.title);
        for dir in [root.clone(), root.join(IMAGES_DIR), root.join(TRASH_DIR)] {
            fs::create_dir_all(&dir).map_err(io_error(format!("The folder {} could not be made", dir.display())))?;
        }
        let mut folder = GuideFolder { root, owns_lock: false, read_only: false, backup_before_first_save: None };
        folder.write_lock()?;
        folder.owns_lock = true;
        folder.save(doc)?;
        Ok(folder)
    }

    /// Open a guide folder. `is_running` says whether a process id belongs to a running copy of Express (the Windows
    /// part supplies the real answer; tests supply a fake). A guide that another copy has open opens read-only.
    pub fn open(root: &Path, is_running: &dyn Fn(u32) -> bool) -> Result<Opened, StoreError> {
        let guide_path = root.join(GUIDE_FILE);
        if !guide_path.is_file() {
            return Err(StoreError::NotAGuide(root.display().to_string()));
        }
        let mut value = read_json(&guide_path, GUIDE_FILE)?;
        let migrated_from = bring_up_to_date(&mut value, SCHEMA_VERSION, MIGRATIONS, GUIDE_FILE)?;
        let guide: Guide = serde_json::from_value(value)
            .map_err(|e| StoreError::Damaged { what: GUIDE_FILE.to_string(), detail: e.to_string() })?;
        let problems = guide.problems();

        let trash_path = root.join(TRASH_DIR).join(TRASH_FILE);
        let trash = if trash_path.is_file() {
            let mut value = read_json(&trash_path, "trash.json")?;
            bring_up_to_date(&mut value, SCHEMA_VERSION, MIGRATIONS, "trash.json")?;
            serde_json::from_value(value)
                .map_err(|e| StoreError::Damaged { what: "trash.json".to_string(), detail: e.to_string() })?
        } else {
            Trash::new()
        };

        let mut folder = GuideFolder {
            root: root.to_path_buf(),
            owns_lock: false,
            read_only: false,
            backup_before_first_save: migrated_from.clone(),
        };
        let (access, recovered_after_crash) = match folder.read_lock() {
            None => {
                folder.write_lock()?;
                (Access::ReadWrite, false)
            }
            Some(pid) if !is_running(pid) => {
                folder.write_lock()?;
                (Access::ReadWrite, true)
            }
            Some(pid) => (Access::ReadOnly { held_by_pid: pid }, false),
        };
        folder.owns_lock = access == Access::ReadWrite;
        folder.read_only = access != Access::ReadWrite;
        Ok(Opened { folder, doc: Document { guide, trash }, problems, access, recovered_after_crash, migrated_from })
    }

    /// The process id in the lock file, if there is a readable one. A lock file nobody can read counts as a stale one
    /// left by a crash (process id 0 is never running).
    fn read_lock(&self) -> Option<u32> {
        let text = fs::read_to_string(self.root.join(LOCK_FILE)).ok()?;
        let pid = serde_json::from_str::<Value>(&text).ok().and_then(|v| v.get("pid")?.as_u64()).unwrap_or(0);
        Some(pid as u32)
    }

    fn write_lock(&self) -> Result<(), StoreError> {
        let text = serde_json::json!({"pid": std::process::id(), "startedAt": crate::fsutil::utc_now_iso()});
        write_atomic(&self.root.join(LOCK_FILE), &to_json_bytes(&text))
            .map_err(io_error("The lock file could not be written"))
    }

    /// Let go of the lock now (it is also let go when the folder is dropped).
    pub fn release(&mut self) {
        if self.owns_lock {
            if self.read_lock() == Some(std::process::id()) {
                let _ = fs::remove_file(self.root.join(LOCK_FILE));
            }
            self.owns_lock = false;
        }
    }

    /// Save the guide: first move pictures to where they belong, then the trash, then `guide.json` last and in one
    /// rename. Refuses if the guide is open read-only.
    pub fn save(&self, doc: &Document) -> Result<SaveReport, StoreError> {
        if self.read_only {
            return Err(StoreError::ReadOnly);
        }
        let guide_path = self.root.join(GUIDE_FILE);
        if let Some(old) = &self.backup_before_first_save {
            let backup = self.root.join(format!("guide.before-{old}.json"));
            if guide_path.is_file() && !backup.exists() {
                fs::copy(&guide_path, &backup).map_err(io_error("A copy of the older guide file could not be made"))?;
            }
        }
        let report = self.file_pictures(doc)?;
        let trash_dir = self.root.join(TRASH_DIR);
        fs::create_dir_all(&trash_dir).map_err(io_error("The trash folder could not be made"))?;
        write_atomic(&trash_dir.join(TRASH_FILE), &to_json_bytes(&doc.trash))
            .map_err(io_error("The trash could not be saved"))?;
        write_atomic(&guide_path, &to_json_bytes(&doc.guide)).map_err(io_error("The guide could not be saved"))?;
        Ok(report)
    }

    /// Put each picture where it belongs: in `images` if a live step uses it, in `trash` if only deleted steps do.
    /// A picture is moved, never copied or deleted, and a name that is not allowed is left alone.
    fn file_pictures(&self, doc: &Document) -> Result<SaveReport, StoreError> {
        let mut report = SaveReport::default();
        let images = self.root.join(IMAGES_DIR);
        let trash = self.root.join(TRASH_DIR);
        fs::create_dir_all(&images).map_err(io_error("The images folder could not be made"))?;
        for logical in doc.live_pictures() {
            let Some(name) = picture_name(&logical) else { continue };
            let (home, away) = (images.join(name), trash.join(name));
            if !home.exists() {
                if away.exists() {
                    fs::rename(&away, &home).map_err(io_error(format!("The picture {name} could not be put back")))?;
                } else {
                    report.missing_pictures.push(logical);
                }
            }
        }
        for logical in doc.trash_only_pictures() {
            let Some(name) = picture_name(&logical) else { continue };
            let (home, away) = (trash.join(name), images.join(name));
            if !home.exists() {
                if away.exists() {
                    fs::rename(&away, &home)
                        .map_err(io_error(format!("The picture {name} could not be moved to the trash")))?;
                } else {
                    report.missing_pictures.push(logical);
                }
            }
        }
        Ok(report)
    }

    /// Where a picture is on disk now (in `images`, or in `trash` if only deleted steps use it). None if the name is
    /// not allowed or the file is missing.
    pub fn picture_path(&self, logical: &str) -> Option<PathBuf> {
        let name = picture_name(logical)?;
        [IMAGES_DIR, TRASH_DIR].iter().map(|dir| self.root.join(dir).join(name)).find(|p| p.is_file())
    }

    /// Store a new PNG picture for a step and return its name (`images/step-0007.png`). An existing picture is never
    /// replaced: a clash gets `-2`, `-3`, ... on the end. Written before the guide that will name it.
    pub fn add_picture(&self, step_number: u32, png: &[u8]) -> Result<String, StoreError> {
        if self.read_only {
            return Err(StoreError::ReadOnly);
        }
        if png.len() < PNG_MAGIC.len() || png[..PNG_MAGIC.len()] != PNG_MAGIC {
            return Err(StoreError::NotAPng);
        }
        let images = self.root.join(IMAGES_DIR);
        fs::create_dir_all(&images).map_err(io_error("The images folder could not be made"))?;
        let taken = |name: &str| images.join(name).exists() || self.root.join(TRASH_DIR).join(name).exists();
        let mut name = format!("step-{step_number:04}.png");
        let mut n = 2;
        while taken(&name) {
            name = format!("step-{step_number:04}-{n}.png");
            n += 1;
        }
        write_atomic(&images.join(&name), png).map_err(io_error(format!("The picture {name} could not be saved")))?;
        Ok(format!("{IMAGES_DIR}/{name}"))
    }

    /// Picture files in the folder that no step, live or deleted, uses. A crash between writing a picture and saving
    /// the guide leaves one. They are listed, never deleted by the program.
    pub fn orphan_pictures(&self, doc: &Document) -> Vec<String> {
        let used: BTreeSet<String> = doc.live_pictures().into_iter().chain(doc.trash_only_pictures()).collect();
        let mut out = Vec::new();
        for dir in [IMAGES_DIR, TRASH_DIR] {
            let Ok(entries) = fs::read_dir(self.root.join(dir)) else { continue };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".png") && !used.contains(&format!("{IMAGES_DIR}/{name}")) {
                    out.push(format!("{dir}/{name}"));
                }
            }
        }
        out.sort();
        out
    }

    /// Empty the trash for good, at the person's request. The guide is saved first (so it no longer names the deleted
    /// steps), then each picture that no remaining step uses is deleted. Returns how many pictures were deleted.
    pub fn empty_trash(&self, session: &mut Session, now: &str) -> Result<usize, StoreError> {
        if self.read_only {
            return Err(StoreError::ReadOnly);
        }
        let removed = session.empty_trash(now);
        self.save(session.doc())?;
        let still_used: BTreeSet<String> = session.doc().live_pictures().into_iter().collect();
        let mut deleted = BTreeSet::new();
        for entry in &removed {
            let Some(image) = &entry.step.image else { continue };
            if still_used.contains(&image.file) || deleted.contains(&image.file) {
                continue;
            }
            if let Some(path) = self.picture_path(&image.file) {
                fs::remove_file(&path).map_err(io_error(format!("The picture {} could not be deleted", image.file)))?;
                deleted.insert(image.file.clone());
            }
        }
        Ok(deleted.len())
    }
}

impl Drop for GuideFolder {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::full_fixture;
    use crate::model::StepImage;
    use crate::ops::tests::{doc, LATER, NOW};
    use crate::ops::{Position, StepDraft};

    /// Write bytes straight to a file (not atomically), to make the half-finished states a crash leaves behind.
    fn write_plain(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        fs::write(path, bytes)
    }

    fn never_running(_: u32) -> bool {
        false
    }

    fn always_running(_: u32) -> bool {
        true
    }

    fn png() -> Vec<u8> {
        let mut bytes = PNG_MAGIC.to_vec();
        bytes.extend_from_slice(b"not really a picture, only the signature matters here");
        bytes
    }

    /// A guide folder with two picture steps, saved and with the pictures stored.
    fn guide_with_pictures(parent: &Path) -> (GuideFolder, Document) {
        let mut d = doc();
        let folder = GuideFolder::create(parent, &d).unwrap();
        for title in ["Click Save", "Click Close"] {
            let number = d.guide.next_step_number;
            let file = folder.add_picture(number, &png()).unwrap();
            d.add_step(StepDraft::with_picture(title, StepImage::new(&file, 640, 360)), Position::End, NOW).unwrap();
        }
        folder.save(&d).unwrap();
        (folder, d)
    }

    #[test]
    fn a_guide_is_created_saved_and_opened_again_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let (folder, d) = guide_with_pictures(dir.path());
        assert!(folder.path().ends_with("Test guide.osguide"));
        assert!(folder.path().join("images/step-0001.png").is_file());
        assert!(folder.path().join("guide.json").is_file());
        let path = folder.path().to_path_buf();
        drop(folder);
        let opened = GuideFolder::open(&path, &never_running).unwrap();
        assert_eq!(opened.doc, d);
        assert!(opened.problems.is_empty());
        assert_eq!(opened.access, Access::ReadWrite);
        assert!(!opened.recovered_after_crash);
        assert_eq!(opened.migrated_from, None);
    }

    #[test]
    fn saving_leaves_no_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let (folder, d) = guide_with_pictures(dir.path());
        folder.save(&d).unwrap();
        let mut names = Vec::new();
        for sub in ["", "images", "trash"] {
            for entry in fs::read_dir(folder.path().join(sub)).unwrap().flatten() {
                names.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
    }

    #[test]
    fn the_lock_file_exists_while_open_and_goes_when_closed() {
        let dir = tempfile::tempdir().unwrap();
        let (folder, _) = guide_with_pictures(dir.path());
        let lock = folder.path().join("guide.lock");
        let text = fs::read_to_string(&lock).unwrap();
        assert!(text.contains(&std::process::id().to_string()) && text.contains("startedAt"), "{text}");
        drop(folder);
        assert!(!lock.exists());
    }

    #[test]
    fn a_second_copy_opens_read_only_and_cannot_save() {
        let dir = tempfile::tempdir().unwrap();
        let (first, d) = guide_with_pictures(dir.path());
        let second = GuideFolder::open(first.path(), &always_running).unwrap();
        assert_eq!(second.access, Access::ReadOnly { held_by_pid: std::process::id() });
        assert!(second.folder.is_read_only());
        assert!(matches!(second.folder.save(&d), Err(StoreError::ReadOnly)));
        assert!(matches!(second.folder.add_picture(9, &png()), Err(StoreError::ReadOnly)));
        drop(second);
        assert!(first.path().join("guide.lock").exists(), "a read-only copy must not remove the other copy's lock");
        assert!(first.save(&d).is_ok());
    }

    #[test]
    fn a_lock_left_by_a_crash_is_taken_over_and_reported() {
        let dir = tempfile::tempdir().unwrap();
        let (folder, _) = guide_with_pictures(dir.path());
        let path = folder.path().to_path_buf();
        std::mem::forget(folder); // a crash: no Drop, so the lock stays
        assert!(path.join("guide.lock").exists());
        let opened = GuideFolder::open(&path, &never_running).unwrap();
        assert_eq!(opened.access, Access::ReadWrite);
        assert!(opened.recovered_after_crash);
        assert!(opened.folder.save(&opened.doc).is_ok());
    }

    #[test]
    fn an_unreadable_lock_counts_as_stale() {
        let dir = tempfile::tempdir().unwrap();
        let (folder, _) = guide_with_pictures(dir.path());
        let path = folder.path().to_path_buf();
        std::mem::forget(folder);
        write_plain(&path.join("guide.lock"), b"garbage").unwrap();
        let opened = GuideFolder::open(&path, &|pid| pid != 0).unwrap();
        assert!(opened.recovered_after_crash && opened.access == Access::ReadWrite);
    }

    #[test]
    fn a_newer_guide_is_refused_without_taking_the_lock() {
        let dir = tempfile::tempdir().unwrap();
        let (folder, _) = guide_with_pictures(dir.path());
        let path = folder.path().to_path_buf();
        drop(folder);
        let mut value: Value = serde_json::from_slice(&fs::read(path.join("guide.json")).unwrap()).unwrap();
        value["schemaVersion"] = Value::String("0.2.0".into());
        fs::write(path.join("guide.json"), serde_json::to_vec(&value).unwrap()).unwrap();
        let error = GuideFolder::open(&path, &never_running).unwrap_err();
        assert!(matches!(error, StoreError::TooNew));
        assert_eq!(error.to_string().split('.').next(), Some("This guide was made by a newer Express"));
        assert!(!path.join("guide.lock").exists());
    }

    #[test]
    fn a_folder_without_a_guide_and_a_damaged_guide_say_so() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("Empty.osguide");
        fs::create_dir(&empty).unwrap();
        assert!(matches!(GuideFolder::open(&empty, &never_running), Err(StoreError::NotAGuide(_))));
        write_plain(&empty.join("guide.json"), b"{ not json").unwrap();
        let error = GuideFolder::open(&empty, &never_running).unwrap_err();
        assert!(matches!(error, StoreError::Damaged { .. }), "{error}");
        assert!(error.to_string().contains("guide.json") && error.to_string().contains("Nothing was changed"));
        write_plain(&empty.join("guide.json"), br#"{"schemaVersion": "0.1.0", "title": "no id"}"#).unwrap();
        assert!(matches!(GuideFolder::open(&empty, &never_running), Err(StoreError::Damaged { .. })));
        assert!(!empty.join("guide.lock").exists());
    }

    #[test]
    fn unknown_fields_survive_open_and_save() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Future.osguide");
        fs::create_dir_all(root.join("images")).unwrap();
        let fixture = full_fixture();
        fs::write(root.join("guide.json"), serde_json::to_vec_pretty(&fixture).unwrap()).unwrap();
        let opened = GuideFolder::open(&root, &never_running).unwrap();
        assert!(opened.problems.is_empty(), "{:?}", opened.problems);
        let report = opened.folder.save(&opened.doc).unwrap();
        assert_eq!(report.missing_pictures, ["images/step-0001.png"], "the fixture names a picture that is not there");
        let saved: Value = serde_json::from_slice(&fs::read(root.join("guide.json")).unwrap()).unwrap();
        assert_eq!(saved, fixture);
    }

    #[test]
    fn an_old_guide_is_backed_up_before_its_first_save() {
        // No real migration exists yet, so make the state by hand: a guide that was brought up to date.
        let dir = tempfile::tempdir().unwrap();
        let (folder, d) = guide_with_pictures(dir.path());
        let path = folder.path().to_path_buf();
        drop(folder);
        let mut opened = GuideFolder::open(&path, &never_running).unwrap();
        opened.folder.backup_before_first_save = Some("0.0.1".into());
        let original = fs::read(path.join("guide.json")).unwrap();
        opened.folder.save(&d).unwrap();
        assert_eq!(fs::read(path.join("guide.before-0.0.1.json")).unwrap(), original);
        opened.folder.save(&d).unwrap();
        assert_eq!(fs::read(path.join("guide.before-0.0.1.json")).unwrap(), original, "the backup is made once");
    }

    #[test]
    fn a_crash_between_the_picture_and_the_guide_leaves_the_last_guide_and_one_orphan() {
        let dir = tempfile::tempdir().unwrap();
        let (folder, d) = guide_with_pictures(dir.path());
        let before = fs::read(folder.path().join("guide.json")).unwrap();
        // a third capture: the picture is written, then the program dies before guide.json is written
        let orphan = folder.add_picture(d.guide.next_step_number, &png()).unwrap();
        let path = folder.path().to_path_buf();
        std::mem::forget(folder);
        let opened = GuideFolder::open(&path, &never_running).unwrap();
        assert_eq!(opened.doc, d, "the guide is exactly the last saved one");
        assert!(opened.problems.is_empty());
        assert_eq!(opened.folder.orphan_pictures(&opened.doc), std::slice::from_ref(&orphan));
        assert!(opened.folder.picture_path(&orphan).is_some(), "the orphan is listed, not deleted");
        assert_eq!(fs::read(path.join("guide.json")).unwrap(), before);
    }

    #[test]
    fn a_crash_while_writing_the_guide_leaves_the_old_guide() {
        let dir = tempfile::tempdir().unwrap();
        let (folder, d) = guide_with_pictures(dir.path());
        let path = folder.path().to_path_buf();
        std::mem::forget(folder);
        // the new bytes reached the temporary file but the rename never happened (and the file is cut short)
        write_plain(&path.join("guide.json.tmp"), b"{ \"schemaVersion\": \"0.1.0\", \"ti").unwrap();
        let opened = GuideFolder::open(&path, &never_running).unwrap();
        assert_eq!(opened.doc, d);
        opened.folder.save(&opened.doc).unwrap();
        assert!(!path.join("guide.json.tmp").exists(), "the next save replaces and removes it");
    }

    #[test]
    fn deleting_a_step_moves_its_picture_to_the_trash_and_restoring_moves_it_back() {
        let dir = tempfile::tempdir().unwrap();
        let (folder, mut d) = guide_with_pictures(dir.path());
        let file = d.guide.step("step-1").unwrap().image.as_ref().unwrap().file.clone();
        d.delete_step("step-1", LATER).unwrap();
        folder.save(&d).unwrap();
        assert!(!folder.path().join("images/step-0001.png").exists());
        assert!(folder.path().join("trash/step-0001.png").is_file());
        assert_eq!(folder.picture_path(&file), Some(folder.path().join("trash/step-0001.png")), "still found");
        assert!(folder.path().join("trash/trash.json").is_file());
        d.restore_step("step-1", LATER).unwrap();
        folder.save(&d).unwrap();
        assert!(folder.path().join("images/step-0001.png").is_file());
        assert!(!folder.path().join("trash/step-0001.png").exists());
    }

    #[test]
    fn a_trashed_step_survives_a_close_and_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let (folder, mut d) = guide_with_pictures(dir.path());
        d.delete_step("step-2", LATER).unwrap();
        folder.save(&d).unwrap();
        let path = folder.path().to_path_buf();
        drop(folder);
        let mut opened = GuideFolder::open(&path, &never_running).unwrap();
        assert_eq!(opened.doc, d);
        assert_eq!(opened.doc.trash.entries.len(), 1);
        opened.doc.restore_step("step-2", LATER).unwrap();
        opened.folder.save(&opened.doc).unwrap();
        assert!(path.join("images/step-0002.png").is_file());
    }

    #[test]
    fn a_picture_shared_by_a_copy_stays_when_one_of_them_is_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let (folder, mut d) = guide_with_pictures(dir.path());
        d.duplicate_step("step-1", NOW).unwrap();
        d.delete_step("step-1", LATER).unwrap();
        folder.save(&d).unwrap();
        assert!(folder.path().join("images/step-0001.png").is_file(), "the copy still needs it");
        assert!(!folder.path().join("trash/step-0001.png").exists());
    }

    #[test]
    fn emptying_the_trash_deletes_only_pictures_nothing_uses() {
        let dir = tempfile::tempdir().unwrap();
        let (folder, d) = guide_with_pictures(dir.path());
        let mut session = Session::new(d);
        session.apply(|d| d.duplicate_step("step-1", NOW).map(|_| ())).unwrap();
        session.apply(|d| d.delete_step("step-1", LATER)).unwrap();
        session.apply(|d| d.delete_step("step-2", LATER)).unwrap();
        folder.save(session.doc()).unwrap();
        assert!(folder.path().join("trash/step-0002.png").is_file());
        let deleted = folder.empty_trash(&mut session, LATER).unwrap();
        assert_eq!(deleted, 1, "only step-0002.png; step-0001.png is still used by the copy");
        assert!(!folder.path().join("trash/step-0002.png").exists());
        assert!(folder.path().join("images/step-0001.png").is_file());
        assert!(session.doc().trash.entries.is_empty());
        assert!(!session.can_undo());
        let saved: Value = serde_json::from_slice(&fs::read(folder.path().join("trash/trash.json")).unwrap()).unwrap();
        assert_eq!(saved["entries"], serde_json::json!([]));
    }

    #[test]
    fn pictures_must_be_pngs_and_never_overwrite_each_other() {
        let dir = tempfile::tempdir().unwrap();
        let d = doc();
        let folder = GuideFolder::create(dir.path(), &d).unwrap();
        assert!(matches!(folder.add_picture(1, b"GIF89a....."), Err(StoreError::NotAPng)));
        assert!(matches!(folder.add_picture(1, b""), Err(StoreError::NotAPng)));
        let first = folder.add_picture(1, &png()).unwrap();
        let mut other = png();
        other.push(b'!');
        let second = folder.add_picture(1, &other).unwrap();
        assert_eq!((first.as_str(), second.as_str()), ("images/step-0001.png", "images/step-0001-2.png"));
        assert_eq!(fs::read(folder.path().join("images/step-0001.png")).unwrap(), png(), "the original is untouched");
    }

    #[test]
    fn a_picture_path_that_climbs_out_of_the_folder_is_never_followed() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("secret.png");
        fs::write(&outside, png()).unwrap();
        let (folder, _) = guide_with_pictures(dir.path());
        for bad in
            ["images/../../secret.png", "../secret.png", "images/..", "C:/secret.png", "images\\..\\..\\secret.png"]
        {
            assert_eq!(folder.picture_path(bad), None, "{bad}");
        }
        let mut d = doc();
        let mut step = crate::model::Step::bare("step-1", crate::model::StepKind::Image, "x");
        step.image = Some(StepImage::new("images/../../secret.png", 1, 1));
        d.guide.steps.push(step);
        d.guide.next_step_number = 2;
        let report = folder.save(&d).unwrap();
        assert!(report.missing_pictures.is_empty(), "a path that is not allowed is skipped, not looked for");
        assert!(outside.is_file());
    }
}
