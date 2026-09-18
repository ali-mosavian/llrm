from dataclasses import replace
from pathlib import Path

import pytest
import corpus

from qbopt.model import ir
from qbopt.model import mir
from qbopt.optimize import transform


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("disturbed", [False, True])
def test_harr_reuses_descriptor_address_only_when_unchanged(tag, disturbed):
    """HARR rebuilt a dead high half and missed its identical descriptor address."""
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


def test_partial_computation_cse_keeps_a_required_high_half():
    """HARR's dead merged high half hid an address CSE; a live one must still block it."""
    path = Path("fixtures/omf/harr-p-g2.obj")
    module = corpus.loaded(path)
    body = mir.bodies(module, corpus.partitioned(path))[0][1]
    addresses = [
        op
        for block in body.blocks
        for op in block.ops
        if op.kind is mir.Kind.ADD and any(ref.symbolic for ref in op.loads)
    ]
    (_, later) = addresses
    held = next(result for result in later.results if isinstance(result, mir.Held))
    observer = mir.Op(
        later.at,
        ir.Operation.NOTHING,
        "",
        (),
        (held.value,),
        kind=mir.Kind.OPAQUE,
        args=(mir.Held(held.value, 4),),
    )
    body = replace(
        body,
        blocks=tuple(
            replace(block, ops=(*block.ops, observer)) if not block.succ else block for block in body.blocks
        ),
    )

    result = transform.subexpressions(body, module.dgroup)

    retained = [
        op
        for block in result.blocks
        for op in block.ops
        if op.kind is mir.Kind.ADD and any(ref.symbolic for ref in op.loads)
    ]
    assert len(retained) == 2
