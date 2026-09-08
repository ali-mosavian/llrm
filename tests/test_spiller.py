"""
qbopt/spiller.py: a value the allocator would not keep, kept in memory.

The case this file exists for beyond the ordinary one: a move that belongs
to a phi's parallel copy. Spilling it the usual way -- a reload before, a
store after -- puts an instruction inside a group whose moves happen at
once, and parcopy.py then sees two runs instead of one. pressx-v-evt
emitted `r24 <- [bp-8]` and then `r27 <- r24`, and R came out 6460 for 7500.
"""

import pytest

from qbopt import ir
from qbopt import lir
from qbopt import spiller
from qbopt import frame as frames


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


def test_a_value_the_body_never_writes_is_stored_where_it_arrives() -> None:
    """A register live at entry has a value and no definition.

    Spilled, every use got a reload and nothing ever wrote the slot:
    lngmix, with both its divides hoisted, read `[bp-24h]` before its last
    call. The store goes at the top, out of the register the raise saw it
    in and named outright -- at entry the value is already there, so it is
    not something the allocator has to place.
    """
    from iced_x86 import Register

    class Value:
        """Enough of a MIR value for `origin`: the id is what is read."""

        def __init__(self, one: int) -> None:
            self.id = one

    live_in = Value(2)
    body = lir.LirBody(
        "one",
        0,
        (lir.LirBlock(at=0, insns=(_add(1, 2),), succ=()),),
        origin={live_in: Register.ESI},
        pins={},
    )
    got, _made = spiller.spilled(body, frozenset({2}), frames.Frame(0))
    insns = [one for block in got.blocks for one in block.insns]
    first = insns[0].what
    assert isinstance(first.dests[0], ir.Mem), f"the first instruction is {first}"
    assert first.sources[0] == ir.Reg(Register.SI, 2), f"stored out of {first.sources[0]}"
    assert not insns[0].uses, "the arrival store asks the allocator for nothing"

    # And the reload below reads the slot it wrote.
    reload = next(one for one in insns[1:] if one.what and isinstance(one.what.sources[0], ir.Mem))
    assert reload.what.sources[0].offset == first.dests[0].offset, "the store and the reload disagree on the slot"


def test_a_lifted_memory_operand_takes_the_fixup_with_it() -> None:
    """The fixup names the operand, so it goes where the operand goes.

    A tie with a memory source has no remedy but a load of its own: `add
    cx,[a]` with the accumulator spilled becomes `mov bx,[a]` and `add
    [bp-22h],bx`. Left on the survivor -- which no longer reads memory --
    the fixup was bound to whatever field it did have, and the address of
    `a` was written over the frame offset the add kept.
    """
    from qbopt.module import Addr
    from qbopt.module import Space

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


def test_nothing_is_stored_at_the_top_of_a_block_something_branches_back_to() -> None:
    """The arrival store is only sound where it runs once.

    At the top of a re-entered block it writes the slot again with
    whatever the register holds by then, which is not the value that
    arrived -- the second trip would read its own second-hand answer.
    """
    from iced_x86 import Register

    class Value:
        def __init__(self, one: int) -> None:
            self.id = one

    live_in = Value(2)
    body = lir.LirBody(
        "one",
        0,
        (lir.LirBlock(at=0, insns=(_add(1, 2),), succ=(0,)),),
        origin={live_in: Register.ESI},
        pins={},
    )
    got, _made = spiller.spilled(body, frozenset({2}), frames.Frame(0))
    insns = [one for block in got.blocks for one in block.insns]
    stores = [one for one in insns if one.what and isinstance(one.what.dests[0], ir.Mem)]
    assert not stores, f"a store was put at the top of a re-entered block: {stores[0].what}"


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


def test_a_grouped_move_with_both_ends_spilled_is_refused() -> None:
    """`mov [bp-2],[bp-4]` is not an instruction, and splitting it puts an
    ungrouped one inside the copy."""
    with pytest.raises(spiller.Simultaneous, match="scratch"):
        spiller.spilled(_body(_move(1, 2, group=1)), frozenset({1, 2}), frames.Frame(0))


def test_an_ordinary_instruction_still_spills_the_way_it_did() -> None:
    """Only a move in a parallel copy is rewritten in place. Anything else
    keeps the reload before and the store after."""
    got = _out(_body(_add(1, 2)), {2})
    assert len(got) == 2, [one.what.name for one in got]
    assert got[0].what.name == "mov" and isinstance(got[0].what.sources[0], ir.Mem)
    assert got[1].what.name == "add"


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

    from qbopt import ir
    from qbopt import lir
    from qbopt import spiller
    from qbopt.module import Addr
    from qbopt.module import Space

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
    from qbopt import lir

    return lir.LirBody("one", 0, (lir.LirBlock(at=0, insns=insns, succ=()),), origin={}, pins={})


def _frame():
    from qbopt import frame as frames

    return frames.Frame(floor=0)


def _through_regalloc(body):
    from qbopt import allocate
    from qbopt import frame as frames

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

    from qbopt import omf
    from qbopt import flow
    from qbopt import lower
    from qbopt import module
    from qbopt import phielim
    from qbopt import runtime
    from qbopt import twoaddr
    from qbopt import allocate
    from qbopt import coalesce
    from qbopt import mir as raise_
    from qbopt import blocks as split
    from qbopt import frame as frames
    from qbopt.blocks import code_map

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
