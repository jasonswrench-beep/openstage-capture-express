// SPDX-License-Identifier: Apache-2.0
//! The guides folder (spec section 7.1): where guides live, and what Home shows of them. A guide is a folder named
//! `Title.osguide`; this module only looks at them, it never changes or deletes anything.

use std::path::{Path, PathBuf};

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
    let steps = value.get("steps").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
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

    #[test]
    fn the_default_place_is_under_documents() {
        let docs = PathBuf::from("Docs");
        assert_eq!(in_documents(docs), PathBuf::from("Docs").join("OpenStage Express").join("Guides"));
    }
}
