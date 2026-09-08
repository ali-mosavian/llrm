"""Spilling: a value the allocator would not keep, kept in memory instead.

LLVM's `InlineSpiller`. Choosing to spill is the allocator's; making the
program work afterwards is this pass's, and until it existed the choice was
made and nothing acted on it -- `allocate.Spilled` was raised on 237 of 487
objects because an operand still named a value with no register.

Every definition of a spilled value becomes a store into its frame slot,
and every use a load out of it into a fresh value that lives only across
that one instruction. LLVM calls the fresh value the reload's, and it is
what makes the spilled value's live range vanish: nothing is live between
the store and the load, so the register the value wanted is free.

**Not folded.** LLVM tries to fold the reload into the instruction that
reads it -- `add ax,[bp-12h]` rather than a load and then an add -- which
is a peephole over the result and belongs after this rather than inside it.
"""

from dataclasses import replace

from qbopt import ir
from qbopt import lir
from qbopt import target
from qbopt import frame as frames
from qbopt.passes import LIRTransform


class Spiller(LIRTransform):
    name = "spill"

    def __init__(self, spilled: "frozenset[int]", frame: "frames.Frame | None" = None) -> None:
        self.spilled = spilled
        self.frame = frame

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return spilled(body, self.spilled, self.frame)


def spilled(
    body: lir.LirBody, values: "frozenset[int]", frame: "frames.Frame | None" = None
) -> "tuple[lir.LirBody, frozenset[int]]":
    """`body` with each of `values` living in a frame slot, and the reloads.

    The second half matters as much as the first. A reload's value is live
    across one instruction and *must* have a register: spilling it again
    puts a load in front of a load and the allocator never settles -- three
    values spilled every round, three instructions added every round, for
    ever. LLVM says so as `LiveInterval::markNotSpillable`, and the caller
    says it here by handing these back to `allocate` as unspillable.
    """
    if not values:
        return body, frozenset()
    frame = frame if frame is not None else frames.of(body)
    fresh = _next_value(body)
    made: set[int] = set()

    blocks = []
    for block in body.blocks:
        insns: list[lir.Insn] = []
        for one in block.insns:
            direct = _in_place(one, values, frame) or _tied(one, values, frame)
            if direct is not None:
                insns.append(direct)
                continue
            loaded = _memory_source_read_first(one, values, fresh)
            if loaded is not None:
                before, one = loaded
                fresh += 1
                insns.append(before)
                direct = _tied(one, values, frame)
                if direct is not None:
                    insns.append(direct)
                    continue
            before, after, rename = [], [], {}
            for value in one.uses:
                if value not in values:
                    continue
                rename[value] = fresh
                before.append(_reload(one, fresh, frame.cell(value, _width(one, value))))
                fresh += 1
            for value in one.defines:
                if value not in values:
                    continue
                rename[value] = fresh
                after.append(_store(one, fresh, frame.cell(value, _width(one, value))))
                fresh += 1
            insns += before
            insns.append(_renamed(one, rename) if rename else one)
            insns += after
            made.update(rename.values())
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks)), frozenset(made)


def _width(one: lir.Insn, value: int) -> int:
    """How wide this instruction reads or writes the value, defaulting to a word.

    A requirement answers for an operation that names no operand at all:
    the restore idiom pushes four bytes and its semantics have no
    destination to say so, so the reload came back two bytes wide and the
    push carried a stale high half.
    """
    for held, _register in one.requires + one.delivers:
        if held.value == value:
            return held.width
    if one.what is None:
        return frames.WORD
    for where in (*one.what.dests, *one.what.sources):
        if isinstance(where, ir.Held) and where.value == value:
            return where.width
    return frames.WORD


def _reload(beside: lir.Insn, into: int, cell: ir.Mem) -> lir.Insn:
    """The load that puts a spilled value back for one instruction."""
    return _inserted(beside, ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(into, cell.width),), (cell,)), (into,), ())


