"""
The loads that need not happen, and the ones that cannot go yet.

memory.py finds a load whose value is already in hand. Acting on one is a
different question from finding it, and the difference is the whole reason
this module is small: replacing a load with the value it would have read
generally means that value is in a *different* register from the one the
load wrote, and nothing can emit that until something allocates registers.

Measured across fixtures/omf and bench/nbody.bas, of 508 redundant loads:
384 are an exact (address, width) match rather than needing several narrower
values composed, and of those 30 land in the register that already holds
the bytes. Those 30 are pure redundancy -- the instruction can simply go, no
register changes, and the bytes around it are untouched. The rest are the
register allocator's, and until it exists they are a measurement rather than
an optimisation.

That 30 was 66 before the register-kill rule below, and every one of the 36
it removed was a deletion that would have corrupted the program. The count
is small enough that the difference is most of it, which is the argument for
checking a transform against the actual instruction stream rather than
against how obviously right it reads.

So this deletes only what needs no allocation. That is not where the value
is, and it is not pretending to be: it is the first thing that changes a
byte through the MIR, which is worth doing on the smallest safe case before
an allocator is built on top of a path nothing has proven end to end.

The conditions for dropping one, all of them necessary:

  - the same address and the same width, so the bytes really are the bytes.
    A 16-bit load off a 32-bit value is not the same instruction with fewer
    steps -- `mov ax,[x]` writes half of eax and leaves the rest, which is
    only harmless when what already holds it got there the same way.
  - the same register, rooted, so nothing has to move.
  - nothing between them that could have written those bytes, which is
    memory.py's own answer and not re-derived here.
"""

from iced_x86 import Register_
from iced_x86 import RegisterExt
from iced_x86 import MemorySizeExt

from qbopt import ir
from qbopt import memory
from qbopt import module
from qbopt.declen import INFO
from qbopt.declen import Insn
from qbopt.module import Addr
from qbopt.blocks import Block
from qbopt.declen import READS
from qbopt.declen import WRITES
from qbopt.lift import Resolver


def _whole_register(insn: Insn) -> bool:
    """Whether this instruction's register write is no wider than its read.

    `mov ax,[x]` writes exactly the two bytes it read. `movsx eax,word [x]`
    reads the same two and writes four, setting the top half from the sign --
    so the two are not interchangeable even though the map that says "this
    register holds the bytes at this address" agrees about both.

    That map is what removable() and the forwarding rules are built on, and
    without this it deleted a widening load whose narrow provider had never
    written the high half. tools/fuzzcheck.py found it on VBDOS /G3, seed
    4200, program F028:

        mov ds:[0],ax              func38%'s INTEGER result, two bytes
        movsx eax,word ptr ds:[0]  widened for a LONG expression

    Deleting the second printed 173682056 where 173747592 was wanted -- the
    function returned 0, so the entire error is one stale high half.

    Only reachable on a second pass, because the movsx is this pass's own
    work: BC writes `mov ax,[x]` followed by `cwd`, and the widening turns
    that pair into one instruction.
    """
    memory = MemorySizeExt.size(insn.insn.memory_size)
    if not memory:
        return True  # touches no memory; the width question does not arise
    return not any(
        RegisterExt.size(one.register) > memory
        for one in INFO.info(insn.insn).used_registers()
        if one.access in WRITES and one.register in ir.ROOT
    )


def _loads_only(insn: Insn) -> bool:
    """Whether this instruction merely reads memory into its register.

    `mov cx,[x]` does. `and cx,[x]` does not, and iced says so plainly: it
    reports cx as READ_WRITE there and WRITE in the mov, because the and
    combines the loaded bytes with what the register already held.

    removable() needs this and did not have it. The hole never fired --
    all 30 of its hits are movs with or without the check -- because it
    only deletes a load whose register ALREADY holds the loaded bytes, and
    `cx = cx and cx` is cx. `cx = cx - cx` is not, so the check is what
    makes that luck into a rule.

    A sibling pass forwarding a load into a register move rather than
    deleting it found 24 sites without this check and none with it: every
    one was `and reg,[x]`, where the forward was right and dropping the
    and was not. Wired up, it computed 0f0f0f0f where arith wants
    1f3f5f7f on nine of the twelve real-compiler configurations -- caught
    by tools/matrix.py with the whole host suite green. There is nothing
    left for it to do, so it is not here: BC keeps values in memory, and a
    reload whose bytes sit in a *different* register than the reload names
    is a shape it does not emit.
    """
    reads = {one.register for one in INFO.info(insn.insn).used_registers() if one.access in READS}
    return not any(one.register in reads for one in INFO.info(insn.insn).used_registers() if one.access in WRITES)


