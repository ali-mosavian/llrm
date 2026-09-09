"""Scalarize a string copy when its addresses and traversal are explicit.

An unknown direction or selector remains unmodelled. Each element is loaded
before it is stored, including overlapping copies; pointer results preserve
their high halves. Nothing about those machine requirements reaches a pass.
"""

from dataclasses import replace

from iced_x86 import Code, Mnemonic, OpKind, Register, RflagsBits

from qbopt.analysis import ssa
from qbopt.model import ir, mir
from qbopt.objectfile import omf
from qbopt.objectfile.module import Addr, Space


def scalar(body: mir.MirBody, found) -> mir.MirBody:
    extents = {index: segment[1] for index, segment in enumerate(omf.segments(found.records)) if segment}
    symbols = {op.results[0].value: op.args[0]
               for block in body.blocks for op in block.ops
               if op.kind is mir.Kind.COPY and not op.barrier
               and len(op.results) == len(op.args) == 1
               and isinstance(op.results[0], mir.Held) and isinstance(op.args[0], mir.Symbol)}
    values = set(body.origin) | set(ssa.values(body))
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    candidates, ancestors = set(), {}
    blocks = []
    for block in body.blocks:
        direction, same_segment, pushed_data = None, False, False
        data_segment = block.at == body.entry
        ops = []
        for op in block.ops:
            node = op.node
            decoded = getattr(node, "insn", None)
            insn = decoded.insn if decoded is not None else None
            if insn is None:
                direction, same_segment, pushed_data, data_segment = None, False, False, False
                ops.append(op)
                continue
            if (insn.code == Code.MOVSW_M16_M16 and insn.op1_kind == OpKind.MEMORY_SEG_SI
                and insn.memory_segment == Register.DS and not insn.has_rep_prefix
                and not insn.has_repne_prefix and direction is not None and same_segment and data_segment):
                pointers = _pointers(op, body.origin, symbols, found.dgroup, extents, 2 * direction)
                if pointers is not None:
                    serial += 1
                    variable += 1
                    temporary = mir.Value(serial, op.at, variable=variable, version=1)
                    source, dest = (mir.MemRef(Addr(symbol.space, symbol.offset, symbol.index), 2)
                                    for _, _, symbol in pointers)
                    held = mir.Held(temporary, 2)
                    ops.append(mir.Op(op.at, ir.Operation.MOVE, "mov", (temporary,), (),
                                      kind=mir.Kind.LOAD, loads=(source,), args=(mir.Cell(source),),
                                      results=(held,), covers=op.covers, id=next(mir._IDS)))
                    ops.append(mir.Op(op.at, ir.Operation.MOVE, "mov", (), (temporary,),
                                      kind=mir.Kind.STORE, stores=(dest,), args=(held,),
                                      results=(mir.Cell(dest),), covers=(op.at, op.at), id=next(mir._IDS)))
                    for before, after, symbol in pointers:
                        advanced = replace(symbol, offset=symbol.offset + 2 * direction)
                        symbols[after] = advanced
                        setup = definitions.get(before)
                        if (setup is not None and setup.id is not None and setup.kind is mir.Kind.COPY and not setup.loads
                            and not setup.stores and not setup.extra_covers and setup.defines == (before,)
                            and len(setup.args) == 1 and isinstance(setup.args[0], mir.Symbol)):
                            candidates.add(setup.id)
                        before = ancestors.get(before, before)
                        ancestors[after] = before
                        identity = next(mir._IDS)
                        candidates.add(identity)
                        ops.append(mir.Op(op.at, ir.Operation.MOVE, "mov", (after,), (before,),
                                          kind=mir.Kind.COPY, args=(advanced,), results=(mir.Held(after, 2),),
                                          merges={before: after}, covers=(op.at, op.at), id=identity))
                    pushed_data = False
                    continue
            if insn.mnemonic in (Mnemonic.CLD, Mnemonic.STD):
                direction = 1 if insn.mnemonic == Mnemonic.CLD else -1
            elif decoded.flow in ir.CLOBBERS or insn.rflags_modified & RflagsBits.DF:
                direction = None
            if node.effects.defs is None or Register.DS in node.effects.defs:
                data_segment = False
            if (insn.mnemonic == Mnemonic.POP and insn.op0_register == Register.ES and pushed_data):
                same_segment = True
            elif (node.effects.defs is None or Register.DS in node.effects.defs
                  or Register.ES in node.effects.defs):
                same_segment = False
            pushed_data = insn.mnemonic == Mnemonic.PUSH and insn.op0_register == Register.DS
            ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    return _observed(replace(body, blocks=tuple(blocks)), candidates)


def _observed(body, candidates):
    if not candidates:
        return body
    definitions = {op.defines[0]: op for block in body.blocks for op in block.ops if op.id in candidates}
    wanted = {value for block in body.blocks for phi in block.phis for value in phi.incoming.values()}
    for block in body.blocks:
        for op in block.ops:
            if op.id not in candidates:
                wanted.update(op.uses)
                wanted.update(arg.value for arg in op.args if isinstance(arg, mir.Held))
                wanted.update(value for ref in (*op.loads, *op.stores)
                              for value in (ref.base, ref.segment) if value is not None)
    pending = list(wanted)
    while pending:
        op = definitions.get(pending.pop())
        if op is not None:
            unseen = set(op.uses) - wanted
            wanted.update(unseen)
            pending.extend(unseen)
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            if op.id in candidates and op.defines[0] not in wanted:
                if op.covers is not None and op.covers[0] != op.covers[1]:
                    ops.append(mir.Op(op.at, ir.Operation.NOTHING, "", (), (),
                                      kind=mir.Kind.NOTHING, covers=op.covers, id=op.id))
            else:
                ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def _pointers(op, origin, symbols, dgroup, extents, step):
    pointers = []
    for register in (Register.ESI, Register.EDI):
        reads = [value for value in op.uses if origin.get(value) == register]
        writes = [value for value in op.defines if origin.get(value) == register]
        if len(reads) != 1 or len(writes) != 1:
            return None
        symbol = symbols.get(reads[0])
        if (symbol is None or symbol.space is not Space.SEGMENT or symbol.index not in dgroup
            or symbol.width != 2 or symbol.addend != 0
            or not 0 <= symbol.offset <= extents.get(symbol.index, 0) - 2
            or not 0 <= symbol.offset + step < min(extents.get(symbol.index, 0), 0x10000)):
            return None
        pointers.append((reads[0], writes[0], symbol))
    return pointers
