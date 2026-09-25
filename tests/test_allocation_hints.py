from dataclasses import replace

import pytest
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import select
from qbopt.backend import allocate


def call_copy() -> lir.LirBody:
    define = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Imm(42, 2),)), (1,), ())
    copy = lir.Insn(3, (3, 3), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(2, 2),), (ir.Held(1, 2),)), (2,), (1,))
    use = lir.Insn(3, (3, 4), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(2, 2),)), (), (2,))
    return lir.LirBody("call-copy", 0, (lir.LirBlock(0, (define, copy, use)),), {}, {2: Register.EDI})


def test_call_input_is_computed_in_its_available_required_register() -> None:
    # Excess call copies grew SCREEN and contributed to E1M1's BASIC error 14.
    body = call_copy()
    assignment = allocate.allocate(body, body.pins)
    result = allocate.applied(body, assignment)
    assert not assignment.spilled
    emitted = []
    for one in result.insns:
        assert one.what is not None
        encoded = select.emit(one.what)
        assert encoded is not None
        emitted.append(encoded.code)
    assert b"".join(emitted) == bytes.fromhex("bf2a0057")


def test_fixed_result_can_remain_in_its_return_register() -> None:
    body = call_copy()
    assignment = allocate.allocate(body, {1: Register.EDI})
    assert not assignment.spilled
    assert assignment.where[1] == assignment.where[2] == Register.EDI


@pytest.mark.parametrize("barrier", ["overlap", "clobber", "class", "fixed"])
def test_copy_hint_cannot_override_a_register_requirement(barrier: str) -> None:
    body = call_copy()
    define, copy, use = body.insns
    assert use.what is not None
    pins = dict(body.pins)
    match barrier:
        case "overlap":
            extra = replace(use, at=4, covers=(4, 5), what=replace(use.what, sources=(ir.Held(1, 2),)), uses=(1,))
            body = replace(body, blocks=(lir.LirBlock(0, (define, copy, use, extra)),))
        case "clobber":
            extra = lir.Insn(2, (2, 2), None, (), (), clobbers=frozenset({Register.EDI}))
            body = replace(body, blocks=(lir.LirBlock(0, (define, extra, copy, use)),))
        case "class":
            byte_use = lir.Insn(
                2, (2, 2), ir.Semantics(ir.Operation.COMPARE, "cmp", (), (ir.Held(1, 1), ir.Imm(0, 1))), (), (1,)
            )
            body = replace(body, blocks=(lir.LirBlock(0, (define, byte_use, copy, use)),))
        case "fixed":
            pins[1] = Register.ECX
    assignment = allocate.allocate(body, pins)
    assert not assignment.spilled
    assert assignment.where[1] != Register.EDI
    assert assignment.where[2] == Register.EDI


def test_a_value_copied_on_to_a_fixed_register_is_seated_there_first() -> None:
    """A loop sum two copies from the return's AX lost AX to a counter placed first; the exit then moved it."""

    def move(at: int, dest: int, source: ir.Held | ir.Imm) -> lir.Insn:
        uses = (source.value,) if isinstance(source, ir.Held) else ()
        what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(dest, 2),), (source,))
        return lir.Insn(at, (at, at + 1), what, (dest,), uses)

    def push(at: int, value: int) -> lir.Insn:
        return lir.Insn(
            at, (at, at + 1), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Held(value, 2),)), (), (value,)
        )

    counter, total, copied, returned = 1, 2, 3, 4
    insns = (
        move(0, counter, ir.Imm(9, 2)),
        move(1, total, ir.Imm(0, 2)),
        push(2, total),
        push(3, counter),
        move(4, copied, ir.Held(total, 2)),
        move(5, returned, ir.Held(copied, 2)),
        push(6, returned),
    )
    body = lir.LirBody("chain", 0, (lir.LirBlock(0, insns),), {}, {returned: Register.EAX})
    assignment = allocate.allocate(body, body.pins)

    assert assignment.where[total] == assignment.where[copied] == Register.EAX
