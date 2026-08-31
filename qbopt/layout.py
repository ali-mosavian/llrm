"""
A whole body, emitted -- and everything that has to move when it does.

select.py turns one operation into bytes and takes the address as a given.
That is not enough to rebuild a body, because the addresses are the thing
that changes: an instruction whose encoding differs in length from the one
BC wrote moves everything after it, and every branch into that region is
then pointing at the wrong byte.

So this is a fixed point, and the shape of it is what keeps it honest.
Every branch starts long. Addresses are assigned, each branch that reaches
its target within a signed byte is marked short, and the addresses are
assigned again. Shrinking only ever brings a target closer, so a branch
marked short stays reachable and the loop only ever goes one way -- which
is why it terminates rather than oscillating between two lengths that each
justify the other.

Then one final pass emits at the addresses the fixed point settled on, with
every target mapped through where it went.

What comes back is bytes plus the relocations, because a relocated
displacement is emitted as zero and the fixup that names it has to be moved
to wherever the field ended up.

Refuses the whole body where it cannot emit one op. A body half of which is
this pass's own code and half BC's is not something anything downstream
could reason about, and the fraction that cannot be emitted is small and
known: the x87 instructions, and the addresses in a space select.py does
not encode.
"""

from dataclasses import dataclass

from qbopt import ir
from qbopt import mir
from iced_x86 import OpKind

from qbopt import select
from qbopt.mir import MirBody
from qbopt.module import Module


@dataclass(frozen=True, slots=True)
class Laid:
    """A body's new bytes, and what moved."""

    code: bytes
    # Where each op ended up, old address -> new. The map a caller needs to
    # move anything that pointed into this body from outside it.
    moved: dict[int, int]
    # (offset within `code`, the original field's own address) for every
    # relocated displacement, so the fixup that names it can be moved.
    relocations: tuple[tuple[int, int], ...]

    @property
    def grew(self) -> int:
        return len(self.code)


def _ordered(body: MirBody) -> list[mir.Op]:
    """Every op in address order, which is the order they are emitted in.

    Blocks are laid out where they already were rather than reordered: a
    different order is a different program's control flow, and nothing here
    is asking for one.
    """
    return sorted((op for block in body.blocks for op in block.ops), key=lambda one: one.at)


def _length_of(op: mir.Op) -> int | None:
    """How many bytes the op occupied in the image it came from.

    From the node's own span rather than from an instruction, because not
    every node has one: calls.py's restore idiom is a single node covering
    four bytes and three instructions, and it is in every object this pass
    has already absorbed a call in.
    """
    if op.node is None:
        return None
    lo, hi = ir.span(op.node)
    return hi - lo


def _trailing_zeros(found: Module, ops: list[mir.Op]) -> "Table | None":
    """The run of zero bytes the ops end on, where it reaches the segment's end.

    Only at the very end, and only all-zero: anything else that happens to
    decode is code until something proves otherwise.
    """
    highest = max(one.at + (_length_of(one) or 0) for one in ops)
    if highest != found.end:
        return None
    lo = found.end
    for one in sorted(ops, key=lambda x: x.at, reverse=True):
        length = _length_of(one) or 0
        if one.at + length != lo or any(found.code[one.at : lo]):
            break
        lo = one.at
    return None if lo == found.end else Table(lo, found.end)


PADDING = frozenset({0x90, 0x00})


def _padding_runs(
    found: Module, ops: list[mir.Op], carried: list["Table"], lowest: int, highest: int
) -> list["Table"]:
    """The gaps between the items that are nothing but padding bytes."""
    covered = set()
    for one in ops:
        covered.update(range(one.at, one.at + (_length_of(one) or 0)))
    for one in carried:
        covered.update(range(one.lo, one.hi))

    out: list[Table] = []
    start: int | None = None
    for at in range(lowest, highest + 1):
        empty = at < highest and at not in covered
        if empty and start is None:
            start = at
        elif not empty and start is not None:
            if all(one in PADDING for one in found.code[start:at]):
                out.append(Table(start, at))
            start = None
    return out


def _semantics(op: mir.Op) -> ir.Semantics | None:
    what = getattr(op.node, "semantics", None)
    return None if what is None or what.op is ir.Operation.BARRIER else what


def _retargeted(what: ir.Semantics, moved: dict[int, int]) -> ir.Semantics | None:
    """`what` with its target moved to wherever that instruction went.

    A target this body does not contain is refused rather than left alone:
    it would be an address into code this layout did not place, and quietly
    keeping the old number would point it at whatever now sits there.
    """
    if what.target is None:
        return what
    landed = moved.get(what.target)
    if landed is None:
        return None
    return ir.Semantics(what.op, what.name, what.dests, what.sources, landed)


