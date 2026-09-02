"""
A whole body, emitted -- and everything that has to move when it does.

select.py turns one operation into bytes and takes the address as a given.
That is not enough to rebuild a body, because the addresses are the thing
that changes: an instruction whose encoding differs in length from the one
BC wrote moves everything after it, and every branch into that region is
then pointing at the wrong byte.

So this is a fixed point, and the shape of it is what keeps it honest.
Every branch starts long. Addresses are assigned, each branch that reaches
its target within a signed byte is marked short, and the addresses are
assigned again. Shrinking only ever brings a target closer, so a branch
marked short stays reachable and the loop only ever goes one way -- which
is why it terminates rather than oscillating between two lengths that each
justify the other.

Then one final pass emits at the addresses the fixed point settled on, with
every target mapped through where it went.

What comes back is bytes plus the relocations, because a relocated
displacement is emitted as zero and the fixup that names it has to be moved
to wherever the field ended up.

Refuses the whole body where it cannot emit one op. A body half of which is
this pass's own code and half BC's is not something anything downstream
could reason about, and the fraction that cannot be emitted is small and
known: the x87 instructions, and the addresses in a space select.py does
not encode.
"""

from dataclasses import field
from dataclasses import dataclass

from iced_x86 import OpKind
from iced_x86 import Register

from qbopt import ir
from qbopt import mir
from qbopt import select
from qbopt.mir import MirBody
from qbopt.module import Module


@dataclass(frozen=True, slots=True)
class Laid:
    """A body's new bytes, and what moved."""

    code: bytes
    # Where each op ended up, old address -> new. The map a caller needs to
    # move anything that pointed into this body from outside it.
    moved: dict[int, int]
    # (offset within `code`, the original field's own address) for every
    # relocated displacement, so the fixup that names it can be moved.
    relocations: tuple[tuple[int, int], ...]
    # Fixups that belonged to an instruction this body no longer contains --
    # the high half of a widened pair reads `[x+2]` and folding it away
    # takes that relocation with it. Reported rather than silently omitted:
    # relocate.py refuses a fixup it cannot place, which is what catches a
    # dropped one, and it can only tell the two apart if told which were
    # meant to go.
    dropped: frozenset[int] = frozenset()
    # Every original address a transform folded into a surviving op, mapped
    # to where that op went. A /Zd build carries LINNUM records naming the
    # first byte of each statement, and folding two instructions into one
    # leaves some of those naming nothing -- the line's code now begins
    # where the survivor begins. Kept apart from `moved` deliberately: a
    # branch target may never resolve through this, and cannot, since a
    # target starts a block and nothing here folds a block's first op.
    covered: dict[int, int] = field(default_factory=dict)

    @property
    def grew(self) -> int:
        return len(self.code)


def _ordered(body: MirBody) -> list[mir.Op]:
    """Every op, in the order they are emitted.

    Blocks in address order, and within a block the order the block lists
    them. Identical to sorting every op by address while nothing reorders
    anything -- which is true of every body raised from BC's code -- and not
    the same rule: a transform that moves a definition within its block
    changes the list and must not have layout put it back.

    Blocks themselves are laid out where they already were rather than
    reordered: a different block order is a different program's control
    flow, and nothing here is asking for one.
    """
    return [op for block in sorted(body.blocks, key=lambda one: one.at) for op in block.ops]


# A root at each width an instruction can name it. ir.ROOT goes the other
# way; an allocation is per value and a value's register is its root.
_AT_WIDTH = {
    Register.EAX: {4: Register.EAX, 2: Register.AX, 1: Register.AL},
    Register.EBX: {4: Register.EBX, 2: Register.BX, 1: Register.BL},
    Register.ECX: {4: Register.ECX, 2: Register.CX, 1: Register.CL},
    Register.EDX: {4: Register.EDX, 2: Register.DX, 1: Register.DL},
    Register.ESI: {4: Register.ESI, 2: Register.SI},
    Register.EDI: {4: Register.EDI, 2: Register.DI},
    Register.EBP: {4: Register.EBP, 2: Register.BP},
}


