"""Phi elimination: the pass that takes LIR out of SSA.

A phi is not an instruction. It says that one value is whichever of several
arrived on the edge control came in on, which is a statement no machine can
make -- so before anything can be assigned a register, each phi becomes a
copy at the end of every predecessor it names.

LLVM's `PHIElimination`, and it runs in the same place: after the SSA
passes, before coalescing and allocation. `llvm/lib/CodeGen/PHIElimination.cpp`.

**Why the allocator cannot wait for it.** `docs/hoist-blocker.md` records a
week spent on a body whose crossing value had one neighbour and kept the
register the loop counter also had. The graph was right about the body it
was given, and the body was wrong: two values met at a phi and nothing
downstream could see a conflict, because in SSA there was not one. A phi
eliminated into copies has no such hiding place -- the copy is an
instruction, the value it writes is live from it, and the interference is
there to be found.

**Critical edges.** A copy goes at the end of the predecessor, which is only
correct where that block goes nowhere else -- otherwise the copy runs on a
path the phi does not describe. LLVM splits the edge and puts the copy in
the new block. Splitting an edge means inventing a block and a branch, and
this refuses instead: a phi on a critical edge is left alone and the pass
says which, so the allocator sees the values still joined rather than a
program that is wrong.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model.passes import LIRTransform


class PhiElimination(LIRTransform):
    name = "phielim"

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return eliminated(body)


def eliminated(body: lir.LirBody) -> lir.LirBody:
    """`body` with every phi it can lower replaced by copies."""
    at_of = {block.at: block for block in body.blocks}
    successors = {block.at: len(block.succ) for block in body.blocks}
    widths = _widths(body)

    once = _read_once(body)
    # One per predecessor->successor edge. Every move a phi becomes on
    # that edge is simultaneous with the others and with none outside it.
    groups: dict[tuple[int, int], int] = {}

    def edge_group(where: int, into: int) -> int:
        return groups.setdefault((where, into), len(groups) + 1)

    split: dict[tuple[int, int], list[tuple[int, int]]] = {}
    copies: dict[int, list[lir.Insn]] = {}
    rename: dict[int, int] = {}
    kept: dict[int, tuple[lir.Phi, ...]] = {}
    for block in body.blocks:
        stays = []
        for phi in block.phis:
            edges = [(where, value) for where, value in phi.incoming if where in at_of]
            if len(edges) != len(phi.incoming):
                stays.append(phi)  # an edge from outside this body
                continue
            if any(successors.get(where, 0) > 1 for where, _ in edges):
                # A critical edge. A copy at the end of that predecessor
                # would run on both its paths and not only the one the phi
                # describes, so the copy cannot go there -- and splitting
                # the edge means a block with no address, which layout
                # orders by.
                #
                # Where every incoming value is defined in its own
                # predecessor and read by nothing but this phi, none is
                # needed: the definition already sits on the edge, so
                # having it define the phi's result outright says what the
                # phi said. That is the same join, stated as one variable
                # instead of two and a copy.
                if all(_defined_in(at_of[w], v) and once.get(v) == 1 for w, v in edges):
                    for _where, value in edges:
                        rename[value] = phi.result
                    continue
                # Split the edge. The copy goes in a block of its own that
                # only the branch reaches, and which jumps back to the
                # successor -- so it runs on that path and no other.
                for where, value in edges:
                    if successors.get(where, 0) > 1:
                        split.setdefault((where, block.at), []).append((phi.result, value))
                    else:
                        copies.setdefault(where, []).append(
                            _copy(at_of[where], phi.result, value, edge_group(where, block.at), widths[phi.result])
                        )
                continue
            for where, value in edges:
                copies.setdefault(where, []).append(_copy(at_of[where], phi.result, value, edge_group(where, block.at), widths[phi.result]))
        kept[block.at] = tuple(stays)

    if not copies and not rename and not split:
        return body
    if split:
        return _split_edges(body, split, copies, rename, kept, widths)
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                insns=tuple(_renamed(one, rename) for one in _before_the_terminator(block, copies.get(block.at, []))),
                phis=kept[block.at],
            )
            for block in body.blocks
        ),
    )


def _read_once(body: lir.LirBody) -> dict[int, int]:
    """How many times each value is read, phi edges included."""
    out: dict[int, int] = {}
    for block in body.blocks:
        for one in block.insns:
            for value in one.uses:
                out[value] = out.get(value, 0) + 1
        for phi in block.phis:
            for _where, value in phi.incoming:
                out[value] = out.get(value, 0) + 1
    return out


def _defined_in(block: lir.LirBlock, value: int) -> bool:
    """Whether exactly one instruction in this block defines the value."""
    return sum(1 for one in block.insns if value in one.defines) == 1


def _renamed(one: lir.Insn, rename: dict[int, int]) -> lir.Insn:
    """One instruction defining the phi's result where it defined its own."""
    if not rename:
        return one
    if not any(v in rename for v in (*one.defines, *one.uses)):
        return one
    what = one.what
    if what is not None:
        what = ir.Semantics(
            what.op,
            what.name,
            tuple(_settled(x, rename) for x in what.dests),
            tuple(_settled(x, rename) for x in what.sources),
            what.target,
        )
    return replace(
        one,
        what=what,
        defines=tuple(rename.get(v, v) for v in one.defines),
        uses=tuple(rename.get(v, v) for v in one.uses),
    )


def _settled(where, rename: dict[int, int]):
    """One operand with every value it names put through the rename.

    Through `ir.mapped` rather than a case per operand shape: a cell names
    the value that computed its address, and a second branch here is a
    second place to forget it. This one looked in `Mem.through`, which has
    been a register since lowering stopped putting values there.
    """
    return ir.mapped(where, lambda one: ir.Held(rename.get(one.value, one.value), one.width))


