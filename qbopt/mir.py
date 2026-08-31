"""
One body as values rather than as registers.

ir.py is still machine code: one node per instruction, and a register is a
place. This is the same body with every value named once and never written
again, which is what every transform worth doing needs -- common
subexpressions are equal values, a dead store is a value nothing reads, and
allocating registers is a question you can only ask once you have stopped
answering it in advance.

What becomes a value, and what does not:

**The six value registers do** -- ax, bx, cx, dx, si, di, taken at their
32-bit root the way ir.ROOT already folds them. Rooting is not a
simplification here, it is the correctness rule: BC writes `mov ax,..` into
the low half of eax and this pass has widened the other half, so a 16-bit
write is a read-modify-write of the root and has to read the old value to
define the new one. ir.ROOT's own docstring is the argument.

**sp, bp and the segment registers do not.** They are not values, they are
where values live: `[bp-18h]` means what it means because bp is the frame,
and promoting it would dissolve every local. They stay physical, and an
instruction that writes one is a barrier to anything that assumed otherwise.

**The flags are a value**, produced by a compare or an arithmetic op and
consumed by the branch that reads them. That is what makes BC's own 32-bit
arithmetic legible: `add` then `adc` is not two instructions that happen to
be adjacent, it is a carry flowing from one to the other, and once it is an
edge in a graph a later pass can fold the pair without pattern-matching
their addresses.

**A memory reference keeps its Addr** rather than becoming a computed
address. module.may_alias() and memory.py's own rules are the entire alias
story this pass has, and they are written against Addr; throwing that away
for a prettier representation would cost the one analysis that already
works. What a MemRef adds is the SSA values of the registers the address is
read through, so the dependence on `bx` in `es:[bx]` is an edge rather than
a name that renaming would have quietly invalidated.

**A barrier reads and writes everything.** ir.py's own contract says its
operands are pinned and nothing may be reordered across it; here that falls
out of giving it every tracked register as both a use and a def, so no value
crosses it and the allocator has no freedom to move one.

Nothing is optimised here and nothing is lowered. Every Op keeps the ir.Node
it came from, so a lowering that applies no transform is that node's own
bytes -- the same discipline that makes ir.emit() verbatim, and the same
reason it can be trusted before anything is built on top of it.
"""

from dataclasses import field
from dataclasses import dataclass

from iced_x86 import Register
from iced_x86 import Register_

from qbopt import ir
from qbopt import loops
from qbopt import module
from qbopt import runtime
from qbopt.module import Addr
from qbopt.blocks import Block
from qbopt.module import Module

# The registers that become values. Rooted, so a write to ax and a write to
# eax are the same variable -- see this module's own docstring.
TRACKED: tuple[Register_, ...] = (
    Register.EAX,
    Register.EBX,
    Register.ECX,
    Register.EDX,
    Register.ESI,
    Register.EDI,
)

# Not values: the frame, the stack, and which segment an access goes through.
PHYSICAL = frozenset(
    {
        Register.SP,
        Register.ESP,
        Register.BP,
        Register.EBP,
        Register.DS,
        Register.ES,
        Register.SS,
        Register.CS,
    }
)

NAMES = {
    Register.EAX: "eax",
    Register.EBX: "ebx",
    Register.ECX: "ecx",
    Register.EDX: "edx",
    Register.ESI: "esi",
    Register.EDI: "edi",
}

# The flags, as one variable. x86 writes them in groups and BC reads them in
# groups; splitting them per-flag would model a precision no consumer here
# has ever needed, and flags.py already answers the per-flag question where
# it matters.
FLAGS = Register.NONE

# runtime.py names a register as its own small enum, iced as an int. One
# table rather than a string round trip, so a name that stops matching shows
# up here instead of a contract silently preserving nothing.
FROM_CONTRACT = {
    runtime.Reg.AX: Register.EAX,
    runtime.Reg.BX: Register.EBX,
    runtime.Reg.CX: Register.ECX,
    runtime.Reg.DX: Register.EDX,
    runtime.Reg.SI: Register.ESI,
    runtime.Reg.DI: Register.EDI,
    runtime.Reg.FLAGS: FLAGS,
}


