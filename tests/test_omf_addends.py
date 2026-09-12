from pathlib import Path

from tests import corpus
from qbopt.objectfile import omf
from qbopt.frontend import blocks
from qbopt.objectfile import module
from qbopt.frontend import fppatches
from qbopt.objectfile import addends


def test_borland_indexed_array_address_includes_encoded_addend() -> None:
    # Rebuilt d_faces drew zero triangles: 0CA0h in the instruction was lost.
    found = corpus.loaded(Path("fixtures/regressions/d_faces-borland.obj"))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    original = fppatches.native_records(found, mapped.starts)
    before = module.of(original)
    assert before is not None
    records = addends.canonical(original, found.seg, len(found.code))
    assert not isinstance(records, str)
    found = module.of(records)
    assert found is not None
    address = found.resolve(0x9F7, 0xCA0)
    assert address.disp == 0xCA0
    assert found.code[0x9F7:0x9F9] == b"\0\0"
    assert addends.canonical(records, found.seg, len(found.code)) is records
    assert not isinstance(blocks.code_map(found), str)
    old = {fix.offset: fix for fix in omf.fixups(original) if fix.seg == found.seg and fix.loc == omf.LOC_OFF16}
    new = {fix.offset: fix for fix in omf.fixups(records) if fix.seg == found.seg and fix.loc == omf.LOC_OFF16}
    assert old.keys() == new.keys()
    for at, fix in old.items():
        expected = (fix.disp + int.from_bytes(before.code[at : at + 2], "little")) & 0xFFFF
        assert (new[at].disp + int.from_bytes(found.code[at : at + 2], "little")) & 0xFFFF == expected
        assert (new[at].target, new[at].index, new[at].frame) == (fix.target, fix.index, fix.frame)
