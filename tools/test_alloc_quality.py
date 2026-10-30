"""What one allocation of a body leaves: counted, not timed."""
from __future__ import annotations

import os
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import llrmbin  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent


def test_a_value_a_rewrite_displaced_may_evict_before_it_is_spilled():
    """The 8x8 matmul kernel's inner counter (weight 34) lost its register to the reloads a spill of another value put in the loop, was
    re-queued as already placed (`Done`), skipped the eviction stage, and was spilled: 4,426 memory operands executed with one allocation
    (-fno-allocation-routes -fno-allocation-search), 1,610 when it may evict the loop-invariant values (weights 0.3-0.6) that held registers
    it needed. LLVM's greedy allocator queues an unassigned interval at RS_Assign."""
    compiler = llrmbin.bin_dir() / "llrm-qb"
    done = subprocess.run(
        [str(compiler), str(ROOT / "bench/matmul/matmul.bas"), "--dialect", "qb45", "-fqb-runtime=llrm", "-O2", "-fno-inline-functions",
         "-fno-inline-functions-called-once", "-fno-allocation-search", "-S", "-o", os.devnull],
        env={**os.environ, "LLRM_DEBUG": "cost"}, capture_output=True, text=True, check=True)
    found = re.search(r"BENCHMATMUL& executes \d+ instructions, (\d+) memory operands", done.stderr)
    assert found, done.stderr[-500:]
    assert int(found.group(1)) < 2500, f"matmul with one allocation executes {found.group(1)} memory operands (2500 allowed; 4426 before)"
