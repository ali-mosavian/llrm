"""Which physical register and flag lanes are dead on exit from each block.

A backward walk inside one block starts by assuming everything is live, so a
copy written as the last instruction of a block always survives it -- and a
parallel copy for a phi is written exactly there. deedlines' plasmablobs ends
its inner loop with `mov di,bx` whose destination no path reads before writing
it again.
"""

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir


def _terminator(what) -> bool:
    return (
        what is not None
        and what.op in (ir.Operation.BRANCH, ir.Operation.JUMP)
        and not what.dests
        and not what.sources
        and isinstance(what.target, int)
    )


# What a caller keeps across a call, and so what a return reads beside its results.
_KEPT = (Register.ESI, Register.EDI, Register.EBP, Register.ESP, Register.DS, Register.SS, Register.CS)


def _universe() -> frozenset:
    """Every lane a body can name. "Dead" here means every lane but the live ones."""
    from qbopt.backend import target
    from qbopt.backend.peephole import _lanes
    from qbopt.backend.peephole import _flag_lanes

    lanes = set(_flag_lanes(0xFFFFFFFF))
    for register in (*target.WIDTHS, *target.SEGMENTS):
        lanes |= _lanes(register)
    return frozenset(lanes)


def _backwards(block, live: frozenset, universe: frozenset) -> frozenset:
    """The lanes live before `block`, given those live after it."""
    from qbopt.backend.peephole import _branch_reads
    from qbopt.backend.peephole import _register_effects

    for one in reversed(block.insns):
        if _terminator(one.what):
            # Its flag read is not in `_register_effects`, which answers only
            # for instructions that fall through. It writes nothing.
            if one.what.op is ir.Operation.BRANCH:
                live = live | _branch_reads(one.what)
            continue
        effects = _register_effects(one, flags=True)
        if effects is None:
            effects = _declared(one)
        if effects is None:
            # It may read anything, but what the block writes before it is still written first.
            live = universe
            continue
        live = (live - effects[1]) | effects[0]
    return live


def _declared(one) -> "tuple[frozenset, frozenset] | None":
    """What a call says it reads and writes, for an instruction no decoder covers.

    `requires` and `clobbers` are the contract the allocation is already built
    on, so reading a call as touching every register only makes a register the
    callee never names look live -- which kept every value a phi copies alive
    across the whole loop.
    """
    from qbopt.backend.peephole import _lanes
    from qbopt.backend.peephole import _flag_lanes

    if one.what is not None and one.what.op is ir.Operation.RETURN and getattr(one.op, "reads_complete", False):
        # Nothing runs after it: it reads its results and what the caller keeps, and no other lane.
        reads = {lane for held, register in one.requires for lane in _lanes(register)}
        for register in _KEPT:
            reads |= _lanes(register)
        return frozenset(reads), _universe() - reads
    if not one.clobbers or one.symbol is True:
        return None
    reads = {lane for held, register in one.requires for lane in _lanes(register)}
    # A transfer's decoded effects are unavailable, but its explicit operands
    # are still real reads. In particular an indirect `call bx` reads BX
    # before the calling convention clobbers it.
    for source in one.what.sources if one.what is not None else ():
        if isinstance(source, ir.Reg):
            reads |= _lanes(source.register)
        elif isinstance(source, (ir.Mem, ir.Address)):
            reads |= _lanes(source.through)
            reads |= _lanes(source.index_through if isinstance(source, ir.Mem) else source.index)
            selector = getattr(source, "selector", None)
            if isinstance(selector, ir.Reg):
                reads |= _lanes(selector.register)
    writes = {lane for held, register in one.delivers for lane in _lanes(register)}
    for register in one.clobbers:
        writes |= _lanes(register)
    for register in one.clobbers_high:
        writes |= {lane for lane in _lanes(register) if lane[1] >= 2}
    return frozenset(reads), frozenset(writes | _flag_lanes(0xFFFFFFFF))


def live_into(body: lir.LirBody) -> "tuple[dict[int, frozenset], dict[int, list[int]], frozenset]":
    """Per block, the lanes live on entry -- with its successors and the universe."""
    universe = _universe()
    at_of = {block.at for block in body.blocks}
    successors = {block.at: [at for at in block.succ if at in at_of] for block in body.blocks}
    blocks = {block.at: block for block in body.blocks}
    into = {at: frozenset() for at in blocks}
    changing = True
    while changing:
        changing = False
        for at, block in blocks.items():
            after = universe if not successors[at] else frozenset().union(*(into[to] for to in successors[at]))
            before = _backwards(block, after, universe)
            if before != into[at]:
                into[at] = before
                changing = True
    return into, successors, universe


def dead_at_exit(body: lir.LirBody) -> dict[int, frozenset]:
    """Per block, the lanes nothing reads again after it."""
    into, successors, universe = live_into(body)
    return {
        at: universe - (universe if not successors[at] else frozenset().union(*(into[to] for to in successors[at])))
        for at in into
    }
