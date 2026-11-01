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
