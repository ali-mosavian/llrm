"""Floating allocation consumes values without changing arithmetic order."""

from dataclasses import replace
from types import SimpleNamespace

import pytest
from iced_x86 import Register


def test_truncation_saves_the_control_word_once_per_body():
    """Every `(int)f` saved the control word and built the truncating one again:
    six instructions a site, 143 sites over qcport, where one save at entry
    leaves three a site. The derived word needs no second x87 state store."""
    from qbopt.backend import frame

    def insn(at, what):
        return lir.Insn(at, (at, at), what, (), ())

    def store(at, disp):
        cell = ir.Mem(Addr(Space.FRAME, disp), 2, Register.BP, 0, 2)
        return insn(at, ir.Semantics(ir.Operation.FLOAT_STORE, "fisttp", (cell,), (ir.St(0),)))

    blocks = (
        lir.LirBlock(0, (insn(0, ir.Semantics(ir.Operation.NOTHING, "")),), (5,)),
        lir.LirBlock(5, (store(5, -2), store(6, -4)), ()),
    )
    result = floatalloc._truncating(lir.LirBody("t", 0, blocks, {}, {}), frame.Frame(-8))
    names = {block.at: [one.what.name for one in block.insns if one.what.name] for block in result.blocks}
    assert names == {
        0: ["fnstcw", "mov", "or", "mov"],
        5: ["fldcw", "fistp", "fldcw", "fldcw", "fistp", "fldcw"],
    }


from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import cpu
from qbopt.backend import select
from qbopt.backend import floatalloc
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def _body(operations):
    insns = tuple(
        lir.Insn(
            index * 8,
            (index * 8, index * 8 + 8),
            what,
            tuple(arg.value for arg in what.dests if isinstance(arg, ir.Held)),
            tuple(arg.value for arg in what.sources if isinstance(arg, ir.Held)),
        )
        for index, what in enumerate(operations)
    )
    return lir.LirBody("floating", 0, (lir.LirBlock(0, insns),), {}, {})


_ARITHMETIC = {
    "fadd": lambda d, s: d + s,
    "fsub": lambda d, s: d - s,
    "fsubr": lambda d, s: s - d,
    "fmul": lambda d, s: d * s,
    "fdiv": lambda d, s: d / s,
    "fdivr": lambda d, s: s / d,
}


def _x87(insns, memory):
    """Run allocated instructions over `memory`, cells to numbers: the memory after and the stack left."""
    memory, stack = dict(memory), []
    for one in insns:
        what = one.what
        if what.op is ir.Operation.NOTHING:
            continue
        if what.op is ir.Operation.BARRIER:
            assert not stack
            continue
        assert select.emit(what) is not None, what
        read = lambda arg: stack[arg.index] if isinstance(arg, ir.St) else memory[arg]
        match what.op:
            case ir.Operation.FLOAT_LOAD:
                stack.insert(0, read(what.sources[0]))
                assert len(stack) <= 8
            case ir.Operation.EXCHANGE:
                index = what.sources[1].index
                stack[0], stack[index] = stack[index], stack[0]
            case ir.Operation.FLOAT_STORE:
                memory[what.dests[0]] = stack[0]
                if what.name.endswith("p"):
                    stack.pop(0)
            case ir.Operation.FLOAT_UNARY:
                assert what.name == "fchs"
                stack[0] = -stack[0]
            case ir.Operation.FLOAT_ARITH:
                name = "f" + what.name[2:] if what.name.startswith("fi") else what.name
                index = what.dests[0].index
                stack[index] = _ARITHMETIC[name](stack[index], read(what.sources[1]))
            case ir.Operation.FLOAT_ARITH_POP:
                index = what.dests[0].index
                stack[index] = _ARITHMETIC[what.name[:-1]](stack[index], stack[0])
                stack.pop(0)
    return memory, stack


def _cells(*displacements, width=4):
    return tuple(ir.Mem(Addr(Space.FRAME, disp), width) for disp in displacements)


def _load(value, cell):
    return ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,))


def _store(cell, value):
    return ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (value,))


def _arithmetic(name, result, left, right):
    return ir.Semantics(ir.Operation.FLOAT_ARITH, name, (result,), (left, right))


def test_a_load_read_by_several_arithmetics_is_each_ones_memory_operand():
    """NBODYS held `falloff` for two multiplies -- `fld`, `fxch st2`, `fmul st2` -- where BC wrote `fmul [m]` twice."""
    x, y, falloff, px, py = _cells(-4, -8, -12, -16, -20)
    a, b, f, p, q = (ir.Held(index, 10) for index in range(1, 6))
    body = _body(
        [
            _load(a, x),
            _load(b, y),
            _load(f, falloff),
            _arithmetic("fmul", p, a, f),
            _store(px, p),
            _arithmetic("fmul", q, b, f),
            _store(py, q),
        ]
    )
    result = floatalloc.allocated(body)
    assert [(one.what.name, one.what.sources) for one in result.insns if falloff in one.what.sources] == [
        ("fmul", (ir.St(0), falloff))
    ] * 2
    memory, stack = _x87(result.insns, {x: 3, y: 5, falloff: 0.5})
    assert (memory[px], memory[py], stack) == (1.5, 2.5, [])


