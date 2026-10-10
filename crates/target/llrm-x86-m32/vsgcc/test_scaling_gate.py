"""scaling_gate.py: a pass gone quadratic reads as 2N/N = 4, a linear one as 2, and neither direction of change passes unseen."""
import os
import subprocess
import sys
from pathlib import Path

import pytest

import scaling
import programs
import scaling_gate as gate
import wrap

REQUIRES = ["perf"]  # the gate leaves this file out where `perf stat` reads no count
STAND_IN = "import sys; n = len(open(sys.argv[1]).read().splitlines()); sum(range({work}))"


def stand_in(work):
    """A 'compiler' spending `work` (a formula in n, the source's line count) in C-speed loop iterations."""
    return lambda compiler, level, source: [sys.executable, "-I", "-c", STAND_IN.format(work=work), str(source)]


def test_a_quadratic_compiler_reads_as_four_and_a_linear_one_as_two(tmp_path):
    """The ratio is of cost net of the empty file, so a fixed start-up cost does not pull a quadratic one towards 2."""
    quadratic = gate.ratio("straight", "O2", tmp_path, stand_in("300 * n * n + 5000000"), "q")
    linear = gate.ratio("straight", "O2", tmp_path, stand_in("60000 * n + 5000000"), "l")
    assert quadratic == pytest.approx(4.0, abs=0.25)
    assert linear == pytest.approx(2.0, abs=0.15)


def test_a_counter_that_reads_zero_is_no_counter_not_free_work(monkeypatch):
    """perf prints `<not supported>` or 0 on a VM with no counters; a zero cost made every ratio 0/0."""
    monkeypatch.setattr(scaling, "sample", lambda *a, **k: (0, 0, ""))
    with pytest.raises(gate.NoCounter):
        gate.count(["true"])
    monkeypatch.setattr(scaling, "sample", lambda *a, **k: (_ for _ in ()).throw(ValueError("<not supported>")))
    with pytest.raises(gate.NoCounter):
        gate.count(["true"])


STEPS = (
    "import sys; n = len(open(sys.argv[1]).read().splitlines())\n"
    "rows = {{'linear step': {linear}, 'quadratic step': {quadratic}, 'tiny step': 0.001 * n}}\n"
    "for name, v in rows.items(): print(f'[instr] {{v:.3f}} {unit} own {{v:.3f}} {unit} total 1x {{name}}', file=sys.stderr)\n"
)


def steps_compiler(linear="0.01 * n", quadratic="0.0002 * n * n", unit="Minstr"):
    """A 'compiler' printing the [instr] rows llrm-c does: one linear step, one quadratic, one too small to count."""
    return lambda compiler, level, source: [sys.executable, "-I", "-c", STEPS.format(linear=linear, quadratic=quadratic, unit=unit), str(source)]


def test_a_step_gone_quadratic_reads_four_and_a_linear_one_two(tmp_path):
    got = gate.pass_ratios("straight", "O2", tmp_path, steps_compiler(), "s")
    assert got["straight O2 quadratic step"][0] == pytest.approx(4.0, abs=0.3)
    assert got["straight O2 linear step"][0] == pytest.approx(2.0, abs=0.1)
    assert "straight O2 tiny step" not in got  # under LOW of the work: its count moves with run order


def test_cpu_time_instead_of_instruction_counts_is_no_counter(tmp_path):
    """compile-time prints Mcpu-ns where the host has no counter: a time, not a count of work done."""
    source = tmp_path / "a.c"
    source.write_text("x\n")
    with pytest.raises(gate.NoCounter):
        gate.own_work(steps_compiler(unit="Mcpu-ns")("llrm", "O2", source))
    assert gate.own_work(steps_compiler()("llrm", "O2", source))["linear step"] == pytest.approx(0.01)


def test_the_costs_read_back_give_a_second_difference_of_fixed_and_linear_work_nil_and_of_quadratic_not(tmp_path):
    """The ratio 2N/N rose when linear work got cheaper, and c(2N) - 2c(N) when a fixed cost went: the gate compares
    c(2N) - 3c(N) + 2c(N/2), which cancels both."""
    second = lambda c: c[2] - 3 * c[1] + 2 * c[0]
    def costs(fixed, linear, quadratic=0):
        def counted(command):
            n = len(Path(command[-1]).read_text().splitlines())
            return fixed + linear * n + quadratic * n * n

        return gate.costs("straight", "O2", tmp_path, stand_in("0"), "test", counter=counted)

    fixed_and_linear = costs(5_000_000, 60_000)
    cheaper = costs(1_000_000, 30_000)
    quadratic = costs(5_000_000, 60_000, 300)
    assert second(fixed_and_linear) == 0 and second(cheaper) == 0
    assert second(quadratic) > 0.1 * quadratic[2]


