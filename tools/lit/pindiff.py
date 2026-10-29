#!/usr/bin/env python3
"""Runs each `opt` RUN line of the given tests through the pin's opt and llrm-opt and compares the outputs.

    pindiff.py [--llrm-opt PATH] [--pin-opt PATH] [--show N] TEST.ll|DIR...

Columns: `text` (same after dropping comments, attribute groups and
attributes), `shape` (same after also numbering values and blocks by first
use), `differs`, `unsupported` (llrm-opt exit 77 or 78), `pin-fails`. The
shape is what the pass decided; the names and attribute groups are the
printer's and are not compared.
"""
import argparse
import collections
import re
import shlex
import subprocess
import sys
from pathlib import Path

HOME = Path.home()
ATTRS = (
    r"nounwind|uwtable|readonly|readnone|ssp|nocapture|noundef|nonnull|willreturn|nosync|nofree|norecurse|"
    r"dso_local|captures\(none\)|memory\([^)]*\)|noinline|optnone|mustprogress|local_unnamed_addr|nsw|nuw"
)


def text_form(output):
    lines = []
    for line in output.splitlines():
        line = re.sub(r"\s*;.*$", "", line)
        if not line.strip() or line.startswith(("source_filename", "attributes #", "target ")) or re.match(r"^%[-\w.$\"]+ = type\b", line):
            continue
        line = re.sub(r"\s#\d+", "", line)
        line = re.sub(r"(?<![\w.])(" + ATTRS + r")(?![\w.])", "", line)
        lines.append(re.sub(r"\s+", " ", line).strip())
    return "\n".join(lines)


def shape_form(output):
    """The text form with `%name` values and block labels numbered by first use, per function."""
    numbering, out = {}, []
    for line in text_form(output).splitlines():
        if line.startswith("define"):
            numbering = {}
        label = re.match(r"^([-\w.$]+):", line)
        if label:
            line = re.sub(r"^[-\w.$]+:", "%" + label.group(1) + ":", line)

        def number(match):
            return "%v" + str(numbering.setdefault(match.group(0), len(numbering)))

        line = re.sub(r"%[-\w.$]+", number, line)
        out.append(line.replace(":", "", 1) if label else line)
    return "\n".join(out)


def run_lines(path):
    text = path.read_text()
    joined = re.sub(r"\\\n[;#]\s*RUN:", " ", re.sub(r"\\\s*\n\s*[;#]\s*RUN:", " ", text))
    return re.findall(r"^[;#]\s*RUN:\s*(.*)$", joined, re.M)


def command(line, test, opt):
    """argv and stdin text for the first `opt` of a RUN line, none if it is not one or needs `%t`."""
    first = line.split("|")[0].strip()
    if "%t" in first or not re.match(r"^(not )?opt\b", first) or first.startswith("not "):
        return None
    stdin = None
    if "< %s" in first:
        first, stdin = first.replace("< %s", ""), test
    words = [w.replace("%s", str(test)) for w in shlex.split(first)[1:]]
    if "-S" not in words:
        words.append("-S")
    return words, stdin


def execute(binary, words, stdin):
    try:
        done = subprocess.run(
            [binary, *words],
            stdin=open(stdin) if stdin else subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=60,
        )
    except subprocess.TimeoutExpired:
        return 124, ""
    return done.returncode, done.stdout


def compare(test, args):
    verdicts = []
    for line in run_lines(test):
        built = command(line, test, args.pin_opt)
        if not built:
            continue
        words, stdin = built
        pin_code, pin_out = execute(args.pin_opt, words, stdin)
        code, out = execute(args.llrm_opt, words, stdin)
        if pin_code != 0:
            verdicts.append("pin-fails")
        elif code in (77, 78):
            verdicts.append("unsupported")
        elif code != 0:
            verdicts.append("differs")
        elif text_form(pin_out) == text_form(out):
            verdicts.append("text")
        elif shape_form(pin_out) == shape_form(out):
            verdicts.append("shape")
        else:
            verdicts.append("differs")
    order = ["pin-fails", "unsupported", "differs", "shape", "text"]
    return min(verdicts, key=order.index) if verdicts else None


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--llrm-opt", default="target/debug/llrm-opt")
    parser.add_argument("--pin-opt", default=str(HOME / "scratch/llvm-build/bin/opt"))
    parser.add_argument("paths", nargs="+", type=Path)
    args = parser.parse_args()
    tests = [t for p in args.paths for t in (sorted(p.rglob("*.ll")) if p.is_dir() else [p])]
    table = collections.defaultdict(collections.Counter)
    for test in tests:
        verdict = compare(test, args)
        if verdict:
            table[str(test.parent.name)][verdict] += 1
            if verdict == "differs":
                print(f"differs: {test}", file=sys.stderr)
    kinds = ["text", "shape", "differs", "unsupported", "pin-fails"]
    print("| directory | " + " | ".join(kinds) + " |\n|---|" + "---|" * len(kinds))
    for directory, counts in sorted(table.items()):
        print(f"| {directory} | " + " | ".join(str(counts[k]) for k in kinds) + " |")


if __name__ == "__main__":
    main()
