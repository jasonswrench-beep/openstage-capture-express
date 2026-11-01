# OpenStage Capture Express

Free, open-source Windows program that turns what a person does on screen into a clean step-by-step guide: a picture for each
step with a marker where the click was, a title already written (such as `Click "Save"`), light editing, and export. It is meant
to be as easy to use as a step-guide program, with no account and no network. Licence: Apache-2.0. Written in Rust.

It is a sibling of OpenStage Capture (the recorder inside OpenStage Author) and reuses its design and code, but it is its own
small program with its own window.

## Where this stands (honest status)

**Milestone M0 is done: the empty workspace builds, tests and passes the rule checks. There is no program logic yet.** The
program starts and prints one line. It has no window and captures nothing.

| Run on Windows, and working | Written and type-checked, not yet run on Windows | Tested on any computer | Not built |
|---|---|---|---|
| `BUILD EXPRESS.bat` built the four crates in release mode and the program printed its one line. `TEST EXPRESS.bat` ran 7 groups of tests (0 tests in all, nothing to test yet) and the rule checks, and said PASS. Both were run on 2026-10-31 on Jason's Windows 11 x64 PC (build 26200, Core i9-13900K), pinned to the efficiency cores, started from a shell with no keyboard attached rather than double-clicked | The part of `TEST EXPRESS.bat` that runs the rule checker again when it stops with a Windows error code (never triggered) | The rule checker (`tools/check_rules.py`) was shown to fail on an em dash, a Rust file with no licence header, a script that calls cargo directly, and a script with LF line ends and a non-ASCII byte; its licence and forbidden-crate logic was run on made-up packages | M1 to M8 and everything in the spec: the guide model, smart titles and step builder, drawing, exports, the window, capture, settings, the self-tests and the portable zip |

Not yet run on an ARM64 PC. Not yet run by double-clicking from Explorer. The icon files are copied in but not yet used by any
program (they are for milestone M7).

## What to do on Windows (no typing needed)

1. Double-click **`BUILD EXPRESS.bat`**. It checks that Rust is installed (and explains how to install it if not), builds the
   program, and says plainly whether it worked. The first build may ask before downloading libraries; there are none yet.
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

## Layout on every computer

The spec is `docs\capture-express\spec.md` in the OpenStage repo (`https://github.com/jasonswrench-beep/openstage-author`, branch
`feature/capture-express`). Express reads OpenStage Capture's crates (hooks, drawing, region picker) through a path link, so on
every computer these two folders must sit side by side:

```
somewhere\openstage-author\          the OpenStage repo, branch feature/capture-express
somewhere\openstage-capture-express\ this folder
```

The link is not wired yet (nothing needs it before milestone M3). Choose a folder that OneDrive does not sync, and extract each
new copy into a new, empty folder.

## How the pieces fit

```
crates/
  openstage-express-core/       the guide model, edit operations, undo, smart titles, step builder, drawing (no Windows code)
  openstage-express-export/     HTML, Markdown, JSON, images, GIF, then PDF, Word, PowerPoint (no Windows code)
  openstage-express-windows/    hooks, UI Automation, screenshots, hotkeys (Windows only)
  openstage-express-app/        the program openstage-capture-express.exe
tools/check_rules.py            licence, header, writing-style and script checks
assets/icon/                    the icon pack (SVG, PNG sizes, ICO)
```

All four crates are empty stubs for now.

## Rules that shape the code

- Apache-2.0 only. No GPL or AGPL, no FFmpeg, no libuiohook. `tools/check_rules.py` fails the build on a dependency licence
  outside the allowed list.
- No network in version 1: no HTTP client, no telemetry, no update check. The checker also fails on known network and
  telemetry crates.
- Accessibility is not optional: every feature works by keyboard and with a screen reader, and nothing traps focus.
- Typed text is read from the field, never logged key by key, and never for a password field.
- Nothing is erased by the software. Delete means the trash folder.
- a step-guide program is commercial. Use facts and behaviour only, never its code, templates, strings, artwork or file format.
- Anything Jason runs is a double-clickable `.bat`. Never say something works that has only compiled.

For developers, in a shell: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, `python -I tools/check_rules.py`. `python -I tools/check_rules.py --write-third-party` regenerates
`THIRD-PARTY.md`.

## The icon

`assets/icon` holds the icon pack Jason supplied on 2026-10-31: a navy tile with an indigo viewfinder and a rounded lowercase e.
It replaces the earlier capital E icon, which is no longer used. The SVG's title text contains an em dash; the file is kept
exactly as supplied. The pack's `README.txt` says it was checked without a vector drawing program or Windows themselves, so how the `.ico`
looks in Windows, and how the e reads at 16 pixels, are still to be checked by eye.

## Licence

Apache License 2.0. See `LICENSE` and `NOTICE`.
