"""One immutable description of a code-generation tuning target.

The public drivers still accept the historical strings.  They are resolved
here once, so selection, allocation and MIR profitability cannot each grow a
slightly different list of targets.  Costs retain their current evidence
status: entries from ``cycles.timings`` are rankings, while ``timing.py``
contains the smaller audited subset used for legality-changing decisions.
"""

from dataclasses import dataclass

from qbopt.cycles import timings


@dataclass(frozen=True, slots=True)
class Profile:
    name: str
    issue_width: int
    in_order: bool
    prefix_cost: int
    partial_register_stall: int
    register_capacity: int = 6
    call_register_capacity: int = 2
    address_scales: frozenset[int] = frozenset({1, 2, 4, 8})
    _costs: tuple[tuple[str, int], ...] = ()
    _latencies: tuple[tuple[str, int], ...] = ()

    def cost(self, operation: str) -> int:
        """The existing target-ranking cost for one named instruction form."""
        try:
            return dict(self._costs)[operation]
        except KeyError as error:
            raise KeyError(f"{self.name} has no cost for {operation}") from error

    def latency(self, operation: str) -> int:
        """The existing dependency latency, distinct from occupancy cost."""
        try:
            return dict(self._latencies)[operation]
        except KeyError as error:
            raise KeyError(f"{self.name} has no latency for {operation}") from error


_I386_COSTS = {
    "shift_ri": 3,
    "alu_rr": 2,
    "mov_rr": 2,
    "imul_r32": 22,
    "idiv_r32": 43,
    "cdq": 2,
}


def _profile(name: str) -> Profile:
    if name == "386":
        costs = tuple(_I386_COSTS.items())
        return Profile(name, 1, True, 0, 0, _costs=costs, _latencies=costs)
    at = timings.ARCHS.index(name)
    return Profile(
        name,
        timings.ISSUE[at],
        bool(timings.INORDER[at]),
        timings.PREFIX[at],
        timings.PARTIAL_STALL[at],
        _costs=tuple((operation, values[at]) for operation, values in timings.COST.items()),
        _latencies=tuple((operation, values[at]) for operation, values in timings.LATENCY.items()),
    )


_PROFILES = tuple(_profile(name) for name in ("386", *timings.ARCHS))
_BY_NAME = {one.name: one for one in _PROFILES}


def names() -> tuple[str, ...]:
    return tuple(_BY_NAME)


def profile(value: str | Profile) -> Profile:
    if isinstance(value, Profile):
        return value
    try:
        return _BY_NAME[value]
    except KeyError as error:
        raise ValueError(f"unknown CPU target: {value}") from error
