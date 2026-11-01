# Things only a person can check

A running list. Nothing here blocks the build: each item says what to look at and why a script cannot do it. Add to this list as
milestones are built; tick an item (and say on which PC and when) once it has been looked at.

## Milestone M0 and M1 (build skeleton, guide model, saving)

- [ ] Double-click `BUILD EXPRESS.bat` and `TEST EXPRESS.bat` from Explorer (not from a shell). Both have only been run through a
      shell with no keyboard attached. Look for: the window stays open until a key is pressed, the PASS or FAIL text is easy to read.
- [ ] Run both scripts on an ARM64 Windows PC. They have only been run on an x64 PC (Core i9-13900K, Windows 11 build 26200).
- [ ] Look at the icon in `assets/icon` at 16, 32 and 48 pixels (the `.ico` in Explorer, once a program uses it). The pack's own README
      says it was checked without Windows or a vector drawing program. Does the lowercase e still read at 16 pixels?
- [ ] The retry in `TEST EXPRESS.bat` (the rule checker is run again, up to three times, if it stops with a Windows error code instead
      of PASS or FAIL) has never been triggered. It matters on a PC whose processor makes Python crash unpredictably.
- [ ] Read `docs/guide-format.md` once and say whether the words and the layout of a guide folder are what you expect to see in
      Explorer (folder name `Title.osguide`, `images`, `trash`).

## Milestone M2 (smart titles and the step builder)

- [ ] Have a German speaker and a French speaker read the titles in `crates/openstage-express-core/src/titles.rs`. The spec proposed
      most of them; the ones for a middle click, for "here" with any verb but Click, and for "Type here" are this program's own
      proposals.
