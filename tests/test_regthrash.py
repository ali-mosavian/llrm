import pytest
from iced_x86 import OpKind
from iced_x86 import Decoder
from iced_x86 import Mnemonic
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import select
from qbopt.backend import regthrash

MASK = 0xFFFFFFFF


def _reg(register):
    return ir.Reg(register, 4)


def _insn(at, name, operation, dests, sources):
    return lir.Insn(at, (at, at), ir.Semantics(operation, name, dests, sources), (), ())


def _run(block, state):
    """What the emitted bytes of `block` leave in the registers."""
    state = dict(state)
    for one in block.insns:
        for insn in Decoder(16, select.emit(one.what).code):
            into = insn.op0_register
            match insn.mnemonic:
                case Mnemonic.MOV:
                    state[into] = (
                        state[insn.op1_register] if insn.op1_kind == OpKind.REGISTER else insn.immediate(1)
                    ) & MASK
                case Mnemonic.ADD:
                    state[into] = (state[into] + state[insn.op1_register]) & MASK
                case Mnemonic.CDQ:
                    state[Register.EDX] = MASK if state[Register.EAX] & 0x80000000 else 0
                case Mnemonic.IDIV:
                    signed = lambda value, bits: value - (1 << bits) if value >> (bits - 1) else value
                    dividend = signed(state[Register.EDX] << 32 | state[Register.EAX], 64)
                    divisor = signed(state[into], 32)
                    assert divisor, "divide by zero"
                    quotient = abs(dividend) // abs(divisor) * (1 if (dividend < 0) == (divisor < 0) else -1)
                    state[Register.EAX], state[Register.EDX] = quotient & MASK, (dividend - quotient * divisor) & MASK
                case other:
                    raise NotImplementedError(other)
    return state


def _body(insns, read):
    """`insns`, then a block that reads only `read` and overwrites the rest.

    A body's exit keeps every register live, so the overwrites are what make
    the others dead after `insns`.
    """
    killed = tuple(
        _insn(10 + index, "mov", ir.Operation.MOVE, (_reg(one),), (ir.Imm(0, 4),))
        for index, one in enumerate(
            (Register.EAX, Register.EBX, Register.ECX, Register.EDX, Register.ESI, Register.EDI)
        )
    )
    after = lir.LirBlock(1, (_insn(9, "push", ir.Operation.PUSH, (), (_reg(read),)), *killed))
    return lir.LirBody("thrash", 0, (lir.LirBlock(0, tuple(insns), (1,)), after), {}, {})


@pytest.mark.parametrize("divisor", [Register.ESI, Register.ECX])
def test_oimad_remainder_is_not_renamed_out_of_edx(divisor):
    """oimad's BENCHMARK adler loop hung before its first mark: `idiv`'s remainder is
    fixed in EDX, so renaming it to ESI emitted the same `idiv esi` behind a
    `mov esi,edx` that overwrote the divisor with the dividend's sign. With the
    divisor elsewhere the copy survives and ESI gets the sign, not the remainder."""
    edi, esi, eax, edx = (_reg(one) for one in (Register.EDI, Register.ESI, Register.EAX, Register.EDX))
    body = _body(
        (
            _insn(0, "mov", ir.Operation.MOVE, (_reg(divisor),), (ir.Imm(0xFFF1, 4),)),
            _insn(1, "mov", ir.Operation.MOVE, (eax,), (edi,)),
            _insn(2, "cdq", ir.Operation.EXTEND, (edx,), (eax,)),
            _insn(3, "idiv", ir.Operation.DIVIDE, (eax, edx), (edx, eax, _reg(divisor))),
            _insn(4, "mov", ir.Operation.MOVE, (esi,), (edx,)),
        ),
        Register.ESI,
    )
    start = {one: 0 for one in (Register.EAX, Register.ECX, Register.EDX, Register.ESI)} | {Register.EDI: 100000}
    result = regthrash.thrashed(body)
    assert _run(result.blocks[0], start)[Register.ESI] == 100000 % 0xFFF1


def test_a_rename_does_not_merge_the_producers_other_operand():
    """`ecx += edx; edx = ecx` renamed to `edx = ecx; edx += edx` doubles ecx
    instead of adding edx: the producer read the register it was renamed into."""
    ecx, edx = _reg(Register.ECX), _reg(Register.EDX)
    body = _body(
        (
            _insn(0, "add", ir.Operation.BINARY, (ecx,), (ecx, edx)),
            _insn(1, "mov", ir.Operation.MOVE, (edx,), (ecx,)),
        ),
        Register.EDX,
    )
    result = regthrash.thrashed(body)
    assert _run(result.blocks[0], {Register.ECX: 5, Register.EDX: 7})[Register.EDX] == 12
