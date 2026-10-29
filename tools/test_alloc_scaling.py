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

# Every test here compiles with llrm-c and reads the old allocator's `[instr]` rows. Debt `skip-alloc-scaling` in docs/working/ci.md.
pytestmark = pytest.mark.skipif(not (llrmbin.bin_dir() / "llrm-c").exists(), reason="llrm-c is not built (debt skip-alloc-scaling)")

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


def steps(compiler: Path, source: Path, level: str = "-O1", *flags: str, target: tuple[str, ...] = ("-m32", "-march=i486")) -> dict[str, float]:
    """Each step's own instructions in millions (`LLRM_DEBUG=time`'s `[instr]` rows); skips where the host has no counter."""
    with llrmbin.with_object([str(compiler), *target, level, *flags, "-o", llrmbin.OBJECT, str(source)]) as command:
        done = subprocess.run(command, capture_output=True, text=True, env={**os.environ, "LLRM_DEBUG": "time", "LLRM_TIME_TOP": "100000"})
    assert done.returncode == 0, done.stderr[-500:]
    rows = re.findall(r"^\[instr\]\s+([\d.]+) (Minstr|Mcpu-ns) own\s+[\d.]+ \S+ total\s+\d+x (.+)$", done.stderr, re.M)
    if not rows or any(unit != "Minstr" for _, unit, _ in rows):
        pytest.skip("no instruction counter on this host: the rows are CPU time")
    return {name: float(own) for own, _, name in rows}


def calls(compiler: Path, source: Path, level: str, step: str) -> int:
    """How many times `LLRM_DEBUG=time` timed `step`."""
    with llrmbin.with_object([str(compiler), "-m32", "-march=i486", level, "-o", llrmbin.OBJECT, str(source)]) as command:
        done = subprocess.run(command, capture_output=True, text=True, env={**os.environ, "LLRM_DEBUG": "time", "LLRM_TIME_TOP": "100000"})
    return int(re.search(rf"^\[instr\].*total\s+(\d+)x {re.escape(step)}$", done.stderr, re.M).group(1))


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


def test_a_split_finds_its_instruction_by_index_not_by_walking_the_block():
    """`rewrite_instr`, `instr_at` and `next_instr` looked for an instruction by walking its block: with trySplit on, `regalloc base`
    cost 143.8 / 400.3 Minstr on the ladder's `mem` at N=800 / 1600 (3.08x per doubling, 2.66x before). A block is in slot order,
    so the position is bisected."""
    sys.path.insert(0, str(Path(__file__).resolve().parent / "ladder"))
    import ladder_gen as gen
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "mem800.c"
        source.write_text(gen.generate("mem", 800))
        run = steps(compiler, source, "-O2", "-mabi=sysv", "-fno-inline-functions")
    assert run["regalloc base"] < 125, f"mem N=800 -O2: regalloc base cost {run['regalloc base']} Minstr (125 allowed; 143.8 before): {run}"


def test_a_spill_lists_the_units_of_a_register_in_time_linear_in_its_operands():
    """`InlineSpiller::units` deduplicated the instructions of a register with `contains` on a list and searched the units made so
    far for each: `regalloc base` on the ladder's `mem` cost 259.7 / 586.4 Minstr at N=1600 / 3200 (2.26x per doubling; 2.09x now)."""
    sys.path.insert(0, str(Path(__file__).resolve().parent / "ladder"))
    import ladder_gen as gen
    compiler = llrmbin.bin_dir() / "llrm-c"
    cost = {}
    with tempfile.TemporaryDirectory() as work:
        for n in (1600, 3200):
            source = Path(work) / f"mem{n}.c"
            source.write_text(gen.generate("mem", n))
            cost[n] = steps(compiler, source, "-O2", "-mabi=sysv", "-fno-inline-functions")["regalloc base"]
    grown = cost[3200] / cost[1600]
    assert grown < 2.15, f"mem -O2: twice the size cost regalloc base {grown:.2f}x as much (2.15 allowed; 2.26 before): {cost}"


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
        # The spiller's route, which -Omax alone makes by default, is the subject.
        run = steps(compiler, source, "-O2")
    assert run["ssa simulated"] < 150, f"branches N=400 -O2: ssa simulated cost {run['ssa simulated']} Minstr (150 allowed; 693 before): {run}"


