from dataclasses import replace

from iced_x86 import Register
from iced_x86 import Register_

from qbopt.backend import machinedce
from qbopt.model import ir
from qbopt.model import lir


def _mov(at: int, value: int, register: Register_ = Register.AX) -> lir.Insn:
    return lir.Insn(
        at,
        (at, at + 3),
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(register, 2),), (ir.Imm(value, 2),)),
        (value,),
        (),
    )


def test_dead_register_definition_is_eliminated_across_a_cfg_edge() -> None:
    """An allocated result overwritten on every successor used to survive."""
    dead = _mov(0, 1)
    overwrite = _mov(5, 2)
    body = lir.LirBody(
        "machine-dce",
        0,
        (
            lir.LirBlock(0, (dead,), (5,)),
            lir.LirBlock(5, (overwrite,), ()),
        ),
        {},
        {},
    )

    result = machinedce.eliminated(body)

    first = next(block for block in result.blocks if block.at == 0)
    assert first.insns[0].what.op is ir.Operation.NOTHING
    assert first.insns[0].defines == dead.defines


def test_definition_live_on_one_successor_is_kept() -> None:
    definition = _mov(0, 1)
    body = lir.LirBody(
        "machine-dce",
        0,
        (
            lir.LirBlock(0, (definition,), (5, 10)),
            lir.LirBlock(5, (_mov(5, 2),), ()),
            lir.LirBlock(
                10,
                (
                    lir.Insn(
                        10,
                        (10, 12),
                        ir.Semantics(
                            ir.Operation.MOVE,
                            "mov",
                            (ir.Reg(Register.BX, 2),),
                            (ir.Reg(Register.AX, 2),),
                        ),
                        (3,),
                        (1,),
                    ),
                ),
                (),
            ),
        ),
        {},
        {},
    )

    result = machinedce.eliminated(body)

    assert result.blocks[0].insns[0].what == definition.what


def test_dead_compare_is_eliminated_when_the_next_compare_replaces_flags() -> None:
    first = lir.Insn(
        0,
        (0, 2),
        ir.Semantics(
            ir.Operation.COMPARE,
            "cmp",
            (),
            (ir.Reg(Register.AX, 2), ir.Reg(Register.BX, 2)),
        ),
        (1,),
        (),
    )
    second = replace(first, at=2, covers=(2, 4), defines=(2,))
    branch = lir.Insn(
        4,
        (4, 6),
        ir.Semantics(ir.Operation.BRANCH, "je", (), (), target=10),
        (),
        (2,),
    )
    body = lir.LirBody(
        "machine-dce",
        0,
        (
            lir.LirBlock(0, (first, second, branch), (10, 20)),
            lir.LirBlock(10, (), ()),
            lir.LirBlock(20, (), ()),
        ),
        {},
        {},
    )

    result = machinedce.eliminated(body)

    assert result.blocks[0].insns[0].what.op is ir.Operation.NOTHING
    assert result.blocks[0].insns[1].what == second.what


def test_dead_value_is_kept_when_its_flags_feed_a_branch() -> None:
    add = lir.Insn(
        0,
        (0, 3),
        ir.Semantics(
            ir.Operation.BINARY,
            "add",
            (ir.Reg(Register.AX, 2),),
            (ir.Reg(Register.AX, 2), ir.Imm(1, 2)),
        ),
        (1,),
        (),
    )
    branch = lir.Insn(
        3,
        (3, 5),
        ir.Semantics(ir.Operation.BRANCH, "jne", (), (), target=10),
        (),
        (1,),
    )
    body = lir.LirBody(
        "machine-dce",
        0,
        (
            lir.LirBlock(0, (add, branch), (10, 20)),
            lir.LirBlock(10, (), ()),
            lir.LirBlock(20, (), ()),
        ),
        {},
        {},
    )

    result = machinedce.eliminated(body)

    assert result.blocks[0].insns[0].what == add.what


def test_dead_memory_load_is_kept() -> None:
    load = lir.Insn(
        0,
        (0, 3),
        ir.Semantics(
            ir.Operation.MOVE,
            "mov",
            (ir.Reg(Register.AX, 2),),
            (ir.Mem(None, 2, Register.BX),),
        ),
        (1,),
        (),
    )
    body = lir.LirBody(
        "machine-dce",
        0,
        (
            lir.LirBlock(0, (load,), (5,)),
            lir.LirBlock(5, (_mov(5, 2),), ()),
        ),
        {},
        {},
    )

    result = machinedce.eliminated(body)

    assert result.blocks[0].insns[0].what == load.what


def test_call_stack_cleanup_is_never_dead_machine_work() -> None:
    """C calls lost ``add sp,N`` cleanup when a later compare killed its flags."""
    cleanup = lir.Insn(
        0,
        (0, 3),
        ir.Semantics(
            ir.Operation.BINARY,
            "add",
            (ir.Reg(Register.SP, 2),),
            (ir.Reg(Register.SP, 2), ir.Imm(4, 2)),
        ),
        (),
        (),
    )
    compare = lir.Insn(
        3,
        (3, 5),
        ir.Semantics(
            ir.Operation.COMPARE,
            "cmp",
            (),
            (ir.Reg(Register.AX, 2), ir.Reg(Register.BX, 2)),
        ),
        (1,),
        (),
    )
    branch = lir.Insn(
        5,
        (5, 7),
        ir.Semantics(ir.Operation.BRANCH, "je", (), (), target=10),
        (),
        (1,),
    )
    body = lir.LirBody(
        "machine-dce",
        0,
        (
            lir.LirBlock(0, (cleanup, compare, branch), (10, 20)),
            lir.LirBlock(10, (), ()),
            lir.LirBlock(20, (), ()),
        ),
        {},
        {},
    )

    result = machinedce.eliminated(body)

    assert result.blocks[0].insns[0].what == cleanup.what
