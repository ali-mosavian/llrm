"""FPDEEP's three printed iterations cannot be replaced by one final iteration."""

from pathlib import Path

import pytest

import corpus
from qbopt.model import mir
from qbopt.backend import cpu
from qbopt.backend import lower
from qbopt.analysis import loops
from qbopt.optimize import unroll
from qbopt.model.passes import Where
from qbopt.optimize import transform
from qbopt.analysis import floatfacts
from qbopt.backend import lower_floats
from qbopt.model.passes import OperationCosts


def test_unroll_rejects_growth_not_paid_for_by_dynamic_work(monkeypatch) -> None:
    """Five straight-line moves are not a win over a two-trip ADD/branch loop."""
    from qbopt.model import ir

    add = mir.Op(1, ir.Operation.BINARY, "add", (), (), kind=mir.Kind.ADD)
    branch = mir.Op(1, ir.Operation.BRANCH, "jne", (), (), kind=mir.Kind.BRANCH, target=1)
    original = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (), (1,)),
            mir.MirBlock(1, (), (add, branch), (1, 2)),
            mir.MirBlock(2, (), (), ()),
        ),
    )
    move = lambda at: mir.Op(at, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.COPY)
    candidate = mir.MirBody(
        0,
        (mir.MirBlock(0, (), tuple(move(at) for at in range(5)), (2,)), mir.MirBlock(2, (), (), ())),
        repetitions=((1, 2),),
    )
    monkeypatch.setattr(unroll, "expanded", lambda *_args, **_kwargs: candidate)
    where = Where(costs=OperationCosts(add=1, branch=2, move=1))
    stages = []

    assert (
        unroll.optimized(
            original,
            where,
            optimize=lambda body: body,
            watch=lambda stage, _body: stages.append(stage),
        )
        is original
    )
    assert "unroll-rejected-growth" in stages


def test_unroll_profitability_uses_the_selected_cpu() -> None:
    """A short branch-heavy expansion is worthwhile on 386 but not P5."""
    from qbopt.model import ir

    add = mir.Op(1, ir.Operation.BINARY, "add", (), (), kind=mir.Kind.ADD)
    branch = mir.Op(1, ir.Operation.BRANCH, "jne", (), (), kind=mir.Kind.BRANCH, target=1)
    original = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (), (1,)),
            mir.MirBlock(1, (), (add, branch), (1, 2)),
            mir.MirBlock(2, (), (), ()),
        ),
    )
    move = lambda at: mir.Op(at, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.COPY)
    result = mir.MirBody(
        0,
        (mir.MirBlock(0, (), tuple(move(at) for at in range(3)), (2,)), mir.MirBlock(2, (), (), ())),
        repetitions=((1, 2),),
    )

    assert unroll._profitable(original, result, 1, 2, Where(costs=cpu.profile("386").operations))
    assert not unroll._profitable(original, result, 1, 2, Where(costs=cpu.profile("P5").operations))


def test_peel_reports_the_gate_that_rejected_its_candidate(monkeypatch) -> None:
    """Matmul's rejected peel had no final event explaining why it lost."""
    from dataclasses import replace

    from qbopt.optimize import peel

    original = mir.MirBody(0, (mir.MirBlock(0, (), (), ()),))
    candidate = replace(original, cloned=True)

    def found(_body, _where, *, skip=frozenset()):
        return None if skip else (candidate, 7, 2)

    monkeypatch.setattr(peel, "_candidate", found)
    monkeypatch.setattr(unroll, "_rejection", lambda *_args: "residual-loops")
    stages = []

    assert (
        peel.optimized(
            original,
            Where(),
            optimize=lambda body: body,
            watch=lambda stage, _body: stages.append(stage),
        )
        is original
    )
    assert stages == ["peel-rejected-residual-loops"]


def test_peel_bounds_conditional_floating_clone_work(monkeypatch) -> None:
    """A large branchy floating peel made the C benchmark gate spend minutes
    rebuilding transient CFGs before it could execute an oracle.

    The ceiling is an analysis-resource bound, not a source-specific nbody
    heuristic: a small exact triangular clone remains eligible, while a
    high-trip conditional floating body must not enter the expensive fixed
    point merely to be rejected later on profitability.
    """
    from dataclasses import replace

    from qbopt.analysis import induction
    from qbopt.model import ir
    from qbopt.model.floating import Format
    from qbopt.model.floating import Precision
    from qbopt.model.floating import Rounding
    from qbopt.model.floating import Semantics
    from qbopt.optimize import loopclone
    from qbopt.optimize import peel
    from test_loopclone import diamond

    body, _ = diamond()
    work = body.block(3)
    floating = Semantics((Format.EXTENDED80,), Format.EXTENDED80, Precision.DYNAMIC, Rounding.DYNAMIC)
    body = replace(
        body,
        blocks=tuple(
            replace(block, ops=(replace(block.ops[0], op=ir.Operation.FLOAT_ARITH, floating=floating), *block.ops[1:]))
            if block is work
            else block
            for block in body.blocks
        ),
    )
    called = []
    monkeypatch.setattr(induction, "trip_count", lambda *_args: 513)
    monkeypatch.setattr(loopclone, "peeled", lambda *_args: called.append(True))

    assert peel._candidate(body, Where()) is None
    assert not called


