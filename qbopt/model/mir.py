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
address. regions.py and memory.py's own rules are the entire alias
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

Nothing is optimised here and nothing is lowered. The raise returns decoded
nodes in a side table keyed by operation identity, so a lowering that applies
no transform can still carry their bytes verbatim without exposing a node to
an optimization pass.
"""

import functools
import itertools
from enum import StrEnum
from typing import overload
from dataclasses import field
from dataclasses import fields
from dataclasses import replace
from dataclasses import dataclass
from collections.abc import Iterator

from iced_x86 import Code
from iced_x86 import Register
from iced_x86 import Register_
from iced_x86 import RegisterExt

from qbopt.model import ir
from qbopt.abi import runtime
from qbopt.model import memory
from qbopt.analysis import loops
from qbopt.frontend import stack
from qbopt.analysis import regions
from qbopt.objectfile import module
from qbopt.frontend.blocks import Block
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space
from qbopt.objectfile.module import Module
from qbopt.model.floating import Semantics as FloatingSemantics

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

# Each contract register at its own name, which is also what says how wide
# it is: a routine that reads ax reads two bytes of it. One table, because
# two would be two places to add a register to. The root -- which variable
# it is, which is the other question -- comes from `ir.ROOT`.
AS_NAMED = {
    runtime.Reg.AX: Register.AX,
    runtime.Reg.BX: Register.BX,
    runtime.Reg.CX: Register.CX,
    runtime.Reg.DX: Register.DX,
    runtime.Reg.SI: Register.SI,
    runtime.Reg.DI: Register.DI,
    runtime.Reg.FLAGS: FLAGS,
}

FROM_CONTRACT = {one: ir.ROOT.get(where, where) for one, where in AS_NAMED.items()}


@dataclass(frozen=True, slots=True)
class Value:
    """One SSA variable: defined by exactly one Op or Phi, in one place.

    Deliberately says nothing about where it lives. A value used to be named
    after the machine register BC kept it in -- `eax#155` -- which made the
    raise/lower round trip checkable and then blocked everything after it:
    two computations cannot be compared by what they compute while their
    names contain where they landed, and a 32-bit root has no way to say
    "the low half of that". docs/variables.md has the measurement.

    Where BC kept it is captured by the raise in ``AllocationHints`` and is
    handed directly to lowering.  It is not reachable from a public MIR body,
    so an analysis cannot accidentally turn historical placement into program
    semantics.
    """

    id: int
    at: int  # the instruction that defined it, or the block for a phi
    flags: bool = False  # the flags variable, which is not data and holds none
    # Which variable this is a version of, and which version. In MIR a
    # register is a variable and nothing more, so every value BC kept in one
    # place is one variable written several times -- and numbering them
    # `v56`, `v394`, `v400` said they were three, which is what makes a loop
    # header's phis read as six variables when they are six registers.
    #
    # `variable` is an index, not a register.  AllocationHints may associate
    # that index with a soft historical home after MIR optimization.
    variable: int = 0
    version: int = 0

    def __repr__(self) -> str:
        kind = "f" if self.flags else "v"
        return f"{kind}{self.variable}_{self.version}" if self.version else f"{kind}{self.id}"


@dataclass(frozen=True, slots=True)
class IntegerRange:
    """A frontend-established, non-wrapping mathematical integer range.

    This is source semantics, not a machine representation: it says what
    values a MIR integer may have.  The frontend boundary is responsible for
    translating any ABI or target rule into this plain fact before an
    optimizer sees it.
    """

    low: int
    high: int
    width: int


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

    addr: Addr | None  # None where nothing can name the exact byte
    width: int
    base: Value | None = None
    segment: Value | None = None
    # Which object this reference is in, known even where the byte is not.
    #
    # LLVM's `PseudoSourceValue`: a machine memory operand whose exact
    # address it cannot name still says whether it is a stack slot, a
    # constant pool, the GOT -- and two different kinds never alias. Ours
    # is the same idea over `Space`.
    #
    # It is what a push needs. The raise names the slot a push lands in
    # while it knows the stack depth, and gives up on the address when it
    # does not -- but the push is still a push, and a push cannot land on a
    # global whatever the depth. Without this, two thirds of the corpus's
    # pushes aliased every named cell in their body, and nothing was
    # promotable anywhere.
    space: "Space | None" = None
    # What this reference can reach inside the program's own data, as
    # (that segment's index, the cells whose address was handed out).
    # None means "anything", which is what every reference but a runtime
    # call's says.
    #
    # A call writes its own data at fixed addresses and never a program's
    # variable -- tools/runtime_writes.py measures that on the linked
    # image -- so what it reaches in BC_DATA is what the program handed it
    # a pointer to, and nothing else. Carried as what it *can* reach
    # rather than what it cannot, because the second is not enumerable.
    #
    # Origins have no recorded extent. If any pointer into this segment
    # escapes, disjoint fields require the explicit `excludes` ranges below.
    beyond: "tuple[int, frozenset] | None" = None
    symbolic: "Symbol | None" = None  # proven effective address; original operands remain for lowering
    allocation: "Symbol | None" = None  # proven in-bounds access to this dynamic allocation
    base_width: int = 4  # width of the address value, independently of the memory data width
    pointer: bool = False  # base is a whole pointer, not a numerical index or offset
    excludes: tuple[tuple[Addr, int], ...] = ()  # exact byte ranges this effect cannot reach
    # The source language's aliasing class, and whether this is a declared object of it rather than an access.
    typed: "tuple[str, bool] | None" = field(default=None, compare=False)
    # The frame objects' bytes (from bp) an address the body took of them stays inside, where the language says so.
    within: "tuple[tuple[int, int], ...] | None" = field(default=None, compare=False)
    # Canonical object/subobject identity. Legacy frontends are normalized by
    # analysis.regions; source frontends attach this directly.
    provenance: "memory.Provenance | None" = None
    # A source-language volatile access is observable even when its value is
    # redundant.  The operation carrying this reference is also a scheduling
    # barrier, but the property belongs on the memory occurrence so cloning,
    # splitting and aggregate expansion cannot silently shed it.
    volatile: bool = False
    # The source language promises this access stays inside one object.
    inbounds: bool = field(default=False, compare=False)

    @property
    def where(self) -> "Space | None":
        """The object this names, from the address where there is one."""
        return self.addr.space if self.addr is not None else self.space


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
class Symbol:
    space: Space
    index: int
    offset: int
    width: int
    addend: int = 0


@dataclass(frozen=True, slots=True)
class FrameAddress:
    offset: int
    width: int
    extent: tuple[int, int] | None = None  # the object's bytes, where the language keeps an address inside them


@dataclass(frozen=True, slots=True)
class FrameSelector:
    """The run-time selector of the current activation's frame segment."""

    width: int = 2


@dataclass(frozen=True, slots=True)
class ArrayRequest:
    descriptor: Symbol
    element_width: int
    bounds: tuple[tuple[int, int], ...]
    replaces: bool = False


@dataclass(frozen=True, slots=True)
class Cell:
    """A memory cell -- the same MemRef the op's loads and stores name."""

    ref: "MemRef"


@dataclass(frozen=True, slots=True)
class Opaque:
    """A named machine resource MIR has no value for.

    The x87 stack, whose registers rotate under push and pop and so are not
    a location the way a register is, and the segment registers, which hold
    a descriptor rather than a number. Everything else -- 96.6% of the
    corpus -- is a value, a constant or a cell.

    `name` is what the resource is called: "es", "st0". A pass that has to
    reason about one says the name, so it imports no register number and
    never looks inside `what`. That is the whole of the permission.
    """

    what: object
    name: str = ""


type Arg = Held | Const | Symbol | FrameAddress | FrameSelector | Cell | Opaque


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
    # c := a op b, plus the carry the operation before it left. The carry
    # is a value here, not an adjacency: the operation reads the flags the
    # one before it defined, so nothing can be scheduled between them
    # without the dataflow saying so. Folded into ADD and SUB, nothing
    # below could tell a carrying add from a plain one -- and a lowering
    # that re-encodes from MIR wrote `add` for `adc`, dropping the borrow
    # `neg ax` had left: negnot printed D=-33752584 for -33818120.
    ADD_CARRY = "addcarry"
    SUB_BORROW = "subborrow"
    # c := a + 1 and c := a - 1, which the machine spells with the operand
    # in the opcode. Their own kinds rather than an addition of a written
    # 1: the two differ in what they leave in the carry, so folding them
    # into ADD/SUB made a lowering that re-encodes from MIR choose between
    # a shape with one operand and operands numbering two -- and select
    # had no form for the result.
    INCREMENT = "increment"
    DECREMENT = "decrement"
    MUL = "mul"
    SMULHI = "smulhi"  # Signed product's upper half, at the operands' common width.
    # Stored-width fixed arithmetic, args (left, right, fractional bits).
    # These remain semantic until target lowering: on a 386, fixed i32 MUL
    # is a native 32x32->64 IMUL plus rescale, not a generic i64 operation.
    FIXED_MUL = "fixed_mul"
    FIXED_DIV = "fixed_div"
    DIV = "div"
    REM = "rem"
    # One computation with two results, quotient then remainder. BC calls
    # B$DVI4 for one and B$RMI4 for the other over the same operands, and
    # a kind each gives CSE two keys for one divide -- which is why
    # lngmix's loop divides ten times and takes the modulus again.
    DIVMOD = "divmod"
    UDIVMOD = "udivmod"  # DIVMOD on unsigned operands
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
    SIGN_EXTEND = "sign_extend"
    ZERO_EXTEND = "zero_extend"
    ADDRESS = "address"  # c := the number an address is
    PTR_OFFSET = "ptr_offset"  # c := pointer a advanced by a byte displacement
    FILL = "fill"  # args (value, count, address): count cells of the value's width from address, each := value
    # Device I/O, always volatile: c := the byte at port a; port a := byte b.
    # Memory reach is the port's device's, from qbopt.abi.ports.
    PORT_IN = "port_in"
    PORT_OUT = "port_out"

    # control
    CALL = "call"
    BRANCH = "branch"  # on a value, to `target` or the next block
    SWITCH = "switch"
    JUMP = "jump"
    RETURN = "return"
    ESCAPE = "escape"  # leaves the body somewhere it does not name

    # floating point, until the x87 stack is resolved at the raise
    FADD = "fadd"
    FSUB = "fsub"
    FMUL = "fmul"
    FDIV = "fdiv"
    FNEG = "fneg"
    FABS = "fabs"
    FSQRT = "fsqrt"
    FLOAD = "fload"
    FSTORE = "fstore"
    FCOMPARE = "fcompare"
    FCHECK = "fcheck"  # Observe pending floating exceptions without computing a value.

    # what has no MIR form yet, each with the step that removes it
    ARG = "arg"  # a call argument still written as a push -- step 3
    RESULT = "result"  # and its pop
    JOIN = "join"  # two halves of a wide value made one -- step 4
    EXTRACT = "extract"  # a bit range of a value; args[1] is the bit offset
    CONCAT = "concat"  # high bits followed by low bits, with explicit operand widths
    OPAQUE = "opaque"  # nothing is claimed; see ir.Operation.BARRIER
    NOTHING = "nothing"


# `a test b` is `b MIRRORED[test] a`.
MIRRORED = {
    Kind.EQ: Kind.EQ, Kind.NE: Kind.NE, Kind.LT: Kind.GT, Kind.GT: Kind.LT, Kind.LE: Kind.GE, Kind.GE: Kind.LE,
    Kind.BELOW: Kind.ABOVE, Kind.ABOVE: Kind.BELOW, Kind.BELOW_EQ: Kind.ABOVE_EQ, Kind.ABOVE_EQ: Kind.BELOW_EQ,
}  # fmt: skip
# `not (a test b)` is `a NEGATED[test] b`.
NEGATED = {
    Kind.EQ: Kind.NE, Kind.NE: Kind.EQ, Kind.LT: Kind.GE, Kind.GE: Kind.LT, Kind.LE: Kind.GT, Kind.GT: Kind.LE,
    Kind.BELOW: Kind.ABOVE_EQ, Kind.ABOVE_EQ: Kind.BELOW, Kind.BELOW_EQ: Kind.ABOVE, Kind.ABOVE: Kind.BELOW_EQ,
}  # fmt: skip


# One x86 instruction to what it computes. The mnemonic is consulted only
# here: BINARY and UNARY do not say which operation they are, so the raise
# is where that is decided and after it nothing needs to ask.
_BY_NAME: dict[str, Kind] = {
    "add": Kind.ADD,
    "adc": Kind.ADD_CARRY,
    "inc": Kind.INCREMENT,
    "sub": Kind.SUB,
    "sbb": Kind.SUB_BORROW,
    "dec": Kind.DECREMENT,
    "cmp": Kind.SUB,
    "and": Kind.AND,
    "test": Kind.AND,
    "or": Kind.OR,
    "xor": Kind.XOR,
    "not": Kind.NOT,
    "neg": Kind.NEG,
    "shl": Kind.SHL,
    "sal": Kind.SHL,
    "shr": Kind.SHR,
    "sar": Kind.SAR,
    "imul": Kind.MUL,
    "mul": Kind.MUL,
    "idiv": Kind.DIV,
    "div": Kind.DIV,
    "fadd": Kind.FADD,
    "faddp": Kind.FADD,
    "fsub": Kind.FSUB,
    "fsubp": Kind.FSUB,
    "fsubr": Kind.FSUB,
    "fsubrp": Kind.FSUB,
    "fmul": Kind.FMUL,
    "fmulp": Kind.FMUL,
    "fdiv": Kind.FDIV,
    "fdivp": Kind.FDIV,
    "fdivr": Kind.FDIV,
    "fdivrp": Kind.FDIV,
    "fchs": Kind.FNEG,
    "fabs": Kind.FABS,
    "fsqrt": Kind.FSQRT,
    "fcom": Kind.FCOMPARE,
    "fcomp": Kind.FCOMPARE,
    "fcompp": Kind.FCOMPARE,
    "ftst": Kind.FCOMPARE,
}

