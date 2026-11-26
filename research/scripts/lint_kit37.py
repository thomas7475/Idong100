#!/usr/bin/env python3
"""Static lint of the kit 37 .bat files (authoritative version).

Checks:
  1. CRLF line endings only, ASCII only.
  2. Every goto/call target label exists; labels unique.
  3. Parenthesis balance per file (outside quotes, rem lines skipped).
  4. Percent sanity: after removing %~X argument refs, an even number
     of % signs per line (%% counts, %VAR% pairs).
  5. No space between an echo argument and a pipe (breaks regex tests).
"""
import os as _os
_HERE = _os.path.dirname(_os.path.abspath(__file__))  # research/scripts
# research/analysis; override with IDONG_ANALYSIS when the extracted
# installer payloads live somewhere else
ANALYSIS = (_os.environ.get('IDONG_ANALYSIS')
            or _os.path.join(_os.path.dirname(_HERE), 'analysis'))
REPO = _os.path.dirname(_os.path.dirname(_HERE))  # the repository root

import re
import sys
import glob

BAD = 0


def err(fname, i, msg):
    global BAD
    BAD += 1
    print(f"{fname}:{i+1}: {msg}")


def check(fname):
    data = open(fname, "rb").read()
    if b"\n" in data.replace(b"\r\n", b""):
        err(fname, 0, "bare LF found")
    text = data.decode("ascii")
    lines = text.split("\r\n")

    labels = {}
    for i, ln in enumerate(lines):
        m = re.match(r"^:([A-Za-z0-9_]+)", ln)
        if m:
            if m.group(1) in labels:
                err(fname, i, f"duplicate label :{m.group(1)}")
            labels[m.group(1)] = i
    for i, ln in enumerate(lines):
        for m in re.finditer(r"\b(?:goto|call)\s+:([A-Za-z0-9_]+)", ln):
            t = m.group(1)
            if t == "eof":
                continue
            if t not in labels:
                err(fname, i, f"goto/call target :{t} does not exist")

    depth = 0
    for i, ln in enumerate(lines):
        low = ln.lower()
        if low.startswith("rem") or ln.startswith("@echo"):
            pass
        if low.startswith("rem"):
            continue
        # echo arg + space + pipe
        if re.search(r"\becho\s+\S+ \|\s*findstr", ln):
            err(fname, i, f"space before pipe after echo: {ln.strip()[:70]!r}")
        # parenthesis balance outside quotes
        inq = False
        bal = 0
        for c in ln:
            if c == '"':
                inq = not inq
            elif not inq:
                if c == "(":
                    bal += 1
                elif c == ")":
                    bal -= 1
        depth += bal
        # percent sanity: strip %%~mods and %~args, then even count
        stripped = re.sub(r"%%~[a-zA-Z]+", "", ln)
        stripped = re.sub(r"%~[a-zA-Z0-9]*", "", stripped)
        stripped = stripped.replace("%%", "")
        if "JScript" in open(fname, "rb").read().decode("ascii")[:20000] and (
                re.match(r"\s*(var |function |if \(|for \(|while \(|//)", ln)):
            continue  # embedded JScript (check_capture.bat polyglot), not cmd
        if stripped.count("%") % 2 != 0:
            err(fname, i, f"odd number of % signs: {ln.strip()[:70]!r}")
    if depth != 0:
        err(fname, 0, f"unbalanced parentheses (depth {depth:+d})")
    print(f"{fname}: {len(labels)} labels, {len(lines)} lines")


for f in sorted(glob.glob(_os.path.join(REPO, "tools", "kit37", "*.bat"))):
    check(f)
print()
print("LINT FAIL" if BAD else "LINT PASS")
sys.exit(1 if BAD else 0)