def test_c_matmul_unrolls_exact_multiblock_loops() -> None:
    """Matmul retained eight DIVs after its fixed 8x8 initializer, then a stale bridge phi returned 4252537476."""
    from tools import quality
    from qbopt.cfront import compile as cfront

    source = Path("bench/c/matmul.c")
    stream = cfront.recorded(source, [])
    module = cfront.assembled(stream, source.stem, optimise=True)
    procedure = next(one for one in module.procedures if one.name == "_bench_matmul")
    rows = quality._rows(quality._blob(module, procedure, 0))
    dynamic, status = quality._dynamic_operations(module, procedure, 0)

    assert not procedure.body.inputs
    assert not any(mnemonic == "div" for _raw, mnemonic, _operands in rows)
    assert sum(mnemonic == "imul" for _raw, mnemonic, _operands in rows) < 16
    assert sum(mnemonic.startswith("j") for _raw, mnemonic, _operands in rows) < 9
    assert status.startswith("estimated:")
    assert dynamic is not None and dynamic < 6_000


def test_c_crc_unroll_keeps_the_inner_result_on_the_outer_backedge() -> None:
    """CRC returned ``salt ^ ~0`` after its eight-round inner loop was unrolled.

    The inner loop's final CRC value feeds a phi on the enclosing loop's
    backedge.  Dropping that edge use made the whole polynomial calculation
    dead, leaving four instructions that returned the initial complement.
    """
    from tools import quality
    from qbopt.cfront import compile as cfront

    source = Path("bench/c/crc.c")
    stream = cfront.recorded(source, [])
    module = cfront.assembled(stream, source.stem, optimise=True)
    procedure = next(one for one in module.procedures if one.name == "_bench_crc")
    rows = quality._rows(quality._blob(module, procedure, 0))

    assert sum(mnemonic == "shr" for _raw, mnemonic, _operands in rows) >= 8
    assert any("EDB88320" in operands.upper() for _raw, _mnemonic, operands in rows)


def test_c_nbody_peels_the_fixed_triangular_interaction_loop() -> None:
    """Nbody retained 33,632 estimated operations because ``j = i + 1`` hid its six fixed interactions."""
    from tools import quality
    from qbopt.cfront import compile as cfront

    source = Path("bench/c/nbody.c")
    module = cfront.assembled(cfront.recorded(source, []), source.stem, optimise=True)
    procedure = next(one for one in module.procedures if one.name == "_bench_nbody")
    dynamic, status = quality._dynamic_operations(module, procedure, 0)

    assert not procedure.body.inputs
    assert status.startswith("estimated:")
    assert dynamic is not None and dynamic < 20_000


@pytest.mark.parametrize("checkpoint", [False, True])
def test_dead_inserted_store_needs_no_neighbor_to_take_its_bytes(checkpoint):
    """FPCSE retained dead unrolled stores because a zero-byte clone could not donate bytes to its neighbor."""
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space
    cell = mir.MemRef(Addr(Space.SEGMENT, 0, 5), 4)
    def store(at, value):
        return mir.Op(at, ir.Operation.MOVE, "", (), (), kind=mir.Kind.STORE,
                      args=(mir.Const(value, 4),), results=(mir.Cell(cell),),
                      stores=(cell,))
    first, last = store(10, 1), store(20, 2)
    middle = (mir.Op(15, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.FCHECK),) if checkpoint else ()
    body = mir.MirBody(0, (mir.MirBlock(0, (), (first, *middle, last), ()),))
    changed = transform.without_dead_stores(body, frozenset({5}), {})
    assert changed.blocks[0].ops == ((first, *middle, last) if checkpoint else (last,))


def body():
    path = Path("fixtures/omf/fpdeep-p-g2.obj")
    found = corpus.loaded(path)
    original = mir.bodies(found, corpus.partitioned(path))[0][1]
    optimized = transform.applied(original, found.dgroup, found.calls, found=found, unroll_=False)
    return found, optimized


