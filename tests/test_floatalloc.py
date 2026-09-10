"""Floating allocation consumes values without changing arithmetic order."""

import pytest

from qbopt.backend import floatalloc, select
from qbopt.model import ir, lir
from qbopt.objectfile.module import Addr, Space
from dataclasses import replace


def _body(operations):
    insns = tuple(lir.Insn(index * 8, (index * 8, index * 8 + 8), what,
        tuple(arg.value for arg in what.dests if isinstance(arg, ir.Held)),
        tuple(arg.value for arg in what.sources if isinstance(arg, ir.Held)))
        for index, what in enumerate(operations))
    return lir.LirBody("floating", 0, (lir.LirBlock(0, insns),), {}, {})


@pytest.mark.parametrize("boundary", [None, ir.Operation.CALL, ir.Operation.BARRIER])
def test_region_value_reuses_one_reload_until_an_unknown_effect(boundary):
    """A shared floating result crossing a fork reloaded its owned slot for every store."""
    from qbopt.backend import frame

    value = ir.Held(1, 10)
    cell = ir.Mem(Addr(Space.FRAME, -4), 4)
    operations = [
        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (value,)),
    ]
    if boundary is not None:
        operations.append(ir.Semantics(boundary, "call" if boundary is ir.Operation.CALL else "", (), ()))
    operations.append(ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (value,)))
    body = _body(operations)
    body = replace(body, blocks=(
        lir.LirBlock(0, body.insns[:1], (8, 80)),
        lir.LirBlock(8, body.insns[1:], ()),
        lir.LirBlock(80, (), ()),
    ))
    result = floatalloc.allocated(body, frame.Frame(-8))
    consumer = next(block for block in result.blocks if block.at == 8)
    reloads = [one for one in consumer.insns if one.what.op is ir.Operation.FLOAT_LOAD
               and any(isinstance(arg, ir.Mem) and arg.width == 10 for arg in one.what.sources)]
    assert len(reloads) == (1 if boundary is None else 2)
    stores = [one.what for one in consumer.insns if one.what.op is ir.Operation.FLOAT_STORE]
    assert [one.name for one in stores] == (["fst", "fstp"] if boundary is None else ["fstp", "fstp"])
    assert all(one.dests == (cell,) and one.sources == (ir.St(0),) for one in stores)
    if boundary is None:
        assert all(select.emit(one.what) is not None for one in consumer.insns)


def test_square_keeps_the_next_used_operand_on_top():
    """FPDEEP shuffled p back to the top immediately after forming p*p."""
    value, square, total, answer = (ir.Held(index, 10) for index in range(1, 5))
    cell = ir.Mem(Addr(Space.FRAME, -4), 4)
    body = _body([
        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)),
        ir.Semantics(ir.Operation.FLOAT_ARITH, "fmul", (square,), (value, value)),
        ir.Semantics(ir.Operation.FLOAT_ARITH, "fadd", (total,), (value, value)),
        ir.Semantics(ir.Operation.FLOAT_ARITH, "fdiv", (answer,), (square, total)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (answer,)),
    ])
    result = floatalloc.allocated(body)
    assert not any(one.what.name == "fxch" for one in result.insns)
    product = next(one.what for one in result.insns if one.what.name == "fmul")
    assert product.dests == (ir.St(1),)
    assert product.sources == (ir.St(1), ir.St(0))
    assert all(select.emit(one.what) is not None for one in result.insns)


@pytest.mark.parametrize("width", [2, 4])
def test_runtime_stack_integer_result_uses_memory_without_named_float_values(width):
    """SYS_INIT_TABLES at 08fa refused FISTP EBX after POW4 returned in physical ST0."""
    from qbopt.backend import frame
    result = ir.Held(2, width)
    cell = ir.Mem(Addr(Space.FRAME, -8), width)
    body = _body([
        ir.Semantics(ir.Operation.FLOAT_STORE, "fistp", (result,), (ir.St(0),)),
        ir.Semantics(ir.Operation.MOVE, "mov", (cell,), (result,)),
    ])
    allocated = floatalloc.allocated(body, frame.Frame(-8))
    assert [one.what.name for one in allocated.insns] == ["wait", "fistp", "wait", "mov", "mov"]
    store, load = allocated.insns[1].what, allocated.insns[3].what
    assert isinstance(store.dests[0], ir.Mem)
    assert store.dests == load.sources
    assert store.dests[0].width == width
    assert load.dests == (result,)
    assert select.emit(store) is not None


