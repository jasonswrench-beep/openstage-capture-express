// SPDX-License-Identifier: Apache-2.0
//! The guides folder (spec section 7.1): where guides live, and what Home shows of them. A guide is a folder named
//! `Title.osguide`. This module looks at them, and does the three things Home can do to one (spec A1): rename it, move it to the
//! Trash and bring it back. Nothing here ever erases a guide: the Trash is a folder called `Trash` inside the guides folder, and
//! what is in it stays until the person deletes it themselves, in Explorer.

use std::fs;
use std::path::{Path, PathBuf};

use openstage_express_core::fsutil::{unique_guide_folder, utc_now_iso};
use openstage_express_core::{Access, GuideFolder};
use serde_json::Value;

/// The place under Documents where guides are kept.
const UNDER_DOCUMENTS: [&str; 2] = ["OpenStage Express", "Guides"];

/// Setting this variable names another guides folder (for tests and for people who keep guides elsewhere).
pub const FOLDER_VARIABLE: &str = "EXPRESS_GUIDES_FOLDER";

/// Where guides are kept: the folder named by `EXPRESS_GUIDES_FOLDER`, else `Documents\OpenStage Express\Guides`, with Documents
/// being wherever Windows has put it.
pub fn default_folder() -> PathBuf {
    if let Some(chosen) = std::env::var_os(FOLDER_VARIABLE).filter(|v| !v.is_empty()) {
        return PathBuf::from(chosen);
    }
    #[cfg(windows)]
    let documents = openstage_express_windows::system::documents_folder();
    #[cfg(not(windows))]
    let documents = std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Documents"));
    in_documents(documents.unwrap_or_else(|| PathBuf::from(".")))
}

/// The guides folder to use: the one named by `EXPRESS_GUIDES_FOLDER` if that is set, else the one in the settings, else the usual
/// place.
pub fn guides_folder(from_settings: Option<PathBuf>) -> PathBuf {
    if std::env::var_os(FOLDER_VARIABLE).is_some_and(|v| !v.is_empty()) {
        return default_folder();
    }
    from_settings.unwrap_or_else(default_folder)
}

/// The exports folder to use: the one in the settings, else `Exports` beside the guides folder.
pub fn exports_folder(from_settings: Option<PathBuf>, guides: &Path) -> PathBuf {
    from_settings
        .unwrap_or_else(|| guides.parent().map(|p| p.join("Exports")).unwrap_or_else(|| PathBuf::from("Exports")))
}

/// The guides folder inside a Documents folder.
pub fn in_documents(documents: PathBuf) -> PathBuf {
    UNDER_DOCUMENTS.iter().fold(documents, |path, part| path.join(part))
}

/// One guide folder, as much as Home needs to list it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub title: String,
    pub steps: usize,
    /// When it was last changed, as the guide says (an ISO time, so it sorts as text).
    pub updated_at: String,
}

fn read_entry(path: &Path) -> Option<Entry> {
    let bytes = std::fs::read(path.join("guide.json")).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    let title = value.get("title")?.as_str()?.to_string();
    // The number of steps the editor and the exports count (a note or a skipped step is not one), when the file can be read as a
    // guide; otherwise however many step entries it has.
    let steps = serde_json::from_value::<openstage_express_core::Guide>(value.clone())
        .map(|guide| crate::ui::rows::step_count(&guide))
        .unwrap_or_else(|_| value.get("steps").and_then(Value::as_array).map(Vec::len).unwrap_or(0));
    let updated_at = value.get("updatedAt").and_then(Value::as_str).unwrap_or("").to_string();
    Some(Entry { path: path.to_path_buf(), title, steps, updated_at })
}

/// The guides in a folder, newest first. A folder that is missing, unreadable or not a guide is left out; a guide that is
/// damaged is left out here and reported when someone tries to open it.
pub fn scan(folder: &Path) -> Vec<Entry> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut found: Vec<Entry> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("osguide")))
        .filter_map(|p| read_entry(&p))
        .collect();
    found.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then_with(|| a.title.cmp(&b.title)));
    found
}

/// The folder inside the guides folder that guides deleted from Home are moved to. It has no `.osguide` in its name, so the list of
/// guides never shows it.
pub const TRASH_FOLDER: &str = "Trash";

pub fn trash_folder(guides: &Path) -> PathBuf {
    guides.join(TRASH_FOLDER)
}

/// The guides in the Trash, newest first.
pub fn scan_trash(guides: &Path) -> Vec<Entry> {
    scan(&trash_folder(guides))
}

