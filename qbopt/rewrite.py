"""
The driver: an .OBJ in, an .OBJ out.

--dry-run decides everything and then writes the input's bytes back. That is
what makes the harness testable before the pass is -- a failure downstream of a
dry run is the harness, not the rewriter. --take is what makes a real failure
bisectable: without it a differential says "something in 54 regions", which is
barely better than "different output".
"""

import sys
import json
import hashlib
import argparse
from pathlib import Path
from dataclasses import asdict
from dataclasses import replace
from dataclasses import dataclass

from iced_x86 import Register

from qbopt import omf
from qbopt import module
from qbopt.lift import Op
from qbopt.flags import ALL
from qbopt.lift import lift
from qbopt.lift import tail
from qbopt.flags import Flag
from qbopt.lift import FIXUP
from qbopt.lift import Value
from qbopt.calls import sites
from qbopt.lift import needed
from qbopt.lift import refuse
from qbopt.blocks import Block
from qbopt.calls import absorb
from qbopt.lift import regions
from qbopt.calls import COMPARE
from qbopt.flags import live_in
from qbopt.relocate import Edit
from qbopt.blocks import CodeMap
from qbopt.calls import CallSite
from qbopt.relocate import Shift
from qbopt.blocks import block_at
from qbopt.blocks import code_map
from qbopt.blocks import partition
from qbopt.flags import live_after
from qbopt.lift import emit_region
from qbopt import registers as regs
from qbopt.relocate import relocate
from qbopt.calls import FIX_MULTIPLY
from qbopt.blocks import instructions
from qbopt.relocate import crossed_pair


@dataclass(frozen=True, slots=True)
class Planned:
    region: "Region"
    edit: Edit | None


@dataclass(frozen=True, slots=True)
class Region:
    id: int
    seg: int
    at: int
    end: int
    before: str
    after: str | None
    taken: bool
    reason: str | None


def anchored_inside(found: module.Module, mapped: CodeMap, at: int, end: int) -> str | None:
    """Whether anything names an offset in the middle of this region.

    A leader can land inside a single value's two instructions -- a jcc to the
    adc half of an add/adc pair is legal, and BC's IF chains do land mid
    statement -- so splitting on leaders is not enough. Line numbers and public
    symbols name offsets too, and a region that swallows one has nowhere to put
    it back.
    """
    named = {
        "something branches to": mapped.leaders,
        "a public symbol is at": found.publics,
        "a line number points at": found.lines,
    }
    for why, offsets in named.items():
        inside = sorted(offset for offset in offsets if at < offset < end)
        if inside:
            return f"{why} {inside[0]:#x}, inside the region"
    if any(lo <= at and end <= hi for lo, hi in found.chunks):
        return None
    # crossing is safe when the region spans exactly two chunks that are
    # adjacent in file order and byte position -- relocate() moves their
    # shared boundary to the edit's own edge rather than merging them, which
    # keeps every fixup with the same LEDATA it always followed. BC's own
    # backpatch records are a later chunk at an earlier offset, never
    # adjacent this way, and that shape is refused rather than guessed at.
    if crossed_pair(found.chunks, at, end) is None:
        return "the region crosses a LEDATA boundary"
    return None


def flags_after(blocks: list[Block], live: dict[int, Flag], at: int, end: int) -> Flag:
    """The flags something reads after the region, or all of them if unknown."""
    block = block_at(blocks, at)
    return live_after(block, end, live) if block is not None else ALL


def dead_pairs_after(blocks: list[Block], live: regs.Liveness, at: int, end: int) -> frozenset[int]:
    """The register pairs whose own dx/bx half is proven dead after the region --
    docs/residue.md's I: nothing lift.emit_region() should bother restoring."""
    block = block_at(blocks, at)
    if block is None:
        return frozenset()  # unknown -- conservative: prove nothing dead
    dead = set()
    if not regs.live_after(block, end, Register.DX, live.dx):
        dead.add(0)
    if not regs.live_after(block, end, Register.BX, live.bx):
        dead.add(1)
    return frozenset(dead)


@dataclass(frozen=True, slots=True)
class Combined:
    planned: list[Planned]
    handled: frozenset[int]  # call-instruction addresses folded into one of `planned`'s edits