def test_taking_a_placed_value_out_does_not_shift_every_value_placed_after_it():
    """The allocator's table of placed values was an insertion-ordered map, and last-chance recoloring takes holders out of its middle
    and puts them back at every try: `IndexMap::shift_remove` moved every entry after the one taken (14% of the compile of `branches`
    at N=800): `regalloc recolor` cost 1,202 Minstr at N=400 and 3,004 at 800. The order is a number each value was given when
    placed, and a list only at the end of an allocation."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "branches.c"
        source.write_text(scaling.AXES["branches"](400))
        run = steps(compiler, source, "-O2")
    assert run["regalloc recolor"] < 1100, f"branches N=400 -O2: regalloc recolor cost {run['regalloc recolor']} Minstr (1100 allowed; 1202 before): {run}"


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


def test_a_candidate_sibling_that_would_join_is_asked_of_one_graph_a_round_not_one_each():
    """The interference of each candidate sibling with its group was asked of a graph of its own (1,568 asks, 930 Minstr, on `nest` at
    N=32, where the intervals meet; 51 Minstr and 131 asks for the graph of the whole web before): a graph a round of the group and
    its candidates, made where one pair meets and for the candidates worth joining only, is 60."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "nest.c"
        source.write_text(scaling.AXES["nest"](32))
        run = steps(compiler, source, "-O2")
    spent = run.get("siblings interference", 0.0) + run["spill siblings"]
    assert spent < 150, f"nest N=32 -O2: spill siblings cost {spent} Minstr (150 allowed; 987 asking one graph a candidate): {run}"


def test_a_spill_rewrite_keeps_the_instructions_that_name_no_spilled_value_at_once():
    """The rewrite loop of a spill walked every instruction of a block that held one spilled value, with a binary search of the
    positions that name one, a clone of the instruction and a push for each: `spill rewrite` cost 167 Minstr on `cells` N=448
    (one long block) and 648 at 896. The runs between the positions that name a value are kept at once."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](448))
        run = steps(compiler, source, "-O2")
    assert run["spill rewrite"] < 100, f"cells N=448 -O2: spill rewrite cost {run['spill rewrite']} Minstr (100 allowed; 167 before): {run}"


def test_a_spill_asks_no_slot_colouring_of_the_homes_it_leaves_behind():
    """Each spill coloured its slot against the intervals of every existing home, found again each time: `intervals homes` cost 165 Minstr
    on `branches` at N=400 and 847 at 800 (slope 2.3 per doubling), `spill color slots` 37 and 142, for allocations that were then
    thrown away. A spill takes a slot of its own and the kept allocation's slots are shared once (`spill slots shared`)."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "branches.c"
        source.write_text(scaling.AXES["branches"](400))
        run = steps(compiler, source, "-O2")
    spent = run.get("intervals homes", 0.0) + run.get("spill color slots", 0.0)
    assert spent < 20, f"branches N=400 -O2: intervals homes and spill color slots cost {spent} Minstr (20 allowed; 202 before): {run}"


def test_a_reload_inserted_in_a_long_block_renumbers_no_occurrence_after_it():
    """The postings named each occurrence by its position in its block, so one reload put in a block of thousands renumbered the
    entries of every value named after it: `spill cleanup` cost 452 Minstr on `cells` at N=448 and 1,882 at 896 (x4.2 per doubling), 
    `regalloc split` 207 and 777. An occurrence is named by a slot its block gave the instruction, which an insertion leaves."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](448))
        run = steps(compiler, source, "-O2")
    spent = run["spill cleanup"] + run["regalloc split"]
    assert spent < 360, f"cells N=448 -O2: spill cleanup and regalloc split cost {spent} Minstr (360 allowed; 660 before): {run}"


def test_the_facts_of_a_rewrite_are_edited_where_they_are_not_copied():
    """After each spill or split the allocator's facts of the body (the numbering of the instructions, the copy hints, the intervals) were
    copied whole and then edited for the few values the rewrite named: `facts slots` cost 83 Minstr and `facts hints` 139 on `cells` at
    N=448 (316 and 519 at 896), one copy of every instruction's slot and every value's hints for each of ~1,800 rounds. They are carried
    to the next round and edited where they are."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](448))
        run = steps(compiler, source, "-O2")
    spent = run["facts slots"] + run["facts hints"]
    assert spent < 110, f"cells N=448 -O2: facts slots and facts hints cost {spent} Minstr (110 allowed; 222 before): {run}"