@dataclass(frozen=True, slots=True)
class Value:
    """One SSA variable: defined by exactly one Op or Phi, in one place.

    Deliberately says nothing about where it lives. A value used to be named
    after the machine register BC kept it in -- `eax#155` -- which made the
    raise/lower round trip checkable and then blocked everything after it:
    two computations cannot be compared by what they compute while their
    names contain where they landed, and a 32-bit root has no way to say
    "the low half of that". docs/variables.md has the measurement.

    Where BC kept it is still known, and is a fact about lowering rather
    than about the value: MirBody.origin holds it. Reading that map from an
    analysis is a choice with a reason, not something that happens by
    accident because the register was sitting on the value.
    """

    id: int
    at: int  # the instruction that defined it, or the block for a phi
    flags: bool = False  # the flags variable, which is not data and holds none

    def __repr__(self) -> str:
        return f"{'f' if self.flags else 'v'}{self.id}"


@dataclass(frozen=True, slots=True)
class MemRef:
    """A memory operand, with the values its own address depends on.

    `addr` is what the alias rules read. `base` and `segment` are the SSA
    values of the registers that address is reached through, so renaming
    cannot silently change which bytes it names -- an `es:[bx]` whose bx was
    just recomputed is a different reference, and the edge says so.
    """

    addr: Addr | None  # None where nothing can name it -- aliases everything
    width: int
    base: Value | None = None
    segment: Value | None = None


@dataclass(frozen=True, slots=True)
class Op:
    """One instruction, as values in and values out."""

    at: int
    op: ir.Operation
    name: str
    defines: tuple[Value, ...]
    uses: tuple[Value, ...]
    loads: tuple[MemRef, ...] = ()
    stores: tuple[MemRef, ...] = ()
    node: ir.Node | None = None  # what it came from, so lowering can be verbatim

    @property
    def barrier(self) -> bool:
        return self.op is ir.Operation.BARRIER


@dataclass(frozen=True, slots=True)
class Phi:
    """Where two definitions of one register meet."""

    result: Value
    incoming: dict[int, Value] = field(default_factory=dict)  # predecessor block -> value


@dataclass(frozen=True, slots=True)
class MirBlock:
    at: int
    phis: tuple[Phi, ...]
    ops: tuple[Op, ...]
    succ: tuple[int, ...]


@dataclass(frozen=True, slots=True)
class MirBody:
    entry: int
    blocks: tuple[MirBlock, ...]
    origin: dict[Value, Register_] = field(default_factory=dict)  # where BC kept each value

    def block(self, at: int) -> MirBlock | None:
        return next((one for one in self.blocks if one.at == at), None)

    @property
    def values(self) -> tuple[Value, ...]:
        return tuple(
            value
            for block in self.blocks
            for value in ([phi.result for phi in block.phis] + [v for op in block.ops for v in op.defines])
        )


def _call_touches(name: str | None) -> tuple[frozenset[Register_], frozenset[Register_]] | None:
    """What a call really disturbs, where runtime.py has established it.

    ir.Effects answers "any register" for every call, which is the right
    answer for a module that knows nothing about the callee. Here it costs
    real precision: it gives si a fresh value across a routine that
    provably preserves it, so two accesses through the same si stop looking
    like the same address. runtime.py read the QuickBASIC 4.5 source for
    exactly this, and a routine with no entry there still comes back
    worst-case, so nothing is assumed by using it.
    """
    routine = runtime.contract(name)
    if not routine.established or runtime.barrier(routine):
        return None
    kept = {FROM_CONTRACT[one] for one in runtime.preserves(routine) if one in FROM_CONTRACT}
    disturbed = frozenset(one for one in TRACKED if one not in kept) | {FLAGS}
    return disturbed, disturbed


def _touched(node: ir.Node, calls: dict[int, str] | None = None) -> tuple[frozenset[Register_], frozenset[Register_]]:
    """(defines, uses) as tracked variables, flags included as FLAGS.

    Reads ir.Effects rather than ir.Semantics, deliberately: Effects is
    iced's own conservative answer and is already rooted, which is exactly
    what an SSA variable has to be. A None there means "any register" -- a
    call, an interrupt, a barrier -- and becomes every tracked variable on
    both sides, which is what pins a barrier in place.
    """
    if calls is not None and isinstance(node, ir.Call) and (known := _call_touches(calls.get(node.insn.at))):
        return known

    effects = node.effects
    defines = set(TRACKED) if effects.defs is None else {r for r in effects.defs if r in TRACKED}
    uses = set(TRACKED) if effects.uses is None else {r for r in effects.uses if r in TRACKED}
    if effects.defs is None or effects.flags_written:
        defines.add(FLAGS)
    if effects.uses is None or effects.flags_read:
        uses.add(FLAGS)
    return frozenset(defines), frozenset(uses)


