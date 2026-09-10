"""IVARM's invariant choice should be tested once, not on all ten iterations."""

from dataclasses import replace
from pathlib import Path

import pytest

from qbopt.analysis import loops
from qbopt.frontend import blocks
from qbopt.model import mir
from qbopt.objectfile import module
from qbopt.optimize import edges, transform, unswitch


@pytest.mark.parametrize("tag", ["q-O", "p-g2", "v-g3"])
def test_production_ivarm_has_no_loop_and_stores_last_value(tag):
    """IVARM should branch once and store 34; its final counter remains 37."""
    from qbopt import wholeseg
    import corpus

    result = wholeseg.emitted(Path(f"fixtures/regressions/ivarm-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    decoded = corpus.partitioned(result.data)
    assert not loops.loops(decoded)
    instructions = [str(one.insn) for block in decoded for one in block.insns]
    assert sum(one.startswith("mov word ptr ") and one.endswith(",22h") for one in instructions) == 2


def original(tag):
    found = module.load(Path(f"fixtures/regressions/ivarm-{tag}.obj"))
    body = mir.bodies(found, blocks.partition(found, blocks.code_map(found)))[0][1]
    return found, transform.applied(body, found.dgroup, found.calls, found=found, unroll_=False)


@pytest.mark.parametrize("tag", ["q-O", "p-g2", "v-g3"])
def test_invariant_branch_specialization_exposes_loop_deletion(tag):
    """IVARM repeated its invariant branch ten times; specialized loops must retain usable exits."""
    found, body = original(tag)
    candidate = unswitch.specialized(body)
    assert candidate is not body
    predecessors = loops.predecessors(candidate.blocks)
    for block in candidate.blocks:
        for phi in block.phis:
            assert set(phi.incoming) == set(predecessors[block.at])
    result = transform.applied(candidate, found.dgroup, found.calls, unroll_=False)
    assert not loops.loops(result.blocks, result.entry)


def test_cloning_provenance_survives_ssa_reconstruction():
    """Cloned IVARM blocks must not be re-sorted by original instruction addresses."""
    _, body = original("p-g2")
    candidate = unswitch.specialized(body)
    assert candidate.cloned
    assert mir.resolved(candidate).cloned


def test_unswitch_rejects_a_candidate_without_loop_removal(monkeypatch):
    """Duplicating IVARM without simplifying its loops is not a profitable default."""
    found, body = original("p-g2")
    monkeypatch.setattr(transform, "applied", lambda body, *args, **kwargs: body)
    assert unswitch.optimized(body, found.dgroup, found.calls) is body


def test_implicit_edge_bridge_does_not_retarget_the_taken_arm():
    """IVARM's dispatch needs distinct preheaders on both sides of its condition."""
    _, body = original("p-g2")
    loop, = loops.loops(body.blocks, body.entry)
    header = body.block(loop.header)
    target, = set(header.succ) - {header.ops[-1].target}
    label = edges.fresh(body)
    result = edges.split(body, header.at, target, label, ())
    assert result.block(header.at).ops[-1].target == header.ops[-1].target
    assert set(result.block(header.at).succ) == {header.ops[-1].target, label}
    assert result.block(label).succ == (target,)


@pytest.mark.parametrize("variant", ["memory", "variant"])
def test_condition_must_be_pure_and_loop_invariant(variant):
    _, body = original("p-g2")
    loop, = loops.loops(body.blocks, body.entry)
    selected = next(block for block in body.blocks
                    if block.at in loop.body and block.at != loop.header and len(block.succ) == 2)
    index, compare = transform._comparison(selected, selected.ops[-1])
    if variant == "memory":
        compare = replace(compare, loads=(mir.MemRef(None, 2),))
    else:
        carried = body.block(loop.header).phis[0].result
        compare = replace(compare, args=(mir.Held(carried, 2), mir.Const(0, 2)), uses=(carried,))
    selected = replace(selected, ops=(*selected.ops[:index], compare, *selected.ops[index + 1:]))
    body = replace(body, blocks=tuple(selected if block.at == selected.at else block for block in body.blocks))
    assert unswitch.specialized(body) is body
