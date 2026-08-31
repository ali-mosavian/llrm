"""
What memory already holds, and what still wants it.

flags.py and registers.py answer this for a flag and for one register;
this answers it for a named memory cell, in both directions and across the
whole control-flow graph rather than one block at a time. Both directions
are the same iterate-to-a-fixed-point shape those two already use.

`available()` runs forwards: a cell is available at a point when, on EVERY
path reaching it, something has already loaded or stored that cell and
nothing has written over it since. A load whose cells are all available is
redundant -- BC had the value and went back to memory for it anyway.

`live()` runs backwards: a cell is live when some path from here reads it
before writing it. A store whose cells are all dead need never have
happened. The two together are what a register allocator would collect,
and measuring them is how this pass decides whether building one is worth
it (tools/memtraffic.py).

Three things decide whether either answer is any good, and all three were
found by measuring rather than by reasoning:

**The stack is not memory this tracks.** A push or a pop names a cell below
sp, which is neither a named static nor a frame local -- frame locals live
above sp. Treating a push's own stack cell as an unknown write is the
mistake that made every one of nbody's loops unanalysable: `push` is around
36% of the instructions in this corpus, so one bad classification there
poisons everything. `used_memory()` reports the stack cell alongside the
real operand (`push [x]` reads x AND writes a stack cell), which is why the
entries are filtered by base register rather than taken first.

**An indexed address is settled by its own base register.** module.may_alias
refuses to say anything about `[si+arr]`, correctly: si is unbounded, so on
its own that address could be any byte of its segment. But two accesses
through the SAME base register differ by exactly their displacements
whatever that register holds, which is the ordinary arithmetic every static
already gets. That is sound only while the register is unchanged between
them -- so every tracked cell whose base an instruction writes is dropped
before that instruction's own access is recorded. Without this, an array
element can never be forwarded to itself, and BC's array code is nothing
but indexed accesses.

A $DYNAMIC array element (`es:[bx]`, module.Space.FAR) is this same argument
with a second register in play: `es:[bx+6]` and `es:[bx+10]` differ by
arithmetic only while BOTH bx and es still hold what they held, so a cell is
dropped on a write to either one, not just to bx -- `_forward`'s own
clobber-drop checks `c.segment` alongside `c.base` for exactly this reason.
`mov es,[desc+2]` (17,593 of them in qb-qrender) is the load that makes this
matter: it is a plain register write as far as `_clobbered` is concerned, and
every tracked es:bx cell has to die there the same way a tracked si-indexed
cell already dies at a write to si.

**A call is what its contract says.** runtime.py, not a name list -- a
routine with no entry there comes back worst-case, so an unknown call is a
barrier by construction. Measured, this buys little on its own (the print
family writes anything, and it dominates), but it is what lets the four
absorbed arithmetic routines stop cutting a loop in half.
"""

from dataclasses import dataclass

from iced_x86 import Register
from iced_x86 import Register_
from iced_x86 import MemorySizeExt

from qbopt import module
from qbopt import runtime
from qbopt.ir import ROOT
from qbopt.declen import INFO
from qbopt.declen import Insn
from qbopt.module import Addr
from qbopt.blocks import Block
from qbopt.declen import READS
from qbopt.lift import operand
from qbopt.module import Space
from qbopt.declen import WRITES
from qbopt.lift import Resolver
from qbopt.flags import CLOBBERS
from qbopt.loops import predecessors

# The stack pointer in both widths -- a memory operand based on it is the
# cell a push or pop moves, never a variable.
STACK = (Register.SP, Register.ESP)


class Unnameable:
    """Memory this cannot name -- an operand lift.operand() refuses.

    Its own type rather than a string or None, because the two answers it
    sits between are opposites: None means "nothing to track here" and is
    safely ignored, this means "something happened and I cannot say what",
    which no caller may treat as harmless.
    """

    __slots__ = ()

    def __repr__(self) -> str:
        return "UNKNOWN"


UNKNOWN = Unnameable()


@dataclass(frozen=True, slots=True)
class Access:
    """The one named memory operand an instruction has, if it has one."""

    addr: Addr
    width: int
    reads: bool
    writes: bool

    @property
    def cells(self) -> tuple[Addr, ...]:
        """The individual bytes, since BC stores a long as two word writes
        and reads it back as one dword -- matching by whole access misses it."""
        return tuple(self.addr.plus(step) for step in range(self.width))


