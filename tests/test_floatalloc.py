"""Floating allocation consumes values without changing arithmetic order."""

import pytest

from qbopt.backend import floatalloc, select
from qbopt.model import ir, lir
from qbopt.objectfile.module import Addr, Space


def _body(operations):
    insns = tuple(lir.Insn(index * 8, (index * 8, index * 8 + 8), what,
        tuple(arg.value for arg in what.dests if isinstance(arg, ir.Held)),
        tuple(arg.value for arg in what.sources if isinstance(arg, ir.Held)))
        for index, what in enumerate(operations))
    return lir.LirBody("floating", 0, (lir.LirBlock(0, insns),), {}, {})


@pytest.mark.parametrize("width", [2, 4])
def test_integer_conversion_materializes_a_frame_operand(width):
    """FPCALC's computed integer must reach FILD through an owned, correctly sized slot."""
    from qbopt.backend import frame
    integer, floating = ir.Held(1, width), ir.Held(2, 10)
    output = ir.Mem(Addr(Space.FRAME, -8), 8)
    body = _body([ir.Semantics(ir.Operation.FLOAT_LOAD, "fild", (floating,), (integer,)),
                  ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (output,), (floating,))])
    slots = frame.Frame(-8)
    result = floatalloc.allocated(body, slots)
    store, load, _ = result.insns
    assert slots.size == width
    assert store.what.sources == (integer,)
    assert store.what.dests == load.what.sources == (slots.cell(2, width),)
    assert store.uses == (1,) and load.uses == ()
    assert store.covers == (0, 0)


@pytest.mark.parametrize("operation", ["fsub", "fchs", "fstp", "fsubp"])
def test_buried_float_operand_is_exchanged_not_reloaded(operation):
    """A second live value must not prevent using the first, or reverse subtraction."""
    first, second, result = (ir.Held(index, 10) for index in (1, 2, 3))
    cell = ir.Mem(Addr(Space.FRAME, -4), 4)
    operations = [ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (first,), (cell,)),
                  ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (second,), (cell,))]
    match operation:
        case "fsub":
            operations.append(ir.Semantics(ir.Operation.FLOAT_ARITH, "fsub", (result,), (first, cell)))
        case "fchs":
            operations.append(ir.Semantics(ir.Operation.FLOAT_UNARY, "fchs", (result,), (first,)))
        case "fstp":
            operations.append(ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (first,)))
        case "fsubp":
            operations.append(ir.Semantics(ir.Operation.FLOAT_ARITH_POP, "fsubp", (result,), (second, first)))
    if operation != "fstp":
        operations.append(ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (result,)))
    if operation != "fsubp":
        operations.append(ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (second,)))
    body = _body(operations)
    allocated = floatalloc.allocated(body)
    insns = allocated.blocks[0].insns
    exchange = insns[2]
    assert exchange.what == ir.Semantics(ir.Operation.EXCHANGE, "fxch", (ir.St(0), ir.St(1)), (ir.St(0), ir.St(1)))
    assert select.emit(exchange.what).code == bytes.fromhex("d9c9")
    assert exchange.covers == (16, 16) and exchange.op is None
    assert not exchange.uses and not exchange.defines and not exchange.spread
    assert [one.covers for one in insns if one is not exchange] == [one.covers for one in body.insns]
    expected = (ir.St(1), ir.St(0)) if operation == "fsubp" else (ir.St(0), cell) if operation == "fsub" else (ir.St(0),)
    assert insns[3].what.sources == expected
    assert all(select.emit(one.what) is not None for one in insns)


def test_missing_float_is_not_created_by_an_exchange():
    from qbopt.backend.lower import Unlowered
    body = _body([ir.Semantics(ir.Operation.FLOAT_UNARY, "fchs", (ir.Held(2, 10),), (ir.Held(1, 10),))])
    with pytest.raises(Unlowered, match="unavailable"):
        floatalloc.allocated(body)


def test_shared_producer_is_kept_across_two_arithmetic_consumers():
    """FPCSE's shared sum needs to survive multiplication before division reads it."""
    shared, product, quotient = (ir.Held(index, 10) for index in (1, 2, 3))
    cell = ir.Mem(Addr(Space.FRAME, -4), 4)
    body = _body([
        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (shared,), (cell,)),
        ir.Semantics(ir.Operation.FLOAT_ARITH, "fmul", (product,), (shared, cell)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (product,)),
        ir.Semantics(ir.Operation.FLOAT_ARITH, "fdiv", (quotient,), (shared, cell)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (quotient,)),
    ])
    result = floatalloc.allocated(body)
    assert [one.what.name for one in result.insns] == ["fld", "fld", "fmul", "fstp", "fdiv", "fstp"]
    duplicate = result.insns[1]
    assert duplicate.what.sources == (ir.St(0),)
    assert duplicate.what.dests == (ir.St(0),)
    assert select.emit(duplicate.what).code == bytes.fromhex("d9c0")
    assert duplicate.covers == (8, 8) and duplicate.op is None
    assert result.insns[4].what.sources == (ir.St(0), cell)


@pytest.mark.parametrize("live", ["left", "right", "both", "same", "same_dead"])
def test_popping_subtraction_preserves_reused_values(live):
    """Shared FP values were refused when FSUBP consumed an operand used later."""
    left, right, result = (ir.Held(index, 10) for index in (1, 2, 3))
    cell = ir.Mem(Addr(Space.FRAME, -4), 4)
    operations = [ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (left,), (cell,))]
    if live in {"same", "same_dead"}:
        right = left
    else:
        operations.append(ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (right,), (cell,)))
    operations.extend([
        ir.Semantics(ir.Operation.FLOAT_ARITH_POP, "fsubp", (result,), (left, right)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (result,)),
    ])
    if live in {"left", "both", "same"}:
        operations.append(ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (left,)))
    if live in {"right", "both"}:
        operations.append(ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (right,)))
    allocated = floatalloc.allocated(_body(operations))
    stack, stored = [], []
    loads = iter((7, 2))
    for one in allocated.insns:
        what = one.what
        assert select.emit(what) is not None
        match what.name:
            case "fld":
                source = what.sources[0]
                stack.insert(0, stack[source.index] if isinstance(source, ir.St) else next(loads))
            case "fxch":
                index = what.sources[1].index
                stack[0], stack[index] = stack[index], stack[0]
            case "fsubp":
                index = what.sources[0].index
                assert index > 0 and what.sources[1] == ir.St(0)
                stack[index] -= stack[0]
                stack.pop(0)
            case "fstp":
                stored.append(stack.pop(0))
    assert stored == {"left": [5, 7], "right": [5, 2], "both": [5, 7, 2], "same": [0, 7], "same_dead": [0]}[live]
    assert not stack


@pytest.mark.parametrize("native", [False, True])
def test_inserted_stack_move_keeps_the_anchor_emulator_mode(native):
    """An inserted FLD beside FPCSE must not silently require a coprocessor under /FPi."""
    from pathlib import Path
    import corpus
    from qbopt.backend import asm
    from qbopt.model import mir
    found = corpus.loaded(Path("fixtures/omf/fpcse-p-g2.obj"))
    at = 0x66
    assert found.code[at:at+2] == bytes.fromhex("cd35")
    what = ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),), (ir.St(0),))
    op = mir.Op(at, what.op, what.name, (), (), made=what, covers=(at, at))
    result = asm.assemble([op], at, found, native_fpu=native)
    assert not isinstance(result, str), result
    assert result.code == bytes.fromhex("d9c0" if native else "cd35c0")
