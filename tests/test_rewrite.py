"""The driver: an object in, an object out, and the same answers.

What this file used to test was the machine arm -- regions planned against
BC's own bytes, taken or refused, and the edits that combined them. The arm
is gone and its tests went with it. What is left is what the driver still
promises: the output parses, a second pass finds nothing new, and it
settles.
"""

from pathlib import Path

import pytest

import corpus
from qbopt.objectfile import omf

pytestmark = pytest.mark.corpus


def test_a_real_pass_writes_a_complete_fresh_object(obj: Path) -> None:
    """Every accepted corpus object is a self-contained OMF module.

    The retired test compared the output's record count and EXTDEF slots with
    BC's input. A fresh writer owns both and may choose either. What LINK needs
    is an intact module whose live external fixups all resolve.
    """
    out, _ = corpus.rewritten(obj, dry_run=False)
    records = omf.parse(out)
    assert records[0].type == omf.THEADR
    assert records[-1].type & 0xFE == omf.MODEND
    assert omf.code_segment(records) is not None
    assert omf.finalised_at(records) is not None
    names = omf.externals(records)
    live = [fixup for fixup in omf.fixups(records) if fixup.target == "external"]
    assert all(0 < fixup.index < len(names) and names[fixup.index] for fixup in live)


def test_rewriting_reaches_a_fixed_point(obj: Path) -> None:
    """A finalized program is returned byte-for-byte without being raised."""
    out, _ = corpus.rewritten(obj, dry_run=False)
    again, _ = corpus.rewritten(out, dry_run=False)
    assert again == out


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
    from qbopt.rewrite import rewrite
    from qbopt.objectfile import module

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
    from qbopt import wholeseg
    from qbopt.objectfile import omf
    from qbopt.rewrite import rewrite
    from qbopt.backend import allocate

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
        out, _ = rewrite(raw, dry_run=False, absorb_calls=False, allow_unchanged=True)
    finally:
        allocate.RegAlloc.transform, wholeseg.emitted = was_alloc, was_emit

    assert seen == ["refused"], "BC's own machine code was raised again"
    assert out == raw, "a failed backend must preserve the original object"
    assert omf.finalised_at(omf.parse(out)) is None, "a fallback was marked as final"


def test_a_refusal_is_unmarked_non_terminal_and_does_not_loop_for_nothing() -> None:
    """A refusal raises. Kept only when asked for: BC's own bytes, unmarked,
    and asked once."""
    from qbopt import wholeseg
    from qbopt.objectfile import omf
    from qbopt.rewrite import rewrite
    from qbopt.rewrite import Unsupported

    raw = Path("fixtures/omf/hotlop-q-evt.obj").read_bytes()
    was = wholeseg.emitted
    seen: list[str] = []

    def refuses(data, *a, **k):
        got = wholeseg.Emitted(data, wholeseg.Emission.REFUSED, "injected refusal")
        seen.append(got.outcome.value)
        return got

    wholeseg.emitted = refuses
    try:
        with pytest.raises(Unsupported, match="injected refusal"):
            rewrite(raw, dry_run=False, absorb_calls=False)
        out, _ = rewrite(raw, dry_run=False, absorb_calls=False, allow_unchanged=True)
    finally:
        wholeseg.emitted = was

    assert seen == ["refused", "refused"], f"one question per call: {seen}"
    assert out == raw, "a refusal changed the object"
    assert omf.finalised_at(omf.parse(out)) is None
