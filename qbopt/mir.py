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

**A barrier's operands are whatever its encoding touches.** ir.py's own
contract says they are pinned and nothing may be reordered across one. That
comes from Effects, not from a blanket rule: where iced cannot say what an
instruction touches -- an interrupt, an indirect call -- Effects is None and
every tracked register becomes both a use and a def, which pins everything.
Where iced can say, the answer is precise, and `movsw` is the shape that
proves the difference: it steps si and di through memory and touches no
other register, so a value in ax may live across it and the allocator is
free to keep it there. The pinning that matters is still absolute, because
it is read off the same Effects by ir.pinned().

Memory is the part that never gets the precise treatment. A barrier's
addresses are its own -- `movsw` writes through es:di, which nothing here
can disambiguate -- so avail.py drops every held cell at one, on the flag
itself rather than on any register set.

Nothing is optimised here and nothing is lowered. Every Op keeps the ir.Node
it came from, so a lowering that applies no transform is that node's own
bytes -- the same discipline that makes ir.emit() verbatim, and the same
reason it can be trusted before anything is built on top of it.
"""

from enum import StrEnum
from dataclasses import field
from dataclasses import replace
import itertools
from dataclasses import dataclass

from iced_x86 import Register
from iced_x86 import Register_
from iced_x86 import RegisterExt

from qbopt import ir
from qbopt import loops
from qbopt import stack
from qbopt import module
from qbopt import runtime
from qbopt.module import Addr
from qbopt.blocks import Block
from qbopt.module import Space
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
# Which root a restore reads and which it writes the high half into.
# ir.FIXUP's own pair numbering: 0 is `push eax / pop ax / pop dx`, 1 is
# `push ecx / pop cx / pop bx`.
RESTORE_PAIR = {
    0: (Register.EAX, Register.EDX),
    1: (Register.ECX, Register.EBX),
}

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


class Synth(StrEnum):
    """Operations no single machine instruction computes.

    ir.Operation is a vocabulary for what one instruction does. These are
    for what a value IS in terms of other values, which is a different
    question and the one a simplifier asks. They exist because a variable
    is 32 bits wide and BC's whole output is 32-bit work written as 16-bit
    halves -- see docs/variables.md.
    """

    # dests[0] <- sources[0] with its low 16 bits replaced by the HIGH 16
    # of sources[1]. calls.py's restore idiom, exactly: `pop dx` after
    # `push eax` puts eax's high half into dx and leaves edx's own high
    # half alone.
    HALF_TO_LOW = "half.tolow"

    # dests[0] <- low16(sources[0]) << 16 | low16(sources[1]). The rejoin:
    # `push dx / push ax / pop eax` reads the two halves back as one dword.
    CONCAT_LOW = "concat.low"


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
class Held:
    """A value, at the width this operation uses it.

    MIR's answer to ir.Reg. Which register holds it is the allocator's, and
    an operation that named one was a pass doing register allocation.
    """

    value: "Value"
    width: int


@dataclass(frozen=True, slots=True)
class Const:
    """A literal, signed as the operation means it."""

    n: int
    width: int


@dataclass(frozen=True, slots=True)
class Cell:
    """A memory cell -- the same MemRef the op's loads and stores name."""

    ref: "MemRef"


@dataclass(frozen=True, slots=True)
class Opaque:
    """An operand MIR has no form for.

    79 operations of the 2,343 in the corpus, all of them the x87 stack,
    whose registers rotate under push and pop and so are not a location the
    way a register is. Everything else -- 96.6% -- is a value, a constant or
    a cell. An Opaque operand is why fpstack.py exists and is the boundary
    it works at; nothing else may look inside one.
    """

    what: object


type Arg = Held | Const | Cell | Opaque


