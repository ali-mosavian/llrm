"""The assembler: instructions in, one image and its relocations out.

LLVM's `MCAssembler`. `select.py` is the `MCCodeEmitter` above it -- what
one instruction's bytes are -- and `relocate.py` the `MCObjectWriter`
below, which turns the image and its relocations into records. This is the
middle: how long each instruction is, where each therefore lands, which
branches can shrink now that everything is closer, and where each fixup
ended up.

Split out of `layout.py`, which held this and the allocation and the
byte-preservation check in one module. The split is not cosmetic. Layout
colours a body when it is handed no assignment, and `objwrite.py` -- which
runs after a real allocator -- had no way to say "already done": passing
nothing made layout allocate a second time over operands that were already
physical registers, and the remap that followed produced a call encoding
with no field for its own fixup. Nineteen objects said
`call has 1 fixups and 0 fields to put them in`.

An assembler does not allocate. Handed `assignment=None` this remaps
nothing, which is the right answer for a body whose registers are already
chosen -- and `layout.rebuild` passes the assignment it computed, which is
the right answer for one whose registers are not.

Two passes over the instructions, because a length depends on where things
land and where things land depends on lengths: measure everything long,
shrink every branch that reaches within a byte, repeat until nothing
changes, then emit.
"""

from dataclasses import field
from dataclasses import dataclass

from iced_x86 import OpKind
from iced_x86 import Register

from qbopt import ir
from qbopt import mir
from qbopt import lower
from qbopt import select
from qbopt import target
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


def _ranges_of(op, found: Module) -> tuple[tuple[int, int], ...]:
    """Every disjoint range of original bytes this op stands for.

    `found.coverage` is the one authoritative record for an op whose bytes
    are not a single run -- a site frames() found may push its arguments,
    let BC put a real instruction between the pushes and the call, and only
    then call, so no one interval names both without also claiming the
    instruction sitting between them. Every other op has no entry there and
    `covers` alone is still the whole answer -- a legacy adapter, since
    only this one idiom needs more than it.
    """
    if isinstance(op, Table):
        return ((op.lo, op.hi),)
    full = found.coverage.get(op.id) if op.id is not None else None
    if full is not None:
        return full
    if op.covers is not None:
        return (op.covers,)
    length = _length_of(op, found)
    return () if length is None else ((op.at, op.at + length),)


def _stands_for(op, found: Module) -> tuple[int, int] | None:
    """The overall span of original bytes this op accounts for.

    Placement asks where an op *goes*; coverage asks which of BC's bytes it
    *stands for*. They are the same for everything BC wrote and differ the
    moment a pass moves an op -- a hoisted load runs in the preheader and
    still accounts for the bytes it came from. Anchoring coverage at `at`
    conflated the two and made moving anything impossible.

    The outer envelope of every disjoint range `_ranges_of` returns --
    right for a caller that only wants an extent, such as the highest byte
    in the body. A caller doing per-byte accounting wants the disjoint
    ranges themselves, not this: the gap between two of them may be a real
    instruction that owns those bytes on its own account.
    """
    ranges = _ranges_of(op, found)
    return (min(lo for lo, _ in ranges), max(hi for _, hi in ranges)) if ranges else None


def _length_of(op: mir.Op, found: Module) -> int | None:
    """How many bytes the op occupied in the image it came from.

    From the node's own span rather than from an instruction, because not
    every node has one: calls.py's restore idiom is a single node covering
    four bytes and three instructions, and it is in every object this pass
    has already absorbed a call in.

    `covers` overrides it, and is how a transform accounts for what it
    replaced: an op standing in for two of BC's says so, and the byte
    arithmetic below still adds up. `found.coverage` is asked first and,
    where it has an answer, is the whole of it: a site whose pushes sit
    apart from its call stands for both runs, and `covers` alone would
    only ever name one of them.
    """
    full = found.coverage.get(op.id) if op.id is not None else None
    if full is not None:
        return sum(hi - lo for lo, hi in full)
    if op.covers is not None:
        lo, hi = op.covers
        return hi - lo
    if op.node is None:
        return None
    # What the operation says it carries, established at the raise. The
    # search below is the fallback for a caller that built ops itself --
    # tests do -- and for anything raised before mir._referenced ran.
    lo, hi = ir.span(op.node)
    return hi - lo


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


