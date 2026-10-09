"""llrm-c finishes: a generated program that once made the register allocator spill its own spills, without end."""
import subprocess
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(next((HERE.parent / "crates/target").glob("*/vsgcc"))))
import llrmbin  # noqa: E402
import scaling  # noqa: E402


@pytest.mark.parametrize("level", ["-O1", "-O2", "-O3"])
@pytest.mark.parametrize("n", [7, 8, 12])
def test_the_chain_axis_compiles_at_16_bits(tmp_path, level, n):
    """`chain` at N=7 with -m16 -O1/-O2/-O3 never finished: a value spilled in the allocator was merged with its updates into a
    register value, which was spilled and merged again (the rewrite made a value it did not report, then spilled it, then ...)."""
    source = tmp_path / "chain.c"
    source.write_text(scaling.AXES["chain"](n))
    done = subprocess.run([str(llrmbin.bin_dir() / "llrm-c"), "-m16", level, str(source), "-o", str(tmp_path / "chain.obj")], capture_output=True, text=True, timeout=30)
    assert done.returncode == 0, done.stderr[-400:]
