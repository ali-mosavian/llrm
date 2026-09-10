"""HUGE2 reads INTEGERs across 64K: expected 123, 456, 789, never wrapped aliases."""

from pathlib import Path

import corpus
import pytest

from qbopt import wholeseg
from qbopt.model import mir
from qbopt.objectfile import module, omf


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_huge_helper_becomes_whole_pointer_mir_and_emitted_accesses(tag):
    path = Path(f"fixtures/regressions/huge2-{tag}.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    ops = [op for block in body.blocks for op in block.ops]
    assert sum(op.kind is mir.Kind.PTR_OFFSET for op in ops) == 10
    pointers = {value for op in ops if op.kind is mir.Kind.PTR_OFFSET for value in op.defines}
    assert not any(op.kind is mir.Kind.EXTRACT and any(value in pointers for value in op.uses) for op in ops)
    assert not any(op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$HARY" for op in ops)
    refs = [ref for op in ops for ref in (*op.loads, *op.stores) if ref.pointer]
    assert len(refs) == 10
    assert all(ref.addr is None and ref.segment is None and ref.base_width == 4 for ref in refs)
    result = wholeseg.emitted(path.read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    output = module.of(omf.parse(result.data))
    assert "B$HARY" not in output.calls.values()
    names = omf.externals(output.records)
    assert any(fix.target == "external" and names[fix.index] == "b$HugeShift"
               for fix in omf.fixups(output.records))
    # VBDOS printed 123,789,789 when all newly introduced descriptor
    # loads read DS:0: zero displacement bytes had no linker relocation.
    descriptors = {fix.disp for fix in omf.fixups(output.records)
                   if fix.seg == output.seg and fix.target == "segment" and fix.index == 5}
    assert {6, 22, 24, 26} <= descriptors
