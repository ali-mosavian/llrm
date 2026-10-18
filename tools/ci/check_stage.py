#!/usr/bin/env python3
"""Exit 1 naming every dangling symlink under a staged toolchain: gcc's dev links to the system's sanitizer and runtime libraries
(libasan.so -> ../../../x86_64-linux-gnu/libasan.so.8) were staged without their targets, so a link that needs one failed late.

    tools/ci/check_stage.py STAGE
"""
import sys
from pathlib import Path


def dangling(stage: Path) -> list[Path]:
    return sorted(p for p in stage.rglob("*") if p.is_symlink() and not p.exists())


def main() -> int:
    bad = dangling(Path(sys.argv[1]))
    for p in bad:
        print(f"dangling: {p} -> {p.readlink()}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
