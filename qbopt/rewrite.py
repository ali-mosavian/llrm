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

from qbopt import omf
from qbopt import module
from qbopt.flags import ALL
from qbopt.lift import lift
from qbopt.flags import Flag
from qbopt.calls import sites
from qbopt.lift import needed
from qbopt.lift import refuse
from qbopt.blocks import Block
from qbopt.calls import absorb
from qbopt.lift import regions
from qbopt.flags import live_in
from qbopt.relocate import Edit
from qbopt.blocks import CodeMap
from qbopt.relocate import Shift
from qbopt.blocks import block_at
from qbopt.blocks import code_map
from qbopt.blocks import partition
from qbopt.flags import live_after
from qbopt.lift import emit_region
from qbopt.relocate import relocate
from qbopt.blocks import instructions


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
    if not any(lo <= at and end <= hi for lo, hi in found.chunks):
        # rewriting across two LEDATA would have to merge them, and BC's
        # backpatch records make that more than a concatenation
        return "the region crosses a LEDATA boundary"
    return None


def flags_after(blocks: list[Block], live: dict[int, Flag], at: int, end: int) -> Flag:
    """The flags something reads after the region, or all of them if unknown."""
    block = block_at(blocks, at)
    return live_after(block, end, live) if block is not None else ALL


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

    reached = instructions(found)
    assert not isinstance(reached, str)
    values, _ = lift(found.code, found.start, found.end, found.resolve, reached)
    need = needed(values)

    planned = []
    for index, region in enumerate(regions(values)):
        at, end = values[region[0]].at, values[region[-1]].end
        after = flags_after(blocks, live, at, end)
        reason = anchored_inside(found, mapped, at, end) or refuse(values, need, region, after)
        if take is not None and index not in take:
            reason = "not selected"
        elif max_regions is not None and sum(1 for one in planned if one.region.taken) >= max_regions:
            reason = "past --max-regions"
        emitted = None if reason else emit_region(values, need, region, after)
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
    for site in sites(found, reached):
        after = flags_after(blocks, live, site.start, site.end)
        emitted = absorb(site, after)
        reason = anchored_inside(found, mapped, site.start, site.end)
        if reason is None and isinstance(emitted, str):
            reason = emitted
        if reason is None and any(one.edit and one.edit.lo < site.end and site.start < one.edit.hi for one in planned):
            reason = "it overlaps a region already taken"
        if reason is None and not isinstance(emitted, str) and len(emitted.code) > site.end - site.start:
            reason = f"absorbing it grows {site.end - site.start} bytes to {len(emitted.code)}"
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
    return planned


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
    return b"".join(record.emit() for record in moved), [one.region for one in planned]


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
