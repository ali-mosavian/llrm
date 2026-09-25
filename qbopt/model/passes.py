"""
What a pass is: MIR in, MIR out, and nothing else.

`transform(body) -> body` is the whole contract. Anything a pass needs to
know about the module it is compiling is given when the pass is made, not
when it runs, so the signature cannot quietly grow a way to ask the machine
a question -- which is how `hoisted(body, dgroup, calls, bounds)` came to
take the module's layout and end up choosing registers.

Rule 5 is the reason this exists. Selected instruction semantics now begin
on ``lir.Insn.what``. Source placement is captured by the raise in an external
``AllocationHints`` table; optimization and shared-analysis passes cannot read
or propagate it. Public ``MirBody`` has no placement fields; the raise's
temporary machine view is private and is consumed before a pass sees the body.

  Op.absorbed
               opaque identities of the raise-time occurrences this stands
               for. Passes may preserve or combine the identities but cannot
               resolve them to object bytes; that happens during lowering.

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
class AddressForm:
    """One legal indexed-address family and its costs above the native form.

    This is deliberately machine-neutral.  MIR may know that an address can
    use a four-byte index with scales 1/2/4/8, and what widening/setup and
    per-use costs that choice carries; it never learns that x86 spells the
    choice with an address-size prefix or which physical registers encode it.
    ``extra_bytes`` is separate from execution cost so a zero-cycle prefix
    still participates in code-size decisions without being invented as
    processor latency. ``secondary`` means a legal non-native form which is
    considered before spill or recomputation; it is not a last-resort form.
    """

    index_width: int
    scales: frozenset[int]
    extra_bytes: int = 0
    use_cost: int = 0
    extension_cost: int = 0
    secondary: bool = False
    # How many distinct bases one index can pair with at once; None is any.
    partners: int | None = None
    # Compatibility for existing Python callers which used the old, easily
    # misread name.  Keep both views identical; production code says
    # ``secondary`` so its place in the selection order is explicit.
    fallback: bool | None = None

    def __post_init__(self) -> None:
        if self.fallback is not None and self.secondary and self.fallback != self.secondary:
            raise ValueError("an address form cannot disagree about whether it is secondary")
        selected = self.secondary if self.fallback is None else self.fallback
        object.__setattr__(self, "secondary", selected)
        object.__setattr__(self, "fallback", selected)

    def before_spill(self, costs: "OperationCosts") -> bool:
        """Whether this form is cheap enough to try before a frame spill.

        The direct case compares one extension plus one prefixed use with a
        source reload.  A one-cycle extension may also amortize across the
        native alternative it removes: materializing the scale, forming the
        address, moving it into place, and storing one displaced live value.
        This is deliberately expressed only in semantic costs; neither MIR
        nor the policy learns how the target spells the form.
        """
        direct = self.extension_cost + self.use_cost <= costs.load
        amortized = (
            self.extension_cost <= costs.move
            and self.extension_cost + self.use_cost <= costs.shift + costs.address + costs.move + costs.store
        )
        return not self.secondary or direct or amortized


@dataclass(frozen=True, slots=True)
class OperationCosts:
    """Machine-neutral costs a MIR profitability decision may compare.

    The backend translates its instruction-form table at the boundary. MIR
    sees only semantic work -- arithmetic, address formation and memory
    traffic -- and therefore cannot name an opcode, register or encoding.
    Unit defaults preserve callers that have not selected a target.
    """

    add: int = 1
    multiply: int = 1
    divide: int = 1
    shift: int = 1
    address: int = 1
    load: int = 1
    store: int = 1
    memory_update: int = 1
    branch: int = 1
    prefix: int = 0
    move: int = 1
    call: int = 1
    return_: int = 1
    float_add: int = 1
    float_multiply: int = 1
    float_divide: int = 1
    float_load: int = 1
    float_store: int = 1
    extend: int = 1
    fill: int = 1
    fill_cell: int = 1


DEFAULT_MAX_UNROLL_ITERATIONS = 16
DEFAULT_MAX_UNROLLED_OPERATIONS = 200


@dataclass(frozen=True, slots=True)
class Options:
    """What GCC's command line says about optimization, as one value.

    `-O` picks the defaults, `--param` the copy budgets, `-f` each pass. They
    are independent of the CPU, as in GCC. `grows=False` is -Os's
    `UL_NO_GROWTH` (tree-ssa-loop-ivcanon.cc): a copy is taken only when it
    is no larger.
    """

    level: str = "O2"
    # --param max-completely-peel-times
    max_unroll_iterations: int = DEFAULT_MAX_UNROLL_ITERATIONS
    # --param max-completely-peeled-insns
    max_unrolled_operations: int = DEFAULT_MAX_UNROLLED_OPERATIONS
    grows: bool = True
    lcssa: bool = True
    floatloop: bool = True
    fold: bool = True
    decide: bool = True
    dead: bool = True
    hoist: bool = True
    forward: bool = True
    drop_loads: bool = True
    drop_stores: bool = True
    promote: bool = True
    strength: bool = True
    unroll: bool = True
    peel: bool = True
    fill: bool = True
    unswitch: bool = False


LEVELS = {"O2": Options(), "Os": Options("Os", grows=False)}
O2 = LEVELS["O2"]


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
    # Complete legal indexed-address families. ``index_scales`` remains the
    # preferred/native compatibility view; fallback forms are not implicitly
    # free merely because the emitter can encode them.
    address_forms: tuple[AddressForm, ...] = ()
    # Semantic work only. The profile boundary translates instruction forms
    # once; no MIR pass can recover an opcode or register from these prices.
    costs: OperationCosts = OperationCosts()
    options: Options = Options()

    @property
    def named(self) -> dict:
        return self.calls or {}