class _Namer:
    """Fresh values, and which one each variable currently holds."""

    def __init__(self) -> None:
        self.next = 0
        self.stack: dict[Register_, list[Value]] = {}
        # Where BC had each value. Kept beside the values rather than on
        # them, so lowering and regalloc's identity baseline can ask and
        # nothing else picks it up for free.
        self.origin: dict[Value, Register_] = {}

    def fresh(self, of: Register_, at: int) -> Value:
        self.next += 1
        made = Value(self.next, at, of is FLAGS)
        self.origin[made] = of
        return made

    def current(self, of: Register_, at: int) -> Value:
        """The value in scope, inventing one where nothing has defined it yet.

        A body reads ax before writing it whenever BC passes something in
        through a register, and an entry value is the honest name for that
        -- it is defined by the caller, so its own `at` is the body's entry.
        """
        held = self.stack.setdefault(of, [])
        if not held:
            held.append(self.fresh(of, at))
        return held[-1]


def _memrefs(cells: tuple[ir.Mem, ...], namer: _Namer, at: int) -> tuple[MemRef, ...]:
    """ir.Mem cells, with the values their own address registers hold now."""
    out = []
    for cell in cells:
        addr = cell.addr
        base = segment = None
        if addr is not None:
            root = ir.ROOT.get(addr.base, addr.base)
            if root in TRACKED:
                base = namer.current(root, at)
            if addr.segment != Register.NONE:
                segment = None  # a segment register is physical, never a value
        out.append(MemRef(addr, cell.width, base, segment))
    return tuple(out)


def _placed(
    blocks: list[Block],
    nodes: dict[int, ir.Node],
    entry: int | None = None,
    calls: dict[int, str] | None = None,
) -> dict[int, frozenset[Register_]]:
    """Which variables need a phi in which block.

    The iterated frontier: a phi is itself a definition, so putting one in
    can force another further down. Runs to a fixed point per variable
    rather than once, which is the whole difference between this and a
    single frontier lookup.
    """
    frontier = loops.frontiers(blocks, entry)
    defines: dict[Register_, set[int]] = {}
    for block in blocks:
        for insn in block.insns:
            node = nodes.get(insn.at)
            if node is None:
                continue
            for one in _touched(node, calls)[0]:
                defines.setdefault(one, set()).add(block.at)

    needed: dict[int, set[Register_]] = {block.at: set() for block in blocks}
    for variable, where in defines.items():
        pending = list(where)
        seen: set[int] = set()
        while pending:
            at = pending.pop()
            for join in frontier.get(at, frozenset()):
                if join in seen:
                    continue
                seen.add(join)
                needed[join].add(variable)
                pending.append(join)
    return {at: frozenset(what) for at, what in needed.items()}