def test_the_16_bit_axes_compile_with_m16_and_the_others_with_m32():
    """Every axis ran -m32 only, and `chain` at N=7 with -m16 never finished unseen. The interprocedural axes run at both."""
    import levels_time
    sixteen = gate.commanded("chain-m16", levels_time.command)("llrm", "O2", Path("x.c"))
    assert "-m16" in sixteen and "-m32" not in sixteen
    assert "-m32" in gate.commanded("chain", levels_time.command)("llrm", "O2", Path("x.c"))
    assert {"chain-m16", "callers-m16"} <= set(gate.SIZES)
    assert scaling.AXES["chain"](3) == gate.generated("chain-m16", 3)


def test_a_count_does_not_inherit_the_callers_llrm_variables(monkeypatch):
    """A caller's LLRM_CHECK_* or LLRM_VERIFY reached the compiler under measurement and added work to the step it checks: regparm16's
    branch read 4.5 Minstr over in 'lir peephole' at every size, which failed measure on a flat constant."""
    monkeypatch.setenv("LLRM_CHECK_FOO", "1")
    monkeypatch.setenv("LLRM_BIN", "/kept")
    seen = scaling.sample([sys.executable, "-I", "-c", "import os, sys; print(sorted(k for k in os.environ if k.startswith('LLRM_')), file=sys.stderr)"], {"LLRM_DEBUG": "time"})[2]
    assert "LLRM_CHECK_FOO" not in seen and "LLRM_BIN" in seen and "LLRM_DEBUG" in seen, seen


def test_the_nest_axis_is_a_loop_nest_as_deep_as_it_says_and_the_gate_sizes_it():
    """gap32's recursive inlining nested loops deeply and rectwo -O2 went 65 M -> 792 M: hoist, lsr, peephole, jumps and the allocator are
    superlinear in nesting depth, which no axis measured (2N/N of a nest 16 deep is 4.3, D/c(2N) 0.44)."""
    text = scaling.AXES["nest"](5)
    assert text.count("for (") == 5
    depths = [len(line) - len(line.lstrip()) for line in text.splitlines() if line.lstrip().startswith("for (")]
    assert depths == sorted(depths) and len(set(depths)) == 5, depths
    assert gate.SIZES["nest"] == 16


def test_the_cells_axis_reads_n_distinct_cells_after_the_stores_to_every_cell_before_and_the_gate_sizes_it():
    """The 66 programs and the other axes carry little memory a pass can scan per load: `straight` has none. Memory forwarding's walks
    and per-cell scans are quadratic in the cells a function touches, which no axis measured (2N/N of 224 cells is 3.8 at -O2)."""
    text = scaling.AXES["cells"](10)
    assert "static unsigned cell[10];" in text
    assert text.count("cell[") == 10 * 3 + 1
    stored = {line.split("]")[0].split("[")[1] for line in text.splitlines() if line.strip().startswith("cell[") and "=" in line}
    assert stored == {str(k) for k in range(10)}, stored
    assert gate.SIZES["cells"] == 112


