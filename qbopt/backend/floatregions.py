"""Owned extended-precision storage between independently allocated x87 regions."""

from collections import defaultdict
from dataclasses import replace

from qbopt.analysis import loops
from qbopt.model import ir, lir


def bridged(body: lir.LirBody, regions: dict[int, int], frame) -> lir.LirBody:
    from qbopt.backend.lower import Unlowered

    definitions, readers = defaultdict(list), defaultdict(list)
    widths = defaultdict(set)
    identifiers = set(body.origin) | set(body.pins)
    for block in body.blocks:
        for phi in block.phis:
            identifiers.update((phi.result, *(value for _, value in phi.incoming)))
        for index, one in enumerate(block.insns):
            identifiers.update((*one.defines, *one.uses))
            if one.what is None:
                continue
            for operands, locations in ((one.what.dests, definitions), (one.what.sources, readers)):
                for arg in operands:
                    if isinstance(arg, ir.Held):
                        identifiers.add(arg.value)
                        widths[arg.value].add(arg.width)
                        if arg.width == 10:
                            locations[arg.value].append((block.at, index))
    floating = set(definitions) | set(readers)
    phis = [(block, phi) for block in body.blocks for phi in block.phis]
    while True:
        expanded = floating | {value for _, phi in phis
            if floating.intersection((phi.result, *(value for _, value in phi.incoming)))
            for value in (phi.result, *(value for _, value in phi.incoming))}
        if expanded == floating:
            break
        floating = expanded
    phis = [(block, phi) for block, phi in phis if phi.result in floating]
    at_of = {block.at: block for block in body.blocks}
    predecessors = loops.predecessors(body.blocks)
    for block, phi in phis:
        if (block.at == body.entry or not phi.incoming
            or len(phi.incoming) != len(predecessors[block.at])
            or {where for where, _ in phi.incoming} != set(predecessors[block.at])):
            raise Unlowered("floating phi does not cover its incoming edges")
        definitions[phi.result].append((block.at, -1))
        for where, value in phi.incoming:
            if where not in at_of:
                raise Unlowered("floating phi has an external predecessor")
            readers[value].append((where, len(at_of[where].insns)))
    crossing = {value for _, phi in phis for value in (phi.result, *(value for _, value in phi.incoming))}
    crossing |= {value for value, uses in readers.items() if value in definitions
                and any(regions[at] != regions[definitions[value][0][0]] for at, _ in uses)}
    if not crossing:
        return body
    if frame is None:
        raise Unlowered("floating region crossing requires an owned frame")
    doms = loops.dominators(body.blocks, body.entry)
    for value in crossing:
        if len(definitions[value]) != 1 or value in body.pins or widths[value] - {10}:
            raise Unlowered("floating region crossing requires an unpinned SSA definition")
        defined_at, defined_index = definitions[value][0]
        for at, index in readers[value]:
            if defined_at not in doms[at] or at == defined_at and index <= defined_index:
                raise Unlowered("floating region input is not dominated by its definition")
    cells = {value: frame.cell(("floating-region", value), 10) for value in sorted(crossing)}
    fresh = max(identifiers, default=0) + 1
    blocks = []
    for block in body.blocks:
        insns = []
        resident = {}
        for one in block.insns:
            what = one.what
            if (what is None or what.op in (ir.Operation.CALL, ir.Operation.BARRIER)
                or any(isinstance(arg, ir.St) for arg in (*what.sources, *what.dests))):
                resident.clear()
            explicit = set() if what is None else {
                arg.value for arg in (*what.sources, *what.dests)
                if isinstance(arg, ir.Held) and arg.width == 10}
            if crossing.intersection((*one.uses, *one.defines)) - explicit:
                raise Unlowered("floating region value has an unmodelled use")
            if any(held.value in crossing for held, _ in (*one.requires, *one.delivers)):
                raise Unlowered("floating region value has an integer register constraint")
            if what is None:
                insns.append(one)
                continue
            renamed = {}
            for arg in what.sources:
                if isinstance(arg, ir.Held) and arg.value in crossing and arg.value not in renamed:
                    local = resident.get(arg.value)
                    if local is None:
                        local = ir.Held(fresh, 10)
                        fresh += 1
                        resident[arg.value] = local
                        insns.append(lir.Insn(one.at, (one.at, one.at),
                            ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (local,), (cells[arg.value],)),
                            (local.value,), ()))
                    renamed[arg.value] = local
            insns.append(replace(one, what=replace(what, sources=tuple(
                renamed.get(arg.value, arg) if isinstance(arg, ir.Held) else arg for arg in what.sources)),
                uses=tuple(renamed[value].value if value in renamed else value for value in one.uses),
                widths=tuple((renamed[value].value if value in renamed else value, width)
                             for value, width in one.widths)))
            for arg in what.dests:
                if isinstance(arg, ir.Held) and arg.value in crossing:
                    insns.append(lir.Insn(one.at, (one.at, one.at),
                        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cells[arg.value],), (arg,)),
                        (), (arg.value,)))
        blocks.append(replace(block, insns=tuple(insns),
                              phis=tuple(phi for phi in block.phis if phi.result not in floating)))
    transfers = defaultdict(list)
    for block, phi in phis:
        for where, value in phi.incoming:
            transfers[where, block.at].append((phi.result, value))
    selected = {}
    for edge, pairs in transfers.items():
        where, _ = edge
        at = at_of[where].insns[-1].at if at_of[where].insns else where
        loads, stores = [], []
        for result, value in pairs:
            local = ir.Held(fresh, 10)
            fresh += 1
            loads.append(lir.Insn(at, (at, at),
                ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (local,), (cells[value],)), (local.value,), ()))
            stores.append(lir.Insn(at, (at, at),
                ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cells[result],), (local,)), (), (local.value,)))
        selected[edge] = loads + stores
    from qbopt.backend.phielim import placed_on_edges
    return placed_on_edges(replace(body, blocks=tuple(blocks)), selected)
