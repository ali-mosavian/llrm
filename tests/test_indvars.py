"""HARR should not advance a counter used only to test the loop bound."""

from pathlib import Path

import pytest

import corpus
from qbopt import wholeseg
from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.model import ir, mir
from qbopt.objectfile.module import Space
from qbopt.model.passes import Options


def _symbolic_control_body(candidate_start: int) -> mir.MirBody:
    """One dynamic counted loop with a second affine recurrence.

    The second value is an address-like offset: it is eligible to replace
    control only when its update reaches zero on the final trip.
    """
    bound = mir.Value(1, 0, variable=1, version=1)
    control_seed = mir.Value(2, 0, variable=2, version=1)
    candidate_seed = mir.Value(3, 0, variable=3, version=1)
    control = mir.Value(4, 1, variable=2, version=2)
    candidate = mir.Value(5, 1, variable=3, version=2)
    flags = mir.Value(6, 1, flags=True, variable=4, version=1)
    control_next = mir.Value(7, 2, variable=2, version=3)
    candidate_next = mir.Value(8, 2, variable=3, version=3)
    offset = mir.Value(9, 2, variable=5, version=1)
    source = mir.MemRef(None, 2, space=Space.FRAME)

    def copy(at: int, result: mir.Value, number: int) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.NOTHING,
            "",
            (result,),
            (),
            kind=mir.Kind.COPY,
            args=(mir.Const(number, 2),),
            results=(mir.Held(result, 2),),
        )

    def add(at: int, result: mir.Value, left: mir.Value, right: int) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.NOTHING,
            "",
            (result,),
            (left,),
            kind=mir.Kind.ADD,
            args=(mir.Held(left, 2), mir.Const(right, 2)),
            results=(mir.Held(result, 2),),
        )

    load = mir.Op(
        0,
        ir.Operation.MOVE,
        "",
        (bound,),
        (),
        loads=(source,),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(source),),
        results=(mir.Held(bound, 2),),
    )
    compare = mir.Op(
        1,
        ir.Operation.COMPARE,
        "cmp",
        (flags,),
        (control, bound),
        kind=mir.Kind.SUB,
        args=(mir.Held(control, 2), mir.Held(bound, 2)),
    )
    branch = mir.Op(
        1,
        ir.Operation.BRANCH,
        "",
        (),
        (flags,),
        kind=mir.Kind.BRANCH,
        test=mir.Kind.ABOVE_EQ,
        target=3,
    )
    jump = mir.Op(2, ir.Operation.JUMP, "", (), (), kind=mir.Kind.JUMP, target=1)
    returned = mir.Op(3, ir.Operation.RETURN, "", (), (), kind=mir.Kind.RETURN)
    return mir.MirBody(
        0,
        (
            mir.MirBlock(
                0,
                (),
                (load, copy(0, control_seed, 0), copy(0, candidate_seed, candidate_start)),
                (1,),
            ),
            mir.MirBlock(
                1,
                (
                    mir.Phi(control, {0: control_seed, 2: control_next}),
                    mir.Phi(candidate, {0: candidate_seed, 2: candidate_next}),
                ),
                (compare, branch),
                (2, 3),
            ),
            mir.MirBlock(
                2,
                (),
                (
                    add(2, offset, candidate, 100),
                    add(2, control_next, control, 1),
                    add(2, candidate_next, candidate, 1),
                    jump,
                ),
                (1,),
            ),
            mir.MirBlock(3, (), (returned,), ()),
        ),
        integer_ranges={bound: mir.IntegerRange(0, 7, 2)},
    )


def test_symbolic_control_rebases_a_nonzero_start_recurrence() -> None:
    """A recurrence seeded at 5 is rebased by its final value, so its last update is still zero."""
    from qbopt.analysis import induction
    from qbopt.optimize import indvars

    body = _symbolic_control_body(5)
    (loop,) = loops.loops(body.blocks, body.entry)
    (proof,) = induction.counted(body, loop)
    candidate = next(one for one in induction.basics(body, loop).values() if one != proof.counter)

    assert induction.zero_terminating_control(body, loop, proof, candidate) is not None
    assert indvars.symbolically_zeroed(body) is not body


