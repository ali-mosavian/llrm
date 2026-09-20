"""Prove concrete loop paths before claiming array/static disjointness."""

from dataclasses import replace
from pathlib import Path


import pytest
import corpus

from qbopt.analysis import consts
from qbopt.model import mir
from qbopt.frontend import raising_array_bounds
from qbopt.objectfile.module import Addr, Space


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_nine_dimensional_loop_proves_its_element_store_extent(tag):
    """NDARR's OR-based zero test defeated the extent proof despite valid 1,12,2 output."""
    path = Path(f"fixtures/regressions/ndarr-{tag}.obj".lower())
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    stores = [ref for block in body.blocks for op in block.ops for ref in op.stores if ref.pointer]
    assert stores
    assert all(ref.allocation is not None for ref in stores)


def test_unknown_logical_loop_condition_proves_no_array_extent(monkeypatch):
    """NDARR's 1,12,2 result cannot justify an extent when its loop test is unknown."""
    with monkeypatch.context() as context:
        context.setattr(raising_array_bounds, "proven", lambda body: body)
        path = Path("fixtures/regressions/ndarr-p-g2.obj")
        body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]

    def change(op):
        if op.kind is mir.Kind.OR and any(value.flags for value in op.defines):
            unknown = mir.Held(mir.Value(900, 0), 2)
            return replace(op, args=(unknown, unknown))
        return op

    changed = replace(body, blocks=tuple(replace(block, ops=tuple(map(change, block.ops)))
                                         for block in body.blocks))
    assert changed != body
    result = raising_array_bounds.proven(changed)
    assert not any(ref.allocation for block in result.blocks for op in block.ops
                   for ref in (*op.loads, *op.stores))


def raw_huge(tag, monkeypatch):
    with monkeypatch.context() as context:
        context.setattr(raising_array_bounds, "proven", lambda body: body)
        path = Path(f"fixtures/regressions/hugelp-{tag}.obj".lower())
        found = corpus.loaded(path)
        body = mir.bodies(found, corpus.partitioned(path))[0][1]
    return found, body


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_huge_loop_descriptor_loads_leave_the_loop(tag, monkeypatch):
    """HUGELP reloaded bounds and pointer twice per iteration despite bounded heap stores."""
    from qbopt import wholeseg
    from qbopt.analysis import loops as loopy
    found, body = raw_huge(tag, monkeypatch)
    proven = raising_array_bounds.proven(body)
    loop = loopy.loops(list(proven.blocks), proven.entry)[0]
    stores = [ref for block in proven.blocks if block.at in loop.body for op in block.ops
              for ref in op.stores if ref.pointer]
    assert len(stores) == 2 and all(ref.allocation is not None for ref in stores)
    states = []
    def watch(stage, name, state):
        if stage == "mir-widen":
            states.append(state)
    result = wholeseg.emitted(Path(f"fixtures/regressions/hugelp-{tag}.obj".lower()).read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    after = states[0]
    loops = loopy.loops(list(after.blocks), after.entry)
    repeated = {at for loop in loops for at in loop.body}
    assert not any(ref.addr is not None and ref.addr.space is Space.SEGMENT
                   and ref.addr.index == found.program_data and 6 <= ref.addr.disp < 28
                   for block in after.blocks if block.at in repeated for op in block.ops for ref in op.loads)


@pytest.mark.parametrize("failure", ["budget", "out_of_bounds", "descriptor_write", "unknown_call"])
def test_huge_proof_does_not_assume_its_own_disjointness(failure, monkeypatch):
    from qbopt.model import ir
    found, body = raw_huge("p-g2", monkeypatch)
    def change(op):
        if failure == "out_of_bounds" and op.kind is mir.Kind.PTR_OFFSET:
            return replace(op, args=(op.args[0], mir.Const(80802, 4)))
        if op.kind is mir.Kind.STORE and any(ref.pointer for ref in op.stores):
            if failure == "descriptor_write":
                ref = mir.MemRef(Addr(Space.SEGMENT, 24, found.program_data), 2)
                return replace(op, stores=(ref,), results=(mir.Cell(ref),))
            if failure == "unknown_call":
                return mir.Op(op.at, ir.Operation.CALL, "call", (), (), kind=mir.Kind.CALL)
        return op
    changed = replace(body, blocks=tuple(replace(block, ops=tuple(map(change, block.ops))) for block in body.blocks))
    result = raising_array_bounds.proven(changed, limit=1 if failure == "budget" else 10000)
    assert not any(ref.allocation for block in result.blocks for op in block.ops for ref in (*op.loads, *op.stores))


def raw(name, tag, monkeypatch):
    with monkeypatch.context() as context:
        context.setattr(raising_array_bounds, "proven", lambda body: body)
        path = Path(f"fixtures/omf/{name}-{tag}.obj".lower())
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
