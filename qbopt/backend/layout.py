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

import collections
from dataclasses import replace

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.backend import asm
from qbopt.objectfile.module import Module
from qbopt.objectfile.module import SourceMap

# The assembler's, re-exported: this module builds them and hands them over.
Laid = asm.Laid
Table = asm.Table


def _emits(block) -> bool:
    """Whether this block puts any byte in the output."""
    return any(_emitting(op) for op in block.insns)


def _emitting(op: lir.Insn) -> bool:
    """Whether `op` puts bytes out, which its machine form answers and its kind does not.

    A phi's copy placed after allocation rides on a NOTHING op. Asked by kind,
    deedlines' `IF ... THEN rc% = -1` read as an empty block, the jump over it
    went, and the copy ran on both paths.
    """
    if op.what is not None:
        return op.what.op is not ir.Operation.NOTHING
    return op.kind is not mir.Kind.NOTHING


def _following(body: lir.LirBody) -> dict[int, int]:
    """Per block, the next one that emits anything.

    Skipping the ones that do not is the whole point: a block whose every
    operation is NOTHING keeps its address and its `covers` and contributes
    no bytes, so control reaching its predecessor's end falls through it to
    whatever comes after. Comparing against the immediately next block
    instead put a `jmp` back over every block `_threaded` had just emptied,
    which is why threading first appeared to do nothing at all.
    """
    ordered = sorted(body.blocks, key=lambda block: block.at)
    out: dict[int, int] = {}
    for index, block in enumerate(ordered):
        for after in ordered[index + 1 :]:
            if _emits(after):
                out[block.at] = after.at
                break
    return out


def _fallthroughs(body: lir.LirBody) -> lir.LirBody:
    """Make implicit CFG edges explicit when address-order placement breaks them.

    To a fixed point: a jump given to an empty block makes it emit, and the
    block before it, which fell through past it, now falls into the jump.
    Blocks only ever gain a terminator here, so this settles.
    """
    while (settled := _fallthroughs_once(body)) != body:
        body = settled
    return body


def _fallthroughs_once(body: lir.LirBody) -> lir.LirBody:
    following = _following(body)
    changed = []
    for block in body.blocks:
        last = block.insns[-1].what if block.insns else None
        destination = None
        if len(block.succ) == 1 and (
            last is None or last.op not in (ir.Operation.JUMP, ir.Operation.BRANCH, ir.Operation.RETURN)
        ):
            (destination,) = block.succ
        elif len(block.succ) == 2 and last is not None and last.op is ir.Operation.BRANCH:
            if last.target in block.succ:
                destination = next(at for at in block.succ if at != last.target)
        if destination is not None and destination != following.get(block.at):
            turned = _turned(block, last, destination, following.get(block.at))
            if turned is not None:
                changed.append(turned)
                continue
            anchor = block.insns[-1].at if block.insns else block.at
            jump = lir.Insn(
                at=anchor,
                covers=(anchor, anchor),
                what=ir.Semantics(ir.Operation.JUMP, "jmp", target=destination),
                defines=(),
                uses=(),
                symbol=False,
            )
            block = replace(block, insns=(*block.insns, jump))
        changed.append(block)
    return replace(body, blocks=tuple(changed))


# Each condition and the one that is true exactly when it is false. `jcxz`
# and `loop*` are absent on purpose: they have no inverse to name, so a
# block ending in one keeps its jump.
_PAIRS = (
    ("je", "jne"),
    ("jz", "jnz"),
    ("jl", "jge"),
    ("jnge", "jnl"),
    ("jle", "jg"),
    ("jng", "jnle"),
    ("jb", "jae"),
    ("jc", "jnc"),
    ("jnae", "jnb"),
    ("jbe", "ja"),
    ("jna", "jnbe"),
    ("js", "jns"),
    ("jo", "jno"),
    ("jp", "jnp"),
    ("jpe", "jpo"),
)
_OPPOSITE = {one: other for pair in _PAIRS for one, other in (pair, pair[::-1])}