@pytest.mark.parametrize("width", [2, 4])
def test_integer_result_waits_before_reading_owned_conversion_storage(width):
    """B$FIST/B$FIS2 wait on both sides of conversion before returning the integer."""
    from qbopt.backend import frame
    value, result = ir.Held(1, 10), ir.Held(2, width)
    cell = ir.Mem(Addr(Space.FRAME, -8), 8)
    body = _body([
        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fistp", (result,), (value,)),
        ir.Semantics(ir.Operation.MOVE, "mov", (cell,), (result,)),
    ])
    allocated = floatalloc.allocated(body, frame.Frame(-8))
    assert [one.what.name for one in allocated.insns] == ["fld", "wait", "fistp", "wait", "mov", "mov"]
    store, load = allocated.insns[2].what, allocated.insns[4].what
    assert store.dests == load.sources
    assert store.dests[0].width == width
    assert store.dests[0].addr.disp < -8
    assert load.dests == (result,)


@pytest.mark.parametrize("width", [2, 4])
def test_unused_integer_conversion_keeps_checkpoints_without_materializing_result(width):
    """Constant print arguments can leave an unused conversion result; both waits must survive."""
    from qbopt.backend import frame
    value, result = ir.Held(1, 10), ir.Held(2, width)
    cell = ir.Mem(Addr(Space.FRAME, -8), 8)
    body = _body([
        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fistp", (result,), (value,)),
    ])
    allocated = floatalloc.allocated(body, frame.Frame(-8))
    assert [one.what.name for one in allocated.insns] == ["fld", "wait", "fistp", "wait"]
    assert allocated.insns[2].what.dests[0].width == width


@pytest.mark.parametrize("reader", ["opaque", "barrier", "pinned", "phi", "uses"])
def test_conversion_result_is_kept_for_non_operand_readers(reader):
    """An invisible reader must not lose the integer returned by B$FIST."""
    from qbopt.backend import frame
    value, result = ir.Held(1, 10), ir.Held(2, 4)
    cell = ir.Mem(Addr(Space.FRAME, -8), 8)
    body = _body([
        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fistp", (result,), (value,)),
    ])
    block = body.blocks[0]
    match reader:
        case "pinned": body = replace(body, pins={2: None})
        case "phi": body = replace(body, blocks=(block, lir.LirBlock(32, (), phis=(lir.Phi(3, ((0, 2),)),))))
        case _:
            what = None if reader == "opaque" else ir.Semantics(
                ir.Operation.BARRIER if reader == "barrier" else ir.Operation.NOTHING, "", (), ())
            extra = lir.Insn(24, (24, 24), what, (), (2,) if reader == "uses" else ())
            body = replace(body, blocks=(replace(block, insns=(*block.insns, extra)),))
    allocated = floatalloc.allocated(body, frame.Frame(-8))
    assert any(one.what and one.what.name == "mov" and one.what.dests == (result,) for one in allocated.insns)


