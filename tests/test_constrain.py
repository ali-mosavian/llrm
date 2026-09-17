"""
qbopt/backend/constrain.py: a required register gets a value of its own.

`imul`'s product is dx:ax and it names neither. While the operands were the
registers BC had, that was satisfied by not moving anything; once lowering
hands the allocator values, the requirement is about one instruction and
the value may be anywhere a moment later. pressx placed the halves in cx
and ax and printed R= 6460 for 7500.
"""

import pytest
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import constrain


def _body(*insns) -> lir.LirBody:
    return lir.LirBody("one", 0, (lir.LirBlock(at=0, insns=insns, succ=()),), origin={}, pins={})


def _insn(what, defines, uses, at=0x100) -> lir.Insn:
    return lir.Insn(at=at, covers=(at, at + 2), what=what, defines=defines, uses=uses, op=None)


def test_call_result_spill_keeps_its_register_after_input_split():
    """H_BENCH printed ft_n=0 and infinite ft_mean: SI's result was spilled from BX."""
    from dataclasses import replace

    from qbopt.backend import spiller
    from qbopt.backend import allocate
    from qbopt.backend import frame as frames

    argument, result = ir.Held(1, 2), ir.Held(2, 2)
    source = _insn(ir.Semantics(ir.Operation.MOVE, "mov", (argument,), (ir.Imm(7, 2),)), (1,), ())
    call = replace(
        _insn(ir.Semantics(ir.Operation.CALL, "call", (), ()), (2,), (1,), at=0x108),
        requires=((argument, Register.AX),),
        delivers=((result, Register.SI),),
    )
    use = _insn(ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(3, 2),), (result,)), (3,), (2,), at=0x110)
    split, pins = constrain.constrained(_body(source, call, use), {2: Register.SI})
    spilled, _ = spiller.spilled(split, frozenset({2}), frames.Frame(0))
    assignment = allocate.allocate(spilled, {**pins, **constrain.required(spilled)})
    placed = allocate.applied(spilled, assignment)
    call_index = next(i for i, one in enumerate(placed.insns) if one.what.op is ir.Operation.CALL)
    store = placed.insns[call_index + 1]
    assert isinstance(store.what.dests[0], ir.Mem)
    assert store.what.sources == (ir.Reg(Register.SI, 2),)


def test_runtime_requirement_renames_its_explicit_source():
    """JUMPS refused emission: ON GOTO read old v20 while its required input became v143."""
    from dataclasses import replace

    value = ir.Held(20, 2)
    call = replace(
        _insn(ir.Semantics(ir.Operation.CALL, "call", (), (value,)), (), (20,)), requires=((value, Register.AX),)
    )
    got, _ = constrain.constrained(_body(call))
    result = next(one for one in got.insns if one.what.op is ir.Operation.CALL)
    assert result.what.sources[0].value == result.uses[0]


def test_spilled_segment_load_still_sets_es():
    """D_SURF returned sc_test=-4000: spilling ES left a far load on the old segment."""
    from iced_x86 import Decoder

    from qbopt.backend import target

    from qbopt.model import mir
    from qbopt.backend import lower
    from qbopt.backend import select
    from qbopt.backend import spiller
    from qbopt.backend import allocate
    from qbopt.objectfile.module import Addr
    from qbopt.backend import frame as frames
    from qbopt.objectfile.module import Space

    segment = mir.Value(1, 0xFCC)
    cell = mir.Cell(mir.MemRef(Addr(Space.FRAME, -2), 2))
    op = mir.Op(
        0xFCC,
        ir.Operation.MOVE,
        "mov",
        (segment,),
        (),
        kind=mir.Kind.LOAD,
        args=(cell,),
        results=(mir.Held(segment, 2),),
    )
    context = mir.MirBody(0xFCC, (mir.MirBlock(0xFCC, (), (op,), ()),), origin={segment: Register.ES})
    (load,) = lower.Lowering(context, {segment.id}, {}, (), {}).expand(op)
    far = ir.Mem(Addr(Space.FAR, 0x10, segment=Register.ES), 2, selector=ir.Held(segment.id, 2))
    use = _insn(ir.Semantics(ir.Operation.MOVE, "mov", (far,), (ir.Imm(7, 2),)), (), (segment.id,), at=0xFCF)
    body, pins = constrain.constrained(_body(load, use), {})
    spilled, _ = spiller.spilled(body, frozenset({segment.id}), frames.Frame(0))
    placed = allocate.applied(spilled, allocate.allocate(spilled, {**pins, **constrain.required(spilled)}))
    reload, access = (next(iter(Decoder(16, select.emit(one.what).code))) for one in placed.insns)
    assert reload.op0_register in target.SELECTORS
    assert access.segment_prefix == reload.op0_register, "the far access reads a segment the reload did not set"


