"""Reuse scalar expressions available along every incoming edge of a join."""

from dataclasses import replace

from qbopt.analysis import loops, ssa
from qbopt.model import ir, mir


def _on_edge(op: mir.Op, phis: tuple[mir.Phi, ...], predecessor: int) -> mir.Op | None:
    """Translate simultaneously: an incoming phi value belongs to the prior edge, not another substitution."""
    incoming = {phi.result: phi.incoming.get(predecessor) for phi in phis}
    args = []
    for arg in op.args:
        if isinstance(arg, mir.Held) and arg.value in incoming:
            value = incoming[arg.value]
            if value is None:
                return None
            arg = replace(arg, value=value)
        args.append(arg)
    return replace(op, args=tuple(args))


def joined(body: mir.MirBody) -> mir.MirBody:
    """Eliminate full redundancy at joins without inserting or speculating work.

    Each predecessor must have an independently dominating provider. A phi
    combines those existing results; lowering and allocation place any moves.
    Memory, floating environments and partially redundant expressions need
    their own availability and profitability proofs.
    """
    from qbopt.optimize import transform

    predecessors = loops.predecessors(body.blocks)
    if not any(len(parents) > 1 for parents in predecessors.values()):
        return body
    dominators = loops.dominators(body.blocks, body.entry)
    natural_loops = loops.loops(body.blocks, body.entry)
    widths = transform._widths(body)
    live = transform.live(body)
    allowed = transform._PURE - {mir.Kind.DIV, mir.Kind.REM, mir.Kind.CONVERT, mir.Kind.COPY}

    def key(op):
        if (op.kind not in allowed or op.floating is not None or op.loads or op.stores
            or op.stack is not None or op.merges or op.barrier
            or len(op.results) != 1 or not isinstance(op.results[0], mir.Held)
            or any(value != op.results[0].value and not value.flags for value in op.defines)):
            return None
        expression = transform._computation(op, {}, widths)
        return (expression[0], *expression[2:]) if expression is not None else None

    expressions = {}
    values = set()
    for block in body.blocks:
        values.update(value for phi in block.phis for value in (phi.result, *phi.incoming.values()))
        for index, op in enumerate(block.ops):
            values.update((*op.defines, *op.uses))
            if (expression := key(op)) is not None:
                expressions.setdefault(expression, []).append((block.at, index, op.results[0].value))

    changed = False
    fresh = max((value.id for value in values), default=0) + 1
    replacements = {}
    blocks = []
    for block in body.blocks:
        parents = predecessors[block.at]
        if block.at == body.entry or len(parents) < 2:
            blocks.append(block)
            continue
        phis, ops = list(block.phis), []
        for op in block.ops:
            expression = key(op)
            incoming = {}
            if expression is not None and not any(value.flags and value in live for value in op.defines):
                for parent in sorted(parents):
                    translated = _on_edge(ssa.substituted(op, replacements), tuple(phis), parent)
                    edge_expression = key(translated) if translated is not None else None
                    candidates = [(at, index, value) for at, index, value in expressions.get(edge_expression, ())
                                  if at != block.at and at in dominators[parent]
                                  and block.at not in dominators[at]
                                  and all(at not in loop.body or block.at in loop.body for loop in natural_loops)]
                    if not candidates:
                        break
                    _, _, value = max(candidates, key=lambda item: (len(dominators[item[0]]), item[1]))
                    incoming[parent] = value
            if len(incoming) != len(parents):
                ops.append(op)
                continue
            result = mir.Value(fresh, block.at)
            fresh += 1
            replacements[op.results[0].value.id] = result
            phis.append(mir.Phi(result, incoming))
            ops.append(replace(op, op=ir.Operation.NOTHING, kind=mir.Kind.NOTHING, name="",
                               args=(), results=(), defines=(), uses=(), node=None, made=None, raised=None))
            changed = True
        blocks.append(replace(block, phis=tuple(phis), ops=tuple(ops)))
    if not changed:
        return body
    return replace(body, blocks=tuple(replace(block,
        phis=tuple(replace(phi, incoming={at: ssa.provider(value, replacements)
                                          for at, value in phi.incoming.items()}) for phi in block.phis),
        ops=tuple(ssa.substituted(op, replacements) for op in block.ops)) for block in blocks))