@pytest.mark.parametrize("written", ["same", "indexed"])
def test_a_load_is_not_read_again_after_a_store_that_may_reach_it(written):
    """Reading a cell again is the value only while nothing may have written it."""
    falloff = ir.Mem(Addr(Space.SEGMENT, 0x10, index=5), 4)
    target = (
        falloff
        if written == "same"
        else ir.Mem(Addr(Space.SEGMENT, 0, index=5), 4, through=Register.SI, base=ir.Held(9, 2), disp_width=2)
    )
    x, y, py = _cells(-4, -8, -20)
    a, b, f, p, q = (ir.Held(index, 10) for index in range(1, 6))
    body = _body(
        [
            _load(f, falloff),
            _load(a, x),
            _arithmetic("fmul", p, a, f),
            _store(target, p),
            _load(b, y),
            _arithmetic("fmul", q, b, f),
            _store(py, q),
        ]
    )
    result = floatalloc.allocated(body)
    assert len([one for one in result.insns if falloff in one.what.sources]) == 1
    memory, stack = _x87(result.insns, {x: 3, y: 5, falloff: 0.5})
    assert (memory[py], stack) == (2.5, [])


@pytest.mark.parametrize("proven", [False, True])
def test_a_first_read_moves_past_a_trapping_instruction_only_for_a_quiet_cell(proven):
    """`fld m32` raises on a signalling NaN; read after a division, that exception would come second."""
    w, x, y, c, z, out, other = _cells(-4, -8, -12, -16, -20, -24, -28)
    h, a, b, f, q, g, r = (ir.Held(index, 10) for index in range(1, 8))
    body = _body(
        ([_load(h, w), _store(c, h)] if proven else [])
        + [
            _load(a, x),
            _load(b, y),
            _load(f, c),
            _arithmetic("fdiv", q, a, b),
            _store(out, q),
            _load(g, z),
            _arithmetic("fmul", r, g, f),
            _store(other, r),
        ]
    )
    result = floatalloc.allocated(body)
    readers = [index for index, one in enumerate(result.insns) if c in one.what.sources]
    division = next(index for index, one in enumerate(result.insns) if one.what.name.startswith("fdiv"))
    assert len(readers) == 1 and (readers[0] > division) == proven
    memory, stack = _x87(result.insns, {w: 0.25, x: 6, y: 3, c: 0.5, z: 4})
    assert (memory[out], memory[other], stack) == (2, 1 if proven else 2, [])


def test_arithmetic_overwrites_the_operand_that_dies():
    """Neither operand on top, the second dying: the first was exchanged up and duplicated where one exchange does."""
    x, y, z, first, second, third = _cells(-10, -20, -30, -40, -50, -60, width=10)
    loaded = [ir.Held(index, 10) for index in range(1, 4)]
    a, b, c, r = (ir.Held(index, 10) for index in range(4, 8))
    body = _body(
        [
            operation
            for load, value, cell in zip(loaded, (a, b, c), (x, y, z), strict=True)
            for operation in (_load(load, cell), ir.Semantics(ir.Operation.FLOAT_UNARY, "fchs", (value,), (load,)))
        ]
        + [_arithmetic("fsub", r, a, b), _store(first, r), _store(second, c), _store(third, a)]
    )
    result = floatalloc.allocated(body)
    names = [one.what.name for one in result.insns]
    assert names.count("fxch") == 1
    assert not any(one.what.name == "fld" and isinstance(one.what.sources[0], ir.St) for one in result.insns)
    memory, stack = _x87(result.insns, {x: 8, y: 2, z: 1})
    assert (memory[first], memory[second], memory[third], stack) == (-6, -1, -8, [])


