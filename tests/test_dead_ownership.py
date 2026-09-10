"""Deleting a computation does not delete ownership of its input bytes."""

from dataclasses import replace

import pytest

from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import lower
from qbopt.backend import select
from qbopt.optimize import transform


def test_dead_store_alone_in_a_block_keeps_only_byte_ownership():
    """BOOLS retained t=0 before t=2 because its block had no surviving neighbor."""
    from qbopt.objectfile.module import Addr, Space
    ref = mir.MemRef(Addr(Space.SEGMENT, 16, 5), 2)
    first = mir.Op(0, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE,
                   args=(mir.Const(0, 2),), results=(mir.Cell(ref),), stores=(ref,), covers=(0, 6))
    second = replace(first, at=10, covers=(10, 16), args=(mir.Const(2, 2),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (first,), (10,)), mir.MirBlock(10, (), (second,), ())))
    result = transform.without_dead_stores(body, frozenset({5}), {})
    marker = result.blocks[0].ops[0]
    assert marker.kind is mir.Kind.NOTHING and not marker.stores
    assert marker.covers == first.covers
    assert select.emit(lower.current(marker)).code == b""
    assert result.blocks[1].ops == (second,)


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
@pytest.mark.parametrize("guard", [None, "opaque_before", "read", "different_variable", "unversioned", "exit"])
def test_local_overwrite_does_not_expose_an_opaque_reader(guard):
    """FPDEEP's dead return halves are replaced before its opaque DOUBLE copy can read them."""
    first = mir.Value(100, 0, variable=7, version=0 if guard == "unversioned" else 1)
    later = mir.Value(101, 2, variable=8 if guard == "different_variable" else 7,
                      version=0 if guard == "unversioned" else 2)
    initial = mir.Op(0, ir.Operation.MOVE, "", (first,), (), kind=mir.Kind.COPY,
                     args=(mir.Const(1, 2),), results=(mir.Held(first, 2),), covers=(0, 2))
    overwrite = mir.Op(2, ir.Operation.MOVE, "", (later,), (), kind=mir.Kind.COPY,
                       args=(mir.Const(2, 2),), results=(mir.Held(later, 2),), covers=(2, 4))
    opaque = mir.Op(4, ir.Operation.BARRIER, "?", (), (), covers=(4, 5))
    ops = (initial, opaque, overwrite) if guard == "opaque_before" else (initial, overwrite, opaque)
    if guard == "read":
        overwrite = replace(overwrite, args=(mir.Held(first, 2),), uses=(first,))
        ops = (initial, overwrite, opaque)
    if guard == "exit":
        ops = (initial, opaque)
    body = mir.MirBody(0, (mir.MirBlock(0, (), ops, ()),), {})
    changed = transform.dead(body)
    assert (changed.blocks[0].ops[0].kind is mir.Kind.NOTHING) == (guard is None)
