"""
Whose code a byte is: the module's own main body, or one of its SUB/FUNCTIONs.

blocks.py answers "is this byte code, and where does control go"; this answers
"which body owns it" -- built entirely on that machinery (`code_map()`,
`partition()`, `Block`), not on CodeView. CodeView is absent from all 110
objects in this corpus (see docs/machine/codeview.md) and is not needed here: every
code-segment PUBDEF is a SUB/FUNCTION's own entry point, measured across
every procedure-bearing fixture, and `blocks.walk()` already seeds
reachability off `module.publics` for exactly this reason. That signal
survives even where the frame-setup idiom does not: PDS 7.1's `/Ot` emits a
plain `push bp`/`pop bp` prologue and epilogue with no call into the runtime
at all (measured on fixtures/omf/procs-p-ot.obj), and PUBDEF is the only
thing both shapes still agree on.

The runtime routines a normal (non-/Ot) prologue/epilogue calls -- B$ENRA
paired with `mov cx,<framesize>` before the far call, B$EXSA before the
closing `retf n` -- were identified the same way blocks.py identifies
B$OGTA: by the EXTDEF name a fixup gives the call, not by opcode shape. Their
own implementations are not in the local QuickBASIC 4.5 runtime source tree
(~/work/ms/msdos_60/45) any more than B$OGTA's is -- only their names and the
frame layout they establish, in runtime/inc/stack.inc ("The basic frame is
set up on entry to the main program, by B$ENSA/B$ENRA SUB and FUNCTION entry
routines"), runtime/inc/rtmint.inc and ulib.inc (both list B$ENRA/B$ENSA/
B$ENRD/B$ENSD/B$ENFA as one entry family and B$EXSA/B$EXFA as the matching
exit family), and runtime/rt/error.asm (calls B$EXSA, "clear frame state
info"). Neither name is used as the detection mechanism below, precisely
because /Ot doesn't emit them; they are cited here as the measured grounding
for what the idiom agents.md describes actually is.

A module's own entry point (blocks.ENTRY) seeds the main body the same way.
A module built under /V or /W carries a third, disconnected body -- the
event-poll stub blocks.event_stub() already finds -- which nothing in the
main body's own control flow falls into (that is the whole reason it needs
its own seed, per blocks.py's own comment on event_stub()).

Reachability, not address order, decides ownership. A procedure is not
"bytes between two PUBDEFs": BC lays SUB/FUNCTION bodies out exactly where
they were declared in the source, and the main body's own control flow jumps
around each one to keep running -- so the main body is generally several
disjoint byte ranges, not one contiguous prefix ending at the first
procedure. Measured on fixtures/omf/procs-v-g3.obj (one FUNCTION, one SUB):
the main body is the three ranges 0x30-0xea, 0x11b-0x11e (a 3-byte trampoline
BC placed *between* the two procedure bodies to skip the second one) and
0x14c-0x153 (the implicit END statement's own call, sitting *after* both
procedures); cvinfo.Procedure.offset/proc_length from a /Zi compile of the
same source confirms this partition exactly, procedure body for procedure
body, on all three compilers.

A block that ends at an inline `ON GOTO` table (blocks.INLINE_TABLE) is not
handled by trusting Block.succ here: `_close()`'s own succ for a TABLE block
is deliberately every fixup target in the module, a safe over-approximation
for liveness that would wrongly let one body's reachability walk swallow
another's. This module recomputes the table's real entries the way
blocks.inline_table() does. A table `_table_at` matches for some other
reason -- the /X RESUME map unexplained_tables() finds, which can start
right where an ordinary call falls (measured: divmod-v-g3.obj's own
`call far B$CENP` ends exactly where that map begins) -- is not a jump table
at all, and contributes no successor.
"""

from enum import StrEnum
from dataclasses import dataclass

from qbopt.objectfile import omf
from qbopt.frontend.blocks import Ends
from qbopt.frontend.blocks import Block
from qbopt.frontend.blocks import ENTRY
from qbopt.frontend.blocks import CodeMap
from qbopt.frontend.blocks import code_map
from qbopt.objectfile.module import Module
from qbopt.frontend.blocks import event_stub
from qbopt.frontend.blocks import has_header
from qbopt.frontend.blocks import INLINE_TABLE
from qbopt.frontend.blocks import local_call_target
from qbopt.frontend.blocks import partition as block_partition

EVENT_STUB_NAME = "event poll stub"


class BodyKind(StrEnum):
    MAIN = "main"
    PROCEDURE = "procedure"
    EVENT_STUB = "event-stub"
    EVENT_HANDLER = "event-handler"
    ERROR_HANDLER = "error-handler"
    RESUME_ENTRY = "resume-entry"


@dataclass(frozen=True, slots=True)
class Body:
    kind: BodyKind
    seed: int
    name: str | None
    ranges: tuple[tuple[int, int], ...]

    @property
    def length(self) -> int:
        return sum(hi - lo for lo, hi in self.ranges)


@dataclass(frozen=True, slots=True)
class Partition:
    bodies: tuple[Body, ...]
    # gaps code_map() itself already proved benign -- nop padding, or dead
    # code BC emitted and never enters. Not counted against completeness.
    benign: tuple[tuple[int, int], ...]
    # real, reached code that no body's own reachability claims
    unexplained: tuple[tuple[int, int], ...]
    # claimed by more than one body -- a sign the ownership graph is wrong
    conflicts: tuple[tuple[int, int], ...]

    @property
    def complete(self) -> bool:
        return not self.unexplained and not self.conflicts