# A conditional branch's mnemonic is the comparison it reads. The flags in
# between are the machine's way of getting one to the other and are not a
# value: step 2 folds the comparison into the branch and they disappear.
_BY_BRANCH: dict[str, Kind] = {
    "jl": Kind.LT,
    "jnge": Kind.LT,
    "jle": Kind.LE,
    "jng": Kind.LE,
    "jg": Kind.GT,
    "jnle": Kind.GT,
    "jge": Kind.GE,
    "jnl": Kind.GE,
    "je": Kind.EQ,
    "jz": Kind.EQ,
    "jne": Kind.NE,
    "jnz": Kind.NE,
    "jb": Kind.BELOW,
    "jc": Kind.BELOW,
    "jnae": Kind.BELOW,
    "jbe": Kind.BELOW_EQ,
    "jna": Kind.BELOW_EQ,
    "ja": Kind.ABOVE,
    "jnbe": Kind.ABOVE,
    "jae": Kind.ABOVE_EQ,
    "jnb": Kind.ABOVE_EQ,
    "jnc": Kind.ABOVE_EQ,
}


# How each modelled float operation moves the stack. Named once, here: the
# shape is ir.py's answer and reading it twice is how the two drift apart.
_FLOAT_DEPTH: dict = {
    ir.Operation.FLOAT_LOAD: 1,
    ir.Operation.FLOAT_STORE: -1,
    ir.Operation.FLOAT_ARITH_POP: -1,
    ir.Operation.FLOAT_ARITH: 0,
    ir.Operation.FLOAT_UNARY: 0,
}


def _stack_effect(what: "ir.Semantics") -> int | None:
    """The net depth change, or None where this is not a modelled shape."""
    return _FLOAT_DEPTH.get(what.op)


def consumed(op: "Op") -> "set[Value]":
    """Every value this operation actually reads.

    A merged use is only carried into the result -- unless the operation
    also names it as an operand: `or ax,[m]` reads AX's low word and
    carries its high one, so the same use is both.
    """
    explicit = {arg.value for arg in op.args if isinstance(arg, Held)}
    cells = [arg.ref for arg in (*op.args, *op.results) if isinstance(arg, Cell)]
    explicit.update(value for cell in cells for value in (cell.base, cell.segment) if isinstance(value, Value))
    return (set(op.uses) - op.merges.keys()) | explicit


def rewritten(op: "Op") -> bool:
    """Whether a pass has changed what this operation computes.

    The operands, not a selected instruction, say whether a pass changed it.
    """
    return op.raised is not None and (op.args, op.results) != op.raised


def _merged(what: "ir.Semantics", holds: dict, written: dict, args: tuple) -> dict:
    """Which use is only the previous contents of which result.

    A word result preserves the upper word even when its low word is an
    arithmetic input. Otherwise a use is carried when it shares a place
    with a result and the operation never names that place as an input. An
    operation naming no operand at all describes nothing, so nothing in it
    is a partial write: a call names none, and every argument it reads
    shares a register with something it clobbers -- reading those as
    previous contents made B$OGTA's branch index look dead.
    """
    if not what.sources and not what.dests:
        return {}
    named = {ir.ROOT.get(one.register, one.register) for one in what.sources if isinstance(one, ir.Reg)}
    narrow = {
        ir.ROOT.get(one.register, one.register) for one in what.dests if isinstance(one, ir.Reg) and one.width == 2
    }
    out: dict = {}
    for register, value in written.items():
        if value.flags:
            continue
        root = ir.ROOT.get(register, register)
        if root in named and root not in narrow:
            continue
        was = holds.get(register)
        if was is not None:
            out[was] = value
    return out


def partial(op: "Op") -> bool:
    """Whether a result keeps part of what its place held before.

    A word result's merge is the register's upper word, which a word value
    does not have; only a narrower or unnamed result is partial.
    """
    words = {result.value for result in op.results if isinstance(result, Held) and result.width == 2}
    return any(value not in words for value in op.merges.values())


# What each absorbable routine computes, in MIR's own vocabulary.
_ABSORBS = {
    "B$MUI4": Kind.MUL,
    "B$DVI4": Kind.DIVMOD,
    "B$RMI4": Kind.DIVMOD,
    "B$CPI4": Kind.SUB,  # a comparison subtracts and keeps only the flags
}


def _absorbed_loads(args: tuple, namer: "_Namer", at: int) -> tuple[MemRef, ...]:
    """The memory a folded site reads: the cells among its own operands.

    The routines absorption knows take their longs by value and touch no
    user memory, so a folded site reads exactly what BC pushed into it --
    and `_absorbing` only folds a site whose every operand is a static
    address or a written-down constant, which is what makes that a list
    and not an approximation.
    """
    return _rebased(tuple(one.ref for one in args if isinstance(one, Cell) and one.ref.addr is not None), namer, at)


def absorbs(name: str) -> "Kind | None":
    """The kind a site of this name raises as, or None if it stays a call."""
    return _ABSORBS.get(name.upper())


def _within(sites: dict) -> frozenset[int]:
    """Every byte a site's pushes occupy, so the raise can pass over them."""
    out = set(at for site in sites.values() for at in range(site.start, site.at))
    out.update(at for site in sites.values() for one in site.consume for at in range(one.at, one.end))
    return frozenset(out)


def _disjoint(site) -> tuple[tuple[int, int], ...]:
    """A site's pushes as contiguous runs, where they sit apart from `covers`.

    match() only finds a site whose pushes run straight into the call, and
    there `covers` already accounts for them as one interval. A site
    frames() found instead may have a real instruction between its last
    push and its call -- lngmix spills the first divide's result there --
    so what it stands for is two runs, not one, and `also` says so.
    """
    if not site.consume:
        return ()
    runs: list[list[int]] = []
    for insn in site.consume:
        if runs and runs[-1][1] == insn.at:
            runs[-1][1] = insn.end
        else:
            runs.append([insn.at, insn.end])
    return tuple((lo, hi) for lo, hi in runs)


def _absorbing(site, written: dict) -> "tuple[Kind, tuple, tuple] | None":
    """One absorbable call as (kind, args, results), or None.

    The operands come from calls.py, which read them off the pushes while
    the bytes were still BC's -- a static address becomes a four-byte cell
    and an immediate a constant. What the routine leaves behind is what its
    contract already said, so the results are the values the call defines.
    """
    from qbopt.legacy import calls as machine

    kind = _ABSORBS.get(site.name.upper())
    if kind is None:
        return None
    args = []
    for one in site.operands:
        if one.kind is machine.Kind.STATIC and one.addr is not None:
            args.append(Cell(MemRef(one.addr, 4)))
        elif one.kind is machine.Kind.CONSTANT:
            args.append(Const(one.value, 4))
        else:
            return None
    if kind is Kind.DIVMOD:
        # By role, not by register number. The runtime hands back
        # whichever of quotient and remainder its own name promises, in
        # eax either way -- so B$DVI4's visible long is the quotient and
        # B$RMI4's is the remainder, and the other role is the answer the
        # call was not asked for. Ordering the pair the same way regardless
        # of which call it came from is what lets the two sites read as one
        # computation.
        #
        # Both come from the site rather than from register order. Taking
        # the second as "the first other register the call clobbers" named
        # the one the divisor is loaded into: for both of lngmix's divides
        # MIR claimed a result in the register holding the constant 7, and
        # only the fact that nothing read it kept that from being wrong.
        visible = written.get(machine.RESULT)
        other = written.get(machine.other_result(site))
        if visible is None or other is None or visible.flags:
            return None
        pair = (other, visible) if site.name.upper() == machine.REMAINDER else (visible, other)
        return kind, tuple(args), tuple(Held(one, 4) for one in pair)
    made = [Held(value, 4) for register, value in sorted(written.items()) if not value.flags]
    return kind, tuple(args), tuple(made[:2])


def stepping(op: "Op") -> "tuple[Arg, Arg] | None":
    """(what it steps, by how much), or None where it steps nothing.

    An increment steps by one and says so here rather than by carrying a
    written 1 in its operands: `inc` and `dec` differ from `add`/`sub` in
    what they leave in the carry, so they are their own operations -- and
    every pass that only wants "an affine step" asks this instead of
    listing kinds.
    """
    if op.loads or op.stores:
        return None
    if op.kind is Kind.INCREMENT and len(op.args) == 1:
        return op.args[0], Const(1, getattr(op.args[0], "width", 2))
    if op.kind is Kind.DECREMENT and len(op.args) == 1:
        return op.args[0], Const(-1, getattr(op.args[0], "width", 2))
    if op.kind in (Kind.ADD, Kind.SUB) and len(op.args) == 2:
        one, other = op.args
        if op.kind is Kind.SUB:
            # Only a constant can be negated into a step. `x - y` for an
            # invariant y is not `x + y`, and reporting it as a step would
            # give a counter the wrong direction.
            return (one, Const(-other.n, other.width)) if isinstance(other, Const) else None
        return one, other
    return None


def _normalised(kind: Kind, name: str, args: tuple, results: tuple) -> tuple:
    """The operands the operation really has, once the machine's are gone.

    `inc ax` is `ax := ax + 1` and the 1 is in the opcode; `dec` likewise.
    Writing them down is the raise translating an instruction into what it
    computes, which is the only way a pass can fold one without knowing
    that x86 has an increment.
    """
    if kind in (Kind.ADD, Kind.SUB) and name in ("inc", "dec") and len(args) == 1:
        width = getattr(args[0], "width", 2)
        return (*args, Const(1, width))
    return args


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
class FloatingOrigin:
    """Lowering's identity baseline while general floating allocation is unfinished."""

    block: int
    sequence: tuple[int, ...]
    at: int
    kind: Kind
    semantics: FloatingSemantics
    inputs: tuple[Arg, ...]
    outputs: tuple[Arg, ...]
    machine_inputs: tuple[Arg, ...]
    machine_outputs: tuple[Arg, ...]


@dataclass(frozen=True, slots=True)
class Op:
    """One instruction, as values in and values out."""

    at: int
    op: ir.Operation | Synth
    name: str
    defines: tuple[Value, ...]
    uses: tuple[Value, ...]
    array: ArrayRequest | None = field(default=None, kw_only=True)
    memory_values: tuple[tuple[MemRef, Const], ...] = field(default=(), kw_only=True)
    floating: FloatingSemantics | None = field(default=None, kw_only=True)
    floating_origin: FloatingOrigin | None = field(default=None, kw_only=True)
    loads: tuple[MemRef, ...] = ()
    stores: tuple[MemRef, ...] = ()
    # Whether this occurrence has a decoded source node in the external
    # SourceMap.  This says only that lowering may recover its original
    # machine occurrence; the node itself never crosses the raise boundary.
    source_backed: bool = False
    # What this operation reads and writes, in its own order, as values,
    # constants and cells. A pass rewriting an operation says it here;
    # selected machine semantics live only on LIR.
    # What this computes, in MIR's own vocabulary. `op` and `name` are the
    # machine's and are on their way out; nothing new may read them.
    kind: Kind = Kind.OPAQUE
    # How this changes the depth of the operand stack the float unit keeps:
    # +1 for a load, -1 for a store or a popping arithmetic form, 0 for one
    # that works in place. None where it is not one of those shapes at all.
    # A number about a stack, not an instruction -- and gone in the step
    # that gives float operands values of their own.
    stack: int | None = None
    # Which comparison a branch tests. The flags between the compare and the
    # branch are how the machine gets one to the other; what the branch
    # means is `a <= b`, and this is where that is said.
    test: Kind | None = None
    # Uses that are only the previous contents of what this writes, and
    # which result each survives into. A narrow write under a wider
    # register model is a read-modify-write: a two-byte store into a
    # four-byte variable leaves the top half alone, so the operation reads
    # it. That read is real and is worth exactly what the write it feeds is
    # worth -- and it is not an input. Decided at the raise, because which
    # halves share a place is the machine's answer and no pass's.
    merges: dict = field(default_factory=dict)
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
    # SWITCH uses target as its explicit default; invalid-selector behavior
    # must be an explicit CFG path, never an implicit assumption about default.
    cases: tuple[tuple[int, int], ...] = field(default=(), kw_only=True)
    # What this operation is, apart from where it is. Given at the raise and
    # carried through every `replace()`, so a fact established then can live
    # in a side table instead of on the op -- which is the only way those
    # facts survive the hoist moving something: a table keyed by `at` does
    # not. None means an operation a pass invented, which has no past and
    # must say what it computes in MIR's own terms.
    id: int | None = None
    # Whether this operation still holds the relocated operand its record
    # names. None is the ordinary answer -- nothing moved it. A pass below
    # the boundary that lifts a symbolic cell onto an instruction of its
    # own sets True there and False on what it left behind, so the fixup
    # follows the operand and is not bound to whatever field remained.
    symbol: "bool | None" = None
    # Whether `args` is the operation's real argument list. Only a CALL can
    # say no: a runtime routine whose code is not in the tree declares no
    # inputs, and `()` would then mean "reads nothing" -- the one thing
    # known to be false about it. `uses` stays the conservative read set
    # either way, so no live value is discarded. At the end of the field
    # list because every construction here is positional.
    args_known: bool = True
    # Complete memory footprints do not imply modeled computation or
    # permission to move/remove an opaque operation.
    memory_complete: bool = False
    # Complete value reads do not imply movable or removable side effects.
    reads_complete: bool = False
    # A source-language volatile access is ordered and observable while its
    # computation remains fully modeled.  This cannot be represented by
    # replacing ``op`` with Operation.BARRIER: doing so erases the encoded
    # floating operation whose precision and rounding lowering must verify.
    volatile: bool = field(default=False, kw_only=True)
    # Effects on resources MIR deliberately does not model as values. Names,
    # not registers: passes may preserve ordering without learning machine
    # locations. None means every such resource.
    opaque_defs: frozenset[str] | None = frozenset()
    opaque_uses: frozenset[str] | None = frozenset()
    # Opaque identities of raise-time occurrences whose bytes this operation
    # replaces. SourceMap owns the actual ranges; passes may only transfer
    # identities when combining or deleting operations.
    absorbed: tuple[int, ...] = ()
    # Whether CALL takes its destination from a value rather than a named
    # procedure.  This is control-flow meaning established by the frontend,
    # not an encoding choice: lowering decides how that value is addressed.
    indirect: bool = False
    # Values observable after this operation exits the body.  They are not
    # operands: a terminal runtime call can define one of them itself.  This
    # separate semantic edge prevents ordinary dataflow analyses and lowering
    # from mistaking ABI visibility for an encoded machine input.
    exits: tuple[Value, ...] = field(default=(), kw_only=True)

    @property
    def barrier(self) -> bool:
        return self.op is ir.Operation.BARRIER or self.volatile

    @property
    def inserted(self) -> bool:
        """An operation invented above lowering, with no source occurrence."""
        return not self.absorbed