def test_a_value_with_more_values_live_than_registers_is_not_recolored():
    """Last-chance recoloring (LLVM's `tryLastChanceRecoloring`) searched for a register for every value no split rescued, and 99% of its
    sessions fail: `regalloc recolor` cost 599 Minstr on `cells` at N=448 and 991 on `branches` at 400. Where more values are live at a point
    of the value, with it, than there are registers for them, no moving about places it: the session is not made."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](448))
        run = steps(compiler, source, "-O2")
    assert run["regalloc recolor"] < 200, f"cells N=448 -O2: regalloc recolor cost {run['regalloc recolor']} Minstr (200 allowed; 599 before): {run}"


def test_a_spill_shares_the_lists_of_the_blocks_it_does_not_change():
    """Copying a `LirBlock` copied its successor and phi lists, and the spiller copied every block of the body at every round:
    `regalloc spill` own cost 590 Minstr on `branches` at N=800 and 2,256 at 1600. A copy of a block shares the lists until one changes."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "branches.c"
        source.write_text(scaling.AXES["branches"](800))
        run = steps(compiler, source, "-O2")
    assert run["regalloc spill"] < 450, f"branches N=800 -O2: regalloc spill cost {run['regalloc spill']} Minstr (450 allowed; 590 before): {run}"


def test_a_spill_copies_no_block_it_does_not_change():
    """The body held its blocks by value: every round of the spiller cloned and dropped every block (each a few allocations),
    `regalloc spill` costing 326 Minstr on `branches` at N=800 once the lists were shared. A body shares the blocks it keeps."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "branches.c"
        source.write_text(scaling.AXES["branches"](800))
        run = steps(compiler, source, "-O2")
    assert run["regalloc spill"] < 300, f"branches N=800 -O2: regalloc spill cost {run['regalloc spill']} Minstr (300 allowed; 326 before): {run}"


def test_a_carve_works_on_the_positions_that_name_the_value_not_on_the_block():
    """Carving a region walked its blocks end to end several times (liveness back from the block end for each border, every
    instruction asked whether it names the value, every position of the block looked up in the copies map) and renamed, by a copy,
    every instruction the region covers whether it names the value or not: `split carving` cost 333 Minstr on `cells` at N=448 and
    1,259 at 896. They are asked of the value's own positions."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](448))
        run = steps(compiler, source, "-O2")
    assert run["split carving"] < 150, f"cells N=448 -O2: split carving cost {run['split carving']} Minstr (150 allowed; 333 before): {run}"


def test_a_short_update_looks_at_the_definitions_of_the_spilled_values_and_not_at_every_instruction_of_the_block():
    """Making an update of a spilled value short walked each block that holds a definition of one, end to end, and copied it:
    `spill short updates` cost 335 Minstr on `cells` at N=896 (x4.1 for x2 the program). It looks at the positions the postings give."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](448))
        run = steps(compiler, source, "-O2")
    assert run["spill short updates"] < 20, f"cells N=448 -O2: spill short updates cost {run['spill short updates']} Minstr (20 allowed; 84 before): {run}"


def test_an_update_made_local_looks_at_the_definitions_of_the_spilled_values_and_copies_a_block_only_where_it_makes_one():
    """`_local_updates` copied each block holding a definition of a spilled value and looked at every instruction of it: `spill local updates`
    cost 70 Minstr on `cells` at N=448 (x4.2 for x2 the program). It looks at the positions the postings give."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](448))
        run = steps(compiler, source, "-O2")
    assert run["spill local updates"] < 25, f"cells N=448 -O2: spill local updates cost {run['spill local updates']} Minstr (25 allowed; 70 before): {run}"


def test_the_liveness_of_the_pieces_a_split_made_is_asked_of_their_postings_and_not_of_a_pass_over_the_body():
    """`split after liveness` scanned the whole body for the occurrences of the pieces it had made: 98.6 Minstr on `cells` at N=448, 401 at 896.
    It reads them from the postings the body is followed with."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](448))
        run = steps(compiler, source, "-O2")
    assert run["split after liveness"] < 60, f"cells N=448 -O2: split after liveness cost {run['split after liveness']} Minstr (60 allowed; 98.6 before): {run}"

def test_a_load_is_asked_of_the_stores_to_its_own_symbol_and_not_of_every_store_in_the_body():
    """Every load of a global was asked, one by one, whether each of the body's stores to other globals keeps it: 300 loads against
    300 stores took `spill stable loads` 1,226 Minstr at -O2 -m32 (5,102 at 600), most of it `may_overlap` building the same answer
    for every pair. The stores to another external symbol meet it by region and are found by block, not named for each load."""
    compiler = llrmbin.bin_dir() / "llrm-c"
    n = 300
    lines = ["extern void sink(int *);", *(f"int a{k}, k{k};" for k in range(n)), "void taken(void){"]
    lines += [f" sink(&a{k}); sink(&k{k});" for k in range(n)]
    lines += ["}", "int fn(int m){ int s=0,i; for(i=0;i<m;i++){"]
    lines += [f" a{k} += i; s += k{k};" for k in range(n)]
    lines += [" } return s; }"]
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "stores.c"
        source.write_text("\n".join(lines) + "\n")
        run = steps(compiler, source, "-O2")
    assert run["spill stable loads"] < 500, f"300 stores and 300 loads: spill stable loads cost {run['spill stable loads']} Minstr (500 allowed; 1,226 before)"


def test_a_local_split_asks_the_postings_for_the_references_of_the_value_and_not_every_instruction_of_its_block():
    """`splitkit::local` collected a value's references by asking each instruction of its block whether it names it: `split local` cost 82
    Minstr on `cells` at N=448 and 291 at 896. They are read from the postings."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "cells.c"
        source.write_text(scaling.AXES["cells"](448))
        run = steps(compiler, source, "-O2")
    assert run["split local"] < 45, f"cells N=448 -O2: split local cost {run['split local']} Minstr (45 allowed; 82 before): {run}"


