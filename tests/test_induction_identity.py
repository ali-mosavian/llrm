from dataclasses import replace

import pytest

from qbopt import ir
from qbopt import mir
from qbopt import ssa
from qbopt import loops
from qbopt import strength
from qbopt import transform
from qbopt import induction
from qbopt.module import Addr
from qbopt.module import Space


def body() -> tuple[mir.MirBody, loops.Loop]:
    start = mir.Value(10, 0, variable=7)
    counter = mir.Value(11, 1, variable=7)
    following = mir.Value(12, 1, variable=7)
    unrelated = mir.Value(13, 1, variable=7)
    answer = mir.Value(14, 1, variable=8)
    step = mir.Op(
        1,
        ir.Operation.UNARY,
        "",
        (following,),
        (counter,),
        kind=mir.Kind.INCREMENT,
        args=(mir.Held(counter, 2),),
        results=(mir.Held(following, 2),),
    )
    multiply = mir.Op(
        2,
        ir.Operation.BINARY,
        "",
        (answer,),
        (unrelated,),
        kind=mir.Kind.MUL,
        args=(mir.Held(unrelated, 2), mir.Const(2, 2)),
        results=(mir.Held(answer, 2),),
    )
    blocks = (
        mir.MirBlock(0, (), (), (1,)),
        mir.MirBlock(1, (mir.Phi(counter, {0: start, 1: following}),), (step, multiply), (1, 2)),
        mir.MirBlock(2, (), (), ()),
    )
    return mir.MirBody(0, blocks), loops.Loop(1, frozenset({1}), frozenset({1}))


def test_a_shared_variable_name_is_not_a_shared_recurrence() -> None:
    """harr's shifted address was mistaken for both loop counters because all shared a variable number."""
    built, loop = body()
    assert induction.basics(built, loop)
    assert not induction.derived(built, loop)
    header = built.blocks[1]
    counter = header.phis[0].result
    multiply = replace(header.ops[1], uses=(counter,), args=(mir.Held(counter, 2), mir.Const(2, 2)))
    positive = replace(built, blocks=(built.blocks[0], replace(header, ops=(header.ops[0], multiply)), built.blocks[2]))
    assert len(induction.derived(positive, loop)) == 1


def test_the_backedge_must_step_the_exact_phi_value() -> None:
    built, loop = body()
    header = built.blocks[1]
    unrelated = header.ops[1].args[0]
    step = replace(header.ops[0], args=(unrelated,), uses=(unrelated.value,))
    built = replace(built, blocks=(built.blocks[0], replace(header, ops=(step, header.ops[1])), built.blocks[2]))
    assert not induction.basics(built, loop)


@pytest.mark.parametrize(("factor", "expected"), [(20, 42), (32767, 0)])
def test_composed_word_address_has_one_recurrence(factor: int, expected: int) -> None:
    """Matrix's (i * 20 + i) << 1 needs stride 42, not a separate stride-20 temporary."""
    built, loop = body()
    header = built.blocks[1]
    counter = header.phis[0].result
    product = header.ops[1].results[0].value
    summed = mir.Value(30, 1, variable=30)
    address = mir.Value(31, 1, variable=31)
    factor_value = mir.Value(32, 0, variable=32)
    constant = replace(
        header.ops[1],
        kind=mir.Kind.COPY,
        defines=(factor_value,),
        uses=(),
        args=(mir.Const(factor, 2),),
        results=(mir.Held(factor_value, 2),),
    )
    multiply = replace(
        header.ops[1], uses=(counter, factor_value), args=(mir.Held(counter, 2), mir.Held(factor_value, 2))
    )
    add = replace(
        multiply,
        kind=mir.Kind.ADD,
        defines=(summed,),
        uses=(product, counter),
        args=(mir.Held(product, 2), mir.Held(counter, 2)),
        results=(mir.Held(summed, 2),),
    )
    shift = replace(
        multiply,
        kind=mir.Kind.SHL,
        defines=(address,),
        uses=(summed,),
        args=(mir.Held(summed, 2), mir.Const(1, 2)),
        results=(mir.Held(address, 2),),
    )
    built = replace(
        built,
        blocks=(
            replace(built.blocks[0], ops=(constant,)),
            replace(header, ops=(*header.ops[:1], multiply, add, shift)),
            built.blocks[2],
        ),
    )
    derived = induction.derived(built, loop)
    assert next(one.by for one in derived if one.op is shift) == mir.Const(expected, 2)


