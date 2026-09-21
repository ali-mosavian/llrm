"""Use an existing recurrence to control a loop instead of a redundant counter."""

from math import gcd
from dataclasses import replace

from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.analysis import loops
from qbopt.analysis import consts
from qbopt.analysis import liveness
from qbopt.analysis import induction
from qbopt.model.passes import OperationCosts


def rewound(
    body: mir.MirBody,
    registers: int = 0,
    costs: OperationCosts | None = None,
) -> mir.MirBody:
    """Reuse an exact inner recurrence instead of reloading its saved start.

    An inner recurrence which runs exactly ``count`` times leaves through its
    sole exit as ``start + count * step``.  If that loop is itself repeated,
    subtracting the proven distance on the outer backedge reconstructs the
    next invocation's start and makes the separately saved start dead across
    the hot inner loop.

    This is deliberately a pressure-and-target decision.  In a register the
    old copy and the rewind are equivalent work; when pressure puts both ends
    in frame cells, the old form is a load plus a store and the new form is a
    memory update.  The 386/486/P5 profiles price the latter higher and retain
    the copy.  Later profiles may take it.  Nothing here names either form.
    """
    from qbopt.optimize import strength
    from qbopt.optimize import transform

    if costs is None:
        costs = OperationCosts()
    if not registers or costs.add > costs.move or costs.memory_update > costs.load + costs.store:
        return body

    found = loops.loops(body.blocks, body.entry)
    if len(found) < 2:
        return body
    blocks = {block.at: block for block in body.blocks}
    predecessors = loops.predecessors(body.blocks)
    dominators = loops.dominators(body.blocks, body.entry)
    facts = consts.known(body)
    live = liveness.live(body)
    values = tuple(ssa.values(body))
    definitions = {value: (block.at, op) for block in body.blocks for op in block.ops for value in op.defines}

    for inner in found:
        parents = [loop for loop in found if inner.body < loop.body and inner.header in loop.body]
        if not parents or len(inner.latches) != 1:
            continue
        parent = min(parents, key=lambda loop: len(loop.body))
        if len(parent.latches) != 1:
            continue
        inner_preheader = transform._preheader(body, inner)
        parent_preheader = transform._preheader(body, parent)
        inner_latch = next(iter(inner.latches))
        parent_latch = next(iter(parent.latches))
        if (
            inner_preheader is None
            or parent_preheader is None
            or inner_preheader not in parent.body
            or blocks[inner_preheader].succ != (inner.header,)
            or blocks[parent_preheader].succ != (parent.header,)
            or predecessors.get(parent.header) != frozenset({parent_preheader, parent_latch})
            or liveness.pressure(body, live, inner.body) < registers
        ):
            continue

        exiting = [
            (block.at, successor)
            for block in body.blocks
            if block.at in inner.body
            for successor in block.succ
            if successor in blocks and successor not in inner.body
        ]
        exits = {target for _source, target in exiting}
        if len(exits) != 1:
            continue
        (exit_at,) = exits
        sources = frozenset(source for source, _target in exiting)
        if (
            exit_at not in parent.body
            or predecessors.get(exit_at) != sources
            or exit_at not in dominators.get(parent_latch, frozenset())
        ):
            continue

        count = induction.trip_count(body, inner, facts)
        if count is None:
            continue
        header = blocks[inner.header]
        basics = induction.basics(body, inner)
        for counter in basics.values():
            phi = next((one for one in header.phis if one.result.id == counter.value), None)
            if phi is None or set(phi.incoming) != {inner_preheader, inner_latch}:
                continue
            start = phi.incoming[inner_preheader]
            update = phi.incoming[inner_latch]
            width = counter.start.width
            step = induction._signed(counter.step, facts, width)
            update_at = definitions.get(update)
            start_definition = definitions.get(start)
            if (
                step is None
                or not step
                or update_at is None
                or start_definition is None
                or start_definition[0] in parent.body
                or start in facts
            ):
                continue
            # A pre-tested loop exits from its header before executing the
            # next update.  Its phi is already ``start + count * step`` and
            # dominates that edge.  A post-tested loop exits from the latch,
            # where the update itself is the corresponding value.  Do not
            # demand the latter dominate an intentionally zero-trip-capable
            # header merely because both shapes share one recurrence proof.
            if all(update_at[0] in dominators.get(source, frozenset()) for source in sources):
                exit_value = update
            elif sources == frozenset({inner.header}):
                exit_value = phi.result
            else:
                continue
            # The saved start must become dead in the enclosing loop.  Other
            # uses would keep its live range and turn an equal-cost register
            # rewrite into a pure code-size loss.
            if any(start in op.uses for block in body.blocks if block.at in parent.body for op in block.ops) or any(
                value == start and other is not phi
                for block in body.blocks
                if block.at in parent.body
                for other in block.phis
                for value in other.incoming.values()
            ):
                continue

            mask = (1 << (width * 8)) - 1
            distance = step * count & mask
            if not distance:
                continue
            next_id = max((value.id for value in values), default=-1) + 1
            next_variable = max((value.variable for value in values), default=-1) + 1
            next_version = (
                max(
                    (value.version for value in values if value.variable == exit_value.variable),
                    default=0,
                )
                + 1
            )
            seed = mir.Value(next_id, parent_preheader, variable=next_variable, version=1)
            closed = mir.Value(next_id + 1, exit_at, variable=exit_value.variable, version=next_version)
            reset = mir.Value(next_id + 2, exit_at, variable=next_variable, version=2)
            carried = mir.Value(next_id + 3, parent.header, variable=next_variable, version=3)
            seeded = strength._made(
                mir.Kind.COPY,
                "",
                seed,
                (mir.Held(start, width),),
                blocks[parent_preheader].ops[-1].at if blocks[parent_preheader].ops else parent_preheader,
                start_definition[1],
            )
            rewind = strength._made(
                mir.Kind.ADD,
                "add",
                reset,
                (mir.Held(closed, width), mir.Const((-distance) & mask, width)),
                exit_at,
                update_at[1],
            )

            changed = []
            for block in body.blocks:
                phis = block.phis
                ops = list(block.ops)
                if block.at == parent_preheader:
                    _before_leaving(ops, [seeded])
                if block.at == parent.header:
                    phis = (*phis, mir.Phi(carried, {parent_preheader: seed, parent_latch: reset}))
                if block.at == inner.header:
                    phis = tuple(
                        replace(
                            other,
                            incoming={**other.incoming, inner_preheader: carried},
                        )
                        if other is phi
                        else other
                        for other in phis
                    )
                if block.at == exit_at:
                    phis = (*phis, mir.Phi(closed, {source: exit_value for source in sorted(sources)}))
                    _before_leaving(ops, [rewind])
                changed.append(replace(block, phis=tuple(phis), ops=tuple(ops)))
            counts = {**dict(body.loop_trip_counts), inner.header: count}
            return replace(body, blocks=tuple(changed), loop_trip_counts=tuple(sorted(counts.items())))
    return body


