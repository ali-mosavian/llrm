"""
qbopt/optimize/transform.py's own gate.

What is checked here is mostly the *order* the transforms run in and what
each one had to be right about, because widening was written wrong twice and
neither time could the host suite see it.
"""

from pathlib import Path
from dataclasses import replace

import pytest
from iced_x86 import Register

import corpus
from qbopt import rewrite
from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import lower
from qbopt.objectfile import omf
from qbopt.objectfile import module
from qbopt.optimize import transform
from qbopt.model.passes import Options


def test_half_liveness_reuses_each_immutable_body_across_a_transaction(monkeypatch: pytest.MonkeyPatch) -> None:
    """Matmul recomputed 595 fixed points for only 170 body objects.

    Analysis of a nested candidate temporarily displaces its parent state.
    Returning to that exact immutable parent must recover its earlier answer,
    rather than retaining only the transaction's most recent body.
    """
    value = mir.Value(1, 0, variable=1)
    copy = mir.Op(
        0,
        ir.Operation.MOVE,
        "mov",
        (value,),
        (mir.Const(1, 2),),
        kind=mir.Kind.COPY,
        results=(mir.Held(value, 2),),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (copy,), ()),))
    leaving = transform._leaving
    calls = 0

    def counted(state):
        nonlocal calls
        calls += 1
        return leaving(state)

    monkeypatch.setattr(transform, "_leaving", counted)
    with transform._reusing_halves():
        other = replace(body, entry=1)
        assert transform.halves(body) == transform.halves(other)
        assert transform.halves(body) == transform.halves(other)

    assert calls == 2


def test_half_liveness_reuses_each_immutable_body_across_a_transaction(monkeypatch: pytest.MonkeyPatch) -> None:
    """Matmul recomputed 595 fixed points for only 170 body objects.

    Analysis of a nested candidate temporarily displaces its parent state.
    Returning to that exact immutable parent must recover its earlier answer,
    rather than retaining only the transaction's most recent body.
    """
    value = mir.Value(1, 0, variable=1)
    copy = mir.Op(
        0,
        ir.Operation.MOVE,
        "mov",
        (value,),
        (mir.Const(1, 2),),
        kind=mir.Kind.COPY,
        results=(mir.Held(value, 2),),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (copy,), ()),))
    leaving = transform._leaving
    calls = 0

    def counted(state):
        nonlocal calls
        calls += 1
        return leaving(state)

    monkeypatch.setattr(transform, "_leaving", counted)
    with transform._reusing_halves():
        other = replace(body, entry=1)
        assert transform.halves(body) == transform.halves(other)
        assert transform.halves(body) == transform.halves(other)

    assert calls == 2


def test_final_pipeline_inerts_unreachable_executable_blocks() -> None:
    """NBODYS could not be emitted after peeling left clone 0x310000002b unreachable.

    Structural candidates may make an entire cloned region dead on their last
    simplifying round.  The optimizer's public result must retain those
    blocks only as byte-ownership markers, never as detached executable work.
    """
    dead_store = mir.Op(9, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE)
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (), ()),
            mir.MirBlock(0x310000002B, (), (dead_store,), ()),
        ),
    )

    result = transform.applied(body, frozenset(), {}, only="no-such-pass")

    orphan = result.block(0x310000002B)
    assert orphan is not None
    assert not orphan.succ
    assert all(op.kind is mir.Kind.NOTHING and not op.stores for op in orphan.ops)


def test_final_pipeline_drops_unreachable_empty_blocks() -> None:
    """Fresh NBODYS retained empty clone 0x310000002b and failed LIR verification.

    A detached block with no operations owns no source bytes, so unlike an
    ownership marker it has no reason to survive the MIR boundary.
    """
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (), ()),
            mir.MirBlock(0x310000002B, (), (), ()),
        ),
    )

    result = transform.applied(body, frozenset(), {}, only="no-such-pass")

    assert result.block(0x310000002B) is None


@pytest.mark.parametrize("guard", ["none", "phi", "store", "cycle"])
def test_empty_jump_threading_preserves_phi_inputs_and_effects(guard):
    """Collapsed FPCSE's trampoline is removable; a phi edge or store is not."""
    from dataclasses import replace

    def jump(at, target):
        return mir.Op(at, ir.Operation.JUMP, "jmp", (), (), kind=mir.Kind.JUMP, target=target)

    entry = mir.MirBlock(0, (), (jump(0, 1),), (1,))
    middle = mir.MirBlock(1, (), (jump(1, 2),), (2,))
    end = mir.MirBlock(2, (), (), ())
    if guard == "phi":
        value = mir.Value(1, 2)
        end = replace(end, phis=(mir.Phi(value, {1: mir.Value(2, 1)}),))
    elif guard == "store":
        store = mir.Op(1, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE)
        middle = replace(middle, ops=(store, *middle.ops))
    elif guard == "cycle":
        middle = replace(middle, ops=(jump(1, 1),), succ=(1,))
    body = mir.MirBody(0, (entry, middle, end))
    result = transform._threaded(body)
    if guard == "none":
        assert result.blocks[0].succ == (2,)
        assert result.blocks[0].ops[-1].target == 2
        assert result.blocks[1].ops[-1].kind is mir.Kind.NOTHING
        assert result.blocks[1].ops[-1].name == ""
    else:
        assert result == body


def test_pipeline_reaches_a_fixed_point_without_emission() -> None:
    """lngmix still changed on a second optimization of the same MIR body."""
    from qbopt.abi import runtime
    from qbopt.frontend import blocks

    found = corpus.loaded(Path("fixtures/omf/lngmix-p-g2.obj"))
    partition = blocks.partition(found, blocks.code_map(found))
    body = mir.bodies(found, partition, runtime.for_module(found))[0][1]
    first = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    second = transform.applied(first, found.dgroup, found.calls, blocks=partition, found=found)
    assert second == first


def test_structural_candidate_pass_stages_are_named_tentative(monkeypatch) -> None:
    """Matmul's rejected peel stages appeared to be the production pipeline."""
    from qbopt.optimize import unroll

    stages = []
    body = mir.MirBody(0, (mir.MirBlock(0, (), (), ()),), sealed=True)

    def candidate(original, _where, *, optimize, watch):
        optimize(original)
        return original

    monkeypatch.setattr(unroll, "optimized", candidate)
    transform.applied(
        body,
        frozenset(),
        {},
        options=Options(peel=False, unswitch=False),
        watch=lambda stage, _state: stages.append(stage),
    )

    assert any(stage.startswith("candidate-unroll-r01-") for stage in stages)


def test_hoisted_variables_do_not_collide_with_promoted_cells() -> None:
    """lngmix printed 4081664 for 142900 after hoisting reused a promoted variable id."""
    from qbopt.abi import runtime
    from qbopt.frontend import blocks

    found = corpus.loaded(Path("fixtures/omf/lngmix-p-g2.obj"))
    partition = blocks.partition(found, blocks.code_map(found))
    body = mir.bodies(found, partition, runtime.for_module(found))[0][1]
    stages = {}
    transform.applied(
        body,
        found.dgroup,
        found.calls,
        blocks=partition,
        found=found,
        watch=lambda name, state: stages.setdefault(name, state),
    )
    body = stages["r01-place"]
    value = next(op for block in body.blocks for op in block.ops if op.at == 0x76 and op.kind is mir.Kind.ADD).defines[
        -1
    ]
    after = transform._reparented(body, {value})
    renamed = next(one for one in after.values if one.id == value.id)
    assert renamed.variable > max(one.variable for one in body.values)


def test_gvn_load_chains_keep_a_defined_return_value() -> None:
    """procs-q-O's return named a deleted intermediate reload and could not allocate."""
    from qbopt.abi import runtime
    from qbopt.frontend import blocks

    found = corpus.loaded(Path("fixtures/omf/procs-q-O.obj".lower()))
    partition = blocks.partition(found, blocks.code_map(found))
    body = next(
        body for name, body in mir.bodies(found, partition, runtime.for_module(found)) if name == "procedure TWICE"
    )
    done = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found, only="gvn")
    defined = {value for block in done.blocks for op in block.ops for value in op.defines}
    exit_call = next(op for block in done.blocks for op in block.ops if op.at == 0x12F)
    assert all(arg.value in defined for arg in exit_call.args if isinstance(arg, mir.Held))


