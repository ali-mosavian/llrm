from dataclasses import replace

from iced_x86 import Code
from iced_x86 import FlowControl

from qbopt.abi import runtime
from qbopt.frontend.blocks import Block
from qbopt.objectfile.module import Module
from qbopt.frontend.extent import Partition
from qbopt.frontend.blocks import local_call_target


def interfaces(
    module: Module, partition: Partition, blocks: tuple[Block, ...], external: dict[int, runtime.Contract]
) -> dict[int, runtime.Contract]:
    cleanup = cleanups(
        module, partition, blocks, {at: rule.cleanup for at, rule in external.items() if rule.cleanup is not None}
    )
    inputs = frozenset({runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX, runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI})
    result = dict(external)
    for block in blocks:
        for insn in block.insns:
            target = local_call_target(module, insn)
            if target is None or insn.at not in cleanup:
                continue
            result[insn.at] = replace(
                runtime.contract(f"native:{target:04x}"),
                inputs=inputs,
                cleanup=cleanup[insn.at],
                evidence="Native C/Pascal input ABI assumed; cleanup from reachable local returns "
                "and a verified PUSH CS near-to-far adapter when required. "
                "No memory, preservation, termination or floating-stack claims.",
            )
    return result


def cleanups(
    module: Module, partition: Partition, blocks: tuple[Block, ...], external: dict[int, int]
) -> dict[int, int]:
    if partition.unexplained or partition.conflicts:
        return {}
    local: dict[int, tuple[int, bool]] = {}
    for body in partition.bodies:
        owned = tuple(block for block in blocks if any(lo <= block.at < hi for lo, hi in body.ranges))
        starts = {block.at for block in owned}
        if any(target not in starts for block in owned for target in block.succ):
            continue
        returns = [insn for block in owned for insn in block.insns if insn.flow == FlowControl.RETURN]
        near = {Code.RETNW, Code.RETNW_IMM16}
        far = {Code.RETFW, Code.RETFW_IMM16}
        if not returns or any(insn.code not in near | far for insn in returns):
            continue
        kinds = {insn.code in far for insn in returns}
        if len(kinds) != 1:
            continue
        returns_far = kinds.pop()
        address = 4 if returns_far else 2
        sizes = {insn.insn.stack_pointer_increment - address for insn in returns}
        if len(sizes) == 1:
            local[body.seed] = sizes.pop(), returns_far
    previous = {
        insn.at: before
        for block in blocks
        for before, insn in zip(block.insns, block.insns[1:], strict=False)
        if before.end == insn.at
    }
    result: dict[int, int] = {}
    for block in blocks:
        for insn in block.insns:
            if insn.flow != FlowControl.CALL:
                continue
            target = local_call_target(module, insn)
            if target in local:
                size, returns_far = local[target]
                adapter = previous.get(insn.at)
                if not returns_far or (adapter is not None and adapter.code == Code.PUSHW_CS):
                    result[insn.at] = size
            elif insn.at in module.calls and insn.at in external:
                result[insn.at] = external[insn.at]
    return result


def stack_recovery(
    module: Module,
    partition: Partition,
    blocks: tuple[Block, ...],
    cleanup: dict[int, int],
) -> dict[int, int]:
    """Visible stack bytes recovered by a call, including a synthetic CS.

    A near CALL followed by a far return needs `push cs` immediately before
    it.  The callee's semantic cleanup remains its argument count; physical
    frame validation additionally credits the far return for consuming that
    explicit segment word.
    """
    semantic = cleanups(module, partition, blocks, cleanup)
    previous = {
        insn.at: before
        for block in blocks
        for before, insn in zip(block.insns, block.insns[1:], strict=False)
        if before.end == insn.at
    }
    far_targets: set[int] = set()
    for body in partition.bodies:
        owned = tuple(block for block in blocks if any(lo <= block.at < hi for lo, hi in body.ranges))
        returns = [insn for block in owned for insn in block.insns if insn.flow == FlowControl.RETURN]
        if returns and all(insn.code in (Code.RETFW, Code.RETFW_IMM16) for insn in returns):
            far_targets.add(body.seed)
    return {
        at: size
        + (
            2
            if (target := local_call_target(module, insn)) in far_targets
            and (before := previous.get(at)) is not None
            and before.code == Code.PUSHW_CS
            else 0
        )
        for block in blocks
        for insn in block.insns
        if (at := insn.at) in semantic
        for size in [semantic[at]]
    }