def simplified(body: mir.MirBody) -> mir.MirBody:
    from qbopt.optimize import loopexit
    from qbopt.optimize import strength
    from qbopt.optimize import transform

    facts = consts.known(body)
    blocks = {block.at: block for block in body.blocks}
    made = {value.id: op for block in body.blocks for op in block.ops for value in op.defines}
    dominators = loops.dominators(body.blocks, body.entry)
    predecessors = loops.predecessors(body.blocks)
    for loop in loops.loops(body.blocks, body.entry):
        if len(loop.latches) != 1:
            continue
        preheader = transform._preheader(body, loop)
        if preheader is None or blocks[preheader].succ != (loop.header,):
            continue
        header = blocks[loop.header]
        counters = induction.basics(body, loop)
        for counter in counters.values():
            width = counter.start.width
            last = induction._last_counter(body, loop, counter, facts, width)
            start = induction._signed(counter.start, facts, width)
            step = induction._signed(counter.step, facts, width)
            if last is None or start is None or not step:
                continue
            count = (last - start) // step + 1
            phi = next(phi for phi in header.phis if phi.result.id == counter.value)
            update = phi.incoming[next(iter(loop.latches))]
            branch = header.ops[-1]
            compare = next(
                op for op in header.ops[:-1] if induction._counter_bound(op, branch, counter, width, made) is not None
            )
            (exit_at,) = [at for at in header.succ if at not in loop.body]
            if set(predecessors.get(exit_at, ())) != {header.at} or not blocks[exit_at].ops:
                continue
            closed = {
                value: other
                for other in blocks[exit_at].phis
                if len(other.incoming) == 1
                for predecessor, value in other.incoming.items()
                if predecessor in loop.body
            }
            following = {at for at in blocks if exit_at in dominators.get(at, ())}
            if any(value in transform._leaving(body) for value in (phi.result, update)):
                continue
            if any(update in op.uses for block in body.blocks for op in block.ops):
                continue
            if any(
                (phi.result in other.incoming.values() or update in other.incoming.values())
                and other is not phi
                and other not in closed.values()
                for block in body.blocks
                for other in block.phis
            ):
                continue
            if any(
                set(compare.defines) & set(op.uses) and op is not branch for block in body.blocks for op in block.ops
            ):
                continue
            if any(
                set(compare.defines) & set(other.incoming.values()) for block in body.blocks for other in block.phis
            ):
                continue
            for alternative in counters.values():
                alternative_width = alternative.start.width
                stride = induction._signed(alternative.step, facts, alternative_width)
                modulus = 1 << (8 * alternative_width)
                if alternative.value == counter.value or not stride or count >= modulus // gcd(abs(stride), modulus):
                    continue
                alternative_phi = next(phi for phi in header.phis if phi.result.id == alternative.value)
                value = alternative_phi.result
                alternative_update = alternative_phi.incoming[next(iter(loop.latches))]
                if not any(
                    value in op.uses and alternative_update not in op.defines
                    for block in body.blocks
                    if block.at in loop.body
                    for op in block.ops
                ):
                    continue
                rebased = _rebased_equalities(
                    body,
                    phi.result,
                    update,
                    compare,
                    value,
                    following,
                )
                if rebased is None:
                    continue
                serial = max(value.id for value in ssa.values(body)) + 1
                variable = max(value.variable for value in ssa.values(body)) + 1
                seed_at = blocks[preheader].ops[-1].at
                bound = mir.Value(serial, seed_at, variable=variable)
                final = mir.Value(serial + 1, exit_at, variable=variable + 1)
                seed = strength._made(
                    mir.Kind.ADD,
                    "",
                    bound,
                    (
                        alternative.start,
                        mir.Const(consts.masked(stride * count, alternative_width), alternative_width),
                    ),
                    seed_at,
                    blocks[preheader].ops[-1],
                )
                finish = strength._made(
                    mir.Kind.COPY,
                    "",
                    final,
                    (mir.Const(consts.masked(last + step, width), width),),
                    exit_at,
                    blocks[exit_at].ops[0],
                )
                swap = {counter.value: final}
                removed = {other.result for value, other in closed.items() if value in (phi.result, update)}
                swap.update({value.id: final for value in removed})
                changed = loopexit._substituted_exits(body, exit_at, following, [finish], swap)
                if removed:
                    changed = replace(
                        changed,
                        blocks=tuple(
                            replace(block, phis=tuple(other for other in block.phis if other.result not in removed))
                            for block in changed.blocks
                        ),
                    )
                out = []
                for block in changed.blocks:
                    ops = []
                    for op in block.ops:
                        if op is compare:
                            op = replace(
                                op,
                                args=(mir.Held(value, alternative_width), mir.Held(bound, alternative_width)),
                                kind=mir.Kind.SUB,
                                results=(),
                                defines=tuple(value for value in op.defines if value.flags),
                                uses=(value, bound),
                                loads=(),
                                source_backed=False,
                                raised=None,
                            )
                        elif op is branch:
                            op = replace(
                                op,
                                test=mir.Kind.NE if branch.target in loop.body else mir.Kind.EQ,
                                name="",
                                source_backed=False,
                                raised=((), ()),
                            )
                        else:
                            op = rebased.get(id(op), op)
                        ops.append(op)
                    if block.at == preheader:
                        _before_leaving(ops, [seed])
                    out.append(replace(block, ops=tuple(ops)))
                return replace(changed, blocks=tuple(out))
    return body


