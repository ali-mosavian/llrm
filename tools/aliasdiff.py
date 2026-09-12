"""
What the alias model answers, over the whole corpus, as a file you can diff.

    uv run python tools/aliasdiff.py record build/alias-base.json
    uv run python tools/aliasdiff.py compare build/alias-base.json
    uv run python tools/aliasdiff.py compare build/alias-base.json --limit 40

The region refactor replaces a table of hand-written `(Space, Space)` arms with
one lattice, and the question at every step is not "does it still build" but
"which pair changed answer, and why". A pass/fail suite cannot answer that.

Three instruments, because one is not enough:

**Decisions.** 9 million `overlapping` calls over 487 objects collapse to about
57,000 distinct keys, so every key is recorded -- but 4.3% of keys answer both
True and False, since the answer depends on provenance the key cannot carry and
on the `known`/`bounds` arguments. So a key holds its true/false *counts* and a
diff compares those. That survives stage 2 deleting the fields a richer key
would have needed.

**Clobber cardinality.** `avail`, `consts` and `loadjoins` all require
`memoryssa.clobbers` to return exactly one access before they will do anything.
Losing precision there switches them off silently -- no error, no wrong answer,
just work not done. The singleton count is the number that must not fall.

**Downstream work.** Hoisted ops, forwarded loads and rebuilt bytes, so a
precision loss that escapes both instruments above still shows up as less work
done.

A "gain" is a pair that used to alias and no longer does. Every one is a claim,
and at stage 4 each must map to a named axiom. A "loss" is the reverse and must
be zero.
"""

import sys
import json
import time
import argparse
from pathlib import Path
from collections import Counter
from collections import defaultdict

sys.path.insert(0, str(Path(__file__).resolve().parent))

from qbopt.model import mir
from qbopt.analysis import avail
from qbopt.analysis import regions
from qbopt.analysis import memoryssa
from qbopt.optimize import transform

CORPUS = Path("fixtures/omf")


def _key(ref) -> str:
    """A reference's identity, in fields the refactor does not delete.

    `Addr`, width and the pointer flag survive every stage of the plan;
    `allocation`, `excludes`, `beyond` and `space` do not, so a key built from
    those would stop matching exactly when it was needed most.
    """
    return f"{ref.addr}|{ref.width}|{'p' if ref.pointer else '-'}"


def _lattice(one, other, bounds, known, other_known) -> bool:
    """`overlapping` with the region set standing where the table stands.

    The two rewrites ahead of it and the same-base arithmetic behind it are
    not part of what stage 2 deletes, so comparing without them would measure
    their absence rather than the lattice.
    """
    if not (one.pointer or other.pointer):
        if known or other_known:
            from qbopt.analysis import ranges

            one = ranges.covering(one, known or {})
            other = ranges.covering(other, other_known or {})
        one, other = mir._symbolic_ref(one), mir._symbolic_ref(other)
        if (
            one.base is not None
            and one.base == other.base
            and one.addr is not None
            and other.addr is not None
            and one.addr.space is other.addr.space
            and one.addr.index == other.addr.index
            and one.segment == other.segment
            and (one.addr.space is not mir.Space.FAR or one.segment is not None)
        ):
            return one.addr.disp < other.addr.disp + other.width and other.addr.disp < one.addr.disp + one.width
    return regions.may_alias(one, other, bounds)


