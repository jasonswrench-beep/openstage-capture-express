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
- [ ] Check the generated wording in a sample program. Does it say Select for dropdowns and radio
      buttons, Click for check boxes, and Type for a field you type in?
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
      percentage is a constant, `DEFAULT_ZOOM_PERCENT` in `focus.rs`.)

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
- [ ] The made-up screenshots in the demo draw their own words as blocks, because they are made up. The words on marks (a text mark,
      a step number) are real Windows text now: add a text mark and a step number to a step of a real guide, and look at them in the
      editor and in an exported picture. Are the size, the font (Segoe UI) and the colours right? Is there any way to choose a font?
      (There is none yet.)
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
- [ ] Marks in the editor (rectangle, oval, line, arrow, text, step number, highlight, blur, spotlight): add some to a captured step and
      use them by keyboard alone (the Add buttons, the marks list with Up, Down and Delete, the Move and size buttons, colours and widths)
      and by mouse (press on a mark and drag it, drag a square handle to change its size). The window shows the whole picture with the
      focused crop as a frame; is that clear? Blur especially: blur a name on a real screenshot, export a PNG, and check it cannot be read
      (it is a coarse mosaic, not a black box; use a solid rectangle to hide something that must stay secret).
- [ ] Draw a shape with the mouse: under Marks, open "Draw a shape with the mouse", choose one (Draw a rectangle, for example), and
      drag on the picture. The shape should follow the pointer while you drag and be kept when you let go (one Undo takes it back);
      the choice is used once. A click without a drag should place the usual-sized shape where you clicked. Escape should stop a
      drawing in progress, and again turn the choice off. The crosshair pointer, and whether dragging a thin line or a step number
      feels right, only a person can say. The tests send the same pointer events through the window without a mouse; nobody has
      done it by hand. Narrator and NVDA: the heading folds open by keyboard and each choice is named ("Draw an oval"); Add and
      Move still do everything by keyboard.
- [ ] Choose several steps by hand (on a guide you can spare): Ctrl+click adds or takes out a step, Shift+click chooses a run, and
      with the keyboard on the list Shift+Up and Shift+Down lengthen or shorten a run and Ctrl+A chooses every step ("Choose all steps"
      does the same by button). The right-hand side should change to "Several steps chosen" with Skip, Put back, Delete and Choose just
      one; one Undo should take back a whole skip or delete. Does it feel right that the picture stays on one step, and that you cannot
      pick separate steps by keyboard alone (only runs, all, or Ctrl+click with a mouse)? Narrator and NVDA: each chosen row should be
      read as selected, and the status line says how many are chosen.
- [ ] Move and copy several steps by hand (on a guide you can spare): with a few chosen, Alt+Up and Alt+Down (or "Move these steps up"
      and "Move these steps down") move them together; "Duplicate these steps" copies them and chooses the copies. Try a run of steps
      next to each other (the copies should come as a block straight after the run), steps that are apart (each copy straight after its
      own step), and a step chosen with its own substeps (copied once, with them). Is the block-after-the-run rule for a run what you
      expect, or would you rather every copy sat straight after its own step? One Undo should take back the whole move or all the
      copies. A single step now has "Move step up" and "Move step down" buttons too. Narrator and NVDA: the new buttons and the status
      line ("Moved 2 steps down.", "Cannot move these steps down any further.") should be read.
- [ ] Deleted steps (on a guide you can spare): the "Delete step" button (beside Duplicate step) and Ctrl+Shift+Delete delete the
      chosen step, and a "Deleted steps (1)" button then appears after "Choose all steps". It opens a screen with a Restore button for
      each step (and "Restore all" from two up); Restore puts the step back where it was and chooses it, and "Back to the steps"
      returns to the list with the keyboard on it. Close the guide and open it again: the step should still be there to restore, with
      its picture. Look in the guide's folder: the picture of a deleted step should be in `trash` and be back in `images` after
      Restore. Does it feel right that the list is a screen of its own, that the button only appears once something is deleted, and
      that a step with substeps comes back without them (they stay where they moved to)? Narrator and NVDA: the button's count, the
      heading "Deleted steps", each button named "Restore Click "Save"", and the status line after a restore should be read.
- [ ] Empty the trash (on a guide you can spare, with a few steps deleted): the "Empty the trash" button at the foot of the Deleted
      steps list asks "Empty the trash for good?" with "Keep them" ready; Enter or Escape keeps everything. "Empty the trash for good"
      deletes the steps and their pictures from the guide's folder (look in `trash`: the pictures should be gone, and the pictures of
      the steps that are left still in `images`), and Undo can no longer go back past it. This is permanent: the pictures do not go to
      the Recycle Bin (decided 2026-11-01: for good, not the Recycle Bin). Does the question say enough? Does it matter
      that Undo is lost for the steps deleted before as well as the deleted ones? Narrator and NVDA: the question's heading, both
      buttons, and the status line after emptying should be read. Not tried: a picture another program has open (the words are
      tested with a locked file, on this PC only).