def jump(beside: Op, destination: int) -> Op:
    """`beside`, a block's last operation, as an unconditional jump to `destination`."""
    return replace(
        beside,
        kind=Kind.JUMP,
        name="",
        args=(),
        results=(),
        uses=(),
        defines=(),
        loads=(),
        stores=(),
        merges={},
        source_backed=False,
        raised=((), ()),
        target=destination,
        test=None,
    )


def computed(at: int, kind: Kind, result: Value, args: tuple[Arg, ...], width: int) -> Op:
    """A source-free MIR computation invented by a semantic transform.

    Passes name only the computation and its operands. The legacy machine
    fields are deliberately blank here; selecting an instruction spelling is
    lowering's responsibility.
    """
    loads = tuple(one.ref for one in args if isinstance(one, Cell))
    uses = dict.fromkeys(one.value for one in args if isinstance(one, Held))
    uses.update((value, None) for ref in loads for value in (ref.base, ref.segment) if value is not None)
    return Op(
        at,
        ir.Operation.NOTHING,
        "",
        (result,),
        tuple(uses),
        loads=loads,
        source_backed=False,
        kind=kind,
        args=args,
        results=(Held(result, width),),
        id=None,
        symbol=False,
        memory_complete=True,
        reads_complete=True,
    )


def cleared(op: Op) -> Op:
    """Retain an occurrence's source ownership while deleting its meaning."""
    return replace(
        op,
        kind=Kind.NOTHING,
        name="",
        defines=(),
        uses=(),
        loads=(),
        stores=(),
        args=(),
        results=(),
        merges={},
        raised=None,
        target=None,
        test=None,
        stack=None,
        symbol=False,
    )


@dataclass(frozen=True, slots=True)
class _RaisedOp(Op):
    """An occurrence inside the raise, before machine provenance is externalized.

    These byte ranges are deliberately absent from :class:`Op`. Recognition
    may inspect and combine physical source occurrences while constructing
    MIR; the completed body retains only opaque ``absorbed`` identities.
    """

    node: ir.Node | None = None
    covers: tuple[int, int] | None = None
    extra_covers: tuple[tuple[int, int], ...] = ()

    @property
    def inserted(self) -> bool:
        """Whether this private raising occurrence owns no input bytes."""
        return (
            not self.source_backed
            and not self.extra_covers
            and self.covers is not None
            and self.covers[0] == self.covers[1]
        )


def detached(operation: Op, **changes) -> Op:
    """A rewritten raising operation that no longer carries its decoded node."""
    changes["source_backed"] = False
    if isinstance(operation, _RaisedOp):
        changes["node"] = None
    return replace(operation, **changes)


def source_free(operation: Op, **changes) -> Op:
    """A raising rewrite that emits work but owns no input occurrence.

    Recognition sometimes expands one source instruction into several MIR
    operations.  Exactly one result claims the input occurrence; companions
    are ordinary public MIR from their creation and therefore cannot leak a
    private byte range past the raise boundary.
    """
    values = {one.name: getattr(operation, one.name) for one in fields(Op)}
    values.update(changes, source_backed=False, absorbed=())
    return Op(**values)


def raising_occurrence(
    operation: Op,
    covers: tuple[int, int],
    *,
    extra: tuple[tuple[int, int], ...] = (),
    node: ir.Node | None = None,
) -> Op:
    """Attach raw ownership to an operation that has not crossed the raise.

    The decoder uses ``_RaisedOp`` directly. Focused frontend tests use this
    constructor so they state explicitly that they are exercising the private
    recognition form rather than manufacturing invalid completed MIR.
    """
    values = {one.name: getattr(operation, one.name) for one in fields(Op)}
    return _RaisedOp(**values, node=node, covers=covers, extra_covers=extra)


def raising_owned(operation: Op, *owners: Op) -> Op:
    """Give a rewritten raising operation the exact occurrences of ``owners``.

    This helper is intentionally meaningful only before ``_externalized``.
    It keeps physical range manipulation inside frontend recognition while
    ensuring the operation handed to optimization is the public range-free
    :class:`Op`.
    """
    ranges = sorted(span for owner in owners for span in _raising_ranges(owner) if span[0] < span[1])
    merged: list[tuple[int, int]] = []
    for low, high in ranges:
        if merged and low <= merged[-1][1]:
            merged[-1] = (merged[-1][0], max(merged[-1][1], high))
        else:
            merged.append((low, high))
    if not merged:
        return operation
    values = {one.name: getattr(operation, one.name) for one in fields(Op)}
    return _RaisedOp(**values, node=getattr(operation, "node", None), covers=merged[0], extra_covers=tuple(merged[1:]))


def raising_adjacent(first: Op, second: Op) -> bool:
    """Whether two single source occurrences touch inside recognition."""
    before, after = _raising_ranges(first), _raising_ranges(second)
    return len(before) == len(after) == 1 and before[0][1] == after[0][0]


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
    # Reached only on a path the frontend expects never to run, such as
    # raising an error. Layout places it after the hot code.
    cold: bool = False


@dataclass(frozen=True, slots=True)
class MirBody:
    entry: int
    blocks: tuple[MirBlock, ...]
    # Loader-established literal bytes, valid on entry only. They are not
    # immutable: ordinary alias and call effects invalidate these facts.
    initial: tuple[tuple[MemRef, Const], ...] = ()
    # Explicit whole-block repetition established by bounded loop expansion.
    # Provenance for lowering; transformations still operate on ordinary values.
    repetitions: tuple[tuple[int, int], ...] = ()
    # Cloned CFGs retain source provenance but have a new semantic sequence.
    cloned: bool = False
    # Control enters only at `entry` and moves only along `succ`: nothing
    # resumes inside the body. The raise's to say -- BC's error and event
    # handlers do resume inside one, C has nothing that does.
    sealed: bool = False
    # SS == DS: the stack is in the data group, so DS reaches a frame object
    # through a near pointer. The raise's to say -- BC runs so, Watcom C not.
    stack_in_data: bool = False
    # Source-language pointer facts. These are semantic metadata, not places.
    pointer_values: frozenset[Value] = frozenset()
    pointer_seeds: dict[Value, "memory.Provenance"] = field(default_factory=dict)
    integer_ranges: dict[Value, IntegerRange] = field(default_factory=dict)
    # Exact positive execution counts already proved by a MIR transform.
    # Most counts are rediscovered from the final recurrence in lowering;
    # transforms such as nested-recurrence rewind deliberately change that
    # spelling while retaining the proof.  Analyses may consume this program
    # fact; it carries no target or placement information.
    loop_trip_counts: tuple[tuple[int, int], ...] = ()

    def block(self, at: int) -> MirBlock | None:
        return next((one for one in self.blocks if one.at == at), None)

    @property
    def values(self) -> tuple[Value, ...]:
        return tuple(
            value
            for block in self.blocks
            for value in ([phi.result for phi in block.phis] + [v for op in block.ops for v in op.defines])
        )


@dataclass(frozen=True, slots=True)
class _RaisedBody(MirBody):
    """The raise's private machine view before the public MIR boundary.

    Recognition is allowed to ask where source values arrived; optimization
    is not.  ``bodies()`` and the C frontend consume these fields into an
    ``AllocationHints`` side table and return an ordinary ``MirBody``.
    """

    origin: dict[Value, Register_] = field(default_factory=dict)
    pins: dict[Value, Register_] = field(default_factory=dict)


def _live_outs(body: _RaisedBody) -> dict[int, frozenset[Value]]:
    """The source values observable at each machine exit.

    This is the last placement-aware question asked by raising.  The result
    is immediately materialized as semantic MIR exit observations, so neither
    this map nor the source registers cross the public boundary.
    """
    from qbopt.analysis import liveness as alive_at

    predecessors = loops.predecessors(list(body.blocks))
    arriving: dict[Register_, set[Value]] = {}
    for value in alive_at.entry_values(body):
        # FLAGS is not part of the procedure's value-return ABI.  Recognition
        # can remove a source instruction while a dead flag phi still names
        # its old result; that dangling SSA name is neither a caller input nor
        # an observable machine exit value.
        if value.flags:
            continue
        register = body.origin.get(value)
        if register is not None:
            arriving.setdefault(ir.ROOT.get(register, register), set()).add(value)

    outof: dict[int, dict[Register_, set[Value]]] = {block.at: {} for block in body.blocks}
    changing = True
    while changing:
        changing = False
        for block in body.blocks:
            here: dict[Register_, set[Value]] = {}
            incoming = [outof[one] for one in predecessors[block.at]]
            if block.at == body.entry:
                incoming.append(arriving)
            for state in incoming:
                for register, values in state.items():
                    here.setdefault(register, set()).update(values)
            for phi in block.phis:
                if phi.result.flags:
                    continue
                register = body.origin.get(phi.result)
                if register is not None:
                    here[ir.ROOT.get(register, register)] = {phi.result}
            for op in block.ops:
                for value in op.defines:
                    if value.flags:
                        continue
                    register = body.origin.get(value)
                    if register is not None:
                        here[ir.ROOT.get(register, register)] = {value}
            if here != outof[block.at]:
                outof[block.at] = here
                changing = True

    result = {}
    for block in body.blocks:
        if block.succ:
            continue
        # A modelled return already names the ABI values its caller can
        # observe.  Keeping every other register merely because the source
        # instruction left bits there turns allocator provenance into public
        # program semantics: an INTEGER procedure kept DX live as though it
        # returned a LONG.  Unknown transfers retain the conservative full
        # machine state assembled above.
        if block.ops and block.ops[-1].kind is Kind.RETURN:
            result[block.at] = frozenset(value for value in consumed(block.ops[-1]) if not value.flags)
        else:
            result[block.at] = frozenset(value for values in outof[block.at].values() for value in values)
    return result


def _with_live_outs(body: _RaisedBody) -> _RaisedBody:
    """Turn placement-derived exit visibility into machine-free MIR uses.

    Exit observations are distinct from ordinary uses: the terminal
    operation does not read them, and may define them itself.  SSA rewrites
    carry this edge explicitly while ordinary analyses remain unchanged.
    """
    live = _live_outs(body)
    blocks = []
    for block in body.blocks:
        leaving = live.get(block.at, frozenset())
        if not leaving:
            blocks.append(block)
            continue
        ordered = tuple(sorted(leaving, key=lambda value: (value.variable, value.version, value.id)))
        if block.ops:
            last = block.ops[-1]
            blocks.append(
                replace(
                    block,
                    ops=(
                        *block.ops[:-1],
                        replace(last, exits=ordered),
                    ),
                )
            )
        else:
            marker = Op(
                block.at,
                ir.Operation.NOTHING,
                "",
                (),
                (),
                kind=Kind.NOTHING,
                source_backed=False,
                id=next(_IDS),
                exits=ordered,
            )
            blocks.append(replace(block, ops=(marker,)))
    return replace(body, blocks=tuple(blocks))


def exposed(body: MirBody) -> frozenset[Value]:
    """Values explicitly observable after control leaves this body.

    The raise records ABI-visible return and escape operands on their MIR
    operations.  Analyses therefore ask program semantics rather than which
    source register happened to contain a value at an exit.
    """
    return frozenset(
        value for block in body.blocks if not block.succ and block.ops for value in exit_values(block.ops[-1])
    )


def ordinary_uses(op: Op) -> tuple[Value, ...]:
    """Uses encoded or otherwise consumed by ``op`` itself."""
    return op.uses


def exit_values(op: Op) -> tuple[Value, ...]:
    """Values observable after ``op`` without being its machine operands."""
    return op.exits


