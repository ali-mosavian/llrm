"""
qbopt/analysis/consts.py's own gate: a fold that is wrong produces a plausible number,
so the width rule and what seeds it are the test.
"""

from pathlib import Path

import pytest

import corpus
from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import lower
from qbopt.analysis import consts
from qbopt.optimize import transform


@pytest.mark.parametrize("width", [2, 4])
def test_pointer_displacement_constants_preserve_order_and_width(width):
    """NDMAX's zero displacement should fold without interpreting its base as an integer offset."""
    pointer, displacement, result = (mir.Value(index, 0) for index in (990, 991, 992))
    args = (mir.Held(pointer, 4), mir.Held(displacement, 4))
    op = mir.Op(0, ir.Operation.NOTHING, "", (result,), (pointer, displacement),
                kind=mir.Kind.PTR_OFFSET, args=args, results=(mir.Held(result, 4),))
    changed = transform._constant_operands(op, {
        pointer: consts.Known(0x12340000, 4), displacement: consts.Known(0, width)})
    assert changed.args == (args[0], mir.Const(0, 4) if width == 4 else args[1])
    assert pointer in changed.uses

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


@pytest.mark.parametrize("number", [0, 1, 32767, 32768, 65535])
def test_signed_widening_produces_a_whole_long_constant(number):
    """ADDRM's explicit word-to-long conversion could not fold even with a known input."""
    source, result = mir.Value(1, 0), mir.Value(2, 0)
    op = mir.Op(0, ir.Operation.EXTEND, "", (result,), (source,),
                kind=mir.Kind.SIGN_EXTEND, args=(mir.Held(source, 2),), results=(mir.Held(result, 4),))
    expected = ((number ^ 0x8000) - 0x8000) & 0xffffffff
    assert consts._result(op, {source: consts.Known(number, 2)}) == consts.Known(expected, 4)
    assert consts._result(op, {source: consts.Known(number, 1)}) is None
    from dataclasses import replace
    literal = replace(op, args=(mir.Const(number, 2),), uses=())
    body = mir.MirBody(0, (mir.MirBlock(0, (), (literal,), ()),))
    folded = transform.folded(body, frozenset(), {})
    assert folded.blocks[0].ops[0].kind is mir.Kind.COPY
    assert folded.blocks[0].ops[0].args == (mir.Const(expected, 4),)


@pytest.mark.parametrize(("high", "low", "answer"), [(4, 0, 262144), (0, 512, 512), (-1, -1, 0xffffffff)])
def test_recovered_argument_constants(high: int, low: int, answer: int) -> None:
    """Nbody kept its 262144 and 512 divisors hidden behind recovered word copies."""
    upper, bottom, result = (mir.Value(index, 0) for index in range(1, 4))
    op = mir.Op(0, mir.Synth.CONCAT_LOW, "concat", (result,), (upper, bottom),
                kind=mir.Kind.CONCAT, args=(mir.Held(upper, 2), mir.Held(bottom, 2)),
                results=(mir.Held(result, 4),))
    facts = {upper: consts.Known(high, 2), bottom: consts.Known(low, 2)}
    assert consts._result(op, facts) == consts.Known(answer, 4)
    facts[upper] = consts.Known(high, 1)
    assert consts._result(op, facts) is None


def test_relocated_descriptor_address_is_not_integer_zero() -> None:
    """HARR's descriptor at segment 5 + 6 was reported as the constant zero."""
    obj = Path("fixtures/omf/harr-p-g2.obj")
    found = corpus.loaded(obj)
    assert found is not None
    assert found.operands[0x70].disp == 6
    body = next(body for body in raised(obj) if any(op.at == 0x6F for block in body.blocks for op in block.ops))
    op = next(op for block in body.blocks for op in block.ops if op.at == 0x6F)
    assert op.defines[0] not in consts.known(body)
    assert lower.operand(op.args[0]).address == found.operands[0x70]


