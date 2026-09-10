"""Huge-array addressing must carry across 64K, not silently wrap within one segment."""

from itertools import count

import pytest

from qbopt.backend import lower, pointers
from qbopt.model import ir, mir
from qbopt.objectfile.module import Addr, Space


def operation():
    base, offset, result = (mir.Value(index, 0) for index in (1, 2, 3))
    return mir.Op(0, ir.Operation.NOTHING, "", (result,), (base, offset),
                  kind=mir.Kind.PTR_OFFSET, args=(mir.Held(base, 4), mir.Held(offset, 4)),
                  results=(mir.Held(result, 4),))


def execute(parts, pointer, offset, memory=None):
    values = {1: pointer, 2: offset}
    for part in parts:
        if part.op is ir.Operation.MOVE:
            values[part.dests[0].value] = memory[part.sources[0].addr]
            continue
        left, right = [arg.value if isinstance(arg, ir.Imm) else values[arg.value] for arg in part.sources]
        match part.name:
            case "and": answer = left & right
            case "add": answer = left + right
            case "shr": answer = left >> right
            case "shl": answer = left << right
            case "or": answer = left | right
            case _: raise AssertionError(part)
        values[part.dests[0].value] = answer & 0xffffffff
    return values[3]


@pytest.mark.parametrize("pointer,offset,expected", [
    (0x20000000, 0, 0x20000000),
    (0x2000fffe, 2, 0x30000000),
    (0x20000000, 0x20002, 0x40000002),
    (0x20000004, -8, 0x1000fffc),
    (0xf000fffe, 2, 0x00000000),
])
def test_dos_pointer_offset_carries_and_borrows(pointer, offset, expected):
    """A huge INTEGER at byte 65536 must advance the selector, not read element zero again."""
    op = operation()
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),))
    making = lower.Lowering(body, {1, 2, 3}, {}, (), pointer_model=pointers.Model(12))
    parts = tuple(part.what for part in making.expand(op))
    assert execute(parts, pointer, offset & 0xffffffff) == expected
    assert all(isinstance(arg, (ir.Held, ir.Imm)) for part in parts for arg in (*part.sources, *part.dests))


def test_pointer_abi_is_not_inferred_from_cpu():
    op = operation()
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),))
    with pytest.raises(lower.Unlowered, match="pointer ABI"):
        lower.Lowering(body, {3}, {}, (), cpu="386").expand(op)
    parts = pointers.Model(3).offset(ir.Held(1, 4), ir.Held(2, 4), ir.Held(3, 4), count(10).__next__)
    assert execute(parts, 0x2000fffe, 2) == 0x20080000


def test_pointer_lowering_cannot_destroy_an_unrelated_live_condition():
    with pytest.raises(lower.Unlowered, match="live condition"):
        lower._check_inserted_conditions((operation(),), frozenset({mir.Value(4, 0, flags=True)}))


@pytest.mark.parametrize("shift", [-1, 16])
def test_unknown_selector_models_are_rejected(shift):
    with pytest.raises(ValueError, match="selector shift"):
        pointers.Model(shift)


@pytest.mark.parametrize("shift,expected", [(12, 0x30000000), (3, 0x20080000)])
def test_runtime_pointer_abi_controls_crossing(shift, expected):
    """Byte 65536 uses the runtime selector stride, not a CPU-derived DOS constant."""
    address = Addr(Space.EXTERNAL, 0, 7)
    model = pointers.Model(ir.Mem(address, 1, disp_width=2))
    parts = model.offset(ir.Held(1, 4), ir.Held(2, 4), ir.Held(3, 4), count(10).__next__)
    assert execute(parts, 0x2000fffe, 2, {address: shift}) == expected
