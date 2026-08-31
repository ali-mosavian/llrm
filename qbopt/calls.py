"""
The runtime calls, and what they can be replaced with.

`*`, `\\`, `MOD` and every comparison are calls into BC's runtime, and they are
what the widening cannot touch: the pass sees a far call and stops. In an object
the call site is a FIXUPP naming an EXTDEF, so which routine it is, is a lookup.

The order the arguments reach the stack is the thing to get right, and it is not
the same for both. Comparison pushes its left operand first; multiply, divide
and remainder push it second. That holds across all four configurations and is
opposite between the two routines, and getting it backwards is a different
answer rather than a crash.
"""

from enum import StrEnum
from dataclasses import replace
from dataclasses import dataclass

from iced_x86 import Code
from iced_x86 import Decoder
from iced_x86 import Register
from iced_x86 import Register_
from iced_x86 import Instruction
from iced_x86 import BlockEncoder
from iced_x86 import MemoryOperand

from qbopt.flags import ALL
from qbopt.flags import Flag
from qbopt.declen import Insn
from qbopt.module import Addr
from qbopt.blocks import Block
from qbopt.lift import Emitted
from qbopt.module import Space
from qbopt.stack import frames
from qbopt.module import Module
from qbopt.declen import BITNESS
from qbopt.declen import to_signed
from qbopt.stack import PUSH_BYTES
from qbopt.lift import relocated_memory

COMPARE = "B$CPI4"
MULTIPLY = "B$MUI4"
DIVIDE = "B$DVI4"
REMAINDER = "B$RMI4"

# B$CPI4's sibling B$CMI4 (runtime/rt/helpi4.asm) does the same signed
# comparison but returns flags meant for an *unsigned* jcc -- ABSORBED's
# CMP_R32_RM32 mapping is only right for B$CPI4's own convention. B$CMI4 is
# not in LEFT_FIRST and never reaches here; it must stay that way unless
# absorb() is taught the different flag meaning.

# A user-declared `declare function fixMul& (byval a as long, byval b as long,
# byval fixShift as long)` has no body anywhere -- LINK never sees it, because
# absorbing the call drops its only fixup. BC never emits a type suffix into
# the EXTDEF, so the name it writes is the identifier alone, uppercased the
# way every BASIC identifier is. Measured: BC pushes a `declare`d function's
# arguments in the order written, first argument first -- the runtime's own
# routines do not, which is what the module docstring above is about, and is
# unrelated to this one.
#
# The shift is a third argument rather than a fixed constant: N.M times N.M is
# N.2M, which does not fit back in 32 bits without the shift that undoes the
# doubled fraction, and the width of that fraction is the caller's format to
# choose, not this pass's to assume. fixShift is `as long` only so it reaches
# the stack the same way a and b do -- one_operand() already knows every shape
# that arrives in.
FIX_MULTIPLY = "FIXMUL"

# True where the left operand is pushed first. Uniform across the compilers,
# opposite between the two runtime routines. FIX_MULTIPLY is not the runtime's:
# it is pushed in the order written, which happens to agree with COMPARE's.
LEFT_FIRST = {COMPARE: True, MULTIPLY: False, DIVIDE: False, REMAINDER: False, FIX_MULTIPLY: True}

# Every routine here takes two long arguments except fixMul&, which takes three.
ARITY = {FIX_MULTIPLY: 3}


def _arity(name: str | None) -> int | None:
    """How many long arguments this routine takes, or None if it is not one
    absorption knows how to handle at all."""
    if name is None or name not in LEFT_FIRST:
        return None
    return ARITY.get(name, 2)


# one dword per argument under VBDOS /G3, two words everywhere else
PUSHES = {Code.PUSH_RM16, Code.PUSH_RM32}
CONSTANTS = {Code.PUSHW_IMM8, Code.PUSHD_IMM8, Code.PUSH_IMM16, Code.PUSHD_IMM32}
WIDE_PUSHES = {Code.PUSH_RM32, Code.PUSHD_IMM8, Code.PUSHD_IMM32}