def access_of(insn: Insn, resolve: Resolver) -> Access | Unnameable | None:
    """The memory this names: an Access, UNKNOWN, or None for stack-only.

    None means "nothing this needs to track" -- an instruction with no memory
    operand at all, or one whose only operand is a push/pop's own stack cell.
    UNKNOWN means "memory this cannot name", which no caller may treat as
    harmless.
    """
    for one in INFO.info(insn.insn).used_memory():
        if one.base in STACK:
            continue
        addr = operand(insn, resolve)
        if addr is None:
            return UNKNOWN
        return Access(
            addr,
            MemorySizeExt.size(one.memory_size),
            one.access in READS,
            one.access in WRITES,
        )
    return None


def aliases(cell: Addr, access: Access, dgroup: frozenset[int]) -> bool:
    """Whether a write through `access` could land on `cell`.

    module.may_alias, plus the one case it refuses that a shared base
    register settles -- see this module's own docstring. The precondition is
    the caller's: `cell` must not have survived a write to its base register,
    which `_step` enforces by dropping those cells first.

    A Space.FAR cell's `segment` register is part of that same precondition,
    one level up: `es:[bx+6]` and `es:[bx+10]` differ by arithmetic only
    while both bx AND es still hold what they held, so the fast path is
    sound only when the segment registers agree too -- `cell.space ==
    access.addr.space` already implies both sides are Space.FAR whenever
    `segment` is not NONE, so this line is a no-op for every other space.
    """
    same_base = (
        cell.base != Register.NONE
        and cell.base == access.addr.base
        and cell.space == access.addr.space
        and cell.index == access.addr.index
        and cell.segment == access.addr.segment
    )
    if same_base:
        return cell.disp < access.addr.disp + access.width and access.addr.disp <= cell.disp
    return module.may_alias(cell, access.addr, dgroup, 1, access.width)


def _clobbered(insn: Insn) -> frozenset[Register_]:
    """The registers this instruction writes, rooted -- a cell indexed by one
    of them stops meaning what it meant."""
    return frozenset(
        ROOT.get(one.register, one.register) for one in INFO.info(insn.insn).used_registers() if one.access in WRITES
    )


SURVIVES_THE_FRAME = (Space.SEGMENT, Space.FAR)


def every_cell(blocks: list[Block], resolve: Resolver) -> frozenset[Addr]:
    """Every cell anything in these blocks names, of any space.

    What a barrier makes live. It may read anything, and a frame slot is
    something -- restricting it to statics silently drops every live frame
    cell, which reported a store to [bp-16h] in bench/nbody.bas's PITSNAP
    dead when the value is read 180 bytes later. Deleting it would have
    corrupted the timer reading, and nothing about the report said so.
    """
    found: set[Addr] = set()
    for block in blocks:
        for insn in block.insns:
            access = access_of(insn, resolve)
            if isinstance(access, Access):
                found.update(access.cells)
    return frozenset(found)


def statics_of(blocks: list[Block], resolve: Resolver) -> frozenset[Addr]:
    """Every SEGMENT or FAR cell anything in these blocks names.

    Backward liveness needs it as the conservative answer at a body's exit
    and at any barrier: a static may be read by another procedure, where a
    frame slot dies with the frame. A $DYNAMIC array element is the same
    case one level removed -- the array itself outlives the procedure that
    happens to be indexing it through es:bx, so a store through one is exactly
    as live past a return as a store to a named static, not as dead as a
    spilled temporary.
    """
    found: set[Addr] = set()
    for block in blocks:
        for insn in block.insns:
            access = access_of(insn, resolve)
            if isinstance(access, Access) and access.addr.space in SURVIVES_THE_FRAME:
                found.update(access.cells)
    return frozenset(found)


@dataclass(frozen=True, slots=True)
class Step:
    """One instruction, with everything the fixed point needs already read.

    The transfer functions run once per block per round until nothing
    changes, and re-deriving an access means an iced info call each time --
    measured at 4.4 calls per instruction on bench/nbody.bas. Reading them
    once is what the usual gen/kill formulation is really for; the sets
    themselves stay ordinary frozensets, because they are tiny (fourteen
    cells at the widest point in this corpus) and a bitmap over a numbered
    universe would cost more to maintain than it saves.
    """

    at: int
    barrier: bool  # a call this cannot see through, or an unnameable operand
    access: Access | None
    clobbers: frozenset[Register_]


