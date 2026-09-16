"""Conservative memory SSA over MIR, rebuilt after a body changes.

Stores, calls and barriers define a single memory state; loads use that state.
The clobber walker skips stores proven disjoint by MIR alias analysis.
Raised call write effects participate in the same alias queries as stores.
Calls without write metadata and barriers remain conservative.
"""

from dataclasses import dataclass
from enum import StrEnum

from qbopt.analysis import effects, loops, pointerfacts
from qbopt.model import mir


def _unknown_write(op: mir.Op) -> bool:
    return effects.unmodeled_write(op) or op.floating is not None or op.kind is mir.Kind.FCHECK


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
    operations: dict[Site, mir.Op]
    pointers: pointerfacts.Offsets

    def at(self, site: Site) -> Access:
        return self.sites[site]

    def clobbers(
        self, site: Site, memory: mir.MemRef, dgroup: frozenset[int] = frozenset(),
    ) -> frozenset[int]:
        """Possible nearest writes before a site, including live-on-entry.

        Walk every phi input. A visited set closes cycles without treating
        the backedge as evidence that memory is unchanged. An empty result
        means no reachable source was found, not a reusable memory value.
        This identifies memory states, not a dominating scalar definition;
        forwarding consumers must establish value availability separately.
        """
        return self._frontier(site, memory, dgroup)

    def unchanged(
        self, earlier: Site, later: Site, memory: mir.MemRef,
        dgroup: frozenset[int] = frozenset(),
    ) -> bool:
        """Whether a dominating earlier read's memory state still applies.

        The caller must establish dominance and equal addresses. Stop at
        the earlier memory version, rejecting any possibly aliasing write
        on the way, including writes carried by loop backedges.
        """
        boundary = self.at(earlier).defining
        return boundary is not None and self._frontier(later, memory, dgroup, boundary) == frozenset({boundary})

    def _frontier(
        self, site: Site, memory: mir.MemRef, dgroup: frozenset[int], boundary: int | None = None,
        edge: int | None = None,
        edge_memory: mir.MemRef | None = None,
    ) -> frozenset[int]:
        accesses = {access.id: access for access in self.accesses}
        pending = [self.at(site).defining]
        seen: set[int] = set()
        found: set[int] = set()
        while pending:
            current = pending.pop()
            if current is None or current in seen:
                continue
            seen.add(current)
            if current == boundary:
                found.add(current)
                continue
            access = accesses[current]
            match access.kind:
                case Kind.LIVE:
                    found.add(current)
                case Kind.PHI:
                    pending.extend(value for parent, value in access.incoming
                                   if edge is None or access.block != site.block or parent == edge)
                case Kind.DEF:
                    op = self.operations[access.site]
                    queried = edge_memory if edge_memory is not None and access.block != site.block else memory
                    if _unknown_write(op) or any(
                        mir.overlapping(queried, store, dgroup) and not self.pointers.disjoint(queried, store)
                        for store in op.stores
                    ):
                        found.add(current)
                    else:
                        pending.append(access.defining)
                case Kind.USE:
                    pending.append(access.defining)
        return frozenset(found)

    def available_on_edge(
        self, earlier: Site, later: Site, predecessor: int, memory: mir.MemRef,
        dgroup: frozenset[int] = frozenset(),
        *, edge_memory: mir.MemRef | None = None,
    ) -> bool:
        """An earlier load/store still supplies these bytes on one incoming edge.

        The caller proves equal addresses and that the scalar provider dominates
        the predecessor. Only the destination's memory phi is edge-selected;
        other intervening joins still require agreement along every path.
        A translated address applies outside the destination; its prefix must
        still be checked against the original phi-based address.
        """
        source = self.at(earlier)
        boundary = source.id if source.kind is Kind.DEF else source.defining
        return boundary is not None and self._frontier(later, memory, dgroup, boundary, predecessor, edge_memory) == frozenset({boundary})


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
            defines = bool(op.stores or _unknown_write(op))
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
    operations = {
        Site(block.at, index): op
        for block in body.blocks for index, op in enumerate(block.ops)
        if Site(block.at, index) in sites
    }
    return MemorySSA(live, accesses, sites, phis, operations, pointerfacts.offsets(body))
