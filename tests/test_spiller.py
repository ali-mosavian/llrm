"""
qbopt/backend/spiller.py: a value the allocator would not keep, kept in memory.

The case this file exists for beyond the ordinary one: a move that belongs
to a phi's parallel copy. Spilling it the usual way -- a reload before, a
store after -- puts an instruction inside a group whose moves happen at
once, and parcopy.py then sees two runs instead of one. pressx-v-evt
emitted `r24 <- [bp-8]` and then `r27 <- r24`, and R came out 6460 for 7500.
"""

from pathlib import Path
from dataclasses import replace

import pytest
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import spiller
from qbopt.objectfile.module import Addr
from qbopt.backend import frame as frames
from qbopt.objectfile.module import Space


def _move(into, out_of, group=None, at=0x100) -> lir.Insn:
    what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(into, 2),), (ir.Held(out_of, 2),))
    return lir.Insn(at=at, covers=(at, at), what=what, defines=(into,), uses=(out_of,), group=group, op=None)


def _add(into, out_of, at=0x100) -> lir.Insn:
    what = ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(into, 2),), (ir.Held(into, 2), ir.Held(out_of, 2)))
    return lir.Insn(at=at, covers=(at, at), what=what, defines=(into,), uses=(into, out_of), op=None)


def _body(*insns) -> lir.LirBody:
    return lir.LirBody("one", 0, (lir.LirBlock(at=0, insns=insns, succ=()),), origin={}, pins={})


def _out(body, values):
    got, _made = spiller.spilled(body, frozenset(values), frames.Frame(0))
    return [one for block in got.blocks for one in block.insns]


def test_a_spilled_move_source_loads_straight_into_its_short_successor() -> None:
    """r_walk spilled the long-lived face before its short shifted copy.

    Reloading the face into a new temporary before `mov shifted, face` made
    three simultaneous registers necessary.  x86 can read the spill slot
    directly into the short successor; the spill rewrite must preserve that
    legal move form instead of introducing a reload interval.
    """
    insns = _out(_body(_move(1, 3, at=0x10), _move(2, 1, at=0x11)), {1})
    copied = next(one for one in insns if one.what and one.what.dests == (ir.Held(2, 2),))
    assert isinstance(copied.what.sources[0], ir.Mem)
    assert copied.uses == ()


def test_a_spilled_byte_move_loads_straight_into_its_constrained_child() -> None:
    """r_walk reloaded a masked face byte before copying it into the shift count.

    `mov cl, byte ptr [slot]` is as legal as the word direct-move fold.  The
    old width gate instead created a byte reload interval and a second copy,
    which became unplaceable under the loop's far-address pressure.  A spill
    source that feeds a byte successor must therefore be folded directly too.
    """
    made = lir.Insn(
        at=0x10,
        covers=(0x10, 0x10),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 1),), (ir.Held(3, 1),)),
        defines=(1,),
        uses=(3,),
        op=None,
    )
    child = lir.Insn(
        at=0x11,
        covers=(0x11, 0x11),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(2, 1),), (ir.Held(1, 1),)),
        defines=(2,),
        uses=(1,),
        op=None,
    )
    insns = _out(_body(made, child), {1})
    copied = next(one for one in insns if one.what and one.what.dests == (ir.Held(2, 1),))
    assert isinstance(copied.what.sources[0], ir.Mem)
    assert copied.what.sources[0].width == 1
    assert copied.uses == ()


def test_a_short_update_is_completed_before_its_result_is_spilled() -> None:
    """qbsp turned ``nodenr * 6`` into a frame RMW before every update.

    A copied result that is immediately updated has a one-instruction local
    split: keep it in the copy's register and store the final value once.
    Long-lived later uses still reload the spill slot normally.
    """
    copied = _move(2, 1, at=0x10)
    shifted = lir.Insn(
        at=0x11,
        covers=(0x11, 0x11),
        what=ir.Semantics(
            ir.Operation.BINARY,
            "shl",
            (ir.Held(2, 2),),
            (ir.Held(2, 2), ir.Imm(1, 1)),
        ),
        defines=(2,),
        uses=(2,),
        op=None,
    )

    insns = _out(_body(copied, shifted), {2})
    update = next(one for one in insns if one.what is not None and one.what.name == "shl")
    spill = next(one for one in insns if one.spill_store)

    assert isinstance(update.what.dests[0], ir.Held)
    assert isinstance(update.what.sources[0], ir.Held)
    assert insns.index(spill) > insns.index(update)


def test_a_short_update_leaves_a_copy_source_that_is_still_live() -> None:
    """nbody's inner loop base `final = start + distance` overwrote `start`.

    The update ran in the copy source's register: `add ax, bx` for
    `mov final, start; add final, bx` with `start` still read by the outer
    latch, which then advanced the wrong value and the answer drifted.
    """
    copied = _move(2, 1, at=0x10)
    shifted = lir.Insn(
        at=0x11,
        covers=(0x11, 0x11),
        what=ir.Semantics(
            ir.Operation.BINARY,
            "shl",
            (ir.Held(2, 2),),
            (ir.Held(2, 2), ir.Imm(1, 1)),
        ),
        defines=(2,),
        uses=(2,),
        op=None,
    )

    insns = _out(_body(copied, shifted, _move(3, 1, at=0x12)), {2})
    read = next(index for index, one in enumerate(insns) if one.defines == (3,))

    assert not any(1 in one.defines for one in insns[:read])


def test_slot_is_as_wide_as_the_widest_use_of_its_value():
    """snd_mix_frame spilled a value first seen as a word, then stored all four
    bytes of it: `mov dword ptr [bp-2], eax` over the saved BP, and a 4-byte
    store into `n`'s word ran into `endt`, so the mixer never returned."""

    def op(name, into, width, *sources):
        what = ir.Semantics(
            ir.Operation.MOVE if name == "mov" else ir.Operation.BINARY,
            name,
            (ir.Held(into, width),),
            tuple(ir.Held(one, width) for one in sources),
        )
        return lir.Insn(
            at=0x100, covers=(0x100, 0x100), what=what, defines=(into,), uses=tuple(dict.fromkeys(sources)), op=None
        )

    body = _body(op("mov", 1, 2, 3), op("mov", 2, 2, 3), op("add", 1, 4, 1, 3), op("add", 2, 2, 2, 3))
    frame = frames.Frame(0)
    got, _made = spiller.spilled(body, frozenset({1, 2}), frame)
    cells = {
        (where.addr.disp, where.width)
        for one in got.insns
        if one.what is not None
        for where in (*one.what.dests, *one.what.sources)
        if isinstance(where, ir.Mem) and where.addr is not None and where.addr.space is Space.FRAME
    }
    assert cells and all(-frame.size <= disp and disp + width <= 0 for disp, width in cells), (cells, frame.size)
    spans = {disp: max(width for other, width in cells if other == disp) for disp, _ in cells}
    assert all(a + spans[a] <= b or b + spans[b] <= a for a in spans for b in spans if a != b), spans


