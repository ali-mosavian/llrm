"""
Total decode: every instruction in a Body becomes a Node, in order.

lift.py's own value tracker treats anything it does not recognise as a wall
-- "anything not recognised invalidates both pairs, because an instruction
this does not understand may write either of them" (lift.py's own module
docstring). That policy is correct as a default and, per docs/residue.md's
patterns G and H, wrong specifically where the unrecognised instruction is
one this pass itself just emitted (calls.py's own restore idiom) and whose
real effect is knowable. This module is what makes that fixable without
redesign: every instruction gets a real, typed node with an iced-derived
def/use/flags/memory effect, so a later pass can reason about an Opaque node
generically instead of only ever falling off a cliff.

Two orthogonal things are recorded per node, and confusing them is how an
optimiser gets silently wrong answers:

  Effects  -- what the instruction disturbs. iced's own answer, widened
              where the encoding is not the whole story (a call, a barrier)
              and never narrowed, and rooted to the 32-bit parent (see ROOT)
              so "does this touch the ax pair" is decided once. It is the
              authority on def/use, and it is complete for every node.
  Semantics -- what the instruction *computes*: an operation, its
              destinations and its sources as typed locations. Complete where
              `op` is not Operation.BARRIER. Its registers are the literal
              ones iced decoded, unrooted, because a value's identity is
              `ax`, not "somewhere in eax" -- the same distinction
              registers.py's own docstring draws for liveness.

So Semantics is what a value-numbering or constant-folding pass reads, and
Effects is what a code-motion or dead-store pass reads. A Semantics that is
merely absent costs precision; an Effects that is wrong costs correctness,
which is why nothing here ever narrows an effect below what iced reports.

An instruction this pass cannot model is not a hole in the IR. It is a
barrier: carried verbatim, with a complete and conservative Effects, and with
a contract a caller honours instead of refusing the body it sits in -- see
Operation.BARRIER and pinned(). So there are exactly two states a node's
operation can be in, modelled and barrier, and no instruction is
unrepresentable. The only remaining "nothing at all" is a module whose *bytes*
could not be decoded or partitioned, which decode_module() answers with a
string before any Node exists.

The node *type* says which idiom claimed the node -- lift.classify()'s six
single-instruction long-pair forms, a far call a fixup names, calls.py's own
three-instruction "restore" idiom, an inline table -- and deliberately not
whether its operation is modelled. Adding an encoding to the vocabulary
therefore never reshuffles a consumer's own isinstance checks; it only fills
in a Semantics that used to be Operation.BARRIER.

The emitter never reads a node's semantic fields, only its own byte span --
`module.code[node.at:node.end]` via `span()`, for every node kind, always.
That is what makes "decode a Body, re-emit it, compare to the original"
hold by construction: a wrong Node type or a wrong Effects field can never
corrupt commit 1's own output, because emission never consults them. It can
only corrupt what a future commit computes from the IR, which is that
commit's own gate to hold, not this one's.
"""

from enum import StrEnum
from dataclasses import dataclass
from collections.abc import Callable

from iced_x86 import Code
from iced_x86 import OpKind
from iced_x86 import Mnemonic
from iced_x86 import Register
from iced_x86 import Register_
from iced_x86 import RegisterExt
from iced_x86 import MemorySizeExt

from qbopt.flags import ALL
from qbopt.flags import Flag
from qbopt.lift import FIXUP
from qbopt.declen import INFO
from qbopt.declen import Insn
from qbopt.extent import Body
from qbopt.module import Addr
from qbopt.blocks import Block
from qbopt.declen import READS
from qbopt.lift import Decoded
from qbopt.declen import WRITES
from qbopt.lift import Resolver
from qbopt.lift import classify
from qbopt.module import Module
from qbopt.blocks import CodeMap
from qbopt.flags import CLOBBERS
from qbopt.blocks import code_map
from qbopt.declen import to_signed
from qbopt.extent import Partition
from qbopt.flags import written_by
from qbopt.blocks import INLINE_TABLE
from qbopt.lift import operand as long_operand
from qbopt.extent import partition as body_partition
from qbopt.blocks import partition as block_partition

# The 32-bit root of every general-purpose register this pass ever reasons
# about. AL/AH/AX/EAX are all "does this touch the ax pair" -- reporting
# whichever sub-register iced happened to decode would push "does a write to
# eax kill dx" (it does not -- that is the entire reason calls.py's restore
# exists) onto every consumer instead of deciding it once, here.
ROOT = {
    Register.AL: Register.EAX,
    Register.AH: Register.EAX,
    Register.AX: Register.EAX,
    Register.EAX: Register.EAX,
    Register.BL: Register.EBX,
    Register.BH: Register.EBX,
    Register.BX: Register.EBX,
    Register.EBX: Register.EBX,
    Register.CL: Register.ECX,
    Register.CH: Register.ECX,
    Register.CX: Register.ECX,
    Register.ECX: Register.ECX,
    Register.DL: Register.EDX,
    Register.DH: Register.EDX,
    Register.DX: Register.EDX,
    Register.EDX: Register.EDX,
    Register.SI: Register.ESI,
    Register.ESI: Register.ESI,
    Register.DI: Register.EDI,
    Register.EDI: Register.EDI,
    Register.BP: Register.EBP,
    Register.EBP: Register.EBP,
    Register.SP: Register.ESP,
    Register.ESP: Register.ESP,
}


