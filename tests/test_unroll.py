"""FPDEEP's three printed iterations cannot be replaced by one final iteration."""

from pathlib import Path

import pytest
import corpus
from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.analysis import floatfacts
from qbopt.backend import lower, lower_floats
from qbopt.optimize import transform, unroll


def body():
    path = Path("fixtures/omf/fpdeep-p-g2.obj")
    found = corpus.loaded(path)
    original = mir.bodies(found, corpus.partitioned(path))[0][1]
    optimized = transform.applied(original, found.dgroup, found.calls, found=found)
    return found, optimized


def test_fpdeep_unroll_preserves_order_and_fresh_definitions():
    found, original = body()
    loop, = loops.loops(original.blocks, original.entry)
    latch = original.block(next(iter(loop.latches)))
    changed = unroll.expanded(original, found.dgroup, found.calls)
    assert not loops.loops(changed.blocks, changed.entry)
    operations = changed.block(latch.at).ops
    effects = lambda ops: [op.id for op in ops if op.floating or op.kind is mir.Kind.CALL]
    assert effects(operations) == effects(latch.ops) * 3
    definitions = [value.id for op in operations for value in op.defines]
    assert len(definitions) == len(set(definitions))
    assert unroll.expanded(changed, found.dgroup, found.calls) == changed


def test_unrolled_latch_explicitly_skips_the_original_header():
    """FPDEEP fell through to its original header and started the expanded body again."""
    found, original = body()
    loop, = loops.loops(original.blocks, original.entry)
    changed = unroll.expanded(original, found.dgroup, found.calls)
    latch = changed.block(next(iter(loop.latches)))
    assert latch.ops[-1].kind is mir.Kind.JUMP
    assert latch.ops[-1].target == latch.succ[0]
    assert latch.ops[-1].target not in loop.body


def test_fpdeep_expansion_exposes_exact_array_arithmetic():
    """FPDEEP's 144/784/3600 squares and 6/14/30 ratios stayed unknown after expansion."""
    found, original = body()
    changed = unroll.expanded(original, found.dgroup, found.calls)
    facts = floatfacts.known(changed, found.dgroup, found.calls)
    for address, expected in ((0x9c, [144, 784, 3600]), (0xe4, [6, 14, 30])):
        values = [facts.get(op.args[0].value) for block in changed.blocks for op in block.ops
                  if op.at == address and op.kind is mir.Kind.FSTORE and not op.stores]
        assert all(value is not None for value in values)
        assert [value.value for value in values] == expected


def test_fpdeep_exact_integer_arguments_keep_floating_checkpoints():
    """FPDEEP kept reading converted square/ratio temporaries instead of known answers."""
    found, original = body()
    expanded = unroll.expanded(original, found.dgroup, found.calls)
    converted = floatfacts.converted(expanded, found.dgroup, found.calls)
    assert sorted(value.n for value in converted.values()) == [6, 14, 30, 144, 784, 3600]
    folded = transform.folded(expanded, found.dgroup, found.calls)
    floating = lambda body: [op for block in body.blocks for op in block.ops if op.floating]
    removed_addresses = {0x97, 0x9c, 0xdf, 0xe4}
    assert floating(folded) == [op for op in floating(expanded) if op.at not in removed_addresses]
    checkpoints = [op for block in folded.blocks for op in block.ops if op.kind is mir.Kind.FCHECK]
    assert len(checkpoints) == 12
    assert {op.at for op in checkpoints} == removed_addresses
    lower_floats.checked(folded)
    arguments = [op.args[0].n for block in folded.blocks for op in block.ops
                 if op.kind is mir.Kind.ARG and op.at in (0xa1, 0xe9)
                 and isinstance(op.args[0], mir.Const)]
    assert arguments == [144, 6, 784, 14, 3600, 30]
    for block in folded.blocks:
        for op in block.ops:
            if op.kind is mir.Kind.ARG:
                assert not any(isinstance(arg, mir.Held) and arg.value in converted for arg in op.args)


@pytest.mark.parametrize("number,expected", [(-6, 0xfffffffa), (2147483647, 2147483647),
                                             (2147483648, None), ("1/3", None)])
def test_integer_conversion_facts_require_exact_in_range_values(monkeypatch, number, expected):
    """FPDEEP's conversion facts must not invent a rounded or overflowing print argument."""
    from dataclasses import replace
    from fractions import Fraction
    found, original = body()
    op = next(op for block in original.blocks for op in block.ops
              if op.kind is mir.Kind.FSTORE and op.at == 0x9c)
    isolated = replace(original, blocks=(mir.MirBlock(original.entry, (), (op,), ()),))
    monkeypatch.setattr(floatfacts, "known", lambda *args, **kwargs:
                        {op.args[0].value: floatfacts.Finite(Fraction(number))})
    facts = floatfacts.converted(isolated, found.dgroup, found.calls)
    assert (facts[op.results[0].value].n if facts else None) == expected


def test_emission_requires_explicit_unrolled_provenance():
    """FPDEEP timed out when repeated input addresses interleaved its calls and lost fixups."""
    found, original = body()
    changed = unroll.expanded(original, found.dgroup, found.calls)
    from dataclasses import replace
    with pytest.raises(lower.Unlowered, match="floating sequence changed"):
        lower_floats.checked(replace(changed, repetitions=()))
    lower_floats.checked(changed)


@pytest.mark.parametrize("damage", ["count", "missing", "reordered", "duplicate"])
def test_expansion_provenance_does_not_allow_arbitrary_float_sequences(damage):
    from dataclasses import replace
    found, original = body()
    changed = unroll.expanded(original, found.dgroup, found.calls)
    at, count = changed.repetitions[0]
    block = changed.block(at)
    positions = [index for index, op in enumerate(block.ops) if op.floating]
    match damage:
        case "count": changed = replace(changed, repetitions=((at, count - 1),))
        case "duplicate": changed = replace(changed, repetitions=changed.repetitions * 2)
        case _:
            ops = list(block.ops)
            first, second = positions[:2]
            if damage == "missing": del ops[first]
            else: ops[first], ops[second] = ops[second], ops[first]
            changed = replace(changed, blocks=tuple(replace(one, ops=tuple(ops)) if one.at == at else one
                                                   for one in changed.blocks))
    with pytest.raises(lower.Unlowered):
        lower_floats.checked(changed)