- [ ] Compare with a step-guide program on the same program (a step-guide program is installed on Jason's PC): does it say Select for dropdowns and radio
      buttons, Click for check boxes, and Type for a field you type in? Only its behaviour may be compared, never its code.
- [ ] Say whether these choices, where the spec is silent, are right (they are listed in the comment at the top of
      `builder.rs`): a plain click waits half a second in case it becomes a double-click; Escape ends a typing step like Enter and
      Tab; typing without a click first still makes a Type step, named from the focused field; any click, even one that is not
      captured, ends a typing step.

## Milestone M3 (drawing the marks, the focused view, finding the OpenStage folder)

- [ ] Look at the marks as drawn, once `EXPORT DEMO.bat` exists (milestone M4): rectangle, oval, arrow, highlight, spotlight, number badge,
      click mark, and the three pointers (pointer, hand, I-beam). The tests prove the pixels, not whether they look good. The hand
      and the I-beam are this program's own simple artwork and have only been checked by their size and position.
- [ ] Look at a blur on real small print: is a block a third of the shorter side (at least 12 pixels) enough to make it unreadable at
      the size it is shown? The tests prove no detail survives inside a block, not that the block is big enough for your text.
- [ ] Text marks use block letters in the tests. Windows' own text drawing comes with milestone M6; check text marks then, in English,
      German, French and one language with accents or other scripts.
- [ ] Double-click `GET OPENSTAGE CODE.bat` on a PC that has no `openstage-author` folder next to Express. The "not found" and the
      "stop without typing Y" branches were tried through a shell; the download itself (about 70 MB from GitHub with Git) has not been
      run. It may ask for a GitHub sign-in if the repository is private.
- [ ] The automatic focused view is 150 percent closer, centred on the click. Does that feel right on your programs? (The
      percentage is a constant, `DEFAULT_ZOOM_PERCENT` in `focus.rs`; a step-guide program's manual does not give its own number.)

## Milestone M4 (exports)

- [ ] Open the web page from `EXPORT DEMO.bat` (the `html` folder) and read it the way a reader would. Does the order, the numbering
      ("2.1.") and the wording feel right? Does the page look clean?
- [ ] Look at the web page in the dark scheme (set Windows to dark mode, then open it). The dark colours pass the 4.5 to 1 test in the
      code, but a headless browser would not show them, so they have not been seen.
- [ ] Use the web page with a screen reader (Narrator and NVDA): the skip link, the contents list, the headings, the picture alt
      text, and opening and closing a picture (Escape, and focus coming back). It has only been checked for structure.
- [ ] Zoom the web page to 400 percent and make the window narrow (320 pixels). Does it reflow without side scrolling?
- [ ] Print the web page (or print it to PDF). Does each step stay together and the pictures stay whole?
- [ ] Open the Markdown file in a Markdown viewer you use (the `markdown` folder), and the JSON in anything that reads it.
- [ ] Open the GIF in a browser and in a program you use. Windows' own decoder reads it, but it has not been seen playing. Is three
      seconds a step about right? Does the progress bar help? (The demo turns on the title card and the progress bar.)
- [ ] The demo's screenshots draw their words as blocks. Real text drawing arrives with capture (milestone M6).
- [ ] Open a few of the `jpg` folder's pictures next to the PNGs. At quality 90 the marks and text should look as sharp. Try
      quality 50 and 70 (not offered anywhere until the window exists, milestone M5): are the marks still readable? JPGs of the
      flat demo screenshots are larger than the PNGs; check a real screenshot with a photograph in it.

## Milestone M5 (the window)

Double-click `OPEN EXPRESS.bat`. It builds the program if needed and opens the window on Home, with an example guide ("How to
submit an assignment") to open from the card under "Your guides".

- [ ] Run the Narrator and NVDA pass. Only when you are away from the computer (NVDA logs everything it speaks) and only on your go.
      The window has been read back through Windows UI Automation (roles, names, which control can be invoked, Invoke works), which
      is what a screen reader reads, but no screen reader has spoken it. Check, on Home and in the editor: each control is named
      and its role is right; Tab goes through the controls in a sensible order and never gets stuck; the steps list is one stop and
      the Up and Down keys move through it, each step is said with its number and title and whether it is chosen; the line at the
      foot is said when a message appears ("Opened the example guide", "Back at your guides"); the same message twice in a row is
      said twice. If this pass fails, the plan is to move the window to Tauri (spec D5).
- [ ] Use the keyboard only, no mouse, on Home and in the editor: Tab and Shift+Tab, Enter on a button or card, Up and Down in the
      steps list, Alt+Up and Alt+Down to move a step, Ctrl+Shift+Delete to delete one, F2 to edit the title, Ctrl+Z and Ctrl+Y.
      The tests send these keys to the window, but how fast and how natural they feel only a person can say. Is the focus ring
      always easy to see?
- [ ] Look at the window in dark and light (Windows Settings, Personalisation, Colours), at 125, 150 and 200 percent display
      scaling, and with a Windows contrast theme on. The colours pass the contrast tests in the code (text at least 4.5 to 1,
      outlines and rings at least 3 to 1), and the window has been seen as a picture in both schemes at 100 percent, but not at the
      other scalings or in a contrast theme. egui does not follow a Windows contrast theme by itself; say if it is unreadable.
- [ ] The two side panels cannot be dragged to a different width (the spec says they are resizable). A drag handle is a mouse-only
      control, so none was built; say whether a keyboard way to change the widths is wanted.
- [ ] A black console box may show for a moment behind the window when the program is started by double-clicking. The program lets
      go of its console as soon as the window opens; the first moment before that has not been watched. A proper fix (a program
      with no console at all) comes with the packaging milestone (M7).
- [ ] The window uses Segoe UI from your Windows fonts folder (Tahoma, then Arial, then the bundled Roboto if it is missing).
      Look at the text: is it sharp, and does it look like a Windows program? egui draws its own text, so it will not look exactly
      like Notepad.
- [ ] The window has no icon yet (the icon files are used only from milestone M7). The title bar and the taskbar show the default.
- [ ] Double-click the finished `openstage-capture-express.exe` in `target\release` from Explorer, on an ARM64 PC, and on a PC with an
      older graphics driver (the window draws through wgpu, which chooses the graphics interface itself). It has been run only on
      this PC.
- [ ] The editor does not yet let you draw marks, blur a name, crop, zoom or add notes; there is no tool row. Those come after capture.
      Home's "Start capturing" says it is not built yet.

## Milestone M6 (capture)

Double-click `CHECK EXPRESS COMPUTER.bat` first, then `TEST EXPRESS CLICKS.bat`, then try a real capture with `OPEN EXPRESS.bat`.

- [ ] Do a real capture by hand: Start capturing, press Start, do a task of your own in two or three programs (a browser, Word, File
      Explorer), press Finish. Do the titles read well? Do the pictures show what you saw, with the menu or tooltip still open?
      Is the marker where you clicked? This is the main thing only a person can judge; the self-test uses a small test window.
- [ ] The first run of `TEST EXPRESS CLICKS.bat` as a double-click (it has only run through a shell). It takes about thirty seconds,
      moves the pointer and sends keys; it waits until you are still. Look at the folder `clicks-test-output`: do the pictures show
      the test window, the typed text, and no magenta rectangle (Express's own stand-in window)?
- [ ] Try the hotkeys while another program has the focus: Ctrl+Shift+F7 starts and finishes, F8 pauses and resumes, F9 captures
      now. They were tested by sending the keys from the test, not by a person at a keyboard. If another program holds one, the
      status line says which; check that message makes sense.
- [ ] The capture bar: is it where you expect (top right of the screen), does it stay on top, is it in the way, does Tab reach its
      buttons? The window changes size and place when the bar opens and closes; look for flicker, and that the editor window
      comes back where it was.
- [ ] Narrator and NVDA with the capture bar and the step announcements ("Step 4 captured: Click Save."). Only when you are away
      from the computer (NVDA logs everything it speaks) and only on your go. The status line is a polite live region, tested
      through the accessibility tree, but nothing has spoken it.
- [ ] Two displays: capture with the main display while working on the second one. This PC has two (the second sits above the
      main one, at a negative place); the self-test clicks on the main display only. Clicks outside the captured display still end
      a typing step but make no step. Is that what you want, or should capturing follow the display you are working on?
- [ ] Windows with a different display scaling (125, 150 or 200 percent): are the marker and cursor the right size, and is the click
      in the right place? The marks scale with the display (tested with a made-up scaling); the click test ran at 100 percent.
- [ ] A program running as administrator: Windows does not tell an ordinary program about clicks in an administrator's windows (spec
      K6), so no step is made. Try a click in Task Manager or a program started as administrator and see what happens.
- [ ] Typing with an input method (Japanese, Chinese, Korean) and into a web page's fields: the text is read from the field when
      typing finishes, which has only been tried with a plain Windows edit field. Some programs give no text; the step then says
      `Type in "Name"`.
- [ ] A drop-down list or menu that is open when you click: Windows' accessibility information does not answer until the list is
      closed, so the step appears a moment late (its picture and place are right). Does a delay of a second or two bother you?
- [ ] Antivirus and SmartScreen: the hooks that watch the mouse and keyboard can make an antivirus warn on a program that is not
      signed (spec K3). Nothing has warned here; try on another PC with an antivirus.
- [ ] Not built yet in capture: capturing one window or a drawn area, the screenshot delay and capture-mode choices (they are in the
      engine and tested, but no setting reaches them), sound cues, cancelling after starting (Finish keeps the steps), HDR and
      protected content.
- [ ] Open a guide in the folder `Documents\OpenStage Express\Guides` after a capture: is that the place you want guides kept? Is
      "Guide 2026-11-01 09.38" a good first title? The editor cannot rename a guide yet (steps only), so every guide keeps it.

## PDF export (WebView2)

- [ ] Open the PDF from `EXPORT DEMO.bat` (the `pdf` folder) in a desktop PDF reader and in Edge. It has been looked at only as pictures drawn
      by Windows' own PDF renderer and read back with a separate program (pypdf), on this PC. Check the type, that the contents
      entries are links that jump to their step, that the pictures are sharp, and that "Page 1 of 4" sits at the bottom right.
- [ ] Run an accessibility checker you use on the PDF (a desktop PDF program's Accessibility Check, or PAC). The PDF is tagged (language, headings,
      lists, links, alt text, bookmarks) but no checker has been run. Expect the page numbers to be reported as untagged content: the
      engine draws them in the margin and does not tag them (the export warns about this).
- [ ] Read the PDF with a screen reader (NVDA or Narrator in a desktop PDF reader or Edge): headings, the contents links, and the picture
      descriptions. Only when you are away from the computer: NVDA logs everything it speaks.
- [ ] Try PDF export on a PC with no WebView2 runtime (a Windows 10 PC without Edge or Microsoft 365). Expect `EXPORT DEMO.bat` to say
      "pdf: skipped" and still make everything else. The "not installed" path has only been exercised with a stand-in engine.
- [ ] The paper sizes A3, A5 and Legal, Letter portrait, other margins, the cover page and the header and footer lines are written and
      their page rules are tested, but only A4 portrait, Letter landscape, the cover and one page per step have been printed with the
      real engine. They have no way to be chosen until the window exists (milestone M5).
- [ ] After a Windows or WebView2 update, run `TEST EXPRESS.bat` again. The tests print every layout with the real engine and check
      the bookmark titles; a runtime update that brings back the doubled titles (see `docs/guide-format.md`) would fail them.
- [ ] Has only been tried on a guide of six steps. Try one of a hundred or more for time and memory; the export gives up after seven
      minutes.
- [ ] Roboto (Apache-2.0, version 2.137) was copied from the fonts installed on your PC into `crates/openstage-express-export/fonts`
      and is recorded in `NOTICE`. Say whether you are happy to ship it, or whether it should come from Google Fonts instead.

## Word and PowerPoint exports

- [ ] Open the Word file from `EXPORT DEMO.bat` (the `word` folder) yourself. It has been opened by a script in Word 16 and its
      pages looked at as pictures, but not by a person. Does it read well? Right click the contents list and choose Update field:
      the entries should stay the same.
- [ ] Open the PowerPoint file (the `powerpoint` folder) and play it as a slide show. Do the pictures, the first lines of text and
      the speaker notes (press S in the show, or open Notes) feel right? Is two lines of text under a picture the right amount?
- [ ] Run Word's Check Accessibility and PowerPoint's Check Accessibility on the two files. Neither has been run. The headings,
      alt text, slide titles, reading order and document language are all set, but no checker has confirmed it.
- [ ] Read both files with a screen reader (NVDA or Narrator), in Word and in PowerPoint. Only when you are away from the
      computer: NVDA logs everything it speaks.
- [ ] Open the files in a program other than Microsoft's (LibreOffice, Google Docs or Slides, Pages or Keynote on a Mac). They have
      only been opened in Microsoft 365 Word and PowerPoint, so how others show the contents list, the footer and the slide
      layout is not known.
- [ ] Both files use Calibri without embedding it. On a computer without Calibri (some Linux machines) the text will use another
      font. Roboto, as the spec suggests, is not embedded in either yet. Say whether that matters.
- [ ] The page-per-step, A3, A5 and Legal options of the Word export, and the cover, agenda and footer options of the PowerPoint
      export, are written and tested as XML, but only the default options have been opened in Word and PowerPoint.
- [ ] `tools/check-office.ps1` repeats the Office checks. Its Word half has not run in its final form (Word was open when it was
      tried): close Word, run `powershell -NoProfile -ExecutionPolicy Bypass -File tools\check-office.ps1` from the Express folder
      after `EXPORT DEMO.bat`, and tell me if any line says FAIL.
- [ ] The optional logo on slides (spec section 10.2) is not built.