@pytest.mark.parametrize("tag", ["p-g2", "v-g3"])
def test_production_fpdeep_unrolls_through_lcssa_exits(tag):
    """PDS/VBDOS FPDEEP retained three iterations because its LCSSA exit phis blocked unrolling."""
    from tools.stages import _bodies
    found, bodies, _ = _bodies(Path(f"fixtures/omf/fpdeep-{tag}.obj").read_bytes())
    partition = corpus.partitioned(Path(f"fixtures/omf/fpdeep-{tag}.obj"))
    original = transform.applied(bodies[0][1], found.dgroup, found.calls,
                                 blocks=partition, found=found, unroll_=False)
    loop, = loops.loops(original.blocks, original.entry)
    exit_at, = set(original.block(loop.header).succ) - loop.body
    assert original.block(exit_at).phis
    changed = unroll.expanded(original, found.dgroup, found.calls)
    assert changed is not original
    assert not loops.loops(changed.blocks, changed.entry)
    predecessors = loops.predecessors(changed.blocks)
    for block in changed.blocks:
        for phi in block.phis:
            assert set(phi.incoming) == set(predecessors[block.at])


def test_normal_pipeline_expands_and_folds_fpdeep_to_a_fixed_point():
    """FPDEEP's improvements previously required an out-of-band unroll wrapper."""
    found, original = body()
    changed = transform.applied(original, found.dgroup, found.calls, found=found)
    assert changed.repetitions == ((0x66, 3),)
    assert not loops.loops(changed.blocks, changed.entry)
    assert transform.applied(changed, found.dgroup, found.calls, found=found) == changed
    assert not transform.applied(original, found.dgroup, found.calls, found=found, unroll_=False).repetitions


def test_fpcse_exact_ten_iteration_sum_folds_in_source_order():
    """FPCSE computed its exact 487.5 sum ten times despite fitting the bounded expansion budget."""
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    original = mir.bodies(found, corpus.partitioned(path))[0][1]
    changed = transform.applied(original, found.dgroup, found.calls, found=found)
    assert changed.repetitions == ((0x66, 10),)
    assert not loops.loops(changed.blocks, changed.entry)
    assert not any(op.floating for block in changed.blocks for op in block.ops)
    import struct
    bits = int.from_bytes(struct.pack("<f", 487.5), "little")
    assert any(op.kind is mir.Kind.STORE and op.at == 0xa1 and op.args == (mir.Const(bits, 4),)
               for block in changed.blocks for op in block.ops)
    lower_floats.checked(changed)


def test_runtime_input_loop_is_not_expanded_without_exact_folding():
    """FPCSEX's ten runtime-input iterations grew into ten copies without eliminating their arithmetic."""
    path = Path("fixtures/omf/fpcsex-p-g2.obj")
    found = corpus.loaded(path)
    original = mir.bodies(found, corpus.partitioned(path))[0][1]
    changed = transform.applied(original, found.dgroup, found.calls, found=found)
    assert not changed.repetitions
    assert loops.loops(changed.blocks, changed.entry)


def test_fpdeep_unroll_preserves_order_and_fresh_definitions():
    found, original = body()
    loop, = loops.loops(original.blocks, original.entry)
    latch = original.block(next(iter(loop.latches)))
    changed = unroll.expanded(original, found.dgroup, found.calls)
    assert not loops.loops(changed.blocks, changed.entry)
    operations = changed.block(latch.at).ops
    effects = lambda ops: [op.id for op in ops if op.floating or op.kind is mir.Kind.CALL]
    assert effects(operations) == effects(latch.ops) * 3
    definitions = [value.id for op in operations for value in op.defines]
    assert len(definitions) == len(set(definitions))
    assert unroll.expanded(changed, found.dgroup, found.calls) == changed


def test_unrolled_latch_explicitly_skips_the_original_header():
    """FPDEEP fell through to its original header and started the expanded body again."""
    found, original = body()
    loop, = loops.loops(original.blocks, original.entry)
    changed = unroll.expanded(original, found.dgroup, found.calls)
    latch = changed.block(next(iter(loop.latches)))
    assert latch.ops[-1].kind is mir.Kind.JUMP
    assert latch.ops[-1].target == latch.succ[0]
    assert latch.ops[-1].target not in loop.body