def test_substitution_preserves_memory_address_edges() -> None:
    """Removing a join must not leave memory addressing its deleted value."""
    old, survivor = mir.Value(901, 0), mir.Value(902, 0)
    ref = mir.MemRef(None, 2, base=old, segment=old)
    op = mir.Op(
        0,
        ir.Operation.MOVE,
        "mov",
        (),
        (old,),
        loads=(ref,),
        stores=(ref,),
        args=(mir.Cell(ref),),
        results=(mir.Cell(ref),),
    )
    done = transform._substituted(op, {old.id: survivor})
    assert done.loads[0].base == survivor
    assert done.loads[0].segment == survivor
    assert done.args == (mir.Cell(done.loads[0]),)
    assert done.results == (mir.Cell(done.stores[0]),)


def test_pipeline_removes_hotlop_obsolete_constant_load() -> None:
    """hotlop kept loading 3 each iteration after its product folded to 21."""
    from qbopt.frontend import blocks

    found = corpus.loaded(Path("fixtures/omf/hotlop-p-g2.obj"))
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    for _, body in mir.bodies(found, blocks.partition(found, mapped)):
        done = transform.applied(body, found.dgroup, found.calls, found=found)
        assert not any(op.at == 0x48 and op.args == (mir.Const(3, 2),) for block in done.blocks for op in block.ops)


def test_dead_store_does_not_delete_a_load_at_the_same_address() -> None:
    """nbody printed PX0=-7627 instead of 1258 after losing its counter load."""
    from dataclasses import replace

    source, loaded = mir.Value(1, 0), mir.Value(2, 3)
    target = mir.MemRef(ir.Addr(module.Space.SEGMENT, 0, index=5), 2)
    counter = mir.MemRef(ir.Addr(module.Space.SEGMENT, 2, index=5), 2)
    first = mir.Op(
        0,
        ir.Operation.MOVE,
        "mov",
        (source,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Const(7, 2),),
        results=(mir.Held(source, 2),),
    )
    store = mir.Op(
        3,
        ir.Operation.MOVE,
        "mov",
        (),
        (source,),
        kind=mir.Kind.STORE,
        args=(mir.Held(source, 2),),
        results=(mir.Cell(target),),
        stores=(target,),
    )
    load = mir.Op(
        3,
        ir.Operation.MOVE,
        "mov",
        (loaded,),
        (),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(counter),),
        results=(mir.Held(loaded, 2),),
        loads=(counter,),
    )
    overwrite = replace(store, at=6)
    use = mir.Op(9, ir.Operation.PUSH, "push", (), (loaded,), kind=mir.Kind.ARG, args=(mir.Held(loaded, 2),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (first, store, load, overwrite, use), ()),))
    done = transform.without_dead_stores(body, frozenset({5}), {})
    ops = done.blocks[0].ops
    assert any(loaded in op.defines for op in ops)
    assert sum(bool(op.stores) for op in ops) == 1


def test_forwarding_extends_lifetime_without_conflating_shared_addresses() -> None:
    """Nbody kept statement reloads because their providers were not already live."""
    source, loaded, unrelated = (mir.Value(index, index) for index in (1, 2, 3))
    target = mir.MemRef(ir.Addr(module.Space.SEGMENT, 0, index=5), 4)
    other = mir.MemRef(ir.Addr(module.Space.SEGMENT, 8, index=5), 4)
    first = mir.Op(
        0,
        ir.Operation.MOVE,
        "mov",
        (source,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Const(7, 4),),
        results=(mir.Held(source, 4),),
    )
    store = mir.Op(
        1,
        ir.Operation.MOVE,
        "mov",
        (),
        (source,),
        kind=mir.Kind.STORE,
        args=(mir.Held(source, 4),),
        results=(mir.Cell(target),),
        stores=(target,),
    )
    load = mir.Op(
        2,
        ir.Operation.MOVE,
        "mov",
        (loaded,),
        (),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(target),),
        results=(mir.Held(loaded, 4),),
        loads=(target,),
    )
    neighbor = mir.Op(
        2,
        ir.Operation.MOVE,
        "mov",
        (unrelated,),
        (),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(other),),
        results=(mir.Held(unrelated, 4),),
        loads=(other,),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (first, store, load, neighbor), ()),))
    done = transform.forwarded(body, frozenset({5}), {}).blocks[0].ops
    assert done[2].args == (mir.Held(source, 4),) and not done[2].loads
    assert done[3] == neighbor


@pytest.mark.parametrize("number,safe", [(7, True), (0, False), (0xFFFFFFFF, False)])
def test_divisor_constants_propagate_without_reordering(number, safe):
    """LNGMXX retained invariant division by 7 because its constant divisor stayed opaque to LICM."""
    from qbopt.analysis import consts

    dividend, divisor, quotient, remainder = (mir.Value(index, 0) for index in range(1, 5))
    op = mir.Op(
        0,
        ir.Operation.DIVIDE,
        "idiv",
        (quotient, remainder),
        (dividend, divisor),
        kind=mir.Kind.DIVMOD,
        args=(mir.Held(dividend, 4), mir.Held(divisor, 4)),
        results=(mir.Held(quotient, 4), mir.Held(remainder, 4)),
    )
    done = transform._constant_operands(op, {divisor: consts.Known(number, 4)})
    assert done.args == (mir.Held(dividend, 4), mir.Const(number, 4))
    assert done.uses == (dividend,)
    assert transform._cannot_fault(done) is safe
    assert transform._constant_operands(op, {divisor: consts.Known(number, 2)}) == op


def test_a_third_equal_divide_reads_the_answer_the_first_computed() -> None:
    """lngmix under SROA refused 0x0071: "mov ... defines [v24_1] through no operand".

    The second divide became a copy of the first's remainder, and the third
    was served the second's quotient, which that copy no longer computes.
    """
    dividend, divisor = mir.Value(1, 0), mir.Value(2, 0)

    def divide(at: int) -> mir.Op:
        quotient, remainder = mir.Value(at + 3, at), mir.Value(at + 4, at)
        return mir.Op(
            at,
            ir.Operation.DIVIDE,
            "idiv",
            (quotient, remainder),
            (dividend, divisor),
            kind=mir.Kind.DIVMOD,
            args=(mir.Held(dividend, 4), mir.Held(divisor, 4)),
            results=(mir.Held(quotient, 4), mir.Held(remainder, 4)),
        )

    first, second, third = divide(0), divide(8), divide(16)
    remainder, quotient, total = second.results[1].value, third.results[0].value, mir.Value(40, 24)
    add = mir.Op(
        24,
        ir.Operation.BINARY,
        "add",
        (total,),
        (remainder, quotient),
        kind=mir.Kind.ADD,
        args=(mir.Held(remainder, 4), mir.Held(quotient, 4)),
        results=(mir.Held(total, 4),),
    )
    returned = mir.Op(28, ir.Operation.RETURN, "ret", (), (total,), kind=mir.Kind.RETURN, args=(mir.Held(total, 4),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (first, second, third, add, returned), ()),))

    ops = transform.reused_divides(body, frozenset()).blocks[0].ops
    assert [op.kind for op in ops[:3]] == [mir.Kind.DIVMOD, mir.Kind.COPY, mir.Kind.COPY]

    computed = {one.value for op in ops for one in op.results}
    assert {value for op in ops for value in op.uses} <= computed | {dividend, divisor}


def test_leading_deletion_does_not_delete_its_survivor() -> None:
    first = mir.Op(0, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.COPY, args=(mir.Const(3, 2),))
    survivor = mir.Op(3, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.COPY, args=(mir.Const(21, 2),))
    last = mir.Op(6, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.COPY, args=(mir.Const(5, 2),))
    done = transform._absorb([first, survivor, last], {0})
    assert [op.args for op in done] == [survivor.args, last.args]


def test_hotlop_add_uses_its_known_product_directly() -> None:
    """hotlop needlessly materialized 21 in a register on every iteration."""
    from qbopt.frontend import blocks

    found = corpus.loaded(Path("fixtures/omf/hotlop-p-g2.obj"))
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    seen = []
    for _, body in mir.bodies(found, blocks.partition(found, mapped)):
        done = transform.folded(body, found.dgroup, found.calls)
        seen.extend(
            op for block in done.blocks for op in block.ops if op.kind is mir.Kind.ADD and mir.Const(21, 2) in op.args
        )
    assert seen


