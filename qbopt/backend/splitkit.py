"""Splitting a live range instead of spilling the whole of it.

LLVM's `SplitKit`, driven by `RegAllocGreedy`. A value live across a loop
it is never read inside costs a register for the whole loop; cut its range
at the loop's edges and the two halves can take different registers, or the
middle can be spilled while the ends stay in one. Spilling the whole value
is what is left when no split helps, which is why this runs first.

**Around a loop, not anywhere.** LLVM splits at every interference point it
can find and prices each candidate. This makes the one cut that pays on
this corpus: a value defined before a loop, read after it, and touched
nowhere inside. Its range crosses the loop and holds a register through
every iteration; cut into "before" and "after", the loop body sees neither,
and the register is free for what the loop actually does -- which is the
whole reason `docs/hoist-blocker.md`'s programs wanted one.

The cut is a copy at each edge and a fresh value for the far side. That is
LLVM's shape too: a split is a copy, and the coalescer is what removes it
again if the two halves land in the same register anyway.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import lir
from qbopt.analysis import loops as loopy
from qbopt.analysis import intervals as ranges
from qbopt.model.passes import LIRTransform


class Splitter(LIRTransform):
    name = "split"

    def __init__(self, only: "frozenset[int] | None" = None) -> None:
        self.only = only

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return split(body, self.only)


def split(body: lir.LirBody, only: "frozenset[int] | None" = None) -> lir.LirBody:
    """`body` with each range that crosses a loop untouched cut at its edges.

    `only` narrows it to the values the allocator could not place, which is
    how LLVM drives it: `RegAllocGreedy` splits in response to a failure to
    assign, not on principle. Run on every crossing range instead, this
    cost 12,329 bytes over the corpus and freed nothing -- the copies it
    inserts are real and the registers it frees were not wanted.
    """
    found = loopy.loops(list(body.blocks), body.entry)
    if not found:
        return body
    index = ranges.indexed(body)
    live = ranges.intervals(body, index)
    fresh = _next_value(body)

    at_of = {block.at: block for block in body.blocks}
    cuts: dict[int, list[tuple[int, int]]] = {}  # block -> [(fresh, original)]
    renames: dict[int, dict[int, int]] = {}  # block -> {original: fresh}

    for loop in found:
        inside = {at for at in loop.body if at in at_of}
        touched = {value for at in inside for one in at_of[at].insns for value in (*one.defines, *one.uses)} | {
            value for at in inside for value in at_of[at].arrives
        }
        crossing = _crossing(body, live, index, inside)
        for value in sorted((crossing - touched) if only is None else (crossing - touched) & only):
            after = [at for at in _exits(body, inside) if at in at_of]
            if not after:
                continue
            for at in after:
                if value in renames.get(at, {}):
                    continue  # two loops share this exit; one cut is enough
                cuts.setdefault(at, []).append((fresh, value))
                renames.setdefault(at, {})[value] = fresh
            fresh += 1

    if not cuts:
        return body
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                insns=tuple(_cut(block, cuts.get(block.at, []), renames.get(block.at, {}))),
            )
            for block in body.blocks
        ),
    )


def _crossing(body: lir.LirBody, live: dict, index: ranges.Indexes, inside: set[int]) -> set[int]:
    """Values live all the way across the loop's slots."""
    spans = [index.span[at] for at in inside if at in index.span]
    if not spans:
        return set()
    lo, hi = min(one[0] for one in spans), max(one[1] for one in spans)
    whole = ranges.Segment(lo, hi)
    return {value for value, one in live.items() if any(seg.start <= lo and seg.end >= hi for seg in one.segments)} | {
        value
        for value, one in live.items()
        if any(seg.overlaps(whole) and seg.start < lo and seg.end > hi for seg in one.segments)
    }


def _exits(body: lir.LirBody, inside: set[int]) -> list[int]:
    """Every block the loop leaves to."""
    out = []
    for block in body.blocks:
        if block.at not in inside:
            continue
        for where in block.succ:
            if where not in inside and where not in out:
                out.append(where)
    return out


def _cut(block: lir.LirBlock, added: list, rename: dict) -> list:
    """The copies at the top of the block, and everything after them renamed."""
    if not added:
        return list(block.insns)
    first = block.insns[0] if block.insns else None
    if first is None:
        return []
    at = first.covers[0] if first.covers else first.at
    copies = [
        lir.Insn(
            at=first.at,
            covers=(at, at),
            what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(fresh, 2),), (ir.Held(value, 2),)),
            defines=(fresh,),
            uses=(value,),
            op=first.op,
        )
        for fresh, value in added
    ]
    return copies + [_renamed(one, rename) for one in block.insns]


def _renamed(one: lir.Insn, rename: dict[int, int]) -> lir.Insn:
    if not rename:
        return one
    what = one.what
    if what is None:
        return replace(
            one,
            defines=tuple(rename.get(v, v) for v in one.defines),
            uses=tuple(rename.get(v, v) for v in one.uses),
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
    )


def _settled(where, rename: dict[int, int]):
    """One operand with every value it names put through the rename.

    Through `ir.mapped`, so a cell's base is renamed with the rest: this
    looked in `Mem.through`, which holds a register, and a cut past a load
    through a pointer renamed `uses` and left the cell on the value the
    cut had just ended.
    """
    return ir.mapped(where, lambda one: ir.Held(rename.get(one.value, one.value), one.width))


def _next_value(body: lir.LirBody) -> int:
    seen = {0}
    for block in body.blocks:
        seen.update(block.arrives)
        for one in block.insns:
            seen.update(one.defines)
            seen.update(one.uses)
    return max(seen) + 1
