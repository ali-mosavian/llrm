"""`uv run --project tools python -m pytest tools/test_alloc_scaling.py`

The register allocator's passes on one function of straight-line statements on four live values, which doubles twice: the
time a pass takes must grow about as the program does (4x for 4x), not as its square (16x)."""
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import llrmbin  # noqa: E402

SMALL, LARGE = 1024, 4096
# 4x the program may cost this much more (a linear pass costs 4x, a quadratic one 16x). regalloc base is allowed more: it
# rebuilds the facts of the body (every interval) at each split and spill, which grows with both.
LIMITS = {"lir coalesce": 11.0, "lir twoaddr": 11.0, "regalloc base": 16.0}
PASSES = tuple(LIMITS)


def lcg(k: int) -> int:
    return (k * 2654435761 + 12345) % 2**32


def straight(n: int) -> str:
    """`scaling.py`'s `straight` without the products: isel costs a chain of adds for each, which is not what is measured."""
    v = "abcd"
    body = "".join(f"    {v[k % 4]} = {v[k % 4]} + ({v[(k + 1) % 4]} ^ ({v[(k + 2) % 4]} >> {k % 7 + 1})) + {lcg(k)}u;\n" for k in range(n))
    return "unsigned fn(unsigned a, unsigned b, unsigned c, unsigned d) {\n" + body + "    return a ^ b ^ c ^ d;\n}\n"


def steps(compiler: Path, source: Path) -> dict[str, float]:
    """Each step's own time in ms (`LLRM_DEBUG=time`)."""
    done = subprocess.run([str(compiler), "-m32", "-march=i486", "-O1", "-o", os.devnull, str(source)], capture_output=True, text=True, env={**os.environ, "LLRM_DEBUG": "time", "LLRM_TIME_TOP": "100000"})
    assert done.returncode == 0, done.stderr[-500:]
    return {m.group(2): float(m.group(1)) for m in re.finditer(r"^\[time\]\s+([\d.]+) ms own\s+[\d.]+ ms total\s+\d+x (.+)$", done.stderr, re.M)}


def test_the_allocators_passes_grow_as_the_program_does():
    """lir coalesce took 23x as long for 4x the statements, regalloc base 54x and lir twoaddr 21x (#943): neighbour sets joined
    whole at each copy, every register's holders looked at for each value, a scan of the block for each tied instruction."""
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        best = {}
        for n in (SMALL, LARGE):
            source = Path(work) / f"s{n}.c"
            source.write_text(straight(n))
            # The least of two: another process on the machine only ever adds.
            runs = [steps(compiler, source) for _ in range(2)]
            best[n] = {name: min(run[name] for run in runs) for name in PASSES}
    grown = {name: best[LARGE][name] / max(best[SMALL][name], 1.0) for name in PASSES}
    assert all(grown[name] < LIMITS[name] for name in PASSES), f"4x the statements cost {grown} times as much (limits {LIMITS}): {best}"