def _turned(block, last, destination: int, after: "int | None"):
    """`block` with its branch inverted, where that is what removes the jump.

    Only when the branch already goes to the block placed next: inverting it
    then sends it where the jump was going and leaves the next block as the
    fall-through, so the jump has nothing left to do. Any other arrangement
    still needs one.
    """
    if last is None or last.op is not ir.Operation.BRANCH or after is None or last.target != after:
        return None
    return _inverted(block, last.name, destination)


def _inverted(block, name: "str | None", target: "int | None" = None):
    """`block` with its closing branch's sense reversed, or None.

    Inverting a condition is the same control flow either way, so this needs
    no proof beyond the mnemonic having an opposite -- which `jcxz` and the
    `loop` forms do not, and they keep their jump.
    """
    opposite = _OPPOSITE.get((name or "").lower())
    if opposite is None or not block.insns:
        return None
    branch = block.insns[-1]
    if branch.what is None or branch.what.op is not ir.Operation.BRANCH:
        return None
    where = branch.what.target if target is None else target
    return replace(
        block,
        insns=(
            *block.insns[:-1],
            replace(branch, what=replace(branch.what, name=opposite, target=where)),
        ),
    )


def _threaded(body: lir.LirBody) -> lir.LirBody:
    """Branch past a block that only jumps somewhere else.

    Blocks stay in BC's address order -- `_ordered` says so -- so which of a
    branch's two edges falls through is BC's choice, and the families do not
    make it the same way. QB's BC writes

        je   dc          ; taken
        jmp  f5          ; the fall-through, a block of its own
      dc: ...

    where PDS's BC writes the inverted branch and no jump at all. Reversing
    the condition and sending it to `f5` leaves `dc` as the fall-through and
    the jump with nothing to do. That is the whole difference between the
    `jumps` target passing on PDS and VBDOS /G2 and missing it on QB /O,
    /noO and VBDOS plain.

    The emptied block keeps its place and its `covers`, because BC's bytes
    have to stay owned by something -- it simply emits nothing, which is
    what `mir.Kind.NOTHING` is for. Only a block whose single operation is
    the jump qualifies, and only where the branch's own target is what
    follows it, so the fall-through the inversion needs is already there.
    """
    following = _following(body)
    at_of = {block.at: block for block in body.blocks}
    reached = collections.Counter(at for block in body.blocks for at in block.succ)

    turned: dict[int, lir.LirBlock] = {}
    for block in body.blocks:
        last = block.insns[-1].what if block.insns else None
        if last is None or last.op is not ir.Operation.BRANCH or len(block.succ) != 2:
            continue
        if last.target not in block.succ:
            continue
        through = next(at for at in block.succ if at != last.target)
        if through != following.get(block.at) or reached[through] != 1 or through in turned:
            continue
        middle = at_of.get(through)
        if middle is None or last.target != following.get(through):
            continue
        alive = [op for op in middle.insns if _emitting(op)]
        if len(alive) != 1 or alive[0].kind is not mir.Kind.JUMP or len(middle.succ) != 1:
            continue
        (beyond,) = middle.succ
        if beyond == through or beyond not in at_of:
            continue
        inverted = _inverted(block, last.name, beyond)
        if inverted is None:
            continue
        turned[block.at] = replace(inverted, succ=(beyond, last.target))
        turned[through] = replace(middle, succ=(), insns=tuple(_emptied(op) for op in middle.insns))
    if not turned:
        return body
    return replace(body, blocks=tuple(turned.get(block.at, block) for block in body.blocks))


def _emptied(op: lir.Insn) -> lir.Insn:
    """`op` emitting nothing, still owning the bytes it covers."""
    return replace(
        op,
        defines=(),
        uses=(),
        what=ir.Semantics(ir.Operation.NOTHING, "", (), ()),
        clobbers=frozenset(),
        clobbers_high=frozenset(),
        requires=(),
        delivers=(),
        symbol=False,
    )


