"""Two-address fixup: x86 writes into one of the registers it reads.

`add ax,bx` is `ax := ax + bx`, not `c := a + b`. Everything above LIR has
been three-address -- MIR says what an operation computes and names its
result separately -- so before a register can be assigned, the instruction
has to say that its destination and its first source are one place.

Where they are already the same value there is nothing to do, which is the
common case: the raise built the operation from the instruction, so its
first source *is* its destination. What needs work is an operation a pass
rewrote into a genuine three-address form, and then a copy has to go in
front of it: `c := a + b` is `c := a` then `c := c + b`.

LLVM's `TwoAddressInstructionPass`, and the same order: after phi
elimination, before coalescing -- which exists partly to remove the copies
this pass and that one just introduced.
"""

from dataclasses import replace

from qbopt import ir
from qbopt import lir
from qbopt.passes import LIRTransform

# Operations that read their destination. Everything else writes it outright.
# ir.py's vocabulary is coarser than a mnemonic: BINARY covers add, sub,
# and, or, xor and adc alike, and every one of them reads its destination.
# Widening multiply and divide use fixed registers; a single-result
# multiply with two sources instead reads its destination.
_TIED = frozenset({ir.Operation.BINARY, ir.Operation.UNARY})


class TwoAddress(LIRTransform):
    name = "twoaddr"

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return tied(body)


def tied(body: lir.LirBody) -> lir.LirBody:
    """`body` with every tied instruction reading what it writes."""
    changed = False
    blocks = []
    for block in body.blocks:
        insns: list[lir.Insn] = []
        for one in block.insns:
            fix = _untied(one)
            if fix is None:
                insns.append(one)
                continue
            insns += fix
            changed = True
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks)) if changed else body


def _nothing(beside: lir.Insn) -> tuple[int, int]:
    """An empty span at the neighbour's address: this claims no bytes.

    Never None. None means "ask the node how long it was", and the node is
    the instruction this was inserted beside -- whose bytes it already
    claims, so both would, and layout reports one byte claimed twice.
    """
    at = beside.covers[0] if beside.covers else beside.at
    return (at, at)


def _untied(one: lir.Insn) -> "list[lir.Insn] | None":
    """The copy and the fixed instruction, or None where it is already tied."""
    what = one.what
    if what is None or not what.dests or not what.sources:
        return None
    multiply = what.op is ir.Operation.MULTIPLY and len(what.dests) == 1 and len(what.sources) == 2
    if what.op not in _TIED and not multiply:
        return None
    into, first = what.dests[0], what.sources[0]
    if not isinstance(into, ir.Held) or not isinstance(first, (ir.Held, ir.Imm)):
        return None
    if isinstance(first, ir.Held) and into.value == first.value:
        return None
    move = lir.Insn(
        at=one.at,
        covers=_nothing(one),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (into,), (first,)),
        defines=(into.value,),
        uses=(first.value,) if isinstance(first, ir.Held) else (),
        op=one.op,
    )
    fixed = replace(one, what=ir.Semantics(what.op, what.name, what.dests, (into, *what.sources[1:]), what.target))
    remaining = {value.value for operand in what.sources[1:] for value in ir.values(operand)}
    uses = tuple(
        value for value in one.uses if not isinstance(first, ir.Held) or value != first.value or value in remaining
    )
    return [move, replace(fixed, uses=tuple(dict.fromkeys((into.value, *uses))))]
