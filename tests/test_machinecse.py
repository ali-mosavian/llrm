from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import machinecse
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def _lea(value: int, at: int) -> lir.Insn:
    address = ir.Address(Addr(Space.FRAME, -100), Register.BP, offset=-100, disp_width=1)
    return lir.Insn(
        at,
        (at, at + 3),
        ir.Semantics(ir.Operation.ADDRESS, "lea", (ir.Reg(Register.BX, 2),), (address,)),
        (value,),
        (),
    )


def _body(*insns: lir.Insn) -> lir.LirBody:
    return lir.LirBody("machine-cse", 0, (lir.LirBlock(0, insns),), {}, {})


def test_repeated_frame_address_is_anchored_when_registers_are_unchanged() -> None:
    """C nbody emitted ``lea bx,[bp-100]`` twice around one x87 update."""
    first, repeated = _lea(1, 0), _lea(2, 5)
    store = lir.Insn(
        3,
        (3, 5),
        ir.Semantics(
            ir.Operation.MOVE,
            "mov",
            (ir.Mem(Addr(Space.FRAME, -2), 2, Register.BP, disp_width=1),),
            (ir.Reg(Register.AX, 2),),
        ),
        (),
        (),
    )

    result = machinecse.eliminated(_body(first, store, repeated))

    assert result.insns[0].what == first.what
    assert result.insns[2].what.op is ir.Operation.NOTHING
    assert result.insns[2].defines == repeated.defines, "the virtual definition and byte ownership must survive"


def test_repeated_frame_address_is_kept_after_destination_changes() -> None:
    first, repeated = _lea(1, 0), _lea(2, 5)
    overwrite = lir.Insn(
        3,
        (3, 5),
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.BX, 2),), (ir.Imm(7, 2),)),
        (3,),
        (),
    )

    result = machinecse.eliminated(_body(first, overwrite, repeated))

    assert result.insns[2].what == repeated.what


def test_repeated_frame_address_is_eliminated_across_a_cfg_edge() -> None:
    """Lowering rebuilt the same frame address in two consecutive blocks."""
    first, repeated = _lea(1, 0), _lea(2, 5)
    body = lir.LirBody(
        "machine-cse",
        0,
        (lir.LirBlock(0, (first,), (5,)), lir.LirBlock(5, (repeated,), ())),
        {},
        {},
    )

    result = machinecse.eliminated(body)

    successor = next(block for block in result.blocks if block.at == 5)
    assert successor.insns[0].what.op is ir.Operation.NOTHING
    assert successor.insns[0].defines == repeated.defines


def test_repeated_frame_address_is_eliminated_after_agreeing_branches() -> None:
    first, second, repeated = _lea(1, 1), _lea(2, 2), _lea(3, 5)
    body = lir.LirBody(
        "machine-cse",
        0,
        (
            lir.LirBlock(0, (), (1, 2)),
            lir.LirBlock(1, (first,), (5,)),
            lir.LirBlock(2, (second,), (5,)),
            lir.LirBlock(5, (repeated,), ()),
        ),
        {},
        {},
    )

    result = machinecse.eliminated(body)

    successor = next(block for block in result.blocks if block.at == 5)
    assert successor.insns[0].what.op is ir.Operation.NOTHING


def test_repeated_frame_address_is_kept_when_one_branch_disagrees() -> None:
    first, repeated = _lea(1, 1), _lea(3, 5)
    overwrite = lir.Insn(
        2,
        (2, 4),
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.BX, 2),), (ir.Imm(7, 2),)),
        (2,),
        (),
    )
    body = lir.LirBody(
        "machine-cse",
        0,
        (
            lir.LirBlock(0, (), (1, 2)),
            lir.LirBlock(1, (first,), (5,)),
            lir.LirBlock(2, (overwrite,), (5,)),
            lir.LirBlock(5, (repeated,), ()),
        ),
        {},
        {},
    )

    result = machinecse.eliminated(body)

    successor = next(block for block in result.blocks if block.at == 5)
    assert successor.insns[0].what == repeated.what


def test_repeated_frame_address_is_kept_across_an_opaque_call() -> None:
    """A callee may replace every physical input and output register."""
    first, repeated = _lea(1, 0), _lea(3, 5)
    call = lir.Insn(
        3,
        (3, 5),
        ir.Semantics(ir.Operation.CALL, "call", (), (), target=10),
        (2,),
        (),
    )
    body = lir.LirBody(
        "machine-cse",
        0,
        (
            lir.LirBlock(0, (first,), (3,)),
            lir.LirBlock(3, (call,), (5,)),
            lir.LirBlock(5, (repeated,), ()),
        ),
        {},
        {},
    )

    result = machinecse.eliminated(body)

    successor = next(block for block in result.blocks if block.at == 5)
    assert successor.insns[0].what == repeated.what


def test_address_coefficients_are_part_of_the_expression_identity() -> None:
    """The IR ignores encoding registers when comparing addresses.

    That is correct for alias identity but not for machine value numbering:
    ``ax + si*2`` and ``si + ax*2`` read the same lanes and compute different
    numbers.  Machine CSE must compare their complete encoded forms.
    """
    first = lir.Insn(
        0,
        (0, 3),
        ir.Semantics(
            ir.Operation.ADDRESS,
            "lea",
            (ir.Reg(Register.BX, 2),),
            (ir.Address(None, Register.AX, Register.SI, 2),),
        ),
        (1,),
        (),
    )
    different = lir.Insn(
        3,
        (3, 6),
        ir.Semantics(
            ir.Operation.ADDRESS,
            "lea",
            (ir.Reg(Register.BX, 2),),
            (ir.Address(None, Register.SI, Register.AX, 2),),
        ),
        (2,),
        (),
    )

    result = machinecse.eliminated(_body(first, different))

    assert result.insns[1].what == different.what