def _fallen(body: lir.LirBody) -> lir.LirBody:
    """A jump to the block placed next emits nothing: control falls through to it."""
    following = _following(body)
    changed = []
    for block in body.blocks:
        last = block.insns[-1].what if block.insns else None
        if (
            last is not None
            and last.op is ir.Operation.JUMP
            and block.succ == (last.target,)
            and last.target == following.get(block.at)
        ):
            block = replace(block, insns=(*block.insns[:-1], _emptied(block.insns[-1])))
        changed.append(block)
    return replace(body, blocks=tuple(changed))


def _ordered(body: lir.LirBody, *, linear: bool = False) -> list[lir.Insn]:
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
    return [op for block in blocks for op in block.insns]


# A root at each width an instruction can name it. ir.ROOT goes the other
# way; an allocation is per value and a value's register is its root.


def _trailing_zeros(found: Module, ops: list[lir.Insn], source: SourceMap | None = None) -> "Table | None":
    """The run of zero bytes the ops end on, where it reaches the segment's end.

    Only at the very end, and only all-zero: anything else that happens to
    decode is code until something proves otherwise.
    """
    highest = max(one.at + (asm._length_of(one, found, source) or 0) for one in ops)
    if highest != found.end:
        return None
    lo = found.end
    for one in sorted(ops, key=lambda x: x.at, reverse=True):
        length = asm._length_of(one, found, source) or 0
        if one.at + length != lo or any(found.code[one.at : lo]):
            break
        lo = one.at
    return None if lo == found.end else Table(lo, found.end)


PADDING = frozenset({0x90, 0x00})


def _padding_runs(
    found: Module,
    ops: list[lir.Insn],
    carried: list["Table"],
    lowest: int,
    highest: int,
    reached: frozenset[int] | None = None,
    source: SourceMap | None = None,
) -> list["Table"]:
    """The gaps between the items that may be carried rather than selected.

    Padding is the easy half: BC aligns its procedures, so runs of `90` sit
    between them and nothing enters those.

    The other half is code the decoder never reached. Under /V, BC emits a
    call to B$EVCK after every statement, and in jumps-q-evt two of them sit
    directly after an unconditional `jmp` -- real relocated calls that
    nothing can arrive at. They were the only thing in the corpus that
    refused a whole-segment rebuild.

    Those are discarded, not carried. What kept them dead was the `jmp` in
    front, and layout drops a jump to the next block: deedlines' ACTIONS
    writes `add [pa],-360 / jmp next / jmp short back`, and carried, the
    dead short jump followed the `add` and ran, landing inside it on an
    illegal `FE`. Padding is still carried; falling into `90` is harmless.

    A computed jump into a discarded gap would name an offset whose
    interior is not in the placement map, so fresh OMF emission still
    refuses the object, as it did when the gap was carried.

    `reached` is what makes the question askable here. Without it only
    padding is carried, which is what this did before.
    """
    covered = set()
    for one in ops:
        for span in asm._ranges_of(one, found, source):
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
            padding = all(one in PADDING for one in found.code[start:at])
            if padding or (reached is not None and not any(one in reached for one in span)):
                out.append(Table(start, at, discarded=not padding))
            start = None
    return out


def selectable(op: lir.Insn) -> bool:
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
    return op.what is not None and op.what.op is not ir.Operation.BARRIER


def lay_out(
    body: lir.LirBody,
    at: int,
    found: Module,
    fields: frozenset[int] = frozenset(),
    source: SourceMap | None = None,
) -> Laid | str:
    """Every op in `body`, emitted in order from `at`, or why it could not be."""
    return asm.assemble(
        _ordered(body),
        at,
        found,
        fields,
        labels=_labels(body),
        anchors=_anchors(body),
        source=source,
    )


def _labels(body: lir.LirBody) -> dict[int, int]:
    labels: dict[int, int] = {}
    following: int | None = None
    for block in sorted(body.blocks, key=lambda one: one.at, reverse=True):
        if block.insns:
            following = block.insns[0].at
        if following is not None:
            labels[block.at] = following
    return labels