def test_symbolic_control_proves_a_zero_terminal_recurrence() -> None:
    """The same bounded recurrence is usable when its final update is zero."""
    from qbopt.analysis import induction

    body = _symbolic_control_body(0)
    (loop,) = loops.loops(body.blocks, body.entry)
    (proof,) = induction.counted(body, loop)
    candidate = next(one for one in induction.basics(body, loop).values() if one != proof.counter)

    got = induction.zero_terminating_control(body, loop, proof, candidate)

    assert got is not None
    assert got.replacement.counted is proof
    assert (got.candidate, got.step, got.maximum, got.period) == (candidate, 1, 7, 65536)


@pytest.mark.parametrize("tag", ["p-g2", "q-O"])
def test_invariant_branch_load_moves_out_but_its_test_stays(tag, monkeypatch):
    """IVWORD reloaded unchanged branchChoice every trip because its test prevented LICM."""
    from qbopt.optimize import unswitch

    monkeypatch.setattr(unswitch, "optimized", lambda body, *args, **kwargs: body)
    result = wholeseg.emitted(Path(f"fixtures/regressions/ivword-{tag}.obj".lower()).read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    blocks = corpus.partitioned(result.data)
    inside = set().union(*(loop.body for loop in loops.loops(blocks)))
    assert inside
    instructions = [str(one.insn) for block in blocks if block.at in inside for one in block.insns]
    assert not any(one.startswith("mov ") and "[" in one.split(",", 1)[-1] for one in instructions)
    assert any(one.startswith(("and ", "test ", "or ")) for one in instructions)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program", ["ivarm", "ivword"])
def test_internal_branch_reuses_the_value_recurrence(tag, program, monkeypatch):
    """IVARM kept a second counter solely for ten trips around a conditional store."""
    from qbopt.optimize import unswitch

    monkeypatch.setattr(unswitch, "optimized", lambda body, *args, **kwargs: body)
    result = wholeseg.emitted(Path(f"fixtures/regressions/{program}-{tag}.obj".lower()).read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert not any(one.startswith("inc ") for one in instructions)
    comparisons = [index for index, one in enumerate(instructions) if one.startswith("cmp ") and one.endswith(",25h")]
    assert len(comparisons) == 1
    assert instructions[comparisons[0] + 1].startswith("jne ")


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program,loop_count,increments", [("harr", 1, 1), ("matrix", 2, 1), ("nested", 2, 0)])
def test_harr_reuses_an_existing_recurrence_for_termination(
    tag: str, program: str, loop_count: int, increments: int
) -> None:
    """HARR has one recurrence per retained loop, or is completely unrolled."""
    result = wholeseg.emitted(Path(f"fixtures/omf/{program}-{tag}.obj".lower()).read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    blocks = corpus.partitioned(result.data)
    instructions = [str(one.insn) for block in blocks for one in block.insns]
    retained = loops.loops(blocks)
    if not retained:
        # All bounds and body values are constants in HARR, so complete
        # unrolling is stronger than retaining the old recurrence.  Do not
        # require a particular conditional-branch spelling from a future
        # lowering pass.
        assert not any(one.startswith("inc ") for one in instructions)
        return
    assert len(retained) == loop_count
    assert sum(one.startswith("inc ") for one in instructions) == increments


def test_harr_initializes_the_reused_counter_before_its_exit_bound():
    """HARR printed 12327 instead of 1100 when the bound read SI before SI was initialized."""
    from iced_x86 import OpKind
    from iced_x86 import Mnemonic

    result = wholeseg.emitted(Path("fixtures/omf/harr-v-g3.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    branch_at, branch = next(
        (index, one)
        for index, one in enumerate(instructions)
        if one.mnemonic == Mnemonic.JNE and one.near_branch_target < one.ip
    )
    compare_at = branch_at - 1
    compare = instructions[compare_at]
    assert compare.mnemonic == Mnemonic.CMP
    wanted = {compare.op0_register, compare.op1_register}
    containing = [
        one.near_branch_target
        for one in instructions[branch_at + 1 :]
        if one.near_branch_target and one.near_branch_target <= branch.near_branch_target < one.ip
    ]
    start_ip = min((branch.near_branch_target, *containing))
    start = next(index for index, one in enumerate(instructions) if one.ip == start_ip)
    defined = set()
    for one in instructions[start:compare_at]:
        if one.op0_kind != OpKind.REGISTER or one.op0_register not in wanted:
            continue
        reads = set()
        if one.mnemonic == Mnemonic.MOV and one.op1_kind == OpKind.REGISTER:
            reads.add(one.op1_register)
        elif one.mnemonic in (Mnemonic.ADD, Mnemonic.SUB, Mnemonic.INC, Mnemonic.DEC):
            reads.add(one.op0_register)
            if one.op1_kind == OpKind.REGISTER:
                reads.add(one.op1_register)
        assert not (reads & wanted) - defined, f"{one} reads the loop bound before it is initialized"
        defined.add(one.op0_register)
    assert defined == wanted


def test_indvar_simplify_reads_through_an_lcssa_exit(monkeypatch) -> None:
    """LCSSA made HARR's redundant loop counter look externally observed."""
    from qbopt.model import mir
    from qbopt.optimize import lcssa
    from qbopt.optimize import indvars
    from qbopt.optimize import transform

    path = Path("fixtures/omf/harr-v-g3.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    with monkeypatch.context() as context:
        context.setattr(indvars, "simplified", lambda body: body)
        body = transform.applied(
            mir.bodies(found, partition)[0][1],
            found.dgroup,
            found.calls,
            blocks=partition,
            found=found,
            options=Options(lcssa=False),
        )
    closed = lcssa.closed(body)

    assert closed != body
    assert indvars.simplified(closed) != closed


@pytest.mark.parametrize("hazard", ["observed-counter", "zero-trip", "wrapping-exit", "short-period"])
def test_counter_elimination_requires_a_complete_trip_count_and_no_body_use(monkeypatch, hazard):
    from dataclasses import replace

    from qbopt.model import mir
    from qbopt.analysis import loops
    from qbopt.analysis import consts
    from qbopt.optimize import indvars
    from qbopt.analysis import induction
    from qbopt.optimize import transform

    path = Path("fixtures/omf/harr-v-g3.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    with monkeypatch.context() as context:
        # Counting to zero rewrites the very compare each hazard edits.
        context.setattr(indvars, "simplified", lambda body: body)
        context.setattr(indvars, "zeroed", lambda body, *args, **kwargs: body)
        body = transform.applied(
            mir.bodies(found, partition)[0][1], found.dgroup, found.calls, blocks=partition, found=found
        )
    loop = next(loop for loop in loops.loops(body.blocks, body.entry) if len(loop.body) == 2)
    facts = consts.known(body)
    counter = next(
        counter
        for counter in induction.basics(body, loop).values()
        if (proof := induction.controlling(body, loop, counter, facts)) is not None and proof.last is not None
    )
    header = next(block for block in body.blocks if block.at == loop.header)
    value = next(phi.result for phi in header.phis if phi.result.id == counter.value)
    alternative_updates = {
        phi.incoming[next(iter(loop.latches))] for phi in header.phis if phi.result.id != counter.value
    }
    changed = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            if hazard == "observed-counter" and block.at in loop.body and op.stores:
                op = replace(op, args=(mir.Held(value, 2),), uses=(*op.uses, value))
            elif hazard == "short-period" and alternative_updates.intersection(op.defines) and len(op.args) == 2:
                op = replace(op, args=(op.args[0], mir.Const(32768, 2)))
            elif hazard in ("zero-trip", "wrapping-exit") and block.at == header.at and op.kind is mir.Kind.SUB:
                op = replace(op, args=(op.args[0], mir.Const(0 if hazard == "zero-trip" else 32767, 2)))
            ops.append(op)
        changed.append(replace(block, ops=tuple(ops)))
    body = replace(body, blocks=tuple(changed))
    assert indvars.simplified(body) is body


@pytest.mark.parametrize("hazard", [None, "different-map", "ordered", "short-period"])
def test_scaled_recurrences_replace_nested_counter_equality(hazard: str | None) -> None:
    """C nbody carried `other` beside `other*4` only for `other != body`.

    The inner and outer byte offsets are injective over their proven 0..5
    domains, so they can answer both equality and loop termination.  Keeping
    the scalar inner counter added an increment, compare and spill traffic to
    every interaction.
    """
    from qbopt.optimize import indvars

    def value(serial: int, at: int, variable: int) -> mir.Value:
        return mir.Value(serial, at, variable=variable)

    def copy(at: int, result: mir.Value, number: int) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.NOTHING,
            "",
            (result,),
            (),
            kind=mir.Kind.COPY,
            args=(mir.Const(number, 2),),
            results=(mir.Held(result, 2),),
        )

    def add(at: int, result: mir.Value, source: mir.Value, amount: int) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.NOTHING,
            "",
            (result,),
            (source,),
            kind=mir.Kind.ADD,
            args=(mir.Held(source, 2), mir.Const(amount, 2)),
            results=(mir.Held(result, 2),),
        )

    def compare(at: int, left: mir.Value, right: mir.Arg) -> tuple[mir.Op, mir.Value]:
        flag = value(100 + at, at, 100 + at)
        flag = mir.Value(flag.id, flag.at, flags=True, variable=flag.variable)
        operand = right if isinstance(right, (mir.Held, mir.Const)) else mir.Held(right, 2)
        uses = (left, operand.value) if isinstance(operand, mir.Held) else (left,)
        return (
            mir.Op(
                at,
                ir.Operation.NOTHING,
                "",
                (flag,),
                uses,
                kind=mir.Kind.SUB,
                args=(mir.Held(left, 2), operand),
                results=(),
            ),
            flag,
        )

    def branch(at: int, flag: mir.Value, test: mir.Kind, target: int) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.NOTHING,
            "",
            (),
            (flag,),
            kind=mir.Kind.BRANCH,
            test=test,
            target=target,
        )

    outer_start, outer_offset_start = value(1, 0, 1), value(2, 0, 2)
    outer, outer_offset = value(3, 1, 1), value(4, 1, 2)
    outer_next, outer_offset_next = value(5, 6, 1), value(6, 6, 2)
    inner_start, inner_offset_start = value(7, 2, 3), value(8, 2, 4)
    inner, inner_offset = value(9, 3, 3), value(10, 3, 4)
    inner_next, inner_offset_next = value(11, 5, 3), value(12, 5, 4)
    outer_test, outer_flag = compare(1, outer, mir.Const(6, 2))
    inner_test, inner_flag = compare(3, inner, mir.Const(6, 2))
    unequal, unequal_flag = compare(4, inner, outer)
    observed = value(20, 7, 20)
    use_offset = mir.Op(
        7,
        ir.Operation.NOTHING,
        "",
        (observed,),
        (inner_offset,),
        kind=mir.Kind.COPY,
        args=(mir.Held(inner_offset, 2),),
        results=(mir.Held(observed, 2),),
    )
    built = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (copy(0, outer_start, 0), copy(0, outer_offset_start, 0)), (1,)),
            mir.MirBlock(
                1,
                (
                    mir.Phi(outer, {0: outer_start, 6: outer_next}),
                    mir.Phi(outer_offset, {0: outer_offset_start, 6: outer_offset_next}),
                ),
                (outer_test, branch(1, outer_flag, mir.Kind.GE, 9)),
                (2, 9),
            ),
            mir.MirBlock(2, (), (copy(2, inner_start, 0), copy(2, inner_offset_start, 0)), (3,)),
            mir.MirBlock(
                3,
                (
                    mir.Phi(inner, {2: inner_start, 5: inner_next}),
                    mir.Phi(inner_offset, {2: inner_offset_start, 5: inner_offset_next}),
                ),
                (inner_test, branch(3, inner_flag, mir.Kind.GE, 6)),
                (4, 6),
            ),
            mir.MirBlock(
                4,
                (),
                (unequal, branch(4, unequal_flag, mir.Kind.LT if hazard == "ordered" else mir.Kind.EQ, 5)),
                (5, 7),
            ),
            mir.MirBlock(
                5,
                (),
                (
                    add(5, inner_next, inner, 1),
                    add(5, inner_offset_next, inner_offset, 32768 if hazard == "short-period" else 4),
                ),
                (3,),
            ),
            mir.MirBlock(
                6,
                (),
                (
                    add(6, outer_next, outer, 1),
                    add(
                        6,
                        outer_offset_next,
                        outer_offset,
                        5 if hazard == "different-map" else 32768 if hazard == "short-period" else 4,
                    ),
                ),
                (1,),
            ),
            mir.MirBlock(7, (), (use_offset,), (5,)),
            mir.MirBlock(9, (), (), ()),
        ),
    )

    changed = indvars.simplified(built)
    header = next(block for block in changed.blocks if block.at == 3)
    condition = next(op for op in header.ops if op.kind is mir.Kind.SUB)
    inner_branch = next(block for block in changed.blocks if block.at == 4)
    equality = next(op for op in inner_branch.ops if op.kind is mir.Kind.SUB)

    if hazard is None:
        assert condition.args[0] == mir.Held(inner_offset, 2)
        assert equality.args == (mir.Held(inner_offset, 2), mir.Held(outer_offset, 2))
    else:
        assert condition.args[0] == mir.Held(inner, 2)
        assert equality.args == (mir.Held(inner, 2), mir.Held(outer, 2))


