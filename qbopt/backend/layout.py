"""
A whole body, emitted -- and everything that has to move when it does.

select.py turns one operation into bytes and takes the address as a given.
That is not enough to rebuild a body, because the addresses are the thing
that changes: an instruction whose encoding differs in length from the one
BC wrote moves everything after it, and every branch into that region is
then pointing at the wrong byte.

So this is a fixed point, and the shape of it is what keeps it honest.
Every branch starts long. Addresses are assigned, each branch that reaches
its target within a signed byte is marked short, and the addresses are
assigned again. Shrinking only ever brings a target closer, so a branch
marked short stays reachable and the loop only ever goes one way -- which
is why it terminates rather than oscillating between two lengths that each
justify the other.

Then one final pass emits at the addresses the fixed point settled on, with
every target mapped through where it went.

What comes back is bytes plus the relocations, because a relocated
displacement is emitted as zero and the fixup that names it has to be moved
to wherever the field ended up.

Refuses the whole body where it cannot emit one op. A body half of which is
this pass's own code and half BC's is not something anything downstream
could reason about, and the fraction that cannot be emitted is small and
known: the x87 instructions, and the addresses in a space select.py does
not encode.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.backend import asm
from qbopt.model import mir
from qbopt.backend import lower
from qbopt.legacy import regalloc
from qbopt.model.mir import MirBody
from qbopt.objectfile.module import Module

# The assembler's, re-exported: this module builds them and hands them over.
Laid = asm.Laid
Table = asm.Table
selectable = mir.rewritable


def _fallthroughs(body: MirBody) -> MirBody:
    """Make implicit CFG edges explicit when address-order placement breaks them."""
    ordered = sorted(body.blocks, key=lambda block: block.at)
    following = {block.at: after.at for block, after in zip(ordered, ordered[1:])}
    changed = []
    for block in body.blocks:
        last = lower.current(block.ops[-1]) if block.ops else None
        destination = None
        if len(block.succ) == 1 and (last is None or last.op not in (
            ir.Operation.JUMP, ir.Operation.BRANCH, ir.Operation.RETURN
        )):
            destination, = block.succ
        elif len(block.succ) == 2 and last is not None and last.op is ir.Operation.BRANCH:
            if last.target in block.succ:
                destination = next(at for at in block.succ if at != last.target)
        if destination is not None and destination != following.get(block.at):
            anchor = block.ops[-1].at if block.ops else block.at
            jump = mir.Op(anchor, ir.Operation.JUMP, "jmp", (), (),
                          kind=mir.Kind.JUMP, target=destination,
                          made=ir.Semantics(ir.Operation.JUMP, "jmp", target=destination),
                          covers=(anchor, anchor), symbol=False)
            block = replace(block, ops=(*block.ops, jump))
        changed.append(block)
    return replace(body, blocks=tuple(changed))


def _ordered(body: MirBody, *, linear: bool = False) -> list[mir.Op]:
    """Every op, in the order they are emitted.

    Blocks in address order, and within a block the order the block lists
    them. Identical to sorting every op by address while nothing reorders
    anything -- which is true of every body raised from BC's code -- and not
    the same rule: a transform that moves a definition within its block
    changes the list and must not have layout put it back.

    An authoritative sequence may lay an entire acyclic single-successor
    body out in execution order. Branching, cycles and disconnected blocks
    retain their original placement; their fallthroughs need a fuller planner.
    """
    blocks = sorted(body.blocks, key=lambda one: one.at)
    if linear:
        at_of = {block.at: block for block in blocks}
        chain, seen = [], set()
        at = body.entry
        while at in at_of and at not in seen:
            block = at_of[at]
            if len(block.succ) > 1:
                break
            chain.append(block)
            seen.add(at)
            if not block.succ:
                if len(chain) == len(blocks):
                    blocks = chain
                break
            at = block.succ[0]
    return [op for block in blocks for op in block.ops]


# A root at each width an instruction can name it. ir.ROOT goes the other
# way; an allocation is per value and a value's register is its root.


def _trailing_zeros(found: Module, ops: list[mir.Op]) -> "Table | None":
    """The run of zero bytes the ops end on, where it reaches the segment's end.

    Only at the very end, and only all-zero: anything else that happens to
    decode is code until something proves otherwise.
    """
    highest = max(one.at + (asm._length_of(one, found) or 0) for one in ops)
    if highest != found.end:
        return None
    lo = found.end
    for one in sorted(ops, key=lambda x: x.at, reverse=True):
        length = asm._length_of(one, found) or 0
        if one.at + length != lo or any(found.code[one.at : lo]):
            break
        lo = one.at
    return None if lo == found.end else Table(lo, found.end)


PADDING = frozenset({0x90, 0x00})


def _padding_runs(
    found: Module,
    ops: list[mir.Op],
    carried: list["Table"],
    lowest: int,
    highest: int,
    reached: frozenset[int] | None = None,
) -> list["Table"]:
    """The gaps between the items that may be carried rather than selected.

    Padding is the easy half: BC aligns its procedures, so runs of `90` sit
    between them and nothing enters those.

    The other half is code the decoder never reached. Under /V, BC emits a
    call to B$EVCK after every statement, and in jumps-q-evt two of them sit
    directly after an unconditional `jmp` -- real relocated calls that
    nothing can arrive at. They were the only thing in the corpus that
    refused a whole-segment rebuild.

    Carrying them rests on one fact: no block walked in. A Table already
    copies its bytes and remaps every fixup inside it by however far it
    moved, so a relocated call travels correctly; what a Table cannot do is
    fix up a branch that lands in the middle of it, and reachability is the
    proof there is no such branch. If that proof were wrong the rebuild
    would already be unsound for the code around them.

    And where reachability is wrong in the one way that would matter -- a
    computed jump into a gap, through a form nothing here models -- the
    entry is a relocated word, so the target is an offset some record or
    fixup names. A gap's interior is not in the placement map, so
    relocate.as_records cannot map it and refuses the whole object. That
    refusal is what makes carrying safe rather than merely usually right,
    and it has to exist on both paths: the record one always did, the fixup
    one is newer.

    `reached` is what makes the question askable here. Without it only
    padding is carried, which is what this did before.
    """
    covered = set()
    for one in ops:
        for span in asm._ranges_of(one, found):
            covered.update(range(*span))
    for one in carried:
        covered.update(range(one.lo, one.hi))

    out: list[Table] = []
    start: int | None = None
    for at in range(lowest, highest + 1):
        empty = at < highest and at not in covered
        if empty and start is None:
            start = at
        elif not empty and start is not None:
            span = range(start, at)
            if all(one in PADDING for one in found.code[start:at]) or (
                reached is not None and not any(one in reached for one in span)
            ):
                out.append(Table(start, at))
            start = None
    return out


def selectable(op: mir.Op) -> bool:
    """Whether this op's bytes come from select.py rather than from the image.

    An op emitted verbatim -- a barrier, calls.py's restore idiom, an
    emulated x87 site -- is exactly as long as the bytes it copies, so its
    `covers` and its length are the same number and a transform may not make
    them differ. One that is selected has no such tie: it emits whatever the
    encoding needs and `covers` only says which of the original bytes it
    stands for.

    transform.py asks before handing a deleted op's bytes to a survivor. It
    used to hand them to whoever was nearest, and a restore idiom that took
    them stopped coming back its own length -- qb-qrender's SCREEN.OBJ, and
    the only object in either corpus with the shape.
    """
    return mir.rewritable(op)


def lay_out(body: MirBody, at: int, found: Module, fields: frozenset[int] = frozenset()) -> Laid | str:
    """Every op in `body`, emitted in order from `at`, or why it could not be."""
    return asm.assemble(_ordered(body), at, found, fields, labels=_labels(body))


def _labels(body: MirBody) -> dict[int, int]:
    labels: dict[int, int] = {}
    following: int | None = None
    for block in sorted(body.blocks, key=lambda one: one.at, reverse=True):
        if block.ops:
            following = block.ops[0].at
        if following is not None:
            labels[block.at] = following
    return labels


def _anchors(body: MirBody) -> dict[int, mir.Op]:
    """Block labels designate occurrences, not repeated source addresses."""
    labels = {}
    following = None
    for block in sorted(body.blocks, key=lambda one: one.at, reverse=True):
        if block.ops:
            following = block.ops[0]
        if following is not None:
            labels[block.at] = following
    return labels


def _names_a_value(body) -> bool:
    """Whether any operand in this body names a value rather than a place."""
    return any(
        any(isinstance(one, mir.Held) for one in (*op.args, *op.results))
        or (op.made is not None and any(isinstance(one, ir.Held) for one in (*op.made.dests, *op.made.sources)))
        for block in body.blocks
        for op in block.ops
    )


def _grounded(body: MirBody, held: dict | None) -> MirBody:
    """Every operand naming a value nothing placed, given a register anyway.

    Machine-side on purpose. MIR says `this value`; where the allocation has
    no answer -- about seventy bodies in the corpus that the allocator
    refuses outright -- the operand the original instruction had in the same
    position is the one the pass took away, so that is what goes back. It is
    written into `made`, which is the emission form and not MIR.

    Reverting the whole operation instead costs 693 bytes: a fold emitted
    from the load it replaced is not a fold.
    """
    covered = set(held or {})

    def settle(one, was):
        if not isinstance(one, ir.Held) or one.value in covered:
            return one
        return was if isinstance(was, ir.Reg) and was.width == one.width else None

    def resolve(op):
        if op.kind is mir.Kind.DIVMOD:
            # Emitted from its own operands by asm, which reads the seats
            # out of the allocation itself -- so there is no ir.Held here
            # for this to settle, and the fallback below would be a wrong
            # answer rather than a conservative one: stripping the operands
            # off an operation a pass rewrote emits the bytes BC wrote,
            # which divide the cell the operation no longer names.
            return op
        what = lower.current(op)
        if what is None or not any(
            isinstance(one, ir.Held) and one.value not in covered for one in (*what.dests, *what.sources)
        ):
            return op
        node = getattr(op.node, "semantics", None)
        dests = [
            settle(one, node.dests[i] if node is not None and i < len(node.dests) else None)
            for i, one in enumerate(what.dests)
        ]
        sources = [
            settle(one, node.sources[i] if node is not None and i < len(node.sources) else None)
            for i, one in enumerate(what.sources)
        ]
        if any(one is None for one in (*dests, *sources)):
            return replace(op, made=None, args=(), results=(), raised=None) if op.node is not None else op
        return replace(op, made=replace(what, dests=tuple(dests), sources=tuple(sources)))

    return replace(
        body,
        blocks=tuple(replace(block, ops=tuple(resolve(op) for op in block.ops)) for block in body.blocks),
    )


def allocated(bodies: list, plain: list | None = None, settle=None) -> tuple[list, dict | None]:
    """The bodies with a register for every value, and the assignment.

    A body the allocator refuses is handed back as it was *raised*, not as
    the passes left it: every operand is remapped through the assignment
    and there is none, so each operation would be written with the register
    BC had -- while a pass has moved the operations that made that true.

    `plain` is the raised bodies, not yet widened. Widening one costs a
    walk of every pair chain in it and the fallback wants about one body in
    seven, so `settle` is applied to the one that needs it rather than to
    all of them: 68 calls became 10 over the corpus.

    Out of the assembler. Colouring is a phase, and one that runs inside
    emission is one nothing downstream can be told has already happened --
    objwrite.py had no way to say so and was allocated over a second time,
    which produced a call encoding with no field for its own fixup.
    """
    was = dict(plain or ())
    got: dict = {}
    settled = []
    for name, body in bodies:
        fixed = regalloc.untangled(body)
        one = regalloc.colour(fixed, fixed.pins)
        if isinstance(one, str):
            fixed, one = body, regalloc.colour(body, body.pins)
        if not isinstance(one, str):
            got.update(one)
            settled.append((name, fixed))
            continue
        # Nothing can colour it. Without this the hoist had to allocate: it
        # moved a run out of a loop and had to find the result a register
        # itself, because nothing downstream would. That is 90 of
        # transform.py's machine references and where every hoist bug came
        # from.
        instead = was.get(name)
        if instead is not None and settle is not None:
            instead = settle(instead)
        settled.append((name, instead if instead is not None else body))
    return settled, got or None


def rebuild(
    found: Module,
    bodies: list[tuple[str, MirBody]],
    tables: tuple[tuple[int, int], ...] = (),
    fields: frozenset[int] = frozenset(),
    reached: frozenset[int] | None = None,
    native_fpu: bool = False,
    assignment: dict | None = None,
    ordered: bool = False,
    ordered_entries: frozenset[int] = frozenset(),
) -> Laid | str:
    """Every body in the module, laid out one after another.

    Whole-segment rather than per-body, because per-body does not work:
    splicing one back into BC's own layout is possible for 1 of the corpus's
    171 bodies -- the rest cross a LEDATA boundary, are not contiguous, or
    are branched into from outside. None of that applies to writing the
    segment, where boundaries and offsets are being produced rather than
    preserved.

    It also settles the targets that refused per-body: a branch from one
    body into another has somewhere to land once every body is in the same
    map.

    What comes back starts at the first body's own address. Whatever sits
    before it -- BC's module header, 48 bytes of `blARITH` and padding, and
    the only thing in the corpus's code segments that is not in a body --
    is the caller's to keep.
    """
    # An ir.Held names a value and resolves through the assignment. A caller
    # that supplied none is not saying "no registers", it is saying "you
    # decide" -- so colour here, or this path and wholeseg's compile the
    # same body two different ways. One the allocation still cannot cover
    # goes back to what its node says: a refusal for the whole module is
    # the wrong answer to one operand.
    # A class two of whose members are live at once cannot be moved, and a
    # copy is what breaks it -- on the phi edge, or before a two-address
    # operation whose source outlives it. Per body, and only where it helps:
    # a body the allocator refuses even untangled is laid out as it was.
    # No allocation here. An assembler emits what it is handed; colouring
    # a body is a phase, and one that runs inside emission is one nothing
    # downstream can be told has already happened. `allocated()` below is
    # the same work, and `wholeseg.py` calls it before this -- which is
    # what let objwrite.py stop being allocated over a second time.
    sequenced = frozenset(body.entry for _, body in bodies) if ordered else ordered_entries
    bodies = [(name, _fallthroughs(body) if body.entry in sequenced else body) for name, body in bodies]
    held = asm._held(assignment)
    bodies = [(name, _grounded(body, held)) for name, body in bodies]

    # Sorted on the address an operation's bytes start at. This tried to
    # honour the order a pass returned instead -- `place` moves a store out
    # of a call's push run and layout put it straight back -- with a key
    # that took the lowest address still ahead of an op in its own list.
    # It miscompiled nots: the right low word of a long and the wrong high
    # one. Two things were wrong and either alone breaks it -- the raise's
    # own lists are not always in byte order, so the key read every such op
    # as one a pass had moved; and `covers[0]` is not a sort key here even
    # though it is the honest answer to where an operation's bytes are.
    # A caller with an authoritative emission sequence opts into `ordered`.
    # Keep the legacy default until every caller's input ordering is proven.
    groups = []
    for _, body in bodies:
        end = max((op.covers[1] for block in body.blocks for op in block.ops if op.covers), default=body.entry)
        embedded = any(start < end and stop > body.entry for start, stop in tables)
        sequence = _ordered(body, linear=body.entry in sequenced and not embedded)
        if ordered or body.entry in sequenced:
            groups.append((body.entry, sequence))
        else:
            groups.extend((op.at, [op]) for op in sequence)
    if not ordered:
        groups.sort(key=lambda group: group[0])
    ops = [op for _, sequence in groups for op in sequence]
    if not ops:
        return "no bodies to rebuild"
    if any(asm._length_of(one, found) is None for one in ops):
        return f"{ops[0].at:#06x}: an op with no instruction behind it"

    lowest = min(body.entry for _, body in bodies)
    highest = max(
        (span[1] for one in ops if (span := asm._stands_for(one, found)) and span[0] < span[1]),
        default=lowest,
    )
    inside = [Table(lo, hi) for lo, hi in tables if lowest <= lo and hi <= found.end]
    highest = max(highest, *(one.hi for one in inside)) if inside else highest

    # BC pads the end of its code segment with zeros, and every object in
    # the corpus ends with four of them. Reachability walks in and the
    # decoder obliges -- `00 00` is `add [bx+si],al` -- so they arrive here
    # as ops, on an address no fixup names and select.py rightly will not
    # encode. They are not instructions and are carried rather than
    # selected: the same bytes, in the same place, which is the only thing
    # that can be right about padding.
    padding = _trailing_zeros(found, ops)
    if padding is not None:
        inside.append(padding)
        ops = [one for one in ops if one.at < padding.lo]
        if not ops:
            return "the body is nothing but padding"
        highest = padding.hi

    # BC aligns its procedures, so runs of `90` sit between them, and
    # nothing reaches those. Carried the same way a table is: the bytes are
    # what they were and nothing enters them, so where they end up does not
    # matter. Only runs that are entirely padding -- anything else in a gap
    # is bytes this cannot account for, and it says so instead.
    inside += _padding_runs(found, ops, inside, lowest, highest, reached)

    # Every byte between the first item and the last has to be one of them.
    # What is left over is data nothing here can name, and emitting only what
    # it understands would drop it silently along with anything it holds.
    covered = sum(asm._length_of(one, found) or 0 for one in ops) + sum(one.hi - one.lo for one in inside)
    if covered != highest - lowest:
        # Named where the gap is, not where the layout starts. It used to
        # report `lowest`, which sent every reading of this straight to the
        # first instruction in the segment and nowhere near the bytes.
        held = set()
        claims: dict[int, list[int]] = {}
        for one in ops:
            span = asm._stands_for(one, found)
            if span is not None:
                held.update(range(*span))
                for byte in range(*span):
                    claims.setdefault(byte, []).append(one.at)
        for one in inside:
            held.update(range(one.lo, one.hi))
        first = next((one for one in range(lowest, highest) if one not in held), None)
        if first is None:
            # Every byte is accounted for and the total still disagrees, so
            # two ops claim the same ones. A transform that moves an op and
            # leaves its `covers` behind does exactly that. Reported, rather
            # than raising StopIteration looking for a gap that is not
            # there -- which is what it did, from inside the error path.
            twice = sorted(one for one in range(lowest, highest) if len(claims.get(one, ())) > 1)
            where = f"{twice[0]:#06x}" if twice else "nowhere"
            return f"{where}: {covered - (highest - lowest)} bytes are claimed by more than one op"
        return f"{first:#06x}: {highest - lowest - covered} bytes between the ops are not instructions"

    origin = {}
    for _name, body in bodies:
        origin.update(body.origin)
    labels = {label: target for _, body in bodies for label, target in _labels(body).items()}
    anchors = {label: op for _, body in bodies for label, op in _anchors(body).items()} if sequenced else None
    return asm.assemble(_interleaved(ops, inside), lowest, found, fields, native_fpu, assignment, origin, labels, anchors)


def _starts_at(op: mir.Op) -> int:
    """The first original byte this operation stands for.

    `covers`, not `at`. The raise gives every operation folded out of one
    runtime call the same `at` -- the site's first push -- so `at` says
    nothing about where an operation's bytes are, and a key built on it put
    nots's operations in an order that computed the right low word of a
    long and the wrong high one.
    """
    return op.covers[0] if op.covers is not None else op.at


def _interleaved(ops: list, inside: list) -> list:
    """`ops` in their own order, with each carried run back where it sat.

    A table follows the operation owning its preceding original bytes, even
    when that operation moved. Clones own no bytes and cannot become anchors.
    """
    rank = {id(one): (index, 1) for index, one in enumerate(ops)}
    for one in inside:
        preceding = [(high, index) for index, op in enumerate(ops)
                     for low, high in (*((op.covers,) if op.covers is not None else ()), *op.extra_covers)
                     if low < high <= one.lo]
        index = max(preceding)[1] + 1 if preceding else 0
        rank[id(one)] = (index, 0)
    return sorted([*ops, *inside], key=lambda one: rank[id(one)])
