"""Exact CFG loop peeling, accepted only after ordinary MIR simplifies it.

Full straight-line unrolling deliberately rejects branches and nested loops.
Peeling is its general CFG counterpart: clone every block of a proven exact
loop, retain the residual loop as a correctness fallback, and let the normal
fixed point prove that the residual is unreachable.  This exposes dependent
bounds such as a triangular ``j = i + 1`` loop without teaching any scalar
pass about that source shape.
"""

from collections.abc import Callable

from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.optimize import lcssa
from qbopt.analysis import consts
from qbopt.optimize import unroll
from qbopt.analysis import induction
from qbopt.model.passes import Where
from qbopt.optimize import loopclone
from qbopt.model.passes import MIRTransform


# Peeling exists specifically for branchy and nested exact loops whose cloned
# control flow collapses after constants reach it.  Bound the transient MIR,
# but leave enough room to evaluate a small fixed outer loop after an eight-way
# inner specialization.  The much smaller straight-line unroller keeps its own
# tighter bound; every candidate here still has to pass the target-priced
# profitability transaction after the ordinary fixed point simplifies it.
MAX_SPECULATIVE_OPERATIONS = 4096
# Conditional floating cloning has a second ceiling.  Every copy may expose a
# different scalar path, and each resulting floating region crosses the full
# strict-FP fixed point before profitability can reject it.  Keep that bounded
# independently of ordinary CFG cloning: it is an analysis resource limit, not
# a claim that larger source loops are semantically illegal.
MAX_CONDITIONAL_FLOAT_OPERATIONS = 512


class Peel(MIRTransform):
    name = "peel"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: mir.MirBody) -> mir.MirBody:
        found = _candidate(body, self.where)
        return body if found is None else found[0]


def _conditional_floating(loop, blocks: dict[int, mir.MirBlock]) -> bool:
    """Whether a loop can multiply strict floating CFG regions when cloned."""
    inside = (blocks[at] for at in loop.body if at in blocks)
    return any(
        block.at != loop.header
        and len(block.succ) > 1
        and any(op.floating is not None for op in block.ops)
        for block in inside
    )


def _candidate(
    body: mir.MirBody,
    where: Where,
    *,
    skip: frozenset[int] = frozenset(),
) -> tuple[mir.MirBody, int, int] | None:
    """Clone the first bounded exact loop, returning body, latch and count."""
    closed = lcssa.closed(body)
    facts = consts.known(closed, where.dgroup, where.named)
    for loop in loops.loops(closed.blocks, closed.entry):
        if len(loop.latches) != 1:
            continue
        latch = next(iter(loop.latches))
        if latch in skip:
            continue
        count = induction.trip_count(closed, loop, facts)
        if count is None or count < 2:
            continue
        # A resource ceiling, not a profitability claim.  The optimized
        # candidate is accepted below using the selected CPU's semantic
        # costs; this only bounds the quadratic scalar analyses on cloned CFG.
        emitted = sum(
            op.kind is not mir.Kind.NOTHING for block in closed.blocks if block.at in loop.body for op in block.ops
        )
        if count * emitted > MAX_SPECULATIVE_OPERATIONS:
            continue
        if (
            _conditional_floating(loop, {block.at: block for block in closed.blocks})
            and count * emitted > MAX_CONDITIONAL_FLOAT_OPERATIONS
        ):
            continue
        candidate = loopclone.peeled(closed, loop, count)
        if candidate is not None:
            return candidate, latch, count
    return None


def optimized(
    body: mir.MirBody,
    where: Where,
    *,
    optimize: Callable[[mir.MirBody], mir.MirBody],
    watch: Callable[[str, mir.MirBody], None] | None = None,
) -> mir.MirBody:
    """Peel exact loops transactionally and retain only target-priced wins."""
    rejected: set[int] = set()
    while True:
        found = _candidate(body, where, skip=frozenset(rejected))
        if found is None:
            return body
        candidate, latch, count = found
        result = optimize(candidate)
        rejection = unroll._rejection(body, result, latch, count, where)
        if rejection is not None:
            if watch is not None:
                watch(f"peel-rejected-{rejection}", result)
            rejected.add(latch)
            continue
        if watch is not None:
            watch("peel-candidate", candidate)
            watch("peel-accepted", result)
        body = result
        rejected.clear()
