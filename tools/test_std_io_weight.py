"""std.io opened and created a file through two copies of the name path: the 80-byte buffer, its `rep stosb` zero fill and the call to DOS.
A program of the library's file calls and nothing else was 232 bytes of code at -Os (414 at -O1); every program that reads or writes a
file pays this (stft_fixed, jpeg_fixed). One `@noipa` handle path makes it 203 (383)."""
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
sys.path.insert(0, str(ROOT / "tools/ab"))
import llrmbin  # noqa: E402

BOUND = {"-Os": 203, "-O1": 383}


@pytest.mark.parametrize("level", sorted(BOUND))
def test_std_io_files_cost_one_copy_of_the_name_path(level):
    import object_bytes

    nib = llrmbin.bin_dir() / "llrm-nib"
    if not nib.exists():
        pytest.skip(f"{nib}: not built")
    with tempfile.TemporaryDirectory() as tmp:
        obj = Path(tmp) / "o.obj"
        subprocess.run([str(nib), str(ROOT / "tests/fixtures/nib/io_skeleton.nib"), "-o", str(obj), level, "--procedure-segments"],
                       check=True, capture_output=True)
        code = object_bytes.footprint("x86_16", obj).code
    assert code <= BOUND[level], f"{code} bytes of code at {level}"