def test_long_pair_recognition_is_not_an_optimizer_pass() -> None:
    """A late widened op lies about how much memory it reads.

    `mov eax,[x]` keeps the low half's own `loads` -- two bytes at [x] --
    while the instruction reads four, so avail.py asked whether [x+2] had
    been written and was told nothing had touched it, and forwarded a stale
    high half. Running widening after the passes that reason about memory
    means none of them ever sees the mismatch.

    Recognition now happens during raising, where the whole access is
    represented correctly before any memory pass runs.
    """
    import inspect

    signature = inspect.signature(transform.applied)
    assert signature.parameters["drop_loads"].default is True
    assert signature.parameters["drop_stores"].default is True

    # Widening is not a pass: raising has already made whole scalar values,
    # so no optimizer pipeline entry may repeat machine-shaped recognition.
    assert "widen" not in transform.PASSES
    assert "drop_stores" in transform.PASSES


def test_value_reuse_is_one_gvn_pre_pass() -> None:
    """Four separately iterated reuse passes made ordering part of semantics."""
    assert "gvn" in transform.PASSES
    assert not {"forward", "drop_loads", "reuse", "cse"} & set(transform.PASSES)


def test_scalar_replacement_precedes_scalar_and_cfg_simplification() -> None:
    """Aggregate leaves must become SSA before scalar and CFG passes run.

    Running promotion after folding, branch selection and loop normalization
    hid constants and values behind memory for the entire first fixed-point
    round, so those passes could not expose the opportunities created by
    scalar replacement at their intended boundary.
    """
    order = {name: transform.PASSES.index(name) for name in ("sroa", "fold", "decide", "loopsimplify")}
    assert order["sroa"] < min(order[name] for name in ("fold", "decide", "loopsimplify"))


def test_fixed_point_budget_scales_with_the_body(monkeypatch: pytest.MonkeyPatch) -> None:
    """Moving all promotion early made C matmul need over sixteen rounds.

    A fixed iteration count is not a convergence rule: a larger body can
    contain a longer simplification chain.  The driver must allow progress
    proportional to the number of operations while still rejecting cycles.
    """
    from dataclasses import replace

    class OneAtATime:
        name = "one_at_a_time"

        def transform(self, body: mir.MirBody) -> mir.MirBody:
            block = body.blocks[0]
            return body if not block.ops else replace(body, blocks=(replace(block, ops=block.ops[:-1]),))

    ops = tuple(
        mir.Op(at, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.NOTHING, source_backed=False) for at in range(20)
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), ops, ()),))
    monkeypatch.setattr(transform, "pipeline", lambda *_args, **_kwargs: [OneAtATime()])

    result = transform.applied(body, frozenset(), {})
    assert not result.blocks[0].ops


def test_fixed_point_reports_a_repeated_state_as_a_cycle(monkeypatch: pytest.MonkeyPatch) -> None:
    """A size-scaled budget must not turn an oscillating pass into a long wait."""
    from dataclasses import replace

    class Toggle:
        name = "toggle"

        def transform(self, body: mir.MirBody) -> mir.MirBody:
            return replace(body, cloned=not body.cloned)

    body = mir.MirBody(0, (mir.MirBlock(0, (), (), ()),))
    monkeypatch.setattr(transform, "pipeline", lambda *_args, **_kwargs: [Toggle()])

    with pytest.raises(RuntimeError, match="cycle"):
        transform.applied(body, frozenset(), {})


def test_sroa_runs_once_before_the_scalar_fixed_point(monkeypatch: pytest.MonkeyPatch) -> None:
    """Range-based SROA took four times longer when repeated every round."""
    from dataclasses import replace

    calls = 0

    class OneAtATime:
        name = "one_at_a_time"

        def transform(self, body: mir.MirBody) -> mir.MirBody:
            block = body.blocks[0]
            return body if not block.ops else replace(body, blocks=(replace(block, ops=block.ops[:-1]),))

    def sroa(body: mir.MirBody) -> mir.MirBody:
        nonlocal calls
        calls += 1
        return body

    from qbopt.optimize import promote
    from qbopt.model.passes import Where

    first = promote.Sroa(Where())
    monkeypatch.setattr(first, "transform", sroa)
    monkeypatch.setattr(transform, "pipeline", lambda *_args, **_kwargs: [first, OneAtATime()])
    ops = tuple(
        mir.Op(at, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.NOTHING, source_backed=False) for at in range(20)
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), ops, ()),))

    transform.applied(body, frozenset(), {})
    assert calls == 1


def test_sroa_crosses_both_structural_candidate_boundaries(monkeypatch: pytest.MonkeyPatch) -> None:
    """Matmul exposed aggregate leaves both at cloning and after scalar convergence."""
    from dataclasses import replace

    from qbopt.optimize import peel
    from qbopt.optimize import promote
    from qbopt.model.passes import Where

    seen = []
    scalarizer = promote.Sroa(Where())
    structural = peel.Peel(Where())

    def sroa(body: mir.MirBody) -> mir.MirBody:
        seen.append(body.cloned)
        return body

    def specialized(body, _where, *, optimize, watch=None):
        return optimize(replace(body, cloned=True))

    monkeypatch.setattr(scalarizer, "transform", sroa)
    monkeypatch.setattr(peel, "optimized", specialized)
    monkeypatch.setattr(transform, "pipeline", lambda *_args, **_kwargs: [scalarizer, structural])
    body = mir.MirBody(0, (mir.MirBlock(0, (), (), ()),))

    result = transform.applied(body, frozenset(), {})

    assert result.cloned
    assert seen == [False, True, True]