# iced-x86's own immediate() is unsigned: an imm8 source (sign-extended to its
# push width by the CPU) comes back as a 64-bit-wide unsigned rendering of
# that sign extension regardless of destination width, an imm16/imm32 source
# comes back as its own natural width unsigned -- to_signed(value, width)
# undoes whichever one it is. Measured directly: `66 6A FF` (PUSHW_IMM8, -1)
# and `6A FF` (PUSHD_IMM8, -1) both read back as 2**64-1; `66 68 FF FF`
# (PUSH_IMM16, -1) reads back as 65535; `68 FF FF FF FF` (PUSHD_IMM32, -1)
# reads back as 2**32-1.
CONSTANT_WIDTH: dict[int, int] = {Code.PUSHW_IMM8: 8, Code.PUSHD_IMM8: 8, Code.PUSH_IMM16: 2, Code.PUSHD_IMM32: 4}


class Kind(StrEnum):
    STATIC = "static"  # a bare displacement, whose address is a fixup
    CONSTANT = "constant"  # an immediate pushed straight to the stack


@dataclass(frozen=True, slots=True)
class Operand:
    kind: Kind
    addr: Addr | None = None
    # where the displacement field was, so its fixup can be reused
    at: int | None = None
    value: int = 0
    length: int = 0  # instructions it took to push


@dataclass(frozen=True, slots=True)
class CallSite:
    at: int  # the call instruction
    end: int
    start: int  # where the first push begins -- the call itself, if consume is set
    name: str
    pushed: tuple[Operand, ...] = ()  # in the order they reach the stack -- empty if consume is set
    # the raw pushes stack.py found for this call, when match()'s own
    # contiguous-and-classifiable scan could not: nothing here is reloaded
    # from an address, because nothing here is left standing to reload from
    # -- consume() pops every byte instead, wherever it actually sits
    consume: tuple[Insn, ...] = ()

    @property
    def operands(self) -> tuple[Operand, Operand]:
        """(left, right), whichever way this routine takes them."""
        first, second = self.pushed
        return (first, second) if LEFT_FIRST[self.name] else (second, first)


def static_at(module: Module, insn: Insn) -> Operand | None:
    """A pushed operand at a fixed address, or an array element indexed by si/di.

    Two elements at the same displacement are different addresses unless the
    register indexing them agrees too. static_at has no frame-slot case for
    an index to be misread against the way lift.py's operand() does -- the
    base whitelist below refuses bp/bx outright -- but a scaled-index push
    (`[eax*4+x]`, memory_base NONE with an index) would otherwise slip past
    that whitelist and needs refusing on its own.
    """
    if insn.code not in PUSHES or insn.disp_at is None or insn.memory_index != Register.NONE:
        return None
    if insn.memory_base not in (Register.NONE, Register.SI, Register.DI):
        return None
    addr = module.operands.get(insn.disp_at)
    if addr is None or addr.space is not Space.SEGMENT:
        return None
    if insn.memory_base != Register.NONE:
        addr = replace(addr, base=insn.memory_base)
    return Operand(Kind.STATIC, addr, insn.disp_at, length=1)


def constant_at(insn: Insn) -> Operand | None:
    if insn.code not in CONSTANTS or insn.imm_at is None:
        return None
    # a negative constant pushed straight to the stack (not through a
    # variable) is otherwise a huge unsigned value here, which a later
    # absorb() hands to iced-x86's own i32 instruction builder -- which
    # raises OverflowError. Found by tools/fuzzcheck.py: a generated LONG
    # multiply against a large negative literal crashed absorb() outright,
    # where suite/divmod.bas's MULOVF only ever multiplies through variables.
    return Operand(Kind.CONSTANT, value=to_signed(insn.insn.immediate(0), CONSTANT_WIDTH[insn.code]), length=1)


def widened_constant_at(reached: list[Insn], last: int) -> Operand | None:
    """An INTEGER literal, widened to the LONG a `byval` parameter takes.

    `mov ax,imm16 / cwd / push dx / push ax` -- PDS and QB 4.5 have no dword
    push, so a constant argument to a user function goes through the same
    sign-extension the language itself does for `INTEGER` to `LONG`, four
    instructions where a runtime call's own small-constant form is one.
    """
    if last < 3:
        return None
    mov_ax, cwd, push_dx, push_ax = reached[last - 3], reached[last - 2], reached[last - 1], reached[last]
    if not (
        mov_ax.code == Code.MOV_R16_IMM16
        and mov_ax.insn.op0_register == Register.AX
        and cwd.code == Code.CWD
        and cwd.at == mov_ax.end
        and push_dx.code == Code.PUSH_R16
        and push_dx.insn.op0_register == Register.DX
        and push_dx.at == cwd.end
        and push_ax.code == Code.PUSH_R16
        and push_ax.insn.op0_register == Register.AX
        and push_ax.at == push_dx.end
    ):
        return None
    return Operand(Kind.CONSTANT, value=to_signed(mov_ax.insn.immediate(1), 2), length=4)