class Watch:
    """Every alias answer the pipeline asks for, while it asks for them."""

    def __init__(self) -> None:
        self.pairs: dict[str, list[int]] = defaultdict(lambda: [0, 0])
        self.clobbers: Counter = Counter()
        self.work: Counter = Counter()
        self.objects: dict[str, dict] = {}
        self.stem = "?"
        self.disagree: Counter = Counter()
        self.sideways = False
        self._saved: list = []

    def install(self) -> None:
        real_over = mir.overlapping
        real_clobbers = memoryssa.MemorySSA.clobbers
        real_invariant = transform._invariant_run
        real_forwardable = avail.forwardable
        self._saved = [
            (mir, "overlapping", real_over),
            (memoryssa.MemorySSA, "clobbers", real_clobbers),
            (transform, "_invariant_run", real_invariant),
            (avail, "forwardable", real_forwardable),
        ]

        def overlapping(one, other, dgroup, bounds=None, known=None, other_known=None):
            got = real_over(one, other, dgroup, bounds, known, other_known)
            self.pairs[f"{self.stem} {_key(one)} {_key(other)}"][1 if got else 0] += 1
            if self.sideways:
                # Stage 0's real question: the lattice against the table, on
                # every pair the corpus actually asks about. A disagreement is
                # named by the two references so it can be read back.
                mine = _lattice(one, other, bounds, known, other_known)
                if mine != got:
                    where = "GAIN" if got and not mine else "LOSS"
                    self.disagree[f"{where} {_key(one)} vs {_key(other)}"] += 1
            return got

        def clobbers(graph, site, memory, dgroup):
            got = real_clobbers(graph, site, memory, dgroup)
            self.clobbers[min(len(got), 8)] += 1
            return got

        def invariant_run(*args, **kwargs):
            # `hoisted` returns a MirBody, so it cannot be counted from
            # outside; `_invariant_run` returns the ops it found, which is the
            # opportunity itself and the thing aliasing decides.
            got = real_invariant(*args, **kwargs)
            self.work["invariant_ops"] += len(got)
            if got:
                self.work["invariant_loops"] += 1
            return got

        def forwardable(body, dgroup, calls, want):
            got = real_forwardable(body, dgroup, calls, want)
            self.work["forwarded"] += sum(1 for one in got if one.value is not None)
            return got

        mir.overlapping = overlapping
        memoryssa.MemorySSA.clobbers = clobbers
        transform._invariant_run = invariant_run
        avail.forwardable = forwardable

    def remove(self) -> None:
        for where, name, was in self._saved:
            setattr(where, name, was)

    def snapshot(self) -> dict:
        return {
            "pairs": {name: counts for name, counts in sorted(self.pairs.items())},
            "clobbers": {str(size): n for size, n in sorted(self.clobbers.items())},
            "work": dict(sorted(self.work.items())),
            "objects": self.objects,
            "disagree": dict(self.disagree.most_common()),
        }


def _run(limit: int | None, sideways: bool = False) -> dict:
    watch = Watch()
    watch.sideways = sideways
    watch.install()
    try:
        from qbopt.rewrite import rewrite

        paths = sorted(CORPUS.glob("*.obj"))
        if limit:
            paths = paths[:limit]
        start = time.time()
        for path in paths:
            watch.stem = path.stem
            data = path.read_bytes()
            try:
                out, _regions = rewrite(data, dry_run=False)
                # `rewrite` returns (bytes, regions). Taken whole, `len(out)`
                # is 2 for every object in the corpus, so every byte
                # comparison this made was between two constants.
                watch.objects[path.stem] = {"before": len(data), "after": len(out)}
            except Exception as error:  # a refusal is data, not a stop
                watch.objects[path.stem] = {"raised": type(error).__name__}
        got = watch.snapshot()
        got["seconds"] = round(time.time() - start, 1)
        got["count"] = len(paths)
        return got
    finally:
        watch.remove()