/// The running copy of Express that has this guide open, if there is one. A lock left behind by a copy that is no longer running
/// does not count.
pub fn held_by(guide: &Path, is_running: &dyn Fn(u32) -> bool) -> Option<u32> {
    let text = fs::read_to_string(guide.join("guide.lock")).ok()?;
    let pid = serde_json::from_str::<Value>(&text).ok().and_then(|v| v.get("pid")?.as_u64()).unwrap_or(0) as u32;
    (pid != 0 && is_running(pid)).then_some(pid)
}

fn is_guide_folder(path: &Path) -> bool {
    path.is_dir()
        && path.extension().is_some_and(|x| x.eq_ignore_ascii_case("osguide"))
        && path.join("guide.json").is_file()
}

/// Whether two folders are the same place (looking through `..` and the like where Windows can).
fn same_place(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn busy_words(pid: u32) -> String {
    format!("It is open in another copy of Express (process {pid}). Close it there first.")
}

/// Move a guide from the guides folder to the Trash. Only a guide folder directly inside the guides folder is moved, and only when
/// no other copy of Express has it open. Returns where it is now.
pub fn move_to_trash(guide: &Path, guides: &Path, is_running: &dyn Fn(u32) -> bool) -> Result<PathBuf, String> {
    if !is_guide_folder(guide) {
        return Err("That is not a guide folder, so it was left where it is.".to_string());
    }
    if !guide.parent().is_some_and(|parent| same_place(parent, guides)) {
        return Err("That guide is not in the guides folder, so it was left where it is.".to_string());
    }
    if let Some(pid) = held_by(guide, is_running) {
        return Err(busy_words(pid));
    }
    let trash = trash_folder(guides);
    fs::create_dir_all(&trash).map_err(|e| format!("The Trash folder could not be made: {e}"))?;
    let title = read_entry(guide).map(|e| e.title).unwrap_or_default();
    let target = unique_guide_folder(&trash, &title);
    fs::rename(guide, &target).map_err(|e| format!("The guide could not be moved to the Trash: {e}"))?;
    Ok(target)
}

/// Bring a guide back from the Trash into the guides folder, under a name that is not taken. Returns where it is now.
pub fn restore(guide: &Path, guides: &Path) -> Result<PathBuf, String> {
    if !is_guide_folder(guide) {
        return Err("That is not a guide folder, so it was left where it is.".to_string());
    }
    if !guide.parent().is_some_and(|parent| same_place(parent, &trash_folder(guides))) {
        return Err("That guide is not in the Trash, so it was left where it is.".to_string());
    }
    let title = read_entry(guide).map(|e| e.title).unwrap_or_default();
    let target = unique_guide_folder(guides, &title);
    fs::rename(guide, &target).map_err(|e| format!("The guide could not be brought back: {e}"))?;
    Ok(target)
}

/// Give a guide another title (as the editor's Guide title box does: the folder keeps its name). Refused when another copy of
/// Express has the guide open. Returns the title as kept, without spaces at either end.
pub fn rename_guide(guide: &Path, title: &str, is_running: &dyn Fn(u32) -> bool) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("A guide needs a name. Type one, or cancel.".to_string());
    }
    if let Some(pid) = held_by(guide, is_running) {
        return Err(busy_words(pid));
    }
    let opened = GuideFolder::open(guide, is_running).map_err(|e| format!("The guide could not be opened: {e}"))?;
    if let Access::ReadOnly { held_by_pid } = opened.access {
        return Err(busy_words(held_by_pid));
    }
    let mut doc = opened.doc;
    doc.set_guide_title(title, &utc_now_iso());
    opened.folder.save(&doc).map_err(|e| format!("The new name could not be saved: {e}"))?;
    Ok(title.to_string())
}

/// The most a copy file may be before it is refused without being read (the reader has its own limits on what is inside it).
const MAX_COPY_BYTES: u64 = 1 << 30;

/// What opening a copy made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyOpened {
    /// The new guide folder.
    pub guide: PathBuf,
    pub title: String,
    /// How many steps the guide has (as Home counts them).
    pub steps: usize,
    /// Things the person may want to know, such as a picture the copy lacked.
    pub warnings: Vec<String>,
}

