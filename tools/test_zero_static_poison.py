"""`LLRM_BIN=<dist> python -m pytest tools/test_zero_static_poison.py`

A C static with no initialiser is zero, but DOS gives a program whatever its memory held: a zero the linked image does not store
must come from the start-up. #8702 left zero tails out of the object, jwlink trimmed them from the image, and a `__huge` array read
0xCCCC under a poisoned run (image 98720 -> 1932 B). The runs start with memory filled with 0xCC."""

import os
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))
sys.path.insert(0, str(Path(__file__).parent / "dosbatch"))
sys.path.insert(0, str(Path(__file__).parents[1] / "crates/target/llrm-x86-m16/bench"))
sys.path.insert(0, str(Path(__file__).parents[1] / "crates/target/llrm-x86-m32/vsgcc"))

try:
    import dosbatch
    import icount
    import llrmbin
except (SystemExit, ImportError) as why:  # no built binaries or no emulator
    pytest.skip(f"toolchain: {why}", allow_module_level=True)

HUGE = "extern void report(long);\nshort __huge h[200][201];\nint main(void){ long s = 0; short r, c; for (r = 0; r < 200; r += 7) for (c = 0; c < 201; c += 5) s += h[r][c]; s += h[199][200]; report(s); return 0; }\n"
TAIL = "extern void report(long);\nint a[100] = {1, 2, 3};\nchar b[3000];\nint main(void){ long s = 0; int i; for (i = 3; i < 100; i++) s += a[i]; for (i = 0; i < 3000; i++) s += b[i]; report(s); return 0; }\n"


def reads(source: str, row: str) -> str:
    """What the program prints with memory poisoned: the sum of statics that must be zero."""
    work = Path(tempfile.mkdtemp())
    (work / "t.c").write_text(source)
    obj, exe = work / "t.obj", work / "t.exe"
    dos = row.endswith("dos")
    include = ["-I", str(dosbatch.c_include(dosbatch.REAL_MODE, work))] if dos else []
    command = [str(dosbatch.BIN / "llrm-cc"), "-O1", *llrmbin.c_args(16)[1:], f"--target={row}", "-c", *include, str(work / "t.c"), "-o", str(obj)]
    subprocess.run(command, env={**os.environ, **llrmbin.cc_environment()}, check=True, capture_output=True)
    defines = {"defines": dosbatch.entry_defines(dosbatch.REAL_MODE, None)} if dos else {}
    dosbatch.link_target(dosbatch.REAL_MODE, obj, exe, work, work / "t.map", **defines)
    return icount.run(exe, None, work, whole=True, poison=0xCC).output.decode("latin1").strip()


@pytest.mark.parametrize("row", ["x86_16-wat", "x86_16-dos"])
def test_an_uninitialised_huge_array_reads_zero_when_the_machine_does_not_clear_memory(row):
    assert reads(HUGE, row) == "0"


@pytest.mark.parametrize("row", ["x86_16-wat", "x86_16-dos"])
def test_the_zero_tail_of_an_initialised_array_and_bss_read_zero_when_the_machine_does_not_clear_memory(row):
    assert reads(TAIL, row) == "0"
