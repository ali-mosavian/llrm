"""tools/sizes.py measures each build with that build's own frontend."""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))
import sizes  # noqa: E402

# Writes the object only when run with the qbfront beside it.
FAKE = """#!/bin/sh
[ "$QBOPT_QBFRONT" = "{bins}/qbfront" ] || exit 1
while [ "$1" != "-o" ]; do shift; done
echo x > "$2"
"""


def _bins(tmp_path: Path, qbfront: bool) -> Path:
    for name in ("llrm-qb", "llrm-c", "llrm-nib"):
        (tmp_path / name).write_text(FAKE.format(bins=tmp_path.resolve()))
        (tmp_path / name).chmod(0o755)
    if qbfront:
        (tmp_path / "qbfront").write_text("")
    return tmp_path


def test_a_copied_build_compiles_basic_with_its_own_qbfront(tmp_path):
    """A baseline's llrm-qb ran qbfront from the tree it was built in, as that
    tree was later: ~/scratch/baseline/2d11a7b7 sized divmod.bas 4693 bytes, a
    fresh 2d11a7b7 4100."""
    built = sizes.table(_bins(tmp_path, qbfront=True), "-O2", demos=False)
    assert built["suite/divmod.bas"] is not None


def test_a_copied_build_without_qbfront_is_refused(tmp_path):
    with pytest.raises(SystemExit):
        sizes.table(_bins(tmp_path, qbfront=False), "-O2", demos=False)