/// Make a new guide of the person's own in `guides` from a copy saved as one file (`.osguide.zip`). The file is only read. The new
/// guide gets a new id and the next free folder name, so it never takes the place of a guide that is already there, even a copy of
/// the same one. Nothing is made unless the whole copy was understood; if something goes wrong while the new folder is being filled,
/// that folder is removed again.
pub fn open_copy(file: &Path, guides: &Path) -> Result<CopyOpened, String> {
    if guides.as_os_str().is_empty() {
        return Err("There is no guides folder to put the copy in.".to_string());
    }
    let meta = fs::metadata(file).map_err(|_| format!("The file {} could not be found or read.", file.display()))?;
    if meta.is_dir() {
        return Err(format!("{} is a folder, not a copy saved as one file.", file.display()));
    }
    if meta.len() > MAX_COPY_BYTES {
        return Err("This file is larger than the copy of a guide should be, so it was not opened.".to_string());
    }
    let bytes = fs::read(file).map_err(|e| format!("The file {} could not be read ({e}).", file.display()))?;
    let mut opened = openstage_express_export::share::read_copy(&bytes).map_err(|e| e.to_string())?;
    let now = utc_now_iso();
    opened.guide.id = openstage_express_core::fsutil::new_guide_id(&now);
    opened.guide.updated_at = now;
    let title = opened.guide.title.clone();
    let steps = crate::ui::rows::step_count(&opened.guide);
    let doc = openstage_express_core::Document::new(opened.guide);
    let folder = GuideFolder::create_with_pictures(guides, &doc, &opened.pictures)
        .map_err(|e| format!("The copy could not be made into a guide: {e}"))?;
    let path = folder.path().to_path_buf();
    drop(folder);
    Ok(CopyOpened { guide: path, title, steps, warnings: opened.warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guide(dir: &Path, folder: &str, title: &str, steps: usize, updated: &str) {
        let root = dir.join(folder);
        std::fs::create_dir_all(&root).unwrap();
        let steps: Vec<Value> = (0..steps).map(|n| serde_json::json!({"id": format!("step-{n}")})).collect();
        let json = serde_json::json!({"title": title, "steps": steps, "updatedAt": updated});
        std::fs::write(root.join("guide.json"), serde_json::to_vec(&json).unwrap()).unwrap();
    }

    #[test]
    fn guides_are_listed_newest_first_with_their_step_counts() {
        let dir = tempfile::tempdir().unwrap();
        guide(dir.path(), "Old.osguide", "Old one", 2, "2026-10-01T10:00:00Z");
        guide(dir.path(), "New.osguide", "New one", 5, "2026-11-01T09:00:00Z");
        guide(dir.path(), "Middle.osguide", "Middle one", 0, "2026-10-20T10:00:00Z");
        let found = scan(dir.path());
        let titles: Vec<_> = found.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, ["New one", "Middle one", "Old one"]);
        assert_eq!(found[0].steps, 5);
        assert_eq!(found[0].path, dir.path().join("New.osguide"));
    }

    #[test]
    fn things_that_are_not_guides_are_left_out() {
        let dir = tempfile::tempdir().unwrap();
        guide(dir.path(), "Real.osguide", "Real", 1, "2026-11-01T09:00:00Z");
        guide(dir.path(), "Not a guide folder", "Wrong name", 1, "2026-11-01T09:00:00Z");
        std::fs::create_dir_all(dir.path().join("Empty.osguide")).unwrap();
        std::fs::create_dir_all(dir.path().join("Damaged.osguide")).unwrap();
        std::fs::write(dir.path().join("Damaged.osguide").join("guide.json"), b"{ not json").unwrap();
        std::fs::write(dir.path().join("loose file.osguide"), b"x").unwrap();
        let found = scan(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Real");
    }

    #[test]
    fn a_missing_folder_has_no_guides() {
        let dir = tempfile::tempdir().unwrap();
        assert!(scan(&dir.path().join("nowhere")).is_empty());
    }

    /// A real guide (the example, with this title) made by Express itself, with its lock let go.
    fn real_guide(parent: &Path, title: &str) -> PathBuf {
        let mut doc = openstage_express_export::demo::demo().document;
        doc.guide.title = title.to_string();
        let folder = GuideFolder::create(parent, &doc).unwrap();
        let path = folder.path().to_path_buf();
        drop(folder);
        path
    }

    /// How many steps the example guide has in its file (the card counts fewer: it leaves out a skipped step and a note).
    fn demo_steps() -> usize {
        openstage_express_export::demo::demo().document.guide.steps.len()
    }

    fn nobody() -> impl Fn(u32) -> bool {
        |_| false
    }

    fn lock_held_by(guide: &Path, pid: u32) {
        std::fs::write(
            guide.join("guide.lock"),
            format!("{{\"pid\": {pid}, \"startedAt\": \"2026-11-01T09:00:00Z\"}}"),
        )
        .unwrap();
    }

    #[test]
    fn a_guide_moved_to_the_trash_is_not_listed_but_is_all_still_there() {
        let dir = tempfile::tempdir().unwrap();
        let keep = real_guide(dir.path(), "Keep me");
        let gone = real_guide(dir.path(), "Delete me");
        let before = std::fs::read(gone.join("guide.json")).unwrap();
        let pictures = std::fs::read_dir(gone.join("images")).unwrap().count();
        let now_in = move_to_trash(&gone, dir.path(), &nobody()).unwrap();
        assert_eq!(now_in, dir.path().join("Trash").join("Delete me.osguide"));
        assert!(!gone.exists(), "it is no longer in the guides folder");
        let titles: Vec<_> = scan(dir.path()).into_iter().map(|e| e.title).collect();
        assert_eq!(titles, ["Keep me"], "the list of guides does not show the Trash, or what is in it");
        assert!(keep.exists());
        let in_trash = scan_trash(dir.path());
        assert_eq!(in_trash.len(), 1);
        assert_eq!((in_trash[0].title.as_str(), in_trash[0].steps), ("Delete me", 5), "counted as the editor counts");
        assert_eq!(std::fs::read(now_in.join("guide.json")).unwrap(), before, "not a byte of it changed");
        assert_eq!(std::fs::read_dir(now_in.join("images")).unwrap().count(), pictures, "the pictures went with it");
    }

    #[test]
    fn a_guide_is_counted_as_the_editor_counts_it_with_no_note_and_no_skipped_step() {
        let dir = tempfile::tempdir().unwrap();
        let guide = real_guide(dir.path(), "Counted");
        assert!(demo_steps() > 5, "the example has a skipped step and a note as well as its five");
        assert_eq!(scan(dir.path())[0].steps, 5);
        // A file that cannot be read as a guide (an older kind, say) still lists, with however many step entries it has.
        let loose = dir.path().join("Loose.osguide");
        std::fs::create_dir_all(&loose).unwrap();
        std::fs::write(loose.join("guide.json"), br#"{"title": "Loose", "steps": [{"id": "a"}, {"id": "b"}]}"#)
            .unwrap();
        let counts: Vec<_> = scan(dir.path()).iter().map(|e| (e.title.clone(), e.steps)).collect();
        assert!(
            counts.contains(&("Loose".to_string(), 2)) && counts.contains(&("Counted".to_string(), 5)),
            "{counts:?}"
        );
        assert!(guide.exists());
    }

    #[test]
    fn two_guides_of_one_name_both_keep_their_place_in_the_trash() {
        let dir = tempfile::tempdir().unwrap();
        let first = real_guide(dir.path(), "Same name");
        let second = real_guide(dir.path(), "Same name");
        move_to_trash(&first, dir.path(), &nobody()).unwrap();
        move_to_trash(&second, dir.path(), &nobody()).unwrap();
        let mut names: Vec<_> = std::fs::read_dir(dir.path().join("Trash"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        assert_eq!(names, ["Same name (2).osguide", "Same name.osguide"]);
    }

    #[test]
    fn what_is_not_a_guide_in_the_guides_folder_is_never_moved() {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let outside = real_guide(elsewhere.path(), "Somewhere else");
        let plain = dir.path().join("Photos");
        std::fs::create_dir_all(&plain).unwrap();
        std::fs::write(plain.join("a.png"), b"x").unwrap();
        let no_json = dir.path().join("Empty.osguide");
        std::fs::create_dir_all(&no_json).unwrap();
        let file = dir.path().join("loose.osguide");
        std::fs::write(&file, b"x").unwrap();
        let missing = dir.path().join("nothing.osguide");
        for path in [&outside, &plain, &no_json, &file, &missing, dir.path()] {
            let said = move_to_trash(path, dir.path(), &nobody()).unwrap_err();
            assert!(said.contains("left where it is"), "{path:?}: {said}");
            assert!(path.exists() || path == missing.as_path());
        }
        assert!(!dir.path().join("Trash").exists(), "no Trash was made for nothing");
        // A guide inside a sub-folder of the guides folder is not directly in it either.
        let deep = real_guide(&plain, "Deep");
        assert!(move_to_trash(&deep, dir.path(), &nobody()).is_err());
    }

    #[test]
    fn a_guide_another_copy_has_open_is_not_moved_but_a_stale_lock_does_not_stop_it() {
        let dir = tempfile::tempdir().unwrap();
        let guide = real_guide(dir.path(), "In use");
        lock_held_by(&guide, 4242);
        let said = move_to_trash(&guide, dir.path(), &|pid| pid == 4242).unwrap_err();
        assert!(said.contains("open in another copy of Express (process 4242)"), "{said}");
        assert!(guide.exists());
        assert_eq!(held_by(&guide, &|pid| pid == 4242), Some(4242));
        assert_eq!(held_by(&guide, &nobody()), None, "nobody is running with that number now");
        assert!(move_to_trash(&guide, dir.path(), &nobody()).is_ok(), "the copy that had it open crashed");
        // A lock nobody can read is a stale one, and a guide with no lock is free.
        let other = real_guide(dir.path(), "Other");
        std::fs::write(other.join("guide.lock"), b"garbage").unwrap();
        assert_eq!(held_by(&other, &|_| true), None);
        assert!(held_by(dir.path(), &|_| true).is_none());
    }

    #[test]
    fn a_guide_is_brought_back_whole_and_under_a_free_name() {
        let dir = tempfile::tempdir().unwrap();
        let guide = real_guide(dir.path(), "Come back");
        let in_trash = move_to_trash(&guide, dir.path(), &nobody()).unwrap();
        // The name was taken while it was away.
        let newer = real_guide(dir.path(), "Come back");
        let back = restore(&in_trash, dir.path()).unwrap();
        assert_eq!(back, dir.path().join("Come back (2).osguide"));
        assert!(newer.exists() && back.exists() && !in_trash.exists());
        assert!(scan_trash(dir.path()).is_empty());
        let titles: Vec<_> = scan(dir.path()).into_iter().map(|e| e.title).collect();
        assert_eq!(titles, ["Come back", "Come back"]);
        let opened = GuideFolder::open(&back, &nobody()).unwrap();
        assert_eq!(opened.access, Access::ReadWrite);
        assert_eq!(opened.doc.guide.steps.len(), demo_steps(), "every step came back");
    }

    #[test]
    fn only_a_guide_that_is_in_the_trash_can_be_brought_back() {
        let dir = tempfile::tempdir().unwrap();
        let guide = real_guide(dir.path(), "Not deleted");
        let said = restore(&guide, dir.path()).unwrap_err();
        assert!(said.contains("not in the Trash"), "{said}");
        assert!(guide.exists());
        assert!(restore(&dir.path().join("Trash").join("nothing.osguide"), dir.path()).is_err());
        let elsewhere = tempfile::tempdir().unwrap();
        let other = real_guide(elsewhere.path(), "Elsewhere");
        assert!(restore(&other, dir.path()).is_err());
    }

    #[test]
    fn renaming_changes_the_title_and_the_time_but_not_the_folder_or_the_steps() {
        let dir = tempfile::tempdir().unwrap();
        let guide = real_guide(dir.path(), "Old name");
        let before = scan(dir.path());
        let kept = rename_guide(&guide, "   A better name  ", &nobody()).unwrap();
        assert_eq!(kept, "A better name");
        let after = scan(dir.path());
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].title, "A better name");
        assert_eq!(after[0].path, guide, "the folder keeps its name, as when the editor renames a guide");
        assert_eq!(after[0].steps, before[0].steps);
        assert!(after[0].updated_at >= before[0].updated_at);
        assert!(!guide.join("guide.lock").exists(), "the guide is let go again");
        let opened = GuideFolder::open(&guide, &nobody()).unwrap();
        assert_eq!(opened.doc.guide.title, "A better name");
        assert_eq!(opened.doc.guide.steps.len(), demo_steps());
    }

    #[test]
    fn a_name_that_cannot_be_used_or_a_guide_in_use_is_refused_and_nothing_changes() {
        let dir = tempfile::tempdir().unwrap();
        let guide = real_guide(dir.path(), "Steady");
        for blank in ["", "   ", "\t"] {
            assert!(rename_guide(&guide, blank, &nobody()).unwrap_err().contains("needs a name"));
        }
        lock_held_by(&guide, 31337);
        let said = rename_guide(&guide, "New", &|pid| pid == 31337).unwrap_err();
        assert!(said.contains("process 31337"), "{said}");
        assert!(rename_guide(&dir.path().join("nothing.osguide"), "New", &nobody())
            .unwrap_err()
            .contains("could not be opened"));
        assert_eq!(scan(dir.path())[0].title, "Steady");
        assert!(guide.join("guide.lock").exists(), "another copy's lock is left alone");
    }

    #[test]
    fn the_default_place_is_under_documents() {
        let docs = PathBuf::from("Docs");
        assert_eq!(in_documents(docs), PathBuf::from("Docs").join("OpenStage Express").join("Guides"));
    }
}
