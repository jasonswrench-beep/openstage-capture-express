#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Lists the English the program says, for the people who translate it and for `check_rules.py`.

Three kinds:
  * `word`    in `tr("...")`, `trf("...")` or `k("...")`: words that are translated where they are shown;
  * `format`  in `tt(format!("..."))`: a sentence with something in it, matched by its shape when it is shown;
  * `status`  a sentence in the files that say things in the status line (`.say(...)`, `Some("...".to_string())`, `format!`), which is
              translated whole when it is said. This one is a list of candidates, found by what a sentence looks like, so it may hold a few
              things that are not said to a person.

    python -I tools/i18n_keys.py            print the keys as JSON, one object for each
    python -I tools/i18n_keys.py --counts   only how many of each kind
"""

import importlib.util
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "crates" / "openstage-express-app" / "src"
spec = importlib.util.spec_from_file_location("i18n_wrap", ROOT / "tools" / "i18n_wrap.py")
wrap = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wrap)

LIT = wrap.LIT
# The files whose sentences reach the status line or the screen without going through tr().
STATUS_FILES = ["ui/app.rs", "ui/editor_state.rs", "ui/capture.rs", "library.rs", "export_run.rs", "guide_writer.rs", "target.rs",
                "settings.rs", "export_options.rs", "export_templates.rs", "capture_win.rs", "ui/mod.rs"]
MARK = re.compile(r"\b(?:trf?|k)\(\s*(" + LIT + r")", re.S)
TT = re.compile(r"\btt\(\s*format!\(\s*(" + LIT + r")", re.S)
ANY = re.compile(LIT, re.S)


def without_tests(text: str) -> str:
    # The test module is the one `#[cfg(test)]` followed by `mod tests`; single items may carry the attribute too.
    cut = text.find("#[cfg(test)]" + chr(10) + "mod tests")
    return text if cut == -1 else text[:cut]


def looks_like_a_sentence(body: str) -> bool:
    if not re.match(r"^[A-Z{(]", body) and not re.match(r"^\d+ [a-z]", body):
        return False
    plain = re.sub(r"\{[^}]*\}", " ", body)
    if len(re.findall(r"[A-Za-z]{2,}", plain)) < 2:
        return False
    if any(c in body for c in ("/", "\\", "_")) and " " not in body:
        return False
    if body.startswith(("http", "C:", "%")):
        return False
    return True


def collect():
    found = {}

    def add(key, kind):
        if key not in found or kind == "word":
            found[key] = kind

    for path in sorted(SRC.rglob("*.rs")):
        if path.name in wrap.SKIP or "i18n" in path.parts:
            continue
        text = without_tests(path.read_bytes().decode("utf-8").replace("\r\n", "\n"))
        for m in MARK.finditer(text):
            add(wrap.unescape(m.group(1)), "word")
        for m in TT.finditer(text):
            add(wrap.unescape(m.group(1)), "format")
    for rel in STATUS_FILES:
        path = SRC / rel
        text = without_tests(path.read_bytes().decode("utf-8").replace("\r\n", "\n"))
        for m in ANY.finditer(text):
            line_start = text.rfind("\n", 0, m.start()) + 1
            line = text[line_start : m.start()]
            if "//" in line or re.search(r"\b(?:trf?|k)\(\s*$", text[max(0, m.start() - 8) : m.start()]):
                continue
            try:
                body = wrap.unescape(m.group(0))
            except Exception:
                continue
            if looks_like_a_sentence(body):
                add(body, "status")
    return found


def main(argv):
    found = collect()
    if "--counts" in argv:
        for kind in ("word", "format", "status"):
            print(kind, sum(1 for v in found.values() if v == kind))
        return
    print(json.dumps([{"key": k, "kind": v} for k, v in sorted(found.items(), key=lambda kv: (kv[1], kv[0]))], ensure_ascii=False, indent=0))


if __name__ == "__main__":
    main(sys.argv[1:])
