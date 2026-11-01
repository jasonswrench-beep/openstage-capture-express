// SPDX-License-Identifier: Apache-2.0
//! `CHECK EXPRESS COMPUTER.bat` (spec section 12.2): is this computer ready for Express to capture? Each check says PASS, WARN
//! (it will work, but something is worth knowing) or FAIL (capturing will not work until it is fixed), in words for a person.
//! It sends no keys, moves no pointer and saves nothing but one small file it makes and deletes in the guides folder.

use std::path::Path;
use std::time::{Duration, Instant};

use openstage_express_windows::hotkeys::{Hotkeys, DEFAULTS};
use openstage_express_windows::input::Hooks;
use openstage_express_windows::pdf::WebView2Pdf;
use openstage_express_windows::screen::{self, ScreenFeed};
use openstage_express_windows::system;
use openstage_express_windows::uia::Uia;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Pass,
    Warn,
    Fail,
}

struct Results {
    lines: Vec<(Level, String)>,
}

impl Results {
    fn add(&mut self, level: Level, text: impl Into<String>) {
        let text = text.into();
        let tag = match level {
            Level::Pass => "PASS",
            Level::Warn => "WARN",
            Level::Fail => "FAIL",
        };
        println!("{tag}  {text}");
        self.lines.push((level, text));
    }
}

/// Windows 10 version 2004 (build 19041) is the first with `WDA_EXCLUDEFROMCAPTURE`, which keeps Express's own windows out of
/// its pictures.
const FIRST_BUILD_THAT_HIDES_OWN_WINDOWS: u32 = 19041;

/// Run every check. `guides` is the guides folder to look at. Returns whether nothing failed.
pub fn run(guides: &Path) -> bool {
    println!("OpenStage Capture Express: is this computer ready?\n");
    let mut r = Results { lines: Vec::new() };

    // Windows.
    match screen::windows_version() {
        Ok((words, build)) => {
            r.add(Level::Pass, format!("Windows: {words}"));
            if build < FIRST_BUILD_THAT_HIDES_OWN_WINDOWS {
                r.add(
                    Level::Fail,
                    "This Windows is too old to keep Express's own windows out of its pictures (it needs Windows 10 version 2004 or later).",
                );
            }
        }
        Err(e) => r.add(Level::Fail, format!("Windows would not say which version it is: {e}")),
    }
    if screen::capture_supported() {
        r.add(Level::Pass, "Windows' screen capture service is available.");
    } else {
        r.add(
            Level::Fail,
            "Windows' screen capture service is not available, so no pictures of the screen can be taken.",
        );
    }

    // Displays.
    match screen::displays() {
        Ok(all) if !all.is_empty() => {
            r.add(Level::Pass, format!("{} display{}:", all.len(), if all.len() == 1 { "" } else { "s" }));
            for d in &all {
                let note = if d.x < 0 || d.y < 0 { " (its corner is left of or above the main display)" } else { "" };
                println!("        {}, scaling {} percent{note}", d.describe(), screen::dpi_percent(d));
            }
        }
        Ok(_) => r.add(Level::Fail, "Windows reports no display."),
        Err(e) => r.add(Level::Fail, format!("Windows would not list the displays: {e}")),
    }

    // Pictures actually arrive.
    let started = Instant::now();
    match ScreenFeed::start(1, Duration::from_millis(40)) {
        Ok(feed) => match feed.next(Duration::from_secs(5)) {
            Ok((_, picture)) => r.add(
                Level::Pass,
                format!(
                    "A picture of the screen arrived in {} ms: {} by {}.",
                    started.elapsed().as_millis(),
                    picture.width,
                    picture.height
                ),
            ),
            Err(_) => r.add(
                Level::Warn,
                "The screen capture started but no picture arrived in five seconds. That can happen when nothing on the screen changes; move a window and try again.",
            ),
        },
        Err(e) => r.add(Level::Fail, format!("The screen capture would not start: {e}")),
    }

    // UI Automation.
    match Uia::new() {
        Some(uia) => {
            let found = screen::displays()
                .ok()
                .and_then(|all| screen::pick_display(&all, 1).cloned())
                .and_then(|d| uia.element_at(d.x + (d.width / 2) as i32, d.y + (d.height / 2) as i32));
            match found {
                Some(e) => r.add(Level::Pass, format!("Windows' accessibility information answers: the middle of the screen is a {} ({:?}).", e.role, e.name)),
                None => r.add(
                    Level::Warn,
                    "Windows' accessibility information started but did not answer for the middle of the screen. Steps may say Click here.",
                ),
            }
        }
        None => r.add(
            Level::Fail,
            "Windows' accessibility information (UI Automation) could not start, so the names of clicked things cannot be found.",
        ),
    }

    // Hooks.
    match Hooks::start() {
        Ok(hooks) => {
            r.add(Level::Pass, "The mouse and keyboard can be watched.");
            drop(hooks);
        }
        Err(why) => r.add(Level::Fail, why),
    }

    // Hotkeys.
    let hotkeys = Hotkeys::start(&DEFAULTS);
    if hotkeys.problems.is_empty() {
        r.add(
            Level::Pass,
            format!(
                "The hotkeys can be set: {}.",
                DEFAULTS.iter().map(|(a, k)| format!("{} {}", k.describe(), a.words())).collect::<Vec<_>>().join(", ")
            ),
        );
    } else {
        for problem in &hotkeys.problems {
            r.add(Level::Warn, problem.clone());
        }
    }
    drop(hotkeys);

    // The guides folder.
    match std::fs::create_dir_all(guides) {
        Ok(()) => {
            let probe = guides.join(format!("express-check-{}.tmp", std::process::id()));
            match std::fs::write(&probe, b"check") {
                Ok(()) => {
                    let _ = std::fs::remove_file(&probe);
                    r.add(Level::Pass, format!("The guides folder can be written to: {}", guides.display()));
                }
                Err(e) => {
                    r.add(Level::Fail, format!("The guides folder {} cannot be written to: {e}", guides.display()))
                }
            }
        }
        Err(e) => r.add(Level::Fail, format!("The guides folder {} could not be made: {e}", guides.display())),
    }
    match system::free_space(guides) {
        Some(bytes) => {
            let gigabytes = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            if bytes < 1024 * 1024 * 1024 {
                r.add(
                    Level::Warn,
                    format!("Only {gigabytes:.1} GB is free where guides are kept. A step is about 0.7 MB."),
                );
            } else {
                r.add(Level::Pass, format!("{gigabytes:.0} GB is free where guides are kept."));
            }
        }
        None => r.add(Level::Warn, "Windows would not say how much disk space is free."),
    }

    // PDF export.
    match WebView2Pdf::runtime_version() {
        Some(version) => {
            r.add(Level::Pass, format!("The WebView2 runtime is installed (version {version}), so PDF export works."))
        }
        None => r.add(
            Level::Warn,
            "The WebView2 runtime is not installed, so PDF export will say so. Every other export works.",
        ),
    }

    let fails = r.lines.iter().filter(|(l, _)| *l == Level::Fail).count();
    let warns = r.lines.iter().filter(|(l, _)| *l == Level::Warn).count();
    println!();
    if fails > 0 {
        println!("FAIL. {fails} thing{} must be fixed before capturing will work.", if fails == 1 { "" } else { "s" });
    } else if warns > 0 {
        println!("PASS, with {warns} thing{} worth knowing (marked WARN above).", if warns == 1 { "" } else { "s" });
    } else {
        println!("PASS. This computer is ready.");
    }
    fails == 0
}
