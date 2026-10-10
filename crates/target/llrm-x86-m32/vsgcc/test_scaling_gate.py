"""scaling_gate.py: a pass gone quadratic reads as 2N/N = 4, a linear one as 2, and neither direction of change passes unseen."""
import sys
from pathlib import Path

import pytest

import scaling
import scaling_gate as gate

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


def test_lsr_does_not_add_up_the_function_or_rebuild_its_graph_for_each_loop(tmp_path):
    """`mir lsr` on `branches` at N=512 (64 loops of a function of 500 blocks) added up the whole function's traffic for each loop and
    built the graph of the whole function to ask three blocks' neighbours (`rotate::_shape`): 1,391 Minstr; on `nest` at N=128 it also
    gathered each block's live sets again for each loop around it and built them as trees: 9,362. The traffic is added up once and
    each loop's instructions taken out, the neighbours are asked of the blocks, the live sets kept and the cells sorted in vectors:
    about 600 and 5,900. Both stay quadratic (the loops are, and each changed loop invalidates what the next asks for), so the bounds
    are on the cost."""
    costs = {}
    for axis, n in (("branches", 512), ("nest", 128)):
        source = tmp_path / f"{axis}_{n}.c"
        source.write_text(scaling.AXES[axis](n))
        costs[axis] = gate.own_work(gate.levels_time.command("llrm", "O2", source))["mir lsr"]
    assert costs["branches"] <= 900 and costs["nest"] <= 7500, costs