class Kind(StrEnum):
    """What an operation computes, in MIR's own terms.

    Three-address and nothing else: `c := a op b`. No mnemonic, no flags,
    no register pair, no stack. What one of BC's instructions *is* -- an
    `adc` that is the top half of a wider add, a `cmp` and the `jle` that
    reads its flags, a run of pushes that is a call's arguments -- is the
    raise's answer, and by the time a pass sees it there is one operation
    here saying so.

    ir.Operation is the vocabulary of one x86 instruction and says so in
    its own docstring: BINARY means "sources[0] IS dests[0]". That is the
    machine's shape, and this exists so no pass has to know it.
    """

    # c := a op b
    ADD = "add"
    SUB = "sub"
    MUL = "mul"
    DIV = "div"
    REM = "rem"
    AND = "and"
    OR = "or"
    XOR = "xor"
    SHL = "shl"
    SHR = "shr"
    SAR = "sar"
    NEG = "neg"
    NOT = "not"

    # c := a <op> b -- a value, never a flag
    LT = "lt"
    LE = "le"
    GT = "gt"
    GE = "ge"
    EQ = "eq"
    NE = "ne"
    BELOW = "below"  # the unsigned pair, kept apart because the width is
    BELOW_EQ = "beloweq"  # not enough to tell which comparison was meant
    ABOVE = "above"
    ABOVE_EQ = "aboveeq"

    # movement and shape
    COPY = "copy"  # c := a
    LOAD = "load"  # c := [m]
    STORE = "store"  # [m] := a
    CONVERT = "convert"  # c := a, at another width or signedness
    ADDRESS = "address"  # c := the number an address is

    # control
    CALL = "call"
    BRANCH = "branch"  # on a value, to `target` or the next block
    JUMP = "jump"
    RETURN = "return"
    ESCAPE = "escape"  # leaves the body somewhere it does not name

    # floating point, until the x87 stack is resolved at the raise
    FADD = "fadd"
    FSUB = "fsub"
    FMUL = "fmul"
    FDIV = "fdiv"
    FNEG = "fneg"
    FLOAD = "fload"
    FSTORE = "fstore"
    FCOMPARE = "fcompare"

    # what has no MIR form yet, each with the step that removes it
    ARG = "arg"  # a call argument still written as a push -- step 3
    RESULT = "result"  # and its pop
    JOIN = "join"  # two halves of a wide value made one -- step 4
    OPAQUE = "opaque"  # nothing is claimed; see ir.Operation.BARRIER
    NOTHING = "nothing"


# One x86 instruction to what it computes. The mnemonic is consulted only
# here: BINARY and UNARY do not say which operation they are, so the raise
# is where that is decided and after it nothing needs to ask.
_BY_NAME: dict[str, Kind] = {
    "add": Kind.ADD, "adc": Kind.ADD, "inc": Kind.ADD,
    "sub": Kind.SUB, "sbb": Kind.SUB, "dec": Kind.SUB, "cmp": Kind.SUB,
    "and": Kind.AND, "test": Kind.AND,
    "or": Kind.OR,
    "xor": Kind.XOR,
    "not": Kind.NOT,
    "neg": Kind.NEG,
    "shl": Kind.SHL, "sal": Kind.SHL,
    "shr": Kind.SHR,
    "sar": Kind.SAR,
    "imul": Kind.MUL, "mul": Kind.MUL,
    "idiv": Kind.DIV, "div": Kind.DIV,
    "fadd": Kind.FADD, "faddp": Kind.FADD,
    "fsub": Kind.FSUB, "fsubp": Kind.FSUB, "fsubr": Kind.FSUB, "fsubrp": Kind.FSUB,
    "fmul": Kind.FMUL, "fmulp": Kind.FMUL,
    "fdiv": Kind.FDIV, "fdivp": Kind.FDIV, "fdivr": Kind.FDIV, "fdivrp": Kind.FDIV,
    "fchs": Kind.FNEG, "fabs": Kind.FNEG,
    "fcom": Kind.FCOMPARE, "fcomp": Kind.FCOMPARE, "fcompp": Kind.FCOMPARE,
    "ftst": Kind.FCOMPARE,
}

# A conditional branch's mnemonic is the comparison it reads. The flags in
# between are the machine's way of getting one to the other and are not a
# value: step 2 folds the comparison into the branch and they disappear.
_BY_BRANCH: dict[str, Kind] = {
    "jl": Kind.LT, "jnge": Kind.LT,
    "jle": Kind.LE, "jng": Kind.LE,
    "jg": Kind.GT, "jnle": Kind.GT,
    "jge": Kind.GE, "jnl": Kind.GE,
    "je": Kind.EQ, "jz": Kind.EQ,
    "jne": Kind.NE, "jnz": Kind.NE,
    "jb": Kind.BELOW, "jc": Kind.BELOW, "jnae": Kind.BELOW,
    "jbe": Kind.BELOW_EQ, "jna": Kind.BELOW_EQ,
    "ja": Kind.ABOVE, "jnbe": Kind.ABOVE,
    "jae": Kind.ABOVE_EQ, "jnb": Kind.ABOVE_EQ, "jnc": Kind.ABOVE_EQ,
}


