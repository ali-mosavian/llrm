"""Address-space identities survive optimization without selecting registers in MIR passes."""

from dataclasses import replace
from pathlib import Path

import pytest
from iced_x86 import Register

from qbopt.frontend import blocks
from qbopt.model import ir
from qbopt.objectfile import module, omf
from qbopt import wholeseg


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_harr_reuses_one_selector_and_forwards_the_array_store(tag):
    """HARR reloaded its selector inside the loop; unequal names also retained an array read."""
    result = wholeseg.emitted(Path(f"fixtures/omf/harr-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    reached = list(blocks.instructions(found))
    selectors = [one.insn for one in reached if one.insn.op0_register == Register.ES]
    assert len(selectors) == 1
    for one in reached:
        branch = one.insn
        if branch.is_jcc_short_or_near or branch.is_jmp_short_or_near:
            assert not branch.near_branch_target <= selectors[0].ip <= branch.ip
    assert not any(one.insn.memory_segment == Register.ES and one.insn.op0_register != Register.NONE
                   for one in reached)


def test_a_clobber_ends_the_raised_selector_dependency():
    from types import SimpleNamespace
    from qbopt.model import mir
    from qbopt.frontend import raising_addresses
    from qbopt.objectfile.module import Addr, Space

    descriptor = mir.MemRef(Addr(Space.SEGMENT, 2, 5), 2)
    element = mir.MemRef(Addr(Space.FAR, 0, segment=Register.ES), 2)
    selector = mir.Op(0, ir.Operation.MOVE, "mov", (), (), loads=(descriptor,),
                      kind=mir.Kind.LOAD, args=(mir.Cell(descriptor),), results=(mir.Opaque(None, "es"),))
    def store(at):
        node = SimpleNamespace(effects=SimpleNamespace(uses=frozenset({Register.ES}), defs=frozenset()))
        return mir._RaisedOp(at, ir.Operation.MOVE, "mov", (), (), stores=(element,), node=node,
                             kind=mir.Kind.STORE, results=(mir.Cell(element),))
    clobber = mir.Op(2, ir.Operation.CALL, "call", (), (), kind=mir.Kind.CALL)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (selector, store(1), clobber, store(3)), ()),), {})
    raised = raising_addresses.loaded(body).blocks[0].ops
    value, = raised[0].defines
    assert raised[1].stores[0].segment == value
    assert value in raised[1].uses
    assert value in raised[2].uses
    after = raised[3].stores[0].segment
    assert after != value and after in raised[2].defines, "the store after the call still names the old selector"


def test_long_extraction_preserves_the_far_store_selector():
    """D_SURF made 163 descriptors instead of 21: long extracts orphaned the store's ES."""
    from qbopt.model import mir
    from qbopt.frontend import raising_addresses
    from qbopt.backend import lower
    from qbopt.objectfile.module import Addr, Space
    from types import SimpleNamespace

    descriptor = mir.MemRef(Addr(Space.SEGMENT, 2, 5), 2)
    element = mir.MemRef(Addr(Space.FAR, 0, segment=Register.ES), 4)
    whole, low = mir.Value(1, 0), mir.Value(2, 2)
    selector = mir.Op(1, ir.Operation.MOVE, "mov", (), (), loads=(descriptor,),
                      kind=mir.Kind.LOAD, args=(mir.Cell(descriptor),), results=(mir.Opaque(None, "es"),))
    extract = mir.Op(2, mir.Synth.HALF_TO_LOW, "extract", (low,), (whole,),
                     kind=mir.Kind.EXTRACT, args=(mir.Held(whole, 4), mir.Const(0, 1)),
                     results=(mir.Held(low, 2),))
    store = mir._RaisedOp(
        3,
        ir.Operation.MOVE,
        "mov",
        (),
        (whole,),
        stores=(element,),
        kind=mir.Kind.STORE,
        args=(mir.Held(whole, 4),),
        results=(mir.Cell(element),),
        id=3,
        node=SimpleNamespace(
            semantics=ir.Semantics(
                ir.Operation.MOVE,
                "mov",
                (ir.Mem(element.addr, 4, Register.BX),),
                (ir.Reg(Register.EAX, 4),),
            )
        ),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (selector, extract, store), ()),), {})
    raised = raising_addresses.loaded(body)
    first, _, last = raised.blocks[0].ops
    last = replace(last, source_backed=True)
    raised = replace(raised, blocks=(replace(raised.blocks[0], ops=(first, raised.blocks[0].ops[1], last)),))
    segment, = first.defines
    write, = lower.Lowering(
        raised, {whole.id, low.id, segment.id}, {}, (), {}, nodes={last.id: last.node}
    ).expand(last)
    # The cell carries its selector; the allocator seats it in a segment register.
    assert write.what.dests[0].selector == ir.Held(segment.id, 2)
    assert segment.id in write.uses
