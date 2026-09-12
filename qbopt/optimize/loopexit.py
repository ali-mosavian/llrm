"""Evaluate affine exit values and delete finite, side-effect-free counted loops.

The integer equivalent of evaluating an AddRec at its backedge count: a
fixed increment sums to N * step; an affine increment also contributes
N(N-1)/2 times its stride. Results are modulo their own width. Only the
controlling recurrence must be proven not to wrap.
"""

from dataclasses import replace

from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.analysis import loops
from qbopt.analysis import consts
from qbopt.analysis import induction


def evaluated(body: mir.MirBody) -> mir.MirBody:
    from qbopt.optimize import strength
    from qbopt.optimize import transform

    facts = consts.known(body)
    blocks = {block.at: block for block in body.blocks}
    for loop in loops.loops(body.blocks, body.entry):
        if len(loop.body) != 2 or len(loop.latches) != 1:
            continue
        header = blocks[loop.header]
        latch = blocks[next(iter(loop.latches))]
        preheader = transform._preheader(body, loop)
        if preheader is None or blocks[preheader].succ != (header.at,) or latch.phis:
            continue
        counters = induction.basics(body, loop)
        if not counters:
            continue
        counts = set()
        for counter in counters.values():
            width = counter.start.width
            last = induction._last_counter(body, loop, counter, facts, width)
            start = induction._signed(counter.start, facts, width)
            step = induction._signed(counter.step, facts, width)
            if last is not None and start is not None and step:
                counts.add((last - start) // step + 1)
        if len(counts) != 1:
            continue
        count = counts.pop()
        exits = _exit_terms(body, loop, counters, count, facts)
        if not exits:
            continue
        (exit_at,) = [at for at in header.succ if at not in loop.body]
        if set(exits) != {phi.result.id for phi in header.phis} or not _disposable(body, loop, header, latch):
            changed = _constant_exits(body, loop, exits, exit_at, facts)
            if changed is not body:
                return changed
            continue
        serial = max(value.id for value in ssa.values(body)) + 1
        variable = max(value.variable for value in ssa.values(body)) + 1
        calculations = []
        for phi in header.phis:
            terms = exits[phi.result.id]
            width = terms[0][0].width
            total = mir.Const(0, width)
            for arg, coefficient in terms:
                if isinstance(arg, mir.Const):
                    product = mir.Const(consts.masked(arg.n * coefficient, width), width)
                elif coefficient == 1:
                    product = arg
                else:
                    temporary = mir.Value(serial, header.at, variable=variable)
                    serial += 1
                    variable += 1
                    calculations.append(
                        strength._made(
                            mir.Kind.MUL,
                            "",
                            temporary,
                            (arg, mir.Const(consts.masked(coefficient, width), width)),
                            header.at,
                            header.ops[0],
                        )
                    )
                    product = mir.Held(temporary, width)
                temporary = mir.Value(serial, header.at, variable=variable)
                serial += 1
                variable += 1
                calculations.append(
                    strength._made(mir.Kind.ADD, "", temporary, (total, product), header.at, header.ops[0])
                )
                total = mir.Held(temporary, width)
            calculations.append(strength._made(mir.Kind.COPY, "", phi.result, (total,), header.at, header.ops[0]))
        jump = replace(
            header.ops[-1],
            kind=mir.Kind.JUMP,
            uses=(),
            args=(),
            results=(),
            target=exit_at,
            made=None,
            raised=((), ()),
            test=None,
        )
        # Preserve all original byte ownership while replacing the header's work.
        replacement = replace(
            header,
            phis=(),
            succ=(exit_at,),
            ops=tuple(calculations) + tuple(_cleared(op) for op in header.ops[:-1]) + (jump,),
        )
        changed = replace(
            body,
            blocks=tuple(
                replacement
                if block.at == header.at
                else replace(block, succ=(), ops=tuple(_cleared(op) for op in block.ops))
                if block.at == latch.at
                else block
                for block in body.blocks
            ),
        )
        return transform._trivial_phis(transform._unreachable(changed))
    return body


def _exit_terms(body, loop, counters, count, facts):
    """Sum a linear increment over N iterations using N(N-1)/2, before modular reduction."""
    header = next(block for block in body.blocks if block.at == loop.header)
    made = {value: op for block in body.blocks if block.at in loop.body for op in block.ops for value in op.defines}
    still = induction.invariant(body, set(loop.body))
    headers = {phi.result for phi in header.phis}
    widened = _widened_counters(body, loop, counters, facts)
    headers.update(widened)
    counters = {**counters, **{value.id: counter for value, counter in widened.items()}}
    exits = {}
    for phi in header.phis:
        if phi.result.id in counters:
            counter = counters[phi.result.id]
            exits[phi.result.id] = [(counter.start, 1), (counter.step, count)]
            continue
        (start,) = [value for at, value in phi.incoming.items() if at not in loop.body]
        (update,) = [value for at, value in phi.incoming.items() if at in loop.body]
        op = made.get(update)
        if op is None or len(op.results) != 1 or not isinstance(op.results[0], mir.Held):
            continue
        width = op.results[0].width
        linear = _linear(mir.Held(update, width), made, headers, width, set(), {})
        if linear is None or linear.pop(mir.Held(phi.result, width), 0) != 1:
            continue
        terms = [(mir.Held(start, width), 1)]
        for arg, coefficient in linear.items():
            if isinstance(arg, mir.Const) or arg.value.id in still:
                terms.append((arg, coefficient * count))
            elif (counter := counters.get(arg.value.id)) is not None and counter.start.width == width:
                terms.extend(
                    ((counter.start, coefficient * count), (counter.step, coefficient * count * (count - 1) // 2))
                )
            else:
                break
        else:
            exits[phi.result.id] = terms
    return exits


def _widened_counters(body, loop, counters, facts):
    from qbopt.analysis import ranges

    if not any(op.kind is mir.Kind.SIGN_EXTEND for block in body.blocks if block.at in loop.body for op in block.ops):
        return {}
    bounds = ranges.bounded(body)
    widened = {}
    for block in body.blocks:
        if block.at not in loop.body:
            continue
        for op in block.ops:
            if (
                op.kind is not mir.Kind.SIGN_EXTEND
                or len(op.args) != len(op.results)
                or len(op.args) != 1
                or op.loads
                or op.stores
                or op.merges
            ):
                continue
            source, result = op.args[0], op.results[0]
            if not isinstance(source, mir.Held) or not isinstance(result, mir.Held) or source.width >= result.width:
                continue
            counter = counters.get(source.value.id)
            interval = bounds.get(block.at, {}).get(source.value)
            if counter is None or interval is None or interval.width != source.width:
                continue
            start = induction._signed(counter.start, facts, source.width)
            step = induction._signed(counter.step, facts, source.width)
            if start is not None and step is not None:
                widened[result.value] = induction.Affine(
                    result.value.id, mir.Const(start, result.width), mir.Const(step, result.width), loop.header
                )
    return widened


def _constant_exits(
    body: mir.MirBody,
    loop: loops.Loop,
    exits: dict[int, list[tuple[mir.Held | mir.Const, int]]],
    exit_at: int,
    facts: dict[mir.Value, consts.Known],
) -> mir.MirBody:
    """Replace constant live-outs after the loop, leaving its observable work intact."""
    from qbopt.optimize import strength
    from qbopt.optimize import transform

    predecessors = loops.predecessors(body.blocks)
    if set(predecessors.get(exit_at, ())) != {loop.header}:
        return body
    dominators = loops.dominators(body.blocks, body.entry)
    following = {block.at for block in body.blocks if exit_at in dominators.get(block.at, ())}
    used = {value.id for block in body.blocks if block.at in following for op in block.ops for value in op.uses}
    used.update(
        value.id
        for block in body.blocks
        for phi in block.phis
        for predecessor, value in phi.incoming.items()
        if predecessor in following
    )
    serial = max(value.id for value in ssa.values(body)) + 1
    variable = max(value.variable for value in ssa.values(body)) + 1
    exit_block = next(block for block in body.blocks if block.at == exit_at)
    if not exit_block.ops:
        return body
    aliases = {
        phi.result.id: phi.incoming[loop.header].id for phi in exit_block.phis if set(phi.incoming) == {loop.header}
    }
    unavailable = {
        aliases[value.id]
        for block in body.blocks
        if block.at not in following
        for op in block.ops
        for value in op.uses
        if value.id in aliases
    }
    unavailable.update(
        aliases[value.id]
        for block in body.blocks
        for phi in block.phis
        for predecessor, value in phi.incoming.items()
        if predecessor not in following and value.id in aliases
    )
    used.update(aliases[value] for value in tuple(used) if value in aliases)
    added, swap = [], {}
    for value, terms in exits.items():
        if value not in used or value in unavailable:
            continue
        width = terms[0][0].width
        total = 0
        for arg, coefficient in terms:
            fact = facts.get(arg.value) if isinstance(arg, mir.Held) else arg
            if fact is None or fact.width < arg.width:
                break
            total += consts.masked(fact.n, arg.width) * coefficient
        else:
            result = mir.Value(serial, exit_at, variable=variable)
            serial += 1
            variable += 1
            added.append(
                strength._made(
                    mir.Kind.COPY,
                    "",
                    result,
                    (mir.Const(consts.masked(total, width), width),),
                    exit_at,
                    exit_block.ops[0],
                )
            )
            swap[value] = result
    if not swap:
        return body

    def with_aliases(replacements: dict[int, mir.Value]) -> dict[int, mir.Value]:
        return {
            **replacements,
            **{alias: replacements[source] for alias, source in aliases.items() if source in replacements},
        }

    changed = _substituted_exits(body, exit_at, following, added, with_aliases(swap))
    alive = {value.id for value in transform.live(changed)}
    profitable = {value: result for value, result in swap.items() if value not in alive}
    if not profitable:
        return body
    if profitable == swap:
        return changed
    added = [op for op in added if op.defines[0] in profitable.values()]
    return _substituted_exits(body, exit_at, following, added, with_aliases(profitable))


def _substituted_exits(
    body: mir.MirBody,
    exit_at: int,
    following: set[int],
    added: list[mir.Op],
    swap: dict[int, mir.Value],
) -> mir.MirBody:
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                phis=tuple(
                    replace(
                        phi,
                        incoming={
                            predecessor: ssa.provider(value, swap) if predecessor in following else value
                            for predecessor, value in phi.incoming.items()
                        },
                    )
                    for phi in block.phis
                    if block.at != exit_at or phi.result.id not in swap
                ),
                ops=(tuple(added) if block.at == exit_at else ())
                + (tuple(ssa.substituted(op, swap) for op in block.ops) if block.at in following else block.ops),
            )
            for block in body.blocks
        ),
    )