def tail_widened_calls(
    found: module.Module,
    mapped: CodeMap,
    blocks: list[Block],
    live: dict[int, Flag],
    reg_live: regs.Liveness,
    call_sites: list[CallSite],
    already: list[Planned],
) -> Combined:
    """docs/residue.md's G and H: BC's own code right after an absorbed call
    reads eax:edx directly, the way it would after a real call -- so a call
    site this pass already knows how to absorb, and whose bytes just after it
    still widen (lift.tail(), seeded with the call's own result already
    correctly valued in pair 0, per ir.RESTORE_EFFECTS[0]), becomes ONE
    combined edit: the call's own absorption (calls.absorb(..., restore=False)
    -- no point putting the high half back only to immediately re-derive it
    from eax) followed by the widened tail, through the exact same
    needed()/refuse()/emit_region() machinery an ordinary widening region
    already goes through. COMPARE is never a candidate here -- its own
    result is flags (B$CPI4's real effect), not a register value.

    A call's own SYNTHESISED/ALL flags gate (inside absorb()) and lift's
    DIVERGENT gate (inside refuse(), over the tail's own ALU/NEG values) are
    BOTH given the SAME, wider `live` -- flags something reads after the
    *whole* combined span, not just after the call -- which is why Op.CALL is
    deliberately left out of lift.computes()'s own divergence check: the
    call's flag safety is entirely absorb()'s gate, checked against this same
    live set, not a second, narrower one. In practice absorb()'s own gate is
    `live & ALL` for every name that reaches here (COMPARE is excluded above,
    and it is the only one of the five whose gate is narrower than ALL), so
    it already subsumes refuse()'s `live & DIVERGENT` outright -- the
    DIVERGENT branch never fires on this path today. It is kept, not
    inlined away, because that subsumption is a fact about today's five
    routines, not a guarantee a sixth one would keep.

    Anything that does not work out here changes nothing: the site is simply
    left for the ordinary, unmodified per-call absorb() loop right after this
    one, at its own narrow site.start/site.end boundary, exactly as before
    this function existed.
    """
    out: list[Planned] = []
    handled: set[int] = set()
    for site in call_sites:
        if site.name == COMPARE:
            continue
        call_block = block_at(blocks, site.at)
        if call_block is None or call_block.at > site.start:
            continue
        call_index = next((i for i, insn in enumerate(call_block.insns) if insn.at == site.at), None)
        if call_index is None:
            continue
        following = list(call_block.insns[call_index + 1 :])
        if not following:
            continue

        seed = Value(Op.CALL, at=site.start, end=site.end, pair=0)
        values = tail(following, found.code, call_block.end, found.resolve, seed)
        if len(values) == 1:
            continue  # nothing right after the call widens -- no benefit to composing

        chain_end = values[-1].end
        after = flags_after(blocks, live, site.start, chain_end)
        emitted = absorb(site, after, restore=False)
        if isinstance(emitted, str):
            continue
        values[0].absorbed = emitted

        need = needed(values)
        need[0] = True  # the call's own bytes replace the deleted pushes/call outright, never optional
        region = list(range(len(values)))
        reason = anchored_inside(found, mapped, site.start, chain_end) or refuse(values, need, region, after)
        dead = dead_pairs_after(blocks, reg_live, site.start, chain_end)
        combined = None if reason else emit_region(values, need, region, after, dead)
        if combined is None:
            continue
        if any(one.edit and one.edit.lo < chain_end and site.start < one.edit.hi for one in already + out):
            continue
        fixups = tuple((at_in, found.fixup_at[field]) for at_in, field in combined.relocations)
        if len(fixups) != len(combined.relocations):
            continue

        handled.add(site.at)
        out.append(
            Planned(
                Region(
                    id=0,  # rewritten to len(planned) by plan() once it is appended
                    seg=found.seg,
                    at=site.start,
                    end=chain_end,
                    before=found.code[site.start : chain_end].hex(),
                    after=combined.code.hex(),
                    taken=True,
                    reason=None,
                ),
                Edit(site.start, chain_end, combined.code, fixups),
            )
        )
    return Combined(out, frozenset(handled))