def _recurrences(body: mir.MirBody, facts: dict) -> dict[int, tuple[induction.Affine, int, int | None, int | None]]:
    """Every basic recurrence, with its width and proven domain when finite."""
    out = {}
    for loop in loops.loops(body.blocks, body.entry):
        for affine in induction.basics(body, loop).values():
            width = affine.start.width
            domain = induction.domain(body, loop, affine, facts) or (None, None)
            out[affine.value] = (affine, width, *domain)
    return out


def _rebased_equalities(
    body: mir.MirBody,
    counter: mir.Value,
    update: mir.Value,
    control: mir.Op,
    alternative: mir.Value,
    following: set[int],
) -> dict[int, mir.Op] | None:
    """Rewrite equality-only body uses through an injective recurrence.

    Strength reduction commonly leaves both ``i`` and ``i * element_size``
    live.  The scaled recurrence may control the loop, but only if every other
    use of ``i`` can use it too.  Equality with another finite recurrence is
    such a use when both sides have the same affine map and their combined
    proven domain is shorter than the map's modular period.
    """
    extra = [
        op
        for block in body.blocks
        if block.at not in following
        for op in block.ops
        if counter in op.uses and op is not control and update not in op.defines
    ]
    if not extra:
        # The original IndVarSimplify case: any sufficiently long-lived
        # recurrence can terminate the loop when the old counter has no other
        # purpose.  No affine relationship between them is required.
        return {}
    facts = consts.known(body)
    recurrences = _recurrences(body, facts)
    source = recurrences.get(counter.id)
    target = recurrences.get(alternative.id)
    if (
        source is None
        or target is None
        or source[2] is None
        or source[3] is None
        or (relation := induction.relation(source[0], target[0], facts)) is None
    ):
        return None
    width = source[1]
    flags_readers = {
        value: [op for block in body.blocks for op in block.ops if value in op.uses]
        for block in body.blocks
        for op in block.ops
        for value in op.defines
        if value.flags
    }
    replacements = {}
    for block in body.blocks:
        if block.at in following:
            continue
        for op in block.ops:
            if counter not in op.uses or op is control or update in op.defines:
                continue
            if (
                op.kind is not mir.Kind.SUB
                or op.results
                or op.loads
                or op.stores
                or op.barrier
                or op.merges
                or len(op.args) != 2
                or len(op.defines) != 1
                or not op.defines[0].flags
                or not flags_readers.get(op.defines[0])
                or any(
                    reader.kind is not mir.Kind.BRANCH or reader.test not in (mir.Kind.EQ, mir.Kind.NE)
                    for reader in flags_readers[op.defines[0]]
                )
            ):
                return None
            positions = [
                index
                for index, arg in enumerate(op.args)
                if isinstance(arg, mir.Held) and arg.value == counter and arg.width == width
            ]
            if len(positions) != 1:
                return None
            other_at = 1 - positions[0]
            other = op.args[other_at]
            if not isinstance(other, mir.Held) or other.width != width:
                return None
            other_source = recurrences.get(other.value.id)
            if other_source is None or other_source[2] is None or other_source[3] is None:
                return None
            partner_id = next(
                (
                    value
                    for value, candidate in recurrences.items()
                    if value != other.value.id
                    and candidate[0].header == other_source[0].header
                    and induction.relation(other_source[0], candidate[0], facts) == relation
                ),
                None,
            )
            if partner_id is None:
                return None
            low = min(source[2], other_source[2])
            high = max(source[3], other_source[3])
            if not relation.injective(low, high):
                return None
            actual = next(
                value
                for block in body.blocks
                for phi in block.phis
                for value in (phi.result,)
                if value.id == partner_id
            )
            args = list(op.args)
            args[positions[0]] = mir.Held(alternative, width)
            args[other_at] = mir.Held(actual, width)
            uses = tuple(
                alternative if value == counter else actual if value == other.value else value for value in op.uses
            )
            replacements[id(op)] = replace(op, args=tuple(args), uses=uses, source_backed=False, raised=None)
    return replacements