def test_structural_profitability_prices_leaves_exposed_by_scalar_convergence(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Matmul's cloned loop made aggregate indexes exact during its scalar
    fixed point, after the candidate's first SROA boundary.

    Accepting that candidate and only then rerunning SROA priced one body but
    committed a different one: the retained row loop became slower than the
    fully expanded baseline.  The transaction must scalarize the newly exact
    leaves and reconverge scalar MIR before structural profitability sees it.
    """
    from dataclasses import replace

    from qbopt.optimize import peel
    from qbopt.optimize import promote
    from qbopt.model.passes import Where

    seen = []
    scalarizer = promote.Sroa(Where())
    structural = peel.Peel(Where())

    class ExposeLeaves:
        name = "expose_leaves"

        def transform(self, body: mir.MirBody) -> mir.MirBody:
            if body.cloned and not body.repetitions:
                return replace(body, repetitions=((1, 1),))
            return body

    def sroa(body: mir.MirBody) -> mir.MirBody:
        seen.append((body.cloned, bool(body.repetitions)))
        return replace(body, sealed=True) if body.cloned and body.repetitions else body

    def specialized(body, _where, *, optimize, watch=None):
        return optimize(replace(body, cloned=True))

    monkeypatch.setattr(scalarizer, "transform", sroa)
    monkeypatch.setattr(peel, "optimized", specialized)
    monkeypatch.setattr(transform, "pipeline", lambda *_args, **_kwargs: [scalarizer, ExposeLeaves(), structural])
    body = mir.MirBody(0, (mir.MirBlock(0, (), (), ()),))

    result = transform.applied(body, frozenset(), {})

    assert result.sealed
    assert seen == [(False, False), (True, False), (True, True)]


def test_structural_candidate_refuses_a_pressure_regression_after_settled_sroa(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Matmul's second SROA boundary removed semantic work but cost 1,407
    weighted 386 cycles after its independent scalar leaves were allocated.

    Price the settled aggregate replacement against the already-converged
    representation of the same structural candidate, so loop savings cannot
    hide a local scalarization loss.
    """
    from dataclasses import replace

    from qbopt.optimize import peel
    from qbopt.optimize import profit
    from qbopt.optimize import promote
    from qbopt.model.passes import Where

    scalarizer = promote.Sroa(Where())
    structural = peel.Peel(Where())

    class ExposeLeaves:
        name = "expose_leaves"

        def transform(self, body: mir.MirBody) -> mir.MirBody:
            if body.cloned and not body.repetitions:
                return replace(body, repetitions=((1, 1),))
            return body

    def sroa(body: mir.MirBody) -> mir.MirBody:
        return replace(body, sealed=True) if body.cloned and body.repetitions else body

    def specialized(body, _where, *, optimize, watch=None):
        return optimize(replace(body, cloned=True))

    def cost(body: mir.MirBody, *_args, **_kwargs) -> int:
        return 2 if body.sealed else 1

    monkeypatch.setattr(scalarizer, "transform", sroa)
    monkeypatch.setattr(peel, "optimized", specialized)
    monkeypatch.setattr(profit, "pressure_adjusted", cost)
    monkeypatch.setattr(transform, "pipeline", lambda *_args, **_kwargs: [scalarizer, ExposeLeaves(), structural])
    body = mir.MirBody(0, (mir.MirBlock(0, (), (), ()),))

    result = transform.applied(body, frozenset(), {})

    assert result.cloned
    assert result.repetitions
    assert not result.sealed


def test_the_rename_alone_is_what_was_unsound() -> None:
    """The concrete fact the chain and the restore exist to handle.

    Both halves of a dx:ax pair rename to their own roots -- ax to eax and
    dx to edx -- so a 32-bit operation on the pair is one register and dx is
    left holding what it held. That is not an argument against the rename;
    it is why a widened chain has to end in `push eax / pop ax / pop dx`.
    """
    assert ir.ROOT[Register.AX] is Register.EAX
    assert ir.ROOT[Register.DX] is Register.EDX
    # the two halves of BC's pair 0 root to different registers, which is
    # exactly why renaming one of them cannot express the whole long
    assert ir.ROOT[Register.AX] is not ir.ROOT[Register.DX]


def test_a_transform_accounts_for_every_byte_it_removes() -> None:
    """A deletion transfers opaque ownership without learning byte ranges."""
    import inspect

    # `_without` and `_reclaimed` preserve deleted occurrences as inert
    # markers. Lowering resolves those opaque identities to byte ranges.
    source = inspect.getsource(transform._without) + inspect.getsource(transform._reclaimed)
    assert "absorbed" in source
    assert "covers" not in source and "extra_covers" not in source


def _corpus():
    from pathlib import Path

    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    for obj in sorted(Path("fixtures/omf").glob("*.obj")):
        found = module.of(omf.parse(obj.read_bytes()))
        if found is None:
            continue
        mapped = code_map(found)
        if isinstance(mapped, str):
            continue
        yield obj, found, split.partition(found, mapped)


def _strategy(found, blocks):
    """{call address: True where calls.py would pop rather than reload}."""
    from qbopt.legacy import calls as machine

    reached = [one for block in blocks for one in block.insns]
    return {one.at: bool(one.consume) for one in machine.sites(found, reached, blocks)}


def _absorbed_ops(obj, found, blocks):
    """Every absorbed site in one object, as (call address, the ops it became).

    Keyed on the region calls.py's own CallSite names: the call alone where
    the operands are popped, and push-through-call where they are reloaded
    and the pushes go too.
    """
    from qbopt.model import mir
    from qbopt.legacy import calls as machine

    reached = [one for block in blocks for one in block.insns]
    sites = {
        one.at: one
        for one in machine.sites(found, reached, blocks)
        if (found.calls.get(one.at) or "").upper() in transform.EMITTED
    }
    for _name, body in mir.bodies(found, blocks):
        after = transform.absorbed(body, blocks, found.calls, found)
        for block in after.blocks:
            for at, site in sites.items():
                ops = [one for one in block.ops if site.start <= one.at < site.end]
                if len(ops) > 1 and all(
                    mir.rewritten(one) or not one.source_backed or one.name == "restore" for one in ops
                ):
                    yield at, ops


def test_the_invariant_run_never_takes_control_flow_a_flag_or_a_carried_value() -> None:
    """What the run is allowed to contain, asserted on the run itself.

    Asserting on the finished body instead was worthless: with any one of
    these three restored the other two refuse the loop anyway, so the test
    passed against each defect on its own. The three, and what each cost:

    A branch reads nothing the loop writes, so `cmp`, `jle` and `jmp` were
    all invariant and hotlop's latch left with them -- the MIR still held
    the loop, the re-parsed object did not. Counting only non-flag values
    as crossing let a compare move out from under the branch reading it.
    And `inside` collected only `op.defines`, missing the phi results,
    which are the loop-carried values themselves.
    """
    from qbopt.model import mir
    from qbopt.analysis import loops as loopy

    seen = 0
    for obj, found, blocks in _corpus():
        raised = mir.bodies(found, blocks)
        for name, body in raised:
            at_of = {one.at: one for one in body.blocks}
            for loop in loopy.loops(list(body.blocks), body.entry):
                ops = [one for at in sorted(loop.body) for one in at_of[at].ops]
                carried = {phi.result for at in loop.body for phi in at_of[at].phis}
                phis = [phi for at in loop.body for phi in at_of[at].phis]
                run = transform._invariant_run(
                    ops, carried, [(ref, None) for one in ops for ref in one.stores], found.dgroup, found.calls, phis
                )
                if not run:
                    continue
                seen += 1
                rest = [one for one in ops if one not in run]
                where = f"{obj.stem} {name} loop {loop.header:#x}"
                for one in run:
                    what = lower.current(one, node=raised.source.nodes.get(one.id))
                    assert what is not None and what.op not in (ir.Operation.JUMP, ir.Operation.BRANCH), (
                        f"{where}: {one.at:#x} {one.name} is control flow"
                    )
                    for value in one.defines:
                        if value.flags:
                            assert not any(value in other.uses for other in rest), (
                                f"{where}: {one.at:#x} sets a flag {rest} still reads"
                            )
                    # Really read, not merely preserved: `merges` is the
                    # raise's account of which uses are only the previous
                    # contents of what the operation writes.
                    taken = {use for use in one.uses if use not in one.merges}
                    assert not (taken & carried), f"{where}: {one.at:#x} {one.name} reads a value the loop carries"
    assert seen, "no loop in the corpus offers an invariant run, so this proves nothing"


def test_hoisting_leaves_every_loop_and_every_terminator_where_it_was() -> None:
    """A hoist may move work out of a loop. It may not move the loop.

    All three ways it did. A branch reads nothing the loop writes, so the
    invariance test called `cmp`, `jle` and `jmp` invariant and hotlop's
    latch left with them -- the MIR still held the loop and the re-parsed
    object did not. Counting only non-flag values as crossing let the
    compare move out from under the branch that reads it. And collecting
    only `op.defines` as defined-in-the-loop missed the phi results, which
    are the loop-carried values themselves: hotlop's counter read as
    something defined outside.
    """
    from qbopt.model import mir
    from qbopt.analysis import loops as loopy

    seen = 0
    for obj, found, blocks in _corpus():
        raised = mir.bodies(found, blocks)
        for name, body in raised:
            was = loopy.loops(list(body.blocks), body.entry)
            if not was:
                continue
            seen += 1
            after = transform.hoisted(body, found.dgroup, found.calls)
            now = loopy.loops(list(after.blocks), after.entry)
            assert len(now) == len(was), f"{obj.stem} {name}: {len(was)} loops became {len(now)}"

            # A preheader legitimately gains operations after its last, so
            # the invariant is the shape and not the address: a block that
            # ended in control flow still ends on that same branch.
            for was, now in zip(body.blocks, after.blocks, strict=True):
                if not was.ops or not now.ops:
                    continue
                before_op = was.ops[-1]
                before = lower.current(before_op, node=raised.source.nodes.get(before_op.id))
                if before is None or before.op not in (ir.Operation.JUMP, ir.Operation.BRANCH):
                    continue
                # By where it goes, not by its address: taking the first
                # operation out of a block moves the branch onto the block's
                # own address, and it is the same branch.
                after_op = now.ops[-1]
                after_it = lower.current(after_op, node=raised.source.nodes.get(after_op.id))
                assert after_it is not None and after_it.op is before.op, (
                    f"{obj.stem} {name}: block {was.at:#x} no longer ends in control flow"
                )
                assert after_it.target == before.target, (
                    f"{obj.stem} {name}: block {was.at:#x} ends on a branch somewhere else"
                )
    assert seen, "no body in the corpus has a loop, so this proves nothing"


def test_a_run_whose_flag_the_loop_still_reads_is_not_hoistable() -> None:
    """A flag cannot travel to the loop in a register.

    `cmp [n],1` reads nothing a loop over i writes, so the invariance test
    calls it invariant -- correctly. What stops it leaving is that the
    branch behind it reads the flag it sets, and a flag is not something a
    pinned register can carry. Counting only non-flag values as crossing
    let hotlop's compare move out from under its own `jle`, and the object
    came back with no back edge at all.

    Driven rather than found: with the other two guards in place no loop in
    the corpus offers a run at all, so this shape cannot be observed there.
    """
    from qbopt.model import ir
    from qbopt.model import mir

    def op(at: int, name: str, defines: tuple, uses: tuple) -> mir.Op:
        return mir.Op(at, ir.Operation.COMPARE, name, defines, uses, kind=mir.Kind.SUB)

    flag = mir.Value(1, 0x10, flags=True)
    got = mir.Value(2, 0x10)
    compare = op(0x10, "cmp", (flag, got), ())
    branch = op(0x14, "jle", (), (flag,))
    reader = op(0x18, "add", (), (got,))

    assert transform._crossing([compare], [reader]) == frozenset({got}), "a plain value crosses in a register"
    assert transform._crossing([compare], [branch, reader]) is None, "a flag the loop reads does not"
    assert transform._crossing([compare], [branch]) is None


def test_an_operand_nothing_writes_down_may_leave_with_its_run() -> None:
    """`imul word [k]` multiplies by ax without naming it, and may still go.

    This used to be the opposite assertion. Pinning one value recolours the
    body and an implicit operand does not move with the rename: hotlop
    hoisted `mov ax,[n]` with the multiply behind it, the recolour wrote
    `mov cx,[n]`, and the multiply went on reading ax. It printed 0 for 630,
    and the run was refused rather than risked.

    Two things now stand in the way of that instead of a refusal.
    regalloc.required() will not allocate such an operand anywhere but where
    its instruction reads it, and a result the machine places is copied out
    of that register rather than re-seated -- because `imul` writes dx:ax
    and cannot be told to write anywhere else.
    """
    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.model import mir

    ax = ir.Reg(register=Register.AX, width=2)
    dx = ir.Reg(register=Register.DX, width=2)
    cell = ir.Mem(None, 2)

    def op(at: int, name: str, what: ir.Semantics, defines: tuple, uses: tuple) -> mir.Op:
        return mir.Op(
            at,
            what.op,
            name,
            defines,
            uses,
            (mir.MemRef(None, 2),),
            (),
            kind=mir._kind_of(what, (), ()),
        )

    moving = ir.Semantics(ir.Operation.MOVE, "mov", dests=(ax,), sources=(cell,))
    load = op(0x10, "mov", moving, (mir.Value(1, 0x10),), ())
    # Two destinations and one source: dx:ax = ax * [k], and ax is written
    # down nowhere.
    widening = op(
        0x13,
        "imul",
        ir.Semantics(ir.Operation.MULTIPLY, "imul", dests=(ax, dx), sources=(cell,)),
        (mir.Value(2, 0x13),),
        (mir.Value(1, 0x10),),
    )

    run = transform._invariant_run([load, widening], set(), [], frozenset(), {}, [])
    assert load in run, "an ordinary load is invariant here"
    assert widening in run, "and the multiply behind it leaves with it"


def test_a_precise_volatile_access_does_not_block_disjoint_invariant_work() -> None:
    """C floats reloaded its nonvolatile argument on every volatile iteration.

    Volatile accesses themselves stay ordered and observable.  They do not
    make a disjoint frame-argument load observable, while a genuine opaque
    machine barrier must continue to refuse the entire motion candidate.
    """
    from dataclasses import replace

    from qbopt.model import ir
    from qbopt.model import mir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    argument = mir.MemRef(Addr(Space.FRAME, 6), 2, space=Space.FRAME)
    local = mir.MemRef(Addr(Space.FRAME, -8), 8, space=Space.FRAME, volatile=True)
    value = mir.Value(1, 0x10)
    load = mir.Op(
        0x10,
        ir.Operation.MOVE,
        "mov",
        (value,),
        (),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(argument),),
        results=(mir.Held(value, 2),),
        loads=(argument,),
    )
    observable = mir.Op(
        0x12,
        ir.Operation.MOVE,
        "fstp",
        (),
        (),
        kind=mir.Kind.FSTORE,
        args=(mir.Const(0, 8),),
        results=(mir.Cell(local),),
        stores=(local,),
        volatile=True,
        memory_complete=True,
        reads_complete=True,
    )
    stores = [(local, None)]

    assert transform._invariant_run([load, observable], set(), stores, frozenset(), {}, []) == [load]
    opaque = replace(observable, op=ir.Operation.BARRIER, volatile=False)
    assert transform._invariant_run([load, opaque], set(), stores, frozenset(), {}, []) == []


