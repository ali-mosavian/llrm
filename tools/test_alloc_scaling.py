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


def test_a_rewrite_counts_what_the_instructions_it_changed_say_of_the_register_classes():
    """`classes scan` read every operand of every instruction after each spill: 371 Minstr on `cells` at N=224 (one block, ~430
    rewrites). The counts of the body before stand for the instructions the rewrite kept."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](224))
        run = steps(compiler, source, "-O2")
    assert run["classes scan"] < 60, f"cells N=224 -O2: classes scan cost {run['classes scan']} Minstr (60 allowed; 371 before): {run}"


def test_a_rewrite_prices_the_siblings_of_the_values_it_changed_only():
    """`facts sibling prices` walked every instruction for plain moves and impure values, and tested every move's intervals for
    overlap, after each spill: 263 Minstr on `cells` at N=224. The values the changed instructions name, and the other sides of
    their copies, are priced again; the rest stand."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](224))
        run = steps(compiler, source, "-O2")
    assert run["facts sibling prices"] < 70, f"cells N=224 -O2: facts sibling prices cost {run['facts sibling prices']} Minstr (70 allowed; 263 before): {run}"


def test_a_rewrite_that_changes_most_instructions_finds_the_homes_afresh():
    """`intervals homes` compared an earlier body's instructions with this one's before taking its answer, and a spill of a value
    read all through a block changes most of them: 895 Minstr on `cells` at N=448, where finding the homes' intervals afresh costs
    316. A comparison longer than the instructions that name a home is not made."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](448))
        run = steps(compiler, source, "-O2")
    assert run["intervals homes"] < 700, f"cells N=448 -O2: intervals homes cost {run['intervals homes']} Minstr (700 allowed; 895 before): {run}"


def test_a_rewrite_patches_the_numbering_and_keeps_the_intervals_it_did_not_change():
    """Each rewrite renumbered the whole body and moved and weighed every live interval: `facts slots` + `facts intervals` were
    459 + 1604 Minstr on `cells` at N=448 (one block, 874 rewrites changing ~70 instructions each, 1500 live values). Slots are
    stable keys (SlotIndexes), a rewrite's added instructions take slots between their neighbours', an untouched interval is the
    same one, and a weight is worked out when read."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](448))
        run = steps(compiler, source, "-O2")
    spent = run["facts slots"] + run["facts intervals"]
    assert spent < 450, f"cells N=448 -O2: facts slots + facts intervals cost {spent} Minstr (450 allowed; 2063 before): {run}"