def _before_leaving(ops: list, inserted: list) -> None:
    """`inserted` at the end of a preheader, before the jump it leaves by if it has one.

    A preheader that falls through ends on an operation like any other, and
    the seed may read what that one defines.
    """
    cut = len(ops) - bool(ops and ops[-1].kind in (mir.Kind.JUMP, mir.Kind.BRANCH))
    ops[cut:cut] = inserted


def zeroed(body: mir.MirBody, *, address_offsets: bool = False) -> mir.MirBody:
    """A counter counted up to zero, so the loop can end on its step's flags.

    `c` from `start` to `last` by `step` becomes `c - final`, `final` being
    `last + step`, and the exit test `c - final` against zero. Every other
    read is an invariant plus the counter, or plus the counter shifted, and
    the invariant takes `final`, shifted the same, once before the loop:
    both sides wrap at the add's own width, so the sum is unchanged.
    """
    from qbopt.optimize import loopexit
    from qbopt.optimize import strength
    from qbopt.optimize import transform

    facts = consts.known(body)
    blocks = {block.at: block for block in body.blocks}
    dominators = loops.dominators(body.blocks, body.entry)
    predecessors = loops.predecessors(body.blocks)
    made = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    home = {value: block.at for block in body.blocks for op in block.ops for value in op.defines}
    home.update({phi.result: block.at for block in body.blocks for phi in block.phis})
    readers: dict[mir.Value, list[mir.Op]] = {}
    for block in body.blocks:
        for op in block.ops:
            for value in op.uses:
                readers.setdefault(value, []).append(op)
    in_phis = {value for block in body.blocks for phi in block.phis for value in phi.incoming.values()}
    placed = {id(op): block.at for block in body.blocks for op in block.ops}
    for loop in loops.loops(body.blocks, body.entry):
        if len(loop.latches) != 1:
            continue
        preheader = transform._preheader(body, loop)
        if preheader is None or blocks[preheader].succ != (loop.header,):
            continue
        header, latch = blocks[loop.header], next(iter(loop.latches))
        if not header.ops or header.ops[-1].kind is not mir.Kind.BRANCH:
            continue
        branch = header.ops[-1]
        inside = set(loop.body)
        for counter in induction.basics(body, loop).values():
            phi = next(phi for phi in header.phis if phi.result.id == counter.value)
            if set(phi.incoming) != {preheader, latch}:
                continue
            initial, update = phi.incoming[preheader], phi.incoming[latch]
            seed, stepping = made.get(initial), made.get(update)
            if (
                seed is None
                or stepping is None
                or not stepping.results
                or not isinstance(stepping.results[0], mir.Held)
            ):
                continue
            width = stepping.results[0].width
            if seed.kind is not mir.Kind.COPY or len(seed.args) != 1 or not isinstance(seed.args[0], mir.Const):
                continue
            compares = [
                (op, compared)
                for op in header.ops[:-1]
                for compared in (2, 4)
                if induction._counter_bound(op, branch, counter, compared, made) is not None
            ]
            if len(compares) != 1:
                continue
            compare, compared = compares[0]
            if compare.args[1] == mir.Const(0, compared) and branch.test in (mir.Kind.NE, mir.Kind.EQ):
                continue  # counts to zero already
            start = induction._signed(counter.start, facts, counter.start.width)
            step = induction._signed(counter.step, facts, counter.step.width)
            if start is None or not step:
                continue
            narrowed = replace(
                counter,
                start=mir.Const(consts.masked(start, compared), compared),
                step=mir.Const(consts.masked(step, compared), compared),
            )
            if induction._signed(narrowed.start, facts, compared) != start:
                continue
            last = induction._last_counter(body, loop, narrowed, facts, compared)
            if last is None:
                continue
            final = last + step
            offsets = _offsets(
                phi.result,
                readers,
                placed,
                home,
                inside,
                {id(compare), id(stepping)},
                address_offsets=address_offsets,
            )
            if offsets is None:
                continue
            read = {value for block in body.blocks for op in block.ops for value in op.uses}
            if any(value.flags and (value in read or value in in_phis) for value in stepping.defines):
                continue
            if any(update in op.uses for block in body.blocks for op in block.ops) or initial in read:
                continue
            if any(
                set(compare.defines) & set(op.uses) and op is not branch for block in body.blocks for op in block.ops
            ):
                continue
            (exit_at,) = [at for at in header.succ if at not in inside]
            if set(predecessors.get(exit_at, ())) != {header.at} or not blocks[exit_at].ops:
                continue
            closed = {
                value: other
                for other in blocks[exit_at].phis
                if len(other.incoming) == 1
                for predecessor, value in other.incoming.items()
                if predecessor in inside
            }
            following = {at for at in blocks if exit_at in dominators.get(at, ())}
            if any(value in transform._leaving(body) for value in (phi.result, update)):
                continue
            if any(
                other is not phi
                and other not in closed.values()
                and {phi.result, update} & set(other.incoming.values())
                for block in body.blocks
                for other in block.phis
            ):
                continue
            if any(
                phi.result in op.uses and block.at not in following and block.at not in inside
                for block in body.blocks
                for op in block.ops
            ):
                continue
            serial = max(value.id for value in ssa.values(body)) + 1
            variable = max(value.variable for value in ssa.values(body)) + 1
            ending = blocks[preheader].ops[-1]
            seeds, rebased = [], {}
            for op, position, multiplier, address, extra in offsets:
                if position is None:
                    assert address is not None
                    source, replacement = address
                    rebased[id(op)] = _rebased_cells(
                        op,
                        source,
                        final * multiplier + extra,
                        replacement,
                    )
                    continue
                base = op.args[position]
                if isinstance(base, mir.Const):
                    args = tuple(
                        mir.Const(consts.masked(base.n + final * multiplier, base.width), base.width)
                        if index == position
                        else arg
                        for index, arg in enumerate(op.args)
                    )
                    rebased[id(op)] = replace(op, args=args, source_backed=False, raised=None)
                    continue
                assert isinstance(base, mir.Held)
                moved = mir.Value(serial, ending.at, variable=variable)
                serial, variable = serial + 1, variable + 1
                seeds.append(
                    strength._made(
                        mir.Kind.ADD,
                        "add",
                        moved,
                        (base, mir.Const(consts.masked(final * multiplier, base.width), base.width)),
                        ending.at,
                        ending,
                    )
                )
                args = tuple(
                    mir.Held(moved, base.width) if index == position else arg for index, arg in enumerate(op.args)
                )
                rebased[id(op)] = replace(
                    op, args=args, uses=tuple(moved if value == base.value else value for value in op.uses)
                )
            finished = mir.Value(serial, exit_at, variable=variable)
            # A start of its own: the constant it was seeded from can start another loop too.
            begun = mir.Value(serial + 1, ending.at, variable=variable + 1)
            seeded = seed.args[0].width
            seeds.append(
                strength._made(
                    mir.Kind.COPY,
                    "",
                    begun,
                    (mir.Const(consts.masked(start - final, seeded), seeded),),
                    ending.at,
                    ending,
                )
            )
            finish = strength._made(
                mir.Kind.COPY,
                "",
                finished,
                (mir.Const(consts.masked(final, width), width),),
                exit_at,
                blocks[exit_at].ops[0],
            )
            swap = {counter.value: finished}
            removed = {other.result for value, other in closed.items() if value in (phi.result, update)}
            swap.update({value.id: finished for value in removed})
            changed = loopexit._substituted_exits(body, exit_at, following, [finish], swap)
            out = []
            for block in changed.blocks:
                ops = []
                for op in block.ops:
                    if op is compare:
                        op = replace(
                            op,
                            args=(mir.Held(phi.result, width), mir.Const(0, width)),
                            kind=mir.Kind.SUB,
                            results=(),
                            defines=tuple(value for value in op.defines if value.flags),
                            uses=(phi.result,),
                            loads=(),
                            source_backed=False,
                            raised=None,
                        )
                    elif op is branch:
                        test = mir.Kind.NE if branch.target in inside else mir.Kind.EQ
                        op = replace(op, test=test, name="", source_backed=False, raised=((), ()))
                    else:
                        op = rebased.get(id(op), op)
                    ops.append(op)
                if block.at == preheader:
                    _before_leaving(ops, seeds)
                out.append(
                    replace(
                        block,
                        ops=tuple(ops),
                        phis=tuple(
                            replace(other, incoming={**other.incoming, preheader: begun})
                            if other.result == phi.result
                            else other
                            for other in block.phis
                            if other.result not in removed
                        ),
                    )
                )
            return zeroed(replace(changed, blocks=tuple(out)), address_offsets=address_offsets)
    return body


