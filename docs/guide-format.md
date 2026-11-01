# The Express guide format

This document is the source of truth for what Express writes to disk. The code in `crates/openstage-express-core` must always agree
with it: change this file in the same commit as any code that reads or writes the format. Every change to the format bumps
`schemaVersion` and ships a migration with a fixture (see "Versions and migrations").

Current version: **0.1.0**. Status: written and tested on any computer (the tests in `crates/openstage-express-core`), used by the
window's editor, and written by real capture (see "How capturing fills a guide"; milestone M6).

## A guide is a folder

```
How to submit an assignment.osguide/
  guide.json            the guide (always written atomically, and last)
  images/step-0001.png  the original pictures, never modified
  trash/                the pictures of deleted steps, and trash.json
  guide.lock            present while a copy of Express has the guide open
  guide.before-<v>.json a copy of the guide file as it was before it was brought up to date from version <v>
```

- The folder name is made from the title (characters Windows forbids become `_`, Windows reserved names get a `_`, at most 80
  characters, `Title (2).osguide` if the name is taken). The `title` inside `guide.json` wins over the folder name.
- Thumbnails and rendered previews are caches, not part of the format. They may be deleted at any time.
- Default location: `Documents\OpenStage Express\Guides\`.
- Times are ISO 8601 in UTC, such as `2026-10-31T14:32:05Z`.

## `guide.json`

```json
{
  "schemaVersion": "0.1.0",
  "id": "g-2026-10-31-1432-a1b2",
  "title": "How to submit an assignment",
  "description": "",
  "language": "en",
  "createdAt": "2026-10-31T14:32:05Z",
  "updatedAt": "2026-10-31T14:40:11Z",
  "focusedView": true,
  "nextStepNumber": 4,
  "steps": [ { "...": "see below" } ]
}
```

| Field | Rule |
|---|---|
| `schemaVersion` | Required. A version this program does not know to be older or equal is refused or migrated (see below). |
| `id`, `title`, `createdAt`, `updatedAt`, `nextStepNumber` | Required. |
| `description` | Optional, default empty. |
| `language` | Optional, default `en`. The language of the step titles and the exports. |
| `focusedView` | Optional, default `true`. The default for new steps. Each step's own `focus` decides what it shows. |
| `nextStepNumber` | The number the next new step gets. It only ever goes up, so **a step id is never used twice**, even after a delete, an undo or emptying the trash. It must be above the number of every step in the guide. |
| `steps` | All the steps, in reading order, as one flat list. |

### Order and nesting

`steps` is the depth-first order of a tree: a step is followed by all its substeps, then by its next brother or sister. A step's
`parent` is the id of its parent, or `null` for a top-level step. **A child always comes after its parent.** Every operation that
changes the order or nesting rewrites the list in this order, so a reader may rely on it, but a reader of a file written by something
else must not crash on a list that is out of order (the program lists it as a problem and repairs it on the next structural edit,
without losing a step).

## A step

```json
{
  "id": "step-1",
  "kind": "image",
  "title": "Click \"Assignments\"",
  "description": "",
  "image": { "file": "images/step-0001.png", "width": 1920, "height": 1080 },
  "imageDescription": "",
  "parent": null,
  "skipped": false,
  "click": { "x": 312, "y": 188, "button": "left" },
  "control": { "name": "Assignments", "role": "link" },
  "typed": null,
  "program": "chrome.exe",
  "focus": { "x": 0, "y": 0, "width": 1280, "height": 720 },
  "annotations": [],
  "capturedAt": "2026-10-31T14:32:41Z"
}
```

| Field | Rule |
|---|---|
| `id` | `step-<n>` with `n` from 1 up. Unique in the guide. Never reused (also not against deleted steps in the trash). |
| `kind` | `image` (a picture, a title, a description), `empty` (no picture), `block` (a plain text content block, not in the table of contents). |
| `title`, `description` | Plain text. The description may use a small Markdown subset (bold, italic, links, bullet and numbered lists), read by the exports. |
| `image` | Required for `image`, and must be absent (`null`) for `empty` and `block`. `file` is `images/` and one plain file name (see "Picture paths"). `width` and `height` are the pixels of the original, above zero. All coordinates in the step are in these pixels, origin top left. |
| `imageDescription` | The alt text the author wrote. If empty, exports use the title. |
| `parent` | See "Order and nesting". |
| `skipped` | Left out of every export. |
| `click` | Where the click happened, in picture pixels. `button` is `left`, `right` or `middle` (default `left`). `null` for steps made by a hotkey. |
| `control` | The control that was clicked, from UI Automation: `name` and `role`. Roles use the English names of OpenStage Capture (`button`, `link`, `edit`, and so on). `null` if unknown. |
| `typed` | The text of a `Type` step, read from the field when typing finished (never logged key by key, never for a password field). **Private**: shown to the author, and in exports only as the step title. `null` otherwise. |
| `program` | The program's file name only. Window titles are never stored. |
| `focus` | The focused view crop in picture pixels, with a width and a height above zero. `null` means the whole picture. It is a stored rectangle, not an edited picture. |
| `annotations` | Marks drawn on the picture, in drawing order. |
| `capturedAt` | When the step was captured, or `null`. |

### Picture paths

A picture path is exactly `images/` followed by one file name. The name may not be empty, start with a dot, or contain `/`, `\`, `:`
or control characters. Anything else (a path that climbs out of the folder, an absolute path, a drive letter) is refused when the
guide is checked and never followed when it is read or saved. A path always names the picture as if it were in `images/`, even
while the file is physically in `trash/` (see below).

### Pictures and the trash

Pictures are originals and are never modified; annotations, blur and the focused view are drawn on top of them at export. Two steps
(a step and its duplicate) may name the same picture file.

Where a picture file lives is decided when the guide is saved: in `images/` if any live step uses it, in `trash/` if only deleted
steps do. A save moves the file, never copies or deletes it. A reader looks in `images/` first and then in `trash/`. A picture that
no step uses (left by a crash between writing a picture and saving the guide) is listed to the person and never deleted by the
program. Only the person's "Empty the trash" deletes files, and only pictures nothing uses.

## Annotations

```json
{ "id": "a1", "type": "clickMark", "x": 312, "y": 188, "size": 40, "fill": "#7D87F233", "stroke": "#7D87F2", "strokeWidth": 4 }
```

`id` is unique within the step (`a1`, `a2`, ...; a new mark takes one above the highest in use). `type` and the fields each type uses:

| `type` | Required | Optional | Meaning of `x`, `y` |
|---|---|---|---|
| `rect`, `oval`, `highlight`, `blur`, `spotlight` | `width`, `height` (above zero) | `fill`, `stroke`, `strokeWidth` | the top left corner |
| `line`, `arrow` | `x2`, `y2` | `stroke`, `strokeWidth` | the start point (`x2`, `y2` is the end) |
| `text` | `text` | `width`, `fontSize`, `fill`, `stroke`, `strokeWidth` | the top left corner of the text |
| `number` | `number` | `size`, `fill`, `stroke` | the centre of the step number badge |
| `cursor` | `style` (such as `pointer`) | `size` | the pointer's hot spot |
| `clickMark` | `size` (above zero) | `fill`, `stroke`, `strokeWidth` | the centre of the circle |

Colours are `#RRGGBB` or `#RRGGBBAA`. Fields a type does not use are left out of the file. This table fixes what is stored. How marks are drawn is below.

