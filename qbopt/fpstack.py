"""
The x87 register stack, as values rather than as positions.

`ir.St(index)` names `st(i)` exactly as the operand does -- relative to
wherever the top happens to be -- and ir.py deliberately refuses to claim
that `st(0)` in one node is `st(0)` in the next. It is not: `fld` pushes, so
every slot below it is renamed, and two mentions of the same index in
different nodes are usually different physical registers.

That refusal is what makes the model sound and what stops anything reasoning
across two x87 instructions. This module supplies what it was missing: walk
a block forward, keep a stack of value identities, and resolve each `St` to
the value actually in that slot. After it, `fld [x]` twice names two
different values, and the two `fld dword ptr [si]` that once looked like a
load and a redundant reload are distinguishable by construction rather than
by remembering that an fld pushes.

Block-scoped, and the stack at a block's entry is unknown rather than empty:
BC leaves values on the x87 stack across a branch, so entering slots are
minted as their own values -- known to be distinct from each other and from
anything pushed later, and known to be nothing else. Depth is measured from
the block's entry the same way mir.Space.STACK measures a push slot from the
top of its own block, and for the same reason.

Nothing here emits. It answers "which value is in st(i) at this op", which
is what an x87 pass would have to ask first.
"""

from dataclasses import field
from dataclasses import dataclass

from qbopt import ir, mir
from qbopt.mir import Op
from qbopt.mir import Opaque
from qbopt.mir import MirBody

# The x87 stack is eight deep and BC never comes close, but a program that
# overflowed it would wrap rather than fault, so the depth is checked.
DEPTH = 8


@dataclass(frozen=True, slots=True)
class Float:
    """One value living on the x87 stack.

    `at` is where it was computed, or None for one that was already there when
    the block was entered. Identity is the id, so two Floats compare equal
    only when they are the same value in the same place.
    """

    id: int
    at: int | None = None

    def __str__(self) -> str:
        return f"f{self.id}" + ("" if self.at is None else f"@{self.at:#x}")


@dataclass(frozen=True, slots=True)
class Reading:
    """One op's x87 operands, resolved."""

    at: int
    uses: dict[int, Float] = field(default_factory=dict)  # st index -> the value there
    defines: Float | None = None  # new load or arithmetic result, before any pop
    popped: tuple[Float, ...] = ()  # what it took off


def _slots(operands) -> list[int]:
    """Every stack slot this operation names, in the order it names them.

    A float operand has no MIR value -- that is what makes this pass exist
    -- so it arrives as mir.Opaque with the resource's own name, "st0" and
    up. The name, never the operand inside it.
    """
    return [
        int(one.name[2:])
        for one in operands
        if isinstance(one, Opaque) and one.name.startswith("st") and one.name[2:].isdigit()
    ]


def readings(body: MirBody) -> dict[int, Reading]:
    """Which value is in each st(i) the ops of `body` name.

    Forward through each block from an unknown entry stack. An op this
    cannot model -- a barrier, a call, anything that is not one of the five
    float shapes but still touches the stack -- makes the whole stack
    unknown again rather than shifting it by a guess, which is the same
    refusal avail.py makes for memory and for the same reason: a wrong
    answer here is silent.
    """
    out: dict[int, Reading] = {}
    minted = 0

    for block in body.blocks:
        # Top first. Entering slots are minted lazily, so a block that never
        # reaches past its own pushes never invents one.
        stack: list[Float] = []
        entering = 0
        known = True

        def at(index: int) -> Float | None:
            nonlocal minted, entering
            if not known or not 0 <= index < DEPTH:
                return None
            while len(stack) <= index:
                if entering >= DEPTH or len(stack) >= DEPTH:
                    return None
                minted += 1
                entering += 1
                stack.append(Float(minted))
            return stack[index]

        for op in block.ops:
            named = _slots(op.args)
            destinations = _slots(op.results)
            if op.barrier or op.kind is mir.Kind.CALL or (op.stack is None and (named or destinations)):
                # It touches the stack in a way this does not model.
                known = False
                stack = []
                out[op.at] = Reading(op.at)
                continue
            if op.stack is None:
                continue
            if op.stack not in (-1, 0, 1):
                known = False
                stack = []
                out[op.at] = Reading(op.at)
                continue

            uses = {}
            for index in named:
                got = at(index)
                if got is None:
                    known = False
                    break
                uses[index] = got
            if not known:
                stack = []
                out[op.at] = Reading(op.at)
                continue

            made: Float | None = None
            popped: tuple[Float, ...] = ()
            if op.stack > 0:
                if op.stack != 1 or destinations != [0] or len(stack) >= DEPTH:
                    known = False
                    stack = []
                    out[op.at] = Reading(op.at)
                    continue
                minted += 1
                made = Float(minted, op.at)
                stack.insert(0, made)
            elif destinations:
                if (len(destinations) != 1 or op.op not in (
                    ir.Operation.FLOAT_ARITH, ir.Operation.FLOAT_ARITH_POP, ir.Operation.FLOAT_UNARY
                ) or at(destinations[0]) is None):
                    known = False
                    stack = []
                    out[op.at] = Reading(op.at)
                    continue
                minted += 1
                made = Float(minted, op.at)
                stack[destinations[0]] = made
            if op.stack < 0:
                popped = (stack[0],) if stack else ()
                if stack:
                    stack.pop(0)
                elif entering < DEPTH:
                    entering += 1
            out[op.at] = Reading(op.at, uses, made, popped)
    return out


def pushed_twice(body: MirBody) -> tuple[tuple[int, int], ...]:
    """Pairs of pushes of the same address that are two values, not one.

    The shape that made forward.py delete the second of two
    `fld dword ptr [si]`: same address, same bytes, and not redundant at
    all, because each one puts another value on the stack. Reported so the
    distinction is a fact this module states rather than one every reader of
    an x87 sequence has to remember.
    """
    found: list[tuple[int, int]] = []
    reads = readings(body)
    for block in body.blocks:
        loads = [op for op in block.ops if op.stack is not None and op.stack > 0]
        for one, other in zip(loads, loads[1:]):
            if not one.loads or not other.loads:
                continue
            if not mir.same_bytes(one.loads[0], other.loads[0]):
                continue
            first, second = reads.get(one.at), reads.get(other.at)
            if first is None or second is None:
                continue
            if first.defines is not None and second.defines is not None and first.defines != second.defines:
                found.append((one.at, other.at))
    return tuple(found)
