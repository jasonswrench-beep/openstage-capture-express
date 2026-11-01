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
folder (`Documents\OpenStage Express\Guides`) with a search box and, once there are two guides, a Sort button that goes round newest first, oldest first, name A to Z and name Z to A (the order is remembered in the settings); each has a Rename and a Delete button (Delete asks first
and moves the guide to a `Trash` folder inside the guides folder, never erasing it), and the Trash button lists what was deleted,
each with a Restore button. **Start capturing** turns the window into a small capture bar that stays on top
while you work in other programs; before Start you choose what to capture: the whole screen, one window (from a list of the open
windows) or an area you draw on a picture of a display (by mouse, or by typing its four numbers); each click and each field you type in becomes a step, with its picture, written to disk as it is
made. **Finish** opens the guide in the editor, where steps can be chosen (one, or several with Ctrl+click, Shift+click, Shift+Up and Down, Ctrl+A or a button, then skipped or deleted together as one undoable change), renamed, moved, skipped, deleted and restored, with undo
and redo, and every change is saved. In the editor the guide can be renamed (a box above the step details), steps can be duplicated, and marks (rectangle, oval, line, arrow, text, step number, highlight, blur,
spotlight) are added with buttons or drawn by dragging on the picture once a shape is chosen (a click places the usual size), listed,
moved and resized by buttons or by dragging, coloured, and deleted, each change one undo; the
whole picture is shown with the focused crop as a frame. **Export** (in the editor) offers every format in a screen of its own and makes them on a
thread of their own, each in a new folder (the folder dialog is Windows' own). **Settings** (on Home) holds the colour scheme, how
clicks make steps, the pointer and circle, the language of step titles, the two folders, the three hotkeys (each written as words,
such as Ctrl+Shift+F7, and changed as soon as the person leaves its box; keys that would take a key from every other program are
refused with the reason), three optional cue sounds (off to begin with, each with a button to hear it) and a few delays; each
change is saved at once to `%APPDATA%\OpenStage Capture Express\settings.json`. **About and licences** shows the licence and what the program is built
from. The window has the program's icon. The program also still makes every export of a made-up guide (`EXPORT DEMO.bat`). The window
has been read back through Windows UI Automation, but not spoken by Narrator or NVDA (see below).

Run on Windows, and working:

- `BUILD EXPRESS.bat`, `TEST EXPRESS.bat`, `EXPORT DEMO.bat` and `OPEN EXPRESS.bat` ran on 2026-10-31 and 2026-11-01 on Jason's
  Windows 11 x64 PC (build 26200, Core i9-13900K), pinned to the efficiency cores, started from a shell with no keyboard
  attached rather than double-clicked. The last full run of the tests: 703 passed (247 in the core, 182 in the exports, 44 in the
  Windows crate, 230 in the app), and the rule checks said PASS. The tests use real files: guide folders were created, saved,
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
  were fixed (the test now closes the list first; a Type step's picture is taken at least 120 ms after the last key).
- Capture of one window and of an area (spec C3) was run for real on 2026-11-01, in a longer run of the same click test that waited
  until the PC was quiet: **all 36 checks passed**, the 25 above (the two earlier failures now passing, 8 steps, 2 screen pictures
  thrown away as too slow to take, no step and no mouse or keyboard action lost) and 10 new ones. In window mode a step's picture was
  exactly the size of the test window's frame, the button was titled `Click "Save"`, and the click was placed inside the picture
  where the button is. After the window was moved with the program running, the pictures were still the window (same size) and the
  next click was placed correctly in them. In area mode (an area drawn around the test window) a picture was exactly the area's
  size, the click was placed inside it and titled from the button's name. No step was lost in either mode. What this run did not
  cover: choosing the window or the area with the mouse or a screen reader (the screens are tested headlessly and were drawn by the
  real program), a window dragged to another display, and menus that hang outside the window's frame (see `TESTING-TODO.md`).
- A hotkey the person changed was proved on this PC on 2026-11-01 by four more checks in the same click test (40 checks in all, all
  passed on a clean run; a first run with the new checks had the four new ones pass but four older ones fail because someone clicked
  in other windows during it, which the saved guide showed). With Ctrl+Shift+Z as the capture-now hotkey, the real `RegisterHotKey`
  heard it when it was pressed with another program holding the keyboard; the usual Ctrl+Shift+F9 was no longer heard once that set
  was replaced; the keyboard hook counted Ctrl+Shift+Z ("redo" in a field) as typing when it was not told the key is a hotkey (the
  control that makes the next check meaningful), and did not when it was told. The run is given the person's own hotkeys when it
  starts. Sixteen deliberate faults in the new code (a rule dropped, a clash not checked, the sound played whatever the setting, the
  listener not told, a cue wrong, the file's keys not checked, and others) were each caught by a test.
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

- Sorting the guides on Home (spec A1): one Sort button beside "Your guides", shown from two guides up, that goes round newest first,
  oldest first, name A to Z (capitals do not matter) and name Z to A, says the new order in the status line, and is remembered in the
  settings (an unknown value in the file means newest first, never an unreadable settings file). Guides that tie keep a steady order, and
  searching, renaming, deleting and restoring all keep the chosen order. Tested through the window and on the ordering itself; nobody has
  used it by hand, and Narrator and NVDA have not read the button (its name is "Sort: newest first. Press to change the order."). Twelve
  deliberate faults were each caught.

- Choosing several steps in the editor (Ctrl+click adds or takes out one, Shift+click or Shift+Up and Down chooses a run, Ctrl+A or the
  "Choose all steps" button chooses every step; the details give way to Skip, Put back, Delete and Choose just one, each done to all
  the chosen steps as one undo; the picture and the marks are left alone, and moving or duplicating asks for one step): tested through
  the window with simulated clicks that hold Ctrl or Shift, and with keys. The steps list is still one stop for the Tab key however many
  are chosen (a test caught every chosen row becoming a stop). Nobody has done it with a real mouse or keyboard, a screen reader has
  not read it, and a person who cannot use Ctrl+click can use Shift+arrows, Ctrl+A and the button, but cannot pick separate steps
  by keyboard alone. Twenty deliberate faults were each caught.

- Renaming, deleting and restoring guides from Home (spec A1): the folder moves and the rename were run for real on this PC, but only
  on made-up guides in temporary folders (a moved guide is byte for byte the same, comes back whole under a free name, is never
  moved from outside the guides folder, and is refused while another running copy of Express holds its lock), and the Home and
  Trash screens are tested through the window with simulated clicks and keys (Delete asks first with "Keep it" ready; Escape and
  Enter work in the rename box). Nobody has used them on a real guide by hand, nothing has been spoken by Narrator or NVDA, and the
  guides folder being inside OneDrive on this PC (so a move is synced) has not been looked at (`TESTING-TODO.md`). Twenty deliberate
  faults in this code were each caught by a test. Home's step count now leaves out skipped steps and notes, as the editor does.

- Drawing a new shape by dragging on the picture in the editor (nine shapes; a click without a drag places the usual size; Escape
  stops it): `core::marks::drawn_mark` turns a drag into a mark (any direction, kept on the picture, a step number's circle across
  the longer side, text taking a wide drag's width) and is tested on any computer; the editor's choice of shape, the shape shown
  while it is dragged and kept as one undoable change when the mouse is let go, the choice used once, Escape, and a drawing dropped
  when another step is chosen or a change is undone are tested through the window with simulated pointer events and keys. Nobody
  has drawn with a real mouse, so the crosshair pointer and the feel of it are unseen (`TESTING-TODO.md`). The nine choices are
  folded behind one heading, so a keyboard user passes one Tab stop; every mark can still be added, moved and resized by buttons.
  Sixteen deliberate faults were each caught.

- The cue sounds (start, stop, step, problem) have never been played. They are made in the core as short quiet tones and held in memory;
  the tests check each is a well-formed WAV file, quiet, short, starts and ends in silence, and has the pitch it should, and that
  each sound plays at the right moment and only when its setting is on (a recording stand-in takes the place of the speaker, so
  nothing is heard from a test). Whether Windows' `PlaySound` plays them, and how they sound, is for a person to try with the
  four "Hear the ... sound" buttons on the Settings screen (`TESTING-TODO.md`).
- Changing a hotkey in the running program: the Settings boxes, the plain-words refusals, saving, and handing the new keys to the
  listener are tested headlessly; letting go of a set of keys and setting another with the real `RegisterHotKey` is tested
  in the Windows crate; the listener thread of the window that does this while the program runs has not been watched by a person.
- Choosing a window or an area by a person: the list of windows and the screen for drawing the area are tested headlessly (a stand-in
  desktop) and were drawn by the real program, and the capture itself ran for real (above), but nobody has used the two screens by
  mouse, keyboard or screen reader on a real desktop, and whether the list of open windows matches what a person sees (which
  windows are listed, which are left out) has not been looked at.
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

Not built: M8 and everything else in the spec: capturing a window that has moved to another display, moving or duplicating several steps at once, a "Save a copy as one file",
panning the focused crop by hand, options on the
Export screen (it offers the formats with their usual options), the language of the program's own words
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
   card under "Your guides"; opening it shows the editor. "Start capturing" opens the capture bar (see item 6 for what it does).

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
the program does not yet do is still to build, not to ask about again. (Changing the hotkeys and the cue sounds were the first of
those to be built after that decision.)
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
- Nothing is erased by the software. Delete means the trash folder, and only the person's "Empty the trash" deletes files (for a
  whole guide there is no such button: it is moved to a `Trash` folder and the person deletes it in Explorer). An
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