def root(register: Register_) -> Register_:
    return ROOT.get(register, register)


@dataclass(frozen=True, slots=True)
class Reg:
    """A register operand at the width the instruction uses it, unrooted."""

    register: Register_
    width: int


@dataclass(frozen=True, slots=True)
class Mem:
    """A memory cell. `addr` is None where the address is not known, and a
    None address is never provably disjoint from anything -- module.may_alias
    is the one place that rule lives."""

    addr: Addr | None
    width: int


@dataclass(frozen=True, slots=True)
class Imm:
    """A literal, signed as the instruction means it and already widened past
    whichever of the short encodings BC chose."""

    value: int
    width: int


@dataclass(frozen=True, slots=True)
class Address:
    """An address as a *value* -- `lea`'s own result. Not a memory access: an
    Address source reads no memory and appears in no Effects.loads."""

    addr: Addr | None


type Loc = Reg | Mem | Imm | Address


@dataclass(frozen=True, slots=True)
class Effects:
    """One instruction's (or idiom's) real, conservative effect.

    None for `defs`/`uses` means "assume any register" -- the answer for a
    call or interrupt, whose real effect is the callee's, not what iced's
    per-instruction info reports for the call site itself (flags.written_by
    already treats a call's flags this way; the same conservatism applies to
    registers and memory here, for the same reason).

    `loads` and `stores` are kept apart because dead-store elimination and
    store-to-load forwarding ask different questions of them, and a single
    "touches memory" answers neither: `push [x]` reads a static and writes a
    stack cell, two addresses with nothing to do with each other. A cell
    whose `addr` is None is an address this layer cannot name -- a stack
    slot, an unresolved operand, a call's own unknown reach -- and aliases
    everything.
    """

    defs: frozenset[Register_] | None
    uses: frozenset[Register_] | None
    flags_written: Flag
    flags_read: Flag = Flag.NONE
    loads: tuple[Mem, ...] = ()
    stores: tuple[Mem, ...] = ()

    @property
    def touches_memory(self) -> bool:
        return bool(self.loads or self.stores)


NO_EFFECT = Effects(frozenset(), frozenset(), Flag.NONE)

# A call's or interrupt's reach: any address, either way, at a width this
# layer has no business guessing.
ANY_MEMORY = (Mem(None, 0),)


class Operation(StrEnum):
    """What a node computes.

    Two states, and only two: modelled, where the fields each member names
    below mean exactly what they say, and BARRIER, where nothing at all is
    claimed. There is no third, unrepresentable state -- see BARRIER.
    """

    MOVE = "move"  # dests[0] <- sources[0]
    ADDRESS = "addr"  # dests[0] <- the numeric value of sources[0]
    BINARY = "binary"  # dests[0] <- sources[0] `name` sources[1], and sources[0] IS dests[0]
    # dests[0] <- sources[0] * sources[1]; unlike BINARY, no source need be the dest. The widening
    # one-operand form has two destinations instead: dests[0] the low half, dests[1] the high.
    MULTIPLY = "mul"
    DIVIDE = "div"  # dests[0] <- the quotient and dests[1] <- the remainder of sources[0]:sources[1] / sources[2]
    COMPARE = "cmp"  # flags only, from sources[0] and sources[1]
    UNARY = "unary"  # dests[0] <- `name` sources[0], and sources[0] IS dests[0]
    EXTEND = "extend"  # dests[0] <- the sign of sources[0]: cwd, cdq
    PUSH = "push"  # sources[0] onto the stack; the cell and sp are Effects' business, not this layer's
    POP = "pop"  # dests[0] off the stack, likewise
    LEAVE = "leave"  # dests[0] <- sources[0], then dests[1] off the stack: `leave` is `mov sp,bp` then `pop bp`
    FILL = "fill"  # sources[1] copies of sources[0] into dests[0], addressed by sources[3]:sources[2], which steps
    JUMP = "jump"  # unconditional, to `target`
    BRANCH = "branch"  # conditional on the flags, to `target`
    # Control leaves the body, to somewhere the instruction does not name -- a direct far `jmp`. All a
    # CFG needs of one, and all that is honest about one. Never named by SHAPE; _jump() returns it.
    ESCAPE = "escape"
    CALL = "call"
    RETURN = "ret"
    NOTHING = "nothing"  # computes nothing, transfers nowhere, touches no flag
    RESTORE = "restore"  # calls.py's own idiom -- see Restore
    DATA = "data"  # not an instruction at all -- see Data

    # An instruction this pass cannot model, but can still carry. Not a
    # refusal of the body it sits in: its bytes are emitted verbatim, its
    # Effects are complete and conservative, and everything around it lifts.
    # Three rules make that safe, and a caller owes all three:
    #
    #   Nothing may be reordered across it, in either direction.
    #   Its registers are pinned -- see pinned() -- because a barrier's
    #     behaviour is its encoding's, and the encoding names physical
    #     registers rather than values.
    #   Memory promoted to a variable is written back before it and re-read
    #     after. Its memory reach is unknown, and instruction_effects() says
    #     so rather than leaving a consumer to remember it.
    #
    # modelled() is false here and nowhere else, so a pass that may only
    # reason about known semantics still declines exactly what it declined
    # before barriers existed.
    BARRIER = "barrier"