@pytest.mark.parametrize(
    "between", ["call sparing the frame", "call", "call unstated", "pointer store sparing the frame", "pointer store"]
)
def test_parameter_is_reloaded_across_what_spares_the_frame(between):
    """ent_move_plats copied `world`, loaded from [bp+6], to a slot of its own:
    a call, and a store through a pointer like `mov es:[bx+10],ax`, were taken
    to write the frame even where their stores said they could not."""
    from iced_x86 import Register

    from qbopt.model import mir

    param = ir.Mem(Addr(Space.FRAME, 6), 2, Register.BP, 0, 2)
    load = lir.Insn(
        at=0x100,
        covers=(0x100, 0x100),
        op=None,
        defines=(1,),
        uses=(),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (param,)),
    )
    spared = (mir.WHOLE_FRAME,) if between.endswith("sparing the frame") else ()
    if between.startswith("call"):
        reach = () if between == "call unstated" else (mir.MemRef(None, 4, excludes=spared),)
        site = mir.Op(0x101, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.CALL, stores=reach)
        if between != "call unstated":
            site = replace(site, memory_complete=True)
        middle = lir.Insn(
            at=0x101,
            covers=(0x101, 0x101),
            op=site,
            defines=(),
            uses=(),
            what=ir.Semantics(ir.Operation.CALL, "call", (), ()),
            clobbers=frozenset({Register.EAX}),
        )
    else:
        base = mir.Value(9, 0)
        ref = mir.MemRef(Addr(Space.LITERAL, 10), 2, base=base, space=Space.LITERAL, base_width=2, excludes=spared)
        site = mir.Op(0x101, ir.Operation.NOTHING, "", (), (base,), kind=mir.Kind.STORE, stores=(ref,))
        cell = ir.Mem(Addr(Space.LITERAL, 10), 2, Register.BX, 10, 2)
        middle = lir.Insn(
            at=0x101,
            covers=(0x101, 0x101),
            op=site,
            defines=(),
            uses=(),
            what=ir.Semantics(ir.Operation.MOVE, "mov", (cell,), (ir.Imm(0, 2),)),
        )
    got = _out(_body(load, middle, _add(2, 1, at=0x102)), {1})
    slots = [
        where
        for one in got
        if one.what is not None
        for where in one.what.dests
        if isinstance(where, ir.Mem) and where.addr is not None and where.addr.space is Space.FRAME
    ]
    assert (slots == []) == bool(spared), [one.what for one in got]


def _parameter_across(store: lir.Insn) -> tuple[lir.LirBody, frames.Frame, ir.Mem]:
    parameter = ir.Mem(Addr(Space.FRAME, 6), 2, Register.BP, 0, 2)
    load = lir.Insn(
        at=0x100,
        covers=(0x100, 0x100),
        op=None,
        defines=(1,),
        uses=(),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (parameter,)),
    )
    frame = frames.Frame(0)
    result, _made = spiller.spilled(_body(load, store, _add(3, 1, at=0x102)), frozenset({1}), frame)
    return result, frame, parameter


def _assert_parameter_rematerialized(store: lir.Insn) -> None:
    result, frame, parameter = _parameter_across(store)
    assert 1 not in frame.slots
    add = next(one for one in result.insns if one.what is not None and one.what.name == "add")
    assert parameter in add.what.sources


def test_parameter_rematerializes_across_an_exact_disjoint_local_store() -> None:
    """Fully specialized matmul copied ``seed`` to a private spill slot.

    Its initializer writes many exact negative BP offsets between the argument
    load and its uses.  Conservative MIR provenance describes those as frame
    writes, but allocated LIR proves none overlaps the positive argument slot;
    the original cell is the cheaper and already initialized spill home.
    """
    from qbopt.model import mir

    local = ir.Mem(Addr(Space.FRAME, -2), 2, Register.BP, 0, 2)
    vague_local = mir.MemRef(None, 2, space=Space.FRAME)
    store = lir.Insn(
        at=0x101,
        covers=(0x101, 0x101),
        op=mir.Op(0x101, ir.Operation.MOVE, "mov", (), (2,), kind=mir.Kind.STORE, stores=(vague_local,)),
        defines=(),
        uses=(2,),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (local,), (ir.Held(2, 2),)),
    )
    _assert_parameter_rematerialized(store)


def test_parameter_rematerializes_across_a_proven_local_array_store() -> None:
    """Matmul copied ``seed`` while initializing an indexed local array.

    The selected destination is dynamic, so its final LIR address alone does
    not prove a fixed disjoint range.  MIR nevertheless identifies the whole
    destination as one current-activation FRAME object, which cannot overlap
    the positive BP-relative incoming argument cell.
    """
    from qbopt.model import mir
    from qbopt.model import memory

    local = ir.Mem(
        Addr(Space.FRAME, -132),
        2,
        Register.BP,
        0,
        2,
        index=ir.Held(4, 2),
        index_through=Register.SI,
    )
    object_ = memory.Object(memory.Kind.FRAME, (7, -132, -4), extent=128)
    indexed_local = mir.MemRef(
        None,
        2,
        base=mir.Value(4, 0),
        space=Space.FRAME,
        provenance=memory.Provenance.one(object_, 0, 128, stride=2, width=2),
    )
    store = lir.Insn(
        at=0x101,
        covers=(0x101, 0x101),
        op=mir.Op(
            0x101,
            ir.Operation.MOVE,
            "mov",
            (),
            (2, 4),
            kind=mir.Kind.STORE,
            stores=(indexed_local,),
        ),
        defines=(),
        uses=(2, 4),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (local,), (ir.Held(2, 2),)),
    )
    _assert_parameter_rematerialized(store)


