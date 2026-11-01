# OpenStage Capture Express

Free, open-source Windows program that turns what a person does on screen into a clean step-by-step guide: a picture for each
step with a marker where the click was, a title already written (such as `Click "Save"`), light editing, and export. It is meant
to be as easy to use as a step-guide program, with no account and no network. Licence: Apache-2.0. Written in Rust.

It is a sibling of OpenStage Capture (the recorder inside OpenStage Author) and reuses its design and code, but it is its own
small program with its own window.

## Where this stands (honest status)

**Milestones M0 to M6 are built (M6 in its first cut: the whole display only, no window or area picker yet): the guide model,
editing, undo, saving, smart titles, the step builder, drawing of marks, the focused view, exports to a web page, Markdown, JSON,
a folder of pictures (PNG and JPG), an animated GIF, a PDF made by the WebView2 engine that Windows ships, a Word file and a
PowerPoint file that Express writes itself, a window, and (since 2026-11-01) real capture.** Home lists the guides in the guides
folder (`Documents\OpenStage Express\Guides`). **Start capturing** turns the window into a small capture bar that stays on top
while you work in other programs; each click and each field you type in becomes a step, with its picture, written to disk as it is
made. **Finish** opens the guide in the editor, where steps can be chosen, renamed, moved, skipped, deleted and restored, with undo
and redo, and every change is saved. In the editor the guide can be renamed (a box above the step details), steps can be duplicated, and marks (rectangle, oval, line, arrow, text, step number, highlight, blur,
spotlight) are added with buttons, listed, moved and resized by buttons or by dragging, coloured, and deleted, each change one undo; the
whole picture is shown with the focused crop as a frame. **Export** (in the editor) offers every format in a screen of its own and makes them on a
thread of their own, each in a new folder (the folder dialog is Windows' own). **Settings** (on Home) holds the colour scheme, how
clicks make steps, the pointer and circle, the language of step titles, the two folders and a few delays; each change is saved at
once to `%APPDATA%\OpenStage Capture Express\settings.json`. **About and licences** shows the licence and what the program is built
from. The window has the program's icon. The program also still makes every export of a made-up guide (`EXPORT DEMO.bat`). The window
has been read back through Windows UI Automation, but not spoken by Narrator or NVDA (see below).

Run on Windows, and working:

- `BUILD EXPRESS.bat`, `TEST EXPRESS.bat`, `EXPORT DEMO.bat` and `OPEN EXPRESS.bat` ran on 2026-10-31 and 2026-11-01 on Jason's
  Windows 11 x64 PC (build 26200, Core i9-13900K), pinned to the efficiency cores, started from a shell with no keyboard
  attached rather than double-clicked. The last full run of the tests: 573 passed (214 in the core, 182 in the exports, 33 in the
  Windows crate, 144 in the app), and the rule checks said PASS. The tests use real files: guide folders were created, saved,
  locked, crashed and reopened in temporary folders.
- Capture (M6) was run for real on 2026-11-01 through the program behind `TEST EXPRESS CLICKS.bat` (`openstage-capture-express
  test-clicks`, a debug build, started from a shell). It opened a test window of known controls in a second process and, with
  `SendInput`, clicked and typed in it while the real hooks, UI Automation and Windows Graphics Capture ran. 23 of its 25 checks
  passed: a button and a check box were titled from their names (`Click "Save"`, `Click "Check me"`); the click was within two
  pixels of the pointer; text typed in the Email field was read back from the field (`Type "ann@example.org"`); a password field
  made no step with text and the password was in no file of the guide; the capture-now hotkey was heard while another program had
  the keyboard; a click on one of Express's own windows made no step; and a magenta window of Express's own showed in a picture
  until it was marked as excluded from capture and was then absent from the next one, so the capture bar and the editor stay out
  of pictures. The guide it wrote opened again with every step and no problems; each step took about 30 ms to write. Two checks
  failed. The drop-down list's step (`Select "Country"`) came out late, because Windows does not answer questions about a control
  while its list is open. And the Type step's picture showed the text a few letters short, because it was taken at the Enter. Both
  are fixed (the test now closes the list first; a Type step's picture is taken at least 120 ms after the last key), and a second
  run after the fixes is still to be done (see below).
- The program behind `CHECK EXPRESS COMPUTER.bat` (`check-computer`) passed on this PC: Windows 11 build 26200, two displays (the
  second above the main one, at a negative place), a picture of the screen in 572 ms, UI Automation answering, the hooks and the
  three hotkeys available, 1002 GB free, WebView2 156.
- The window (M5) opened from `OPEN EXPRESS.bat` as a release build and closed cleanly. Before that it was drawn in both colour
  schemes by the real program and the pictures were looked at. Its tree of controls was read back through Windows UI Automation
  (the interface screen readers use): each button, the steps list and its options, the headings and the status line have the
  right role and name, and pressing a button that way works. The app's tests drive the same window without opening it: Tab order,
  one stop for the steps list, Up and Down, every shortcut, undo and redo, the live status line, reading order, and colour
  contrast of both schemes (text at least 4.5 to 1, outlines and the focus ring at least 3 to 1).
- The demo's output was checked with programs that are not ours: Windows' own image decoder read the GIF (800 by 450, seven
  frames, three seconds each, looping forever) and the PNGs, and a headless Microsoft Edge drew the web page, which was looked at
  (light colours; headings, contents list, pictures with marks, the blurred name, the list and the link all showed correctly).
- The PDF export ran with the real WebView2 runtime (version 156). Eleven tests in `openstage-express-windows` start the engine and
  print the demo guide: a tagged PDF with the language, headings, lists, links, every picture's alt text and bookmarks; A4 portrait
  and Letter landscape come out at those paper sizes; six layouts (cover, a page for every step, no contents list, Letter
  landscape) each have the right bookmark titles and the right number of pages; a heading in Japanese gets one bookmark; no
  working folder is left behind. The file was also read back with a separate program (pypdf) and its pages were drawn by Windows'
  own PDF renderer and looked at (Roboto text, links, pictures with their marks, "Page 1 of 4" at the bottom right).
- The Word and PowerPoint files of the demo were opened in the real Word and PowerPoint (Microsoft 365, version 16) on 2026-11-01,
  with no repair prompt. Word found the Title and heading styles, one table of contents (and, when asked to update it, produced the
  same entries), five pictures each with alt text, the footer and the A4 size, and drew the pages (looked at). PowerPoint found
  seven 16:9 slides with titles, alt text, slide numbers and speaker notes, and drew the slides (looked at). Windows' own ZIP
  reader and Python's `zipfile` read both files too.

Written and type-checked, not yet run on Windows:

- The part of `TEST EXPRESS.bat` that runs the rule checker again when it stops with a Windows error code (never triggered), and
  the part that lists skipped tests (no skip has happened on this PC).
- `GET OPENSTAGE CODE.bat` downloading (its "not found" and "stop" paths were tried).
- `tools/check-office.ps1`, which repeats the Office checks: its PowerPoint half ran and passed (7 checks); its Word half has
  not run in its final form, because Word was already open when it was tried (the script skips a program that is open, so it
  cannot disturb anyone). The checks it makes were each run before, by hand, in Word.
- Narrator and NVDA with the window: neither has been run, so it is not yet known whether the Narrator and NVDA pass of M5 holds.
  The plan if it fails is Tauri (spec D5). The window letting go of its console box has been checked only in that no console
  window is left among the program's windows, not by watching a double-click. Keyboard feel, 125 to 200 percent scaling and a
  Windows contrast theme are unchecked (`TESTING-TODO.md`).
- PDF export on a computer with no WebView2 runtime: the "not installed" message and the demo's "pdf: skipped" line are tested only
  with a stand-in engine. The paper sizes A3, A5 and Legal, other margins and the header and footer lines are written and their page
  rules tested, but not printed with the real engine.

Tested on any computer (pure logic; run so far only on this Windows PC, nothing in them is Windows specific):

- The model and its round trip with unknown fields kept; every edit operation; undo and redo, with a 300-step random walk on four
  seeds; saving, the lock file, the trash and its pictures, migrations (with made-up versions, since none exists yet) and crash
  states.
- Smart titles in English, German and French; the step builder, with 40 scenario tests of rules R1 to R12.
- Every kind of mark drawn pixel by pixel on OpenStage Capture's canvas; the focused view arithmetic.
- The exports: a golden Markdown file, JSON structure, HTML structure (one heading, no skipped heading levels, alt text, no outside
  requests, the page's security rule naming its one script, every piece of the author's text escaped, colours that pass 4.5 to 1), a
  GIF encoder checked against a separate decoder, and files never replacing each other.
- The Word and PowerPoint exports (every XML part well formed, every relationship and content type resolving, headings, the contents
  field, alt text, notes, language, options, escaping of the author's words) and the ZIP writer they share (read back by a
  separate strict reader: every checksum, size and signature), with the real Word and PowerPoint checks above.
- The JPG encoder (the standard tables, quality scaling, byte stuffing, odd picture sizes, transparency over white), read back by a
  small separate decoder written for the tests, and the demo's five JPG files decoded by Windows' own imaging code (1280 by 720,
  49 to 51 dB from the PNGs at quality 90).
- The PDF's print page (paper, margins, cover, contents, page numbers, header and footer, languages, escaping of the author's
  words in the page and in its style rules, no script, no outside request, Roboto inside the page) and the check of the finished
  PDF (tags, language, bookmarks, alt text, "not a PDF"), with a stand-in for the engine.
- 25 deliberate breakages of the code were each caught by a test (one slipped through the first time and got a better test). Seven
  more were tried on the Word, PowerPoint, ZIP and shared code on 2026-11-01 (a wrong checksum, lost alt text in each, contents links
  pointing at the wrong heading, a slide title that is not a title, no speaker notes, `<` not escaped): every one was caught. Five
  more were tried on the JPG encoder (two zigzag places swapped, the cosine transform scaled wrongly, the DC change not remembered,
  a colour sign flipped, no byte stuffing): every one was caught. Five
  more were tried on the PDF export on 2026-11-01: four were caught by the portable tests and the one that switches tagging off was
  caught by the real-engine tests. Four more were tried on the window on 2026-11-01 (the status line no longer a live region, the
  chosen step no longer marked as chosen, the same message twice made identical, every step a Tab stop): each was caught by the
  test written for it. Several attempts to bring back the doubled bookmark titles (see `docs/guide-format.md`) did not
  bring them back with the Roboto page, so the real-engine bookmark test has not been seen to fail on that fault, only its
  checking function has (against the real doubled titles).

Not built: M8 and everything else in the spec: capturing one window or a drawn area, dragging to draw a new shape, picking several steps, deleting or renaming a guide from Home,
panning the focused crop by hand, options on the
Export screen (it offers the formats with their usual options), changing the hotkeys, sounds, the language of the program's own words
(English only), the program's icon on the `.exe` file in Explorer, the program without a console box (it lets go of it when the window
opens), first-run, WebP, the optional logo on slides, and signing.