def one_operand(module: Module, reached: list[Insn], last: int) -> Operand | None:
    """The long argument whose pushes end at `reached[last]`, or None.

    A long reaches the stack either as one dword -- VBDOS /G3, and immediates
    everywhere -- or as two words, high first. The `+2` on the word form is the
    same discipline as the pair test in the lifter and fails the same silent way
    if it is dropped.
    """
    insn = reached[last]
    if insn.code in WIDE_PUSHES:
        return static_at(module, insn) or constant_at(insn)

    low = static_at(module, insn) or constant_at(insn)
    if low is None:
        return widened_constant_at(reached, last)
    if last == 0:
        return None
    high = static_at(module, reached[last - 1]) or constant_at(reached[last - 1])
    if high is None or reached[last - 1].end != insn.at:
        return None
    if low.kind is not high.kind:
        return None
    if low.kind is Kind.STATIC:
        if low.addr is None or high.addr != low.addr.plus(2):
            return None
        return replace(low, length=2)
    return replace(low, value=(high.value << 16) | (low.value & 0xFFFF), length=2)


def match(module: Module, reached: list[Insn], index: int) -> CallSite | None:
    """The call at `reached[index]` with its arguments, or None.

    Refuses anything it cannot account for exactly: every long argument the
    routine takes, nothing else in between, and every push adjacent to the
    next.
    """
    call = reached[index]
    name = module.calls.get(call.at)
    if name not in LEFT_FIRST:
        return None
    arity = ARITY.get(name, 2)

    found: list[Operand] = []
    last = index - 1
    while len(found) < arity and last >= 0:
        if reached[last].end != (call.at if not found else reached[last + 1].at):
            return None
        operand = one_operand(module, reached, last)
        if operand is None:
            return None
        found.append(operand)
        last -= operand.length
    if len(found) != arity:
        return None

    return CallSite(call.at, call.end, reached[last + 1].at, name, tuple(reversed(found)))


def sites(module: Module, reached: list[Insn], blocks: list[Block]) -> list[CallSite]:
    """Every call whose arguments are known -- classified from an address or
    an immediate where match() can see one, popped from the stack where it
    cannot.

    match()'s backward scan only sees a push immediately, contiguously before
    the call; stack.py's block-scoped depth tracking sees further, including
    a value pushed early and left stranded under an entirely separate,
    self-contained call. Every site match() already finds is left to it --
    frames() is only asked about what match() could not classify.
    """
    found = []
    handled: set[int] = set()
    for index, insn in enumerate(reached):
        if insn.at in module.calls and (site := match(module, reached, index)) is not None:
            found.append(site)
            handled.add(insn.at)
    for block in blocks:
        for frame in frames(block, module.calls, _arity):
            if frame.call.at in handled:
                continue
            name = module.calls[frame.call.at]
            found.append(CallSite(frame.call.at, frame.call.end, frame.call.at, name, consume=frame.pushed))
    return found


# B$CPI4 rebuilds its answer through lahf/sahf, because an 8086 cannot compare a
# long in one go. A 386 can, and the flags a cmp leaves are the ones the
# following jcc wants -- but only the signed and equality ones. CF, PF and AF
# are the runtime's synthesis rather than the comparison's, so a site whose CF
# is read afterwards has to be left alone.
#
# "The signed ones agree" is true of the cmp this emits and NOT of the runtime
# it replaces, which is a divergence rather than a hazard. Where the high words
# are equal B$CPI4 compares the low words unsigned and folds CF into SF through
# sahf -- and sahf writes only the low byte of FLAGS, so it cannot touch OF at
# bit 11. OF is left over from that low-word compare, and BC's own jl/jle/jg/jge
# read SF <> OF against it, answering backwards whenever the low halves straddle
# 0x8000. ZF is never touched, so = and <> are right either way. This cmp is
# correct on those operands and the runtime is not; suite/cmpof.bas pins it and
# configs.DIVERGES records that BC's build is expected to disagree.
SYNTHESISED = Flag.CF | Flag.PF | Flag.AF

