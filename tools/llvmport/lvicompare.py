#!/usr/bin/env python3
"""Compares LazyValueInfo's answers, LLVM's against llrm's, on .ll files.

    tools/llvmport/lvicompare.py PATH... [--show N] [--verbose] [--kind K]

PATH is a file or a directory (every `.ll` under it); the totals are per PATH. A file either side cannot read is
skipped and counted. Both dumps print one line per query (see `lvi_dump.cpp`); a line that differs is classified:
MORE (llrm is more precise than LLVM: a bug), LESS (llrm is less precise), or OTHER (the two disagree, neither
contains the other: a soundness bug on one side). `--kind val|edge|pred|epred|use` shows only that kind.
LLVM's side is `lvi_dump.sh`, llrm's the `lvidump` example of llrm-mir (CARGO_TARGET_DIR is honoured).
"""
import os
import re
import subprocess
import tempfile
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


DECLARES = [
    (r"assume", lambda w: "declare void @llvm.assume(i1)"),
    (r"(?:[su](?:min|max))\.i(\d+)", lambda w: f"declare i{w} @llvm.{{n}}(i{w}, i{w})"),
    (r"[su](?:add|sub)\.sat\.i(\d+)", lambda w: f"declare i{w} @llvm.{{n}}(i{w}, i{w})"),
    (r"ctpop\.i(\d+)", lambda w: f"declare i{w} @llvm.{{n}}(i{w})"),
    (r"(?:abs|ctlz|cttz)\.i(\d+)", lambda w: f"declare i{w} @llvm.{{n}}(i{w}, i1)"),
    (r"[su](?:add|sub|mul)\.with\.overflow\.i(\d+)", lambda w: f"declare {{{{ i{w}, i1 }}}} @llvm.{{n}}(i{w}, i{w})"),
]


def normalize(text):
    """What MIR's subset can read of `text`: no target triple, no linkage-visibility words, no attachments but
    `!range`, and a `declare` for each intrinsic of a few families that the file calls without one."""
    text = "\n".join(l for l in text.splitlines() if not l.startswith("target triple"))
    text = re.sub(r" (dso_local|hidden|protected)\b", "", text)
    text = re.sub(r",? !(?!range\b)[A-Za-z_.]+ !\d+", "", text)
    text = re.sub(r" !prof !\d+", "", text)
    declared = set(re.findall(r"^declare .*@(llvm\.[\w.]+)\(", text, re.M))
    extra = []
    for name in sorted(set(re.findall(r"@(llvm\.[\w.]+)\(", text)) - declared):
        for pattern, make in DECLARES:
            m = re.fullmatch(r"llvm\." + pattern, name)
            if m:
                extra.append(make(m.group(1) if m.groups() else 0).replace("{n}", name[5:]))
                break
    return text + "\n" + "\n".join(extra) + "\n"


def run(cmd):
    done = subprocess.run(cmd, capture_output=True, text=True)
    return done.returncode, done.stdout.splitlines(), done.stderr


def parse_range(text):
    """`i32:[0,10)`, `i8:full-set` or `i8:empty-set` as (width, lower, size)."""
    width, rest = text[1:].split(":", 1)
    width = int(width)
    modulus = 1 << width
    if rest == "full-set":
        return width, 0, modulus
    if rest == "empty-set":
        return width, 0, 0
    lower, upper = (int(x) for x in re.fullmatch(r"\[(-?\d+),(-?\d+)\)", rest).groups())
    lower %= modulus
    return width, lower, (upper - lower) % modulus


def within(inner, outer):
    """Whether the circular interval `inner` lies in `outer`."""
    width, lower, size = inner
    _, outer_lower, outer_size = outer
    modulus = 1 << width
    if size == 0 or outer_size == modulus:
        return True
    return size <= outer_size and (lower - outer_lower) % modulus <= outer_size - size


