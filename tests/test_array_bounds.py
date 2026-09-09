"""Prove concrete loop paths before claiming array/static disjointness."""

from dataclasses import replace
from pathlib import Path

import pytest
import corpus

from qbopt import consts, mir, raising_array_bounds
from qbopt.module import Addr, Space


def raw(name, tag, monkeypatch):
    with monkeypatch.context() as context:
        context.setattr(raising_array_bounds, "proven", lambda body: body)
        path = Path(f"fixtures/omf/{name}-{tag}.obj")
        module = corpus.loaded(path)
        body = mir.bodies(module, corpus.partitioned(path))[0][1]
    return module, body


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_harr_dimension_survives_element_stores(tag, monkeypatch):
    """HARR repeatedly loaded dimension 21 because its bounded element stores appeared to overwrite it."""
    module, body = raw("harr", tag, monkeypatch)
    result = raising_array_bounds.proven(body)
    facts = consts.known(result, module.dgroup, module.calls)
    dimension = Addr(Space.SEGMENT, 24, module.program_data)
    loads = [
        op
        for block in result.blocks
        for op in block.ops
        if op.kind is mir.Kind.LOAD and any(ref.addr == dimension for ref in op.loads)
    ]
    assert loads
    assert all(facts.get(op.results[0].value) == consts.Known(21, 2) for op in loads)


@pytest.mark.parametrize("failure", ["budget", "out_of_bounds", "unknown_condition", "descriptor_write", "segment"])
def test_failed_path_proof_discards_all_disjointness(failure, monkeypatch):
    module, body = raw("harr", "p-g2", monkeypatch)

    def change(op):
        if failure in ("out_of_bounds", "unknown_condition") and op.at == 0x91:
            bound = mir.Const(30, 2) if failure == "out_of_bounds" else mir.Held(mir.Value(900, 0), 2)
            return replace(op, args=(op.args[0], bound))
        if failure == "descriptor_write" and op.at == 0x8E:
            ref = mir.MemRef(Addr(Space.SEGMENT, 24, module.program_data), 2)
            return replace(op, stores=(ref,), results=(mir.Cell(ref),))
        if failure == "segment" and op.at == 0x75:
            ref = mir.MemRef(Addr(Space.SEGMENT, 100, module.program_data), 2)
            return replace(op, loads=(ref,), args=(mir.Cell(ref),))
        return op

    body = replace(body, blocks=tuple(replace(block, ops=tuple(map(change, block.ops))) for block in body.blocks))
    assert raising_array_bounds.proven(body, limit=1 if failure == "budget" else 10000) is body
