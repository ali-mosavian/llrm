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
the new block. Synthetic labels keep those blocks distinct from original
addresses, and retained phis name the new predecessor. Floating allocation
uses the same placement for its selected extended-precision transfers.
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
        crossing: dict[int, list[tuple[int, int]]] = {}
        for phi in block.phis:
            edges = [(where, value) for where, value in phi.incoming if where in at_of]
            if len(edges) != len(phi.incoming):
                stays.append(phi)  # an edge from outside this body
                continue
            # A one-input phi computes its input. Unify the identities before
            # allocation instead of creating a copy that can perturb register
            # assignment or force an otherwise empty edge block.
            if len(edges) == 1:
                rename[phi.result] = edges[0][1]
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
                # Split the edge, unless nothing on the predecessor's other
                # paths can read what the copies write (decided per edge
                # below, since its copies happen at once). The copy goes in a
                # block of its own that only the branch reaches, and which
                # jumps back to the successor -- so it runs on that path and
                # no other.
                for where, value in edges:
                    if successors.get(where, 0) > 1:
                        crossing.setdefault(where, []).append((phi.result, value))
                    else:
                        copies.setdefault(where, []).append(
                            _copy(at_of[where], phi.result, value, edge_group(where, block.at), widths[phi.result])
                        )
                continue
            for where, value in edges:
                copies.setdefault(where, []).append(
                    _copy(
                        at_of[where],
                        phi.result,
                        value,
                        edge_group(where, block.at),
                        widths[phi.result],
                    )
                )
        for where, pairs in crossing.items():
            # A copy may run in the predecessor only when neither end needs
            # a distinct value on its other paths.  A result observed there
            # makes the early write wrong outright; a source observed there
            # makes source and result overlap, preventing the coalescer from
            # proving the copy free.  Put either shape on its actual edge.
            # nbody's two accumulator exits stayed in the branching block,
            # forced four values live at once, and spilled the hotter loop
            # counter instead.
            if any(
                _observed(body, at_of, where, block.at, result) or _observed(body, at_of, where, block.at, value)
                for result, value in pairs
            ):
                split.setdefault((where, block.at), []).extend(pairs)
                continue
            for result, value in pairs:
                copies.setdefault(where, []).append(
                    _copy(at_of[where], result, value, edge_group(where, block.at), widths[result])
                )
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
                phis=tuple(_renamed_phi(phi, rename) for phi in kept[block.at]),
            )
            for block in body.blocks
        ),
    )


def _observed(body: lir.LirBody, at_of: dict, where: int, into: int, value: int) -> bool:
    """Whether `value` can be read after leaving `where` other than into `into`.

    `into` defines it, so a path through `into` reads a new one.
    """
    pending = [at for at in at_of[where].succ if at != into]
    # A phi reads on the incoming edge, before any instruction in its block.
    # The walk below sees phis on later edges, but an immediate alternate
    # successor has no intervening block from which to discover this one.
    if any((where, value) in phi.incoming for at in pending for phi in getattr(at_of.get(at), "phis", ())):
        return True
    seen = set(pending)
    while pending:
        block = at_of.get(pending.pop())
        if block is None:
            return True
        if any(value in one.uses or any(held.value == value for held, _ in one.requires) for one in block.insns):
            return True
        for successor in block.succ:
            follower = at_of.get(successor)
            if follower is not None and any((block.at, value) in phi.incoming for phi in follower.phis):
                return True
            if successor != into and successor not in seen:
                seen.add(successor)
                pending.append(successor)
    return False


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
        defines=tuple(_name(v, rename) for v in one.defines),
        uses=tuple(_name(v, rename) for v in one.uses),
        requires=tuple((ir.Held(_name(held.value, rename), held.width), register) for held, register in one.requires),
        delivers=tuple((ir.Held(_name(held.value, rename), held.width), register) for held, register in one.delivers),
        widths=tuple((_name(value, rename), width) for value, width in one.widths),
    )


def _name(value: int, rename: dict[int, int]) -> int:
    """The final identity after chained trivial phis are unified."""
    seen = set()
    while value in rename and rename[value] != value:
        if value in seen:
            raise ValueError("cyclic phi rename")
        seen.add(value)
        value = rename[value]
    return value


def _settled(where: ir.Loc | ir.Held, rename: dict[int, int]) -> ir.Loc | ir.Held:
    """One operand with every value it names put through the rename.

    Through `ir.mapped` rather than a case per operand shape: a cell names
    the value that computed its address, and a second branch here is a
    second place to forget it. This one looked in `Mem.through`, which has
    been a register since lowering stopped putting values there.
    """
    return ir.mapped(where, lambda one: ir.Held(_name(one.value, rename), one.width))