def _widths(body: lir.LirBody) -> dict[int, int]:
    widths = {}
    for block in body.blocks:
        for op in block.insns:
            operands = [held for held, _ in (*op.requires, *op.delivers)]
            if op.what is not None:
                operands += [held for arg in (*op.what.dests, *op.what.sources) for held in ir.values(arg)]
            for value, width in (*op.widths, *((held.value, held.width) for held in operands)):
                widths[value] = max(widths.get(value, 0), width)
    phis = [phi for block in body.blocks for phi in block.phis]
    changing = True
    while changing:
        changing = False
        for phi in phis:
            values = (phi.result, *(value for _, value in phi.incoming))
            width = max(widths.get(value, 2) for value in values)
            for value in values:
                if widths.get(value) != width:
                    widths[value] = width
                    changing = True
    return widths


def _copy(where: lir.LirBlock, into: int, out_of: int, group: int | None = None, width: int = 2) -> lir.Insn:
    """The move a phi becomes, at the end of the block it arrives from.

    Placed on the predecessor's last instruction's address and claiming
    none of its bytes: `covers` is which of BC's bytes an instruction
    stands for, and this one stands for none -- it is work the phi always
    described and nobody ever emitted.
    """
    last = where.insns[-1] if where.insns else None
    at = last.at if last is not None else where.at
    edge = _nothing(last, at)
    return lir.Insn(
        group=group,
        at=at,
        covers=edge,
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(into, width),), (ir.Held(out_of, width),)),
        defines=(into,),
        uses=(out_of,),
        op=last.op if last is not None else None,
    )


def _nothing(beside: "lir.Insn | None", at: int) -> tuple[int, int]:
    """An empty span: an inserted instruction claims no original bytes.

    Never None. None means "ask the node how long it was", and the node is
    the instruction this was inserted beside -- whose bytes it already
    claims, so both would, and layout reports one byte claimed twice.
    """
    start = beside.covers[0] if beside is not None and beside.covers else at
    return (start, start)


def _before_the_terminator(block: lir.LirBlock, added: list[lir.Insn]) -> tuple[lir.Insn, ...]:
    """The copies at the end of the block, but ahead of what leaves it.

    A branch reads the flags something before it set, so a move between the
    two would be read as having changed them. After the branch is not the
    end of the block either -- it is a place nothing reaches.
    """
    if not added:
        return block.insns
    insns = list(block.insns)
    cut = len(insns)
    while cut and _leaves(insns[cut - 1]):
        cut -= 1
    return tuple(insns[:cut] + added + insns[cut:])


def _leaves(one: lir.Insn) -> bool:
    """Whether this instruction ends the block."""
    return one.what is not None and one.what.op in (ir.Operation.JUMP, ir.Operation.BRANCH, ir.Operation.RETURN)


def _split_edges(body, split: dict, copies: dict, rename: dict, kept: dict, widths: dict) -> lir.LirBody:
    """A block of its own on each critical edge, holding that edge's copies.

    The copies cannot go at the end of the predecessor -- it has another
    successor and they would run on that path too -- and they cannot go at
    the top of the successor, which has another predecessor. The block
    between the two is the only place they belong, which is what splitting
    an edge means.

    Synthetic labels are above physical offsets, in a namespace per body
    entry. A label just past this body can be another body's real entry.
    Layout emits the blocks out of line and each jumps to its successor.
    """
    at_of = {block.at: block for block in body.blocks}
    made: dict[tuple[int, int], lir.LirBlock] = {}
    landing: dict[tuple[int, int], int] = {}
    for number, ((where, into), pairs) in enumerate(sorted(split.items()), 1):
        at = ((body.entry + 1) << 32) + number
        landing[(where, into)] = at
        beside = at_of[where].insns[-1]
        insns = [
            replace(
                _made(
                    beside, at, ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(a, widths[a]),), (ir.Held(b, widths[a]),)), (a,), (b,)
                ),
                group=number,
            )
            for a, b in pairs
        ]
        insns.append(_made(beside, at, ir.Semantics(ir.Operation.JUMP, "jmp", (), (), into), (), ()))
        made[(where, into)] = lir.LirBlock(at=at, insns=tuple(insns), succ=(into,), phis=())

    blocks = []
    for block in body.blocks:
        succ = tuple(landing.get((block.at, one), one) for one in block.succ)
        insns = [_retargeted(one, landing, block.at) for one in _before_the_terminator(block, copies.get(block.at, []))]
        last = block.insns[-1]
        if last.what is not None and last.what.op is ir.Operation.BRANCH:
            fallthrough = next((into for into in block.succ if into != last.what.target), None)
            if (edge := landing.get((block.at, fallthrough))) is not None:
                insns.append(_made(last, last.at, ir.Semantics(ir.Operation.JUMP, "jmp", (), (), edge), (), ()))
        blocks.append(
            replace(block, insns=tuple(_renamed(one, rename) for one in insns), succ=succ, phis=kept[block.at])
        )
    return replace(body, blocks=tuple([*blocks, *made.values()]))


def _made(beside: lir.Insn, at: int, what: ir.Semantics, defines: tuple, uses: tuple) -> lir.Insn:
    """One instruction in a split block, claiming none of BC's own bytes."""
    return lir.Insn(at=at, covers=(at, at), what=what, defines=defines, uses=uses, op=beside.op)


def _retargeted(one: lir.Insn, landing: dict, here: int) -> lir.Insn:
    """A branch or jump pointing at the split block instead of the successor."""
    what = one.what
    if what is None or what.target is None:
        return one
    at = landing.get((here, what.target))
    if at is None:
        return one
    return replace(one, what=ir.Semantics(what.op, what.name, what.dests, what.sources, at))