ABSORBED = {COMPARE: Code.CMP_R32_RM32, MULTIPLY: Code.IMUL_R32_RM32}

# Divide and remainder are C's: one idiv, no test of the divisor. `x / 0` and
# `-2147483648 / -1` are undefined in C and fault on this machine, where BC's
# runtime raised BASIC error 11 for the first and returned silently from the
# second. That behaviour does not survive, deliberately.
DIVIDES = {DIVIDE, REMAINDER}

RESULT = Register.EAX  # what the runtime returns a long in, as ax:dx

# B$CPI4's actual body (runtime/rt/helpi4.asm of the QuickBASIC 4.5 source)
# never touches cx, dx or bx at all, and its cProc save-list -- <AX>, where
# B$MUI4/B$DVI4/B$RMI4 all declare an empty one -- preserves ax too. Its own
# "Uses: ax,cx,dx,bx" comment overstates what a real call actually clobbers:
# nothing but the flags. BC's own code relies on that, keeping a value live
# in eax across a compare embedded in a larger expression -- absorbing the
# call still needs a scratch register to hold one side of the comparison,
# but wrapping it in push/pop is the only way to match a call that changes
# no register at all. See absorb()'s COMPARE branch.


def relocated_addr(operand: Operand) -> Addr:
    """Where a Kind.STATIC operand's value lives, once it is read at codegen
    time rather than reused from wherever it was pushed.

    static_at is Kind.STATIC's only producer and always gives it a relocated
    SEGMENT address; a caller passing anything else is a bug in this module,
    not a shape refuse() left for absorb() to catch, so this raises rather
    than silently building the address of whatever offset 0 happens to be.
    """
    if operand.addr is None or operand.addr.space is not Space.SEGMENT:
        raise ValueError(f"a {operand.kind} operand has no relocated address")
    return operand.addr


def memory_of(operand: Operand) -> MemoryOperand:
    return relocated_memory(relocated_addr(operand).base)


def load_of(operand: Operand) -> Instruction:
    if operand.kind is Kind.CONSTANT:
        return Instruction.create_reg_i32(Code.MOV_R32_IMM32, RESULT, operand.value)
    base = relocated_addr(operand).base
    # the moffs form is shorter, but it has no ModRM byte and so no way to
    # carry an index register -- the general r32,rm32 form is the only one
    # that can, and is the one an indexed operand has to fall back to
    code = Code.MOV_R32_RM32 if base != Register.NONE else Code.MOV_EAX_MOFFS32
    return Instruction.create_reg_mem(code, RESULT, relocated_memory(base))


def fits_in_a_byte(value: int) -> bool:
    return -128 <= value < 128


# A multiply by a constant the 386 can do without multiplying. `shl` is the
# same four bytes as `imul r32,imm8` and several times faster; `lea` through
# the SIB scale costs one byte more and is faster still, which is the trade
# this pass already makes elsewhere and these sites are all inside loops.
# gcc and clang at -O3 pick exactly these on i386 -- ×3 is one lea, not a
# shift and an add.
#
# Only where nothing reads the flags afterwards, which absorb() has already
# established for every MULTIPLY site it reaches: imul writes them, shl
# writes them differently, and lea writes none at all.
SCALES = {3: 2, 5: 4, 9: 8}


def _without_multiplying(value: int) -> Instruction | None:
    """One instruction that multiplies RESULT by `value`, or None.

    Negative and zero are refused rather than special-cased. A negative
    power of two is a shift and a negation, which is two instructions and a
    different shape; nothing in the corpus asks for one, and inventing the
    sequence unmeasured is how a fold acquires a case nothing checks.
    """
    if value <= 0:
        return None
    if value & (value - 1) == 0 and value != 1:
        return Instruction.create_reg_i32(Code.SHL_RM32_IMM8, RESULT, value.bit_length() - 1)
    if (scale := SCALES.get(value)) is not None:
        return Instruction.create_reg_mem(Code.LEA_R32_M, RESULT, MemoryOperand(base=RESULT, index=RESULT, scale=scale))
    return None


