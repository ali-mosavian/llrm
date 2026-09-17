"""Block order, and the jumps it makes redundant.

Runs on the allocated body, after the last phase that adds or empties blocks:
edge splits and their undoing leave jumps to the next block, and the raise's
`if false goto` beside a `goto` leaves a `jcc` over a block that only jumps.
`placed` orders the blocks; `threaded` keeps that order and drops the jumps
and the blocks nothing reaches.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend.layout import _OPPOSITE


def placed(body: lir.LirBody) -> lir.LirBody:
    """Each block placed after the jump that reaches it, where no block is already.

    The raise lays a C loop out as it reads, test first, so even entered at
    its body the latch jumped back to the test every pass. Every fall-through
    is written as a jump first, which makes any order correct; `threaded` then
    drops the jumps the order made redundant and turns the test into one
    branch back to the body.
    """
    from qbopt.backend import masm
    from qbopt.analysis import loops

    explicit = []
    for block in body.blocks:
        fall = masm._falls_to(block, body.name)
        if fall is not None:
            at = block.insns[-1].at if block.insns else block.at
            jump = lir.Insn(
                at=at, covers=(at, at), what=ir.Semantics(ir.Operation.JUMP, "jmp", (), (), fall), defines=(), uses=()
            )
            block = replace(block, insns=(*block.insns, jump))
        explicit.append(block)
    by_at = {block.at: block for block in explicit}
    natural = loops.loops(explicit, body.entry)
    tests = _tests(natural, body.entry, by_at)
    # loops() is innermost first.  A block in nested loops follows the nearest
    # loop's trace before an exit from it; the outer trace resumes afterwards.
    inside: dict[int, frozenset[int]] = {}
    for loop in natural:
        for at in loop.body:
            inside.setdefault(at, loop.body)
    order: list[lir.LirBlock] = []
    done: set[int] = set()
    current: int | None = body.entry
    source: int | None = None
    while len(order) < len(explicit):
        if current is None or current in done or current not in by_at:
            current, source = next(block.at for block in explicit if block.at not in done), None
        if current in tests and source not in tests[current][1] and tests[current][0] not in done:
            # Reached from outside the loop: the body goes here, and the test after the latch
            # that jumps back to it, so a pass takes one branch.
            current, source = tests[current][0], None
            continue
        block = by_at[current]
        order.append(block)
        done.add(current)
        current, source = _onward(block, done, inside.get(block.at, frozenset())), current
    return replace(body, blocks=tuple(order))


def _onward(block: lir.LirBlock, done: set[int], inside: frozenset[int] = frozenset()) -> int | None:
    """The block to place next: where the final jump goes, or else where the branch before it goes.

    The branch's target second, so that `jcc target; jmp placed` becomes one inverted branch.
    """
    real = [one.what for one in block.insns if one.what.op is not ir.Operation.NOTHING]
    if not real or real[-1].op is not ir.Operation.JUMP:
        return None
    targets = [real[-1].target]
    if len(real) > 1 and real[-2].op is ir.Operation.BRANCH:
        targets.append(real[-2].target)
    # Keep a loop chain together before following an exit.  The final jump is
    # still preferred when both edges stay in the loop, preserving the source
    # fall-through unless doing so would strand the rest of the loop.
    for target in targets:
        if target not in done and target in inside:
            return target
    for target in targets:
        if target not in done:
            return target
    return None


def _tests(natural: list, entry: int, by_at: dict) -> dict[int, tuple[int, frozenset[int]]]:
    """Each loop header that only decides whether to go round: its one successor inside, and its latches."""
    found = {}
    for loop in natural:
        block = by_at[loop.header]
        real = [one.what for one in block.insns if one.what.op is not ir.Operation.NOTHING]
        inner = [at for at in block.succ if at in loop.body and at != loop.header]
        if (
            loop.header != entry
            and len(real) >= 2
            and real[-2].op is ir.Operation.BRANCH
            and len(set(block.succ)) == 2
            and len(inner) == 1
        ):
            found[loop.header] = (inner[0], loop.latches)
    return found


def threaded(body: lir.LirBody) -> lir.LirBody:
    body, changed = _reachable(body, list(body.blocks)), True
    while changed:
        body, changed = _step(body)
    return body


def merged(body: lir.LirBody) -> lir.LirBody:
    """Merge physically identical allocated tails after fallthroughs are explicit."""
    from qbopt.backend import masm

    # Removing a duplicate block is sound only when every incoming edge is an
    # instruction that can be retargeted. ``placed`` establishes exactly that
    # form. Decline a body presented at an earlier pipeline boundary.
    if any(masm._falls_to(block, body.name) is not None for block in body.blocks):
        return body
    while True:
        groups: dict[tuple, list[lir.LirBlock]] = {}
        for block in body.blocks:
            key = _tail_key(block)
            if key is not None:
                groups.setdefault(key, []).append(block)
        redirect = {}
        for copies in groups.values():
            if len(copies) < 2:
                continue
            canonical = next((block for block in copies if block.at == body.entry), copies[0])
            redirect.update({block.at: canonical.at for block in copies if block is not canonical})
        if not redirect:
            return body
        body = _redirected(body, redirect)


def _tail_key(block: lir.LirBlock) -> tuple | None:
    """The complete physical form of a mergeable block."""
    if block.phis:
        return None
    shaped = []
    for one in _real(block):
        what = one.what
        if (
            what is None
            or what.op in {ir.Operation.BARRIER, ir.Operation.CALL, ir.Operation.DATA}
            or one.symbol is True
            or one.group is not None
        ):
            return None
        shaped.append(
            (
                what,
                one.clobbers,
                one.clobbers_high,
                tuple(register for _value, register in one.requires),
                tuple(register for _value, register in one.delivers),
                one.frame_adjust,
            )
        )
    return (tuple(shaped), tuple(sorted(set(block.succ)))) if shaped else None


def _redirected(body: lir.LirBody, redirect: dict[int, int]) -> lir.LirBody:
    """Redirect every explicit edge, remove duplicate blocks, and fold diamonds."""

    def target(at: int) -> int:
        seen = set()
        while at in redirect and at not in seen:
            seen.add(at)
            at = redirect[at]
        return at

    blocks = []
    for block in body.blocks:
        if block.at in redirect:
            continue
        insns = []
        for one in block.insns:
            what = one.what
            if what is not None and what.op in {ir.Operation.BRANCH, ir.Operation.JUMP} and what.target is not None:
                one = replace(one, what=replace(what, target=target(what.target)))
            insns.append(one)
        successors = tuple(dict.fromkeys(target(at) for at in block.succ))
        blocks.append(_fold_converged(replace(block, insns=tuple(insns), succ=successors)))
    entry = target(body.entry)
    return replace(body, entry=entry, blocks=tuple(blocks))


def _fold_converged(block: lir.LirBlock) -> lir.LirBlock:
    """A conditional whose two CFG edges became one is an unconditional edge."""
    if len(block.succ) != 1:
        return block
    destination = block.succ[0]
    real = _real(block)
    last = real[-1] if real else None
    if (
        last is not None
        and last.what is not None
        and last.what.op is ir.Operation.JUMP
        and last.what.target == destination
        and len(real) > 1
    ):
        branch = real[-2]
        if branch.what is not None and branch.what.op is ir.Operation.BRANCH and branch.what.target == destination:
            return replace(
                block,
                insns=tuple(lir.anchor(one) if one is branch else one for one in block.insns),
            )
    if last is not None and last.what is not None and last.what.op is ir.Operation.BRANCH:
        jump = replace(last, what=ir.Semantics(ir.Operation.JUMP, "jmp", target=destination), uses=())
        return replace(block, insns=tuple(jump if one is last else one for one in block.insns))
    return block


def _work(body: lir.LirBody) -> tuple[int, int]:
    """Static and profile-free dynamic machine-instruction counts."""
    from qbopt.analysis import intervals

    depth = intervals.depths(body)
    counts = {block.at: len(_real(block)) for block in body.blocks}
    return sum(counts.values()), sum(count * 10 ** depth[at] for at, count in counts.items())


def preferred(before: lir.LirBody, after: lir.LirBody) -> lir.LirBody:
    """Take tail sharing only when size falls without adding executed work."""
    before_static, before_dynamic = _work(before)
    after_static, after_dynamic = _work(after)
    return after if after_static < before_static and after_dynamic <= before_dynamic else before


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