def test_ninth_float_uses_an_owned_extended_precision_spill():
    """Nine live FP values previously refused allocation instead of preserving 80 bits."""
    from qbopt.backend import frame
    cell = ir.Mem(Addr(Space.FRAME, -4), 4)
    values = [ir.Held(index, 10) for index in range(1, 10)]
    body = _body([ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)) for value in values]
                 + [ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (value,)) for value in values])
    slots = frame.Frame(-4)
    integer_scratch = slots.cell(1, 2)
    allocated = floatalloc.allocated(body, slots)
    spills = [one for one in allocated.insns if one.what.name == "fstp" and one.what.dests[0].width == 10]
    assert len(spills) == 1 and slots.size >= 10
    assert all(one.what.dests[0].addr != integer_scratch.addr for one in spills)
    stack, memory, answers = [], {}, []
    inputs = iter(range(1, 10))
    for one in allocated.insns:
        what = one.what
        assert select.emit(what) is not None
        match what.name:
            case "fld":
                source = what.sources[0]
                value = memory[source.addr.disp] if source.width == 10 else next(inputs)
                stack.insert(0, value)
                assert len(stack) <= 8
            case "fxch":
                index = what.sources[1].index
                stack[0], stack[index] = stack[index], stack[0]
            case "fstp":
                value = stack.pop(0)
                if what.dests[0].width == 10:
                    memory[what.dests[0].addr.disp] = value
                    assert one.covers[0] == one.covers[1]
                else:
                    answers.append(value)
    assert answers == list(range(1, 10)) and not stack


@pytest.mark.parametrize("path", [(0, 16, 48), (0, 32, 48), (0, 16, 16, 48)])
def test_float_survives_fork_join_and_loop_without_rereading_source(path):
    """A dominating extended value was refused at forks, joins and loop boundaries."""
    from fractions import Fraction
    from qbopt.backend import frame
    value = ir.Held(1, 10)
    cell = ir.Mem(Addr(Space.FRAME, -20), 10)
    source = ir.Mem(Addr(Space.FRAME, -10), 10)
    original = _body([
        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (source,)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (value,)),
    ])
    load, store = original.insns
    body = replace(original, blocks=(
        lir.LirBlock(48, (replace(store, at=48, covers=(48, 56)),)),
        lir.LirBlock(0, (load,), (16, 32)),
        lir.LirBlock(16, (replace(store, at=16, covers=(16, 24)),), (16, 48)),
        lir.LirBlock(32, (replace(store, at=32, covers=(32, 40)),), (48,)),
    ))
    slots = frame.Frame(-20)
    result = floatalloc.allocated(body, slots)
    assert [block.at for block in result.blocks] == [block.at for block in body.blocks]
    precise = Fraction(1) + Fraction(1, 2**63)
    memory, stack, answers = {source.addr.disp: precise}, [], []
    by_at = {block.at: block for block in result.blocks}
    for at in path:
        for one in by_at[at].insns:
            what = one.what
            assert select.emit(what) is not None
            match what.name:
                case "fld":
                    stack.insert(0, memory[what.sources[0].addr.disp])
                case "fstp":
                    destination = what.dests[0]
                    answer = stack.pop(0)
                    if destination.addr == cell.addr:
                        answers.append(answer)
                    else:
                        assert destination.width == 10
                    memory[destination.addr.disp] = answer
                case _: pytest.fail(f"Unexpected allocation instruction: {what}")
        assert not stack
        memory[source.addr.disp] = -99
    assert answers == [precise] * (len(path) - 1)
    assert slots.size == 10


@pytest.mark.parametrize("defect", ["entry", "bypass", "pinned", "duplicate"])
def test_floating_bridge_never_reads_an_unestablished_slot(defect):
    """Cross-block allocation must not turn a missing definition into a frame read."""
    from qbopt.backend import frame
    from qbopt.backend.lower import Unlowered
    value = ir.Held(1, 10)
    cell = ir.Mem(Addr(Space.FRAME, -10), 10)
    body = _body([
        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (value,)),
    ])
    load, store = body.insns
    blocks = (lir.LirBlock(0, (load,), (16, 32)),
              lir.LirBlock(16, (store,)), lir.LirBlock(32, (), (16,)))
    match defect:
        case "entry": body = replace(body, entry=16)
        case "bypass":
            body = replace(body, entry=48)
            blocks += (lir.LirBlock(48, (), (0, 16)),)
        case "pinned": body = replace(body, pins={1: 0})
        case "duplicate": blocks = (replace(blocks[0], insns=(load, load)), *blocks[1:])
    with pytest.raises(Unlowered):
        floatalloc.allocated(replace(body, blocks=blocks), frame.Frame(-10))


