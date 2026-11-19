"""Copy a binary into a dist directory without ever writing through a symlink there.

A hybrid dist links most of its binaries to another dist's; a plain copy over such a link replaces the other dist's file. The copy
is refused when the destination is a link, or resolves outside the dist; remove the link first to replace it.

    python3 tools/distcopy.py SRC DISTDIR
"""

import shutil
import sys
from pathlib import Path


class Refused(Exception):
    pass


def copy_into(source: Path, dist: Path) -> Path:
    target = dist / source.name
    if target.is_symlink() or target.resolve().parent != dist.resolve():
        raise Refused(f"{target} is a link to {target.resolve()}: remove it first to replace it with a copy")
    shutil.copyfile(source, target)
    shutil.copymode(source, target)
    return target


if __name__ == "__main__":
    try:
        copy_into(Path(sys.argv[1]), Path(sys.argv[2]))
    except Refused as error:
        sys.exit(f"distcopy: {error}")