def plan(
    records: list[omf.Record],
    *,
    take: set[int] | None = None,
    max_regions: int | None = None,
) -> list[Planned]:
    found = module.of(records)
    if found is None:
        return []

    mapped = code_map(found)
    if isinstance(mapped, str):
        return []
    blocks = partition(found, mapped)
    live = live_in(blocks)
    reg_live = regs.analyse(blocks)

    reached = instructions(found)
    assert not isinstance(reached, str)
    values, _stores, bridges = lift(found.code, found.start, found.end, found.resolve, reached)
    need = needed(values, bridges=bridges)

    planned = []
    for index, region in enumerate(regions(values, bridges)):
        at, end = values[region[0]].at, values[region[-1]].end
        after = flags_after(blocks, live, at, end)
        reason = anchored_inside(found, mapped, at, end) or refuse(values, need, region, after)
        if take is not None and index not in take:
            reason = "not selected"
        elif max_regions is not None and sum(1 for one in planned if one.region.taken) >= max_regions:
            reason = "past --max-regions"
        dead = dead_pairs_after(blocks, reg_live, at, end)
        emitted = None if reason else emit_region(values, need, region, after, dead, found.code, bridges)
        if emitted is None and reason is None:
            reason = "nothing survived the region"
        if emitted is not None and len(emitted.code) > end - at:
            # The prize is the store and reload between operations, not the
            # width of any one of them. A region of a single pair widens to more
            # instructions than BC wrote, because putting the high half back
            # costs more than the widening saves.
            reason, emitted = f"widening it grows {end - at} bytes to {len(emitted.code)}", None
        edit = None
        if emitted is not None:
            fixups = tuple((at_in, found.fixup_at[field]) for at_in, field in emitted.relocations)
            if len(fixups) != len(emitted.relocations):
                reason, emitted = "an operand has no fixup to reuse", None
            else:
                edit = Edit(at, end, emitted.code, fixups)
        planned.append(
            Planned(
                Region(
                    id=len(planned),
                    seg=found.seg,
                    at=at,
                    end=end,
                    before=found.code[at:end].hex(),
                    after=emitted.code.hex() if emitted else None,
                    taken=emitted is not None,
                    reason=reason,
                ),
                edit,
            )
        )
    call_sites = sites(found, reached, blocks)
    # --take exists to make a failure bisectable; folding a call and its tail
    # into one edit here would make that edit un-selectable by the widening
    # loop's own region index, so bisecting skips this step entirely and
    # falls back to plain call absorption for every site instead.
    combined = (
        Combined([], frozenset())
        if take is not None
        else tail_widened_calls(found, mapped, blocks, live, reg_live, call_sites, planned)
    )
    for one in combined.planned:
        planned.append(Planned(replace(one.region, id=len(planned)), one.edit))

    for site in call_sites:
        if site.at in combined.handled:
            continue
        after = flags_after(blocks, live, site.start, site.end)
        emitted = absorb(site, after)
        reason = anchored_inside(found, mapped, site.start, site.end)
        if reason is None and isinstance(emitted, str):
            reason = emitted
        if reason is None and any(one.edit and one.edit.lo < site.end and site.start < one.edit.hi for one in planned):
            reason = "it overlaps a region already taken"
        # A call site may grow. Absorbing removes a far call and the routine
        # behind it, so the win is cycles rather than bytes -- a divide is
        # eighteen bytes and a remainder twenty-one, against fifteen under /G3.
        edit = None
        if reason is None and not isinstance(emitted, str):
            edit = Edit(
                site.start,
                site.end,
                emitted.code,
                tuple((at, found.fixup_at[field]) for at, field in emitted.relocations),
            )
        widened = None if edit is None else edit.data.hex()
        planned.append(
            Planned(
                Region(
                    id=len(planned),
                    seg=found.seg,
                    at=site.start,
                    end=site.end,
                    before=found.code[site.start : site.end].hex(),
                    after=widened,
                    taken=reason is None,
                    reason=reason,
                ),
                edit,
            )
        )
    planned = drop_restore_repush_round_trips(found, mapped, planned)
    return drop_chained_crossings(planned, found.chunks)


