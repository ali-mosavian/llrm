"""Where each value is live, as ranges rather than as sets per block.

`allocate.live()` answers live-in and live-out per block, which is enough
to say whether two values ever overlap and not enough for anything the
allocator wants to do about it. Splitting a range needs to know where it
begins; spilling well needs to know how long it is; coalescing two values
needs to know that their ranges meet at exactly one point.

LLVM's shape, and the names are its: a **slot index** numbers every point a
value can start or stop being live, a **segment** is a half-open run of
them, and a **live interval** is the segments one value occupies.
`llvm/lib/CodeGen/LiveIntervals.cpp` is the original.

Two slots per instruction rather than LLVM's four. It uses Slot_Block,
Slot_EarlyClobber, Slot_Register and Slot_Dead to separate an operand read
before the write from one written before the read; nothing here yet needs
early-clobber, and inventing the distinction before a pass asks for it
would be four times the indices for none of the answers.
"""

from dataclasses import dataclass
from dataclasses import replace

from qbopt.model import lir
from qbopt.analysis import loops as loopy

# Points per instruction: where it reads, and where it writes. A value read
# at `use` and one written at `def` of the same instruction do not overlap,
# which is what makes `add ax,bx` not interfere ax with itself.
USE = 0
DEF = 1
PER_INSN = 2


@dataclass(frozen=True, slots=True)
class Segment:
    """A half-open run of slot indices one value occupies."""

    start: int
    end: int

    def overlaps(self, other: "Segment") -> bool:
        return self.start < other.end and other.start < self.end


@dataclass(frozen=True, slots=True)
class Interval:
    """Every segment one value occupies, and what spilling it would cost."""

    value: int
    segments: tuple[Segment, ...]
    weight: float = 0.0

    @property
    def size(self) -> int:
        """How many slots this value is live for, which is not its span.

        A value live at the top and bottom of a body and dead in between
        occupies two short segments, and spilling it frees a register for
        everything between them. Measuring the span instead would say it
        frees nothing.
        """
        return sum(one.end - one.start for one in self.segments)

    def overlaps(self, other: "Interval") -> bool:
        mine, theirs = iter(self.segments), iter(other.segments)
        one, two = next(mine, None), next(theirs, None)
        while one is not None and two is not None:
            if one.overlaps(two):
                return True
            if one.end <= two.end:
                one = next(mine, None)
            else:
                two = next(theirs, None)
        return False


@dataclass(frozen=True, slots=True)
class Indexes:
    """Every instruction's slot number, and every block's span."""

    at: dict[int, int]  # id(insn) -> the instruction's first slot
    span: dict[int, tuple[int, int]]  # block address -> [first, last)
    order: tuple[int, ...]  # block addresses, in the order they are numbered


def indexed(body: lir.LirBody) -> Indexes:
    """Number every point a value can start or stop being live.

    In block order, which is the order they are emitted in. LLVM numbers in
    layout order for the same reason: a range that looks contiguous has to
    be contiguous in the thing that runs.
    """
    at: dict[int, int] = {}
    span: dict[int, tuple[int, int]] = {}
    next_slot = 0
    for block in body.blocks:
        first = next_slot
        # A phi's result is defined before the block's first instruction, on
        # the edge rather than in the block. One slot, so it can be live
        # out of the predecessor and in here without the two touching.
        next_slot += PER_INSN
        for one in block.insns:
            at[id(one)] = next_slot
            next_slot += PER_INSN
        span[block.at] = (first, next_slot)
    return Indexes(at, span, tuple(block.at for block in body.blocks))


def intervals(body: lir.LirBody, index: Indexes | None = None) -> dict[int, Interval]:
    """The live interval of every value in this body, weighted.

    Built from block liveness and then refined inside each block, which is
    how LiveIntervalCalc does it: the set says which values cross the
    block's edges, and walking the instructions backwards says where inside
    it each one actually starts and stops.
    """
    index = index or indexed(body)
    ranges = _ranges(body, index)
    weight = _weights(body, index, ranges)
    return {value: replace(one, weight=weight.get(value, 0.0)) for value, one in ranges.items()}


