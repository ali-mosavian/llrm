"""Select complete far-pointer loads before allocation loses their address.

Open Watcom's C front end represents a far-pointer field load as two word
loads: its offset followed by its selector.  The final physical peephole can
recognise that shape only when both loads still use the same physical address
register.  Under pressure the allocator rematerializes a near owner for each
word, so it is too late there even though the pre-allocation LIR still proves
the words are adjacent parts of one cell.

This is lowering, not an LIR optimization pass.  It chooses the one x86 form
for a machine-neutral pair of loads while the pair's address identity is
available; allocation remains responsible for giving the offset an address
register and the selector ES.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import lir


def selected(insns: tuple[lir.Insn, ...]) -> tuple[lir.Insn, ...]:
    """Select private ``mov offset,[p]; mov selector,[p+2]`` pairs as ``les``.

    A combined load reads both words before writing either result, unlike the
    two-source sequence.  Refuse a pair if either result reaches either
    address; in that case the two forms can observe different addresses.
    Source-backed operations are likewise left to the post-allocation
    peephole, which owns the necessary byte-coverage rewrite.  Fresh C OMF
    operations carry zero-width source anchors, which remain safe to join.
    """
    made: dict[int, lir.Insn] = {}
    erased: set[int] = set()
    for at, first in enumerate(insns[:-1]):
        if at in erased:
            continue
        second = insns[at + 1]
        joined = _pair(first, second)
        if joined is None:
            continue
        made[at] = joined
        erased.add(at + 1)
    return tuple(
        made[at]
        if at in made
        else replace(lir.anchor(one), defines=(), uses=(), widths=())
        if at in erased
        else one
        for at, one in enumerate(insns)
    )


def _pair(first: lir.Insn, second: lir.Insn) -> lir.Insn | None:
    if (
        not _plain(first)
        or not _plain(second)
        or first.covers is not None
        and first.covers[0] != first.covers[1]
        or second.covers is not None
        and second.covers[0] != second.covers[1]
    ):
        return None
    words = []
    for one in (first, second):
        match one.what:
            case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held() as dest,), (ir.Mem() as cell,)):
                if dest.width != cell.width != 2:
                    return None
                words.append((dest, cell))
            case _:
                return None
    (first_dest, first_cell), (second_dest, second_cell) = words
    if not _next_word(first_cell, second_cell) or not _far_pointer_words(first, second):
        return None
    defined = {first_dest.value, second_dest.value}
    if any(value.value in defined for cell in (first_cell, second_cell) for value in ir.values(cell)):
        return None
    # C's far-pointer representation is low offset then high selector.  The
    # selector is confined by its later far-cell uses to a segment register;
    # the allocator's existing selector order prefers ES, making LES the
    # exact one-instruction load without introducing a fixed-register pin.
    what = ir.Semantics(ir.Operation.MOVE, "les", (first_dest, second_dest), (replace(first_cell, width=4),))
    return replace(
        first,
        what=what,
        defines=tuple(dict.fromkeys((*first.defines, *second.defines))),
        uses=tuple(dict.fromkeys((*first.uses, *second.uses))),
    )


def _plain(one: lir.Insn) -> bool:
    return not (
        one.clobbers
        or one.clobbers_high
        or one.requires
        or one.delivers
        or one.spread
        or one.group is not None
        or one.symbol is True
        or one.frame_adjust
        or one.spill_reload
        or one.spill_store
        or one.rematerialized
    )


def _next_word(low: ir.Mem, high: ir.Mem) -> bool:
    """Whether ``high`` is the word immediately after ``low`` by one address."""
    same = lambda cell: replace(cell, addr=None if cell.addr is None else replace(cell.addr, disp=0), offset=0)  # noqa: E731
    if same(low) != same(high):
        return False
    if low.addr is None or high.addr is None:
        return low.addr is None and high.addr is None and high.offset == low.offset + 2
    moved = high.addr.disp - low.addr.disp
    return moved == 2 and high.offset - low.offset in (0, 2) or moved == 0 and high.offset == low.offset + 2


def _far_pointer_words(first: lir.Insn, second: lir.Insn) -> bool:
    """Whether the two loads are adjacent halves of one typed far pointer.

    Adjacent machine addresses alone prove nothing: two near parameters also
    occupy consecutive words.  The C raise records a far pointer as two
    ``pointer4`` slices of the same source object, so require that semantic
    fact as well as the encoding-level address proof above.
    """
    refs = []
    for one in (first, second):
        loaded = () if one.op is None else one.op.loads
        if len(loaded) != 1 or loaded[0].width != 2 or loaded[0].volatile or not loaded[0].typed:
            return False
        if loaded[0].typed[0] != "pointer4" or len(loaded[0].provenance.slices) != 1:
            return False
        refs.append((loaded[0], next(iter(loaded[0].provenance.slices))))
    (low, low_slice), (high, high_slice) = refs
    return (
        low_slice.object == high_slice.object
        and low_slice.high + 1 == high_slice.low
        and low_slice.stride == high_slice.stride
        and low_slice.width == high_slice.width
    )
