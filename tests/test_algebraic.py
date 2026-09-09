from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt import ir
from qbopt import mir
from qbopt import algebraic
from qbopt import transform


@pytest.mark.parametrize("divisor", [16, 512, 262144])
@pytest.mark.parametrize("immediate", [False, True])
def test_signed_power_division_preserves_quotient_and_remainder(divisor: int, immediate: bool) -> None:
    """Nbody paid for IDIV by fixed scales; negative deltas require truncation, not flooring."""
    source, constant, quotient, remainder = (mir.Value(index, 0) for index in range(1, 5))
    copy = mir.Op(0, ir.Operation.MOVE, "mov", (constant,), (), kind=mir.Kind.COPY,
                  args=(mir.Const(divisor, 4),), results=(mir.Held(constant, 4),))
    divide = mir.Op(1, ir.Operation.DIVIDE, "idiv", (quotient, remainder), (source, constant),
                    kind=mir.Kind.DIVMOD, args=(mir.Held(source, 4), mir.Held(constant, 4)),
                    results=(mir.Held(quotient, 4), mir.Held(remainder, 4)), covers=(1, 5))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (copy, divide), ()),))
    if immediate:
        divide = replace(divide, args=(mir.Held(source, 4), mir.Const(divisor, 4)), uses=(source,))
        body = replace(body, blocks=(replace(body.blocks[0], ops=(divide,)),))
    done = algebraic._divisions(body)
    assert all(op.kind is not mir.Kind.DIVMOD for op in done.blocks[0].ops)
    for number in [-2147483648, -divisor-1, -divisor, -divisor+1, -1, 0, 1, divisor-1, divisor, 2147483647]:
        values = {source: number}
        for op in done.blocks[0].ops:
            args = [arg.n if isinstance(arg, mir.Const) else values[arg.value] for arg in op.args]
            match op.kind:
                case mir.Kind.COPY: answer = args[0]
                case mir.Kind.SAR: answer = args[0] >> args[1]
                case mir.Kind.SHL: answer = args[0] << args[1]
                case mir.Kind.AND: answer = args[0] & args[1]
                case mir.Kind.ADD: answer = args[0] + args[1]
                case mir.Kind.SUB: answer = args[0] - args[1]
                case _: pytest.fail(str(op.kind))
            values[op.results[0].value] = ((answer & 0xffffffff) ^ 0x80000000) - 0x80000000
        expected = abs(number) // divisor * (-1 if number < 0 else 1)
        assert values[quotient] == expected
        assert values[remainder] == number - expected * divisor


@pytest.mark.parametrize(("high", "low", "answer"), [(4, 0, 262144), (0, 512, 512), (-1, -1, 0xffffffff), (1, -1, 0x1ffff)])
def test_constant_word_concatenation(high: int, low: int, answer: int) -> None:
    result = mir.Value(1, 0)
    op = mir.Op(0, mir.Synth.CONCAT_LOW, "concat", (result,), (), kind=mir.Kind.CONCAT,
                args=(mir.Const(high, 2), mir.Const(low, 2)), results=(mir.Held(result, 4),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),), {})
    done = algebraic.simplified(body, {result}, set()).blocks[0].ops[0]
    assert done.kind is mir.Kind.COPY
    assert done.args == (mir.Const(answer, 4),)


