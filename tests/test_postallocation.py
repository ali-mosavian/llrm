"""Small Tier-1 checks for physical-register rewrites after allocation."""

from dataclasses import replace

import pytest
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.backend import select
from qbopt.backend import peephole
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def _pair(operation: ir.Operation, name: str, tail: tuple[lir.Insn, ...]) -> tuple[lir.LirBody, lir.Insn]:
    left = ir.Reg(Register.EBX, 4)
    right = ir.Reg(Register.ECX, 4)
    combined = lir.Insn(
        1,
        (1, 3),
        ir.Semantics(operation, name, (left,), (left, right)),
        (10,),
        (1, 2),
    )
    copied = lir.Insn(
        3,
        (3, 3),
        ir.Semantics(ir.Operation.MOVE, "mov", (right,), (left,)),
        (11,),
        (10,),
    )
    return lir.LirBody("pair", 0, (lir.LirBlock(0, (combined, copied, *tail), ()),), {}, {}), copied


@pytest.mark.parametrize(
    "operation,name",
    [
        (ir.Operation.BINARY, "add"),
        (ir.Operation.BINARY, "and"),
        (ir.Operation.BINARY, "or"),
        (ir.Operation.BINARY, "xor"),
        (ir.Operation.MULTIPLY, "imul"),
    ],
)
def test_commutative_result_copy_uses_the_dying_other_operand(operation: ir.Operation, name: str) -> None:
    """Mandel's inner loop emitted ``imul ebx,ecx; mov ecx,ebx``.

    Both input values die at the multiply and ECX is immediately overwritten
    with its result.  The equivalent ``imul ecx,ebx`` leaves the only live
    physical result in the same place without executing the copy.
    """
    left = ir.Reg(Register.EBX, 4)
    right = ir.Reg(Register.ECX, 4)
    shift = lir.Insn(
        3,
        (3, 5),
        ir.Semantics(ir.Operation.BINARY, "sar", (right,), (right, ir.Imm(7, 1))),
        (12,),
        (11,),
    )
    overwrite = lir.Insn(
        5,
        (5, 7),
        ir.Semantics(ir.Operation.MOVE, "mov", (left,), (ir.Imm(0, 4),)),
        (13,),
        (),
    )
    body, copied = _pair(operation, name, (shift, overwrite))

    result = peephole.transferred(body)

    assert result.insns[0].what == ir.Semantics(operation, name, (right,), (right, left))
    assert result.insns[1].what.op is ir.Operation.NOTHING
    assert result.insns[1].defines == copied.defines
    assert result.insns[2:] == (shift, overwrite)


def test_commutative_result_copy_keeps_a_still_live_first_operand() -> None:
    """Changing which multiply input is destroyed is legal only when the old destination dies."""
    left = ir.Reg(Register.EBX, 4)
    use = lir.Insn(
        3,
        (3, 5),
        ir.Semantics(ir.Operation.COMPARE, "cmp", (), (left, ir.Imm(0, 4))),
        (),
        (10,),
    )
    body, _copied = _pair(ir.Operation.MULTIPLY, "imul", (use,))

    assert peephole.transferred(body) == body


def test_commutative_result_copy_keeps_source_owned_copy_bytes() -> None:
    """A real input instruction is not the synthetic transfer this rewrite may erase."""
    left = ir.Reg(Register.EBX, 4)
    overwrite = lir.Insn(
        5,
        (5, 7),
        ir.Semantics(ir.Operation.MOVE, "mov", (left,), (ir.Imm(0, 4),)),
        (13,),
        (),
    )
    body, copied = _pair(ir.Operation.MULTIPLY, "imul", (overwrite,))
    owned = replace(copied, covers=(3, 5))
    body = replace(body, blocks=(replace(body.blocks[0], insns=(body.insns[0], owned, overwrite)),))

    assert peephole.transferred(body) == body