### How marks are drawn

Marks are drawn on a copy of the whole original picture, in list order (a later mark is drawn over an earlier one), and only then
is the picture cropped to the step's `focus`. So a mark that crosses the edge of the crop stays where it belongs, and nothing is
ever drawn on the original file. Everything uses whole numbers and plain arithmetic, so the same step always gives the same
pixels on every computer.

| `type` | What it looks like, and what is used when a field is missing |
|---|---|
| `rect` | The `fill` (if any) over the whole rectangle, then the `stroke` as an outline inside it. With neither, a default indigo outline (`#7D87F2`). |
| `oval` | The same, as an ellipse that fills the rectangle, with smoothed edges. |
| `line`, `arrow` | A straight line with smoothed edges; an arrow has a head at the end point. `stroke` defaults to indigo. |
| `highlight` | A translucent yellow (`#FFE60066`) over the rectangle unless `fill` says otherwise. |
| `blur` | A mosaic: the rectangle is cut into square blocks, a third of its shorter side across (at least 12 and at most 48 pixels), and each block becomes the average colour of what was in it. Detail cannot be got back. To hide a secret, black it out with a solid `rect` instead. |
| `spotlight` | Everything outside the rectangle is darkened (`fill`, default `#000000A0`); the inside is left as it is. A `stroke` outlines the rectangle. |
| `text` | A box with its words. `fill` is the box (default navy), `stroke` is the text colour (default white or navy, whichever reads better on the box). `width` is optional: without it the box is made as wide as the words need and kept on the picture; a stated width wraps the words. `fontSize` defaults to one fortieth of the picture's height. If the words do not all fit, the export says which mark. |
| `number` | A filled circle (`fill`, default indigo) with the number inside in white or navy, whichever reads better (navy on the indigo). `size` is the diameter. |
| `cursor` | A mouse pointer whose hot spot is at `x`, `y`. `style` is `pointer` (the default, also used for any other word), `hand` or `text` (an I-beam). White with a black edge unless `fill` and `stroke` say otherwise. The artwork is this program's own. |
| `clickMark` | A circle of diameter `size` round `x`, `y`: `fill` default `#7D87F233` (translucent indigo), `stroke` default indigo. |

