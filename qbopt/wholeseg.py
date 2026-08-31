"""
An object whose code segment this pass wrote, rather than edited.

Every other path in qbopt patches BC's own bytes in place -- a region here,
an instruction there -- and lives with the layout BC chose. This one does
not: `layout.rebuild` places every body afresh and `relocate.as_records`
writes the records to match, so chunk boundaries, branch displacements and
fixup offsets are all produced rather than preserved.

That is what per-body splicing could not do. Fitting a laid-out body back
into BC's own layout works for 1 of the corpus's 171 bodies -- the rest
cross a LEDATA boundary, are not contiguous, or are branched into from
outside -- and none of those apply when the boundaries are yours to choose.

Refuses far more than it accepts, and says why each time. A body holding an
op select.py cannot emit, or data BC put between the instructions, refuses
the whole object: half this pass's code and half BC's is not something
anything downstream could reason about.
"""

from qbopt import mir
from qbopt import omf
from qbopt import layout
from qbopt import module
from qbopt import relocate
from qbopt import blocks as split
from qbopt.blocks import code_map


def rebuilt(data: bytes) -> tuple[bytes, str]:
    """The object with its code segment rewritten, and what happened.

    Returns the input unchanged where anything refuses, so a caller can use
    this as a transform without deciding first whether it will work.
    """
    records = omf.parse(data)
    found = module.of(records)
    if found is None:
        return data, "the module has no code segment"
    mapped = code_map(found)
    if isinstance(mapped, str):
        return data, mapped

    bodies = list(mir.bodies(found, split.partition(found, mapped)))
    if not bodies:
        return data, "no bodies were raised"

    laid = layout.rebuild(found, bodies)
    if isinstance(laid, str):
        return data, laid

    # Whatever sits before the first instruction is BC's own module header --
    # 48 bytes of name and padding, and the only thing in these code segments
    # that is not in a body. Kept, and the layout starts after it.
    kept = min(op.at for _, body in bodies for op in layout._ordered(body))
    image = found.code[:kept] + laid.code
    made = relocate.as_records(
        records,
        found.seg,
        kept,
        image,
        laid.moved,
        {old: kept + new for new, old in laid.relocations},
    )
    if isinstance(made, str):
        return data, made
    return b"".join(record.emit() for record in made), REBUILT


REBUILT = "rebuilt"
