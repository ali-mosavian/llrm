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
from dataclasses import field
from dataclasses import replace
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
    is the one place that rule lives.

    `through` is the register the operand is reached by, kept even where the
    address cannot be named: `mov ax,[si]` has no displacement and so no
    fixup, so nothing can say which bytes it means -- and it is still an
    instruction something has to be able to emit.

    Out of the comparison on purpose. It is how to encode the operand, not
    which bytes it is, and two cells with an unknown address were already
    equal whichever register reached them. Including it would change what
    every consumer means by "the same cell" to buy nothing: a based address
    is never provably disjoint from anything either way.
    """

    addr: Addr | None
    width: int
    through: Register_ = field(default=Register.NONE, compare=False)
    offset: int = field(default=0, compare=False)
    # How wide the displacement field was, which is not the same as how wide
    # the number needs: `mov ax,[bx]` in d_poly is `8b 87 00 00`, a two-byte
    # displacement of zero because a fixup fills it in. Emitted without one
    # it is `8b 07`, the right instruction reading the wrong address.
    disp_width: int = field(default=0, compare=False)
    # The value that computed the address, where one did. In the
    # comparison, unlike `through`: two cells reached by different values
    # are different cells, and conflating them is how arrprm stored both
    # array elements through one stale register. `through` stays what it
    # was -- how to encode the operand once something has placed it.
    base: "Held | None" = field(default=None)


@dataclass(frozen=True, slots=True)
class Held:
    """Whatever register holds this SSA value, resolved at emission.

    The operand kind rule 5 needs. A pass rewriting an operand from a cell
    to a register had no way to say "the register this value is in" and so
    said `ir.Reg(register=BX)` -- naming a register, which is the
    allocator's answer and not a transform's. forward.py holds 22 machine
    references for exactly this reason.

    `value` is a mir.Value's id rather than the value, because ir.py is
    below mir.py and may not import it. select.emit resolves it through the
    allocation; an unresolved Held is a bug and emit refuses rather than
    guessing a register.
    """

    value: int
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
    Address source reads no memory and appears in no Effects.loads.

    The rest is how to encode it, out of the comparison the way ir.Mem's is.
    `lea` is where an address stops being a name and becomes arithmetic:
    calls.py writes `lea eax,[eax+eax*2]` for a multiply by three, and there
    is no `addr` for that at all -- the scale IS the operation.
    """

    addr: Addr | None
    through: Register_ = field(default=Register.NONE, compare=False)
    index: Register_ = field(default=Register.NONE, compare=False)
    scale: int = field(default=1, compare=False)
    offset: int = field(default=0, compare=False)
    disp_width: int = field(default=0, compare=False)


@dataclass(frozen=True, slots=True)
class St:
    """One x87 stack register, `st(index)` -- named exactly as the operand
    names it, relative to whatever the stack's own top currently is.

    Not a fixed physical location the way Reg's registers are: `fld` pushes,
    so `st(0)` in one node's own dests and `st(0)` in the next node's own
    sources are two different physical registers if anything in between
    pushed or popped. Nothing in this module tracks that rotation across
    nodes -- see the x87 section of SHAPE's own comment -- so two St
    mentions in different nodes are never claimed to name the same value;
    only ever compared within the one node that names them both.
    """

    index: int