def _kind_of(what: "ir.Semantics", args, results) -> Kind:
    """What one node computes, as MIR says it.

    The mnemonic is read here and nowhere else. `args` and `results` decide
    move against load against store, because that is a question about the
    operands and not about the instruction.
    """
    op, name = what.op, (what.name or "")
    if op is ir.Operation.BINARY or op is ir.Operation.UNARY:
        return _BY_NAME.get(name, Kind.OPAQUE)
    if op is ir.Operation.MOVE:
        if any(isinstance(one, Cell) for one in results):
            return Kind.STORE
        if any(isinstance(one, Cell) for one in args):
            return Kind.LOAD
        return Kind.COPY
    if op is ir.Operation.MULTIPLY:
        return Kind.MUL
    if op is ir.Operation.DIVIDE:
        return Kind.DIV
    if op is ir.Operation.COMPARE:
        return _BY_NAME.get(name, Kind.SUB)
    if op is ir.Operation.EXTEND:
        return Kind.CONVERT
    if op is ir.Operation.ADDRESS:
        return Kind.ADDRESS
    if op is ir.Operation.PUSH:
        return Kind.ARG
    if op is ir.Operation.POP:
        return Kind.RESULT
    if op is ir.Operation.JUMP:
        return Kind.JUMP
    if op is ir.Operation.BRANCH:
        return Kind.BRANCH
    if op is ir.Operation.CALL:
        return Kind.CALL
    if op is ir.Operation.RETURN:
        return Kind.RETURN
    if op is ir.Operation.ESCAPE:
        return Kind.ESCAPE
    if op is ir.Operation.NOTHING:
        return Kind.NOTHING
    if op is ir.Operation.RESTORE:
        return Kind.JOIN
    if op is ir.Operation.FLOAT_LOAD:
        return Kind.FLOAD
    if op is ir.Operation.FLOAT_STORE:
        return Kind.FSTORE
    if op in (ir.Operation.FLOAT_ARITH, ir.Operation.FLOAT_ARITH_POP, ir.Operation.FLOAT_UNARY):
        return _BY_NAME.get(name, Kind.OPAQUE)
    return Kind.OPAQUE


