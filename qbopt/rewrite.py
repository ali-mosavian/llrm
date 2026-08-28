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
from qbopt.lift import lift
from qbopt.lift import needed
from qbopt.lift import refuse
from qbopt.lift import regions
from qbopt.lift import emit_region


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


def plan(
    records: list[omf.Record],
    *,
    take: set[int] | None = None,
    max_regions: int | None = None,
) -> list[Region]:
    found = module.of(records)
    if found is None:
        return []

    values, _ = lift(found.code, found.start, found.end, found.resolve)
    need = needed(values)

    planned = []
    for index, region in enumerate(regions(values)):
        at, end = values[region[0]].at, values[region[-1]].end
        reason = refuse(values, need, region)
        if take is not None and index not in take:
            reason = "not selected"
        elif max_regions is not None and sum(1 for r in planned if r.taken) >= max_regions:
            reason = "past --max-regions"
        emitted = None if reason else emit_region(values, need, region)
        if emitted is None and reason is None:
            reason = "does not fit"
        planned.append(
            Region(
                id=index,
                seg=found.seg,
                at=at,
                end=end,
                before=found.code[at:end].hex(),
                after=emitted.hex() if emitted else None,
                taken=emitted is not None,
                reason=reason,
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
    recs = omf.parse(data)
    found = plan(recs, take=take, max_regions=max_regions)
    if dry_run:
        return data, [replace(r, taken=False, reason="dry run") for r in found]
    # Nothing is written back yet: the rewriter needs addresses out of the
    # FIXUPPs before a region can fire at all, and the relocation machinery
    # before one can be placed. Until then this is an honest pass-through.
    return b"".join(r.emit() for r in recs), found


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
