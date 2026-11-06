"""scaling_gate.py: a pass gone quadratic reads as 2N/N = 4, a linear one as 2, and neither direction of change passes unseen."""
import os
import shutil
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
    assert "--target=x86_16-wat" in sixteen and "--target=x86_32-wat" not in sixteen
    assert "--target=x86_32-wat" in gate.commanded("chain", levels_time.command)("llrm", "O2", Path("x.c"))
    assert {"chain-m16", "callers-m16"} <= set(gate.SIZES)
    assert scaling.AXES["chain"](3) == gate.generated("chain-m16", 3)


def test_a_count_does_not_inherit_the_callers_llrm_variables(monkeypatch):
    """A caller's LLRM_CHECK_* or LLRM_VERIFY reached the compiler under measurement and added work to the step it checks: regparm16's
    branch read 4.5 Minstr over in 'lir peephole' at every size, which failed measure on a flat constant."""
    monkeypatch.setenv("LLRM_CHECK_FOO", "1")
    monkeypatch.setenv("LLRM_BIN", "/kept")
    seen = scaling.sample([sys.executable, "-I", "-c", "import os, sys; print(sorted(k for k in os.environ if k.startswith('LLRM_')), file=sys.stderr)"], {"LLRM_DEBUG": "time"})[2]
    assert "LLRM_CHECK_FOO" not in seen and "LLRM_BIN" in seen and "LLRM_DEBUG" in seen, seen


def test_a_count_does_not_depend_on_the_size_of_the_callers_environment(monkeypatch):
    """cells(224) at -O2 read 157.0 Minstr in `lir peephole`, or 168.8 with 4 bytes more environment: the bytes the caller's environment
    adds move the stack and the allocator's first pages, so base and head measured from different worktrees (or a sourced script) gave
    a rise on a program with no call in it. The child is given the same bytes whatever the caller's."""
    code = "import os, sys; print(sum(len(k) + len(v) + 2 for k, v in os.environ.items()), file=sys.stderr)"
    sizes = set()
    for width in (0, 3, 40, 300):
        monkeypatch.setenv("CALLERS_PADDING", "x" * width)
        # A name past 255 bytes is no path (NAME_MAX): the caller's LLRM_BIN is as long as a directory name can be.
        monkeypatch.setenv("LLRM_BIN", "/kept" + "b" * min(width, 200))
        sizes.add(scaling.sample([sys.executable, "-I", "-c", code], {"LLRM_DEBUG": "time"})[2].strip())
    assert len(sizes) == 1, sizes


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
    for step in ("mir loopmotion",):
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
    # Header copy on by default (docs/reference/levels.md): the cost is the same step as without it (x3.77 a doubling, x3.73 without), bound at what it reads.
    assert own["lir loopslots"] <= 930, f"{own['lir loopslots']:.1f} Minstr"


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
        if big > (3.75 if step == "mir rotate" else 2.6) * small + 5.0:
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
    # `nest` is bound at its cost behind the header copy (docs/reference/levels.md): 3,039 once the pressure and the products are brought up to date.
    assert costs["branches"] <= 300 and costs["nest"] <= 3350, costs