@dataclass(frozen=True, slots=True)
class Op:
    """One instruction, as values in and values out."""

    at: int
    op: ir.Operation | Synth
    name: str
    defines: tuple[Value, ...]
    uses: tuple[Value, ...]
    loads: tuple[MemRef, ...] = ()
    stores: tuple[MemRef, ...] = ()
    node: ir.Node | None = None  # what it came from, so lowering can be verbatim
    # What a transform decided this op should be, overriding the node's own
    # semantics. The node stays rather than being replaced: it is what
    # layout.py asks for the op's original bytes and, more importantly, for
    # the fixup inside them. A widened `add eax,[x]` reads the same
    # relocated address the pair's low half did, and an op with no node has
    # no fixup to find -- that address would come out a bare zero.
    made: ir.Semantics | None = None
    # Which of the original bytes this op stands for, when that is not just
    # its own node's span. A transform that replaces two instructions with
    # one leaves the second's bytes belonging to nothing, and layout.py
    # refuses a body it cannot account for every byte of -- rightly, since
    # that is how it catches data BC put between the instructions. So a
    # replacement says what it replaced.
    covers: tuple[int, int] | None = None
    # What this operation reads and writes, in its own order, as values,
    # constants and cells. This is what `made` was for and what a pass
    # rewriting an operation now says instead -- ir.Semantics over ir.Reg
    # is machine form, and MIR holding it is the whole of rule 5's problem.
    # What this computes, in MIR's own vocabulary. `op` and `name` are the
    # machine's and are on their way out; nothing new may read them.
    kind: Kind = Kind.OPAQUE
    args: tuple[Arg, ...] = ()
    results: tuple[Arg, ...] = ()
    # What those were at the raise, so "did a pass rewrite this" is a
    # question MIR can answer about itself. Lowering needs it: an operation
    # nothing touched is emitted from its original bytes rather than
    # re-encoded, and re-encoding one onto a longer form for the same
    # instruction is how a rebuild grows without anything being optimised.
    # In MIR's own operands, so it says nothing about the machine.
    raised: tuple[tuple[Arg, ...], tuple[Arg, ...]] | None = None
    # Where a branch goes, as a block address. Control flow, not machine
    # form: the blocks are MIR's own and the address is what names one.
    target: int | None = None
    # What this operation is, apart from where it is. Given at the raise and
    # carried through every `replace()`, so a fact established then can live
    # in a side table instead of on the op -- which is the only way those
    # facts survive the hoist moving something: a table keyed by `at` does
    # not. None means an operation a pass invented, which has no past and
    # must say what it computes in MIR's own terms.
    id: int | None = None

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
    # Where a transform has asked for a value to go instead. Empty for a
    # body as raised, and the reason a pass can move anything at all:
    # regalloc.colour() returns the identity assignment unless something
    # pins, and hoisting a load out of a loop is exactly a request for a
    # register the loop does not already use. wholeseg.py colours with
    # these and hands the result to layout.
    pins: dict[Value, Register_] = field(default_factory=dict)

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
    # Clobbering is not reading, and this returned the same set for both.
    # Every routine runtime.py has established takes its arguments on the
    # stack -- cmacros' cProc with parmW, and the print family's own AX is
    # set by the stub before it jumps to B$PRINT, so it is not an input from
    # the caller. Saying a call reads bx made bx's entry value live from the
    # top of the body to the call, across every loop in between, and no
    # register was ever free for anything to be hoisted into.
    #
    # A contract says which registers it reads, and for nearly all of them
    # that is none: cmacros' cProc with parmW/parmD puts arguments on the
    # stack, and the print family's own ax is set by the stub before it
    # jumps to B$PRINT rather than by the caller. Saying a call reads bx
    # made bx's entry value live from the top of the body to the call,
    # across every loop between, and nothing could be hoisted anywhere.
    #
    # The x87 loads are the exception and say so: B$FILD takes a long in
    # dx:ax, B$FIL2 an integer in ax. Leaving those out of the use list is
    # what let dead code elimination delete the moves that set them up.
    if routine.inputs is None:
        # Established for what it clobbers and not for what it reads. Both
        # answers are needed and they are not the same question: B$ENRA and
        # B$EXSA preserve a documented set and their code is not in the
        # tree, so every register is an input until it is.
        return disturbed, frozenset(TRACKED) | {FLAGS}
    reads = {FROM_CONTRACT[one] for one in routine.inputs if one in FROM_CONTRACT}
    return disturbed, frozenset(reads) | {FLAGS}


def _restore_touches(node: ir.Node) -> tuple[frozenset[Register_], frozenset[Register_]] | None:
    """What calls.py's restore idiom really disturbs.

    ir.RESTORE_EFFECTS says it defines both roots, and has to: `pop ax` is a
    partial write, and a layer answering per-register has no way to say that
    the bits written are the ones already there. Here that is sayable, and
    the truth is narrower.

        push eax   [sp] = low16(eax), [sp+2] = high16(eax)
        pop ax     ax = low16(eax)   -- which is what ax already held
        pop dx     dx = high16(eax)  -- edx's own high half untouched

    So eax is not redefined at all. Only the high-half register changes, and
    saying otherwise puts a false definition in the middle of every absorbed
    site -- which ends the live range of the value being restored and is
    exactly what stopped the round-trip churn from being provable.
    """
    if not isinstance(node, ir.Restore):
        return None
    source, into = RESTORE_PAIR[node.pair]
    return frozenset({into}), frozenset({source, into})


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
    if (halves := _restore_touches(node)) is not None:
        return halves

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


# How many bytes each runtime routine pops off before it returns: four per
# long argument. Named here rather than imported from calls.py, which is the
# machine arm and is what M5 retires -- and kept to the routines runtime.py
# has actually read, so an unrecognised call still makes the depth unknown.
CONSUMES = {
    "B$MUI4": 8,
    "B$DVI4": 8,
    "B$RMI4": 8,
    "B$CPI4": 8,
    "FIXMUL": 12,
}


def _consumed(name: str) -> int | None:
    """The bytes this routine takes off the stack, or None if unknown."""
    return CONSUMES.get(name.upper())


