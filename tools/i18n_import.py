#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Writes a catalog (`crates/openstage-express-app/src/i18n/de.rs` or `fr.rs`) from a JSON file of {"key", "translation"} objects.

    python -I tools/i18n_import.py de translations-de.json [more.json ...]

Later files win over earlier ones, and a text that is not in the English any more is left out, so the catalog never holds words the
program does not say. `tools/i18n_keys.py` lists what has to be in it.
"""

import importlib.util
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("i18n_keys", ROOT / "tools" / "i18n_keys.py")
keys = importlib.util.module_from_spec(spec)
spec.loader.exec_module(keys)

NAMES = {"de": "German", "fr": "French"}


def rust(text: str) -> str:
    out = []
    for c in text:
        if c == "\\":
            out.append("\\\\")
        elif c == '"':
            out.append('\\"')
        elif c == "\n":
            out.append("\\n")
        elif c == "\t":
            out.append("\\t")
        else:
            out.append(c)
    return '"' + "".join(out) + '"'


def main(argv):
    if len(argv) < 2 or argv[0] not in NAMES:
        print(__doc__)
        return 2
    code, files = argv[0], argv[1:]
    translations = {}
    for f in files:
        for item in json.loads(Path(f).read_text(encoding="utf-8")):
            translations[item["key"]] = item["translation"]
    wanted = keys.collect()
    kept = {k: v for k, v in translations.items() if k in wanted}
    left_out = sorted(set(translations) - set(kept))
    missing = sorted(k for k in wanted if k not in kept)
    lines = [
        "// SPDX-License-Identifier: Apache-2.0",
        f"//! {NAMES[code]}: each of the program's English words, and what it says in {NAMES[code]}. A place in the English (`{{}}`, `{{name}}`) is",
        "//! `{1}`, `{2}`... in the translation, counted in the order the English has them. Made by `tools/i18n_import.py`; the translations have",
        "//! not been read by someone who writes the language as their own (`TESTING-TODO.md`).",
        "",
        "pub static ENTRIES: &[(&str, &str)] = &[",
    ]
    for k in sorted(kept, key=lambda s: (s.lower(), s)):
        lines.append(f"    ({rust(k)}, {rust(kept[k])}),")
    lines.append("];")
    path = ROOT / "crates" / "openstage-express-app" / "src" / "i18n" / f"{code}.rs"
    path.write_bytes(("\r\n".join(lines) + "\r\n").encode("utf-8"))
    print(f"{path.name}: {len(kept)} texts written; {len(left_out)} left out (not in the English now); {len(missing)} still missing")
    for k in missing:
        print("  missing:", json.dumps(k, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
