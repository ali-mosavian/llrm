"""What a counted-loop rewrite places before the loop: its seeds, skip test and exit values.

Built only from `induction.CountedLoop`, so every pass that replaces a loop's
counter enters and leaves the loop the same way.
"""

from dataclasses import field
from dataclasses import replace
from dataclasses import dataclass

from qbopt.model import mir
from qbopt.analysis import consts
from qbopt.analysis import induction


@dataclass
class Seeds:
    """Preheader values, constructed after the symbolic proof is complete.

    An operand the body proves constant is that constant, and `x+0`, `x-0`
    and `x*1` are `x`: no pass spells a zero start as a separate case.
    """

    serial: int
    variable: int
    at: int
    width: int
    ops: list[mir.Op]
    facts: dict = field(default_factory=dict)

    def _known(self, arg: mir.Arg) -> mir.Arg:
        fact = self.facts.get(arg.value) if isinstance(arg, mir.Held) else None
        return (
            mir.Const(consts.masked(fact.n, arg.width), arg.width)
            if fact is not None and fact.width >= arg.width
            else arg
        )

    def computed(self, kind: mir.Kind, args: tuple[mir.Arg, ...]) -> mir.Held | mir.Const:
        known = tuple(self._known(arg) for arg in args)
        match kind, known:
            case mir.Kind.ADD | mir.Kind.SUB, (_, mir.Const(n=n)) if consts.masked(n, self.width) == 0:
                return args[0]
            case mir.Kind.ADD, (mir.Const(n=n), _) if consts.masked(n, self.width) == 0:
                return args[1]
            case mir.Kind.MUL, (_, mir.Const(n=1)):
                return args[0]
            case mir.Kind.MUL, (mir.Const(n=1), _):
                return args[1]
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