def _renamed_phi(phi: lir.Phi, rename: dict[int, int]) -> lir.Phi:
    """A surviving phi with every trivial-phi identity made final."""
    return replace(
        phi,
        result=_name(phi.result, rename),
        incoming=tuple((where, _name(value, rename)) for where, value in phi.incoming),
    )


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


def placed_on_edges(body: lir.LirBody, transfers: dict[tuple[int, int], list[lir.Insn]]) -> lir.LirBody:
    """Place already selected parallel transfers on their exact CFG edges."""
    at_of = {block.at: block for block in body.blocks}
    copies, split = {}, {}
    for (where, into), insns in transfers.items():
        if len(at_of[where].succ) > 1:
            split[where, into] = insns
        else:
            copies.setdefault(where, []).extend(insns)
    if not split:
        return replace(
            body,
            blocks=tuple(
                replace(block, insns=_before_the_terminator(block, copies.get(block.at, []))) for block in body.blocks
            ),
        )
    return _split_edges(body, split, copies, {}, {block.at: block.phis for block in body.blocks}, {}, selected=True)


def _split_edges(
    body: lir.LirBody,
    split: dict,
    copies: dict,
    rename: dict,
    kept: dict,
    widths: dict,
    *,
    selected: bool = False,
) -> lir.LirBody:
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
        at = max((body.entry + 1) << 32, max(at_of)) + number
        landing[(where, into)] = at
        beside = at_of[where].insns[-1]
        insns = (
            [replace(one, at=at, covers=(at, at)) for one in pairs]
            if selected
            else [
                replace(
                    _made(
                        beside,
                        at,
                        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(a, widths[a]),), (ir.Held(b, widths[a]),)),
                        (a,),
                        (b,),
                    ),
                    group=number,
                )
                for a, b in pairs
            ]
        )
        insns.append(_made(beside, at, ir.Semantics(ir.Operation.JUMP, "jmp", (), (), into), (), ()))
        # These instructions live outside `body.blocks` until the end of the
        # transformation, so the ordinary rename walk below cannot see them.
        # A critical-edge copy that reads a one-input phi's old identity then
        # names a value whose only definition this pass just removed.
        made[(where, into)] = lir.LirBlock(
            at=at,
            insns=tuple(_renamed(one, rename) for one in insns),
            succ=(into,),
            phis=(),
        )

    blocks = []
    for block in body.blocks:
        succ = tuple(landing.get((block.at, one), one) for one in block.succ)
        insns = [_retargeted(one, landing, block.at) for one in _before_the_terminator(block, copies.get(block.at, []))]
        last = block.insns[-1] if block.insns else None
        if last is not None and last.what is not None and last.what.op is ir.Operation.BRANCH:
            fallthrough = next((into for into in block.succ if into != last.what.target), None)
            if (edge := landing.get((block.at, fallthrough))) is not None:
                insns.append(_made(last, last.at, ir.Semantics(ir.Operation.JUMP, "jmp", (), (), edge), (), ()))
        blocks.append(
            replace(
                block,
                insns=tuple(_renamed(one, rename) for one in insns),
                succ=succ,
                phis=tuple(
                    _renamed_phi(
                        replace(
                            phi,
                            incoming=tuple(
                                (landing.get((where, block.at), where), value) for where, value in phi.incoming
                            ),
                        ),
                        rename,
                    )
                    for phi in kept[block.at]
                ),
            )
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


def unsplit(body: lir.LirBody) -> lir.LirBody:
    """A split edge whose copies all went away is the edge again.

    Coalescing gives both ends of a copy one register and the copy goes,
    leaving the block `_split_edges` made holding only its jump. On a loop
    entered at its body the back edge is the critical one, and that jump,
    placed out of line, was taken on every pass.
    """
    floor = (body.entry + 1) << 32
    bypass = {}
    for block in body.blocks:
        if block.at < floor or block.phis or len(block.succ) != 1:
            continue
        live = [
            one
            for one in block.insns
            if not (one.what is not None and one.what.op is ir.Operation.NOTHING and not one.what.name)
        ]
        if (
            len(live) == 1
            and live[0].what is not None
            and live[0].what.op is ir.Operation.JUMP
            and live[0].what.target == block.succ[0]
        ):
            bypass[block.at] = block.succ[0]
    if not bypass:
        return body

    def where(at: int) -> int:
        seen = set()
        while at in bypass and at not in seen:
            seen.add(at)
            at = bypass[at]
        return at

    blocks = []
    for block in body.blocks:
        if block.at in bypass:
            continue
        insns = tuple(
            replace(one, what=replace(one.what, target=where(one.what.target)))
            if one.what is not None and one.what.target in bypass
            else one
            for one in block.insns
        )
        phis = tuple(
            replace(phi, incoming=tuple((where(at), value) for at, value in phi.incoming)) for phi in block.phis
        )
        blocks.append(replace(block, insns=insns, succ=tuple(where(at) for at in block.succ), phis=phis))
    return replace(body, blocks=tuple(blocks))