def _store(beside: lir.Insn, out_of: int, cell: ir.Mem) -> lir.Insn:
    """The store that puts a spilled value away as soon as it is written."""
    return _inserted(
        beside, ir.Semantics(ir.Operation.MOVE, "mov", (cell,), (ir.Held(out_of, cell.width),)), (), (out_of,)
    )


def _inserted(beside: lir.Insn, what: ir.Semantics, defines: tuple, uses: tuple) -> lir.Insn:
    """An instruction that stands beside another and claims none of its bytes."""
    at = beside.covers[0] if beside.covers else beside.at
    return lir.Insn(at=beside.at, covers=(at, at), what=what, defines=defines, uses=uses, op=beside.op)


def _wants(side: tuple, rename: dict[int, int]) -> tuple:
    """A requirement, naming whichever value now feeds the instruction.

    The register an instruction demands is a fact about the instruction,
    so replacing the value that reaches it does not lift the demand. Left
    naming the value that was spilled, the restore idiom's reload arrived
    unpinned and the allocation put it in cx while `push eax` went on
    reading eax.
    """
    return tuple(
        (ir.Held(rename.get(held.value, held.value), held.width), register) for held, register in side
    )


def _renamed(one: lir.Insn, rename: dict[int, int]) -> lir.Insn:
    """The instruction reading and writing the reload's value instead."""
    what = one.what
    if what is None:
        return replace(
            one,
            defines=tuple(rename.get(v, v) for v in one.defines),
            uses=tuple(rename.get(v, v) for v in one.uses),
            requires=_wants(one.requires, rename),
            delivers=_wants(one.delivers, rename),
        )
    return replace(
        one,
        what=ir.Semantics(
            what.op,
            what.name,
            tuple(_settled(x, rename) for x in what.dests),
            tuple(_settled(x, rename) for x in what.sources),
            what.target,
        ),
        defines=tuple(rename.get(v, v) for v in one.defines),
        uses=tuple(rename.get(v, v) for v in one.uses),
        requires=_wants(one.requires, rename),
        delivers=_wants(one.delivers, rename),
    )


def _settled(where, rename: dict[int, int]):
    """One operand with every value it names put through the rename.

    Through `ir.mapped`, so a cell's base is renamed with the rest: this
    looked in `Mem.through`, which holds a register, and spilling a
    pointer renamed the load's `uses` and left the cell addressed through
    a value that now lives in a frame slot.
    """
    return ir.mapped(where, lambda one: ir.Held(rename.get(one.value, one.value), one.width))


class Simultaneous(Exception):
    """A move in a parallel copy with both ends spilled.

    `mov [bp-2],[bp-4]` is not an instruction, and breaking it into a load
    and a store puts an ungrouped one inside a copy whose moves happen at
    once -- which is what parcopy.py then cannot schedule as one group.
    Refused by name until a scratch register can be reserved for it.
    """


def _in_place(one: lir.Insn, values: "frozenset[int]", frame) -> "lir.Insn | None":
    """One move of a parallel copy, with its spilled end read or written where it lives.

    A phi's moves happen at once. Spilling one of them the ordinary way --
    a reload before it and a store after it -- puts an instruction inside
    the group that is not part of it, and the group stops being one run.
    x86 reads and writes memory in a move, so the slot goes in the operand
    and the copy stays one instruction.
    """
    what = one.what
    if one.group is None or what is None or what.op is not ir.Operation.MOVE:
        return None
    if len(what.dests) != 1 or len(what.sources) != 1:
        return None
    into = [v for v in one.defines if v in values]
    outof = [v for v in one.uses if v in values]
    if not into and not outof:
        return None
    if into and outof:
        raise Simultaneous(
            f"{one.at:#06x}: a move in a parallel copy has both ends spilled and "
            "needs a scratch register this does not reserve yet"
        )
    value = (into or outof)[0]
    cell = frame.cell(value, _width(one, value))
    if into:
        return replace(
            one,
            what=ir.Semantics(what.op, what.name, (cell,), what.sources),
            defines=tuple(v for v in one.defines if v != value),
        )
    return replace(
        one,
        what=ir.Semantics(what.op, what.name, what.dests, (cell,)),
        uses=tuple(v for v in one.uses if v != value),
    )


