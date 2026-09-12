"""Registers a caller relies on a call leaving alone.

A clobber is a may-clobber. The QB45 table lists SI or DI for some routines
whose documented convention preserves them, because a header elsewhere says
otherwise (runtime.py's B$FOUTBX note). BC's own code reads SI after such
calls without reloading it -- BENCHMARK's `mov si,[bp+6]` survives ten runtime
calls to a `push word [si]`. Read as "the callee defines SI", the value before
the call is dead, its load is deleted, and the read sees whatever the rebuild
left in SI.

So where a caller reads, after a call, a convention-preserved register the
contract lists as clobbered, the value it held before the call is an input:
whatever the callee does with it, it starts from the same state BC's code did.
"""

from dataclasses import replace

from qbopt.abi import runtime
from qbopt.frontend.blocks import Block

# The registers cmacros' convention keeps and the raise tracks as values.
_CANDIDATES = frozenset({runtime.Reg.SI, runtime.Reg.DI}) - runtime.PER_CONVENTION


def carried(
    blocks: list[Block],
    nodes: dict,
    calls: dict[int, str],
    contracts: dict[int, runtime.Contract],
) -> dict[int, runtime.Contract]:
    """Each call site whose contract needs a carried register as an input."""
    from qbopt.model import mir

    chosen = dict(contracts)
    changed: dict[int, runtime.Contract] = {}
    by_at = {block.at: block for block in blocks}
    while True:
        live_in: dict[int, frozenset] = {block.at: frozenset() for block in blocks}
        after: dict[int, frozenset] = {}
        moving = True
        while moving:
            moving = False
            for block in reversed(blocks):
                live = set().union(*(live_in[one] for one in block.succ if one in by_at))
                for insn in reversed(block.insns):
                    node = nodes.get(insn.at)
                    if node is None:
                        continue
                    after[insn.at] = frozenset(live)
                    defines, uses = mir._touched(node, calls, chosen)
                    live = (live - defines) | uses
                if live != live_in[block.at]:
                    live_in[block.at] = frozenset(live)
                    moving = True
        grown = False
        for at, routine in chosen.items():
            if at not in after or not runtime.established_inputs(routine):
                continue
            extra = frozenset(
                one
                for one in (runtime.disturbs(routine) - routine.inputs) & _CANDIDATES
                if mir.FROM_CONTRACT[one] in after[at]
            )
            if extra:
                chosen[at] = changed[at] = replace(
                    routine,
                    inputs=routine.inputs | extra,
                    evidence=f"{routine.evidence} Carried: the caller reads "
                    f"{', '.join(sorted(extra))} after this call without redefining it.",
                )
                grown = True
        if not grown:
            return changed