def _remap(values: tuple, assignment: dict, origin: dict) -> dict:
    """One side's register remap, out of a whole-body allocation."""
    out: dict = {}
    for value in values:
        want = assignment.get(value)
        was = origin.get(value)
        if want is None or was is None or want is was:
            continue
        # At every width, not only the root. An allocation is per value and
        # origin holds the 32-bit root; the instruction names `ax`, so a map
        # keyed on `eax` alone never matches and select emits what it always
        # did -- which is what happened, silently, until this was measured.
        for width in (4, 2, 1):
            here, there = _AT_WIDTH.get(was, {}).get(width), _AT_WIDTH.get(want, {}).get(width)
            if here is not None and there is not None:
                out[here] = there
    return out


def _where(op: mir.Op, assignment: dict | None, origin: dict | None) -> tuple[dict, dict] | None:
    """This op's register remap, by side, out of a whole-body allocation.

    An allocation is per value and an instruction names registers, so the
    map is rebuilt per op out of the values it touches. By side, because one
    map cannot say two things about one register: `mov ax,1` defines a value
    and *uses* the eax before it -- writing ax preserves the high half --
    and with a single map, moving that older value rewrites the destination
    too. hotlop's counter became `mov bx,1` and was clobbered by the very
    value the move was meant to make room for.

    Where the allocation put every value back where BC had it -- which is
    every value unless something asked otherwise -- both are empty and
    select emits exactly what it did before.
    """
    if not assignment or origin is None:
        return None
    into = _remap(op.defines, assignment, origin)
    outof = _remap(op.uses, assignment, origin)
    return (into, outof) if into or outof else None


def _stands_for(op) -> tuple[int, int] | None:
    """The original bytes this op accounts for, as a range.

    Placement asks where an op *goes*; coverage asks which of BC's bytes it
    *stands for*. They are the same for everything BC wrote and differ the
    moment a pass moves an op -- a hoisted load runs in the preheader and
    still accounts for the bytes it came from. Anchoring coverage at `at`
    conflated the two and made moving anything impossible.
    """
    if isinstance(op, Table):
        return op.lo, op.hi
    if op.covers is not None:
        return op.covers
    length = _length_of(op)
    return None if length is None else (op.at, op.at + length)


def _length_of(op: mir.Op) -> int | None:
    """How many bytes the op occupied in the image it came from.

    From the node's own span rather than from an instruction, because not
    every node has one: calls.py's restore idiom is a single node covering
    four bytes and three instructions, and it is in every object this pass
    has already absorbed a call in.

    `covers` overrides it, and is how a transform accounts for what it
    replaced: an op standing in for two of BC's says so, and the byte
    arithmetic below still adds up.
    """
    if op.covers is not None:
        lo, hi = op.covers
        return hi - lo
    if op.node is None:
        return None
    lo, hi = ir.span(op.node)
    return hi - lo


def _trailing_zeros(found: Module, ops: list[mir.Op]) -> "Table | None":
    """The run of zero bytes the ops end on, where it reaches the segment's end.

    Only at the very end, and only all-zero: anything else that happens to
    decode is code until something proves otherwise.
    """
    highest = max(one.at + (_length_of(one) or 0) for one in ops)
    if highest != found.end:
        return None
    lo = found.end
    for one in sorted(ops, key=lambda x: x.at, reverse=True):
        length = _length_of(one) or 0
        if one.at + length != lo or any(found.code[one.at : lo]):
            break
        lo = one.at
    return None if lo == found.end else Table(lo, found.end)


PADDING = frozenset({0x90, 0x00})