@pytest.mark.parametrize("width", [2, 4])
@pytest.mark.parametrize(
    ("kind", "constant", "answer"),
    [
        (mir.Kind.ADD, 0, None),
        (mir.Kind.SUB, 0, None),
        (mir.Kind.MUL, 1, None),
        (mir.Kind.OR, 0, None),
        (mir.Kind.XOR, 0, None),
        (mir.Kind.AND, -1, None),
        (mir.Kind.AND, 0, 0),
        (mir.Kind.MUL, 0, 0),
        (mir.Kind.OR, -1, -1),
        (mir.Kind.SHL, 0, None),
        (mir.Kind.SHR, 0, None),
        (mir.Kind.SAR, 0, None),
    ],
)
def test_integer_identities(kind: mir.Kind, constant: int, answer: int | None, width: int) -> None:
    source, result = mir.Value(10, 0), mir.Value(11, 1)
    count_width = 1 if kind in (mir.Kind.SHL, mir.Kind.SHR, mir.Kind.SAR) else width
    op = mir.Op(
        1,
        ir.Operation.BINARY,
        "",
        (result,),
        (source,),
        kind=kind,
        args=(mir.Held(source, width), mir.Const(constant, count_width)),
        results=(mir.Held(result, width),),
    )
    changed = algebraic._simplified(op, {result}, set())
    assert changed.kind is mir.Kind.COPY
    expected = mir.Held(source, width) if answer is None else mir.Const(answer & ((1 << (width * 8)) - 1), width)
    assert changed.args == (expected,)
    assert changed.defines == (result,)
    if width == 2:
        assert algebraic._simplified(op, {result}, {result}) == op
    flags = mir.Value(12, 1, flags=True)
    observed_flags = replace(op, defines=(result, flags))
    assert algebraic._simplified(observed_flags, {result, flags}, set()) == observed_flags


def test_algebraic_pass_runs_without_constant_propagation_facts() -> None:
    source, result = mir.Value(10, 0), mir.Value(11, 1)
    op = mir.Op(
        1,
        ir.Operation.BINARY,
        "add",
        (result,),
        (source,),
        kind=mir.Kind.ADD,
        args=(mir.Held(source, 2), mir.Const(0, 2)),
        results=(mir.Held(result, 2),),
    )
    use = mir.Op(2, ir.Operation.PUSH, "push", (), (result,), kind=mir.Kind.ARG, args=(mir.Held(result, 2),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op, use), ()),))
    assert transform.Algebraic().transform(body).blocks[0].ops[0].kind is mir.Kind.COPY


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_harr_only_needs_the_low_product(tag: str) -> None:
    """HARR paid for a widening product although its high answer and flags were unused."""
    obj = Path("fixtures/omf") / f"harr-{tag}.obj"
    found = corpus.loaded(obj)
    assert found is not None
    before = mir.bodies(found, corpus.partitioned(obj))[0][1]
    after = transform.Algebraic().transform(before)
    products = [op for block in after.blocks for op in block.ops if op.kind is mir.Kind.MUL]
    assert products and all(len(op.results) == 1 for op in products)


@pytest.mark.parametrize("observed", ["high", "flags", "upper", "none"])
def test_product_projection_retains_observed_outputs(observed: str) -> None:
    source, low, high, flags = mir.Value(1, 0), mir.Value(2, 1), mir.Value(3, 1), mir.Value(4, 1, flags=True)
    op = mir.Op(
        1,
        ir.Operation.MULTIPLY,
        "imul",
        (flags, low, high),
        (source,),
        kind=mir.Kind.MUL,
        args=(mir.Held(source, 2), mir.Const(20, 2)),
        results=(mir.Held(low, 2), mir.Held(high, 2)),
        merges={source: high},
    )
    wanted = {low} | ({high} if observed == "high" else {flags} if observed == "flags" else set())
    changed = algebraic._product(op, wanted, {low} if observed == "upper" else set())
    if observed == "none":
        assert changed.results == (mir.Held(low, 2),)
        assert changed.defines == (low,)
        assert changed.uses == (source,)
    else:
        assert changed == op


def test_dead_phis_do_not_keep_matrix_product_halves() -> None:
    """Matrix retained widening multiplies solely for unused loop phi results."""
    obj = Path("fixtures/omf/matrix-p-g2.obj")
    found = corpus.loaded(obj)
    assert found is not None
    blocks = corpus.partitioned(obj)
    body = mir.bodies(found, blocks)[0][1]
    after = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
    products = [op for block in after.blocks for op in block.ops if op.kind is mir.Kind.MUL]
    assert products and all(len(op.results) == 1 for op in products)