def apply_to(name: str, operand: Operand) -> Instruction:
    if operand.kind is not Kind.CONSTANT:
        return Instruction.create_reg_mem(ABSORBED[name], RESULT, memory_of(operand))
    # the sign-extended byte forms are two or three bytes shorter, and a long
    # compared or multiplied by a small constant is the common case
    short = fits_in_a_byte(operand.value)
    if name == COMPARE:
        code = Code.CMP_RM32_IMM8 if short else Code.CMP_EAX_IMM32
        return Instruction.create_reg_i32(code, RESULT, operand.value)
    if name == MULTIPLY and (cheaper := _without_multiplying(operand.value)) is not None:
        return cheaper
    code = Code.IMUL_R32_RM32_IMM8 if short else Code.IMUL_R32_RM32_IMM32
    return Instruction.create_reg_reg_i32(code, RESULT, RESULT, operand.value)


def absorb(site: CallSite, live: Flag, restore: bool = True) -> Emitted | str:
    """The call replaced by 386 instructions, or why it cannot be.

    Nine to sixteen bytes against fifteen and twenty-one for multiply, and it
    removes a far call and the routine behind it. Compare is thirteen against
    fifteen: four of the nine bytes a bare load-and-cmp would take are a
    push/pop wrapped around eax, because a real call to B$CPI4 changes no
    register at all (see the note above RESULT) and BC's own code can be
    relying on that anywhere around the call, not only in the flags.

    `restore=False` drops the trailing high-half restore MULTIPLY (and
    consume()/dividing()/fix_multiply(), below) would otherwise emit -- for a
    site lift.tail() has already proven BC's own following code widens
    against, so putting the high half back only to immediately re-derive it
    from eax would be the round trip docs/residue.md calls G and H. COMPARE
    has no restore to drop: its own result is flags, not a register value.
    """
    if site.consume:
        return consume(site, live, restore)
    if site.name == FIX_MULTIPLY:
        return fix_multiply(site, live, restore)
    if site.name in DIVIDES:
        return dividing(site, live, restore)
    if site.name not in ABSORBED:
        return f"{site.name} is not absorbed"
    if site.name == COMPARE and live & SYNTHESISED:
        return f"the site's {live & SYNTHESISED!r} comes from the runtime, not from a comparison"
    if site.name == MULTIPLY and live & ALL:
        # imul sets the flags where the runtime left whatever it happened to
        return f"something reads {live & ALL!r} after the multiply"

    left, right = site.operands
    # x*x (or x==x): the same address read twice is one load, not two -- the
    # second step becomes reg,reg and needs no fixup of its own.
    same_address = left.kind is Kind.STATIC and right.kind is Kind.STATIC and left.addr == right.addr

    steps: list[Instruction] = []
    relocated: dict[int, int] = {}

    def add(insn: Instruction) -> int:
        steps.append(insn)
        return len(steps) - 1

    if site.name == COMPARE:
        add(Instruction.create_reg(Code.PUSH_R32, RESULT))

    where = add(load_of(left))
    if left.kind is Kind.STATIC and left.at is not None:
        relocated[where] = left.at

    if same_address:
        add(Instruction.create_reg_reg(ABSORBED[site.name], RESULT, RESULT))
    else:
        where = add(apply_to(site.name, right))
        if right.kind is Kind.STATIC and right.at is not None:
            relocated[where] = right.at

    if site.name == COMPARE:
        # pop does not touch the flags the cmp above just set
        add(Instruction.create_reg(Code.POP_R32, RESULT))
    elif restore:
        # a multiply leaves a value, and BC reads its high half from dx
        for insn in restoring():
            add(insn)

    return assemble(steps, relocated)


def assemble(steps: list[Instruction], relocated: dict[int, int]) -> Emitted:
    """Encode a block whose branches name one another by instruction index.

    Each instruction carries its index as its ip, so a branch target is an
    index; iced resolves them and picks the short forms. Where the operands
    finally landed is read back off the encoded bytes rather than predicted.
    """
    for index, insn in enumerate(steps):
        insn.ip = index
    encoder = BlockEncoder(BITNESS)
    encoder.add_many(steps)
    code = encoder.encode(0)

    decoder = Decoder(BITNESS, code, ip=0)
    placed = [(insn.ip, decoder.get_constant_offsets(insn)) for insn in decoder]
    if len(placed) != len(steps):
        raise ValueError(f"encoded {len(placed)} instructions from {len(steps)}")
    return Emitted(
        code,
        tuple((placed[index][0] + placed[index][1].displacement_offset, field) for index, field in relocated.items()),
    )


