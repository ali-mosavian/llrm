"""One immutable description of a code-generation tuning target.

The public drivers still accept the historical strings.  They are resolved
here once, so selection, allocation and MIR profitability cannot each grow a
slightly different list of targets.  Costs retain their current evidence
status: entries from ``cycles.timings`` are rankings, while ``timing.py``
contains the smaller audited subset used for legality-changing decisions.
"""

from dataclasses import dataclass

from qbopt.cycles import timings
from qbopt.model.passes import AddressForm
from qbopt.model.passes import OperationCosts


@dataclass(frozen=True, slots=True)
class Profile:
    name: str
    issue_width: int
    in_order: bool
    prefix_cost: int
    partial_register_stall: int
    register_capacity: int = 6
    call_register_capacity: int = 2
    # Preferred forms only. The complete legal set, including the costed
    # secondary address-size-prefixed form, is in ``address_forms`` below.
    address_scales: frozenset[int] = frozenset({1})
    _costs: tuple[tuple[str, int], ...] = ()
    _latencies: tuple[tuple[str, int], ...] = ()
    # Appended so the positional shape of the pre-profile interface remains
    # compatible. Drivers and MIR deliberately use this field by name.
    operations: OperationCosts = OperationCosts()
    # P5 is in-order but can issue a U/V pair only for a restricted set of
    # forms.  This is deliberately distinct from generic issue width and is
    # appended for the same positional compatibility as ``operations``.
    pentium_pairing: bool = False
    # Appended to retain the public positional shape above. 16-bit medium
    # model can use 386 32-bit SIB addressing through an address-size prefix;
    # it is legal but never silently promoted to a native/free scale, and is
    # nevertheless considered before spill/recompute.
    address_forms: tuple[AddressForm, ...] = ()
    # GCC's target-independent complete-peel default is sixteen iterations.
    # Keep it in the immutable profile so another target can extend the
    # policy without teaching MIR a CPU name.
    max_unroll_iterations: int = 16

    def cost(self, operation: str) -> int:
        """The existing target-ranking cost for one named instruction form."""
        try:
            return dict(self._costs)[operation]
        except KeyError as error:
            raise KeyError(f"{self.name} has no cost for {operation}") from error

    def prices(self, operation: str) -> bool:
        """Whether this profile has an explicit ranking for a form."""
        return operation in dict(self._costs)

    def latency(self, operation: str) -> int:
        """The existing dependency latency, distinct from occupancy cost."""
        try:
            return dict(self._latencies)[operation]
        except KeyError as error:
            raise KeyError(f"{self.name} has no latency for {operation}") from error


_I386_COSTS = {
    "alu_rr": 2,
    "alu_rm": 6,
    "alu_mr": 8,
    "mov_rr": 2,
    "mov_rm": 4,
    "mov_mr": 2,
    "mov_ri": 2,
    "shift_ri": 3,
    "movzx": 4,
    "imul_r32": 22,
    "imul_m32": 26,
    "mul_r16": 22,
    "mul_r32": 38,
    "div_r16": 27,
    "idiv_r32": 43,
    "idiv_m32": 47,
    "cdq": 2,
    "push_r": 2,
    "push_m": 6,
    "push_i": 2,
    "pop_r": 4,
    # Intel's 80386 instruction table: POP m16/m32 is five clocks. This is
    # not the four-unit register form used by the compiler-tuning table.
    "pop_m": 5,
    "pop_seg": 8,
    "mov_seg_r": 8,
    "les": 8,
    "nop": 3,
    "jmp_short": 7,
    "jcc": 7,
    "call_far": 37,
    "ret_far": 18,
    "lahf": 2,
    "sahf": 3,
    "lea": 2,
    "leave": 6,
    # GCC's i386 tuning table prices x87 loads/stores at eight units and
    # arithmetic at 23/27/88. Memory arithmetic includes both components.
    "x87_load": 8,
    "x87_store": 8,
    "x87_convert_store": 35,
    "x87_add": 23,
    "x87_add_m": 31,
    "x87_mul": 27,
    "x87_mul_m": 35,
    "x87_div": 88,
    "x87_div_m": 96,
    "x87_control_load": 8,
    "x87_control_store": 8,
}


def _operation_costs(costs: dict[str, int], prefix: int) -> OperationCosts:
    """Translate backend instruction forms into MIR's semantic vocabulary."""
    return OperationCosts(
        add=costs["alu_rr"],
        multiply=costs["mul_r16"],
        divide=costs["div_r16"],
        shift=costs["shift_ri"],
        address=costs["lea"],
        load=costs["mov_rm"],
        store=costs["mov_mr"],
        memory_update=costs["alu_mr"],
        branch=costs["jcc"],
        prefix=prefix,
        move=costs["mov_rr"],
        call=costs["call_far"],
        return_=costs["ret_far"],
        float_add=costs["x87_add"],
        float_multiply=costs["x87_mul"],
        float_divide=costs["x87_div"],
        float_load=costs["x87_load"],
        float_store=costs["x87_store"],
        extend=costs["movzx"],
    )


def _address_forms(costs: OperationCosts, prefix: int) -> tuple[AddressForm, ...]:
    """Native medium-model addressing, then the legal secondary 67h form."""
    return (
        AddressForm(2, frozenset({1})),
        AddressForm(
            4,
            frozenset({1, 2, 4, 8}),
            extra_bytes=1,
            use_cost=prefix,
            extension_cost=costs.extend,
            secondary=True,
        ),
    )


def _profile(name: str) -> Profile:
    if name == "386":
        costs = dict(_I386_COSTS)
        operations = _operation_costs(costs, 0)
        return Profile(
            name,
            1,
            True,
            0,
            0,
            operations=operations,
            _costs=tuple(costs.items()),
            _latencies=tuple(costs.items()),
            address_forms=_address_forms(operations, 0),
        )
    at = timings.ARCHS.index(name)
    costs = {operation: values[at] for operation, values in timings.COST.items()}
    operations = _operation_costs(costs, timings.PREFIX[at])
    return Profile(
        name,
        timings.ISSUE[at],
        bool(timings.INORDER[at]),
        timings.PREFIX[at],
        timings.PARTIAL_STALL[at],
        pentium_pairing=name == "P5",
        operations=operations,
        _costs=tuple(costs.items()),
        _latencies=tuple((operation, values[at]) for operation, values in timings.LATENCY.items()),
        address_forms=_address_forms(operations, timings.PREFIX[at]),
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
