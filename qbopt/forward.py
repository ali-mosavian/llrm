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

from qbopt import ir
from qbopt import memory
from qbopt import module
from qbopt.declen import INFO
from qbopt.declen import Insn
from qbopt.module import Addr
from qbopt.blocks import Block
from qbopt.declen import WRITES
from qbopt.lift import Resolver


def _lands_in(insn: Insn) -> Register_ | None:
    """The tracked register this access reads into or writes from, rooted.

    One register, or nothing: an instruction naming two of them is not a
    plain load and is not what this deletes.
    """
    from qbopt.mir import TRACKED

    found = {
        ir.ROOT.get(one.register, one.register)
        for one in INFO.info(insn.insn).used_registers()
        if ir.ROOT.get(one.register, one.register) in TRACKED
    }
    return next(iter(found)) if len(found) == 1 else None


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
