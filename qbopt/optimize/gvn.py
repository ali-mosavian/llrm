"""Reuse scalar expressions at joins, completing availability on dedicated edges."""

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
    uses = tuple(incoming.get(value, value) for value in op.uses)
    return None if any(value is None for value in uses) else replace(op, args=tuple(args), uses=uses)


def _insertion(op, parent, join, prefix, definitions, dominators, natural_loops):
    """Insert only on an unconditional edge, with available scalar inputs."""
    if (parent.succ != (join.at,) or join.at in dominators[parent.at]
        or any((parent.at in loop.body) != (join.at in loop.body) for loop in natural_loops)
        or any(value.flags for prior in prefix for value in prior.uses)
        or any(not isinstance(arg, (mir.Held, mir.Const)) for arg in op.args)
        or any(value.flags for value in op.uses)
        or any(value not in {arg.value for arg in op.args if isinstance(arg, mir.Held)} for value in op.uses)):
        return None
    cut = len(parent.ops)
    if cut and parent.ops[-1].kind is mir.Kind.JUMP:
        cut -= 1
    if any(one.kind in {mir.Kind.BRANCH, mir.Kind.RETURN, mir.Kind.ESCAPE} for one in parent.ops):
        return None
    for arg in op.args:
        if not isinstance(arg, mir.Held):
            continue
        definition = definitions.get(arg.value)
        if definition is None:
            return None
        at, index = definition
        if at not in dominators[parent.at] or (at == parent.at and index >= cut):
            return None
    return cut


def joined(body: mir.MirBody, *, insert: bool = True) -> mir.MirBody:
    """Eliminate scalar redundancy without adding execution to any path.

    A phi combines independently dominating providers. Missing providers may
    be inserted on unconditional incoming edges, but only when another edge
    already supplies the result. Memory and floating expressions stay out.
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
    definitions = {}
    by_at = {block.at: block for block in body.blocks}
    values = set()
    for block in body.blocks:
        definitions.update((phi.result, (block.at, -1)) for phi in block.phis)
        values.update(value for phi in block.phis for value in (phi.result, *phi.incoming.values()))
        for index, op in enumerate(block.ops):
            definitions.update((value, (block.at, index)) for value in op.defines)
            values.update((*op.defines, *op.uses))
            if (expression := key(op)) is not None:
                expressions.setdefault(expression, []).append((block.at, index, op.results[0].value))

    changed = False
    fresh = max((value.id for value in values), default=0) + 1
    replacements = {}
    insertions = {}
    blocks = []
    for block in body.blocks:
        parents = predecessors[block.at]
        if block.at == body.entry or len(parents) < 2:
            blocks.append(block)
            continue
        phis, ops = list(block.phis), []
        for index, op in enumerate(block.ops):
            expression = key(op)
            incoming = {}
            missing = {}
            if expression is not None and not any(value.flags and value in live for value in op.defines):
                for parent in sorted(parents):
                    translated = _on_edge(ssa.substituted(op, replacements), tuple(phis), parent)
                    edge_expression = key(translated) if translated is not None else None
                    candidates = [(at, index, value) for at, index, value in expressions.get(edge_expression, ())
                                  if at != block.at and at in dominators[parent]
                                  and block.at not in dominators[at]
                                  and all(at not in loop.body or block.at in loop.body for loop in natural_loops)]
                    if not candidates:
                        if not insert:
                            break
                        cut = (_insertion(translated, by_at[parent], block, block.ops[:index],
                                          definitions, dominators, natural_loops)
                               if edge_expression is not None else None)
                        if cut is None:
                            break
                        missing[parent] = (cut, translated)
                        continue
                    _, _, value = max(candidates, key=lambda item: (len(dominators[item[0]]), item[1]))
                    incoming[parent] = value
            if not incoming or len(incoming) + len(missing) != len(parents):
                ops.append(op)
                continue
            for parent, (cut, translated) in missing.items():
                predecessor = by_at[parent]
                # The predecessor owns this occurrence; its end may be the successor's branch label.
                at = predecessor.ops[min(cut, len(predecessor.ops) - 1)].at if predecessor.ops else predecessor.at
                value = mir.Value(fresh, at)
                fresh += 1
                made = mir.Op(at, op.op, "", (value,),
                              tuple(dict.fromkeys(arg.value for arg in translated.args if isinstance(arg, mir.Held))),
                              kind=op.kind, args=translated.args,
                              results=(mir.Held(value, op.results[0].width),), covers=(at, at))
                insertions.setdefault(parent, []).append((cut, made))
                incoming[parent] = value
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
    for index, block in enumerate(blocks):
        ops = list(block.ops)
        for cut, made in sorted(insertions.get(block.at, ()), key=lambda item: item[0], reverse=True):
            ops.insert(cut, made)
        blocks[index] = replace(block, ops=tuple(ops))
    return replace(body, blocks=tuple(replace(block,
        phis=tuple(replace(phi, incoming={at: ssa.provider(value, replacements)
                                          for at, value in phi.incoming.items()}) for phi in block.phis),
        ops=tuple(ssa.substituted(op, replacements) for op in block.ops)) for block in blocks))
