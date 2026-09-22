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
from qbopt.optimize import counting


def entered(body: mir.MirBody) -> mir.MirBody:
    """Every proven loop entered at its body, each test merged into its latch.

    After the passes, not among them: a rotated loop is no longer the
    pretested shape the counted-loop analyses read, and peeling and
    unrolling it in a later round found no loop to work on.
    """
    from qbopt.optimize import cfg
    from qbopt.optimize import canonical

    return cfg.merged(canonical.identities(rotated(_counted_down(body))))


def _counted_down(body: mir.MirBody) -> mir.MirBody:
    """Rotate a dead counted counter into a guarded countdown.

    A dynamic bound cannot prove that the loop is entered, so the ordinary
    rotation below correctly leaves its initial test in place.  If the
    induction value itself is otherwise dead, its only useful meaning is the
    number of trips remaining:

        i = start; while (i < n) { body; ++i; }

    becomes a zero-trip guard followed by ``--trips`` and a branch on that
    operation's own flags.  This is an induction-variable formula choice,
    not a peephole: the guard is what makes a zero count exact, and refusing
    an observed counter is what makes replacing its values sound.

    The first implementation deliberately takes the canonical one-body-block
    form produced by loop simplification.  More involved loops remain on the
    original representation rather than acquiring a partially repaired CFG.
    """
    blocks = {block.at: block for block in body.blocks}
    facts = consts.known(body)
    made = {value.id: op for block in body.blocks for op in block.ops for value in op.defines}
    all_values = tuple(ssa.values(body))

    for loop in loops.loops(body.blocks, body.entry):
        proofs = induction.counted(body, loop, facts, inbounds=True)
        if len(proofs) != 1:
            continue
        proof = proofs[0]
        replacement = induction.control_replacement(body, loop, proof)
        if replacement is None:
            continue
        preheader, latch_at = proof.preheader, proof.latch
        header, latch = blocks[loop.header], blocks[latch_at]
        counter, phi = proof.counter, proof.phi
        compare, branch = proof.compare, proof.branch
        width = counter.start.width
        update, stepping = replacement.update, replacement.stepping
        at = blocks[preheader].ops[-1].at if blocks[preheader].ops else preheader
        seeds = counting.Seeds(
            max((value.id for value in all_values), default=0) + 1,
            max((value.variable for value in all_values), default=0) + 1,
            at,
            width,
            [],
        )
        count = induction.trips(proof, seeds.computed)
        # A constant count is handled more profitably by the ordinary
        # finite-domain induction transforms.  This rewrite exists for a
        # symbolic value which may be zero at run time.
        if not isinstance(count, mir.Held):
            continue
        exits = counting.leaving(replacement, seeds)

        step_flags = mir.Value(seeds.serial, stepping.at, flags=True, variable=seeds.variable, version=1)
        guard_flags = mir.Value(seeds.serial + 1, preheader, flags=True, variable=seeds.variable + 1, version=1)
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
        guard_compare, guard_branch = counting.skip_guard(proof, at, guard_flags)
        entry_ops = list(blocks[preheader].ops)
        if entry_ops and entry_ops[-1].kind is mir.Kind.JUMP:
            entry_ops[-1] = mir.cleared(entry_ops[-1])
        elif entry_ops and entry_ops[-1].kind is mir.Kind.BRANCH:
            continue
        entry_ops += [*seeds.ops, guard_compare, guard_branch]

        start = phi.incoming[preheader]
        start_definition = made.get(start.id)
        start_is_private = (
            start_definition is not None
            and not any(start in op.uses for block in body.blocks for op in block.ops)
            and not any(start in op.uses for op in (*seeds.ops, guard_compare))
            and not exits
            and not any(
                start in other.incoming.values() for block in body.blocks for other in block.phis if other is not phi
            )
        )
        if start_is_private:
            entry_ops = [mir.cleared(op) if op is start_definition else op for op in entry_ops]
        rewritten = []
        for block in body.blocks:
            ops = []
            for op in block.ops:
                if op is stepping:
                    # Canonicalize the control update after every data
                    # recurrence.  The rotated branch consumes its flags;
                    # leaving a strength-reduced address update after it
                    # would silently make those flags describe the address.
                    continue
                elif op is compare:
                    op = mir.cleared(op)
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
                    op = mir.cleared(op)
                ops.append(op)
            if block.at == latch_at:
                cut = len(ops) - bool(ops and ops[-1].kind is mir.Kind.JUMP)
                ops.insert(cut, decrement)
            rewritten.append(
                replace(
                    block,
                    ops=tuple(ops),
                    phis=tuple(
                        replace(other, incoming={preheader: count.value, latch_at: update}) if other is phi else other
                        for other in block.phis
                    )
                    if block.at == header.at
                    else tuple(exits.get(id(other), other) for other in block.phis),
                )
            )
        changed = replace(body, blocks=tuple(rewritten))
        changed_header = changed.block(header.at)
        changed_latch = changed.block(latch.at)
        return _counted_down(
            at_body(
                changed,
                loop,
                preheader,
                changed_header,
                changed_latch,
                entry_ops,
                entry_succ=(changed_latch.at, proof.exit),
            )
        )
    return body