def restoring() -> list[Instruction]:
    """Put the high half back where BC reads it, through the stack."""
    return [
        Instruction.create_reg(Code.PUSH_R32, RESULT),
        Instruction.create_reg(Code.POP_R16, Register.AX),
        Instruction.create_reg(Code.POP_R16, Register.DX),
    ]


def grouped(pushed: tuple[Insn, ...]) -> list[tuple[Insn, ...]] | None:
    """One argument's worth of pushes per group, deepest first, or None if
    they do not split cleanly into 4-byte arguments.

    Byte-counted from the top rather than address-matched: stack.py's own
    frames() guarantees the *total* is exactly arity*4, but not that a word
    pair stays adjacent to its own other half rather than a neighbour's --
    real BC can't produce that (the low half would have to survive in ax
    across a call that clobbers it), but this walk does not get to assume it.
    """
    groups: list[tuple[Insn, ...]] = []
    remaining = list(pushed)
    while remaining:
        have = 0
        take: list[Insn] = []
        while have < 4 and remaining:
            insn = remaining.pop()
            have += PUSH_BYTES.get(insn.code, 0)
            take.append(insn)
        if have != 4:
            return None
        take.reverse()
        groups.append(tuple(take))
    groups.reverse()
    return groups


# bx is never a target below and never holds a value this pass has to
# preserve across the call, so it is always free as scratch -- any other
# register risks clobbering a different argument already popped into it, or
# clobbering an array index BC's own code is still holding across this call.
# For the four runtime routines that rests on the QuickBASIC 4.5 runtime
# source (stack.py's own docstring): callee-cleanup, clobbers only ax/cx/dx/bx.
# fixMul& is not a runtime routine -- it is a user-declared FUNCTION this pass
# invents a body for, and nothing here has measured what BC assumes survives a
# call to one. No fixMul& site in fixtures/omf or build/ ever reaches Consume
# (every one there is address-or-immediate, absorbed by match() already), so
# this is unexercised, not merely untested.


def popped_into(target: Register_) -> Instruction:
    """One argument, off the real stack and into `target`.

    Whether BC pushed it as one dword or two words, high half first, the four
    bytes already sit in dword layout on top of the stack -- a single 32-bit
    pop reads them correctly either way, with no recombination needed.
    """
    return Instruction.create_reg(Code.POP_R32, target)


# Which physical register each pop lands in, topmost group (the one nearest
# the call, popped first) to deepest -- forced by the real stack, independent
# of LEFT_FIRST's logical left/right. Multiplication commutes, so which of
# the two operands loads into eax cannot change a*b; comparison and division
# are not commutative, but eax/ecx here is the same assignment dividing()
# and ABSORBED's reg,rm forms already use for a Delete site, just populated
# by a pop instead of a load. COMPARE has no entry: it is never popped into a
# register at all -- see compare_consume() -- and _arity() already knows how
# many arguments it takes without this table repeating it.
CONSUME_TARGETS = {
    MULTIPLY: (Register.EAX, Register.ECX),
    DIVIDE: (Register.EAX, Register.ECX),
    REMAINDER: (Register.EAX, Register.ECX),
    FIX_MULTIPLY: (Register.ECX, Register.EDX, Register.EAX),
}