def test_a_split_asks_the_values_segments_where_it_is_live_and_makes_no_rows_of_the_body():
    """`split placing` solved liveness for every value over the whole body at each round of a function with branches: 868 Minstr on
    `branches` at N=400 and 6,710 at 800 (slope 2.95), 21% of the compile. A split asks of one value, and its segments, which the
    facts already keep and carry across a rewrite, say where it is live."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "branches.c"
        source.write_text(scaling.AXES["branches"](400))
        run = steps(compiler, source, "-O2")
    assert run.get("split placing", 0.0) < 5, f"branches N=400 -O2: split placing cost {run.get('split placing')} Minstr (5 allowed; 868 before): {run}"


def test_the_points_that_destroy_registers_are_not_looked_for_again_in_every_instruction_at_every_rewrite():
    """Every rewrite walked every instruction of the body for the calls and other points that destroy registers: `facts masks` cost
    397 Minstr on `cells` N=448 -O2 and 4.1x per doubling. The points of the instructions a rewrite kept are carried (LLVM's
    `SlotIndexes` keep an instruction's index across an edit), and only those it added are looked at."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](448))
        run = steps(compiler, source, "-O2")
    assert run["facts masks"] < 100, f"cells N=448 -O2: facts masks cost {run['facts masks']} Minstr (100 allowed; 397 before): {run}"


def test_a_rewrite_looks_again_at_the_values_it_disturbed_only_for_a_register_they_share():
    """`after overlapping` found the values placed in one register that now clash by looking at every placed value after every
    rewrite: 1,332 Minstr on `branches` at N=400 and 5,716 at 800 (9.5% of the compile). Values placed before a rewrite did not
    clash, and nothing else moved: the values it named, those whose class moved and those held across a point it added are
    looked at, with the holders they overlap."""


def test_a_split_asks_what_holds_a_register_where_it_splits_and_gathers_none_of_it():
    """`split occupied` gathered the segments of every value placed in any register, sorted, at each split: 220 Minstr on `branches` at
    N=400 and 933 at 800 (slope 2.07). The union of holders answers by the span asked, as LLVM's `LiveIntervalUnion::Query` does."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "branches.c"
        source.write_text(scaling.AXES["branches"](400))
        run = steps(compiler, source, "-O2")
    assert run["after overlapping"] < 500, f"branches N=400 -O2: after overlapping cost {run['after overlapping']} Minstr (500 allowed; 1332 before): {run}"


    assert run.get("split occupied", 0.0) < 20, f"branches N=400 -O2: split occupied cost {run.get('split occupied')} Minstr (20 allowed; ~220 before): {run}"


def test_a_rewrite_moves_the_holders_of_a_register_it_named_in_the_index_and_does_not_build_it_again():
    """`union.refresh()` dropped the index of every register after every rewrite, and the next questions built it again from every
    holder: `regalloc evict` 692 Minstr on `branches` at N=400 and 2,601 at 800, `recolor` and `base` their shares of 4 G. Only the
    intervals of the values the rewrite named moved, and those are moved in the index."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "branches.c"
        source.write_text(scaling.AXES["branches"](400))
        run = steps(compiler, source, "-O2")
    assert run["regalloc evict"] < 200, f"branches N=400 -O2: regalloc evict cost {run['regalloc evict']} Minstr (200 allowed; 692 before): {run}"


def test_the_recolor_step_costs_what_it_did_before_the_intervals_were_shared():
    """`LiveRanges::get` was not inlined into the interference scans of `regalloc recolor`: the same 578k `busy` calls and 2.2M holder
    steps cost 702 Minstr on `cells` N=448 -O2, against 644 before the intervals were shared behind an `Arc` (the gate's recolor
    rows rose 8-19% at 2N). Forced inlining brings it to ~657."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](448))
        run = steps(compiler, source, "-O2")
    assert run["regalloc recolor"] < 680, f"cells N=448 -O2: regalloc recolor cost {run['regalloc recolor']} Minstr (680 allowed; 702 before the fix): {run}"


def test_a_memory_phi_asks_its_own_slot_sharing_and_not_a_pass_over_every_block():
    """`memory_phis` asked `shared_slots` of each phi it priced, and each ask built the body's block and phi tables and searched every
    block's definitions for the values it compared: `ssa simulated` cost 693 Minstr on `branches` at N=400 and 2,597 at 800 (slope
    1.9 to 2.0 per doubling), 6% of the compile. The tables are found once for the body and a phi looks up its own place."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "branches.c"
        source.write_text(scaling.AXES["branches"](400))
        run = steps(compiler, source, "-O2")
    assert run["ssa simulated"] < 150, f"branches N=400 -O2: ssa simulated cost {run['ssa simulated']} Minstr (150 allowed; 693 before): {run}"


def test_a_spill_asks_whether_a_candidate_sibling_meets_its_group_and_not_the_web_of_copies_whole():
    """`siblings` worked out the interference graph of the whole copy web (150 values at `branches` N=400, whose phi families are
    large) for each spill, to ask it of a group of one or two: `siblings interference` cost 877 Minstr at N=400 and 3,343 at 800 (slope
    1.95 per doubling), 8% of the compile. A candidate is asked of the intervals the allocator holds, and of the graph of the
    group and itself where they meet."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "branches.c"
        source.write_text(scaling.AXES["branches"](400))
        run = steps(compiler, source, "-O2")
    spent = run.get("siblings interference", 0.0) + run["spill siblings"]
    assert spent < 60, f"branches N=400 -O2: spill siblings cost {spent} Minstr (60 allowed; 985 before): {run}"