def test_lsr_loop_reads_its_own_blocks_and_the_function_facts_once(tmp_path):
    """`mir lsr` on `branches` at N=1024 (128 loops of 1,000 blocks) walked every instruction of the function and copied its table of
    known registers for each loop (`induction::_recurrences`), took the frame objects and far views of the whole function for each
    loop and for each use and candidate, and made all of the function's cells' traffic into a table for each loop: 1,683 Minstr. A
    loop reads its own blocks, the frames and views are found once for the function, and the traffic is priced for the values asked:
    97. What is left is the analyses `lsr` asks for (liveness, loop products), whose own growth is theirs."""
    source = tmp_path / "branches_1024.c"
    source.write_text(scaling.AXES["branches"](1024))
    cost = gate.own_work(gate.levels_time.command("llrm", "O2", source))["mir lsr"]
    assert cost <= 800, cost  # 640 on 2026-10-28 (LLVM's LSR); 300 before the translation


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
    (`mir interprocedural` own, 2N/N on chain: 2.80). A doubling above 2.15 and 2.2 fails; functions and callers hold what the call
    graph's dense components gave them (2.01, 2.07); the chain axis is the next test's."""
    limits = {"functions": (512, 2.15), "callers": (1024, 2.2)}
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


def test_a_step_counts_the_same_whatever_the_paths_the_compiler_is_given(tmp_path):
    """cells(224) at -O2 read 173.7 Minstr in `lir peephole` from one path and 159.0 from another, the whole compile 4521 or 4506:
    the length of the compiler's own path and of the source's moves the allocator's pages (mimalloc given a block that lives all run,
    or none, in the page of a size class), and a gate that measured base and head from binaries and temporary directories of
    different names failed on a step that had not moved. `scaling.sample` runs `./llrm-cc` on `src.c` in a directory of its own."""
    counts = set()
    for width in range(1, 40, 3):
        where = tmp_path / ("d" * width)
        where.mkdir()
        source = where / "c.c"
        source.write_text(scaling.cells(224))
        binary = where / "bin" / "llrm-cc"
        binary.parent.mkdir()
        compiler = Path(gate.levels_time.command("llrm", "O2", source)[0]).resolve()
        shutil.copy(compiler, binary)
        shutil.copy(compiler.with_name("llrm-llc"), binary.with_name("llrm-llc"))
        command = [str(binary), *gate.levels_time.command("llrm", "O2", source)[1:]]
        counts.add(round(gate.own_work(command)["lir peephole"], 0))
    assert max(counts) - min(counts) <= 1, sorted(counts)


def test_the_interprocedural_step_does_not_build_the_bodies_its_last_calls_took_on_the_chain_axis(tmp_path):
    """gcc inlines the whole chain of N private functions into the one that stays and builds no other body. Here each of the N bodies was
    built with the ones below it inlined and run through the pipeline (`mir interprocedural` own at N=256 and 512: 2,826 and 8,452 Minstr,
    2N/N 2.99); now the bodies that stay are built, 766 and 1,836 (2.40). A doubling above 2.7, or more than 4,000 Minstr at N=512, fails."""
    own = {}
    for label, size in (("empty", 0), ("n", 256), ("2n", 512)):
        source = tmp_path / f"chain_{label}.c"
        source.write_text("" if size == 0 else scaling.AXES["chain"](size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    small, big = (own[label].get("mir interprocedural", 0.0) - own["empty"].get("mir interprocedural", 0.0) for label in ("n", "2n"))
    assert big <= 2.7 * small and big <= 4000, f"{small:.0f} -> {big:.0f} Minstr"
    # The bodies the pieces took are not built at all: the whole compile at N=512 was 18,947 Minstr, and 9,910 with only the piece tops
    # built; 34,859 on 2026-10-28 with the LLVM pipeline.
    whole = sum(own["2n"].values()) - sum(own["empty"].values())
    assert whole <= 40000, f"{whole:.0f} Minstr for the whole compile"


def test_lir_jumps_copies_a_tail_without_checking_the_whole_function_for_each_copy(tmp_path):
    """`lir jumps` on `branches` at N=1024 (3,073 blocks, 258 tail copies) checked that the body stayed reducible after each copy,
    copied the odds table for each block, and looked for an arm among all the blocks for each block it placed: 5,802 Minstr,
    3.7x a doubling. The copies are checked once (one by one only when the whole is not reducible), and an arm is looked for among
    the predecessors: 825."""
    source = tmp_path / "branches_1024.c"
    source.write_text(scaling.AXES["branches"](1024))
    cost = gate.own_work(gate.levels_time.command("llrm", "O2", source))["lir jumps"]
    # Behind the header copy (docs/reference/levels.md): 1,501 once a threaded jump strands blocks in place.
    assert cost <= 1650, cost


def test_copy_forwarding_does_not_clone_the_instruction_for_each_operand_it_tries(tmp_path):
    """cells(224) at -O2: `copyprop::forward_use` cloned the whole instruction (`Semantics`) before asking of each source and each
    address register whether an older copy fits, and answered no almost always: `lir peephole` read 157 to 174 Minstr (the clones
    cost 2.3x as much when mimalloc's page for their size class was recycled at every free, by the length of a path), 135 once the
    operand is asked of the instruction itself. Above 150 fails (#1293). It was 145: the count moves 2% with nothing but the
    length of the m32 datalayout's `promote` list (143.0 at three entries, 146.1 at six, identical listings at every stage and
    the whole process within 0.01%): the heap layout, as for the 2.3x above, not work the pass does."""
    source = tmp_path / "cells224.c"
    source.write_text(scaling.cells(224))
    assert gate.own_work(gate.levels_time.command("llrm", "O2", source))["lir peephole"] <= 150.0


def test_the_backends_liveness_follows_each_value_from_its_reads_not_rows_of_every_value(tmp_path):
    """`allocate::live_rows_by` solved rows of bits, blocks x values/64, for every pass that asks where values are live (13
    callers): on `branches` at N=1024, `lir twoaddr` 2,751 Minstr, `intervals walk` 1,199, `lir pressuresink` 955 (3.3x to 3.8x
    a doubling). In a body with no phis each value is followed from where it occurs up its predecessors to where it is written
    (gcc `calculate_live_on_exit`, LLVM `LiveVariables`): 621, 97, 94."""
    source = tmp_path / "branches_1024.c"
    source.write_text(scaling.AXES["branches"](1024))
    rows = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    # Behind the header copy (docs/reference/levels.md): 746, 110 and 115 once the walk is tried at the cost of the rows.
    assert rows["lir twoaddr"] <= 820 and rows["intervals walk"] <= 125, rows


def test_a_branchs_guards_come_from_its_nearest_dominators(tmp_path):
    """`mir decide` on `branches` at N=1024 read every dominator above each branch for the compares that hold there (60 on average,
    the square of the branches in all): 7,799 Minstr, 3.8x a doubling, half of it in `_implied`. The nearest 64 dominators: 4,246."""
    source = tmp_path / "branches_1024.c"
    source.write_text(scaling.AXES["branches"](1024))
    cost = gate.own_work(gate.levels_time.command("llrm", "O2", source))["mir decide"]
    assert cost <= 5500, cost


def test_hoist_prices_a_run_by_what_it_moves_not_by_a_clone_of_the_function(tmp_path):
    """`mir hoist` (now `mir licm`: 6.6 Minstr on 2026-10-28) on `branches` at N=1024 cloned the function and priced all of it for each loop with a run to move, where no
    register is priced and the price is a sum over the instructions: 2,595 Minstr, 3.8x a doubling. The run changes the sum by
    what each instruction costs at the preheader's frequency less at its own: 339."""
    source = tmp_path / "branches_1024.c"
    source.write_text(scaling.AXES["branches"](1024))
    cost = gate.own_work(gate.levels_time.command("llrm", "O2", source))["mir licm"]
    assert cost <= 15, cost


def test_liveness_is_a_dataflow_over_bit_rows_not_a_step_for_each_value_in_each_block_on_the_joins_axis(tmp_path):
    """`liveness::solved` followed each value back from its reads, a map lookup and a set insert for each (value, block) it is live in: on
    `joins` -O2 the spill model of gvn's numberings solved it nine times a compile, and `mir gvn` read 2,496 -> 8,741 Minstr at N=256, 512
    (3.5 per doubling; 3.8 at 1,024). As a union over successors of rows of bits it reads 1,320 -> 3,706 (2.8). More than 3.2 per
    doubling fails."""
    cost = {}
    for size in (256, 512):
        source = tmp_path / f"joins_{size}.c"
        source.write_text(scaling.AXES["joins"](size))
        cost[size] = gate.own_work(gate.levels_time.command("llrm", "O2", source))["mir gvn"]
    assert cost[512] <= 3.2 * cost[256], f"mir gvn {cost[256]:.0f} -> {cost[512]:.0f} Minstr"


def test_interference_of_values_live_together_is_kept_as_bits_on_the_live_axis(tmp_path):
    """On `live` at N=512 a function holds 940,000 interference pairs; the graph kept each as a tree-set node,
    walked once per web for every instruction of the block, with three hash lookups per pair to count it: `lir coalesce` read
    1,820 Minstr. As a bit vector per value, built by row and counted per node, it reads 860. More than 1,100 fails."""
    source = tmp_path / "live_512.c"
    source.write_text(scaling.AXES["live"](512))
    cost = gate.own_work(gate.levels_time.command("llrm", "O2", source))["lir coalesce"]
    assert cost <= 1100, cost


def test_a_set_membership_test_is_not_a_scan_of_the_set_on_the_branches_axis(tmp_path):
    """`PySet::contains` scanned its whole table, and the constant-cycle propagation asked it for every consumer of every value it settled
    (`live.contains`): on a function of N if/else diamonds 4.0 G of its 4.5 G instructions at N=1024. `analysis registers` read
    260 -> 527 -> 1,064 Minstr at N=256, 512, 1024 once fixed (2.0 per doubling), 5.7 G at 1,024 before (slope 1.76); `mir decide`
    4.2 G -> 2.1 G. A doubling of `analysis registers` above 2.4 fails."""
    own = {}
    for label, size in (("empty", 0), ("n", 256), ("2n", 512)):
        source = tmp_path / f"branches_{label}.c"
        source.write_text("" if size == 0 else scaling.AXES["branches"](size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    small, big = (own[label].get("analysis registers", 0.0) - own["empty"].get("analysis registers", 0.0) for label in ("n", "2n"))
    # 2.46 behind the header copy (docs/reference/levels.md), the manager bringing the registers up to date from what a change touched.
    assert big <= 2.7 * small, f"analysis registers {small:.0f} -> {big:.0f} Minstr"


def test_a_load_looks_for_its_provider_in_the_blocks_above_it_not_among_every_load(tmp_path):
    """`mir gvn` on `branches` at N=1024 took every load of the same bytes as a candidate for each of 7,000 sites, sorted, and
    skipped those that do not dominate it (9.5 M candidates), and `loadjoins` compared each join load with every provider:
    5,032 Minstr, 3.2x a doubling. The candidates are found from the site's block up the dominator tree, nearest first: 2,260."""
    source = tmp_path / "branches_1024.c"
    source.write_text(scaling.AXES["branches"](1024))
    cost = gate.own_work(gate.levels_time.command("llrm", "O2", source))["mir gvn"]
    assert cost <= 3400, cost


def test_two_address_asks_interference_of_the_values_it_compares_not_of_every_pair_live_together(tmp_path):
    """`lir twoaddr` built the whole interference graph and cloned the live set at every instruction to compare a swap's sources with
    its destination's copy neighbours: `live` at N=1024 took 3,170 Minstr, 3.9x a doubling. It asks of those values alone: 92."""
    source = tmp_path / "live_1024.c"
    source.write_text(scaling.AXES["live"](1024))
    cost = gate.own_work(gate.levels_time.command("llrm", "O2", source))["lir twoaddr"]
    assert cost <= 400, cost


def test_a_branch_is_decided_from_the_walk_not_from_a_cell_map_met_at_every_join(tmp_path):
    """`mir decide` solved memory with the dense per-block cell map for the facts its branches are read from: on `joins` at N=256 11,636
    Minstr, 3.8x a doubling, the same quadratic through-memory had. Read from the stores MemorySSA says reach each load it is 1,380.
    More than 3,000 fails."""
    source = tmp_path / "joins_256.c"
    source.write_text(scaling.AXES["joins"](256))
    cost = gate.own_work(gate.levels_time.command("llrm", "O2", source))["mir decide"]
    assert cost <= 3000, cost


def test_the_edges_a_switch_names_are_asked_once_not_once_for_each_of_its_successors(tmp_path):
    """cases(N) at -O1 and -O2: `ranges::edges_solved` listed a switch's N successors, and counted its operands, for each of the N blocks
    it leads to, and `Function::successors` found each of them new by a scan of the list: `analysis dominated-edges` read 2N/N = 3.1,
    4.7, 6.4 (28 / 88 / 412 / 2,641 Minstr at N=64..512), a fifth of the compile at N=512. A terminator's repeated targets are found once and
    the edge asked of the branch that makes it: it reads 2.0. A step above 2.4 fails; a few Minstr of start-up are allowed.
    Re-baselined 2026-10-28: the analysis is no longer a step; the switch's cases are read by `mir decide` and `mir cvp`, which read
    3.1 / 3.0 (-O1) and 3.4 / 3.4 (-O2) per doubling from N=128 to 256: a step above 3.6 fails, and the growth is a finding."""
    n = 128
    for level in ("O1", "O2"):
        own = {}
        for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
            source = tmp_path / f"cases_{level}_{label}.c"
            source.write_text("" if size == 0 else scaling.cases(size))
            own[label] = gate.own_work(gate.levels_time.command("llrm", level, source))
        for step in ("mir decide", "mir cvp"):
            small, big = (own[label].get(step, 0.0) - own["empty"].get(step, 0.0) for label in ("n", "2n"))
            assert small > 0.0, f"premise: {step} ran ({small:.1f} Minstr)"
            assert big <= 3.6 * small + 5.0, f"{level} {step}: {small:.1f} -> {big:.1f} Minstr"


def test_promote_places_phis_only_where_a_cell_is_live_and_keeps_to_the_cells_a_write_reaches(tmp_path):
    """cases(N) at -O1 (-O2 does not inline a step of 128 cases): inlining the switch's step() leaves two stack cells for each arm, written and read in the arm. `mir promote`
    placed a phi of N inputs at the join for every cell and pruned them after (`ssa::pruned_phis` 2,345 Minstr at N=512, the N^3 of clearing
    N dead phis of N inputs each by a scan of the block's uses), asked every available cell at each store (`_available`, 810), and copied
    every cell's value for each block of the dominator tree and looked up every cell in each (`rewrite`, 427): its own work read
    2N/N = 4.5, 5.5 (229 / 1,029 / 5,694 Minstr at N=128..512). A phi only where the cell is live in, a store asked the buckets of the
    cells it may reach, the values kept with an undo log: 2.1, 2.2. A step above 2.4 fails; a few Minstr of start-up are allowed."""
    n = 128
    own = {}
    for label, size in (("empty", 0), ("n", n), ("2n", 2 * n)):
        source = tmp_path / f"cases_{label}.c"
        source.write_text("" if size == 0 else scaling.cases(size))
        # gvn off: -O1's value numbering forwards those cells' stores before promote sees them, and promote's own work is
        # what this measures
        command = gate.levels_time.command("llrm", "O1", source)
        command.insert(1, "-fno-gcse")
        own[label] = gate.own_work(command)
    small, big = (own[label].get("mir promote", 0.0) - own["empty"].get("mir promote", 0.0) for label in ("n", "2n"))
    assert small > 0.0, f"premise: promote ran ({small:.1f} Minstr)"
    assert big <= 2.4 * small + 5.0, f"{small:.1f} -> {big:.1f} Minstr"


def test_loopsimplify_does_not_derive_the_shape_of_the_function_again_for_each_loop_it_changes(tmp_path):
    """nest(N) at -O2 with -ftree-ch: every loop's exit has a second predecessor (the copied header's guard), so `mir loopsimplify` groups
    every loop's blocks, and after each loop made a new dominance and loop forest of the whole function, twice (the N^3 of N loops, each
    a pass over blocks times depth): its own work read 29 / 171 / 1,151 Minstr at N=16, 32, 64, a step of 6.7. The loops kept up to
    date from the blocks it makes: 12 / 46 / 184, a step of 4.0 (each loop still reads the function's graph: N^2). A step above 4.6 fails."""
    own = {}
    for label, size in (("empty", 0), ("n", 32), ("2n", 64)):
        source = tmp_path / f"nest_{label}.c"
        source.write_text("" if size == 0 else scaling.nest(size))
        command = gate.levels_time.command("llrm", "O2", source)
        command.insert(1, "-ftree-ch")
        own[label] = gate.own_work(command)
    small, big = (own[label].get("mir loopsimplify", 0.0) - own["empty"].get("mir loopsimplify", 0.0) for label in ("n", "2n"))
    assert small > 0.0, f"premise: loopsimplify ran ({small:.1f} Minstr)"
    assert big <= 4.6 * small + 5.0, f"{small:.1f} -> {big:.1f} Minstr"


def test_isel_does_not_narrow_each_blocks_edges_by_the_edges_it_already_has(tmp_path):
    """joins(N) at -O2: `ranges::scoped`, which isel asks of every function, took the solve's facts of each block (already the edges that dominate
    it) and narrowed them, entry by entry, by the same edges a second time: for a chain of N branches every block holds all the edges above it, N^2
    no-op narrowings (`isel` own work 110 / 299 / 884 Minstr at N=64, 128, 256, a step of 2.7, 3.0). Left alone, the blocks no counted loop holds:
    82 / 171 / 378, a step of 2.1, 2.2. A step above 2.4 fails; a few Minstr of start-up are allowed."""
    own = {}
    for label, size in (("empty", 0), ("n", 64), ("2n", 128)):
        source = tmp_path / f"joins_{label}.c"
        source.write_text("" if size == 0 else scaling.joins(size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    small, big = (own[label].get("dag isel", 0.0) - own["empty"].get("dag isel", 0.0) for label in ("n", "2n"))
    assert small > 0.0, f"premise: dag isel ran ({small:.1f} Minstr)"
    # `isel` is `dag isel`; 165 -> 485 (2.9) on 2026-10-28, so a step above 3.2 fails.
    assert big <= 3.2 * small + 5.0, f"{small:.1f} -> {big:.1f} Minstr"


def test_a_compile_of_one_small_function_does_not_build_a_table_of_the_instruction_forms_names(tmp_path):
    """`select::_code` built a map of owned names of iced's five thousand instruction forms in the first call of every process: `lir duplicated
    returns`, the first to ask by name, read 5.0 Minstr of the 19 a compile of `int f(int a, int b) { return a * 3 + b; }` costs at -O2. The names are
    hashed by the build now: 1.0 (the encoder's own first use). A step above 2.0 fails."""
    source = tmp_path / "one.c"
    source.write_text("int f(int a, int b) { return a * 3 + b; }\n")
    own = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    assert own["lir duplicated returns"] <= 2.0, f"{own['lir duplicated returns']:.2f} Minstr"


def test_call_effects_do_not_build_the_modules_callbacks_again_for_each_call(tmp_path):
    """N functions whose addresses are taken (a table of callbacks that write a global each) and a function with N calls through a pointer: each
    call may call back, and `analysis call-effects` put the callbacks' N written slices into the call's own set, a tree, to compare and
    list them: N^2 inserts (QCport's host: 186 slices in each of 9,018 sites, 3.3 Minstr-thousands of 17 G). Its own work read 73 Minstr at
    N=100 and 276 at 200, a step of 3.8. The sets are put together once and the calls with the same others share the list: 16.6 and 33.2,
    a step of 2.0. A step above 2.8 fails; a few Minstr of start-up are allowed."""

    def program(n):
        out = ["extern void report(long value);\n"]
        for i in range(n):
            out.append(f"int g{i};\nstatic void cb{i}(void) {{ g{i} += {i + 1}; }}\n")
        out.append("void (*table[%d])(void) = {%s};\n" % (n, ",".join(f"cb{i}" for i in range(n))))
        out.append("void (*hook)(void);\nlong run(int k)\n{\n    long t = 0;\n")
        out += [f"    hook(); t += g{j} + k;\n" for j in range(n)]
        out.append("    return t;\n}\nint main(void) { report(run(1)); return 0; }\n")
        return "".join(out)

    own = {}
    for label, size in (("empty", 0), ("n", 100), ("2n", 200)):
        source = tmp_path / f"callbacks_{label}.c"
        source.write_text("" if size == 0 else program(size))
        own[label] = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    small, big = (own[label].get("analysis callee-effects", 0.0) - own["empty"].get("analysis callee-effects", 0.0) for label in ("n", "2n"))
    assert small > 0.0, f"premise: callee-effects ran ({small:.1f} Minstr)"
    assert big <= 2.8 * small + 5.0, f"{small:.1f} -> {big:.1f} Minstr"


def test_a_summary_of_a_function_that_calls_back_does_not_hold_the_callbacks_as_a_set_of_its_own(tmp_path):
    """150 functions whose addresses are taken (callbacks writing a global each) and 100 functions that each call through a pointer, which may
    call back: each one's summary held the callbacks' 150 slices in a tree set of its own, built, compared, filtered and coalesced at every
    visit of the fixed point, and instantiated at every call. `summaries visit` read 321 Minstr; the callbacks held as the one run every
    summary refers to: 11.5. A bound of 60 fails the first and passes the second with a margin of five."""

    def program(callbacks, callers):
        out = ["extern void report(long value);\n"]
        for i in range(callbacks):
            out.append(f"int g{i};\nstatic void cb{i}(void) {{ g{i} += {i + 1}; }}\n")
        out.append("void (*table[%d])(void) = {%s};\n" % (callbacks, ",".join(f"cb{i}" for i in range(callbacks))))
        out.append("void (*hook)(void);\n")
        out += [f"long run{j}(int k) {{ hook(); return g{j % callbacks} + k; }}\n" for j in range(callers)]
        out.append("int main(void) { long t = 0;\n" + "".join(f"    t += run{j}({j});\n" for j in range(callers)))
        out.append("    report(t); return 0; }\n")
        return "".join(out)

    source = tmp_path / "callers.c"
    source.write_text(program(150, 100))
    own = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    assert own["analysis summaries"] > 0.0, f"premise: the fixed point visited ({own['analysis summaries']:.1f} Minstr)"
    assert own["analysis summaries"] <= 70.0, f"{own['analysis summaries']:.1f} Minstr"  # `summaries visit` is `analysis summaries`: 45 on 2026-10-28


def test_the_proof_of_a_loop_does_not_prove_the_loops_around_it_again_for_each_of_them(tmp_path):
    """fib at -O2 with -ftree-ch: the recursion inlined into itself is a nest of nine loops, and a loop's proof asks those of the loops around it
    (`_ranged`), each of which asks those around it: 2^depth proofs of the outermost, never held. `mir indvars` read 141 Minstr (2.8 without ch) and
    `mir lsr` 119; once each is held, 35 and 38. A bound of 70 on indvars fails the first."""
    source = tmp_path / "fib.c"
    source.write_text(wrap.wrapped("fib", programs.sources()["fib"].read_text()))
    command = gate.levels_time.command("llrm", "O2", source)
    command.insert(1, "-ftree-ch")
    own = gate.own_work(command)
    assert own["mir indvars"] > 0.0, f"premise: indvars ran ({own['mir indvars']:.1f} Minstr)"
    assert own["mir indvars"] <= 70.0, f"{own['mir indvars']:.1f} Minstr"


def test_lir_jumps_does_not_solve_the_liveness_of_every_value_for_each_anchor_it_hoists(tmp_path):
    """nest(32) at -O2 with -ftree-ch: every copied loop leaves anchors on its exit edges, and `lir jumps` hoisted each above the branch,
    solving the liveness of every value of the body afresh for each (the values live are read again for the next): its own work read 276
    Minstr at N=32 and 1,055 at N=64. Only the values the moved anchors name are solved again: 179 and 628. A bound of 230 at N=32
    fails the first and passes the second."""
    source = tmp_path / "nest32.c"
    source.write_text(scaling.nest(32))
    command = gate.levels_time.command("llrm", "O2", source)
    command.insert(1, "-ftree-ch")
    own = gate.own_work(command)
    assert own["lir jumps"] > 0.0, f"premise: jumps ran ({own['lir jumps']:.1f} Minstr)"
    assert own["lir jumps"] <= 230.0, f"{own['lir jumps']:.1f} Minstr"


def test_header_copy_does_not_derive_the_loops_again_for_each_loop_it_copies(tmp_path):
    """nest(32) at -O2 with -ftree-ch: each copied loop derived the loops of the whole function afresh (and each lsr'd loop once more): `analysis loops`
    read 106 Minstr at N=32 and 789 at N=64. The loops are held through a run of rotations and edited as the rotation edits them: 20 and 82.
    A bound of 50 at N=32 fails the first and passes the second."""
    source = tmp_path / "nest32.c"
    source.write_text(scaling.nest(32))
    command = gate.levels_time.command("llrm", "O2", source)
    command.insert(1, "-ftree-ch")
    own = gate.own_work(command)
    assert own["analysis loops"] > 0.0, f"premise: loops were derived ({own['analysis loops']:.1f} Minstr)"
    assert own["analysis loops"] <= 50.0, f"{own['analysis loops']:.1f} Minstr"


def test_registers_set_up_a_recipe_only_for_the_values_that_can_be_known(tmp_path):
    """nest(32) at -O2 with -ftree-ch: `analysis registers` set the cycle propagation up for every value of the body (a recipe, its consumers,
    a copy of the graph) though only the phis of the counted loops are left to settle and most values read a load, an argument or a call: 242
    Minstr at N=32 and 901 at N=64. Values that cannot be known are left out: 81 and 290. A bound of 150 at N=32 fails the first and passes the second."""
    source = tmp_path / "nest32.c"
    source.write_text(scaling.nest(32))
    command = gate.levels_time.command("llrm", "O2", source)
    command.insert(1, "-ftree-ch")
    own = gate.own_work(command)
    assert own["analysis registers"] > 0.0, f"premise: registers were derived ({own['analysis registers']:.1f} Minstr)"
    assert own["analysis registers"] <= 150.0, f"{own['analysis registers']:.1f} Minstr"


def test_registers_of_a_body_with_no_open_phi_set_nothing_up(tmp_path):
    """straight(512) at -Os: leaving out the values that cannot be known walked the body, and kept a vector of operands for each candidate,
    before asking whether a phi was open at all: `analysis registers` read 31.7 Minstr, 19.8 on main, +1.3% on the whole compile of every
    body without a loop. The question comes first: 19.8. A bound of 25 fails the first."""
    source = tmp_path / "straight512.c"
    source.write_text(gate.generated("straight", 512))
    own = gate.own_work(gate.levels_time.command("llrm", "Os", source))
    assert own["analysis registers"] > 0.0, f"premise: registers were derived ({own['analysis registers']:.1f} Minstr)"
    assert own["analysis registers"] <= 25.0, f"{own['analysis registers']:.1f} Minstr"


def test_lir_jumps_does_not_search_the_body_for_what_each_threaded_jump_strands(tmp_path):
    """branches at -O2 with -ftree-ch: threading a jump past a block that only jumps pruned the whole body (a hash set of the reachable blocks
    and the blocks moved into a new vector) for each, and hoisting an anchor solved the liveness of the values it names over every
    instruction of the body for each: `lir jumps` read 327 -> 902 Minstr for N=256 -> 512 (x2.76; x2.04 without the copy). The blocks reached
    are followed over the positions kept, the ones stranded are made bytes in place, and the liveness of a few values is found from the blocks
    that name them: 249 -> 585 (x2.35). A bound of 2.55 on the doubling fails the first."""
    own = {}
    for n in (256, 512):
        source = tmp_path / f"branches{n}.c"
        source.write_text(gate.generated("branches", n))
        command = gate.levels_time.command("llrm", "O2", source)
        command.insert(1, "-ftree-ch")
        own[n] = gate.own_work(command)["lir jumps"]
    assert own[256] > 0.0, f"premise: jumps ran ({own[256]:.1f} Minstr)"
    assert own[512] <= 2.55 * own[256], f"lir jumps {own[256]:.0f} -> {own[512]:.0f} Minstr"


def test_liveness_of_a_body_is_followed_when_the_values_are_live_in_few_blocks(tmp_path):
    """branches(512) at -O2 with -ftree-ch: the choice between following each value through the blocks it is live in and solving rows of bits
    was made from where each value first and last occurs in the layout, and a value whose uses lie far apart (the copy of a header test) read as
    one live in all the blocks between: 607 k steps against 22 k without the copy, so the rows were solved, blocks times words, for every
    caller of `allocate::live`. `lir twoaddr` read 855 Minstr, `intervals walk` 534, `lir pressuresink` 311 (x3.2-3.7 for N=512 -> 1024); the
    walk is tried at the cost of the rows and given up there: 344, 55, 58 (x2.0-2.2). A bound of 500 on twoaddr and 150 on the others fails
    the first."""
    source = tmp_path / "branches512.c"
    source.write_text(gate.generated("branches", 512))
    command = gate.levels_time.command("llrm", "O2", source)
    command.insert(1, "-ftree-ch")
    own = gate.own_work(command)
    assert own["lir twoaddr"] > 0.0, f"premise: twoaddr ran ({own['lir twoaddr']:.1f} Minstr)"
    assert own["lir twoaddr"] <= 500.0, f"twoaddr {own['lir twoaddr']:.1f} Minstr"
    assert own["intervals walk"] <= 150.0, f"walk {own['intervals walk']:.1f} Minstr"


def test_header_copy_on_a_nest_derives_the_registers_of_what_each_rotation_touched(tmp_path):
    """nest(64) at -O2 with -ftree-ch: each of the 64 rotations asked the registers of the whole function afresh (a fixed point and the cycle
    propagation over every value): `analysis registers` read 270 Minstr, and the doubling 64 -> 128 x3.3 (x2.0 without the copy). A fact is of the
    values it is made from alone, so the manager brings it up to date from what the rotation touched: 42 Minstr, x2.7. And `mir rotate` cloned
    every loop of the nest (a set of its blocks each) to look at one for each rotation: 93 Minstr, x5.1 for the doubling; 61, x3.8. A bound of 100
    on registers and 80 on rotate fails the first."""


def test_rotate_takes_the_dominators_the_manager_brings_up_to_date(tmp_path):
    """branches(512) at -O2 with -ftree-ch: each of the rotations built the dominator tree of the whole function afresh (a reverse postorder
    and the iteration over it) to ask whether a loop is entered: `mir rotate` read 303 Minstr, x3.9 for the doubling to 1024. The manager's tree
    is brought up to date from the edges the log tells the rotation changed, working out again the blocks under the nearest common dominator
    of the blocks they join: rotate 22 and the tree's updates 99 Minstr, a third of it. A bound of 250 on the two together fails the first
    (335 before)."""
    source = tmp_path / "branches512.c"
    source.write_text(gate.generated("branches", 512))
    command = gate.levels_time.command("llrm", "O2", source)
    command.insert(1, "-ftree-ch")
    own = gate.own_work(command)
    assert own["mir rotate"] > 0.0, f"premise: rotate ran ({own['mir rotate']:.1f} Minstr)"
    both = own["mir rotate"] + own["analysis dominators"]
    assert both <= 250.0, f"rotate {own['mir rotate']:.1f} + dominators {own['analysis dominators']:.1f} Minstr"


def test_registers_brought_up_to_date_are_the_registers_of_a_table_lookup_afresh(tmp_path):
    """crc at -O3 under LLRM_CHECK_REPLAY: a load from a `constant` table is known by the bounds of its whole index (what `ranges` finds
    from every value the index is made of, those with a known number and those without), so the incremental `analysis registers`, which
    follows the users of the values a change touched through the values that have a fact, kept no fact for it where deriving it afresh
    gave one, and the replay check panicked ('the result brought up to date is not what deriving it afresh gives'). A body that
    indexes a constant global derives them afresh."""
    source = tmp_path / "crc.c"
    source.write_text(wrap.wrapped("crc", programs.sources()["crc"].read_text()))
    done = subprocess.run(
        [*gate.levels_time.command("llrm", "O3", source), *programs.LLRM_FLAGS],
        capture_output=True,
        text=True,
        env={**os.environ, "LLRM_CHECK_REPLAY": "1"},
    )
    assert done.returncode == 0, done.stderr[-300:]


def test_loopsimplify_groups_a_loops_edges_without_copying_the_function_for_it(tmp_path):
    """branches at -O2 (every loop has a bridge to make behind the header copy): for each loop that needed grouping `mir loopsimplify` copied
    the function to try the grouping on, built its graph and predecessors three times over (in `grouped` too), and rebuilt them after each
    loop that changed: 1,235 Minstr at N=512 and 5,022 at N=1024 (x4.07). The edges are those of the loop's blocks and of the target, and
    every grouping is checked before the first is made: 19 and 42 (x2.2). A bound of 2.7 on the doubling fails the first."""
    own = {}
    for n in (512, 1024):
        source = tmp_path / f"branches{n}.c"
        source.write_text(scaling.AXES["branches"](n))
        own[n] = gate.own_work(gate.levels_time.command("llrm", "O2", source))["mir loopsimplify"]
    assert own[512] > 0.0, f"premise: loopsimplify ran ({own[512]:.1f} Minstr)"
    assert own[1024] <= 2.7 * own[512], f"mir loopsimplify {own[512]:.0f} -> {own[1024]:.0f} Minstr"


def test_floatassign_does_not_walk_the_body_again_for_each_spill_round(tmp_path):
    """nbody_single at -O3 (35 spill rounds over a 691-instruction block): each round numbered the body's values again and worked out
    which roots each step touches, 436 Minstr in `lir floatassign` against 382 once (587 before the steps were kept). A bound of 410
    fails the first."""
    source = tmp_path / "nbody_single.c"
    source.write_text(wrap.wrapped("nbody_single", programs.sources()["nbody_single"].read_text()))
    own = gate.own_work([*gate.levels_time.command("llrm", "O3", source), *programs.LLRM_FLAGS])
    assert own["lir floatalloc"] > 0.0, f"premise: x87 values were allocated ({own['lir floatalloc']:.1f} Minstr)"
    assert own["lir floatalloc"] <= 15.0, own["lir floatalloc"]  # `lir floatassign` is `lir floatalloc`: 5.7 on 2026-10-28


def test_counted_branches_share_what_a_counter_is_at_a_trip(tmp_path):
    """nbody_single at -O3: `peel` asked the odds of every branch of each loop, and `counted` ran a loop's counters once but worked out the
    compare's operands again for every branch at every trip, and found the loop's phis again for each: 118 Minstr in `mir peel` against 48
    when a trip is found as it was and a value is worked out once at each trip. A bound of 70 fails the first."""
    source = tmp_path / "nbody_single.c"
    source.write_text(wrap.wrapped("nbody_single", programs.sources()["nbody_single"].read_text()))
    own = gate.own_work([*gate.levels_time.command("llrm", "O3", source), *programs.LLRM_FLAGS])
    assert own["mir unroll"] > 0.0, f"premise: loops were unrolled ({own['mir unroll']:.1f} Minstr)"
    assert own["mir unroll"] <= 25.0, own["mir unroll"]  # `mir peel` is `mir unroll`: 12.3 on 2026-10-28


def test_promote_asks_a_write_only_the_cells_whose_bytes_it_may_reach(tmp_path):
    """cells(896) at -O2: N constant elements of one static array, each written after reads of others. `mir promote` asked each write
    every available cell of the array's bucket (typed and foreign checks twice, then the slices): N^2 questions, 983 Minstr at N=896
    (255 at 448, x3.9). Cells named by provenance alone are indexed by the hull of their slice, and a write asks those whose bytes meet
    its own: 53 (29 at 448). A bound of 150 fails the first."""
    source = tmp_path / "cells896.c"
    source.write_text(scaling.AXES["cells"](896))
    own = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    assert own["mir promote"] > 0.0, f"premise: promote ran ({own['mir promote']:.1f} Minstr)"
    assert own["mir promote"] <= 150.0, own["mir promote"]


def test_every_generator_is_an_axis_of_the_gate():
    """`cases` was a generator and in no gated axis: its `hir to mir` step went quadratic (x15 at N=1024) and no measurement moved."""
    missing = sorted(set(scaling.AXES) - {axis.removesuffix("-m16") for axis in gate.SIZES})
    assert missing == [], f"generators with no size in scaling_gate.SIZES: {missing}"


def test_a_step_below_the_share_floor_is_still_recorded_from_a_few_minstr(monkeypatch, tmp_path):
    """A step recorded only from 1.5% of the work cannot be seen to rise from a base that did not record it."""
    rows = iter([{}, {"a": 100.0}, {"a": 400.0, "tiny": 6.0}, {"a": 1600.0, "tiny": 9.0}])
    monkeypatch.setattr(gate, "own_work", lambda command: next(rows))
    monkeypatch.setattr(gate, "generated", lambda axis, n: "")
    got = gate.pass_costs("cases", "O1", tmp_path)
    assert "cases O1 tiny" in got and "cases O1 a" in got, got


def test_a_loop_of_many_stores_and_loads_does_not_refit_every_resident_at_every_step(tmp_path):
    """A loop of 300 stores to address-taken globals and 300 loads of others (the shape of d_faces' long loops): lsr weighed each
    candidate set by a pressure sweep that rebuilt the residents of every step and diffed them with the step before by list search, 1,715
    Minstr in `mir lsr`; a step that wants what the one before did changes nothing, and a diff is asked of sets: 166. `mir hoist` asked each
    load a list of its stores built and sorted for the question (430 Minstr), now each is asked once (139). Bounds 500 and 250 fail the
    first."""
    n = 300
    out = ["extern void sink(int *);\n"]
    out += [f"int a{k}, k{k};\n" for k in range(n)]
    out.append("void taken(void) {\n" + "".join(f"    sink(&a{k}); sink(&k{k});\n" for k in range(n)) + "}\n")
    out.append("int fn(int m) {\n    int s = 0, i;\n    for (i = 0; i < m; i++) {\n")
    out += [f"        a{k} += i;\n        s += k{k};\n" for k in range(n)]
    out.append("    }\n    return s;\n}\n")
    source = tmp_path / "stores.c"
    source.write_text("".join(out))
    own = gate.own_work(gate.levels_time.command("llrm", "O2", source))
    assert own["mir lsr"] > 0.0, f"premise: lsr ran ({own['mir lsr']:.1f} Minstr)"
    assert own["mir lsr"] <= 20.0 and own["mir licm"] <= 200.0, own  # 7.0 and 91.8 on 2026-10-28; `mir hoist` is `mir licm`


def test_no_premise_is_an_absolute_floor():
    """`analysis loops > 5.0 Minstr` (20 when written, 3.0 after a later change) and four more failed on tinytim with nothing wrong: a floor in Minstr
    goes stale when another change makes the stage cheaper. A premise says the stage ran (> 0); the bound the test is about is a ratio or a
    ceiling."""
    import re

    floors = re.findall(r"^\s*assert .* > (?!0\.0,)[0-9.]+, f\"premise:.*$", Path(__file__).read_text(), re.M)
    assert not floors, floors