def _padding_runs(
    found: Module,
    ops: list[mir.Op],
    carried: list["Table"],
    lowest: int,
    highest: int,
    reached: frozenset[int] | None = None,
) -> list["Table"]:
    """The gaps between the items that may be carried rather than selected.

    Padding is the easy half: BC aligns its procedures, so runs of `90` sit
    between them and nothing enters those.

    The other half is code the decoder never reached. Under /V, BC emits a
    call to B$EVCK after every statement, and in jumps-q-evt two of them sit
    directly after an unconditional `jmp` -- real relocated calls that
    nothing can arrive at. They were the only thing in the corpus that
    refused a whole-segment rebuild.

    Carrying them rests on one fact: no block walked in. A Table already
    copies its bytes and remaps every fixup inside it by however far it
    moved, so a relocated call travels correctly; what a Table cannot do is
    fix up a branch that lands in the middle of it, and reachability is the
    proof there is no such branch. If that proof were wrong the rebuild
    would already be unsound for the code around them.

    And where reachability is wrong in the one way that would matter -- a
    computed jump into a gap, through a form nothing here models -- the
    entry is a relocated word, so the target is an offset some record or
    fixup names. A gap's interior is not in the placement map, so
    relocate.as_records cannot map it and refuses the whole object. That
    refusal is what makes carrying safe rather than merely usually right,
    and it has to exist on both paths: the record one always did, the fixup
    one is newer.

    `reached` is what makes the question askable here. Without it only
    padding is carried, which is what this did before.
    """
    covered = set()
    for one in ops:
        span = _stands_for(one)
        if span is not None:
            covered.update(range(*span))
    for one in carried:
        covered.update(range(one.lo, one.hi))

    out: list[Table] = []
    start: int | None = None
    for at in range(lowest, highest + 1):
        empty = at < highest and at not in covered
        if empty and start is None:
            start = at
        elif not empty and start is not None:
            span = range(start, at)
            if all(one in PADDING for one in found.code[start:at]) or (
                reached is not None and not any(one in reached for one in span)
            ):
                out.append(Table(start, at))
            start = None
    return out


def _semantics(op: mir.Op) -> ir.Semantics | None:
    """What to select for this op, or None to carry its bytes.

    An op a MIR transform built has no node and no bytes to carry, so its
    own `made` is the only answer. An op raised from BC's code has a node,
    and that node's semantics is the authority.
    """
    what = op.made if op.made is not None else getattr(op.node, "semantics", None)
    return None if what is None or what.op is ir.Operation.BARRIER else what


def selectable(op: mir.Op) -> bool:
    """Whether this op's bytes come from select.py rather than from the image.

    An op emitted verbatim -- a barrier, calls.py's restore idiom, an
    emulated x87 site -- is exactly as long as the bytes it copies, so its
    `covers` and its length are the same number and a transform may not make
    them differ. One that is selected has no such tie: it emits whatever the
    encoding needs and `covers` only says which of the original bytes it
    stands for.

    transform.py asks before handing a deleted op's bytes to a survivor. It
    used to hand them to whoever was nearest, and a restore idiom that took
    them stopped coming back its own length -- qb-qrender's SCREEN.OBJ, and
    the only object in either corpus with the shape.
    """
    if isinstance(op.node, ir.Restore):
        return False
    return _semantics(op) is not None


def _retargeted(what: ir.Semantics, moved: dict[int, int]) -> ir.Semantics | None:
    """`what` with its target moved to wherever that instruction went.

    A target this body does not contain is refused rather than left alone:
    it would be an address into code this layout did not place, and quietly
    keeping the old number would point it at whatever now sits there.
    """
    if what.target is None:
        return what
    landed = moved.get(what.target)
    if landed is None:
        return None
    return ir.Semantics(what.op, what.name, what.dests, what.sources, landed)


