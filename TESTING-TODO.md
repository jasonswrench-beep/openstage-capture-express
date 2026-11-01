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
- [ ] PDF, Word, PowerPoint and JPG are not built. See the report for what is needed (a decision about how to make PDFs).
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
- [ ] PDF, Word, PowerPoint and JPG are not built. See the report for what is needed (a decision about how to make PDFs).