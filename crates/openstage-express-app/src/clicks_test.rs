// SPDX-License-Identifier: Apache-2.0
//! `TEST EXPRESS CLICKS.bat` (spec section 12.2): the Windows proof of the capture path. It opens a small window of known
//! controls in a second process, then clicks and types in it with `SendInput` while the real capture runs (the real hooks, UI
//! Automation and Windows Graphics Capture), and checks the steps that come out.
//!
//! It moves the real pointer and sends real keys for about thirty seconds, so the person is asked to leave the computer alone.
//! Before it sends any key it checks that its own test window has the keyboard; if not, it sends none.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use openstage_express_core::capture::{Area, Captured, MarkOptions};
use openstage_express_core::GuideFolder;
use openstage_express_windows::hotkeys::{Action, Hotkeys, DEFAULTS};
use openstage_express_windows::run::{CaptureRun, Command as RunCommand, RunEvent, RunOptions, RunState};
use openstage_express_windows::synthetic;
use openstage_express_windows::testwin::{Beacon, Layout, BEACON_BGRA, READY};
use openstage_express_windows::{affinity, screen};

use crate::guide_writer::GuideWriter;

struct Report {
    failures: Vec<String>,
    checks: u32,
}

impl Report {
    fn pass(&mut self, name: &str, detail: &str) {
        self.checks += 1;
        if detail.is_empty() {
            println!("PASS  {name}");
        } else {
            println!("PASS  {name}: {detail}");
        }
    }

    fn fail(&mut self, name: &str, detail: &str) {
        self.checks += 1;
        self.failures.push(format!("{name}: {detail}"));
        println!("FAIL  {name}: {detail}");
    }

    fn check(&mut self, ok: bool, name: &str, detail: &str) {
        if ok {
            self.pass(name, "");
        } else {
            self.fail(name, detail);
        }
    }

    fn skip(&mut self, name: &str, why: &str) {
        println!("SKIP  {name}: {why}");
    }
}

/// The test window's process, ended when this goes out of scope.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_test_window() -> Result<(ChildGuard, Layout), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut child = Command::new(exe)
        .arg("test-window")
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("The test window could not be started: {e}"))?;
    let stdout = child.stdout.take().ok_or("The test window has no output to read.")?;
    let guard = ChildGuard(child);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(json) = line.strip_prefix(READY) {
                let _ = tx.send(serde_json::from_str::<Layout>(json).map_err(|e| e.to_string()));
                break;
            }
        }
    });
    match rx.recv_timeout(Duration::from_secs(15)) {
        Ok(Ok(layout)) => Ok((guard, layout)),
        Ok(Err(why)) => Err(format!("The test window's report could not be read: {why}")),
        Err(_) => Err("The test window did not appear in 15 seconds.".to_string()),
    }
}

/// Everything the run has said so far.
#[derive(Default)]
struct Heard {
    steps: Vec<Captured>,
    notices: Vec<String>,
    finished: bool,
}

/// Read what the run says for `time`, adding it to `heard`. Returns the steps that arrived in this stretch.
fn listen(run: &CaptureRun, heard: &mut Heard, time: Duration) -> Vec<Captured> {
    let until = Instant::now() + time;
    let mut arrived = Vec::new();
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        match run.event(left.min(Duration::from_millis(100))) {
            Some(RunEvent::Step(step)) => {
                arrived.push(step.clone());
                heard.steps.push(step);
            }
            Some(RunEvent::Notice(words)) => heard.notices.push(words),
            Some(RunEvent::State(RunState::Finished)) => {
                heard.finished = true;
                break;
            }
            Some(RunEvent::State(_)) | None => {}
        }
    }
    arrived
}

/// How many pixels of a picture, inside the screen rectangle `(x, y, width, height)`, are the beacon's colour.
fn beacon_pixels(step: &Captured, area: &Area, rect: (i32, i32, i32, i32)) -> usize {
    let (px, py) = area.to_picture(rect.0, rect.1);
    let pic = &step.picture;
    let mut count = 0;
    for y in py.max(0)..(py + rect.3).min(pic.height as i32) {
        for x in px.max(0)..(px + rect.2).min(pic.width as i32) {
            let at = (y as usize * pic.width as usize + x as usize) * 4;
            if pic.bgra[at..at + 3] == BEACON_BGRA[..3] {
                count += 1;
            }
        }
    }
    count
}

