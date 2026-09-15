"""Jumps the block order makes redundant.

Runs on the allocated body, after the last phase that adds or empties blocks:
edge splits and their undoing leave jumps to the next block, and the raise's
`if false goto` beside a `goto` leaves a `jcc` over a block that only jumps.
Block order is kept; only jumps and the blocks nothing reaches go.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend.layout import _OPPOSITE


def threaded(body: lir.LirBody) -> lir.LirBody:
    body, changed = _reachable(body, list(body.blocks)), True
    while changed:
        body, changed = _step(body)
    return body


def _step(body: lir.LirBody) -> tuple[lir.LirBody, bool]:
    blocks = list(body.blocks)
    at = {block.at: index for index, block in enumerate(blocks)}
    for index, block in enumerate(blocks):
        real = _real(block)
        after = blocks[index + 1].at if index + 1 < len(blocks) else None
        last = real[-1] if real else None
        if last is None or last.what.op not in (ir.Operation.BRANCH, ir.Operation.JUMP):
            continue
        target = _through(blocks, at, last.what.target)
        if target != last.what.target:
            blocks[index] = _retargeted(block, last, target)
            return _reachable(body, blocks), True
        # Dropped outright, not through lir.without: that keeps an instruction whose bytes it
        # cannot hand on, and the printed path has no bytes to account for.
        if last.what.op is ir.Operation.JUMP and target == after:
            blocks[index] = replace(block, insns=tuple(one for one in block.insns if one is not last))
            return _reachable(body, blocks), True
        if last.what.op is ir.Operation.JUMP and len(real) > 1 and real[-2].what.op is ir.Operation.BRANCH:
            branch = real[-2]
            if branch.what.target == after and branch.what.name in _OPPOSITE:
                inverted = replace(branch, what=replace(branch.what, name=_OPPOSITE[branch.what.name], target=target))
                insns = [inverted if one is branch else one for one in block.insns if one is not last]
                blocks[index] = replace(block, insns=tuple(insns))
                return _reachable(body, blocks), True
        if last.what.op is ir.Operation.BRANCH and after is not None and last.what.name in _OPPOSITE:
            # Falling into a block that only jumps, with the branch taken to the block past it.
            over = blocks[index + 1]
            beyond = blocks[index + 2].at if index + 2 < len(blocks) else None
            onward = _passage(over)
            if (
                onward is not None
                and _real(over)
                and last.what.target == beyond
                and _predecessors(blocks).get(over.at) == {block.at}
            ):
                inverted = replace(last, what=replace(last.what, name=_OPPOSITE[last.what.name], target=onward))
                blocks[index] = replace(
                    block,
                    insns=tuple(inverted if one is last else one for one in block.insns),
                    succ=(onward, beyond),
                )
                blocks[index + 1] = replace(over, succ=())
                return _reachable(body, blocks), True
    return body, False


def _real(block: lir.LirBlock) -> list[lir.Insn]:
    """The instructions that print."""
    return [
        one
        for one in block.insns
        if one.what is None or one.what.op is not ir.Operation.NOTHING or (one.what.name or "") not in ("", "nop")
    ]


def _passage(block: lir.LirBlock) -> int | None:
    """Where a block that does nothing but go somewhere goes."""
    if block.phis:
        return None
    real = _real(block)
    if not real and len(block.succ) == 1:
        return block.succ[0]
    if len(real) == 1 and real[0].what is not None and real[0].what.op is ir.Operation.JUMP:
        return real[0].what.target
    return None


def _through(blocks: list, at: dict, target: int) -> int:
    """The first block past every block that only passes control on; unchanged on a cycle of them."""
    start, seen = target, set()
    while target in at and (onward := _passage(blocks[at[target]])) is not None:
        if target in seen:
            return start
        seen.add(target)
        target = onward
    return target


def _retargeted(block: lir.LirBlock, last: lir.Insn, target: int) -> lir.LirBlock:
    old = last.what.target
    moved = replace(last, what=replace(last.what, target=target))
    succ = tuple(dict.fromkeys(target if one == old else one for one in block.succ))
    return replace(block, insns=tuple(moved if one is last else one for one in block.insns), succ=succ)


def _predecessors(blocks: list) -> dict[int, set[int]]:
    found: dict[int, set[int]] = {}
    for block in blocks:
        for one in block.succ:
            found.setdefault(one, set()).add(block.at)
    return found


def _reachable(body: lir.LirBody, blocks: list) -> lir.LirBody:
    by_at = {block.at: block for block in blocks}
    reached, work = set(), [body.entry]
    while work:
        one = work.pop()
        if one in reached or one not in by_at:
            continue
        reached.add(one)
        work.extend(by_at[one].succ)
    return replace(body, blocks=tuple(block for block in blocks if block.at in reached))