@pytest.mark.parametrize("boundary", [ir.Operation.CALL, ir.Operation.BARRIER])
def test_a_float_live_across_a_call_waits_in_an_owned_cell(boundary):
    """A value loaded before a call and stored after it refused the whole object: 'floating stack crosses an unmodelled instruction'."""
    from qbopt.backend import frame

    value = ir.Held(1, 10)
    source, target = ir.Mem(Addr(Space.FRAME, -4), 4), ir.Mem(Addr(Space.FRAME, -8), 4)
    body = _body(
        [
            ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (source,)),
            ir.Semantics(boundary, "call" if boundary is ir.Operation.CALL else "", (), ()),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (target,), (value,)),
        ]
    )
    result = floatalloc.allocated(body, frame.Frame(-8))
    shape = [
        (one.what.name, one.what.dests, one.what.sources)
        for one in result.insns
        if one.what.op is not ir.Operation.NOTHING
    ]
    cell = shape[1][1][0]
    assert isinstance(cell, ir.Mem) and cell.width == 10
    assert shape == [
        ("fld", (ir.St(0),), (source,)),
        ("fstp", (cell,), (ir.St(0),)),
        shape[2],
        ("fld", (ir.St(0),), (cell,)),
        ("fstp", (target,), (ir.St(0),)),
    ]
    assert shape[2][0] == ("call" if boundary is ir.Operation.CALL else "")


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
    body = replace(
        body,
        blocks=(
            lir.LirBlock(0, body.insns[:1], (8, 80)),
            lir.LirBlock(8, body.insns[1:], ()),
            lir.LirBlock(80, (), ()),
        ),
    )
    result = floatalloc.allocated(body, frame.Frame(-8))
    consumer = next(block for block in result.blocks if block.at == 8)
    reloads = [
        one
        for one in consumer.insns
        if one.what.op is ir.Operation.FLOAT_LOAD
        and any(isinstance(arg, ir.Mem) and arg.width == 10 for arg in one.what.sources)
    ]
    assert len(reloads) == (1 if boundary is None else 2)
    stores = [one.what for one in consumer.insns if one.what.op is ir.Operation.FLOAT_STORE]
    assert [one.name for one in stores] == (["fst", "fstp"] if boundary is None else ["fstp", "fstp"])
    assert all(one.dests == (cell,) and one.sources == (ir.St(0),) for one in stores)
    if boundary is None:
        assert all(select.emit(one.what) is not None for one in consumer.insns)


@pytest.mark.parametrize(
    "loaded, name, loaded_first, fused",
    [("fld", "fmul", False, "fmul"), ("fld", "fsub", True, "fsubr"), ("fild", "fdiv", False, "fidiv")],
)
def test_a_load_read_once_by_the_next_arithmetic_is_its_memory_operand(loaded, name, loaded_first, fused):
    """`fld [x]` then a popping multiply spent an instruction and a stack slot `fmul [x]` does not."""
    value, temporary, answer = (ir.Held(index, 10) for index in range(1, 4))
    home, cell = ir.Mem(Addr(Space.FRAME, -4), 4), ir.Mem(Addr(Space.FRAME, -8), 4)
    body = _body(
        [
            ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (home,)),
            ir.Semantics(ir.Operation.FLOAT_LOAD, loaded, (temporary,), (cell,)),
            ir.Semantics(
                ir.Operation.FLOAT_ARITH, name, (answer,), (temporary, value) if loaded_first else (value, temporary)
            ),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (home,), (answer,)),
        ]
    )
    result = floatalloc.allocated(body)
    emitted = [one for one in result.insns if one.what.op is not ir.Operation.NOTHING]
    assert [(one.what.name, one.what.sources) for one in emitted] == [
        ("fld", (home,)),
        (fused, (ir.St(0), cell)),
        ("fstp", (ir.St(0),)),
    ]
    assert all(select.emit(one.what) is not None for one in result.insns)


def test_square_keeps_the_next_used_operand_on_top():
    """FPDEEP shuffled p back to the top immediately after forming p*p."""
    value, square, total, answer = (ir.Held(index, 10) for index in range(1, 5))
    cell = ir.Mem(Addr(Space.FRAME, -4), 4)
    body = _body(
        [
            ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)),
            ir.Semantics(ir.Operation.FLOAT_ARITH, "fmul", (square,), (value, value)),
            ir.Semantics(ir.Operation.FLOAT_ARITH, "fadd", (total,), (value, value)),
            ir.Semantics(ir.Operation.FLOAT_ARITH, "fdiv", (answer,), (square, total)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (answer,)),
        ]
    )
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
    body = _body(
        [
            ir.Semantics(ir.Operation.FLOAT_STORE, "fistp", (result,), (ir.St(0),)),
            ir.Semantics(ir.Operation.MOVE, "mov", (cell,), (result,)),
        ]
    )
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
    body = _body(
        [
            ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fistp", (result,), (value,)),
            ir.Semantics(ir.Operation.MOVE, "mov", (cell,), (result,)),
        ]
    )
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
    body = _body(
        [
            ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fistp", (result,), (value,)),
        ]
    )
    allocated = floatalloc.allocated(body, frame.Frame(-8))
    assert [one.what.name for one in allocated.insns] == ["fld", "wait", "fistp", "wait"]
    assert allocated.insns[2].what.dests[0].width == width


@pytest.mark.parametrize("reader", ["opaque", "barrier", "pinned", "phi", "uses"])
def test_conversion_result_is_kept_for_non_operand_readers(reader):
    """An invisible reader must not lose the integer returned by B$FIST."""
    from qbopt.backend import frame

    value, result = ir.Held(1, 10), ir.Held(2, 4)
    cell = ir.Mem(Addr(Space.FRAME, -8), 8)
    body = _body(
        [
            ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fistp", (result,), (value,)),
        ]
    )
    block = body.blocks[0]
    match reader:
        case "pinned":
            body = replace(body, pins={2: None})
        case "phi":
            body = replace(body, blocks=(block, lir.LirBlock(32, (), phis=(lir.Phi(3, ((0, 2),)),))))
        case _:
            what = (
                None
                if reader == "opaque"
                else ir.Semantics(ir.Operation.BARRIER if reader == "barrier" else ir.Operation.NOTHING, "", (), ())
            )
            extra = lir.Insn(24, (24, 24), what, (), (2,) if reader == "uses" else ())
            body = replace(body, blocks=(replace(block, insns=(*block.insns, extra)),))
    allocated = floatalloc.allocated(body, frame.Frame(-8))
    assert any(one.what and one.what.name == "mov" and one.what.dests == (result,) for one in allocated.insns)


