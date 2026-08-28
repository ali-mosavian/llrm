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
    assert after[2] <= before[2], "widening a region never makes the segment longer"
    assert omf.externals(omf.parse(out)) == omf.externals(omf.parse(data)), (
        "an absorbed call may drop its fixup but never its EXTDEF, or every later index shifts"
    )


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