def raise_body(
    blocks: list[Block],
    nodes: dict[int, ir.Node],
    entry: int | None = None,
    calls: dict[int, str] | None = None,
) -> MirBody | str:
    """One body's blocks, in SSA, or why they could not be.

    `nodes` is keyed on each node's own span start, not on an instruction
    address: a Restore covers three instructions and a Data table is not an
    instruction at all, so span() is the only key every Node kind has. The
    instructions a multi-instruction idiom covers past its first are simply
    absent from the map and skipped, which is right -- the idiom's own
    Effects already account for all of them, and walking them again would
    define the same value twice.

    Standard construction: place phis on the iterated dominance frontier of
    every variable's definitions, then rename down the dominator tree with a
    stack per variable. The only thing here that is not textbook is what
    counts as a variable, and that is this module's own docstring.
    """
    if not blocks:
        return "no blocks to raise"
    start = entry if entry is not None else blocks[0].at

    # One body, and only one: a procedure is reached by a call, which is not
    # a CFG edge, so handing this a whole module's blocks would leave another
    # body's blocks sitting in the list with no path from this entry. They
    # would still have predecessors -- their own -- so a phi would be placed
    # in them that the dominator-tree walk never reaches to fill, and the
    # value arriving along that edge would vanish. Dropped here rather than
    # tolerated, so the caller's unit is the same as this function's.
    everything = {block.at: block for block in blocks}
    if start not in everything:
        return f"the entry {start:#06x} is not one of these blocks"
    reachable = {start}
    pending = [start]
    while pending:
        here = everything[pending.pop()]
        for successor in here.succ:
            if successor in everything and successor not in reachable:
                reachable.add(successor)
                pending.append(successor)
    blocks = [block for block in blocks if block.at in reachable]

    if loops.irreducible(blocks, start):
        return "the body's control flow is irreducible, so it has no dominator tree"

    by_at = {block.at: block for block in blocks}
    idom = loops.immediate_dominators(blocks, start)
    children: dict[int, list[int]] = {block.at: [] for block in blocks}
    for block in blocks:
        parent = idom.get(block.at)
        if parent is not None:
            children[parent].append(block.at)

    needed = _placed(blocks, nodes, start, calls)
    namer = _Namer()
    phis: dict[int, dict[Register_, Phi]] = {block.at: {} for block in blocks}
    ops: dict[int, list[Op]] = {block.at: [] for block in blocks}

    # Every phi exists before any renaming starts. Creating one on entry to
    # its own block instead is a real bug and a quiet one: a predecessor
    # renamed earlier in the walk finds nothing to fill, so the phi silently
    # loses that edge and the value arriving along it disappears. verify()
    # catches it; nothing else would.
    for block in blocks:
        for variable in sorted(needed[block.at], key=lambda one: (one is not FLAGS, one)):
            phis[block.at][variable] = Phi(namer.fresh(variable, block.at), {})

    def rename(at: int) -> None:
        block = by_at[at]
        pushed: list[Register_] = []

        for variable, phi in phis[at].items():
            namer.stack.setdefault(variable, []).append(phi.result)
            pushed.append(variable)

        for insn in block.insns:
            node = nodes.get(insn.at)
            if node is None:
                continue
            defines, uses = _touched(node, calls)
            used = tuple(namer.current(one, start) for one in sorted(uses, key=lambda o: (o is not FLAGS, o)))
            loads = _memrefs(node.effects.loads, namer, start)
            stores = _memrefs(node.effects.stores, namer, start)
            made = []
            for one in sorted(defines, key=lambda o: (o is not FLAGS, o)):
                value = namer.fresh(one, insn.at)
                namer.stack.setdefault(one, []).append(value)
                pushed.append(one)
                made.append(value)
            ops[at].append(
                Op(
                    insn.at,
                    node.semantics.op,
                    node.semantics.name or "",
                    tuple(made),
                    used,
                    loads,
                    stores,
                    node,
                )
            )

        for successor in block.succ:
            if successor not in phis:
                continue
            for variable, phi in phis[successor].items():
                phi.incoming[at] = namer.current(variable, start)

        for child in sorted(children[at]):
            rename(child)

        for variable in reversed(pushed):
            namer.stack[variable].pop()

    rename(start)
    return MirBody(
        start,
        tuple(
            MirBlock(
                block.at,
                tuple(phis[block.at].values()),
                tuple(ops[block.at]),
                tuple(one for one in block.succ if one in reachable),
            )
            for block in blocks
        ),
        dict(namer.origin),
    )


def verify(body: MirBody, blocks: list[Block]) -> list[str]:
    """Everything SSA promises, checked. Empty means the form holds.

    Three properties, and each one is load-bearing for a different consumer:
    a value defined twice makes "equal values are the same computation"
    false, so common-subexpression elimination would merge things that are
    not equal; a use its definition does not dominate reads a register on
    some path where nothing wrote it, so any motion built on the edge is
    wrong; and a phi missing an argument means a predecessor's value simply
    vanishes at the join. Checked rather than assumed because construction
    is the one place a renaming bug hides silently -- the graph still looks
    well-formed, it just describes a different program.
    """
    doms = loops.dominators(blocks, body.entry)
    problems: list[str] = []

    defined_at: dict[Value, int] = {}
    for block in body.blocks:
        for phi in block.phis:
            if phi.result in defined_at:
                problems.append(f"{phi.result} defined twice")
            defined_at[phi.result] = block.at
        for op in block.ops:
            for value in op.defines:
                if value in defined_at:
                    problems.append(f"{value} defined twice, at {op.at:#06x}")
                defined_at[value] = block.at

    preds = loops.predecessors(blocks)
    for block in body.blocks:
        for phi in block.phis:
            want = {one for one in preds[block.at] if body.block(one) is not None}
            if set(phi.incoming) != want:
                problems.append(
                    f"{phi.result} at {block.at:#06x} has {sorted(map(hex, phi.incoming))},"
                    f" its predecessors are {sorted(map(hex, want))}"
                )
            for came_from, value in phi.incoming.items():
                # a phi argument has to be in scope where the edge leaves,
                # not where the phi sits -- that is the whole point of one
                where = defined_at.get(value)
                if where is not None and where not in doms.get(came_from, frozenset()):
                    problems.append(f"{phi.result} takes {value} from {came_from:#06x}, which it does not reach")
        for op in block.ops:
            for value in op.uses:
                where = defined_at.get(value)
                if where is None:
                    continue  # defined by the caller, in scope everywhere
                if where not in doms.get(block.at, frozenset()):
                    problems.append(f"{op.at:#06x} uses {value}, defined in {where:#06x}, which does not dominate it")
    return problems


