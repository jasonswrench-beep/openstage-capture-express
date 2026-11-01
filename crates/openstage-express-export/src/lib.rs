// SPDX-License-Identifier: Apache-2.0
//! Exports for OpenStage Capture Express, with no Windows code in it (spec section 10).
//!
//! Built so far (milestone M4): HTML ([`html`]), Markdown ([`markdown`]), JSON ([`json`]), a folder of pictures ([`images`])
//! and an animated GIF ([`gif`]). Each one takes the same [`plan::Plan`], so they agree about which steps are exported, how
//! they are numbered and what every picture looks like. PDF ([`pdf`]) writes a print page and hands it to a renderer, which
//! on Windows is the WebView2 engine in `openstage-express-windows`. Still to come: Word and PowerPoint.
//!
//! An export is a list of files in memory ([`Export`]); writing them to a folder is a separate step that refuses to
//! replace a file that already exists, so nothing a person has is ever overwritten.
//!
//! Exports contain flattened pixels: the marks and the blur are burned into the pictures, and a blurred area can never be
//! undone. The originals stay in the guide folder.

pub mod demo;
pub mod encoding;
pub mod gif;
pub mod html;
pub mod images;
pub mod json;
pub mod markdown;
pub mod pdf;
pub mod picture;
pub mod plan;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub use picture::{FolderPictures, MemoryPictures, PictureSource};
pub use plan::{ExportOptions, Plan, PlannedStep};

/// Why an export could not be made or written. The text is written for the person.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("The file name {0} is not allowed in an export.")]
    BadPath(String),
    #[error("{0} already exists, so the export stopped without replacing it. Choose a new folder.")]
    AlreadyExists(String),
    #[error("{what}: {source}. Nothing already in the folder was changed.")]
    Io {
        what: String,
        #[source]
        source: std::io::Error,
    },
}

/// One file of an export: a path inside the export folder (with `/` between parts) and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

/// What an export made, and what the person may want to fix.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Export {
    pub files: Vec<OutFile>,
    pub warnings: Vec<String>,
}

/// True for a relative path made of plain names: no `..`, no drive, no leading slash, no backslash, no control characters.
pub fn is_plain_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains(['\\', ':', '\0'])
        && !path.chars().any(char::is_control)
        && path.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

impl Export {
    /// Write every file into `folder` (made if needed). A file that already exists is never replaced: the export stops
    /// there with an error, and files written before it stay. Returns the paths written, in order.
    pub fn write_to(&self, folder: &Path) -> Result<Vec<PathBuf>, ExportError> {
        for file in &self.files {
            if !is_plain_relative_path(&file.path) {
                return Err(ExportError::BadPath(file.path.clone()));
            }
        }
        fs::create_dir_all(folder).map_err(|source| ExportError::Io {
            what: format!("The folder {} could not be made", folder.display()),
            source,
        })?;
        let mut written = Vec::new();
        for file in &self.files {
            let target = folder.join(file.path.replace('/', std::path::MAIN_SEPARATOR_STR));
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|source| ExportError::Io {
                    what: format!("The folder {} could not be made", parent.display()),
                    source,
                })?;
            }
            let mut handle = fs::OpenOptions::new().write(true).create_new(true).open(&target).map_err(|source| {
                if source.kind() == std::io::ErrorKind::AlreadyExists {
                    ExportError::AlreadyExists(target.display().to_string())
                } else {
                    ExportError::Io { what: format!("The file {} could not be made", target.display()), source }
                }
            })?;
            handle.write_all(&file.bytes).and_then(|()| handle.sync_all()).map_err(|source| ExportError::Io {
                what: format!("The file {} could not be written", target.display()),
                source,
            })?;
            written.push(target);
        }
        Ok(written)
    }

    /// The bytes of the file at `path`, if the export has one.
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files.iter().find(|f| f.path == path).map(|f| f.bytes.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(path: &str, text: &str) -> OutFile {
        OutFile { path: path.to_string(), bytes: text.as_bytes().to_vec() }
    }

    #[test]
    fn plain_relative_paths() {
        for good in ["a.html", "images/step-01.png", "a b/c d.txt"] {
            assert!(is_plain_relative_path(good), "{good}");
        }
        for bad in ["", "/a", "../a", "a/../b", "a//b", "a/./b", "C:/x", "a\\b", "a/", "a\u{0}b", "a\nb", "."] {
            assert!(!is_plain_relative_path(bad), "{bad:?}");
        }
    }

    #[test]
    fn files_are_written_into_nested_folders() {
        let dir = tempfile::tempdir().unwrap();
        let export =
            Export { files: vec![out("guide.html", "<p>"), out("images/step-01.png", "png")], warnings: vec![] };
        let written = export.write_to(&dir.path().join("made")).unwrap();
        assert_eq!(written.len(), 2);
        assert_eq!(fs::read(dir.path().join("made/guide.html")).unwrap(), b"<p>");
        assert_eq!(fs::read(dir.path().join("made/images/step-01.png")).unwrap(), b"png");
        assert_eq!(export.file("guide.html"), Some(&b"<p>"[..]));
        assert_eq!(export.file("nope"), None);
    }

    #[test]
    fn nothing_that_exists_is_ever_replaced() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("guide.html"), "mine").unwrap();
        let export = Export { files: vec![out("a.txt", "new"), out("guide.html", "theirs")], warnings: vec![] };
        let error = export.write_to(dir.path()).unwrap_err();
        assert!(matches!(error, ExportError::AlreadyExists(_)), "{error}");
        assert_eq!(fs::read(dir.path().join("guide.html")).unwrap(), b"mine");
        assert!(error.to_string().contains("without replacing it"));
    }

    #[test]
    fn a_bad_path_stops_the_export_before_anything_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let export = Export { files: vec![out("ok.txt", "x"), out("../escape.txt", "y")], warnings: vec![] };
        assert!(matches!(export.write_to(&dir.path().join("made")), Err(ExportError::BadPath(_))));
        assert!(!dir.path().join("made").exists());
        assert!(!dir.path().join("escape.txt").exists());
    }
}
