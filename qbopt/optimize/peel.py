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


class Peel(MIRTransform):
    name = "peel"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: mir.MirBody) -> mir.MirBody:
        found = _candidate(body, self.where)
        return body if found is None else found[0]


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
        if count * emitted > 512:
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
        if not unroll._profitable(body, result, latch, count, where):
            rejected.add(latch)
            continue
        if watch is not None:
            watch("peel-candidate", candidate)
            watch("peel-accepted", result)
        body = result
        rejected.clear()
