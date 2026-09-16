"""Remove reloads of a slot a register already holds on every path here.

Within a block a register holds the bytes of every slot it has been read out
of or written into since. Across blocks that fact has to be met at every
incoming edge, which is an availability problem and not something the first
instruction of a block and the last of its predecessor can answer: the x loop
of deedlines' plasmablobs reloads its counter at the top although the latch
left it in `ax`, because the edge between them is a block holding one `jmp`.
"""

from dataclasses import replace

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir


def _plain(one):
    return one.what is not None and not one.clobbers and not one.requires and not one.delivers


def _empty(one):
    return (
        _plain(one)
        and one.what.op is ir.Operation.NOTHING
        and one.what.name in (None, "")
        and not one.what.dests
        and not one.what.sources
        and one.what.target is None
    )


def _held(one, facts):
    """`facts` after `one`, and whether it is a reload `facts` makes redundant.

    A fact is a (register, slot) pair meaning the register holds that slot's
    bytes. Anything whose register effects cannot be read, or which writes
    memory the displacement alone does not name, ends every fact: the write
    could be to any slot.
    """
    from qbopt.backend.peephole import _lanes
    from qbopt.backend.peephole import _frame_cell
    from qbopt.backend.peephole import _overlapping
    from qbopt.backend.peephole import _frame_written
    from qbopt.backend.peephole import _register_effects

    if _empty(one):
        return facts, False
    what = one.what
    if what is None:
        return frozenset(), False
    if (
        what.op in (ir.Operation.BRANCH, ir.Operation.JUMP)
        and not what.dests
        and not what.sources
        and isinstance(what.target, int)
    ):
        # Its own effects cannot be read -- `_register_effects` answers only for
        # instructions that fall through. It writes no register and no memory,
        # and a block ending in one is otherwise the end of every fact.
        return facts, False
    effects = _register_effects(one)
    if effects is None:
        return frozenset(), False
    writes = effects[1]
    if writes & _lanes(Register.EBP):
        return frozenset(), False

    # `op.stores` is the last word only for an instruction that is its own op.
    # A reload carries the op of whatever it stands beside, stores and all, and
    # is a load: reading it as a write to memory ended every fact at the very
    # instruction the facts were there to answer.
    writing = (
        any(isinstance(dest, ir.Mem) for dest in what.dests) or not one.spill_reload and getattr(one.op, "stores", ())
    )
    if writing:
        written = _frame_written(one)
        if written is None:
            return frozenset(), False
        facts = frozenset((register, cell) for register, cell in facts if not _overlapping(cell, written))
    else:
        written = None

    match what:
        case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as register,), (ir.Mem() as cell,)) if (
            _frame_cell(cell) and register.width == cell.width
        ):
            if (register, cell) in facts:
                # The program's own load of a local is as redundant as the
                # allocator's: cycleblobs keeps `x%` in bx across its inner
                # loop and reloads it in the latch to increment it.
                return facts, not (
                    one.clobbers or one.requires or one.delivers or one.group is not None or one.symbol is True
                )
            facts = frozenset(pair for pair in facts if not _lanes(pair[0].register) & writes)
            return facts | {(register, cell)}, False
        case _:
            pass

    facts = frozenset(pair for pair in facts if not _lanes(pair[0].register) & writes)
    if (
        written is not None
        and what.op is ir.Operation.MOVE
        and len(what.sources) == 1
        and isinstance(what.sources[0], ir.Reg)
        and what.sources[0].width == written.width
        and not _lanes(what.sources[0].register) & writes
    ):
        facts |= {(what.sources[0], written)}
    return facts, False


def _available(body: lir.LirBody):
    """The facts true on entry to each block, met over every incoming edge.

    `None` stands for the top of the lattice -- a block not reached yet, whose
    contribution to the meet is every fact. A loop header needs that: met
    against nothing its backedge would start out killing the very fact the
    header is there to establish.
    """
    predecessors = {block.at: [] for block in body.blocks}
    for block in body.blocks:
        for at in block.succ:
            if at in predecessors:
                predecessors[at].append(block.at)
    blocks = {block.at: block for block in body.blocks}
    into: dict[int, frozenset | None] = {
        block.at: frozenset() if block.at == body.entry or not predecessors[block.at] else None for block in body.blocks
    }
    outof: dict[int, frozenset] = {}
    changing = True
    while changing:
        changing = False
        for at, facts in into.items():
            if facts is not None:
                leaving = _transfer(blocks[at], facts)[1]
                if outof.get(at) != leaving:
                    outof[at] = leaving
                    changing = True
        for at in into:
            if at == body.entry or not predecessors[at]:
                continue
            met = None
            for parent in predecessors[at]:
                if parent not in outof:
                    continue
                met = outof[parent] if met is None else met & outof[parent]
            if met is not None and met != into[at]:
                into[at] = met
                changing = True
    return {at: facts if facts is not None else frozenset() for at, facts in into.items()}


def _transfer(block, facts):
    """The reloads `facts` makes redundant in `block`, and the facts after it."""
    redundant = []
    for one in block.insns:
        facts, drop = _held(one, facts)
        if drop:
            redundant.append(id(one))
    return redundant, facts


def forwarded(body: lir.LirBody) -> lir.LirBody:
    into = _available(body)
    blocks = []
    for block in body.blocks:
        redundant = set(_transfer(block, into[block.at])[0])
        if redundant:
            block = replace(
                block,
                insns=tuple(lir.anchor(one) if id(one) in redundant else one for one in block.insns),
            )
        blocks.append(block)
    return replace(body, blocks=tuple(blocks))
