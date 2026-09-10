"""HUGE2 reads INTEGERs across 64K: expected 123, 456, 789, never wrapped aliases."""

from pathlib import Path

import corpus
import pytest

from qbopt import wholeseg
from qbopt.model import mir
from qbopt.objectfile import module, omf


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program,count", [("huge2", 10), ("hugelp", 6)])
def test_huge_helper_becomes_whole_pointer_mir_and_emitted_accesses(tag, program, count):
    path = Path(f"fixtures/regressions/{program}-{tag}.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    ops = [op for block in body.blocks for op in block.ops]
    assert sum(op.kind is mir.Kind.PTR_OFFSET for op in ops) == count
    pointers = {value for op in ops if op.kind is mir.Kind.PTR_OFFSET for value in op.defines}
    assert not any(op.kind is mir.Kind.EXTRACT and any(value in pointers for value in op.uses) for op in ops)
    assert not any(op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$HARY" for op in ops)
    refs = [ref for op in ops for ref in (*op.loads, *op.stores) if ref.pointer]
    assert len(refs) == count
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


def test_selector_proof_checks_the_loop_exit_path():
    """HUGELP may remove HARY's selector only when neither backedge nor exit observes it."""
    from dataclasses import replace
    from qbopt.abi import runtime
    from qbopt.frontend import raising_array_access

    path = Path("fixtures/regressions/hugelp-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path), bounds_checks=True)[0][1]
    contracts = runtime.for_module(found)
    block = next(block for block in body.blocks if sum(
        op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$HARY" for op in block.ops) == 2)
    position, consumer = [(index, op) for index, op in enumerate(block.ops)
                          if op.kind is mir.Kind.STORE and any(ref.base is not None for ref in op.stores)][-1]
    assert raising_array_access._selector_dead(body, block, position + 1, contracts)
    exit_block = body.blocks[-1]
    changed = replace(body, blocks=tuple(replace(one, ops=(consumer, *one.ops))
                                        if one is exit_block else one for one in body.blocks))
    assert not raising_array_access._selector_dead(changed, block, position + 1, contracts)


def test_offset_overwrite_keeps_a_transitively_observed_high_half():
    """Removing HUGELP's dead offset merges must not erase high bits a later whole read uses."""
    from qbopt.frontend import raising_array_access
    from qbopt.model import ir
    old, first, second = (mir.Value(index, index, variable=index) for index in (1, 2, 3))
    copies = tuple(mir.Op(index, ir.Operation.MOVE, "mov", (result,), (source,),
        kind=mir.Kind.COPY, args=(mir.Const(index, 2),), results=(mir.Held(result, 2),),
        merges={source: result}) for index, source, result in ((0, old, first), (1, first, second)))
    read = mir.Op(2, ir.Operation.PUSH, "push", (), (second,), kind=mir.Kind.ARG,
                  args=(mir.Held(second, 4),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (*copies, read), ()),))
    assert not raising_array_access._overwrites_offset(body, copies[0], old)
