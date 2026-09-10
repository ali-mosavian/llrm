"""Remove entry reloads when every incoming edge already carries their value."""

from dataclasses import replace

from qbopt.model import ir, lir


def _plain(one):
    return one.what is not None and not one.clobbers and not one.requires and not one.delivers


def _empty(one):
    return (_plain(one) and one.what.op is ir.Operation.NOTHING
            and one.what.name in (None, "") and not one.what.dests
            and not one.what.sources and one.what.target is None)


def _stored(block, register, cell):
    for one in reversed(block.insns):
        if _empty(one):
            continue
        if not _plain(one):
            return False
        what = one.what
        if (what.op in (ir.Operation.BRANCH, ir.Operation.JUMP)
            and not what.dests and not what.sources and isinstance(what.target, int)):
            continue
        return (what.op is ir.Operation.MOVE and what.name == "mov"
                and what.dests == (cell,) and what.sources == (register,))
    return False


def forwarded(body: lir.LirBody) -> lir.LirBody:
    predecessors = {block.at: [] for block in body.blocks}
    for block in body.blocks:
        for at in block.succ:
            if at in predecessors:
                predecessors[at].append(block)
    blocks = []
    for block in body.blocks:
        first = next((one for one in block.insns if not _empty(one)), None)
        incoming = predecessors[block.at]
        if block.at != body.entry and incoming and first is not None and first.spill_reload and _plain(first):
            match first.what:
                case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as register,), (ir.Mem() as cell,)):
                    if (register.width == cell.width
                        and all(_stored(parent, register, cell) for parent in incoming)):
                        block = replace(block, insns=tuple(lir.without(block.insns, lambda one: one is first)))
        blocks.append(block)
    return replace(body, blocks=tuple(blocks))
