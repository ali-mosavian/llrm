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


def test_c_matmul_multiplies_spilled_rows_directly_from_memory() -> None:
    """Matmul reloaded all eight unrolled lhs values solely to feed ``imul``.

    The other factor dies at the multiply and can own its result register, so
    commutative two-address selection must leave the long-lived row value in
    its spill slot and use x86's register-by-memory multiply form.
    """
    from qbopt.cfront import compile as cfront
    from tools import quality

    source = Path("bench/c/matmul.c")
    module = cfront.assembled(cfront.recorded(source, []), source.stem, optimise=True)
    procedure = next(one for one in module.procedures if one.name == "_bench_matmul")
    multiplies = [
        operands
        for _raw, mnemonic, operands in quality._rows(quality._blob(module, procedure, 0))
        if mnemonic == "imul"
    ]

    assert len(multiplies) >= 8
    assert sum("[bp" in operands.lower() for operands in multiplies) >= 8, multiplies


def test_nonoverlapping_spills_share_one_compatible_frame_slot() -> None:
    """Two sequential spills reserved separate words although their lifetimes never overlap."""
    frame = frames.Frame(0)
    body = _body(_move(1, 10, at=0x10), _add(11, 1, at=0x12), _move(2, 20, at=0x14), _add(21, 2, at=0x16))
    spiller.spilled(body, frozenset({1, 2}), frame)
    assert frame.slots[1] == frame.slots[2]
    assert frame.size == 2


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