def _anchors(body: lir.LirBody) -> dict[int, lir.Insn]:
    """Block labels designate occurrences, not repeated source addresses."""
    labels = {}
    following = None
    for block in sorted(body.blocks, key=lambda one: one.at, reverse=True):
        if block.insns:
            following = block.insns[0]
        if following is not None:
            labels[block.at] = following
    return labels


def rebuild(
    found: Module,
    bodies: list[tuple[str, lir.LirBody]],
    tables: tuple[tuple[int, int], ...] = (),
    fields: frozenset[int] = frozenset(),
    reached: frozenset[int] | None = None,
    native_fpu: bool = False,
    assignment: dict | None = None,
    ordered: bool = False,
    ordered_entries: frozenset[int] = frozenset(),
    source: SourceMap | None = None,
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
    # Bodies are allocated LIR.  Layout changes placement and branches; it
    # never chooses registers or reconstructs machine form from MIR.
    sequenced = frozenset(body.entry for _, body in bodies) if ordered else ordered_entries
    bodies = [(name, _fallen(_fallthroughs(_threaded(body)))) for name, body in bodies]
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
        end = max((op.covers[1] for block in body.blocks for op in block.insns if op.covers), default=body.entry)
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
    if any(asm._length_of(one, found, source) is None for one in ops):
        return f"{ops[0].at:#06x}: an op with no instruction behind it"

    lowest = min(body.entry for _, body in bodies)
    highest = max(
        (span[1] for one in ops if (span := asm._stands_for(one, found, source)) and span[0] < span[1]),
        default=lowest,
    )
    dead_dispatch_ends = {
        op.node.insn.end
        for op in ops
        if op.what is not None
        and op.what.op is ir.Operation.NOTHING
        and isinstance(op.node, ir.Call)
        and op.node.name == "B$OGTA"
    }
    inside = [
        Table(lo, hi, discarded=lo in dead_dispatch_ends) for lo, hi in tables if lowest <= lo and hi <= found.end
    ]
    highest = max(highest, *(one.hi for one in inside)) if inside else highest

    # BC pads the end of its code segment with zeros, and every object in
    # the corpus ends with four of them. Reachability walks in and the
    # decoder obliges -- `00 00` is `add [bx+si],al` -- so they arrive here
    # as ops, on an address no fixup names and select.py rightly will not
    # encode. They are not instructions and are carried rather than
    # selected: the same bytes, in the same place, which is the only thing
    # that can be right about padding.
    padding = _trailing_zeros(found, ops, source)
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
    inside += _padding_runs(found, ops, inside, lowest, highest, reached, source)

    # Every byte between the first item and the last has to be one of them.
    # What is left over is data nothing here can name, and emitting only what
    # it understands would drop it silently along with anything it holds.
    covered = sum(asm._length_of(one, found, source) or 0 for one in ops) + sum(one.hi - one.lo for one in inside)
    if covered != highest - lowest:
        # Named where the gap is, not where the layout starts. It used to
        # report `lowest`, which sent every reading of this straight to the
        # first instruction in the segment and nowhere near the bytes.
        held = set()
        claims: dict[int, list[int]] = {}
        for one in ops:
            span = asm._stands_for(one, found, source)
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
    # A block label names an instruction occurrence.  Its source address is
    # only provenance and may be shared by inserted instructions in several
    # blocks.  Address-based labels are retained for external mappings, but
    # every internal block target is grounded on its concrete occurrence.
    anchors = {label: op for _, body in bodies for label, op in _anchors(body).items()}
    return asm.assemble(
        _interleaved(ops, inside),
        lowest,
        found,
        fields,
        native_fpu,
        assignment,
        origin,
        labels,
        anchors,
        source,
    )


def _starts_at(op: lir.Insn) -> int:
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
        preceding = [
            (high, index)
            for index, op in enumerate(ops)
            for low, high in (*((op.covers,) if op.covers is not None else ()), *op.extra_covers)
            if low < high <= one.lo
        ]
        index = max(preceding)[1] + 1 if preceding else 0
        rank[id(one)] = (index, 0)
    return sorted([*ops, *inside], key=lambda one: rank[id(one)])