def test_a_definition_a_phi_carries_and_the_loop_rewrites_does_not_leave_it() -> None:
    """`mov ax,1` starting an inner counter is invariant, and must not move.

    It reads nothing the outer loop writes, so every other test here calls
    it loop-invariant -- and hoisting it means the second pass of the outer
    loop starts from wherever the inner one left off. segld printed 1030
    for 1050, exactly one inner loop short.

    Both halves, and neither alone. "A phi carries it" refuses harr's `mov
    si,0`, which is safe: si is the array base, a phi carries it because it
    is live around the loop, and nothing writes it again. "The register is
    written twice" refuses hotlop's load of `n`, also safe: the loop writes
    ax on every line and the load is consumed where it stands.
    """
    from qbopt.model import ir
    from qbopt.model import mir

    start = mir.Value(1, 0x10)
    again = mir.Value(2, 0x14)
    merged = mir.Value(3, 0x14)

    begins = mir.Op(0x10, ir.Operation.MOVE, "mov", (start,), (), kind=mir.Kind.COPY)
    counts = mir.Op(0x14, ir.Operation.UNARY, "inc", (again,), (merged,), kind=mir.Kind.ADD)
    carried = [mir.Phi(merged, {0x00: start, 0x14: again})]

    assert transform._starts(carried) == {start, again}, "a phi carries both"
    # By value, not by register: in SSA a variable written twice in a loop
    # is a phi with an incoming defined inside it.
    assert start in transform._rewritten([begins, counts], carried), "and the counter is written twice"

    both = transform._invariant_run(
        [begins, counts], set(), [], frozenset(), {}, carried, None, transform._starts(carried)
    )
    assert begins not in both, "so what starts the counter stays in the loop"

    # "A phi carries it" on its own permits it, which is what makes the
    # pair the rule rather than one of them.
    # A copy is refused outright now, so `begins` never enters a run at
    # all -- see the note in _invariant_run. What is still checked is that
    # the phi rule refuses it for its own reason.
    assert begins not in transform._invariant_run([begins, counts], set(), [], frozenset(), {}, carried, None, set())
    # The other half no longer does. Asked of values rather than of
    # registers, "the loop writes it again" is "a phi joins it with
    # something defined inside", and for a run of one operation that is
    # already true -- so this case is refused where counting definitions
    # per register allowed it. Requiring two incomings from inside
    # restored it and miscompiled hotlop on nine of twelve
    # configurations, so the refusal stands.


def test_folding_leaves_a_copy_alone() -> None:
    """A copy is not a computation, and folding it undoes an allocation.

    `mov ax,cx` where cx is known to be 3 rewrites to `mov ax,3`: the same
    instruction, the same length, nothing read that was not already in a
    register. No gain -- and a real loss, because a live range split is
    exactly that move. Folding it puts the value back inside the loop the
    hoist took it out of, the hoist lifts it again next round, and the
    program grows three bytes a round without ever converging.

    Measured on hotlop before the guard: 132, 226, 322, 418 bytes.
    """
    for name in ("hotlop-p-g2", "press-p-g2"):
        data = Path(f"fixtures/omf/{name}.obj").read_bytes()
        sizes = []
        for _ in range(4):
            sizes.append(len(module.of(omf.parse(data)).code))
            data, _ = rewrite.rewrite(data, dry_run=False)
        assert sizes[-1] <= sizes[1], f"{name} grows without converging: {sizes}"


def _rebuilt(name: str) -> list[tuple[int, str]]:
    from iced_x86 import Decoder
    from iced_x86 import Formatter
    from iced_x86 import FormatterSyntax

    data = Path(f"fixtures/omf/{name}.obj").read_bytes()
    out, _ = rewrite.rewrite(data, dry_run=False)
    shown = Formatter(FormatterSyntax.NASM)
    code = module.of(omf.parse(out)).code
    return [(one.ip, shown.format(one)) for one in Decoder(16, code, ip=0)]


