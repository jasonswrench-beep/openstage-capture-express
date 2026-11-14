// SPDX-License-Identifier: Apache-2.0
//! Small file and clock helpers with no Windows code in them.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Whether a path is a link, a junction or any other reparse point, which Express never follows inside a guide folder or the
/// guides folder: one could lead a move, a rename or an emptied trash to files elsewhere (the security review of 2026-11-13).
/// A path that cannot be looked at is not a link.
pub fn is_link(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(meta) => meta.file_type().is_symlink() || is_reparse_point(&meta),
        Err(_) => false,
    }
}

#[cfg(windows)]
fn is_reparse_point(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_meta: &fs::Metadata) -> bool {
    false
}

/// A file that is a plain file (not a link) and whose real place is inside `root`.
pub fn is_plain_file_inside(path: &Path, root: &Path) -> bool {
    if is_link(path) || !path.is_file() {
        return false;
    }
    match (fs::canonicalize(path), fs::canonicalize(root)) {
        (Ok(real), Ok(root)) => real.starts_with(root),
        _ => false,
    }
}

/// Whether a path names a folder on another computer (`\\server\share`) or is written in one of Windows' long forms (`\\?\`,
/// `\\.\`). Reading one makes Windows connect to that server with the person's sign-in, so the guides and exports folders are
/// never such a path (the security review of 2026-11-13). A network drive with a letter, such as Z:, is an ordinary path.
pub fn is_network_path(path: &Path) -> bool {
    let text = path.to_string_lossy();
    text.starts_with("\\\\") || text.starts_with("//")
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".tmp");
    PathBuf::from(name)
}

/// Write a file so that a crash leaves either the old file or the new one, never half of each: the bytes go to a
/// temporary file beside it, are forced to disk, and the temporary file is renamed over the real one.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = tmp_path(path);
    {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

/// A UTC time as `2026-10-31T14:32:05Z`, from a count of seconds since 1 January 1970.
pub fn iso_from_unix_seconds(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

/// Days since 1 January 1970 to a calendar date (the proleptic Gregorian calendar).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (if month <= 2 { year + 1 } else { year }, month as u32, day as u32)
}

/// The current time in UTC.
pub fn utc_now_iso() -> String {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    iso_from_unix_seconds(seconds)
}

/// A new guide id such as `g-2026-10-31-1432-a1b2`, made from the time and four hex digits. Unique enough to tell
/// guides apart; not secret.
pub fn new_guide_id(now_iso: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos().hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    COUNTER.fetch_add(1, Ordering::Relaxed).hash(&mut hasher);
    // The digits of 2026-10-31T14:32:05Z are 20261031143205: year, month, day, hour, minute, second.
    let digits: String = now_iso.chars().filter(char::is_ascii_digit).collect();
    let part =
        |range: std::ops::Range<usize>, fallback: &'static str| digits.get(range).unwrap_or(fallback).to_string();
    format!(
        "g-{}-{}-{}-{}-{:04x}",
        part(0..4, "0000"),
        part(4..6, "00"),
        part(6..8, "00"),
        part(8..12, "0000"),
        hasher.finish() & 0xffff
    )
}

/// A folder name made from a guide title: no characters Windows forbids, no reserved names, not too long.
pub fn sanitise_folder_name(title: &str) -> String {
    let mut name: String =
        title.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).collect();
    name = name.trim().trim_end_matches(['.', ' ']).to_string();
    if name.chars().count() > 80 {
        name = name.chars().take(80).collect::<String>().trim_end().to_string();
    }
    if name.is_empty() {
        return "Untitled guide".to_string();
    }
    // Windows treats a name as the device whatever follows a dot, and spaces before the dot do not count ("CON .txt" is CON).
    let stem = name.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&stem.as_str())
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    if reserved {
        name.push('_');
    }
    name
}

