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
from dataclasses import dataclass

from qbopt import omf
from qbopt import wholeseg


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


def rewrite(
    data: bytes,
    *,
    dry_run: bool,
    take: set[int] | None = None,
    max_regions: int | None = None,
    native_fpu: bool = False,
    whole_segment: bool = True,
    absorb_calls: bool = True,
) -> tuple[bytes, list[Region]]:
    """Rewrite to a fixed point, or once where the caller is bisecting.

    One pass is not a fixed point and the reason is worth stating: absorbing
    a call removes a barrier, and a store and reload the call used to sit
    between only becomes visible afterwards. procs-q-O ends up with
    `mov [bp-12h],eax` immediately followed by `mov eax,[bp-12h]`, which the
    first pass could not see because the call was still there when it
    looked. Every deletion in the corpus after the first pass is of that
    shape.

    `take` and `max_regions` exist to bisect a failure by region index, and
    an index only means anything within one pass -- so those run once, as
    they did before.
    """
    # `take`, `max_regions` and `dry_run` bisected the machine arm by
    # region index. There are no regions to bisect: what replaced them is
    # `transform.applied(only=...)`, which runs one MIR pass and is a
    # better question anyway -- a pass has a name, a region had a number.
    if dry_run:
        return data, []

    regions: list[Region] = []  # nothing plans one now; the CLI still reports the list

    # The machine arm is gone. It patched BC's own bytes in place -- an
    # idiom matched by address and adjacency, rewritten where it stood --
    # and everything it did the MIR arm now does from values. Measured
    # before it went, over fixtures/omf:
    #
    #     machine arm alone   682,978    -26,252
    #     MIR arm alone       642,807    -66,423
    #     both                641,542    -67,688
    #
    # So it was worth 1,265 bytes on top of the MIR arm, 1.9% of the gain,
    # for 2,764 lines whose every matcher is an address and an adjacency --
    # which is what stops any pass above from moving anything. The suite
    # links and runs on the MIR arm alone.
    made_by = _configuration(whole_segment, native_fpu, absorb_calls)
    was = omf.finalised_at(omf.parse(data))
    if was is not None:
        # Already emitted by this pass. What came out is a program -- a
        # prologue, the copies a phi became, the slots a spill took -- and
        # raising it again reads all of that as code BC wrote: 35 ops in
        # and 50 out on hotlop-q-evt, a second frame on top of the first,
        # and S= 0 for 630. Given back untouched, before anything decodes
        # it.
        if was != made_by:
            raise Finalised(f"this object was written by {was!r}, and this run is {made_by!r}")
        return data, regions

    for _ in range(PASS_ROUNDS):
        out, terminal = _written(data, whole_segment, native_fpu, absorb_calls)
        if terminal:
            # Only the LIR emitter's own output is final. A fallback is
            # BC's layout with this pass's choices in it, and raising that
            # again is what the loop is for.
            return b"".join(one.emit() for one in omf.finalised(omf.parse(out), made_by)), regions
        if out == data:
            break
        data = out
    return data, regions


class Finalised(Exception):
    """An object this pass already wrote, asked for with other options."""


def _configuration(whole_segment: bool, native_fpu: bool, absorb_calls: bool) -> str:
    """Every option that can change what the emitter writes, as one string.

    The marker holds it so a second run can tell "already done" from
    "done, but not the way you are asking for now". Its own schema is
    first: a later version of this pass reading an older marker has to
    refuse rather than assume the bytes mean what they would today.
    """
    return "1;" + ",".join(
        name for name, on in (("whole", whole_segment), ("fpu", native_fpu), ("absorb", absorb_calls)) if on
    )