def test_nbody_reads_spilled_position_directly_in_subtraction():
    """NBODY loaded [BP-2Ch] into EAX solely for SUB ESI,EAX on every force pair."""
    from qbopt import wholeseg

    states = []

    def watch(stage, name, body):
        if stage == "peephole" and body.entry == 0x30:
            states.append(body)

    result = wholeseg.emitted(Path("fixtures/bench/nbody-v-g3.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    assert any(
        one.at == 0x122 and one.what is not None and one.what.name == "sub" and isinstance(one.what.sources[-1], ir.Mem)
        for one in states[0].insns
    )


def test_nbody_compares_spilled_bound_without_scratch_reload():
    """NBODY reloaded its saved step bound into EBX solely for the outer-loop CMP."""
    from qbopt import wholeseg

    states = []

    def watch(stage, name, body):
        if stage == "peephole" and body.entry == 0x30:
            states.append(body)

    result = wholeseg.emitted(Path("fixtures/bench/nbody-v-g3.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    header = next(block for block in states[0].blocks if block.at == 0x2F0)
    assert any(
        one.what is not None and one.what.name == "cmp" and isinstance(one.what.sources[-1], ir.Mem)
        for one in header.insns
    )


def test_nbody_loop_initializers_do_not_reload_a_spilled_zero():
    """NBODY stored zero at [BP-1Ch] and read it for five separate loop initializers."""
    from qbopt import wholeseg

    states = []

    def watch(stage, name, body):
        if stage == "peephole" and body.entry == 0x30:
            states.append(body)

    result = wholeseg.emitted(Path("fixtures/bench/nbody-v-g3.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    initializer = next(block for block in states[0].blocks if block.at == 0x5F)
    assert not any(
        one.at == 0x5F and one.what is not None and any(isinstance(dest, ir.Mem) for dest in one.what.dests)
        for one in initializer.insns
    )


@pytest.mark.parametrize("width", [2, 4])
@pytest.mark.parametrize("name", ["add", "sub", "and", "or", "xor"])
def test_untied_spill_source_is_read_directly_by_arithmetic(name, width):
    """NBODY reloaded spilled position/acceleration operands into scratch registers before arithmetic."""
    from dataclasses import replace

    op = _add(1, 2)
    op = replace(
        op, what=replace(op.what, name=name, dests=(ir.Held(1, width),), sources=(ir.Held(1, width), ir.Held(2, width)))
    )
    result = _out(_body(op), {2})
    assert len(result) == 1
    assert result[0].what.sources[0] == ir.Held(1, width)
    assert isinstance(result[0].what.sources[1], ir.Mem)
    assert result[0].what.sources[1].width == width
    assert result[0].uses == (1,)


@pytest.mark.parametrize("width", [2, 4])
def test_untied_spill_source_is_read_directly_by_multiply(width: int) -> None:
    """A commuted matmul factor needed no scratch reload before ``imul``."""
    op = lir.Insn(
        at=0x100,
        covers=(0x100, 0x103),
        what=ir.Semantics(
            ir.Operation.MULTIPLY,
            "imul",
            (ir.Held(1, width),),
            (ir.Held(1, width), ir.Held(2, width)),
        ),
        defines=(1,),
        uses=(1, 2),
    )

    result = _out(_body(op), {2})

    assert len(result) == 1
    assert result[0].what.sources[0] == ir.Held(1, width)
    assert isinstance(result[0].what.sources[1], ir.Mem)
    assert result[0].uses == (1,)


@pytest.mark.parametrize("name", ["xor", "add", "and", "sub"])
def test_repeated_tied_operand_does_not_become_memory_to_memory(name: str) -> None:
    """nbody-q-O refused XOR at 0x54 after both sources became the same frame slot."""
    from dataclasses import replace

    op = _add(1, 1)
    op = replace(op, what=replace(op.what, name=name), uses=(1,))
    reload, arithmetic, store = _out(_body(op), {1})
    assert isinstance(reload.what.sources[0], ir.Mem)
    assert all(isinstance(arg, ir.Held) for arg in (*arithmetic.what.dests, *arithmetic.what.sources))
    assert arithmetic.what.sources[0] == arithmetic.what.sources[1] == arithmetic.what.dests[0]
    assert isinstance(store.what.dests[0], ir.Mem)


def test_repeated_operand_reloads_a_spill_only_once() -> None:
    """NESTED emitted two identical frame reloads before each outer-loop IMUL."""
    multiply = lir.Insn(
        at=0x57,
        covers=(0x57, 0x5A),
        what=ir.Semantics(
            ir.Operation.BINARY,
            "imul",
            (ir.Held(2, 2),),
            (ir.Held(1, 2), ir.Held(1, 2), ir.Imm(6, 2)),
        ),
        defines=(2,),
        uses=(1, 1),
    )
    result = _out(_body(multiply), {1})
    assert len(result) == 2
    reload, product = result
    assert isinstance(reload.what.sources[0], ir.Mem)
    assert product.uses == reload.defines * 2
    assert product.what.sources[:2] == reload.what.dests * 2


@pytest.mark.parametrize(
    "name, expected",
    [("add", 15000), ("sub", 9000), ("and", 12000 & 3000), ("or", 12000 | 3000), ("xor", 12000 ^ 3000)],
)
def test_two_spilled_operands_keep_the_accumulator_value(name, expected) -> None:
    """LNGMXX printed 169330 instead of 142900 after a tied spill discarded its loaded accumulator."""
    from dataclasses import replace

    frame = frames.Frame(0)
    op = _add(1, 2)
    op = replace(op, what=replace(op.what, name=name))
    body, _ = spiller.spilled(_body(op), frozenset({1, 2}), frame)
    values = {frame.cell(1, 2): 12000, frame.cell(2, 2): 3000}
    for one in body.insns:
        args = [values[arg] for arg in one.what.sources]
        match one.what.name:
            case "mov":
                result = args[0]
            case "add":
                result = args[0] + args[1]
            case "sub":
                result = args[0] - args[1]
            case "and":
                result = args[0] & args[1]
            case "or":
                result = args[0] | args[1]
            case "xor":
                result = args[0] ^ args[1]
            case _:
                pytest.fail(str(one.what))
        values[one.what.dests[0]] = result & 0xFFFF
    assert values[frame.cell(1, 2)] == expected
    assert values[frame.cell(2, 2)] == 3000


def test_two_spilled_operands_do_not_need_two_scratch_registers() -> None:
    """LNGMXX's two spilled operands must retain the accumulator but need only one scratch."""
    result = _out(_body(_add(1, 2)), {1, 2})
    assert len(result) == 2
    assert isinstance(result[-1].what.dests[0], ir.Mem)


@pytest.mark.parametrize("width", [2, 4])
@pytest.mark.parametrize("both", [False, True])
def test_spilled_compare_preserves_order_flags_and_frame_address(width, both):
    """NBODY's spilled bound must not swap CMP operands or inherit its old global relocation."""
    op = lir.Insn(
        0, (0, 3), ir.Semantics(ir.Operation.COMPARE, "cmp", (), (ir.Held(1, width), ir.Held(2, width))), (3,), (1, 2)
    )
    result = _out(_body(op), {1, 2} if both else {2})
    assert len(result) == (2 if both else 1)
    comparison = result[-1]
    assert comparison.defines == (3,)
    assert comparison.symbol is False
    assert isinstance(comparison.what.sources[0], ir.Held)
    assert isinstance(comparison.what.sources[1], ir.Mem)
    if both:
        assert comparison.what.sources[0] == result[0].what.dests[0]
        assert comparison.what.sources[1] != result[0].what.sources[0]
    else:
        assert comparison.what.sources[0] == ir.Held(1, width)


def test_spilled_constant_is_rematerialized_without_a_frame_slot() -> None:
    """matrix spilled the invariant 20, storing it once and reloading it inside loops."""
    constant = lir.Insn(
        at=0,
        covers=(0, 3),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Imm(20, 2),)),
        defines=(1,),
        uses=(),
    )
    result = _out(_body(constant, _add(2, 1)), {1})
    assert not any(
        isinstance(operand, ir.Mem) for one in result if one.what for operand in (*one.what.dests, *one.what.sources)
    )
    assert result[-2].what.sources == (ir.Imm(20, 2),)
    assert result[-2].rematerialized
    assert result[-1].what.sources[1].value == result[-2].defines[0]


def test_spilled_frame_address_is_rematerialized_without_a_frame_slot() -> None:
    """matmul spilled a shared ``lea bp-84h`` and reloaded the pointer in its inner loop."""
    source = ir.Address(Addr(Space.FRAME, -132), Register.BP, offset=-132, disp_width=2)
    address = lir.Insn(
        at=0,
        covers=(0, 3),
        what=ir.Semantics(ir.Operation.ADDRESS, "lea", (ir.Held(1, 2),), (source,)),
        defines=(1,),
        uses=(),
    )
    frame = frames.Frame(0)

    result, _made = spiller.spilled(_body(address, _add(2, 1)), frozenset({1}), frame)

    assert 1 not in frame.slots
    recreated = [one for one in result.insns if one.what and one.what.op is ir.Operation.ADDRESS]
    assert len(recreated) == 1
    assert recreated[0].what.sources == (source,)
    assert recreated[0].rematerialized
    assert result.insns[-1].what.sources[1].value == recreated[0].defines[0]


def test_spilled_frame_address_folds_directly_into_its_only_memory_use() -> None:
    """Frontend parity's BASIC loopmix could not allocate its array load.

    Spilling the descriptor address made ``lea temporary,[bp-38]`` beside
    ``mov value,[temporary+10]``.  All four 16-bit address registers were
    occupied, although the exact cell is directly encodable as ``[bp-28]``.
    Compose the address during spill rewriting instead of creating an
    unspillable one-instruction range.
    """
    source = ir.Address(Addr(Space.FRAME, -38), Register.BP, offset=-38, disp_width=1)
    address = lir.Insn(
        at=0x10,
        covers=(0x10, 0x13),
        what=ir.Semantics(ir.Operation.ADDRESS, "lea", (ir.Held(1, 2),), (source,)),
        defines=(1,),
        uses=(),
    )
    cell = ir.Mem(Addr(Space.LITERAL, 10, base=Register.SI), 2, base=ir.Held(1, 2))
    load = lir.Insn(
        at=0x14,
        covers=(0x14, 0x17),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(2, 2),), (cell,)),
        defines=(2,),
        uses=(1,),
    )

    result, made = spiller.spilled(_body(address, load), frozenset({1}), frames.Frame(0))

    assert made == frozenset()
    assert not any(one.what and one.what.op is ir.Operation.ADDRESS for one in result.insns)
    folded = result.insns[-1].what.sources[0]
    assert isinstance(folded, ir.Mem)
    assert folded.addr == Addr(Space.FRAME, -28)
    assert folded.base is None
    assert result.insns[-1].uses == ()


@pytest.mark.parametrize("space", [Space.SEGMENT, Space.EXTERNAL])
def test_spilled_relocatable_address_is_rematerialized_without_a_frame_slot(space) -> None:
    """QCport's global descriptor address spilled into a private frame cell.

    A direct SEGDEF/EXTDEF address has no dynamic input: recreating its
    ``lea`` at the use is cheaper than storing an offset and reloading it.
    Fresh OMF emission owns the duplicated relocation, so this is valid for
    both module data and imported data rather than only BP-relative locals.
    """
    source = ir.Address(Addr(space, 12, 7), Register.NONE, disp_width=2)
    address = lir.Insn(
        at=0,
        covers=(0, 3),
        what=ir.Semantics(ir.Operation.ADDRESS, "lea", (ir.Held(1, 2),), (source,)),
        defines=(1,),
        uses=(),
    )
    frame = frames.Frame(0)

    result, _made = spiller.spilled(_body(address, _add(2, 1)), frozenset({1}), frame)

    assert 1 not in frame.slots
    recreated = [one for one in result.insns if one.what and one.what.op is ir.Operation.ADDRESS]
    assert len(recreated) == 1
    assert recreated[0].what.sources == (source,)
    assert recreated[0].rematerialized
    # The rematerialized spelling is not an unrelocated literal zero: fresh
    # OMF emission places the original symbol fixup on its new displacement.
    from qbopt.backend import omfwrite

    emitted = omfwrite._encoded(
        ir.Semantics(ir.Operation.ADDRESS, "lea", (ir.Reg(Register.BX, 2),), (source,)),
        {(space, 7): "_descriptor"},
    )
    assert emitted.code == bytes.fromhex("8d1e0c00")
    assert emitted.fixups == (omfwrite.Fixup(2, omfwrite.OFFSET, "_descriptor"),)


@pytest.mark.parametrize("name", ["movsx", "movzx"])
def test_single_use_extension_is_rematerialized_at_its_use(name: str) -> None:
    """Mandelbrot stored a one-use ``movsx`` result in a private slot and
    read that slot back in its hot loop.

    Within one block, moving the same extension beside its only use cannot
    duplicate work: it replaces the extended value's live range with its
    narrower source's and removes one spill store plus one reload or folded
    memory operand.
    """
    extension = lir.Insn(
        at=0x10,
        covers=(0x10, 0x10),
        what=ir.Semantics(ir.Operation.EXTEND, name, (ir.Held(2, 4),), (ir.Held(1, 2),)),
        defines=(2,),
        uses=(1,),
    )
    use = lir.Insn(
        at=0x14,
        covers=(0x14, 0x16),
        what=ir.Semantics(
            ir.Operation.BINARY,
            "add",
            (ir.Held(3, 4),),
            (ir.Held(3, 4), ir.Held(2, 4)),
        ),
        defines=(3,),
        uses=(3, 2),
    )
    frame = frames.Frame(0)

    result, _made = spiller.spilled(_body(_move(1, 0, at=0x0F), extension, use), frozenset({2}), frame)

    assert 2 not in frame.slots
    assert [one.what.name for one in result.insns] == ["mov", name, "add"]
    assert result.insns[1].rematerialized
    assert result.insns[1].uses == (1,)
    assert result.insns[2].uses == (3, result.insns[1].defines[0])
    assert not any(
        isinstance(operand, ir.Mem) for one in result.insns for operand in (*one.what.dests, *one.what.sources)
    )


@pytest.mark.parametrize("unsafe", ["multiple uses", "source redefined", "parallel copy"])
def test_extension_rematerialization_requires_one_unchanged_ordinary_use(unsafe: str) -> None:
    """Delaying an extension must neither duplicate it nor read a newer
    source value, and cannot split a simultaneous parallel-copy run."""
    extension = lir.Insn(
        at=0x10,
        covers=(0x10, 0x10),
        what=ir.Semantics(ir.Operation.EXTEND, "movsx", (ir.Held(2, 4),), (ir.Held(1, 2),)),
        defines=(2,),
        uses=(1,),
    )
    use = lir.Insn(
        at=0x14,
        covers=(0x14, 0x16),
        what=ir.Semantics(
            ir.Operation.BINARY,
            "add",
            (ir.Held(3, 4),),
            (ir.Held(3, 4), ir.Held(2, 4)),
        ),
        defines=(3,),
        uses=(3, 2),
        group=7 if unsafe == "parallel copy" else None,
    )
    middle = (_move(1, 4, at=0x12),) if unsafe == "source redefined" else ()
    later = (
        (
            lir.Insn(
                at=0x18,
                covers=(0x18, 0x1A),
                what=replace(use.what, dests=(ir.Held(5, 4),), sources=(ir.Held(5, 4), ir.Held(2, 4))),
                defines=(5,),
                uses=(5, 2),
            ),
        )
        if unsafe == "multiple uses"
        else ()
    )
    frame = frames.Frame(0)

    spiller.spilled(_body(extension, *middle, use, *later), frozenset({2}), frame)

    assert 2 in frame.slots


def test_extension_is_not_rematerialized_from_an_entry_into_a_loop() -> None:
    """Mandelbrot's nominally one-use argument extension moved from function
    entry into the outer loop, raising estimated executed instructions from
    10309 to 10317.

    A static use count is not an execution count. Cross-block rematerializing
    an expression needs a frequency-and-pressure proof; the local spelling
    cannot assume its consumer runs as often as its definition.
    """
    source = _move(1, 0, at=0x10)
    extension = lir.Insn(
        at=0x12,
        covers=(0x12, 0x12),
        what=ir.Semantics(ir.Operation.EXTEND, "movsx", (ir.Held(2, 4),), (ir.Held(1, 2),)),
        defines=(2,),
        uses=(1,),
    )
    jump = lir.Insn(
        at=0x14,
        covers=(0x14, 0x16),
        what=ir.Semantics(ir.Operation.JUMP, "jmp", (), (), 0x20),
        defines=(),
        uses=(),
    )
    use = lir.Insn(
        at=0x20,
        covers=(0x20, 0x22),
        what=ir.Semantics(
            ir.Operation.BINARY,
            "add",
            (ir.Held(3, 4),),
            (ir.Held(3, 4), ir.Held(2, 4)),
        ),
        defines=(3,),
        uses=(3, 2),
    )
    branch = lir.Insn(
        at=0x22,
        covers=(0x22, 0x24),
        what=ir.Semantics(ir.Operation.BRANCH, "jne", (), (), 0x20),
        defines=(),
        uses=(),
    )
    body = lir.LirBody(
        "loop",
        0x10,
        (
            lir.LirBlock(0x10, (source, extension, jump), (0x20,)),
            lir.LirBlock(0x20, (use, branch), (0x20, 0x30)),
            lir.LirBlock(0x30, (), ()),
        ),
        origin={},
        pins={},
    )
    frame = frames.Frame(0)

    spiller.spilled(body, frozenset({2}), frame)

    assert 2 in frame.slots


def test_c_matmul_multiplies_spilled_rows_directly_from_memory() -> None:
    """Matmul reloaded all eight unrolled lhs values solely to feed ``imul``.

    The other factor dies at the multiply and can own its result register, so
    commutative two-address selection must leave the long-lived row value in
    its spill slot and use x86's register-by-memory multiply form.
    """
    from tools import quality
    from qbopt.cfront import compile as cfront

    source = Path("bench/c/matmul.c")
    module = cfront.assembled(cfront.recorded(source, []), source.stem, optimise=True)
    procedure = next(one for one in module.procedures if one.name == "_bench_matmul")
    rows = quality._rows(quality._blob(module, procedure, 0))
    multiplies = [operands for _raw, mnemonic, operands in rows if mnemonic == "imul"]
    extensions = [operands for _raw, mnemonic, operands in rows if mnemonic == "movsx"]

    assert len(multiplies) >= 8
    assert sum("[bp" in operands.lower() for operands in multiplies) >= 8, multiplies
    assert sum("[" in operands for operands in extensions) >= 8, extensions


def test_nonoverlapping_spills_share_one_compatible_frame_slot() -> None:
    """Two sequential spills reserved separate words although their lifetimes never overlap."""
    frame = frames.Frame(0)
    body = _body(_move(1, 10, at=0x10), _add(11, 1, at=0x12), _move(2, 20, at=0x14), _add(21, 2, at=0x16))
    spiller.spilled(body, frozenset({1, 2}), frame)
    assert frame.slots[1] == frame.slots[2]
    assert frame.size == 2


def test_nonoverlapping_spills_from_later_rounds_reuse_the_frame_slot() -> None:
    """Matmul allocated a new slot in every spill round even after the
    previous slot's value was dead.

    RegAlloc rewrites one spill batch before choosing the next. Slot coloring
    therefore has to recover the earlier slot's live range from the rewritten
    LIR rather than forgetting every color at the end of one batch.
    """
    frame = frames.Frame(0)
    body = _body(_move(1, 10, at=0x10), _add(11, 1, at=0x12), _move(2, 20, at=0x14), _add(21, 2, at=0x16))

    first, _made = spiller.spilled(body, frozenset({1}), frame)
    spiller.spilled(first, frozenset({2}), frame)

    assert frame.slots[1] == frame.slots[2]
    assert frame.size == 2


def test_overlapping_spills_from_later_rounds_keep_distinct_slots() -> None:
    """Cross-round coloring must not overwrite an earlier value still live."""
    frame = frames.Frame(0)
    body = _body(_move(1, 10, at=0x10), _move(2, 20, at=0x12), _add(11, 1, at=0x14), _add(21, 2, at=0x16))

    first, _made = spiller.spilled(body, frozenset({1}), frame)
    spiller.spilled(first, frozenset({2}), frame)

    assert frame.slots[1] != frame.slots[2]


def test_cross_round_coloring_includes_current_preassigned_spill_webs() -> None:
    """Mandelbrot returned 1654 instead of 8873 when `work` and `cy` shared
    `[bp-8]`.

    Sibling spilling reserves a home for part of the current batch before slot
    coloring runs. That home has no memory operations yet, so recovering only
    materialized slot lifetimes makes it look empty. Its still-virtual members
    must occupy the color while the remaining spills are assigned.
    """
    frame = frames.Frame(0, slots={1: -2}, capacities={-2: 2})
    body = _body(_move(1, 10, at=0x10), _move(2, 20, at=0x12), _add(11, 1, at=0x14), _add(21, 2, at=0x16))

    spiller._color_slots(body, frozenset({1, 2}), {1: 2, 2: 2}, frame)

    assert frame.slots[1] != frame.slots[2]


def test_later_narrow_spill_reuses_a_dead_wider_slot() -> None:
    """Cross-round coloring preserves the stack object's capacity."""
    frame = frames.Frame(0)
    wide = lir.Insn(
        0x10,
        (0x10, 0x10),
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 4),), (ir.Held(10, 4),)),
        (1,),
        (10,),
    )
    use_wide = lir.Insn(
        0x12,
        (0x12, 0x12),
        ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(11, 4),), (ir.Held(11, 4), ir.Held(1, 4))),
        (11,),
        (11, 1),
    )
    body = _body(wide, use_wide, _move(2, 20, at=0x14), _add(21, 2, at=0x16))

    first, _made = spiller.spilled(body, frozenset({1}), frame)
    spiller.spilled(first, frozenset({2}), frame)

    assert frame.slots[1] == frame.slots[2]
    assert frame.size == 4