/// The first of `Title.osguide`, `Title (2).osguide`, `Title (3).osguide`, ... that does not exist yet in `parent`.
pub fn unique_guide_folder(parent: &Path, title: &str) -> PathBuf {
    let base = sanitise_folder_name(title);
    let mut candidate = parent.join(format!("{base}.osguide"));
    let mut n = 2;
    while candidate.exists() {
        candidate = parent.join(format!("{base} ({n}).osguide"));
        n += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_are_right_for_known_moments() {
        assert_eq!(iso_from_unix_seconds(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_from_unix_seconds(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(iso_from_unix_seconds(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(iso_from_unix_seconds(951_782_400), "2000-02-29T00:00:00Z", "a leap day");
        assert_eq!(iso_from_unix_seconds(-1), "1969-12-31T23:59:59Z");
        let now = utc_now_iso();
        assert!(now.starts_with("20") && now.ends_with('Z') && now.len() == 20, "{now}");
    }

    #[test]
    fn guide_ids_look_right_and_differ() {
        let a = new_guide_id("2026-10-31T14:32:05Z");
        let b = new_guide_id("2026-10-31T14:32:05Z");
        assert_ne!(a, b);
        assert!(a.starts_with("g-2026-10-31-1432-"), "{a}");
        assert_eq!(a.len(), "g-2026-10-31-1432-a1b2".len(), "{a}");
    }

    #[test]
    fn atomic_write_replaces_a_file_and_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        assert!(!tmp_path(&path).exists());
    }

    #[test]
    fn a_network_path_is_told_from_a_folder_on_this_computer() {
        for network in [r"\\server\share\Guides", r"\\?\UNC\server\share", r"\\?\C:\Guides", r"\\.\pipe\x", "//server/share"] {
            assert!(is_network_path(Path::new(network)), "{network}");
        }
        for local in [r"C:\Users\you\Documents", r"Z:\Guides", r"D:\", "relative\\folder", ""] {
            assert!(!is_network_path(Path::new(local)), "{local}");
        }
    }

    #[test]
    fn a_link_is_never_a_plain_file_inside_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Guide.osguide");
        fs::create_dir_all(root.join("images")).unwrap();
        fs::write(root.join("images").join("step-0001.png"), b"x").unwrap();
        let outside = dir.path().join("outside.png");
        fs::write(&outside, b"y").unwrap();
        assert!(is_plain_file_inside(&root.join("images").join("step-0001.png"), &root));
        assert!(!is_plain_file_inside(&outside, &root), "a file outside the guide folder");
        assert!(!is_plain_file_inside(&root.join("images").join("missing.png"), &root));
        assert!(!is_link(&root.join("images")), "an ordinary folder");
        // A link (where this computer lets a test make one) is never followed.
        #[cfg(windows)]
        if std::os::windows::fs::symlink_file(&outside, root.join("images").join("step-0002.png")).is_ok() {
            assert!(is_link(&root.join("images").join("step-0002.png")));
            assert!(!is_plain_file_inside(&root.join("images").join("step-0002.png"), &root));
        }
    }

    #[test]
    fn folder_names_are_made_safe() {
        assert_eq!(sanitise_folder_name("How to submit an assignment"), "How to submit an assignment");
        assert_eq!(sanitise_folder_name("Set up: two/step <sign-in>?"), "Set up_ two_step _sign-in__");
        assert_eq!(sanitise_folder_name("  trailing dots...  "), "trailing dots");
        assert_eq!(sanitise_folder_name(""), "Untitled guide");
        assert_eq!(sanitise_folder_name("   "), "Untitled guide");
        assert_eq!(sanitise_folder_name("..."), "Untitled guide");
        for reserved in ["CON", "con", "Nul.txt", "COM1", "lpt9", "AUX", "CONIN$", "conout$", "CON .txt", "Nul   .x"] {
            assert!(sanitise_folder_name(reserved).ends_with('_'), "{reserved}");
        }
        assert_eq!(sanitise_folder_name("COM10"), "COM10");
        assert_eq!(sanitise_folder_name("Console"), "Console");
        assert_eq!(sanitise_folder_name(&"x".repeat(200)).chars().count(), 80);
        assert_eq!(sanitise_folder_name("caf\u{e9} au lait"), "caf\u{e9} au lait");
    }

    #[test]
    fn folder_names_are_made_unique() {
        let dir = tempfile::tempdir().unwrap();
        let first = unique_guide_folder(dir.path(), "My guide");
        assert!(first.ends_with("My guide.osguide"));
        fs::create_dir(&first).unwrap();
        let second = unique_guide_folder(dir.path(), "My guide");
        assert!(second.ends_with("My guide (2).osguide"));
        fs::create_dir(&second).unwrap();
        assert!(unique_guide_folder(dir.path(), "My guide").ends_with("My guide (3).osguide"));
    }
}
