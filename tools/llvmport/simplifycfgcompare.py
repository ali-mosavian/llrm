#!/usr/bin/env python3
"""SimplifyCFG's answers, LLVM's `opt -passes=simplifycfg` against llrm's, on .ll files.

    tools/llvmport/simplifycfgcompare.py [--hoist-common-insts] PATH... [--show N]

PATH is a .ll file or a directory (every `.ll` in it). llrm's side is the `simplifycfg` example of llrm-transforms (CARGO_TARGET_DIR is
honoured), LLVM's the pinned `opt` (tools/difftest). Both are read through difftest's `normalise` and `canonical`. A file either side cannot
read is skipped and counted. Per function: identical, or the first hunk. Exit 1 on a difference, when --strict.
"""
import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools/difftest"))
import difftest  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("paths", nargs="+", type=Path)
    parser.add_argument("--hoist-common-insts", action="store_true")
    parser.add_argument("--sink-common-insts", action="store_true")
    parser.add_argument("--no-keep-loops", action="store_true", help="needCanonicalLoops(false)")
    parser.add_argument("--show", type=int, default=3)
    parser.add_argument("--strict", action="store_true")
    parser.add_argument("--list", action="store_true", help="one line per function: FILE@NAME identical|differs")
    args = parser.parse_args()
    example = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "release/examples/simplifycfg"
    named = [
        name
        for name, on in (
            ("no-keep-loops", args.no_keep_loops),
            ("hoist-common-insts", args.hoist_common_insts),
            ("sink-common-insts", args.sink_common_insts),
        )
        if on
    ]
    passes = f"simplifycfg<{';'.join(named)}>" if named else "simplifycfg"
    files = [f for p in args.paths for f in ([p] if p.is_file() else sorted(p.glob("*.ll")))]
    skipped, same, differ, shown = 0, 0, 0, 0
    for file in files:
        text = file.read_text(errors="replace")
        text = "\n".join(line for line in text.split("\n") if not line.startswith("target triple"))
        with tempfile.NamedTemporaryFile("w", suffix=".ll", delete=False) as handle:
            handle.write(text)
        flags = [f"--{name}" for name in named]
        done = subprocess.run([str(example), *flags, handle.name], capture_output=True, text=True)
        os.unlink(handle.name)
        if done.returncode:
            skipped += 1
            continue
        report = difftest.compare(text, done.stdout, passes)
        if "error" in report:
            skipped += 1
            continue
        for name, row in report.items():
            if args.list:
                print(f"{file.name}@{name} {'identical' if row['identical'] else 'differs'}")
            if row["identical"]:
                same += 1
                continue
            differ += 1
            if shown < args.show:
                shown += 1
                print(f"--- {file.name} @{name}")
                print("\n".join(row["hunk"]))
    print(f"{passes}: files {len(files)}, skipped {skipped}, functions identical {same}, differing {differ}")
    return 1 if args.strict and differ else 0


if __name__ == "__main__":
    sys.exit(main())
