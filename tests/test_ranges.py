from pathlib import Path

import corpus
import pytest
from iced_x86 import Register
from qbopt.model import ir, mir
from qbopt.analysis import ranges
from qbopt.optimize import transform
from qbopt.objectfile.module import Addr, Space


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_fpdeep_one_based_index_has_a_bounded_byte_offset(tag):
    """FPDEEP lost its 1..3 bound at i-1, leaving p(i)'s byte extent unknown."""
    path = Path(f"fixtures/omf/fpdeep-{tag}.obj".lower())
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = transform.applied(mir.bodies(found, partition)[0][1], found.dgroup,
                             found.calls, blocks=partition, found=found, unroll_=False)  # Unrolled, i is a constant.
    known = ranges.bounded(body)
    accesses = [(block.at, ref) for block in body.blocks for op in block.ops
                for ref in op.loads if op.kind is mir.Kind.FLOAD and ref.base is not None]
    assert accesses
    for at, ref in accesses:
        covered = ranges.covering(ref, known[at])
        assert covered.base is None
        assert covered.addr.disp == 6
        assert covered.width == 12


@pytest.mark.parametrize("kind,low,high,expected", [
    (mir.Kind.DECREMENT, 1, 3, ranges.Interval(0, 2, 2)),
    (mir.Kind.INCREMENT, -3, -1, ranges.Interval(-2, 0, 2)),
    (mir.Kind.DECREMENT, -32768, 0, None),
    (mir.Kind.INCREMENT, 0, 32767, None),
])
def test_unit_steps_require_nonwrapping_intervals(kind, low, high, expected):
    source, result = mir.Value(1, 0), mir.Value(2, 0)
    op = mir.Op(0, ir.Operation.UNARY, "", (result,), (source,), kind=kind,
                args=(mir.Held(source, 2),), results=(mir.Held(result, 2),))
    assert ranges._computed(op, {source: ranges.Interval(low, high, 2)}, {}) == expected


@pytest.mark.parametrize("low,high", [(1, 20), (-32768, -1), (-10, 10)])
def test_signed_widening_keeps_the_numeric_range(low, high):
    """ADDRM's bounded 1..20 counter lost its interval when converted to a long array value."""
    source, result = mir.Value(1, 0), mir.Value(2, 0)
    op = mir.Op(0, ir.Operation.EXTEND, "", (result,), (source,),
                kind=mir.Kind.SIGN_EXTEND, args=(mir.Held(source, 2),), results=(mir.Held(result, 4),))
    assert ranges._computed(op, {source: ranges.Interval(low, high, 2)}, {}) == ranges.Interval(low, high, 4)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_addrm_long_array_value_keeps_counter_bounds(tag):
    """ADDRM lost the 1..20 bound at the integer-to-long conversion feeding b(i)."""
    path = Path(f"fixtures/omf/addrm-{tag}.obj".lower())
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = transform.applied(mir.bodies(found, partition)[0][1], found.dgroup,
                             found.calls, blocks=partition, found=found)
    known = ranges.bounded(body)
    converted = [(block.at, op.results[0].value) for block in body.blocks for op in block.ops
                 if op.kind is mir.Kind.SIGN_EXTEND]
    assert converted
    for at, value in converted:
        assert known[at][value] == ranges.Interval(1, 20, 4)


@pytest.mark.parametrize("recurrences", [False, True])
def test_nbody_scaled_index_is_bounded_only_inside_its_loop(recurrences, monkeypatch) -> None:
    """Nbody's other*4 had no interval, so position reads aliased every scalar store."""
    if recurrences:
        from qbopt.optimize import strength
        original = strength._multiplies
        monkeypatch.setattr(strength, "_multiplies", lambda one, derived: one.op.kind is mir.Kind.SHL or original(one, derived))
    # Needs the stores drop_stores proves unobservable.
    monkeypatch.setattr("qbopt.analysis.observers.private", lambda *args: None)
    path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
    found = corpus.loaded(path)
    blocks = corpus.partitioned(path)
    body = mir.bodies(found, blocks)[0][1]
    current_id = next(op.id for block in body.blocks for op in block.ops if op.at == 0x12d and op.loads)
    other_id = next(op.id for block in body.blocks for op in block.ops if op.at == 0x11d and op.loads)
    body = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
    known = ranges.bounded(body)
    index = next(op for block in body.blocks for op in block.ops if op.id == other_id).loads[0].base
    assert known[0x117][index] == ranges.Interval(0, 20, 2)
    access = next(op for block in body.blocks for op in block.ops if op.id == current_id).loads[0]
    current = access.base
    assert known[0x117][current] == ranges.Interval(0, 20, 2)
    stored = next(op for block in body.blocks for op in block.ops if op.at == 0x135).stores[0]
    assert mir.overlapping(access, stored, found.dgroup)
    assert not mir.overlapping(access, stored, found.dgroup, known=known[0x117])
    assert index not in known.get(0x21C, {})
    assert index not in known.get(0x227, {})


@pytest.mark.parametrize(("start", "step", "advances", "expected"), [
    (0, 4, 5, ranges.Interval(0, 20, 2)),
    (20, -4, 5, ranges.Interval(0, 20, 2)),
    (7, 0, 5, ranges.Interval(7, 7, 2)),
    (32760, 4, 1, None),
    (-32760, -4, 2, None),
    (0, 16384, 4, None),
    (0, 4, -1, None),
])
def test_secondary_recurrence_bounds_reject_wrap(start, step, advances, expected):
    """A derived address range must not hide wraparound, including on the final latch update."""
    assert ranges._recurrence_span(start, step, advances, 2) == expected


@pytest.mark.parametrize(
    ("low", "high", "count", "expected"),
    [
        (0, 5, 2, ranges.Interval(0, 20, 2)),
        (-5, -1, 2, ranges.Interval(-20, -4, 2)),
        (0, 16384, 1, None),
        (-32768, -1, 1, None),
        (0, 5, 32, None),
    ],
)
def test_shift_ranges_refuse_wraparound(low, high, count, expected) -> None:
    source, result = mir.Value(1, 0), mir.Value(2, 0)
    op = mir.Op(
        0,
        ir.Operation.BINARY,
        "shl",
        (result,),
        (source,),
        kind=mir.Kind.SHL,
        args=(mir.Held(source, 2), mir.Const(count, 1)),
        results=(mir.Held(result, 2),),
    )
    assert ranges._computed(op, {source: ranges.Interval(low, high, 2)}, {}) == expected


@pytest.mark.parametrize(("interval", "offset", "width", "disjoint"), [
    (ranges.Interval(0, 20, 2), 26, 2, True),
    (ranges.Interval(0, 20, 2), 25, 2, False),
    (ranges.Interval(0, 20, 2), 100, 4, True),
    (ranges.Interval(-8, 20, 2), 100, 4, False),
    (ranges.Interval(0, 65535, 2), 100, 4, False),
    (ranges.Interval(0, 20, 4), 100, 4, False),
])
def test_range_alias_checks_cover_width_and_wrap(interval, offset, width, disjoint) -> None:
    base = mir.Value(1, 0)
    indexed = mir.MemRef(Addr(Space.SEGMENT, 4, 5, base=Register.SI), 2, base=base, base_width=2)
    static = mir.MemRef(Addr(Space.SEGMENT, offset, 5), width)
    assert mir.overlapping(indexed, static, frozenset({5}), known={base: interval}) is not disjoint
    assert mir.overlapping(indexed, static, frozenset({5}))