def _public(body: MirBody) -> MirBody:
    """Drop the raise-only machine view at the public MIR boundary."""
    return MirBody(
        entry=body.entry,
        blocks=body.blocks,
        initial=body.initial,
        repetitions=body.repetitions,
        cloned=body.cloned,
        sealed=body.sealed,
        stack_in_data=body.stack_in_data,
        pointer_values=body.pointer_values,
        pointer_seeds=body.pointer_seeds,
        integer_ranges=body.integer_ranges,
        loop_trip_counts=body.loop_trip_counts,
    )


@dataclass(frozen=True, slots=True)
class AllocationHints:
    """Backend-only placement history, outside program semantics.

    A pass may renumber value occurrences while keeping the variable they are
    versions of.  An origin is only a preference, so keying it by that variable
    makes it survive ordinary SSA reconstruction without copying a physical
    register through MIR.  A pin is different: it is the fixed result of one
    source occurrence, not a requirement on every version of the variable.
    Its operation identity and result position survive SSA reconstruction and
    cloning without broadening that hard constraint to an unrelated phi.
    """

    origins: dict[int, Register_] = field(default_factory=dict)
    pins: dict[tuple[int, int], Register_] = field(default_factory=dict)

    @classmethod
    def from_body(cls, body: _RaisedBody) -> "AllocationHints":
        def variables(locations: dict[Value, Register_]) -> dict[int, Register_]:
            out: dict[int, Register_] = {}
            for value, location in locations.items():
                previous = out.setdefault(value.variable, location)
                if previous != location:
                    raise ValueError(
                        f"variable {value.variable} has conflicting allocation hints: {previous} and {location}"
                    )
            return out

        definitions = {
            value: (op.id, index)
            for block in body.blocks
            for op in block.ops
            if op.id is not None
            for index, value in enumerate(op.defines)
        }
        pins: dict[tuple[int, int], Register_] = {}
        for value, location in body.pins.items():
            key = definitions.get(value)
            if key is None:
                raise ValueError(f"pinned {value!r} has no source definition identity")
            previous = pins.setdefault(key, location)
            if previous != location:
                raise ValueError(f"definition {key} has conflicting allocation pins: {previous} and {location}")

        return cls(variables(body.origin), pins)

    def origin_of(self, value: Value) -> Register_ | None:
        return self.origins.get(value.variable)

    def pin_of(self, operation: Op, result: int) -> Register_ | None:
        return None if operation.id is None else self.pins.get((operation.id, result))


def _with_hints(body: MirBody, hints: AllocationHints) -> _RaisedBody:
    """Reattach placement only for raise-time recognition or legacy diagnostics.

    Production optimization never calls this inverse boundary.  It exists for
    frontend unit tests and the historical comparison tools that deliberately
    inspect BC's assignment.
    """
    values = {
        value
        for block in body.blocks
        for value in (
            *(phi.result for phi in block.phis),
            *(value for phi in block.phis for value in phi.incoming.values()),
            *(value for op in block.ops for value in (*op.defines, *op.uses, *op.exits)),
        )
    }
    origins = {value: where for value in values if (where := hints.origin_of(value)) is not None}
    pins = {
        value: where
        for block in body.blocks
        for op in block.ops
        for index, value in enumerate(op.defines)
        if (where := hints.pin_of(op, index)) is not None
    }
    return _RaisedBody(
        entry=body.entry,
        blocks=body.blocks,
        initial=body.initial,
        repetitions=body.repetitions,
        cloned=body.cloned,
        sealed=body.sealed,
        stack_in_data=body.stack_in_data,
        pointer_values=body.pointer_values,
        pointer_seeds=body.pointer_seeds,
        integer_ranges=body.integer_ranges,
        loop_trip_counts=body.loop_trip_counts,
        origin=origins,
        pins=pins,
    )


def _with_raise_context(body: MirBody, hints: AllocationHints, source: module.SourceMap) -> _RaisedBody:
    """Reconstruct the private raise view for focused frontend tests.

    Production never crosses the public boundary backwards.  A recognition
    test deliberately does: it disables one raising step, perturbs its input,
    and invokes that step directly.  Such a test needs both placement hints
    and the occurrence ranges that were externalized at the boundary.
    """

    def occurrence(op: Op) -> Op:
        spans = tuple(span for identity in op.absorbed for span in source.occurrences.get(identity, ()))
        node = source.nodes.get(op.id) if op.id is not None else None
        if not spans and node is None:
            return op
        return raising_occurrence(
            op,
            spans[0] if spans else (op.at, op.at),
            extra=spans[1:],
            node=node,
        )

    private = replace(
        body,
        blocks=tuple(replace(block, ops=tuple(occurrence(op) for op in block.ops)) for block in body.blocks),
    )
    return _with_hints(private, hints)


@dataclass(frozen=True, slots=True)
class RaisedBodies:
    """The raised bodies and their external machine-provenance side table.

    Sequence methods preserve the long-standing ``bodies()[0]`` and
    ``for ... in bodies()`` interface while making provenance an explicit
    result instead of an invisible mutation of the parsed module.
    """

    values: tuple[tuple[str, MirBody], ...]
    source: module.SourceMap
    hints: dict[int, AllocationHints]

    def __iter__(self) -> Iterator[tuple[str, MirBody]]:
        return iter(self.values)

    def __len__(self) -> int:
        return len(self.values)

    @overload
    def __getitem__(self, index: int) -> tuple[str, MirBody]: ...

    @overload
    def __getitem__(self, index: slice) -> tuple[tuple[str, MirBody], ...]: ...

    def __getitem__(self, index: int | slice) -> tuple[str, MirBody] | tuple[tuple[str, MirBody], ...]:
        return self.values[index]


def _opaque_effects(node: ir.Node | None) -> tuple[frozenset[str] | None, frozenset[str] | None]:
    """Effects on resources not represented by SSA values, without register identities."""

    def outside(registers) -> frozenset[str] | None:
        if registers is None:
            return None
        return frozenset(f"resource-{one}" for one in registers if one not in TRACKED)

    if node is None:
        return frozenset(), frozenset()
    return outside(node.effects.defs), outside(node.effects.uses)


def _raising_ranges(op: Op) -> tuple[tuple[int, int], ...]:
    """Concrete ownership while recognition is still inside the raise."""
    if not isinstance(op, _RaisedOp):
        return ()
    return (*((op.covers,) if op.covers is not None else ()), *op.extra_covers)


def _record_provenance(body: MirBody, source: module.SourceMap) -> tuple[int, ...]:
    """Record every raise-time occurrence before recognition removes or combines it."""
    recorded = []
    for block in body.blocks:
        for op in block.ops:
            if op.id is None:
                continue
            recorded.append(op.id)
            node = getattr(op, "node", None)
            if node is not None:
                source.nodes[op.id] = node
            spans = _raising_ranges(op)
            source.occurrences[op.id] = tuple(span for span in spans if span[0] < span[1])
    return tuple(recorded)


def _absorbed_ids(
    op: Op,
    source: module.SourceMap,
    candidates: tuple[int, ...],
    owned: tuple[tuple[int, int], ...] | None = None,
) -> tuple[int, ...]:
    """Raise-time occurrences wholly represented by this operation's owned ranges."""
    ranges = owned or _raising_ranges(op)
    ranges = tuple(span for span in ranges if span[0] < span[1])
    if not ranges:
        return op.absorbed

    def within(span: tuple[int, int]) -> bool:
        return any(low <= span[0] and span[1] <= high for low, high in ranges)

    found = tuple(
        identity
        for identity in candidates
        if (spans := source.occurrences.get(identity, ())) and all(within(span) for span in spans)
    )
    return tuple(dict.fromkeys((*op.absorbed, *found)))


def _completed_ownership(
    body: MirBody,
    source: module.SourceMap,
    candidates: tuple[int, ...],
    coverage: dict[int, tuple[tuple[int, int], ...]],
) -> MirBody:
    """Include disjoint folded-site ranges discovered after recognition."""
    if not coverage:
        return body
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    replace(op, absorbed=_absorbed_ids(op, source, candidates, coverage[op.id]))
                    if op.id in coverage
                    else op
                    for op in block.ops
                ),
            )
            for block in body.blocks
        ),
    )


def _externalized(body: MirBody, source: module.SourceMap, candidates: tuple[int, ...]) -> MirBody:
    """Move every decoded node out of a completed raise and into ``source``."""

    names = tuple(one.name for one in fields(Op))

    def operation(op: Op) -> Op:
        node = getattr(op, "node", None)
        if node is not None and op.id is not None:
            source.nodes[op.id] = node
        opaque_defs, opaque_uses = _opaque_effects(node)
        values = {name: getattr(op, name) for name in names}
        values.update(
            source_backed=node is not None,
            opaque_defs=opaque_defs,
            opaque_uses=opaque_uses,
            absorbed=_absorbed_ids(op, source, candidates),
        )
        return Op(**values)

    return replace(
        body,
        blocks=tuple(replace(block, ops=tuple(operation(op) for op in block.ops)) for block in body.blocks),
    )


class Unraisable(Exception):
    """A contract this cannot honour: a declared input with no value."""


def _call_args(routine: "runtime.Contract | None", holds: dict, at: int = 0) -> "tuple[tuple, bool]":
    """A call's declared arguments, as operands, in contract slot order.

    Only what the routine's own contract establishes. The read set a call
    carries is every tracked register wherever nothing is established --
    "every register is an input until it is" -- and that is a liveness
    dependency rather than an argument list. An operand here says which
    value and how wide; which register belongs to which slot is
    `runtime.slots`, and both sides ask it.
    """
    from qbopt.backend import target  # target reads mir; only needed when a call is raised

    if routine is None:
        # No runtime contract for this site, which is what a call to
        # another of the program's own procedures looks like: `found.calls`
        # names runtime routines and nothing else. Its interface is
        # unestablished for the same reason an unknown routine's is.
        return (), False
    if not runtime.established_inputs(routine):
        return (), False
    made = []
    for one in runtime.direct_slots(routine):
        value = holds.get(FROM_CONTRACT.get(one))
        width = target.width_of(AS_NAMED[one]) if one in AS_NAMED else None
        if value is None or width is None:
            # A declared slot with no value reaching it is a contract this
            # cannot honour, and returning nothing would say the routine
            # declares nothing -- which is a different fact and the one
            # that leaves the argument unconstrained.
            raise Unraisable(f"{routine.name} reads {one} and nothing reaches it")
        made.append(Held(value, width))
    return tuple(made), True


def _call_touches(
    name: str | None, routine: "runtime.Contract | None" = None
) -> tuple[frozenset[Register_], frozenset[Register_]] | None:
    """What a call really disturbs, where runtime.py has established it.

    ir.Effects answers "any register" for every call, which is the right
    answer for a module that knows nothing about the callee. Here it costs
    real precision: it gives si a fresh value across a routine that
    provably preserves it, so two accesses through the same si stop looking
    like the same address. runtime.py read the QuickBASIC 4.5 source for
    exactly this, and a routine with no entry there still comes back
    worst-case, so nothing is assumed by using it.
    """
    routine = routine if routine is not None else runtime.contract(name)
    if not routine.established and not runtime.established_inputs(routine):
        return None
    # The lowering's clobbers come from the same answer.
    changed = {FROM_CONTRACT[one] for one in runtime.disturbs(routine) if one in FROM_CONTRACT}
    disturbed = frozenset(one for one in TRACKED if one in changed) | {FLAGS}
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
    if not runtime.established_inputs(routine):
        # Established for what it clobbers and not for what it reads. Both
        # answers are needed and they are not the same question: B$ENRA and
        # B$EXSA preserve a documented set and their code is not in the
        # tree, so every register is an input until it is.
        return disturbed, frozenset(TRACKED) | {FLAGS}
    direct = routine.inputs if routine.direct_inputs is None else routine.direct_inputs
    reads = {FROM_CONTRACT[one] for one in direct if one in FROM_CONTRACT}
    return disturbed, frozenset(reads)


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


def _touched(
    node: ir.Node, calls: dict[int, str] | None = None, contracts: "dict[int, runtime.Contract] | None" = None
) -> tuple[frozenset[Register_], frozenset[Register_]]:
    """(defines, uses) as tracked variables, flags included as FLAGS.

    Reads ir.Effects rather than ir.Semantics, deliberately: Effects is
    iced's own conservative answer and is already rooted, which is exactly
    what an SSA variable has to be. A None there means "any register" -- a
    call, an interrupt, a barrier -- and becomes every tracked variable on
    both sides, which is what pins a barrier in place.
    """
    if isinstance(node, ir.Opaque) and node.insn.insn.code == Code.INTO:
        # On its fallthrough path INTO only observes OF. The exceptional
        # path remains a memory/control barrier, not fictitious GP results.
        return frozenset(), frozenset({FLAGS})
    if (
        calls is not None
        and isinstance(node, ir.Call)
        and (
            known := _call_touches(
                calls.get(node.insn.at), contracts.get(node.insn.at) if contracts is not None else None
            )
        )
    ):
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
        # One number per variable, in the order they are first written, and
        # a version counter for each. A register is a variable, so this is
        # what makes v3_1 and v3_7 legibly the same thing twice.
        self.named: dict[Register_, int] = {}
        self.versions: dict[Register_, int] = {}
        # Where BC had each value. Kept beside the values rather than on
        # them, so lowering and regalloc's identity baseline can ask and
        # nothing else picks it up for free.
        self.origin: dict[Value, Register_] = {}

    def fresh(self, of: Register_, at: int) -> Value:
        self.next += 1
        which = self.named.setdefault(of, len(self.named))
        self.versions[of] = self.versions.get(of, 0) + 1
        made = Value(self.next, at, of is FLAGS, which, self.versions[of])
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