- [ ] Move and zoom the focused crop by hand (on a step with a click): turn Focused view on, then try Move crop left, right, up and
      down, Zoom crop in and out, and Reset crop; the frame over the picture should follow, and the words above the buttons say where
      it is. Drag the picture with the mouse to move the crop (the frame should follow the pointer and stay inside the picture); drag
      a mark to move the mark, and hold Space to move the crop over a mark. Export the guide and check that the pictures follow the
      crop you set. Is a tenth of the crop the right size for one press, and would you like a mouse wheel or keys for zooming? One
      Undo should take back each press and each drag. Narrator and NVDA: the words above the buttons, the seven buttons and the
      status line ("Moved the crop right. The crop shows ...") should be read.
- [ ] Save a copy as one file (on a guide you can spare, with a blur on one step and one step skipped): on the Export screen leave
      "Include the original pictures" off and press "Save a copy as one file". Open the new `...osguide.zip` in Explorer: it should hold
      `guide.json`, `README.txt` and `images`, and no `trash`. Then on Home press "Open a copy", paste the file's place (with the
      quotation marks Explorer's "Copy as path" adds) and open it: you get a new guide with the marks drawn into the pictures, no
      marks to edit, the skipped step missing and the blur unable to be undone. Do it again with the original pictures ticked: the
      blur should be removable, and the skipped step there. Try Browse (Windows' own file dialog, never run by anyone yet). Try the
      copy on another PC, and try a zip that is not a copy (it should say so, in words, and make nothing). Narrator and NVDA: the
      Export section, the Open a copy screen and its messages should be read. Does the default (marks drawn in, skipped steps left
      out) match what you would want to send someone?
- [ ] The focused view for a whole guide (on a guide you can spare, with a few steps): under the guide's title the line says how many
      steps have the focused view; press "Focused view off for every step" (every picture should show whole, and the frame over the
      picture goes), then "on" (the usual crops come back). Move one step's crop by hand and press "on" again: your crop should be kept.
      Is it right that "on" only fills in steps that have none, and that "off" throws away hand-set crops (Undo brings them back)?
      Capture a new guide with "Start new steps with a close-up of the click" off in Settings and look at the line: it should say
      new steps start with it off. Narrator and NVDA: the line and the two buttons, and the status line after each press, should be read.
- [ ] Not built in the editor yet: cutting the picture.
- [ ] Rename and delete guides from Home, on guides you can spare (copy a guide's folder to a spare guides folder first, and set
      Settings, Folders to it): Rename opens a box with the name, Enter saves, Escape cancels; Delete asks first with Keep it ready,
      then the guide leaves the list and its folder is in `Trash` beside the guides; Trash lists it and Restore brings it back and it
      opens with all its steps. Does it feel right that each guide has three Tab stops (the card, Rename, Delete)? Is Trash easy
      to find? Try it while the guide is open in a second copy of Express: both should say it is in use. With the guides folder in
      OneDrive (it is on this PC), the move syncs like any other: check OneDrive shows the folder in `Trash` and not in both places.
      Home's step count now leaves out skipped steps and notes, as the editor does (it used to count them).
- [ ] Sort the guides on Home (with two or more): the Sort button goes round newest first, oldest first, name A to Z and Z to A; the list
      changes at once and the status line says which. Is the order what you expect for your own guides (capitals ignored; guides
      changed in the same second are by name)? Close and open the program: is the order kept? Is one button that cycles clear enough, or
      would you rather see all four choices?
- [ ] Narrator and NVDA on Home with real guides, only when you are away and on your go: each button is named for its guide
      ("Rename Submit an assignment"), the question's heading and the status line are read, and the Trash screen reads its list.

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
- [ ] Capture one window by hand: on the capture bar choose One window, Choose window..., pick a program you use, Use this window, Start.
      (The click test already did this with its test window on 2026-11-01, including a moved window, and passed; what is left is a real
      program, the choosing screens and how it feels.) Click inside it and open a menu or a drop-down list. A menu that lies inside the window's frame stays in the picture; one that hangs
      outside the frame is outside the area, so a click in it makes no step (spec K8 chose cutting the display, not capturing the window
      alone, so menus still show; say whether clicks in menus that leave the frame should count). Move the window and click again: the
      pictures should follow it. Close it: capturing should stop and keep the steps. Drag it to your second display: steps should
      be left out, with a message (the window is only captured on the display it was on when you started).
- [ ] Capture an area by hand: choose An area I draw, Draw the area..., drag on the picture to draw, to move and to resize, try the number
      boxes and the buttons, then Use this area and Start. Is the picture of the display clear enough to place the area? (It shows
      the screen without Express's own windows.) Is the area what you meant in the pictures?
- [ ] The window list and the area screen with Narrator and NVDA, only when you are away and on your go: the list is read row by row with
      Up and Down, the number boxes are named, the picture has words with the area's numbers.
- [ ] Change a hotkey by hand: Settings, the Hotkeys boxes. Write a new key in "Capture now", such as Ctrl+Shift+Q, and press Tab: the
      status line should say "Saved." and name the keys. Then, while another program has the focus and a capture is running, press
      the new key (a step should be made) and the old one (nothing). Try some that should be refused (F7 alone, Ctrl+C, Ctrl+Alt+K,
      Alt+F4, F12, two jobs the same key, "banana"): is each reason clear? Try a key another program holds (the status line should
      say it is taken). Close and open the program: are the keys kept? "Use the usual hotkeys" puts the three back.
- [ ] Hear the sounds, only when you want to: Settings, Sounds, the four "Hear the ... sound" buttons. Nothing in the tests has ever
      played one. Are they quiet enough, different enough from each other, and pleasant (start rises, stop falls, step is a short
      soft note, problem is a low double note)? Then turn each on and capture: do they come at the right moments (start and resume,
      pause and finish, each step, each problem)? Does a quick run of steps become a drone?
- [ ] The hotkey boxes and the Sounds check boxes with Narrator and NVDA, only when you are away and on your go: each box is named
      ("Start or finish" and so on), a refusal is read from the status line, and Tab and Enter leave a box.
- [ ] Not built yet in capture: the screenshot delay and capture-mode choices (they are in the
      engine and tested, but no setting reaches them), cancelling after starting (Finish keeps the steps), HDR and
      protected content.
- [ ] Open a guide in the folder `Documents\OpenStage Express\Guides` after a capture: is that the place you want guides kept? Is
      "Guide 2026-11-01 09.38" a good first title? Rename it in the editor (the Guide title box) if you like.

## Milestone M7 (so far: the Export screen)

- [ ] OneDrive: on this PC Documents is inside OneDrive (`C:\Users\Jason\OneDrive\Documents`), so the default guides folder and the
      default exports folder are both inside OneDrive and their pictures (screenshots of your screen, about 0.7 MB each) would be
      uploaded to it. Is that what you want? There is no settings screen yet to move them; until there is, set the environment
      variable `EXPRESS_GUIDES_FOLDER` to another folder to keep guides elsewhere.
- [ ] The Export screen by hand: open a guide you captured, press Export, choose a few formats, press `Choose folder...` (Windows' own
      folder dialog: has never been opened by a person; does it appear in front, start in the right place, and work with the keyboard
      and Narrator?), press Export. Does the window stay usable while a PDF is made? Is the new folder opened? Open each file.
- [ ] The export options (GIF speed and size, PDF paper and margins, Word and PowerPoint choices) exist in the code and are tested,
      but the Export screen still offers only the formats with those options at their usual values. (The picture width, the JPG
      quality and a preview of the first steps are on the screen now: see the next item.) Say which of the others you want on the
      screen.
- [ ] The picture choices on the Export screen (2026-11-01, written and tested, never used by hand). Open a guide of eight or more
      steps that you captured, press Export, and between the formats and the folder you should see "How wide the pictures are"
      (six widths, 1280 the usual), "Quality of the JPG pictures" (100, 90, 70, 50 and another number in a box) and "Try it on a
      few steps first" (a tick and a box, 5 to begin with). Choose width 640, tick Pictures (JPG), choose quality 50, tick the
      preview and type 3, and export. Check: the status line and the report say it is a preview; the new folder is called
      "... (preview of 3 steps)"; there are three pictures, 640 across; the web page, Word, PowerPoint and PDF files (if you also
      choose them) have "Preview, first 3 of N steps - " before the guide's title. Then export again at quality 100 with the preview
      unticked: the folder has no "preview" in its name and every step is there. Questions for you: is a JPG at quality 50 good
      enough for text in your screenshots (compare it with 100)? Do the short words on the screen ("640", "100 (best picture)")
      and the longer names a screen reader says ("Picture width 640 pixels", "JPG quality 100, best picture") feel right with
      Narrator? Is typing into the quality box and the steps box easy by keyboard? Should a preview rename the guide's title in
      the files (it does now, so a preview is never taken for the whole guide) or only the folder?
- [ ] JPG inside the web page, Word, PowerPoint and PDF (2026-11-01, written and tested; the real PowerPoint opened the JPG
      presentation and the real PDF engine made a PDF with JPEG pictures, both by script; the real Word was NOT tried, because it was
      already open on this PC and the check leaves an open Word alone; nobody has used it by hand). On the Export screen, between
      "How wide the pictures are" and "Quality of the JPG pictures", choose JPG under "Pictures inside the web page, Word,
      PowerPoint and PDF", choose a quality (90 is the usual), tick those four formats and export. Then export again with PNG.
      Check: (1) Open the Word file in Word. Does it open without a repair message, are the pictures all there with their alt
      text, and are they crisp enough? (2) Compare the file sizes of the two runs with a guide of your own. In my two samples of
      this program's own screens JPG at 90 was 40 to 50 percent of the PNG; the demo guide's flat blocks come out about twice as big
      as JPG, because PNG is very good at flat colour. If your screens come out bigger as JPG, say so. (3) Zoom a PDF page to 200
      percent and look at the text in a picture at quality 90 and at 70. (4) In the web page, press a picture: does the larger
      view open? (5) Quit and reopen the program: is your choice remembered?
- [ ] "Whichever is smaller for each picture" (2026-11-02, written and tested; you said 'Yes, build it'). It is the third choice
      under "Pictures inside the web page, Word, PowerPoint and PDF". For each picture it makes the JPG (at the quality below) and
      keeps it only when that file is smaller than the PNG; otherwise the picture stays an exact PNG. Check: (1) Export a guide of
      your own as the web page and Word three times, with PNG, JPG and "whichever is smaller", and compare the three file sizes.
      The smaller choice should never be bigger than either of the other two. (2) Open the Word file made with the smaller choice:
      do the pictures all show, with no repair message, even though some are PNG and some JPG? (Rename the .docx to .zip and look
      in word\media: the busy screens should be .jpg and the plain ones .png.) (3) Choose it, quit and reopen the program: is it
      still chosen? (4) With a keyboard alone, Tab to the three choices and press Space on each; with Narrator or NVDA (only when
      you say so), does each one say what it is a choice of, for example "Pictures inside the web page, Word, PowerPoint and
      PDF: whichever is smaller for each picture, PNG or JPG"? Question for you: is the shown label "Whichever is smaller for each
      picture" clear, or would you rather it said "Smallest file"?
- [ ] Settings by hand: open Settings from Home, change the colour scheme (does the window change at once, in both schemes, with a
      Windows contrast theme?), the capture mode and key, the language of step titles (do German and French titles read well?), and
      the two folders (the folder dialog again). Then start a capture and check the choices took effect. The choices reach the capture
      through tests with a stand-in; only the colour scheme and the folders have been tried in the real window by the tests' own
      pictures.
- [ ] About and licences: read it. Is the licence text the one you want shown, and are the credits right (Roboto, the libraries)?
- [ ] The icon: look at the title bar and the task bar while the window is open. The icon is in the window, and `PACK EXPRESS.bat`
      and `MAKE EXPRESS INSTALLER.bat` now also put it on the program file they ship (the program in `target\` still has none, so
      `OPEN EXPRESS.bat` shows the plain one in Explorer). Look at the shipped program in Explorer at the large, medium and small
      sizes, and at the Start menu entry, the desktop shortcut and the Settings, Apps entry.
- [ ] The portable zip (`PACK EXPRESS.bat`): unzip it on another PC (not this one), double-click the program, read START-HERE.txt,
      and see what SmartScreen and any antivirus say about a program that is not signed. This PC has only unzipped it into a
      scratch folder and run `--version` and the computer check from there.
- [ ] The installer (`MAKE EXPRESS INSTALLER.bat`, then `dist\OpenStage-Capture-Express-0.1.0-x64-setup.exe`; see
      `docs/installer.md`): (Jason clicked through it once on 2026-11-01 and said it looked good; the rest is still open.) Run it by hand on a PC you can spare,
      ideally another one. Look at every page: the product icon should be
      in the tall picture on the first and last pages and small in the corner of the others, and the dark or light look should follow
      Windows. Tick the desktop shortcut once and leave it unticked once. Open the program from the last page. Check the Start menu
      entry, then Settings, Apps: it should show the icon, and Uninstall should leave your guides and settings alone and say so. Also
      install over an older copy. Try it with Narrator (Inno Setup's wizard is an ordinary Windows dialog, but it has not been read
      aloud here). SmartScreen will warn, because nothing is signed yet.
- [ ] The exports of a captured guide have only been checked by their tests and by reading the files, not by opening each in Word,
      PowerPoint and a PDF reader as was done for the demo.

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
