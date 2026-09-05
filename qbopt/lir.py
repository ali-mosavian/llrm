"""What each operation requires of a register, so nothing else has to guess.

MIR names values, not registers -- `docs/variables.md` is the plan for
getting the last of the register names out of it -- and a pass that moves
code has no business choosing where a value lives. But the machine does
have requirements, and until they were written down they were scattered:
`transform._implicit` was a predicate a pass consulted to decide whether to
give up, `target.BASES` was a set applied ad hoc, and the rest lived
in whatever `select.emit` happened to encode.

They belong in one place, between MIR and the allocator: lowering says what
an instruction needs, and the allocator satisfies it or splits a live range
until it can. A pass says only that a value is live somewhere.

Two kinds:

**Fixed.** The instruction reads or writes a particular register and does
not name it. `imul word [k]` multiplies by ax and leaves dx:ax; `cwd`
extends ax into dx; a shift by a variable count takes it in cl. Nothing in
the operand list says so, which is exactly why a rename breaks it.

**A class.** Any register from a set. 16-bit addressing reaches memory
through bx, bp, si or di and nothing else -- `[dx+0Ah]` has no encoding --
so a value some instruction reaches a cell by lives in one of those.
"""

from dataclasses import dataclass

from iced_x86 import Register
from iced_x86 import Register_

from qbopt import ir
from qbopt import target

@dataclass(frozen=True, slots=True)
class Insn:
    """One machine instruction, as the thing that emits it needs it.

    `what` is None where the bytes are carried rather than generated -- a
    barrier, the restore idiom, an emulated x87 site. `covers` says which of
    the original bytes it stands for, and `at` is where it began, which is
    what a fixup and a branch target are still keyed on.

    `op` is the MIR operation it came from. It is here because select.py,
    layout.py and relocate.py all still ask MIR questions of a machine
    instruction, and taking that away is a change to three modules rather
    than to this one. Nothing above LIR may read it.
    """

    at: int
    covers: "tuple[int, int] | None"
    what: "ir.Semantics | None"
    # Which values this instruction writes and reads, by id. Carried
    # because the allocator is a LIR pass and needs the interference graph:
    # without them it had to be handed the MIR body the instruction came
    # from, and read `op.defines` off that. Ids rather than values, because
    # that is what an operand names -- ir.py sits below mir.py.
    #
    # Flags are not here. They are one register nothing is placed in, and
    # every consumer of liveness dropped them again on the way past.
    defines: tuple[int, ...]
    uses: tuple[int, ...]
    op: object


@dataclass(frozen=True, slots=True)
class Phi:
    """One value that is two definitions above this block, by id."""

    result: int
    incoming: "tuple[tuple[int, int], ...]"  # (predecessor block, the value arriving)


@dataclass(frozen=True, slots=True)
class LirBlock:
    at: int
    insns: tuple[Insn, ...]
    succ: tuple[int, ...] = ()
    # Where two definitions of one value meet: the result, and which value
    # arrives on each predecessor's edge. Not instructions -- nothing is
    # emitted for a phi -- and liveness still has to know the block defines
    # the result, or it stays live around every path reaching its use.
    #
    # Carried in full rather than as results alone because eliminating them
    # is a pass, and it needs the edges: LLVM runs PHIElimination before
    # allocation for the same reason, replacing each with a copy at the end
    # of the predecessor it came from.
    phis: tuple["Phi", ...] = ()

    @property
    def arrives(self) -> tuple[int, ...]:
        """What this block defines before its first instruction."""
        return tuple(one.result for one in self.phis)


@dataclass(frozen=True, slots=True)
class LirBody:
    """One procedure, lowered. Blocks in the order they are emitted."""

    name: str
    entry: int
    blocks: tuple[LirBlock, ...]
    # What the raise saw each value in. The allocator's input, not its
    # answer, and the fallback for an operand it could not place.
    origin: dict
    pins: dict

    @property
    def insns(self) -> "tuple[Insn, ...]":
        return tuple(one for block in self.blocks for one in block.insns)