def prepare(block: Block, resolve: Resolver, calls: dict[int, str]) -> tuple[Step, ...]:
    """Read every instruction's own effect once, for reuse across rounds."""
    steps = []
    for insn in block.insns:
        if insn.at in calls:
            steps.append(Step(insn.at, not survives(calls.get(insn.at)), None, frozenset()))
            continue
        if insn.flow in CLOBBERS:
            steps.append(Step(insn.at, True, None, frozenset()))
            continue
        access = access_of(insn, resolve)
        steps.append(
            Step(
                insn.at,
                access is not None and not isinstance(access, Access),
                access if isinstance(access, Access) else None,
                _clobbered(insn),
            )
        )
    return tuple(steps)


def _forward(
    steps: tuple[Step, ...],
    incoming: frozenset[Addr],
    dgroup: frozenset[int],
    found: list[int] | None = None,
) -> frozenset[Addr]:
    """Availability after this block; `found` collects redundant loads."""
    have = set(incoming)
    for step in steps:
        if step.barrier:
            have.clear()
            continue
        access = step.access
        if access is None:
            continue
        if wrote := step.clobbers:
            # a Space.FAR cell's segment register is dropped the same way its
            # base is -- `mov es,[si+2]` reloading es invalidates an es:bx
            # cell exactly as `mov bx,...` invalidates it, and es is never a
            # ROOT key so ROOT.get leaves it as itself, same as any other
            # register this pass does not root.
            have = {
                c
                for c in have
                if (c.base == Register.NONE or ROOT.get(c.base, c.base) not in wrote)
                and (c.segment == Register.NONE or c.segment not in wrote)
            }
        if access.reads and not access.writes and found is not None and all(c in have for c in access.cells):
            found.append(step.at)
        if access.writes:
            have = {c for c in have if not aliases(c, access, dgroup)}
        have.update(access.cells)
    return frozenset(have)


def _backward(
    steps: tuple[Step, ...],
    outgoing: frozenset[Addr],
    statics: frozenset[Addr],
    everything: frozenset[Addr],
    found: list[int] | None = None,
) -> frozenset[Addr]:
    """Liveness before this block; `found` collects dead stores.

    `statics` is what survives past this body -- another procedure may read
    a module-level DIM, where a frame slot dies with the frame. `everything`
    is what a barrier makes live, which is a wider set: a barrier may read
    any memory at all, frame slots included, so restricting it to statics
    drops live frame cells and calls a store to one dead.
    """
    have = set(outgoing)
    for step in reversed(steps):
        if step.barrier:
            have = set(everything)  # it may read anything at all
            continue
        # An instruction that writes a base or segment register separates
        # two different sets of cells: `es:[bx]` after it is not the address
        # `es:[bx]` named before it. Six stores through es:[bx] in arrprm are
        # six elements of one array, bx recomputed between each, and they
        # share an Addr because an Addr names the register rather than its
        # value.
        #
        # Conservatism runs the other way here than in _forward. There, a
        # cell whose base changed is dropped, because availability must not
        # claim a value it might not have. Here, absent from `have` means
        # "something overwrites it before anything reads it", so dropping a
        # cell asserts a store is dead -- exactly the wrong direction. Cells
        # through a clobbered register become live again instead.
        #
        # Getting this backwards deleted `arr(0) = 7`: the later store to
        # es:[bx] looked like an overwrite of the earlier one, and arrprm
        # printed 0 where it had stored 7 on nine of twelve configurations.
        if step.clobbers:
            have |= {
                one
                for one in everything
                if (one.base != Register.NONE and ROOT.get(one.base, one.base) in step.clobbers)
                or (one.segment != Register.NONE and one.segment in step.clobbers)
            }

        access = step.access
        if access is None:
            continue
        if access.writes and not access.reads:
            if found is not None and not any(c in have for c in access.cells):
                found.append(step.at)
            have -= set(access.cells)
        if access.reads:
            have.update(access.cells)
    return frozenset(have)