def _stack_slot(node: ir.Node, offset: int | None, name: str | None = None) -> tuple[int | None, Addr | None]:
    """Where a push or pop's own cell sits, and where sp is afterwards.

    `offset` is how far sp has moved since the top of this block, so a slot
    is named by a depth rather than an address and only ever compared with
    another slot from the same block. That is enough to link a push to the
    pop that reads it, which is what the round-trip idiom absorption emits
    is made of, and it needs nothing about where sp sits relative to bp.

    A push names the slot it lands in, so sp moves first; a pop names the
    slot it reads, so sp moves after. Anything else that touches sp gives up
    -- None, which is the address that aliases everything, and the honest
    answer once the depth is no longer known.
    """
    if offset is None:
        return None, None
    match node.semantics.op:
        case ir.Operation.PUSH:
            width = node.effects.stores[0].width if node.effects.stores else 0
            if not width:
                return None, None
            offset -= width
            return offset, Addr(Space.STACK, offset)
        case ir.Operation.POP:
            width = node.effects.loads[0].width if node.effects.loads else 0
            if not width:
                return None, None
            return offset + width, Addr(Space.STACK, offset)
        case _:
            # A restore is push/pop/pop and nets to nothing, so the depth
            # survives it.
            if isinstance(node, ir.Restore):
                return offset, None
            # A call pushes a return address and the callee pops both that
            # and its own arguments, so a routine whose arity is known nets
            # to exactly the arguments it consumed and the depth survives.
            # This used to give up at every call, which is what put 831 of
            # the corpus's 923 absorbable calls out of reach: an argument
            # pushed before some other call runs is stranded under it, and
            # its slot is only nameable if the depth crossed that call.
            #
            # Trusting it is the same three claims stack.py's own docstring
            # sets out -- the count, that cleanup is the callee's, and that
            # the call returns to the next byte -- and they rest on the
            # QuickBASIC 4.5 runtime source, not on inference from the
            # bytes. A routine this does not recognise still gives up.
            if offset is not None and name is not None:
                consumed = _consumed(name)
                if consumed is not None:
                    return offset + consumed, None
            found = getattr(node, "insn", None)
            if found is not None and stack.touches_sp(found):
                return None, None
            return offset, None


def _memrefs(cells: tuple[ir.Mem, ...], namer: _Namer, at: int, slot: Addr | None = None) -> tuple[MemRef, ...]:
    """ir.Mem cells, with the values their own address registers hold now."""
    out = []
    for cell in cells:
        addr = slot if cell.addr is None and slot is not None else cell.addr
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


# Which register a root is, at a width. ir.Held and mir.Held both name a
# value and a width, so a register a root cannot reach that way -- the high
# byte of a 16-bit register -- has no MIR form.
_AT_WIDTH: dict[Register_, dict[int, Register_]] = {}
for _one, _root in ir.ROOT.items():
    _size = RegisterExt.size(_one)
    # al and ah are both one byte and both root to eax; the first one wins
    # and the other is the form with no name, which is the point.
    _AT_WIDTH.setdefault(_root, {}).setdefault(_size, _one)


def _operands(
    what: ir.Semantics,
    holds: dict[Register_, "Value"],
    written: dict[Register_, "Value"],
    loads: tuple["MemRef", ...],
    stores: tuple["MemRef", ...],
) -> tuple[tuple[Arg, ...], tuple[Arg, ...]]:
    """One node's operands as MIR's own, in the operation's own order.

    A register operand becomes the value that register holds here, which is
    what the renaming just decided; a cell becomes the MemRef already built
    for the same access, matched in order because that is the order both
    were read in. What is left is the x87 stack, which stays opaque.
    """

    def one(loc: ir.Loc, cells: list["MemRef"], where: dict[Register_, "Value"]) -> Arg:
        if isinstance(loc, ir.Reg):
            root = ir.ROOT.get(loc.register, loc.register)
            value = where.get(root)
            # A high byte -- `ah` against `al` -- is not named by a root and
            # a width, so Held cannot say it. One operation in the corpus.
            if value is None or _AT_WIDTH.get(root, {}).get(loc.width) is not loc.register:
                return Opaque(loc)
            return Held(value, loc.width)
        if isinstance(loc, ir.Imm):
            return Const(loc.value, loc.width)
        if isinstance(loc, ir.Mem):
            return Cell(cells.pop(0)) if cells else Opaque(loc)
        return Opaque(loc)

    read, write = list(loads), list(stores)
    return (
        tuple(one(loc, read, holds) for loc in what.sources),
        tuple(one(loc, write, written) for loc in what.dests),
    )



