from dataclasses import replace

from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.analysis import loops
from qbopt.analysis import liveness


def sunk(body: mir.MirBody) -> mir.MirBody:
    from qbopt.optimize import strength
    from qbopt.optimize import transform

    if any(
        (op.barrier or op.kind is mir.Kind.OPAQUE) and not op.reads_complete
        for block in body.blocks
        for op in block.ops
    ):
        return body
    definitions = {value: (block.at, op) for block in body.blocks for op in block.ops for value in op.defines}
    live = transform.live(body)
    live_in = liveness.live(body).live_in
    all_values = tuple(ssa.values(body))
    serial = max((value.id for value in all_values), default=0) + 1
    variable = max((value.variable for value in all_values), default=0) + 1
    for loop in loops.loops(body.blocks, body.entry):
        for block in body.blocks:
            if block.at in loop.body or not block.ops or any(value.flags for value in live_in.get(block.at, ())):
                continue
            for phi in block.phis:
                if len(phi.incoming) != 1:
                    continue
                predecessor, value = next(iter(phi.incoming.items()))
                if predecessor not in loop.body or value not in definitions:
                    continue
                where, op = definitions[value]
                if (
                    where not in loop.body
                    or op.kind not in {mir.Kind.ADD, mir.Kind.SUB}
                    or op.barrier
                    or op.loads
                    or op.stores
                    or op.stack is not None
                    or op.floating is not None
                    or len(op.results) != 1
                    or not isinstance(op.results[0], mir.Held)
                    or any(other in live for other in op.defines if other != value)
                ):
                    continue
                if any(value in other.uses for one in body.blocks for other in one.ops):
                    continue
                if any(
                    value in other.incoming.values() for one in body.blocks for other in one.phis if other is not phi
                ):
                    continue
                if any(not isinstance(arg, (mir.Held, mir.Const)) for arg in op.args):
                    continue
                exported: dict[mir.Value, mir.Value] = {}
                for source in (*op.uses, *op.merges):
                    if source not in exported:
                        exported[source] = mir.Value(serial, block.at, variable=variable)
                        serial += 1
                        variable += 1
                args = tuple(
                    mir.Held(exported[arg.value], arg.width) if isinstance(arg, mir.Held) else arg for arg in op.args
                )
                moved = strength._made(op.kind, "", value, args, block.at, op)
                moved = replace(
                    moved, merges={exported[source]: value for source in op.merges}, uses=tuple(exported.values())
                )
                exports = tuple(mir.Phi(result, {predecessor: source}) for source, result in exported.items())
                changed = replace(
                    body,
                    blocks=tuple(
                        replace(
                            one,
                            phis=tuple(other for other in one.phis if other is not phi) + exports,
                            ops=(moved, *one.ops),
                        )
                        if one is block
                        else replace(
                            one,
                            ops=tuple(transform._empty_operation(other) if other is op else other for other in one.ops),
                        )
                        for one in body.blocks
                    ),
                )
                return replace(
                    changed,
                    blocks=tuple(
                        replace(
                            one,
                            ops=tuple(ssa.substituted(other, {phi.result.id: value}) for other in one.ops),
                            phis=tuple(
                                replace(
                                    other,
                                    incoming={
                                        edge: value if source == phi.result else source
                                        for edge, source in other.incoming.items()
                                    },
                                )
                                for other in one.phis
                            ),
                        )
                        for one in changed.blocks
                    ),
                )
    return body