@dataclass(frozen=True, slots=True)
class Semantics:
    """What one node computes, as typed locations.

    Complete only where `op` is not Operation.BARRIER. `name` is the operation's
    own mnemonic, lowercase, and is what distinguishes `add` from `adc` and
    `je` from `jne` -- so it is the operator identity a value number is keyed
    on, stable across every encoding of the same mnemonic.

    `dests` is a tuple rather than one location because the absorbed divide
    has two -- `idiv` leaves the quotient in eax and the remainder in edx,
    and AGENTS.md's own "divide and remainder are C's" is exactly that one
    instruction. Naming only the quotient would tell a value-numbering pass
    that edx still held what it held before.
    """

    op: Operation
    name: str | None = None
    dests: tuple[Loc, ...] = ()
    sources: tuple[Loc, ...] = ()
    target: int | None = None


# Named for what it claims -- nothing -- where Operation.BARRIER is named for
# what a caller must do about it. The mnemonic is deliberately not carried:
# a barrier node always wraps a real Insn, so there is nowhere for a second,
# drifting copy of it to live.
UNMODELLED = Semantics(Operation.BARRIER)
RESTORE_IDIOM = Semantics(Operation.RESTORE, "restore")
TABLE_DATA = Semantics(Operation.DATA)


def modelled(semantics: Semantics) -> bool:
    return semantics.op is not Operation.BARRIER


def barrier(semantics: Semantics) -> bool:
    """Exactly `not modelled()`, named for what a caller must do rather than
    for what it may not assume: a barrier is carried, not refused."""
    return semantics.op is Operation.BARRIER


def _register_effects(insn: Insn) -> tuple[frozenset[Register_], frozenset[Register_]]:
    """Which roots this instruction may write, and which it may read.

    A write iced reports against a sub-register (`mov ax,cx` writes AX, not
    EAX) is a partial write of its root: the bits it does not touch survive,
    so the root also belongs in `uses` -- not just `defs` -- or a consumer
    would think the whole register was freshly defined. `lift.py`'s own
    MOVE comment names exactly this ("mov ax,cx writes sixteen bits and
    leaves the top half of eax stale"), and it is the entire reason
    calls.py's restore idiom exists at all.
    """
    used = list(INFO.info(insn.insn).used_registers())
    defs: set[Register_] = set()
    uses: set[Register_] = set()
    for one in used:
        target = root(one.register)
        if one.access in WRITES:
            defs.add(target)
            if target != one.register:
                uses.add(target)
        if one.access in READS:
            uses.add(target)
    return frozenset(defs), frozenset(uses)


# A memory access through sp is a stack cell, whose address this layer does
# not name -- and it is never the same reference as the instruction's own
# written operand, which is the whole reason loads and stores carry their own
# addresses. `push [x]` reports both.
STACK_BASES = frozenset({Register.SP, Register.ESP})


def _memory_effects(insn: Insn, resolve: Resolver) -> tuple[tuple[Mem, ...], tuple[Mem, ...]]:
    """Every cell this instruction reads, and every one it writes.

    lift.operand() resolves one memory operand -- the one whose displacement
    field a fixup names -- so it can only be attributed when the instruction
    has exactly one non-stack access. Two of them (a string move) leave both
    unnamed rather than both claiming the same address.
    """
    used = list(INFO.info(insn.insn).used_memory())
    named = [one for one in used if one.base not in STACK_BASES]
    where = long_operand(insn, resolve) if len(named) == 1 else None

    loads: list[Mem] = []
    stores: list[Mem] = []
    for one in used:
        cell = Mem(None if one.base in STACK_BASES else where, MemorySizeExt.size(one.memory_size))
        if one.access in READS:
            loads.append(cell)
        if one.access in WRITES:
            stores.append(cell)
    return tuple(loads), tuple(stores)


def instruction_effects(insn: Insn, resolve: Resolver) -> Effects:
    """The conservative effect of one real instruction.

    iced's own answer, widened in the two places where the encoding is not
    the whole story and never narrowed anywhere.

    A call or interrupt, because the real effect is the callee's.

    A barrier, in its memory reach only. An emulated x87 site is the case
    that proves it: declen.py hands iced a reconstruction of the ESC opcode,
    but whether that opcode ever executes is LINK's decision (AGENTS.md --
    "the emulator patch is driven by a linker symbol"), and where it does not,
    what runs is a software routine with memory of its own that is nowhere in
    the encoding. `out` is the second: programming a DMA controller through a
    port writes memory this layer cannot see. Registers are *not* widened to
    match, because a barrier already pins them and nothing may be reordered
    across it -- so exact liveness across one costs no safety and buys the
    2130 emulator sites in qb-qrender.
    """
    if insn.flow in CLOBBERS:
        # The callee's flag reads are as unknowable as its writes. Costs
        # nothing in practice -- flags_written is already ALL, so nothing
        # set before the call survives it either way.
        return Effects(None, None, written_by(insn), ALL, ANY_MEMORY, ANY_MEMORY)
    defs, uses = _register_effects(insn)
    read = Flag(insn.reads & ALL)
    if barrier(instruction_semantics(insn, resolve)):
        return Effects(defs, uses, written_by(insn), read, ANY_MEMORY, ANY_MEMORY)
    loads, stores = _memory_effects(insn, resolve)
    return Effects(defs, uses, written_by(insn), read, loads, stores)


