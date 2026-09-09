"""Recognize adjacent BC word pairs as whole scalar loads and arithmetic."""

from dataclasses import replace

from qbopt import ir, mir, pairs


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
                    and low.covers[1] == op.covers[0]
                    and low.stores[0].addr is not None
                    and replace(low.stores[0], addr=low.stores[0].addr.plus(2)) == op.stores[0]):
                    ref = replace(low.stores[0], width=4)
                    value = mir.Const(((op.args[0].n & 0xffff) << 16) | (low.args[0].n & 0xffff), 4)
                    ops[-1] = replace(low, args=(value,), results=(mir.Cell(ref),), stores=(ref,),
                                      uses=tuple(dict.fromkeys((*low.uses, *op.uses))), raised=None,
                                      covers=(low.covers[0], op.covers[1]))
                    continue
            ops.append(op)
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
                                          args=(lower,), results=(source,), covers=(low.at, low.at)))
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
            widened = replace(low, kind=kind, args=args, results=results, uses=uses,
                              defines=() if stores else (results[0].value,),
                              loads=() if stores or ref is None else (ref,), stores=(ref,) if stores else (),
                              merges={}, made=None, raised=None,
                              covers=(min(low.covers[0], high.covers[0]), max(low.covers[1], high.covers[1])))
            if pair.kind is pairs.Kind.ALU:
                loaded = fresh(low.at)
                ops.append(replace(widened, op=ir.Operation.MOVE, name="mov", kind=mir.Kind.LOAD,
                                   args=(mir.Cell(ref),), results=(loaded,), defines=(loaded.value,),
                                   uses=tuple(value for value in (ref.base, ref.segment) if value is not None),
                                   symbol=True))
                widened = replace(widened, args=(args[0], loaded), uses=(args[0].value, loaded.value),
                                  loads=(), node=None, id=None, symbol=False,
                                  covers=(low.at, low.at), extra_covers=())
            ops.append(widened)
            if not stores:
                whole[(high.results[0], low.results[0])] = results[0]
                for half, offset in ((low, 0), (high, 16)):
                    ops.append(mir.Op(high.at, mir.Synth.HALF_TO_LOW, "extract", (half.results[0].value,),
                                      (results[0].value,), kind=mir.Kind.EXTRACT,
                                      args=(results[0], mir.Const(offset, 4)), results=half.results,
                                      covers=(high.at, high.at)))
                    definitions[half.results[0].value] = ops[-1]
            dropped.update((id(low), id(high)))
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))