@pytest.mark.parametrize("selected_site", [False, True])
def test_far_read_restores_its_forwarded_selector(selected_site):
    """D_SURF sc_test=-4000: a reused slot selector read through the LRU array's ES."""
    from qbopt.model import mir
    from qbopt.backend import lower
    from qbopt.backend import allocate
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space
    from types import SimpleNamespace

    segment, result = mir.Value(1, 0), mir.Value(2, 8)
    addr = Addr(Space.FAR, 0, segment=Register.ES)
    ref = mir.MemRef(addr, 2, segment=segment)
    machine = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), (ir.Mem(addr, 2, Register.BX),))
    op = mir.Op(
        8,
        ir.Operation.MOVE,
        "mov",
        (result,),
        (segment,),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(ref),),
        results=(mir.Held(result, 2),),
        loads=(ref,),
        source_backed=True,
        id=8,
        raised=((), ()),
    )
    context = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),), origin={segment: Register.ES})
    sites = {op.id: ()} if selected_site else {}
    (read,) = lower.Lowering(
        context, {1, 2}, {}, sites, {}, nodes={op.id: SimpleNamespace(semantics=machine)}
    ).expand(op)
    assert segment.id in read.uses, "the selector must remain live until the far read"
    saved = _insn(
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Mem(Addr(Space.FRAME, -2), 2, Register.BP),)),
        (1,),
        (),
    )
    overwrite = _insn(
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.ES, 2),), (ir.Reg(Register.DX, 2),)), (), (), at=4
    )
    body = allocate.explicit_selectors(_body(saved, overwrite, read), {1: Register.CX})
    body, pins = constrain.constrained(body, {1: Register.CX})
    placed = allocate.applied(body, allocate.allocate(body, {1: Register.CX, **pins}))
    restore = placed.insns[-2].what
    assert restore.dests == (ir.Reg(Register.ES, 2),)
    assert restore.sources == (ir.Reg(Register.CX, 2),)


@pytest.mark.parametrize("registers", [(Register.BX, Register.CX), (Register.AX, Register.BX, Register.CX)])
def test_shared_zero_is_supplied_to_every_runtime_input(registers):
    """QGLDIFF hung in heap compaction: ENRA lost BX=0 after CSE joined its CX=0."""
    from dataclasses import replace

    from qbopt.backend import allocate

    value = ir.Held(1, 2)
    constant = _insn(ir.Semantics(ir.Operation.MOVE, "mov", (value,), (ir.Imm(0, 2),)), (1,), ())
    call = replace(
        _insn(ir.Semantics(ir.Operation.CALL, "call", (), ()), (), (1,), at=0x108),
        requires=tuple((value, register) for register in registers),
    )
    body, pins = constrain.constrained(_body(constant, call))
    placed = allocate.applied(body, allocate.allocate(body, pins))
    zeros = {
        one.what.dests[0].register
        for one in placed.insns
        if one.what.name == "mov" and one.what.sources == (ir.Imm(0, 2),)
    }
    assert set(registers) <= zeros
    assert {pins[value] for value in body.insns[-1].uses} == set(registers)


def test_fixed_address_requirement_renames_memory_base():
    """Qrender FIDIV 035c retained unplaced v398 after its SI input became v1036."""
    from dataclasses import replace

    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    value = ir.Held(20, 2)
    cell = ir.Mem(Addr(Space.LITERAL, 0), 2, base=value)
    instruction = replace(
        _insn(ir.Semantics(ir.Operation.FLOAT_ARITH, "fidiv", (ir.St(0),), (ir.St(0), cell)), (), (20,)),
        requires=((value, Register.SI),),
    )
    got, pins = constrain.constrained(_body(instruction))
    result = got.insns[-1]
    assert result.what.sources[-1].base.value == result.uses[0]
    assert pins[result.uses[0]] == Register.SI
    from qbopt.backend import select
    from qbopt.backend import allocate

    placed = allocate.applied(got, allocate.allocate(got, pins))
    emitted = select.emit(placed.insns[-1].what)
    assert emitted is not None
    assert emitted.code == bytes.fromhex("de34")  # fidiv word [si]


def _shift(count: int) -> lir.Insn:
    what = ir.Semantics(ir.Operation.BINARY, "shl", (ir.Held(9, 2),), (ir.Held(9, 2), ir.Held(count, 2)))
    return _insn(what, (9,), (9, count))


def _extend(one: int, into: int) -> lir.Insn:
    return _insn(ir.Semantics(ir.Operation.EXTEND, "cwd", (ir.Held(into, 2),), (ir.Held(one, 2),)), (into,), (one,))