@pytest.mark.parametrize("target", [16, 48])
def test_floating_loop_phis_swap_in_parallel_on_the_critical_backedge(target):
    """Floating loop phis were refused; serial slot copies would turn (1,2) into (2,2)."""
    from qbopt.backend import frame
    first, second, left, right = (ir.Held(index, 10) for index in range(1, 5))
    cells = [ir.Mem(Addr(Space.FRAME, -10 * index), 10) for index in (1, 2, 3)]
    body = _body([
        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (first,), (cells[0],)),
        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (second,), (cells[1],)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cells[2],), (left,)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cells[2],), (right,)),
        ir.Semantics(ir.Operation.BRANCH, "jne", (), (), target),
    ])
    body = replace(body, blocks=(
        lir.LirBlock(0, body.insns[:2], (16,)),
        lir.LirBlock(16, body.insns[2:], (16, 48), (
            lir.Phi(3, ((0, 1), (16, 4))), lir.Phi(4, ((0, 2), (16, 3))),
            lir.Phi(93, ((0, 91), (16, 92))),
        )), lir.LirBlock(48, ()),
    ))
    result = floatalloc.allocated(body, frame.Frame(-30))
    by_at = {block.at: block for block in result.blocks}
    edge = next(at for at in by_at[16].succ if at != 48)
    assert edge not in (0, 16, 48) and by_at[edge].succ == (16,)
    assert by_at[16].phis == (lir.Phi(93, ((0, 91), (edge, 92))),)
    assert by_at[16].insns[-1].what.target == edge
    memory = {cells[0].addr.disp: 1, cells[1].addr.disp: 2}
    stack, answers = [], []
    for at in (0, 16, edge, 16, 48):
        for one in by_at[at].insns:
            what = one.what
            match what.name:
                case "fld":
                    source = what.sources[0]
                    stack.insert(0, stack[source.index] if isinstance(source, ir.St)
                                 else memory[source.addr.disp])
                case "fxch":
                    index = what.sources[1].index
                    stack[0], stack[index] = stack[index], stack[0]
                case "fstp":
                    destination = what.dests[0]
                    value = stack.pop(0)
                    assert destination.width == 10
                    memory[destination.addr.disp] = value
                    if destination.addr == cells[2].addr:
                        answers.append(value)
                case "jmp" | "jne": continue
                case _: pytest.fail(f"Unexpected allocation instruction: {what}")
            assert select.emit(what) is not None
        assert not stack
    assert answers == [1, 2, 2, 1]
    from qbopt.backend import phielim
    integer_result = phielim.eliminated(result)
    assert not any(block.phis for block in integer_result.blocks)
    edge_copies = next(block for block in integer_result.blocks if block.at == edge).insns
    assert any(one.defines == (93,) and one.uses == (92,) for one in edge_copies)


@pytest.mark.parametrize("width", [4, 8, 10])
def test_live_store_uses_nonpopping_encoding_when_available(width):
    """Exact-store reuse duplicated ST0 solely to pop the duplicate into memory."""
    value = ir.Held(1, 10)
    cell = ir.Mem(Addr(Space.FRAME, -16), width)
    body = _body([
        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (value,)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (value,)),
    ])
    result = floatalloc.allocated(body)
    assert [one.what.name for one in result.insns] == (
        ["fld", "fst", "fstp"] if width in (4, 8) else ["fld", "fld", "fstp", "fstp"])
    assert all(select.emit(one.what) is not None for one in result.insns)