def test_ninth_float_uses_an_owned_extended_precision_spill():
    """Nine live FP values previously refused allocation instead of preserving 80 bits."""
    from qbopt.backend import frame

    sources = _cells(*range(-40, -4, 4))
    answers = _cells(*range(-80, -44, 4))
    loaded = [ir.Held(index, 10) for index in range(1, 10)]
    values = [ir.Held(index, 10) for index in range(11, 20)]
    body = _body(
        [
            operation
            for load, value, cell in zip(loaded, values, sources, strict=True)
            for operation in (_load(load, cell), ir.Semantics(ir.Operation.FLOAT_UNARY, "fchs", (value,), (load,)))
        ]
        + [_store(cell, value) for value, cell in zip(values, answers, strict=True)]
    )
    slots = frame.Frame(-80)
    integer_scratch = slots.cell(1, 2)
    allocated = floatalloc.allocated(body, slots)
    spills = [one for one in allocated.insns if one.what.name == "fstp" and one.what.dests[0].width == 10]
    assert len(spills) == 1 and slots.size >= 10
    assert all(one.what.dests[0].addr != integer_scratch.addr and one.covers[0] == one.covers[1] for one in spills)
    memory, stack = _x87(allocated.insns, {cell: index for index, cell in enumerate(sources, 1)})
    assert [memory[cell] for cell in answers] == [-index for index in range(1, 10)] and not stack


@pytest.mark.parametrize("path", [(0, 16, 48), (0, 32, 48), (0, 16, 16, 48)])
def test_float_survives_fork_join_and_loop_without_rereading_source(path):
    """A dominating extended value was refused at forks, joins and loop boundaries."""
    from fractions import Fraction

    from qbopt.backend import frame

    value = ir.Held(1, 10)
    cell = ir.Mem(Addr(Space.FRAME, -20), 10)
    source = ir.Mem(Addr(Space.FRAME, -10), 10)
    original = _body(
        [
            ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (source,)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (value,)),
        ]
    )
    load, store = original.insns
    body = replace(
        original,
        blocks=(
            lir.LirBlock(48, (replace(store, at=48, covers=(48, 56)),)),
            lir.LirBlock(0, (load,), (16, 32)),
            lir.LirBlock(16, (replace(store, at=16, covers=(16, 24)),), (16, 48)),
            lir.LirBlock(32, (replace(store, at=32, covers=(32, 40)),), (48,)),
        ),
    )
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
                case "":
                    pass
                case _:
                    pytest.fail(f"Unexpected allocation instruction: {what}")
        assert not stack
        memory[source.addr.disp] = -99
    assert answers == [precise] * (len(path) - 1)
    assert slots.size == 10


@pytest.mark.parametrize("defect", ["entry", "bypass", "pinned", "typed-pinned", "duplicate"])
def test_floating_bridge_never_reads_an_unestablished_slot(defect):
    """Cross-block allocation must not turn a missing definition into a frame read."""
    from qbopt.backend import frame
    from qbopt.backend.lower import Unlowered

    value = ir.Held(1, 10)
    cell = ir.Mem(Addr(Space.FRAME, -10), 10)
    body = _body(
        [
            ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (value,)),
        ]
    )
    load, store = body.insns
    blocks = (lir.LirBlock(0, (load,), (16, 32)), lir.LirBlock(16, (store,)), lir.LirBlock(32, (), (16,)))
    match defect:
        case "entry":
            body = replace(body, entry=16)
        case "bypass":
            body = replace(body, entry=48)
            blocks += (lir.LirBlock(48, (), (0, 16)),)
        case "pinned":
            body = replace(body, pins={1: 0})
        case "typed-pinned":
            from qbopt.model import mir

            body = replace(body, pins={mir.Value(1, 0): 0})
        case "duplicate":
            blocks = (replace(blocks[0], insns=(load, load)), *blocks[1:])
    with pytest.raises(Unlowered):
        floatalloc.allocated(replace(body, blocks=blocks), frame.Frame(-10))


