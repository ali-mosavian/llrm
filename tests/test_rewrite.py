"""The driver: an object in, an object out, and the same answers.

What this file used to test was the machine arm -- regions planned against
BC's own bytes, taken or refused, and the edits that combined them. The arm
is gone and its tests went with it. What is left is what the driver still
promises: the output parses, a second pass finds nothing new, and it
settles.
"""

from pathlib import Path

import pytest
from iced_x86 import Code

import corpus
from qbopt.objectfile import omf

pytestmark = pytest.mark.corpus

# The only opcodes calls.absorb() ever replaces a non-COMPARE call with, so
# a combined edit's own bytes must decode to at least one of them or the
# call's own arithmetic never actually happened.
#
# SHL and LEA are here because a multiply by a constant the 386 can do
# without multiplying does not emit an imul at all -- calls.SCALES and
# _without_multiplying(). They belong to this set for the same reason the
# imul forms do: they ARE the arithmetic, not something around it.
CALL_REPLACEMENT_OPCODES = {
    Code.IMUL_R32_RM32,
    Code.IMUL_R32_RM32_IMM8,
    Code.IMUL_R32_RM32_IMM32,
    Code.IMUL_RM32,
    Code.IDIV_RM32,
    Code.SHRD_RM32_R32_CL,
    Code.SHRD_RM32_R32_IMM8,
    Code.SHL_RM32_IMM8,
    Code.LEA_R32_M,
}


def test_a_real_pass_rewrites_the_code_and_keeps_the_records_readable(obj: Path) -> None:
    data = obj.read_bytes()
    out, found = corpus.rewritten(obj, dry_run=False)
    if not any(region.taken for region in found):
        # Byte-identical only with the segment left alone. Writing it from
        # MIR is a transform in its own right -- it picks shorter encodings
        # than BC's layout could use, 201 bytes over this corpus -- so it
        # applies whether or not a region was taken. The invariant that a
        # pass finding nothing changes nothing still holds for the patching
        # half, which is what this asks.
        from qbopt.rewrite import rewrite

        patched, _ = rewrite(data, dry_run=False, whole_segment=False)
        assert patched == b"".join(record.emit() for record in omf.parse(data))
        assert omf.parse(out), "and the rewritten segment is still a readable module"
        return
    before, after = omf.code_segment(omf.parse(data)), omf.code_segment(omf.parse(out))
    assert before is not None and after is not None
    # The segment may grow. Widening never makes it longer, but absorbing a
    # call can: a divide is eighteen bytes against fifteen under /G3, and what
    # it buys is a far call and the routine behind it.
    assert after[2] <= before[2] * 2, "and never by more than the code it replaces"
    before_names, after_names = omf.externals(omf.parse(data)), omf.externals(omf.parse(out))
    assert len(after_names) == len(before_names), "an absorbed call may drop its fixup but never its EXTDEF slot"
    live = {f.index for f in omf.fixups(omf.parse(out)) if f.target == "external"}
    assert all(after_names[i] == before_names[i] for i in live), (
        "a still-referenced external must keep the name its fixups were resolved against"
    )
    # an orphaned entry may be left with its own name -- the routine's own
    # .LIB satisfies it regardless of whether anything here still calls it --
    # or renamed to one something in the object still actually does call, but
    # never to a third, unrelated, unresolvable name
    for index in range(1, len(after_names)):
        if index in live:
            continue
        assert after_names[index] in (before_names[index], *(after_names[i] for i in live))


def test_rewriting_the_output_widens_or_absorbs_nothing_new(obj: Path) -> None:
    """A second pass finds no widening and no call left to absorb.

    It may find a load to delete, and that is not a failure of idempotence
    but the point: absorbing a call removes a barrier, and a store and
    reload the call used to sit between becomes visible only afterwards.
    procs-q-O ends up with `mov [bp-12h],eax` immediately followed by
    `mov eax,[bp-12h]`, which the first pass could not see because the call
    was still there when it looked.

    So the invariant is narrowed rather than dropped: everything that
    rewrites bytes in place must still reach a fixed point in one pass, and
    only deletion -- which is what a later pass creates work for -- may
    appear on the second. Running the pass to a fixed point would collect
    those too, and is its own piece of work.
    """
    out, _ = corpus.rewritten(obj, dry_run=False)
    _, again = corpus.rewritten(out, dry_run=False)
    left = [r for r in again if r.taken and r.after != ""]
    assert left == [], f"a second pass rewrote {len(left)} regions in place"


def test_rewriting_reaches_a_fixed_point(obj: Path) -> None:
    """Rewriting the output changes nothing further.

    One pass is not a fixed point: absorbing a call removes a barrier, and
    a store and reload the call used to sit between only becomes visible
    afterwards. Nine of the corpus's objects change bytes on a second pass
    and none on a third.
    """
    out, _ = corpus.rewritten(obj, dry_run=False)
    again, regions = corpus.rewritten(out, dry_run=False)
    assert again == out
    assert [one for one in regions if one.taken] == []


