"""Exact facts are evidence, not permission to discard strict FP effects."""

from fractions import Fraction
from pathlib import Path

import corpus
import pytest

from qbopt.analysis import floatfacts
from qbopt.model import mir
from qbopt.model.floating import Format, Precision, Rounding, Semantics


@pytest.mark.parametrize("kind,number,negative_zero,expected,expected_negative_zero", [
    (mir.Kind.FNEG, 2, False, -2, False),
    (mir.Kind.FABS, 2, False, 2, False),
    (mir.Kind.FNEG, -2, False, 2, False),
    (mir.Kind.FABS, -2, False, 2, False),
    (mir.Kind.FNEG, 0, False, 0, True),
    (mir.Kind.FNEG, 0, True, 0, False),
    (mir.Kind.FABS, 0, True, 0, False),
])
def test_unary_facts_preserve_negation_absolute_value_and_zero_sign(kind, number, negative_zero, expected, expected_negative_zero):
    """Conflating FABS with FNEG would number abs(2) as -2 and abs(-0) as -0."""
    rule = Semantics((Format.EXTENDED80,), Format.EXTENDED80, Precision.EXACT, Rounding.NONE)
    fact = floatfacts.evaluated(kind, rule, (floatfacts.Finite(Fraction(number), negative_zero),))
    assert fact == floatfacts.Finite(Fraction(expected), expected_negative_zero)


@pytest.mark.parametrize("bits,expected,negative_zero", [
    (0x40000000, 2, False), (0x40800000, 4, False),
    (0x3f400000, Fraction(3, 4), False), (0xc0400000, -3, False),
    (0, 0, False), (0x80000000, 0, True),
])
def test_single_bit_patterns_decode_without_host_float(bits, expected, negative_zero):
    fact = floatfacts.decoded(bits, Format.BINARY32)
    assert fact.value == expected and fact.negative_zero == negative_zero
    assert floatfacts.encoded(fact, Format.BINARY32) == bits


@pytest.mark.parametrize("bits", [1, 0x7f800000, 0xff800000, 0x7fc00000, 0x7f800001])
def test_subnormals_infinities_and_nans_are_not_exception_free_inputs(bits):
    assert floatfacts.decoded(bits, Format.BINARY32) is None


@pytest.mark.parametrize("kind,left,right,expected", [
    (mir.Kind.FADD, 2, 4, 6), (mir.Kind.FMUL, 6, 8, 48),
    (mir.Kind.FDIV, 6, 8, Fraction(3, 4)),
    (mir.Kind.FDIV, 1, 3, None), (mir.Kind.FDIV, 1, 0, None),
    (mir.Kind.FADD, 2**24, 1, None), (mir.Kind.FSUB, 1, 1, None),
])
def test_exact_arithmetic_respects_dynamic_precision_and_zero_sign(kind, left, right, expected):
    rule = Semantics((Format.EXTENDED80, Format.EXTENDED80), Format.EXTENDED80, Precision.DYNAMIC, Rounding.DYNAMIC)
    result = floatfacts.evaluated(kind, rule, (floatfacts.Finite(Fraction(left)), floatfacts.Finite(Fraction(right))))
    assert (result.value if result else None) == expected


def test_single_store_does_not_keep_an_extended_intermediate():
    """FPCSE's p/q stores must round before later reloads; extended 16777217 is not SINGLE."""
    rule = Semantics((Format.EXTENDED80,), Format.BINARY32, Precision.DESTINATION, Rounding.DYNAMIC)
    assert floatfacts.evaluated(mir.Kind.FSTORE, rule, (floatfacts.Finite(Fraction(2**24 + 1)),)) is None


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_fpcse_known_inputs_reach_float_computations(tag):
    """FPCSE's 2+4, product 48 and quotient 0.75 should not remain opaque facts."""
    path = Path(f"fixtures/omf/fpcse-{tag}.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    from qbopt.objectfile import omf
    from qbopt.objectfile.module import Addr, Space
    # Explicit initial contents of this fixture's constant pool, not a default
    # assumption about arbitrary procedure-entry memory.
    segments = omf.segments(found.records)
    initial = {Addr(Space.SEGMENT, offset + index, segment): byte
               for _, segment, offset, data in omf.ledata(found.records)
               if segments[segment][0] == "BC_CN" for index, byte in enumerate(data)}
    facts = floatfacts.known(body, found.dgroup, found.calls, initial=initial)
    computed = {op.kind: facts[result.value].value for block in body.blocks for op in block.ops
                for result in op.results if isinstance(result, mir.Held) and result.value in facts}
    assert computed[mir.Kind.FADD] == 6
    assert computed[mir.Kind.FMUL] == 48
    assert computed[mir.Kind.FDIV] == Fraction(3, 4)


def test_entry_bytes_are_killed_by_a_store():
    """A constant-pool seed is an entry fact, not immutable memory after a write."""
    from dataclasses import replace
    from qbopt.analysis import consts
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    store = next(op for block in body.blocks for op in block.ops if op.kind is mir.Kind.STORE)
    ref = store.stores[0]
    body = replace(body, blocks=(mir.MirBlock(body.entry, (), (store,), ()),))
    seed = {(ref.addr, 1): consts.Known(255, 1)}
    before = consts.cells(body, found.dgroup, found.calls, initial=seed)
    assert before[(body.entry, 0)] == seed
    after = consts._kills(seed, store, {}, found.dgroup, found.calls)
    assert after[(ref.addr, 1)].n == 0
