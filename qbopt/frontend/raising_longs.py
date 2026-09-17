"""Recognize adjacent BC word pairs as whole scalar loads and arithmetic."""

from dataclasses import replace

from qbopt.model import ir, mir
from qbopt.frontend import pairs


def sign_fills(body: mir.MirBody) -> mir.MirBody:
    """Expose the sign word as an extraction from a signed whole value."""
    values = set(body.origin)
    for block in body.blocks:
        values.update(value for op in block.ops for value in (*op.uses, *op.defines))
        values.update(phi.result for phi in block.phis)
        values.update(value for phi in block.phis for value in phi.incoming.values())
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            if (op.kind is not mir.Kind.CONVERT or op.op is not ir.Operation.EXTEND
                or op.name != "cwd" or op.loads or op.stores or op.barrier
                or len(op.args) != 1 or len(op.results) != 1
                or not isinstance(op.args[0], mir.Held) or op.args[0].width != 2
                or not isinstance(op.results[0], mir.Held) or op.results[0].width != 2
                or op.defines != (op.results[0].value,)):
                ops.append(op)
                continue
            serial += 1
            variable += 1
            whole = mir.Held(mir.Value(serial, op.at, variable=variable, version=1), 4)
            ops.append(mir.Op(op.at, ir.Operation.EXTEND, "sign_extend", (whole.value,),
                              (op.args[0].value,), kind=mir.Kind.SIGN_EXTEND,
                              args=op.args, results=(whole,)))
            ops.append(mir.detached(op, kind=mir.Kind.EXTRACT, op=mir.Synth.HALF_TO_LOW,
                               name="extract", args=(whole, mir.Const(16, 4)),
                               uses=(whole.value,), merges={}, raised=None))
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def arguments(body: mir.MirBody) -> mir.MirBody:
    """Rejoin high/low argument words extracted from the same whole value."""
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    blocks = []
    for block in body.blocks:
        ops = []
        for low in block.ops:
            high = ops[-1] if ops else None
            if (high is not None and mir.raising_adjacent(high, low)
                and all(op.kind is mir.Kind.ARG and not op.defines and not op.loads
                        and not op.barrier and not op.merges and op.stack is None
                        and len(op.args) == len(op.stores) == 1
                        and isinstance(op.args[0], mir.Held) and op.args[0].width == 2
                        and op.stores[0] == mir.MemRef(None, 2, space=mir.Space.STACK)
                        for op in (high, low))):
                source = mir.extracted_whole(high.args[0], low.args[0], definitions)
                if source is not None:
                    ops[-1] = mir.raising_owned(
                        mir.detached(high, args=(source,), uses=(source.value,),
                                     stores=(replace(high.stores[0], width=4),), raised=None),
                        high,
                        low,
                    )
                    continue
            ops.append(low)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def _negated_whole(high, low, definitions):
    """BC negates a long with NEG low, ADC high,0, NEG high."""
    if not all(isinstance(arg, mir.Held) and arg.width == 2 for arg in (high, low)):
        return None
    lower, upper = definitions.get(low.value), definitions.get(high.value)
    if any(op is None or op.kind is not mir.Kind.NEG or len(op.args) != 1
           or op.loads or op.stores or op.barrier or mir.partial(op) for op in (lower, upper)):
        return None
    if lower.results != (low,) or upper.results != (high,):
        return None
    carried = upper.args[0]
    if not isinstance(carried, mir.Held) or carried.width != 2:
        return None
    carry = definitions.get(carried.value)
    if (carry is None or carry.kind is not mir.Kind.ADD_CARRY or carry.loads or carry.stores
        or carry.barrier or mir.partial(carry) or carry.results != (carried,) or len(carry.args) != 2
        or carry.args[1] != mir.Const(0, 2)):
        return None
    flags = {value for value in lower.defines if value.flags}
    if len(flags) != 1 or {value for value in carry.uses if value.flags} != flags:
        return None
    return mir.extracted_whole(carry.args[0], lower.args[0], definitions)