@pytest.mark.parametrize(("coordinate_width", "stride", "reused"), [(4, 24, True), (1, 64, False)])
def test_cross_width_recurrence_replaces_counter_only_for_its_full_period(
    coordinate_width: int, stride: int, reused: bool
) -> None:
    """C Mandelbrot kept 16-bit ``px``/``py`` counters beside the 32-bit
    ``cx``/``cy`` recurrences that already advance once per iteration.

    Their two update stores survive allocation. Loop termination may use a
    wider recurrence when its modular period proves that the computed final
    value cannot occur on an earlier iteration. The first implementation lost
    the positive trip-count proof and left Mandel with six branches and a
    redundant entry test; a symbolic sentinel must retain that proof.
    """
    from qbopt.optimize import indvars

    coordinate_argument = mir.Value(0, 0, variable=0)
    control_start = mir.Value(1, 0, variable=1)
    coordinate_start = mir.Value(2, 0, variable=2)
    control = mir.Value(3, 1, variable=1)
    coordinate = mir.Value(4, 1, variable=2)
    control_next = mir.Value(5, 2, variable=1)
    coordinate_next = mir.Value(6, 2, variable=2)
    observed = mir.Value(7, 2, variable=3)
    flags = mir.Value(8, 1, flags=True, variable=4)

    def copy(at: int, result: mir.Value, value: int, width: int) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.NOTHING,
            "",
            (result,),
            (),
            kind=mir.Kind.COPY,
            args=(mir.Const(value, width),),
            results=(mir.Held(result, width),),
        )

    def copy_argument(at: int, result: mir.Value, value: mir.Value, width: int) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.NOTHING,
            "",
            (result,),
            (value,),
            kind=mir.Kind.COPY,
            args=(mir.Held(value, width),),
            results=(mir.Held(result, width),),
        )

    def add(at: int, result: mir.Value, source: mir.Value, value: int, width: int) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.NOTHING,
            "",
            (result,),
            (source,),
            kind=mir.Kind.ADD,
            args=(mir.Held(source, width), mir.Const(value, width)),
            results=(mir.Held(result, width),),
        )

    compare = mir.Op(
        1,
        ir.Operation.NOTHING,
        "",
        (flags,),
        (control,),
        kind=mir.Kind.SUB,
        args=(mir.Held(control, 2), mir.Const(4, 2)),
        results=(),
    )
    branch = mir.Op(
        1,
        ir.Operation.NOTHING,
        "",
        (),
        (flags,),
        kind=mir.Kind.BRANCH,
        test=mir.Kind.GE,
        target=9,
    )
    use = mir.Op(
        2,
        ir.Operation.NOTHING,
        "",
        (observed,),
        (coordinate,),
        kind=mir.Kind.COPY,
        args=(mir.Held(coordinate, coordinate_width),),
        results=(mir.Held(observed, coordinate_width),),
    )
    jump = mir.Op(2, ir.Operation.JUMP, "", (), (), kind=mir.Kind.JUMP, target=1)
    returned = mir.Op(9, ir.Operation.RETURN, "", (), (), kind=mir.Kind.RETURN)
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(
                0,
                (),
                (
                    copy(0, control_start, 0, 2),
                    copy_argument(0, coordinate_start, coordinate_argument, coordinate_width),
                ),
                (1,),
            ),
            mir.MirBlock(
                1,
                (
                    mir.Phi(control, {0: control_start, 2: control_next}),
                    mir.Phi(coordinate, {0: coordinate_start, 2: coordinate_next}),
                ),
                (compare, branch),
                (2, 9),
            ),
            mir.MirBlock(
                2,
                (),
                (
                    use,
                    add(2, control_next, control, 1, 2),
                    add(2, coordinate_next, coordinate, stride, coordinate_width),
                    jump,
                ),
                (1,),
            ),
            mir.MirBlock(9, (), (returned,), ()),
        ),
    )

    changed = indvars.simplified(body)
    condition = next(op for op in changed.blocks[1].ops if op.kind is mir.Kind.SUB)

    if reused:
        assert condition.args[0] == mir.Held(coordinate, coordinate_width)
        assert isinstance(condition.args[1], mir.Held) and condition.args[1].width == coordinate_width
        from qbopt.analysis import loops
        from qbopt.analysis import consts
        from qbopt.optimize import rotate
        from qbopt.analysis import induction

        loop = loops.loops(changed.blocks, changed.entry)[0]
        assert induction.trip_count(changed, loop, consts.known(changed)) == 4
        assert induction.nonempty(changed, loop)
        assert rotate.rotated(changed).block(0).succ == (2,)
    else:
        assert condition.args[0] == mir.Held(control, 2)