def _still_has_an_operand_for_it(op: mir.Op) -> bool:
    """Whether the operand the fixup named is still in this operation.

    A transform that serves a read from a register removes the only operand
    a displacement could sit in and leaves an immediate: `cmp word [k],1`
    becomes `cmp ax,1`, and relocating that immediate writes an address over
    it and over the branch behind it. suite/jumps.bas took the CASE ELSE arm
    for k = 1 that way.

    The question is whether a memory operand *went*, not whether a transform
    touched the operation: hoisting rewrites `mov ax,offset x` to name
    another register and its relocated immediate is still its own.
    """
    # What rewrote it, or None for an operation nothing did -- the question
    # this asks. `op.made` used to be that marker; a pass that says what it
    # computes in MIR's own operands sets nothing, so ask lower.
    what = lower.rewritten(op)
    if what is None:
        return True
    holds = [one for one in (*what.dests, *what.sources) if isinstance(one, (ir.Mem, ir.Address, ir.Imm))]
    if not holds:
        return False
    was = getattr(op.node, "semantics", None)
    had = was is not None and any(isinstance(one, (ir.Mem, ir.Address)) for one in (*was.dests, *was.sources))
    return not (had and not any(isinstance(one, (ir.Mem, ir.Address)) for one in holds))


def _absorbed(site, read):
    """The instructions one folded runtime call becomes.

    The raise turned a push run and its call into one operation over the
    argument values; this is where that operation becomes code again. Four
    instructions for a long divide, two of them relocated -- which is the
    case Emitted.places exists for.
    """
    from qbopt import calls as machine

    # A divide hands its answer's high half back through an operation of
    # its own, so the sequence must not end in the idiom as well: emitted
    # twice, the second pops what the first had already put back.
    made = select.absorbed(site, read, site.name not in machine.DIVIDES)
    return None if isinstance(made, str) else made


def _folded_site(op: mir.Op, found: Module):
    """The site this op stands for, where it still stands for one.

    The record is what says to emit a divide rather than the call BC
    wrote, and a pass that rewrites the operation into something else --
    a copy of an answer already computed -- leaves the id where it was.
    So the op's own kind decides: where it no longer raises as the kind
    its site does, the site is not what it is any more and the operation
    speaks for itself. Emitted from the record, the copy came back as the
    whole divide.

    Dropping the record instead is what the pass must not do. A body the
    allocator refuses is laid out as it was *raised* and still holds the
    divide: with no record it emitted as BC's bare call with its push run
    already folded away, and lngmix stopped early under DOSBox. That is
    the refusal below -- the bytes at a divide's address are its call and
    the last of its pushes, so carrying them verbatim writes a call with
    one argument, silently.
    """
    if op.id is None:
        return None
    folded = found.absorbed.get(op.id)
    if folded is None:
        if op.kind is mir.Kind.DIVMOD:
            return f"{op.at:#06x}: a divide with no site of its own cannot be emitted"
        return None
    kind = mir.absorbs(folded[0].name)
    if kind is not None and op.kind is not kind:
        return None
    return folded


def _seats(op: mir.Op, assignment: dict | None, origin: dict | None) -> "tuple | None":
    """Which register each of an operation's results is in.

    The allocation where there is one, and where BC had it otherwise --
    the same two sources `_where` reads, asked per result rather than per
    operand, because what an idiom has to emit is where its answers go.

    The two are not interchangeable, and reading them as one map hid the
    case that matters. No allocation at all is the identity baseline: the
    assembler remaps nothing and BC's own register is where the value is.
    An allocation that was supplied and does not mention this value is a
    different thing entirely -- something placed every other value and
    not this one -- and falling back to BC's register there emits an
    answer into a register the allocation has given to something else.
    So origin answers only when there is no allocation at all -- None, the
    sentinel -- and a value missing from one that exists refuses the
    emission. An empty map that was supplied is an allocation that placed
    nothing, not the absence of one.

    A value a pass invented has neither, and is refused by both arms.
    """
    seats = []
    for one in op.results:
        value = getattr(one, "value", None)
        if value is None:
            return None
        where = assignment.get(value) if assignment is not None else (origin or {}).get(value)
        if where is None:
            return None
        seats.append(where)
    return tuple(seats) if len(seats) == 2 else None