type Loc = Reg | Mem | Imm | Address | St


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

    `fp_stack` is the one further resource worth keeping separate from both
    memory and the GPR roots: the x87 register stack an `fld`-family
    instruction pushes, an `fstp`-family one pops, or an in-place one
    (`fadd m32fp`, `fchs`) reads and writes without moving. It is not a Mem
    cell, and it is not always visible in `defs`/`uses` either -- iced's own
    used_registers() names no destination register for a push at all (there
    is nothing existing to name the new top with), so `fp_stack` is what
    still says a push touched something. See _touches_fp_stack.
    """

    defs: frozenset[Register_] | None
    uses: frozenset[Register_] | None
    flags_written: Flag
    flags_read: Flag = Flag.NONE
    loads: tuple[Mem, ...] = ()
    stores: tuple[Mem, ...] = ()
    fp_stack: bool = False

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
    # a genuine swap: dests[0] <- sources[0] and dests[1] <- sources[1], where sources[0] is the OTHER
    # operand's old value -- both operands are read and both written, unlike MOVE's one-way copy.
    EXCHANGE = "xchg"
    ADDRESS = "addr"  # dests[0] <- the numeric value of sources[0]
    BINARY = "binary"  # dests[0] <- sources[0] `name` sources[1], and sources[0] IS dests[0]
    # dests[0] <- sources[0] * sources[1]; unlike BINARY, no source need be the dest. The widening
    # one-operand form has two destinations instead: dests[0] the low half, dests[1] the high.
    MULTIPLY = "mul"
    DIVIDE = "div"  # dests[0] <- the quotient and dests[1] <- the remainder of sources[0]:sources[1] / sources[2]
    COMPARE = "cmp"  # flags only, from sources[0] and sources[1]
    UNARY = "unary"  # dests[0] <- `name` sources[0], and sources[0] IS dests[0]
    # dests[0] <- (sources[1]:dests[0]) shifted right by sources[2], and
    # sources[0] IS dests[0]. Two registers shifted as one number, which is
    # how a 64-bit product is brought back down to 32: `shrd`. The count is
    # an immediate or cl, and nothing else about it is fixed.
    FUNNEL = "funnel"
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

    # The x87 vocabulary. A separate stack (St, not Reg or Mem) is why these
    # need their own shapes rather than reusing BINARY/UNARY/MOVE: "dest IS
    # sources[0]" (BINARY's own rule) is not true of the popping arithmetic
    # forms, whose dest is st(i) but whose first source is also st(i) only
    # in the pre-pop numbering, and "push" and "pop" are effects integer
    # BINARY/UNARY have nothing analogous to at all. `wait` stays NOTHING --
    # SHAPE's own comment says why it needs no shape of its own.
    FLOAT_LOAD = "fload"  # dests[0] (the new st(0)) <- sources[0]; fld/fild push
    FLOAT_STORE = "fstore"  # dests[0] <- sources[0] (st(0)), then the stack pops; fstp/fistp
    FLOAT_ARITH = "farith"  # dests[0] (st(0)) <- dests[0] `name` sources[1]; no push, no pop
    FLOAT_ARITH_POP = "farithp"  # dests[0] (st(i)) <- dests[0] `name` sources[1] (st(0)), then pops
    FLOAT_UNARY = "funary"  # dests[0] (st(0)) <- `name` sources[0] (st(0)); no push, no pop

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
def values(where) -> "list[Held]":
    """Every abstract value an operand names, nested ones included.

    One place, because "which values does this read" is one question and
    an operand may hold another: a memory cell names the value that
    computed its address, and an instruction that does not record reading
    it tells the allocator the range ended.
    """
    if isinstance(where, Held):
        return [where]
    if isinstance(where, Mem) and where.base is not None:
        return [where.base]
    return []


def mapped(where, made):
    """`where` with every abstract value it names put through `made`."""
    if isinstance(where, Held):
        return made(where)
    if isinstance(where, Mem) and where.base is not None:
        return replace(where, base=made(where.base))
    return where


UNMODELLED = Semantics(Operation.BARRIER)
RESTORE_IDIOM = Semantics(Operation.RESTORE, "restore")


def restoring(wide: "Loc", low: "Loc", high: "Loc") -> Semantics:
    """A wide value split back into the two halves BC's own code reads.

    The one place a restore is built with operands. It used to have none:
    a `pair` number stood for the registers, so whoever made the node
    chose them and the allocation was told rather than asked. Saying which
    value and which halves lets select encode whatever the allocation
    picked, and `RESTORE_EFFECTS`' two pairs become two of the answers
    rather than the only ones.
    """
    return Semantics(Operation.RESTORE, "restore", dests=(low, high), sources=(wide,))


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


def _touches_fp_stack(insn: Insn) -> bool:
    """Whether this instruction pushes, pops, or reads/writes any x87 stack
    register -- the fact Effects.fp_stack carries.

    iced's own fpu_stack_increment_info() reports the push and the pop
    (`fld`/`fild` are -1, `fstp`/`fistp` and the `p`-suffixed arithmetic are
    +1); it is 0 for an instruction that touches the stack without moving
    it, such as `fadd m32fp` (reads and writes st(0) in place) or `fchs`
    (likewise) -- which is what the used_registers() half catches instead,
    via RegisterExt.is_st(), true of exactly Register.ST0..ST7, the one
    register family ROOT has no entry for and _register_effects() therefore
    already passes through into `defs`/`uses` unchanged wherever iced names
    one. `wait` sets neither: it reads no st(i), and iced's own info agrees
    it moves nothing, which is what licenses treating it (in SHAPE, as
    Operation.NOTHING) as touching no resource this module tracks at all,
    rather than guessing at a coupling to whatever FP instruction sits next
    to it.
    """
    if insn.insn.fpu_stack_increment_info().increment != 0:
        return True
    return any(RegisterExt.is_st(one.register) for one in INFO.info(insn.insn).used_registers())


def instruction_effects(insn: Insn, resolve: Resolver) -> Effects:
    """The conservative effect of one real instruction.

    iced's own answer, widened in the two places where the encoding is not
    the whole story and never narrowed anywhere.

    A call or interrupt, because the real effect is the callee's -- including
    its own reach into the x87 stack, which nothing here can rule out any
    more than a register or a memory cell, so `fp_stack` is True for exactly
    the reason `loads`/`stores` are ANY_MEMORY: unknown, not absent.

    A barrier, in its memory reach only. `out` is the one that stays one:
    programming a DMA controller through a port writes memory this layer
    cannot see. A handful of x87 shapes stay barriers too -- SHAPE's own
    comment names them -- and get the same ANY_MEMORY treatment; the 18
    shapes that *are* modelled do not, even on an emulated site, and
    instruction_semantics' own x87 section says what makes that safe.
    Registers are *not* widened to match a barrier's memory reach, because a
    barrier already pins them and nothing may be reordered across it -- so
    exact liveness across one costs no safety.
    """
    if insn.flow in CLOBBERS:
        # The callee's flag reads are as unknowable as its writes. Costs
        # nothing in practice -- flags_written is already ALL, so nothing
        # set before the call survives it either way.
        return Effects(None, None, written_by(insn), ALL, ANY_MEMORY, ANY_MEMORY, True)
    defs, uses = _register_effects(insn)
    read = Flag(insn.reads & ALL)
    fp_stack = _touches_fp_stack(insn)
    if barrier(instruction_semantics(insn, resolve)):
        return Effects(defs, uses, written_by(insn), read, ANY_MEMORY, ANY_MEMORY, fp_stack)
    loads, stores = _memory_effects(insn, resolve)
    return Effects(defs, uses, written_by(insn), read, loads, stores, fp_stack)


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

    A segment register is a location like any other GPR -- `mov es,[si+2]`
    reloads it the same way `mov ax,[si+2]` reloads ax, and Reg already
    carries a register rather than assuming one of the roots ROOT names, so
    nothing about it needs a GPR. The ISA is what keeps this from over-
    claiming: es/ds/ss/cs appear only as a mov, push or pop operand, never in
    an arithmetic one, so BINARY/COMPARE/etc. never actually see one here.

    None is the refusal that keeps the vocabulary honest: a far branch, a
    string operand's implicit es:di, a segment override this pass cannot
    name (lift.operand()'s own refusal) -- anything a builder cannot express
    reaches here, and the whole instruction falls back to Operation.BARRIER
    rather than being described half-right.
    """
    match insn.insn.op_kind(index):
        case OpKind.REGISTER:
            register = insn.insn.op_register(index)
            if RegisterExt.is_gpr(register) or RegisterExt.is_segment_register(register):
                return Reg(register, RegisterExt.size(register))
            return None
        case OpKind.MEMORY:
            return Mem(
                long_operand(insn, resolve),
                MemorySizeExt.size(insn.insn.memory_size),
                insn.insn.memory_base,
                insn.displacement,
                insn.disp_len,
            )
        case kind if kind in IMMEDIATE_WIDTH:
            width = IMMEDIATE_WIDTH[kind]
            return Imm(to_signed(insn.insn.immediate(index) & ((1 << (width * 8)) - 1), width), width)
        case _:
            return None


