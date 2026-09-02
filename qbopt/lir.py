"""What each operation requires of a register, so nothing else has to guess.

MIR names values, not registers -- `docs/variables.md` is the plan for
getting the last of the register names out of it -- and a pass that moves
code has no business choosing where a value lives. But the machine does
have requirements, and until they were written down they were scattered:
`transform._implicit` was a predicate a pass consulted to decide whether to
give up, `regalloc.ADDRESSING` was a set applied ad hoc, and the rest lived
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

# What the encoding permits, which is not what the allocator may hand out:
# bp is legal and is how every frame slot is reached, and it is also the
# frame pointer, so regalloc keeps it out of AVAILABLE. Asking the two
# questions with one set made every `[bp-12h]` in the corpus look like a
# violated requirement -- 183 of them.
#
# See docs/a32.md for what the 67h prefix does to this: behind it every
# register is a base, and this becomes the 8086 fallback rather than the
# rule.
ADDRESSING: frozenset[Register_] = frozenset({Register.BX, Register.BP, Register.SI, Register.DI})


@dataclass(frozen=True, slots=True)
class Need:
    """Where an operand has to live: one register, or any of a set."""

    where: frozenset[Register_]

    @property
    def fixed(self) -> Register_ | None:
        """The register, where there is only one it can be."""
        return next(iter(self.where)) if len(self.where) == 1 else None


def _root(register: Register_) -> Register_:
    return ir.ROOT.get(register, register)


def _shifted(what: ir.Semantics) -> bool:
    """A shift whose count is a register takes it in cl and says so."""
    return (what.name or "") in ("shl", "shr", "sar", "rol", "ror", "rcl", "rcr") and any(
        isinstance(one, ir.Reg) and _root(one.register) is Register.ECX for one in what.sources
    )


def _on_the_stack(what: ir.Semantics) -> bool:
    """Whether this is an x87 operation, which shares no register with the rest.

    `ir` models `fdivp` as a DIVIDE, so the widening rule below claimed it
    reads dx:ax -- 45 of them in the corpus, every one a requirement BC's
    own code does not satisfy, because there is nothing in ax to satisfy it
    with.
    """
    return any(isinstance(one, ir.St) for one in (*what.dests, *what.sources)) or (
        what.name or ""
    ).startswith("f")


def reads(what: ir.Semantics) -> dict[Register_, Need]:
    """Registers this operation reads whether or not it names them.

    Keyed on the root it reads, so a caller can ask "does this operation
    require the value in that register to be there" without knowing how the
    instruction is written.
    """
    out: dict[Register_, Need] = {}
    if _on_the_stack(what):
        return out
    # The widening forms: one destination written down means `imul r,r/m`,
    # which names everything. More than one means dx:ax, which names
    # neither and reads ax.
    if what.op in (ir.Operation.MULTIPLY, ir.Operation.DIVIDE) and len(what.dests) != 1:
        out[Register.EAX] = Need(frozenset({Register.EAX}))
        if what.op is ir.Operation.DIVIDE:
            out[Register.EDX] = Need(frozenset({Register.EDX}))
    if what.op is ir.Operation.EXTEND:
        out[Register.EAX] = Need(frozenset({Register.EAX}))
    if _shifted(what):
        out[Register.ECX] = Need(frozenset({Register.ECX}))
    for one in (*what.dests, *what.sources):
        for where in (
            getattr(one, "through", None),
            getattr(one, "index", None),
            getattr(getattr(one, "addr", None), "base", None),
        ):
            if where is not None and where is not Register.NONE:
                out[_root(where)] = Need(frozenset(_root(x) for x in ADDRESSING))
    return out


def writes(what: ir.Semantics) -> dict[Register_, Need]:
    """Registers this operation writes whether or not it names them."""
    out: dict[Register_, Need] = {}
    if _on_the_stack(what):
        return out
    if what.op in (ir.Operation.MULTIPLY, ir.Operation.DIVIDE) and len(what.dests) != 1:
        out[Register.EAX] = Need(frozenset({Register.EAX}))
        out[Register.EDX] = Need(frozenset({Register.EDX}))
    if what.op is ir.Operation.EXTEND:
        out[Register.EDX] = Need(frozenset({Register.EDX}))
    return out
