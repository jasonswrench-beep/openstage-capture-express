# OpenStage Capture Express

Free, open-source Windows program that turns what a person does on screen into a clean step-by-step guide: a picture for each
step with a marker where the click was, a title already written (such as `Click "Save"`), light editing, and export. It is meant
to be as easy to use as a step-guide program, with no account and no network. Licence: Apache-2.0. Written in Rust.

It is a sibling of OpenStage Capture (the recorder inside OpenStage Author) and reuses its design and code, but it is its own
small program with its own window.

## Where this stands (honest status)

**Milestones M0 to M3 are done. The guide model, the edit operations, undo and redo, saving a guide folder, smart titles, the step builder, drawing of marks and the focused view are written and
tested. There is still no window and no capture: the program starts and prints one line.**

| Run on Windows, and working | Written and type-checked, not yet run on Windows | Tested on any computer (pure logic) | Not built |
|---|---|---|---|
| `BUILD EXPRESS.bat` and `TEST EXPRESS.bat` ran on 2026-10-31 and 2026-11-01 on Jason's Windows 11 x64 PC (build 26200, Core i9-13900K), pinned to the efficiency cores, started from a shell with no keyboard attached rather than double-clicked. The last run: 174 tests passed and the rule checks said PASS. The 75 tests include real files: guide folders were created, saved, locked, crashed and reopened in temporary folders on this PC | The part of `TEST EXPRESS.bat` that runs the rule checker again when it stops with a Windows error code (never triggered) | 174 tests in `openstage-express-core`: the model and its round trip with unknown fields kept; every edit operation; smart titles in English, German and French; the step builder with 40 scenario tests covering rules R1 to R12 of the spec; every kind of mark drawn pixel by pixel on OpenStage Capture's drawing canvas (exact colour mixing, blur that cannot be undone, a mark crossing the edge of a crop); the focused view arithmetic on a grid of 11 clicks by 11 positions by 11 zooms on 7 picture sizes; undo and redo, with a 300-step random walk on four seeds that returns to every earlier state; saving, the lock file, the trash and its pictures, migrations (with made-up versions, since no real migration exists yet), and crash states (a picture written but the guide not, a guide file half written). Seventeen deliberate breakages of the code were each caught by a test. These tests use nothing Windows-specific but have only been run on this Windows PC, not on Linux | M4 to M8 and everything else in the spec: the exports, the window, real capture, settings, the self-tests that need Windows, and the portable zip |

Not yet run on an ARM64 PC. Not yet run by double-clicking from Explorer. The icon files are copied in but not used by any program
yet (milestone M7). `TESTING-TODO.md` lists what only a person can check.

## What to do on Windows (no typing needed)

1. Double-click **`BUILD EXPRESS.bat`**. It checks that Rust is installed (and explains how to install it if not), builds the
   program, and says plainly whether it worked. The first build may ask before downloading libraries.
2. Double-click **`TEST EXPRESS.bat`**. It runs every test, then the rule checks, and says PASS or what failed. It needs Python 3
   for the rule checks. If the checker stops with a Windows error code (not a rule failing) it tries up to three times.

More scripts arrive with their milestones: `EXPORT DEMO.bat` (M4), `OPEN EXPRESS.bat` (M5), `CHECK EXPRESS COMPUTER.bat` and
`TEST EXPRESS CLICKS.bat` (M6).

## Decisions

Made by Jason on 2026-10-31, when the kickoff questions were put to him:

| Question | Answer | What it changes |
|---|---|---|
| Where does it live? | A separate folder with its own git repo, not `capture-express/` inside the OpenStage repo (spec D4 is replaced) | This folder, beside the `openstage-author` clone. Nothing is pushed anywhere, and no remote is set |
| How does it reuse Capture's code? | A path link to the sibling clone | See "Layout on every computer" below |
| Window toolkit? | egui with AccessKit, all Rust (spec D5) | Milestone M5 includes the Narrator and NVDA pass, with Tauri as the fallback |
| First slice? | Capture, edit, **and** PDF, Word and PowerPoint (spec D6, widened) | The first version includes the formats the spec calls S2 |

Everything else in the spec (D7 to D9 and questions 4 to 12 in its section 16) is still a proposal and has not been put to Jason.
a step-guide program is installed on Jason's computer (`C:\Program Files\a step-guide program`) and is the reference program; its behaviour may be looked at
there, but never its code, templates, strings or artwork.

## Layout on every computer

The spec is `docs\capture-express\spec.md` in the OpenStage repo (`https://github.com/jasonswrench-beep/openstage-author`, branch
`feature/capture-express`). Express reads OpenStage Capture's crates (hooks, drawing, region picker) through a path link, so on
every computer these two folders must sit side by side:

```
somewhere\openstage-author\          the OpenStage repo, branch feature/capture-express
somewhere\openstage-capture-express\ this folder
```

The link is in use since milestone M3. `BUILD EXPRESS.bat` and `TEST EXPRESS.bat` check that the folder is there and, if not, say so; `GET OPENSTAGE CODE.bat` copies it from GitHub (it asks first, and needs Git). Choose a folder that OneDrive does not sync, and extract each
new copy into a new, empty folder.

## How the pieces fit

```
crates/
  openstage-express-core/       the guide model, edit operations, undo, saving, smart titles, step builder, drawing and focused view
  openstage-express-export/     HTML, Markdown, JSON, images, GIF, then PDF, Word, PowerPoint (empty so far)
  openstage-express-windows/    hooks, UI Automation, screenshots, hotkeys (Windows only; empty so far)
  openstage-express-app/        the program openstage-capture-express.exe (a placeholder so far)
docs/guide-format.md            the guide format: the source of truth, kept equal to the code
tools/check_rules.py            licence, header, writing-style and script checks
assets/icon/                    the icon pack (SVG, PNG sizes, ICO)
TESTING-TODO.md                 what only a person can check
```

## Rules that shape the code

- Apache-2.0 only. No GPL or AGPL, no FFmpeg, no libuiohook. `tools/check_rules.py` fails the build on a dependency licence
  outside the allowed list.
- No network in version 1: no HTTP client, no telemetry, no update check. The checker also fails on known network and
  telemetry crates.
- Accessibility is not optional: every feature works by keyboard and with a screen reader, and nothing traps focus.
- Typed text is read from the field, never logged key by key, and never for a password field.
- Nothing is erased by the software. Delete means the trash folder, and only the person's "Empty the trash" deletes files.
- The format is the source of truth: change `docs/guide-format.md` in the same commit as any code that reads or writes it.
- a step-guide program is commercial. Use facts and behaviour only, never its code, templates, strings, artwork or file format.
- Anything Jason runs is a double-clickable `.bat`. Never say something works that has only compiled.

For developers, in a shell: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, `python -I tools/check_rules.py`. `python -I tools/check_rules.py --write-third-party` regenerates
`THIRD-PARTY.md`. (On a PC whose processor makes Python crash unpredictably, run these pinned to the efficiency cores.)

## The icon

`assets/icon` holds the icon pack Jason supplied on 2026-10-31: a navy tile with an indigo viewfinder and a rounded lowercase e.
It replaces the earlier capital E icon, which is no longer used. The SVG's title text contains an em dash; the file is kept
exactly as supplied. The pack's `README.txt` says it was checked without a vector drawing program or Windows themselves, so how the `.ico`
looks in Windows, and how the e reads at 16 pixels, are still to be checked by eye.

## Licence

Apache License 2.0. See `LICENSE` and `NOTICE`.
