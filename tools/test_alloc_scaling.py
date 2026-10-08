"""`uv run --project tools python -m pytest tools/test_alloc_scaling.py`

The register allocator's passes on one function of straight-line statements on four live values, which doubles twice: the
instructions a pass retires must grow about as the program does (4x for 4x), not as its square (16x). Instructions, not
milliseconds: the same binary read 6x to 21x on a loaded host, and failed the gate on it (`LLRM_DEBUG=time`'s `[instr]` rows)."""
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import llrmbin  # noqa: E402

SMALL, LARGE = 1024, 4096
# 4x the program may cost this much more (a linear pass costs 4x, a quadratic one 16x). Measured 2026-10-09 on main 61be539b2:
# 4.09, 4.05 and 12.42, repeating to 0.01%. regalloc base is allowed more: it rebuilds the facts of the body (every interval)
# at each split and spill, which grows with both (step 3 of the compile-time plan).
LIMITS = {"lir coalesce": 4.5, "lir twoaddr": 4.5, "regalloc base": 13.0}
PASSES = tuple(LIMITS)


def lcg(k: int) -> int:
    return (k * 2654435761 + 12345) % 2**32


def straight(n: int) -> str:
    """`scaling.py`'s `straight` without the products: isel costs a chain of adds for each, which is not what is measured."""
    v = "abcd"
    body = "".join(f"    {v[k % 4]} = {v[k % 4]} + ({v[(k + 1) % 4]} ^ ({v[(k + 2) % 4]} >> {k % 7 + 1})) + {lcg(k)}u;\n" for k in range(n))
    return "unsigned fn(unsigned a, unsigned b, unsigned c, unsigned d) {\n" + body + "    return a ^ b ^ c ^ d;\n}\n"


def steps(compiler: Path, source: Path) -> dict[str, float]:
    """Each step's own instructions in millions (`LLRM_DEBUG=time`'s `[instr]` rows); skips where the host has no counter."""
    done = subprocess.run([str(compiler), "-m32", "-march=i486", "-O1", "-o", os.devnull, str(source)], capture_output=True, text=True, env={**os.environ, "LLRM_DEBUG": "time", "LLRM_TIME_TOP": "100000"})
    assert done.returncode == 0, done.stderr[-500:]
    rows = re.findall(r"^\[instr\]\s+([\d.]+) (Minstr|Mcpu-ns) own\s+[\d.]+ \S+ total\s+\d+x (.+)$", done.stderr, re.M)
    if not rows or any(unit != "Minstr" for _, unit, _ in rows):
        pytest.skip("no instruction counter on this host: the rows are CPU time")
    return {name: float(own) for own, _, name in rows}


def test_the_allocators_passes_grow_as_the_program_does():
    """lir coalesce took 23x as long for 4x the statements, regalloc base 54x and lir twoaddr 21x (#943): neighbour sets joined
    whole at each copy, every register's holders looked at for each value, a scan of the block for each tied instruction."""
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        best = {}
        for n in (SMALL, LARGE):
            source = Path(work) / f"s{n}.c"
            source.write_text(straight(n))
            run = steps(compiler, source)
            best[n] = {name: run[name] for name in PASSES}
    grown = {name: best[LARGE][name] / max(best[SMALL][name], 1e-9) for name in PASSES}
    assert all(grown[name] < LIMITS[name] for name in PASSES), f"4x the statements cost {grown} times as much (limits {LIMITS}): {best}"