def test_fixed_input_rematerializes_constant_without_retaining_source():
    """Nbody kept 512 in EDI while copying it into EAX, displacing its accumulator."""
    constant = _insn(ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Imm(512, 2),)), (1,), ())
    done, pins = constrain.constrained(_body(constant, _extend(1, 2)))
    prepared = next(one for one in done.insns if one.defines and pins.get(one.defines[0]) == Register.EAX)
    assert prepared.what.sources == (ir.Imm(512, 2),)
    assert not prepared.uses


def _multiply(low: int, high: int, by: int) -> lir.Insn:
    what = ir.Semantics(
        ir.Operation.MULTIPLY,
        "imul",
        (ir.Held(low, 2), ir.Held(high, 2)),
        (ir.Held(low, 2), ir.Held(by, 2)),
    )
    return _insn(what, (low, high), (low, by))


def _shape(body) -> list[str]:
    out = []
    for block in body.blocks:
        for one in block.insns:
            w = one.what
            out.append(f"{w.name} {w.dests} <- {w.sources}")
    return out


def test_a_required_source_is_copied_in_before_the_instruction() -> None:
    """A shift counts from cl and names it nowhere."""
    got, pins = constrain.constrained(_body(_shift(7)))
    assert len(got.blocks[0].insns) == 2
    first, then = got.blocks[0].insns
    assert first.what.name == "mov" and first.what.sources == (ir.Held(7, 2),)
    fresh = first.what.dests[0].value
    assert then.what.sources[1] == ir.Held(fresh, 2)
    assert pins == {fresh: Register.ECX}
    assert 7 not in pins, "the original keeps no hardware requirement"


def test_a_required_destination_is_copied_out_after_the_instruction() -> None:
    """`cwd` writes dx and names it nowhere."""
    got, pins = constrain.constrained(_body(_extend(1, 8)))
    insns = got.blocks[0].insns
    assert len(insns) == 3, _shape(got)
    assert insns[-1].what.name == "mov" and insns[-1].what.dests == (ir.Held(8, 2),)
    fresh = insns[-1].what.sources[0].value
    assert pins[fresh] == Register.EDX and 8 not in pins


def test_a_tied_source_and_destination_share_one_fresh_value() -> None:
    """`imul`'s low half is its first source and its first destination.
    Two fresh values would name two registers and the tie would be gone."""
    got, pins = constrain.constrained(_body(_multiply(1, 2, 3)))
    insns = got.blocks[0].insns
    middle = [one for one in insns if one.what.name == "imul"]
    assert len(middle) == 1
    low_in, low_out = middle[0].what.sources[0], middle[0].what.dests[0]
    assert low_in == low_out, f"the tie was broken: {low_in} vs {low_out}"
    assert pins[low_in.value] == Register.EAX
    assert pins[middle[0].what.dests[1].value] == Register.EDX
    assert 1 not in pins and 2 not in pins


def test_one_value_required_in_two_registers_is_refused() -> None:
    """A value that is both a shift's count and a multiply's high half
    cannot be placed at all, and saying so beats placing it in one of
    them."""
    what = ir.Semantics(
        ir.Operation.MULTIPLY,
        "imul",
        (ir.Held(1, 2), ir.Held(7, 2)),
        (ir.Held(7, 2), ir.Held(3, 2)),
    )
    with pytest.raises(constrain.Impossible, match="two registers"):
        constrain.constrained(_body(_insn(what, (1, 7), (7, 3))))


def test_an_explicit_word_requirement_and_its_root_are_one_register() -> None:
    """PROCS-P-OT crashed rebuilding TWICE's `rep stosw`: lowering required
    its value in AX while the target described the same operand at its EAX
    allocation root, and constrain called those two registers at once."""
    value, count, address = ir.Held(11, 2), ir.Held(8, 2), ir.Held(9, 2)
    what = ir.Semantics(
        ir.Operation.FILL,
        "stosw",
        (ir.Mem(None, 0), ir.Held(12, 2), ir.Held(13, 2)),
        (value, count, address, ir.Reg(Register.ES, 2)),
    )
    fill = lir.Insn(
        at=0x104,
        covers=(0x104, 0x106),
        what=what,
        defines=(12, 13),
        uses=(11, 8, 9),
        requires=((value, Register.AX), (count, Register.CX), (address, Register.DI)),
    )

    got, pins = constrain.constrained(_body(fill))

    filled = next(one for one in got.insns if one.what.op is ir.Operation.FILL)
    assert len(filled.what.sources) == 4
    assert {ir.ROOT[where] for where in pins.values()} == {Register.EAX, Register.ECX, Register.EDI}