def test_the_peephole_grows_as_a_loop_nest_does():
    """`lir peephole` took 6.3x as long for a loop nest 4x as deep (4.3x the instructions) at -O2: `copysink` walked every block of a loop
    to find the path from each copy to the exit and solved the loop's liveness for each loop around it, whether or not a copy could move.
    A copy its own block settles is left there, and the paths and the liveness are made once for a block and a loop."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        spent = {}
        for n in (16, 64):
            source = Path(work) / f"nest{n}.c"
            source.write_text(scaling.AXES["nest"](n))
            spent[n] = steps(compiler, source, "-O2")["lir peephole"]
    grown = spent[64] / max(spent[16], 1e-9)
    assert grown < 5.2, f"a nest 4x as deep cost {grown:.2f} times as much in `lir peephole` (5.2 allowed; 6.29 before): {spent}"


def test_a_split_by_region_costs_a_fraction_of_a_million_instructions_not_a_million():
    """`split placed` built a Region and walked the value's blocks several times for each candidate register: 1.4 Minstr per split
    on d_faces -O2 (spec budget 0.2) and 0.38 on `branches` at N=200 (50 splits). A candidate meets each live block once and only
    the best builds a region: 0.21 on `branches`, 0.53 on d_faces."""
    vsgcc = next((Path(__file__).resolve().parent.parent / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import scaling
    compiler = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        source = Path(work) / "branches.c"
        source.write_text(scaling.AXES["branches"](200))
        run = steps(compiler, source, "-O2")
        splits = calls(compiler, source, "-O2", "split placed")
    assert splits >= 20, f"premise: branches N=200 -O2 made {splits} splits"
    each = run["split placed"] / splits
    assert each < 0.3, f"branches N=200 -O2: split placed cost {each:.3f} Minstr a split (0.3 allowed; 0.38 before)"


def far_stores(n: int) -> str:
    """A loop through a far pointer, then `n` guarded stores through it: every store splits the pointer's live range."""
    body = "".join(f"    if (c[{k}]) fr[{k}].y = s + {k}.0f;\n" for k in range(n))
    return (
        "typedef struct { float x, y; } P;\nextern float g(float);\nfloat f(P far *fr, int *c, int n)\n{\n    int i; float s = 0;\n"
        "    for (i = 0; i < n; i++) s += g(fr[i].x);\n" + body + "    return s;\n}\n"
    )


def test_a_split_rewrites_an_instruction_without_scanning_the_operands_of_its_register():
    """`LiveIntervals::rewrite_instr` removed an instruction's operands with `retain` over the whole list of the register: each
    split child of a far pointer rewrote ~n instructions, each a scan of ~n operands. `regalloc base` at -Os (code16) cost
    41.96 / 233.45 Minstr at 400 / 1600 guarded stores (5.56x for 4x; clang's trySplit grows 4.18x). The operands sit in slots."""
    compiler = llrmbin.bin_dir() / "llrm-c"
    cost = {}
    with tempfile.TemporaryDirectory() as work:
        for n in (400, 1600):
            source = Path(work) / f"far{n}.c"
            source.write_text(far_stores(n))
            cost[n] = steps(compiler, source, "-Os", "-S", target=())["regalloc base"]
    grown = cost[1600] / cost[400]
    assert grown < 5.0, f"far stores -Os: 4x the stores cost regalloc base {grown:.2f}x as much (5.0 allowed; 5.56 before): {cost}"