@pytest.mark.parametrize("target", [16, 48])
def test_floating_loop_phis_swap_in_parallel_on_the_critical_backedge(target):
    """Floating loop phis were refused; serial slot copies would turn (1,2) into (2,2)."""
    from qbopt.backend import frame

    first, second, left, right = (ir.Held(index, 10) for index in range(1, 5))
    cells = [ir.Mem(Addr(Space.FRAME, -10 * index), 10) for index in (1, 2, 3)]
    body = _body(
        [
            ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (first,), (cells[0],)),
            ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (second,), (cells[1],)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cells[2],), (left,)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cells[2],), (right,)),
            ir.Semantics(ir.Operation.BRANCH, "jne", (), (), target),
        ]
    )
    body = replace(
        body,
        blocks=(
            lir.LirBlock(0, body.insns[:2], (16,)),
            lir.LirBlock(
                16,
                body.insns[2:],
                (16, 48),
                (
                    lir.Phi(3, ((0, 1), (16, 4))),
                    lir.Phi(4, ((0, 2), (16, 3))),
                    lir.Phi(93, ((0, 91), (16, 92))),
                ),
            ),
            lir.LirBlock(48, ()),
        ),
    )
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
                    stack.insert(0, stack[source.index] if isinstance(source, ir.St) else memory[source.addr.disp])
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
                case "jmp" | "jne":
                    continue
                case "":
                    pass
                case _:
                    pytest.fail(f"Unexpected allocation instruction: {what}")
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
    body = _body(
        [
            ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (value,)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (value,)),
        ]
    )
    result = floatalloc.allocated(body)
    assert [one.what.name for one in result.insns] == (
        ["fld", "fst", "fstp"] if width in (4, 8) else ["fld", "fld", "fstp", "fstp"]
    )
    assert all(select.emit(one.what) is not None for one in result.insns)


@pytest.mark.parametrize("boundary", ["linear", "reversed", "separated", "fork", "join", "entry"])
def test_shared_float_crosses_only_a_unique_straight_line_edge(boundary):
    """A shared sum was refused at a block edge despite one unchanged stack path."""
    from qbopt.backend.lower import Unlowered

    shared, product, quotient = (ir.Held(index, 10) for index in (1, 2, 3))
    cell = ir.Mem(Addr(Space.FRAME, -4), 4)
    body = _body(
        [
            ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (shared,), (cell,)),
            ir.Semantics(ir.Operation.FLOAT_ARITH, "fmul", (product,), (shared, cell)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (product,)),
            ir.Semantics(ir.Operation.FLOAT_ARITH, "fdiv", (quotient,), (shared, cell)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (quotient,)),
        ]
    )
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
    if boundary in {"fork", "entry"}:
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
    body = _body(
        [
            ir.Semantics(ir.Operation.FLOAT_LOAD, "fild", (floating,), (integer,)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (output,), (floating,)),
        ]
    )
    slots = frame.Frame(-8)
    result = floatalloc.allocated(body, slots)
    store, load, _ = [one for one in result.insns if one.what.op is not ir.Operation.NOTHING]
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
def test_buried_float_operand_is_exchanged_not_duplicated(operation):
    """A second live value must not prevent using the first, or reverse subtraction."""
    first_cell, second_cell, answer, kept = _cells(-4, -8, -12, -16)
    first_load, second_load, first, second, result = (ir.Held(index, 10) for index in range(1, 6))
    operations = [
        _load(first_load, first_cell),
        ir.Semantics(ir.Operation.FLOAT_UNARY, "fchs", (first,), (first_load,)),
        _load(second_load, second_cell),
        ir.Semantics(ir.Operation.FLOAT_UNARY, "fchs", (second,), (second_load,)),
    ]
    match operation:
        case "fsub":
            operations.append(ir.Semantics(ir.Operation.FLOAT_ARITH, "fsub", (result,), (first, second_cell)))
        case "fchs":
            operations.append(ir.Semantics(ir.Operation.FLOAT_UNARY, "fchs", (result,), (first,)))
        case "fstp":
            operations.append(_store(answer, first))
        case "fsubp":
            operations.append(ir.Semantics(ir.Operation.FLOAT_ARITH_POP, "fsubp", (result,), (second, first)))
    if operation != "fstp":
        operations.append(_store(answer, result))
    if operation != "fsubp":
        operations.append(_store(kept, second))
    allocated = floatalloc.allocated(_body(operations))
    moves = [
        one
        for one in allocated.insns
        if one.what.op is ir.Operation.EXCHANGE
        or one.what.sources
        and isinstance(one.what.sources[0], ir.St)
        and one.what.op is ir.Operation.FLOAT_LOAD
    ]
    # Both operands of the popping subtraction die, so the result overwrites one in place.
    assert [one.what.name for one in moves] == ([] if operation == "fsubp" else ["fxch"])
    assert all(one.covers[0] == one.covers[1] and not one.uses and not one.defines for one in moves)
    memory, stack = _x87(allocated.insns, {first_cell: 7, second_cell: 2})
    assert memory[answer] == {"fsub": -9, "fchs": 7, "fstp": -7, "fsubp": 5}[operation] and not stack
    assert operation == "fsubp" or memory[kept] == -2


