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


def steps(compiler: Path, source: Path, level: str = "-O1") -> dict[str, float]:
    """Each step's own instructions in millions (`LLRM_DEBUG=time`'s `[instr]` rows); skips where the host has no counter."""
    done = subprocess.run([str(compiler), "-m32", "-march=i486", level, "-o", os.devnull, str(source)], capture_output=True, text=True, env={**os.environ, "LLRM_DEBUG": "time", "LLRM_TIME_TOP": "100000"})
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


def test_a_rewrite_finds_what_it_changed_by_pointer_not_by_looking_up_every_instruction():
    """`facts intervals` and `intervals homes` hashed every instruction of an edited block, old and new, at each of a function's
    ~430 rewrites to find the few that changed: 380 + 71 Minstr became 625 + 74 on `cells` at N=224 (one block, ~900 spills),
    a pointer comparison against the parent's list finds the same instructions."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](224))
        run = steps(compiler, source, "-O2")
    spent = run["facts intervals"] + run["intervals homes"]
    assert spent < 520, f"cells N=224 -O2: facts intervals + intervals homes cost {spent} Minstr (520 allowed; 700 before the pointer diff): {run}"


def test_a_spill_patches_the_postings_of_the_block_it_changed_instead_of_making_them_again():
    """`spill cleanup` rebuilt the postings of a long block entry by entry after every spill: 699 Minstr on `cells` at N=224 (one
    block, ~600 spills) and 4.3x that for twice the size. The kept instructions' entries move to their new positions."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](224))
        run = steps(compiler, source, "-O2")
    assert run["spill cleanup"] < 250, f"cells N=224 -O2: spill cleanup cost {run['spill cleanup']} Minstr (250 allowed; 699 before): {run}"


def test_a_spill_passes_over_the_instructions_that_name_no_spilled_value():
    """`spill rewrite` ran its dozen rewrites over every instruction of a block that held one spilled value: 570 Minstr on `cells`
    at N=224 (one block, ~600 spills), 1.3k instructions of work for each of ~3000 instructions per spill. An instruction that
    names none of the values is left as it is, found from the postings."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](224))
        run = steps(compiler, source, "-O2")
    assert run["spill rewrite"] < 150, f"cells N=224 -O2: spill rewrite cost {run['spill rewrite']} Minstr (150 allowed; 570 before): {run}"


def test_a_rewrite_works_out_the_widths_of_the_values_it_changed_only():
    """`facts widths` walked every operand of every instruction after each spill: 385 Minstr on `cells` at N=224 (one block, ~430
    rewrites). The widths of the body before stand for the values no changed instruction names."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](224))
        run = steps(compiler, source, "-O2")
    assert run["facts widths"] < 100, f"cells N=224 -O2: facts widths cost {run['facts widths']} Minstr (100 allowed; 385 before): {run}"
