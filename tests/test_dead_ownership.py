"""Deleting a computation does not delete ownership of its input bytes."""

from dataclasses import replace

import pytest

from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import lower
from qbopt.backend import select
from qbopt.optimize import transform


def test_explicit_mask_input_is_live_even_when_it_also_preserves_upper_bits():
    """R_BSP drew 103/266 polygons: DCE deleted PEEK before its live AND 255."""
    from qbopt.objectfile.module import Addr, Space
    before = mir.Value(1, 0, variable=1, version=1)
    after = mir.Value(2, 2, variable=1, version=2)
    source = mir.MemRef(Addr(Space.SEGMENT, 0, 5), 1)
    target = mir.MemRef(Addr(Space.SEGMENT, 2, 5), 2)
    load = mir.Op(0, ir.Operation.MOVE, "mov", (before,), (), kind=mir.Kind.LOAD,
                  args=(mir.Cell(source),), results=(mir.Held(before, 1),), loads=(source,))
    mask = mir.Op(2, ir.Operation.BINARY, "and", (after,), (before,), kind=mir.Kind.AND,
                  args=(mir.Held(before, 2), mir.Const(255, 2)), results=(mir.Held(after, 2),),
                  merges={before: after})
    store = mir.Op(4, ir.Operation.MOVE, "mov", (), (after,), kind=mir.Kind.STORE,
                   args=(mir.Held(after, 2),), results=(mir.Cell(target),), stores=(target,))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (load, mask, store), ()),))
    changed = transform.dead(body)
    assert changed.blocks[0].ops[0].kind is mir.Kind.LOAD


def test_dead_store_alone_in_a_block_keeps_only_byte_ownership():
    """BOOLS retained t=0 before t=2 because its block had no surviving neighbor."""
    from qbopt.objectfile.module import Addr, Space
    ref = mir.MemRef(Addr(Space.SEGMENT, 16, 5), 2)
    first = mir.Op(0, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE,
                   args=(mir.Const(0, 2),), results=(mir.Cell(ref),), stores=(ref,), absorbed=(1,))
    second = replace(first, at=10, absorbed=(2,), args=(mir.Const(2, 2),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (first,), (10,)), mir.MirBlock(10, (), (second,), ())))
    result = transform.without_dead_stores(body, frozenset({5}), {})
    marker = result.blocks[0].ops[0]
    assert marker.kind is mir.Kind.NOTHING and not marker.stores
    assert marker.absorbed == first.absorbed
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
        absorbed=(1, 2),
    )
    live = mir.Op(10, ir.Operation.PUSH, "push", (), (), kind=mir.Kind.ARG,
                  args=(mir.Const(7, 2),), absorbed=(3,))
    body = mir.MirBody(10, (mir.MirBlock(10, (), (dead, live), ()),))
    result = transform.dead(body)
    marker, survivor = result.blocks[0].ops
    assert not marker.defines and not marker.uses
    assert marker.absorbed == dead.absorbed
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
                     args=(mir.Const(1, 2),), results=(mir.Held(first, 2),), absorbed=(1,))
    overwrite = mir.Op(2, ir.Operation.MOVE, "", (later,), (), kind=mir.Kind.COPY,
                       args=(mir.Const(2, 2),), results=(mir.Held(later, 2),), absorbed=(2,))
    opaque = mir.Op(4, ir.Operation.BARRIER, "?", (), (), absorbed=(3,))
    ops = (initial, opaque, overwrite) if guard == "opaque_before" else (initial, overwrite, opaque)
    if guard == "read":
        overwrite = replace(overwrite, args=(mir.Held(first, 2),), uses=(first,))
        ops = (initial, overwrite, opaque)
    if guard == "exit":
        ops = (initial, opaque)
    body = mir.MirBody(0, (mir.MirBlock(0, (), ops, ()),), {})
    changed = transform.dead(body)
    assert (changed.blocks[0].ops[0].kind is mir.Kind.NOTHING) == (guard is None)