def _report(base: dict, now: dict) -> int:
    gains, losses, appeared = [], [], []
    for name, counts in now["pairs"].items():
        was = base["pairs"].get(name)
        if was is None:
            # Not skipped: once the model answers differently the pipeline
            # rewrites differently and the set of queries diverges, so a diff
            # over the intersection alone measures noise. These are the
            # divergence, and a large count means the comparison is no longer
            # apples to apples.
            appeared.append(name)
            continue
        if counts[1] < was[1]:
            gains.append((was[1] - counts[1], name))
        elif counts[1] > was[1]:
            losses.append((counts[1] - was[1], name))
    vanished = [name for name in base["pairs"] if name not in now["pairs"]]

    singles = int(now["clobbers"].get("1", 0)), int(base["clobbers"].get("1", 0))
    print(f"objects {now['count']} in {now['seconds']}s")
    print(f"  pairs asked      {len(now['pairs']):>8,}  (baseline {len(base['pairs']):,})")
    print(f"  GAINS            {len(gains):>8,}  pairs that used to alias and no longer do")
    print(f"  LOSSES           {len(losses):>8,}  pairs that now alias and did not")
    print(f"  appeared         {len(appeared):>8,}  asked now, not in baseline -- query set diverging")
    print(f"  vanished         {len(vanished):>8,}  asked in baseline, not now")
    print(f"  clobbers n=1     {singles[0]:>8,}  (baseline {singles[1]:,})  must not fall")
    for size in sorted(set(now["clobbers"]) | set(base["clobbers"]), key=int):
        here, there = int(now["clobbers"].get(size, 0)), int(base["clobbers"].get(size, 0))
        if here != there:
            print(f"    n={size:<3s}         {here:>8,}  (baseline {there:,})")
    for name in sorted(set(now["work"]) | set(base["work"])):
        here, there = now["work"].get(name, 0), base["work"].get(name, 0)
        mark = "" if here >= there else "   LOWER"
        print(f"  {name:16s} {here:>8,}  (baseline {there:,}){mark}")

    grew = sum(
        1
        for name, one in now["objects"].items()
        if "after" in one
        and name in base["objects"]
        and "after" in base["objects"][name]
        and one["after"] > base["objects"][name]["after"]
    )
    raised = [name for name, one in now["objects"].items() if "raised" in one]
    print(f"  objects larger   {grew:>8,}")
    print(f"  objects raising  {len(raised):>8,}  {raised[:4]}")

    for label, rows in (("LOSS", losses), ("GAIN", gains)):
        for count, name in sorted(rows, reverse=True)[:12]:
            print(f"  {label} x{count:<5d} {name}")
    return 1 if losses else 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("action", choices=("record", "compare", "sideways"))
    ap.add_argument("path")
    ap.add_argument("--limit", type=int, default=None, help="first N objects, for the fast loop")
    args = ap.parse_args(argv)

    now = _run(args.limit, sideways=args.action == "sideways")
    if args.action == "sideways":
        rows = now["disagree"]
        gains = {k: v for k, v in rows.items() if k.startswith("GAIN")}
        losses = {k: v for k, v in rows.items() if k.startswith("LOSS")}
        print(f"objects {now['count']} in {now['seconds']}s   pairs {len(now['pairs']):,}")
        print(f"  GAIN shapes  {len(gains):>6,}   lattice disjoint, table aliased -- each needs an axiom")
        print(f"  LOSS shapes  {len(losses):>6,}   lattice aliased, table disjoint -- must be zero")
        for name, count in list(losses.items())[:15]:
            print(f"    {name}   x{count}")
        for name, count in list(gains.items())[:15]:
            print(f"    {name}   x{count}")
        return 1 if losses else 0
    if args.action == "record":
        Path(args.path).parent.mkdir(parents=True, exist_ok=True)
        Path(args.path).write_text(json.dumps(now))
        raised = [name for name, one in now["objects"].items() if "raised" in one]
        print(f"recorded {len(now['pairs']):,} pairs from {now['count']} objects in {now['seconds']}s -> {args.path}")
        print(f"  clobbers n=1 {int(now['clobbers'].get('1', 0)):,}   work {dict(now['work'])}")
        if raised:
            # Loud: a wrapper with the wrong signature aborts every object and
            # still writes a plausible-looking file. That cost one baseline.
            print(f"  {len(raised)} OBJECTS RAISED -- the record is not usable: {raised[:5]}")
            return 1
        return 0
    return _report(json.loads(Path(args.path).read_text()), now)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
