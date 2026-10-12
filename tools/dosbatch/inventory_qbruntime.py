"""Record the QB runtime symbols the fixed milestone-1 corpus actually links."""

from __future__ import annotations

import argparse
import json
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qbruntime  # noqa: E402
import run_tests  # noqa: E402


def compile_one(source: Path, obj: Path) -> str | None:
    return run_tests.compile_one(run_tests.Program(source, ["-O2"], None, "qb45"), obj)


def inventory(work: Path, archive: Path) -> dict:
    """Link every milestone source against `archive` and preserve every LINK log."""
    sources = qbruntime.milestone_sources()
    objects = work / "objects"
    objects.mkdir(parents=True, exist_ok=True)
    stems = {source: f"I{at:03d}" for at, source in enumerate(sources)}
    with ThreadPoolExecutor() as pool:
        compiled = dict(zip(sources, pool.map(lambda source: compile_one(source, objects / f"{stems[source]}.OBJ"), sources)))
    jobs = [
        dosbatch.Job(stems[source], "obj", objects / f"{stems[source]}.OBJ", runtime="empty", runtime_file=archive)
        for source in sources
        if compiled[source] is None
    ]
    ran = dosbatch.run(jobs, work / "links")
    programs = []
    symbols: set[str] = set()
    for source in sources:
        stem = stems[source]
        if problem := compiled[source]:
            programs.append({"source": str(source.relative_to(dosbatch.ROOT)), "compile": problem, "symbols": []})
            continue
        log = dosbatch.read_dos(work / "links", f"{stem}.LNK")
        required = qbruntime.undefined_symbols(log)
        symbols.update(required)
        programs.append(
            {
                "source": str(source.relative_to(dosbatch.ROOT)),
                "link": ran[stem].status,
                "symbols": required,
                "log": f"links/{stem}.LNK",
            }
        )
    return {"runtime": "empty", "archive": str(archive), "programs": programs, "symbols": sorted(symbols)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if not args.archive.is_file():
        parser.error(f"archive does not exist: {args.archive}")
    found = inventory(args.work, args.archive)
    args.out.write_text(json.dumps(found, indent=2) + "\n")
    print(f"{len(found['programs'])} programs, {len(found['symbols'])} B$ symbols")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
