from dataclasses import replace
from pathlib import Path

import pytest
import corpus

from qbopt import mir, transform


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("disturbed", [False, True])
def test_harr_reuses_descriptor_address_only_when_unchanged(tag, disturbed):
    """HARR computed its identical descriptor-adjusted address twice per element."""
    path = Path(f"fixtures/omf/harr-{tag}.obj")
    module = corpus.loaded(path)
    body = mir.bodies(module, corpus.partitioned(path))[0][1]

    def change(op):
        if disturbed and any(ref.allocation for ref in op.stores):
            return replace(op, stores=(mir.MemRef(None, 2),))
        return op

    body = replace(body, blocks=tuple(replace(block, ops=tuple(map(change, block.ops))) for block in body.blocks))
    result = transform.subexpressions(body, module.dgroup)
    addresses = [
        op
        for block in result.blocks
        for op in block.ops
        if op.kind is mir.Kind.ADD and any(ref.symbolic for ref in op.loads)
    ]
    assert len(addresses) == (2 if disturbed else 1)