# docs/residue.md's B: the literal 2 bytes BC's own code puts right after a
# restore, immediately before a call this pass consumes -- push <hi>, push
# <lo>, per pair. push eax/pop ax/pop dx (lift.FIXUP -- calls.py's own
# restoring() is byte-identical) leaves eax completely intact, so push
# dx/push ax/pop eax right after it reconstructs exactly what eax already
# held.
PUSH_HI_LO = {0: bytes([0x52, 0x50]), 1: bytes([0x53, 0x51])}  # push dx,ax / push bx,cx
POP_ROOT = {0: bytes([0x66, 0x58]), 1: bytes([0x66, 0x59])}  # pop eax / pop ecx


def _restore_tail_pair(data: bytes) -> int | None:
    return next((pair for pair, pattern in FIXUP.items() if data.endswith(pattern)), None)


def drop_restore_repush_round_trips(found: module.Module, mapped: CodeMap, planned: list[Planned]) -> list[Planned]:
    """Fold a restore, BC's own untouched push of the same pair's halves, and
    the very next absorbed call's own leading pop of that pair's root into
    one edit with all three removed.

    Adjacency only, no dataflow: nothing between the restore and the pop
    touches ax/dx or cx/bx (checked byte-for-byte, not inferred), so the
    round trip is a pure no-op regardless of what the popped value is then
    used for -- calls.py's own codegen has no view past its own call site,
    and lift.emit_region() has no view of what comes after its own region,
    which is why this runs here instead, over the edits both already decided.
    """
    by_lo = {one.edit.lo: one for one in planned if one.edit is not None}
    drop: set[int] = set()
    extra: list[Planned] = []
    for a in planned:
        if a.edit is None or a.region.id in drop:
            continue
        pair = _restore_tail_pair(a.edit.data)
        if pair is None:
            continue
        gap_lo = a.edit.hi
        if found.code[gap_lo : gap_lo + 2] != PUSH_HI_LO[pair]:
            continue
        b = by_lo.get(gap_lo + 2)
        if b is None or b.edit is None or b.region.id in drop or not b.edit.data.startswith(POP_ROOT[pair]):
            continue
        if anchored_inside(found, mapped, a.edit.lo, b.edit.hi) is not None:
            continue

        prefix_len = len(a.edit.data) - len(FIXUP[pair])
        pop_len = len(POP_ROOT[pair])
        combined_data = a.edit.data[:prefix_len] + b.edit.data[pop_len:]
        fixups = a.edit.fixups + tuple((prefix_len + at - pop_len, field) for at, field in b.edit.fixups)

        drop.add(a.region.id)
        drop.add(b.region.id)
        extra.append(
            Planned(
                Region(
                    id=0,  # rewritten to len(kept) once appended, below
                    seg=found.seg,
                    at=a.edit.lo,
                    end=b.edit.hi,
                    before=found.code[a.edit.lo : b.edit.hi].hex(),
                    after=combined_data.hex(),
                    taken=True,
                    reason=None,
                ),
                Edit(a.edit.lo, b.edit.hi, combined_data, fixups),
            )
        )
    if not extra:
        return planned
    kept = [
        Planned(replace(one.region, taken=False, reason="folded into a restore/re-push round-trip removal"), None)
        if one.region.id in drop
        else one
        for one in planned
    ]
    for one in extra:
        kept.append(Planned(replace(one.region, id=len(kept)), one.edit))
    return kept


def drop_chained_crossings(planned: list[Planned], chunks: tuple[tuple[int, int], ...]) -> list[Planned]:
    """Refuse the later of two crossings that would both move the same boundary.

    Each is safe alone -- anchored_inside already checked that. A chunk with a
    crossing on each side is fine: relocate() moves the two boundaries
    independently, one region eating into it from the left and another out of
    it to the right. Only two regions wanting to move the exact same boundary
    conflict, and that needs every taken edit at once to see, which a single
    region's own check cannot -- so it runs here, after every region has
    already been decided independently, rather than refusing the whole module
    the way relocate() would have to.
    """
    touched: set[int] = set()  # the shared offset of each boundary already being moved
    drop: set[int] = set()
    for one in sorted(planned, key=lambda p: p.region.at):
        if one.edit is None:
            continue
        pair = crossed_pair(chunks, one.edit.lo, one.edit.hi)
        if pair is None:
            continue
        boundary = pair[0][1]  # == pair[1][0]
        if boundary in touched:
            drop.add(one.region.id)
            continue
        touched.add(boundary)
    if not drop:
        return planned
    return [
        Planned(replace(one.region, taken=False, reason="another region already moves this LEDATA boundary"), None)
        if one.region.id in drop
        else one
        for one in planned
    ]