@pytest.mark.parametrize(
    "name,popping,expected",
    [
        ("fadd", "faddp", 9),
        ("fmul", "fmulp", 14),
        ("fsub", "fsubrp", 5),
        ("fdiv", "fdivrp", 3.5),
    ],
)
@pytest.mark.parametrize("top", ["left", "right"])
def test_last_register_operand_is_consumed_without_reversing_arithmetic(name, popping, expected, top):
    """Forwarded floating memory operands must die without leaking a stack slot or reversing division."""
    left, right, result = (ir.Held(index, 10) for index in (1, 2, 3))
    # No arithmetic reads m80, so the loads stay register operands.
    cell = ir.Mem(Addr(Space.FRAME, -10), 10)
    inputs = (right, left) if top == "left" else (left, right)
    body = _body(
        [
            *(ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (value,), (cell,)) for value in inputs),
            ir.Semantics(ir.Operation.FLOAT_ARITH, name, (result,), (left, right)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (result,)),
        ]
    )
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
            case "":
                continue
            case _:
                assert what.name == (popping if top == "left" else popping.replace("rp", "p"))
                index = what.sources[0].index
                a, b = stack[index], stack[0]
                stack[index] = {
                    "faddp": lambda: a + b,
                    "fmulp": lambda: a * b,
                    "fsubp": lambda: a - b,
                    "fdivp": lambda: a / b,
                    "fsubrp": lambda: b - a,
                    "fdivrp": lambda: b / a,
                }[what.name]()
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
    body = _body(
        [
            ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (shared,), (cell,)),
            ir.Semantics(ir.Operation.FLOAT_ARITH, "fmul", (product,), (shared, cell)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (product,)),
            ir.Semantics(ir.Operation.FLOAT_ARITH, "fdiv", (quotient,), (shared, cell)),
            ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cell,), (quotient,)),
        ]
    )
    result = floatalloc.allocated(body)
    assert [one.what.name for one in result.insns] == ["fld", "fld", "fmul", "fstp", "fdiv", "fstp"]
    duplicate = result.insns[1]
    assert duplicate.what.sources == (ir.St(0),)
    assert duplicate.what.dests == (ir.St(0),)
    assert select.emit(duplicate.what).code == bytes.fromhex("d9c0")
    assert duplicate.covers == (8, 8) and duplicate.op is None
    assert result.insns[4].what.sources == (ir.St(0), cell)


def test_x87_memory_operand_follows_the_selected_cpu_cost():
    """A costly memory multiply must materialize the home instead of folding it.

    The stack allocator used to select an x87 memory form whenever it was
    encodable.  A target table that prices ``fld`` plus a register multiply
    lower must choose that sequence; ordinary public profiles retain their
    audited memory-form choice.
    """
    raw, left, right, product = (ir.Held(index, 10) for index in range(1, 5))
    source, home, out = _cells(-4, -8, -12)
    body = _body(
        [
            _load(raw, source),
            ir.Semantics(ir.Operation.FLOAT_UNARY, "fchs", (left,), (raw,)),
            _load(right, home),
            _arithmetic("fmul", product, left, right),
            _store(out, product),
        ]
    )
    costs = dict(cpu.profile("386")._costs)
    costs.update(x87_load=1, x87_mul=1, x87_mul_m=99)
    slow_memory = replace(cpu.profile("386"), name="test-x87", _costs=tuple(costs.items()))

    result = floatalloc.allocated(body, cpu=slow_memory)

    multiply = next(one.what for one in result.insns if one.what.name.startswith("fmul"))
    assert all(isinstance(arg, ir.St) for arg in multiply.sources)
    memory, stack = _x87(result.insns, {source: 7, home: 3})
    assert memory[out] == -21 and not stack


@pytest.mark.parametrize("profile", cpu.names())
def test_x87_memory_operand_matches_each_public_cpu_cost(profile):
    """Each public CPU's emitted form agrees with its own audited cost table."""
    raw, left, right, product = (ir.Held(index, 10) for index in range(1, 5))
    source, home, out = _cells(-4, -8, -12)
    body = _body(
        [
            _load(raw, source),
            ir.Semantics(ir.Operation.FLOAT_UNARY, "fchs", (left,), (raw,)),
            _load(right, home),
            _arithmetic("fmul", product, left, right),
            _store(out, product),
        ]
    )
    target = cpu.profile(profile)

    result = floatalloc.allocated(body, cpu=target)

    multiply = next(one.what for one in result.insns if one.what.name.startswith("fmul"))
    folded = any(isinstance(arg, ir.Mem) for arg in multiply.sources)
    assert folded is (target.cost("x87_mul_m") <= target.cost("x87_load") + target.cost("x87_mul"))


