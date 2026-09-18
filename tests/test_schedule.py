"""Post-allocation instruction scheduling must retain physical dependencies."""

from iced_x86 import Register

from qbopt.backend import cpu
from qbopt.backend import schedule
from qbopt.model import ir
from qbopt.model import lir


def _insn(at: int, operation: ir.Operation, name: str, dests, sources) -> lir.Insn:
    return lir.Insn(at, (at, at + 1), ir.Semantics(operation, name, dests, sources), (), ())


def _body(*insns: lir.Insn) -> lir.LirBody:
    return lir.LirBody("latency", 0, (lir.LirBlock(0, insns),), {}, {})


def _latency_chain() -> lir.LirBody:
    """C matmul had independent register work after a multiply's consumer."""
    multiply = _insn(
        0,
        ir.Operation.MULTIPLY,
        "imul",
        (ir.Reg(Register.EAX, 4),),
        (ir.Reg(Register.EAX, 4), ir.Reg(Register.ECX, 4)),
    )
    consume = _insn(
        1,
        ir.Operation.BINARY,
        "add",
        (ir.Reg(Register.EAX, 4),),
        (ir.Reg(Register.EAX, 4), ir.Reg(Register.EDX, 4)),
    )
    independent = _insn(
        2,
        ir.Operation.MOVE,
        "mov",
        (ir.Reg(Register.EBX, 4),),
        (ir.Reg(Register.ESI, 4),),
    )
    return _body(multiply, consume, independent)


def test_out_of_order_profile_fills_an_imul_dependency_gap() -> None:
    """P6 emitted ``imul; add; mov`` although the move hides multiply latency."""
    result = schedule.scheduled(_latency_chain(), cpu.profile("P6"))

    assert [one.what.name for one in result.insns] == ["imul", "mov", "add"]


def test_p6_partial_write_penalty_exposes_independent_work() -> None:
    """C CRC32 emitted ``mov ax,bx; add eax,esi; mov di,si`` across P6's merge stall."""
    partial = _insn(
        0,
        ir.Operation.MOVE,
        "mov",
        (ir.Reg(Register.AX, 2),),
        (ir.Reg(Register.BX, 2),),
    )
    wide_use = _insn(
        1,
        ir.Operation.BINARY,
        "add",
        (ir.Reg(Register.EAX, 4),),
        (ir.Reg(Register.EAX, 4), ir.Reg(Register.ESI, 4)),
    )
    independent = _insn(
        2,
        ir.Operation.MOVE,
        "mov",
        (ir.Reg(Register.DI, 2),),
        (ir.Reg(Register.SI, 2),),
    )

    result = schedule.scheduled(_body(partial, wide_use, independent), cpu.profile("P6"))

    assert [one.what.dests[0].register for one in result.insns] == [Register.AX, Register.DI, Register.EAX]


def test_in_order_profiles_keep_the_established_source_order() -> None:
    """386/486 have no safe latency-hiding issue window to exploit."""
    original = _latency_chain()

    assert schedule.scheduled(original, cpu.profile("386")) is original
    assert schedule.scheduled(original, cpu.profile("486")) is original


def test_pentium_orders_a_prefixed_move_before_its_uv_pair() -> None:
    """Pentium lost a U/V pair when ``mov di,si`` preceded prefixed ``mov eax,ecx``."""
    pairable = _insn(
        0,
        ir.Operation.MOVE,
        "mov",
        (ir.Reg(Register.DI, 2),),
        (ir.Reg(Register.SI, 2),),
    )
    u_only = _insn(
        1,
        ir.Operation.MOVE,
        "mov",
        (ir.Reg(Register.EAX, 4),),
        (ir.Reg(Register.ECX, 4),),
    )

    result = schedule.scheduled(_body(pairable, u_only), cpu.profile("P5"))

    assert [one.what.dests[0].register for one in result.insns] == [Register.EAX, Register.DI]


def test_flag_writers_remain_in_program_order() -> None:
    """A flag-producing add must not cross imul before a later flag reader."""
    multiply = _latency_chain().insns[0]
    add = _insn(
        1,
        ir.Operation.BINARY,
        "add",
        (ir.Reg(Register.EBX, 4),),
        (ir.Reg(Register.EBX, 4), ir.Reg(Register.ESI, 4)),
    )
    independent = _insn(
        2,
        ir.Operation.MOVE,
        "mov",
        (ir.Reg(Register.EDI, 4),),
        (ir.Reg(Register.EDX, 4),),
    )

    result = schedule.scheduled(_body(multiply, add, independent), cpu.profile("P6"))

    names = [one.what.name for one in result.insns]
    assert names.index("imul") < names.index("add")


def test_memory_access_is_a_hard_scheduling_boundary() -> None:
    """A potentially trapping source load must retain its position exactly."""
    multiply = _latency_chain().insns[0]
    load = _insn(
        1,
        ir.Operation.MOVE,
        "mov",
        (ir.Reg(Register.EBX, 4),),
        (ir.Mem(None, 4, Register.ESI),),
    )
    consume = _latency_chain().insns[1]
    independent = _latency_chain().insns[2]
    original = _body(multiply, load, consume, independent)

    assert schedule.scheduled(original, cpu.profile("P6")) is original
