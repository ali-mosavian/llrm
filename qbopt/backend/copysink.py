"""Move a copy out of a loop when only what is after the loop reads it.

A phi whose value is computed in a loop and read after it becomes a copy on
the edge back to the header, so it runs every iteration to hand over a value
nothing inside the loop looks at. deedlines' plasmablobs ends its inner loop
with `mov di,bx` for a call after both loops -- 64000 copies for one use.

The copy belongs on the exit edge. That is sound while the destination is
read nowhere in the loop and the source is not written between the copy and
the exit, which is what this checks. LLVM reaches the same placement from the
other side: `MachineSink` moves an instruction to the successor that uses it.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import lir
from qbopt.analysis import loops as loopy


def _plain(one) -> bool:
    return (
        one.what is not None
        and not one.clobbers
        and not one.requires
        and not one.delivers
        and not one.spread
        and one.group is None
        and one.symbol is not True
        and not one.frame_adjust
        and not one.spill_reload
        and not one.spill_store
    )


def _copy(one) -> "tuple[ir.Reg, ir.Reg] | None":
    """The (destination, source) of a plain register-to-register move."""
    if not _plain(one):
        return None
    match one.what:
        case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as dest,), (ir.Reg() as source,)):
            if dest.width == source.width and dest != source:
                return dest, source
    return None


def _touches(one, lanes: set, *, reading: bool) -> bool:
    """Whether `one` reads (or writes) any of `lanes`, conservatively."""
    from qbopt.backend.liveness import _declared
    from qbopt.backend.liveness import _terminator
    from qbopt.backend.peephole import _register_effects

    if _terminator(one.what):
        return False
    effects = _register_effects(one, flags=True, may_write=True)
    if effects is None:
        effects = _declared(one)
    if effects is None:
        return True
    return bool(lanes & effects[0 if reading else 1])


def _between(at_of, inside: set, copy_at: int, exit_from: int) -> "set | None":
    """Blocks on a path from the copy's block to the exit that avoids it.

    A path that comes back to the copy's block runs the copy again, so what it
    writes on the way cannot be why the last copy before the exit is wrong.
    plasmablobs' is in the block before the branch, and its loop body writes
    the source -- only by leaving the copy's own block out of the walk does
    that stop being an objection.
    """
    forward = set()
    queue = [to for to in at_of[copy_at].succ if to in inside and to != copy_at]
    while queue:
        at = queue.pop()
        if at in forward or at == copy_at:
            continue
        forward.add(at)
        queue.extend(to for to in at_of[at].succ if to in inside and to != copy_at)
    if exit_from != copy_at and exit_from not in forward:
        return None
    backward = set()
    queue = [exit_from] if exit_from != copy_at else []
    while queue:
        at = queue.pop()
        if at in backward or at == copy_at:
            continue
        backward.add(at)
        queue.extend(one for one in inside if at in at_of[one].succ and one != copy_at and one in forward)
    return forward & backward


def sunk(body: lir.LirBody) -> lir.LirBody:
    """`body` with each such copy moved from inside its loop to the exit."""
    from qbopt.backend.peephole import _lanes
    from qbopt.backend.liveness import live_into

    found = loopy.loops(list(body.blocks), body.entry)
    if not found:
        return body
    into, _successors, _universe = live_into(body)
    at_of = {block.at: block for block in body.blocks}
    predecessors: dict[int, list[int]] = {at: [] for at in at_of}
    for block in body.blocks:
        for at in block.succ:
            if at in predecessors:
                predecessors[at].append(block.at)

    moved: dict[int, list[lir.Insn]] = {}
    removed: set[int] = set()
    for loop in found:
        inside = {at for at in loop.body if at in at_of}
        leaving = [(at, to) for at in inside for to in at_of[at].succ if to not in inside]
        # One way out, reached from one place. Any other shape needs the copy
        # on each edge, and an exit block with another predecessor needs that
        # edge split first -- neither is worth inventing for the case at hand.
        if len(leaving) != 1:
            continue
        ((source_at, exit_at),) = leaving
        if exit_at not in at_of or predecessors[exit_at] != [source_at]:
            continue
        for block in (at_of[at] for at in sorted(inside)):
            for index, one in enumerate(block.insns):
                pair = _copy(one)
                if pair is None or id(one) in removed:
                    continue
                dest, register = pair
                written, read = _lanes(dest.register), _lanes(register.register)
                if not written or not read or written & read:
                    continue
                # Dead on the way round. Asking it of the copy's own successors
                # answers the wrong question: the branch block is live-in for
                # `di` because the *exit* reads it, which is the path the sunk
                # copy is for. What matters is the way back to the header.
                if any(written & into[to] for to in at_of[source_at].succ if to in inside):
                    continue
                rest = _between(at_of, inside, block.at, source_at)
                if rest is None:
                    continue
                later = [*block.insns[index + 1 :], *(other for at in sorted(rest) for other in at_of[at].insns)]
                if any(
                    _touches(other, written | read, reading=False) or _touches(other, written, reading=True)
                    for other in later
                ):
                    continue
                removed.add(id(one))
                moved.setdefault(exit_at, []).append(one)

    if not removed:
        return body
    blocks = []
    for block in body.blocks:
        insns = block.insns
        if any(id(one) in removed for one in insns):
            insns = tuple(lir.without(insns, lambda one: id(one) in removed))
        if block.at in moved:
            insns = (*moved[block.at], *insns)
        blocks.append(replace(block, insns=insns) if insns is not block.insns else block)
    return replace(body, blocks=tuple(blocks))
