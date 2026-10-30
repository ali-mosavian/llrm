"""`--isel=dag` refused tests/llc/fnptr.c at m32 wat -O2 (`Unsupported("a sibling call with no free register")`): the sibling-call check counted ecx free, the emitter's volatile set under the Watcom convention is {eax}. The DOS-path llrm-cc check's one red; it must compile, as the pin's eligibility check leaves such a call a call."""
from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import llrmbin  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
OW = Path(os.environ.get("LLRM_OW", Path.home() / "scratch/ow-src"))


@pytest.mark.skipif(not OW.is_dir(), reason="LLRM_OW (the Open Watcom headers) is not available")
def test_an_indirect_call_through_every_volatile_register_compiles_under_dag_at_m32_wat():
    binary = llrmbin.bin_dir() / "llrm-cc"
    env = {**os.environ, "LLRM_CC_TARGET": "x86_32-wat", "LLRM_OW": str(OW)}
    with tempfile.TemporaryDirectory() as work:
        done = subprocess.run(
            [str(binary), "-O2", "-fno-builtin", "--isel=dag", "-Dfar=", "-Dnear=", "-Dhuge=", str(ROOT / "tests/llc/fnptr.c"), "-c", "-o", f"{work}/x.obj"],
            capture_output=True,
            text=True,
            env=env,
        )
    assert done.returncode == 0, done.stderr