def _memory_source_read_first(
    one: lir.Insn, values: "frozenset[int]", fresh: int
) -> "tuple[lir.Insn, lir.Insn] | None":
    """The load, and the instruction reading the loaded value instead.

    Only for a tie: a value an instruction both reads and writes has no
    remedy but the operand, and that is unavailable while another operand
    is already a cell. Reading the cell into a value first leaves the one
    memory operand for the tie. The load is short and the allocator places
    it like any other reload.
    """
    what = one.what
    if what is None or one.group is not None:
        return None
    tied = [value for value in one.defines if value in values and value in one.uses]
    if len(tied) != 1:
        return None
    cells = [x for x in what.sources if isinstance(x, ir.Mem)]
    if len(cells) != 1 or any(isinstance(x, ir.Mem) for x in what.dests):
        return None
    cell = cells[0]
    held = ir.Held(fresh, cell.width)
    load = _inserted(
        one,
        ir.Semantics(ir.Operation.MOVE, "mov", (held,), (cell,)),
        (fresh,),
        tuple(where.value for where in ir.values(cell)),
    )
    return load, replace(
        one,
        what=ir.Semantics(
            what.op,
            what.name,
            what.dests,
            tuple(held if x is cell else x for x in what.sources),
            what.target,
        ),
        uses=tuple(one.uses) + (fresh,),
    )


def _tied(one: lir.Insn, values: "frozenset[int]", frame) -> "lir.Insn | None":
    """A value an instruction both reads and writes, kept in its slot.

    Spilling one of these buys nothing: the reload is tied at the same
    instruction, so the next round faces the same conflict with a new
    value, and the allocator adds two instructions per round for ever --
    pressx spilled 207, then 212, then 215, at one `add`.

    x86 reads and writes memory in place, so the slot goes in the operand
    on both sides and no reload or store is needed. Only where nothing
    else in the instruction wants memory: one memory operand is all an
    instruction has. What can be encoded is select.py's to say -- an
    unsupported form refuses there, which is a refusal and not a wrong
    program.
    """
    what = one.what
    if what is None or one.group is not None:
        return None
    tied = [v for v in one.defines if v in values and v in one.uses]
    if len(tied) != 1:
        return None
    value = tied[0]
    # A fixed register is a fixed register: a slot is not one.
    if any(
        isinstance(side[where.index], ir.Held) and side[where.index].value == value
        for where in target.requirements(what)
        for side in ((what.dests if where.side == "dest" else what.sources),)
        if where.index < len(side)
    ):
        return None
    # One memory operand is all there is, so nothing else may want it.
    for operand in (*what.dests, *what.sources):
        if isinstance(operand, ir.Mem):
            return None
        if isinstance(operand, ir.Held) and operand.value != value and operand.value in values:
            return None
    cell = frame.cell(value, _width(one, value))
    swap = lambda x: cell if isinstance(x, ir.Held) and x.value == value else x  # noqa: E731
    return replace(
        one,
        what=ir.Semantics(
            what.op, what.name, tuple(swap(x) for x in what.dests), tuple(swap(x) for x in what.sources), what.target
        ),
        defines=tuple(v for v in one.defines if v != value),
        uses=tuple(v for v in one.uses if v != value),
    )


def _next_value(body: lir.LirBody) -> int:
    """One past the highest value id this body names."""
    seen = {0}
    for block in body.blocks:
        seen.update(block.arrives)
        for one in block.insns:
            seen.update(one.defines)
            seen.update(one.uses)
    return max(seen) + 1