`strokeWidth` defaults to one 270th of the picture's height (at least 3), so 4 pixels on a 1080-pixel-tall picture. A colour with
an alpha part (`#RRGGBBAA`) is mixed with what is under it by that much; a colour that cannot be read is treated as missing.
A mark that is partly or wholly off the picture is cut off or does nothing; it is never an error.

In the editor, marks are added with a default size (round the click if the step has one), moved and resized in whole picture pixels, and kept on the picture; the editor shows the whole picture with the focused crop as a frame, because marks are drawn before the crop.

## The focused view

`focus` is a stored rectangle (picture pixels). When a step is captured with the guide's `focusedView` on, the program works out the
automatic crop: the picture's own shape, 150 percent closer, centred on the click and moved to stay inside the picture (no click,
no automatic crop). The person can then move it (drag with Space held) or zoom it (never closer than eight times). A crop is always
kept inside the picture, at least 8 pixels each way. Exports scale every crop to the same width, so a guide's pictures all line up.

## `trash/trash.json`

```json
{
  "schemaVersion": "0.1.0",
  "entries": [
    { "step": { "...": "a whole step, with parent null" }, "deletedAt": "2026-10-31T15:00:00Z", "wasParent": "step-1", "wasIndex": 2 }
  ]
}
```

Deleting a step moves it here. Its substeps are not deleted: they move up one level into its place. `wasParent` and `wasIndex` (its
place among its brothers and sisters) let "restore" put it back; if its parent is gone it comes back at the top level. Substeps that
moved up when it was deleted stay where they are. Steps in the trash keep their ids, and those ids are never reused.

## `guide.lock`

```json
{ "pid": 1234, "startedAt": "2026-10-31T14:32:05Z" }
```

Written when a copy of Express opens the guide for editing and removed when it closes it. A second copy that finds the lock held
by a running process opens the guide read-only and says so. A lock whose process is gone (or that cannot be read) was left by a
crash: the next copy takes it over, and tells the person that Express closed unexpectedly and that every step up to the last one
is safe.

## Saving, and what a crash can lose

A save does these in order: move pictures to where they belong, write `trash/trash.json`, write `guide.json`. Each JSON file is
written to a temporary file beside it (`guide.json.tmp`), forced to disk, and renamed over the real one, so a crash leaves either
the old file or the new one. A new picture is written **before** the guide that names it. So a crash costs at most the step being
made: the last complete guide is there, plus possibly one picture nothing uses yet.

## How capturing fills a guide