def compare_consume() -> Emitted:
    """A popped compare, without popping: B$CPI4 changes no register at all
    (see the note above RESULT), and its two arguments have to come off the
    stack the same way a real call's callee-cleanup would remove them.

    bp is the only register 16-bit addressing can use as a base with a
    displacement -- sp itself cannot be -- so bp stands in as a frame
    pointer just long enough to read both arguments in place, and one more
    register (edx) holds one side of the cmp. Both are saved on entry and
    put back after the flags are set; nothing but the flags is left changed.

    The saved bp cannot simply be read back where push left it: that slot is
    below sp the moment sp is raised past it, and DOS services interrupts at
    any instruction boundary -- every one of them pushes onto whatever stack
    is live, at and below sp, and is free to have clobbered it by the time
    this code reads it back. So the restore reads bp before sp moves at all,
    parks it in the call's own dead argument space (still above the final
    sp), and only the last, final pop ever reads at an address below where
    sp already sits.
    """
    steps: list[Instruction] = [
        Instruction.create_reg(Code.PUSH_R16, Register.BP),
        Instruction.create_reg(Code.PUSH_R32, Register.EDX),
        Instruction.create_reg_reg(Code.MOV_R16_RM16, Register.BP, Register.SP),
        # +6 pushed ahead of the arguments (bp, then edx) puts left at +10
        # and right, the topmost original argument, at +6
        Instruction.create_reg_mem(
            Code.MOV_R32_RM32, Register.EDX, MemoryOperand(base=Register.BP, displ=10, displ_size=1)
        ),
        Instruction.create_reg_mem(
            Code.CMP_R32_RM32, Register.EDX, MemoryOperand(base=Register.BP, displ=6, displ_size=1)
        ),
        # everything from here on must leave the flags alone
        Instruction.create_reg_mem(
            Code.MOV_R16_RM16, Register.DX, MemoryOperand(base=Register.BP, displ=4, displ_size=1)
        ),
        # +12 is the top two bytes of the left argument's own four -- already
        # read into edx above, so overwriting them here is safe, and it is
        # still above where sp ends up
        Instruction.create_mem_reg(
            Code.MOV_RM16_R16, MemoryOperand(base=Register.BP, displ=12, displ_size=1), Register.DX
        ),
        Instruction.create_reg_mem(
            Code.MOV_R32_RM32, Register.EDX, MemoryOperand(base=Register.BP, displ=0, displ_size=1)
        ),
        Instruction.create_reg_mem(
            Code.LEA_R16_M, Register.SP, MemoryOperand(base=Register.BP, displ=12, displ_size=1)
        ),
        Instruction.create_reg(Code.POP_R16, Register.BP),
    ]
    return assemble(steps, {})


def consume(site: CallSite, live: Flag, restore: bool = True) -> Emitted | str:
    """A call whose arguments only the stack knows, popped rather than reloaded.

    Nothing here is classified as an address or a constant, because nothing
    here is left standing to classify: match() already owns every site whose
    operands are contiguous and address-or-immediate, and this is only ever
    asked about a site it refused. Popping every byte, regardless of what any
    one push looks like, is what makes that sound -- a site with one static
    operand and one stack-only operand still has BOTH pushed, and reloading
    the static one from memory while leaving its push on the stack would
    leak four bytes of stack per call, forever, since nothing else here ever
    removes a push.

    fixShift always goes through cl here, even when it turns out to have been
    a compile-time constant -- shrd's own immediate form needs the value at
    codegen time, which a popped operand never has. shrd masks its count
    modulo 32 regardless, so the range refusal fix_multiply() applies to a
    known-constant shift does not apply and is not needed.
    """
    if site.name == COMPARE and live & SYNTHESISED:
        return f"the site's {live & SYNTHESISED!r} comes from the runtime, not from a comparison"
    if site.name != COMPARE and live & ALL:
        return f"something reads {live & ALL!r} after it"

    groups = grouped(site.consume)
    if groups is None:
        return f"{site.name}'s pushes do not split cleanly into 4-byte arguments"
    arity = _arity(site.name)
    if arity is None or len(groups) != arity:
        return f"{site.name} takes {arity} arguments, not {len(groups)}"

    if site.name == COMPARE:
        return compare_consume()

    # each argument is exactly one dword, popped topmost (nearest the call)
    # to deepest, which is CONSUME_TARGETS' own order -- grouped() has
    # already confirmed there are as many 4-byte groups as targets; their
    # contents no longer matter, since popped_into() reads any group the
    # same way.
    targets = CONSUME_TARGETS[site.name]
    steps: list[Instruction] = [popped_into(target) for target in targets]
    if site.name == MULTIPLY:
        steps.append(Instruction.create_reg_reg(Code.IMUL_R32_RM32, Register.EAX, Register.ECX))
    elif site.name in DIVIDES:
        steps.append(Instruction.create(Code.CDQ))
        steps.append(Instruction.create_reg(Code.IDIV_RM32, Register.ECX))
        if site.name == REMAINDER:
            steps.append(Instruction.create_reg_reg(Code.MOV_R32_RM32, RESULT, Register.EDX))
    elif site.name == FIX_MULTIPLY:
        steps.append(Instruction.create_reg(Code.IMUL_RM32, Register.EDX))
        steps.append(Instruction.create_reg_reg_reg(Code.SHRD_RM32_R32_CL, RESULT, Register.EDX, Register.CL))
    if restore:
        steps.extend(restoring())
    return assemble(steps, {})


