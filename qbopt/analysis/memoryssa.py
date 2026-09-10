"""Conservative memory SSA over MIR, rebuilt after a body changes.

Stores, calls and barriers define a single memory state; loads use that state.
No alias or call-purity assumptions are made here. A later clobber walker
may skip definitions only with independent alias/mod-ref evidence.
"""

from dataclasses import dataclass
from enum import StrEnum

from qbopt.analysis import loops
from qbopt.model import mir


class Kind(StrEnum):
    LIVE = "live-on-entry"
    USE = "use"
    DEF = "def"
    PHI = "phi"


@dataclass(frozen=True, slots=True)
class Site:
    block: int
    index: int


@dataclass(frozen=True, slots=True)
class Access:
    id: int
    kind: Kind
    block: int | None = None
    site: Site | None = None
    defining: int | None = None
    # None denotes the invocation edge into the entry block.
    incoming: tuple[tuple[int | None, int], ...] = ()


@dataclass(frozen=True, slots=True)
class MemorySSA:
    live: Access
    accesses: tuple[Access, ...]
    sites: dict[Site, Access]
    phis: dict[int, Access]

    def at(self, site: Site) -> Access:
        return self.sites[site]


def built(body: mir.MirBody) -> MemorySSA:
    """Wire block entries first, then eliminate identity memory phis.

Preallocating entries handles backedges without iterative guesses about
memory versions. Entry blocks with backedges retain an invocation input.
"""
    live = Access(0, Kind.LIVE)
    entries = {block.at: index + 1 for index, block in enumerate(body.blocks)}
    sites: dict[Site, Access] = {}
    outgoing: dict[int, int] = {}
    next_id = len(entries) + 1
    for block in body.blocks:
        current = entries[block.at]
        for index, op in enumerate(block.ops):
            defines = bool(op.stores or op.barrier or op.kind is mir.Kind.CALL)
            if not (op.loads or defines):
                continue
            kind = Kind.DEF if defines else Kind.USE
            site = Site(block.at, index)
            sites[site] = Access(next_id, kind, block.at, site, current)
            if kind is Kind.DEF:
                current = next_id
            next_id += 1
        outgoing[block.at] = current

    predecessors = loops.predecessors(body.blocks)
    incoming = {
        block.at: tuple((pred, outgoing[pred]) for pred in sorted(predecessors[block.at]))
        + (((None, live.id),) if block.at == body.entry or not predecessors[block.at] else ())
        for block in body.blocks
    }
    replacements: dict[int, int] = {}

    def resolved(value: int) -> int:
        while value in replacements:
            value = replacements[value]
        return value

    changed = True
    while changed:
        changed = False
        for block, entry in entries.items():
            if entry in replacements:
                continue
            values = {resolved(value) for _, value in incoming[block]} - {entry}
            if len(values) <= 1:
                replacements[entry] = next(iter(values), live.id)
                changed = True

    phis = {
        block: Access(entry, Kind.PHI, block, incoming=tuple(
            (pred, resolved(value)) for pred, value in incoming[block]
        ))
        for block, entry in entries.items()
        if entry not in replacements
    }
    sites = {
        site: Access(access.id, access.kind, access.block, site, resolved(access.defining))
        for site, access in sites.items()
    }
    accesses = tuple(sorted((live, *phis.values(), *sites.values()), key=lambda access: access.id))
    return MemorySSA(live, accesses, sites, phis)