@pytest.mark.parametrize("boundary", ["linear", "reversed", "separated", "fork", "join", "entry"])
def test_shared_float_crosses_only_a_unique_straight_line_edge(boundary):
    """A shared sum was refused at a block edge despite one unchanged stack path."""
    from qbopt.backend.lower import Unlowered
    shared, product, quotient = (ir.Held(index, 10) for index in (1, 2, 3))
    cell = ir.Mem(Addr(Space.FRAME, -4), 4)
    body = _body([
        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (shared,), (cell,)),
        ir.Semantics(ir.Operation.FLOAT_ARITH, "fmul", (product,), (shared, cell)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (product,)),
        ir.Semantics(ir.Operation.FLOAT_ARITH, "fdiv", (quotient,), (shared, cell)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (quotient,)),
    ])
    first = lir.LirBlock(0, body.insns[:3], (24, 80) if boundary == "fork" else (24,))
    second = lir.LirBlock(24, body.insns[3:], ())
    blocks = (first, second)
    if boundary in {"fork", "join"}:
        blocks += (lir.LirBlock(80, (), (24,) if boundary == "join" else ()),)
    if boundary == "reversed":
        blocks = blocks[::-1]
    if boundary == "separated":
        blocks = (first, lir.LirBlock(80, (), ()), second)
    body = replace(body, entry=24 if boundary == "entry" else 0, blocks=blocks)
    if boundary in {"fork", "join", "entry"}:
        with pytest.raises(Unlowered):
            floatalloc.allocated(body)
        return
    allocated = floatalloc.allocated(body)
    assert [block.at for block in allocated.blocks] == [block.at for block in body.blocks]
    by_at = {block.at: block for block in allocated.blocks}
    assert [one.what.name for one in by_at[0].insns] == ["fld", "fld", "fmul", "fstp"]
    assert [one.what.name for one in by_at[24].insns] == ["fdiv", "fstp"]
    assert by_at[24].insns[0].what.sources == (ir.St(0), cell)
    assert all(select.emit(one.what) is not None for one in allocated.insns)


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


@pytest.mark.parametrize("width", [2, 4])
def test_integer_conversion_into_physical_x87_stack_materializes_memory(width):
    """Qrender camera 0335 emitted impossible FILD BX after integer promotion."""
    from qbopt.backend import frame
    integer = ir.Held(1, width)
    body = _body([ir.Semantics(ir.Operation.FLOAT_LOAD, "fild", (ir.St(0),), (integer,))])
    slots = frame.Frame(-8)
    result = floatalloc.allocated(body, slots)
    assert len(result.insns) == 2
    store, load = result.insns
    assert store.what.sources == (integer,)
    assert isinstance(load.what.sources[0], ir.Mem)
    assert store.what.dests == load.what.sources
    assert load.what.dests == (ir.St(0),)
    assert select.emit(load.what) is not None


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


@pytest.mark.parametrize("name,popping,expected", [
    ("fadd", "faddp", 9), ("fmul", "fmulp", 14),
    ("fsub", "fsubrp", 5), ("fdiv", "fdivrp", 3.5),
])
@pytest.mark.parametrize("top", ["left", "right"])
def test_last_register_operand_is_consumed_without_reversing_arithmetic(name, popping, expected, top):
    """Forwarded floating memory operands must die without leaking a stack slot or reversing division."""
    left, right, result = (ir.Held(index, 10) for index in (1, 2, 3))
    cell = ir.Mem(Addr(Space.FRAME, -4), 4)
    inputs = (right, left) if top == "left" else (left, right)
    body = _body([
        *(ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)) for value in inputs),
        ir.Semantics(ir.Operation.FLOAT_ARITH, name, (result,), (left, right)),
        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (result,)),
    ])
    allocated = floatalloc.allocated(body)
    assert not any(one.what.name == "fxch" for one in allocated.insns)
    stack, stored = [], []
    values = iter((2, 7) if top == "left" else (7, 2))
    for one in allocated.insns:
        what = one.what
        assert select.emit(what) is not None
        match what.name:
            case "fld":
                stack.insert(0, next(values))
            case "fxch":
                index = what.sources[1].index
                stack[0], stack[index] = stack[index], stack[0]
            case "fstp":
                stored.append(stack.pop(0))
            case _:
                assert what.name == (popping if top == "left" else popping.replace("rp", "p"))
                index = what.sources[0].index
                a, b = stack[index], stack[0]
                stack[index] = {"faddp": lambda: a + b, "fmulp": lambda: a * b,
                                "fsubp": lambda: a - b, "fdivp": lambda: a / b,
                                "fsubrp": lambda: b - a, "fdivrp": lambda: b / a}[what.name]()
                stack.pop(0)
    assert stored == [expected] and not stack


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