def lower(body: MirBody) -> tuple[ir.Node, ...]:
    """The nodes this body is made of, in address order.

    With nothing transformed this is exactly what was raised, so emitting it
    gives back the bytes it came from. That is the whole point of keeping an
    origin on every Op: the identity case is checkable before any transform
    exists, which is the only moment the machinery can be trusted for free.
    Once something does change a body, this is where a real instruction
    selector goes, and this round trip is what it will be measured against.

    A phi emits nothing. It is not an instruction and never was -- it names
    where two definitions of a register met, which BC's own code said by
    writing the same register on both paths. Only a lowering that has
    actually split those definitions into different registers has to put
    anything back, and nothing here does yet.
    """
    return tuple(op.node for block in body.blocks for op in block.ops if op.node is not None)


def relowered(found: Module, body: MirBody) -> bytes:
    """This body's own bytes, rebuilt from the graph."""
    return ir.emit(found, lower(body))


def same_bytes(one: MemRef, other: MemRef) -> bool:
    """Whether two references certainly name the same bytes.

    Keyed on the base *value*, never on which register holds it. That is
    the whole difference between this and module.may_alias: an Addr says
    `[si+6]`, and the moment anything reallocates registers that name is
    about a register which may now hold something else, while the value it
    stood for is still the same value. Two references agree here because
    the same computation produced their offset, which no allocation can
    change.

    Certainly, not possibly -- this answers the forwarding question ("is
    this the load I already did"), and its negation is not a disjointness
    proof. `may_alias` still answers that one.
    """
    if one.addr is None or other.addr is None:
        return False  # nothing this can name is never known to be anything
    if one.width != other.width or one.base != other.base or one.segment != other.segment:
        return False
    return one.addr == other.addr


def overlapping(one: MemRef, other: MemRef, dgroup: frozenset[int]) -> bool:
    """Whether a write through `other` could land on `one`.

    module.may_alias for the symbolic part, and the base value for the rest.
    Where both name the same base value their displacements settle it by
    arithmetic, exactly as two bare statics do -- and soundly for the same
    reason memory.aliases() gives, except that this holds it by value
    identity rather than by the caller having promised the register was not
    written in between.
    """
    if one.addr is None or other.addr is None:
        return True
    if one.base is not None and one.base == other.base and one.addr.space is other.addr.space:
        return one.addr.disp < other.addr.disp + other.width and other.addr.disp < one.addr.disp + one.width
    return module.may_alias(one.addr, other.addr, dgroup, one.width, other.width)


def bodies(found: Module, blocks: list[Block]) -> list[tuple[str, MirBody]]:
    """Every body in the module, raised, labelled, and skipping what will not.

    One place rather than three: dump.py, the measurement scripts and now
    rewrite.py all need the same walk, and the part worth not rewriting
    twice is the block-to-body assignment -- a procedure is reached by a
    call, which is not a CFG edge, so raise_body() has to be handed one
    body's blocks and no others.
    """
    result = ir.decode_module(found)
    if isinstance(result, str):
        return []
    nodes = {ir.span(node)[0]: node for body in result for node in body.nodes}
    out: list[tuple[str, MirBody]] = []
    for body in result:
        mine = [one for one in blocks if any(lo <= one.at < hi for lo, hi in body.body.ranges)]
        if not mine:
            continue
        built = raise_body(mine, nodes, body.body.seed, found.calls)
        if not isinstance(built, str):
            out.append((f"{body.body.kind} {body.body.name or '(main)'}", built))
    return out