def test_c_mandel_reuses_coordinate_recurrences_for_both_outer_loops() -> None:
    """C Mandelbrot recomputed ``24 * px`` and ``24 * py`` in memory.

    The selector proved both affine coordinate formulas profitable even with
    no register headroom, then the rewrite's register-only gate silently
    discarded them.  That left two hot ``shl/add/shl`` chains targeting frame
    slots.  Keep the coordinates as ``+24`` recurrences whether allocation
    gives them registers or spill slots.  The narrower ``px``/``py`` counters
    themselves remain redundant, so only the required iteration increment is
    present too.
    """
    from tools import quality
    from qbopt.cfront import compile as cfront

    source = Path("bench/c/mandel.c")
    module = cfront.assembled(cfront.recorded(source, []), source.stem, optimise=True)
    procedure = next(one for one in module.procedures if one.name == "_bench_mandel")
    assert dict(procedure.body.loop_trip_counts) == {8: 24, 14: 32}
    rows = quality._rows(quality._blob(module, procedure, 0))

    def immediate(operands: str) -> int | None:
        text = operands.rsplit(",", 1)[-1].strip()
        try:
            return int(text[:-1], 16) if text.endswith("h") else int(text)
        except ValueError:
            return None

    unit_steps = [
        (mnemonic, operands)
        for _raw, mnemonic, operands in rows
        if mnemonic == "inc" or mnemonic == "add" and immediate(operands) == 1
    ]
    coordinate_steps = [
        (mnemonic, operands) for _raw, mnemonic, operands in rows if mnemonic == "add" and immediate(operands) == 24
    ]
    frame_shifts = [
        (mnemonic, operands)
        for _raw, mnemonic, operands in rows
        if mnemonic == "shl" and operands.lstrip().startswith("dword ptr [bp-")
    ]
    copied_commutative_results = []
    widened_high_extracts = []
    for first, second in zip(rows, rows[1:], strict=False):
        _raw, mnemonic, operands = first
        _next_raw, next_mnemonic, next_operands = second
        first_operands = [operand.strip() for operand in operands.split(",")]
        copied = [operand.strip() for operand in next_operands.split(",")]
        if (
            mnemonic == "imul"
            and len(first_operands) == 2
            and next_mnemonic == "mov"
            and len(copied) == 2
            and copied == [first_operands[1], first_operands[0]]
        ):
            copied_commutative_results.append((first, second))
        shifted = [operand.strip() for operand in next_operands.split(",")]
        if (
            mnemonic == "mov"
            and len(first_operands) == 2
            and first_operands[0] in {"eax", "ebx", "ecx", "edx", "esi", "edi"}
            and "[" in first_operands[1]
            and next_mnemonic == "shr"
            and len(shifted) == 2
            and shifted[0] == first_operands[0]
            and immediate(next_operands) == 16
        ):
            widened_high_extracts.append((first, second))

    assert len(unit_steps) == 1, unit_steps
    assert "[" not in unit_steps[0][1]
    assert len(coordinate_steps) == 2, coordinate_steps
    assert not frame_shifts, frame_shifts
    assert not copied_commutative_results, copied_commutative_results
    assert not widened_high_extracts, widened_high_extracts