# Operation identity. Opaque and per-process: what it keys is a table
# built in the same call that hands the bodies out.
_IDS = itertools.count(1)


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

        # How far sp has moved since the top of this block. Reset per block
        # and never carried across one: a slot is named by its depth, so the
        # same depth in two blocks is two different addresses, and joining
        # them would be the one way this could be unsound.
        offset: int | None = 0

        for insn in block.insns:
            node = nodes.get(insn.at)
            if node is None:
                continue
            offset, slot = _stack_slot(node, offset, calls.get(insn.at) if calls else None)
            defines, uses = _touched(node, calls)
            used = tuple(namer.current(one, start) for one in sorted(uses, key=lambda o: (o is not FLAGS, o)))
            loads = _memrefs(node.effects.loads, namer, start, slot)
            stores = _memrefs(node.effects.stores, namer, start, slot)
            holds = dict(zip(sorted(uses, key=lambda o: (o is not FLAGS, o)), used))
            made = []
            for one in sorted(defines, key=lambda o: (o is not FLAGS, o)):
                value = namer.fresh(one, insn.at)
                namer.stack.setdefault(one, []).append(value)
                pushed.append(one)
                made.append(value)
            where = _operands(
                node.semantics,
                holds,
                dict(zip(sorted(defines, key=lambda o: (o is not FLAGS, o)), made)),
                loads,
                stores,
            )
            ops[at].append(
                Op(
                    insn.at,
                    Synth.HALF_TO_LOW if isinstance(node, ir.Restore) else node.semantics.op,
                    node.semantics.name or "",
                    tuple(made),
                    used,
                    loads,
                    stores,
                    node,
                    kind=_kind_of(node.semantics, where[0], where[1]),
                    args=where[0],
                    results=where[1],
                    raised=where,
                    target=node.semantics.target,
                    id=next(_IDS),
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


def _touched_op(op: Op, calls: dict[int, str] | None = None) -> tuple[frozenset[Register_], frozenset[Register_]]:
    """(defines, uses) for an operation, after a transform may have changed it.

    The node's own Effects are iced's conservative answer about the
    instruction BC wrote. A pass that gave the op new semantics changed what
    it touches -- serving a read from a register adds a use of that register
    and `_touched` would never know -- so the two are unioned rather than
    chosen between. Over-approximating a use costs a phi; missing one is a
    value read where nothing wrote it.
    """
    defines: set[Register_] = set()
    uses: set[Register_] = set()
    if op.node is not None:
        was, read = _touched(op.node, calls)
        defines, uses = set(was), set(read)

    what = op.made
    if what is None:
        return frozenset(defines), frozenset(uses)

    def tracked(one: Register_ | None) -> Register_ | None:
        if one is None or one == Register.NONE:
            return None
        root = ir.ROOT.get(one, one)
        return root if root in TRACKED else None

    for one in what.dests:
        if isinstance(one, ir.Reg) and (root := tracked(one.register)) is not None:
            defines.add(root)
    for one in what.sources:
        if isinstance(one, ir.Reg) and (root := tracked(one.register)) is not None:
            uses.add(root)
    # A cell is reached by a register, and reaching it is a read.
    for one in (*what.dests, *what.sources):
        for where in (
            getattr(one, "through", None),
            getattr(one, "index", None),
            getattr(getattr(one, "addr", None), "base", None),
        ):
            if (root := tracked(where)) is not None:
                uses.add(root)
    return frozenset(defines), frozenset(uses)


def _rebased(refs: tuple[MemRef, ...], namer: "_Namer", at: int) -> tuple[MemRef, ...]:
    """The same cells, holding whichever value reaches them now."""
    out = []
    for ref in refs:
        base = None
        if ref.addr is not None:
            root = ir.ROOT.get(ref.addr.base, ref.addr.base)
            if root in TRACKED:
                base = namer.current(root, at)
        out.append(MemRef(ref.addr, ref.width, base, ref.segment))
    return tuple(out)


def resolved(body: MirBody, calls: dict[int, str] | None = None) -> MirBody | str:
    """The same operations, in SSA again, after a pass has moved them.

    raise_body() answers this from machine code and is where the algorithm
    is explained. This answers it from a body that already exists, which a
    pass needs the moment it rewrites one and then asks a question about the
    result: hoisting a load out of a loop makes it live once ahead of the
    loop instead of loop-carried, and until this runs the phis still say
    otherwise. rewrite.py re-raises between passes through emission, so this
    is for *within* a pass, where those bytes do not exist yet.
    """
    blocks = list(body.blocks)
    if not blocks:
        return "no blocks to resolve"
    start = body.entry
    everything = {block.at: block for block in blocks}
    if start not in everything:
        return f"the entry {start:#06x} is not one of these blocks"

    reachable = {start}
    pending = [start]
    while pending:
        for successor in everything[pending.pop()].succ:
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

    frontier = loops.frontiers(blocks, start)
    where: dict[Register_, set[int]] = {}
    for block in blocks:
        for op in block.ops:
            for one in _touched_op(op, calls)[0]:
                where.setdefault(one, set()).add(block.at)
    needed: dict[int, set[Register_]] = {block.at: set() for block in blocks}
    for variable, defined in where.items():
        pending = list(defined)
        seen: set[int] = set()
        while pending:
            for join in frontier.get(pending.pop(), frozenset()):
                if join not in seen:
                    seen.add(join)
                    needed[join].add(variable)
                    pending.append(join)

    namer = _Namer()
    phis: dict[int, dict[Register_, Phi]] = {block.at: {} for block in blocks}
    out: dict[int, list[Op]] = {block.at: [] for block in blocks}
    for block in blocks:
        for variable in sorted(needed[block.at], key=lambda one: (one is not FLAGS, one)):
            phis[block.at][variable] = Phi(namer.fresh(variable, block.at), {})

    def rename(at: int) -> None:
        block = by_at[at]
        pushed: list[Register_] = []
        for variable, phi in phis[at].items():
            namer.stack.setdefault(variable, []).append(phi.result)
            pushed.append(variable)

        for op in block.ops:
            defines, uses = _touched_op(op, calls)
            used = tuple(namer.current(one, start) for one in sorted(uses, key=lambda o: (o is not FLAGS, o)))
            loads = _rebased(op.loads, namer, start)
            stores = _rebased(op.stores, namer, start)
            fresh = []
            for one in sorted(defines, key=lambda o: (o is not FLAGS, o)):
                value = namer.fresh(one, op.at)
                namer.stack.setdefault(one, []).append(value)
                pushed.append(one)
                fresh.append(value)
            out[at].append(replace(op, defines=tuple(fresh), uses=used, loads=loads, stores=stores))

        for successor in block.succ:
            for variable, phi in phis.get(successor, {}).items():
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
                tuple(out[block.at]),
                tuple(one for one in block.succ if one in reachable),
            )
            for block in blocks
        ),
        dict(namer.origin),
        dict(body.pins),
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


