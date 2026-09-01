"""
The divergence hunter: N generated programs (tools/fuzzgen.py), run through
BC's own build first and only then through qbopt's rewritten one, across
whichever of the twelve configurations are available -- reporting any place
BC's own build disagrees with the reference evaluator separately from any
place qbopt's rewritten build disagrees with it, because those are different
bugs (evaluator or BC, versus qbopt) and byte-diff against BC cannot tell them
apart once an optimizing pass starts emitting different code on purpose.

Every generated program runs through tools/e2e.py's own compile/link/run/judge
machinery -- the same one suite/*.bas gets -- pointed at a disposable program
set via `names`/`source_dir`/`golden_dir` rather than a parallel harness. Two
passes per configuration:

  1. An identity transform, so `judge()`'s own BASEDIFF verdict means exactly
     "BC disagrees with the evaluator", with qbopt not asked yet. This is
     docs/testing.md's own rule ("an unexplained base is not a base") applied
     to a generated corpus instead of a hand-written one, and it is this
     project's own prescribed order: confirm BC agrees with the evaluator
     before ever judging qbopt against either.
  2. qbopt's real rewrite, run only over the programs that passed pass 1 --
     a program BC itself could not build cleanly is not a qbopt finding.

A generated program BC's own compiler rejects (BCFAIL) is not a finding
either: BC's constant folder applies its own compile-time overflow check
(see fuzzgen.py's module docstring) that the grammar mostly avoids but not,
apparently, perfectly. Since qbopt only ever sees objects BC successfully
produced, a program BC refuses to compile is out of scope by construction --
logged as a skip, not chased as a bug.

The launch cache (tools/cache.py) is left on, deliberately. A fixed `--seed`
regenerates byte-identical source every run, so pass 1 -- BC's own build,
untouched by anything qbopt does -- is a cache hit on every re-run during
iteration, not just the first. Pass 2's link+run step is keyed on the
workdir's own content at call time, which includes the just-rewritten .OBJ:
a change to qbopt's own code changes those bytes, which changes the key, so
a stale hit there is not possible by construction -- the same guarantee
every other `cached_launch()` caller already relies on. Only a genuinely
new `--seed`/`--count` combination pays full DOSBox cost; re-running the
same sweep after fixing a bug should not.
"""

import sys
import shutil
import argparse
from pathlib import Path
from dataclasses import dataclass
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, str(Path(__file__).resolve().parent))

import e2e
from configs import CONFIGS
from fuzzgen import golden_lines
from fuzzgen import render_program
from fuzzgen import generate_program

ROOT = Path(__file__).resolve().parents[1]
BUILD = ROOT / "build" / "fuzz"


@dataclass(frozen=True, slots=True)
class TagReport:
    tag: str
    total: int
    rejected: list[str]  # BC would not compile it -- out of scope, not a finding
    base_diffs: list[e2e.Verdict]  # BC's own build disagrees with the evaluator
    qbopt_diffs: list[e2e.Verdict]  # qbopt's rewrite disagrees with the evaluator
    passed: int

    @property
    def ok(self) -> bool:
        return not self.base_diffs and not self.qbopt_diffs


def identity(data: bytes) -> bytes:
    return data


def build_corpus(seed: int, count: int, work: Path) -> tuple[Path, Path, list[str]]:
    source_dir = work / "src"
    golden_dir = work / "golden"
    source_dir.mkdir(parents=True)
    golden_dir.mkdir(parents=True)

    names = [f"F{i:03d}" for i in range(count)]
    for i, name in enumerate(names):
        program = generate_program(seed=seed * 1_000_000 + i)
        (source_dir / f"{name}.bas").write_text(render_program(program))
        (golden_dir / f"{name}.txt").write_text("\n".join(golden_lines(program)) + "\n")
    return source_dir, golden_dir, names


def run_tag(
    tag: str,
    source_dir: Path,
    golden_dir: Path,
    names: list[str],
    timeout: int,
    absorb_calls: bool = True,
) -> TagReport:
    base = e2e.run(
        tag,
        names=names,
        source_dir=source_dir,
        golden_dir=golden_dir,
        transform=identity,
        work=BUILD / tag / "base",
        timeout=timeout,
    )
    by_status: dict[str, list[e2e.Verdict]] = {}
    for v in base.verdicts:
        by_status.setdefault(v.status, []).append(v)

    rejected = [v.program for v in by_status.get("BCFAIL", [])]
    base_diffs = by_status.get("BASEDIFF", [])
    # LINKFAIL/RUNFAIL/NODONE/REWRITEFAIL at the identity pass means BC's own
    # build could not even be judged -- out of scope the same way BCFAIL is,
    # not a finding about the evaluator, which never got a real base output
    excluded = {"PASS", "BASEDIFF", "BCFAIL"}
    unjudged = [v.program for s, vs in by_status.items() for v in vs if s not in excluded]

    survivors = [n for n in names if n not in set(rejected) | set(unjudged) | {v.program for v in base_diffs}]
    if not survivors:
        return TagReport(tag, len(names), rejected + unjudged, base_diffs, [], 0)

    opt = e2e.run(
        tag,
        names=survivors,
        source_dir=source_dir,
        golden_dir=golden_dir,
        work=BUILD / tag / "opt",
        timeout=timeout,
        transform=None
        if absorb_calls
        else (lambda data: __import__("qbopt.rewrite", fromlist=["rewrite"]).rewrite(
            data, dry_run=False, absorb_calls=False
        )[0]),
    )
    qbopt_diffs = [v for v in opt.verdicts if not v.ok]
    passed = sum(1 for v in opt.verdicts if v.ok)
    return TagReport(tag, len(names), rejected + unjudged, base_diffs, qbopt_diffs, passed)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="fuzzcheck")
    ap.add_argument("--count", type=int, default=60)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--timeout", type=int, default=300)
    ap.add_argument(
        "--no-absorb-calls",
        action="store_true",
        help="leave the arithmetic calls to the MIR tower instead of calls.py",
    )
    ap.add_argument("--tag", action="append", dest="tags", help="repeatable; default is every available config")
    args = ap.parse_args(argv)

    tags = args.tags or [t for t, c in CONFIGS.items() if c.available]
    if skipped := [t for t in CONFIGS if t not in tags]:
        print(f"no toolchain for {', '.join(skipped)}; see docs/testing.md")

    shutil.rmtree(BUILD, ignore_errors=True)
    corpus_dir = BUILD / "corpus"
    source_dir, golden_dir, names = build_corpus(args.seed, args.count, corpus_dir)
    print(f"generated {len(names)} programs, seed {args.seed}, into {corpus_dir}")

    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        reports = list(
            pool.map(
                lambda t: run_tag(
                    t, source_dir, golden_dir, names, args.timeout, not args.no_absorb_calls
                ),
                tags,
            )
        )

    bad = 0
    for r in reports:
        print(f"  {r.tag:9} {r.passed}/{len(names)} pass, {len(r.rejected)} BC-rejected")
        for v in r.base_diffs:
            print(f"    BASEDIFF (evaluator/BC disagreement) {v.program}: {v.detail}")
        for v in r.qbopt_diffs:
            print(f"    {v.status} (qbopt regression) {v.program}: {v.detail}")
        if not r.ok:
            bad += 1

    print(f"\n{len(reports) - bad} of {len(reports)} configurations found no divergence")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