@pytest.mark.parametrize("profile", cpu.names())
def test_profitable_multiuse_float_home_is_retained_for_the_selected_cpu(profile):
    """C nbody reread each rounded dx/dy temporary for every force term.

    Retaining a value used by several x87 operations can avoid repeated memory
    operands, but only if its initial load and any copies needed by a self-use
    cost less for this CPU.  K5's unusually cheap memory multiply is the
    deliberate counterexample: it should continue to use the rounded home.
    """
    home, left_cell, right_cell, square_out, left_out, right_out = _cells(-4, -8, -12, -16, -20, -24)
    shared, left, right, square, left_product, right_product = (ir.Held(index, 10) for index in range(1, 7))
    body = _body(
        [
            _load(shared, home),
            _arithmetic("fmul", square, shared, shared),
            _store(square_out, square),
            _load(left, left_cell),
            _arithmetic("fmul", left_product, shared, left),
            _store(left_out, left_product),
            _load(right, right_cell),
            _arithmetic("fmul", right_product, shared, right),
            _store(right_out, right_product),
        ]
    )
    target = cpu.profile(profile)
    result = floatalloc.allocated(body, cpu=target)
    load = target.cost("x87_load")
    multiply = target.cost("x87_mul")
    memory_multiply = target.cost("x87_mul_m")
    retain_cost = 2 * load + 3 * multiply
    home_cost = load + multiply + 2 * min(memory_multiply, load + multiply)
    expected_reads = 1 if retain_cost < home_cost else 3

    assert sum(home in one.what.sources for one in result.insns) == expected_reads
    memory, stack = _x87(result.insns, {home: 7, left_cell: 2, right_cell: 3})
    assert (memory[square_out], memory[left_out], memory[right_out], stack) == (49, 14, 21, [])


def test_complete_x87_candidate_rejects_locally_profitable_overlapping_homes(monkeypatch):
    """C nbody fell from 2,085 to 2,121 estimated instructions after retaining dx.

    The individual load saving did not include the final stack exchanges.
    Force every rereadable value into the speculative candidate and prove the
    386 allocator still selects the fully allocated, cheaper home-reading form.
    """
    left_cell, right_cell, rounded, square_out, left_out, right_out = _cells(-4, -8, -12, -16, -20, -24)
    left, right, difference, shared, square, left_product, right_product = (ir.Held(index, 10) for index in range(1, 8))
    body = _body(
        [
            _load(left, left_cell),
            _load(right, right_cell),
            _arithmetic("fsub", difference, left, right),
            _store(rounded, difference),
            _load(shared, rounded),
            _arithmetic("fmul", square, shared, shared),
            _store(square_out, square),
            _arithmetic("fmul", left_product, shared, left),
            _store(left_out, left_product),
            _arithmetic("fmul", right_product, shared, right),
            _store(right_out, right_product),
        ]
    )
    monkeypatch.setattr(floatalloc._Stack, "retain_home", lambda self, _value: self.retain_homes)

    result = floatalloc.allocated(body, cpu="386")

    assert not any(one.what.name == "fxch" for one in result.insns)
    assert sum(rounded in one.what.sources for one in result.insns) == 3
    memory, stack = _x87(result.insns, {left_cell: 7, right_cell: 2})
    assert (memory[square_out], memory[left_out], memory[right_out], stack) == (25, 35, 10, [])


def test_independent_x87_regions_select_their_own_complete_candidate():
    """One bad stack region made the old whole-body choice discard a cheaper sibling.

    The first region saves a rounded-home read without adding instructions.
    The second would need an expensive 80387 exchange to retain its value, so
    its ordinary three reads must not suppress the first region's independent
    improvement.
    """
    home, left_cell, right_cell, square_out, left_out, right_out = _cells(-4, -8, -12, -16, -20, -24)
    shared, left, right, square, left_product, right_product = (ir.Held(index, 10) for index in range(1, 7))
    operations = [
        _load(shared, home),
        _arithmetic("fmul", square, shared, shared),
        _store(square_out, square),
        _load(left, left_cell),
        _arithmetic("fmul", left_product, shared, left),
        _store(left_out, left_product),
        _load(right, right_cell),
        _arithmetic("fmul", right_product, shared, right),
        _store(right_out, right_product),
        ir.Semantics(ir.Operation.BARRIER, "wait", (), ()),
    ]
    other, scale_cell, first_acc, second_acc, other_square = _cells(-28, -32, -36, -40, -44)
    value, squared, scale, product, old, added, scale_again, product_again, old_again, subtracted = (
        ir.Held(index, 10) for index in range(10, 20)
    )
    operations += [
        _load(value, other),
        _arithmetic("fmul", squared, value, value),
        _store(other_square, squared),
        _load(old, first_acc),
        _load(scale, scale_cell),
        _arithmetic("fmul", product, value, scale),
        _arithmetic("fadd", added, old, product),
        _store(first_acc, added),
        _load(old_again, second_acc),
        _load(scale_again, scale_cell),
        _arithmetic("fmul", product_again, value, scale_again),
        _arithmetic("fsub", subtracted, old_again, product_again),
        _store(second_acc, subtracted),
    ]

    result = floatalloc.allocated(_body(operations), cpu="386")

    assert sum(home in one.what.sources for one in result.insns) == 1
    assert sum(other in one.what.sources for one in result.insns) == 3
    assert not any(one.what.name == "fxch" for one in result.insns)
    memory, stack = _x87(
        result.insns,
        {home: 7, left_cell: 2, right_cell: 3, other: 5, scale_cell: 2, first_acc: 10, second_acc: 20},
    )
    assert (
        memory[square_out],
        memory[left_out],
        memory[right_out],
        memory[other_square],
        memory[first_acc],
        memory[second_acc],
        stack,
    ) == (49, 14, 21, 25, 20, 10, [])