def _table_targets(module: Module, table: tuple[int, int], call_name: str | None) -> tuple[int, ...]:
    """A table block's real successors, not Block.succ's own safe over-approximation.

    Only an INLINE_TABLE call (B$OGTA) is a real jump table with entries worth
    following; anything else `_table_at` matches -- the /X RESUME map, which
    can start right where an unrelated call happens to end (measured:
    divmod-v-g3.obj's own `call far B$CENP` ends exactly where that map
    begins) -- is data nothing here jumps into, so only the byte past it
    (the same "falls through past the table" fallthrough blocks.py's own
    inline_table() establishes) is a real successor, never module.targets.
    """
    lo, hi = table
    if call_name not in INLINE_TABLE:
        return (hi,)
    entries = sorted(at for at in module.operands if lo < at < hi)
    return (*(module.operands[at].disp for at in entries), hi)


def _table_at(mapped: CodeMap, end: int) -> tuple[int, int] | None:
    return next((table for table in mapped.tables if table[0] == end), None)


def _successors(module: Module, mapped: CodeMap, block: Block) -> tuple[int, ...]:
    if block.ends is not Ends.TABLE:
        return block.succ
    table = _table_at(mapped, block.end)
    if table is None:
        return ()
    return _table_targets(module, table, module.calls.get(block.insns[-1].at))


def _reachable(
    seed: int, others: frozenset[int], blocks_by_at: dict[int, Block], module: Module, mapped: CodeMap
) -> frozenset[int]:
    """Every block this seed's own control flow reaches, refusing to cross into another body's."""
    visited: set[int] = set()
    frontier = [seed]
    while frontier:
        at = frontier.pop()
        if at in visited or at not in blocks_by_at:
            continue
        visited.add(at)
        for target in _successors(module, mapped, blocks_by_at[at]):
            if target not in visited and target not in others:
                frontier.append(target)
    return frozenset(visited)


def _merge(spans: list[tuple[int, int]]) -> tuple[tuple[int, int], ...]:
    out: list[tuple[int, int]] = []
    for lo, hi in sorted(spans):
        if out and out[-1][1] == lo:
            out[-1] = (out[-1][0], hi)
        else:
            out.append((lo, hi))
    return tuple(out)


def _ranges(mapped: CodeMap, blocks_by_at: dict[int, Block], owned: frozenset[int]) -> tuple[tuple[int, int], ...]:
    spans = [(blocks_by_at[at].at, blocks_by_at[at].end) for at in owned]
    for lo, hi in mapped.tables:
        owner = next((b for b in blocks_by_at.values() if b.end == lo), None)
        if owner is not None and owner.at in owned:
            spans.append((lo, hi))
    return _merge(spans)


def partition(module: Module) -> Partition | str:
    """The module's code, cut into its main body, its procedures, and whatever neither accounts for."""
    basic = has_header(module)
    native = not basic and module.name.endswith("_TEXT") and omf.code_segment(module.records) is not None
    if not basic and not native:
        return "no module header: which offset is the entry is not known without one"
    mapped = code_map(module)
    if isinstance(mapped, str):
        return mapped

    all_blocks = block_partition(module, mapped)
    from qbopt.abi import runtime
    from qbopt.frontend import raising_control

    all_blocks = raising_control.terminal_edges(all_blocks, runtime.for_module(module))
    blocks_by_at = {blk.at: blk for blk in all_blocks}
    names = omf.pubdef_names(module.records, module.seg)

    seeds: list[tuple[BodyKind, int, str | None]] = [(BodyKind.MAIN, ENTRY, None)] if basic else []
    if (stub := event_stub(module)) is not None:
        seeds.append((BodyKind.EVENT_STUB, stub, EVENT_STUB_NAME))
    seeds += [(BodyKind.PROCEDURE, at, names.get(at)) for at in sorted(module.publics)]
    if native:
        private = {
            target
            for block in all_blocks
            for insn in block.insns
            if (target := local_call_target(module, insn)) is not None
        } | set(mapped.procedures)
        private -= module.publics
        seeds += [(BodyKind.PROCEDURE, at, None) for at in sorted(private)]
    from qbopt.abi.events import handler_entries

    seeds += [(BodyKind.EVENT_HANDLER, at, "timer handler") for at in sorted(handler_entries(module) - module.publics)]
    from qbopt.abi.handlers import error_entries

    handlers = error_entries(module)
    occupied = {seed for _, seed, _ in seeds}
    seeds += [(BodyKind.ERROR_HANDLER, at, "error handler") for at in sorted(handlers - occupied)]

    seed_offsets = frozenset(seed for _, seed, _ in seeds)
    reached = {seed: _reachable(seed, seed_offsets - {seed}, blocks_by_at, module, mapped) for _, seed, _ in seeds}
    if handlers:
        owned = frozenset(at for group in reached.values() for at in group)
        resumable = (frozenset(module.targets) & blocks_by_at.keys()) - owned
        for seed in sorted(resumable):
            seeds.append((BodyKind.RESUME_ENTRY, seed, "resume entry"))
            reached[seed] = _reachable(seed, owned | (resumable - {seed}), blocks_by_at, module, mapped)

    owners: dict[int, int] = {}
    for _, seed, _ in seeds:
        for at in reached[seed]:
            owners[at] = owners.get(at, 0) + 1

    bodies = tuple(Body(kind, seed, name, _ranges(mapped, blocks_by_at, reached[seed])) for kind, seed, name in seeds)
    conflicts = _merge([(blk.at, blk.end) for at, blk in blocks_by_at.items() if owners.get(at, 0) > 1])
    unexplained = _merge([(blk.at, blk.end) for at, blk in blocks_by_at.items() if owners.get(at, 0) == 0])
    benign = tuple(gap for gap in mapped.unreached if gap[0] >= (ENTRY if basic else module.start))

    return Partition(bodies, benign, unexplained, conflicts)
