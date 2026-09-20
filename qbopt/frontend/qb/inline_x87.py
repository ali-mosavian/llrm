"""QB-owned final spelling of rich inline math operations.

The common backend intentionally knows only the x87 operations already in
MIR.  The QB HIR has a few additional mathematical intrinsics.  They survive
optimization and x87 stack allocation by name, then this final frontend step
replaces the now-physical ``st(0) -> st(0)`` operation with measured bytes.
It is deliberately too late to affect optimization or allocation.
"""

from dataclasses import replace
from dataclasses import dataclass

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import masm

# 80387 encodings, decoded in the regression test as a second source of truth.
# FEXP2 splits x into nearest integer n and fraction f, then computes
# (2**f) * (2**n).  F2XM1's required f range is satisfied by that split.
_CODE = {
    "fsin": bytes.fromhex("d9fe"),
    "fcos": bytes.fromhex("d9ff"),
    "fatan": bytes.fromhex("d9e8d9f3"),  # fld1; fpatan
    "flog2": bytes.fromhex("d9e8d9c9d9f1"),  # fld1; fxch; fyl2x
    "fexp2": bytes.fromhex("d9c0d9fcd9c9d8e1d9f0d9e8dec1d9fdddd9"),
}


@dataclass(frozen=True, slots=True)
class Finalized:
    body: lir.LirBody
    callees: dict[int, masm.Callee]


def finalized(body: lir.LirBody, *, parameter_bytes: int = 0) -> Finalized:
    """Replace allocated QB intrinsic pseudos with inline-byte placeholders."""
    if not 0 <= parameter_bytes <= 0xFFFF:
        raise ValueError("QB far-return cleanup exceeds 16 bits")
    sites: dict[int, masm.Callee] = {}
    blocks = []
    for block in body.blocks:
        instructions = []
        for instruction in block.insns:
            what = instruction.what
            code = _CODE.get(what.name) if what is not None else None
            if code is None:
                if what is not None and what.op is ir.Operation.RETURN and parameter_bytes:
                    instruction = replace(
                        instruction,
                        what=replace(what, sources=(ir.Imm(parameter_bytes, 2),)),
                    )
                instructions.append(instruction)
                continue
            if what.op is not ir.Operation.FLOAT_UNARY or what.dests != (ir.St(0),) or what.sources != (ir.St(0),):
                raise ValueError(f"{what.name} must be allocated as st(0) -> st(0)")
            sites[instruction.at] = masm.Callee(f"$inline_{what.name}", False, (code,))
            instructions.append(replace(instruction, what=ir.Semantics(ir.Operation.CALL, what.name, (), ())))
        blocks.append(replace(block, insns=tuple(instructions)))
    return Finalized(replace(body, blocks=tuple(blocks)), sites)


def expansion(name: str) -> bytes:
    """Return one audited expansion for diagnostics and stage dumps."""
    return _CODE[name]
