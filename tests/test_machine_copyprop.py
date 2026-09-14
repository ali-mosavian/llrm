"""Allocated copies are redundant only when all incoming paths agree byte by byte."""

from dataclasses import replace

import pytest
from iced_x86 import Register

from qbopt.backend import peephole
from qbopt.model import ir, lir


def move(at, dest, source):
    return lir.Insn(at, (at, at), ir.Semantics(ir.Operation.MOVE, "mov", (dest,), (source,)), (), ())


@pytest.mark.parametrize("reverse_order", [False, True])
@pytest.mark.parametrize("change", ["none", "other", "partial", "source", "conditional", "unknown", "loop"])
def test_copy_at_join_requires_agreement_on_every_path(change, reverse_order):
    """An AX=BX copy survived a diamond despite unchanged contents; writing AH must keep it."""
    dest, source = ir.Reg(Register.AX, 2), ir.Reg(Register.BX, 2)
    first, last = move(0, dest, source), move(30, dest, source)
    branch = lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.BRANCH, "je", (), (), 20), (), ())
    middle = ()
    if change in ("other", "partial", "source", "loop"):
        register = {"other": Register.CX, "partial": Register.AH,
                    "source": Register.BX, "loop": Register.AH}[change]
        width = 1 if register == Register.AH else 2
        middle = (move(20, ir.Reg(register, width), ir.Imm(7, width)),)
    if change == "unknown":
        middle = (replace(last, at=20, what=None),)
    if change == "conditional":
        from types import SimpleNamespace
        from qbopt.frontend import declen
        decoded = declen.decode(bytes.fromhex("0f44c1"), 0)  # cmove ax,cx
        node = ir.Opaque(decoded, ir.instruction_effects(decoded, lambda *_: None))
        middle = (replace(last, at=20, what=None, op=SimpleNamespace(node=node)),)
    body = lir.LirBody("copies", 0, (
        lir.LirBlock(0, (first, branch), (10, 20)),
        lir.LirBlock(10, (), (30,)),
        lir.LirBlock(20, middle, (30,)),
        lir.LirBlock(30, (last,), (20, 40) if change == "loop" else ()),
        *((lir.LirBlock(40, (), ()),) if change == "loop" else ()),
    ), {}, {})
    if reverse_order:
        body = replace(body, blocks=body.blocks[::-1])
    result = peephole.Peephole().transform(body)
    assert any(one.at == 30 and one.what == last.what for one in result.insns) == (change not in ("none", "other"))


def test_high_byte_write_keeps_low_byte_copy_available():
    """Writing AH does not invalidate a known AL=BL relation across a block edge."""
    first = move(0, ir.Reg(Register.AX, 2), ir.Reg(Register.BX, 2))
    high = move(1, ir.Reg(Register.AH, 1), ir.Imm(7, 1))
    low = move(10, ir.Reg(Register.AL, 1), ir.Reg(Register.BL, 1))
    body = lir.LirBody("lanes", 0, (lir.LirBlock(0, (first, high), (10,)),
                                    lir.LirBlock(10, (low,), ())), {}, {})
    result = peephole.Peephole().transform(body)
    assert not any(one.at == 10 and one.what == low.what for one in result.insns)