@pytest.mark.parametrize("mismatch", ["start", "step", "unchanged", "none"])
def test_every_incoming_path_agrees_on_the_recurrence(mismatch: str) -> None:
    built, loop = body()
    header = built.blocks[1]
    phi = header.phis[0]
    start = phi.incoming[0]
    following = mir.Value(20, 3, variable=7)
    step = replace(header.ops[0], at=3, defines=(following,), results=(mir.Held(following, 2),))
    if mismatch == "step":
        step = replace(step, kind=mir.Kind.DECREMENT)
    incoming = {
        **phi.incoming,
        4: mir.Value(21, 4, variable=7) if mismatch == "start" else start,
        3: phi.result if mismatch == "unchanged" else following,
    }
    built = replace(
        built,
        blocks=(
            built.blocks[0],
            replace(header, phis=(replace(phi, incoming=incoming),), succ=(1, 2, 3)),
            built.blocks[2],
            mir.MirBlock(3, (), (step,), (1,)),
            mir.MirBlock(4, (), (), (1,)),
        ),
    )
    loop = replace(loop, body=frozenset({1, 3}), latches=frozenset({1, 3}))
    assert bool(induction.basics(built, loop)) == (mismatch == "none")


@pytest.mark.parametrize(("width", "count"), [(2, 1), (4, 0)])
def test_only_width_preserving_copies_carry_the_recurrence(width: int, count: int) -> None:
    built, loop = body()
    header = built.blocks[1]
    counter = header.phis[0].result
    copied = header.ops[1].args[0].value
    copy = mir.Op(
        1,
        ir.Operation.MOVE,
        "",
        (copied,),
        (counter,),
        kind=mir.Kind.COPY,
        args=(mir.Held(counter, width),),
        results=(mir.Held(copied, 2),),
    )
    built = replace(built, blocks=(built.blocks[0], replace(header, ops=(copy, *header.ops)), built.blocks[2]))
    assert len(induction.derived(built, loop)) == count


@pytest.mark.parametrize("shape", ["variable", "counter_count", "constant", "oversized"])
def test_a_shift_recurrence_requires_a_constant_count(shape: str) -> None:
    built, loop = body()
    header = built.blocks[1]
    counter = mir.Held(header.phis[0].result, 2)
    amount = mir.Const(3, 2) if shape == "constant" else mir.Held(header.phis[0].incoming[0], 2)
    if shape == "oversized":
        amount = mir.Const(32, 2)
    args = (mir.Const(3, 2), counter) if shape == "counter_count" else (counter, amount)
    shift = replace(
        header.ops[1],
        kind=mir.Kind.SHL,
        args=args,
        uses=tuple(one.value for one in args if isinstance(one, mir.Held)),
    )
    built = replace(built, blocks=(built.blocks[0], replace(header, ops=(header.ops[0], shift)), built.blocks[2]))
    derived = induction.derived(built, loop)
    assert len(derived) == (1 if shape == "constant" else 0)
    if derived:
        assert derived[0].by == mir.Const(8, 2)