def overlapping(
    one: MemRef,
    other: MemRef,
    dgroup: frozenset[int],
    bounds: dict | None = None,
) -> bool:
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
    return module.may_alias(one.addr, other.addr, dgroup, one.width, other.width, bounds)


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
            found.refs.update(_referenced(built, found))
            out.append((f"{body.body.kind} {body.body.name or '(main)'}", built))
    return out


def _referenced(body: MirBody, found: Module) -> dict[int, int]:
    """Which fixup each operation's own operand carries, by op id.

    Asked once, here, while every operation still stands exactly where BC
    wrote it -- which is the only moment the question can be answered from
    the bytes. After this a pass may move an operation anywhere and the
    relocation goes with it, because it belongs to the operand and not to a
    place in the layout.

    A far call is the one whose four relocated bytes are a target rather
    than a displacement, and `at + 1` is not a guess: `9a` then four bytes
    is the only encoding it has.
    """
    known = frozenset(found.fixup_at)
    if not known:
        return {}

    def owned(op: Op) -> int | None:
        if op.node is None or isinstance(op.node, ir.Restore):
            return None
        if found.code[op.at : op.at + 1] in (b"\x9a", b"\xea") and op.at + 1 in known:
            return op.at + 1
        lo, hi = ir.span(op.node)
        inside = [one for one in known if lo <= one < hi]
        return inside[0] if len(inside) == 1 else None

    return {
        op.id: at
        for block in body.blocks
        for op in block.ops
        if op.id is not None and (at := owned(op)) is not None
    }
