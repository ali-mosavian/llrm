"""Join corresponding word phis using proven whole values on every edge."""

from dataclasses import replace

from qbopt.analysis import consts, loops, ssa
from qbopt.model import ir, mir


def _source(arg, definitions):
    seen = set()
    while isinstance(arg, mir.Held) and arg.value not in seen:
        seen.add(arg.value)
        op = definitions.get(arg.value)
        if (op is None or op.kind is not mir.Kind.COPY or op.loads or op.stores or op.barrier
            or op.results != (arg,) or len(op.args) != 1
            or not isinstance(op.args[0], mir.Held) or op.args[0].width != arg.width):
            break
        arg = op.args[0]
    return arg


def joined(body: mir.MirBody) -> mir.MirBody:
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    facts = consts.known(body)
    predecessors = loops.predecessors(body.blocks)
    blocks = {block.at: block for block in body.blocks}
    values = tuple(ssa.values(body))
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)
    additions, replacements, new_phis, extracts, removed = {}, {}, {}, {}, set()

    def fresh(at):
        nonlocal serial
        serial += 1
        return mir.Value(serial, at, variable=variable, version=serial)

    for block in body.blocks:
        phis = {phi.result: phi for phi in block.phis}
        for op in block.ops:
            if (op.kind is not mir.Kind.CONCAT or op.loads or op.stores or op.barrier
                or len(op.args) != 2 or len(op.results) != 1
                or not isinstance(op.results[0], mir.Held) or op.results[0].width != 4
                or op.defines != (op.results[0].value,)):
                continue
            args = tuple(_source(arg, definitions) for arg in op.args)
            if any(not isinstance(arg, mir.Held) or arg.width != 2 or arg.value not in phis for arg in args):
                continue
            high, low = (phis[arg.value] for arg in args)
            if (not high.incoming or high.incoming.keys() != low.incoming.keys()
                or set(high.incoming) != set(predecessors.get(block.at, ()))):
                continue
            sources = {}
            for at in high.incoming:
                upper, lower = high.incoming[at], low.incoming[at]
                whole = mir.extracted_whole(mir.Held(upper, 2), mir.Held(lower, 2), definitions)
                if whole is None:
                    upper_fact, lower_fact = facts.get(upper), facts.get(lower)
                    if (upper_fact is None or lower_fact is None
                        or upper_fact.width != 2 or lower_fact.width != 2):
                        break
                    whole = mir.Const(((upper_fact.n & 0xffff) << 16) | (lower_fact.n & 0xffff), 4)
                if at not in blocks or not blocks[at].ops:
                    break
                sources[at] = whole
            if len(sources) != len(high.incoming):
                continue
            variable += 1
            incoming = {}
            for at, source in sources.items():
                position = blocks[at].ops[-1].at
                value = fresh(position)
                incoming[at] = value
                additions.setdefault(at, []).append(mir.Op(
                    position, ir.Operation.MOVE, "mov", (value,),
                    (source.value,) if isinstance(source, mir.Held) else (),
                    kind=mir.Kind.COPY, args=(source,), results=(mir.Held(value, 4),),
                    covers=(position, position)))
            result = fresh(block.at)
            new_phis.setdefault(block.at, []).append(mir.Phi(result, incoming))
            for phi, offset in ((high, 16), (low, 0)):
                removed.add(phi.result)
                phis.pop(phi.result, None)
                extracts.setdefault(block.at, []).append(mir.Op(
                    block.at, mir.Synth.HALF_TO_LOW, "extract", (phi.result,), (result,),
                    kind=mir.Kind.EXTRACT, args=(mir.Held(result, 4), mir.Const(offset, 4)),
                    results=(mir.Held(phi.result, 2),), covers=(block.at, block.at)))
            replacements[id(op)] = replace(op, kind=mir.Kind.COPY, args=(mir.Held(result, 4),),
                                            uses=(result,), merges={}, source_backed=False, raised=None)
    if not replacements:
        return body
    changed = []
    for block in body.blocks:
        ops = [replacements.get(id(op), op) for op in block.ops]
        position = len(ops) - int(bool(ops) and ops[-1].kind in
                                  (mir.Kind.BRANCH, mir.Kind.JUMP, mir.Kind.RETURN))
        ops[position:position] = additions.get(block.at, ())
        ops[:0] = extracts.get(block.at, ())
        changed.append(replace(block, ops=tuple(ops),
                               phis=(*(phi for phi in block.phis if phi.result not in removed),
                                     *new_phis.get(block.at, ()))))
    return replace(body, blocks=tuple(changed))
