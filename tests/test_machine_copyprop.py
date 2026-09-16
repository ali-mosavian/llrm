"""Allocated copies are redundant only when all incoming paths agree byte by byte."""

from dataclasses import replace

import pytest
from iced_x86 import Register
from iced_x86 import Register_

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import peephole


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
        register = {"other": Register.CX, "partial": Register.AH, "source": Register.BX, "loop": Register.AH}[change]
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
    body = lir.LirBody(
        "copies",
        0,
        (
            lir.LirBlock(0, (first, branch), (10, 20)),
            lir.LirBlock(10, (), (30,)),
            lir.LirBlock(20, middle, (30,)),
            lir.LirBlock(30, (last,), (20, 40) if change == "loop" else ()),
            *((lir.LirBlock(40, (), ()),) if change == "loop" else ()),
        ),
        {},
        {},
    )
    if reverse_order:
        body = replace(body, blocks=body.blocks[::-1])
    result = peephole.Peephole().transform(body)
    assert any(one.at == 30 and one.what == last.what for one in result.insns) == (change not in ("none", "other"))


def test_high_byte_write_keeps_low_byte_copy_available():
    """Writing AH does not invalidate a known AL=BL relation across a block edge."""
    first = move(0, ir.Reg(Register.AX, 2), ir.Reg(Register.BX, 2))
    high = move(1, ir.Reg(Register.AH, 1), ir.Imm(7, 1))
    low = move(10, ir.Reg(Register.AL, 1), ir.Reg(Register.BL, 1))
    body = lir.LirBody("lanes", 0, (lir.LirBlock(0, (first, high), (10,)), lir.LirBlock(10, (low,), ())), {}, {})
    result = peephole.Peephole().transform(body)
    assert not any(one.at == 10 and one.what == low.what for one in result.insns)


def test_copy_source_is_forwarded_to_an_explicit_use() -> None:
    """ls_face_key spent `mov bx,ax` only to compare BX; the loop paid one instruction each trip."""
    ax, bx = ir.Reg(Register.AX, 2), ir.Reg(Register.BX, 2)
    copied = move(0, bx, ax)
    compared = lir.Insn(
        1,
        (1, 1),
        ir.Semantics(ir.Operation.COMPARE, "cmp", (), (bx, ir.Imm(116, 2))),
        (),
        (),
    )
    overwritten = move(2, bx, ir.Imm(7, 2))
    body = lir.LirBody("ls_face_key", 0, (lir.LirBlock(0, (copied, compared, overwritten)),), {}, {})

    result = peephole.Peephole().transform(body)

    assert copied.what not in [one.what for one in result.insns]
    assert any(one.at == compared.at and one.what.sources[0] == ax for one in result.insns)


@pytest.mark.parametrize("changed", [None, Register.AX, Register.AH, Register.BX, Register.BH])
def test_copy_forwarding_across_a_join_requires_every_lane_on_every_path(changed: Register_ | None) -> None:
    """A reaching copy crosses a diamond only while neither full nor partial register is clobbered."""
    ax, bx = ir.Reg(Register.AX, 2), ir.Reg(Register.BX, 2)
    copied = move(0, bx, ax)
    branch = lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.BRANCH, "je", (), (), 20), (), ())
    middle = ()
    if changed is not None:
        width = 1 if changed in (Register.AH, Register.BH) else 2
        middle = (move(20, ir.Reg(changed, width), ir.Imm(7, width)),)
    compared = lir.Insn(
        30,
        (30, 30),
        ir.Semantics(ir.Operation.COMPARE, "cmp", (), (bx, ir.Imm(116, 2))),
        (),
        (),
    )
    body = lir.LirBody(
        "join",
        0,
        (
            lir.LirBlock(0, (copied, branch), (10, 20)),
            lir.LirBlock(10, (), (30,)),
            lir.LirBlock(20, middle, (30,)),
            lir.LirBlock(30, (compared,), ()),
        ),
        {},
        {},
    )

    result = peephole.Peephole().transform(body)
    actual = next(one.what.sources[0] for one in result.insns if one.at == compared.at)

    assert actual == (ax if changed is None else bx)


def test_copy_forwarding_does_not_rename_a_fixed_register_use() -> None:
    """GCC/LLVM both recheck constraints: CWD still reads AX even when AX is a copy of BX."""
    ax, bx, dx = ir.Reg(Register.AX, 2), ir.Reg(Register.BX, 2), ir.Reg(Register.DX, 2)
    copied = move(0, ax, bx)
    fixed = lir.Insn(
        1,
        (1, 1),
        ir.Semantics(ir.Operation.EXTEND, "cwd", (dx,), (ax,)),
        (),
        (),
    )
    body = lir.LirBody("fixed", 0, (lir.LirBlock(0, (copied, fixed)),), {}, {})

    result = peephole.Peephole().transform(body)

    assert next(one.what.sources[0] for one in result.insns if one.at == fixed.at) == ax


def test_copy_forwarding_maps_matching_subregister_lanes() -> None:
    """An AX=BX fact forwards AL to BL after AH changes; the surviving low-byte fact is sufficient."""
    copied = move(0, ir.Reg(Register.AX, 2), ir.Reg(Register.BX, 2))
    high = move(1, ir.Reg(Register.AH, 1), ir.Imm(7, 1))
    compared = lir.Insn(
        2,
        (2, 2),
        ir.Semantics(ir.Operation.COMPARE, "cmp", (), (ir.Reg(Register.AL, 1), ir.Imm(3, 1))),
        (),
        (),
    )
    body = lir.LirBody("subregister", 0, (lir.LirBlock(0, (copied, high, compared)),), {}, {})

    result = peephole.Peephole().transform(body)

    assert next(one.what.sources[0] for one in result.insns if one.at == compared.at) == ir.Reg(Register.BL, 1)
