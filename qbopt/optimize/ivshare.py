from dataclasses import replace

from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.analysis import consts
from qbopt.analysis import induction


def shared(body: mir.MirBody) -> mir.MirBody:
    from qbopt.optimize import transform
    from qbopt.optimize.strength import _made

    definitions = {value.id: op for block in body.blocks for op in block.ops for value in op.defines}
    required = transform.halves(body)
    for loop in loops.loops(body.blocks, body.entry):
        counters = induction.basics(body, loop)
        header = next(block for block in body.blocks if block.at == loop.header)
        for derived in counters.values():
            twin = _twin(counters, derived, header, required)
            if twin is not None:
                return _replacing(body, header, derived, twin, derived.start.width)
            found = _offset(counters, derived, definitions)
            if found is None:
                continue
            base, offset, seed = found
            width = derived.start.width
            phi = next(one for one in header.phis if one.result.id == derived.value)
            upper = None
            if width < 4 and (phi.result, transform.HIGH) in required:
                carried = []
                for incoming in phi.incoming.values():
                    producer = definitions.get(incoming.id)
                    if (
                        producer is None
                        or producer.results != (mir.Held(incoming, width),)
                        or len(producer.merges) != 1
                    ):
                        break
                    carried.append(next(iter(producer.merges)))
                else:
                    if len(set(carried)) == 1:
                        upper = carried[0]
                if upper is None or upper.id not in induction.invariant(body, set(loop.body)):
                    continue
            root = next(one.result for one in header.phis if one.result.id == base.value)
            operation = _made(
                mir.Kind.ADD, "add", phi.result, (mir.Held(root, width), offset), header.at, seed or header.ops[0]
            )
            if upper is not None:
                operation = replace(operation, merges={upper: phi.result}, uses=(*operation.uses, upper))
            changed = replace(
                header, phis=tuple(one for one in header.phis if one is not phi), ops=(operation, *header.ops)
            )
            return replace(body, blocks=tuple(changed if block is header else block for block in body.blocks))
    return body


def _offset(
    counters: dict[int, induction.Affine], derived: induction.Affine, definitions: dict[int, mir.Op]
) -> tuple[induction.Affine, mir.Const, mir.Op | None] | None:
    """Another counter stepping as `derived` does a constant distance behind it, the distance, and a seed.

    Two starts are that far apart when both are one root plus a constant:
    `add source,c` over the other's start, or strength reduction's `a[i].x`
    and `a[i].y`, starting 4 apart from the same or no root. Otherwise the
    canonical counter is the lower id, so the pair converges.
    """
    if not isinstance(derived.step, mir.Const) or not isinstance(derived.start, (mir.Held, mir.Const)):
        return None
    width = derived.start.width
    root, at = _anchor(derived.start, definitions, width)
    seed = definitions.get(derived.start.value.id) if isinstance(derived.start, mir.Held) else None
    direct = seed.args if seed is not None and seed.kind is mir.Kind.ADD else ()
    for one in sorted(counters.values(), key=lambda one: (one.start not in direct, one.value)):
        if one.value == derived.value or one.step != derived.step or not isinstance(one.start, (mir.Held, mir.Const)):
            continue
        if one.start not in direct and one.value > derived.value:
            continue
        if one.start.width != width or _anchor(one.start, definitions, width)[0] != root:
            continue
        distance = at - _anchor(one.start, definitions, width)[1]
        return one, mir.Const(consts.masked(distance, width), width), seed
    return None


def _anchor(arg: mir.Held | mir.Const, definitions: dict[int, mir.Op], width: int) -> tuple[mir.Value | None, int]:
    """`arg` as a root value plus a constant, through copies and constant adds; a constant has no root."""
    offset = 0
    while isinstance(arg, mir.Held) and arg.width == width:
        op = definitions.get(arg.value.id)
        if op is None or op.loads or op.stores or op.barrier or op.merges or op.results != (arg,):
            break
        if op.kind is mir.Kind.COPY and len(op.args) == 1 and isinstance(op.args[0], (mir.Held, mir.Const)):
            arg = op.args[0]
        elif op.kind is mir.Kind.ADD and len(op.args) == 2 and sum(isinstance(one, mir.Const) for one in op.args) == 1:
            constant, arg = sorted(op.args, key=lambda one: not isinstance(one, mir.Const))
            offset += constant.n
        else:
            break
    if isinstance(arg, mir.Const):
        return None, consts.masked(arg.n + offset, width)
    return arg.value, offset


def _twin(counters, derived, header, required):
    """Another counter of this loop that advances identically, or None.

    The offset case below is the general one and this is its degenerate
    form: two recurrences with the same start and the same step are the
    same value, and `offset` would be zero. It is not reachable through
    that path -- it looks for a start defined by `add source,const`, and
    two identical counters share a start rather than computing one from the
    other -- and nothing else merges them, because a phi is not one of the
    computations `transform`'s value numbering considers.

    BC writes one for every use of a subscript, so segld's inner loop
    advanced two offsets into `a` that were always equal. The cost is not
    only the extra `add`: `mir.same_bytes` keys a reference on its base
    *value*, so `a(i)` stored through one and read back through the other
    were two different cells and the reload could not be forwarded.

    Canonical is the lower id, so eliminating the higher one converges
    rather than swapping the pair back and forth across rounds.
    """
    from qbopt.optimize import transform

    if any(one.result.id == derived.value and (one.result, transform.HIGH) in required for one in header.phis):
        return None  # a long pair's high half is tied to this phi; the offset path owns that
    return next(
        (
            one
            for one in counters.values()
            if one.value < derived.value and one.start == derived.start and one.step == derived.step
        ),
        None,
    )


def _replacing(body, header, derived, twin, width):
    """`body` with `derived`'s phi replaced by a copy of `twin`'s."""
    from qbopt.optimize.strength import _made

    phi = next(one for one in header.phis if one.result.id == derived.value)
    root = next(one.result for one in header.phis if one.result.id == twin.value)
    operation = _made(mir.Kind.COPY, "mov", phi.result, (mir.Held(root, width),), header.at, header.ops[0])
    changed = replace(header, phis=tuple(one for one in header.phis if one is not phi), ops=(operation, *header.ops))
    return replace(body, blocks=tuple(changed if block is header else block for block in body.blocks))
