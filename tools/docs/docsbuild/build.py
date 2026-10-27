#!/usr/bin/env python3
"""Keep the generated documentation current, and the mermaid figures well formed.

    python3 tools/docs/docsbuild/build.py --sync            write the pass, analysis and backend pages
    python3 tools/docs/docsbuild/build.py --sync --force    ... ignoring the digests that say what is up to date
    python3 tools/docs/docsbuild/build.py --check           exit 1 if --sync would change a file, a figure is malformed or a header has no `# Example`
    python3 tools/docs/docsbuild/build.py --fix-style       give the figures of docs/ (not the generated pages) the shared init line
    python3 tools/docs/docsbuild/build.py --check --render  also draw each figure with mmdc ($MMDC or PATH; skipped if absent)

--sync also rewrites the Contents list of docs/architecture/readme.md from the pages' front matter (archindex.py).
--sync writes one page per documented source file under docs/optimizations/{passes,analyses,backend}/ from its
`//!` header, with an index in each. Commit what it writes: docs/ is read as committed. Only a page whose header
(or whose link targets) changed is written again. --check lints every ```mermaid fence of a tracked Markdown
file and of a `//!` header, and rejects a legacy ```fig fence. Figures are written as mermaid; see
docs/working/figures.md.
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE.parent))

from docsbuild import archindex, lint, sync  # noqa: E402


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--sync", action="store_true", help="write the generated pages into docs/")
    ap.add_argument("--check", action="store_true", help="fail on stale pages or a malformed figure")
    ap.add_argument("--force", action="store_true", help="with --sync, write every page again")
    ap.add_argument("--render", action="store_true", help="with --check, draw each figure with mmdc")
    ap.add_argument("--fix-style", action="store_true", help="give the hand-written figures the shared init line")
    args = ap.parse_args(argv)
    if args.fix_style:
        print("\n".join(lint.fix_style_tree(ROOT)))
        return 0
    if not (args.sync or args.check):
        ap.error("give --sync, --check or --fix-style")
    p = sync.plan(ROOT, force=args.force)
    errors = list(p.errors)
    if args.sync and not errors:
        sync.apply(ROOT, p)
        print(f"synced {len(p.writes)} written, {len(p.deletes)} deleted" + ("; architecture index written" if archindex.sync(ROOT) else ""))
    if args.check:
        if p.drift:
            errors.append(sync.drift_message(p))
        errors += sync.missing_example(ROOT)
        if archindex.drift(ROOT):
            errors.append("docs/architecture/readme.md: the Contents list is stale; run python3 tools/docs/docsbuild/build.py --sync")
        errors += lint.check_tree(ROOT)
        if args.render:
            errors += lint.render_errors(ROOT)
    errors = list(dict.fromkeys(errors))
    print("\n".join(errors))
    if errors:
        print(f"{len(errors)} problem(s)", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