def _written(
    data: bytes, whole_segment: bool, native_fpu: bool = False, absorb_calls: bool = True
) -> tuple[bytes, bool]:
    """The object with its code segment emitted from MIR, where that works.

    Every edit above patches BC's own bytes in place and keeps the layout BC
    chose. This replaces the segment: layout.py places every instruction and
    relocate.py writes the records, so chunk boundaries, branch
    displacements and fixup offsets are all produced rather than preserved.

    Silent fallback is the point. wholeseg.rebuilt() returns the input
    unchanged and says why whenever anything refuses, and a refusal is a
    module this pass has already improved by absorption -- there is nothing
    to gain by throwing that away too.

    It is not a size cost: measured over fixtures/omf it saves 201 bytes
    against the patched output and over qb-qrender 616, because the selector
    picks the shorter encoding in places BC's own layout could not be
    changed to use. docs/numbers.md has the table.

    It was off for a while, because it was not correct: `add ax,offset X`
    arrives at the selector as `add ax,0`, the sign-extended byte form fits
    zero, and the two-byte fixup then named a one-byte field. The linker
    patched two bytes regardless, over the immediate and the byte after it,
    and a generated program read 0 for an array element. select.emit had a
    `relocated` flag for exactly this and none of its callers passed it.

    Nothing that compared the emitted code could see it -- before linking,
    both forms disassemble as `add ax,0`. It took reducing the program to
    thirty-one lines and diffing the two linked images, where one says
    `add ax,0DCh` and the other `add ax,0FFDCh`.
    """
    if not whole_segment:
        # The legacy arm patches BC's own bytes and leaves a program this
        # can raise again. It is the only one the loop above is for.
        return data, False
    # To a fixed point, because that is what re-raises the SSA. Every
    # transform here rewrites the op list, and the values an op defines and
    # uses are computed when the body is raised -- so after one transform
    # they describe the body that went in, not the one that came out. The
    # ordering in transform.applied() is what has been holding that
    # together, and it is load-bearing: widening ran before avail.py once
    # and avail forwarded a stale high half across an op that said it read
    # two bytes where the instruction read four.
    #
    # Re-parsing the emitted object is the honest way to get the values
    # back, and the only one that needs no transform to maintain them. Each
    # round sees a body raised from what the last round actually wrote.
    # One transform per round, each seeing a body raised from what the last
    # round wrote. Not stopped early on "nothing changed": a pass that finds
    # nothing says nothing about the one after it.
    # Each pass once is not a fixed point: a pass can only see what the
    # round before it wrote, so one that fires on the result of another
    # needs the sequence run again. Capped, and the corpus settles in two.
    # Every pass on one MIR body, then lowered once. This ran each pass
    # through its own rebuilt() -- parse, raise, one pass, lower, allocate,
    # lay out, emit -- so the program round-tripped through machine code
    # between every pair of passes and resolved() re-derived SSA by register
    # each time. transform.applied() already runs them all on one body.
    #
    # It is what the architecture says and it is also why a live range split
    # "comes back through emission as an ordinary move": emission was
    # happening between the passes. Same output on every program, 2.4x less
    # work to get it.
    # Once. Only the LIR emitter's output is terminal, and it says so
    # rather than being inferred from bytes that stopped changing.
    got = wholeseg.emitted(data, native_fpu=native_fpu)
    return got.data, got.outcome is wholeseg.Emission.LIR


PASS_ROUNDS = 4


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
    ap.add_argument(
        "--native-fpu",
        action="store_true",
        help="replace the FP emulator's interrupts with real x87 -- REQUIRES A COPROCESSOR",
    )
    ap.add_argument(
        "--no-absorb-calls",
        action="store_true",
        help="leave the arithmetic calls to the MIR tower instead of calls.py",
    )
    ap.add_argument(
        "--no-whole-segment",
        action="store_true",
        help="patch BC's own bytes rather than writing the code segment from MIR",
    )
    args = ap.parse_args(argv)

    data = args.input.read_bytes()
    take = {int(x) for x in args.take.split(",")} if args.take else None
    out, found = rewrite(
        data,
        dry_run=args.dry_run,
        take=take,
        max_regions=args.max_regions,
        native_fpu=args.native_fpu,
        whole_segment=not args.no_whole_segment,
        absorb_calls=not args.no_absorb_calls,
    )

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
