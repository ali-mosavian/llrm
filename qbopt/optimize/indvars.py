"""Use an existing recurrence to control a loop instead of a redundant counter."""

from math import gcd
from dataclasses import replace

from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.analysis import loops
from qbopt.analysis import consts
from qbopt.analysis import induction


def simplified(body: mir.MirBody) -> mir.MirBody:
    from qbopt.optimize import loopexit
    from qbopt.optimize import strength
    from qbopt.optimize import transform

    facts = consts.known(body)
    blocks = {block.at: block for block in body.blocks}
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
                op for op in header.ops[:-1] if induction._counter_bound(op, branch, counter, width) is not None
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
            if any(
                phi.result in op.uses and op is not compare and update not in op.defines
                for block in body.blocks
                if block.at not in following
                for op in block.ops
            ):
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
                stride = induction._signed(alternative.step, facts, width)
                if (
                    alternative.value == counter.value
                    or alternative.start.width != width
                    or not stride
                    or count >= (1 << (8 * width)) // gcd(abs(stride), 1 << (8 * width))
                ):
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
                serial = max(value.id for value in ssa.values(body)) + 1
                variable = max(value.variable for value in ssa.values(body)) + 1
                seed_at = blocks[preheader].ops[-1].at
                bound = mir.Value(serial, seed_at, variable=variable)
                final = mir.Value(serial + 1, exit_at, variable=variable + 1)
                seed = strength._made(
                    mir.Kind.ADD,
                    "",
                    bound,
                    (alternative.start, mir.Const(consts.masked(stride * count, width), width)),
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
                                args=(mir.Held(value, width), mir.Held(bound, width)),
                                kind=mir.Kind.SUB,
                                results=(),
                                defines=tuple(value for value in op.defines if value.flags),
                                uses=(value, bound),
                                loads=(),
                                node=None,
                                made=None,
                                raised=None,
                            )
                        elif op is branch:
                            op = replace(
                                op,
                                test=mir.Kind.NE if branch.target in loop.body else mir.Kind.EQ,
                                name="",
                                node=None,
                                made=None,
                                raised=((), ()),
                            )
                        ops.append(op)
                    if block.at == preheader:
                        ops.insert(len(ops) - 1, seed)
                    out.append(replace(block, ops=tuple(ops)))
                return replace(changed, blocks=tuple(out))
    return body


def zeroed(body: mir.MirBody) -> mir.MirBody:
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
            if seed is None or stepping is None or not stepping.results or not isinstance(stepping.results[0], mir.Held):
                continue
            width = stepping.results[0].width
            if seed.kind is not mir.Kind.COPY or len(seed.args) != 1 or not isinstance(seed.args[0], mir.Const):
                continue
            compares = [
                (op, compared)
                for op in header.ops[:-1]
                for compared in (2, 4)
                if induction._counter_bound(op, branch, counter, compared) is not None
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
            offsets = _offsets(phi.result, readers, placed, home, inside, {id(compare), id(stepping)})
            if offsets is None:
                continue
            read = {value for block in body.blocks for op in block.ops for value in op.uses}
            if any(value.flags and (value in read or value in in_phis) for value in stepping.defines):
                continue
            if any(update in op.uses for block in body.blocks for op in block.ops) or initial in read:
                continue
            if any(set(compare.defines) & set(op.uses) and op is not branch for block in body.blocks for op in block.ops):
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
                other is not phi and other not in closed.values() and {phi.result, update} & set(other.incoming.values())
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
            for op, position, multiplier in offsets:
                base = op.args[position]
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
                args = tuple(mir.Held(moved, base.width) if index == position else arg for index, arg in enumerate(op.args))
                rebased[id(op)] = replace(
                    op, args=args, uses=tuple(moved if value == base.value else value for value in op.uses)
                )
            finished = mir.Value(serial, exit_at, variable=variable)
            # A start of its own: the constant it was seeded from can start another loop too.
            begun = mir.Value(serial + 1, ending.at, variable=variable + 1)
            seeded = seed.args[0].width
            seeds.append(
                strength._made(
                    mir.Kind.COPY, "", begun, (mir.Const(consts.masked(start - final, seeded), seeded),), ending.at, ending
                )
            )
            finish = strength._made(
                mir.Kind.COPY, "", finished, (mir.Const(consts.masked(final, width), width),), exit_at, blocks[exit_at].ops[0]
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
                            node=None,
                            made=None,
                            raised=None,
                        )
                    elif op is branch:
                        test = mir.Kind.NE if branch.target in inside else mir.Kind.EQ
                        op = replace(op, test=test, name="", node=None, made=None, raised=((), ()))
                    else:
                        op = rebased.get(id(op), op)
                    ops.append(op)
                if block.at == preheader:
                    cut = len(ops) - (ops[-1].kind in (mir.Kind.JUMP, mir.Kind.BRANCH))
                    ops[cut:cut] = seeds
                out.append(
                    replace(
                        block,
                        ops=tuple(ops),
                        phis=tuple(
                            replace(other, incoming={**other.incoming, preheader: begun}) if other.result == phi.result else other
                            for other in block.phis
                            if other.result not in removed
                        ),
                    )
                )
            return zeroed(replace(changed, blocks=tuple(out)))
    return body


def _offsets(counter, readers, placed, home, inside, own):
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
        if not all(isinstance(arg, mir.Held) and arg.width == op.results[0].width for arg in op.args):
            return None
        positions = [index for index, arg in enumerate(op.args) if arg.value != value]
        if len(positions) != 1 or home.get(op.args[positions[0]].value) in inside:
            return None
        return (op, positions[0], multiplier)

    out = []
    for op in readers.get(counter, []):
        if id(op) in own or placed[id(op)] not in inside:
            continue
        form = added(op, counter, 1)
        if form is None and plain(op, mir.Kind.SHL) and len(op.args) == 2 and isinstance(op.args[1], mir.Const):
            shifted = op.results[0].value
            forms = [added(reader, shifted, 1 << op.args[1].n) for reader in readers.get(shifted, [])]
            if forms and all(forms) and op.args[0] == mir.Held(counter, op.results[0].width):
                out.extend(forms)
                continue
        if form is None:
            return None
        out.append(form)
    return out