# What each immediate encoding means once sign extension has been applied.
# iced's own immediate() already reports the extended value, so the width
# here is the width of the *result*, not of the encoded field.
IMMEDIATE_WIDTH = {
    OpKind.IMMEDIATE8: 1,
    OpKind.IMMEDIATE8TO16: 2,
    OpKind.IMMEDIATE8TO32: 4,
    OpKind.IMMEDIATE16: 2,
    OpKind.IMMEDIATE32: 4,
}


def _location(insn: Insn, index: int, resolve: Resolver) -> Loc | None:
    """One operand as a typed location, or None if this layer cannot say.

    None is the refusal that keeps the vocabulary honest: a segment register,
    a far branch, a string operand's implicit es:di -- anything a builder
    cannot express reaches here, and the whole instruction falls back to
    Operation.BARRIER rather than being described half-right.
    """
    match insn.insn.op_kind(index):
        case OpKind.REGISTER:
            register = insn.insn.op_register(index)
            return Reg(register, RegisterExt.size(register)) if RegisterExt.is_gpr(register) else None
        case OpKind.MEMORY:
            return Mem(long_operand(insn, resolve), MemorySizeExt.size(insn.insn.memory_size))
        case kind if kind in IMMEDIATE_WIDTH:
            width = IMMEDIATE_WIDTH[kind]
            return Imm(to_signed(insn.insn.immediate(index) & ((1 << (width * 8)) - 1), width), width)
        case _:
            return None


def _destination(insn: Insn, index: int, resolve: Resolver) -> Loc | None:
    found = _location(insn, index, resolve)
    return found if isinstance(found, Reg | Mem) else None


type Builder = Callable[[Insn, Resolver, Operation, str], Semantics | None]