def test_later_wide_spill_does_not_outgrow_a_narrow_slot() -> None:
    """A dead word is not four bytes of storage merely because it is free."""
    frame = frames.Frame(0)
    wide = lir.Insn(
        0x14,
        (0x14, 0x14),
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(2, 4),), (ir.Held(20, 4),)),
        (2,),
        (20,),
    )
    use_wide = lir.Insn(
        0x16,
        (0x16, 0x16),
        ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(21, 4),), (ir.Held(21, 4), ir.Held(2, 4))),
        (21,),
        (21, 2),
    )
    body = _body(_move(1, 10, at=0x10), _add(11, 1, at=0x12), wide, use_wide)

    first, _made = spiller.spilled(body, frozenset({1}), frame)
    spiller.spilled(first, frozenset({2}), frame)

    assert frame.slots[1] != frame.slots[2]
    assert frame.size == 6


def test_overlapping_spills_keep_distinct_frame_slots() -> None:
    """Slot coloring must follow liveness, not merely source instruction order."""
    frame = frames.Frame(0)
    body = _body(_move(1, 10, at=0x10), _move(2, 20, at=0x12), _add(11, 1, at=0x14), _add(21, 2, at=0x16))
    spiller.spilled(body, frozenset({1, 2}), frame)
    assert frame.slots[1] != frame.slots[2]
    assert frame.size == 4