def _offsets(counter, readers, placed, home, inside, own, *, address_offsets: bool = False):
    """Every add of an invariant to the counter, as (add, invariant's position, multiplier).

    Also through a shift of the counter read only by such adds. None where
    the counter is read any other way inside the loop, or a flag an add or
    shift sets is read.
    """

    def flagless(op):
        return not any(value.flags and value in readers for value in op.defines)

    def plain(op, kind):
        return (
            op.kind is kind
            and not (op.loads or op.stores or op.merges or op.barrier)
            and len(op.results) == 1
            and isinstance(op.results[0], mir.Held)
            and flagless(op)
        )

    def added(op, value, multiplier):
        if not plain(op, mir.Kind.ADD) or len(op.args) != 2:
            return None
        accepted = (mir.Held, mir.Const) if address_offsets else (mir.Held,)
        if not all(isinstance(arg, accepted) and arg.width == op.results[0].width for arg in op.args):
            return None
        counted = [index for index, arg in enumerate(op.args) if isinstance(arg, mir.Held) and arg.value == value]
        if len(counted) != 1:
            return None
        position = 1 - counted[0]
        invariant = op.args[position]
        if isinstance(invariant, mir.Held) and home.get(invariant.value) in inside:
            return None
        return (op, position, multiplier, None, 0)

    def addressed(op, value, multiplier, replacement=None, extra=0):
        refs = (*op.loads, *op.stores, *(ref for ref, _known in op.memory_values))
        found = [ref for ref in refs if ref.base == value]
        if (
            not found
            or any(ref.addr is None or ref.symbolic is not None for ref in found)
            or any(isinstance(arg, mir.Held) and arg.value == value for arg in op.args)
            or any(ref.segment == value for ref in refs)
            or any(ref.base == value for ref in refs if ref not in found)
        ):
            return None
        return (op, None, multiplier, (value, replacement or value), extra)

    def derived(op, value, multiplier):
        form = added(op, value, multiplier)
        if form is not None or not address_offsets:
            return form
        return addressed(op, value, multiplier)

    def equality(op, value):
        """The invariant side of an equality, shifted with the counter.

        Replacing ``counter`` by ``counter - final`` preserves identity only
        when the other side becomes ``other - final`` too.  This is safe for
        equality and inequality flags; ordered comparisons would change.
        """
        if (
            not address_offsets
            or op.kind is not mir.Kind.SUB
            or op.results
            or op.loads
            or op.stores
            or op.barrier
            or op.merges
            or len(op.args) != 2
            or len(op.defines) != 1
            or not op.defines[0].flags
            or not readers.get(op.defines[0])
            or any(
                reader.kind is not mir.Kind.BRANCH or reader.test not in (mir.Kind.EQ, mir.Kind.NE)
                for reader in readers[op.defines[0]]
            )
        ):
            return None
        positions = [index for index, arg in enumerate(op.args) if isinstance(arg, mir.Held) and arg.value == value]
        if len(positions) != 1:
            return None
        position = 1 - positions[0]
        other = op.args[position]
        if (
            not isinstance(other, (mir.Held, mir.Const))
            or other.width != op.args[positions[0]].width
            or isinstance(other, mir.Held)
            and home.get(other.value) in inside
        ):
            return None
        return (op, position, -1, None, 0)

    out = []
    for op in readers.get(counter, []):
        if id(op) in own or placed[id(op)] not in inside:
            continue
        form = addressed(op, counter, 1) if address_offsets else None
        added_form = added(op, counter, 1)
        if address_offsets and added_form is not None:
            position = added_form[1]
            invariant = op.args[position]
            if isinstance(invariant, mir.Const):
                result = op.results[0].value
                constant = induction._signed(invariant, {}, invariant.width)
                forms = [
                    addressed(reader, result, 1, replacement=counter, extra=constant)
                    for reader in readers.get(result, [])
                ]
                if forms and all(forms):
                    out.extend(forms)
                    continue
        if form is None:
            form = added_form
        if form is None:
            form = equality(op, counter)
        scale = None
        if plain(op, mir.Kind.SHL) and len(op.args) == 2 and isinstance(op.args[1], mir.Const):
            if op.args[0] == mir.Held(counter, op.results[0].width):
                scale = 1 << op.args[1].n
        elif address_offsets and plain(op, mir.Kind.MUL) and len(op.args) == 2:
            constants = [arg for arg in op.args if isinstance(arg, mir.Const)]
            held = [arg for arg in op.args if isinstance(arg, mir.Held) and arg.value == counter]
            if len(constants) == len(held) == 1 and constants[0].width == held[0].width == op.results[0].width:
                scale = induction._signed(constants[0], {}, constants[0].width)
        if form is None and scale is not None:
            shifted = op.results[0].value
            forms = [derived(reader, shifted, scale) for reader in readers.get(shifted, [])]
            if forms and all(forms):
                out.extend(forms)
                continue
        if form is None:
            return None
        out.append(form)
    return out


def _rebased_cells(
    op: mir.Op,
    base: mir.Value,
    displacement: int,
    replacement: mir.Value,
) -> mir.Op:
    """Move the fixed part of every address using ``base`` by displacement."""

    def reference(ref: mir.MemRef) -> mir.MemRef:
        if ref.base != base:
            return ref
        assert ref.addr is not None and ref.symbolic is None
        return replace(ref, addr=ref.addr.plus(displacement), base=replacement)

    def operand(arg):
        return replace(arg, ref=reference(arg.ref)) if isinstance(arg, mir.Cell) else arg

    return replace(
        op,
        args=tuple(operand(arg) for arg in op.args),
        results=tuple(operand(arg) if isinstance(arg, mir.Cell) else arg for arg in op.results),
        loads=tuple(reference(ref) for ref in op.loads),
        stores=tuple(reference(ref) for ref in op.stores),
        memory_values=tuple((reference(ref), known) for ref, known in op.memory_values),
        uses=tuple(replacement if value == base else value for value in op.uses),
        source_backed=False,
        raised=None,
    )
