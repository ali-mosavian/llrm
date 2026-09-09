from pathlib import Path

import corpus
import pytest
from qbopt import ir, mir, ranges, transform


def test_nbody_scaled_index_is_bounded_only_inside_its_loop() -> None:
    """Nbody's other*4 had no interval, so position reads aliased every scalar store."""
    path = Path("fixtures/regressions/nbody-stack-p-g2.obj")
    found = corpus.loaded(path)
    blocks = corpus.partitioned(path)
    body = mir.bodies(found, blocks)[0][1]
    body = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
    known = ranges.bounded(body)
    index = next(op for block in body.blocks for op in block.ops if op.at == 0x11B).results[0].value
    assert known[0x117][index] == ranges.Interval(0, 20, 2)
    current = next(op for block in body.blocks for op in block.ops if op.at == 0x12d).loads[0].base
    assert known[0x117][current] == ranges.Interval(0, 20, 2)
    assert index not in known.get(0x21C, {})
    assert index not in known.get(0x227, {})


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
