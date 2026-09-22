"""Legacy region alias answers against provenance answers, over the OBJ corpus.

    uv run python tools/provdiff.py [--limit N] [fixtures/omf/*.obj ...]

Every reference pair in every raised body is asked twice: as the reference
is, and with its region set translated to provenance. A pair the translation
calls disjoint that regions call overlapping is unsound and is listed; one it
widens is counted by object kinds, which is what the model has to learn next.
"""

from __future__ import annotations

import sys
import argparse
import itertools
from pathlib import Path
from dataclasses import replace
from collections import Counter

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))

import corpus  # noqa: E402

from qbopt.model import mir  # noqa: E402
from qbopt.analysis import regions  # noqa: E402
from qbopt.objectfile import module  # noqa: E402


def translated(ref: mir.MemRef, bounds: dict | None, private: frozenset[int], spared: frozenset[int]) -> mir.MemRef:
    if ref.provenance is not None:
        return ref
    return replace(ref, provenance=regions.provenance(ref, bounds, private=private, spared=spared))


def legacy(ref: mir.MemRef, bounds: dict | None, private: frozenset[int], spared: frozenset[int]) -> mir.MemRef:
    """The reference as regions alone answered it: without provenance the raise translated rather than stated."""
    bare = replace(ref, provenance=None)
    return bare if translated(bare, bounds, private, spared) == ref else ref


def kinds(ref: mir.MemRef) -> str:
    return "+".join(sorted({one.object.kind.value for one in ref.provenance.slices}))


# The legacy facts a widening can come from, in the order they are stripped.
FACTS = (("beyond", None), ("excludes", ()), ("within", None), ("allocation", None))


def reason(one: mir.MemRef, other: mir.MemRef, dgroup, bounds) -> str:
    """The first legacy fact whose removal makes the pair overlap; "other" if none alone does."""
    for name, empty in FACTS:
        if mir.overlapping(replace(one, **{name: empty}), replace(other, **{name: empty}), dgroup, bounds):
            return name
    return "other"


def compared(path: Path, limit: int) -> tuple[Counter, Counter, list[str]]:
    found = corpus.loaded(path)
    if found is None:
        return Counter(), Counter(), []
    bounds = module.landmarks(found)
    totals: Counter = Counter()
    widened: Counter = Counter()
    unsound: list[str] = []
    for name, body in mir.bodies(found, corpus.partitioned(path)):
        refs = list(dict.fromkeys(ref for block in body.blocks for op in block.ops for ref in (*op.loads, *op.stores)))
        refs = refs[:limit]
        private = frozenset() if found.program_data is None else frozenset({found.program_data})
        spared = frozenset(addr.index for ref in refs for addr, _ in ref.excludes if addr.space is module.Space.SEGMENT)
        refs = list(dict.fromkeys(legacy(ref, bounds, private, spared) for ref in refs))
        moved = {ref: translated(ref, bounds, private, spared) for ref in refs}
        for one, other in itertools.combinations(refs, 2):
            before = mir.overlapping(one, other, found.dgroup, bounds)
            after = mir.overlapping(moved[one], moved[other], found.dgroup, bounds)
            if before == after:
                totals["agree"] += 1
            elif after:
                totals["widened"] += 1
                pair = " x ".join(sorted((kinds(moved[one]), kinds(moved[other]))))
                widened[f"{reason(one, other, found.dgroup, bounds):10} {pair}"] += 1
            else:
                totals["unsound"] += 1
                unsound.append(f"{path.name} {name}: {one.addr} {one.space} / {other.addr} {other.space}")
    return totals, widened, unsound


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("objects", nargs="*", type=Path)
    parser.add_argument("--limit", type=int, default=120, help="references per body")
    args = parser.parse_args(argv)
    paths = args.objects or sorted((ROOT / "fixtures" / "omf").glob("*.obj"))
    totals: Counter = Counter()
    widened: Counter = Counter()
    unsound: list[str] = []
    for path in paths:
        one, two, three = compared(path, args.limit)
        totals += one
        widened += two
        unsound += three
    print(dict(totals))
    for pair, count in widened.most_common(15):
        print(f"  widened {count:8}  {pair}")
    for line in unsound[:20]:
        print("  UNSOUND", line)
    return 1 if unsound else 0


if __name__ == "__main__":
    raise SystemExit(main())