fn titles(steps: &[Captured]) -> Vec<String> {
    steps.iter().map(|s| s.draft.title.clone()).collect()
}

fn files_under(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files_under(&path, out);
            } else {
                out.push(path);
            }
        }
    }
}

/// Run the self-test. `out` is a folder to write the guide it captures into (a new folder is made inside it). It waits up to
/// `wait_for_quiet` for the mouse and keyboard to be left alone for a few seconds before it sends anything. Returns whether every
/// check passed.
pub fn run(out: &Path, wait_for_quiet: Duration) -> bool {
    let mut report = Report { failures: Vec::new(), checks: 0 };
    println!("OpenStage Capture Express click test\n");
    println!("Please leave the mouse and keyboard alone until this says it has finished (about thirty seconds).\n");

    // Do not send input while someone is using the computer: wait for a few quiet seconds.
    let quiet = Duration::from_secs(6);
    let waited = Instant::now();
    while synthetic::idle_ms().is_some_and(|idle| Duration::from_millis(idle as u64) < quiet) {
        if waited.elapsed() > wait_for_quiet {
            report.fail(
                "the computer is quiet",
                &format!(
                    "the mouse or keyboard was in use for {} seconds, so nothing was sent",
                    wait_for_quiet.as_secs()
                ),
            );
            return finish(report);
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    // The computer.
    let all = match screen::displays() {
        Ok(all) if !all.is_empty() => all,
        Ok(_) => {
            report.fail("displays", "Windows reports no display.");
            return finish(report);
        }
        Err(why) => {
            report.fail("displays", &why);
            return finish(report);
        }
    };
    let main = screen::pick_display(&all, 1).expect("a display exists");
    println!("      {}", main.describe());
    let area = screen::area_of(main);

    // The window to click in, in a process of its own.
    let (_child, layout) = match start_test_window() {
        Ok(pair) => pair,
        Err(why) => {
            report.fail("test window", &why);
            return finish(report);
        }
    };
    report.pass("test window", "a window of known controls is open");
    std::thread::sleep(Duration::from_millis(700));

    // A rectangle of Express's own, magenta, in a corner of the display.
    let beacon_rect = (area.x + area.width as i32 - 260, area.y + 60, 200, 120);
    let beacon = match Beacon::show(beacon_rect.0, beacon_rect.1, beacon_rect.2, beacon_rect.3) {
        Ok(b) => b,
        Err(why) => {
            report.fail("beacon window", &why);
            return finish(report);
        }
    };
    let beacon_centre = (beacon_rect.0 + beacon_rect.2 / 2, beacon_rect.1 + beacon_rect.3 / 2);

    // The real capture.
    let run = match CaptureRun::start(RunOptions::default()) {
        Ok(run) => run,
        Err(why) => {
            report.fail("capture starts", &why);
            return finish(report);
        }
    };
    report.pass("capture starts", &run.display);
    let mut heard = Heard::default();
    listen(&run, &mut heard, Duration::from_millis(1200));

    // 1. Express's own windows stay out of the pictures. First prove the test can see an ordinary window, then exclude it.
    run.send(RunCommand::CaptureNow);
    let seen = listen(&run, &mut heard, Duration::from_millis(900));
    match seen.first() {
        Some(step) => {
            let n = beacon_pixels(step, &area, beacon_rect);
            report.check(
                n > 1000,
                "an ordinary window shows in a picture",
                &format!("only {n} magenta pixels were found"),
            );
            report.check(
                (step.picture.width, step.picture.height) == (area.width, area.height),
                "a picture is the whole display",
                &format!("{} by {}", step.picture.width, step.picture.height),
            );
        }
        None => report.fail("an ordinary window shows in a picture", "the capture-now command made no step"),
    }
    let (changed, own) = affinity::exclude_own_windows();
    report.check(
        changed >= 1 && affinity::is_excluded(beacon.hwnd()),
        "own windows are marked",
        &format!("{changed} of {own} marked"),
    );
    listen(&run, &mut heard, Duration::from_millis(900));
    run.send(RunCommand::CaptureNow);
    let seen = listen(&run, &mut heard, Duration::from_millis(900));
    match seen.first() {
        Some(step) => {
            let n = beacon_pixels(step, &area, beacon_rect);
            report.check(
                n == 0,
                "Express's own window is not in the picture",
                &format!("{n} magenta pixels were found"),
            );
        }
        None => report.fail("Express's own window is not in the picture", "the capture-now command made no step"),
    }

    // 2. Clicks on controls.
    let before = heard.steps.len();
    synthetic::left_click(layout.save.0, layout.save.1);
    let arrived = listen(&run, &mut heard, Duration::from_millis(1600));
    match arrived.first() {
        Some(step) => {
            report.check(
                step.draft.title == "Click \"Save\"",
                "a button click is titled with the button's name",
                &step.draft.title,
            );
            let click = step.draft.click.as_ref();
            let near = click.is_some_and(|c| {
                let (ex, ey) = area.to_picture(layout.save.0, layout.save.1);
                (c.x - ex).abs() <= 2 && (c.y - ey).abs() <= 2
            });
            report.check(near, "the click is placed where the pointer was", &format!("{click:?}"));
        }
        None => report.fail("a button click is titled with the button's name", "no step was made"),
    }
    report.check(
        heard.steps.len() == before + 1,
        "one click makes one step",
        &format!("{} new steps", heard.steps.len() - before),
    );

    synthetic::left_click(layout.check.0, layout.check.1);
    let arrived = listen(&run, &mut heard, Duration::from_millis(1400));
    report.check(
        titles(&arrived) == ["Click \"Check me\""],
        "a check box click says Click",
        &format!("{:?}", titles(&arrived)),
    );

    synthetic::left_click(layout.country.0, layout.country.1);
    // A program that is showing a drop-down list does not answer questions about its controls until the list is closed, so the
    // list is closed first (as a person would, by choosing something). The step then comes out a moment later.
    std::thread::sleep(Duration::from_millis(500));
    synthetic::key(0x1B); // Escape closes the list
    let arrived = listen(&run, &mut heard, Duration::from_millis(2000));
    report.check(
        titles(&arrived) == ["Select \"Country\""],
        "a drop-down list says Select",
        &format!("{:?}", titles(&arrived)),
    );

    // 3. Typing: the text is read from the field.
    synthetic::left_click(layout.email.0, layout.email.1);
    std::thread::sleep(Duration::from_millis(300));
    let typed_ok = synthetic::foreground_window() == layout.window;
    if !typed_ok {
        report.fail("the test window has the keyboard", "another window has it, so no keys were sent");
    } else {
        report.pass("the test window has the keyboard", "");
        synthetic::type_text("ann@example.org");
        synthetic::key(0x0D); // Enter
        let arrived = listen(&run, &mut heard, Duration::from_millis(1800));
        let typed: Vec<_> = arrived.iter().filter_map(|s| s.draft.typed.clone()).collect();
        report.check(
            typed == ["ann@example.org"],
            "typing makes one Type step with the text read from the field",
            &format!("{:?}", titles(&arrived)),
        );
        report.check(
            titles(&arrived) == ["Type \"ann@example.org\""],
            "the Type step is titled with the text",
            &format!("{:?}", titles(&arrived)),
        );

        // 4. A password field: no text, anywhere.
        synthetic::left_click(layout.password.0, layout.password.1);
        std::thread::sleep(Duration::from_millis(300));
        if synthetic::foreground_window() == layout.window {
            synthetic::type_text("hunter2 secret");
            synthetic::key(0x09); // Tab
            let arrived = listen(&run, &mut heard, Duration::from_millis(2600));
            let leaked = arrived.iter().any(|s| {
                let d = &s.draft;
                let blob = format!("{} {:?} {:?}", d.title, d.typed, d.control);
                blob.contains("hunter2") || blob.contains("secret") || d.typed.is_some()
            });
            report.check(
                !leaked,
                "a password field makes no Type step and no text",
                &format!("{:?}", titles(&arrived)),
            );
            report.check(
                titles(&arrived) == ["Click \"Password\""],
                "the password field is a plain click on its name",
                &format!("{:?}", titles(&arrived)),
            );
        } else {
            report.fail("the password field", "the test window lost the keyboard, so no keys were sent");
        }
    }

    // 5. The hotkey makes a step without a click, and its keys are not typing.
    let hotkeys = Hotkeys::start(&DEFAULTS);
    let capture_now_free = !hotkeys.problems.iter().any(|p| p.contains("capture now"));
    if capture_now_free && synthetic::foreground_window() != 0 {
        synthetic::ctrl_shift(0x78); // F9
        match hotkeys.next(Duration::from_secs(2)) {
            Ok(Action::CaptureNow) => {
                report.pass("the capture-now hotkey is heard while another program has the keyboard", "");
                run.send(RunCommand::CaptureNow);
                let arrived = listen(&run, &mut heard, Duration::from_millis(1200));
                report.check(
                    titles(&arrived) == ["Screenshot"],
                    "capture now makes a Screenshot step",
                    &format!("{:?}", titles(&arrived)),
                );
            }
            other => report.fail("the capture-now hotkey is heard", &format!("it said {other:?}")),
        }
    } else {
        report.skip("the capture-now hotkey", &hotkeys.problems.join(" "));
    }
    drop(hotkeys);

    // 6. A click on one of Express's own windows is never captured.
    let before = heard.steps.len();
    synthetic::left_click(beacon_centre.0, beacon_centre.1);
    listen(&run, &mut heard, Duration::from_millis(1400));
    report.check(
        heard.steps.len() == before,
        "a click on an Express window makes no step",
        &format!("{} steps were made", heard.steps.len() - before),
    );

    // 7. Finish.
    run.send(RunCommand::Finish);
    listen(&run, &mut heard, Duration::from_millis(3000));
    report.check(heard.finished, "finishing ends the run", "the run did not say it had finished");
    let summary = run.join();
    report.check(summary.lost == 0, "no step was lost", &format!("{} lost", summary.lost));
    report.check(
        summary.dropped_events == 0,
        "no mouse or keyboard action was dropped",
        &format!("{} dropped", summary.dropped_events),
    );
    println!("      {} steps, {} pictures skipped", summary.steps, summary.skipped_pictures);
    for notice in &heard.notices {
        println!("NOTE  {notice}");
    }

    // 8. The guide: written step by step, readable again, and the password nowhere in it.
    let mut writer = match GuideWriter::create(out, "Express click test", "en", MarkOptions::default()) {
        Ok(w) => w,
        Err(why) => {
            report.fail("the guide is written", &why);
            return finish(report);
        }
    };
    let mut write_ms = Vec::new();
    for step in &heard.steps {
        let t = Instant::now();
        if let Err(why) = writer.add(step) {
            report.fail("a step is written to the guide", &why);
        }
        write_ms.push(t.elapsed().as_millis());
    }
    println!("      writing a step took {write_ms:?} ms (picture and guide)");
    let folder = writer.path().to_path_buf();
    let wrote = writer.steps();
    drop(writer);
    match GuideFolder::open(&folder, &|_| false) {
        Ok(opened) => {
            report.check(
                opened.doc.guide.steps.len() == wrote && wrote == heard.steps.len(),
                "the guide opens again with every step",
                &format!("{} of {}", opened.doc.guide.steps.len(), heard.steps.len()),
            );
            report.check(opened.problems.is_empty(), "the guide has no problems", &format!("{:?}", opened.problems));
        }
        Err(why) => report.fail("the guide opens again", &why.to_string()),
    }
    let mut files = Vec::new();
    files_under(&folder, &mut files);
    let leaked: Vec<_> = files
        .iter()
        .filter(|f| {
            std::fs::read(f).is_ok_and(|bytes| {
                bytes.windows(7).any(|w| w == b"hunter2") || bytes.windows(6).any(|w| w == b"secret")
            })
        })
        .collect();
    report.check(leaked.is_empty(), "the password is in no file of the guide", &format!("{leaked:?}"));
    println!("\nThe guide it captured is in {}", folder.display());
    finish(report)
}

fn finish(report: Report) -> bool {
    println!();
    if report.failures.is_empty() {
        println!("PASS. All {} checks passed.", report.checks);
        true
    } else {
        println!("FAIL. {} of {} checks failed:", report.failures.len(), report.checks);
        for failure in &report.failures {
            println!("  - {failure}");
        }
        false
    }
}