def test_a_finalised_object_is_given_back_before_anything_decodes_it() -> None:
    """What the LIR emitter writes is a program, not a body. Raising it
    again read `sub sp,4` as an ordinary subtract -- 35 ops in, 50 out on
    hotlop-q-evt -- and reserved a second frame on top of the first: S= 0
    for 630."""
    from qbopt import wholeseg
    from qbopt.rewrite import rewrite

    raw = Path("fixtures/omf/hotlop-q-evt.obj").read_bytes()
    once, _ = rewrite(raw, dry_run=False, absorb_calls=False)
    seen = []
    was = wholeseg.emitted
    try:
        wholeseg.emitted = lambda *a, **k: (seen.append(1), was(*a, **k))[1]
        again, _ = rewrite(once, dry_run=False, absorb_calls=False)
    finally:
        wholeseg.emitted = was
    assert again == once, "a finalised object came back changed"
    assert not seen, "it was decoded and rebuilt anyway"


def test_the_same_object_asked_for_other_options_is_refused() -> None:
    """Bytes produced for one set of options do not mean the same thing
    under another, and reinterpreting them silently is how a second run
    would quietly emit something else."""
    from qbopt.rewrite import rewrite
    from qbopt.rewrite import Finalised

    raw = Path("fixtures/omf/hotlop-q-evt.obj").read_bytes()
    once, _ = rewrite(raw, dry_run=False, absorb_calls=False)
    with pytest.raises(Finalised, match="absorb"):
        rewrite(once, dry_run=False, absorb_calls=True)


def test_exactly_one_marker_and_one_frame() -> None:
    from iced_x86 import Decoder
    from iced_x86 import Formatter
    from iced_x86 import FormatterSyntax

    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.rewrite import rewrite

    raw = Path("fixtures/omf/hotlop-q-evt.obj").read_bytes()
    out, _ = rewrite(raw, dry_run=False, absorb_calls=False)
    records = omf.parse(out)
    assert omf.finalised_at(records) is not None  # raises if there are two
    code = bytes(module.of(records).code)
    formatter = Formatter(FormatterSyntax.NASM)
    reserved = [one for one in Decoder(16, code[0x30:], ip=0x30) if formatter.format(one).startswith("sub sp")]
    assert len(reserved) <= 1, f"a frame reserved {len(reserved)} times"


def test_a_fallback_is_not_finalised_or_raised_again() -> None:
    """Re-raising fallback machine code loses SSA and can reserve a second frame."""
    from qbopt.objectfile import omf
    from qbopt.backend import allocate
    from qbopt import wholeseg
    from qbopt.rewrite import rewrite

    raw = Path("fixtures/omf/hotlop-q-evt.obj").read_bytes()
    was_alloc, was_emit = allocate.RegAlloc.transform, wholeseg.emitted
    seen: list[str] = []

    def refuses(self, body):
        raise allocate.Spilled("injected")

    def spy(*a, **k):
        got = was_emit(*a, **k)
        seen.append(got.outcome.value)
        return got

    allocate.RegAlloc.transform, wholeseg.emitted = refuses, spy
    try:
        out, _ = rewrite(raw, dry_run=False, absorb_calls=False)
    finally:
        allocate.RegAlloc.transform, wholeseg.emitted = was_alloc, was_emit

    assert seen and set(seen) == {"mir"}, seen
    assert len(seen) == 1, "fallback machine code was raised again"
    assert out == raw, "a failed backend must preserve the original object"
    assert omf.finalised_at(omf.parse(out)) is None, "a fallback was marked as final"


def test_a_refusal_is_unmarked_non_terminal_and_does_not_loop_for_nothing() -> None:
    """A refusal leaves BC's own bytes. Nothing is marked, and once they
    stop changing the driver stops asking."""
    from qbopt.objectfile import omf
    from qbopt import wholeseg
    from qbopt.rewrite import rewrite

    raw = Path("fixtures/omf/hotlop-q-evt.obj").read_bytes()
    was = wholeseg.emitted
    seen: list[str] = []

    def refuses(data, *a, **k):
        got = wholeseg.Emitted(data, wholeseg.Emission.REFUSED, "injected refusal")
        seen.append(got.outcome.value)
        return got

    wholeseg.emitted = refuses
    try:
        out, _ = rewrite(raw, dry_run=False, absorb_calls=False)
    finally:
        wholeseg.emitted = was

    assert set(seen) == {"refused"}, seen
    assert len(seen) == 1, f"it kept asking after the bytes were stable: {len(seen)}"
    assert out == raw, "a refusal changed the object"
    assert omf.finalised_at(omf.parse(out)) is None
