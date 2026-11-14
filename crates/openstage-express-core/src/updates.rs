// SPDX-License-Identifier: Apache-2.0
//! Check for updates: when the person chooses Help > Check for updates (never by itself), one request to the list of Express's releases
//! on GitHub, and an answer in words with a link to the newer release's page. Nothing is downloaded or installed; the person runs the new
//! setup, which updates the copy in place and keeps the guides, the exports and the settings. It is the same in every tool of the OpenStage
//! suite (docs/update-check.md says what and why).
//!
//! This part is the words and the rules, with no network in it: which release this copy is (`OPENSTAGE_RELEASE`, set when the installer
//! is built), how versions sort, what to say about GitHub's list, and which pages may be opened. The request itself is Windows' own HTTPS
//! code, in the Windows part (`openstage_express_windows::updates`). It is the one network request the program makes.

/// GitHub's list of Express's releases, and the page of them.
pub const RELEASES_API_HOST: &str = "api.github.com";
pub const RELEASES_API_PATH: &str = "/repos/jasonswrench-beep/openstage-capture-express/releases?per_page=30";
pub const RELEASES_PAGE: &str = "https://github.com/jasonswrench-beep/openstage-capture-express/releases";

/// The release this copy was built as, such as 0.1.0-alpha.4. Empty when it was not built for a release.
pub fn release() -> &'static str {
    option_env!("OPENSTAGE_RELEASE").unwrap_or("")
}

/// What the answer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateState {
    /// A newer release is published.
    Newer,
    /// This copy is the newest, or newer than any published.
    Current,
    /// This copy was not built for a release, so it cannot be compared; the newest is named.
    Unknown,
    /// Checking is turned off on this computer.
    Off,
    /// GitHub could not be reached, or its answer could not be read.
    Error,
}

/// The answer to Check for updates, in words, with the page to offer (only ever a page of Express's releases).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateAnswer {
    pub state: UpdateState,
    pub message: String,
    pub url: Option<String>,
}

impl UpdateAnswer {
    pub fn off() -> UpdateAnswer {
        UpdateAnswer {
            state: UpdateState::Off,
            message: "Checking for updates is turned off on this computer (by whoever looks after it).".to_string(),
            url: None,
        }
    }

    pub fn error(message: impl Into<String>) -> UpdateAnswer {
        UpdateAnswer { state: UpdateState::Error, message: message.into(), url: Some(RELEASES_PAGE.to_string()) }
    }
}

/// A version as it sorts: the three numbers, then where it stands (alpha, beta, rc, final) and its number.
pub fn version_key(text: &str) -> Option<(u64, u64, u64, u8, u64)> {
    let text = text.trim().trim_start_matches('v');
    let (main, pre) = match text.split_once('-') {
        Some((m, p)) => (m, Some(p)),
        None => (text, None),
    };
    let number = |n: &str| if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) { None } else { n.parse::<u64>().ok() };
    let mut nums = main.split('.').map(number);
    let (a, b, c) = (nums.next()??, nums.next()??, nums.next()??);
    if nums.next().is_some() {
        return None;
    }
    let (stage, n) = match pre {
        None => (3, 0),
        Some(p) => {
            let (word, num) = p.split_once('.')?;
            let stage = match word {
                "alpha" => 0,
                "beta" => 1,
                "rc" => 2,
                _ => return None,
            };
            (stage, number(num)?)
        }
    };
    Some((a, b, c, stage, n))
}

/// "0.1.0 alpha 4" for 0.1.0-alpha.4.
pub fn spoken(version: &str) -> String {
    let v = version.trim_start_matches('v');
    match v.split_once('-') {
        Some((main, pre)) => format!("{main} {}", pre.replace('.', " ")),
        None => v.to_string(),
    }
}

