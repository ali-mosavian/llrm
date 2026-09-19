from dataclasses import replace

from qbopt.model import mir


def inferred(
    bodies: dict[int, mir.MirBody], local_calls: dict[int, int], terminal_calls: frozenset[int]
) -> frozenset[int]:
    """Bodies whose CFG cannot reach a normal return.

    The result is the greatest fixed point over local direct calls.  Starting
    at every local body permits a closed recursive SCC to prove terminal;
    every member with a real return, malformed fallthrough, or path through a
    nonterminal call is removed, and that removal propagates to its callers.
    ``terminal_calls`` remains the independently established runtime fact.
    """
    proven = frozenset(bodies)
    while True:
        terminals = terminal_calls | frozenset(at for at, target in local_calls.items() if target in proven)
        found = frozenset(entry for entry, body in bodies.items() if _cannot_return(body, terminals))
        if found == proven:
            return proven
        proven = found


def _cannot_return(body: mir.MirBody, terminal_calls: frozenset[int]) -> bool:
    blocks = {block.at: block for block in body.blocks}
    pending = [body.entry]
    visited: set[int] = set()
    while pending:
        at = pending.pop()
        if at in visited:
            continue
        visited.add(at)
        block = blocks.get(at)
        if block is None:
            return False
        for op in block.ops:
            if op.kind is mir.Kind.RETURN:
                return False
            if op.kind is mir.Kind.CALL and op.at in terminal_calls:
                break
        else:
            if not block.succ:
                return False
            pending.extend(block.succ)
    return True


def after_terminal_calls(body: mir.MirBody, terminal_calls: frozenset[int]) -> mir.MirBody:
    """Remove MIR work whose execution requires a proven terminal call.

    The call remains, in program order, because it is the observable terminal
    action.  Everything after its first occurrence in that block and the
    block's outgoing CFG edges are unreachable.  Other predecessors may still
    reach the former successors, so this local transform deliberately leaves
    their blocks in place for ordinary CFG cleanup.  Already-truncated blocks
    are returned unchanged, making it safe to use at the no-return fixed
    point boundary.
    """
    from qbopt.optimize import transform

    blocks = []
    changed = False
    for block in body.blocks:
        cut = next(
            (index for index, op in enumerate(block.ops) if op.kind is mir.Kind.CALL and op.at in terminal_calls), None
        )
        if cut is None:
            blocks.append(block)
            continue
        inert_tail = transform._without(list(block.ops[cut + 1 :]), lambda _op: True)
        ops = block.ops[: cut + 1] + tuple(inert_tail)
        if ops == block.ops and not block.succ:
            blocks.append(block)
            continue
        blocks.append(replace(block, ops=ops, succ=()))
        changed = True
    if not changed:
        return body

    # This runs after the object path's ordinary fixed point.  A block which
    # was reachable only through the just-removed edge must therefore be
    # normalized here, rather than left as executable work for lowering.  The
    # shared CFG normalizer retains its source-byte owner as inert MIR, which
    # is the required object-emission provenance contract.
    return transform._unreachable(replace(body, blocks=tuple(blocks)))