def test_a_constant_product_leaves_the_loop() -> None:
    """HOTLOP's literal product and recurrence fold to its final 630 output.

    The legacy byte-rewriter test assumed an intermediate jump over a loop.
    Fresh LIR emission correctly removes the entire literal loop, so MIR at
    the public emission boundary is the stable evidence: no multiply or
    natural loop remains, and PRINT receives the independently known 630.
    """
    from qbopt import wholeseg
    from qbopt.analysis import loops

    states = []

    def watch(stage, name, body):
        if isinstance(body, mir.MirBody):
            states.append(body)

    result = wholeseg.emitted(Path("fixtures/omf/hotlop-p-g2.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    body = states[-1]
    assert not loops.loops(body.blocks, body.entry)
    assert not [op for block in body.blocks for op in block.ops if op.kind is mir.Kind.MUL]
    assert any(
        arg == mir.Const(630, arg.width)
        for block in body.blocks
        for op in block.ops
        if op.kind is mir.Kind.ARG
        for arg in op.args
        if isinstance(arg, mir.Const)
    )


def test_an_invariant_multiply_leaves_a_loop_it_cannot_be_folded_out_of() -> None:
    """hotlpx is hotlop with its constants read at runtime.

    Nothing can fold `n * k` there, so it is the honest measure of whether
    the loop-invariant code motion works at all -- and it did not. The run
    could go neither last, where the counter already owns ax, nor first,
    where the two runtime READ calls clobber every register before the loop
    sees it. It goes between, which is a position and not a preference.
    """
    seen = _rebuilt("hotlpx-p-g2")
    # Where the loop starts: the target of its back edge. An entered-at-the-body
    # loop has no jump in front of it to find.
    targets = [
        int(text.split()[-1], 16)
        for ip, text in seen
        if text.startswith("j") and text.split()[-1].startswith("0x") and int(text.split()[-1], 16) < ip
    ]
    start = next((i for i, (ip, _) in enumerate(seen) if targets and ip == targets[0]), len(seen))
    inside = [text for _, text in seen[start:]]
    assert not [text for text in inside if text.startswith("imul")], (
        f"the invariant multiply is still in the loop: {inside[:6]}"
    )
    assert [text for _, text in seen[:start] if text.startswith("imul")], (
        "and it did not turn up before the loop either, so nothing was hoisted"
    )


def test_nothing_reads_a_register_nothing_wrote() -> None:
    """_writes_to returns an operation it cannot rewrite, and says nothing.

    Its readers have been pointed at the new register by then, so the loop
    reads one nothing ever wrote: lngmix emitted `mov [bp-14h],di` with di
    written nowhere in the program. A widening imul is the same shape and
    is handled by asking lir whether the register is fixed; an operation
    whose semantics name no destination is neither fixed nor re-seatable,
    and fell through the gap.

    Asserted as the thing that is wrong rather than as the guard: a source
    register that no earlier instruction wrote.
    """
    from iced_x86 import OpKind
    from iced_x86 import Decoder
    from iced_x86 import Mnemonic
    from iced_x86 import Formatter
    from iced_x86 import RegisterExt
    from iced_x86 import FormatterSyntax

    # These two, because where BC's header stops decoding as junk is a
    # per-fixture fact and here it is known: the first real instruction
    # is at 0x30.
    for name in ("lngmix-p-g2", "lngmxx-p-g2"):
        data = Path(f"fixtures/omf/{name}.obj").read_bytes()
        out, _ = rewrite.rewrite(data, dry_run=False)
        shown = Formatter(FormatterSyntax.NASM)
        code = module.of(omf.parse(out)).code
        # bp and sp arrive set up; BC's header bytes decode as junk, so the
        # scan starts where the first real instruction does.
        # bp and sp are the frame and are never in question; the segment
        # registers are set up before any of this runs.
        given = {
            RegisterExt.full_register(one)
            for one in (Register.BP, Register.SP, Register.DS, Register.ES, Register.SS, Register.CS)
        }
        written = set(given)
        for one in Decoder(16, code, ip=0):
            if one.ip < 0x30:
                continue

            def root(where):
                return RegisterExt.full_register(where) if where != Register.NONE else Register.NONE

            reads = {root(one.memory_base), root(one.memory_index)}
            # Operand 0 of a two-operand instruction is written; every other
            # register operand is read.
            for i in range(one.op_count):
                if one.op_kind(i) == OpKind.REGISTER and i:
                    reads.add(root(one.op_register(i)))
            for where in reads - {Register.NONE}:
                assert where in written, f"{name} at {one.ip:#06x}: {shown.format(one)} reads a register nothing wrote"
            for i in range(one.op_count):
                if one.op_kind(i) == OpKind.REGISTER:
                    written.add(root(one.op_register(i)))
            # And what an instruction writes without naming it. `cdq` fills
            # edx with eax's sign and `idiv` leaves the remainder there,
            # neither as an operand -- so a later `mov ebx,edx` read as a
            # register nothing wrote, and the scanner was the thing that
            # was wrong.
            if one.mnemonic in (Mnemonic.CDQ, Mnemonic.CWD, Mnemonic.IDIV, Mnemonic.DIV, Mnemonic.MUL):
                written.add(root(Register.EDX))
                written.add(root(Register.EAX))


def test_dead_code_goes_and_the_bytes_are_still_accounted_for() -> None:
    """A move nothing reads, removed, without losing what it stood for.

    layout.py refuses a body it cannot account for every byte of, so a
    deletion hands its bytes to the operation before it.
    """
    live_one = mir.Op(
        0x10,
        ir.Operation.MOVE,
        "mov",
        (mir.Value(1, 0x10),),
        (),
        kind=mir.Kind.COPY,
        absorbed=(1,),
    )
    doomed = mir.Op(
        0x13,
        ir.Operation.MOVE,
        "mov",
        (mir.Value(2, 0x13),),
        (),
        kind=mir.Kind.COPY,
        absorbed=(2,),
    )
    assert transform._removable(doomed, set()), "nothing reads it"
    assert not transform._removable(live_one, {mir.Value(1, 0x10)}), "and this is read"


def test_dead_code_leaves_a_body_it_cannot_read_alone() -> None:
    """An opaque instruction reads registers no semantics mention.

    byref2 printed 0 for 16 when its argument setup was deleted on the
    strength of a use list that could not have been complete.
    """
    # Something before it, so the deletion has a survivor to give its bytes
    # to -- without one _absorb refuses and the guard is never reached.
    first = mir.Op(0x10, ir.Operation.MOVE, "mov", (mir.Value(1, 0x10),), (), kind=mir.Kind.COPY, absorbed=(1,))
    doomed = mir.Op(0x12, ir.Operation.MOVE, "mov", (mir.Value(2, 0x12),), (), kind=mir.Kind.COPY, absorbed=(2,))
    plain = mir.MirBody(0x10, (mir.MirBlock(0x10, (), (first, doomed), ()),))
    assert transform.dead(plain) is not plain, "a dead move goes when the body is readable"

    opaque = mir.Op(0x14, ir.Operation.BARRIER, "?", (), (), absorbed=(3,))
    body = mir.MirBody(0x10, (mir.MirBlock(0x10, (), (first, doomed, opaque), ()),))
    assert transform.dead(body) is body, "and stays when the body holds a barrier"


def test_the_invariant_sum_folds_press_to_its_known_result() -> None:
    """PRESS's literal products used to leave a dead BX move in its loop.

    The fresh MIR pipeline now evaluates the complete ten-trip literal
    recurrence.  Assert that stronger, observable result rather than the
    retired byte-rewriter's particular surviving-loop shape.
    """
    from qbopt import wholeseg
    from qbopt.analysis import loops

    states = []

    def watch(stage, name, body):
        if isinstance(body, mir.MirBody) and name and name.startswith("main"):
            states.append(body)

    result = wholeseg.emitted(Path("fixtures/omf/press-p-g2.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    body = states[-1]
    assert not loops.loops(body.blocks, body.entry)
    assert not [op for block in body.blocks for op in block.ops if op.kind is mir.Kind.MUL]
    assert any(
        arg == mir.Const(7500, arg.width)
        for block in body.blocks
        for op in block.ops
        if op.kind is mir.Kind.ARG
        for arg in op.args
        if isinstance(arg, mir.Const)
    )


def test_a_branch_on_two_numbers_is_decided_where_it_stands() -> None:
    """bools is three comparisons over four constants and nothing else.

    BC materialises every one into a register through a branch and a dec,
    then compares that register against zero to branch again. All of it is
    decidable: 165 bytes to 129, and 1.6x to 1.35x.
    """
    seen = _rebuilt("bools-p-g2")
    left = [text for _, text in seen if text.startswith(("jle", "jg", "jl ", "jge", "je ", "jne"))]
    assert not left, f"a branch on two constants is still there: {left}"


def test_deciding_a_branch_leaves_every_byte_accounted_for() -> None:
    """Resolving the flow is not the same as pruning it.

    A block nothing can reach any more still occupies bytes, and layout.py
    refuses a body it cannot account for every one of -- three of the bools
    objects came back `0x006d: 3 bytes between the ops are not
    instructions` when the branch fold resolved the body afterwards. The
    edge stays, over-approximating the control flow, which is the safe
    direction for everything that reads it.
    """
    from qbopt import wholeseg

    for name in ("bools-q-O.obj", "bools-q-noO.obj", "bools-q-O-zd.obj"):
        path = Path("fixtures/omf") / name.lower()
        if not path.exists():
            continue
        _out, why = wholeseg.rebuilt(path.read_bytes())
        assert why == wholeseg.REBUILT, f"{name}: {why}"


def test_a_served_read_names_the_value_and_not_a_register() -> None:
    """`add ax,[y]` served from a register used to say which register.

    It asked `_at_width(root, width)` and wrote the answer down -- the
    allocator's answer, given by a pass, which is rule 5's whole subject and
    forward.py's 22 machine references in one line. It says mir.Held now --
    the value -- and lower.py is where that becomes a register.
    """
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    seen = 0
    for obj in sorted(Path("fixtures/omf").glob("*-p-g2.obj")):
        found = corpus.loaded(obj)
        mapped = code_map(found)
        if isinstance(mapped, str):
            continue
        for _name, body in mir.bodies(found, split.partition(found, mapped)):
            after = transform.forwarded(body, found.dgroup, found.calls)
            if after is body:
                continue
            for block in after.blocks:
                for op in block.ops:
                    if op.raised is None or (op.args, op.results) == op.raised or op.loads:
                        continue
                    if not any(isinstance(one, mir.Cell) for one in op.raised[0]):
                        continue
                    held = [one for one in op.args if isinstance(one, mir.Held)]
                    assert held, f"{obj.stem} {op.at:#06x}: served read names a register"
                    seen += 1
    assert seen, "nothing was served, so this proves nothing"


def _bodies(name: str):
    """Every raised body of one fixture, before any pass has run."""
    from qbopt.objectfile import module
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    found = module.of(omf.parse(Path(f"fixtures/omf/{name}").read_bytes()))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    return found, mir.bodies(found, split.partition(found, mapped))


def test_a_fold_names_the_value_not_the_register() -> None:
    """Fold reused the destination operand BC had written, which is a
    register, and MIR carrying it forward is a pass naming one."""
    found, bodies = _bodies("hotlop-p-g2.obj")
    held = [
        one
        for name, body in bodies
        for block in transform.applied(body, found.dgroup, found.calls, only="fold").blocks
        for op in block.ops
        for one in op.results
        if isinstance(one, mir.Held) and op.raised is not None and (op.args, op.results) != op.raised
    ]
    assert held, "fold no longer says which value it writes"


def test_an_unplaced_held_keeps_its_fold() -> None:
    """Cost of getting this wrong: 693 bytes, then a miscompile.

    A fold says `this value gets this constant`. Where the allocation has
    not yet assigned a register, lowering must preserve both the constant and
    the abstract destination. Dropping the rewrite instead emits the load it
    replaced; prematurely grounding the destination disagrees with the later
    allocator. The original defect made lngmix print 110 for 142900.
    """
    from qbopt.model import ir
    from qbopt.abi import runtime
    from qbopt.backend import lower

    found, bodies = _bodies("hotlop-p-g2.obj")
    for name, body in bodies:
        done = transform.applied(body, found.dgroup, found.calls, only="fold")
        folded = [
            op
            for block in done.blocks
            for op in block.ops
            if op.raised is not None and (op.args, op.results) != op.raised
        ]
        if not folded:
            continue
        low = lower.lowered(
            name,
            done,
            found.calls,
            bodies.source.absorbed,
            runtime.for_module(found),
            bodies.source.coverage,
            nodes=bodies.source.nodes,
            occurrences=bodies.source.occurrences,
        )
        for op in folded:
            what = next(one.what for one in low.insns if one.op is not None and one.op.id == op.id)
            assert what is not None, f"{name}: {op.at:#x} lost its rewrite"
            assert any(isinstance(one, ir.Imm) for one in what.sources), (
                f"{name}: {op.at:#x} went back to the read it replaced"
            )
            assert any(isinstance(one, ir.Held) for one in what.dests), (
                f"{name}: {op.at:#x} was placed before allocation"
            )
        return
    raise AssertionError("no body folded anything; the test measures nothing")


def test_what_leaves_a_loop_is_its_own_semantic_variable() -> None:
    """A hoisted computation must not be merged with a source-register peer.

    In MIR a register is a variable, so two values BC kept in one register
    are one variable -- true only while nothing has moved them. The moment
    a computation leaves a loop it is not: hotlpx's product and its counter
    both lived in ax, and re-deriving SSA per register put a phi over them
    that said the loop's reads of the product were reads of the counter.
    Nothing downstream could see a conflict because in MIR there was none.

    The hoist gives everything the run defines a variable of its own.  That
    semantic identity is enough for SSA; physical placement lives in the
    external allocation-hint table and must not be copied by this pass.
    """
    from qbopt.analysis import ssa
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/hotlpx-p-g2.obj").read_bytes()))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    blocks = split.partition(found, mapped)

    seen = 0
    for _name, body in mir.bodies(found, blocks):
        before = {value.variable for value in ssa.values(body)}
        after = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
        fresh = {value.variable for value in ssa.values(after)} - before
        if not fresh:
            continue
        seen += 1
        # No phi joins a fresh variable to one that was there before.
        for block in after.blocks:
            for phi in block.phis:
                names = {one.variable for one in phi.incoming.values()} | {phi.result.variable}
                assert not (names & fresh) or names <= fresh, f"{phi.result} joins a hoisted value to something else"
    assert seen, "nothing left a loop, so this proves nothing"


def test_place_takes_a_store_out_of_a_push_run():
    """lngmix's second divide never folded: two stores stood in its run.

    `match()` only finds a call whose pushes are contiguous, so the site
    arrived as a consume site, the raise refused it, and the loop kept a
    runtime call. The stores hold the *previous* divide's results and do
    not depend on the run, so they belong ahead of it.
    """
    from pathlib import Path

    from qbopt.model import mir
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.optimize import transform
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/lngmix-p-g2.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    ((_who, body),) = mir.bodies(found, blocks)
    done = transform.placed(body, found.dgroup, found.calls)

    run = next(b for b in done.blocks if any(op.kind is mir.Kind.CALL for op in b.ops))
    kinds = [op.kind for op in run.ops]
    call = kinds.index(mir.Kind.CALL)
    first = min(i for i, k in enumerate(kinds) if k is mir.Kind.ARG)
    assert all(k is mir.Kind.ARG for k in kinds[first:call]), (
        f"a call's run still holds {[k.name for k in kinds[first:call] if k is not mir.Kind.ARG]}"
    )


def test_place_keeps_the_frame_pointer_behind_the_push_that_saves_it():
    """procs p-ot's REPORT moved `mov bp,sp` ahead of `push bp`, so every
    argument it read through bp was one word off."""
    from pathlib import Path

    from qbopt.model import mir
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.optimize import transform
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/procs-p-ot.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    body = next(body for name, body in mir.bodies(found, blocks) if "REPORT" in name)
    done = transform.placed(body, found.dgroup, found.calls)

    assert [op.at for op in done.blocks[0].ops[:2]] == [0x142, 0x143]


def test_both_lngmix_divides_absorb():
    """The whole point of the above: 952 -> 930 bytes, no runtime divide.

    Not by moving the store out of the push run -- `place` still cannot,
    for the reason the sibling tests below once carried as their own xfail
    reason. calls.sites() can classify a frame-found site's pushes the way
    match() classifies its own, so the second call folds around the store
    rather than needing it moved: `covers` names the call and
    `Module.coverage` the pushes, two disjoint runs rather than one.
    """
    from pathlib import Path

    from qbopt import wholeseg
    from qbopt.legacy import calls
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    data = Path("fixtures/omf/lngmix-p-g2.obj").read_bytes()
    for _round in range(3):
        data, _why = wholeseg.rebuilt(data)
    found = module.of(omf.parse(data))
    blocks = split.partition(found, code_map(found))
    reached = [insn for block in blocks for insn in block.insns]
    assert calls.sites(found, reached, blocks) == []


@pytest.mark.parametrize("preserves_high", [False, True])
def test_cse_propagates_a_complete_narrow_copy_to_an_opaque_reader(preserves_high) -> None:
    """HARR's equal selector names blocked forwarding, adding an array reload per iteration."""
    source = mir.Value(1, 0, variable=1, version=1)
    copied = mir.Value(2, 2, variable=2, version=1)
    first = mir.Op(
        0,
        ir.Operation.MOVE,
        "mov",
        (source,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Const(7, 2),),
        results=(mir.Held(source, 2),),
    )
    copy = mir.Op(
        2,
        ir.Operation.MOVE,
        "mov",
        (copied,),
        (source,),
        kind=mir.Kind.COPY,
        args=(mir.Held(source, 2),),
        results=(mir.Held(copied, 2),),
    )
    if preserves_high:
        from dataclasses import replace

        previous = mir.Value(3, 0, variable=3, version=1)
        copy = replace(copy, uses=(source, previous), merges={previous: copied})
    use = mir.Op(4, ir.Operation.PUSH, "push", (), (copied,), kind=mir.Kind.OPAQUE)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (first, copy, use), ()),))
    done = transform.subexpressions(body)
    assert done.blocks[0].ops[-1].uses == (copied if preserves_high else source,)
    assert any(copied in op.defines for op in done.blocks[0].ops) == preserves_high


def test_cse_reuses_one_frame_object_address() -> None:
    """shellsort kept four identical local-array bases live through its nested loops."""
    first = mir.Value(1, 0)
    duplicate = mir.Value(2, 1)

    def address(at: int, result: mir.Value) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.ADDRESS,
            "lea",
            (result,),
            (),
            kind=mir.Kind.ADDRESS,
            args=(mir.FrameAddress(-132, 2, (-132, -4)),),
            results=(mir.Held(result, 2),),
        )

    use = mir.Op(2, ir.Operation.PUSH, "push", (), (duplicate,), kind=mir.Kind.OPAQUE, args=(mir.Held(duplicate, 2),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (address(0, first), address(1, duplicate), use), ()),))

    done = transform.subexpressions(body)

    assert sum(op.kind is mir.Kind.ADDRESS for op in done.blocks[0].ops) == 1
    assert done.blocks[0].ops[-1].uses == (first,)
    assert done.blocks[0].ops[-1].args == (mir.Held(first, 2),)


