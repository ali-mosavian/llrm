"""
The segment registers, and the one thing worth doing about them.

`mir.PHYSICAL` keeps them out of the values: a segment register is where an
access goes through, not something the program computes. That is right for
ds and ss, which BC sets once, and arguable for es, which holds a $DYNAMIC
array's base and is reloaded before every access to one.

The obvious next move is to make es a value and let regalloc.colour() have
it. **A real program says that is not the win.** Of qb-qrender's 621
`mov es,<x>`, every block loads es from a single source: there is no
alternation between two far pointers anywhere in it, which is the only thing
a second segment register would buy. What there is, is reloading -- 42 sites
where es already holds what the instruction loads into it, worth about 170
bytes in 26,290 instructions. A naive textual match says 283, which is what
measuring the easy way costs.

So this finds the reload and nothing else. Making es an SSA value would
change what every op in the corpus uses and defines, what regalloc has to
colour, and what select.py has to emit, in exchange for an optimisation
nothing has been found to need.

Block-scoped, like everything else here that reasons about a location rather
than a name.
"""

from qbopt import mir
from qbopt import runtime
from qbopt.mir import Cell
from qbopt.mir import Kind
from qbopt.mir import Op
from qbopt.mir import Opaque
from qbopt.mir import MemRef
from qbopt.mir import MirBody

# The segment registers a program loads. cs and ss are not among them: BC
# sets ss once at entry and cs is the code it is running.
LOADABLE = frozenset({"es", "ds"})


def _loads_a_segment(op: Op) -> tuple[str, MemRef] | None:
    """`es := [x]` as (which resource, which cell), or None.

    Only the memory form. A load from a register holds a value this does
    not track, so two of them are not comparable and neither is redundant
    as far as anything here can say.

    MIR only: the descriptor lands in a machine resource MIR has no value
    for, and `Opaque.name` is what that resource is called. This read the
    register number out of the instruction and mapped it to a name, which
    is a pass importing iced.
    """
    if op.kind is not Kind.LOAD or len(op.results) != 1 or len(op.args) != 1:
        return None
    into = op.results[0]
    if not isinstance(into, Opaque) or into.name not in LOADABLE:
        return None
    if not isinstance(op.args[0], Cell) or len(op.loads) != 1 or op.loads[0].addr is None:
        return None
    return into.name, op.loads[0]


def _clean(op: Op, calls: dict[int, str]) -> bool:
    """Whether this call provably leaves caller memory alone.

    The same question avail.py asks, and the same answer: runtime.py has
    read these routines, and one that touches no caller memory cannot have
    changed the descriptor a segment was loaded from. It has still clobbered
    the general registers, which is why the base register is checked
    separately.
    """
    routine = runtime.contract(calls.get(op.at))
    return routine.established and not runtime.barrier(routine) and not runtime.writes_caller_memory(routine)


def redundant(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> tuple[int, ...]:
    """Every `mov <seg>,[x]` that loads what that register already holds.

    Three things have to hold between the two loads, and each is a way the
    answer could have changed without this seeing it: nothing may store to a
    cell that could alias the descriptor, nothing may write the register the
    address is reached through, and no call may run that is not one
    runtime.py has proved leaves caller memory alone.

    A barrier ends it outright. So does a call that is not clean -- and a
    clean one still clobbers ax, cx, dx and bx, which the base-register
    check catches on its own.
    """
    found: list[int] = []
    for block in body.blocks:
        held: dict[str, MemRef] = {}
        for op in block.ops:
            if op.barrier:
                held = {}
                continue
            if op.at in calls and not _clean(op, calls):
                held = {}
                continue

            got = _loads_a_segment(op)
            if got is not None:
                named, ref = got
                was = held.get(named)
                if was is not None and mir.same_bytes(was, ref):
                    found.append(op.at)
                else:
                    held[named] = ref
                continue

            # A store that may alias any descriptor, or a write to a base
            # register one is reached through, and the entry stops meaning
            # what it meant.
            for ref in op.stores:
                held = {one: cell for one, cell in held.items() if not mir.overlapping(cell, ref, dgroup)}
            written = set(op.defines)
            if written:
                held = {
                    one: cell for one, cell in held.items() if cell.base is None or cell.base not in written
                }
    return tuple(found)