/// The page of a release of Express, and only of Express: a tag that is not a plain version gives the list of releases.
pub fn release_page(tag: &str) -> String {
    if version_key(tag).is_some() && tag.starts_with('v') {
        format!("{RELEASES_PAGE}/tag/{tag}")
    } else {
        RELEASES_PAGE.to_string()
    }
}

/// Whether an address is a page of Express's releases (the list, or one release by its tag): the only page the program opens.
pub fn is_release_page(url: &str) -> bool {
    url == RELEASES_PAGE
        || url
            .strip_prefix(RELEASES_PAGE)
            .and_then(|rest| rest.strip_prefix("/tag/"))
            .is_some_and(|tag| tag.starts_with('v') && version_key(tag).is_some())
}

/// The answer, from the release this copy is and the text of GitHub's list.
pub fn answer(installed: &str, releases_json: &str) -> UpdateAnswer {
    let Ok(serde_json::Value::Array(list)) = serde_json::from_str::<serde_json::Value>(releases_json) else {
        return UpdateAnswer::error("The list of releases could not be read. Try again later, or look at the releases page.");
    };
    let newest = list
        .iter()
        .filter(|r| r["draft"].as_bool() != Some(true))
        .filter_map(|r| r["tag_name"].as_str())
        .filter(|tag| tag.starts_with('v'))
        .filter_map(|tag| version_key(tag).map(|k| (k, tag.to_string())))
        .max();
    let Some((newest_key, tag)) = newest else {
        return UpdateAnswer::error("No release of Express is published yet.");
    };
    let latest = tag.trim_start_matches('v').to_string();
    let url = Some(release_page(&tag));
    match version_key(installed) {
        None => UpdateAnswer {
            state: UpdateState::Unknown,
            message: format!(
                "This copy was not made by the setup, so its release is not known. The newest published version is {}.",
                spoken(&latest)
            ),
            url,
        },
        Some(mine) if newest_key > mine => UpdateAnswer {
            state: UpdateState::Newer,
            message: format!(
                "A newer version is published: {} (this copy is {}). Its page has what changed and the setup; running the setup \
                 updates this copy and keeps your guides, exports and settings.",
                spoken(&latest),
                spoken(installed)
            ),
            url,
        },
        Some(mine) => UpdateAnswer {
            state: UpdateState::Current,
            message: if newest_key == mine {
                format!("This copy is up to date: {} is the newest published version.", spoken(installed))
            } else {
                format!(
                    "This copy is up to date. It is {}, and the newest published version is {}.",
                    spoken(installed),
                    spoken(&latest)
                )
            },
            url: None,
        },
    }
}

