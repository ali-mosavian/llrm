"""
A whole Body, replaced rather than widened in place.

relocate.py's Edit/Shift/relocate() already work at the whole code-segment
level: an Edit is a byte span and a Shift a set of them, and neither assumes
the span came from lift.py's own peephole regions or calls.py's absorption
sites -- relocate() rewrites every code LEDATA's span, every FIXUPP offset and
target displacement anywhere in the file, every PUBDEF/LINNUM, the SEGDEF
length, and (via retarget_branches) every self-relative branch in the whole
segment, all generically, none of it scoped to "inside one block."

What is new here is the one check neither lift.py nor calls.py ever needed:
does this edit stay inside the Body (extent.py) it means to replace. Unlike a
region, a Body may be several disjoint ranges -- extent.py's own docstring:
a SUB/FUNCTION sits exactly where BC laid it out in source order, and the
main body's own control flow runs around each one, so the main body is
generally several byte ranges rather than one contiguous prefix.

Composing a whole-body edit with rewrite.py's own region edits in the same
Shift is deliberately not attempted here. Nothing in Shift/relocate() stops
it, but nothing here proves it safe, and it is exactly the shape of
AGENTS.md's two documented traps (a boundary two edits both move; a chunk two
edits touch from either side) if a region edit and a whole-body edit ever
shared a LEDATA chunk. Left for commit 3, when an optimizer actually needs
both kinds of edit in the same rewrite.
"""

from qbopt import ir
from qbopt import omf
from qbopt import module
from qbopt.extent import Body
from qbopt.relocate import Edit
from qbopt.relocate import Shift
from qbopt.extent import BodyKind
from qbopt.relocate import relocate


def edit(body: Body, at: int, hi: int, data: bytes) -> Edit | str:
    """A replacement for body's own [at, hi) -- hi == at is a pure insertion.

    relocate() is the authority on whether at/hi are real instruction
    boundaries and whether the span clears a table or an ON GOTO/RESUME
    table (tested already by test_relocate.py, unaffected by this module);
    what this adds is the fact relocate() cannot know on its own, which body
    a byte belongs to. [at, hi) must sit inside exactly one of body's own
    ranges. A pure insertion (at == hi) exactly on that range's own leading
    or trailing edge is refused either way: on the trailing edge it is
    ambiguous with whatever comes right after (another body's own range, a
    gap, a table); on the leading edge it is placed in front of whatever a
    branch targeting that offset lands on, so the inserted bytes are never
    reached from there -- proven on fixtures/omf/*-evt.obj, whose main body
    opens with a jmp short over the event-poll stub straight onto its second
    range's own first byte.
    """
    if at > hi:
        return f"{at:#x}..{hi:#x} is backwards"
    owner = next((r for r in body.ranges if r[0] <= at and hi <= r[1]), None)
    if owner is None:
        return f"{at:#x}..{hi:#x} is not inside one range of {body.kind} {body.name or ''}"
    if at == hi and at in owner:
        return f"{at:#x} is a pure insertion exactly on a range edge of {body.kind} {body.name or ''}"
    return Edit(at, hi, data, ())


def insert_nop(body_ir: ir.BodyIR) -> Edit | str:
    """One 0x90 at the first legal instruction boundary after body's own first node.

    Skipping index 0 keeps the entry point itself untouched -- MAIN's is
    blocks.ENTRY, fixed by the runtime header, and a PROCEDURE's is its own
    PUBDEF. Everything after that is a candidate; edit() is the one place
    that decides whether a candidate's own position is legal, so this only
    ever tries candidates in order and takes the first one accepted. A Data
    node (an inline table) is skipped outright -- its own start is never a
    real instruction, and relocate() would refuse it regardless.
    """
    for node in body_ir.nodes[1:]:
        if isinstance(node, ir.Data):
            continue
        at, _ = ir.span(node)
        made = edit(body_ir.body, at, at, b"\x90")
        if not isinstance(made, str):
            return made
    return "no instruction boundary after the body's own first node accepts an insertion"


def rewritten(data: bytes, body_kind: BodyKind) -> bytes:
    """The object with one nop inserted into the first body of this kind, otherwise untouched.

    The same shape a real whole-body replacement will use once commit 3 has
    one to offer, minus the replacement itself: parse, decode, build one
    Edit, hand it to the same relocate() a region edit already goes through.
    """
    records = omf.parse(data)
    found = module.of(records)
    if found is None:
        raise ValueError("no code segment")
    decoded = ir.decode_module(found)
    if isinstance(decoded, str):
        raise ValueError(decoded)
    body_ir = next((b for b in decoded if b.body.kind == body_kind), None)
    if body_ir is None:
        raise ValueError(f"no {body_kind} body")
    made = insert_nop(body_ir)
    if isinstance(made, str):
        raise ValueError(made)
    moved = relocate(records, found.seg, found.code, Shift.of([made]))
    if isinstance(moved, str):
        raise ValueError(moved)
    return b"".join(record.emit() for record in moved)
