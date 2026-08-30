"""
What a call's arguments are, when they are not all pushed immediately before it.

calls.py's own matcher assumes every argument is pushed contiguously, right
before the call -- true most of the time, but BC's optimizer sometimes pushes
one argument early, runs an entirely separate, self-contained call, and only
then pushes the second argument and calls. That value is stranded on the real
stack under a nested call, and finding it needs tracking stack depth, not
address adjacency.

Scoped to one basic block, deliberately: a push's position on the stack is
only knowable if nothing between it and its call could have run instead of
what actually did -- exactly what a block boundary already means. An
ordinary instruction that provably never touches sp (iced's own
`stack_pointer_increment` and register-write info agree on this) is passed
over rather than treated as a gap; anything else -- a pop, arithmetic on sp,
a call this cannot account for -- resets the tracked depth to "unknown"
instead of guessing. That reset is a weakening of the invariant "`stack` is
exactly the topmost region of the real stack, in order", so it can only ever
cost a frame that was findable, never invent one that was not: whatever
happens below the reset point cannot un-happen to anything pushed above it.

What licenses trusting a *recognised* call's own gap -- treating it as
consuming exactly `arity(name)` long arguments and returning with nothing
else disturbed -- is not in this file. It rests on `calls.py`'s own callers
knowing, from the QuickBASIC 4.5 runtime source, that these routines are
callee-cleanup and clobber only ax/cx/dx/bx, never caller memory. That is
three separate claims collapsed into one `arity()` answer -- the count, that
cleanup is the callee's, and that the call returns to the very next byte --
and a wrong one does not fail loudly: it silently shifts every frame found
afterward in the block, since nothing here re-checks absolute depth once a
call's own popped amount has been trusted.
"""

from dataclasses import dataclass
from collections.abc import Callable

from iced_x86 import Code
from iced_x86 import Register

from qbopt.declen import Insn
from qbopt.blocks import Block
from qbopt.declen import INFO as _INFO
from qbopt.declen import WRITES as _WRITES

# a candidate argument push, and how many bytes it adds -- not every
# instruction that moves sp by this much is one of these (`push cs` moves it
# by 2 and is never an argument), so this is deliberately not the same table
# as "instructions that touch sp" below
PUSH_BYTES = {
    Code.PUSH_R16: 2,
    Code.PUSH_RM16: 2,
    Code.PUSHW_IMM8: 2,
    Code.PUSH_IMM16: 2,
    Code.PUSH_RM32: 4,
    Code.PUSHD_IMM8: 4,
    Code.PUSHD_IMM32: 4,
    # BC never emits this, but calls.py's own restoring() does -- re-analysing
    # already-rewritten code should not mistake it for an sp-mover it cannot
    # explain
    Code.PUSH_R32: 4,
}

_SP = (Register.SP, Register.ESP)


def _touches_sp(insn: Insn) -> bool:
    """Whether this instruction could move the stack pointer by any means.

    `stack_pointer_increment` is iced's own answer for the instructions that
    have one -- push, pop, call, ret and the like -- but reports 0 for a
    plain `add sp,imm` or `leave` exactly as it would for a `nop`, because
    neither is a "stack instruction" to it. Only a register-write check over
    sp/esp catches those too.
    """
    if insn.insn.stack_pointer_increment != 0:
        return True
    return any(used.register in _SP and used.access in _WRITES for used in _INFO.info(insn.insn).used_registers())


@dataclass(frozen=True, slots=True)
class Frame:
    """The pushes one call consumes, in push order -- deepest first."""

    call: Insn
    pushed: tuple[Insn, ...]


def frames(block: Block, calls: dict[int, str], arity: Callable[[str], int | None]) -> list[Frame]:
    """Every call in this block whose arguments are named, stack position only.

    `calls` maps a call instruction's own address to the routine it targets --
    module.of()'s own map, so membership already means "this is a call far".
    `arity` says how many long arguments a routine by that name takes, or
    None if it is not one this can reason about at all; an answer under one
    is treated the same as None; nothing here can name a call that takes no
    stack arguments.
    """
    stack: list[Insn] = []
    found: list[Frame] = []

    for insn in block.insns:
        name = calls.get(insn.at)
        needed = arity(name) if name is not None else None
        if needed is not None and needed >= 1:
            have = 0
            take: list[Insn] = []
            for pushed in reversed(stack):
                if have >= needed * 4:
                    break
                # every entry in `stack` passed the `in PUSH_BYTES` check below
                # before being appended, so this always finds one
                have += PUSH_BYTES.get(pushed.code, 0)
                take.append(pushed)
            if have != needed * 4:
                stack = []  # the arity crosses into bytes this block never explained
                continue
            take.reverse()
            found.append(Frame(insn, tuple(take)))
            del stack[len(stack) - len(take) :]
            continue
        if insn.code in PUSH_BYTES:
            stack.append(insn)
            continue
        if not _touches_sp(insn):
            continue
        # a pop, arithmetic on sp, a call this cannot account for -- anything
        # that could move the stack pointer without this knowing by how much
        stack = []

    return found
