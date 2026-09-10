"""A new huge-pointer runtime dependency must not retarget BC's existing calls."""

from pathlib import Path

import pytest

from qbopt.objectfile import omf


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_adding_pointer_dependency_keeps_existing_fixups(tag):
    records = omf.read(Path(f"fixtures/regressions/huge2-{tag}.obj"))
    before = omf.externals(records)
    added, index = omf.with_external(records, "b$HugeShift")
    assert index == len(before)
    assert omf.externals(added) == [*before, "b$HugeShift"]
    assert omf.fixups(added) == omf.fixups(records)
    assert [record for record in added if record not in records] == [
        omf.extdef_record([(b"b$HugeShift", b"\x00")])]
    repeated, again = omf.with_external(added, "b$HugeShift")
    assert again == index and repeated == added
    assert omf.parse(b"".join(record.emit() for record in added)) == added
