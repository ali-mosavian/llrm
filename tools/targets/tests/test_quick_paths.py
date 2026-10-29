"""The x87 quick paths of runtime/qb/i8out.c stay on at every x86 row, the Watcom rows' 53-bit long double included.

A guard of `__LDBL_MANT_DIG__ == 64` turned them off at x86_16-wat, whose -mlong-double-64 makes it 53: the DOS runtime's objects changed and gate run 4's NIBBLES m16 demo died (38 pixels)."""
import os
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[3]
CC = Path(os.environ.get("LLRM_BIN", ROOT / "target/release")) / "llrm-cc"
ROWS = ["x86_16-wat", "x86_32-wat", "x86_16-dos", "x86_32-dos"]


@pytest.mark.parametrize("row", ROWS)
def test_the_quick_path_is_on_at_the_x86_row(row):
    if not CC.is_file() or not os.environ.get("LLRM_CLANG_M16"):
        pytest.skip("needs target/release/llrm-cc and LLRM_CLANG_M16 (the pinned m16 clang)")
    out = subprocess.run(
        [CC, "-target", row, "-E", "-dM", "-I", "runtime/qb", "-I", "runtime/qb/dos/m16", "-I", "runtime/qb/dos/m32", "-I", "runtime/qb/dos", "runtime/qb/i8out.c"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout
    assert "#define QUICK_EXTENDED 1" in out
