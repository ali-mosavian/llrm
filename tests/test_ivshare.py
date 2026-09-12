from pathlib import Path
from dataclasses import replace

import pytest

from qbopt import wholeseg
from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.analysis import loops
from qbopt.optimize import ivshare
from qbopt.optimize import exitsink
from qbopt.optimize import strength
from qbopt.analysis import induction
from qbopt.optimize import transform


@pytest.fixture
def culling(monkeypatch: pytest.MonkeyPatch) -> mir.MirBody:
    captured = []
    original = strength.reduced

    def capture(body: mir.MirBody, dgroup: frozenset[int] = frozenset(), bounds: dict | None = None) -> mir.MirBody:
        if body.entry == 0 and not captured:
            captured.append(body)
        return original(body, dgroup, bounds)

    monkeypatch.setattr(strength, "reduced", capture)
    contract = replace(
        runtime.worst("R_EMIT_ENTITIES"),
        cleanup=28,
        inputs=frozenset(
            {runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX, runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI}
        ),
    )
    result = wholeseg.emitted(
        Path("fixtures/regressions/r_walk-borland.obj").read_bytes(),
        native_fpu=True,
        external_contracts={contract.name: contract},
    )
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    return captured[0]


def test_equal_stride_pointers_share_the_base_recurrence(culling: mir.MirBody) -> None:
    # r_walk carried base, base+4 and base+8 through every iteration.
    loop = next(one for one in loops.loops(culling.blocks, culling.entry) if one.header == 0x4A)
    before = induction.basics(culling, loop)
    changed = ivshare.shared(ivshare.shared(culling))
    after = induction.basics(changed, loop)
    assert sum(one.step == mir.Const(20, 2) for one in before.values()) == 3
    assert sum(one.step == mir.Const(20, 2) for one in after.values()) == 1
    header = next(block for block in changed.blocks if block.at == loop.header)
    reconstructed = {op.results[0].value.id: op for op in header.ops[:2] if isinstance(op.results[0], mir.Held)}
    assert len(reconstructed) == 2
    assert set(reconstructed) == before.keys() - after.keys()
    for value, op in reconstructed.items():
        assert op.kind is mir.Kind.ADD
        assert isinstance(op.args[0], mir.Held)
        assert isinstance(op.args[1], mir.Const)
        assert isinstance(op.results[0], mir.Held)
        assert op.args[0].value.id in after
        assert op.args[1].n in (4, 8)
        assert op.results[0].width == before[value].start.width
    assert ivshare.shared(changed) is changed


def test_word_recurrences_preserve_observed_upper_bits(culling: mir.MirBody) -> None:
    # Equal low-word strides do not prove equal upper words. Reconstructing
    # a word pointer from another pointer must not replace a wider value.
    header = next(block for block in culling.blocks if block.at == 0x4A)
    pointers = tuple(phi.result for phi in header.phis if not phi.result.flags)
    reader = replace(
        header.ops[0],
        args=tuple(mir.Held(value, 4) for value in pointers),
        uses=pointers,
        loads=(),
        stores=(),
        results=(),
        defines=(),
        merges={},
        kind=mir.Kind.OPAQUE,
    )
    observed = replace(
        culling,
        blocks=tuple(
            replace(block, ops=(*block.ops, reader)) if block is header else block for block in culling.blocks
        ),
    )
    changed = ivshare.shared(observed)
    assert changed is not observed
    changed_header = next(block for block in changed.blocks if block.at == header.at)
    removed = {phi.result for phi in header.phis} - {phi.result for phi in changed_header.phis}
    assert len(removed) == 1
    result = removed.pop()
    rebuilt = next(op for op in changed_header.ops if result in op.defines)
    assert len(rebuilt.merges) == 1
    upper = next(iter(rebuilt.merges))
    original = next(phi for phi in header.phis if phi.result == result)
    for incoming in original.incoming.values():
        producer = next(op for block in observed.blocks for op in block.ops if incoming in op.defines)
        assert producer.merges == {upper: incoming}
    assert upper in rebuilt.uses
    unknown = replace(
        observed,
        blocks=tuple(
            replace(block, ops=tuple(replace(op, merges={}) for op in block.ops)) for block in observed.blocks
        ),
    )
    assert ivshare.shared(unknown) is unknown


def test_different_strides_are_not_shared(culling: mir.MirBody) -> None:
    changed = replace(
        culling,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    replace(
                        op,
                        args=tuple(
                            mir.Const(24 if op.at == 0x2DA else 28, arg.width)
                            if isinstance(arg, mir.Const) and arg.n == 20
                            else arg
                            for arg in op.args
                        ),
                    )
                    if op.at in (0x2DA, 0x2DD)
                    else op
                    for op in block.ops
                ),
            )
            for block in culling.blocks
        ),
    )
    assert ivshare.shared(changed) is changed


def test_final_pointer_update_moves_to_the_exit(culling: mir.MirBody) -> None:
    # r_walk computed the returned pointer's +20 on every iteration.
    shared = transform.dead(ivshare.shared(ivshare.shared(culling)))
    changed = exitsink.sunk(shared)
    loop = next(one for one in loops.loops(changed.blocks, changed.entry) if one.header == 0x4A)
    assert not next(op for block in changed.blocks for op in block.ops if op.at == 0x2DA).defines
    exit_block = next(block for block in changed.blocks if block.at == 0x2EC)
    assert exit_block.at not in loop.body
    assert any(op.kind is mir.Kind.ADD and mir.Const(20, 2) in op.args for op in exit_block.ops)
    unknown = replace(
        shared,
        blocks=tuple(
            replace(block, ops=tuple(replace(op, reads_complete=False) if op.barrier else op for op in block.ops))
            for block in shared.blocks
        ),
    )
    assert exitsink.sunk(unknown) is unknown


def test_pointer_exit_phi_no_longer_carries_the_restores_upper_half(culling: mir.MirBody) -> None:
    # The raise now traces POP SI's upper word to the incoming value, rather
    # than routing unchanged bits through the pointer's loop-exit phi.
    shared = ivshare.shared(ivshare.shared(culling))
    update = next(op for block in shared.blocks for op in block.ops if op.at == 0x2DD)
    value = update.results[0]
    assert isinstance(value, mir.Held)
    assert (value.value, transform.HIGH) not in transform.halves(shared)
    assert (value.value, transform.LOW) not in transform.halves(shared)
    changed = transform.dead(shared)
    preserved = next(op for block in changed.blocks for op in block.ops if op.at == 0x2DD)
    returned = next(op for block in changed.blocks for op in block.ops if op.at == 0x2DA)
    assert not preserved.defines
    assert returned.kind is mir.Kind.ADD
    assert [(op.at, op.kind) for block in changed.blocks for op in block.ops if op.barrier] == [
        (op.at, op.kind) for block in shared.blocks for op in block.ops if op.barrier
    ]
    unknown = replace(
        shared,
        blocks=tuple(
            replace(block, ops=tuple(replace(op, reads_complete=False) if op.barrier else op for op in block.ops))
            for block in shared.blocks
        ),
    )
    conservative = transform.dead(unknown)
    assert next(op for block in conservative.blocks for op in block.ops if op.at == 0x2DD).defines
