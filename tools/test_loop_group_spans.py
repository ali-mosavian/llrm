"""`uv run --project tools python -m pytest tools/test_loop_group_spans.py`

The loop pass manager's group (indvars, loop-deletion, loop-unroll-full, indvars) ran inside the span of its first slot, so
`LLRM_DEBUG=time` and `compile_map` charged the unroll's 115 M of a 40-loop function to a row named `mir indvars@738` (148 M
own) and callgrind said indvars was 2% of the compile. Each part is a step of its own."""
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import llrmbin  # noqa: E402

LOOPS = 6


def source() -> str:
    body = "".join(f"  for (int i = 0; i < n; ++i) {{ acc += a[i] * {k}.5; t += ia[(i + {k}) & 63]; }}\n" for k in range(LOOPS))
    return f"double a[64]; int ia[64];\ndouble f(int n) {{ double acc = 0; int t = 0;\n{body}  return acc + t; }}\n"


def rows() -> dict[str, tuple[float, int]]:
    compiler = llrmbin.bin_dir() / "llrm-cc"
    pin = os.environ.get("LLRM_PIN_CLANG")
    if not compiler.exists() or not pin:
        pytest.skip("llrm-cc is not built or LLRM_PIN_CLANG is not set")
    with tempfile.TemporaryDirectory() as work:
        path = Path(work) / "loops.c"
        path.write_text(source())
        with llrmbin.with_object([str(compiler), "-m32", "-march=i486", "-w", "-O2", "-c", "-o", llrmbin.OBJECT, str(path)]) as command:
            done = subprocess.run(command, capture_output=True, text=True, env={**os.environ, "LLRM_DEBUG": "time", "LLRM_TIME_TOP": "100000", "LLRM_PIN_CLANG": pin, "LLRM_CC_TARGET": "x86_32-linux"})
    assert done.returncode == 0, done.stderr[-500:]
    found = re.findall(r"^\[instr\]\s+([\d.]+) Minstr own\s+[\d.]+ \S+ total\s+(\d+)x (.+)$", done.stderr, re.M)
    if not found:
        pytest.skip("no instruction counter on this host")
    return {name: (float(own), int(count)) for own, count, name in found}


def test_the_loop_groups_parts_are_steps_of_their_own():
    run = rows()
    assert "mir loopfullunroll" in run, f"the unroll inside the group has no row: {sorted(run)}"
    assert run["mir loopfullunroll"][1] >= LOOPS, f"the unroll ran once a loop, not {run['mir loopfullunroll']}"
    head_own = run["mir indvars@738"][0]
    assert head_own < 0.2 * max(run["mir loopfullunroll"][0] + run["mir indvars"][0], 1.0), f"the group's slot still holds its parts' work: {head_own} M own, {run}"
