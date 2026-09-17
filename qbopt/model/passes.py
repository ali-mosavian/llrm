"""
What a pass is: MIR in, MIR out, and nothing else.

`transform(body) -> body` is the whole contract. Anything a pass needs to
know about the module it is compiling is given when the pass is made, not
when it runs, so the signature cannot quietly grow a way to ask the machine
a question -- which is how `hoisted(body, dgroup, calls, bounds)` came to
take the module's layout and end up choosing registers.

Rule 5 is the reason this exists. Selected instruction semantics now begin
on ``lir.Insn.what``; what still ties MIR to its source machine is:

  Op.node      the instruction this was raised from. Lowering reads it, and
               so does anything asking what BC originally wrote.

  MirBody.origin
               value -> register. Sanctioned today for four readers with a
               reason -- which register pair a long arrived in, where a use
               came from -- and the last thing to go, because lowering and
               the allocator's identity baseline are built on it.

  Op.covers, Op.ref
               which of BC's bytes this stands for, and which fixup it
               carries. Facts about an object file, not about a program.

Each is a separate step with its own measurement. The class is what makes
them checkable: a pass whose only
entry point is `transform(body)` cannot reach anything else by accident.
"""

from dataclasses import dataclass

from qbopt.model.lir import LirBody
from qbopt.model.mir import MirBody


class MIRTransform:
    """One transformation over a body.

    Subclasses override `transform` and nothing else. `name` is what the
    pipeline lists it by and what `--only` matches.
    """

    name: str = ""

    def transform(self, body: MirBody) -> MirBody:
        raise NotImplementedError(f"{type(self).__name__} has no transform")

    def __repr__(self) -> str:
        return f"<{self.name or type(self).__name__}>"


class LIRTransform:
    """One transformation over a lowered body.

    The same contract as MIRTransform, one form down: subclasses override
    `transform` and nothing else, and a phase is added by writing a class
    rather than by editing the driver.

    Two bases rather than one generic over the form, because the two halves
    are what the split is: a MIR pass may name no register and a LIR pass
    may name nothing else, and a shared base would be a place for a pass to
    be written that does not know which half it is in.
    """

    name: str = ""

    def transform(self, body: "LirBody") -> "LirBody":
        raise NotImplementedError(f"{type(self).__name__} has no transform")

    def __repr__(self) -> str:
        return f"<{self.name or type(self).__name__}>"


@dataclass(frozen=True, slots=True)
class Where:
    """What a pass may be told about the module it is compiling.

    Not part of the contract above and not reachable from `transform`: a
    pass is handed this when it is made. Every field here is something MIR
    should eventually carry itself --

      dgroup   which segments the data group holds, which is how two
               addresses are known to be disjoint. Belongs on the MemRef:
               whether two references may alias is a question about the
               references.

      calls    address -> runtime routine name, which is how a call's
               effects are known. Belongs on the CALL operation, whose
               uses and defines already model them.

      bounds   where each variable begins, so an indexed operand can be
               bounded by the next thing named after it. Belongs on the
               MemRef for the same reason as dgroup.

    -- and until each does, this is where they are, named rather than
    threaded through every signature.

    `registers` is not one of those. It is how many values the target can
    keep live at once, which a pass that creates loop-carried values has to
    be told: strength reduction gave deedlines and qbdemo five new counters
    in a loop with six registers and every one of them was spilled. A
    number is not machine form -- a pass told "you have six" still knows
    nothing about x86 -- and zero means nothing was said, so nothing is
    priced.
    """

    dgroup: frozenset[int] = frozenset()
    calls: dict | None = None
    bounds: dict | None = None
    blocks: list | None = None
    found: object | None = None
    registers: int = 0
    # Values the target can keep live across an ordinary call. Like
    # ``registers``, this is a capacity rather than a register name. A loop
    # containing a call cannot spend the volatile part of the register file
    # on recurrences that live around its backedge.
    call_registers: int = 0
    # The multipliers an address may apply to an index register, empty where
    # nothing was said. Like `registers`, a fact about the target that names
    # no register: a pass told "an address may be base + index*2" still
    # knows nothing about how that is spelt.
    index_scales: frozenset[int] = frozenset()

    @property
    def named(self) -> dict:
        return self.calls or {}
