"""Owned extended-precision storage between independently allocated x87 regions."""

from collections import defaultdict
from dataclasses import replace

from qbopt.analysis import loops
from qbopt.model import ir, lir


def bridged(body: lir.LirBody, regions: dict[int, int], frame) -> lir.LirBody:
    from qbopt.backend.lower import Unlowered

    definitions, readers = defaultdict(list), defaultdict(list)
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
                        if arg.width == 10:
                            locations[arg.value].append((block.at, index))
    crossing = {value for value, uses in readers.items() if value in definitions
                and any(regions[at] != regions[definitions[value][0][0]] for at, _ in uses)}
    if not crossing:
        return body
    if frame is None:
        raise Unlowered("floating region crossing requires an owned frame")
    doms = loops.dominators(body.blocks, body.entry)
    for value in crossing:
        if len(definitions[value]) != 1 or value in body.pins:
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
        for one in block.insns:
            what = one.what
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
                    local = ir.Held(fresh, 10)
                    fresh += 1
                    renamed[arg.value] = local
                    insns.append(lir.Insn(one.at, (one.at, one.at),
                        ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (local,), (cells[arg.value],)),
                        (local.value,), ()))
            insns.append(replace(one, what=replace(what, sources=tuple(
                renamed.get(arg.value, arg) if isinstance(arg, ir.Held) else arg for arg in what.sources)),
                uses=tuple(renamed[value].value if value in renamed else value for value in one.uses)))
            for arg in what.dests:
                if isinstance(arg, ir.Held) and arg.value in crossing:
                    insns.append(lir.Insn(one.at, (one.at, one.at),
                        ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (cells[arg.value],), (arg,)),
                        (), (arg.value,)))
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks))
