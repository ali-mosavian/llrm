"""The target: what the machine has, and what each instruction requires.

LLVM splits this in two -- `TargetRegisterInfo` for the register file and
`TargetInstrInfo` for what an opcode does with it -- and binds them through
a subtarget. One target here, so one module; the two halves are the two
sections below.

Written down in one place because it was in six. `AVAILABLE` and the
narrow-name table lived in `regalloc.py`, a second copy of the same table
in `select.py`, the addressing class in `lir.py`, the root map in `ir.py`,
and a pass that wanted any of them reached for whichever module it already
imported. Two copies of one table is one table and a bug waiting on
whichever copy is edited.

**A register class is the unit an allocator works in.** LLVM allocates
within a `TargetRegisterClass` and orders the candidates with an
`AllocationOrder`; asking "any of the six" is only right when every operand
can take any of the six, and 16-bit addressing reaches memory through bx,
bp, si and di and nothing else. `[dx+0Ah]` has no encoding.
"""

from dataclasses import dataclass

from iced_x86 import Register
from iced_x86 import Register_

from qbopt import ir
from qbopt import mir

# ---------------------------------------------------------------- registers

ADDRESSING: frozenset[Register_] = frozenset({Register.BX, Register.BP, Register.SI, Register.DI})


@dataclass(frozen=True, slots=True)
class Need:
    """Where an operand has to live: one register, or any of a set."""

    where: frozenset[Register_]

    @property
    def fixed(self) -> Register_ | None:
        """The register, where there is only one it can be."""
        return next(iter(self.where)) if len(self.where) == 1 else None


@dataclass(frozen=True, slots=True)
class Occurrence:
    """One operand of one instruction, by side and position.

    A requirement is about an operand, not about the register it happens
    to name -- which is only an answer while it names one. Lowering hands
    the allocator values, and `imul`'s dx:ax is then a fact about the
    first and second destination.
    """

    side: str  # "dest" or "source"
    index: int


def requirements(what: "ir.Semantics") -> dict[Occurrence, Register_]:
    """Every operand this instruction requires in one particular register.

    The one place those are written down. `reads` and `writes` below read
    it too, so a machine fact cannot drift between the two questions --
    "which register does this need" and "which operand does".
    """
    out: dict[Occurrence, Register_] = {}
    if _on_the_stack(what):
        return out
    # The widening forms name neither half: the product and the dividend
    # are both dx:ax, low first.
    if what.op in (ir.Operation.MULTIPLY, ir.Operation.DIVIDE) and len(what.dests) != 1:
        out[Occurrence("dest", 0)] = Register.EAX
        out[Occurrence("dest", 1)] = Register.EDX
        # A multiply reads one source, the accumulator. A divide reads the
        # pair, and `ir.DIVIDE_PAIR` has it high first -- `idiv` reads
        # edx:eax -- so its two sources are the other way round.
        if what.op is ir.Operation.DIVIDE:
            out[Occurrence("source", 0)] = Register.EDX
            out[Occurrence("source", 1)] = Register.EAX
        else:
            out[Occurrence("source", 0)] = Register.EAX
    if what.op is ir.Operation.EXTEND and what.name in {"cwd", "cdq"}:
        out[Occurrence("source", 0)] = Register.EAX
        out[Occurrence("dest", 0)] = Register.EDX
    # A shift or rotate by anything but a literal counts from cl. Asked of
    # the operand's shape rather than of the register it names: a value the
    # allocator has not placed yet names none, and keying on cl said such a
    # shift had no requirement at all.
    counted = (what.name or "") in _COUNTED or what.op is ir.Operation.FUNNEL
    if counted and len(what.sources) > 1:
        count = what.sources[-1]
        if not isinstance(count, ir.Imm):
            out[Occurrence("source", len(what.sources) - 1)] = Register.ECX
    return out


_COUNTED = ("shl", "sal", "shr", "sar", "rol", "ror", "rcl", "rcr")


def _root(register: Register_) -> Register_:
    return ir.ROOT.get(register, register)


