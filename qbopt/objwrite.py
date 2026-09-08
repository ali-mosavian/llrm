"""LIR to an object file. The last pass, and the only one that writes bytes.

Everything above this has been values, then locations. Here a location
becomes an encoding, an encoding becomes a length, a length decides where
the next instruction starts, and a branch and a fixup both have to point at
where things actually landed.

Three steps, in the order they have to happen:

    select      each instruction's bytes, at the shortest encoding that fits
    layout      one image, with every branch pointing at where its target went
    relocate    the OMF records, with every fixup moved to its new offset

They are `select.py`, `layout.py` and `relocate.py`, and this is the pass
that runs them. It does not reimplement them: layout is handed the same
operations LIR was lowered from, each carrying the machine form the
allocator settled, so there is one emitter rather than two that drift.
"""

from dataclasses import replace

from qbopt import layout
from qbopt import lir
from qbopt import ir
from qbopt import mir
from qbopt import omf
from qbopt import relocate
from qbopt.module import Module


def written(
    found: Module,
    bodies: "list[lir.LirBody]",
    records: list,
    assignment: dict,
    tables: tuple = (),
    fields: frozenset[int] = frozenset(),
    reached: frozenset[int] | None = None,
    native_fpu: bool = False,
) -> bytes | str:
    """The object, with its code segment written from these bodies.

    A string instead, saying why, wherever anything refuses. The caller
    keeps what it had: a refusal is a module some earlier pass has already
    improved, and throwing that away to report a failure helps nobody.
    """
    laid = layout.rebuild(
        found,
        [(one.name, _as_mir(one)) for one in bodies],
        tables,
        fields,
        reached,
        native_fpu,
        assignment=assignment or None,
    )
    if isinstance(laid, str):
        return laid

    # Whatever sits before the first instruction is BC's own module header,
    # 48 bytes of name and padding, and the only thing in these code
    # segments that is not in a body. Kept, and the layout starts after it.
    kept = min(one.at for body in bodies for one in body.insns)
    image = found.code[:kept] + laid.code
    made = relocate.as_records(
        records,
        found.seg,
        kept,
        image,
        {**laid.covered, **laid.moved},
        {old: kept + new for new, old in laid.relocations},
        laid.dropped,
    )
    if isinstance(made, str):
        return made
    return b"".join(record.emit() for record in made)


class Survived(Exception):
    """A phi reached emission. Always a bug in phi elimination.

    A phi is not an instruction, so emitting one emits nothing at all --
    which is how three of bools-q-O's went missing without a word, and why
    it printed T=1 for 2. This is the hard invariant: nothing below
    elimination may guess what a phi meant.
    """


def _as_mir(body: "lir.LirBody") -> mir.MirBody:
    """The operations layout still asks its questions of, carrying LIR.

    `made` is the emission form: layout reads it in preference to the
    instruction an operation was raised from, so putting the lowered and
    allocated semantics there is how LIR reaches the emitter.

    `at` and `covers` come from the instruction, not from the operation it
    was lowered from. An inserted one -- a phi's copy, a two-address move
    -- carries its neighbour's operation, and taking the span from there
    made both claim the same byte: layout said "1 bytes are claimed by more
    than one op" on twelve objects. The instruction is the authority on
    which bytes it stands for, which for an inserted one is none.

    This is the seam. When layout takes LirBody directly it goes.
    """
    stuck = [block.at for block in body.blocks if block.phis]
    if stuck:
        raise Survived(
            "a phi survives at " + ", ".join(f"{one:#06x}" for one in stuck) + "; nothing below can emit one"
        )
    return mir.MirBody(
        entry=body.entry,
        blocks=tuple(
            mir.MirBlock(
                at=block.at,
                phis=(),  # checked above: a surviving one is refused, not dropped
                ops=tuple(_carried(one) for one in block.insns),
                succ=block.succ,
            )
            for block in body.blocks
        ),
        origin=body.origin,
        pins=body.pins,
    )


# The seam. `assignment` is handed down beside the bodies because select.py
# still resolves an operand through it rather than reading the register LIR
# already put there -- allocate.applied() has done that, and this passes the
# same answer a second way so the two cannot disagree. It goes when select
# takes an lir.Insn.


def _carried(one: "lir.Insn") -> mir.Op:
    """One instruction as the operation layout still asks its questions of.

    An inserted one -- a phi's copy, a two-address move, a spill's store --
    is given no node. That is not a detail: `node` is the instruction the
    operation was raised from, and every question layout answers by reading
    the original bytes goes through it. An inserted instruction carries the
    address of the one it stands beside, so a far call's `9a` was read at
    its address and it claimed the call's own fixup -- nineteen objects
    said `call has 1 fixups and 0 fields to put them in`, naming the call,
    which was not the operation asking.

    No node and no bytes; `made` is its whole definition and `covers` says
    it stands for none of BC's.
    """
    # Owning no original bytes is not the same as having no identity. The
    # half an absorbed divide hands back stands for none of BC's -- the
    # site's range belongs to the divide, once -- and its node is still
    # what says which idiom it is: stripped, it reached the general
    # encoder as an operation named "restore" with no operands, and every
    # body holding an absorbed divide fell out of this route.
    idiom = isinstance(one.op.node, ir.Restore)
    inserted = not idiom and one.covers is not None and one.covers[0] == one.covers[1]
    return replace(
        one.op,
        made=one.what,
        at=one.at,
        covers=one.covers,
        node=None if inserted else one.op.node,
        id=None if inserted else one.op.id,
    )