def dividing(site: CallSite, live: Flag, restore: bool = True) -> Emitted | str:
    """A long divide, as C compiles one.

        mov eax,[a] / mov ecx,[b] / cdq / idiv ecx

    and the remainder from edx. No test of the divisor, because C does not make
    one: `x / 0` and `-2147483648 / -1` are undefined, and on this machine they
    fault. BC's runtime raised BASIC error 11 for the first and returned
    silently from the second; neither survives, and that is the point.
    """
    if live & ALL:
        return f"something reads {live & ALL!r} after it, and idiv leaves the flags undefined"

    left, right = site.operands
    divisor = Register.ECX
    steps: list[Instruction] = []
    relocated: dict[int, int] = {}

    def add(insn: Instruction) -> int:
        steps.append(insn)
        return len(steps) - 1

    where = add(load_of(left))
    if left.kind is Kind.STATIC and left.at is not None:
        relocated[where] = left.at

    if right.kind is Kind.CONSTANT:
        add(Instruction.create_reg_i32(Code.MOV_R32_IMM32, divisor, right.value))
    else:
        where = add(Instruction.create_reg_mem(Code.MOV_R32_RM32, divisor, memory_of(right)))
        if right.at is not None:
            relocated[where] = right.at

    add(Instruction.create(Code.CDQ))
    add(Instruction.create_reg(Code.IDIV_RM32, divisor))
    if site.name == REMAINDER:
        add(Instruction.create_reg_reg(Code.MOV_R32_RM32, RESULT, Register.EDX))
    if restore:
        for insn in restoring():
            add(insn)

    return assemble(steps, relocated)


def fix_multiply(site: CallSite, live: Flag, restore: bool = True) -> Emitted | str:
    """`fixMul&(a, b, fixShift)`, as C would write the shift it means:
    `(int32)(((int64)a * b) >> fixShift)`.

    One `imul` against the register form gives the full 64-bit product in
    `edx:eax`, and `shrd` is a pure bit shift across the pair -- extracting a
    32-bit window of a two's-complement value needs no sign correction, so it
    is right whatever the signs of `a` and `b` are. Multiplication commutes
    exactly in two's complement, so which of `a`/`b` loads into `eax` cannot
    change the result; `LEFT_FIRST` records the order BC actually pushed them
    in, but nothing here depends on it.

    `fixShift` is always known at compile time in practice -- nobody picks
    their fixed-point format at runtime -- so a literal goes straight into
    `shrd`'s own immediate byte. A variable still works: it loads into `cl`,
    the one register `shrd` can take a shift count from.
    """
    if live & ALL:
        return f"something reads {live & ALL!r} after it, and imul leaves the flags undefined"

    a, b, shift = site.pushed
    factor = Register.ECX
    steps: list[Instruction] = []
    relocated: dict[int, int] = {}

    def add(insn: Instruction) -> int:
        steps.append(insn)
        return len(steps) - 1

    where = add(load_of(a))
    if a.kind is Kind.STATIC and a.at is not None:
        relocated[where] = a.at

    if b.kind is Kind.CONSTANT:
        add(Instruction.create_reg_i32(Code.MOV_R32_IMM32, factor, b.value))
        add(Instruction.create_reg(Code.IMUL_RM32, factor))
    else:
        # memory_of raises for anything without a relocated address, rather
        # than treating a kind that is not CONSTANT as license to assume it
        # must be STATIC -- the one other kind there is today, but not
        # necessarily the only one there ever will be
        where = add(Instruction.create_mem(Code.IMUL_RM32, memory_of(b)))
        if b.at is not None:
            relocated[where] = b.at

    if shift.kind is Kind.CONSTANT:
        if not 0 <= shift.value < 32:
            return f"a shift of {shift.value} normalises nothing back into 32 bits"
        add(Instruction.create_reg_reg_i32(Code.SHRD_RM32_R32_IMM8, RESULT, Register.EDX, shift.value))
    else:
        where = add(Instruction.create_reg_mem(Code.MOV_R16_RM16, Register.CX, memory_of(shift)))
        if shift.at is not None:
            relocated[where] = shift.at
        add(Instruction.create_reg_reg_reg(Code.SHRD_RM32_R32_CL, RESULT, Register.EDX, Register.CL))

    if restore:
        for insn in restoring():
            add(insn)

    return assemble(steps, relocated)