def test_narrow_spill_can_reuse_a_dead_wider_slot() -> None:
    """Compatible slot coloring is by capacity, not only exact value width."""
    frame = frames.Frame(0)
    wide = lir.Insn(
        0x10,
        (0x10, 0x10),
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 4),), (ir.Held(10, 4),)),
        (1,),
        (10,),
    )
    use_wide = lir.Insn(
        0x12,
        (0x12, 0x12),
        ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(11, 4),), (ir.Held(11, 4), ir.Held(1, 4))),
        (11,),
        (11, 1),
    )
    body = _body(wide, use_wide, _move(2, 20, at=0x14), _add(21, 2, at=0x16))
    spiller.spilled(body, frozenset({1, 2}), frame)
    assert frame.slots[1] == frame.slots[2]
    assert frame.size == 4


@pytest.mark.parametrize("destination_spilled", [False, True])
def test_grouped_constant_rematerializes_without_splitting_parallel_copy(destination_spilled):
    """NBODY's literal zero was spilled because its loop initializers belonged to parallel copies."""
    constant = lir.Insn(0, (0, 0), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Imm(0, 2),)), (1,), ())
    body = _body(constant, _move(2, 1, group=7), _move(3, 4, group=7))
    frame = frames.Frame(0)
    done, _ = spiller.spilled(body, frozenset({1, 2} if destination_spilled else {1}), frame)
    group = [one for one in done.insns if one.group == 7]
    assert len(group) == 2
    assert group[0].what.sources == (ir.Imm(0, 2),)
    assert group[0].uses == ()
    assert isinstance(group[0].what.dests[0], ir.Mem) == destination_spilled
    positions = [index for index, one in enumerate(done.insns) if one.group == 7]
    assert positions[1] == positions[0] + 1
    assert not any(one.spill_reload for one in done.insns)


def test_parallel_copy_destination_is_not_mistaken_for_a_constant():
    """A grouped assignment to the same value invalidates a literal seed."""
    constant = lir.Insn(0, (0, 0), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Imm(0, 2),)), (1,), ())
    body = _body(constant, _move(1, 4, group=7), _move(2, 1, group=7))
    assert spiller._constants(body, frozenset({1})) == {}


