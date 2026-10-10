"""Code and data of each bench program as linked, from the linker maps: BCOM45 (MS LINK), the llrm runtime on m16 (MS LINK
with LLRMQB.LIB) and on m32 (jwlink, LE under DOS/32A).

    python tools/dosbatch/qbcodedata.py [--work DIR]
"""

from __future__ import annotations

import argparse
import re
import shutil
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qb32  # noqa: E402
import qbruntime  # noqa: E402


def classify(name: str, cls: str) -> str:
    """code, stack, or data (initialised, constants, bss: all that is not code)."""
    if cls.upper().endswith("CODE") or name.upper().endswith("_TEXT"):
        return "code"
    return "stack" if cls.upper() == "STACK" else "data"


def ms_map(path: Path) -> dict[str, int]:
    """Segment sizes of an MS LINK map by kind."""
    totals = {"code": 0, "data": 0, "stack": 0}
    for line in path.read_text(errors="replace").splitlines():
        match = re.match(r"\s*([0-9A-F]{5})H\s+([0-9A-F]{5})H\s+([0-9A-F]{5})H\s+(\S+)\s+(\S+)", line)
        if match:
            totals[classify(match.group(4), match.group(5))] += int(match.group(3), 16)
    return totals


def jw_map(path: Path) -> dict[str, int]:
    """Segment sizes of a jwlink map by kind."""
    totals = {"code": 0, "data": 0, "stack": 0}
    for line in path.read_text(errors="replace").splitlines():
        match = re.match(r"(\S+)\s+(\S+)\s+\S+\s+[0-9a-f]{4}:[0-9a-f]{8}\s+([0-9a-f]{8})\s*$", line)
        if match:
            totals[classify(match.group(1), match.group(2))] += int(match.group(3), 16)
    return totals


def m16(work: Path, sources: list[Path]) -> tuple[dict, dict]:
    """BCOM45's and the llrm runtime's maps, from one differential run: the maps are copied after each session."""
    kept = {"ref": {}, "cand": {}}
    original = dosbatch.run
    sessions = iter(("ref", "cand"))

    def run(jobs, where, **options):
        results = original(jobs, where, **options)
        side = next(sessions)
        for job in jobs:
            shutil.copy(where / f"{job.stem}.MAP", work / f"{side}_{job.stem}.MAP")
            kept[side][job.stem] = work / f"{side}_{job.stem}.MAP"
        return results

    dosbatch.run = run
    try:
        objects = {}
        for source in sources:
            pair = (work / f"{source.stem}.qb45.obj", work / f"{source.stem}.llrm.obj")
            for runtime, obj in zip(("qb45", "llrm"), pair):
                if reason := qbruntime.compile_basic(source, obj, runtime):
                    raise SystemExit(f"{source}: {reason}")
            objects[source.stem] = pair
        archive, _ = qbruntime.build(work / "archive")
        qbruntime.differential_batch(objects, archive, work / "differential")
    finally:
        dosbatch.run = original
    names = {source.stem: f"J{at:03d}" for at, source in enumerate(sources)}
    return {n: ms_map(kept["ref"][s]) for n, s in names.items()}, {n: ms_map(kept["cand"][s]) for n, s in names.items()}


def m32(work: Path, sources: list[Path]) -> dict:
    runtime = qb32.build(work / "rt32")
    out = {}
    for source in sources:
        obj, exe, listing = work / f"{source.stem}.obj32", work / f"{source.stem}.exe", work / f"{source.stem}.map32"
        if reason := qb32.compile_basic(source, obj):
            raise SystemExit(f"{source}: {reason}")
        dosbatch.link_target(qb32.TARGET, obj, exe, work, listing=listing, runtime=(qb32.START_FILES, []), objects_after=tuple(qb32.closure(obj, runtime)))
        out[source.stem] = jw_map(listing)
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work", type=Path, default=None)
    args = parser.parse_args()
    work = args.work or Path(tempfile.mkdtemp(prefix="qbcodedata-"))
    work.mkdir(parents=True, exist_ok=True)
    sources = qbruntime.milestone_sources()
    ref, cand = m16(work, sources)
    big = m32(work, sources)
    print(f"{'program':<13}| {'BCOM45 m16':^20} | {'llrm m16':^20} | {'llrm m32':^20}")
    print(f"{'':<13}| {'code':>6}{'data':>7}{'stack':>7} | {'code':>6}{'data':>7}{'stack':>7} | {'code':>6}{'data':>7}{'stack':>7}")
    for source in sources:
        n = source.stem
        cells = [ref[n], cand[n], big[n]]
        print(f"{n:<13}| " + " | ".join(f"{c['code']:>6}{c['data']:>7}{c['stack']:>7}" for c in cells))
    return 0


if __name__ == "__main__":
    sys.exit(main())