def _field_in(found: Module, op: mir.Op, fields: frozenset[int] = frozenset()) -> int | None:
    """The address of the one relocated field inside `op`'s own bytes.

    Asked of the module rather than taken from the instruction's `disp_at`,
    which is the displacement and not always the field.

    A far call is the case that forced this and is handled apart: its four
    relocated bytes are a target rather than a displacement, so `disp_at` is
    None, and `fixup_at` does not carry it either -- that map holds the
    OFF16 fixups behind memory operands, and a far call's is a PTR32. What
    does know is `Module.calls`, and `at + 1` is not a guess: `9a` then four
    bytes is the only encoding a far call has.

    Otherwise exactly one fixup in the instruction's own span, or nothing.
    Two would mean an instruction with two relocated operands, which nothing
    here emits and which would have to say which field went where.
    """
    if op.node is None:
        return None
    # `push eax / pop ax / pop dx` is three register instructions and has no
    # field for a relocation to go in. It needs saying because the idiom is
    # put wherever a transform has an address to spare: on the chain's last
    # high half, which may be a store through a relocated displacement, or
    # on the very byte of a far call whose target is a fixup. Both would be
    # found by the search below and neither belongs to it. The fixup itself
    # is not lost -- it falls inside the covers of whatever stands for those
    # bytes, which is what `Laid.dropped` reports and relocate.py skips.
    if isinstance(op.node, ir.Restore):
        return None
    known = fields or frozenset(found.fixup_at)
    lo, hi = ir.span(op.node)
    # A far call and a far jmp put their four relocated bytes right after a
    # one-byte opcode. Neither is a displacement, so neither is where a
    # general search would look.
    if found.code[op.at : op.at + 1] in (b"\x9a", b"\xea") and op.at + 1 in known:
        return op.at + 1
    # An operation with nothing but registers has no field to put one in. A
    # transform that serves a read from a register leaves the instruction
    # standing where a memory operand was, and the fixup that named that
    # operand belongs to the read it replaced -- `covers` still accounts for
    # it, which is what Laid.dropped reports. After the far call above,
    # whose four relocated bytes are a target and not an operand.
    what = _semantics(op)
    if what is not None:
        holds = [one for one in (*what.dests, *what.sources) if isinstance(one, (ir.Mem, ir.Address, ir.Imm))]
        if not holds:
            return None
        # A transform that served a read from a register removed the only
        # operand a displacement could sit in, and left an immediate behind:
        # `cmp word [k],1` becomes `cmp ax,1`. The fixup still inside this
        # span named the operand that went, so relocating this instruction's
        # immediate writes an address over it and over the branch after it.
        # suite/jumps.bas took the CASE ELSE arm for k = 1 that way.
        # The question is whether a memory operand *went*, not whether a
        # transform touched the op: hoisting rewrites `mov ax,offset x` to
        # name a different register and its relocated immediate is still its
        # own. Asked the broad way, that fixup had nowhere to go and the
        # segment was refused.
        was = getattr(op.node, "semantics", None)
        had = was is not None and any(
            isinstance(one, (ir.Mem, ir.Address)) for one in (*was.dests, *was.sources)
        )
        if op.made is not None and had and not any(isinstance(one, (ir.Mem, ir.Address)) for one in holds):
            return None
    inside = [one for one in known if lo <= one < hi]
    return inside[0] if len(inside) == 1 else None


# A signed byte's worth of displacement, measured from the end of the
# instruction. The short branch's whole range.
REACH = range(-128, 128)


def _placed(ops: list, at: int, lengths: list[int]) -> tuple[list[int], dict[int, int]]:
    """Where each op lands, given what each one measures.

    Two things, because they are two questions. The list is where each op in
    this list goes, one entry per op. The map is what an *address* means
    afterwards, and a transform may put several ops on one address -- a
    replacement needs somewhere to hang each operation it emits and a call
    is five bytes wide however many it becomes. The first of a group is what
    that address means to everything outside: a branch to it arrives at the
    start of what replaced it, never into the middle.
    """
    placed: list[int] = []
    moved: dict[int, int] = {}
    where = at
    for op, length in zip(ops, lengths):
        placed.append(where)
        moved.setdefault(op.at, where)
        where += length
    return placed, moved


def lay_out(body: MirBody, at: int, found: Module, fields: frozenset[int] = frozenset()) -> Laid | str:
    """Every op in `body`, emitted in order from `at`, or why it could not be."""
    return _emitted(_ordered(body), at, found, fields)


@dataclass(frozen=True, slots=True)
class Table:
    """A run of bytes between the instructions, copied rather than selected.

    BC drops an ON GOTO table inline: a count byte and one relocated word
    per destination. The words are fixups, so copying the bytes and moving
    the fixups is enough -- as_records remaps each one's own displacement,
    which is where the destination actually lives.
    """

    lo: int
    hi: int

    @property
    def at(self) -> int:
        return self.lo


