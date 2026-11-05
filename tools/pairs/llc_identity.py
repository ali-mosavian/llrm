#!/usr/bin/env python3
"""Two llc binaries over the same IR: no input that both accept changes its assembly.

    llc_identity.py --pre BIN --post BIN [--flags "-O2 -m32"] [--datalayout STR] FILE.ll...

An input both refuse is counted apart (identical error text is not identical output). Exit 0 where something was compared and nothing differs,
1 where an input differs (or one binary refuses what the other takes), 2 where nothing was compared: an empty comparison reads as a pass.
`--datalayout` rewrites each input's `target datalayout` line first (a front end's layout is not always the compiler's own).
"""
import argparse
import re
import subprocess
import sys
import tempfile
from pathlib import Path


def run(binary: str, flags: list[str], source: Path, out: Path) -> tuple[int, bytes]:
    result = subprocess.run([binary, *flags, "-S", "-o", str(out), str(source)], capture_output=True)
    return result.returncode, out.read_bytes() if result.returncode == 0 and out.exists() else b""


def compare(pre: str, post: str, flags: list[str], sources: list[Path], datalayout: str | None = None) -> tuple[int, list[str], int]:
    """(compared, names that differ, refused by both)."""
    compared, differing, both = 0, [], 0
    with tempfile.TemporaryDirectory() as work:
        work = Path(work)
        for source in sources:
            if datalayout is not None:
                rewritten = work / source.name
                rewritten.write_text(re.sub(r'(?m)^target datalayout = .*$', f'target datalayout = "{datalayout}"', source.read_text()))
                source = rewritten
            (ra, a), (rb, b) = run(pre, flags, source, work / "pre.s"), run(post, flags, source, work / "post.s")
            if ra != 0 and rb != 0:
                both += 1
                continue
            compared += 1
            if ra != rb or a != b:
                differing.append(f"{source.name} (rc {ra}/{rb})")
    return compared, differing, both


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pre", required=True)
    parser.add_argument("--post", required=True)
    parser.add_argument("--flags", default="-O2")
    parser.add_argument("--datalayout")
    parser.add_argument("sources", nargs="+")
    args = parser.parse_args()
    compared, differing, both = compare(args.pre, args.post, args.flags.split(), [Path(s) for s in args.sources], args.datalayout)
    for name in differing:
        print(f"DIFF {name}")
    print(f"llc_identity: compared {compared}, differing {len(differing)}, refused by both {both}")
    return 2 if compared == 0 else (1 if differing else 0)


if __name__ == "__main__":
    sys.exit(main())