@pytest.mark.parametrize("redefined", [False, True])
def test_copied_constant_rematerializes_only_with_a_unique_definition(redefined):
    """A copy of MATRIX's invariant 20 must not need a stack reload; a later redefinition invalidates it."""
    constant = lir.Insn(
        at=0,
        covers=(0, 3),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Imm(20, 2),)),
        defines=(1,),
        uses=(),
    )
    operations = [constant, _move(2, 1), _move(3, 2)]
    if redefined:
        operations.append(_add(1, 4))
    operations.append(_add(5, 3))
    result = _out(_body(*operations), {3})
    memory = [
        operand
        for one in result
        if one.what
        for operand in (*one.what.dests, *one.what.sources)
        if isinstance(operand, ir.Mem)
    ]
    assert bool(memory) is redefined
    if not redefined:
        assert result[-2].what.sources == (ir.Imm(20, 2),)
        assert result[-1].what.sources[1].value == result[-2].defines[0]


def test_copy_cycle_is_not_a_constant():
    """A copy cycle with no literal seed cannot justify rematerializing any value."""
    assert spiller._constants(_body(_move(1, 2), _move(2, 1)), frozenset({1, 2})) == {}


def test_relocated_address_is_not_rematerialized_as_literal_zero() -> None:
    """HARR's descriptor pointer has zero bytes, but LINK supplies its address."""
    address = ir.Imm(0, 2, Addr(Space.SEGMENT, 6, 5))
    defining = lir.Insn(
        at=0,
        covers=(0, 3),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (address,)),
        defines=(1,),
        uses=(),
    )
    result = _out(_body(defining, _add(2, 1)), {1})
    assert sum(one.what.sources == (address,) for one in result if one.what) == 1


def test_constant_reload_precedes_an_in_place_spilled_update() -> None:
    constant = lir.Insn(
        at=0,
        covers=(0, 3),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Imm(20, 2),)),
        defines=(1,),
        uses=(),
    )
    result = _out(_body(constant, _add(2, 1)), {1, 2})
    assert all(1 not in one.uses for one in result)
    (add,) = [one for one in result if one.what and one.what.name == "add"]
    made = {value: one for one in result[: result.index(add)] for value in one.defines}
    added = {made[value].what.sources[0] for value in add.uses}
    assert ir.Imm(20, 2) in added
    assert isinstance(add.what.dests[0], ir.Mem)
    assert add.what.sources[0] == add.what.dests[0]
    assert not any(one.spill_store for one in result)


def test_a_lifted_memory_operand_takes_the_fixup_with_it() -> None:
    """The fixup names the operand, so it goes where the operand goes.

    A tie with a memory source has no remedy but a load of its own: `add
    cx,[a]` with the accumulator spilled becomes `mov bx,[a]` and `add
    [bp-22h],bx`. Left on the survivor -- which no longer reads memory --
    the fixup was bound to whatever field it did have, and the address of
    `a` was written over the frame offset the add kept.
    """
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    cell = ir.Mem(Addr(Space.SEGMENT, 0xA), 2)
    what = ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(1, 2),), (ir.Held(1, 2), cell))
    add = lir.Insn(at=0x100, covers=(0x100, 0x104), what=what, defines=(1,), uses=(1,), op=None)
    got = _out(_body(add), {1})

    def symbolic(one) -> bool:
        every = (one.what.dests if one.what else ()) + (one.what.sources if one.what else ())
        return any(isinstance(x, ir.Mem) and x.addr is not None and x.addr.space is Space.SEGMENT for x in every)

    lifted = [one for one in got if symbolic(one)]
    assert len(lifted) == 1, [one.what for one in got]
    assert lifted[0].symbol is True, "the load does not claim the operand it now holds"
    kept = [one for one in got if one is not lifted[0] and one.what and one.what.name == "add"]
    assert kept and kept[0].symbol is False, "the survivor still claims a fixup for an operand it lost"


def test_a_grouped_move_reads_its_spilled_source_where_it_lives() -> None:
    got = _out(_body(_move(1, 2, group=1)), {2})
    assert len(got) == 1, [one.what for one in got]
    assert got[0].group == 1 and got[0].uses == ()
    assert isinstance(got[0].what.sources[0], ir.Mem)
    assert got[0].what.dests == (ir.Held(1, 2),)


def test_a_grouped_move_writes_its_spilled_destination_where_it_lives() -> None:
    got = _out(_body(_move(1, 2, group=1)), {1})
    assert len(got) == 1
    assert got[0].group == 1 and got[0].defines == ()
    assert isinstance(got[0].what.dests[0], ir.Mem)
    assert got[0].what.sources == (ir.Held(2, 2),)


def test_a_grouped_move_with_both_ends_spilled_stays_grouped() -> None:
    """NESTED refused a phi copy between two spilled values before scheduling."""
    frame = frames.Frame(0, slots={1: -2, 2: -4})
    got, _ = spiller.spilled(_body(_move(1, 2, group=1)), frozenset({1, 2}), frame)
    got = list(got.insns)
    assert len(got) == 1 and got[0].group == 1
    assert all(isinstance(cell, ir.Mem) for cell in (*got[0].what.dests, *got[0].what.sources))
    assert got[0].defines == got[0].uses == ()


def test_a_grouped_move_coalesced_to_one_spill_slot_is_an_identity() -> None:
    """A parallel copy between noninterfering spill values need not survive scheduling."""
    frame = frames.Frame(0)
    got, _ = spiller.spilled(_body(_move(1, 2, group=1)), frozenset({1, 2}), frame)
    assert frame.slots[1] == frame.slots[2]
    assert got.insns == ()


def test_an_unhandled_arithmetic_form_keeps_its_reload() -> None:
    """Carry-dependent arithmetic is outside the explicit spill-source folding forms."""
    from dataclasses import replace

    add = _add(1, 2)
    got = _out(_body(replace(add, what=replace(add.what, name="adc"))), {2})
    assert len(got) == 2, [one.what.name for one in got]
    assert got[0].what.name == "mov" and isinstance(got[0].what.sources[0], ir.Mem)
    assert got[1].what.name == "adc"


def test_the_group_comes_out_one_contiguous_run() -> None:
    got = _out(_body(_move(1, 2, group=1), _move(3, 4, group=1), _move(5, 6, group=1)), {4})
    assert [one.group for one in got] == [1, 1, 1], [one.what for one in got]


def _binary(name: str, into: int, other: int, at: int = 0x200) -> lir.Insn:
    what = ir.Semantics(ir.Operation.BINARY, name, (ir.Held(into, 2),), (ir.Held(into, 2), ir.Held(other, 2)))
    return lir.Insn(at=at, covers=(at, at + 2), what=what, defines=(into,), uses=(into, other), op=None)


def test_a_tied_value_is_spilled_into_the_operand_itself() -> None:
    """pressx spilled 207, then 212, then 215, at one `add`, two
    instructions added every round. Spilling a value an instruction both
    reads and writes buys nothing -- the reload is tied at the same place
    -- and x86 reads and writes memory in one instruction anyway."""
    got = _out(_body(_binary("add", 1, 2)), {1})
    assert len(got) == 1, [one.what.name for one in got]
    what = got[0].what
    assert isinstance(what.dests[0], ir.Mem) and what.dests[0] == what.sources[0]
    assert what.sources[1] == ir.Held(2, 2)
    assert got[0].defines == () and got[0].uses == (2,)


def test_a_tied_value_an_instruction_requires_in_a_register_keeps_the_reload() -> None:
    """`imul`'s low half is tied and must be ax; a slot is not a register."""
    what = ir.Semantics(
        ir.Operation.MULTIPLY,
        "imul",
        (ir.Held(1, 2), ir.Held(2, 2)),
        (ir.Held(1, 2), ir.Held(3, 2)),
    )
    imul = lir.Insn(at=0x200, covers=(0x200, 0x202), what=what, defines=(1, 2), uses=(1, 3), op=None)
    got = _out(_body(imul), {1})
    assert len(got) > 1, "the fixed tie took the in-place path"
    assert any(isinstance(one.what.sources[0], ir.Mem) for one in got if one.what.name == "mov")


