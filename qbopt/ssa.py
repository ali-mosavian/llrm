from dataclasses import replace
from collections.abc import Iterator

from qbopt import mir
from qbopt.mir import Op
from qbopt.mir import MirBody


def constructed(body: MirBody, variables: frozenset[int]) -> MirBody:
    def owned(values: tuple[mir.Value, ...]) -> tuple[mir.Value, ...]:
        return tuple(one for one in values if one.variable in variables)

    skeleton = replace(
        body,
        origin={},
        pins={},
        blocks=tuple(
            replace(
                block,
                phis=(),
                ops=tuple(
                    replace(
                        op,
                        defines=owned(op.defines),
                        uses=owned(op.uses),
                        args=(),
                        results=(),
                        loads=(),
                        stores=(),
                        merges={},
                        raised=None,
                    )
                    for op in block.ops
                ),
            )
            for block in body.blocks
        ),
    )
    repaired = mir.resolved(skeleton)
    if isinstance(repaired, str):
        raise mir.Unraisable(repaired)

    def merge(op: Op, fixed: Op) -> Op:
        uses = {one.variable: one for one in fixed.uses}
        defines = {one.variable: one for one in fixed.defines}

        def operand(one: mir.Arg, names: dict[int, mir.Value]) -> mir.Arg:
            return (
                mir.Held(names[one.value.variable], one.width)
                if (isinstance(one, mir.Held) and one.value.variable in names)
                else one
            )

        return replace(
            op,
            uses=tuple(uses.get(one.variable, one) for one in op.uses),
            defines=tuple(defines.get(one.variable, one) for one in op.defines),
            args=tuple(operand(one, uses) for one in op.args),
            results=tuple(operand(one, defines) for one in op.results),
        )

    # The isolated renamer starts ids at zero; keep its namespace disjoint.
    offset = max((one.id for one in values(body)), default=0) + 1

    def shifted(value: mir.Value) -> mir.Value:
        return replace(value, id=value.id + offset)

    mapping = {one: shifted(one) for one in values(repaired)}
    repaired = replace(
        repaired,
        blocks=tuple(
            replace(
                block,
                phis=tuple(
                    mir.Phi(mapping[phi.result], {at: mapping[value] for at, value in phi.incoming.items()})
                    for phi in block.phis
                ),
                ops=tuple(
                    replace(
                        op,
                        uses=tuple(mapping[one] for one in op.uses),
                        defines=tuple(mapping[one] for one in op.defines),
                    )
                    for op in block.ops
                ),
            )
            for block in repaired.blocks
        ),
    )
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                phis=block.phis + fixed.phis,
                ops=tuple(merge(op, changed) for op, changed in zip(block.ops, fixed.ops, strict=True)),
            )
            for block, fixed in zip(body.blocks, repaired.blocks, strict=True)
        ),
    )


def values(body: MirBody) -> Iterator[mir.Value]:
    for block in body.blocks:
        for op in block.ops:
            yield from op.defines
            yield from op.uses
        for phi in block.phis:
            yield phi.result
            yield from phi.incoming.values()