def _constant_stores(body: mir.MirBody) -> mir.MirBody:
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            if ops:
                low = ops[-1]
                if (all(one.kind is mir.Kind.STORE and not one.barrier and not one.defines
                        and not one.loads and len(one.stores) == len(one.args) == 1
                        and isinstance(one.args[0], mir.Const) and one.args[0].width == 2
                        and one.stores[0].width == 2 for one in (low, op))
                    and mir.raising_adjacent(low, op)
                    and low.stores[0].addr is not None
                    and replace(low.stores[0], addr=low.stores[0].addr.plus(2)) == op.stores[0]):
                    ref = replace(low.stores[0], width=4)
                    value = mir.Const(((op.args[0].n & 0xffff) << 16) | (low.args[0].n & 0xffff), 4)
                    ops[-1] = mir.raising_owned(
                        replace(low, args=(value,), results=(mir.Cell(ref),), stores=(ref,),
                                uses=tuple(dict.fromkeys((*low.uses, *op.uses))), raised=None),
                        low,
                        op,
                    )
                    continue
            ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def unary(body: mir.MirBody) -> mir.MirBody:
    """Recognize whole NOT and NEG before their word results cross an ABI."""
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    values = set(body.origin) | set(definitions)
    values |= {value for block in body.blocks for op in block.ops for value in op.uses}
    values |= {phi.result for block in body.blocks for phi in block.phis}
    values |= {value for block in body.blocks for phi in block.phis for value in phi.incoming.values()}
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)
    users = {value: {id(op) for block in body.blocks for op in block.ops if value in op.uses}
             for value in values}
    phi_reads = {value for block in body.blocks for phi in block.phis for value in phi.incoming.values()}
    blocks = []
    for block in body.blocks:
        ops, index = [], 0
        while index < len(block.ops):
            low = block.ops[index]
            count = 2 if low.kind is mir.Kind.NOT else 3
            group = block.ops[index:index + count]
            source = None
            if (low.kind in (mir.Kind.NOT, mir.Kind.NEG) and len(group) == count
                and all(not op.loads and not op.stores and not op.barrier and not mir.partial(op)
                        and len(op.results) == 1 and isinstance(op.results[0], mir.Held)
                        and op.results[0].width == 2 for op in group)
                and all(mir.raising_adjacent(one, other) for one, other in zip(group, group[1:]))):
                high = group[-1]
                if low.kind is mir.Kind.NOT and high.kind is mir.Kind.NOT and len(low.args) == len(high.args) == 1:
                    source = mir.extracted_whole(high.args[0], low.args[0], definitions)
                elif low.kind is mir.Kind.NEG:
                    source = _negated_whole(high.results[0], low.results[0], definitions)
                internal = {id(op) for op in group}
                removed = {value for op in group for value in op.defines} - {low.results[0].value, high.results[0].value}
                if any(value in phi_reads or users.get(value, set()) - internal for value in removed):
                    source = None
            if source is None:
                ops.append(low)
                index += 1
                continue
            serial += 1
            variable += 1
            result = mir.Held(mir.Value(serial, low.at, variable=variable, version=1), 4)
            ops.append(mir.raising_owned(
                mir.Op(low.at, ir.Operation.UNARY, low.kind.value, (result.value,),
                       (source.value,), kind=low.kind, args=(source,), results=(result,)),
                *group,
            ))
            for half, offset in ((low, 0), (high, 16)):
                extract = mir.Op(high.at, mir.Synth.HALF_TO_LOW, "extract", (half.results[0].value,),
                                 (result.value,), kind=mir.Kind.EXTRACT,
                                 args=(result, mir.Const(offset, 4)), results=half.results)
                ops.append(extract)
                definitions[half.results[0].value] = extract
            index += count
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def scalar(body: mir.MirBody) -> mir.MirBody:
    body = _constant_stores(body)
    candidates = {id(pair.first): pair for pair in pairs.found(body)
                  if pair.kind in (pairs.Kind.LOAD, pairs.Kind.ALU, pairs.Kind.ALU_IMM, pairs.Kind.STORE)}
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    values = {value for block in body.blocks for op in block.ops for value in (*op.uses, *op.defines)} | set(body.origin)
    phi_reads = {value for block in body.blocks for phi in block.phis for value in phi.incoming.values()}
    values |= phi_reads | {phi.result for block in body.blocks for phi in block.phis}
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)
    readers = {value for block in body.blocks for op in block.ops for value in op.uses if value not in op.merges}
    readers |= phi_reads
    users = {value: {id(op) for block in body.blocks for op in block.ops if value in op.uses and value not in op.merges}
             for value in values if value.flags}
    wide_reads = {arg.value for block in body.blocks for op in block.ops for arg in op.args
                  if isinstance(arg, mir.Held) and arg.width > 2}

    def fresh(at):
        nonlocal serial, variable
        serial += 1
        variable += 1
        return mir.Held(mir.Value(serial, at, variable=variable, version=1), 4)

    blocks = []
    for block in body.blocks:
        whole, dropped, ops = {}, set(), []
        for op in block.ops:
            if id(op) in dropped:
                continue
            pair = candidates.get(id(op))
            if pair is None:
                ops.append(op)
                continue
            low, high = pair.low, pair.high
            stores = pair.kind is pairs.Kind.STORE
            immediate = pair.kind is pairs.Kind.ALU_IMM
            refs = (low.stores, high.stores) if stores else (low.loads, high.loads)
            if low.barrier or high.barrier:
                ops.append(op)
                continue
            if not immediate and (len(refs[0]) != 1 or len(refs[1]) != 1 or refs[0][0].addr is None
                or replace(refs[0][0], addr=refs[0][0].addr.plus(2)) != refs[1][0]
                ):
                ops.append(op)
                continue
            ref = None if immediate else replace(refs[0][0], width=4)
            if stores:
                source = whole.get((high.args[0], low.args[0])) if len(low.args) == len(high.args) == 1 else None
                if source is None and len(low.args) == len(high.args) == 1:
                    source = mir.extracted_whole(high.args[0], low.args[0], definitions)
                if source is None and len(low.args) == len(high.args) == 1:
                    original = _negated_whole(high.args[0], low.args[0], definitions)
                    if original is not None:
                        source = fresh(low.at)
                        ops.append(mir.Op(low.at, ir.Operation.UNARY, "neg", (source.value,),
                                          (original.value,), kind=mir.Kind.NEG,
                                          args=(original,), results=(source,)))
                if source is None and len(low.args) == len(high.args) == 1:
                    upper, lower = high.args[0], low.args[0]
                    extension = definitions.get(upper.value) if isinstance(upper, mir.Held) else None
                    if (isinstance(lower, mir.Held) and lower.width == 2
                        and extension is not None and extension.kind is mir.Kind.CONVERT
                        and extension.op is ir.Operation.EXTEND and extension.name == "cwd"
                        and extension.args == (lower,) and extension.results == (upper,)
                        and upper.width == 2 and not extension.loads and not extension.stores):
                        source = fresh(low.at)
                        ops.append(mir.Op(low.at, ir.Operation.EXTEND, "sign_extend", (source.value,),
                                          (lower.value,), kind=mir.Kind.SIGN_EXTEND,
                                          args=(lower,), results=(source,)))
                if source is None:
                    ops.append(op)
                    continue
                args, results, kind = (source,), (mir.Cell(ref),), mir.Kind.STORE
            else:
                if (len(low.results) != 1 or len(high.results) != 1
                    or any(not isinstance(result, mir.Held) or result.width != 2
                           or result.value in wide_reads for result in (*low.results, *high.results))
                    or any(value.flags and value in readers for value in high.defines)
                    or any(value.flags and (value in phi_reads or users.get(value, set()) - {id(high)})
                           for value in low.defines)):
                    ops.append(op)
                    continue
                if pair.kind is pairs.Kind.LOAD:
                    args, kind = (mir.Cell(ref),), mir.Kind.LOAD
                else:
                    source = whole.get((high.args[0], low.args[0])) or mir.extracted_whole(high.args[0], low.args[0], definitions)
                    if source is None or low.kind not in (mir.Kind.ADD, mir.Kind.SUB, mir.Kind.AND, mir.Kind.OR, mir.Kind.XOR):
                        ops.append(op)
                        continue
                    if immediate:
                        upper, lower = high.args[-1], low.args[-1]
                        if not all(isinstance(arg, mir.Const) and arg.width == 2 for arg in (upper, lower)):
                            ops.append(op)
                            continue
                        operand = mir.Const(((upper.n & 0xffff) << 16) | (lower.n & 0xffff), 4)
                    else:
                        operand = mir.Cell(ref)
                    args, kind = (source, operand), low.kind
                results = (fresh(low.at),)
            uses = tuple(dict.fromkeys(
                [arg.value for arg in args if isinstance(arg, mir.Held)]
                + ([value for value in (ref.base, ref.segment) if value is not None] if ref else [])))
            widened = mir.raising_owned(
                replace(low, kind=kind, args=args, results=results, uses=uses,
                        defines=() if stores else (results[0].value,),
                        loads=() if stores or ref is None else (ref,), stores=(ref,) if stores else (),
                        merges={}, raised=None),
                low,
                high,
            )
            if pair.kind is pairs.Kind.ALU:
                loaded = fresh(low.at)
                ops.append(replace(widened, op=ir.Operation.MOVE, name="mov", kind=mir.Kind.LOAD,
                                   args=(mir.Cell(ref),), results=(loaded,), defines=(loaded.value,),
                                   uses=tuple(value for value in (ref.base, ref.segment) if value is not None),
                                   symbol=True))
                widened = mir.source_free(widened, args=(args[0], loaded),
                                          uses=(args[0].value, loaded.value), loads=(), id=None, symbol=False)
            ops.append(widened)
            if not stores:
                whole[(high.results[0], low.results[0])] = results[0]
                for half, offset in ((low, 0), (high, 16)):
                    ops.append(mir.Op(high.at, mir.Synth.HALF_TO_LOW, "extract", (half.results[0].value,),
                                      (results[0].value,), kind=mir.Kind.EXTRACT,
                                      args=(results[0], mir.Const(offset, 4)), results=half.results))
                    definitions[half.results[0].value] = ops[-1]
            dropped.update((id(low), id(high)))
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))