def rebuild(
    found: Module,
    bodies: list[tuple[str, MirBody]],
    tables: tuple[tuple[int, int], ...] = (),
    fields: frozenset[int] = frozenset(),
    reached: frozenset[int] | None = None,
    native_fpu: bool = False,
    assignment: dict | None = None,
) -> Laid | str:
    """Every body in the module, laid out one after another.

    Whole-segment rather than per-body, because per-body does not work:
    splicing one back into BC's own layout is possible for 1 of the corpus's
    171 bodies -- the rest cross a LEDATA boundary, are not contiguous, or
    are branched into from outside. None of that applies to writing the
    segment, where boundaries and offsets are being produced rather than
    preserved.

    It also settles the targets that refused per-body: a branch from one
    body into another has somewhere to land once every body is in the same
    map.

    What comes back starts at the first body's own address. Whatever sits
    before it -- BC's module header, 48 bytes of `blARITH` and padding, and
    the only thing in the corpus's code segments that is not in a body --
    is the caller's to keep.
    """
    ops = sorted(
        (op for _, body in bodies for op in _ordered(body)),
        key=lambda one: one.at,
    )
    if not ops:
        return "no bodies to rebuild"
    if any(_length_of(one) is None for one in ops):
        return f"{ops[0].at:#06x}: an op with no instruction behind it"

    lowest = ops[0].at
    highest = max((_stands_for(one) or (one.at, one.at))[1] for one in ops)
    inside = [Table(lo, hi) for lo, hi in tables if lowest <= lo and hi <= highest]

    # BC pads the end of its code segment with zeros, and every object in
    # the corpus ends with four of them. Reachability walks in and the
    # decoder obliges -- `00 00` is `add [bx+si],al` -- so they arrive here
    # as ops, on an address no fixup names and select.py rightly will not
    # encode. They are not instructions and are carried rather than
    # selected: the same bytes, in the same place, which is the only thing
    # that can be right about padding.
    padding = _trailing_zeros(found, ops)
    if padding is not None:
        inside.append(padding)
        ops = [one for one in ops if one.at < padding.lo]
        if not ops:
            return "the body is nothing but padding"
        highest = padding.hi

    # BC aligns its procedures, so runs of `90` sit between them, and
    # nothing reaches those. Carried the same way a table is: the bytes are
    # what they were and nothing enters them, so where they end up does not
    # matter. Only runs that are entirely padding -- anything else in a gap
    # is bytes this cannot account for, and it says so instead.
    inside += _padding_runs(found, ops, inside, lowest, highest, reached)

    # Every byte between the first item and the last has to be one of them.
    # What is left over is data nothing here can name, and emitting only what
    # it understands would drop it silently along with anything it holds.
    covered = sum(_length_of(one) or 0 for one in ops) + sum(one.hi - one.lo for one in inside)
    if covered != highest - lowest:
        # Named where the gap is, not where the layout starts. It used to
        # report `lowest`, which sent every reading of this straight to the
        # first instruction in the segment and nowhere near the bytes.
        held = set()
        claims: dict[int, list[int]] = {}
        for one in ops:
            span = _stands_for(one)
            if span is not None:
                held.update(range(*span))
                for byte in range(*span):
                    claims.setdefault(byte, []).append(one.at)
        for one in inside:
            held.update(range(one.lo, one.hi))
        first = next((one for one in range(lowest, highest) if one not in held), None)
        if first is None:
            # Every byte is accounted for and the total still disagrees, so
            # two ops claim the same ones. A transform that moves an op and
            # leaves its `covers` behind does exactly that. Reported, rather
            # than raising StopIteration looking for a gap that is not
            # there -- which is what it did, from inside the error path.
            twice = sorted(one for one in range(lowest, highest) if len(claims.get(one, ())) > 1)
            where = f"{twice[0]:#06x}" if twice else "nowhere"
            return f"{where}: {covered - (highest - lowest)} bytes are claimed by more than one op"
        return f"{first:#06x}: {highest - lowest - covered} bytes between the ops are not instructions"

    origin = {}
    for _name, body in bodies:
        origin.update(body.origin)
    return _emitted(
        sorted([*ops, *inside], key=lambda one: one.at), lowest, found, fields, native_fpu, assignment, origin
    )


