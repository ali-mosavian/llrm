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

from dataclasses import dataclass
from enum import StrEnum

from qbopt import mir
from qbopt import omf
from qbopt import layout
from qbopt import regalloc
from qbopt import module
from qbopt import relocate
from qbopt import transform
from qbopt import blocks as split
from qbopt.blocks import code_map


class Emission(StrEnum):
    """Which emitter produced an object, or that none did.

    `rebuilt` says only whether it worked, and two emitters are coming:
    the one below, from MIR, and objwrite.py's, from LIR. A caller that
    has to tell them apart -- one whose output is a program rather than a
    body, and must not be raised again -- cannot ask a boolean, and "it
    worked" would treat a fallback as the other's own output.
    """

    LIR = "lir"
    MIR = "mir"
    REFUSED = "refused"


@dataclass(frozen=True)
class Emitted:
    """An object, what made it, and what it said about doing so."""

    data: bytes
    outcome: Emission
    reason: str


def emitted(
    data: bytes,
    optimise: bool = True,
    native_fpu: bool = False,
    only: str | None = None,
) -> Emitted:
    """The object rewritten, and which emitter did it."""
    out, why = _rebuilt(data, optimise, native_fpu, only)
    return Emitted(out, Emission.MIR if why == REBUILT else Emission.REFUSED, why)


def rebuilt(
    data: bytes,
    optimise: bool = True,
    native_fpu: bool = False,
    only: str | None = None,
) -> tuple[bytes, str]:
    """`emitted`, as every caller already reads it."""
    got = emitted(data, optimise, native_fpu, only)
    return got.data, got.reason


def _rebuilt(
    data: bytes,
    optimise: bool = True,
    native_fpu: bool = False,
    only: str | None = None,
) -> tuple[bytes, str]:
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

    blocks = split.partition(found, mapped)
    bodies = list(mir.bodies(found, blocks))
    if not bodies:
        return data, "no bodies were raised"

    # Optimised as values before being written as bytes. transform.py's own
    # docstring has why every original byte still has to be accounted for
    # after a deletion; `optimise=False` emits the body exactly as raised,
    # which is what a caller bisecting a layout question wants.
    plain = bodies
    if optimise:
        # Widening is not a MIR pass and is no longer in the list. It
        # recognises an idiom -- a long written as two halves joined by a
        # carry -- and writes the one 32-bit operation that replaces it,
        # which is machine form: 95 register references, all of them BC's
        # ax:dx convention. Recognition belongs at the raise and emission
        # below the boundary; until the two are separated it runs here,
        # after every pass and before lowering, which is where it ran
        # anyway and is where rule 5 puts it.
        def one(body):
            # `only="widen"` is the step on its own, which tools/stages.py
            # asks for; every other name selects a pass and leaves widening
            # out, so the two can be diffed apart.
            if only == "widen":
                return transform.widened(body)
            done = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found, only=only)
            return done if only is not None else transform.widened(done)

        bodies = [(name, one(body)) for name, body in bodies]

    # Every byte the decoder walked into, so layout.py can tell a gap it may
    # carry from one that is real code it simply did not raise.
    reached = frozenset(at for block in blocks for insn in block.insns for at in range(insn.at, insn.end))
    fields = frozenset(one.offset for one in omf.fixups(records) if one.seg == found.seg)
    # An allocation, where a pass asked for one. colour() gives back the
    # identity unless something pinned, so this costs nothing when nothing
    # did -- and refuses the pin rather than guessing when it cannot be had.
    # A body the allocator refuses is laid out as it was raised -- widened,
    # because widening writes machine form with the registers BC had, so it
    # needs no allocation and is right either way; without it nbody lost
    # every byte the object gained. Handed as the raise plus the step
    # rather than pre-widened, so the walk happens for the body that needs
    # it instead of for all of them.
    # Allocation first, and separately: the assembler emits what it is
    # handed. It used to colour inside rebuild(), where nothing downstream
    # could be told it had already happened -- objwrite.py runs after a
    # real allocator and was allocated over a second time, which produced a
    # call encoding with no field for its own fixup.
    settled, assignment = layout.allocated(
        bodies, plain=plain, settle=transform.widened if optimise else None
    )
    laid = layout.rebuild(
        found, settled, mapped.tables, fields, reached, native_fpu, assignment=assignment
    )
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
        {**laid.covered, **laid.moved},
        {old: kept + new for new, old in laid.relocations},
        laid.dropped,
    )
    if isinstance(made, str):
        return data, made
    return b"".join(record.emit() for record in made), REBUILT


REBUILT = "rebuilt"
