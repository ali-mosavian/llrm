from pathlib import Path

from iced_x86 import Register

from tests import corpus
from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import lower
from qbopt.backend import select


def test_ls_animate_string_address_is_explicit_and_round_trips() -> None:
    # LS_ANIMATE passed [bp-20h] as a string descriptor, but MIR showed &?.
    path = Path("fixtures/regressions/qrender-d-surf-v-g3.obj")
    found = corpus.loaded(path)
    assert found is not None
    raised = mir.bodies(found, corpus.partitioned(path))
    body = next(body for name, body in raised if name.endswith(" LS_ANIMATE"))
    addresses = [op for block in body.blocks for op in block.ops if op.at in (0x2BB, 0x2C4)]
    assert len(addresses) == 2
    for op in addresses:
        assert not isinstance(op.args[0], mir.Opaque)
        assert op.args[0] == mir.FrameAddress(-32, 2)
        assert not op.loads and not op.stores
        machine = lower.current(op, node=raised.source.nodes.get(op.id))
        assert machine is not None
        assert machine.op is ir.Operation.ADDRESS
        source = machine.sources[0]
        assert isinstance(source, ir.Address)
        assert source.through == Register.BP
        assert source.index == Register.NONE
        assert source.offset == -32
        emitted = select.emit(machine, op.at)
        assert emitted is not None
        assert emitted.code == found.code[op.at : op.at + 3]