def _emitted(
    ops: list,
    at: int,
    found: Module,
    fields: frozenset[int] = frozenset(),
    native_fpu: bool = False,
    assignment: dict | None = None,
    origin: dict | None = None,
) -> Laid | str:
    """Every item in order from `at`, shrunk to a fixed point and emitted.

    An item is an op, which select.py encodes, or a Table, which is copied.
    """
    if not ops:
        return "no ops to lay out"

    # Kept per op rather than per address. An op's address is where it came
    # from, and several may share one: a transform that replaces a five-byte
    # call with six operations has five addresses to give them and needs the
    # sixth anyway. Keying the measurement by address made that impossible
    # and silently -- one entry overwrote the other and the body came out
    # the wrong length.
    lengths: list[int] = []
    for op in ops:
        if isinstance(op, Table):
            lengths.append(op.hi - op.lo)
            continue
        what = _semantics(op)
        emulated = not native_fpu and found.code[op.at : op.at + 1] == bytes([0xCD])
        if isinstance(op.node, ir.Restore):
            # Measured from what it emits, not from `covers`. The two are
            # different questions -- covers says which of BC's bytes this op
            # stands for, and a transform sets it to whatever makes the
            # chain tile -- and reading the emitted length off covers forced
            # every restore to claim exactly four bytes, which pairs.py could
            # only arrange by putting it on an address a widened op already
            # held.
            made = select.restore(op.node.pair)
            if made is None:
                return f"{op.at:#06x}: the restore idiom is not one select.py can emit"
            lengths.append(len(made.code))
            continue
        if what is None or emulated:
            lengths.append(_length_of(op) or 0)
            continue
        made = select.emit(
            what, at=at, where=_where(op, assignment, origin), relocated=_field_in(found, op, fields) is not None
        )
        if made is None:
            return f"{op.at:#06x}: {op.name} is not one select.py can emit"
        lengths.append(len(made.code))

    # Shrink to a fixed point. Every branch starts long; one that reaches its
    # target within a signed byte becomes short, which moves everything after
    # it closer and can only let more of them shrink.
    short: set[int] = set()  # by position, since an address may hold several
    placed, moved = _placed(ops, at, lengths)
    changing = True
    while changing:
        changing = False
        for index, op in enumerate(ops):
            if isinstance(op, Table):
                continue
            what = _semantics(op)
            if what is None or what.target is None or index in short:
                continue
            landed = moved.get(what.target)
            if landed is None:
                continue
            # Through `moved`, the same as the emission below. Asking for the
            # short form of a branch that still names its *original* target,
            # from the address it has *moved* to, measures a displacement
            # that is neither -- and iced refuses the byte form when that
            # overflows, so the branch stayed long. Invisible while a body
            # barely moves, and worth 56 bytes an object once absorption
            # takes 328 out of one.
            aimed = _retargeted(what, moved)
            if aimed is None:
                continue
            made = select.emit(
                aimed,
                at=placed[index],
                where=_where(op, assignment, origin),
                short=True,
                relocated=_field_in(found, op, fields) is not None,
            )
            if made is None:
                continue  # a call has no short form, and says so by refusing
            if landed - (placed[index] + len(made.code)) not in REACH:
                continue
            short.add(index)
            lengths[index] = len(made.code)
            changing = True
        if changing:
            placed, moved = _placed(ops, at, lengths)

    # The bytes, at the addresses the fixed point settled on.
    out = bytearray()
    relocations: list[tuple[int, int]] = []
    for index, op in enumerate(ops):
        if isinstance(op, Table):
            # Copied verbatim, with every fixup inside it moved by the same
            # amount the table itself moved. The entries are relocated words
            # and their destinations live in the fixups' own displacements,
            # which as_records remaps.
            out += found.code[op.lo : op.hi]
            for field in sorted(one for one in (fields or frozenset(found.fixup_at)) if op.lo <= one < op.hi):
                relocations.append((placed[index] - at + (field - op.lo), field))
            continue
        # An emulated x87 site is emitted as it was found. declen.py decodes
        # `cd 35 46 c8` as the fld it stands for, so selecting from the
        # semantics would emit `d9 46 c8` -- a native instruction, on a
        # machine that may have no coprocessor. That conversion is a
        # decision fpu.py gates behind --native-fpu -- so laying a segment
        # out does not make it silently, and makes it when asked: with
        # native_fpu the site is selected from its own semantics instead,
        # which is the same x87 instruction the emulator stands for and is
        # what M5 means by expressing fpu.py's pass over MIR.
        if not native_fpu and found.code[op.at : op.at + 1] == bytes([0xCD]) and (length := _length_of(op)):
            # Copied, so any fixup inside it keeps its place within the
            # instruction and only the instruction itself has moved.
            field = _field_in(found, op, fields)
            if field is not None:
                relocations.append((len(out) + (field - op.at), field))
            out += found.code[op.at : op.at + length]
            continue
        # A barrier is an instruction ir.py models nothing about --
        # `movsx eax,bx` is one -- so there is nothing to select from and
        # its own bytes are the only right answer. Carried, unless it names
        # a branch target: that would move, and keeping the old number
        # would point it at whatever now sits there.
        if _semantics(op) is None and (length := _length_of(op)):
            found_insn = getattr(op.node, "insn", None)
            if found_insn is not None and found_insn.insn.op0_kind == OpKind.NEAR_BRANCH16:
                return f"{op.at:#06x}: a branch this cannot model would keep a stale target"
            field = _field_in(found, op, fields)
            if field is not None:
                relocations.append((len(out) + (field - op.at), field))
            out += found.code[op.at : op.at + length]
            continue
        if isinstance(op.node, ir.Restore):
            made = select.restore(op.node.pair)
            if made is None or len(made.code) != lengths[index]:
                return f"{op.at:#06x}: the restore idiom did not come back its own length"
            out += made.code
            continue
        before = _semantics(op)
        if before is None:
            return f"{op.at:#06x}: {op.name} has no semantics to select from"
        what = _retargeted(before, moved)
        if what is None:
            return f"{op.at:#06x}: its target is not in this body"
        made = select.emit(
            what,
            at=placed[index],
            where=_where(op, assignment, origin),
            short=index in short,
            relocated=_field_in(found, op, fields) is not None,
        )
        if made is None or len(made.code) != lengths[index]:
            return f"{op.at:#06x}: it changed length between the two passes"
        # A fixup goes wherever this instruction's one relocatable field
        # landed -- the displacement for a memory operand, the immediate for
        # `push offset X` and `mov ax,offset X`, which are 464 of the
        # corpus's fixups on their own.
        field = _field_in(found, op, fields)
        if field is not None:
            landed = made.relocated_at
            if landed is None:
                return f"{op.at:#06x}: {op.name} has a fixup and no field to put it in"
            relocations.append((len(out) + landed, field))
        out += made.code
    # Every fixup inside a surviving op's `covers` but outside its own
    # node's span belonged to something a transform folded away. One
    # outside every op's covers is not explained by anything here, and
    # relocate.py still refuses it.
    kept_fields = {one for _where, one in relocations}
    explained: set[int] = set()
    known = fields or frozenset(found.fixup_at)
    folded: dict[int, int] = {}
    for op in ops:
        if isinstance(op, Table):
            continue
        lo, hi = op.covers if op.covers is not None else (op.at, op.at + (_length_of(op) or 0))
        explained.update(one for one in known if lo <= one < hi)
        landed = moved.get(op.at)
        if landed is not None:
            folded.update({one: landed for one in range(lo, hi) if one not in moved})
    return Laid(bytes(out), moved, tuple(relocations), frozenset(explained - kept_fields), folded)
