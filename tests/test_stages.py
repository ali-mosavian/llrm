"""The dump rule 4 sends you to, and what it was not saying.

`_operand` printed a cell's address and nothing else, so a based cell --
one that names the value that computed its address, and after allocation
the register that value was placed in -- rendered identically to one BC
addressed itself. Two stage files could differ in the only field that
mattered and diff clean.
"""

import sys
from pathlib import Path

from iced_x86 import Register

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))

import stages

from qbopt import ir
from qbopt.module import Addr
from qbopt.module import Space


def test_a_based_cell_renders_the_value_and_the_register_it_was_placed_in() -> None:
    """`[es:bx+0x0]` said nothing about which value reached it."""
    where = Addr(Space.LITERAL, 0x2, base=Register.SI)
    placed = ir.Mem(where, 2, Register.BX, 2, 1, base=ir.Held(17, 2))
    said = stages._operand(placed)
    assert "v17" in said, f"the cell does not say which value reached it: {said}"
    assert "BX" in said, f"the cell does not say where that value was placed: {said}"


def test_a_cell_nothing_based_renders_as_it_did() -> None:
    """Every other line in every stage file stays byte-identical."""
    where = Addr(Space.LITERAL, 0x2, base=Register.SI)
    assert stages._operand(ir.Mem(where, 2)) == f"[{where}]"


def test_a_based_cell_nothing_placed_says_so() -> None:
    """Before allocation there is no register, and the dump has to show
    that rather than print a register that is not there."""
    where = Addr(Space.LITERAL, 0x2, base=Register.SI)
    unplaced = ir.Mem(where, 2, Register.NONE, 2, 1, base=ir.Held(17, 2))
    said = stages._operand(unplaced)
    assert "v17" in said and "BX" not in said, said