/// Whether checking is turned off on this computer: `OPENSTAGE_UPDATE_CHECK=off`, or a file `update-check.txt` beside the program that
/// says `off` (the way an IT department turns it off for the copies it installs).
pub fn turned_off(program_dir: Option<&std::path::Path>) -> bool {
    let off = |s: &str| s.trim().eq_ignore_ascii_case("off");
    std::env::var("OPENSTAGE_UPDATE_CHECK").is_ok_and(|v| off(&v))
        || program_dir.and_then(|d| std::fs::read_to_string(d.join("update-check.txt")).ok()).is_some_and(|t| off(&t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_sort_with_pre_releases_before_the_final_one() {
        let order =
            ["0.1.0-alpha.1", "0.1.0-alpha.2", "0.1.0-alpha.10", "0.1.0-beta.1", "0.1.0-rc.1", "0.1.0", "0.1.1-alpha.1", "0.2.0", "1.0.0"];
        for pair in order.windows(2) {
            assert!(version_key(pair[0]).unwrap() < version_key(pair[1]).unwrap(), "{} < {}", pair[0], pair[1]);
        }
        assert_eq!(version_key("v0.1.0-alpha.4"), version_key("0.1.0-alpha.4"));
        for bad in ["", "0.1", "0.1.0.1", "0.1.0-gamma.1", "0.1.0-alpha", "0.1.0-alpha.x", "x.1.0", "0.1.0-alpha.1.2", "0.1.0-alpha.+1"] {
            assert_eq!(version_key(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_version_is_said_in_words() {
        assert_eq!(spoken("0.1.0-alpha.4"), "0.1.0 alpha 4");
        assert_eq!(spoken("v1.2.3"), "1.2.3");
    }

    #[test]
    fn only_express_release_pages_are_offered_or_opened() {
        assert_eq!(release_page("v0.1.0-alpha.4"), format!("{RELEASES_PAGE}/tag/v0.1.0-alpha.4"));
        assert_eq!(release_page("v0.1.0-alpha.4/../../../evil"), RELEASES_PAGE);
        assert_eq!(release_page("https://example.org"), RELEASES_PAGE);
        assert_eq!(release_page("0.1.0"), RELEASES_PAGE, "a tag without its v is not one of ours");
        assert!(is_release_page(RELEASES_PAGE));
        assert!(is_release_page(&format!("{RELEASES_PAGE}/tag/v0.1.0-alpha.4")));
        for bad in [
            format!("{RELEASES_PAGE}/tag/v0.1.0-alpha.4/../../x"),
            format!("{RELEASES_PAGE}/tag/0.1.0"),
            format!("{RELEASES_PAGE}/"),
            "https://github.com/jasonswrench-beep/openstage-author/releases".to_string(),
            format!("{RELEASES_PAGE}.example.org"),
            RELEASES_PAGE.replace("https://", "http://"),
            "file:///C:/Windows/System32/calc.exe".to_string(),
            String::new(),
        ] {
            assert!(!is_release_page(&bad), "{bad}");
        }
    }

    #[test]
    fn the_answer_says_newer_current_or_unknown_and_never_counts_a_draft() {
        let list = r#"[{"tag_name":"v0.1.0-alpha.5","draft":true},{"tag_name":"v0.1.0-alpha.4","draft":false,"prerelease":true},{"tag_name":"v0.1.0-alpha.3","draft":false},{"tag_name":"nonsense"},{"tag_name":"9.9.9"}]"#;
        let newer = answer("0.1.0-alpha.3", list);
        assert_eq!(newer.state, UpdateState::Newer);
        assert_eq!(newer.url.as_deref(), Some(format!("{RELEASES_PAGE}/tag/v0.1.0-alpha.4").as_str()));
        assert!(newer.message.contains("0.1.0 alpha 4") && newer.message.contains("keeps your guides"));
        let current = answer("0.1.0-alpha.4", list);
        assert_eq!(current.state, UpdateState::Current, "the draft alpha 5 is not offered, nor a tag without its v");
        assert!(current.message.contains("0.1.0 alpha 4 is the newest published version") && current.url.is_none());
        let ahead = answer("0.1.0-alpha.5", list);
        assert_eq!(ahead.state, UpdateState::Current);
        assert!(ahead.message.contains("It is 0.1.0 alpha 5, and the newest published version is 0.1.0 alpha 4."));
        assert_eq!(answer("0.1.0", list).state, UpdateState::Current, "a final release is newer than its pre-releases");
        assert_eq!(answer("", list).state, UpdateState::Unknown);
        for broken in ["not json", "[]", r#"{"message":"rate limited"}"#] {
            let said = answer("0.1.0-alpha.3", broken);
            assert_eq!(said.state, UpdateState::Error, "{broken}");
            assert_eq!(said.url.as_deref(), Some(RELEASES_PAGE));
        }
    }

    #[test]
    fn it_can_be_turned_off_with_a_file_beside_the_program() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!turned_off(Some(dir.path())));
        std::fs::write(dir.path().join("update-check.txt"), " OFF \r\n").unwrap();
        assert!(turned_off(Some(dir.path())));
        std::fs::write(dir.path().join("update-check.txt"), "manual").unwrap();
        assert!(!turned_off(Some(dir.path())));
        assert!(!turned_off(None));
    }
}
