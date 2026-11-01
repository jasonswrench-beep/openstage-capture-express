#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Checks the OpenStage Capture Express rules on every change (spec sections 2 and 13.2).

It is a trimmed copy of OpenStage Capture's own checker (capture/tools/check_rules.py in the
openstage-author repository), with Express's extra rules added. It fails if:
  * a dependency has a license that is not on the allowed list (no GPL, no unknown licenses);
  * a dependency looks like FFmpeg (FFmpeg is never bundled);
  * a dependency is a network client, a telemetry or crash-report crate, an OCR engine that downloads
    data, or libuiohook (Express makes no network connection in version 1, spec rule P1);
  * any build file or script asks for an FFmpeg build with --enable-gpl or --enable-nonfree
    (--enable-version3 is only flagged for review);
  * a source file is missing its Apache-2.0 license header;
  * project prose or comments contain an em dash (OpenStage's writing rules avoid them);
  * a Windows batch file breaks the script rules (plain ASCII, CR LF, no brackets in echo lines inside
    blocks, cargo found through rust-setup.bat and run through cargo-run.bat).

Run it from anywhere:   python -I tools/check_rules.py
Write the third-party list:   python -I tools/check_rules.py --write-third-party
Exit code: 0 if every check passes, 1 if any fails.
"""

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent  # the openstage-capture-express folder

# Licenses that can be shipped inside an Apache-2.0 project (the same list Capture uses).
ALLOWED = {
    "Apache-2.0", "MIT", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "Unlicense", "CC0-1.0",
    "0BSD", "BSL-1.0", "Unicode-3.0", "Unicode-DFS-2016",
}

# Crates that must never be in the dependency tree, and why (spec sections 2 and 6.3).
FORBIDDEN_CRATES = {
    "reqwest": "an HTTP client (no network in version 1)",
    "hyper": "an HTTP library (no network in version 1)",
    "hyper-util": "an HTTP library (no network in version 1)",
    "hyper-tls": "an HTTP library (no network in version 1)",
    "ureq": "an HTTP client (no network in version 1)",
    "curl": "an HTTP client (no network in version 1)",
    "curl-sys": "an HTTP client (no network in version 1)",
    "isahc": "an HTTP client (no network in version 1)",
    "surf": "an HTTP client (no network in version 1)",
    "attohttpc": "an HTTP client (no network in version 1)",
    "minreq": "an HTTP client (no network in version 1)",
    "awc": "an HTTP client (no network in version 1)",
    "sentry": "telemetry or crash reporting (nothing leaves this computer)",
    "sentry-core": "telemetry or crash reporting (nothing leaves this computer)",
    "opentelemetry": "telemetry (nothing leaves this computer)",
    "uiohook": "libuiohook is LGPL; Express writes its own hooks",
    "libuiohook": "libuiohook is LGPL; Express writes its own hooks",
    "uiohook-sys": "libuiohook is LGPL; Express writes its own hooks",
    "tesseract": "an OCR engine that downloads language data (use Windows' own OCR)",
    "tesseract-sys": "an OCR engine that downloads language data (use Windows' own OCR)",
    "resvg": "MPL-2.0, outside the allowed list",
}

SKIP_DIRS = {"target", ".git", "node_modules"}
HEADER_SUFFIXES = {".rs", ".py"}
FLAG_SUFFIXES = {".toml", ".rs", ".bat", ".cmd", ".sh", ".yml", ".yaml", ".py", ".lock"}
STYLE_SUFFIXES = {".md", ".rs", ".bat", ".py", ".yml", ".yaml", ".toml", ".json", ".html", ".css", ".ts"}
EM_DASH = chr(0x2014)  # built from its code so this file does not trip its own check

results = []


def record(status, name, detail=""):
    results.append((status, name, detail))


def files(base, suffixes):
    for path in sorted(base.rglob("*")):
        if path.is_file() and path.suffix in suffixes and not (set(path.parts) & SKIP_DIRS):
            yield path


# ---- license expressions: "MIT OR Apache-2.0", "(A OR B) AND C", "Apache-2.0 WITH LLVM-exception"

def tokenize(expr):
    return re.findall(r"\(|\)|[A-Za-z0-9.+\-]+", expr.replace("/", " OR "))


def license_ok(expr):
    tokens = tokenize(expr)
    pos = 0

    def parse_or():
        nonlocal pos
        value = parse_and()
        while pos < len(tokens) and tokens[pos] == "OR":
            pos += 1
            other = parse_and()
            value = value or other
        return value

    def parse_and():
        nonlocal pos
        value = parse_atom()
        while pos < len(tokens) and tokens[pos] == "AND":
            pos += 1
            other = parse_atom()
            value = value and other
        return value

    def parse_atom():
        nonlocal pos
        if pos >= len(tokens):
            return False
        token = tokens[pos]
        pos += 1
        if token == "(":
            value = parse_or()
            if pos < len(tokens) and tokens[pos] == ")":
                pos += 1
            return value
        ok = token.rstrip("+") in ALLOWED
        if pos < len(tokens) and tokens[pos] == "WITH":
            pos += 2  # an exception such as LLVM-exception only loosens a license
        return ok

    return parse_or()


def cargo_packages():
    """The packages Express is built from on this computer, read with `cargo tree` and never downloading anything.

    Express is a Windows program, so the list is the one for Windows: `cargo metadata` would also list (and fetch) the crates
    that only other systems use, such as the ones the window library needs for Linux and the web. Each package is a dict with
    its name, version and license; `source` is None for the project's own crates.
    """
    out = subprocess.run(
        ["cargo", "tree", "--offline", "--workspace", "-e", "normal,build,dev", "--prefix", "none", "--format", "{p}|{l}"],
        capture_output=True, text=True, cwd=str(ROOT),
    )
    if out.returncode != 0:
        record("FAIL", "read the dependency list", out.stderr.strip()[-400:])
        return []
    found = {}
    for line in out.stdout.splitlines():
        text, bar, license_text = line.strip().partition("|")
        if not bar:
            continue
        text = re.sub(r" \(\*\)$", "", text.strip()).replace(" (proc-macro)", "")
        match = re.match(r"^(\S+) v(\S+)(?: \((.*)\))?$", text)
        if not match:
            continue
        name, version, where = match.groups()
        is_own = bool(where) and not where.startswith(("registry+", "git+", "http"))
        found[(name, version)] = {
            "name": name,
            "version": version,
            "license": license_text.strip() or None,
            "source": None if is_own else "registry",
        }
    return sorted(found.values(), key=lambda p: (p["name"], p["version"]))


def check_dependencies(packages):
    bad = []
    for pkg in packages:
        expr = pkg.get("license")
        if not expr:
            bad.append(f"{pkg['name']} {pkg['version']}: no license listed")
        elif not license_ok(expr):
            bad.append(f"{pkg['name']} {pkg['version']}: {expr}")
    if bad:
        record("FAIL", "dependency licenses", "not on the allowed list: " + "; ".join(bad))
    else:
        record("PASS", "dependency licenses", f"{len(packages)} packages, all on the allowed list")

    ffmpeg = [p["name"] for p in packages if "ffmpeg" in p["name"].lower()]
    if ffmpeg:
        record("FAIL", "no FFmpeg dependency", "FFmpeg is never bundled: " + ", ".join(ffmpeg))
    else:
        record("PASS", "no FFmpeg dependency")

    banned = sorted(f"{p['name']} ({FORBIDDEN_CRATES[p['name'].lower()]})" for p in packages
                    if p["name"].lower() in FORBIDDEN_CRATES)
    if banned:
        record("FAIL", "no network, telemetry or LGPL hook crates", "; ".join(banned))
    else:
        record("PASS", "no network, telemetry or LGPL hook crates")


def check_headers():
    missing = []
    for path in files(ROOT, HEADER_SUFFIXES):
        head = "\n".join(path.read_text(encoding="utf-8").splitlines()[:5])
        if "SPDX-License-Identifier: Apache-2.0" not in head:
            missing.append(str(path.relative_to(ROOT)))
    if missing:
        record("FAIL", "license headers", "missing SPDX header: " + ", ".join(missing))
    else:
        record("PASS", "license headers")


def check_project_files():
    missing = [name for name in ("LICENSE", "NOTICE") if not (ROOT / name).is_file()]
    if missing:
        record("FAIL", "LICENSE and NOTICE files", "missing: " + ", ".join(missing))
    else:
        record("PASS", "LICENSE and NOTICE files")


def check_ffmpeg_flags():
    this = Path(__file__).resolve()
    hard, review = [], []
    for path in files(ROOT, FLAG_SUFFIXES):
        if path.resolve() == this:
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for flag in ("--enable-gpl", "--enable-nonfree"):
            if flag in text:
                hard.append(f"{path.relative_to(ROOT)} uses {flag}")
        if "--enable-version3" in text:
            review.append(str(path.relative_to(ROOT)))
    if hard:
        record("FAIL", "FFmpeg build flags", "; ".join(hard))
    else:
        record("PASS", "FFmpeg build flags", "no --enable-gpl or --enable-nonfree")
    if review:
        record("WARN", "FFmpeg --enable-version3", "needs review: " + ", ".join(review))


def check_style():
    found = []
    for path in files(ROOT, STYLE_SUFFIXES):
        if EM_DASH in path.read_text(encoding="utf-8", errors="replace"):
            found.append(str(path.relative_to(ROOT)))
    if found:
        record("FAIL", "writing style (no em dashes)", ", ".join(sorted(set(found))))
    else:
        record("PASS", "writing style (no em dashes)")


def batch_block_problems(text):
    """Lines of a Windows batch file that break it: an `echo` inside a ( ... ) block that contains a
    bracket. In a block, the first `)` ends it, so the rest of the line is run as a command and the
    whole script stops with a syntax error (and its window closes, which looks like a crash). Returns
    (line number, line) pairs. Escaping with ^ is allowed."""
    problems = []
    depth = 0
    for number, raw in enumerate(text.splitlines(), start=1):
        line = raw.strip()
        if depth > 0 and line.lower().startswith("echo"):
            unescaped = re.sub(r"\^[()]", "", line)
            if "(" in unescaped or ")" in unescaped:
                problems.append((number, line))
        # A line ending in ( opens a block; a line starting with ) closes one.
        if line.endswith("("):
            depth += 1
        if line.startswith(")") and depth > 0:
            depth -= 1
    return problems


def batch_path_problems(text):
    """Lines that pass "%~dp0" (the script's folder, which ends in a backslash) as a quoted argument. A backslash
    before the closing quote escapes it, so the program receives the path with a stray quote and glued to the
    next argument. `cd /d "%~dp0"` is fine; for a program, write "%~dp0." instead. Returns (line number, line) pairs."""
    problems = []
    for number, raw in enumerate(text.splitlines(), start=1):
        line = raw.strip()
        if '"%~dp0"' in line and not line.lower().startswith("cd "):
            problems.append((number, line))
    return problems


def check_batch_files():
    bad = []
    for path in sorted(ROOT.glob("*.bat")):
        text = path.read_text(encoding="ascii", errors="replace")
        for number, line in batch_block_problems(text):
            bad.append(f"{path.name} line {number}")
        for number, line in batch_path_problems(text):
            bad.append(f"{path.name} line {number} (a quoted folder ending in a backslash)")
    name = "batch files (no brackets in echo lines inside blocks)"
    if bad:
        record("FAIL", name, ", ".join(bad))
    else:
        record("PASS", name)


HELPER_SCRIPTS = {"rust-setup.bat", "cargo-run.bat"}


def batch_rust_problems(name, raw):
    """Problems with how a script finds and runs cargo. Some computers have Rust installed but not on the PATH
    (rust-setup.bat deals with that), and the libraries may need to be downloaded (cargo-run.bat asks first), so no
    script may call cargo for a build or a test on its own."""
    problems = []
    if b"\n" in raw.replace(b"\r\n", b""):
        problems.append("lines must end with CR LF")
    if any(byte > 127 for byte in raw):
        problems.append("only plain ASCII is allowed")
    if name in HELPER_SCRIPTS:
        return problems
    text = raw.decode("ascii", errors="replace")
    lines = [line.strip() for line in text.splitlines()]
    uses_cargo = [i for i, line in enumerate(lines) if line.startswith("cargo ") or "cargo-run.bat" in line]
    for i in uses_cargo:
        line = lines[i]
        if line.startswith("cargo ") and not line.startswith("cargo --version"):
            problems.append(f"line {i + 1} runs cargo directly: use call \"%~dp0cargo-run.bat\"")
    if uses_cargo:
        setup = [i for i, line in enumerate(lines) if line.startswith("call") and "rust-setup.bat" in line]
        if not setup or setup[0] > uses_cargo[0]:
            problems.append("must call rust-setup.bat before the first use of cargo")
    return problems


def check_batch_rust():
    bad = []
    for helper in sorted(HELPER_SCRIPTS):
        if not (ROOT / helper).is_file():
            bad.append(f"{helper} is missing")
    for path in sorted(ROOT.glob("*.bat")):
        for problem in batch_rust_problems(path.name, path.read_bytes()):
            bad.append(f"{path.name}: {problem}")
    name = "batch files (find Rust through rust-setup.bat, run cargo through cargo-run.bat)"
    if bad:
        record("FAIL", name, "; ".join(bad))
    else:
        record("PASS", name)


def write_third_party(packages):
    members = {p["name"] for p in packages if p.get("source") is None}
    rows = sorted((p["name"], p["version"], p.get("license") or "unknown") for p in packages if p["name"] not in members)
    lines = [
        "# Third-party components",
        "",
        "Generated by `tools/check_rules.py --write-third-party`. Do not edit by hand.",
        "OpenStage Capture Express is Apache-2.0. These Rust crates are built into it, each under the license shown.",
        "",
        "| Crate | Version | License |",
        "|---|---|---|",
    ]
    lines += [f"| {name} | {version} | {lic} |" for name, version, lic in rows]
    (ROOT / "THIRD-PARTY.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"Wrote {ROOT / 'THIRD-PARTY.md'} ({len(rows)} crates)")


def main():
    packages = cargo_packages()
    if packages:
        check_dependencies(packages)
    check_project_files()
    check_headers()
    check_ffmpeg_flags()
    check_style()
    check_batch_files()
    check_batch_rust()
    if "--write-third-party" in sys.argv and packages:
        write_third_party(packages)

    failed = 0
    for status, name, detail in results:
        print(f"{status:<5} {name}" + (f": {detail}" if detail else ""))
        failed += status == "FAIL"
    print()
    print("All rule checks passed." if not failed else f"{failed} rule check(s) FAILED.")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