def _shifted(what: ir.Semantics) -> bool:
    """A shift whose count is a register takes it in cl and says so.

    A funnel shift is one of them: `shrd` counts from cl and from nowhere
    else, and its own two register sources are wherever the allocation put
    them.
    """
    if what.op is ir.Operation.FUNNEL:
        return len(what.sources) == 3 and isinstance(what.sources[2], ir.Reg)
    return (what.name or "") in ("shl", "shr", "sar", "rol", "ror", "rcl", "rcr") and any(
        isinstance(one, ir.Reg) and _root(one.register) is Register.ECX for one in what.sources
    )


def _on_the_stack(what: ir.Semantics) -> bool:
    """Whether this is an x87 operation, which shares no register with the rest.

    `ir` models `fdivp` as a DIVIDE, so the widening rule below claimed it
    reads dx:ax -- 45 of them in the corpus, every one a requirement BC's
    own code does not satisfy, because there is nothing in ax to satisfy it
    with.
    """
    return any(isinstance(one, ir.St) for one in (*what.dests, *what.sources)) or (what.name or "").startswith("f")


def tied(what: ir.Semantics) -> Register_ | None:
    """The register a two-address instruction reads and writes as one.

    `add ax,[c]` computes ax + [c] and puts it back in ax. The value it
    defines and the value it reads are not two places -- they are one
    register at two moments, and an allocation that moves one without the
    other emits `add di,[c]`, which adds to whatever di held.

    x86 says this by naming the same operand twice and nothing else does,
    which is why it belongs here beside the operands nothing names at all.
    """
    if _on_the_stack(what) or not what.dests or not what.sources:
        return None
    into, outof = what.dests[0], what.sources[0]
    if isinstance(into, ir.Reg) and isinstance(outof, ir.Reg) and _root(into.register) is _root(outof.register):
        return _root(into.register)
    return None


def reads(what: ir.Semantics) -> dict[Register_, Need]:
    """Registers this operation reads whether or not it names them.

    Keyed on the root it reads, so a caller can ask "does this operation
    require the value in that register to be there" without knowing how the
    instruction is written.
    """
    out: dict[Register_, Need] = {}
    for where, register in requirements(what).items():
        if where.side == "source":
            out[register] = Need(frozenset({register}))
    for one in (*what.dests, *what.sources):
        for where in (
            getattr(one, "through", None),
            getattr(one, "index", None),
            getattr(getattr(one, "addr", None), "base", None),
        ):
            if where is not None and where is not Register.NONE:
                out[_root(where)] = Need(frozenset(_root(x) for x in ADDRESSING))
    return out


def writes(what: ir.Semantics) -> dict[Register_, Need]:
    """Registers this operation writes whether or not it names them."""
    out: dict[Register_, Need] = {}
    if _on_the_stack(what):
        return out
    for where, register in requirements(what).items():
        if where.side == "dest":
            out[register] = Need(frozenset({register}))
    return out


# Every register a value may be placed in. `mir.TRACKED` is what the raise
# follows, and a register nothing tracks is a register no value is in.
AVAILABLE: tuple[Register_, ...] = mir.TRACKED

# What this may hand out for an operand that reaches memory, which is not
# what the encoding permits: bp is a legal base and is how every frame slot
# is reached, and it is also the frame pointer. Asking both questions with
# one set made every `[bp-12h]` in the corpus look like a violated
# requirement -- 183 of them.
BASES: tuple[Register_, ...] = tuple(one for one in AVAILABLE if one in {ir.ROOT.get(x, x) for x in ADDRESSING})


WIDE = {Register.EAX, Register.ECX, Register.EDX, Register.EBX, Register.ESI, Register.EDI, Register.EBP, Register.ESP}
NARROW = {Register.AX, Register.CX, Register.DX, Register.BX, Register.SI, Register.DI, Register.BP, Register.SP}
# The byte halves. BC reaches for them to clear a high byte (`xor bh,bh`)
# and to read one byte of an array.
BYTE = {
    Register.AL,
    Register.CL,
    Register.DL,
    Register.BL,
    Register.AH,
    Register.CH,
    Register.DH,
    Register.BH,
}