def _memrefs(
    cells: tuple[ir.Mem, ...],
    namer: _Namer,
    at: int,
    slot: Addr | None = None,
    space: "Space | None" = None,
    beyond: "tuple[int, frozenset] | None" = None,
) -> tuple[MemRef, ...]:
    """ir.Mem cells, with the values their own address registers hold now.

    `space` is what the reference is in when nothing can name where: a push
    whose stack depth is unknown is still a push, and a push cannot land on
    a global. LLVM's PseudoSourceValue, and without it two thirds of the
    corpus's pushes aliased every named cell in their own body.
    """
    out = []
    for cell in cells:
        addr = slot if cell.addr is None and slot is not None else cell.addr
        base = segment = None
        base_width = 4
        root = ir.ROOT.get(cell.through, cell.through)
        if root in TRACKED:
            base = namer.current(root, at)
            base_width = RegisterExt.size(cell.through)
        if addr is not None:
            root = ir.ROOT.get(addr.base, addr.base)
            if root in TRACKED:
                base = namer.current(root, at)
                base_width = RegisterExt.size(addr.base)
            if addr.segment != Register.NONE:
                segment = None  # a segment register is physical, never a value
        out.append(MemRef(addr, cell.width, base, segment, space, beyond, base_width=base_width))
    return tuple(out)