@pytest.mark.parametrize("address", [False, True])
def test_a_reduced_counter_has_its_own_loop_phi_and_fresh_variable(address: bool) -> None:
    """harr's experimental stride reused a promoted variable and read its initial value on every iteration."""
    built, _loop = body()
    header = built.blocks[1]
    counter = header.phis[0].result
    answer = header.ops[1].defines[0]
    multiply = replace(header.ops[1], uses=(counter,), args=(mir.Held(counter, 2), mir.Const(2, 2)), covers=(2, 4))
    livein = mir.Value(99, 0, variable=77)
    use = mir.Op(
        4,
        ir.Operation.PUSH,
        "",
        (),
        (answer, livein),
        kind=mir.Kind.ARG,
        args=(mir.Held(answer, 2), mir.Held(livein, 2)),
        covers=(4, 6),
    )
    if address:
        memory = mir.MemRef(Addr(Space.SEGMENT, 0x20, 1), 2, answer)
        use = replace(use, args=(mir.Cell(memory), mir.Held(livein, 2)), loads=(memory,))
    built = replace(
        built,
        blocks=(
            built.blocks[0],
            replace(header, ops=(replace(header.ops[0], covers=(1, 2)), multiply, use)),
            built.blocks[2],
        ),
    )
    result = strength.reduced(built)
    after = result.blocks[1]
    added = [phi for phi in after.phis if phi.result != counter]
    assert len(added) == 1
    phi = added[0]
    assert phi.result.variable > livein.variable
    consumer = next(op for op in after.ops if op.kind is mir.Kind.ARG)

    def source(value: mir.Value) -> mir.Value:
        definition = next((op for op in after.ops if value in op.defines), None)
        return definition.args[0].value if definition is not None and definition.kind is mir.Kind.COPY else value

    if address:
        assert source(consumer.args[0].ref.base) == phi.result
        assert source(consumer.loads[0].base) == phi.result
    else:
        assert source(consumer.args[0].value) == phi.result
    assert phi.incoming[0] != phi.incoming[1]
    step = next(op for op in after.ops if phi.incoming[1] in op.defines)
    assert step.args[0].value == phi.result


@pytest.mark.parametrize("use", ["low", "high", "both_through_phis"])
def test_reduction_preserves_every_live_product_result(use: str) -> None:
    built, _loop = body()
    header = built.blocks[1]
    low = header.ops[1].defines[0]
    high = mir.Value(30, 1, variable=9)
    middle = mir.Value(31, 2, variable=9)
    final = mir.Value(32, 3, variable=9)
    product = replace(header.ops[1], defines=(low, high), results=(mir.Held(low, 2), mir.Held(high, 2)))
    values = (low,) if use == "low" else (high,) if use == "high" else (low, final)
    consume = mir.Op(
        3, ir.Operation.PUSH, "", (), values, kind=mir.Kind.ARG, args=tuple(mir.Held(value, 2) for value in values)
    )
    built = replace(
        built,
        blocks=(
            built.blocks[0],
            replace(header, ops=(header.ops[0], product)),
            mir.MirBlock(2, (mir.Phi(middle, {1: high}),), (), (3,)),
            mir.MirBlock(3, (mir.Phi(final, {2: middle}),), (consume,), ()),
        ),
    )
    assert strength._answer(built, product) == (low if use == "low" else None)


@pytest.mark.parametrize("bypass", [False, True])
def test_reduction_does_not_speculate_on_a_loop_bypass(bypass: bool) -> None:
    built, _loop = body()
    header = built.blocks[1]
    counter = header.phis[0].result
    answer = header.ops[1].defines[0]
    memory = mir.MemRef(Addr(Space.SEGMENT, 0x20, 1), 2)
    product = replace(
        header.ops[1], uses=(counter,), args=(mir.Held(counter, 2), mir.Cell(memory)), loads=(memory,), covers=(2, 4)
    )
    consume = mir.Op(
        4, ir.Operation.PUSH, "", (), (answer,), kind=mir.Kind.ARG, args=(mir.Held(answer, 2),), covers=(4, 6)
    )
    built = replace(
        built,
        blocks=(
            replace(built.blocks[0], succ=(1, 2) if bypass else (1,)),
            replace(header, ops=(replace(header.ops[0], covers=(1, 2)), product, consume)),
            built.blocks[2],
        ),
    )
    assert (strength.reduced(built) == built) == bypass


