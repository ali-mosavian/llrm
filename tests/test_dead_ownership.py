"""Deleting a computation does not delete ownership of its input bytes."""

from qbopt import ir
from qbopt import mir
from qbopt import lower
from qbopt import select
from qbopt import transform


def test_dead_sibling_emits_no_bytes_and_keeps_its_ranges() -> None:
    """LNGMIX retained dead constant copies sharing a live operation's address."""
    value = mir.Value(1, 10)
    dead = mir.Op(
        10,
        ir.Operation.MOVE,
        "mov",
        (value,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Const(5, 4),),
        results=(mir.Held(value, 4),),
        covers=(10, 15),
        extra_covers=((20, 32),),
    )
    live = mir.Op(10, ir.Operation.PUSH, "push", (), (), kind=mir.Kind.ARG, args=(mir.Const(7, 2),), covers=(15, 18))
    body = mir.MirBody(10, (mir.MirBlock(10, (), (dead, live), ()),))
    result = transform.dead(body)
    marker, survivor = result.blocks[0].ops
    assert not marker.defines and not marker.uses
    assert marker.covers == dead.covers and marker.extra_covers == dead.extra_covers
    assert select.emit(lower.current(marker)).code == b""
    assert survivor == live
    assert transform.dead(result) == result