def _lands_in(insn: Insn) -> Register_ | None:
    """The tracked register this access reads into or writes from, rooted.

    One register, or nothing: an instruction naming two of them is not a
    plain load and is not what this deletes.

    A register that only says WHERE the bytes are is not one of them.
    `fld dword ptr [si]` names si and nothing else, and si is no more the
    destination there than the address on an envelope is its contents --
    the value goes on the x87 stack, which this does not track. Reading si
    as the destination made two consecutive `fld [si]` look like a load
    and a redundant reload, and they are two pushes: deleting the second
    left one value where the program wanted two and slid every x87 slot
    after it by one. bench/fpbench.bas printed -2147483648 for every
    coordinate before this line was here.
    """
    from qbopt.mir import TRACKED

    if not _loads_only(insn) or not _whole_register(insn):
        return None
    addressing = {_root_of(insn.memory_base), _root_of(insn.memory_index)}
    found = {
        ir.ROOT.get(one.register, one.register)
        for one in INFO.info(insn.insn).used_registers()
        if ir.ROOT.get(one.register, one.register) in TRACKED
    } - addressing
    return next(iter(found)) if len(found) == 1 else None


def _root_of(register: Register_ | None) -> Register_ | None:
    return None if register is None else ir.ROOT.get(register, register)


def removable(
    blocks: list[Block],
    resolve: Resolver,
    calls: dict[int, str],
    dgroup: frozenset[int],
) -> frozenset[int]:
    """Loads that can be dropped with nothing else changing.

    Only within a block. A value crossing a block boundary is available on
    every path or it is not, which memory.py answers, but the register
    holding it there is a question about the join, and a phi is exactly the
    thing that says the answer differs by path -- so a cross-block drop is
    an allocator's to make, not this one's.
    """
    reported = memory.redundant_loads(blocks, resolve, calls, dgroup)
    found: set[int] = set()

    for block in blocks:
        hits = set(reported.get(block.at, ()))
        held: dict[tuple[Addr, int], Register_ | None] = {}
        for insn in block.insns:
            if insn.at in calls:
                # Every call, including one runtime.py proves memory-clean.
                # This map is "which register holds these bytes", and even
                # B$MUI4 -- which touches no caller memory at all -- returns
                # with ax, cx, dx and bx changed. What survives a call is the
                # memory, not the register that was holding a copy of it.
                held.clear()
                continue
            # Anything that writes a register stops it holding what it held,
            # whether or not it touches memory. Leaving this out is a deletion
            # that corrupts: at arith-q-O 0x136 a reload of [x] was called
            # redundant because ax had loaded it at 0x10d, with a plain
            # `mov ax,cx` at 0x12d in between.
            #
            # Applied after this instruction's own match, not before. A load
            # writes the very register the match is about, so killing first
            # would clear the entry being tested and no load could ever
            # qualify -- the question is what the register held on the way in.
            written = {
                ir.ROOT.get(one.register, one.register)
                for one in INFO.info(insn.insn).used_registers()
                if one.access in WRITES
            }

            access = memory.access_of(insn, resolve)
            if not isinstance(access, memory.Access):
                if access is not None:
                    held.clear()  # memory nothing can name
                elif written:
                    held = {one: who for one, who in held.items() if who not in written}
                continue
            where = _lands_in(insn)
            key = (access.addr, access.width)
            if insn.at in hits and held.get(key) is not None and held[key] == where:
                found.add(insn.at)
            if written:
                held = {one: who for one, who in held.items() if who not in written}
            if access.writes:
                held = {
                    one: who
                    for one, who in held.items()
                    if not module.may_alias(one[0], access.addr, dgroup, one[1], access.width)
                }
            held[key] = where
    return frozenset(found)