def test_existing_phi_inputs_follow_their_predecessor_versions() -> None:
    initial = mir.Value(10, 0, variable=7)
    updated = mir.Value(11, 1, variable=7)
    joined = mir.Value(12, 2, variable=8)
    define = mir.Op(
        0,
        ir.Operation.MOVE,
        "",
        (initial,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Const(1, 2),),
        results=(mir.Held(initial, 2),),
    )
    step = replace(
        define,
        at=1,
        defines=(updated,),
        uses=(initial,),
        kind=mir.Kind.ADD,
        args=(mir.Held(initial, 2), mir.Const(1, 2)),
        results=(mir.Held(updated, 2),),
    )
    built = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (define,), (1, 2)),
            mir.MirBlock(1, (), (step,), (2,)),
            mir.MirBlock(2, (mir.Phi(joined, {0: initial, 1: initial}),), (), ()),
        ),
    )
    result = ssa.constructed(built, frozenset({7}))
    phi = result.blocks[2].phis[0]
    assert phi.result == joined
    assert phi.incoming[0] == result.blocks[0].ops[0].defines[0]
    assert phi.incoming[1] == result.blocks[1].ops[0].defines[0]
    assert [len(block.ops) for block in result.blocks] == [1, 1, 0]


def test_reduced_product_keeps_the_current_iteration_on_exit() -> None:
    built, _loop = body()
    header = built.blocks[1]
    counter = header.phis[0].result
    product = replace(header.ops[1], uses=(counter,), args=(mir.Held(counter, 2), mir.Const(3, 2)), covers=(2, 4))
    answer = product.defines[0]
    exit_value = mir.Value(40, 2, variable=10)
    consume = mir.Op(5, ir.Operation.PUSH, "", (), (exit_value,), kind=mir.Kind.ARG, args=(mir.Held(exit_value, 2),))
    built = replace(
        built,
        blocks=(
            built.blocks[0],
            replace(header, ops=(replace(header.ops[0], covers=(1, 2)), product)),
            mir.MirBlock(2, (mir.Phi(exit_value, {1: answer}),), (consume,), ()),
        ),
    )
    result = strength.reduced(built)
    after = result.blocks[1]
    phi = next(one for one in after.phis if one.result != counter)
    preserved = next((op for op in after.ops if answer in op.defines), None)
    assert preserved is not None
    assert preserved.kind is mir.Kind.COPY
    assert preserved.args == (mir.Held(phi.result, 2),)
    assert result.blocks[2].phis[0].incoming[1] == answer
    assert preserved.args[0].value != phi.incoming[1]


def test_inserted_counter_operations_own_their_insertion_location() -> None:
    built, _loop = body()
    header = built.blocks[1]
    counter = header.phis[0].result
    product = replace(header.ops[1], uses=(counter,), args=(mir.Held(counter, 2), mir.Const(3, 2)), covers=(2, 4))
    answer = product.defines[0]
    consume = mir.Op(
        4, ir.Operation.PUSH, "", (), (answer,), kind=mir.Kind.ARG, args=(mir.Held(answer, 2),), covers=(4, 6)
    )
    built = replace(
        built,
        blocks=(
            built.blocks[0],
            replace(header, ops=(replace(header.ops[0], covers=(1, 2)), product, consume)),
            built.blocks[2],
        ),
    )
    result = strength.reduced(built)
    setup = result.blocks[0].ops[0]
    update = result.blocks[1].ops[-1]
    assert setup.at == 0 and setup.covers == (0, 0)
    assert update.at == 4 and update.covers == (6, 6)


