"""
A whole body, emitted -- and everything that has to move when it does.

select.py turns one operation into bytes and takes the address as a given.
That is not enough to rebuild a body, because the addresses are the thing
that changes: an instruction whose encoding differs in length from the one
BC wrote moves everything after it, and every branch into that region is
then pointing at the wrong byte.

So this is two passes and no iteration. The first learns each op's length,
which is knowable without any target because select.py always emits the
near form of a branch -- BC writes `e9 0b 00` where `eb 0c` reaches, so
this is no worse, and refusing to choose is what keeps a length from
depending on a displacement that depends on a length. The second emits at
the address the first assigned, with every target mapped through where it
went.

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


def lay_out(body: MirBody, at: int, found: Module) -> Laid | str:
    """Every op in `body`, emitted in order from `at`, or why it could not be."""
    ops = _ordered(body)
    if not ops:
        return "no ops to lay out"

    # Pass one: lengths. A near branch is three bytes whatever it targets, so
    # this needs no addresses and cannot disagree with pass two about them.
    lengths: dict[int, int] = {}
    for op in ops:
        what = _semantics(op)
        if what is None:
            return f"{op.at:#06x}: {op.name} has no semantics to select from"
        made = select.emit(what, at=at)
        if made is None:
            return f"{op.at:#06x}: {op.name} is not one select.py can emit"
        lengths[op.at] = len(made.code)

    moved: dict[int, int] = {}
    where = at
    for op in ops:
        moved[op.at] = where
        where += lengths[op.at]

    # Pass two: the bytes, at the addresses pass one assigned.
    out = bytearray()
    relocations: list[tuple[int, int]] = []
    for op in ops:
        before = _semantics(op)
        if before is None:
            return f"{op.at:#06x}: {op.name} has no semantics to select from"
        what = _retargeted(before, moved)
        if what is None:
            return f"{op.at:#06x}: its target is not in this body"
        made = select.emit(what, at=moved[op.at])
        if made is None or len(made.code) != lengths[op.at]:
            return f"{op.at:#06x}: it changed length between the two passes"
        if made.displacement_at is not None:
            field = _field_in(found, op)
            if field is None:
                return f"{op.at:#06x}: a relocated displacement with no field to move"
            relocations.append((len(out) + made.displacement_at, field))
        out += made.code
    return Laid(bytes(out), moved, tuple(relocations))