def _field_in(found: Module, op: mir.Op, fields: frozenset[int] = frozenset()) -> int | None:
    """The address of the one relocated field inside `op`'s own bytes.

    Asked of the module rather than taken from the instruction's `disp_at`,
    which is the displacement and not always the field.

    A far call is the case that forced this and is handled apart: its four
    relocated bytes are a target rather than a displacement, so `disp_at` is
    None, and `fixup_at` does not carry it either -- that map holds the
    OFF16 fixups behind memory operands, and a far call's is a PTR32. What
    does know is `Module.calls`, and `at + 1` is not a guess: `9a` then four
    bytes is the only encoding a far call has.

    Otherwise exactly one fixup in the instruction's own span, or nothing.
    Two would mean an instruction with two relocated operands, which nothing
    here emits and which would have to say which field went where.
    """
    if op.node is None:
        return None
    known = fields or frozenset(found.fixup_at)
    lo, hi = ir.span(op.node)
    # A far call and a far jmp put their four relocated bytes right after a
    # one-byte opcode. Neither is a displacement, so neither is where a
    # general search would look.
    if found.code[op.at : op.at + 1] in (b"\x9a", b"\xea") and op.at + 1 in known:
        return op.at + 1
    inside = [one for one in known if lo <= one < hi]
    return inside[0] if len(inside) == 1 else None


# A signed byte's worth of displacement, measured from the end of the
# instruction. The short branch's whole range.
REACH = range(-128, 128)


def _placed(ops: list[mir.Op], at: int, lengths: dict[int, int]) -> dict[int, int]:
    """Where each op lands, given what each one measures."""
    moved: dict[int, int] = {}
    where = at
    for op in ops:
        moved[op.at] = where
        where += lengths[op.at]
    return moved


def lay_out(body: MirBody, at: int, found: Module, fields: frozenset[int] = frozenset()) -> Laid | str:
    """Every op in `body`, emitted in order from `at`, or why it could not be."""
    return _emitted(_ordered(body), at, found, fields)


@dataclass(frozen=True, slots=True)
class Table:
    """A run of bytes between the instructions, copied rather than selected.

    BC drops an ON GOTO table inline: a count byte and one relocated word
    per destination. The words are fixups, so copying the bytes and moving
    the fixups is enough -- as_records remaps each one's own displacement,
    which is where the destination actually lives.
    """

    lo: int
    hi: int

    @property
    def at(self) -> int:
        return self.lo


def rebuild(
    found: Module,
    bodies: list[tuple[str, MirBody]],
    tables: tuple[tuple[int, int], ...] = (),
    fields: frozenset[int] = frozenset(),
) -> Laid | str:
    """Every body in the module, laid out one after another.

    Whole-segment rather than per-body, because per-body does not work:
    splicing one back into BC's own layout is possible for 1 of the corpus's
    171 bodies -- the rest cross a LEDATA boundary, are not contiguous, or
    are branched into from outside. None of that applies to writing the
    segment, where boundaries and offsets are being produced rather than
    preserved.

    It also settles the targets that refused per-body: a branch from one
    body into another has somewhere to land once every body is in the same
    map.

    What comes back starts at the first body's own address. Whatever sits
    before it -- BC's module header, 48 bytes of `blARITH` and padding, and
    the only thing in the corpus's code segments that is not in a body --
    is the caller's to keep.
    """
    ops = sorted(
        (op for _, body in bodies for op in _ordered(body)),
        key=lambda one: one.at,
    )
    if not ops:
        return "no bodies to rebuild"
    if any(_length_of(one) is None for one in ops):
        return f"{ops[0].at:#06x}: an op with no instruction behind it"

    lowest = ops[0].at
    highest = max(one.at + (_length_of(one) or 0) for one in ops)
    inside = [Table(lo, hi) for lo, hi in tables if lowest <= lo and hi <= highest]

    # BC pads the end of its code segment with zeros, and every object in
    # the corpus ends with four of them. Reachability walks in and the
    # decoder obliges -- `00 00` is `add [bx+si],al` -- so they arrive here
    # as ops, on an address no fixup names and select.py rightly will not
    # encode. They are not instructions and are carried rather than
    # selected: the same bytes, in the same place, which is the only thing
    # that can be right about padding.
    padding = _trailing_zeros(found, ops)
    if padding is not None:
        inside.append(padding)
        ops = [one for one in ops if one.at < padding.lo]
        if not ops:
            return "the body is nothing but padding"
        highest = padding.hi

    # BC aligns its procedures, so runs of `90` sit between them, and
    # nothing reaches those. Carried the same way a table is: the bytes are
    # what they were and nothing enters them, so where they end up does not
    # matter. Only runs that are entirely padding -- anything else in a gap
    # is bytes this cannot account for, and it says so instead.
    inside += _padding_runs(found, ops, inside, lowest, highest)

    # Every byte between the first item and the last has to be one of them.
    # What is left over is data nothing here can name, and emitting only what
    # it understands would drop it silently along with anything it holds.
    covered = sum(_length_of(one) or 0 for one in ops) + sum(one.hi - one.lo for one in inside)
    if covered != highest - lowest:
        return f"{lowest:#06x}: {highest - lowest - covered} bytes between the ops are not instructions"

    return _emitted(sorted([*ops, *inside], key=lambda one: one.at), lowest, found, fields)


