#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Wraps the words the window puts on screen in `tr(...)`, so that they can be translated (interface languages, milestone 8).

It looks only at calls where the first or a named argument is plain text for a person (`ui.label("...")`, `widgets::button(ui, "...")`,
`egui::Checkbox::new(&mut x, "...")` and the like) and leaves every other string alone (ids, icon names, file names, keys that are
compared). Run it again and it changes nothing: a literal already inside `tr(` is skipped.

    python -I tools/i18n_wrap.py --dry-run     show what would be wrapped
    python -I tools/i18n_wrap.py               wrap it, in place
    python -I tools/i18n_wrap.py --keys        print every English key now wrapped in tr() or trf(), one per line (JSON strings)
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "crates" / "openstage-express-app" / "src"
# The files whose words are the window's. tests.rs and the generated icon table are left out.
SKIP = {"tests.rs", "uiicons_gen.rs"}

LIT = r'"(?:[^"\\]|\\.)*"'

# (pattern, which captured groups are text). Each pattern ends just before the literal it wraps; the literal is group "lit".
PATTERNS = [
    # ui.label("...")  and  .label("...")
    r'(?P<pre>\.label\(\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>RichText::new\(\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>widgets::(?:button|primary_button|heading|subheading)\(\s*ui\s*,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>widgets::button_enabled\(\s*ui\s*,\s*[^,]+,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>widgets::(?:icon_button|primary_icon_button)\(\s*ui\s*,\s*[^,]+,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>widgets::button_named\(\s*ui\s*,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>egui::Button::new\(\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>egui::Checkbox::new\(\s*[^,]+,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>egui::RadioButton::new\(\s*[^,]+,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>screen_header\(\s*ui\s*,\s*' + LIT + r'\s*,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>screen_header\(\s*ui\s*,\s*' + LIT + r'\s*,\s*' + LIT + r'\s*,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>(?:shell::)?section\(\s*ui\s*,\s*' + LIT + r'\s*,\s*[^,]+,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>\.on_hover_text\(\s*)(?P<lit>' + LIT + r')',
    # The Export screen's own helpers: tick boxes, radio rows and boxes for a line of text.
    r'(?P<pre>option_check\(\s*ui\s*,\s*[^,]+,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>option_check\(\s*ui\s*,\s*[^,]+,\s*' + LIT + r'\s*,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>option_line\(\s*ui\s*,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>option_line\(\s*ui\s*,\s*' + LIT + r'\s*,\s*[^,]+,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>option_radios\(\s*ui\s*,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>[^_a-z]choice\(\s*ui\s*,\s*[^,]+,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>[^_a-z]choice\(\s*ui\s*,\s*[^,]+,\s*' + LIT + r'\s*,\s*)(?P<lit>' + LIT + r')',
    r'(?P<pre>\.heading\(\s*)(?P<lit>' + LIT + r')',
]
COMPILED = [re.compile(p, re.S) for p in PATTERNS]


def is_text(literal: str) -> bool:
    """A literal worth translating: it has a letter, and is not an identifier-like token."""
    body = literal[1:-1]
    if not re.search(r"[A-Za-z]{2}", body):
        return False
    return True


def wrap(source: str):
    """Return the new source and how many literals were wrapped."""
    spans = []
    for rx in COMPILED:
        for m in rx.finditer(source):
            start, end = m.span("lit")
            # Already wrapped: the text just before the literal ends in `tr(`.
            before = source[max(0, start - 8) : start]
            if before.rstrip().endswith("tr(") or before.rstrip().endswith("trf("):
                continue
            if is_text(m.group("lit")):
                spans.append((start, end))
    spans = sorted(set(spans))
    out, last = [], 0
    for start, end in spans:
        out.append(source[last:start])
        out.append("tr(" + source[start:end] + ")")
        last = end
    out.append(source[last:])
    return "".join(out), len(spans)


def unescape(literal: str) -> str:
    """The text a Rust string literal holds, for the plain escapes the window's words use."""
    body = literal[1:-1]
    out, i = [], 0
    while i < len(body):
        c = body[i]
        if c != "\\":
            out.append(c)
            i += 1
            continue
        n = body[i + 1]
        if n == "\n" or n == "\r":
            # A backslash at the end of a line: the line break and the spaces that start the next line are not part of the text.
            i += 2
            while i < len(body) and body[i] in " \t\r\n":
                i += 1
        elif n == "n":
            out.append("\n"); i += 2
        elif n == "t":
            out.append("\t"); i += 2
        elif n in '"\\\'':
            out.append(n); i += 2
        elif n == "u":
            end = body.index("}", i)
            out.append(chr(int(body[i + 3 : end], 16))); i = end + 1
        else:
            raise ValueError(f"escape \\{n} in {literal[:40]}")
    return "".join(out)


TR_CALL = re.compile(r"\btrf?\(\s*(" + LIT + r")")


def keys_in(source: str):
    return [unescape(m.group(1)) for m in TR_CALL.finditer(source)]


def main(argv):
    dry = "--dry-run" in argv
    show_keys = "--keys" in argv
    total, keys = 0, []
    for path in sorted(SRC.rglob("*.rs")):
        if path.name in SKIP or "i18n" in path.parts:
            continue
        original = path.read_bytes().decode("utf-8")
        crlf = "\r\n" in original
        text = original.replace("\r\n", "\n")
        changed, n = wrap(text)
        if n:
            print(f"{path.relative_to(ROOT)}: {n} wrapped")
            total += n
            if not dry:
                path.write_bytes((changed.replace("\n", "\r\n") if crlf else changed).encode("utf-8"))
        keys.extend(keys_in(changed))
    if show_keys:
        for key in sorted(set(keys)):
            print(json.dumps(key, ensure_ascii=False))
    else:
        print(f"{total} literals {'would be ' if dry else ''}wrapped; {len(set(keys))} different keys in all")


if __name__ == "__main__":
    main(sys.argv[1:])
