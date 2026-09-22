"""What a counted-loop rewrite places before the loop: its seeds, skip test and exit values.

Built only from `induction.CountedLoop`, so every pass that replaces a loop's
counter enters and leaves the loop the same way.
"""

from dataclasses import replace
from dataclasses import dataclass

from qbopt.model import mir
from qbopt.analysis import induction


@dataclass
class Seeds:
    """Preheader values, placed after the symbolic proof is complete.

    Stated in full: `canonical.identities` folds the neutral terms.
    """

    serial: int
    variable: int
    at: int
    width: int
    ops: list[mir.Op]

    def computed(self, kind: mir.Kind, args: tuple[mir.Arg, ...]) -> mir.Held:
        value = mir.Value(self.serial, self.at, variable=self.variable)
        self.serial += 1
        self.variable += 1
        self.ops.append(mir.computed(self.at, kind, value, args, self.width))
        return mir.Held(value, self.width)

    def held(self, arg: mir.Held | mir.Const) -> mir.Held:
        """`arg` as a value, for a phi to name."""
        if isinstance(arg, mir.Held):
            return arg
        value = mir.Value(self.serial, self.at, variable=self.variable)
        self.serial += 1
        self.variable += 1
        self.ops.append(mir.computed(self.at, mir.Kind.COPY, value, (arg,), self.width))
        return mir.Held(value, self.width)


def skip_guard(proof: induction.CountedLoop, at: int, flags: mir.Value) -> tuple[mir.Op, mir.Op]:
    """The preheader compare and branch that leave a counted loop before its first trip."""
    args, test = induction.skipped(proof)
    compare = replace(
        proof.compare,
        at=at,
        defines=(flags,),
        uses=tuple(arg.value for arg in args if isinstance(arg, mir.Held)),
        source_backed=False,
        args=args,
        raised=None,
        absorbed=(),
        symbol=False,
    )
    branch = replace(
        proof.branch,
        at=at,
        name="",
        defines=(),
        uses=(flags,),
        source_backed=False,
        test=test,
        target=proof.exit,
        raised=None,
        absorbed=(),
        symbol=False,
    )
    return compare, branch


def leaving(replacement: induction.ControlReplacement, seeds: Seeds) -> dict[int, mir.Phi]:
    """Exit phis of a replaced counter, reading its exit value after a trip and its start after none."""
    proof = replacement.counted
    if not replacement.exits:
        return {}
    value = seeds.held(induction.exit_value(proof, seeds.computed))
    start = proof.phi.incoming[proof.preheader]
    return {
        id(phi): replace(phi, incoming={**dict.fromkeys(phi.incoming, value.value), proof.preheader: start})
        for phi in replacement.exits
    }