def test_the_helper_moves_belong_to_no_parallel_copy() -> None:
    got, _pins = constrain.constrained(_body(_shift(7)))
    assert all(one.group is None for one in got.blocks[0].insns)
    assert got.blocks[0].insns[0].covers == (0x100, 0x100), "a helper claims no bytes"


def test_a_value_a_call_reads_in_a_register_it_names_nowhere() -> None:
    """A runtime routine takes its arguments in fixed registers and
    mentions none of them, so no occurrence can say where they go.

    Pinning the argument itself would fix it everywhere it lives; what is
    pinned is a value of its own, live from the copy to the call. arrprm's
    B$ENRA wanted cx and bx and got si and di.
    """
    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.model import lir
    from qbopt.backend import constrain

    call = lir.Insn(
        at=0x106,
        covers=(0x106, 0x10B),
        what=ir.Semantics(ir.Operation.CALL, "call", (), ()),
        defines=(11,),
        uses=(2,),
        op=None,
        requires=((ir.Held(2, 2), Register.CX),),
    )
    body = _body(call)
    got, pins = constrain.constrained(body)
    insns = got.blocks[0].insns
    assert len(insns) == 2, f"expected one copy before the call, got {len(insns)}"
    moved, after = insns
    assert moved.what.op is ir.Operation.MOVE and moved.what.sources == (ir.Held(2, 2),)
    fresh = moved.defines[0]
    assert pins == {fresh: Register.CX}, f"pinned {pins}"
    assert 2 not in pins, "the argument itself was pinned"
    assert after.uses == (fresh,), f"the call still reads {after.uses}"
    assert after.requires == ((ir.Held(fresh, 2), Register.CX),), "a later spill must retain the ABI slot"

    # The returned pin map makes a retained requirement already satisfied.
    again, more = constrain.constrained(got, pins)
    assert again == got and not more, "constraining twice is not constraining once"


def test_a_value_already_in_the_register_a_call_needs_is_not_split() -> None:
    """The split was supposed to free the value, and made it unplaceable.

    A call result is pinned to its 32-bit root; the ABI requirement names
    the 16-bit half of that same root. Splitting inserts `fresh <- value`
    with both ends pinned to one physical register, and neither can be
    placed: procs-p-g2 reported `value#11 at width 2 has no register`.
    """
    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.model import lir
    from qbopt.backend import constrain

    call = lir.Insn(
        at=0x10,
        covers=(0x10, 0x15),
        what=ir.Semantics(ir.Operation.CALL, "call", (), ()),
        defines=(),
        uses=(11,),
        op=None,
        requires=((ir.Held(11, 2), Register.CX),),
    )
    # The pin arrives the way the real one does: beside the body, from the
    # raise, not inside it -- which is the shape procs-p-g2 has.
    got, pins = constrain.constrained(_body(call), {11: Register.ECX})
    insns = got.blocks[0].insns
    assert len(insns) == 1, f"a copy was inserted for a value already there: {len(insns)}"
    assert got.blocks[0].insns[0].uses == (11,), f"the call reads {insns[0].uses}"
    assert 11 not in pins or pins[11] == Register.ECX, f"it was re-pinned to {pins.get(11)}"
    assert not any(where is Register.CX and value != 11 for value, where in pins.items()), (
        f"a fresh value was pinned onto the same register: {pins}"
    )


def test_a_value_already_in_the_register_an_idiom_wants_is_not_copied() -> None:
    """The declared width is the only statement an idiom makes of one.

    `_width` reads the semantics, and an operation that names no operand
    has none -- the restore idiom is three instructions behind one node.
    Asked at a word, its answer looked unlike the eax it was already
    pinned to, so a copy went in that could not be placed (pinned to eax
    beside the value it copied) and was spilled and reloaded for nothing:
    four instructions in lngmix's loop, every iteration.
    """
    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.model import lir
    from qbopt.backend import constrain

    held = ir.Held(2, 4)
    one = lir.Insn(
        at=0x10,
        covers=(0x10, 0x14),
        what=ir.Semantics(ir.Operation.RESTORE, "restore", (), ()),
        defines=(),
        uses=(2,),
        requires=((held, Register.EAX),),
    )
    body = lir.LirBody(
        name="one",
        entry=0x10,
        blocks=(lir.LirBlock(at=0x10, insns=(one,)),),
        origin={},
        pins={},
    )
    got, fixed = constrain.constrained(body, {2: Register.EAX})
    assert fixed == {}, f"a copy was minted for a value already in eax: {fixed}"
    assert got.insns[0].uses == (2,), "the instruction was given a fresh value it did not need"