def rewrite(
    data: bytes,
    *,
    dry_run: bool,
    take: set[int] | None = None,
    max_regions: int | None = None,
) -> tuple[bytes, list[Region]]:
    records = omf.parse(data)
    planned = plan(records, take=take, max_regions=max_regions)
    if dry_run:
        return data, [replace(one.region, taken=False, reason="dry run") for one in planned]

    edits = [one.edit for one in planned if one.edit is not None]
    if not edits:
        return b"".join(record.emit() for record in records), [one.region for one in planned]

    segment = omf.code_segment(records)
    assert segment is not None
    seg, _name, size = segment
    moved = relocate(records, seg, omf.segment_image(records, seg, size), Shift.of(edits))
    if isinstance(moved, str):
        # the whole module is left alone; a partial rewrite is not a thing
        refused = [replace(one.region, taken=False, reason=moved) for one in planned]
        return b"".join(record.emit() for record in records), refused
    moved = orphaned_externals_renamed(moved)
    return b"".join(record.emit() for record in moved), [one.region for one in planned]


# The only names ever renamed by orphaned_externals_renamed. "Zero remaining
# fixups" is not by itself a safe test for "nothing needs this EXTDEF": FIDRQQ
# has none in any object that carries it -- LINK recognises the FP emulator
# patch by that EXTDEF's presence, not by a fixup naming it, so renaming it
# switches emulator patching off for the whole module. See AGENTS.md. These
# are the names qbopt itself invented, whose only role is a fixup target, so
# their absence is provably safe to signal by absence.
RENAMABLE_IF_ORPHANED = {FIX_MULTIPLY}


def orphaned_externals_renamed(records: list[omf.Record]) -> list[omf.Record]:
    """A qbopt-owned EXTDEF nothing points at any more, renamed to one that resolves.

    Absorbing every call to fixMul& drops every fixup that named it, and that
    is deliberately not the same as dropping the EXTDEF: doing that would
    renumber every later index, in every fixup and every THREAD, file-wide.
    Renaming costs none of that -- the index stays where every fixup and
    thread already expects it, and only the one EXTDEF entry's bytes change.
    """
    names = omf.externals(records)
    live = {f.index for f in omf.fixups(records) if f.target == "external"}
    survivor = next((n for i, n in enumerate(names) if i in live and n), None)
    if survivor is None:
        return records
    for index, name in enumerate(names):
        if name in RENAMABLE_IF_ORPHANED and index not in live:
            records = omf.rename_external(records, index, survivor)
    return records


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="qbopt.rewrite")
    ap.add_argument("input", type=Path)
    ap.add_argument("-o", "--output", type=Path)
    ap.add_argument("--manifest", type=Path)
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--take", help="comma-separated region ids; refuse the rest")
    ap.add_argument("--max-regions", type=int)
    ap.add_argument("--report", action="store_true")
    args = ap.parse_args(argv)

    data = args.input.read_bytes()
    take = {int(x) for x in args.take.split(",")} if args.take else None
    out, found = rewrite(data, dry_run=args.dry_run, take=take, max_regions=args.max_regions)

    if args.output:
        args.output.write_bytes(out)

    manifest = {
        "input": str(args.input),
        "input_sha256": hashlib.sha256(data).hexdigest(),
        "output_sha256": hashlib.sha256(out).hexdigest(),
        "dry_run": args.dry_run,
        "regions": [asdict(r) for r in found],
        "taken": sum(1 for r in found if r.taken),
    }
    path = args.manifest or (args.output.with_suffix(".json") if args.output else None)
    if path:
        path.write_text(json.dumps(manifest, indent=2))

    if args.report:
        print(f"{args.input.name}: {len(found)} regions, {manifest['taken']} taken")
        for r in found:
            span = r.end - r.at
            note = "taken" if r.taken else f"refused -- {r.reason}"
            print(f"   {r.id:3} {r.at:#06x}..{r.end:#06x}  {span:4}b  {note}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