def _divide_fields(op: mir.Op, found: Module, fields: frozenset[int]) -> "tuple[int, ...] | None":
    """The fixup each of this op's memory operands still names, in order.

    A fixup belongs to the operand it was read off. The raise recorded them
    in the order it emitted them, which is the order of the operands that
    read memory -- so operand i's fixup is entry i, and it is still that
    operand's only while the operand is the one it was recorded for.

    An operand a pass turned into a constant or a register reads no memory
    and its fixup goes with it: nothing is emitted for it, and asm's own
    accounting drops a fixup no surviving field claims. An operand pointed
    at a different cell is the case counting cannot see -- the count is the
    same and every byte the fixup lands on is wrong -- so it has no binding
    and this returns None rather than guessing one.
    """
    was = list(op.raised[0]) if op.raised is not None else list(op.args)
    recorded = _fields_in(found, op, fields)
    known = {
        position: recorded[order]
        for order, position in enumerate(index for index, one in enumerate(was) if isinstance(one, mir.Cell))
        if order < len(recorded)
    }
    out: list[int] = []
    for index, one in enumerate(op.args):
        if not isinstance(one, mir.Cell):
            continue
        if index >= len(was) or was[index] != one or index not in known:
            return None
        out.append(known[index])
    return tuple(out)


def _selected_divide(op: mir.Op, found: Module, assignment, origin, fields):
    """A divide emitted from its own operands: (bytes, its fixups), or why not.

    A string is a refusal of the whole emission and never a signal to fall
    back. Emitting the site frozen at the raise after a pass has rewritten
    the operation would run the operands BC pushed as if they were the new
    ones -- a wrong answer rather than a refusal. An operation nothing has
    rewritten is the one case where the two are the same program, and
    mir.rewritten() is what says so.
    """
    if op.kind is not mir.Kind.DIVMOD:
        return None
    seats = _seats(op, assignment, origin)
    made = select.divides(op, seats, restore=False) if seats is not None else "no register holds a result"
    wanted = _divide_fields(op, found, fields)
    if not isinstance(made, str) and wanted is not None and len(made.places) == len(wanted):
        return made, wanted
    if mir.rewritten(op):
        why = made if isinstance(made, str) else "no fixup here names the operands it now reads"
        return f"{op.at:#06x}: {why}, and the bytes the site was raised with are not this operation"
    return None


def _fields_in(found: Module, op: mir.Op, fields: frozenset[int] = frozenset()) -> tuple[int, ...]:
    """Every fixup this operation's own operands carry, in operand order.

    One for one instruction, which is every operation the raise makes. An
    operation that stands for an idiom has one per relocated instruction in
    it, and they are placed in the order the instructions were emitted.
    """
    said = found.refs.get(op.id) if op.id is not None else None
    if said is not None and len(said) > 1:
        wanted = tuple(one for one in said if not fields or one in fields)
        if wanted and _still_has_an_operand_for_it(op):
            return wanted
    one = _field_in(found, op, fields)
    return () if one is None else (one,)


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
    # What the operation says it carries, established at the raise while the
    # spans were still BC's. Subject to the caller's own set: `fields` is how
    # a caller says which fixups it is accounting for.
    said = found.refs.get(op.id) if op.id is not None else None
    ref = said[0] if said else None
    if ref is not None and (not fields or ref in fields):
        return ref if _still_has_an_operand_for_it(op) else None
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
        had = was is not None and any(isinstance(one, (ir.Mem, ir.Address)) for one in (*was.dests, *was.sources))
        rewrote = op.made is not None or lower.semantics(op, was) is not None
        if rewrote and had and not any(isinstance(one, (ir.Mem, ir.Address)) for one in holds):
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


