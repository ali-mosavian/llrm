"""Enter a loop proven to run at least once at its body, not at its test.

BC writes `FOR` as `jmp test; body: ...; test: cmp; jle body`. Where the
first test is proven to pass the entry jump goes straight to the body, and
the test is then reached only from the latch, which it can merge into.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.analysis import loops
from qbopt.analysis import consts
from qbopt.analysis import induction


def entered(body: mir.MirBody, *, step_tests: bool = False) -> mir.MirBody:
    """Every proven loop entered at its body, each test merged into its latch.

    After the passes, not among them: a rotated loop is no longer the
    pretested shape the counted-loop analyses read, and peeling and
    unrolling it in a later round found no loop to work on.
    """
    from qbopt.optimize import cfg

    return cfg.merged(rotated(_counted_down(body), step_tests=step_tests))


def _counted_down(body: mir.MirBody) -> mir.MirBody:
    """Rotate a dead ``0..bound-1`` counter into a guarded countdown.

    A dynamic unsigned bound cannot prove that the loop is entered, so the
    ordinary rotation below correctly leaves its initial test in place.  If
    the induction value itself is otherwise dead, its only useful meaning is
    the number of trips remaining:

        i = 0; while (i < n) { body; ++i; }

    becomes a zero-trip guard followed by ``--n`` and a branch on that
    operation's own flags.  This is an induction-variable formula choice,
    not a peephole: the guard is what makes ``n == 0`` exact, and refusing an
    observed counter is what makes replacing its values sound.

    The first implementation deliberately takes the canonical one-body-block
    form produced by loop simplification.  More involved loops remain on the
    original representation rather than acquiring a partially repaired CFG.
    """
    from qbopt.optimize import transform

    blocks = {block.at: block for block in body.blocks}
    predecessors = loops.predecessors(body.blocks)
    facts = consts.known(body)
    made = {value.id: op for block in body.blocks for op in block.ops for value in op.defines}
    all_values = tuple(ssa.values(body))

    for loop in loops.loops(body.blocks, body.entry):
        if len(loop.body) != 2 or len(loop.latches) != 1:
            continue
        preheader = transform._preheader(body, loop)
        if preheader is None or blocks[preheader].succ != (loop.header,):
            continue
        header = blocks[loop.header]
        latch_at = next(iter(loop.latches))
        latch = blocks[latch_at]
        inside = set(loop.body)
        entered_at = [at for at in header.succ if at in inside and at != header.at]
        exits = [at for at in header.succ if at not in inside]
        if (
            entered_at != [latch_at]
            or len(exits) != 1
            or latch.succ != (header.at,)
            or latch.phis
            or set(predecessors.get(latch.at, ())) != {header.at}
            or len(header.phis) != 1
            or not header.ops
            or header.ops[-1].kind is not mir.Kind.BRANCH
            or not all(_tests(op) for op in header.ops[:-1])
            or blocks[exits[0]].phis
        ):
            continue
        branch = header.ops[-1]
        counter = next(iter(induction.basics(body, loop).values()), None)
        if (
            counter is None
            or induction._signed(counter.start, facts, counter.start.width) != 0
            or induction._signed(counter.step, facts, counter.step.width) != 1
            or induction._continuing_test(branch, inside) is not mir.Kind.BELOW
        ):
            continue
        phi = header.phis[0]
        if phi.result.id != counter.value or set(phi.incoming) != {preheader, latch_at}:
            continue
        width = counter.start.width
        comparisons = [
            (op, bound)
            for op in header.ops[:-1]
            if (bound := induction._counter_bound(op, branch, counter, width, made)) is not None
        ]
        if len(comparisons) != 1:
            continue
        compare, bound = comparisons[0]
        if (
            not isinstance(bound, mir.Held)
            or bound.width != width
            or bound.value.id not in induction.invariant(body, inside)
        ):
            continue
        update = phi.incoming[latch_at]
        stepping = made.get(update.id)
        stepped = mir.stepping(stepping) if stepping is not None else None
        if (
            stepping is None
            or stepped != (mir.Held(phi.result, width), mir.Const(1, width))
            or stepping.results != (mir.Held(update, width),)
            or stepping.loads
            or stepping.stores
            or stepping.barrier
            or stepping.merges
        ):
            continue
        if any(
            (set(compare.defines) & set(op.uses) and op is not branch)
            or {value for value in stepping.defines if value.flags} & set(op.uses)
            for block in body.blocks
            for op in block.ops
        ):
            continue
        # The recurrence may be replaced only when it is control, not a
        # source-language value.  Count operation reads and phi edges alike;
        # an exit use hidden in either representation must reject the change.
        allowed = {id(compare), id(stepping)}
        if any(
            phi.result in op.uses and id(op) not in allowed or update in op.uses
            for block in body.blocks
            for op in block.ops
        ):
            continue
        if any(
            other is not phi and {phi.result, update} & set(other.incoming.values())
            for block in body.blocks
            for other in block.phis
        ):
            continue

        serial = max((value.id for value in all_values), default=0) + 1
        variable = max((value.variable for value in all_values), default=0) + 1
        step_flags = mir.Value(serial, stepping.at, flags=True, variable=variable, version=1)
        guard_flags = mir.Value(serial + 1, preheader, flags=True, variable=variable + 1, version=1)
        decrement = replace(
            stepping,
            name="",
            defines=tuple(value for value in stepping.defines if not value.flags) + (step_flags,),
            uses=(phi.result,),
            source_backed=False,
            kind=mir.Kind.DECREMENT,
            args=(mir.Held(phi.result, width),),
            raised=None,
            symbol=False,
        )
        guard_compare = replace(
            compare,
            at=blocks[preheader].ops[-1].at if blocks[preheader].ops else preheader,
            defines=(guard_flags,),
            uses=(bound.value,),
            source_backed=False,
            args=(bound, mir.Const(0, width)),
            raised=None,
            absorbed=(),
            symbol=False,
        )
        guard_branch = replace(
            branch,
            at=guard_compare.at,
            name="",
            defines=(),
            uses=(guard_flags,),
            source_backed=False,
            test=mir.Kind.EQ,
            target=exits[0],
            raised=None,
            absorbed=(),
            symbol=False,
        )
        entry_ops = list(blocks[preheader].ops)
        if entry_ops and entry_ops[-1].kind is mir.Kind.JUMP:
            entry_ops[-1] = _cleared(entry_ops[-1])
        elif entry_ops and entry_ops[-1].kind is mir.Kind.BRANCH:
            continue
        entry_ops += [guard_compare, guard_branch]

        start = phi.incoming[preheader]
        start_definition = made.get(start.id)
        start_is_private = (
            start_definition is not None
            and not any(start in op.uses for block in body.blocks for op in block.ops)
            and not any(
                start in other.incoming.values() for block in body.blocks for other in block.phis if other is not phi
            )
        )
        if start_is_private:
            entry_ops = [_cleared(op) if op is start_definition else op for op in entry_ops]
        rewritten = []
        for block in body.blocks:
            ops = []
            for op in block.ops:
                if op is stepping:
                    op = decrement
                elif op is compare:
                    op = _cleared(op)
                elif op is branch:
                    op = replace(
                        op,
                        name="",
                        uses=(step_flags,),
                        source_backed=False,
                        test=mir.Kind.NE,
                        target=latch.at,
                        raised=None,
                        symbol=False,
                    )
                elif start_is_private and op is start_definition:
                    op = _cleared(op)
                ops.append(op)
            rewritten.append(
                replace(
                    block,
                    ops=tuple(ops),
                    phis=(replace(phi, incoming={preheader: bound.value, latch_at: update}),)
                    if block.at == header.at
                    else block.phis,
                )
            )
        changed = replace(body, blocks=tuple(rewritten))
        changed_header = changed.block(header.at)
        changed_latch = changed.block(latch.at)
        return _counted_down(
            _entered(
                changed,
                loop,
                preheader,
                changed_header,
                changed_latch,
                entry_ops,
                entry_succ=(changed_latch.at, exits[0]),
            )
        )
    return body


def rotated(body: mir.MirBody, *, step_tests: bool = False) -> mir.MirBody:
    from qbopt.optimize import transform

    blocks = {block.at: block for block in body.blocks}
    predecessors = loops.predecessors(body.blocks)
    for loop in loops.loops(body.blocks, body.entry):
        if len(loop.latches) != 1:
            continue
        preheader = transform._preheader(body, loop)
        if preheader is None or blocks[preheader].succ != (loop.header,):
            continue
        header = blocks[loop.header]
        inside = [at for at in header.succ if at in loop.body]
        if len(header.succ) != 2 or len(inside) != 1 or inside[0] == header.at:
            continue
        first = blocks[inside[0]]
        if first.phis or set(predecessors.get(first.at, ())) != {header.at}:
            continue
        if not header.ops or header.ops[-1].kind is not mir.Kind.BRANCH:
            continue
        if not all(_tests(op) for op in header.ops[:-1]) or not induction.nonempty(body, loop):
            continue
        entry = blocks[preheader]
        ops = list(entry.ops)
        if ops and ops[-1].kind is mir.Kind.BRANCH:
            continue
        if ops and ops[-1].kind is mir.Kind.JUMP:
            ops[-1] = replace(ops[-1], target=first.at)
        else:
            at = ops[-1].at if ops else entry.at
            ops.append(mir.Op(at, ir.Operation.JUMP, "", (), (), kind=mir.Kind.JUMP, target=first.at, symbol=False))
        if step_tests:
            body = _step_test(body, loop, header)
        header = body.block(header.at)
        first = body.block(first.at)
        return rotated(
            _entered(body, loop, preheader, header, first, ops),
            step_tests=step_tests,
        )
    return body


def _step_test(body: mir.MirBody, loop: loops.Loop, header: mir.MirBlock) -> mir.MirBody:
    """Let a zero-ending recurrence's latch step provide the branch flags.

    ``rotated`` has proved that the preheader will bypass this test, so the
    header is reached only after the latch update.  When its sole question is
    whether that updated recurrence is zero, a second compare computes the
    flags the update already produced.  Keep this in MIR: the relationship is
    a loop fact, not a post-allocation instruction coincidence.
    """
    if len(loop.latches) != 1 or not header.ops or header.ops[-1].kind is not mir.Kind.BRANCH:
        return body
    latch_at = next(iter(loop.latches))
    latch = body.block(latch_at)
    branch = header.ops[-1]
    if branch.test not in (mir.Kind.EQ, mir.Kind.NE):
        return body
    made = {value.id: op for block in body.blocks for op in block.ops for value in op.defines}
    readers = {value: [] for block in body.blocks for op in block.ops for value in op.defines}
    for block in body.blocks:
        for op in block.ops:
            for value in op.uses:
                readers.setdefault(value, []).append(op)
    facts = consts.known(body)

    for counter in induction.basics(body, loop).values():
        phi = next((one for one in header.phis if one.result.id == counter.value), None)
        if phi is None or latch_at not in phi.incoming:
            continue
        comparisons = [
            op
            for op in header.ops[:-1]
            if induction._counter_bound(op, branch, counter, counter.start.width, made)
            == mir.Const(0, counter.start.width)
        ]
        if len(comparisons) != 1:
            continue
        compare = comparisons[0]
        flags = [value for value in compare.defines if value.flags]
        if len(flags) != 1 or readers.get(flags[0]) != [branch]:
            continue
        update = phi.incoming[latch_at]
        stepping = made.get(update.id)
        stepped = mir.stepping(stepping) if stepping is not None else None
        if (
            stepping is None
            or stepped is None
            or stepped[0] != mir.Held(phi.result, counter.start.width)
            or stepping.results != (mir.Held(update, counter.start.width),)
            or stepping.loads
            or stepping.stores
            or stepping.barrier
            or stepping.merges
        ):
            continue
        step_index = latch.ops.index(stepping)
        if any(op.kind not in (mir.Kind.NOTHING, mir.Kind.JUMP) for op in latch.ops[step_index + 1 :]):
            continue
        if any(value.flags and readers.get(value) for value in stepping.defines):
            continue
        # The modular recurrence must reach zero exactly at the proven exit;
        # nonempty() established a finite positive trip count before rotation.
        if induction.trip_count(body, loop, facts) is None:
            continue
        serial = max((value.id for value in ssa.values(body)), default=0) + 1
        variable = max((value.variable for value in ssa.values(body)), default=0) + 1
        step_flags = mir.Value(serial, stepping.at, flags=True, variable=variable, version=1)
        rewritten = []
        for block in body.blocks:
            ops = []
            for op in block.ops:
                if op is stepping:
                    op = replace(op, defines=(*op.defines, step_flags), source_backed=False, raised=None)
                elif op is compare:
                    op = _cleared(op)
                elif op is branch:
                    op = replace(op, uses=(step_flags,), source_backed=False, raised=None)
                ops.append(op)
            rewritten.append(replace(block, ops=tuple(ops)))
        return replace(body, blocks=tuple(rewritten))
    return body


def _entered(
    body: mir.MirBody,
    loop: loops.Loop,
    preheader: int,
    header: mir.MirBlock,
    first: mir.MirBlock,
    ops: list[mir.Op],
    entry_succ: tuple[int, ...] | None = None,
) -> mir.MirBody:
    """`body` entering `loop` at `first`, its phis moved there by hand.

    Not re-derived by variable: two values of one variable a pass has
    already split are not one another's reaching definitions, and renaming
    `main`'s exit copy of a call's answer to the loop counter let decide
    fold the exit away.

    A header phi `p(preheader: a, latch: b)` becomes `q(preheader: a,
    header: b)` at `first`. The loop reads `q`; the test, now reached only
    from the latch, and everything after the loop read `b`.
    """
    latch = next(iter(loop.latches))
    serial = max(value.id for value in ssa.values(body)) + 1
    moved = {
        phi.result.id: replace(phi.result, id=serial + index, at=first.at) for index, phi in enumerate(header.phis)
    }

    def latest(value: mir.Value) -> mir.Value:
        # A latch value that is itself a header phi is that phi one pass on.
        return moved.get(value.id, value)

    ending = {phi.result.id: latest(phi.incoming[latch]) for phi in header.phis}
    inside = {at for at in loop.body if at != header.at}
    entry = tuple(
        mir.Phi(moved[phi.result.id], {preheader: phi.incoming[preheader], header.at: ending[phi.result.id]})
        for phi in header.phis
    )

    def rewired(block: mir.MirBlock) -> mir.MirBlock:
        swap = moved if block.at in inside else ending
        phis = tuple(
            replace(
                phi,
                incoming={
                    at: (moved if at in inside else ending).get(value.id, value) for at, value in phi.incoming.items()
                },
            )
            for phi in block.phis
        )
        changed = replace(block, phis=phis, ops=tuple(_swapped(op, swap) for op in block.ops))
        if block.at == preheader:
            return replace(changed, ops=tuple(ops), succ=entry_succ or (first.at,))
        if block.at == header.at:
            return replace(changed, phis=())
        if block.at == first.at:
            return replace(changed, phis=entry)
        return changed

    counts = dict(body.loop_trip_counts)
    if header.at in counts:
        count = counts.pop(header.at)
        # Rotation makes ``first`` the natural-loop header.  Preserve an
        # exact fact only when it does not collide with a distinct loop fact;
        # losing a measurement is preferable to attaching the wrong count.
        if first.at not in counts or counts[first.at] == count:
            counts[first.at] = count
    return replace(
        body,
        blocks=tuple(rewired(block) for block in body.blocks),
        loop_trip_counts=tuple(sorted(counts.items())),
    )


def _cleared(op: mir.Op) -> mir.Op:
    """Retain an occurrence's ownership while deleting its computation."""
    return replace(
        op,
        kind=mir.Kind.NOTHING,
        name="",
        defines=(),
        uses=(),
        loads=(),
        stores=(),
        args=(),
        results=(),
        merges={},
        raised=None,
        target=None,
        test=None,
        stack=None,
        symbol=False,
    )


def _swapped(op: mir.Op, swap: dict[int, mir.Value]) -> mir.Op:
    """One substitution step, never chained: a replacement is not itself replaced."""
    if not swap or not any(value.id in swap for value in op.uses):
        return op
    return ssa.substituted(op, {key: value for key, value in swap.items() if value.id not in swap or value.id == key})


def _tests(op: mir.Op) -> bool:
    """A comparison or nothing: the entry skips it, so it may compute nothing else.

    A floating check computes no value and still moves the x87 stack.
    """
    from qbopt.optimize import cfg

    if cfg._empty(op):
        return True
    return (
        op.kind in (mir.Kind.SUB, mir.Kind.AND, mir.Kind.OR)
        and not (op.results or op.loads or op.stores or op.merges or op.barrier)
        and op.floating is None
        and op.stack is None
        and op.floating_origin is None
        and bool(op.defines)
        and all(value.flags for value in op.defines)
    )
