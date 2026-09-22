"""IVARG reloads an unchanged argument slot because its value survives the loop."""

from pathlib import Path
from dataclasses import replace

import pytest

from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.frontend import blocks
from qbopt.objectfile import module
from qbopt.optimize import transform
from qbopt.model.passes import Options


@pytest.mark.parametrize("tag", ["q-O", "p-g2", "v-g3"])
def test_argument_slot_is_loaded_once_before_the_emitted_loop(tag):
    from iced_x86 import Register

    import corpus
    from qbopt import wholeseg

    result = wholeseg.emitted(Path(f"fixtures/regressions/ivarg-{tag}.obj".lower()).read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    decoded = corpus.partitioned(result.data)
    hot = {at for loop in loops.loops(decoded) for at in loop.body}
    slots = [
        (block.at, one)
        for block in decoded
        for one in block.insns
        if one.insn.memory_base == Register.BP and one.insn.memory_displacement == 6
    ]
    assert len(slots) == 1 and slots[0][0] not in hot


@pytest.mark.parametrize("nonempty", [False, True])
def test_loop_carried_argument_slot_load_moves_only_when_loop_executes(nonempty):
    """IVARG must load bp+6 once, but must not change a zero-trip exit's carried value."""
    found = module.load(Path("fixtures/regressions/ivarg-p-g2.obj"))
    body = mir.bodies(found, blocks.partition(found, blocks.code_map(found)))[1][1]
    body = transform.applied(body, found.dgroup, found.calls, found=found, options=Options(hoist=False))
    (loop,) = loops.loops(body.blocks, body.entry)
    if not nonempty:
        header = body.block(loop.header)
        comparison = header.ops[-2]
        header = replace(
            header,
            ops=(*header.ops[:-2], replace(comparison, args=(comparison.args[0], mir.Const(0, 2))), header.ops[-1]),
        )
        body = replace(body, blocks=tuple(header if block.at == header.at else block for block in body.blocks))

    def slot(op):
        return any(
            ref.addr is not None and ref.addr.space is module.Space.FRAME and ref.addr.disp == 6 for ref in op.loads
        )

    assert any(slot(op) for block in body.blocks if block.at in loop.body for op in block.ops)
    after = transform.hoisted(body, found.dgroup, found.calls, module.landmarks(found))
    locations = [block.at for block in after.blocks for op in block.ops if slot(op)]
    assert len(locations) == 1
    assert (locations[0] not in loop.body) is nonempty
    assert any(
        ref.base is not None for block in after.blocks if block.at in loop.body for op in block.ops for ref in op.loads
    )
