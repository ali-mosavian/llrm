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


def _as_mir(body: "lir.LirBody") -> mir.MirBody:
    """The operations layout still asks its questions of, carrying LIR.

    `made` is the emission form: layout reads it in preference to the
    instruction an operation was raised from, so putting the lowered and
    allocated semantics there is how LIR reaches the emitter. The MIR is
    along for its addresses and its block structure, both of which layout
    needs and LIR keeps unchanged.

    This is the seam. When layout takes LirBody directly it goes.
    """
    return mir.MirBody(
        entry=body.entry,
        blocks=tuple(
            mir.MirBlock(
                at=block.at,
                phis=(),
                ops=tuple(replace(one.op, made=one.what) for one in block.insns),
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
