"""Frame-derived near pointers retain the stack segment through lowering."""

from pathlib import Path

import pytest
from iced_x86 import Decoder
from iced_x86 import Register

from qbopt.backend import lower
from qbopt.backend import select
from qbopt.cfront import compile as cfront
from qbopt.frontend.declen import BITNESS
from qbopt.model import ir
from qbopt.model import mir
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


FIXTURES = Path(__file__).resolve().parents[1] / "fixtures" / "c"


def test_a_frame_derived_indirect_cell_keeps_its_stack_segment() -> None:
    """Matmul overwrote loop spills when ``[bx]`` defaulted to DS instead of SS."""
    cell = ir.Mem(Addr(Space.LITERAL, 0, segment=Register.SS), 4, Register.BX, base=ir.Held(1, 2))
    made = select.move_into(cell, Register.EAX)

    assert made is not None
    got = next(iter(Decoder(BITNESS, made.code, ip=0)))
    assert got.memory_base == Register.BX
    assert got.memory_segment == Register.SS


def test_frame_fill_lowering_uses_the_stack_segment() -> None:
    """A frame memset must copy SS, rather than DS, into string-destination ES."""

    class Values:
        next = 100

        def fresh(self) -> int:
            self.next += 1
            return self.next

    op = mir.Op(
        1,
        ir.Operation.NOTHING,
        "",
        (),
        (),
        kind=mir.Kind.FILL,
        args=(mir.Const(0, 1), mir.Const(32, 2), mir.Const(0, 2)),
        stores=(mir.MemRef(Addr(Space.FRAME, -32), 1, space=Space.FRAME),),
    )

    lowered = lower._fill(op, Values())

    pushed = [one.sources for one in lowered if one.op is ir.Operation.PUSH]
    assert (ir.Reg(Register.SS, 2),) in pushed
    assert (ir.Reg(Register.DS, 2),) not in pushed


@pytest.mark.full
def test_local_array_fill_uses_the_stack_segment_end_to_end() -> None:
    """Sieve returned zero: its frame memset copied DS to ES and cleared unrelated data."""
    text = cfront.compiled((FIXTURES / "fill.cgs").read_text(), "fill", optimise=True)
    lines = [line.strip() for line in text.splitlines()]
    start, end = lines.index("_fill_local proc far"), lines.index("_fill_local endp")
    body = lines[start:end]

    assert "push ss" in body
    assert "push ds" not in body
