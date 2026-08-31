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
    """How many bytes the op occupied in BC's own image."""
    found = getattr(op.node, "insn", None)
    return None if found is None else found.length


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


def _field_in(found: Module, op: mir.Op) -> int | None:
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
    if op.at in found.calls and found.code[op.at : op.at + 1] == b"\x9a":
        return op.at + 1
    span = ir.span(op.node)
    inside = [one for one in found.fixup_at if span[0] <= one < span[1]]
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


def lay_out(body: MirBody, at: int, found: Module) -> Laid | str:
    """Every op in `body`, emitted in order from `at`, or why it could not be."""
    return _emitted(_ordered(body), at, found)


def rebuild(found: Module, bodies: list[tuple[str, MirBody]]) -> Laid | str:
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

    # Every byte between the first op and the last has to be an op. A gap is
    # data sitting in the middle of the code -- an ON GOTO table, which BC
    # puts inline -- and emitting only the instructions would drop it
    # silently along with every code offset it holds. Eight of the corpus's
    # forty-two rebuildable objects have one, and remapping its entries
    # through `moved` is its own piece of work rather than something to
    # improvise here.
    sized = [(one.at, _length_of(one)) for one in ops]
    if any(length is None for _, length in sized):
        return f"{ops[0].at:#06x}: an op with no instruction behind it"
    covered = sum(length for _, length in sized if length is not None)
    span = max(at + (length or 0) for at, length in sized) - ops[0].at
    if covered != span:
        return f"{ops[0].at:#06x}: {span - covered} bytes between the ops are not instructions"

    return _emitted(ops, ops[0].at, found)


def _emitted(ops: list[mir.Op], at: int, found: Module) -> Laid | str:
    """`ops` in order from `at`, shrunk to a fixed point and emitted."""
    if not ops:
        return "no ops to lay out"

    lengths: dict[int, int] = {}
    for op in ops:
        what = _semantics(op)
        if what is None:
            return f"{op.at:#06x}: {op.name} has no semantics to select from"
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
        field = _field_in(found, op)
        if field is not None:
            landed = made.relocated_at
            if landed is None:
                return f"{op.at:#06x}: {op.name} has a fixup and no field to put it in"
            relocations.append((len(out) + landed, field))
        out += made.code
    return Laid(bytes(out), moved, tuple(relocations))
