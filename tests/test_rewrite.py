"""
The rewriter over the committed objects. No emulator, no compilers.

These are the invariants that stand between a change to the emitter and a
program that links, runs, and quietly computes the wrong thing.
"""

from pathlib import Path

import pytest

from qbopt import omf
from qbopt.rewrite import rewrite

pytestmark = pytest.mark.corpus


def test_a_dry_run_writes_the_input_back_unchanged(obj: Path) -> None:
    data = obj.read_bytes()
    out, _ = rewrite(data, dry_run=True)
    assert out == data


def test_a_dry_run_takes_no_region(obj: Path) -> None:
    _, found = rewrite(obj.read_bytes(), dry_run=True)
    assert [r for r in found if r.taken] == []


def test_a_real_pass_rewrites_the_code_and_keeps_the_records_readable(obj: Path) -> None:
    data = obj.read_bytes()
    out, found = rewrite(data, dry_run=False)
    if not any(region.taken for region in found):
        assert out == b"".join(record.emit() for record in omf.parse(data))
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


def test_rewriting_the_output_finds_nothing_new(obj: Path) -> None:
    out, _ = rewrite(obj.read_bytes(), dry_run=False)
    _, again = rewrite(out, dry_run=False)
    assert [r for r in again if r.taken] == []


def test_a_region_is_either_taken_or_says_why_not(obj: Path) -> None:
    _, found = rewrite(obj.read_bytes(), dry_run=False)
    for region in found:
        assert region.taken != bool(region.reason), "taken and refused are exclusive, and one holds"


def test_regions_are_taken_and_come_out_smaller(fixtures: Path) -> None:
    # A pass that refused everything would satisfy every invariant here.
    #
    # Measured: 195 of 229 regions are taken and shrink by a fifth. What is left
    # is 16 that cross a LEDATA boundary, 14 single pairs that still grow, and 4
    # that swallow a line number.
    taken = total = before = after = 0
    for path in sorted(fixtures.glob("*.obj")):
        _, found = rewrite(path.read_bytes(), dry_run=False)
        total += len(found)
        for region in found:
            if region.taken:
                taken += 1
                before += region.end - region.at
                after += len(region.after or "") // 2
    assert total > 200
    assert taken > total * 3 // 4, f"only {taken} of {total} regions taken"
    assert after < before * 85 // 100, "a taken region is meaningfully smaller"
