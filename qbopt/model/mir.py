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

import itertools
from enum import StrEnum
from dataclasses import field
from dataclasses import replace
from dataclasses import dataclass

from iced_x86 import Register
from iced_x86 import Register_
from iced_x86 import RegisterExt

from qbopt.model import ir
from qbopt.model.floating import Semantics as FloatingSemantics
from qbopt.analysis import loops
from qbopt.frontend import stack
from qbopt.objectfile import module
from qbopt.abi import runtime
from qbopt.objectfile.module import Addr
from qbopt.frontend.blocks import Block
from qbopt.objectfile.module import Space
from qbopt.objectfile.module import Module

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

    Where BC kept it is still known, and is a fact about lowering rather
    than about the value: MirBody.origin holds it. Reading that map from an
    analysis is a choice with a reason, not something that happens by
    accident because the register was sitting on the value.
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
    # `variable` is an index, not a register: which register it was is
    # MirBody.origin's, and it stays there.
    variable: int = 0
    version: int = 0

    def __repr__(self) -> str:
        kind = "f" if self.flags else "v"
        return f"{kind}{self.variable}_{self.version}" if self.version else f"{kind}{self.id}"


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
    # Per cell, not per body. Asked per body -- does this body hand out an
    # address at all -- all 32 of the corpus's programs do, and the
    # guarantee is worth nothing.
    beyond: "tuple[int, frozenset] | None" = None
    symbolic: "Symbol | None" = None  # proven effective address; original operands remain for lowering
    allocation: "Symbol | None" = None  # proven in-bounds access to this dynamic allocation
    base_width: int = 4  # width of the address value, independently of the memory data width

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


type Arg = Held | Const | Symbol | Cell | Opaque


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
    DIV = "div"
    REM = "rem"
    # One computation with two results, quotient then remainder. BC calls
    # B$DVI4 for one and B$RMI4 for the other over the same operands, and
    # a kind each gives CSE two keys for one divide -- which is why
    # lngmix's loop divides ten times and takes the modulus again.
    DIVMOD = "divmod"
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
    FABS = "fabs"
    FLOAD = "fload"
    FSTORE = "fstore"
    FCOMPARE = "fcompare"

    # what has no MIR form yet, each with the step that removes it
    ARG = "arg"  # a call argument still written as a push -- step 3
    RESULT = "result"  # and its pop
    JOIN = "join"  # two halves of a wide value made one -- step 4
    EXTRACT = "extract"  # a bit range of a value; args[1] is the bit offset
    CONCAT = "concat"  # high bits followed by low bits, with explicit operand widths
    OPAQUE = "opaque"  # nothing is claimed; see ir.Operation.BARRIER
    NOTHING = "nothing"


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


def rewritten(op: "Op") -> bool:
    """Whether a pass has changed what this operation computes.

    The question every caller used to ask as `op.made is not None`, which
    named the machine form a pass wrote down. What it means is that the
    operands are not the ones the raise gave it.
    """
    if op.made is not None:
        return True
    return op.raised is not None and (op.args, op.results) != op.raised


