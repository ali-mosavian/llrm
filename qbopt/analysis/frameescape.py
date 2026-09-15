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


def framed(body: mir.MirBody) -> dict[mir.Value, frozenset[tuple[int, int]]]:
    """Values that hold an address inside frame objects of known extent on every path.

    C's pointer arithmetic stays inside its object, so an address the body took
    of a local, moved by an integer, still reaches only that local's bytes.
    """
    ops = [op for block in body.blocks for op in block.ops]
    phis = [phi for block in body.blocks for phi in block.phis]
    defined = {phi.result for phi in phis} | {value for op in ops for value in op.defines}
    moving: dict[mir.Value, mir.Op] = {}
    refuted: set[mir.Value] = set()
    for op in ops:
        held = len(op.results) == 1 and isinstance(op.results[0], mir.Held) and op.results[0].width == 2
        pure = not (op.loads or op.stores or op.barrier)
        kinds = (mir.Kind.ADDRESS, mir.Kind.COPY, mir.Kind.ADD, mir.Kind.SUB)
        for value in op.defines:
            if held and pure and op.kind in kinds and value == op.results[0].value:
                moving[value] = op
            else:
                refuted.add(value)
    refuted |= {one for phi in phis for one in phi.incoming.values() if one not in defined}
    state: dict[mir.Value, frozenset] = {}

    def side(arg) -> str:
        if isinstance(arg, mir.Const) or isinstance(arg, mir.Held) and arg.value in refuted:
            return "number"
        if isinstance(arg, mir.Held) and arg.value in state:
            return "address"
        return "unknown"

    def moved(op: mir.Op, final: bool):
        """The extents `op` leaves its result in, None where it cannot, `...` while unknown."""
        match op.kind, op.args:
            case mir.Kind.ADDRESS, (mir.FrameAddress(extent=extent),) if extent is not None:
                return frozenset({extent})
            case mir.Kind.COPY, (mir.Held() as source,):
                kind = side(source)
            case mir.Kind.ADD, (left, right):
                sides = (side(left), side(right))
                if sides in (("address", "number"), ("number", "address")):
                    return state[(left if sides[0] == "address" else right).value]
                return None if "unknown" not in sides else ...
            case mir.Kind.SUB, (left, right):
                sides = (side(left), side(right))
                if sides == ("address", "number"):
                    return state[left.value]
                return None if "unknown" not in sides else ...
            case _:
                return None
        if kind == "address":
            return state[source.value]
        return None if kind == "number" else ...

    while True:
        changed = True
        while changed:
            changed = False
            for phi in phis:
                if phi.result in refuted:
                    continue
                if any(one in refuted for one in phi.incoming.values()):
                    refuted.add(phi.result)
                    changed = True
                    continue
                union = frozenset().union(*(state.get(one, frozenset()) for one in phi.incoming.values()))
                if union - state.get(phi.result, frozenset()):
                    state[phi.result] = state.get(phi.result, frozenset()) | union
                    changed = True
            for value, op in moving.items():
                if value in refuted:
                    continue
                got = moved(op, False)
                if got is None:
                    refuted.add(value)
                    changed = True
                elif got is not ... and got - state.get(value, frozenset()):
                    state[value] = state.get(value, frozenset()) | got
                    changed = True
        # Optimism settles cycles; anything still unproven is refuted and the rest looked at again.
        unproven = {
            value
            for value in (*moving, *(phi.result for phi in phis))
            if value not in refuted
            and (
                value not in state
                or value in moving
                and not isinstance(moved(moving[value], True), frozenset)
                or value not in moving
                and any(one not in state for phi in phis if phi.result == value for one in phi.incoming.values())
            )
        }
        if not unproven:
            return {value: extents for value, extents in state.items() if value not in refuted}
        refuted |= unproven
