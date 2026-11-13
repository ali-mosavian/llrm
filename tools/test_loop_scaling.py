"""`uv run --project tools python -m pytest tools/test_loop_scaling.py`

One function of sequential counted loops, 4x the loops: each step's own instructions (`LLRM_DEBUG=time` `[instr]` rows) must
grow about as the program does (4x), not as its square (16x). gcc and clang both take 4.8-4.9x in total; llrm took 8.0x
(1,333 -> 10,714 M at 40 -> 160 loops, -m32 -O2, 2026-11-13). A step over the limit is a pass that asks the whole function
again for each loop."""
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import llrmbin  # noqa: E402

SMALL, LARGE = 20, 80
# What 4x the loops may cost (4 is linear). Steps not named are held to DEFAULT. The named ones are mid-end passes measured over
# it on 2026-11-13 (20 -> 80 loops, release/next a12d6adfdb) and are owed a fix (owner: compile-time-bench / w8): each limit is
# the measured growth plus 15%, lowered when its step is fixed. `unroll` and `indvars` are fixed (the float liveness of DEVIATE U3 is
# followed from the reads, 15.1x -> 4.1x and 14.7x -> 6.0x on 2026-11-13); the others are not yet diagnosed.
DEFAULT = 6.0
OWED = {
    "mir indvars": 6.9,
    "mir unroll": 4.8,
    "mir rotate": 11.3,
    "mir constraint-elimination": 9.6,
    "mir loopidioms": 9.3,
    "mir jumpthreading": 7.7,
}
FLOOR_MINSTR = 12.0  # a step under this many M at LARGE is noise


def sequential(k: int) -> str:
    out = ["double a[256], b[256]; int ia[256], ib[256]; unsigned char ch[256]; void use(double, int);", "double f(int n, int m, double s) {", "  double acc = s; int t = 0;"]
    for i in range(k):
        out.append(f"  for (int i = 0; i < n; ++i) {{ acc += a[i] * b[(i + {i}) & 255] + {i}.5; t += ia[i] ^ ib[(i * 3 + {i}) & 255]; ch[i & 255] = (unsigned char)(t >> {i % 7}); }}")
        out.append(f"  if (t & {i + 1}) {{ a[{i % 200}] = acc; ib[{i % 200}] = t + m; }} else use(acc, t - {i});")
    out += ["  return acc + t;", "}"]
    return "\n".join(out)


def steps(compiler: Path, source: Path) -> dict[str, float]:
    with llrmbin.with_object([str(compiler), "-m32", "-march=i486", "-w", "-O2", "-c", "-o", llrmbin.OBJECT, str(source)]) as command:
        done = subprocess.run(command, capture_output=True, text=True, env={**os.environ, "LLRM_DEBUG": "time", "LLRM_TIME_TOP": "100000", "LLRM_PIN_CLANG": os.environ.get("LLRM_PIN_CLANG", ""), "LLRM_CC_TARGET": "x86_32-linux"})
    assert done.returncode == 0, done.stderr[-500:]
    rows = re.findall(r"^\[instr\]\s+([\d.]+) (Minstr|Mcpu-ns) own\s+[\d.]+ \S+ total\s+\d+x (.+)$", done.stderr, re.M)
    if not rows or any(unit != "Minstr" for _, unit, _ in rows):
        pytest.skip("no instruction counter on this host: the rows are CPU time")
    cost: dict[str, float] = {}
    for own, _, name in rows:
        name = re.sub(r"@\d+$", "", name)  # the pipeline slot's line moves with the source
        cost[name] = cost.get(name, 0.0) + float(own)
    return cost


def test_every_step_grows_about_as_the_loops_do():
    compiler = llrmbin.bin_dir() / "llrm-cc"
    if not compiler.exists() or not os.environ.get("LLRM_PIN_CLANG"):
        pytest.skip("llrm-cc is not built or LLRM_PIN_CLANG is not set")
    with tempfile.TemporaryDirectory() as work:
        run = {}
        for k in (SMALL, LARGE):
            source = Path(work) / f"k{k}.c"
            source.write_text(sequential(k))
            run[k] = steps(compiler, source)
    grown = {name: run[LARGE][name] / max(run[SMALL].get(name, 0.0), 0.5) for name in run[LARGE] if run[LARGE][name] >= FLOOR_MINSTR}
    over = {name: round(ratio, 1) for name, ratio in grown.items() if ratio > OWED.get(name, DEFAULT)}
    assert not over, f"4x the loops cost these steps this many times as much (limit {DEFAULT}, owed {OWED}): {over}"