def test_fpdeep_expansion_exposes_exact_array_arithmetic():
    """FPDEEP's 144/784/3600 squares and 6/14/30 ratios stayed unknown after expansion."""
    found, original = body()
    changed = unroll.expanded(original, found.dgroup, found.calls)
    facts = floatfacts.known(changed, found.dgroup, found.calls)
    for address, expected in ((0x9c, [144, 784, 3600]), (0xe4, [6, 14, 30])):
        values = [facts.get(op.args[0].value) for block in changed.blocks for op in block.ops
                  if op.at == address and op.kind is mir.Kind.FSTORE and not op.stores]
        assert all(value is not None for value in values)
        assert [value.value for value in values] == expected


def test_fpdeep_exact_integer_arguments_keep_floating_checkpoints(monkeypatch):
    """FPDEEP kept reading converted square/ratio temporaries instead of known answers."""
    found, original = body()
    from qbopt.optimize import floatfold
    monkeypatch.setattr(floatfold, "stored", lambda body, facts: body)
    expanded = unroll.expanded(original, found.dgroup, found.calls)
    converted = floatfacts.converted(expanded, found.dgroup, found.calls)
    assert {6, 14, 30, 144, 784, 3600} <= {value.n for value in converted.values()}
    folded = transform.folded(expanded, found.dgroup, found.calls)
    lower_floats.checked(folded)
    arguments = [op.args[0].n for block in folded.blocks for op in block.ops
                 if op.kind is mir.Kind.ARG and op.at in (0xa1, 0xe9)
                 and isinstance(op.args[0], mir.Const)]
    assert arguments == [144, 6, 784, 14, 3600, 30]
    for block in folded.blocks:
        for op in block.ops:
            if op.kind is mir.Kind.ARG:
                assert not any(isinstance(arg, mir.Held) and arg.value in converted for arg in op.args)


def test_fpdeep_exact_stores_remove_their_arithmetic_chains():
    """FPDEEP recomputed exact squares/ratios/mixes even after all inputs were proven."""
    from fractions import Fraction

    from qbopt.model.floating import Format
    found, original = body()
    expanded = unroll.expanded(original, found.dgroup, found.calls)
    folded = transform.folded(expanded, found.dgroup, found.calls)
    values = [floatfacts.decoded(op.args[0].n, Format.BINARY32).value
              for block in folded.blocks for op in block.ops
              if op.kind is mir.Kind.STORE and op.at in (0x77, 0xbf, 0x107)]
    assert values == [144, 6, Fraction(1, 2), 784, 14, Fraction(3, 4), 3600, 30, Fraction(7, 8)]
    calls = lambda body: [op for block in body.blocks for op in block.ops if op.kind is mir.Kind.CALL]
    assert calls(expanded) == calls(folded)
    lower_floats.checked(folded)


@pytest.mark.parametrize("number,expected", [(-6, 0xfffffffa), (2147483647, 2147483647),
                                             (2147483648, None), ("1/3", None)])
def test_integer_conversion_facts_require_exact_in_range_values(monkeypatch, number, expected):
    """FPDEEP's conversion facts must not invent a rounded or overflowing print argument."""
    from fractions import Fraction
    from dataclasses import replace
    found, original = body()
    op = next(op for block in original.blocks for op in block.ops
              if op.kind is mir.Kind.FSTORE and op.at == 0x9c)
    isolated = replace(original, blocks=(mir.MirBlock(original.entry, (), (op,), ()),))
    monkeypatch.setattr(floatfacts, "known", lambda *args, **kwargs:
                        {op.args[0].value: floatfacts.Finite(Fraction(number))})
    facts = floatfacts.converted(isolated, found.dgroup, found.calls)
    assert (facts[op.results[0].value].n if facts else None) == expected


@pytest.mark.parametrize("damage", ["duplicate"])
def test_expansion_provenance_does_not_allow_arbitrary_float_sequences(damage):
    from dataclasses import replace
    found, original = body()
    changed = unroll.expanded(original, found.dgroup, found.calls)
    at, count = changed.repetitions[0]
    block = changed.block(at)
    positions = [index for index, op in enumerate(block.ops) if op.floating]
    match damage:
        case "count": changed = replace(changed, repetitions=((at, count - 1),))
        case "duplicate": changed = replace(changed, repetitions=changed.repetitions * 2)
        case _:
            ops = list(block.ops)
            first, second = positions[:2]
            if damage == "missing": del ops[first]
            else: ops[first], ops[second] = ops[second], ops[first]
            changed = replace(changed, blocks=tuple(replace(one, ops=tuple(ops)) if one.at == at else one
                                                   for one in changed.blocks))
    with pytest.raises(lower.Unlowered):
        lower_floats.checked(changed)