def test_a_second_memory_operand_keeps_the_reload() -> None:
    """One memory operand is all an instruction has."""
    what = ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(1, 2),), (ir.Held(1, 2), ir.Held(2, 2)))
    both = lir.Insn(at=0x200, covers=(0x200, 0x202), what=what, defines=(1,), uses=(1, 2), op=None)
    got = _out(_body(both), {1, 2})
    assert len(got) > 1, "two spilled operands took the in-place path"


def test_spilling_a_pointer_renames_the_cell_it_is_the_base_of() -> None:
    """A spilled pointer's reload renamed `uses` and not the cell.

    `_settled` looked for a Held in `Mem.through`, which holds a register
    since lowering, so the reload defined v6, the load said it read v6, and
    the byte it encoded still addressed through v3 -- which after the spill
    lives in a frame slot and no register.
    """
    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.model import lir
    from qbopt.backend import spiller
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    where = Addr(Space.SEGMENT, 0x10, base=Register.SI)
    cell = ir.Mem(where, 2, Register.NONE, 0, 2, base=ir.Held(3, 2))
    load = lir.Insn(
        at=0x20,
        covers=(0x20, 0x22),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(5, 2),), (cell,)),
        defines=(5,),
        uses=(3,),
        op=None,
    )
    body = lir.LirBody(
        name="one",
        entry=0,
        blocks=(lir.LirBlock(at=0, insns=(load,), succ=()),),
        origin={},
        pins={},
    )
    out, made = spiller.spilled(body, frozenset({3}))
    got = next(
        one
        for one in out.blocks[0].insns
        if any(isinstance(x, ir.Mem) and x.base is not None for x in one.what.sources)
    )
    read = got.what.sources[0]
    assert got.uses == (read.base.value,), f"uses {got.uses}, cell on {read.base}"
    assert got.uses != (3,), "nothing was spilled; the fixture does not reach the rename"
    assert read.base.value in made, f"{read.base} is not one of the reloads {sorted(made)}"
    assert read.through == Register.NONE, "the rename placed it"
    assert (read.addr, read.width, read.offset, read.disp_width) == (where, 2, 0, 2)


def _one_block(*insns) -> "lir.LirBody":
    from qbopt.model import lir

    return lir.LirBody("one", 0, (lir.LirBlock(at=0, insns=insns, succ=()),), origin={}, pins={})


def _frame():
    from qbopt.backend import frame as frames

    return frames.Frame(floor=0)


def _through_regalloc(body):
    from qbopt.backend import allocate
    from qbopt.backend import frame as frames

    return allocate.RegAlloc({}, frames.of(body)).transform(body)


def test_the_body_that_never_settled_allocates() -> None:
    """nested-p-g2's `main`, which the allocator gave up on.

    `0x0061 add v, [cell]` ties v to its own destination while the other
    source is already memory, so the tied value cannot go in a slot -- one
    memory operand is all an instruction has. The generic path minted a
    reload tied at the same place, and the next round spilled that: 84,
    then 87, 90, 93 ... 111, three instructions added every round, until
    the twelve rounds ran out.
    """
    from pathlib import Path

    from qbopt import flow
    from qbopt.abi import runtime
    from qbopt.backend import lower
    from qbopt.objectfile import omf
    from qbopt.backend import phielim
    from qbopt.backend import twoaddr
    from qbopt.backend import allocate
    from qbopt.backend import coalesce
    from qbopt.objectfile import module
    from qbopt.model import mir as raise_
    from qbopt.backend import frame as frames
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    records = omf.parse(Path("fixtures/omf/nested-p-g2.obj").read_bytes())
    found = module.of(records)
    blocks = split.partition(found, code_map(found))
    contracts = runtime.for_module(found)
    name, body = next((one, other) for one, other in raise_.bodies(found, blocks, contracts) if one == "main (main)")
    low = lower.lowered(name, body, found.calls, set(found.absorbed), contracts)
    frame = frames.of(low)
    for phase in (phielim.PhiElimination(), twoaddr.TwoAddress(), coalesce.Coalescer()):
        low = phase.transform(low)
    got = allocate.RegAlloc(flow._pinned(body), frame).transform(low)
    assert got is not None


def test_a_dword_index_is_reloaded_as_a_dword():
    """A spilled dword counter indexing `[eax+edx*2]` came back as a word:
    PLASMA's first pass read the cell through edx's stale high half."""
    from iced_x86 import Register

    counter = _move(1, 3)
    counter = replace(counter, what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 4),), (ir.Held(3, 4),)))
    cell = ir.Mem(Addr(Space.FAR, 0, segment=Register.ES), 2, base=ir.Held(5, 4), index=ir.Held(1, 4), scale=2)
    read = lir.Insn(
        at=0x100,
        covers=(0x100, 0x100),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(4, 2),), (cell,)),
        defines=(4,),
        uses=(5, 1),
        op=None,
    )
    out = _out(_body(counter, read), {1})
    reloads = [one for one in out if one.spill_reload]
    assert reloads and all(one.what.dests[0].width == 4 for one in reloads), [one.what for one in out]


def test_a_spilled_word_index_folds_into_a_dead_address_base():
    """farloadloop reloaded a carried word index solely to address one cell.

    When the base dies at that cell, ``add base,[slot]`` is legal and leaves
    the same address without consuming a reload register.  Reloading the
    index first made the retained-invariant pressure plan unplaceable even
    though the medium-model encoding can consume the slot directly.
    """
    update = _add(1, 2)
    cell = ir.Mem(
        Addr(Space.FAR, 0, segment=Register.ES),
        2,
        base=ir.Held(5, 2),
        index=ir.Held(1, 2),
    )
    read = lir.Insn(
        at=0x102,
        covers=(0x102, 0x104),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(4, 2),), (cell,)),
        defines=(4,),
        uses=(5, 1),
        op=None,
    )

    result = _out(_body(update, read), {1})
    folded = [
        one
        for one in result
        if one.what is not None
        and one.what.name == "add"
        and len(one.what.sources) == 2
        and isinstance(one.what.sources[1], ir.Mem)
    ]
    assert len(folded) == 1, [one.what for one in result]
    addressed = next(
        one
        for one in result
        if one.what is not None
        and one.what.name == "mov"
        and isinstance(one.what.sources[0], ir.Mem)
        and one.what.sources[0].addr is not None
        and one.what.sources[0].addr.space is Space.FAR
    )
    cell_after = addressed.what.sources[0]
    assert isinstance(cell_after, ir.Mem) and cell_after.index is None, addressed.what
    assert not any(one.spill_reload for one in result), [one.what for one in result]


def test_a_spilled_word_index_does_not_mutate_a_base_used_later():
    """The index fold may not move a second access through the same base."""
    update = _add(1, 2)
    first = ir.Mem(Addr(Space.FAR, 0, segment=Register.ES), 2, base=ir.Held(5, 2), index=ir.Held(1, 2))
    later = ir.Mem(Addr(Space.FAR, 2, segment=Register.ES), 2, base=ir.Held(5, 2))
    reads = (
        lir.Insn(
            at=0x102,
            covers=(0x102, 0x104),
            what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(4, 2),), (first,)),
            defines=(4,),
            uses=(5, 1),
            op=None,
        ),
        lir.Insn(
            at=0x104,
            covers=(0x104, 0x106),
            what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(6, 2),), (later,)),
            defines=(6,),
            uses=(5,),
            op=None,
        ),
    )

    result = _out(_body(update, *reads), {1})
    assert not any(
        one.what is not None
        and one.what.name == "add"
        and len(one.what.sources) == 2
        and isinstance(one.what.sources[1], ir.Mem)
        for one in result
    ), [one.what for one in result]