def _high_extract(tail: tuple[lir.Insn, ...]) -> lir.LirBody:
    value = mir.Value(1, 1)
    source = mir.Value(2, 1)
    operation = mir.Op(
        1,
        ir.Operation.BINARY,
        "shr",
        (value,),
        (source,),
        kind=mir.Kind.SHR,
        args=(mir.Held(source, 4), mir.Const(16, 1)),
        results=(mir.Held(value, 4),),
    )
    wide = ir.Reg(Register.EDX, 4)
    cell = ir.Mem(Addr(Space.FRAME, -4), 4, through=Register.BP)
    load = lir.Insn(
        1,
        (1, 1),
        ir.Semantics(ir.Operation.MOVE, "mov", (wide,), (cell,)),
        (1,),
        (),
        op=operation,
        symbol=False,
    )
    shift = lir.Insn(
        1,
        (1, 1),
        ir.Semantics(ir.Operation.BINARY, "shr", (wide,), (wide, ir.Imm(16, 1))),
        (1,),
        (1,),
        op=operation,
    )
    return lir.LirBody("extract", 0, (lir.LirBlock(0, (load, shift, *tail), ()),), {}, {})


def _return_high() -> lir.Insn:
    value = mir.Value(1, 1)
    operation = mir.Op(
        2,
        ir.Operation.RETURN,
        "",
        (),
        (value,),
        kind=mir.Kind.RETURN,
        args=(mir.Held(value, 2),),
        reads_complete=True,
    )
    return lir.Insn(
        2,
        (2, 3),
        ir.Semantics(ir.Operation.RETURN, "retf", (), ()),
        (),
        (1,),
        op=operation,
        requires=((ir.Held(1, 2), Register.DX),),
    )


def test_synthetic_high_extract_loads_only_the_high_word() -> None:
    """Mandel loaded a spilled dword and shifted it solely to return the high word."""
    low = ir.Reg(Register.DX, 2)
    returned = _return_high()

    result = peephole.high_extracts(_high_extract((returned,)))

    load, anchor = result.insns[:2]
    assert load.what == ir.Semantics(
        ir.Operation.MOVE,
        "mov",
        (low,),
        (ir.Mem(Addr(Space.FRAME, -2), 2, through=Register.BP),),
    )
    assert anchor.what.op is ir.Operation.NOTHING
    assert anchor.defines == (1,)
    assert result.insns[2:] == (returned,)


@pytest.mark.parametrize("hazard", ["full-result", "flags", "source-load"])
def test_high_extract_keeps_observable_wide_load_or_shift_effects(hazard: str) -> None:
    """Narrowing is legal only for a synthetic load with dead upper lanes and flags."""
    if hazard == "full-result":
        tail = (
            lir.Insn(
                2,
                (2, 4),
                ir.Semantics(ir.Operation.COMPARE, "cmp", (), (ir.Reg(Register.EDX, 4), ir.Imm(0, 4))),
                (),
                (1,),
            ),
            _return_high(),
        )
    elif hazard == "flags":
        tail = (lir.Insn(2, (2, 4), ir.Semantics(ir.Operation.BRANCH, "je", (), (), 9), (), ()),)
    else:
        tail = (_return_high(),)
    body = _high_extract(tail)
    if hazard == "source-load":
        load = replace(body.insns[0], covers=(1, 3), symbol=None)
        body = replace(body, blocks=(replace(body.blocks[0], insns=(load, *body.insns[1:])),))

    assert peephole.high_extracts(body) == body


def test_discarded_x87_result_has_a_register_pop_encoding() -> None:
    """qcport gib.c's ``(void)gib_crandom(rng)`` reached fresh OMF output as
    ``fstp st(0)``.  The allocated instruction was valid but selection knew
    only memory stores, so the complete optimized build was unencodable.
    """
    what = ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (ir.St(0),), (ir.St(0),))

    made = select.emit(what)

    assert made is not None
    assert made.code == bytes.fromhex("ddd8")