def _merged(what: "ir.Semantics", holds: dict, written: dict, args: tuple) -> dict:
    """Which use is only the previous contents of which result.

    A use is carried when it shares a place with something the operation
    writes and the operation never names that place as an input. An
    operation naming no operand at all describes nothing, so nothing in it
    is a partial write: a call names none, and every argument it reads
    shares a register with something it clobbers -- reading those as
    previous contents made B$OGTA's branch index look dead.
    """
    if not what.sources and not what.dests:
        return {}
    named = {ir.ROOT.get(one.register, one.register) for one in what.sources if isinstance(one, ir.Reg)}
    out: dict = {}
    for register, value in written.items():
        if value.flags:
            continue
        root = ir.ROOT.get(register, register)
        if root in named:
            continue
        was = holds.get(register)
        if was is not None:
            out[was] = value
    return out


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
    """The kind a site of this name raises as, or None where it has no one
    kind of its own -- fixMul& is absorbed and is still a call."""
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
    # Noncontiguous input bytes (such as an absorbed call's argument pushes).
    # Deletion transfers these alongside covers; no machine semantics live here.
    extra_covers: tuple[tuple[int, int], ...] = ()

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
    # Loader-established literal bytes, valid on entry only. They are not
    # immutable: ordinary alias and call effects invalidate these facts.
    initial: tuple[tuple[MemRef, Const], ...] = ()

    def block(self, at: int) -> MirBlock | None:
        return next((one for one in self.blocks if one.at == at), None)

    @property
    def values(self) -> tuple[Value, ...]:
        return tuple(
            value
            for block in self.blocks
            for value in ([phi.result for phi in block.phis] + [v for op in block.ops for v in op.defines])
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
    for one in runtime.slots(routine):
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
    if not routine.established:
        return None
    kept = {FROM_CONTRACT[one] for one in runtime.preserves(routine) if one in FROM_CONTRACT}
    disturbed = frozenset(one for one in TRACKED if one not in kept) | {FLAGS}
    if runtime.barrier(routine):
        # A barrier reaches code this module cannot see -- an event handler,
        # an ON ERROR target -- and what the routine's own body preserves
        # says nothing about what that code leaves behind. So it disturbs
        # everything, which is what the worst-case ir.Effects said before
        # any of this. What it *reads* is a different question, and the two
        # were answered together: B$CENP, whose contract is `cProc B$CENP`
        # with no parameters, was made to read every tracked register, so
        # esi's entry value stayed live to the end of the body with nothing
        # having written it, and spilling that phantom put a reload of a
        # slot no one stored.
        disturbed = frozenset(TRACKED) | {FLAGS}
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
    reads = {FROM_CONTRACT[one] for one in routine.inputs if one in FROM_CONTRACT}
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


# What each operation's unnamed memory is in, where the operation says so.
# A push and a pop reach the stack and nothing else, whatever the depth.
_IN: dict = {}


def _object_of(node) -> "Space | None":
    """Which object this instruction's memory is in, or None for anything."""
    what = getattr(node, "semantics", None)
    if what is None:
        return None
    if what.op in (ir.Operation.PUSH, ir.Operation.POP):
        return Space.STACK
    return None


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
    return Op(
        at,
        Synth.HALF_TO_LOW,
        "restore",
        (now,),
        (was, answer),
        (),
        (),
        ir.Restore(at=at, end=at, pair=0, effects=ir.RESTORE_EFFECTS[0]),
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

    again: dict[Value, Value] = {}

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
            defines, uses = _touched(node, calls)
            used = tuple(namer.current(one, start) for one in sorted(uses, key=lambda o: (o is not FLAGS, o)))
            where = _object_of(node)
            keeps = unreached if _narrowed(calls, insn.at) else None
            loads = _memrefs(node.effects.loads, namer, start, slot, where, keeps)
            stores = _memrefs(node.effects.stores, namer, start, slot, where, keeps)
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
                    used = tuple(dict.fromkeys(
                        [arg.value for arg in operands if isinstance(arg, Held)]
                        + [value for ref in loads for value in (ref.base, ref.segment) if value is not None]
                    ))
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
                Op(
                    where_at,
                    Synth.HALF_TO_LOW if isinstance(node, ir.Restore) else node.semantics.op,
                    node.semantics.name or "",
                    kept,
                    used,
                    loads,
                    stores,
                    node,
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
        out.append(replace(ref, base=base))
    return tuple(out)


class _Renamer:
    """Fresh values per MIR variable, and which one each currently holds."""

    def __init__(self, home: dict[int, Register_]) -> None:
        self.next = 0
        self.home = home
        self.stack: dict[int, list[Value]] = {}
        self.versions: dict[int, int] = {}
        self.origin: dict[Value, Register_] = {}

    def fresh(self, variable: int, at: int, flags: bool = False) -> Value:
        self.next += 1
        self.versions[variable] = self.versions.get(variable, 0) + 1
        made = Value(self.next, at, flags, variable, self.versions[variable])
        where = self.home.get(variable)
        if where is not None:
            self.origin[made] = where
        return made

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

    # Keyed on the MIR variable, not on the register. In MIR a register is
    # a variable and nothing more -- but two values BC happened to keep in
    # one register are one variable only while nothing has moved them, and
    # the moment the hoist lifts a computation out of a loop they are not:
    # the product and the counter both lived in ax, and re-deriving by
    # register put a phi over them that said the loop's reads of the
    # product were reads of the counter. hotlpx printed the wrong sum with
    # every host test agreeing.
    frontier = loops.frontiers(blocks, start)
    home: dict[int, Register_] = {}
    for value, register in (body.origin or {}).items():
        home.setdefault(value.variable, register)

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

    namer = _Renamer(home)
    phis: dict[int, dict[int, Phi]] = {block.at: {} for block in blocks}
    out: dict[int, list[Op]] = {block.at: [] for block in blocks}
    flagged = {value.variable for block in blocks for op in block.ops for value in op.defines if value.flags}
    for block in blocks:
        for variable in sorted(needed[block.at], key=lambda one: (one not in flagged, one)):
            phis[block.at][variable] = Phi(namer.fresh(variable, block.at, variable in flagged), {})

    again: dict[Value, Value] = {}

    def rename(at: int) -> None:
        block = by_at[at]
        pushed: list[int] = []
        for variable, phi in phis[at].items():
            namer.stack.setdefault(variable, []).append(phi.result)
            pushed.append(variable)

        for op in block.ops:
            used = tuple(namer.current(one, start) for one in op.uses)
            swap = {one.variable: now for one, now in zip(op.uses, used)}
            loads = tuple(_rehomed(one, namer, start) for one in op.loads)
            stores = tuple(_rehomed(one, namer, start) for one in op.stores)
            refs = dict(zip((*op.loads, *op.stores), (*loads, *stores)))
            fresh = []
            for one in op.defines:
                value = namer.fresh(one.variable, op.at, one.flags)
                namer.stack.setdefault(one.variable, []).append(value)
                pushed.append(one.variable)
                fresh.append(value)
                # In SSA a value is defined once, so this is a whole map
                # from the old name to the new one -- which is what a pin
                # needs to survive. Carried unremapped, every pin naming a
                # value some pass had caused to be re-versioned addressed
                # nothing, and the allocator moved what a call hands back.
                again[one] = value
            made = {one.variable: now for one, now in zip(op.defines, fresh)}
            out[at].append(
                replace(
                    op,
                    defines=tuple(fresh),
                    uses=used,
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
        {again.get(one, one): where for one, where in body.pins.items()},
        body.initial,
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
    one, other = _symbolic_ref(one), _symbolic_ref(other)
    if one.addr is None or other.addr is None:
        return False  # nothing this can name is never known to be anything
    if one.addr.space is Space.FAR and one.segment is None and not (
        one.allocation is not None and one.allocation == other.allocation
    ):
        return False
    if one.width != other.width or one.base != other.base or one.segment != other.segment:
        return False
    return one.addr == other.addr


def overlapping(
    one: MemRef,
    other: MemRef,
    dgroup: frozenset[int],
    bounds: dict | None = None,
    known: dict | None = None,
    other_known: dict | None = None,
) -> bool:
    """Whether a write through `other` could land on `one`.

    module.may_alias for the symbolic part, and the base value for the rest.
    Where both name the same base value their displacements settle it by
    arithmetic, exactly as two bare statics do -- and soundly for the same
    reason memory.aliases() gives, except that this holds it by value
    identity rather than by the caller having promised the register was not
    written in between.
    """
    if known or other_known:
        from qbopt.analysis import ranges
        one, other = ranges.covering(one, known or {}), ranges.covering(other, other_known or {})
    one, other = _symbolic_ref(one), _symbolic_ref(other)
    if _allocation_disjoint(one, other) or _allocation_disjoint(other, one):
        return False
    if one.addr is None or other.addr is None:
        # Neither names the byte, but each may name the object. LLVM's
        # PseudoSourceValue rule: two different kinds never alias, and one
        # whose kind is unknown aliases anything.
        if _out_of_reach(one, other) or _out_of_reach(other, one):
            return False
        return _may_reach(one.where, other.where)
    if (
        one.base is not None
        and one.base == other.base
        and one.addr.space is other.addr.space
        and one.addr.index == other.addr.index
        and one.segment == other.segment
        and (one.addr.space is not Space.FAR or one.segment is not None)
    ):
        return one.addr.disp < other.addr.disp + other.width and other.addr.disp < one.addr.disp + one.width
    return module.may_alias(one.addr, other.addr, dgroup, one.width, other.width, bounds)


def _symbolic_ref(ref: MemRef) -> MemRef:
    if ref.symbolic is None:
        return ref
    symbol = ref.symbolic
    return replace(ref, addr=module.Addr(symbol.space, symbol.offset + symbol.addend, symbol.index), base=None, segment=None)


def _allocation_disjoint(allocated: MemRef, static: MemRef) -> bool:
    return (
        allocated.allocation is not None and static.allocation is None
        and static.addr is not None and static.addr.space is Space.SEGMENT
        and static.addr.index == allocated.allocation.index
        and static.base is None and static.segment is None and static.addr.base == Register.NONE
    )


def _out_of_reach(blind: MemRef, named: MemRef) -> bool:
    """Whether `blind` says it cannot reach the cell `named` is in."""
    if blind.beyond is None or named.addr is None:
        return False
    owner, reaches = blind.beyond
    if named.addr.space is not Space.SEGMENT or named.addr.index != owner:
        return False  # not the program's own data; this says nothing about it
    return (named.addr.index, named.addr.disp) not in reaches


def _may_reach(one: "Space | None", other: "Space | None") -> bool:
    """Whether a reference in one object could land in the other.

    The kinds alone, for a pair where at least one cannot name its byte.
    `module.may_alias` is the same question with the displacements known;
    this is what is left when they are not.
    """
    if one is None or other is None:
        return True
    if one is other:
        return True  # same object, unknown offsets
    # The stack and the frame are one region reached two ways, so they
    # alias each other and nothing else. Everything else is a different
    # object entirely.
    return {one, other} == {Space.STACK, Space.FRAME}


def _narrowed(calls: dict | None, at: int) -> bool:
    """Whether this call's contract lets it be told what it cannot reach.

    Only a call, and only one whose writes are its own. A routine that
    hands control back to the program -- B$EVCK polls for an event and may
    run an event GOSUB -- writes whatever that code writes, and GCC's
    modref gives up on an indirect call for the same reason.
    """
    from qbopt.abi import runtime

    if calls is None or at not in calls:
        return False
    contract = runtime.contract(calls[at])
    # One that never comes back writes nothing anybody can observe. LLVM
    # and GCC both say this with `noreturn`, and it matters here because
    # B$CENP ends every program: its own contract is ANY, it stands in
    # every body, and taken at face value it aliases every variable in
    # every one of them.
    if contract.control is runtime.Control.NEVER:
        return True
    return contract.writes is not runtime.Memory.ANY and contract.reads is not runtime.Memory.ANY


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


def extracted_whole(high, low, definitions):
    """The scalar whose exact high and low word extractions are these operands."""
    original = None
    for arg, offset in ((high, 16), (low, 0)):
        seen = set()
        while isinstance(arg, Held) and arg.width == 2 and arg.value not in seen:
            seen.add(arg.value)
            copy = definitions.get(arg.value)
            if (copy is None or copy.kind is not Kind.COPY or copy.loads or copy.stores or copy.barrier
                or copy.results != (arg,) or len(copy.args) != 1
                or not isinstance(copy.args[0], Held) or copy.args[0].width != arg.width):
                break
            arg = copy.args[0]
        if not isinstance(arg, Held) or arg.width != 2:
            return None
        op = definitions.get(arg.value)
        if (op is None or op.kind is not Kind.EXTRACT or op.results != (arg,) or len(op.args) != 2
            or not isinstance(op.args[0], Held) or op.args[0].width != 4
            or not isinstance(op.args[1], Const) or op.args[1].n != offset):
            return None
        if original is not None and original != op.args[0]:
            return None
        original = op.args[0]
    return original


def bodies(
    found: Module, blocks: list[Block], contracts: "dict[int, runtime.Contract] | None" = None
) -> list[tuple[str, MirBody]]:
    """Every body in the module, raised, labelled, and skipping what will not.

    One place rather than three: dump.py, the measurement scripts and now
    rewrite.py all need the same walk, and the part worth not rewriting
    twice is the block-to-body assignment -- a procedure is reached by a
    call, which is not a CFG edge, so raise_body() has to be handed one
    body's blocks and no others.
    """
    unreached = _unreached(found)
    result = ir.decode_module(found)
    if isinstance(result, str):
        return []
    nodes = {ir.span(node)[0]: node for body in result for node in body.nodes}
    if contracts is None:
        # The module's own toolchain, which is where a per-family contract
        # is chosen and the only place a family is read at all.
        contracts = runtime.for_module(found)
    out: list[tuple[str, MirBody]] = []
    for body in result:
        mine = [one for one in blocks if any(lo <= one.at < hi for lo, hi in body.body.ranges)]
        if not mine:
            continue
        built = raise_body(
            mine,
            nodes,
            body.body.seed,
            found.calls,
            _sites(found, blocks),
            unreached,
            contracts,
        )
        if not isinstance(built, str):
            from qbopt.frontend import raising_arrays
            from qbopt.frontend import raising_division
            from qbopt.frontend import raising_calls

            built = raising_division.scalar(built)
            built = raising_calls.arithmetic(built, found, mine)
            from qbopt.frontend import raising_longs
            built = raising_longs.scalar(built)
            from qbopt.frontend import raising_copies
            built = raising_copies.scalar(built, found)
            defined = module.defines(found.records, found.seg)
            array_calls = {at: name for at, name in found.calls.items() if name not in defined}
            built = raising_arrays.annotated(built, array_calls, family=module.family(found.records))
            from qbopt.frontend import raising_addresses
            built = raising_addresses.loaded(built)
            from qbopt.frontend import raising_float_calls
            built = raising_float_calls.raised(built, found, contracts)
            from qbopt.frontend import raising_floats
            built = raising_floats.annotated(built)
            from qbopt.frontend import raising_float_values
            built = raising_float_values.raised(built)
            if body.body.kind == "main":
                from qbopt.frontend import raising_literals
                built = raising_literals.initialized(built, found)
            found.refs.update(_referenced(built, found))
            held = {**_returned(built), **_folded(built, found, blocks)}
            if held:
                built = replace(built, pins={**built.pins, **held})
            out.append((f"{body.body.kind} {body.body.name or '(main)'}", built))
    return out


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


def _folded(body: MirBody, found: Module, blocks: list[Block]) -> dict:
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
        return {}
    held: dict = {}
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
            found.absorbed[op.id] = (site, read)
            held.update({one: body.origin[one] for one in op.defines if not one.flags and one in body.origin})
            made = machine.absorb(site, read)
            if not isinstance(made, str) and made.relocations:
                found.refs[op.id] = tuple(field for _where, field in made.relocations)
            pushes = _disjoint(site)
            if pushes and op.covers is not None:
                found.coverage[op.id] = (op.covers, *pushes)
    return held


def instruction(op: "Op") -> bool:
    """Whether this operation stands for one of BC's own instructions.

    A pass has to be able to tell one from a marker the raise put there --
    a join, a barrier, an argument -- and the answer is what the raise made
    it from, which is this module's to know and not a pass's. Asking
    lower.py directly is a MIR pass calling a machine one, which is the
    thing rule 5 forbids.
    """
    if op.floating_origin is not None:
        return op.kind is not Kind.NOTHING
    from qbopt.backend import lower

    return lower.current(op) is not None


def rewritable(op: "Op") -> bool:
    """Whether this operation's bytes may be generated rather than copied.

    One emitted verbatim -- a barrier, the restore idiom, an emulated x87
    site -- is exactly as long as the bytes it stands for, so its `covers`
    and its length are one number and a pass may not make them differ. One
    that is selected has no such tie.

    Asked before a pass hands a deleted operation's bytes to a survivor. A
    restore idiom that took them stopped coming back its own length --
    qb-qrender's SCREEN.OBJ, the only object in either corpus with the
    shape.
    """
    if op.floating_origin is not None:
        return op.kind is not Kind.NOTHING
    from qbopt.backend import lower

    if isinstance(op.node, ir.Restore):
        return False
    what = lower.current(op)
    return what is not None and what.op is not ir.Operation.BARRIER


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
        if op.node is None or isinstance(op.node, ir.Restore):
            return None
        if found.code[op.at : op.at + 1] in (b"\x9a", b"\xea") and op.at + 1 in known:
            return op.at + 1
        lo, hi = ir.span(op.node)
        inside = [one for one in known if lo <= one < hi]
        return inside[0] if len(inside) == 1 else None

    return {
        op.id: (at,) for block in body.blocks for op in block.ops if op.id is not None and (at := owned(op)) is not None
    }
