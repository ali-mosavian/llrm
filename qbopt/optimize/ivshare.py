from dataclasses import replace

from qbopt.model import mir
from qbopt.analysis import loops
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
            if not isinstance(derived.start, mir.Held) or not isinstance(derived.step, mir.Const):
                continue
            seed = definitions.get(derived.start.value.id)
            if seed is None or seed.kind is not mir.Kind.ADD or seed.loads or seed.stores or seed.barrier:
                continue
            match seed.args:
                case (mir.Held() as source, mir.Const() as offset):
                    pass
                case (mir.Const() as offset, mir.Held() as source):
                    pass
                case _:
                    continue
            width = derived.start.width
            if source.width != width or offset.width != width:
                continue
            base = next(
                (
                    one
                    for one in counters.values()
                    if one.value != derived.value and one.start == source and one.step == derived.step
                ),
                None,
            )
            if base is None:
                continue
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
            operation = _made(mir.Kind.ADD, "add", phi.result, (mir.Held(root, width), offset), header.at, seed)
            if upper is not None:
                operation = replace(operation, merges={upper: phi.result}, uses=(*operation.uses, upper))
            changed = replace(
                header, phis=tuple(one for one in header.phis if one is not phi), ops=(operation, *header.ops)
            )
            return replace(body, blocks=tuple(changed if block is header else block for block in body.blocks))
    return body


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
