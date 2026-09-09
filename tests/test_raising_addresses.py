"""Address-space identities survive optimization without selecting registers in MIR passes."""

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
        return mir.Op(at, ir.Operation.MOVE, "mov", (), (), stores=(element,), node=node,
                      kind=mir.Kind.STORE, results=(mir.Cell(element),))
    clobber = mir.Op(2, ir.Operation.CALL, "call", (), (), kind=mir.Kind.CALL)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (selector, store(1), clobber, store(3)), ()),), {})
    raised = raising_addresses.loaded(body).blocks[0].ops
    value, = raised[0].defines
    assert raised[1].stores[0].segment == value
    assert value in raised[1].uses
    assert value in raised[2].uses
    assert raised[3].stores[0].segment is None