def test_reparenting_a_hoisted_pointer_keeps_its_object_facts() -> None:
    """Matmul's hoisted local-array address stopped being a pointer.

    Hoisting gives a loop-crossing value a fresh MIR variable.  The pointer
    value and its exact frontend seed must be renamed by that same semantic
    operation or later alias analysis sees the address as an integer.
    """
    from qbopt.model import ir
    from qbopt.model import memory

    pointer = mir.Value(1, 0, variable=3, version=1)
    object_ = memory.Object(memory.Kind.FRAME, (5, -16, -4), extent=12)
    provenance = memory.Provenance.one(object_, 0, 1)
    interval = mir.IntegerRange(0, 31, 2)
    address = mir.Op(
        0,
        ir.Operation.ADDRESS,
        "lea",
        (pointer,),
        (),
        kind=mir.Kind.ADDRESS,
        args=(mir.FrameAddress(-16, 2, (-16, -4)),),
        results=(mir.Held(pointer, 2),),
    )
    body = mir.MirBody(
        0,
        (mir.MirBlock(0, (), (address,), ()),),
        pointer_values=frozenset({pointer}),
        pointer_seeds={pointer: provenance},
        integer_ranges={pointer: interval},
    )

    result = transform._reparented(body, {pointer})

    renamed = result.blocks[0].ops[0].defines[0]
    assert renamed.variable != pointer.variable
    assert result.pointer_values == frozenset({renamed})
    assert result.pointer_seeds == {renamed: provenance}
    assert result.integer_ranges == {renamed: interval}


