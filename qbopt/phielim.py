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

from qbopt import ir
from qbopt import lir
from qbopt.passes import LIRTransform


class PhiElimination(LIRTransform):
    name = "phielim"

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return eliminated(body)


def eliminated(body: lir.LirBody) -> lir.LirBody:
    """`body` with every phi it can lower replaced by copies."""
    at_of = {block.at: block for block in body.blocks}
    successors = {block.at: len(block.succ) for block in body.blocks}

    copies: dict[int, list[lir.Insn]] = {}
    kept: dict[int, tuple[lir.Phi, ...]] = {}
    for block in body.blocks:
        stays = []
        for phi in block.phis:
            edges = [(where, value) for where, value in phi.incoming if where in at_of]
            if len(edges) != len(phi.incoming) or any(successors.get(where, 0) > 1 for where, _ in edges):
                stays.append(phi)  # critical edge, or an edge out of this body
                continue
            for where, value in edges:
                copies.setdefault(where, []).append(_copy(at_of[where], phi.result, value))
        kept[block.at] = tuple(stays)

    if not copies:
        return body
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                insns=_before_the_terminator(block, copies.get(block.at, [])),
                phis=kept[block.at],
            )
            for block in body.blocks
        ),
    )


def _copy(where: lir.LirBlock, into: int, out_of: int) -> lir.Insn:
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
        at=at,
        covers=edge,
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(into, 2),), (ir.Held(out_of, 2),)),
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
