"""The driver: one LINK unit in, each standalone .OBJ optimized once.

Positional inputs are the object and library inputs in linker order. External
calls are resolved across that unit before any body is raised, and all output
objects are buffered until every one has completed. A refusal is an error by
default; `--allow-unchanged` names the compatibility behavior explicitly.

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

from qbopt import wholeseg
from qbopt.abi import profile
from qbopt.abi import runtime
from qbopt.objectfile import omf


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
    native_fpu: bool = True,
    whole_segment: bool = True,
    absorb_calls: bool = True,
    cpu: str = "386",
    basic_semantics: bool = False,
    bounds_checks: bool = False,
    contract_profile: profile.Profile | None = None,
    external_contracts: dict[str, runtime.Contract] | None = None,
    contract_fingerprint: str | None = None,
    allow_unchanged: bool = False,
) -> tuple[bytes, list[Region]]:
    """Optimize one raised body and lower it once.

    Repeated optimization belongs on MIR, never on emitted machine code.
    Only output from the allocating backend receives the completion marker.
    A refusal raises by default: returning the input makes an unsupported
    construct indistinguishable from a successful no-op optimization.
    """
    # `take`, `max_regions` and `dry_run` bisected the machine arm by
    # region index. There are no regions to bisect: what replaced them is
    # `transform.applied(only=...)`, which runs one MIR pass and is a
    # better question anyway -- a pass has a name, a region had a number.
    from qbopt.backend import arithmetic

    arithmetic.validate(cpu)
    wholeseg._native_only(native_fpu)
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
    made_by = _configuration(whole_segment, native_fpu, absorb_calls, cpu, basic_semantics, bounds_checks)
    fingerprints = tuple(
        one
        for one in (contract_fingerprint, contract_profile.fingerprint if contract_profile is not None else None)
        if one is not None
    )
    if fingerprints:
        made_by += ",contracts=" + hashlib.sha256(";".join(fingerprints).encode()).hexdigest()
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

    combined = dict(external_contracts or {})
    if contract_profile is not None:
        combined.update({rule.name: rule for rule in contract_profile.rules})
    out, terminal, reason = _written(
        data,
        whole_segment,
        native_fpu,
        absorb_calls,
        cpu,
        basic_semantics,
        bounds_checks,
        combined or None,
    )
    if terminal:
        return b"".join(one.emit() for one in omf.finalised(omf.parse(out), made_by)), regions
    if allow_unchanged:
        # Explicit compatibility mode only. Machine output is never fed back
        # into the raise to approximate a MIR fixed point.
        return data, regions
    raise Unsupported(reason)


class Finalised(Exception):
    """An object this pass already wrote, asked for with other options."""


class Unsupported(RuntimeError):
    """The strict optimizer encountered a construct it cannot reproduce."""


def _configuration(
    whole_segment: bool,
    native_fpu: bool,
    absorb_calls: bool,
    cpu: str = "386",
    basic_semantics: bool = False,
    bounds_checks: bool = False,
) -> str:
    """Every option that can change what the emitter writes, as one string.

    The marker holds it so a second run can tell "already done" from
    "done, but not the way you are asking for now". Its own schema is
    first: a later version of this pass reading an older marker has to
    refuse rather than assume the bytes mean what they would today.
    """
    return (
        "1;"
        + ",".join(name for name, on in (("whole", whole_segment), ("fpu", native_fpu), ("absorb", absorb_calls)) if on)
        + (f",cpu={cpu}" if cpu != "386" else "")
        + (",basic-semantics" if basic_semantics else "")
        + (",bounds-checks" if bounds_checks else "")
    )


def _written(
    data: bytes,
    whole_segment: bool,
    native_fpu: bool = True,
    absorb_calls: bool = True,
    cpu: str = "386",
    basic_semantics: bool = False,
    bounds_checks: bool = False,
    external_contracts: dict[str, runtime.Contract] | None = None,
) -> tuple[bytes, bool, str]:
    """Lower and emit once; report whether the allocating backend completed."""
    if not whole_segment:
        return data, False, "whole-segment emission is disabled"
    got = wholeseg.emitted(
        data,
        native_fpu=native_fpu,
        cpu=cpu,
        basic_semantics=basic_semantics,
        bounds_checks=bounds_checks,
        external_contracts=external_contracts,
    )
    if not bounds_checks and got.reason.startswith("unchecked array lowering unsupported"):
        raise ValueError(got.reason + "; use --bounds-checks to retain the checked helper")
    return got.data, got.outcome is wholeseg.Emission.LIR, got.reason


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="qbopt.rewrite")
    ap.add_argument(
        "inputs",
        type=Path,
        nargs="+",
        help="OMF .OBJ files to optimize and .LIB files used to resolve them, in LINK order",
    )
    from qbopt.cycles.timings import ARCHS

    ap.add_argument("--cpu", choices=("386", *ARCHS), default="386", help="arithmetic tuning target")
    ap.add_argument("-o", "--output", type=Path, help="output file; valid for a single input OBJ")
    ap.add_argument("--output-dir", type=Path, help="directory receiving every optimized input OBJ")
    ap.add_argument("--manifest", type=Path)
    ap.add_argument(
        "--contracts",
        type=Path,
        action="append",
        help="audited, hash-checked external call profile (JSON); repeat to combine profiles",
    )
    ap.add_argument(
        "--contract-root",
        type=Path,
        help="artifact directory shared by all profiles; defaults to each profile directory",
    )
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--take", help="comma-separated region ids; refuse the rest")
    ap.add_argument("--max-regions", type=int)
    ap.add_argument("--report", action="store_true")
    ap.add_argument(
        "--basic-semantics",
        action="store_true",
        help="preserve BASIC numeric runtime errors, conversions and floating behavior",
    )
    ap.add_argument(
        "--bounds-checks",
        action="store_true",
        help="retain BASIC array bounds checks (independent of numeric semantics)",
    )
    ap.add_argument(
        "--native-fpu",
        action="store_true",
        help="accepted and ignored: real x87 is the only floating-point path, and every build REQUIRES A COPROCESSOR",
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
    ap.add_argument(
        "--allow-unchanged",
        action="store_true",
        help="explicitly retain an input OBJ when the backend refuses it (strict failure is the default)",
    )
    args = ap.parse_args(argv)
    if args.contract_root is not None and args.contracts is None:
        ap.error("--contract-root requires --contracts")
    if args.output is not None and args.output_dir is not None:
        ap.error("use either --output or --output-dir, not both")
    take = {int(x) for x in args.take.split(",")} if args.take else None
    try:
        from qbopt.abi import linkunit

        unit = linkunit.LinkUnit.read(args.inputs)
        if args.output is not None and len(unit.objects) != 1:
            raise ValueError("--output requires exactly one standalone OBJ; use --output-dir for several")
        contracts = profile.load_many(args.contracts, args.contract_root) if args.contracts is not None else None
        optimized = []
        # Finish the whole unit before writing any member. One unsupported
        # object therefore leaves every caller input and prior output intact.
        for source in unit.objects:
            try:
                out, regions = rewrite(
                    source.data,
                    dry_run=args.dry_run,
                    take=take,
                    max_regions=args.max_regions,
                    whole_segment=not args.no_whole_segment,
                    absorb_calls=not args.no_absorb_calls,
                    cpu=args.cpu,
                    basic_semantics=args.basic_semantics,
                    bounds_checks=args.bounds_checks,
                    contract_profile=contracts,
                    external_contracts=unit.contracts_for(source),
                    contract_fingerprint=unit.fingerprint,
                    allow_unchanged=args.allow_unchanged,
                )
            except Unsupported as error:
                raise Unsupported(f"{source.path}: {error}") from error
            optimized.append((source, out, regions))
    except (ValueError, OSError, Finalised, Unsupported) as error:
        ap.error(str(error))

    if args.output_dir is not None:
        names = [source.path.name.casefold() for source, _out, _regions in optimized]
        if len(names) != len(set(names)):
            ap.error("--output-dir cannot represent input OBJs with duplicate filenames")
        args.output_dir.mkdir(parents=True, exist_ok=True)
        for source, out, _regions in optimized:
            (args.output_dir / source.path.name).write_bytes(out)
    elif args.output is not None:
        args.output.write_bytes(optimized[0][1])

    objects = [
        {
            "input": str(source.path),
            "input_sha256": hashlib.sha256(source.data).hexdigest(),
            "output_sha256": hashlib.sha256(out).hexdigest(),
            "input_bytes": len(source.data),
            "output_bytes": len(out),
            "regions": [asdict(region) for region in regions],
            "taken": sum(1 for region in regions if region.taken),
        }
        for source, out, regions in optimized
    ]
    common = {
        "dry_run": args.dry_run,
        "cpu": args.cpu,
        "semantics": "basic" if args.basic_semantics else "native",
        "bounds_checks": args.bounds_checks,
        "link_inputs": [str(path) for path in unit.inputs],
        "link_unit_sha256": unit.fingerprint,
        "contract_profile_sha256": contracts.fingerprint if contracts is not None else None,
    }
    manifest = {**objects[0], **common} if len(objects) == 1 else {**common, "objects": objects}
    path = args.manifest
    if path is None and args.output is not None:
        path = args.output.with_suffix(".json")
    if path is None and args.output_dir is not None:
        path = args.output_dir / "qbopt-manifest.json"
    if path:
        path.write_text(json.dumps(manifest, indent=2))

    if args.report:
        for source, out, regions in optimized:
            taken = sum(1 for region in regions if region.taken)
            print(f"{source.path.name}: {len(source.data)} -> {len(out)} bytes; {len(regions)} regions, {taken} taken")
            for region in regions:
                span = region.end - region.at
                note = "taken" if region.taken else f"refused -- {region.reason}"
                print(f"   {region.id:3} {region.at:#06x}..{region.end:#06x}  {span:4}b  {note}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