PDFs are tagged but have not been run through an accessibility checker, and the page numbers, the header and the footer are not
tagged (the export warns about that). The Word and PowerPoint files have not been run through Word's or PowerPoint's own
accessibility checker or read with a screen reader, and Word and PowerPoint use Calibri without embedding it. Not yet run on an ARM64 PC. Not yet run by double-clicking from Explorer. The dark colour
scheme of the web page has not been seen rendered. The icon files are copied in but not used by any program yet (milestone M7).
`TESTING-TODO.md` lists what only a person can check.

## What to do on Windows (no typing needed)

1. Double-click **`BUILD EXPRESS.bat`**. It checks that Rust is installed (and explains how to install it if not), builds the
   program, and says plainly whether it worked. The first build may ask before downloading libraries.
2. Double-click **`TEST EXPRESS.bat`**. It runs every test, then the rule checks, and says PASS or what failed. It needs Python 3
   for the rule checks. If the checker stops with a Windows error code (not a rule failing) it tries up to three times.
3. Double-click **`EXPORT DEMO.bat`**. It makes a made-up guide and exports it in every format that is built, into a new folder each
   time, and opens it. Open the web page in the `html` folder first.

If the OpenStage code is not next to this folder, the scripts say so; double-click **`GET OPENSTAGE CODE.bat`** (it asks first).

