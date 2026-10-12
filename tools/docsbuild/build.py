#!/usr/bin/env python3
"""Keep the generated documentation current.

    python3 tools/docsbuild/build.py --sync     write the generated files into docs/
    python3 tools/docsbuild/build.py --check    exit 1 if --sync would change a file, or a source is wrong

--sync renders every figure source (docs/assets/figures/*.fig, and the ```fig fences in
`//!` headers) to an SVG, writes the pass, analysis and backend reference to
docs/optimizations/reference/ from the headers, and moves a ```fig fence found in a
document to a .fig, leaving the image reference. Commit what it writes: docs/ is read
as committed.
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE.parent))

from docsbuild import sync  # noqa: E402


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--sync", action="store_true", help="write the generated files into docs/")
    ap.add_argument("--check", action="store_true", help="fail on stale generated files or a bad figure source")
    args = ap.parse_args(argv)
    if not (args.sync or args.check):
        ap.error("give --sync or --check")
    p = sync.plan(ROOT)
    errors = list(p.errors)
    if args.sync and not errors:
        sync.apply(ROOT, p)
        print(f"synced {len(p.writes)} written, {len(p.deletes)} deleted")
    elif args.check:
        errors += sync.fence_errors(ROOT)
        if p.drift and not p.errors:
            errors.append(sync.drift_message(p))
    errors = list(dict.fromkeys(errors))
    print("\n".join(errors))
    if errors:
        print(f"{len(errors)} problem(s)", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
