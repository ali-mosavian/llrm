"""Whole values must exist before LICM, not be reconstructed after it."""

from dataclasses import replace
from pathlib import Path

import corpus
import pytest

from qbopt.model import ir, mir
from qbopt.frontend import raising_longs
from qbopt.optimize import transform


@pytest.mark.parametrize("seed,expected", [(0, 0), (1, 0), (0x7fff, 0), (0x8000, 0xffff), (-1, 0xffff)])
def test_nbody_counter_seed_has_a_known_high_word(seed, expected):
    """NBODY's initial long 1 had an opaque high word, blocking whole-value loop phis."""
    from qbopt.analysis import consts
    path = Path("fixtures/bench/nbody-v-g3.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    body = replace(body, blocks=tuple(replace(block, ops=tuple(
        replace(op, args=(mir.Const(seed, 2),)) if op.at == 0xe0 and op.kind is mir.Kind.COPY else op
        for op in block.ops)) for block in body.blocks))
    facts = consts.known(body, found.dgroup, found.calls)
    high = next(op for block in body.blocks for op in block.ops
                if op.at == 0xe3 and op.results and op.results[0].width == 2)
    assert facts.get(high.results[0].value) == consts.Known(expected, 2)


@pytest.mark.parametrize("mismatch", ["none", "carry", "constant", "width", "source"])
def test_whole_negation_requires_exact_carry_chain(mismatch, monkeypatch):
    """NBODY damping paid for split negation; unrelated carry or halves are not a long negation."""
    recognize = raising_longs._negated_whole
    with monkeypatch.context() as patch:
        patch.setattr(raising_longs, "_negated_whole", lambda *args: None)
        path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
        body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    ops = {op.at: op for block in body.blocks for op in block.ops}
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    low, high, carry = ops[0x278], ops[0x27d], ops[0x27a]
    match mismatch:
        case "carry":
            carry = replace(carry, uses=tuple(mir.Value(9999, 0, flags=True) if value.flags else value
                                              for value in carry.uses))
        case "constant":
            carry = replace(carry, args=(carry.args[0], mir.Const(1, 2)))
        case "width":
            carry = replace(carry, results=(replace(carry.results[0], width=4),))
        case "source":
            carry = replace(carry, args=(mir.Held(mir.Value(9998, 0), 2), carry.args[1]))
    definitions.update({value: carry for value in carry.defines})
    source = recognize(high.results[0], low.results[0], definitions)
    if mismatch != "none":
        assert source is None
        return
    assert source is not None and source.width == 4
    for value in (0, 1, 65535, 65536, 0x7fffffff, 0x80000000, 0xffffffff):
        lower, upper = value & 65535, value >> 16
        result = (((-(upper + (lower != 0))) & 65535) << 16) | ((-lower) & 65535)
        assert result == (-value) & 0xffffffff


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_addrm_stores_and_reuses_the_signed_whole_value(tag):
    """ADDRM stored b(i) in two words and immediately reloaded the same long on every iteration."""
    path = Path(f"fixtures/omf/addrm-{tag}.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = mir.bodies(found, partition)[0][1]
    stores = [op for block in body.blocks for op in block.ops
              if any(ref.base is not None and ref.width == 4 for ref in op.stores)]
    assert stores
    result = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    assert not any(ref.base is not None and ref.width == 4
                   for block in result.blocks for op in block.ops for ref in op.loads)


@pytest.mark.parametrize("mismatch", ["source", "width", "kind"])
def test_signed_store_requires_the_matching_sign_word(mismatch, monkeypatch):
    recognize = raising_longs.scalar
    with monkeypatch.context() as patch:
        patch.setattr(raising_longs, "scalar", lambda body: body)
        patch.setattr(raising_longs, "sign_fills", lambda body: body)
        path = Path("fixtures/omf/addrm-p-g2.obj")
        body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    extension = next(op for block in body.blocks for op in block.ops if op.at == 0x5c)
    match mismatch:
        case "source":
            changed = replace(extension, args=(mir.Held(mir.Value(9999, 0), 2),))
        case "width":
            changed = replace(extension, results=(replace(extension.results[0], width=4),))
        case "kind":
            changed = replace(extension, kind=mir.Kind.COPY)
    body = replace(body, blocks=tuple(replace(block, ops=tuple(changed if op is extension else op for op in block.ops))
                                     for block in body.blocks))
    result = recognize(body)
    assert not any(op.kind is mir.Kind.SIGN_EXTEND for block in result.blocks for op in block.ops)


@pytest.fixture
def nbody(monkeypatch):
    recognize = raising_longs.scalar
    with monkeypatch.context() as patch:
        patch.setattr(raising_longs, "scalar", lambda body: body)
        path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
        found = corpus.loaded(path)
        body = mir.bodies(found, corpus.partitioned(path))[0][1]
    return body, recognize


def test_accumulator_initializers_are_whole_values(nbody):
    """Nbody's split zero stores blocked promotion of its whole-long accumulators."""
    body, recognize = nbody
    done = recognize(body)
    for at in (0xf0, 0xfc):
        op = next(op for block in done.blocks for op in block.ops if op.at == at)
        assert op.args == (mir.Const(0, 4),)
        assert op.stores[0].width == 4


@pytest.mark.parametrize("mismatch", ["base", "gap", "address"])
def test_constant_stores_require_identical_adjacent_addresses(nbody, mismatch):
    body, _ = nbody
    block = next(block for block in body.blocks if block.at == 0xf0)
    low, high = block.ops[:2]
    match mismatch:
        case "base":
            high = replace(high, stores=(replace(high.stores[0], base=mir.Value(99999, 0)),))
        case "gap":
            high = replace(high, covers=(high.covers[0] + 1, high.covers[1]))
        case "address":
            high = replace(high, stores=(replace(high.stores[0], addr=high.stores[0].addr.plus(2)),))
    sample = replace(body, blocks=(replace(block, ops=(low, high)),))
    assert raising_longs._constant_stores(sample) == sample


def test_position_arithmetic_is_scalar_before_optimization(nbody):
    """Nbody's four position halves blocked LICM; extracting them separately increased spills."""
    body, recognize = nbody
    done = recognize(body)
    ops = [op for block in done.blocks for op in block.ops]
    load = next(op for op in ops if op.at == 0x11d)
    subtract = next(op for op in ops if op.at == 0x12d and op.kind is mir.Kind.SUB)
    current = next(op for op in ops if op.at == 0x12d and op.kind is mir.Kind.LOAD)
    store = next(op for op in ops if op.at == 0x135)
    assert load.kind is mir.Kind.LOAD and load.results[0].width == 4
    assert subtract.kind is mir.Kind.SUB and subtract.args[0] == load.results[0]
    assert current.loads[0].width == 4
    assert subtract.args[1] == current.results[0] and not subtract.loads
    assert store.kind is mir.Kind.STORE and store.args == subtract.results
    assert store.stores[0].width == 4


def test_nbody_whole_position_loads_leave_the_inner_loop():
    """Nbody recomputed invariant position reads; hoisting four halves had increased spill cost."""
    path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
    found = corpus.loaded(path)
    blocks = corpus.partitioned(path)
    body = mir.bodies(found, blocks)[0][1]
    sites = [op for block in body.blocks for op in block.ops if op.at in (0x12d, 0x144) and op.loads]
    assert len(sites) == 2
    done = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
    for site in sites:
        block, load = next((block, op) for block in done.blocks for op in block.ops if op.id == site.id)
        assert block.at == 0xf0
        assert load.kind is mir.Kind.LOAD and load.results[0].width == 4
        assert load.loads[0].addr == site.loads[0].addr


def test_nbody_scalar_results_feed_constant_and_accumulator_arithmetic(nbody):
    """Nbody split division results for +1 and ACCX, forcing repeated stack reconstruction."""
    body, recognize = nbody
    done = recognize(body)
    ops = [op for block in done.blocks for op in block.ops]
    increment = next(op for op in ops if op.at == 0x1a5 and op.kind is mir.Kind.ADD)
    accumulator = next(op for op in ops if op.at == 0x1d9 and op.kind is mir.Kind.ADD)
    assert increment.args[1] == mir.Const(1, 4)
    assert all(arg.width == 4 for arg in increment.args)
    assert all(arg.width == 4 for arg in accumulator.args)
    assert increment.results[0].width == accumulator.results[0].width == 4


def test_nbody_stores_whole_results_through_half_copies(nbody):
    """Nbody stored DELTAY and FALLOFF as halves despite already having their whole scalar values."""
    body, recognize = nbody
    done = recognize(body)
    ops = [op for block in done.blocks for op in block.ops]
    delta = next(op for op in ops if op.at == 0x144 and op.kind is mir.Kind.SUB).results[0]
    falloff = next(op for op in ops if op.at == 0x1b2 and op.kind is mir.Kind.DIVMOD).results[0]
    for at, source in ((0x150, delta), (0x1b7, falloff)):
        store = next(op for op in ops if op.at == at and op.kind is mir.Kind.STORE)
        assert store.stores[0].width == 4
        assert store.args == (source,)


@pytest.mark.parametrize("producer", [0x12d, 0x131])
def test_live_half_flags_prevent_scalar_arithmetic(nbody, producer):
    body, recognize = nbody
    block = next(block for block in body.blocks if block.at == 0x117)
    original = next(op for op in block.ops if op.at == producer)
    flag = next(value for value in original.defines if value.flags)
    reader = mir.Op(0x217, ir.Operation.JUMP, "jz", (), (flag,), kind=mir.Kind.BRANCH)
    changed = replace(block, ops=(*block.ops, reader))
    body = replace(body, blocks=tuple(changed if one is block else one for one in body.blocks))
    done = recognize(body)
    subtract = next(op for block in done.blocks for op in block.ops if op.at == 0x12d)
    assert subtract.results[0].width == 2


def test_same_machine_address_with_different_ssa_base_is_not_a_pair(nbody):
    body, recognize = nbody
    def changed(op):
        if op.at != 0x121:
            return op
        ref = replace(op.loads[0], base=mir.Value(99999, op.at))
        return replace(op, loads=(ref,), args=(mir.Cell(ref),))
    body = replace(body, blocks=tuple(replace(block, ops=tuple(map(changed, block.ops))) for block in body.blocks))
    done = recognize(body)
    load = next(op for block in done.blocks for op in block.ops if op.at == 0x11d)
    assert load.results[0].width == 2