4. Double-click **`OPEN EXPRESS.bat`**. It builds the program if needed (a few seconds when nothing has changed) and opens the
   window. Press Tab to move between controls and Enter to press one. The example guide ("How to submit an assignment") is a
   card under "Your guides"; opening it shows the editor. "Start capturing" says that capturing is not built yet.

5. Double-click **`CHECK EXPRESS COMPUTER.bat`**. It looks at the Windows version, the displays, whether pictures of the screen arrive,
   whether the names of clicked things can be found, whether the hotkeys are free and whether there is room for guides. It clicks
   and types nothing.
6. Double-click **`TEST EXPRESS CLICKS.bat`**. It opens a test window, then clicks and types in it by itself for about thirty seconds
   (it waits until you have left the mouse and keyboard alone) while Express captures, and checks the steps. It takes real pictures
   of your screen, so close anything private first.
7. Double-click **`PACK EXPRESS.bat`**. It builds the program and makes the portable folder and zip in the folder `dist` (the program,
   its licence and notices, and a short guide). It never replaces one that is there.

## Decisions

Made by Jason on 2026-10-31, when the kickoff questions were put to him:

| Question | Answer | What it changes |
|---|---|---|
| Where does it live? | A separate folder with its own git repo, not `capture-express/` inside the OpenStage repo (spec D4 is replaced) | This folder, beside the `openstage-author` clone. Nothing is pushed anywhere, and no remote is set |
| How does it reuse Capture's code? | A path link to the sibling clone | See "Layout on every computer" below |
| Window toolkit? | egui with AccessKit, all Rust (spec D5) | Milestone M5 includes the Narrator and NVDA pass, with Tauri as the fallback |
| First slice? | Capture, edit, **and** PDF, Word and PowerPoint (spec D6, widened) | The first version includes the formats the spec calls S2 |

