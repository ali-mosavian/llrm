"""
What a pass is: MIR in, MIR out, and nothing else.

`transform(body) -> body` is the whole contract. Anything a pass needs to
know about the module it is compiling is given when the pass is made, not
when it runs, so the signature cannot quietly grow a way to ask the machine
a question -- which is how `hoisted(body, dgroup, calls, bounds)` came to
take the module's layout and end up choosing registers.

Rule 5 is the reason this exists, and the rule is not yet true. What still
ties MIR to the machine, in the order it has to go:

  Op.made      an ir.Semantics over ir.Reg -- machine form, inside MIR.
               ir.Held is the operand kind that says a value instead, and
               the passes that write `made` have to use it.

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

Each of those is a separate step with its own measurement. The class is
first because it is what makes the others checkable: a pass whose only
entry point is `transform(body)` cannot reach anything else by accident.
"""

from dataclasses import dataclass

from qbopt.mir import MirBody


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
    """

    dgroup: frozenset[int] = frozenset()
    calls: dict | None = None
    bounds: dict | None = None
    blocks: list | None = None
    found: object | None = None

    @property
    def named(self) -> dict:
        return self.calls or {}
