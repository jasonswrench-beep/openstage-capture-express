# Requests to the shared core

What OpenStage Capture Express needs from `openstage-core` that the core does not offer yet. Nothing here is built in Express in its
place: each is waiting on the core. Read the core's `CORE-NOTES.md` for what it offers now. Newest first.

Asked 15 November 2026, for the Capture Express development list of that day (items 2 and 3). Core state when asked: `CORE-NOTES.md` of
2026-11-15 (second round, public). It lists the packages for C1 (the TypeScript part), C2 and C4 as "Not started (priority 3)", and gives
the capture crates at tag `capture-v0.1.0`, which Express now uses (its three pins moved from OpenStage Author's commit 4c8d23f, with no
change to any code).

## 1. C1, the capture format: a guide made of still pictures (list item 2)

The Shared Core Development List says C1 is to be the one format every capture tool writes or reads, with Capture Express named as one of
its users. Express cannot write it today, because the format is a format for recordings (video, sound and a log) and Express's guides are
made of pictures.

**What Express keeps, in its own format** (`docs/guide-format.md`): a guide is a folder `Title.osguide` holding `guide.json` and `images/`.
A guide has an id, a title, a description, a language and its steps in reading order. A step has:

- an id that is never used again, a kind (`image`, `empty` or `block`), a title and a description (plain text with a small subset of
  Markdown), and a parent (steps nest);
- one original picture (PNG, never changed), its size, and the author's description of it (alt text);
- where the click was (`x`, `y`, `button`) and the control that was clicked (`name`, `role`, from Windows UI Automation), and the program's
  file name (never a window title, never what was typed);
- marks drawn over the picture (rectangle, oval, line, arrow, text, step number, highlight, blur, spotlight, cursor, click circle), each in
  picture pixels, and a focused view (a crop rectangle);
- whether it is skipped, when it was captured, and a trash for deleted steps.

**What C1 has** (`specs/capture`): a `.capture` folder with `capture.json`, `edit.json` and one folder for each take, each take with a
video, sound files and `events.jsonl`. A reader ignores a take's media kind it does not know, and `tracks.altText` exists but is empty.
There is no media kind for a picture, so a guide of pictures has nowhere to go, and OpenStage Author, which grabs its step pictures from the
take's video (ADR 0003, `steps.json`), would have no video to grab them from.

**What Express needs the core to settle** (the format change starts in the core, with a new revision, before Express writes it):

1. **Whether C1 is for pictures at all.** Either a media kind for a take made of still pictures (each with its time, the display and region
   it came from, its size and its file), or an answer that C1 stays a format for recordings and a second, small format for a guide of
   pictures is defined beside it. Express will write whichever is chosen.
2. **Where the steps live.** Author keeps steps as `steps.json` in the recording, with an id that is never reused, and a picture and a target box
   in percent. Express's step carries more (nesting, marks, a focused view, a trash). Which of those belong in the shared format, and
   which stay in Express's own file?
3. **Marks.** `edit.json` has callouts and zooms for video. Express has nine kinds of mark and a focused view on a picture, and its blur must
   be burned in when a copy is shared. Say whether marks are in the format, and if not, that a reader may ignore Express's.
4. **What a capture may not hold.** Express never stores a window title or typed text, and `events.jsonl` can hold both (the spec says the log
   "can contain private information"). A way to say in the file that a capture has neither, so that a tool which refuses private logs can
   take it.
5. **The package.** A reader and writer for the format for Rust (Express is Rust, one program, no Node and no network) as well as for
   TypeScript, and conformance files that both are held to.

**What Express does until then:** it exports a book for OpenExplainer (the "JSON with images" file OpenExplainer documents, and its *Import
an export* reads) and a PowerPoint file for OpenStage Author (its *PowerPoint slides* import), both from the finished pictures. They carry
the steps, the words and the pictures with their marks. They do not carry the click positions or the control names, so Author cannot make
a Practice lesson (click the right place) from them, and OpenExplainer cannot offer its own callouts from the clicked item. That is
what C1 is for.

### The "send to" path (also C1)

One guide opened in OpenExplainer or Author without exporting it by hand: how Express finds which of them is installed and starts it with
the file (a file association, a registered command, or a small hand-off file), without knowing their install folders. Express would only
send, never receive. Today it writes the two files above into a folder and opens the folder.

## 2. C2, the accessibility rule engine: its HTML, PDF and Word exports (list item 3)

The list asks Express to run C2 on its HTML, PDF and .docx exports, because its own contrast tests cover its window, not the guides it
makes. C2 is not in the core (priority 3, not started), so Express has run nothing and built no engine of its own.

**What Express has instead, which is not a rule engine:** the tests of its own exports (the web page has one `h1`, no skipped heading level,
alt text on every picture, no outside request, text colours at least 4.5 to 1; the PDF audit reads the finished file for tags, language,
bookmarks and alt text and warns in words; the Word and PowerPoint files are checked as packages). They say nothing about WCAG success
criteria and nothing is run on a person's file after the export.

**What Express needs from C2:**

- **A Rust crate, or a command-line program Express can start.** Express is one Rust program with no Node, no network and no outside
  runtime; a package for TypeScript alone cannot be used from it.
- **Input:** the files an export made: the single-page web page (its pictures inside it as `data:` addresses), the tagged PDF, the `.docx`,
  and the `.pptx`.
- **Output:** a list of findings, each with the rule, the WCAG 2.1 AA success criterion, a status in PDF Remediator's words (pass, fail,
  needs review, not tested), where it is (a step, a page or a slide), a plain sentence and how to put it right. Findings warn and never block;
  a project may turn chosen rules into blockers, off unless the person turns it on.
- **A test set:** Express can supply the demo guide's web page, PDF and Word file, and wants the findings the engine should give for each.

**What Express does when it exists:** a line in each export's report per format ("Web page: 0 failures, 2 to review"), the findings
listed in the Export screen after the run, and a setting for blockers, off to begin with.

## Not asked of the core: for information

- **Interface languages (list item 4).** Express keeps its German and French wording in catalogs inside the program (`crates/openstage-express-app/src/i18n`),
  because C6 (the translation layer) is not in the core yet. When C6 has a shared string extractor and importer, Express's catalogs can be
  offered as XLIFF so one translator can work on every tool's words.
- **Export templates.** A template is a small file of Express's own (`.osexport`) that holds what the Export screen chose, so two
  people can share the same look. Nothing for the core, unless other tools want the same kind of file.
