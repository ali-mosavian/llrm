from dataclasses import dataclass

from qbopt.model import mir


@dataclass(frozen=True, slots=True)
class Escapes:
    origins: dict[mir.Value, frozenset[int]]
    exposed: frozenset[int]
    opaque_addresses: frozenset[int]
    # The bytes the exposed addresses reach, or None where one is not bounded to its object.
    reach: frozenset[tuple[int, int]] | None = frozenset()


def analysed(body: mir.MirBody) -> Escapes:
    # Origins are not allocation bounds. Absence here says nothing about
    # runtime frame walking, callbacks, or pointers loaded from memory.
    origins: dict[mir.Value, frozenset[int]] = {}
    operations = tuple(op for block in body.blocks for op in block.ops)
    phis = tuple(phi for block in body.blocks for phi in block.phis)

    def inputs(op: mir.Op) -> frozenset[int]:
        direct = frozenset(arg.offset for arg in op.args if isinstance(arg, mir.FrameAddress))
        values = set(op.uses) | {arg.value for arg in op.args if isinstance(arg, mir.Held)}
        values |= {value for ref in (*op.loads, *op.stores) for value in (ref.base, ref.segment) if value is not None}
        return direct.union(*(origins.get(value, frozenset()) for value in values))

    while True:
        changed = False
        for phi in phis:
            incoming = frozenset().union(*(origins.get(value, frozenset()) for value in phi.incoming.values()))
            previous = origins.get(phi.result, frozenset())
            if incoming - previous:
                origins[phi.result] = previous | incoming
                changed = True
        for op in operations:
            if op.kind not in (mir.Kind.COPY, mir.Kind.ADDRESS) or op.loads or op.stores or op.barrier:
                continue
            incoming = inputs(op)
            for value in op.defines:
                if value.flags:
                    continue
                previous = origins.get(value, frozenset())
                if incoming - previous:
                    origins[value] = previous | incoming
                    changed = True
        if not changed:
            break

    exposed = frozenset().union(
        *(
            inputs(op)
            for op in operations
            if op.kind not in (mir.Kind.COPY, mir.Kind.ADDRESS) or op.loads or op.stores or op.barrier
        )
    )
    opaque = frozenset(
        op.at
        for op in operations
        if op.kind is mir.Kind.ADDRESS and any(isinstance(arg, mir.Opaque) for arg in op.args)
    )
    extents: dict[int, set] = {}
    for op in operations:
        for arg in op.args:
            if isinstance(arg, mir.FrameAddress):
                extents.setdefault(arg.offset, set()).add(arg.extent)
    reach: set | None = set()
    for offset in exposed:
        found = extents.get(offset, {None})
        if None in found:
            reach = None
            break
        reach |= found
    return Escapes(origins, exposed, opaque, None if reach is None else frozenset(reach))
