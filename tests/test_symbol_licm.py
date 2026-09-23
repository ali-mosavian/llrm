from pathlib import Path

import corpus
import pytest

from qbopt.model import mir
from qbopt.optimize import transform
from qbopt import wholeseg


@pytest.mark.parametrize("program", ["harr", "segld"])
@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_descriptor_address_survives_hoisting_out_of_nested_loops(program: str, tag: str) -> None:
    """HARR reloaded its descriptor 100 times; hoisting lost the jump at 0x55 and refused emission."""
    path = Path(f"fixtures/omf/{program}-{tag}.obj".lower())
    module = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = mir.bodies(module, partition)[0][1]
    descriptor = next(op.array.descriptor for block in body.blocks for op in block.ops if op.array)
    result = transform.applied(body, module.dgroup, module.calls, blocks=partition, found=module)
    addresses = [
        (block, op)
        for block in result.blocks
        for op in block.ops
        if op.kind is mir.Kind.COPY and descriptor in op.args
    ]
    assert addresses
    assert all(block.at == result.entry for block, op in addresses)
    emitted = wholeseg.emitted(path.read_bytes())
    assert emitted.outcome is wholeseg.Emission.LIR, emitted.reason
