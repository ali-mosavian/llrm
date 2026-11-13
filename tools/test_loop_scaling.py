"""`uv run --project tools python -m pytest tools/test_loop_scaling.py`

One function of sequential counted loops, 4x the loops: each step's own instructions (`LLRM_DEBUG=time` `[instr]` rows) must
grow about as the program does (4x), not as its square (16x). gcc and clang both take 4.8-4.9x in total; llrm took 8.0x
(1,333 -> 10,714 M at 40 -> 160 loops, -m32 -O2, 2026-11-13). A step over the limit is a pass that asks the whole function
again for each loop."""
import json
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
# What 4x the loops may cost (4 is linear). Steps not named are held to DEFAULT. The named ones are measured over it
# (20 -> 80 loops, 2026-11-13): each limit is the measured growth plus 15%, lowered when its step is fixed. `unroll` and `indvars` are
# fixed (the float liveness of DEVIATE U3 is followed from the reads, w8's #9562); `loopfullunroll` (its per-loop part, named since the group's spans were split, #9528) is under DEFAULT too
# now. rotate, constraint-elimination, indvars, loopidioms and jumpthreading
# grow in the pin's own `opt -passes='default<O2>' -time-passes` too (LoopRotate 9.1x here, ours 9.7x): the ceilings keep them from
# growing further, they are not defects.
DEFAULT = 6.0
OWED = {
    "mir indvars": 6.9,
    "mir unroll": 4.8,
    "mir rotate": 11.3,
    "mir constraint-elimination": 9.6,
    "mir loopidioms": 5.2,
    "mir jumpthreading": 7.7,
}
PIN_FACTOR = 1.5  # a step of ours may grow this much more than the pin's own pass (tools/pin_scaling_ratios.json)
FLOOR_MINSTR = 12.0  # a step under this many M at LARGE is noise


def limit(name: str, pin: dict[str, float]) -> float:
    """What 4x the loops may cost this step: its owed ceiling, else the pin's ratio x PIN_FACTOR where the pin has the pass, else DEFAULT."""
    if name in OWED:
        return OWED[name]
    return pin[name] * PIN_FACTOR if name in pin else DEFAULT


def pin_ratios() -> dict[str, float]:
    return json.loads((Path(__file__).resolve().parent / "pin_scaling_ratios.json").read_text())["ratio"]


def test_a_step_is_held_to_the_pins_growth_of_the_same_pass():
    """A pass the pin grows 8x for 4x the loops may take 12x here (a flat DEFAULT of 6 called it a defect); one at 20x is over."""
    pin = {"mir planted": 8.0}
    assert limit("mir planted", pin) == 12.0
    assert 10.0 <= limit("mir planted", pin) < 20.0
    assert limit("mir unnamed", pin) == DEFAULT
    assert limit("mir rotate", pin) == OWED["mir rotate"], "an owed ceiling stays the bar"


def test_the_pin_ratios_are_those_of_the_pinned_llvm():
    """A pin bump with the old ratios would hold our steps to another LLVM's growth: regenerate with tools/pin_scaling_ratios.py."""
    data = json.loads((Path(__file__).resolve().parent / "pin_scaling_ratios.json").read_text())
    assert data["pin"] == (Path(__file__).resolve().parents[1] / "toolchain/clang/pin").read_text().split()[0]
    assert (data["small"], data["large"]) == (SMALL, LARGE) and data["ratio"]


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
    pin = pin_ratios()
    over = {name: round(ratio, 1) for name, ratio in grown.items() if ratio > limit(name, pin)}
    assert not over, f"4x the loops cost these steps this many times as much (limit {DEFAULT}, owed {OWED}, else the pin x {PIN_FACTOR}): {over}"