def test_the_steps_that_scanned_every_function_per_function_stay_linear_in_the_functions(tmp_path):
    """4,096 small functions at -O2 cost 385 G instructions: call-effects and summaries callbacks added up every entry's summary
    for each body and for each entry that changed (126 G and 89 G, 2N/N = 3.9 each), and trivialunswitch copied the callees map for
    each function (9 G, 3.9). The front end read every fact stated so far to say one more, scanned every wccq node for each parameter's
    facts (frontend translate 26 G, 1.9) and rebuilt the function's value and place maps and searched all its calls' ABIs for each call
    instruction (hir to mir 12 G, 1.9). A step 2N/N above 2.2 (slope 1.1) on the `functions` axis fails; a few Minstr of start-up are allowed."""
    n = 128
    own = {}
    for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
        source = tmp_path / f"functions_{label}.c"
        source.write_text("" if size == 0 else scaling.functions(size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    grown = {}
    for step in ("analysis call-effects", "summaries callbacks", "mir trivialunswitch", "frontend translate", "hir to mir"):
        small, big = (own[label].get(step, 0.0) - own["empty"].get(step, 0.0) for label in ("n", "2n"))
        if big > 2.2 * small + 5.0:
            grown[step] = f"{small:.1f} -> {big:.1f} Minstr"
    assert not grown, grown


def test_the_steps_that_ran_for_every_inline_trial_stay_linear_in_the_callers(tmp_path):
    """The inliner tried each call site of the caller of N callees by splicing it and putting the whole body through the pipeline
    (N=64: 63 runs of the one big body), so call-effects, through-memory, points-to, annotated, float-facts and pointer-values read
    2N/N = 3.9 each on the `callers` axis (190 G instructions at N=512; 10.8 G since). The sites the estimates refuse are tried
    together, once. A step above 2.2 per doubling fails."""
    n = 128
    own = {}
    for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
        source = tmp_path / f"callers_{label}.c"
        source.write_text("" if size == 0 else scaling.callers(size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    grown = {}
    for step in ("analysis call-effects", "analysis through-memory", "analysis points-to", "analysis annotated", "analysis float-facts", "analysis pointer-values"):
        small, big = (own[label].get(step, 0.0) - own["empty"].get(step, 0.0) for label in ("n", "2n"))
        if big > 2.2 * small + 5.0:
            grown[step] = f"{small:.1f} -> {big:.1f} Minstr"
    assert not grown, grown


def test_loopmotion_and_the_cells_it_asks_stay_linear_in_the_loops_of_one_function(tmp_path):
    """branches(512) at -O2: loopmotion's own work was 6.3 G and the memory cells it derived afresh for each loop it sank a store
    from 5.5 G (2N/N = 3.7 and 4.0, 10.9 G inclusive), a dominator set for every block (quadratic in a chain of diamonds), the trips
    of each loop proved again and the accesses built again. A step 2N/N above 2.6 (slope 1.4) on the `branches` axis fails; a few
    Minstr of start-up are allowed."""
    n = 128
    own = {}
    for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
        source = tmp_path / f"branches_{label}.c"
        source.write_text("" if size == 0 else scaling.branches(size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    grown = {}
    for step in ("mir loopmotion", "analysis memory-cells"):
        small, big = (own[label].get(step, 0.0) - own["empty"].get(step, 0.0) for label in ("n", "2n"))
        if big > 2.6 * small + 5.0:
            grown[step] = f"{small:.1f} -> {big:.1f} Minstr"
    assert not grown, grown


def test_sroa_stays_linear_in_the_accesses_of_a_few_locals(tmp_path):
    """`mir sroa` compared each pair of accesses of one object for overlap: on `straight`, four locals read and written by every
    statement, it read 2N/N = 3.4, 3.6, 3.8 (345 Minstr of 6.9 G at N=2048) and does nothing with accesses that are one leaf. A leaf is
    now compared once. A step above 2.6 (slope 1.4) fails; a few Minstr of start-up are allowed."""
    n = 256
    own = {}
    for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
        source = tmp_path / f"straight_{label}.c"
        source.write_text("" if size == 0 else scaling.AXES["straight"](size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    small, big = (own[label].get("mir sroa", 0.0) - own["empty"].get("mir sroa", 0.0) for label in ("n", "2n"))
    assert big <= 2.6 * small + 5.0, f"{small:.1f} -> {big:.1f} Minstr"


def test_gvn_stays_below_quadratic_in_the_live_values_and_the_cells(tmp_path):
    """Pricing copied the live set at every instruction and the MemorySSA walk compared each load with every write it passed by the
    full alias rules: `mir gvn` at -O2 read 2N/N = 3.4 on `live` and on `cells` at N=128 (146 -> 494 and 122 -> 417 Minstr; 6.8 G
    and 5.8 G at N=1024). Since #1234 it reads 2.4 and 2.7; a step above 3.2 (slope 1.7) fails; a few Minstr of start-up are allowed."""
    grown = {}
    for axis, n in (("live", 128), ("cells", 128)):
        own = {}
        for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
            source = tmp_path / f"{axis}_{label}.c"
            source.write_text("" if size == 0 else scaling.AXES[axis](size))
            own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
        small, big = (own[label].get("mir gvn", 0.0) - own["empty"].get("mir gvn", 0.0) for label in ("n", "2n"))
        if big > 3.2 * small + 5.0:
            grown[axis] = f"{small:.1f} -> {big:.1f} Minstr"
    assert not grown, grown


def test_gvn_stays_below_cubic_in_the_blocks_of_a_chain_of_branches(tmp_path):
    """`mir gvn` on `branches` read 2N/N = 4.3, 5.2, 5.7 per doubling at N = 128..1024 (82.7 G at 1024): liveness took a round per block
    of a chain, every pass that asked who dominates built each block's whole set, and a load tried every earlier load of its bytes
    with a walk each. It reads 2.6 at N=128 (3.1 after #1247 alone); a step above 2.9 fails; a few Minstr of start-up are allowed."""
    n = 128
    own = {}
    for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
        source = tmp_path / f"branches_{label}.c"
        source.write_text("" if size == 0 else scaling.AXES["branches"](size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    small, big = (own[label].get("mir gvn", 0.0) - own["empty"].get("mir gvn", 0.0) for label in ("n", "2n"))
    assert big <= 2.9 * small + 5.0, f"{small:.1f} -> {big:.1f} Minstr"


def test_hoist_and_loopmotion_stay_quadratic_at_worst_in_the_depth_of_a_loop_nest(tmp_path):
    """nest(64) at -O2 (#1110): each load asked every instruction of the loop whether it writes it, and each store every access
    of the loop whether it reaches it, per loop: hoist read 2N/N = 5.5, 6.5, 7.1 (7.1 G at N=128) and loopmotion 6.0, 7.0 (N^3).
    Every block lies in as many loops as it is deep, so N^2 is the least; they now read 3.6 to 4.0, gcc's slope here being 2.1
    (4.3). A step above 4.8 (slope 2.26) fails; a few Minstr of start-up are allowed."""
    n = 32
    own = {}
    for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
        source = tmp_path / f"nest_{label}.c"
        source.write_text("" if size == 0 else scaling.nest(size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    grown = {}
    for step in ("mir hoist", "mir loopmotion"):
        small, big = (own[label].get(step, 0.0) - own["empty"].get(step, 0.0) for label in ("n", "2n"))
        if big > 4.8 * small + 5.0:
            grown[step] = f"{small:.1f} -> {big:.1f} Minstr"
    assert not grown, grown


def test_gvn_stays_linear_in_the_statements_of_a_straight_line(tmp_path):
    """Erasing the loads gvn forwarded looked each erased use up in the list of uses of its operand: a parameter read by every
    statement made `mir gvn` on `straight` read 2N/N = 2.4 at N=2048 (19.6 G at N=32768, 3.25x a doubling there). It reads 2.0; a step
    above 2.2 fails; a few Minstr of start-up are allowed."""
    n = 2048
    own = {}
    for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
        source = tmp_path / f"straight_{label}.c"
        source.write_text("" if size == 0 else scaling.AXES["straight"](size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    small, big = (own[label].get("mir gvn", 0.0) - own["empty"].get("mir gvn", 0.0) for label in ("n", "2n"))
    assert big <= 2.2 * small + 5.0, f"{small:.1f} -> {big:.1f} Minstr"


def test_ssa_flow_is_not_swept_per_step_of_the_longest_way_in_a_loop_nest(tmp_path):
    """`ssa flow` found each live value's distance to its next use by sweeping every block and live value up to 64 times: on `nest` at
    N=128 (a counter per loop, all live in the inner blocks) it cost 1,295 Minstr, 64 sweeps of N^2 pairs. Each value is walked back from
    its readers alone: 81 Minstr, the N^2 pairs once. The bound is on the cost, 2N/N being 4 for N^2 work."""
    source = tmp_path / "nest_128.c"
    source.write_text(scaling.AXES["nest"](128))
    own = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    assert own["ssa flow"] <= 200, f"{own['ssa flow']:.1f} Minstr"


def test_algebraic_stays_below_cubic_in_the_masks_of_one_function(tmp_path):
    """branches(N) at -O2 (#1254's follow-ups): each mask asked `ranges::scope_at` for its block's intervals, which built the map
    of every block's scope to give one (algebraic's own work read 2N/N = 3.7 to 4.0, 5.9 G at N=1024), and then copied the block's
    whole scope to read one value of it (3.3, 0.9 G). The edges hold a block's scope; the value is asked of them. It reads 2.0 (the
    analyses it asks for are theirs). A step above 2.4 (slope 1.26) fails; a few Minstr of start-up are allowed."""
    n = 128
    own = {}
    for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
        source = tmp_path / f"branches_{label}.c"
        source.write_text("" if size == 0 else scaling.branches(size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    small, big = (own[label].get("mir algebraic", 0.0) - own["empty"].get("mir algebraic", 0.0) for label in ("n", "2n"))
    assert big <= 2.4 * small + 5.0, f"{small:.1f} -> {big:.1f} Minstr"


def test_decide_stays_below_cubic_in_the_blocks_of_one_function(tmp_path):
    """branches(N) at -O2: `mir decide` asked every live block which way it goes at each round of the constant propagation (a round
    for each level of a chain of branches), and scanned every value for the ones left with no state at each stall: its own work
    read 2N/N = 3.5, 3.8 (450 / 1580 / 5985 Minstr at N=128..512). A block is asked again when a value it reads changes, and the
    values left are kept: it reads 2.9, 3.4 (the guards of each block, the next cost, are the rest). A step above 3.1 fails;
    a few Minstr of start-up are allowed."""
    n = 128
    own = {}
    for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
        source = tmp_path / f"branches_{label}.c"
        source.write_text("" if size == 0 else scaling.branches(size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    small, big = (own[label].get("mir decide", 0.0) - own["empty"].get("mir decide", 0.0) for label in ("n", "2n"))
    assert big <= 3.1 * small + 5.0, f"{small:.1f} -> {big:.1f} Minstr"


def test_loopslots_asks_each_instruction_once_not_once_for_each_loop_around_it(tmp_path):
    """`lir loopslots` worked out each instruction's effect on the frame and its registers, and whether it fits a slot in a register, for
    every loop around it: on `nest` at N=128 (a loop around the innermost instructions N deep) it cost 1,025 Minstr. Each is now worked
    out once for the body: 395 Minstr. The step is still quadratic in the depth, each loop gathering its blocks' slots (3.7x a doubling), so
    the bound is on the cost."""
    source = tmp_path / "nest_128.c"
    source.write_text(scaling.AXES["nest"](128))
    own = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    assert own["lir loopslots"] <= 600, f"{own['lir loopslots']:.1f} Minstr"


def test_the_inliner_splices_the_callers_sites_in_one_scan_and_interprocedural_stays_near_linear_in_the_callers(tmp_path):
    """Each splice of the caller of N callees counted the body again and the round found the module's recursive and addressed
    functions twice (`mir interprocedural` own 465 -> 1,232 Minstr from N=256 to 512, 2.65 per doubling). The sites are spliced in one scan, last to first, and the
    module is scanned once per round: 330 -> 698, 2.12. A step above 2.4 per doubling (slope 1.26) on the `callers` axis fails."""
    n = 256
    own = {}
    for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
        source = tmp_path / f"callers_{label}.c"
        source.write_text("" if size == 0 else scaling.callers(size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    small, big = (own[label].get("mir interprocedural", 0.0) - own["empty"].get("mir interprocedural", 0.0) for label in ("n", "2n"))
    assert big <= 2.4 * small + 5.0, f"{small:.1f} -> {big:.1f} Minstr"

def test_counted_stays_below_cubic_in_the_loops_of_one_function(tmp_path):
    """branches(N) at -O2: each loop's proof built the graph of the whole function (a vector of every block and what it names, a
    map of them) to read the blocks around the loop: `analysis counted` read 2N/N = 3.7, 3.8, 3.9 (46.7 / 170.6 / 648 / 2525 Minstr
    at N=128..1024). It reads 2.1 to 2.2 (8.8 / 18.8 / 40.4 / 88.5). A step above 2.6 (slope 1.38) fails; a few Minstr of
    start-up are allowed."""
    n = 128
    own = {}
    for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
        source = tmp_path / f"branches_{label}.c"
        source.write_text("" if size == 0 else scaling.branches(size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    small, big = (own[label].get("analysis counted", 0.0) - own["empty"].get("analysis counted", 0.0) for label in ("n", "2n"))
    assert big <= 2.6 * small + 5.0, f"{small:.1f} -> {big:.1f} Minstr"


def test_summaries_do_not_work_out_every_caller_of_an_edited_body_again(tmp_path):
    """chain(N) at -O2: an edited body started its whole chain of callers again from nothing (`summaries_updating`'s closure), so
    each of the run's ~N edits visited ~N bodies: `summaries visit` read 2N/N = 3.5, 3.7 (8.7 / 30.6 / 114 Minstr at N=32..128).
    A body is visited again from what it reads, and its readers where its summary came out other than it was (gcc's summaries
    stop where one does not change): 2.2, 2.2 (2.6 / 5.9 / 12.8). A step above 2.7 fails."""
    own = {}
    for label, size in (("empty", 0), ("n", 64), ("2n", 128)):
        source = tmp_path / f"chain_{label}.c"
        source.write_text("" if size == 0 else scaling.chain(size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    small, big = (own[label].get("summaries visit", 0.0) - own["empty"].get("summaries visit", 0.0) for label in ("n", "2n"))
    assert big <= 2.7 * small + 2.0, f"{small:.1f} -> {big:.1f} Minstr"


def test_decide_does_not_work_out_a_loop_for_each_block_in_it(tmp_path):
    """nest(N) at -O2: `Given::holds` worked out the followers of each counter of every loop around the block (a scan of the loop's
    values) for each block it was asked of, `mir decide` read 2N/N = 5.5 and 6.6 (160 / 874 / 5799 Minstr at N=32..128, 10% of the
    compile). A loop's followers are the loop's: the manager holds them (`Followers`), as it holds its counted proofs. It reads
    3.1, 3.7 (71 / 220 / 814). A step above 4.2 (slope 2.07) fails; a few Minstr of start-up are allowed."""
    n = 32
    own = {}
    for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
        source = tmp_path / f"nest_{label}.c"
        source.write_text("" if size == 0 else scaling.nest(size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    small, big = (own[label].get("mir decide", 0.0) - own["empty"].get("mir decide", 0.0) for label in ("n", "2n"))
    assert big <= 4.2 * small + 5.0, f"{small:.1f} -> {big:.1f} Minstr"


def test_loop_passes_read_a_loop_not_the_body_around_it(tmp_path):
    """branches(N) at -O2: lcssa, rotate and trivialunswitch each built the graph of the whole function for each loop (a vector of
    every block and its successors), lcssa also read every instruction outside the loop for its uses of what the loop defines and
    found the dominators of the body for each loop: 2N/N = 3.9, 4.0 and 3.5 at N=128 (lcssa 4.3 G at N=1024). A loop is asked of its own
    blocks and the header's users (`cfg::Around`), uses are found from the uses of a value, and the dominators once: 2.1, 2.0, 2.0
    (lcssa 105 M). A step above 2.6 (slope 1.38) fails; a few Minstr of start-up are allowed."""
    n = 128
    own = {}
    for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
        source = tmp_path / f"branches_{label}.c"
        source.write_text("" if size == 0 else scaling.branches(size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    grown = {}
    for step in ("mir lcssa", "mir rotate", "mir trivialunswitch"):
        small, big = (own[label].get(step, 0.0) - own["empty"].get(step, 0.0) for label in ("n", "2n"))
        if big > 2.6 * small + 5.0:
            grown[step] = f"{small:.1f} -> {big:.1f} Minstr"
    assert not grown, grown


def test_lsr_does_not_add_up_the_function_or_rebuild_its_graph_for_each_loop(tmp_path):
    """`mir lsr` on `branches` at N=512 (64 loops of a function of 500 blocks) added up the whole function's traffic for each loop and
    built the graph of the whole function to ask three blocks' neighbours (`rotate::_shape`): 1,391 Minstr; on `nest` at N=128 it also
    gathered each block's live sets again for each loop around it and built them as trees: 9,362. The traffic is added up once and
    each loop's instructions taken out, the neighbours are asked of the blocks, the live sets kept and the cells sorted in vectors:
    about 600 and 5,900; the spill forecast is a sweep of the loop's blocks, not a list of residents at each point of them: 1,340 on
    `nest`. The bounds are on the cost."""
    costs = {}
    for axis, n in (("branches", 512), ("nest", 128)):
        source = tmp_path / f"{axis}_{n}.c"
        source.write_text(scaling.AXES[axis](n))
        costs[axis] = gate.own_work(gate.levels_time.command("llrm", "O2", source))["mir lsr"]
    assert costs["branches"] <= 900 and costs["nest"] <= 2000, costs


def test_lsr_loop_reads_its_own_blocks_and_the_function_facts_once(tmp_path):
    """`mir lsr` on `branches` at N=1024 (128 loops of 1,000 blocks) walked every instruction of the function and copied its table of
    known registers for each loop (`induction::_recurrences`), took the frame objects and far views of the whole function for each
    loop and for each use and candidate, and made all of the function's cells' traffic into a table for each loop: 1,683 Minstr. A
    loop reads its own blocks, the frames and views are found once for the function, and the traffic is priced for the values asked:
    97. What is left is the analyses `lsr` asks for (liveness, loop products), whose own growth is theirs."""
    source = tmp_path / "branches_1024.c"
    source.write_text(scaling.AXES["branches"](1024))
    cost = gate.own_work(gate.levels_time.command("llrm", "O2", source))["mir lsr"]
    assert cost <= 300, cost


def test_an_entry_does_not_hold_up_the_summary_of_what_calls_through_a_pointer(tmp_path):
    """An address-taken function is an entry, and `main` calls it through a pointer: the two feed each other. Bringing the
    summaries up to date kept `main`'s from before the entry's was lowered (the callbacks, which `main` is part of, did not
    move), and `LLRM_CHECK_MODULES` found them differing from a whole run: it died on x_strlen at -O2."""
    source = tmp_path / "x_strlen.c"
    source.write_text(wrap.wrapped("x_strlen", programs.sources()["x_strlen"].read_text()))
    done = subprocess.run(
        [*gate.levels_time.command("llrm", "O2", source), *programs.LLRM_FLAGS],
        capture_output=True,
        text=True,
        env={**os.environ, "LLRM_CHECK_MODULES": "1"},
    )
    assert done.returncode == 0, done.stderr[-300:]


def test_merging_blocks_does_not_build_the_graph_of_the_function_for_each_merge(tmp_path):
    """callers(N) at -O2: `cfg::merged` (run by decide, rotate and lsr) built the graph of the whole function and its predecessor
    sets, again after each merge it made: `mir decide` read 2N/N = 3.2, 3.65 (2.5 / 7.9 / 29.0 G at N=1024..4096). Each block
    is asked of its own neighbours (the uses of its jump's target): 2.0, 2.1 (0.9 / 1.9 / 4.0 G). A step above 2.6 fails."""
    own = {}
    for label, size in (("empty", 0), ("n", 1024), ("2n", 2048)):
        source = tmp_path / f"callers_{label}.c"
        source.write_text("" if size == 0 else scaling.callers(size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    small, big = (own[label].get("mir decide", 0.0) - own["empty"].get("mir decide", 0.0) for label in ("n", "2n"))
    assert big <= 2.6 * small + 5.0, f"{small:.1f} -> {big:.1f} Minstr"


def test_summaries_keep_the_call_graph_when_a_body_calls_a_deeper_one(tmp_path):
    """chain(N) at -O2: each splice of a callee into its caller gives the caller calls to bodies below the callee, and the
    graph of all N bodies (components, order, readers) was made again for it: `summaries topology` read 23.8 / 107.5 Minstr at
    N=64 / 128 (457 at 256). A new call to a body of an earlier component leaves the order valid, and the graph is brought up
    to date for that body alone: 2.6 / 8.0. Above 30 Minstr at N=128 fails."""
    source = tmp_path / "chain128.c"
    source.write_text(scaling.chain(128))
    assert gate.own_work(gate.levels_time.command("llrm", "O2", source)).get("summaries topology", 0.0) <= 30.0


def test_interprocedural_own_work_stays_near_linear_on_the_chain_functions_and_callers_axes(tmp_path):
    """The module-wide step did work per body that grew with the module: the declarations were compared global by global after each of
    N bodies, the noreturn fixed point rounds took N bodies N times, and the no-recurse proof walked everything each function reaches
    (`mir interprocedural` own, 2N/N on chain: 2.80). A doubling above 2.65, 2.15 and 2.2 fails (chain keeps what `analysis summaries`
    leaves in it); functions and callers hold what the call graph's dense components gave them (2.01, 2.07)."""
    limits = {"chain": (128, 2.65), "functions": (512, 2.15), "callers": (1024, 2.2)}
    grown = {}
    for axis, (n, limit) in limits.items():
        own = {}
        for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
            source = tmp_path / f"{axis}_{label}.c"
            source.write_text("" if size == 0 else scaling.AXES[axis](size))
            own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
        small, big = (own[label].get("mir interprocedural", 0.0) - own["empty"].get("mir interprocedural", 0.0) for label in ("n", "2n"))
        if big > limit * small:
            grown[axis] = f"{small:.0f} -> {big:.0f} Minstr"
    assert not grown, grown