# The width each register names, and the register file at each width. One
# table rather than three lookups, and the only place widths are written
# down. Built from the three rows rather than from ir.ROOT: the root map
# has no byte-wide entries, and an `ir.Held` of width 1 resolved through a
# table without them keeps the root -- `mov [k],eax` where the instruction
# meant `mov [k],al`. regalloc.py had its own two-width copy and select.py
# this one, and reading them as duplicates broke every object in the corpus.
WIDTHS: dict[Register_, int] = {}
AT_WIDTH: dict[Register_, dict[int, Register_]] = {}
for _row, _size in ((WIDE, 4), (NARROW, 2), (BYTE, 1)):
    for _one in _row:
        WIDTHS[_one] = _size
        # setdefault, not assignment: al and ah are both one byte and both
        # root to eax, and the later one was winning -- an ir.Held of width
        # 1 resolved to `ah`, which is a different register holding a
        # different byte.
        AT_WIDTH.setdefault(ir.ROOT.get(_one, _one), {}).setdefault(_size, _one)


def named(register: Register_, width: int) -> Register_:
    """The same register named at the width an operand needs."""
    return AT_WIDTH.get(ir.ROOT.get(register, register), {}).get(width, register)


def order(where: "frozenset[Register_] | None") -> tuple[Register_, ...]:
    """The registers an operand may take, in the order to try them.

    LLVM's `AllocationOrder`. `None` means the operand said nothing, which
    is every operand whose encoding names it outright.
    """
    if where is None:
        return AVAILABLE
    wanted = {ir.ROOT.get(one, one) for one in where}
    return tuple(one for one in AVAILABLE if one in wanted)


# The segment registers. Operands, not allocatable: `mov ax,ds` names one
# and nothing may be placed in one. docs/roadmap.md wants them to become a
# class the allocator works in, which is a change to the allocator; naming
# them here is what lets everything else stop treating them as scenery.
SEGMENTS: frozenset[Register_] = frozenset(
    {Register.ES, Register.CS, Register.SS, Register.DS, Register.FS, Register.GS}
)


def known(register: Register_) -> bool:
    """Whether this is a register this target describes at all."""
    return register in WIDTHS or register in SEGMENTS


def width_of(register: Register_) -> int | None:
    """How wide this register is, or None where the target does not say."""
    return 2 if register in SEGMENTS else WIDTHS.get(register)


# ------------------------------------------------------------ subregisters

# Which bytes of its root each register is, as a mask. LLVM calls these
# lane masks and composes them through subregister indices; there are four
# lanes here and they can be written down.
#
# `ir.ROOT` folds every name to its 32-bit parent, which answers "same
# register file entry" and not "same bytes" -- and al and ah are the case
# where those differ. Two operations on opposite halves of one long
# compared equal on the value they named, and nots printed the right low
# word of NOTOR and the wrong high one. LANES is the distinction.
LANES: dict[Register_, int] = {}
for _row, _mask in ((WIDE, 0b1111), (NARROW, 0b0011)):
    for _one in _row:
        LANES[_one] = _mask
for _one in BYTE:
    LANES[_one] = 0b0010 if _one in {Register.AH, Register.CH, Register.DH, Register.BH} else 0b0001


def lanes(register: Register_) -> int:
    """Which bytes of its root this register names."""
    return LANES.get(register, 0b1111)


def overlaps(one: Register_, other: Register_) -> bool:
    """Whether writing one can be seen by reading the other.

    The same root is not enough: al and ah share eax and share no byte, so
    a write to one is invisible to the other. LLVM asks this through lane
    masks; this is the same question with four lanes.
    """
    if ir.ROOT.get(one, one) is not ir.ROOT.get(other, other):
        return False
    return bool(lanes(one) & lanes(other))


# Each register by the name a human writes, which is what runtime.py's own
# contracts are keyed on. iced gives an integer; the name is the join.
NAMES: dict[Register_, str] = {
    getattr(Register, name): name.lower()
    for name in dir(Register)
    if not name.startswith("_") and isinstance(getattr(Register, name), int)
}


def name_of(register: Register_) -> str:
    """This register's own name, lowercase."""
    return NAMES.get(register, str(register))