def survives(name: str | None) -> bool:
    """Whether a call leaves every tracked cell still valid -- runtime.py's
    own contract, so a routine with no entry is a barrier by construction."""
    routine = runtime.contract(name)
    if routine.control is not runtime.Control.RETURNS or runtime.barrier(routine):
        return False
    return routine.writes <= runtime.Memory.ARGUMENTS


def available(
    blocks: list[Block],
    resolve: Resolver,
    calls: dict[int, str],
    dgroup: frozenset[int],
) -> dict[int, frozenset[Addr]]:
    """What is available on entry to each block, to a fixed point.

    Intersection at a join, so this only ever claims what holds on every
    path. The universe -- every cell anything names -- is the top element
    those intersections start from; without one, a block reached only by a
    back edge would start at "nothing available" and never recover.
    """
    if not blocks:
        return {}
    entry = blocks[0].at
    preds = predecessors(blocks)
    universe = frozenset(
        cell
        for block in blocks
        for insn in block.insns
        if isinstance(access := access_of(insn, resolve), Access)
        for cell in access.cells
    )

    ready = {block.at: prepare(block, resolve, calls) for block in blocks}
    out = {block.at: universe for block in blocks}
    out[entry] = _forward(ready[entry], frozenset(), dgroup)
    incoming = {block.at: frozenset() for block in blocks}

    changing = True
    while changing:
        changing = False
        for block in blocks:
            if block.at == entry:
                continue
            reaching = [out[one] for one in preds[block.at] if one in out]
            now_in = frozenset.intersection(*reaching) if reaching else frozenset()
            now_out = _forward(ready[block.at], now_in, dgroup)
            if now_out != out[block.at] or now_in != incoming[block.at]:
                out[block.at], incoming[block.at] = now_out, now_in
                changing = True
    return incoming


def live(
    blocks: list[Block],
    resolve: Resolver,
    calls: dict[int, str],
    dgroup: frozenset[int],
) -> dict[int, frozenset[Addr]]:
    """What is still wanted on exit from each block, to a fixed point.

    Union at a branch, and every static is live where control leaves this
    body: another procedure may read a module-level DIM, where a frame slot
    dies with the frame. That asymmetry is why a spill is easier to prove
    dead than a named variable.
    """
    if not blocks:
        return {}
    statics = statics_of(blocks, resolve)
    everything = every_cell(blocks, resolve)
    ready = {block.at: prepare(block, resolve, calls) for block in blocks}
    known = {block.at for block in blocks}
    inside = {block.at: frozenset() for block in blocks}
    outgoing = {block.at: frozenset() for block in blocks}

    changing = True
    while changing:
        changing = False
        for block in blocks:
            out = statics if block.leaves else frozenset()
            for successor in block.succ:
                out |= inside[successor] if successor in known else statics
            now = _backward(ready[block.at], out, statics, everything)
            if now != inside[block.at] or out != outgoing[block.at]:
                inside[block.at], outgoing[block.at] = now, out
                changing = True
    return outgoing


def redundant_loads(
    blocks: list[Block],
    resolve: Resolver,
    calls: dict[int, str],
    dgroup: frozenset[int],
) -> dict[int, tuple[int, ...]]:
    """Per block, the loads whose value was already in hand."""
    incoming = available(blocks, resolve, calls, dgroup)
    found: dict[int, tuple[int, ...]] = {}
    for block in blocks:
        hits: list[int] = []
        _forward(prepare(block, resolve, calls), incoming.get(block.at, frozenset()), dgroup, hits)
        found[block.at] = tuple(hits)
    return found


def dead_stores(
    blocks: list[Block],
    resolve: Resolver,
    calls: dict[int, str],
    dgroup: frozenset[int],
) -> dict[int, tuple[int, ...]]:
    """Per block, the stores nothing reads before something overwrites them."""
    statics = statics_of(blocks, resolve)
    everything = every_cell(blocks, resolve)
    outgoing = live(blocks, resolve, calls, dgroup)
    found: dict[int, tuple[int, ...]] = {}
    for block in blocks:
        hits: list[int] = []
        _backward(prepare(block, resolve, calls), outgoing.get(block.at, statics), statics, everything, hits)
        found[block.at] = tuple(hits)
    return found
