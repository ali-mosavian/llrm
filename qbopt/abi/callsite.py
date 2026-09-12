from dataclasses import replace

from iced_x86 import Code
from iced_x86 import Register

from qbopt.abi import runtime
from qbopt.frontend import blocks
from qbopt.frontend.declen import Insn
from qbopt.objectfile.module import Module


def caller_cleanup(following: Insn | None) -> bool:
    if following is None:
        return False
    instruction = following.insn
    return (
        instruction.code in (Code.ADD_RM16_IMM8, Code.ADD_RM16_IMM16)
        and instruction.op0_register == Register.SP
        and 0 < instruction.immediate(1) < 0x8000
    )


def inferred(found: Module, contracts: dict[int, runtime.Contract]) -> dict[int, runtime.Contract]:
    candidates = {
        at: rule for at, rule in contracts.items() if rule.inputs is None and not rule.name.upper().startswith("B$")
    }
    if not candidates:
        return {}
    decoded = blocks.instructions(found)
    if isinstance(decoded, str):
        return {}
    by_address = {one.at: one for one in decoded}
    inputs = frozenset({runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX, runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI})
    result = {}
    for at, rule in candidates.items():
        call = by_address.get(at)
        if call is None:
            continue
        caller = caller_cleanup(by_address.get(call.end))
        result[at] = replace(
            rule,
            inputs=inputs,
            cleanup=0 if caller else rule.cleanup,
            evidence=(
                "C caller cleanup assumed from adjacent ADD SP; "
                if caller
                else "Pascal callee cleanup assumed; argument byte count remains unknown; "
            )
            + "language ABI takes no incoming arithmetic flags; all GP inputs retained; "
            "memory, clobber and control effects unchanged",
        )
    return result
