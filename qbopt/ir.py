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

  Effects  -- what the instruction disturbs. Always iced's own answer, never
              this module's opinion, and rooted to the 32-bit parent (see
              ROOT) so "does this touch the ax pair" is decided once. It is
              the authority on def/use, and it is complete for every node.
  Semantics -- what the instruction *computes*: an operation, its
              destinations and its sources as typed locations. Complete where
              `op` is not Operation.OPAQUE. Its registers are the literal
              ones iced decoded, unrooted, because a value's identity is
              `ax`, not "somewhere in eax" -- the same distinction
              registers.py's own docstring draws for liveness.

So Semantics is what a value-numbering or constant-folding pass reads, and
Effects is what a code-motion or dead-store pass reads. A Semantics that is
merely absent costs precision; an Effects that is wrong costs correctness,
which is why nothing here ever narrows an effect below what iced reports.

The node *type* says which idiom claimed the node -- lift.classify()'s six
single-instruction long-pair forms, a far call a fixup names, calls.py's own
three-instruction "restore" idiom, an inline table -- and deliberately not
whether its operation is modelled. Adding an encoding to the vocabulary
therefore never reshuffles a consumer's own isinstance checks; it only fills
in a Semantics that used to be Operation.OPAQUE.

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
    """What a node computes. OPAQUE is the refusal, and it is always safe:
    a pass that cannot read a node's operation refuses the region."""

    MOVE = "move"  # dests[0] <- sources[0]
    ADDRESS = "addr"  # dests[0] <- the numeric value of sources[0]
    BINARY = "binary"  # dests[0] <- sources[0] `name` sources[1], and sources[0] IS dests[0]
    MULTIPLY = "mul"  # dests[0] <- sources[0] * sources[1]; unlike BINARY, no source need be the dest
    DIVIDE = "div"  # dests[0] <- the quotient and dests[1] <- the remainder of sources[0]:sources[1] / sources[2]
    COMPARE = "cmp"  # flags only, from sources[0] and sources[1]
    UNARY = "unary"  # dests[0] <- `name` sources[0], and sources[0] IS dests[0]
    EXTEND = "extend"  # dests[0] <- the sign of sources[0]: cwd, cdq
    PUSH = "push"  # sources[0] onto the stack; the cell and sp are Effects' business, not this layer's
    POP = "pop"  # dests[0] off the stack, likewise
    JUMP = "jump"  # unconditional, to `target`
    BRANCH = "branch"  # conditional on the flags, to `target`
    CALL = "call"
    RETURN = "ret"
    RESTORE = "restore"  # calls.py's own idiom -- see Restore
    DATA = "data"  # not an instruction at all -- see Data
    OPAQUE = "opaque"


@dataclass(frozen=True, slots=True)
class Semantics:
    """What one node computes, as typed locations.

    Complete only where `op` is not Operation.OPAQUE. `name` is the operation's
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


UNMODELLED = Semantics(Operation.OPAQUE)
RESTORE_IDIOM = Semantics(Operation.RESTORE, "restore")
TABLE_DATA = Semantics(Operation.DATA)


def modelled(semantics: Semantics) -> bool:
    return semantics.op is not Operation.OPAQUE


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
    """The conservative, iced-derived effect of one real instruction."""
    if insn.flow in CLOBBERS:
        # The callee's flag reads are as unknowable as its writes. Costs
        # nothing in practice -- flags_written is already ALL, so nothing
        # set before the call survives it either way.
        return Effects(None, None, written_by(insn), ALL, ANY_MEMORY, ANY_MEMORY)
    defs, uses = _register_effects(insn)
    loads, stores = _memory_effects(insn, resolve)
    return Effects(defs, uses, written_by(insn), Flag(insn.reads & ALL), loads, stores)


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
    Operation.OPAQUE rather than being described half-right.
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


def _multiply(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """`imul` in its two- and three-operand forms only.

    The one-operand form reads and writes the dx:ax (or edx:eax) pair the
    way `idiv` does, and is not what calls.py absorbs a B$MUI4 into -- that
    is a plain `imul r32,rm32`, because AGENTS.md's own measurement is that
    B$MUI4 wraps exactly as `imul` does.
    """
    dest = _destination(insn, 0, resolve)
    if dest is None:
        return None
    match insn.insn.op_count:
        case 2:
            source = _location(insn, 1, resolve)
            return None if source is None else Semantics(op, name, (dest,), (dest, source))
        case 3:
            left, right = _location(insn, 1, resolve), _location(insn, 2, resolve)
            return None if left is None or right is None else Semantics(op, name, (dest,), (left, right))
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


def _push(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    if insn.insn.op_count != 1:
        return None
    source = _location(insn, 0, resolve)
    return None if source is None else Semantics(op, name, sources=(source,))


def _pop(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    if insn.insn.op_count != 1:
        return None
    dest = _destination(insn, 0, resolve)
    return None if dest is None else Semantics(op, name, (dest,))


def _transfer(insn: Insn, _resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """A jump or a conditional branch whose target is a computable offset.

    A far jump (`jmp 0:0`, the event stub's own tail) reports no near-branch
    target at all and stays opaque: where it goes is not in the instruction.
    """
    target = insn.target
    return None if target is None else Semantics(op, name, target=target)


def _call(insn: Insn, _resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """A call site's own shape. Its *effect* stays conservative -- what the
    callee clobbers is the callee's business, and 30 distinct targets across
    this corpus is not a licence to guess at any of them."""
    return Semantics(op, name, target=insn.target)


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
    Operation.JUMP: _transfer,
    Operation.BRANCH: _transfer,
    Operation.CALL: _call,
    Operation.RETURN: _return,
}

# The vocabulary, keyed on the mnemonic rather than on iced's Code so that
# every encoding of one operation is covered by one line -- BC picks the
# shortest encoding independently for each half of a long (lift.py's
# IMM_FAMILY documents the three it chooses between), and this pass emits
# 32-bit forms of the same mnemonics on top of that. Operand shapes are read
# from iced generically, and a builder that cannot express one refuses.
#
# What is deliberately absent, and stays Operation.OPAQUE: the x87 emulator's
# own int 34h-3Dh sites (declen.py decodes their length; floats are a later
# phase entirely), `retf n`, `push cs`/`push ss`/`pop es`, `stosw`, `leave`,
# one-operand `imul`, a byte-wide `idiv`, and a far `jmp`. Refusing is always
# safe; a wrong effect is not.
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
    Mnemonic.PUSH: (Operation.PUSH, "push"),
    Mnemonic.POP: (Operation.POP, "pop"),
    Mnemonic.JMP: (Operation.JUMP, "jmp"),
    Mnemonic.CALL: (Operation.CALL, "call"),
    Mnemonic.RET: (Operation.RETURN, "ret"),
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
    """What one real instruction computes, or Operation.OPAQUE.

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
    Operation.OPAQUE is the one value that means nothing is claimed. The
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