def _placed(
    blocks: list[Block],
    nodes: dict[int, ir.Node],
    entry: int | None = None,
    calls: dict[int, str] | None = None,
    contracts: "dict[int, runtime.Contract] | None" = None,
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
            for one in _touched(node, calls, contracts)[0]:
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


# What a machine resource MIR cannot hold as a value is called. Named here
# so a pass that has to reason about one -- segments.py about `es`,
# fpstack.py about the x87 stack -- says the name and imports no register.
_RESOURCE: dict[Register_, str] = {
    getattr(Register, _one): _one.lower() for _one in ("ES", "DS", "SS", "CS", "FS", "GS") if hasattr(Register, _one)
}


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
                return Opaque(loc, _RESOURCE.get(loc.register, ""))
            return Held(value, loc.width)
        if isinstance(loc, ir.Imm):
            if loc.address is not None:
                address = loc.address
                return Symbol(address.space, address.index, address.disp, loc.width, loc.value)
            return Const(loc.value, loc.width)
        if (
            isinstance(loc, ir.Address)
            and loc.through == Register.BP
            and loc.index == Register.NONE
            and loc.addr is not None
            and loc.addr.space is Space.FRAME
            and what.dests
            and isinstance(what.dests[0], ir.Reg)
            and what.dests[0].width == 2
        ):
            return FrameAddress(loc.offset, 2)
        if isinstance(loc, ir.Mem):
            if not cells:
                return Opaque(loc)
            ref = cells.pop(0)
            root = ir.ROOT.get(loc.through, loc.through)
            if ref.base is None and root in TRACKED:
                ref = replace(ref, base=holds.get(root))
            return Cell(ref)
        if isinstance(loc, ir.St):
            return Opaque(loc, f"st{loc.index}")
        return Opaque(loc)

    read, write = list(loads), list(stores)
    return (
        tuple(one(loc, read, holds) for loc in what.sources),
        tuple(one(loc, write, written) for loc in what.dests),
    )


# Operation identity. Opaque and per-process: what it keys is a table
# built in the same call that hands the bodies out.
_IDS = itertools.count(1)


def _hands_back(kind: "Kind", written: dict, before: dict) -> "tuple | None":
    """(what the handed-back half defines, what it was, the answer it is of).

    An absorbed divide leaves its visible answer as one 32-bit value, and
    BC's own code reads a long as two 16-bit halves -- so the site ends by
    handing the high one over. Until that is said, the value it writes is
    one more register the call clobbered, and the two are not the same
    fact: a fold reading it as a clobber substitutes what the *previous*
    divide left there, which is the other answer's high half.

    RESTORE_PAIR says where the half lands, for the same pair the emitted
    sequence pushes. None where this is not a shape that hands one back.
    """
    from qbopt.legacy import calls as machine

    if kind not in (Kind.DIVMOD, Kind.MUL):
        return None
    source, into = RESTORE_PAIR[0]
    if machine.RESULT is not source:
        return None
    answer, was, now = written.get(source), before.get(into), written.get(into)
    if answer is None or was is None or now is None:
        return None
    return now, was, answer


def _handing_back(at: int, node: "ir.Node", now: "Value", was: "Value", answer: "Value") -> "Op":
    """The half a site hands back, as what it is rather than as a clobber.

    Synth.HALF_TO_LOW is MIR's own word for it: this value is the one
    before it with its low half replaced by the answer's high half. The
    answer is among what it reads, so substituting the answer carries to
    the half -- which is the point, and what was missing while the half
    read as a register the call happened to write.

    It owns none of BC's bytes. The site's range belongs to the divide,
    once; what this emits is four bytes that stand for no original ones,
    which is a different fact and the one `covers` is not for.

    The bit range is explicit, so optimization sees a value extraction.
    Lowering chooses the instructions; the old node retains provenance only.
    """
    return _RaisedOp(
        at,
        Synth.HALF_TO_LOW,
        "restore",
        (now,),
        (was, answer),
        (),
        (),
        node=ir.Restore(at=at, end=at, pair=0, effects=ir.RESTORE_EFFECTS[0]),
        kind=Kind.EXTRACT,
        args=(Held(answer, 4), Const(16, 4)),
        results=(Held(now, 2),),
        covers=(at, at),
        # `was` is the previous contents of the place the half lands in,
        # not an input. Said here because nothing else can say it: read as
        # an input it is a value crossing the divide in front of this, and
        # the allocator moved lngmix's accumulator out of dx to keep it
        # safe from a clobber that was never going to reach it.
        merges={was: now},
        id=next(_IDS),
    )


def raise_body(
    blocks: list[Block],
    nodes: dict[int, ir.Node],
    entry: int | None = None,
    calls: dict[int, str] | None = None,
    sites: dict | None = None,
    unreached: "tuple[int, frozenset] | None" = None,
    contracts: "dict[int, runtime.Contract] | None" = None,
    spared: "dict[int, tuple] | None" = None,
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
    # One contract per call site, and the same one the lowering reads.
    # Built here only for a caller with none to give -- a tool, a test.
    chosen = contracts if contracts is not None else runtime.per_call(calls or {})
    handles_errors = any(contract.error_handling for contract in chosen.values())
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

    needed = _placed(blocks, nodes, start, calls, chosen)
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

        inside = _within(sites or {})
        for insn in block.insns:
            node = nodes.get(insn.at)
            if node is None:
                continue
            # A push that belongs to an absorbable call goes with the call.
            # What the site computes is one operation over its argument
            # values, and the pushes are how BC handed them to a routine
            # that no longer runs.
            if insn.at in inside:
                offset, _slot = _stack_slot(node, offset, None)
                continue
            offset, slot = _stack_slot(node, offset, calls.get(insn.at) if calls else None)
            defines, uses = _touched(node, calls, chosen)
            used = tuple(namer.current(one, start) for one in sorted(uses, key=lambda o: (o is not FLAGS, o)))
            from qbopt.frontend import raising_call_memory

            contract = chosen.get(insn.at)
            read_reach = raising_call_memory.reachable(
                contract, contract.reads if contract else runtime.Memory.ANY, unreached, handles_errors
            )
            write_reach = raising_call_memory.reachable(
                contract, contract.writes if contract else runtime.Memory.ANY, unreached, handles_errors
            )
            loading_stack = node.semantics.op is ir.Operation.POP
            pushing = node.semantics.op is ir.Operation.PUSH
            # A call stores twice: its return address, which is a push spelled
            # differently, and whatever the callee writes, which `write_reach`
            # bounds. One reference cannot say both. Named STACK, the callee's
            # writes vanished and a user SUB wrote nothing; named nowhere, the
            # return address reached every cell in the program. Both carry the
            # call's bound: the return address lands where the callee's own
            # pushes do, and without it every frame slot died at every call.
            calling = node.semantics.op is ir.Operation.CALL
            loads = _memrefs(
                node.effects.loads,
                namer,
                start,
                slot if loading_stack else None,
                Space.STACK if loading_stack else None,
                read_reach,
            )
            stores = _memrefs(
                node.effects.stores,
                namer,
                start,
                slot if pushing else None,
                Space.STACK if pushing or calling else None,
                write_reach,
            )
            if calling:
                stores += (MemRef(None, 4, None, None, None, write_reach, excludes=(spared or {}).get(insn.at, ())),)
            # For a call the decoder's footprint describes only the CALL
            # instruction.  The selected contract supplies the callee's real
            # footprint.  Both halves must be bounded before an empty list can
            # mean "none" rather than "unknown" to MemorySSA and its users.
            memory_complete = node.effects.memory_complete or (
                calling and read_reach is not None and write_reach is not None
            )
            holds = dict(zip(sorted(uses, key=lambda o: (o is not FLAGS, o)), used))
            # What each variable held before this instruction writes
            # anything. The half an absorbed divide hands back is a
            # read-modify-write of the variable it lands in, and `uses`
            # does not carry that: a routine's contract names what it
            # reads, and it does not read the register its own answer's
            # high half goes to.
            before = {one: namer.current(one, start) for one in TRACKED}
            made = []
            for one in sorted(defines, key=lambda o: (o is not FLAGS, o)):
                value = namer.fresh(one, insn.at)
                namer.stack.setdefault(one, []).append(value)
                pushed.append(one)
                made.append(value)
            written = dict(zip(sorted(defines, key=lambda o: (o is not FLAGS, o)), made))
            where = _operands(node.semantics, holds, written, loads, stores)
            loads = tuple(one.ref for one in where[0] if isinstance(one, Cell)) or loads
            stores = tuple(one.ref for one in where[1] if isinstance(one, Cell)) or stores
            kind = _kind_of(node.semantics, where[0], where[1])
            operands = _normalised(kind, node.semantics.name or "", where[0], where[1])
            site = (sites or {}).get(insn.at)
            covers, where_at = ir.span(node), insn.at
            handed: tuple | None = None
            if site is not None:
                folded = _absorbing(site, written)
                if folded is not None:
                    kind, operands, where = folded[0], folded[1], (folded[1], folded[2])
                    # And it stands for the pushes too. layout refuses a body
                    # it cannot account for every byte of -- rightly, since
                    # that is how it catches data BC put between the
                    # instructions -- so a fold says what it replaced.
                    #
                    # Its address is where those bytes begin, not where the
                    # call was: a loop whose back edge lands on the first
                    # push has to find something there, and lngmix's does.
                    covers = (site.start, site.end)
                    where_at = site.start
                    handed = _hands_back(kind, written, before)
                    # And it reads what its operands name and writes no
                    # memory at all. The effects are iced's answer about
                    # the `call` -- everything the reached set does not
                    # cover, in both directions -- and carrying them past
                    # the fold said a divide of two static cells might
                    # land on any of them: licm refused lngmix's loop
                    # because the operation it had already turned into
                    # arithmetic still claimed a call's memory.
                    loads = _absorbed_loads(where[0], namer, start)
                    references = iter(loads)
                    operands = tuple(Cell(next(references)) if isinstance(one, Cell) else one for one in operands)
                    where = operands, where[1]
                    used = tuple(
                        dict.fromkeys(
                            [arg.value for arg in operands if isinstance(arg, Held)]
                            + [value for ref in loads for value in (ref.base, ref.segment) if value is not None]
                        )
                    )
                    stores = ()
            _called = _call_args(chosen.get(insn.at), holds, insn.at) if kind is Kind.CALL else ((), True)
            # The snapshot the raise took, arguments included. `semantics`
            # carries an operation's own bytes only while its operands are
            # the ones it was raised with, and naming what a call reads is
            # bookkeeping rather than a rewrite -- recorded in one and not
            # the other, every residual call read as rewritten and select
            # cannot encode a call.
            _args = _called[0] if kind is Kind.CALL else operands
            # The half the site hands back belongs to the operation that
            # says what it is, not to the divide: read as one of the
            # divide's clobbers it is indistinguishable from a register
            # left alone, and a fold then puts the quotient's high half
            # where the remainder's belongs.
            kept = tuple(one for one in made if handed is None or one is not handed[0])
            ops[at].append(
                _RaisedOp(
                    where_at,
                    Synth.HALF_TO_LOW if isinstance(node, ir.Restore) else node.semantics.op,
                    node.semantics.name or "",
                    kept,
                    used,
                    loads,
                    stores,
                    node=node,
                    kind=kind,
                    merges=_merged(node.semantics, holds, written, where[0]),
                    covers=covers,
                    stack=_stack_effect(node.semantics),
                    test=_BY_BRANCH.get(node.semantics.name or "") if kind is Kind.BRANCH else None,
                    args=_args,
                    results=where[1],
                    raised=(_args, where[1]),
                    target=node.semantics.target,
                    id=next(_IDS),
                    args_known=_called[1],
                    memory_complete=memory_complete,
                    reads_complete=node.effects.uses is not None and _called[1],
                )
            )
            if handed is not None:
                ops[at].append(_handing_back(where_at, node, *handed))

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
    return _RaisedBody(
        entry=start,
        blocks=tuple(
            MirBlock(
                block.at,
                tuple(phis[block.at].values()),
                tuple(ops[block.at]),
                tuple(one for one in block.succ if one in reachable),
            )
            for block in blocks
        ),
        origin=dict(namer.origin),
    )


def unheld(op: Op) -> tuple[frozenset | None, frozenset | None]:
    """(written, read) of opaque resources; None means every resource."""
    return op.opaque_defs, op.opaque_uses


def _rebased(refs: tuple[MemRef, ...], namer: "_Namer", at: int) -> tuple[MemRef, ...]:
    """The same cells, holding whichever value reaches them now."""
    out = []
    for ref in refs:
        base = None
        if ref.addr is not None:
            root = ir.ROOT.get(ref.addr.base, ref.addr.base)
            if root in TRACKED:
                base = namer.current(root, at)
        out.append(replace(ref, base=base))
    return tuple(out)


class _Renamer:
    """Fresh values per MIR variable, and which one each currently holds."""

    def __init__(self) -> None:
        self.next = 0
        self.stack: dict[int, list[Value]] = {}
        self.versions: dict[int, int] = {}

    def fresh(self, variable: int, at: int, flags: bool = False) -> Value:
        self.next += 1
        self.versions[variable] = self.versions.get(variable, 0) + 1
        return Value(self.next, at, flags, variable, self.versions[variable])

    def current_of(self, variable: int, at: int) -> Value:
        held = self.stack.setdefault(variable, [])
        if not held:
            held.append(self.fresh(variable, at))
        return held[-1]

    def current(self, one: Value, at: int) -> Value:
        held = self.stack.setdefault(one.variable, [])
        if not held:
            held.append(self.fresh(one.variable, at, one.flags))
        return held[-1]


def _renamed_arg(one, swap: dict, refs: dict):
    """One operand with its value replaced by the version in scope."""
    if isinstance(one, Held) and one.value.variable in swap:
        return Held(swap[one.value.variable], one.width)
    if isinstance(one, Cell) and one.ref in refs:
        return Cell(refs[one.ref])
    return one


def _rehomed(ref: "MemRef", namer: "_Renamer", at: int) -> "MemRef":
    """A cell's own base and segment, at the versions in scope."""
    base = namer.current(ref.base, at) if ref.base is not None else None
    segment = namer.current(ref.segment, at) if ref.segment is not None else None
    if base is ref.base and segment is ref.segment:
        return ref
    return replace(ref, base=base, segment=segment)


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

    # Keyed on the MIR variable, never on a source register. Two values that
    # happened to occupy one register are not one variable after a transform
    # separates their computations.
    frontier = loops.frontiers(blocks, start)

    where: dict[int, set[int]] = {}
    for block in blocks:
        for op in block.ops:
            for value in op.defines:
                where.setdefault(value.variable, set()).add(block.at)
    needed: dict[int, set[int]] = {block.at: set() for block in blocks}
    for variable, defined in where.items():
        pending = list(defined)
        seen: set[int] = set()
        while pending:
            for join in frontier.get(pending.pop(), frozenset()):
                if join not in seen:
                    seen.add(join)
                    needed[join].add(variable)
                    pending.append(join)

    namer = _Renamer()
    phis: dict[int, dict[int, Phi]] = {block.at: {} for block in blocks}
    out: dict[int, list[Op]] = {block.at: [] for block in blocks}
    renamed: dict[Value, set[Value]] = {}

    def remember(old: Value | None, new: Value | None) -> None:
        if old is not None and new is not None:
            renamed.setdefault(old, set()).add(new)

    flagged = {value.variable for block in blocks for op in block.ops for value in op.defines if value.flags}
    for block in blocks:
        for variable in sorted(needed[block.at], key=lambda one: (one not in flagged, one)):
            phis[block.at][variable] = Phi(namer.fresh(variable, block.at, variable in flagged), {})
        for old in block.phis:
            made = phis[block.at].get(old.result.variable)
            if made is not None:
                remember(old.result, made.result)

    def rename(at: int) -> None:
        block = by_at[at]
        pushed: list[int] = []
        for variable, phi in phis[at].items():
            namer.stack.setdefault(variable, []).append(phi.result)
            pushed.append(variable)

        for op in block.ops:
            used = tuple(namer.current(one, start) for one in op.uses)
            exits = tuple(namer.current(one, start) for one in op.exits)
            for old, new in zip(op.uses, used, strict=True):
                remember(old, new)
            for old, new in zip(op.exits, exits, strict=True):
                remember(old, new)
            swap = {one.variable: now for one, now in zip(op.uses, used)}
            loads = tuple(_rehomed(one, namer, start) for one in op.loads)
            stores = tuple(_rehomed(one, namer, start) for one in op.stores)
            for old, new in zip((*op.loads, *op.stores), (*loads, *stores), strict=True):
                remember(old.base, new.base)
                remember(old.segment, new.segment)
            refs = dict(zip((*op.loads, *op.stores), (*loads, *stores)))
            fresh = []
            for one in op.defines:
                value = namer.fresh(one.variable, op.at, one.flags)
                namer.stack.setdefault(one.variable, []).append(value)
                pushed.append(one.variable)
                fresh.append(value)
                remember(one, value)
            made = {one.variable: now for one, now in zip(op.defines, fresh)}
            out[at].append(
                replace(
                    op,
                    defines=tuple(fresh),
                    uses=used,
                    exits=exits,
                    loads=loads,
                    stores=stores,
                    args=tuple(_renamed_arg(one, swap, refs) for one in op.args),
                    results=tuple(_renamed_arg(one, made, refs) for one in op.results),
                    # And what it was raised as, at the same versions. It is
                    # the two compared that say whether a pass rewrote the
                    # operation, so renaming one and not the other makes
                    # every operation look rewritten -- and lowering then
                    # re-encodes what it should have emitted verbatim.
                    raised=None
                    if op.raised is None
                    else (
                        tuple(_renamed_arg(one, swap, refs) for one in op.raised[0]),
                        tuple(_renamed_arg(one, made, refs) for one in op.raised[1]),
                    ),
                    merges={swap.get(a.variable, a): made.get(b.variable, b) for a, b in op.merges.items()},
                )
            )

        for successor in block.succ:
            for variable, phi in phis.get(successor, {}).items():
                phi.incoming[at] = namer.current_of(variable, start)

        for child in sorted(children[at]):
            rename(child)
        for variable in reversed(pushed):
            namer.stack[variable].pop()

    rename(start)
    resolved_blocks = tuple(
        MirBlock(
            block.at,
            tuple(phis[block.at].values()),
            tuple(out[block.at]),
            tuple(one for one in block.succ if one in reachable),
            block.cold,
        )
        for block in blocks
    )
    pointer_values = frozenset(new for old in body.pointer_values for new in renamed.get(old, ()))
    pointer_seeds: dict[Value, memory.Provenance] = {}
    conflicting: set[Value] = set()
    for old, provenance in body.pointer_seeds.items():
        for new in renamed.get(old, ()):
            if new in pointer_seeds and pointer_seeds[new] != provenance:
                conflicting.add(new)
            else:
                pointer_seeds[new] = provenance
    for value in conflicting:
        del pointer_seeds[value]
    integer_ranges: dict[Value, IntegerRange] = {}
    range_conflicts: set[Value] = set()
    for old, interval in body.integer_ranges.items():
        for new in renamed.get(old, ()):
            if new in integer_ranges and integer_ranges[new] != interval:
                range_conflicts.add(new)
            else:
                integer_ranges[new] = interval
    for value in range_conflicts:
        del integer_ranges[value]
    return MirBody(
        entry=start,
        blocks=resolved_blocks,
        initial=body.initial,
        repetitions=body.repetitions,
        cloned=body.cloned,
        sealed=body.sealed,
        stack_in_data=body.stack_in_data,
        pointer_values=pointer_values | frozenset(pointer_seeds),
        pointer_seeds=pointer_seeds,
        integer_ranges=integer_ranges,
        loop_trip_counts=body.loop_trip_counts,
    )


def verify(body: MirBody) -> list[str]:
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

    Over the body's own blocks, not the machine code's: the raise splits a
    jump table's dispatch into blocks BC never had, and against BC's graph
    every phi after one names predecessors that do not exist.
    """
    doms = loops.dominators(list(body.blocks), body.entry)
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

    preds = loops.predecessors(list(body.blocks))
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
        pending = {value for op in block.ops for value in op.defines}
        for op in block.ops:
            for value in op.uses:
                where = defined_at.get(value)
                if where is None:
                    continue  # defined by the caller, in scope everywhere
                if value in pending:
                    problems.append(f"{op.at:#06x} uses {value} before its definition in {block.at:#06x}")
                elif where not in doms.get(block.at, frozenset()):
                    problems.append(f"{op.at:#06x} uses {value}, defined in {where:#06x}, which does not dominate it")
            pending.difference_update(op.defines)
            for value in op.exits:
                where = defined_at.get(value)
                if where is not None and where not in doms.get(block.at, frozenset()):
                    problems.append(
                        f"{op.at:#06x} exposes {value}, defined in {where:#06x}, which does not dominate it"
                    )
    return problems


def lower(body: MirBody, nodes: dict[int, object] | None = None) -> tuple[ir.Node, ...]:
    """The nodes this body is made of, in address order.

    With nothing transformed this is exactly what was raised, so emitting it
    gives back the bytes it came from. That is the whole point of keeping the
    decoded occurrences in the raise's external source map: the identity case
    is checkable before any transform exists, which is the only moment the
    machinery can be trusted for free.
    Once something does change a body, this is where a real instruction
    selector goes, and this round trip is what it will be measured against.

    A phi emits nothing. It is not an instruction and never was -- it names
    where two definitions of a register met, which BC's own code said by
    writing the same register on both paths. Only a lowering that has
    actually split those definitions into different registers has to put
    anything back, and nothing here does yet.
    """
    source = nodes or {}
    return tuple(
        node
        for block in body.blocks
        for op in block.ops
        if (node := source.get(op.id) if op.source_backed and op.id is not None else getattr(op, "node", None))
        is not None
    )


def relowered(found: Module, body: MirBody, nodes: dict[int, object] | None = None) -> bytes:
    """This body's own bytes, rebuilt from the graph."""
    return ir.emit(found, lower(body, nodes))


def same_bytes(one: MemRef, other: MemRef) -> bool:
    """Whether two references certainly name the same bytes.

    Keyed on the base *value*, never on which register holds it. That is
    the whole difference between this and regions.addresses: an Addr says
    `[si+6]`, and the moment anything reallocates registers that name is
    about a register which may now hold something else, while the value it
    stood for is still the same value. Two references agree here because
    the same computation produced their offset, which no allocation can
    change.

    Certainly, not possibly -- this answers the forwarding question ("is
    this the load I already did"), and its negation is not a disjointness
    proof. `regions` still answers that one.
    """
    if one.provenance is not None and other.provenance is not None and one.provenance != other.provenance:
        return False
    if one.pointer or other.pointer:
        return (
            one.pointer
            and other.pointer
            and one.base is not None
            and one.base == other.base
            and one.width == other.width
            and one.addr is None
            and other.addr is None
            and one.segment is None
            and other.segment is None
            and one.base_width == other.base_width
        )
    one, other = _symbolic_ref(one), _symbolic_ref(other)
    if one.addr is None or other.addr is None:
        return False  # nothing this can name is never known to be anything
    if (
        one.addr.space is Space.FAR
        and one.segment is None
        and not (one.allocation is not None and one.allocation == other.allocation)
    ):
        return False
    if one.width != other.width or one.base != other.base or one.segment != other.segment:
        return False
    return one.addr == other.addr


def _unescaped(ref: MemRef) -> bool:
    """Whether only a reference naming its objects can reach what `ref` names."""
    return (
        ref.provenance is not None
        and bool(ref.provenance.slices)
        and not any(one.object.addressed or one.object.kind is memory.Kind.ABSOLUTE for one in ref.provenance.slices)
    )


def _through_pointer(ref: MemRef) -> bool:
    """Whether `ref`'s address is a value rather than a named object plus an index."""
    return (ref.base is not None or ref.segment is not None) and (
        ref.addr is None or ref.addr.space in (Space.LITERAL, Space.FAR)
    )


def overlapping(
    one: MemRef,
    other: MemRef,
    dgroup: frozenset[int],
    bounds: dict | None = None,
    known: dict | None = None,
    other_known: dict | None = None,
) -> bool:
    """Whether a write through `other` could land on `one`.

    `regions` for the symbolic part, and the base value for the rest.
    Where both name the same base value their displacements settle it by
    arithmetic, exactly as two bare statics do -- and soundly for the same
    reason memory.aliases() gives, except that this holds it by value
    identity rather than by the caller having promised the register was not
    written in between.
    """
    if regions.typed_apart(one, other):
        return False
    # Canonical references carry their own object identity. Keep their base
    # value available to the canonical range query; the legacy covering
    # rewrite below erases it after widening the address to a byte hull.
    if one.provenance is not None and other.provenance is not None:
        apart = None if one.pointer or other.pointer else _displaced(_symbolic_ref(one), _symbolic_ref(other))
        return not apart if apart is not None else regions.may_alias(one, other, bounds, known, other_known, dgroup)
    if (_unescaped(one) and _through_pointer(other)) or (_unescaped(other) and _through_pointer(one)):
        return False
    if not (one.pointer or other.pointer):
        if known or other_known:
            from qbopt.analysis import ranges

            one, other = ranges.covering(one, known or {}), ranges.covering(other, other_known or {})
        one, other = _symbolic_ref(one), _symbolic_ref(other)
        apart = _displaced(one, other)
        if apart is not None:
            return not apart
    return regions.may_alias(one, other, bounds, known, other_known, dgroup)


def overlap_bucket(ref: MemRef) -> tuple:
    """What `overlapping` needs of a cell to rule a write out unseen: its one
    object (None if it has no single one), the frame `_displaced` compares
    displacements in (None for a pointer), and the object's alias class."""
    slices = ref.provenance.slices if ref.provenance is not None else ()
    one = next(iter(slices)).object if len(slices) == 1 else None
    return object_bucket(one, _frame(ref))


def object_bucket(one: "memory.Object | None", frame: tuple | None) -> tuple:
    return (one, frame, None if one is None else memory.alias_class(one))


def overlap_buckets(ref: MemRef, cells) -> set | None:
    """The buckets of `cells` (a CellMap keyed by `object_bucket`) a write
    through `ref` may reach; None for all of them.

    Only these can hold a cell `overlapping` does not rule out: one whose
    object is unknown, one in the write's `_displaced` frame, one in the
    write's own object, and one whose alias class may alias the write's.
    """
    if ref.provenance is None:
        return None
    if not cells:
        return set()
    objects, frames, classes = cells.parts
    reached = set(objects.get(None, ()))
    if (frame := _frame(ref)) is not None and frame in frames:
        reached |= frames[frame]
    written, kinds = _write_reach(ref.provenance)
    for one in written:
        if one in objects:
            reached |= objects[one]
    for kind, buckets in classes.items():
        if kind is not None and _classes_reach(kinds, kind):
            reached |= buckets
    return reached


@functools.lru_cache(maxsize=1 << 12)
def _write_reach(provenance: memory.Provenance) -> tuple[frozenset, frozenset]:
    """A write's objects and their alias classes."""
    written = frozenset(one.object for one in provenance.slices)
    return written, frozenset(memory.alias_class(one) for one in written)


@functools.lru_cache(maxsize=1 << 12)
def _classes_reach(kinds: frozenset, kind: memory.AliasClass) -> bool:
    return any(memory.classes_may_alias(one, kind) for one in kinds)


def _frame(ref: MemRef) -> tuple | None:
    if ref.pointer:
        return None
    if ref.symbolic is not None:
        return (None, None, ref.symbolic.space, ref.symbolic.index)
    if ref.addr is None:
        return None
    return (ref.base, ref.segment, ref.addr.space, ref.addr.index)


def _displaced(one: MemRef, other: MemRef) -> bool | None:
    """Whether two references off one base value are disjoint by displacement; None if not one base.

    LLVM's constant-offset GEP compare: a fact about values, so it holds
    whatever object either reference names.
    """
    one, other = _span(one), _span(other)
    if one is None or other is None or one[0] != other[0]:
        return None
    return not (one[1] < other[2] and other[1] < one[2])


def _span(ref: MemRef) -> tuple[tuple, int, int] | None:
    """The frame `_displaced` measures `ref` in and the bytes [low, high) it covers there; `ref` has no symbol."""
    addr = ref.addr
    if addr is None or (addr.space is Space.FAR and ref.segment is None):
        return None
    return (ref.base, ref.segment, addr.space, addr.index), addr.disp, addr.disp + ref.width


def displaced_span(ref: MemRef) -> tuple[tuple, int, int] | None:
    """The frame and bytes where `overlapping` settles `ref` against every
    reference of that frame by displacement alone: one whose bytes miss
    these cannot overlap it.

    None for a pointer, and for an address off a base value, which
    `ranges.covering` may widen before the displacements are compared.
    """
    if ref.pointer:
        return None
    span = _span(_symbolic_ref(ref))
    return span if span is not None and span[0][0] is None else None


def overlap_span(ref: MemRef) -> tuple[int, int] | None:
    """`ref`'s bytes as `displaced_span` names them, for a ``CellMap``'s `span_of`."""
    span = displaced_span(ref)
    return None if span is None else span[1:]


def displaced_buckets(ref: MemRef, cells) -> tuple[set, int, int] | None:
    """The buckets of `cells` (a CellMap keyed by `object_bucket`) in the
    frame of `ref`'s `displaced_span`, and its bytes: a cell there that the
    bytes miss is one a write through `ref` cannot reach."""
    if not cells or (span := displaced_span(ref)) is None or (buckets := cells.parts[1].get(span[0])) is None:
        return None
    return buckets, span[1], span[2]


def _symbolic_ref(ref: MemRef) -> MemRef:
    if ref.symbolic is None:
        return ref
    symbol = ref.symbolic
    return replace(
        ref, addr=module.Addr(symbol.space, symbol.offset + symbol.addend, symbol.index), base=None, segment=None
    )


# Every bp-relative displacement there is. `beyond` bounds a call inside the
# program's data segment and says nothing about the frame, which is a
# different region -- so a call proven to reach no caller variable still
# aliased every local, and the counter of every loop holding one round-tripped
# through its slot. Written as an exclusion rather than a flag because that is
# what it is, and one kind of negative fact is enough.
WHOLE_FRAME = (module.Addr(Space.FRAME, -(1 << 15)), 1 << 16)


def _outside(reach) -> tuple[tuple[Addr, int], ...]:
    """The frame's bytes no range in `reach` covers, as exclusions."""
    low, high = WHOLE_FRAME[0].disp, WHOLE_FRAME[0].disp + WHOLE_FRAME[1]
    out, at = [], low
    for start, end in sorted(reach):
        if start > at:
            out.append((module.Addr(Space.FRAME, at), start - at))
        at = max(at, end)
    if at < high:
        out.append((module.Addr(Space.FRAME, at), high - at))
    return tuple(out)


def _through_frame(body: MirBody, framed: dict) -> MirBody:
    """Every reference through an address into frame objects, as reaching only them."""
    if not framed:
        return body

    def tag(ref: MemRef) -> MemRef:
        if (
            ref.base in framed
            and ref.segment is None
            and not ref.pointer
            and ref.addr is not None
            and ref.addr.space is Space.LITERAL
        ):
            return replace(ref, within=tuple(sorted(framed[ref.base])))
        return ref

    def operand(one):
        return Cell(tag(one.ref)) if isinstance(one, Cell) else one

    blocks = []
    for block in body.blocks:
        ops = tuple(
            replace(
                op,
                loads=tuple(map(tag, op.loads)),
                stores=tuple(map(tag, op.stores)),
                args=tuple(map(operand, op.args)),
                results=tuple(map(operand, op.results)),
            )
            if any(ref.base in framed for ref in (*op.loads, *op.stores))
            else op
            for op in block.ops
        )
        blocks.append(replace(block, ops=ops))
    return replace(body, blocks=tuple(blocks))


def _frame_bounded(body: MirBody, pointers: bool = False) -> MirBody:
    """Exclude this body's frame slots from every bounded effect.

    Runs once the body is complete because the escape set is a fact about
    the whole body, and the references that carry the bound are built while
    it is still being assembled.

    `pointers` bounds every cell reached through a value as well: a source
    language whose pointers reach this frame only through an address the
    body itself took. C is one; BC, which walks frames, is not.
    """
    from qbopt.analysis import frameescape

    if pointers:
        body = _through_frame(body, frameescape.framed(body))
    # Only the bytes no escaped address can reach: an address the raise bounds
    # to its object reaches that object, and one it does not bound reaches all.
    escapes = frameescape.analysed(body)
    if escapes.opaque_addresses or escapes.reach is None:
        return body
    holes = _outside(escapes.reach)
    if not holes:
        return body

    def bounded(ref: MemRef) -> bool:
        if holes[0] in ref.excludes:
            return False
        if ref.beyond is not None:
            return True
        # No address is a callee's reach: its own data or what a pointer leads to.
        reached = ref.base is not None or ref.segment is not None or ref.pointer or ref.addr is None
        return pointers and reached and ref.where not in (Space.FRAME, Space.SEGMENT, Space.EXTERNAL, Space.STACK)

    def bound(ref: MemRef) -> MemRef:
        return replace(ref, excludes=ref.excludes + holes) if bounded(ref) else ref

    blocks = []
    for block in body.blocks:
        ops = [
            replace(op, loads=tuple(bound(one) for one in op.loads), stores=tuple(bound(one) for one in op.stores))
            if any(bounded(one) for one in (*op.loads, *op.stores))
            else op
            for op in block.ops
        ]
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def _unreached(found: Module) -> "tuple[int, frozenset] | None":
    """What a runtime call can reach inside this program's own data.

    The runtime writes its own data at fixed addresses and never a cell in
    BC_DATA -- `tools/runtime_writes.py` measures that on three linked
    images. So a call reaches what the program handed it a pointer to, and
    nothing else in that segment.

    None where there is no such segment, which says nothing and is what
    every object without debug information gets.
    """
    if found.program_data is None:
        return None
    return (found.program_data, module.escaped(found))


def _copied_word(arg, definitions):
    """Follow only exact word copies to their common source."""
    seen = set()
    while isinstance(arg, Held) and arg.width == 2 and arg.value not in seen:
        seen.add(arg.value)
        copy = definitions.get(arg.value)
        if (
            copy is None
            or copy.kind is not Kind.COPY
            or copy.loads
            or copy.stores
            or copy.barrier
            or copy.results != (arg,)
            or len(copy.args) != 1
            or not isinstance(copy.args[0], Held)
            or copy.args[0].width != arg.width
        ):
            break
        arg = copy.args[0]
    return arg


def extracted_whole(high, low, definitions):
    """The scalar whose exact high and low words are these operands."""
    original = None
    for arg, offset in ((high, 16), (low, 0)):
        arg = _copied_word(arg, definitions)
        if not isinstance(arg, Held) or arg.width != 2:
            return None
        if offset == 0 and original is not None:
            extension = definitions.get(original.value)
            if (
                extension is not None
                and extension.kind is Kind.SIGN_EXTEND
                and len(extension.args) == 1
                and _copied_word(extension.args[0], definitions) == arg
                and extension.results == (original,)
                and not extension.loads
                and not extension.stores
                and not extension.barrier
            ):
                return original
        op = definitions.get(arg.value)
        if (
            op is None
            or op.kind is not Kind.EXTRACT
            or op.results != (arg,)
            or len(op.args) != 2
            or not isinstance(op.args[0], Held)
            or op.args[0].width != 4
            or not isinstance(op.args[1], Const)
            or op.args[1].n != offset
        ):
            return None
        if original is not None and original != op.args[0]:
            return None
        original = op.args[0]
    return original


def bodies(
    found: Module,
    blocks: list[Block],
    contracts: "dict[int, runtime.Contract] | None" = None,
    *,
    basic_semantics: bool = False,
    bounds_checks: bool = False,
) -> RaisedBodies:
    """Every body in the module, raised, labelled, and skipping what will not.

    One place rather than three: dump.py, the measurement scripts and now
    rewrite.py all need the same walk, and the part worth not rewriting
    twice is the block-to-body assignment -- a procedure is reached by a
    call, which is not a CFG edge, so raise_body() has to be handed one
    body's blocks and no others.
    """
    source = module.SourceMap.from_module(found)
    unreached = _unreached(found)
    result = ir.decode_module(found)
    if isinstance(result, str):
        return RaisedBodies((), source, {})
    decoded_nodes = {ir.span(node)[0]: node for body in result for node in body.nodes}
    from qbopt.frontend import blocks as split
    from qbopt.frontend import raising_returns

    header = split.has_header(found)
    nodes = {at: raising_returns.returned(node, header) for at, node in decoded_nodes.items()}
    return_registers = {}
    if header:
        # $$SYMBOLS cannot tell a SUB from an implicit-INTEGER FUNCTION, but
        # its procedure signature does establish whether DX participates in
        # the return representation.  Both ambiguous cases are AX-only, so
        # the useful half of the fact remains exact.  Objects without debug
        # types keep the conservative DX:AX default above.
        from qbopt.objectfile import cvinfo

        for procedure in cvinfo.parse(found.records).procedures:
            if procedure.signature is None:
                continue
            returned_type = cvinfo.type_name(procedure.signature.return_type, procedure.types)
            return_registers[procedure.offset] = (
                (Register.AX, Register.DX) if returned_type == "LONG" else (Register.AX,)
            )
    if contracts is None:
        # The module's own toolchain, which is where a per-family contract
        # is chosen and the only place a family is read at all.
        contracts = runtime.for_module(found)
    # B$EXSA preserves the procedure's return registers across frame teardown.
    # Its generic contract must cover a LONG result, but a typed procedure's
    # syntactic continuation observes only the established subset.  Specialize
    # the per-site direct edge; hidden/error paths retain Contract.inputs.
    for body in result:
        registers = return_registers.get(body.body.seed)
        if registers is None:
            continue
        direct = frozenset(
            register
            for register, machine in ((runtime.Reg.AX, Register.AX), (runtime.Reg.DX, Register.DX))
            if machine in registers
        )
        for at, name in found.calls.items():
            if name == "B$EXSA" and any(lo <= at < hi for lo, hi in body.body.ranges):
                contracts[at] = replace(contracts[at], direct_inputs=direct)
    from qbopt.frontend import raising_call_memory

    spared = raising_call_memory.spared(found, result, contracts)
    out: list[tuple[str, _RaisedBody]] = []
    error_handlers: list[_RaisedBody] = []
    for body in result:
        mine = [one for one in blocks if any(lo <= one.at < hi for lo, hi in body.body.ranges)]
        procedure_nodes = nodes
        if body.body.seed in return_registers:
            registers = return_registers[body.body.seed]
            procedure_nodes = {
                at: raising_returns.returned(node, header, registers) for at, node in decoded_nodes.items()
            }
        from qbopt.frontend import raising_control

        mine = raising_control.terminal_edges(mine, contracts)
        if not mine:
            continue
        from qbopt.frontend import raising_carried

        # In place: the lowering reads this same map, and must pin what the
        # raise made an input.
        contracts.update(raising_carried.carried(mine, nodes, found.calls, contracts))
        built = raise_body(
            mine,
            procedure_nodes,
            body.body.seed,
            found.calls,
            _sites(found, blocks),
            unreached,
            contracts,
            spared,
        )
        if not isinstance(built, str):
            provenance = _record_provenance(built, source)
            from qbopt.frontend import raising_frame

            built = raising_frame.annotated(built, found, mine, contracts)
            # Values visible at a machine exit are semantic edges, not a
            # late lowering constraint.  Materialize them before any
            # recognition can replace source operations: a word-pair ALU
            # leaves its high-word flags whereas a scalar ALU does not.
            built = _with_live_outs(built)
            if not basic_semantics:
                from qbopt.frontend import raising_numeric_policy

                built = raising_numeric_policy.native(built)
            from qbopt.frontend import raising_calls
            from qbopt.frontend import raising_arrays
            from qbopt.frontend import raising_division

            built = raising_division.scalar(built)
            built = raising_calls.arithmetic(built, found, mine, basic_semantics=basic_semantics)
            from qbopt.frontend import raising_bytes

            built = raising_bytes.scalar(built)
            from qbopt.frontend import raising_longs

            built = raising_longs.sign_fills(built)
            built = raising_longs.scalar(built)
            built = raising_longs.unary(built)
            # Unary recognition exposes whole sources for adjacent word stores.
            built = raising_longs.scalar(built)
            built = raising_longs.arguments(built)
            from qbopt.frontend import raising_copies

            built = raising_copies.scalar(built, found)
            from qbopt.frontend import raising_conditions

            built = raising_conditions.loaded(built)
            defined = module.defines(found.records, found.seg)
            array_calls = {at: name for at, name in found.calls.items() if name not in defined}
            built = raising_arrays.annotated(built, array_calls, family=module.family(found.records))
            from qbopt.frontend import raising_array_access

            built = raising_array_access.native(built, found, bounds_checks=bounds_checks)
            from qbopt.frontend import raising_addresses

            built = raising_addresses.loaded(built, contracts)
            built = raising_call_memory.fixed_assignments(built, found)
            built = raising_call_memory.indirect_results(built, found)
            from qbopt.frontend import raising_defseg

            built = raising_defseg.raised(built, found, contracts, source)
            from qbopt.frontend import raising_float_calls

            if not basic_semantics:
                built = raising_float_calls.raised(built, found, contracts, source)
            from qbopt.frontend import raising_float_results

            if not basic_semantics:
                built = raising_float_results.raised(built, found, contracts, source)
            built = raising_longs.arguments(built)
            from qbopt.frontend import raising_floats

            built = raising_longs.sign_fills(built)
            built = raising_floats.annotated(built)
            if not basic_semantics:
                built = raising_numeric_policy.checkpoints(built)
            from qbopt.frontend import raising_float_values

            built = raising_float_values.loaded(raising_float_values.raised(built))
            from qbopt.frontend import raising_words

            built = raising_words.scalar(built)
            if body.body.kind == "main":
                from qbopt.frontend import raising_literals

                built = raising_literals.initialized(built, found, contracts)
            from qbopt.frontend import raising_dispatch

            built = raising_dispatch.raised(built, found, mine)
            source.refs.update(_referenced(built, found))
            built = _externalized(built, source, provenance)
            built = _frame_bounded(built)
            folded, absorbed, refs, coverage = _folded(built, found, blocks)
            built = _completed_ownership(built, source, provenance, coverage)
            source.absorbed.update(absorbed)
            source.refs.update(refs)
            source.coverage.update(coverage)
            held = {**_returned(built), **folded}
            if held:
                built = replace(built, pins={**built.pins, **held})
            out.append((f"{body.body.kind} {body.body.name or '(main)'}", built))
            if body.body.kind == "error-handler":
                error_handlers.append(built)
    if not basic_semantics:
        result_only = raising_call_memory.result_only_functions(
            [(name, body) for name, body in out if name.startswith("procedure ")], found
        )
        if result_only:
            out = [
                (name, raising_call_memory.complete_result_calls(body, found.calls, result_only)) for name, body in out
            ]
    if error_handlers:
        summaries = [
            raising_call_memory.handler_effects(one, found.calls, contracts, unreached) for one in error_handlers
        ]
        summary = (
            None
            if any(one is None for one in summaries)
            else (
                tuple(ref for one in summaries for ref in one[0]),
                tuple(ref for one in summaries for ref in one[1]),
            )
        )
        out = [
            (
                name,
                _frame_bounded(
                    raising_call_memory.with_handler_effects(body, summary, found.calls, contracts, unreached)
                ),
            )
            for name, body in out
        ]
    out = [(name, _provenanced(replace(_with_live_outs(body), stack_in_data=True), found)) for name, body in out]
    hints = {body.entry: AllocationHints.from_body(body) for _name, body in out}
    return RaisedBodies(tuple((name, _public(body)) for name, body in out), source, hints)


def _provenanced(body: MirBody, found: Module) -> MirBody:
    """Every reference with its region set as provenance, until the raise states it directly."""
    bounds = module.landmarks(found)
    private = frozenset() if found.program_data is None else frozenset({found.program_data})
    refs = [ref for block in body.blocks for op in block.ops for ref in (*op.loads, *op.stores)]
    spared = frozenset(addr.index for ref in refs for addr, _ in ref.excludes if addr.space is Space.SEGMENT)

    def moved(ref: MemRef) -> MemRef:
        if ref.provenance is not None:
            return ref
        return replace(ref, provenance=regions.provenance(ref, bounds, private=private, spared=spared))

    def operand(one):
        return Cell(moved(one.ref)) if isinstance(one, Cell) else one

    blocks = tuple(
        replace(
            block,
            ops=tuple(
                replace(
                    op,
                    loads=tuple(map(moved, op.loads)),
                    stores=tuple(map(moved, op.stores)),
                    args=tuple(map(operand, op.args)),
                    results=tuple(map(operand, op.results)),
                    memory_values=tuple((moved(ref), value) for ref, value in op.memory_values),
                )
                for op in block.ops
            ),
        )
        for block in body.blocks
    )
    return replace(body, blocks=blocks, initial=tuple((moved(ref), value) for ref, value in body.initial))


def _sites(found: Module, blocks: list[Block]) -> dict:
    """Every absorbable runtime call, by the address of its first push.

    calls.py answers this and is the machine arm, which phase D retires --
    what it knows that MIR does not is where an argument was pushed from,
    and that is a question about the bytes BC wrote, which is the raise's
    to ask.
    """
    from qbopt.legacy import calls as machine
    from qbopt.analysis import flags as flagged

    reached = [insn for block in blocks for insn in block.insns]
    try:
        found_sites = machine.sites(found, reached, blocks)
    except Exception:
        return {}
    live = flagged.live_in(blocks)
    out = {}
    for one in found_sites:
        if one.name in (*machine.DIVIDES, machine.MULTIPLY):
            continue  # raising_calls recovers values at the pushes, not a frozen machine sequence.
        if not one.pushed or one.name.upper() not in _ABSORBS:
            continue
        # Only where the sequence exists. The raise and the emitter have to
        # agree about which sites are folded -- folding one the emitter then
        # refuses leaves an operation over argument values that nothing will
        # write instructions for -- so both ask calls.py, which is the one
        # place that knows what a site becomes.
        if isinstance(machine.absorb(one, _flags_after(blocks, live, one.start, one.end)), str):
            continue
        out[one.at] = one
    return out


def _returned(body: MirBody) -> dict:
    """What a call hands back, held to the registers BC reads it from.

    A runtime routine writes its result into the registers its own code
    names -- a long comes back in dx:ax -- and the operation standing for
    the call says nothing about it. The allocator moved the high half to
    bx and re-encoded every reader, which then agreed with each other and
    not with the callee: chain printed CONST= 39649280 for 0.
    """
    # Only what something reads. A call defines a value for every register
    # it destroys, and holding a marker nothing reads to its old register
    # is a constraint with no fact behind it -- four of the flow's programs
    # stopped allocating for want of a register held by one.
    read = {one for block in body.blocks for op in block.ops for one in op.uses} | {
        one for block in body.blocks for phi in block.phis for one in phi.incoming.values()
    }
    return {
        one: body.origin[one]
        for block in body.blocks
        for op in block.ops
        if op.kind is Kind.CALL
        for one in op.defines
        if not one.flags and one in read and one in body.origin
    }


def _folded(
    body: MirBody, found: Module, blocks: list[Block]
) -> tuple[dict, dict[int, object], dict[int, tuple[int, ...]], dict[int, tuple[tuple[int, int], ...]]]:
    """Each folded operation told which site it stands for, and which fixups.

    Both by op id, beside the refs, because they are the same kind of fact:
    established at the raise while the bytes are still BC's, and carried by
    the operation wherever a pass moves it.

    Returns what the sites hand back, held to the registers they name. A
    site is emitted by select.absorbed as seventeen fixed bytes ending in
    `pop ax / pop dx`, and its operation's semantics say none of that, so
    the allocator moved the result's high half to bx and re-encoded every
    reader of it. The readers agreed with each other and not with the
    idiom that produced them: chain printed CONST= 23068672 for 0.
    """
    from qbopt.legacy import calls as machine
    from qbopt.analysis import flags as flagged

    sites = {one.start: one for one in _sites(found, blocks).values()}
    if not sites:
        return {}, {}, {}, {}
    held: dict = {}
    absorbed: dict[int, object] = {}
    refs: dict[int, tuple[int, ...]] = {}
    coverage: dict[int, tuple[tuple[int, int], ...]] = {}
    live = flagged.live_in(blocks)
    for block in body.blocks:
        for op in block.ops:
            site = sites.get(op.at)
            if site is None or op.id is None or op.kind is Kind.CALL:
                continue
            # The half a site hands back stands beside it at the same
            # address, and everything here is keyed on the address. Without
            # this it is recorded as an absorbed site of its own, claiming
            # the same bytes twice and emitting the same sequence again.
            if op.op is Synth.HALF_TO_LOW:
                continue
            # The flags go with it. Which flags something reads after the
            # site decides what the sequence may be -- a comparison wraps
            # eax in push/pop -- so the emitter has to be given the same
            # answer the raise filtered on, or the two disagree on length.
            read = _flags_after(blocks, live, site.start, site.end)
            absorbed[op.id] = (site, read)
            held.update({one: body.origin[one] for one in op.defines if not one.flags and one in body.origin})
            made = machine.absorb(site, read)
            if not isinstance(made, str) and made.relocations:
                refs[op.id] = tuple(field for _where, field in made.relocations)
            pushes = _disjoint(site)
            ranges = _raising_ranges(op)
            if pushes and ranges:
                coverage[op.id] = (ranges[0], *pushes)
    return held, absorbed, refs, coverage


def instruction(op: "Op") -> bool:
    """Whether this operation stands for one of BC's own instructions.

    A pass has to be able to tell one from a marker the raise put there --
    a join, a barrier, an argument -- and the answer is what the raise made
    it from, which is this module's to know and not a pass's. Asking
    lower.py directly is a MIR pass calling a machine one, which is the
    thing rule 5 forbids.
    """
    # The raise already answered this in MIR's own vocabulary. NOTHING is a
    # marker with no instruction behind it; every other kind is an operation
    # that either came from one or was deliberately introduced for lowering.
    # Asking lower.current() here used machine encodability as identity and
    # made a MIR pass fail on a perfectly explicit far-memory Cell before the
    # backend had even been entered.
    return op.kind is not Kind.NOTHING


def rewritable(op: "Op") -> bool:
    """Whether this operation's bytes may be generated rather than copied.

    One emitted verbatim -- a barrier, the restore idiom, an emulated x87
    site -- is exactly as long as its source occurrence. One that is selected
    has no such tie.

    Asked before a pass hands a deleted operation's bytes to a survivor. A
    restore idiom that took them stopped coming back its own length --
    qb-qrender's SCREEN.OBJ, the only object in either corpus with the
    shape.
    """
    if op.kind is Kind.PTR_OFFSET:
        return True
    if any(ref.pointer for ref in (*op.loads, *op.stores)):
        return op.kind in (Kind.LOAD, Kind.STORE)
    if op.floating_origin is not None:
        return op.kind is not Kind.NOTHING
    if op.op is Synth.HALF_TO_LOW and op.name == "restore":
        return False
    return not op.barrier and op.kind not in (Kind.NOTHING, Kind.OPAQUE)


def _flags_after(blocks: list[Block], live: dict, lo: int, hi: int):
    """Which flags something reads after this region.

    flags.py's own analysis rather than MIR's values: MIR has one FLAGS
    pseudo-register and cannot say *which* flag, and for a comparison that
    is the whole question -- CF, PF and AF are the runtime's own synthesis
    and a `cmp` does not reproduce them.
    """
    from qbopt.analysis import flags as flagged

    for block in blocks:
        if block.at <= lo < block.end:
            return flagged.live_after(block, hi, live)
    return flagged.Flag(0)


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
        node = getattr(op, "node", None)
        if node is None or isinstance(node, ir.Restore):
            return None
        if found.code[op.at : op.at + 1] in (b"\x9a", b"\xea") and op.at + 1 in known:
            return op.at + 1
        lo, hi = ir.span(node)
        inside = [one for one in known if lo <= one < hi]
        return inside[0] if len(inside) == 1 else None

    return {
        op.id: (at,) for block in body.blocks for op in block.ops if op.id is not None and (at := owned(op)) is not None
    }