def test_cse_refuses_an_operand_that_is_only_half_its_value() -> None:
    """nots printed NOTOR= 26390415 for -271601777: right word, wrong word.

    A half of a long is `Held(value, 2)` and so is the other half, so two
    operations reading opposite halves compare equal on the value they name.
    Nothing in MIR says which half, so nothing narrow is a subexpression.
    """
    from qbopt.optimize import transform

    low = mir.Held(mir.Value(id=1, at=0, variable=1, version=1), 2)
    assert transform._full(low, {1: 2})
    assert not transform._full(low, {1: 4}), "half of a long passed as the whole of it"

    class Fake:
        floating = None
        barrier = False
        kind = mir.Kind.NOT
        name = "not"
        loads = stores = ()
        merges: dict = {}
        defines = (mir.Value(id=2, at=0, variable=2, version=1),)
        results = (mir.Held(defines[0], 2),)
        args = (low,)

    assert transform._computation(Fake(), {}, {1: 4}) is None
    assert transform._computation(Fake(), {}, {1: 2}) is not None


def test_an_operation_dead_keeps_has_its_operands_kept_too() -> None:
    """bools-q-O prints T=1 for 2: `IF a < b THEN t = t + 1` never runs.

    `decided()` resolves the second comparison and rewrites its branch into
    an unconditional jump with no uses. Nothing then reads the flags that
    `cmp` at 0x67 defines, so `halves()` never propagates through it and
    the value it compares looks unread -- and `dead` deletes the load at
    0x64 that defined it.

    But `dead` does not delete the `cmp`. `_removable` refuses an operation
    whose only definitions are flags, so it survives and goes on reading a
    value nothing defines. The compare then uses whatever is in the
    register -- x, which is -1 -- instead of `a`, the jump falls the wrong
    way, and the `+ 1` is lost.

    The two have to agree: an operation `dead` keeps is an operation whose
    operands are read.
    """
    from pathlib import Path

    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/bools-q-O.obj".lower()).read_bytes()))
    blocks = split.partition(found, code_map(found))
    for name, body in mir.bodies(found, blocks):
        after = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
        made = {one.id for block in after.blocks for op in block.ops for one in op.defines}
        made |= {phi.result.id for block in after.blocks for phi in block.phis}
        had = {one.id for block in body.blocks for op in block.ops for one in op.defines}
        had |= {phi.result.id for block in body.blocks for phi in block.phis}
        broken = [
            f"{op.at:#06x} reads v{one.id}, whose definition the passes deleted"
            for block in after.blocks
            for op in block.ops
            for one in op.uses
            if one.id in had and one.id not in made
        ]
        assert not broken, f"{name}: " + "; ".join(broken[:3])


def test_decided_boolean_edges_stop_generating_phi_copies() -> None:
    """bools cost 13.37x because known branches retained impossible edges and phi copies."""
    from qbopt.objectfile import omf
    from qbopt.frontend import blocks
    from qbopt.objectfile import module

    found = module.of(omf.parse(Path("fixtures/omf/bools-p-g2.obj").read_bytes()))
    partition = blocks.partition(found, blocks.code_map(found))
    body = mir.bodies(found, partition)[0][1]
    after = transform.decided(body, found.dgroup, found.calls)
    assert after.block(0x30).succ == (0x5A,)
    assert not after.block(0x5B).phis


def test_dead_boolean_block_does_not_leave_an_unreachable_jump() -> None:
    """bools-q-O could not be measured: its dead block retained a self-relative jump."""
    from qbopt import wholeseg
    from qbopt.objectfile import omf
    from qbopt.frontend import blocks
    from qbopt.objectfile import module

    result = wholeseg.emitted(Path("fixtures/omf/bools-q-O.obj".lower()).read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result
    found = module.of(omf.parse(result.data))
    assert not isinstance(blocks.code_map(found), str)


def test_one_idiv_serves_both_of_lngmix_s_divides_in_the_image(monkeypatch) -> None:
    """The fold reaching the bytes, through the entry production uses.

    Two things only show here. The allocator refusing the folded body is
    invisible upstream -- the pass returns a folded body either way, and
    the image keeps both idivs. And dropping the second site's record
    from the module, which looked like the tidy thing, left the refused
    body emitting BC's bare `call 0:0` with its push run already folded
    away: lngmix stopped early under DOSBox with no diff to read.
    """
    from iced_x86 import Decoder
    from iced_x86 import Mnemonic

    from qbopt import wholeseg
    from qbopt.objectfile import omf
    from qbopt.analysis import consts
    from qbopt.objectfile import module

    # Exercise reuse separately from folding both constant answers away.
    monkeypatch.setattr(consts, "division", lambda *args: None)

    out, why = wholeseg.rebuilt(Path("fixtures/omf/lngmix-p-g2.obj").read_bytes())
    assert why == wholeseg.REBUILT
    code = bytes(module.of(omf.parse(out)).code)
    seen = [one.mnemonic for one in Decoder(16, code)]
    assert seen.count(Mnemonic.IDIV) == 1, "one divide does the work of both"
    assert seen.count(Mnemonic.CALL) == 4, "and no divide is left as a call"