def _emitted(ops: list, at: int, found: Module, fields: frozenset[int] = frozenset()) -> Laid | str:
    """Every item in order from `at`, shrunk to a fixed point and emitted.

    An item is an op, which select.py encodes, or a Table, which is copied.
    """
    if not ops:
        return "no ops to lay out"

    lengths: dict[int, int] = {}
    for op in ops:
        if isinstance(op, Table):
            lengths[op.at] = op.hi - op.lo
            continue
        what = _semantics(op)
        if isinstance(op.node, ir.Restore) or what is None or found.code[op.at : op.at + 1] == bytes([0xCD]):
            lengths[op.at] = _length_of(op) or 0
            continue
        made = select.emit(what, at=at)
        if made is None:
            return f"{op.at:#06x}: {op.name} is not one select.py can emit"
        lengths[op.at] = len(made.code)

    # Shrink to a fixed point. Every branch starts long; one that reaches its
    # target within a signed byte becomes short, which moves everything after
    # it closer and can only let more of them shrink.
    short: set[int] = set()
    moved = _placed(ops, at, lengths)
    changing = True
    while changing:
        changing = False
        for op in ops:
            if isinstance(op, Table):
                continue
            what = _semantics(op)
            if what is None or what.target is None or op.at in short:
                continue
            landed = moved.get(what.target)
            if landed is None:
                continue
            made = select.emit(what, at=moved[op.at], short=True)
            if made is None:
                continue  # a call has no short form, and says so by refusing
            if landed - (moved[op.at] + len(made.code)) not in REACH:
                continue
            short.add(op.at)
            lengths[op.at] = len(made.code)
            changing = True
        if changing:
            moved = _placed(ops, at, lengths)

    # The bytes, at the addresses the fixed point settled on.
    out = bytearray()
    relocations: list[tuple[int, int]] = []
    for op in ops:
        if isinstance(op, Table):
            # Copied verbatim, with every fixup inside it moved by the same
            # amount the table itself moved. The entries are relocated words
            # and their destinations live in the fixups' own displacements,
            # which as_records remaps.
            out += found.code[op.lo : op.hi]
            for field in sorted(one for one in (fields or frozenset(found.fixup_at)) if op.lo <= one < op.hi):
                relocations.append((moved[op.at] - at + (field - op.lo), field))
            continue
        # An emulated x87 site is emitted as it was found. declen.py decodes
        # `cd 35 46 c8` as the fld it stands for, so selecting from the
        # semantics would emit `d9 46 c8` -- a native instruction, on a
        # machine that may have no coprocessor. That conversion is a
        # decision fpu.py gates behind --native-fpu, and laying a segment
        # out is not the place to make it silently.
        if found.code[op.at : op.at + 1] == bytes([0xCD]) and (length := _length_of(op)):
            # Copied, so any fixup inside it keeps its place within the
            # instruction and only the instruction itself has moved.
            field = _field_in(found, op, fields)
            if field is not None:
                relocations.append((len(out) + (field - op.at), field))
            out += found.code[op.at : op.at + length]
            continue
        # A barrier is an instruction ir.py models nothing about --
        # `movsx eax,bx` is one -- so there is nothing to select from and
        # its own bytes are the only right answer. Carried, unless it names
        # a branch target: that would move, and keeping the old number
        # would point it at whatever now sits there.
        if _semantics(op) is None and (length := _length_of(op)):
            found_insn = getattr(op.node, "insn", None)
            if found_insn is not None and found_insn.insn.op0_kind == OpKind.NEAR_BRANCH16:
                return f"{op.at:#06x}: a branch this cannot model would keep a stale target"
            field = _field_in(found, op, fields)
            if field is not None:
                relocations.append((len(out) + (field - op.at), field))
            out += found.code[op.at : op.at + length]
            continue
        if isinstance(op.node, ir.Restore):
            made = select.restore(op.node.pair)
            if made is None or len(made.code) != lengths[op.at]:
                return f"{op.at:#06x}: the restore idiom did not come back its own length"
            out += made.code
            continue
        before = _semantics(op)
        if before is None:
            return f"{op.at:#06x}: {op.name} has no semantics to select from"
        what = _retargeted(before, moved)
        if what is None:
            return f"{op.at:#06x}: its target is not in this body"
        made = select.emit(what, at=moved[op.at], short=op.at in short)
        if made is None or len(made.code) != lengths[op.at]:
            return f"{op.at:#06x}: it changed length between the two passes"
        # A fixup goes wherever this instruction's one relocatable field
        # landed -- the displacement for a memory operand, the immediate for
        # `push offset X` and `mov ax,offset X`, which are 464 of the
        # corpus's fixups on their own.
        field = _field_in(found, op, fields)
        if field is not None:
            landed = made.relocated_at
            if landed is None:
                return f"{op.at:#06x}: {op.name} has a fixup and no field to put it in"
            relocations.append((len(out) + landed, field))
        out += made.code
    return Laid(bytes(out), moved, tuple(relocations))