Made by Jason on 2026-11-01, when asked whether to keep the spec's defaults for its open questions (4 to 12 in section 16): **yes, keep
them.** So the name stays OpenStage Capture Express, a guide stays a folder named *.osguide, typing is read from the field and never
logged key by key, descriptions are plain text with a small Markdown subset, the hotkeys are Ctrl+Shift+F7, F8 and F9, step titles come in
English, German and French, and the link to OpenStage Author, REA and signing stay later decisions. Anything the spec proposed that
the program does not yet do (changing the hotkeys, for one) is still to build, not to ask about again.
a step-guide program is installed on Jason's computer (`C:\Program Files\a step-guide program`) and is the reference program; its behaviour may be looked at
there, but never its code, templates, strings or artwork.

## Layout on every computer

The spec is `docs\capture-express\spec.md` in the OpenStage repo (`https://github.com/jasonswrench-beep/openstage-author`, branch
`feature/capture-express`). Express reads OpenStage Capture's crates (drawing, text layout, the edit types) through a path link, so
on every computer these two folders must sit side by side:

```
somewhere\openstage-author\          the OpenStage repo, branch feature/capture-express
somewhere\openstage-capture-express\ this folder
```

The link is in use since milestone M3. `BUILD EXPRESS.bat`, `TEST EXPRESS.bat` and `EXPORT DEMO.bat` check that the folder is there
and, if not, say so; `GET OPENSTAGE CODE.bat` copies it from GitHub (it asks first, and needs Git). Choose a folder that OneDrive
does not sync, and extract each new copy into a new, empty folder.

## How the pieces fit

```
crates/
  openstage-express-core/       the guide model, edit operations, undo, saving, smart titles, step builder, drawing, focused view
  openstage-express-export/     HTML, Markdown, JSON, pictures, GIF, the PDF's print page, Word and PowerPoint (with a small ZIP
                                writer); a made-up demo guide; the Roboto font files for the PDF (fonts/)
  openstage-express-windows/    Windows only: the PDF engine (WebView2); the mouse and keyboard hooks, UI Automation, pictures of the
                                screen, hotkeys and the capture run that ties them together; the test window and synthetic input
  openstage-express-app/        the program openstage-capture-express.exe: the egui window (Home, the capture bar and the editor, in src/ui),
                                the capture bar and the saving of captured steps, the computer check, the click test,
                                and the command-line export demo
docs/guide-format.md            the guide format and the exports: the source of truth, kept equal to the code
tools/check_rules.py            licence, header, writing-style and script checks
assets/icon/                    the icon pack (SVG, PNG sizes, ICO)
TESTING-TODO.md                 what only a person can check
```

## Rules that shape the code

- Apache-2.0 only. No GPL or AGPL, no FFmpeg, no libuiohook. `tools/check_rules.py` fails the build on a dependency licence
  outside the allowed list.
- No network in version 1: no HTTP client, no telemetry, no update check. The checker also fails on known network and
  telemetry crates. Exported web pages make no outside requests either.
- Accessibility is not optional: every feature works by keyboard and with a screen reader, and nothing traps focus.
- Typed text is read from the field, never logged key by key, and never for a password field.
- Nothing is erased by the software. Delete means the trash folder, and only the person's "Empty the trash" deletes files. An
  export never replaces a file that is already there.
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