def test_folded_extraction_has_no_implicit_machine_result() -> None:
    """CHAIN printed MODMOD=92344 instead of 13106 after stale DX replaced a folded high word."""
    path = Path("fixtures/regressions/chain-stack-q-O.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    folded = transform.folded(body, found.dgroup, found.calls)
    folded = transform.folded(folded, found.dgroup, found.calls)
    extracts = {op.results[0].value for block in body.blocks for op in block.ops if op.kind is mir.Kind.EXTRACT}
    copies = [op for block in folded.blocks for op in block.ops
              if op.kind is mir.Kind.COPY and op.results and op.results[0].value in extracts
              and isinstance(op.args[0], mir.Const)]
    assert copies
    lowering = lower.Lowering(folded, {value.id for value in extracts}, {}, ())
    for op in copies:
        assert lowering._idiom(op) == ()


def raised(obj: Path) -> list[mir.MirBody]:
    found = corpus.loaded(obj)
    assert found is not None
    partitioned = corpus.partitioned(obj)
    result = corpus.bodies(obj)
    if isinstance(result, str) or not partitioned:
        return []
    nodes = {ir.span(n)[0]: n for body in result for n in body.nodes}
    out = []
    for body in result:
        mine = [b for b in partitioned if any(lo <= b.at < hi for lo, hi in body.body.ranges)]
        if not mine:
            continue
        built = mir.raise_body(mine, nodes, body.body.seed, found.calls)
        assert not isinstance(built, str), built
        out.append(built)
    return out


@pytest.mark.parametrize(
    ("n", "width", "want"),
    [(5, 2, 5), (-1, 2, 0xFFFF), (0x1FFFF, 2, 0xFFFF), (-1, 4, 0xFFFFFFFF)],
)
def test_a_fact_is_masked_to_its_own_width(n: int, width: int, want: int) -> None:
    assert consts.masked(n, width) == want


@pytest.mark.parametrize("kind", [mir.Kind.XOR, mir.Kind.SUB])
@pytest.mark.parametrize("width", [1, 2, 4])
def test_equal_integer_operands_are_zero_without_input_facts(kind: mir.Kind, width: int) -> None:
    """matrix kept multiplying its zero initializer because self-cancellation had no fact."""
    source, result = mir.Value(900, 0), mir.Value(901, 1)
    held = mir.Held(source, width)
    op = mir.Op(
        1,
        ir.Operation.BINARY,
        "",
        (result,),
        (source,),
        kind=kind,
        args=(held, held),
        results=(mir.Held(result, width),),
    )
    assert consts._result(op, {}) == consts.Known(0, width)


@pytest.mark.parametrize("constant_first", [False, True])
@pytest.mark.parametrize("same", [False, True])
def test_constant_subtraction_preserves_operand_order(constant_first: bool, same: bool) -> None:
    """NESTED kept constant loop bounds in registers; propagating them must not reverse subtraction."""
    source, bound, result = mir.Value(910, 0), mir.Value(911, 0), mir.Value(912, 1)
    if same:
        source = bound
    args = (mir.Held(source, 2), mir.Held(bound, 2))
    if constant_first:
        args = args[::-1]
    op = mir.Op(
        1, ir.Operation.COMPARE, "cmp", (result,), tuple(dict.fromkeys((source, bound))),
        kind=mir.Kind.SUB, args=args, results=(),
    )
    changed = transform._constant_operands(op, {bound: consts.Known(5, 2)})
    expected = (args[0], mir.Const(5, 2)) if not constant_first or same else args
    assert changed.args == expected
    assert args[0].value in changed.uses


@pytest.mark.parametrize("address_part", ["base", "segment"])
def test_constant_operand_keeps_its_memory_address_dependency(address_part: str) -> None:
    """Substituting p in memory[p] - p orphaned the address value while the load still used it."""
    pointer, result = mir.Value(920, 0), mir.Value(921, 1)
    ref = mir.MemRef(None, 2, **{address_part: pointer})
    op = mir.Op(
        1, ir.Operation.BINARY, "sub", (result,), (pointer,), kind=mir.Kind.SUB,
        args=(mir.Cell(ref), mir.Held(pointer, 2)), results=(mir.Held(result, 2),), loads=(ref,),
    )
    changed = transform._constant_operands(op, {pointer: consts.Known(16, 2)})
    assert changed.args == (mir.Cell(ref), mir.Const(16, 2))
    assert changed.uses == (pointer,)


@pytest.mark.parametrize("constant_first", [False, True])
def test_a_known_factor_becomes_a_multiply_operand(constant_first: bool) -> None:
    source, factor, result = mir.Value(900, 0), mir.Value(901, 0), mir.Value(902, 1)
    args = (mir.Held(source, 2), mir.Held(factor, 2))
    if constant_first:
        args = args[::-1]
    op = mir.Op(
        1,
        ir.Operation.MULTIPLY,
        "",
        (result,),
        (source, factor),
        kind=mir.Kind.MUL,
        args=args,
        results=(mir.Held(result, 2),),
    )
    changed = transform._constant_operands(op, {factor: consts.Known(20, 2)})
    assert changed.args == (mir.Held(source, 2), mir.Const(20, 2))
    assert changed.uses == (source,)


@pytest.mark.parametrize(("width", "number", "answer"), [(1, 0x101, 0), (2, 0x12350000, 0), (2, 0x12358000, 0x4000)])
def test_a_narrow_shift_cannot_pull_bits_from_outside_its_operand(width: int, number: int, answer: int) -> None:
    """A word read of 0x12350000 shifted right once folded to 0x8000, not 0."""
    source, result = mir.Value(1, 0), mir.Value(2, 1)
    op = mir.Op(
        1,
        ir.Operation.BINARY,
        "shr",
        (result,),
        (source,),
        kind=mir.Kind.SHR,
        args=(mir.Held(source, width), mir.Const(1, width)),
        results=(mir.Held(result, width),),
    )
    facts = {source: consts.Known(number, 4)}
    assert consts._result(op, facts) == consts.Known(answer, width)


def test_flags_do_not_stop_an_operation_being_folded() -> None:
    """Nearly every arithmetic instruction defines its result and the flags
    together, so a rule wanting one definition rejects all of them. It did,
    and the propagation found nothing but its own seeds until this."""
    result = mir.Value(1, 0)
    flags = mir.Value(2, 0, flags=True)
    assert consts._defined(mir.Op(0, ir.Operation.UNARY, "dec", (flags, result), ())) == result
    assert consts._defined(mir.Op(0, ir.Operation.UNARY, "dec", (flags,), ())) is None


@pytest.mark.parametrize("width", [1, 2, 4])
@pytest.mark.parametrize(("kind", "number", "answer"), [(mir.Kind.INCREMENT, -1, 0), (mir.Kind.DECREMENT, 0, -1)])
def test_constant_steps_wrap_at_the_value_width(kind: mir.Kind, number: int, answer: int, width: int) -> None:
    """cmpord's zero decremented into BASIC true was not known to be -1."""
    result = mir.Value(1, 0)
    op = mir.Op(
        0,
        ir.Operation.UNARY,
        "",
        (result,),
        (),
        kind=kind,
        args=(mir.Const(number, width),),
        results=(mir.Held(result, width),),
    )
    assert consts._result(op, {}) == consts.Known(consts.masked(answer, width), width)


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_fact_never_claims_more_bytes_than_the_instruction_wrote(obj: Path) -> None:
    """`mov ax,5` does not make eax five. Claiming it would fold a 32-bit
    use of a value only half of which is known, and the answer would look
    entirely reasonable."""
    for body in raised(obj):
        for fact in consts.known(body).values():
            assert fact.width in (1, 2, 4)
            assert 0 <= fact.n < (1 << (fact.width * 8))


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_known_value_is_defined_by_an_operation_that_computes_it(obj: Path) -> None:
    for body in raised(obj):
        facts = consts.known(body)
        defined = {consts._defined(op): op for block in body.blocks for op in block.ops}
        for value in facts:
            assert value in defined, f"{value} is known but nothing defines it"
            node = defined[value].node
            assert node is not None
            assert ir.modelled(node.semantics)


def test_a_comparison_result_folds_to_basics_own_true() -> None:
    """BC materialises a comparison as `mov ax,0` then a conditional `dec ax`
    -- and -1 is what BASIC calls true. There is one reaching definition and
    so no phi, which is why the fact survives the block boundary at all."""
    seen = 0
    for body in raised(Path("fixtures/omf/cmpord-p-evt.obj")):
        facts = consts.known(body)
        for block in body.blocks:
            for op in block.ops:
                target = consts._defined(op)
                if op.name == "dec" and target in facts:
                    assert facts[target] == consts.Known(0xFFFF, 2)
                    seen += 1
    assert seen, "cmpord materialises comparison results"


def test_nothing_is_folded_through_a_phi() -> None:
    """A phi is where two definitions meet, so its value is not one of them.
    Folding one needs the conditional half of a sparse propagation, which
    consts.py says it does not do -- this holds it to that."""
    for obj in FIXTURES[:20]:
        for body in raised(obj):
            facts = consts.known(body)
            for block in body.blocks:
                for phi in block.phis:
                    assert phi.result not in facts


def test_division_is_not_folded() -> None:
    """BC's own divide has semantics this pass already refuses to reproduce
    -- calls.py on `x/0` and the signed extreme -- so a folder that answered
    them here would be inventing a result the running program never
    produces."""
    assert "idiv" not in consts.ARITH
    assert "div" not in consts.ARITH
    assert not (set(consts.ARITH) & set(consts.UNARY)), "one table each, no operation in both"


def test_a_fact_is_never_wider_than_the_operation_that_made_it() -> None:
    """`mov ax,5` makes the low half five and says nothing above it. Taking
    the number without the width folds a 32-bit use of a half-known value
    and gives a plausible answer that is not the program's."""
    assert consts.masked(0x1FFFF, 2) == 0xFFFF
    assert consts.Known(5, 2).width == 2
