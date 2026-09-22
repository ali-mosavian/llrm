"""Select complete far-pointer loads before allocation loses their address.

A far pointer reaches lowering as two word loads -- its offset, then its
selector -- whether a frontend spelt it so or `narrow` split a dword read only
through its halves.  They are one far load when the high word is only ever a
cell's selector; that use, not a source type, is what makes `les` the right
instruction.  The final physical peephole can
recognise that shape only when both loads still use the same physical address
register.  Under pressure the allocator rematerializes a near owner for each
word, so it is too late there even though the pre-allocation LIR still proves
the words are adjacent parts of one cell.

This is lowering, not an LIR optimization pass.  It chooses the one x86 form
for a machine-neutral pair of loads while the pair's address identity is
available; allocation remains responsible for giving the offset an address
register and the selector ES, FS or GS.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import lir


def selected(insns: tuple[lir.Insn, ...], selectors: frozenset[int]) -> tuple[lir.Insn, ...]:
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
        joined = _pair(first, second, selectors)
        if joined is None:
            continue
        made[at] = joined
        erased.add(at + 1)
    return tuple(
        made[at] if at in made else replace(lir.anchor(one), defines=(), uses=(), widths=()) if at in erased else one
        for at, one in enumerate(insns)
    )


def _pair(first: lir.Insn, second: lir.Insn, selectors: frozenset[int]) -> lir.Insn | None:
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
    if not _next_word(first_cell, second_cell) or second_dest.value not in selectors or _volatile(first, second):
        return None
    # A fixed address survives allocation unchanged.  Leave its two semantic
    # values independent so spilling may rematerialize either word from the
    # original cell and selector allocation may choose ES, FS or GS directly;
    # the physical far-load peephole can still fuse adjacent survivors.  A
    # virtual base/selector/index is different: allocation can reconstruct its
    # owner separately for each word, destroying the common address before the
    # physical peephole sees it.  Only that genuinely lossy boundary needs the
    # early complete-load selection performed here.
    if not tuple(ir.values(first_cell)):
        return None
    defined = {first_dest.value, second_dest.value}
    if any(value.value in defined for cell in (first_cell, second_cell) for value in ir.values(cell)):
        return None
    # Spelt `les`; the rewriter respells it for the segment register the
    # selector is given.
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


def _volatile(first: lir.Insn, second: lir.Insn) -> bool:
    return any(ref.volatile for one in (first, second) if one.op is not None for ref in one.op.loads)


def selectors(made: dict[int, tuple[lir.Insn, ...]], kept: frozenset[int]) -> frozenset[int]:
    """Values read only as a cell's selector: what makes a far load the right load.

    `kept` holds values a phi reads, which no instruction here shows.
    """
    selecting: set[int] = set()
    numeric: set[int] = set(kept)
    for block in made.values():
        for one in block:
            numeric.update(held.value for held, _ in (*one.requires, *one.delivers))
            if one.what is None:
                continue
            for where in (*one.what.dests, *one.what.sources):
                if isinstance(where, ir.Mem):
                    numeric.update(held.value for held in (where.base, where.index) if held is not None)
                    if where.selector is not None:
                        selecting.add(where.selector.value)
            numeric.update(where.value for where in one.what.sources if isinstance(where, ir.Held))
    return frozenset(selecting - numeric)