def compare_token(ours, theirs):
    """EQUAL, MORE, LESS or OTHER for one `name=value` token."""
    if ours == theirs:
        return "EQUAL"
    (name, a), (_, b) = ours.split("=", 1), theirs.split("=", 1)
    name = ours.split("=", 1)[0]
    if name == "cr" and a != "-" and b != "-":
        x, y = parse_range(a), parse_range(b)
        if x[1:] == y[1:] and x[0] == y[0]:
            return "EQUAL"
        if within(x, y):
            return "MORE"
        if within(y, x):
            return "LESS"
        return "OTHER"
    if name in ("a", "b", "pn") or name == "":
        if b == "-":
            return "MORE"
        if a == "-":
            return "LESS"
        return "OTHER"
    if name == "cst":
        if b == "-":
            return "MORE"
        if a == "-":
            return "LESS"
        return "OTHER"
    return "OTHER"


def classify(ours, theirs):
    a, b = ours.split(), theirs.split()
    if len(a) != len(b):
        return "OTHER"
    worst = "EQUAL"
    rank = {"EQUAL": 0, "LESS": 1, "MORE": 2, "OTHER": 3}
    for x, y in zip(a, b):
        if "=" not in x:
            if x == y:
                continue
            if len(x) == 1 and len(y) == 1:
                x, y = "=" + x, "=" + y  # epred's lone T/F/-
            else:
                return "OTHER"
        verdict = compare_token(x, y)
        if rank[verdict] > rank[worst]:
            worst = verdict
    return worst


def main():
    args = sys.argv[1:]
    show, verbose, kind = 10, "--verbose" in args, None
    if "--show" in args:
        show = int(args[args.index("--show") + 1])
    if "--kind" in args:
        kind = args[args.index("--kind") + 1]
    paths = []
    skip = False
    for i, arg in enumerate(args):
        if skip:
            skip = False
        elif arg in ("--show", "--kind"):
            skip = True
        elif not arg.startswith("--"):
            paths.append(arg)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    ours_bin = target / "release/examples/lvidump"
    status = 0
    for path in paths:
        files = sorted(str(f) for f in Path(path).rglob("*.ll")) if Path(path).is_dir() else [path]
        total = Counter()
        tmp = Path(tempfile.mkdtemp())
        kinds = Counter()
        skipped = 0
        shown = 0
        for original in files:
            file = tmp / Path(original).name
            file.write_text(normalize(Path(original).read_text(errors="replace")))
            file = str(file)
            code, theirs, _ = run([str(ROOT / "tools/llvmport/lvi_dump.sh"), file])
            if code:
                skipped += 1
                continue
            code, ours, err = run([str(ours_bin), file])
            if code:
                skipped += 1
                if verbose:
                    print(f"{original}: llrm failed: {err.strip().splitlines()[-1] if err.strip() else code}")
                continue
            if len(ours) != len(theirs):
                total["LINES"] += 1
                print(f"{original}: {len(ours)} lines here, {len(theirs)} in LLVM")
                continue
            for a, b in zip(ours, theirs):
                k = a.split()[0]
                total["compared"] += 1
                if a == b:
                    continue
                verdict = classify(a.split(None, 2)[2] if a.count(" ") >= 2 else a, b.split(None, 2)[2])
                total[verdict] += 1
                kinds[(k, verdict)] += 1
                if (kind is None or kind == k) and shown < show and verdict != "LESS" or verbose and kind in (None, k):
                    shown += 1
                    print(f"{verdict} {original}\n  llrm: {a}\n  llvm: {b}")
        differ = total["MORE"] + total["LESS"] + total["OTHER"] + total["LINES"]
        print(
            f"{path}: {total['compared']} answers compared, {differ} differ "
            f"(MORE {total['MORE']}, LESS {total['LESS']}, OTHER {total['OTHER']}), "
            f"{skipped} of {len(files)} files skipped"
        )
        for (k, verdict), count in sorted(kinds.items()):
            print(f"    {k:6} {verdict:5} {count}")
        if total["MORE"] or total["OTHER"] or total["LINES"]:
            status = 1
    return status


if __name__ == "__main__":
    sys.exit(main())