def _destination(insn: Insn, index: int, resolve: Resolver) -> Loc | None:
    found = _location(insn, index, resolve)
    return found if isinstance(found, Reg | Mem) else None


def _stack_register(insn: Insn, index: int) -> St | None:
    """One x87 register operand, `st(i)` -- the one shape `_location`
    refuses on purpose (RegisterExt.is_gpr() is false of it, same as any
    other non-GPR register), so the float builders below read it themselves
    rather than widen the shared GPR helper."""
    if insn.insn.op_kind(index) != OpKind.REGISTER:
        return None
    register = insn.insn.op_register(index)
    return St(register - Register.ST0) if RegisterExt.is_st(register) else None


type Builder = Callable[[Insn, Resolver, Operation, str], Semantics | None]


def _move(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    if insn.insn.op_count != 2:
        return None
    dest = _destination(insn, 0, resolve)
    source = _location(insn, 1, resolve)
    return None if dest is None or source is None else Semantics(op, name, (dest,), (source,))


def _exchange(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """`xchg`: both operands read AND written, each receiving the other's old
    value. Both must be destinations (a register or a memory cell) -- xchg
    has no immediate form. Measured, all 74 in qb-qrender are register pairs
    (`xchg bx,ax` 54, `xchg cx,ax` 20), never through memory."""
    if insn.insn.op_count != 2:
        return None
    first, second = _destination(insn, 0, resolve), _destination(insn, 1, resolve)
    return None if first is None or second is None else Semantics(op, name, (first, second), (second, first))


def _address(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    if insn.insn.op_count != 2 or insn.insn.op_kind(1) != OpKind.MEMORY:
        return None
    dest = _destination(insn, 0, resolve)
    if dest is None:
        return None
    where = Address(
        long_operand(insn, resolve),
        insn.insn.memory_base,
        insn.insn.memory_index,
        insn.insn.memory_index_scale,
        insn.displacement,
        insn.disp_len,
    )
    return Semantics(op, name, (dest,), (where,))


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


# push/pop of a segment register moves sixteen bits to or from the stack and
# changes no addressing on the way -- _location() already models one as a
# Reg like any other, so push and pop need nothing beyond it. BC emits three
# shapes and nothing else: `push cs` under the offset of the far pointer it
# hands B$OEGA (15 main bodies) and in the event-poll stub's own far-jump
# trampoline (14), and `push ss` / `pop es` to point es at the frame for
# PDS 7.1's /Ot `rep stosw` (1).


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


def _float_load(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """`fld`/`fild`: pushes the one real memory operand onto the stack.

    iced's own used_registers() names no destination for a push at all --
    there is no existing register to name the new top with, since nothing
    was there before -- so St(0) here is this module's own convention for
    "the top, right after this instruction" rather than something iced
    itself reports. The register form (`fld st(i)`, a stack duplicate) is
    not in qb-qrender's own object corpus and stays a barrier; SHAPE's own
    comment says so.
    """
    if insn.insn.op_count != 1 or insn.insn.op_kind(0) != OpKind.MEMORY:
        return None
    source = _location(insn, 0, resolve)
    return None if source is None else Semantics(op, name, (St(0),), (source,))


def _float_store(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """`fstp`/`fistp`: the current top, written to the one real memory
    operand, then popped. `name` alone tells a real store from an
    int-converting one -- the width comes from the operand itself either
    way, via MemorySizeExt.size().
    """
    if insn.insn.op_count != 1 or insn.insn.op_kind(0) != OpKind.MEMORY:
        return None
    dest = _location(insn, 0, resolve)
    return None if dest is None else Semantics(op, name, (dest,), (St(0),))


def _float_arith(insn: Insn, resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """`fadd`/`fsub`/`fmul`/`fdiv`/`fidiv`/`fisub`, the memory form only:
    the top combined with the one real memory operand, in place -- no push,
    no pop. Refuses the register-register form of the same mnemonic (`fadd
    st(0),st(1)`, a different shape iced files under the same Mnemonic)
    rather than guess that it does not pop either -- it does not, but BC
    never emits it (measured over qb-qrender's own object corpus), so
    nothing here claims it.
    """
    if insn.insn.op_count != 1 or insn.insn.op_kind(0) != OpKind.MEMORY:
        return None
    source = _location(insn, 0, resolve)
    return None if source is None else Semantics(op, name, (St(0),), (St(0), source))


def _float_arith_pop(insn: Insn, _resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """`faddp`/`fsubp`/`fmulp`/`fdivp st(i),st(0)`: combines the two, writes
    the result to st(i) -- named pre-pop, the only numbering this operand
    ever had -- then pops. Refuses any second operand other than st(0),
    which is the only pairing BC's own object corpus ever contains.
    """
    if insn.insn.op_count != 2:
        return None
    dest, second = _stack_register(insn, 0), _stack_register(insn, 1)
    if dest is None or second is None or second.index != 0:
        return None
    return Semantics(op, name, (dest,), (dest, second))


def _float_unary(insn: Insn, _resolve: Resolver, op: Operation, name: str) -> Semantics | None:
    """`fchs`/`fabs`/`fsqrt`: the top, transformed in place -- no operand,
    no push, no pop."""
    return None if insn.insn.op_count != 0 else Semantics(op, name, (St(0),), (St(0),))


BUILD: dict[Operation, Builder] = {
    Operation.MOVE: _move,
    Operation.EXCHANGE: _exchange,
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
    Operation.FLOAT_LOAD: _float_load,
    Operation.FLOAT_STORE: _float_store,
    Operation.FLOAT_ARITH: _float_arith,
    Operation.FLOAT_ARITH_POP: _float_arith_pop,
    Operation.FLOAT_UNARY: _float_unary,
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
#   `in` and `out`. What they do happens in a device, not in this machine,
#     and it is not knowledge to claim.
#   a segment override this pass cannot name -- lift.operand()'s own refusal,
#     quoted at instruction_semantics: everything but `es:[bx]`/`es:[bx+2]`,
#     the one shape qb-qrender's own 11,150 segment-override instructions
#     actually are (module.Space.FAR's own comment has the breakdown).
#   `mov`, `push` or `pop` reading a segment register's OWN value rather than
#     writing it -- `mov ax,es`, `mov [x],ds`. Not the $DYNAMIC array pattern
#     (that always writes es, never reads it) and not measured as its own
#     shape, so there is nothing to model against yet: 426 of qb-qrender's
#     own MOV barriers are this, not the 17,593 `mov <segreg>,[x]` this pass
#     now models.
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
# LEAVE, STOSW, the segment-register push and pop (modelled by _location()
# like any other register, since push/pop change no addressing), the
# widening one-operand `imul` (WIDE_MULTIPLY) and the far `jmp` (_jump) came
# in together, and between them they were every unmodelled instruction in
# the 110-fixture corpus: 47 of 17970, refusing 30 of its 154 bodies. None of
# the five needed a guess -- each is either a move, a frame mechanic, or
# control leaving. `xchg` (_exchange) and a mov to a segment register
# (_move, via _location()'s own widening) are absent from that corpus
# entirely -- both are qb-qrender-only, measured there instead.
SHAPE: dict[int, tuple[Operation, str]] = {
    Mnemonic.MOV: (Operation.MOVE, "mov"),
    Mnemonic.XCHG: (Operation.EXCHANGE, "xchg"),
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
    # The x87 vocabulary: 18 mnemonics, in exactly the one operand shape
    # qb-qrender's own object corpus uses each in (measured over every
    # obj/OBJ under it -- 61,609 x87-family instructions, none outside
    # these 18 shapes). declen.py's own EMULATED/STANDS_IN account already
    # decodes an emulated int 34h-3Dh site to the same Mnemonic/Code an
    # unemulated one gets, so nothing here has to tell the two apart.
    #
    # Left as barriers, deliberately, because none of them is in that
    # corpus:
    #   `fld st(i)`, a stack-duplicate with no memory operand at all --
    #     _float_load only accepts the memory form.
    #   the non-popping register-register form of `fadd`/`fsub`/`fmul`/
    #     `fdiv` (`fadd st(0),st(1)`) -- the same Mnemonic as the memory
    #     form covers, a different shape, and `_float_arith` only accepts
    #     the memory one.
    #   `fiadd`/`fimul` and every `r`-suffixed reversed form (`fsubr`,
    #     `fdivr`, `fsubrp`, `fdivrp`, `fisubr`, `fidivr`) -- not in SHAPE
    #     at all, so they fall to instruction_semantics' own `found is
    #     None` case like any other uncovered mnemonic.
    #   the non-popping stores `fst`/`fist`, `fbld`/`fbstp`, the compares
    #     (`fcom` and family), and everything else x87 that is not one of
    #     the 18 -- likewise absent from SHAPE.
    # A form joining this corpus is a decision to model, not a gap to
    # paper over by widening one of the five builders below to guess at it.
    Mnemonic.FLD: (Operation.FLOAT_LOAD, "fld"),
    Mnemonic.FILD: (Operation.FLOAT_LOAD, "fild"),
    Mnemonic.FSTP: (Operation.FLOAT_STORE, "fstp"),
    Mnemonic.FISTP: (Operation.FLOAT_STORE, "fistp"),
    Mnemonic.FADD: (Operation.FLOAT_ARITH, "fadd"),
    Mnemonic.FSUB: (Operation.FLOAT_ARITH, "fsub"),
    Mnemonic.FMUL: (Operation.FLOAT_ARITH, "fmul"),
    Mnemonic.FDIV: (Operation.FLOAT_ARITH, "fdiv"),
    Mnemonic.FIDIV: (Operation.FLOAT_ARITH, "fidiv"),
    Mnemonic.FISUB: (Operation.FLOAT_ARITH, "fisub"),
    Mnemonic.FADDP: (Operation.FLOAT_ARITH_POP, "faddp"),
    Mnemonic.FSUBP: (Operation.FLOAT_ARITH_POP, "fsubp"),
    Mnemonic.FMULP: (Operation.FLOAT_ARITH_POP, "fmulp"),
    Mnemonic.FDIVP: (Operation.FLOAT_ARITH_POP, "fdivp"),
    Mnemonic.FCHS: (Operation.FLOAT_UNARY, "fchs"),
    Mnemonic.FABS: (Operation.FLOAT_UNARY, "fabs"),
    Mnemonic.FSQRT: (Operation.FLOAT_UNARY, "fsqrt"),
    # A synchronisation point, not arithmetic: it neither reads nor writes
    # any st(i) (_touches_fp_stack's own docstring measures this from
    # iced), and declen.py's own account of int 3Dh -- "stands in for the
    # whole of WAIT, and nothing follows" -- is what says it is a complete,
    # standalone instruction rather than a prefix fused to whatever FP
    # opcode happens to sit next to it in the byte stream (unlike int
    # 3Ch's segment-override stand-in, which is exactly that kind of
    # prefix). So it needs no shape of its own -- Operation.NOTHING and
    # _nothing() already say "computes nothing, transfers nowhere, touches
    # no flag", and that is the whole of what WAIT is.
    Mnemonic.WAIT: (Operation.NOTHING, "wait"),
}


def instruction_semantics(insn: Insn, resolve: Resolver) -> Semantics:
    """What one real instruction computes, or Operation.BARRIER.

    A segment override is no longer refused here at the mnemonic level: it
    is lift.operand()'s own business, one memory operand at a time. Where it
    resolves (Space.FAR, or a redundant `ds:`), the builder sees a real
    address and models the instruction whole. Where it does not -- an
    override on a shape nothing measured -- `_location()`'s MEMORY case
    still returns a `Mem(None, ...)`, an address this layer cannot name, the
    same answer a `rep stosw` destination already gets: the instruction is
    still modelled (its registers are real, nameable locations regardless of
    what its memory operand resolves to), and the unnamed cell is what
    keeps it from ever being claimed disjoint from anything -- module.
    may_alias's own "None is never provably disjoint" rule, not a pin.
    """
    found = SHAPE.get(insn.insn.mnemonic)
    if found is None:
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