@pytest.mark.parametrize("second_variable", [7, 8])
@pytest.mark.parametrize("has_origin", [False, True])
@pytest.mark.parametrize("symbolic", [False, True])
def test_cse_replaces_phi_uses_of_a_deleted_initializer(second_variable: int, has_origin: bool, symbolic: bool) -> None:
    """matrix printed T=0 for T=380 after CSE deleted a zero still named by its loop phi."""
    from pathlib import Path

    import corpus

    node = next(
        node
        for body in corpus.bodies(Path("fixtures/omf/matrix-p-g2.obj"))
        for node in body.nodes
        if isinstance(node, ir.Opaque)
    )
    first = mir.Value(10, 0, variable=7)
    second = mir.Value(11, 2, variable=second_variable)
    result = mir.Value(12, 4, variable=7)
    define = mir.Op(
        0,
        ir.Operation.MOVE,
        "",
        (first,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Symbol(Space.SEGMENT, 5, 6, 2) if symbolic else mir.Const(0, 2),),
        results=(mir.Held(first, 2),),
        covers=(0, 2),
        node=node if has_origin else None,
    )
    duplicate = replace(define, at=2, defines=(second,), results=(mir.Held(second, 2),), covers=(2, 4))
    jump = mir.Op(4, ir.Operation.JUMP, "", (), (), kind=mir.Kind.JUMP, covers=(4, 6), target=6)
    use = mir.Op(6, ir.Operation.PUSH, "", (), (result,), kind=mir.Kind.ARG, args=(mir.Held(result, 2),))
    direct = replace(use, at=7, uses=(second,), args=(mir.Held(second, 2),))
    built = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (define,), (2,)),
            mir.MirBlock(2, (), (duplicate, jump), (6,)),
            mir.MirBlock(6, (mir.Phi(result, {2: second}),), (use, direct), ()),
        ),
    )
    after = transform.subexpressions(built)
    assert all(second not in op.defines for block in after.blocks for op in block.ops)
    assert after.blocks[2].phis[0].incoming[2] == first
    assert after.blocks[2].ops[1].args == (mir.Held(first, 2),)


@pytest.mark.parametrize("other", [mir.Const(0, 2), mir.Symbol(Space.SEGMENT, 5, 8, 2)])
def test_cse_keeps_distinct_linker_addresses(other: mir.Arg) -> None:
    """Descriptor addresses encoded as zero must not become the same value."""
    first, second = mir.Value(100, 0), mir.Value(101, 2)
    define = mir.Op(
        0,
        ir.Operation.MOVE,
        "mov",
        (first,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Symbol(Space.SEGMENT, 5, 6, 2),),
        results=(mir.Held(first, 2),),
        covers=(0, 2),
    )
    different = replace(define, at=2, defines=(second,), args=(other,), results=(mir.Held(second, 2),), covers=(2, 4))
    use = mir.Op(4, ir.Operation.PUSH, "push", (), (second,), kind=mir.Kind.ARG, args=(mir.Held(second, 2),))
    built = mir.MirBody(0, (mir.MirBlock(0, (), (define, different, use), ()),))
    assert transform.subexpressions(built) == built


def test_dead_byte_transfer_cannot_span_a_surviving_jump() -> None:
    """matrix refused emission after dead assigned the live jump's nine bytes twice."""
    from pathlib import Path

    import corpus

    node = next(
        node
        for body in corpus.bodies(Path("fixtures/omf/matrix-p-g2.obj"))
        for node in body.nodes
        if isinstance(node, ir.Opaque)
    )
    first = mir.Op(0, ir.Operation.MOVE, "", (), (), kind=mir.Kind.COPY, node=node, covers=(0, 4))
    removed = replace(first, at=4, covers=(8, 10))
    jump = replace(first, at=4, kind=mir.Kind.JUMP, covers=(4, 8))
    result = transform._without([first, removed, jump], lambda op: op is removed)
    spans = sorted(op.covers for op in result if op.covers)
    assert all(left[1] <= right[0] for left, right in zip(spans, spans[1:]))