Capturing makes a guide folder when the **first step** arrives, so a capture of nothing leaves nothing behind. The guide is called
"Guide" followed by the date and time capturing started, such as `Guide 2026-11-01 09.38` (the editor's Guide title box renames it), and
takes the language `en` until the settings can say otherwise. Each step is then written as it is
made: its picture first (`images/step-0001.png`, a PNG of the whole display, named by the step number), then `guide.json` in one
atomic rename, so a crash loses at most the step in flight (see "Saving, and what a crash can lose").

A captured step has these fields filled in, and no others beyond what the format lists:

- `kind` is `image`, `title` is the smart title (`Click "Save"`, `Select "Country"`, `Type "ann@example.org"`, `Screenshot`), and
  `image` names the picture with its width and height in pixels.
- `click` is where the click was, **in the picture's own pixels** (the display's top left corner is 0, 0 even when the display sits
  at a negative place on the desktop). A step made by the capture-now hotkey has no `click` unless the setting "hotkey captures
  cursor" is on.
- `control` is the name and kind (English role: `button`, `edit`, `combo box`, ...) Windows' accessibility information gave for the
  thing clicked. It is absent when Windows gave neither.
- `typed` is the text of a `Type` step, **read from the field when typing finished**, never built from keys. Express's keyboard hook
  never passes on which key was pressed. A password field never has a `typed`, a `Type` step, or any of its text anywhere; a field
  that gives no text, or more than 500 letters (a whole document), gives `Type in "Name"` and no `typed`.
- `annotations` hold the marker (`clickMark`, size 56) and the cursor (`cursor`, style `pointer`, or `text` over an edit field or
  document, size 32), both scaled by the display's scaling percent, with ids `a1` and `a2` as the editor would give them. They are
  ordinary marks: the person can remove or restyle either.
- `focus` is the automatic focused view (150 percent), set when the focused view is on for new steps and the step has a click.
- `capturedAt` is the time the step was written.

The picture of a click is the **last picture of the screen taken at or before the click** (so an open menu or a tooltip is still in
it), kept in a ring of the latest pictures bounded to 64 megabytes. The picture of a `Type` step is taken at least 120 ms after the
last key, or at the Enter, Tab, Escape or click that ended the typing if that is later, so that the field shows its text. With a
screenshot delay, the picture is the first one taken that long after the click, or the newest one if the screen did not change.
Express's own windows are marked `WDA_EXCLUDEFROMCAPTURE`, so they are not in any picture, and a click on one of them is never a
step.
## Versions and migrations

- Opening a guide whose `schemaVersion` is newer than this program's is refused with "This guide was made by a newer Express", and
  nothing is changed.
- A guide with an older version is brought up to date in memory, one migration at a time, in order. Before the first save of a
  migrated guide, a copy of the old file is kept as `guide.before-<old version>.json`.
- A migration is a function from the old JSON to the new JSON plus a fixture file of the old format and a test that opens it. There
  are **no migrations yet** (0.1.0 is the first version); the mechanism is tested with made-up versions.
- Fields a reader does not know are kept and written back unchanged, in every object of the file, so a guide made by a slightly
  newer Express with extra fields survives being opened and saved here.

## The exports

Every export is made from the same plan, so they agree about which steps are in, how they are numbered, and what each picture is.

- **Which steps.** A skipped step is left out, and so are all its substeps. The rest are numbered `1`, `2`, `2.1`, `2.2`, `3` and so
  on (a substep's number starts with its parent's). A content block (`kind` `block`) is exported but not numbered, and is not in
  any contents list. A step whose parent cannot be found is left out.
- **The pictures.** Each is the step's original with its marks drawn on, cropped to its `focus`, and scaled to the same width
  (1280 pixels by default), as a PNG. Marks and blur are burned in: a blurred area can never be undone in an export. Pictures are
  named in export order, `images/step-01.png` and so on (not by step id). A picture that cannot be found is left out with a
  warning, and the step stays.
- **The words.** A picture's alt text is the author's `imageDescription`, or the step title if there is none. Words drawn on a
  picture by a `text` mark that are not also in the step's title, description or alt text produce a warning, because a screen
  reader cannot read a picture.
- **Nothing is replaced.** An export is written into a folder and stops, without replacing anything, if a file of the same name
  is already there.