def _move(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    if insn.insn.op_count != 2:
        return None
    dest = _destination(insn, 0, resolve)
    source = _location(insn, 1, resolve)
    return None if dest is None or source is None else Semantics(op, name, (dest,), (source,))


def _address(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    if insn.insn.op_count != 2 or insn.insn.op_kind(1) != OpKind.MEMORY:
        return None
    dest = _destination(insn, 0, resolve)
    return None if dest is None else Semantics(op, name, (dest,), (Address(long_operand(insn, resolve)),))


def _binary(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    if insn.insn.op_count != 2:
        return None
    dest = _destination(insn, 0, resolve)
    source = _location(insn, 1, resolve)
    return None if dest is None or source is None else Semantics(op, name, (dest,), (dest, source))


# imul's own implicit pair in its one-operand, widening form: the (low, high)
# destinations and the accumulator half it reads, by the width of its one
# explicit operand. The 8-bit form puts the whole 16-bit product in ax --
# one destination, a different shape altogether -- so it is left out rather
# than bent to fit, exactly as DIVIDE_PAIR leaves out its own byte form.
WIDE_MULTIPLY = {
    4: ((Reg(Register.EAX, 4), Reg(Register.EDX, 4)), Reg(Register.EAX, 4)),
    2: ((Reg(Register.AX, 2), Reg(Register.DX, 2)), Reg(Register.AX, 2)),
}


def _multiply(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """`imul` in all three of its forms.

    The one-operand form writes the dx:ax (or edx:eax) pair the way `idiv`
    does -- two implicit destinations, the shape Semantics.dests already
    exists for. It is not what calls.py absorbs a B$MUI4 into: that is a
    plain `imul r32,rm32`, because AGENTS.md's own measurement is that
    B$MUI4 wraps exactly as `imul` does. Not in the 110-fixture corpus at
    all -- reported twice in bench/nbody.bas's own main body, for which
    there is no object here -- so what pins this shape is the unit case,
    not a census.
    """
    match insn.insn.op_count:
        case 1:
            factor = _location(insn, 0, resolve)
            if not isinstance(factor, Reg | Mem):
                return None
            found = WIDE_MULTIPLY.get(factor.width)
            return None if found is None else Semantics(op, name, found[0], (found[1], factor))
        case 2:
            dest, source = _destination(insn, 0, resolve), _location(insn, 1, resolve)
            return None if dest is None or source is None else Semantics(op, name, (dest,), (dest, source))
        case 3:
            dest = _destination(insn, 0, resolve)
            left, right = _location(insn, 1, resolve), _location(insn, 2, resolve)
            if dest is None or left is None or right is None:
                return None
            return Semantics(op, name, (dest,), (left, right))
        case _:
            return None


# idiv's own implicit pair, by the width of its one explicit operand:
# (quotient, remainder), and the dividend halves it reads. The 8-bit form
# uses ax alone for both halves and both results, a different shape
# altogether, so it is left out rather than bent to fit.
DIVIDE_PAIR = {
    4: ((Reg(Register.EAX, 4), Reg(Register.EDX, 4)), (Reg(Register.EDX, 4), Reg(Register.EAX, 4))),
    2: ((Reg(Register.AX, 2), Reg(Register.DX, 2)), (Reg(Register.DX, 2), Reg(Register.AX, 2))),
}


def _divide(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """`idiv rm`, which calls.py emits for a B$DVI4 or B$MDI4 it absorbs.

    Two destinations, both implicit -- quotient in the accumulator, remainder
    in its partner -- which is why Semantics carries `dests` as a tuple.
    """
    if insn.insn.op_count != 1:
        return None
    divisor = _location(insn, 0, resolve)
    if not isinstance(divisor, Reg | Mem):
        return None
    found = DIVIDE_PAIR.get(divisor.width)
    return None if found is None else Semantics(op, name, found[0], (*found[1], divisor))


def _compare(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    if insn.insn.op_count != 2:
        return None
    left, right = _location(insn, 0, resolve), _location(insn, 1, resolve)
    return None if left is None or right is None else Semantics(op, name, sources=(left, right))


def _unary(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    if insn.insn.op_count != 1:
        return None
    dest = _destination(insn, 0, resolve)
    return None if dest is None else Semantics(op, name, (dest,), (dest,))


# cwd/cdq: which register receives the sign of which, at what width. Only the
# two BC and this pass actually emit -- cbw/cwde would be one line each, and
# guessing them in advance is how a table stops being measured.
EXTEND_PAIR = {
    Mnemonic.CWD: (Reg(Register.DX, 2), Reg(Register.AX, 2)),
    Mnemonic.CDQ: (Reg(Register.EDX, 4), Reg(Register.EAX, 4)),
}


def _extend(insn: Insn, _resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    found = EXTEND_PAIR.get(insn.insn.mnemonic)
    return None if found is None else Semantics(op, name, (found[0],), (found[1],))


def _stack_slot(insn: Insn, index: int, resolve: Resolver) -> Loc | None:
    """One operand of a push or a pop, where a segment register is a location too.

    A push or a pop of one moves sixteen bits to or from the stack and changes
    no addressing on the way. BC emits three shapes of it and nothing else:
    `push cs` under the offset of the far pointer it hands B$OEGA (15 main
    bodies) and in the event-poll stub's own far-jump trampoline (14), and
    `push ss` / `pop es` to point es at the frame for PDS 7.1's /Ot
    `rep stosw` (1). Reading *through* a segment register is refused whole --
    lift.operand() declines a segment override outright -- which is why the
    allowance is here and not in _location(): `mov es,[x]` changes what every
    later es access means, and this pass has no notion of a segment to record
    that in.
    """
    if insn.insn.op_kind(index) == OpKind.REGISTER:
        register = insn.insn.op_register(index)
        if RegisterExt.is_segment_register(register):
            return Reg(register, RegisterExt.size(register))
    return _location(insn, index, resolve)


def _push(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    if insn.insn.op_count != 1:
        return None
    source = _stack_slot(insn, 0, resolve)
    return None if source is None else Semantics(op, name, sources=(source,))


def _pop(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    if insn.insn.op_count != 1:
        return None
    dest = _stack_slot(insn, 0, resolve)
    return None if not isinstance(dest, Reg | Mem) else Semantics(op, name, (dest,))


def _leave(insn: Insn, _resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """`leave` is exactly `mov sp,bp` then `pop bp`.

    PDS 7.1 under /Ot closes a procedure with it where every other
    configuration far-calls B$EXSA (extent.py's own docstring names that
    difference); one site in this corpus, procs-p-ot's own TWICE. The stack
    cell the pop reads is Effects' business rather than this layer's, the
    line Operation.POP already draws. Gated on the 16-bit encoding: `leaved`
    is one line more and BC emits none, and EXTEND_PAIR's own comment says
    why guessing it in advance is how a table stops being measured.
    """
    if insn.code != Code.LEAVEW:
        return None
    stack, frame = Reg(Register.SP, 2), Reg(Register.BP, 2)
    return Semantics(op, name, (stack, frame), (frame,))


def _fill(insn: Insn, _resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """`rep stosw`: cx words of ax written through es:di, di stepping by DF.

    PDS 7.1's /Ot prologue zeroes a procedure's whole frame this way, where
    every other configuration calls B$ENRA instead -- one site, procs-p-ot's
    own TWICE again. The destination is unnamed and unsized on purpose: es:di
    is not an address lift.operand() can name, and the extent is cx words
    rather than one, which is exactly what iced reports by giving that access
    MemorySize.UNKNOWN. An unnamed cell aliases everything, which is the
    answer a fill wants. Without the REP prefix the count is implicit and the
    shape is a different one; there is none in this corpus, so there is none
    here.
    """
    if insn.code != Code.STOSW_M16_AX or not insn.insn.has_rep_prefix:
        return None
    value, count = Reg(Register.AX, 2), Reg(Register.CX, 2)
    through, segment = Reg(Register.DI, 2), Reg(Register.ES, 2)
    return Semantics(op, name, (Mem(None, 0),), (value, count, through, segment))


def _transfer(insn: Insn, _resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """A jump or a conditional branch whose target is a computable offset."""
    target = insn.target
    return None if target is None else Semantics(op, name, target=target)


# A direct far branch's target: a segment:offset immediate, which is where a
# fixup writes rather than something computable from the instruction.
FAR_BRANCH = (OpKind.FAR_BRANCH16, OpKind.FAR_BRANCH32)


def _jump(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """A near `jmp` goes where the instruction says. A direct far `jmp` does not.

    `jmp far ptr 0:0` holds a zeroed segment:offset that a fixup names, so
    there is no edge in the instruction to build and none is invented: it
    becomes Operation.ESCAPE, which says control leaves the body and does not
    fall through, and says nothing else. Measured, all 14 in this corpus are
    the tail of an event-poll stub's own `pop ax / push cs / push ax /
    jmp far 0:0` trampoline -- the shape /V and /W emit to hand control back
    to the runtime.

    The indirect far forms stay barriers: they read their target out of
    memory, and AGENTS.md's own census finds not one `FF /4` or `/5` in any
    module, so there is nothing to measure a model against.
    """
    near = _transfer(insn, resolve, op, name)
    if near is not None:
        return near
    return Semantics(Operation.ESCAPE, name) if insn.insn.op0_kind in FAR_BRANCH else None


def _call(insn: Insn, _resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """A call site's own shape. Its *effect* stays conservative -- what the
    callee clobbers is the callee's business, and 30 distinct targets across
    this corpus is not a licence to guess at any of them."""
    return Semantics(op, name, target=insn.target)


def _nothing(insn: Insn, _resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """An instruction that does nothing at all -- padding between bodies."""
    return Semantics(op, name) if insn.insn.op_count == 0 else None


def _return(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    match insn.insn.op_count:
        case 0:
            return Semantics(op, name)
        case 1:
            popped = _location(insn, 0, resolve)
            return None if not isinstance(popped, Imm) else Semantics(op, name, sources=(popped,))
        case _:
            return None


BUILD: dict[Operation, Builder] = {
    Operation.MOVE: _move,
    Operation.ADDRESS: _address,
    Operation.BINARY: _binary,
    Operation.MULTIPLY: _multiply,
    Operation.DIVIDE: _divide,
    Operation.COMPARE: _compare,
    Operation.UNARY: _unary,
    Operation.EXTEND: _extend,
    Operation.PUSH: _push,
    Operation.POP: _pop,
    Operation.LEAVE: _leave,
    Operation.FILL: _fill,
    Operation.JUMP: _jump,
    Operation.BRANCH: _transfer,
    Operation.CALL: _call,
    Operation.RETURN: _return,
    Operation.NOTHING: _nothing,
}

# The vocabulary, keyed on the mnemonic rather than on iced's Code so that
# every encoding of one operation is covered by one line -- BC picks the
# shortest encoding independently for each half of a long (lift.py's
# IMM_FAMILY documents the three it chooses between), and this pass emits
# 32-bit forms of the same mnemonics on top of that. Operand shapes are read
# from iced generically, and a builder that cannot express one refuses.
#
# What is deliberately absent, and stays Operation.BARRIER -- carried
# verbatim, never reasoned about:
#
#   x87, both the real mnemonics and the emulator's own int 34h-3Dh sites.
#     Modelling them means representing an eight-deep register stack, and
#     floats are a later phase entirely; declen.py already decodes their
#     length, which is the whole of what carrying one needs.
#   `in` and `out`. What they do happens in a device, not in this machine,
#     and it is not knowledge to claim.
#   anything carrying a segment override. This pass has no notion of a
#     segment, so `es:[x]` and `ds:[x]` would read as the same location --
#     lift.operand()'s own refusal, quoted at instruction_semantics.
#   `mov es,[x]`, which carries no override and is refused for the other half
#     of the same reason: it changes what every later es access means, and
#     there is nowhere here to record that. Only a push or a pop of a segment
#     register is modelled (_stack_slot).
#   a byte-wide `imul` or `idiv`, whose product or quotient lands in ax alone
#     rather than in a pair. A different shape, and absent from this corpus.
#   the indirect far transfers, `FF /4` and `/5`. Not one appears in any
#     module (AGENTS.md's own census), so there is nothing to model against.
#
# RETF is its own mnemonic rather than a form of RET, which is why it was
# absent: _return already handled both its shapes. Measured, that one line
# is the whole reason no procedure in the corpus could be lifted -- every
# one of the 30 ends in `retf n`, and one unmodelled epilogue refuses the
# body it closes.
#
# LEAVE, STOSW, the segment-register push and pop (_stack_slot), the widening
# one-operand `imul` (WIDE_MULTIPLY) and the far `jmp` (_jump) came in
# together, and between them they were every unmodelled instruction in this
# corpus: 47 of 17970, refusing 30 of its 154 bodies. None of the five needed
# a guess -- each is either a move, a frame mechanic, or control leaving.
SHAPE: dict[int, tuple[Operation, str]] = {
    Mnemonic.MOV: (Operation.MOVE, "mov"),
    Mnemonic.LEA: (Operation.ADDRESS, "lea"),
    Mnemonic.ADD: (Operation.BINARY, "add"),
    Mnemonic.ADC: (Operation.BINARY, "adc"),
    Mnemonic.SUB: (Operation.BINARY, "sub"),
    Mnemonic.SBB: (Operation.BINARY, "sbb"),
    Mnemonic.AND: (Operation.BINARY, "and"),
    Mnemonic.OR: (Operation.BINARY, "or"),
    Mnemonic.XOR: (Operation.BINARY, "xor"),
    Mnemonic.SHL: (Operation.BINARY, "shl"),
    Mnemonic.SHR: (Operation.BINARY, "shr"),
    Mnemonic.SAR: (Operation.BINARY, "sar"),
    Mnemonic.IMUL: (Operation.MULTIPLY, "imul"),
    Mnemonic.IDIV: (Operation.DIVIDE, "idiv"),
    Mnemonic.CMP: (Operation.COMPARE, "cmp"),
    Mnemonic.TEST: (Operation.COMPARE, "test"),
    Mnemonic.NEG: (Operation.UNARY, "neg"),
    Mnemonic.NOT: (Operation.UNARY, "not"),
    Mnemonic.INC: (Operation.UNARY, "inc"),
    Mnemonic.DEC: (Operation.UNARY, "dec"),
    Mnemonic.CWD: (Operation.EXTEND, "cwd"),
    Mnemonic.CDQ: (Operation.EXTEND, "cdq"),
    Mnemonic.NOP: (Operation.NOTHING, "nop"),
    Mnemonic.PUSH: (Operation.PUSH, "push"),
    Mnemonic.POP: (Operation.POP, "pop"),
    Mnemonic.LEAVE: (Operation.LEAVE, "leave"),
    Mnemonic.STOSW: (Operation.FILL, "stosw"),
    Mnemonic.JMP: (Operation.JUMP, "jmp"),
    Mnemonic.CALL: (Operation.CALL, "call"),
    Mnemonic.RET: (Operation.RETURN, "ret"),
    Mnemonic.RETF: (Operation.RETURN, "retf"),
    Mnemonic.JA: (Operation.BRANCH, "ja"),
    Mnemonic.JAE: (Operation.BRANCH, "jae"),
    Mnemonic.JB: (Operation.BRANCH, "jb"),
    Mnemonic.JBE: (Operation.BRANCH, "jbe"),
    Mnemonic.JE: (Operation.BRANCH, "je"),
    Mnemonic.JG: (Operation.BRANCH, "jg"),
    Mnemonic.JGE: (Operation.BRANCH, "jge"),
    Mnemonic.JL: (Operation.BRANCH, "jl"),
    Mnemonic.JLE: (Operation.BRANCH, "jle"),
    Mnemonic.JNE: (Operation.BRANCH, "jne"),
    Mnemonic.JNO: (Operation.BRANCH, "jno"),
    Mnemonic.JNP: (Operation.BRANCH, "jnp"),
    Mnemonic.JNS: (Operation.BRANCH, "jns"),
    Mnemonic.JO: (Operation.BRANCH, "jo"),
    Mnemonic.JP: (Operation.BRANCH, "jp"),
    Mnemonic.JS: (Operation.BRANCH, "js"),
}


def instruction_semantics(insn: Insn, resolve: Resolver) -> Semantics:
    """What one real instruction computes, or Operation.BARRIER.

    A segment-override prefix is refused outright, the way lift.operand()
    refuses one: this pass has no notion of a segment, so `es:[x]` and
    `ds:[x]` would otherwise read as the same location.
    """
    found = SHAPE.get(insn.insn.mnemonic)
    if found is None or insn.has_segment_override:
        return UNMODELLED
    op, name = found
    return BUILD[op](insn, resolve, op, name) or UNMODELLED


@dataclass(frozen=True, slots=True)
class Opaque:
    """An instruction with no *idiom* this pass recognises by name.

    Its own operation may still be modelled -- `semantics.op` says, and
    Operation.BARRIER is the one value that means nothing is claimed. The
    two are separate on purpose; see this module's own docstring.
    """

    insn: Insn
    effects: Effects
    semantics: Semantics = UNMODELLED


@dataclass(frozen=True, slots=True)
class Long:
    """One of lift.classify()'s six single-instruction long-pair shapes."""

    insn: Insn
    decoded: Decoded
    effects: Effects
    semantics: Semantics = UNMODELLED


@dataclass(frozen=True, slots=True)
class Call:
    """A far call whose target a fixup names (module.calls) -- a runtime
    routine or a user external, not only the ones calls.py knows how to
    absorb (calls.py's own LEFT_FIRST vocabulary is deliberately not
    imported here: "the target is named" is all this layer asserts)."""

    insn: Insn
    name: str
    effects: Effects
    semantics: Semantics = UNMODELLED


# The pair each restore idiom's own bytes belong to, per lift.FIXUP -- reused
# rather than re-declared, since the bytes have to be exactly these to be
# this idiom at all.
RESTORE_EFFECTS = {
    # Net stack effect is nothing: sp returns to where it started and
    # nothing outside this idiom ever reads the stack cells it transiently
    # used, so no load or store is reported even though push/pop
    # individually touch memory. Writes no flags at all -- lift.py chose
    # this idiom over `shr` specifically because it does not, and
    # tests/test_flags.py asserts that transparency. Both halves' `pop` is a
    # partial write of its own root (`pop ax` touches only eax's low 16
    # bits), so -- per _register_effects' own rule for a partial write --
    # both roots belong in `uses` too, not only `defs`.
    0: Effects(frozenset({Register.EAX, Register.EDX}), frozenset({Register.EAX, Register.EDX}), Flag.NONE),
    1: Effects(frozenset({Register.ECX, Register.EBX}), frozenset({Register.ECX, Register.EBX}), Flag.NONE),
}


@dataclass(frozen=True, slots=True)
class Restore:
    """calls.py's own `push e?x / pop ?x / pop ?x` idiom, byte-identical to
    lift.FIXUP[pair] -- puts a widened value's high half back where BC's
    un-widened code reads it. BC never emits this; it exists in real code
    only after this pass's own absorption has already run once, so it is
    exercised by re-decoding a rewritten object, not by the untouched
    110-fixture corpus."""

    at: int
    end: int
    pair: int
    effects: Effects
    semantics: Semantics = RESTORE_IDIOM


class TableKind(StrEnum):
    # B$OGTA's own inline data: real jump targets, per extent._table_targets.
    JUMP = "jump"
    # anything else a TABLE-ending block owns -- the /X RESUME map is the
    # one blocks.py names, data nothing jumps into.
    MAP = "map"


@dataclass(frozen=True, slots=True)
class Data:
    """Bytes a Body owns that are not instructions at all -- always an
    inline table appended to a body's range by extent.py's own _ranges()."""

    at: int
    end: int
    kind: TableKind
    entries: tuple[int, ...]
    effects: Effects
    semantics: Semantics = TABLE_DATA


type Node = Opaque | Long | Call | Restore | Data


def pinned(node: Node) -> frozenset[Register_] | None:
    """Which registers a register allocator may not reassign at this node.

    Empty for anything modelled -- being free to rename its registers is most
    of what modelling it was for. For a barrier it is every root the
    instruction touches, because a barrier's behaviour is its encoding's and
    an encoding names physical registers rather than values: `rep stosw`
    fills through es:di and counts cx, and the same bytes stepping si would
    be a different instruction, not this one renamed.

    None, as everywhere else here, is "assume every register" -- the answer
    for a barrier whose flow already made Effects say so, an `int 21h` among
    them.
    """
    if not barrier(node.semantics):
        return frozenset()
    if node.effects.defs is None or node.effects.uses is None:
        return None
    return node.effects.defs | node.effects.uses


def span(node: Node) -> tuple[int, int]:
    match node:
        case Opaque(insn=insn) | Long(insn=insn) | Call(insn=insn):
            return insn.at, insn.end
        case Restore(at=at, end=end) | Data(at=at, end=end):
            return at, end


def emit(module: Module, nodes: tuple[Node, ...]) -> bytes:
    """A node list's own bytes, verbatim -- never reconstructed, always
    sliced from the original code by each node's own span. See this
    module's own docstring for why that is deliberate."""
    return b"".join(module.code[lo:hi] for lo, hi in map(span, nodes))


def _restore_at(module: Module, insns_by_at: dict[int, Insn], at: int, hi: int) -> Restore | None:
    """A restore idiom starting exactly at `at`, or None.

    Guarded on real instruction starts, not just a byte match: `at+2` (pop
    lo16) and `at+3` (pop hi16) must themselves be instruction boundaries
    inside this range, so a coincidental byte match cannot claim to be this
    idiom while actually straddling something else.
    """
    if at + 4 > hi:
        return None
    for pair, pattern in FIXUP.items():
        if module.code[at : at + 4] != pattern:
            continue
        second, third = insns_by_at.get(at + 2), insns_by_at.get(at + 3)
        if second is None or second.end != at + 3 or third is None or third.end != at + 4:
            continue
        return Restore(at, at + 4, pair, RESTORE_EFFECTS[pair])
    return None


def _table_node(module: Module, last: Node | None, lo: int, hi: int) -> Data:
    kind = TableKind.MAP
    if isinstance(last, Call) and last.insn.end == lo and last.name in INLINE_TABLE:
        kind = TableKind.JUMP
    # `lo` is a real fixup site for a MAP table (blocks.unexplained_tables()
    # returns the first fixup offset itself as its span's own start) but
    # never one for a JUMP table (`lo` there is B$OGTA's own count byte, one
    # short of its first offset16 entry) -- `<=` is correct for both, since
    # a JUMP table's `lo` is simply never a key in module.operands.
    entries = tuple(sorted(at for at in module.operands if lo <= at < hi))
    return Data(lo, hi, kind, entries, NO_EFFECT)


def _instruction_node(module: Module, insn: Insn) -> Node:
    effects = instruction_effects(insn, module.resolve)
    semantics = instruction_semantics(insn, module.resolve)
    if insn.at in module.calls:
        return Call(insn, module.calls[insn.at], effects, semantics)
    if (decoded := classify(insn, module.resolve)) is not None:
        return Long(insn, decoded, effects, semantics)
    return Opaque(insn, effects, semantics)


def decode_body(module: Module, mapped: CodeMap, blocks: list[Block], body: Body) -> tuple[Node, ...]:
    """Every byte of `body`'s own ranges, as ordered Nodes.

    Instruction boundaries come from `blocks` (already the exact, reachability-
    proven decode blocks.py produced) rather than a second, independent
    decode walk -- one source of truth for "where an instruction is", the
    same discipline extent.py itself follows.
    """
    insns_by_at = {insn.at: insn for block in blocks for insn in block.insns}
    tables_by_start = dict(mapped.tables)

    nodes: list[Node] = []
    last: Node | None = None
    for lo, hi in body.ranges:
        at = lo
        while at < hi:
            if at in tables_by_start:
                node: Node = _table_node(module, last, at, tables_by_start[at])
            elif (restore := _restore_at(module, insns_by_at, at, hi)) is not None:
                node = restore
            else:
                node = _instruction_node(module, insns_by_at[at])
            nodes.append(node)
            last = node
            at = span(node)[1]
    return tuple(nodes)


@dataclass(frozen=True, slots=True)
class BodyIR:
    body: Body
    nodes: tuple[Node, ...]


def decode_module(module: Module) -> tuple[BodyIR, ...] | str:
    """Every body of `module`, total-decoded -- or why it could not be."""
    mapped = code_map(module)
    if isinstance(mapped, str):
        return mapped
    found = body_partition(module)
    if isinstance(found, str):
        return found
    if not found.complete:
        return _incomplete(found)
    blocks = block_partition(module, mapped)
    return tuple(BodyIR(body, decode_body(module, mapped, blocks, body)) for body in found.bodies)


def _incomplete(found: Partition) -> str:
    return f"{len(found.unexplained)} unexplained range(s), {len(found.conflicts)} conflicting"
