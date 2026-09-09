"""Printing must not erase constants in cells its raised effects cannot reach."""

import pytest

from qbopt.analysis import consts
from qbopt.model import mir
from qbopt.model import ir
from qbopt.objectfile.module import Addr, Space


@pytest.mark.parametrize("offset", [0, 1, 2, 3])
def test_call_invalidates_every_byte_of_escaped_long(offset):
    """A write through an escaped long retained its upper three constant bytes."""
    address = Addr(Space.SEGMENT, 6, index=5)
    cells = consts._fragments(mir.MemRef(address, 4), consts.Known(0x12345678, 4))
    effect = mir.MemRef(None, 0, beyond=(5, frozenset({(5, 6)})))
    call = mir.Op(10, ir.Operation.MOVE, "", (), (), kind=mir.Kind.CALL, stores=(effect,))
    after = consts._kills(cells, call, {}, frozenset({5}), {10: "B$PSSD"})
    assert consts._cell(after, mir.MemRef(address.plus(offset), 1)) is None


def test_call_preserves_constants_when_no_program_address_escapes():
    """PRINT discarded an input constant even with an empty proven escape set."""
    address = Addr(Space.SEGMENT, 6, index=5)
    cells = consts._fragments(mir.MemRef(address, 4), consts.Known(0x12345678, 4))
    effect = mir.MemRef(None, 0, beyond=(5, frozenset()))
    call = mir.Op(10, ir.Operation.MOVE, "", (), (), kind=mir.Kind.CALL, stores=(effect,))
    after = consts._kills(cells, call, {}, frozenset({5}), {10: "B$PSSD"})
    assert consts._cell(after, mir.MemRef(address, 4)) == consts.Known(0x12345678, 4)


@pytest.mark.parametrize("name,effects", [("UNKNOWN", True), ("B$PSSD", False)])
def test_unproven_call_still_invalidates_constants(name, effects):
    """An empty escape set cannot override an unknown call or missing write effects."""
    address = Addr(Space.SEGMENT, 6, index=5)
    cells = consts._fragments(mir.MemRef(address, 4), consts.Known(7, 4))
    stores = (mir.MemRef(None, 0, beyond=(5, frozenset())),) if effects else ()
    call = mir.Op(10, ir.Operation.MOVE, "", (), (), kind=mir.Kind.CALL, stores=stores)
    assert consts._kills(cells, call, {}, frozenset({5}), {10: name}) == {}