| Export | Files |
|---|---|
| HTML | `<title>.html`: one self-contained page (pictures inside it as data, no outside requests, a Content-Security-Policy, headings, a contents list, a skip link, alt text, light and dark colours that pass 4.5 to 1, a print style, pictures that open larger in a dialog). |
| Markdown | `<title>.md` and `images/step-NN.png`. A step is a heading (`## 1. Title`, one level deeper for a substep), its description as written, and `![alt](images/step-NN.png)`. A content block is a quotation. |
| JSON | `<title>.json` and `images/step-NN.png`. See below. |
| Images | `step-NN.png`, or `step-NN.jpg` with a quality from 1 to 100 (the dialog offers 100, 90, 70 and 50, and 90 is the default). Both come from the same lossless picture, so the marks are crisp; the JPG is baseline, full colour resolution (no chroma subsampling, so text edges stay sharp), written by Express itself. A JPG is smaller than the PNG only for photograph-like pictures: for ordinary screenshots the PNG is usually smaller as well as sharper. |
| GIF | `<title>.gif`: one frame per step (a step with no picture is a card with its title), the same width, shorter pictures centred on a background colour, each frame 1, 2, 3, 5 or 8 seconds (3 by default), looping unless turned off, with an optional title card and progress bar. Width between 320 and 1920 (800 by default). A GIF cannot be made accessible. |
| PDF | `<title>.pdf`, made by the WebView2 engine on Windows. See below. |
| Word | `<title>.docx`, written by Express as Office Open XML. See below. |
| PowerPoint | `<title>.pptx`, one slide for each step. See below. |

### The PDF export