def assemble(
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
        # `op.node is not None` because this reads the original bytes at
        # `op.at`, and an inserted instruction has none: it carries the
        # address of the one it stands beside. A `sub sp,4` the prologue
        # put at an emulated x87 site's address was read as that site and
        # carried verbatim -- zero bytes in the length pass, three in the
        # emit pass, and layout said it changed length between them.
        emulated = not native_fpu and op.node is not None and found.code[op.at : op.at + 1] == bytes([0xCD])
        folded = _folded_site(op, found)
        if isinstance(folded, str):
            return folded
        if folded is not None:
            chosen = _selected_divide(op, found, assignment, origin, fields)
            if isinstance(chosen, str):
                return chosen
            made = chosen[0] if chosen is not None else _absorbed(*folded)
            if made is None:
                return f"{op.at:#06x}: the absorbed call is not one select.py can emit"
            lengths.append(len(made.code))
            continue
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
            lengths.append(_length_of(op, found) or 0)
            continue
        made = select.emit(
            what,
            at=at,
            where=_where(op, assignment, origin),
            held=_held(assignment),
            relocated=_field_in(found, op, fields) is not None,
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
                held=_held(assignment),
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
        if (
            not native_fpu
            and op.node is not None  # an inserted instruction has no original bytes
            and found.code[op.at : op.at + 1] == bytes([0xCD])
            and (length := _length_of(op, found))
        ):
            # Copied, so any fixup inside it keeps its place within the
            # instruction and only the instruction itself has moved.
            field = _field_in(found, op, fields)
            if field is not None:
                relocations.append((len(out) + (field - op.at), field))
            out += found.code[op.at : op.at + length]
            continue
        # Before the carry below, which is the order the length pass asks
        # in. A restore has no semantics -- it stands for three
        # instructions, not one -- so asking the other way round took the
        # carry and wrote the two bytes it sits on where four were
        # measured. negnot printed A=-7317895 for 305419897.
        if isinstance(op.node, ir.Restore):
            made = select.restore(op.node.pair)
            if made is None or len(made.code) != lengths[index]:
                return f"{op.at:#06x}: the restore idiom did not come back its own length"
            out += made.code
            continue
        # A barrier is an instruction ir.py models nothing about --
        # `movsx eax,bx` is one -- so there is nothing to select from and
        # its own bytes are the only right answer. Carried, unless it names
        # a branch target: that would move, and keeping the old number
        # would point it at whatever now sits there.
        if _semantics(op) is None and (length := _length_of(op, found)):
            found_insn = getattr(op.node, "insn", None)
            if found_insn is not None and found_insn.insn.op0_kind == OpKind.NEAR_BRANCH16:
                return f"{op.at:#06x}: a branch this cannot model would keep a stale target"
            field = _field_in(found, op, fields)
            if field is not None:
                relocations.append((len(out) + (field - op.at), field))
            out += found.code[op.at : op.at + length]
            continue
        folded = _folded_site(op, found)
        if isinstance(folded, str):
            return folded
        if folded is not None:
            chosen = _selected_divide(op, found, assignment, origin, fields)
            if isinstance(chosen, str):
                return chosen
            made = chosen[0] if chosen is not None else _absorbed(*folded)
            if made is None or len(made.code) != lengths[index]:
                return f"{op.at:#06x}: the absorbed call changed length between the two passes"
            binds = chosen[1] if chosen is not None else _fields_in(found, op, fields)
            for where, field in zip(made.places, binds, strict=False):
                relocations.append((len(out) + where, field))
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
            held=_held(assignment),
            short=index in short,
            relocated=_field_in(found, op, fields) is not None,
        )
        if made is None or len(made.code) != lengths[index]:
            return f"{op.at:#06x}: it changed length between the two passes"
        # A fixup goes wherever the field it names landed -- the
        # displacement for a memory operand, the immediate for
        # `push offset X` and `mov ax,offset X`, which are 464 of the
        # corpus's fixups on their own.
        #
        # Paired in order, because an operation is not always one
        # instruction: absorbing a long divide is four and two of them are
        # relocated, and each fixup belongs to the operand it was read off.
        wanted = _fields_in(found, op, fields)
        if wanted:
            landed = made.places
            if len(landed) < len(wanted):
                return f"{op.at:#06x}: {op.name} has {len(wanted)} fixups and {len(landed)} fields to put them in"
            for where, field in zip(landed, wanted, strict=False):
                relocations.append((len(out) + where, field))
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
        landed = moved.get(op.at)
        for lo, hi in _ranges_of(op, found):
            explained.update(one for one in known if lo <= one < hi)
            if landed is not None:
                folded.update({one: landed for one in range(lo, hi) if one not in moved})
    return Laid(bytes(out), moved, tuple(relocations), frozenset(explained - kept_fields), folded)


def _semantics(op: mir.Op) -> ir.Semantics | None:
    """What to select for this op, or None to carry its bytes.

    An op a MIR transform built has no node and no bytes to carry, so its
    own `made` is the only answer. An op raised from BC's code has a node,
    and that node's semantics is the authority.
    """
    what = lower.current(op)
    return None if what is None or what.op is ir.Operation.BARRIER else what


# The registers a remap may name. Not `target.AT_WIDTH`, which is the whole
# file: sp is in it, and reading a rename of the stack pointer as possible
# dropped a fixup on 52 of the corpus's objects. bp is here and is not
# allocatable either -- an operand reached through it is still an operand a
# remap has to be able to follow. The file says what a register *is*; this
# says which of them one value can be moved between.
_RENAMEABLE = {where: widths for where, widths in target.AT_WIDTH.items() if where is not Register.ESP}


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
            here, there = _RENAMEABLE.get(was, {}).get(width), _RENAMEABLE.get(want, {}).get(width)
            if here is not None and there is not None:
                out[here] = there
    return out


def _held(assignment: dict | None) -> dict | None:
    """The allocation keyed by value id, which is what ir.Held names.

    ir.py sits below mir.py and cannot import Value, so a Held carries the
    id. This is the other end of that.
    """
    if not assignment:
        return None
    return {value.id: register for value, register in assignment.items()}


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