def test_repeated_stable_float_cell_load_is_kept_across_consumers():
    """C nbody reloaded one rounded frame temporary for every force term.

    Two distinct LIR ``fld`` values read the same frame cell, with no write
    between them.  They are one floating value: retain the first stack copy
    across the first multiply rather than issue a second ``fld``.
    """
    cell, factor, first_out, second_out = _cells(-4, -8, -12, -16)
    first_load, second_load, first_product, second_product = (ir.Held(index, 10) for index in (1, 3, 5, 6))
    body = _body(
        [
            _load(first_load, cell),
            _arithmetic("fmul", first_product, first_load, factor),
            _store(first_out, first_product),
            _load(second_load, cell),
            _arithmetic("fmul", second_product, second_load, factor),
            _store(second_out, second_product),
        ]
    )

    result = floatalloc.allocated(body)

    assert sum(one.what.name == "fld" and one.what.sources == (cell,) for one in result.insns) == 1
    memory, stack = _x87(result.insns, {cell: 7, factor: 3})
    assert memory[first_out] == memory[second_out] == 21 and not stack


def test_float_cell_write_breaks_reload_equivalence():
    """A changed frame temporary must be loaded again, not reused from x87."""
    cell, out = _cells(-4, -8)
    first_load, negated, second_load = (ir.Held(index, 10) for index in range(1, 4))
    body = _body(
        [
            _load(first_load, cell),
            ir.Semantics(ir.Operation.FLOAT_UNARY, "fchs", (negated,), (first_load,)),
            _store(cell, negated),
            _load(second_load, cell),
            _store(out, second_load),
        ]
    )

    result = floatalloc.allocated(body)

    assert sum(one.what.name == "fld" and one.what.sources == (cell,) for one in result.insns) == 2
    memory, stack = _x87(result.insns, {cell: 7})
    assert memory[cell] == memory[out] == -7 and not stack


def test_volatile_float_load_breaks_reload_equivalence():
    """A volatile scalar read is observable and may see device state change."""
    cell = _cells(-4)[0]
    first, volatile, later = (ir.Held(index, 10) for index in range(1, 4))
    body = _body([_load(first, cell), _load(volatile, cell), _load(later, cell)])
    insns = list(body.insns)
    insns[1] = replace(insns[1], op=SimpleNamespace(volatile=True))

    assert floatalloc._equivalent_loads(insns) == {}


@pytest.mark.parametrize("live", ["left", "right", "both", "same", "same_dead"])
def test_popping_subtraction_preserves_reused_values(live):
    """Shared FP values were refused when FSUBP consumed an operand used later."""
    left_cell, right_cell, out, again, other = _cells(-4, -8, -12, -16, -20)
    left_load, right_load, left, right, result = (ir.Held(index, 10) for index in range(1, 6))
    operations = [_load(left_load, left_cell), ir.Semantics(ir.Operation.FLOAT_UNARY, "fchs", (left,), (left_load,))]
    if live in {"same", "same_dead"}:
        right = left
    else:
        operations += [
            _load(right_load, right_cell),
            ir.Semantics(ir.Operation.FLOAT_UNARY, "fchs", (right,), (right_load,)),
        ]
    operations += [ir.Semantics(ir.Operation.FLOAT_ARITH_POP, "fsubp", (result,), (left, right)), _store(out, result)]
    if live in {"left", "both", "same"}:
        operations.append(_store(again, left))
    if live in {"right", "both"}:
        operations.append(_store(other, right))
    memory, stack = _x87(floatalloc.allocated(_body(operations)).insns, {left_cell: 7, right_cell: 2})
    expected = {
        "left": {out: -5, again: -7},
        "right": {out: -5, other: -2},
        "both": {out: -5, again: -7, other: -2},
        "same": {out: 0, again: -7},
        "same_dead": {out: 0},
    }[live]
    assert {cell: memory[cell] for cell in expected} == expected and not stack


@pytest.mark.parametrize("native", [False, True])
def test_inserted_stack_move_keeps_the_anchor_emulator_mode(native):
    """An inserted FLD beside FPCSE must not silently require a coprocessor under /FPi."""
    from pathlib import Path

    import corpus
    from qbopt.model import lir
    from qbopt.backend import asm

    found = corpus.loaded(Path("fixtures/omf/fpcse-p-g2.obj"))
    at = 0x66
    assert found.code[at : at + 2] == bytes.fromhex("cd35")
    what = ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),), (ir.St(0),))
    op = lir.Insn(at, (at, at), what, (), ())
    result = asm.assemble([op], at, found, native_fpu=native)
    assert not isinstance(result, str), result
    assert result.code == bytes.fromhex("d9c0" if native else "cd35c0")