def _ranges(body: lir.LirBody, index: Indexes) -> dict[int, Interval]:
    """Where each value is live, before anything prices it."""
    from qbopt.backend import allocate

    live_in, live_out = allocate.live(body)
    pieces: dict[int, list[Segment]] = {}
    for block in body.blocks:
        first, last = index.span[block.at]
        alive: dict[int, int] = {one: last for one in live_out[block.at]}
        written: set[int] = set()
        for one in reversed(block.insns):
            slot = index.at[id(one)]
            for value in one.defines:
                written.add(value)
                pieces.setdefault(value, []).append(Segment(slot + DEF, alive.pop(value, slot + DEF + 1)))
            for value in one.uses:
                alive.setdefault(value, slot + USE + 1)
        # A phi's result is defined at the top of the block, on the edge
        # rather than by any instruction in it.
        for phi in block.phis:
            written.add(phi.result)
            pieces.setdefault(phi.result, []).append(Segment(first + DEF, alive.pop(phi.result, first + DEF + 1)))
        # Whatever is still alive arrived from a predecessor and was live
        # from the block's first slot.
        for value, end in alive.items():
            if end > first:
                pieces.setdefault(value, []).append(Segment(first, end))
        # Live through: in at the top, out at the bottom, untouched between.
        for value in live_in[block.at]:
            if value not in written and value not in alive:
                pieces.setdefault(value, []).append(Segment(first, last))
    return {value: Interval(value, tuple(_merged(runs))) for value, runs in pieces.items()}


def _merged(runs: list[Segment]) -> list[Segment]:
    """Overlapping or touching segments joined, in order."""
    out: list[Segment] = []
    for one in sorted(runs, key=lambda x: (x.start, x.end)):
        if out and one.start <= out[-1].end:
            out[-1] = Segment(out[-1].start, max(out[-1].end, one.end))
            continue
        out.append(one)
    return out


def depths(body: lir.LirBody) -> dict[int, int]:
    """How deeply each block is nested in loops."""
    out = {block.at: 0 for block in body.blocks}
    for loop in loopy.loops(list(body.blocks), body.entry):
        for at in loop.body:
            if at in out:
                out[at] += 1
    return out


# What one reference costs per level of loop nesting. LLVM asks
# BlockFrequencyInfo for the block's frequency relative to the entry, which
# with no profile is this estimate: ten iterations assumed per loop.
PER_LEVEL = 10

# Added to the size before dividing, so that a very short interval's weight
# is dominated by how often it is referenced rather than by an accident of
# where the slots fell. LLVM's `25 * InstrDist`, in this module's slots.
GRACE = 25 * PER_INSN


def weights(body: lir.LirBody, index: Indexes | None = None) -> dict[int, float]:
    """What spilling each value would cost. See `_weights` for the formula."""
    index = index or indexed(body)
    return _weights(body, index, _ranges(body, index))


def _weights(body: lir.LirBody, index: Indexes, ranges: dict[int, Interval]) -> dict[int, float]:
    """`references weighted by loop depth / (live slots + grace)`.

    LLVM's normalizeSpillWeight. The division is the part a plain count
    misses: two values referenced equally often are not equally worth
    keeping if one is live for three instructions and the other for the
    whole body -- spilling the long one frees a register for longer, so it
    is the cheaper one to spill and its weight has to say so.
    """
    deep = depths(body)
    total: dict[int, float] = {}
    for block in body.blocks:
        each = float(PER_LEVEL ** deep.get(block.at, 0))
        for one in block.insns:
            for value in (*one.defines, *one.uses):
                total[value] = total.get(value, 0.0) + each
    return {value: found / (ranges[value].size + GRACE if value in ranges else GRACE) for value, found in total.items()}