def test_a_spilled_word_index_does_not_mutate_a_base_read_later_as_a_value():
    """A base used later outside a memory operand was omitted from the death proof.

    The direct spill fold destructively adds the index to the address base.
    Counting only later encoded memory bases therefore changed an ordinary
    subsequent register use to the indexed address.  Every semantic use, not
    merely every memory spelling, must participate in the last-use proof.
    """
    update = _add(1, 2)
    cell = ir.Mem(Addr(Space.FAR, 0, segment=Register.ES), 2, base=ir.Held(5, 2), index=ir.Held(1, 2))
    read = lir.Insn(
        at=0x102,
        covers=(0x102, 0x104),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(4, 2),), (cell,)),
        defines=(4,),
        uses=(5, 1),
        op=None,
    )
    later = _move(6, 5)

    result = _out(_body(update, read, later), {1})

    assert not any(
        one.what is not None
        and one.what.name == "add"
        and len(one.what.sources) == 2
        and isinstance(one.what.sources[1], ir.Mem)
        for one in result
    ), [one.what for one in result]


@pytest.mark.parametrize("data_value", [1, 5], ids=["index", "base"])
def test_a_spilled_word_index_does_not_mutate_an_address_operand_used_as_data(data_value):
    """A destructive address fold also changed a base/index data operand.

    One instruction can use the same value both to form its memory address
    and as the value stored or combined there.  Last-instruction liveness is
    not enough: the fold is legal only when every occurrence of the base and
    index in that instruction belongs to the indexed cells being rewritten.
    """
    update = _add(1, 2)
    cell = ir.Mem(Addr(Space.FAR, 0, segment=Register.ES), 2, base=ir.Held(5, 2), index=ir.Held(1, 2))
    source = ir.Held(data_value, 2)
    write = lir.Insn(
        at=0x102,
        covers=(0x102, 0x104),
        what=ir.Semantics(ir.Operation.BINARY, "add", (cell,), (cell, source)),
        defines=(),
        uses=(5, 1),
        op=None,
    )

    result = _out(_body(update, write), {1})

    assert not any(
        one.what is not None
        and one.what.name == "add"
        and len(one.what.sources) == 2
        and isinstance(one.what.sources[1], ir.Mem)
        for one in result
    ), [one.what for one in result]


def test_nbody_accumulates_in_the_slots_it_spills() -> None:
    """accX and accY were reloaded on the skipping edge and stored at the latch.

    A phi, the sum the loop writes and the copies between them are one
    accumulator: spilled together into one slot, the copies are gone and
    `acc += d` is an `add` to the cell. Spilled apart, every pass loaded and
    stored both -- and once the sum tied the wrong operand, accY went
    `mov eax,[s] / mov [s],ecx / add [s],eax`.
    """
    from iced_x86 import OpKind
    from iced_x86 import Mnemonic
    from iced_x86 import Register
    from test_observers import _nbody_inner_loop

    loop = _nbody_inner_loop()
    updates = [one for one in loop if one.mnemonic == Mnemonic.ADD and one.op0_kind == OpKind.MEMORY]
    stores = [one for one in loop if one.mnemonic == Mnemonic.MOV and one.op0_kind == OpKind.MEMORY]
    loads = [
        one
        for one in loop
        if one.mnemonic == Mnemonic.MOV and one.op1_kind == OpKind.MEMORY and one.memory_base == Register.BP
    ]
    assert len(updates) == 2
    assert not stores and not loads


def test_a_value_defined_twice_keeps_its_increment_in_its_home() -> None:
    """UNWHITEFADE's frame counter, coalesced with its initial zero, lost its increment.

    Homed in its own local, the increment was renamed to a reload while the
    store kept reading the value, whose only definition left was the zero.
    The next round rebuilt that zero at the store and the fade never ended.
    """
    from iced_x86 import Register

    home = ir.Mem(Addr(Space.FRAME, -0x2A), 2, Register.BP, -0x2A, 1)

    def insn(at, what, defines=(), uses=()):
        return lir.Insn(at, (at, at + 2), what, defines, uses)

    body = lir.LirBody(
        "one",
        0,
        (
            lir.LirBlock(
                0,
                (
                    insn(0, ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Imm(0, 2),)), (1,)),
                    insn(2, ir.Semantics(ir.Operation.JUMP, "jmp", (), (), 0x20)),
                ),
                succ=(0x20,),
            ),
            lir.LirBlock(
                0x10,
                (
                    insn(0x10, ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(1, 2),)), (), (1,)),
                    insn(
                        0x12,
                        ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(1, 2),), (ir.Held(1, 2), ir.Imm(1, 2))),
                        (1,),
                        (1,),
                    ),
                ),
                succ=(0x20,),
            ),
            lir.LirBlock(
                0x20,
                (
                    insn(0x20, ir.Semantics(ir.Operation.MOVE, "mov", (home,), (ir.Held(1, 2),)), (), (1,)),
                    insn(
                        0x22, ir.Semantics(ir.Operation.COMPARE, "cmp", (), (ir.Held(1, 2), ir.Imm(5, 2))), (9,), (1,)
                    ),
                    insn(0x24, ir.Semantics(ir.Operation.BRANCH, "jl", (), (), 0x10), (), (9,)),
                ),
                succ=(0x10, 0x30),
            ),
            lir.LirBlock(0x30, (insn(0x30, ir.Semantics(ir.Operation.RETURN, "ret", (), ())),), succ=()),
        ),
        origin={},
        pins={},
    )
    done, _made = spiller.spilled(body, frozenset({1}), frames.Frame(0))
    held, memory, pushed = {}, {}, []

    def read(operand):
        if isinstance(operand, ir.Imm):
            return operand.value
        if isinstance(operand, ir.Mem):
            return memory[operand.addr.disp]
        return held[operand.value]

    blocks = {block.at: block for block in done.blocks}
    for at in (0, 0x20, 0x10, 0x20, 0x10, 0x20):
        for one in blocks[at].insns:
            what = one.what
            if what.op in (ir.Operation.MOVE, ir.Operation.BINARY):
                result = sum(read(source) for source in what.sources)
                dest = what.dests[0]
                if isinstance(dest, ir.Mem):
                    memory[dest.addr.disp] = result
                else:
                    held[dest.value] = result
            elif what.op is ir.Operation.PUSH:
                pushed.append(read(what.sources[0]))
    assert pushed == [0, 1]
    assert memory[-0x2A] == 2


def test_a_stable_load_stored_to_a_local_keeps_its_store_defined() -> None:
    """A value reloadable from its cell and homed in a local lost the value its store read.

    deedlines' plasmablobs loads `k1%` and stores it as the fade loop's limit.
    The spiller dropped the load as a stable one and kept the store reading
    the value as its home's initializer. Nothing defined it, the next round
    gave it a slot nothing wrote, and the loop ran past 191 through the DAC.
    """
    from iced_x86 import Register

    def cell(disp):
        return ir.Mem(Addr(Space.FRAME, disp), 2, Register.BP, disp, 1)

    load = lir.Insn(
        0x10, (0x10, 0x13), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (cell(-0x3C),)), (1,), ()
    )
    store = lir.Insn(
        0x13, (0x13, 0x16), ir.Semantics(ir.Operation.MOVE, "mov", (cell(-0x4E),), (ir.Held(1, 2),)), (), (1,)
    )
    limit = lir.Insn(
        0x16, (0x16, 0x18), ir.Semantics(ir.Operation.COMPARE, "cmp", (), (ir.Held(2, 2), ir.Held(1, 2))), (3,), (2, 1)
    )
    out = _out(_body(load, store, limit), {1})
    defined = {value for one in out for value in one.defines}
    assert all(value in defined for one in out for value in one.uses if value != 2), [str(one.what) for one in out]