def _trip_counts(data: bytes) -> list[int]:
    """How often each emitted counted loop runs: its counter's start, step and exit test, simulated."""
    from iced_x86 import OpKind
    from iced_x86 import Mnemonic

    insns = [one.insn for block in corpus.partitioned(data) for one in block.insns]
    immediates = (OpKind.IMMEDIATE8, OpKind.IMMEDIATE8TO16, OpKind.IMMEDIATE16)

    def initialized(register, before, seen=frozenset()):
        """The first-iteration constant in ``register``, following copies."""
        if register in seen:
            return None
        for index in range(len(before) - 1, -1, -1):
            one = before[index]
            if one.op0_kind != OpKind.REGISTER or one.op0_register != register:
                continue
            if one.mnemonic == Mnemonic.MOV and one.op1_kind in immediates:
                return (one.immediate16 ^ 0x8000) - 0x8000
            if one.mnemonic == Mnemonic.MOV and one.op1_kind == OpKind.REGISTER:
                return initialized(one.op1_register, before[:index], seen | {register})
            if one.mnemonic == Mnemonic.XOR and one.op1_kind == OpKind.REGISTER and one.op1_register == register:
                return 0
            return None
        return None

    counts = []
    for index, branch in enumerate(insns):
        if branch.mnemonic not in (Mnemonic.JLE, Mnemonic.JL, Mnemonic.JNE) or branch.near_branch_target >= branch.ip:
            continue
        inside = [one for one in insns[:index] if one.ip >= branch.near_branch_target]
        steps = [
            one for one in inside if one.mnemonic in (Mnemonic.INC, Mnemonic.DEC) and one.op0_kind == OpKind.REGISTER
        ]
        if not steps:
            continue
        step = steps[-1]
        register, delta = step.op0_register, 1 if step.mnemonic == Mnemonic.INC else -1
        # The counter can move between registers inside the loop: `mov dx,cx / inc dx / mov cx,dx`.
        held = {register} | {
            one.op1_register
            for one in inside
            if one.mnemonic == Mnemonic.MOV and one.op1_kind == OpKind.REGISTER and one.op0_register == register
        }
        test = insns[index - 1]
        if test.mnemonic == Mnemonic.MOV and test.op0_kind == OpKind.REGISTER and test.op0_register in held:
            test = insns[index - 2]
        if (
            test.mnemonic == Mnemonic.CMP
            and test.op0_kind == OpKind.REGISTER
            and test.op0_register in held
            and test.op1_kind in immediates
        ):
            bound = (test.immediate16 ^ 0x8000) - 0x8000
        # `or r,r` tests for zero as `test r,r` does; the peephole writes it for `cmp r,0`.
        elif test is step or (
            test.mnemonic in (Mnemonic.TEST, Mnemonic.OR)
            and test.op0_register in held
            and test.op0_kind == test.op1_kind == OpKind.REGISTER
            and test.op0_register == test.op1_register
        ):
            bound = 0
        else:
            continue
        value = initialized(register, insns[: insns.index(step)])
        if value is None:
            continue
        trips = 0
        while trips < 1 << 17:
            trips += 1
            value = ((value + delta + 0x8000) & 0xFFFF) - 0x8000
            taken = {Mnemonic.JLE: value <= bound, Mnemonic.JL: value < bound, Mnemonic.JNE: value != bound}
            if not taken[branch.mnemonic]:
                break
        counts.append(trips)
    return sorted(counts)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program,trips", [("spill", [10]), ("segld", [5, 20])])
def test_counting_one_loop_to_zero_leaves_a_loop_sharing_its_start_alone(tag, program, trips):
    """SPILL printed T= 4620 and SEGLD T= 975: both loops of each nest start at 1, one
    constant, and counting one to zero rewrote that constant, so the other ran from -10
    (or -5) up to its own bound."""
    result = wholeseg.emitted(Path(f"fixtures/omf/{program}-{tag}.obj".lower()).read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    assert _trip_counts(result.data) == trips