def _linear(arg, made, headers, width, visiting, cached):
    if not isinstance(arg, (mir.Const, mir.Held)) or arg.width != width:
        return None
    if isinstance(arg, mir.Const) or arg.value in headers or arg.value not in made:
        return {arg: 1}
    if arg.value in visiting:
        return None
    if arg in cached:
        return cached[arg]
    op = made[arg.value]
    # Not `op.merges`. That is the two-address tie -- which use shares a
    # register with which definition -- and it says nothing about whether
    # the operation is a linear function of its own arguments. BC writes
    # every accumulator as a two-address `add`, so refusing on it refused
    # every accumulator there is: hotlpx's `s = s + (n*k) + i` linearised
    # to None, so the sum had no exit value and the loop could not go.
    if op.results != (arg,) or op.loads or op.stores:
        return None
    if op.kind is mir.Kind.COPY and len(op.args) == 1:
        parts = ((op.args[0], 1),)
    elif op.kind in (mir.Kind.ADD, mir.Kind.SUB) and len(op.args) == 2:
        parts = ((op.args[0], 1), (op.args[1], -1 if op.kind is mir.Kind.SUB else 1))
    elif op.kind in (mir.Kind.INCREMENT, mir.Kind.DECREMENT) and len(op.args) == 1:
        parts = ((op.args[0], 1), (mir.Const(1, width), -1 if op.kind is mir.Kind.DECREMENT else 1))
    else:
        return None
    result = {}
    for source, coefficient in parts:
        terms = _linear(source, made, headers, width, visiting | {arg.value}, cached)
        if terms is None:
            return None
        for term, factor in terms.items():
            result[term] = result.get(term, 0) + coefficient * factor
    cached[arg] = {term: factor for term, factor in result.items() if factor}
    return cached[arg]


def _cleared(op):
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
        made=None,
        raised=None,
        target=None,
        test=None,
        stack=None,
        symbol=False,
    )


def _disposable(body, loop, header, latch) -> bool:
    allowed = {mir.Kind.NOTHING, mir.Kind.COPY, mir.Kind.ADD, mir.Kind.SUB, mir.Kind.INCREMENT, mir.Kind.DECREMENT}
    for block in (header, latch):
        for op in block.ops:
            # `merges` again: deleting the block removes the tied use and
            # the tied definition together, so the tie cannot outlive it.
            if op.loads or op.stores or op.stack is not None:
                return False
            if block is header and op is header.ops[-1]:
                continue  # The trip-count proof checked this branch.
            if op.kind not in allowed or any(not isinstance(arg, (mir.Held, mir.Const)) for arg in op.args):
                return False
    internal = {value for block in (header, latch) for op in block.ops for value in op.defines}
    for block in body.blocks:
        if block.at in loop.body:
            continue
        if any(value in internal for op in block.ops for value in op.uses):
            return False
        if any(value in internal for phi in block.phis for value in phi.incoming.values()):
            return False
    return True
