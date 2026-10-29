#!/usr/bin/env python3
"""Cut a mismatching difftest program down by deleting lines while the mismatch stays.

usage: reduce.py <difftest binary> <program.c> <out.c> [hang | needle <text>]

The syntax check that keeps a candidate defined is `DIFF_STRICT_CMD` (default: clang for msp430, the 6502 test's
front end) plus the -Werror flags. A candidate counts only if the harness still reports MISMATCH (ours differs from both cc65 modes it
checks), so a program that stops compiling or turns into a cc65 bug is dropped.
"""
import os
import re
import subprocess
import sys

binary, source, out = sys.argv[1:4]
hang = len(sys.argv) > 4 and sys.argv[4] == "hang"
needle = sys.argv[5] if len(sys.argv) > 5 and sys.argv[4] == "needle" else None


def strict_ok(path):
    check = subprocess.run(
        os.environ.get("DIFF_STRICT_CMD", "clang --target=msp430 -funsigned-char -fsyntax-only").split()
        + ["-Werror=return-type", "-Werror=uninitialized", "-Werror=sometimes-uninitialized",
           "-Werror=conditional-uninitialized", path],
        capture_output=True,
    )
    return check.returncode == 0


def mismatches(lines):
    path = out + ".try"
    with open(path, "w") as f:
        f.write("".join(lines))
    env = dict(os.environ, DIFF_FILE=path, DIFF_SEEDS=f"{900000 + os.getpid() % 90000}:1", DIFF_JOBS="1", DIFF_STRICT="1")
    try:
        run = subprocess.run([binary, "--ignored", "--nocapture"], env=env, capture_output=True, text=True, timeout=10 if hang else 60)
    except subprocess.TimeoutExpired:
        # in hang mode the compile not finishing is what is kept, once clang accepts the program
        return hang and strict_ok(path)
    if needle:
        return needle in run.stdout + run.stderr and strict_ok(path)
    return not hang and "MISMATCH" in run.stdout


# the lines that set up an array or the struct stay: without them the program reads unset memory
keep = re.compile(r'^\s*(for \(j = 0; j < \d+; j\+\+\) a\d+\[j\] = |s\.a = )')
lines = open(source).readlines()
assert mismatches(lines), "the program does not " + ("hang" if hang else f"print {needle}" if needle else "mismatch")
chunk = max(len(lines) // 2, 1)
while chunk >= 1:
    at = 0
    while at < len(lines):
        removed = lines[at:at + chunk]
        trial = lines[:at] + lines[at + chunk:]
        if trial and not any(keep.match(l) for l in removed) and mismatches(trial):
            lines = trial
        else:
            at += chunk
    chunk //= 2


def spans(text):
    """Every balanced parenthesis pair as (start, end), outermost first."""
    stack, found = [], []
    for at, ch in enumerate(text):
        if ch == "(":
            stack.append(at)
        elif ch == ")" and stack:
            found.append((stack.pop(), at + 1))
    return sorted(found, key=lambda s: s[0] - s[1])


# then each parenthesized expression of the test body becomes 0 or 1, or its own first operand
text = "".join(lines)
body_from = text.index("unsigned test(")
changed = True
while changed:
    changed = False
    for a, b in spans(text[body_from:]):
        a, b = a + body_from, b + body_from
        inner = text[a:b]
        if inner in ("(0)", "(1)") or inner.startswith("(for") or re.match(r"\((unsigned|int|signed)[ a-z]*\)$", inner):
            continue
        for repl in ("(0)", "(1)"):
            trial = text[:a] + repl + text[b:]
            if mismatches([trial]):
                text, changed = trial, True
                break
        if changed:
            break
lines = text.splitlines(keepends=True)
open(out, "w").writelines(lines)
print(f"{len(lines)} lines left in {out}")
