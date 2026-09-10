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


def test_runtime_requirement_renames_its_explicit_source():
    """JUMPS refused emission: ON GOTO read old v20 while its required input became v143."""
    from dataclasses import replace
    value = ir.Held(20, 2)
    call = replace(_insn(ir.Semantics(ir.Operation.CALL, "call", (), (value,)), (), (20,)),
                   requires=((value, Register.AX),))
    got, _ = constrain.constrained(_body(call))
    result = next(one for one in got.insns if one.what.op is ir.Operation.CALL)
    assert result.what.sources[0].value == result.uses[0]


def test_fixed_address_requirement_renames_memory_base():
    """Qrender FIDIV 035c retained unplaced v398 after its SI input became v1036."""
    from dataclasses import replace
    from qbopt.objectfile.module import Addr, Space
    value = ir.Held(20, 2)
    cell = ir.Mem(Addr(Space.LITERAL, 0), 2, base=value)
    instruction = replace(_insn(ir.Semantics(ir.Operation.FLOAT_ARITH, "fidiv",
        (ir.St(0),), (ir.St(0), cell)), (), (20,)), requires=((value, Register.SI),))
    got, pins = constrain.constrained(_body(instruction))
    result = got.insns[-1]
    assert result.what.sources[-1].base.value == result.uses[0]
    assert pins[result.uses[0]] == Register.SI
    from qbopt.backend import allocate, select
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
    assert after.requires == (), "the requirement was not consumed"

    # Asking again changes nothing: a consumed requirement does not split
    # the split, which is what an unterminating constrain loop looks like.
    again, more = constrain.constrained(got)
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
