"""FPDEEP's constant per-iteration array offsets did not expose initialized values."""

from dataclasses import replace

import pytest

from qbopt.analysis import consts
from qbopt.model import ir, mir
from qbopt.objectfile.module import Addr, Space


def test_known_array_offset_reads_the_element_not_the_base():
    index = mir.Value(1, 0)
    start = Addr(Space.SEGMENT, 6, 5)
    ref = mir.MemRef(start, 4, base=index, base_width=2)
    op = mir.Op(0, ir.Operation.FLOAT_LOAD, "fld", (), (index,), args=(mir.Cell(ref),), loads=(ref,))
    memory = {(start, 4): consts.Known(0x41400000, 4),
              (start.plus(4), 4): consts.Known(0x41e00000, 4)}
    assert consts._operand(op, op.args[0], {index: consts.Known(4, 2)}, memory) == consts.Known(0x41e00000, 4)


@pytest.mark.parametrize("guard", ["unknown", "partial", "segment", "wrap", "missing", "wide"])
def test_unproved_indexed_reads_remain_unknown(guard):
    index = mir.Value(1, 0)
    start = Addr(Space.SEGMENT, 6, 5)
    ref = mir.MemRef(start, 4, base=index, base_width=2)
    known = {index: consts.Known(4, 2)}
    memory = {(start.plus(4), 4): consts.Known(123, 4)}
    match guard:
        case "unknown": known = {}
        case "partial": known[index] = consts.Known(4, 1)
        case "segment": ref = replace(ref, segment=mir.Value(2, 0))
        case "wrap": ref = replace(ref, addr=Addr(Space.SEGMENT, 65530, 5))
        case "missing": memory = {}
        case "wide": ref = replace(ref, base_width=4)
    op = mir.Op(0, ir.Operation.MOVE, "mov", (), (index,), args=(mir.Cell(ref),), loads=(ref,))
    assert consts._operand(op, op.args[0], known, memory) is None


def test_known_indexed_store_updates_only_its_element():
    index = mir.Value(1, 0)
    start = Addr(Space.SEGMENT, 6, 5)
    ref = mir.MemRef(start, 2, base=index, base_width=2)
    store = mir.Op(0, ir.Operation.MOVE, "mov", (), (index,), kind=mir.Kind.STORE,
                   args=(mir.Const(9, 2),), stores=(ref,))
    memory = consts._fragments(mir.MemRef(start, 2), consts.Known(7, 2))
    after = consts._kills(memory, store, {index: consts.Known(4, 2)}, frozenset({5}), {})
    assert consts._cell(after, mir.MemRef(start, 2)) == consts.Known(7, 2)
    assert consts._cell(after, mir.MemRef(start.plus(4), 2)) == consts.Known(9, 2)


@pytest.mark.parametrize("kind,number,count,expected", [(mir.Kind.SHL, 255, 2, 1020),
                                                       (mir.Kind.SHR, 65535, 8, 255)])
def test_shift_count_width_does_not_narrow_the_value(kind, number, count, expected):
    """FPDEEP's word index became a byte fact because its shift count occupied one byte."""
    source, result = mir.Value(1, 0), mir.Value(2, 1)
    op = mir.Op(1, ir.Operation.BINARY, str(kind), (result,), (source,), kind=kind,
                args=(mir.Held(source, 2), mir.Const(count, 1)), results=(mir.Held(result, 2),))
    assert consts._result(op, {source: consts.Known(number, 2)}) == consts.Known(expected, 2)