Express writes a *print page* from the plan (the HTML export's contents and numbering, made for paper) and has the Microsoft
WebView2 runtime print it with the DevTools command `Page.printToPDF`, which can write a tagged PDF. The runtime is the one that
ships with Windows 11; Express does not copy any of it. If it is not installed, PDF export says so and everything else still works.

- **Options.** Paper A4, Letter, A3, A5 or Legal; portrait or landscape; margins of 10 to 40 mm (20 by default); a cover page
  (title and description only); a contents list (every entry a link); a header line and a footer line; "Page 3 of 12" at the bottom
  right (words in the guide's language: English, German, French); every top-level step on a new page.
- **Fonts.** Roboto Regular, Bold and Italic (Apache-2.0), carried inside the page as data, so no Windows font is copied into the
  file. Roboto has no Chinese, Japanese, Korean or Arabic letters; those fall back to a Windows font.
- **No outside requests.** The page has a Content-Security-Policy that allows only its own style, pictures and fonts, no script, and
  the engine runs with scripts off, from a private working folder under a made-up host name, with background networking, updates
  and reputation checks turned off. The folder is removed afterwards.
- **What the PDF is tagged with** (WebView2 156, seen by reading the file with a separate program): the document language, the
  title shown in the window bar, headings (`H1` for the title, `H2` and deeper for steps), lists and list items, links, bold
  text, the note box (`Aside`), pictures as `Figure` with their alt text, and bookmarks (one for the title, the contents and every step).
- **What it is not tagged with.** The page numbers, the header and the footer are drawn in the page margins and are neither
  tagged nor marked as decoration, so a strict checker will report them. The export warns about this whenever any of them is on.
  The contents list has no page numbers (the engine does not support that). A PDF passed through no accessibility checker is not
  called accessible here.
- **Checks on the finished file.** The export reads the PDF back and warns when it finds no tags, no document language, fewer
  alt texts than pictures, or no bookmarks. Something that is not a PDF is never written as one.
- **Page breaks.** A picture never splits across pages, a heading is never left alone at the foot of a page, and a step's text goes
  with its picture (`break-after: avoid` chains). No block holding a heading is kept together with `break-inside: avoid`, because
  WebView2 once wrote some headings' bookmarks twice ("2.2. Click Save Click Save") from such a page set in Segoe UI; the cause is
  not understood, and the Windows tests print every layout option with the real engine and check the bookmark titles.

### The JSON export

Not the guide's own file: a flatter one for other programs. Field names follow the ones a step-guide program documents publicly for the same
meanings, as a convenience and not a promise of compatibility. `exportVersion` changes when a field is renamed or removed.

```json
{
  "exportFormat": "openstage-express-guide",
  "exportVersion": 1,
  "guide": { "title": "How to submit an assignment", "description": "", "language": "en" },
  "steps": [
    {
      "id": "step-1", "index": 1, "indexString": "1", "parentId": null, "kind": "image",
      "title": "Click \"Assignments\"", "description": "",
      "altText": "The course site with the Assignments item circled",
      "screenshotFilename": "step-01.png", "screenshotRelativePath": "images/step-01.png",
      "width": 1280, "height": 720
    }
  ]
}
```

`index` is the step's place in the export (from 1). `indexString` is null for a content block. `parentId` is the step above in the
tree, or null. `altText`, the picture fields and `width` and `height` are null for a step with no picture. Left out on purpose
because they are private or are for editing: the text typed into a field (it appears only as the step's title), the click, the
control, the program and the capture time.

### The Word export

A `.docx` written by Express itself (a ZIP file of XML and pictures, with fixed dates, so the same guide gives the same bytes).

- **Structure.** The guide's title is in the Title style. Every step is a heading: Heading 1 for a step, Heading 2 for a substep,
  Heading 3 for anything deeper. The step number is real text (`2.1. Type "Week 3 reflection"`). A content block is a shaded
  "Express Note" paragraph with no number, and is not in the contents list. Word's navigation pane, its accessibility checker and
  screen readers find all of this.
- **Contents.** A real Word `TOC` field (`TOC \o "1-3" \h \z \n`) that is already filled in with a link to every step, so it is
  complete when the file is opened. It has no page numbers, which only Word can work out; a right click and "Update field" in Word
  rebuilds it from the headings and gives the same entries (checked in Word).
- **Pictures.** Inline, each with its alt text, at most the width of the text and never larger than their natural size (96 pixels
  to the inch), and never taller than the page less its margins and 40 mm for the heading.
- **Descriptions** keep bold, italic, links (only web and mail addresses become links) and bullet and numbered lists; every
  numbered list starts again at 1.
- **Options.** Paper A4, Letter, A3, A5 or Legal; portrait or landscape; margins of 10 to 40 mm (25 by default); the contents list;
  "Page 3 of 12" in the footer (in the guide's language); every top-level step on a new page.
- **Language and properties.** The text is marked with the guide's language (`en-US`, `de-DE`, `fr-FR`, or the code as written);
  the title and language are in the document properties. No dates are written.
- **Fonts.** Calibri, which Word has. Nothing is embedded: Roboto, which the PDF carries, is not embedded in Word files yet.
- **Checked in Word** (Microsoft 365, version 16) on 2026-11-01: it opens with no repair prompt, not in compatibility mode, with
  the Title and heading styles and outline levels, one table of contents, a bookmark behind every heading, every picture's alt
  text, the list paragraphs, the footer fields and the A4 page size.

### The PowerPoint export

A `.pptx` written by Express itself, in the same way.

- **One 16:9 slide for each step** (13.33 by 7.5 inches). The slide's title is the step's number and title, in a real title
  placeholder, so the outline view, the accessibility checker and screen readers see it. Under it are the picture (with alt text)
  and the first two lines of the description, cut short with an ellipsis after 240 characters or when there is more. A step with
  no picture (a content block, or a picture that could not be found) shows all its text in a large box.
- **Speaker notes** hold the *full* description of every step that has one: paragraphs, list items with a bullet or their number,
  links written out as `label (address)`.
- **Order for a screen reader:** title, picture, description, footer, slide number.
- **Options.** A cover slide with the guide's title and description (on by default); agenda slides listing the top-level steps, ten
  to a slide (off by default); a footer line; slide numbers (on); speaker notes (on).
- **Fonts and colours.** Calibri, not embedded; the theme uses the same colours as the web page. The spec's optional logo is not
  built.
- **Checked in PowerPoint** (Microsoft 365, version 16) on 2026-11-01: it opens with no repair prompt, with seven slides at
  960 by 540 points, each with its title, the right layout, every picture's alt text, the slide numbers and the notes.
- **`tools/check-office.ps1`** opens the demo's Word and PowerPoint files in the real programs and checks these things again. It
  skips a program that is already open, so it cannot disturb a person's work.

## Not in this document yet

The settings file is written when it is built. WebP is not built (spec S3). Capturing a window or a drawn area (rather than the whole
display) is not built.