def rotated(body: mir.MirBody) -> mir.MirBody:
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
        if not all(induction.test_only(op) for op in header.ops[:-1]) or not induction.nonempty(body, loop):
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
        body = _step_test(body, loop, header)
        header = body.block(header.at)
        first = body.block(first.at)
        return rotated(at_body(body, loop, preheader, header, first, ops))
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
        proof = induction.controlling(body, loop, counter, facts)
        if proof is None or proof.posttested or proof.bound != mir.Const(0, counter.start.width):
            continue
        compare = proof.compare
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
                    op = mir.cleared(op)
                elif op is branch:
                    op = replace(op, uses=(step_flags,), source_backed=False, raised=None)
                ops.append(op)
            rewritten.append(replace(block, ops=tuple(ops)))
        return replace(body, blocks=tuple(rewritten))
    return body


def at_body(
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
    # ``ops`` may contain values the caller has just constructed for the new
    # preheader and which are not in ``body`` yet.  Allocate moved phis after
    # both sets.  Looking only at the old body reused a guard flag's id for an
    # accumulator phi; dead-code elimination then erased the accumulator's
    # zero seed and sum read an uninitialized register.
    existing = (
        *ssa.values(body),
        *(value for op in ops for value in (*op.defines, *op.uses, *op.exits)),
    )
    serial = max(value.id for value in existing) + 1
    moved = {
        phi.result.id: replace(phi.result, id=serial + index, at=first.at) for index, phi in enumerate(header.phis)
    }

    def latest(value: mir.Value) -> mir.Value:
        # A latch value that is itself a header phi is that phi one pass on.
        return moved.get(value.id, value)

    ending = {phi.result.id: latest(phi.incoming[latch]) for phi in header.phis}
    initial = {phi.result.id: phi.incoming[preheader] for phi in header.phis}
    inside = {at for at in loop.body if at != header.at}
    entry = tuple(
        mir.Phi(moved[phi.result.id], {preheader: phi.incoming[preheader], header.at: ending[phi.result.id]})
        for phi in header.phis
    )

    def rewired(block: mir.MirBlock) -> mir.MirBlock:
        swap = moved if block.at in inside else ending
        phis = []
        for phi in block.phis:
            incoming = {
                at: (moved if at in inside else ending).get(value.id, value) for at, value in phi.incoming.items()
            }
            # A guarded countdown adds a direct zero-trip edge from the
            # preheader to the old exit.  Values which used to arrive there
            # from the header must then be their pre-loop versions, not the
            # latch versions used by the nonzero path.
            if entry_succ is not None and block.at in entry_succ and block.at != first.at:
                zero = phi.incoming.get(header.at)
                if zero is not None and preheader not in phi.incoming:
                    incoming[preheader] = initial.get(zero.id, zero)
            phis.append(replace(phi, incoming=incoming))
        changed = replace(block, phis=tuple(phis), ops=tuple(_swapped(op, swap) for op in block.ops))
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
    integer_ranges = dict(body.integer_ranges)
    for phi in header.phis:
        interval = integer_ranges.pop(phi.result, None)
        if interval is not None:
            integer_ranges[moved[phi.result.id]] = interval
    return replace(
        body,
        blocks=tuple(rewired(block) for block in body.blocks),
        integer_ranges=integer_ranges,
        loop_trip_counts=tuple(sorted(counts.items())),
    )


def _swapped(op: mir.Op, swap: dict[int, mir.Value]) -> mir.Op:
    """One substitution step, never chained: a replacement is not itself replaced."""
    if not swap or not any(value.id in swap for value in op.uses):
        return op
    return ssa.substituted(op, {key: value for key, value in swap.items() if value.id not in swap or value.id == key})
