#!/usr/bin/env python3
"""Callgrind callee splits, checked: every call edge between our own files must name the callee in the caller's source file.

    tools/callgrind-check.py record [--bin llrm-c] [--out profile.out] [--cwd DIR] -- ARGS...   # run under callgrind
    tools/callgrind-check.py check profile.out [--top 20] [--under CALLER]                      # exit 1 on a false edge

Record with a dist build that has line tables (`CARGO_PROFILE_DIST_DEBUG=line-tables-only cargo build --profile dist --bin llrm-c`);
`record` runs callgrind with `--read-inline-info=yes --dump-instr=no --compress-pos=no --compress-strings=no`. Pass totals
(a function's inclusive row) are reliable; a callee split is not, because under fat LTO identical-code folding and merged
tails put a callee under a caller that never calls it (machinedce::eliminated under tails_merged, parity 59c834686).
`check` reads each edge `caller -> callee` and fails when the caller's source file never spells the callee's name: a
callee under a caller in another module (tails_merged -> eliminated) is such an edge. A split is trusted only for a
profile that passes. Edges into closures, drop glue, std and the allocator are not checked (no name at the call site).
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

CALLGRIND = [
    "valgrind", "--tool=callgrind", "--read-inline-info=yes", "--dump-instr=no", "--compress-pos=no", "--compress-strings=no",
]
IMPLICIT = {"deref", "deref_mut", "index", "index_mut", "clone", "drop", "from", "into", "eq", "ne", "cmp", "partial_cmp", "hash", "fmt", "default", "next", "borrow", "as_ref", "as_mut", "try_from", "try_into", "extend", "from_iter", "into_iter", "neg", "not"}
SKIP_CALLEE = re.compile(r"\{closure|drop_glue|^core::|^alloc::|^std::|\.S:|^\?\?\?|<alloc::|<core::|<std::")


def short(name: str) -> str:
    """The identifier a call site spells: `<a::B>::run::<T>` -> `run`, `a::b::f` -> `f`."""
    name = re.sub(r"::<[^()]*>$", "", name.strip())
    name = re.sub(r"<[^<>]*>", "", name) if "<" in name else name
    last = name.rsplit("::", 1)[-1].strip(" ()'0123456789").rstrip("'")
    return re.sub(r"'\d+$", "", last)


def parse(text: str):
    """The call edges with their call-site line: the line after `calls=` is `line cost`."""
    out = []
    file = fn = cfile = cfn = None
    want = None
    for raw in text.splitlines():
        if want is not None:
            parts = raw.split()
            if parts and parts[0].isdigit():
                out.append((want[0], want[1], int(parts[0]), want[2], want[3], want[4]))
            want = None
            continue
        if raw.startswith(("fl=", "fi=", "fe=")):
            file = raw[3:].strip()
        elif raw.startswith("fn="):
            fn = raw[3:].strip()
        elif raw.startswith(("cfi=", "cfl=")):
            cfile = raw[4:].strip()
        elif raw.startswith("cfn="):
            cfn = raw[4:].strip()
        elif raw.startswith("calls="):
            want = (file, fn, cfn, cfile, int(raw[6:].split()[0]))
    return out


def ours(path: str | None) -> bool:
    return bool(path) and "/crates/" in path or bool(path) and "/src/" in path and "/rustc/" not in path and "/.cargo/" not in path


def check(text: str, read=lambda path: Path(path).read_text().splitlines()):
    """The edges whose call line does not name the callee, as (caller, line, callee, calls) heaviest first."""
    bad = []
    cache: dict[str, list[str]] = {}
    for file, fn, line, cfn, cfile, count in parse(text):
        if not ours(file) or not ours(cfile) or not cfn or SKIP_CALLEE.search(cfn):
            continue
        name = short(cfn)
        if len(name) < 3 or name in IMPLICIT or line == 0:
            continue
        try:
            lines = cache.setdefault(file, read(file))
        except OSError:
            continue
        if name not in "\n".join(lines):
            bad.append((f"{fn} ({Path(file).name}:{line})", line, cfn, count))
    return sorted(bad, key=lambda row: -row[3])


def record(args: argparse.Namespace) -> int:
    command = [*CALLGRIND, f"--callgrind-out-file={args.out}", args.bin, *args.rest]
    return subprocess.run(command, cwd=args.cwd).returncode


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="mode", required=True)
    rec = sub.add_parser("record")
    rec.add_argument("--bin", default="llrm-c")
    rec.add_argument("--out", default="profile.out")
    rec.add_argument("--cwd", default=".")
    rec.add_argument("rest", nargs=argparse.REMAINDER)
    chk = sub.add_parser("check")
    chk.add_argument("profile")
    chk.add_argument("--top", type=int, default=20)
    chk.add_argument("--under", help="trust the split of the callers whose name has this text: only their edges are checked")
    args = parser.parse_args(argv)
    if args.mode == "record":
        args.rest = [a for a in args.rest if a != "--"]
        return record(args)
    bad = check(Path(args.profile).read_text())
    if args.under:
        bad = [row for row in bad if args.under in row[0]]
    for caller, _, callee, count in bad[: args.top]:
        print(f"false edge: {caller} -> {callee} ({count}x)")
    if bad:
        print(f"{len(bad)} call edges name a callee their call line does not: callee splits of this profile are not trusted", file=sys.stderr)
        return 1
    print("every checked call edge names its callee")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
