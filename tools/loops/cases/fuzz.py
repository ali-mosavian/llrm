"""
`fuzz`: loops drawn at random from the space `cross` and `concurrent`
describe, every dimension at once. The seed and the case's number name it,
and its note holds every parameter, so a failure replays:
`cases(seed=S)` rebuilds the same list.
"""

from __future__ import annotations

import random
from dataclasses import replace

from spec import I16, U16
from cases import cross, concurrent
from cases.concurrent import Walk, Shape, SIZES, TRANSFORMERS

SEED = 98
COUNT = 400


def knobs(rng: random.Random) -> cross.Knobs:
    while True:
        k = cross.Knobs(**{d: rng.choice(values) for d, values in cross.DIMENSIONS.items()})
        if cross.valid(k):
            return k


def shape(rng: random.Random) -> Shape:
    n = rng.randint(1, 12)
    elems = [e for sizes in SIZES.values() for e in sizes]
    forms = [("i",), ("off", rng.randint(-7, 7)), ("sym",), ("rev",), ("scale", rng.choice([2, 3])), ("stride",),
             ("plusn",)]
    bases = list(concurrent.BASES.values())
    walks = []
    for k in range(n):
        where, ptr = rng.choice(bases)
        same = rng.randrange(k) if k and rng.random() < 0.15 else None
        walks.append(Walk(rng.choice(elems) if same is None else walks[same].elem, rng.choice(forms), where, ptr,
                          start=rng.choice([0, 0, 1, 3, 5]), same_as=same))
    form = rng.choice(["index", "index", "ptr", "end", "mixed", "do"])
    step = rng.choice([1, 1, 2, -1]) if form != "end" else 1
    s = Shape(tuple(walks), form=form, trip=rng.choice(["n", "const", "n-k", "n+k"]), step=step,
              use=rng.choice(["sum", "store", "copy", "mixed"]), call=rng.random() < 0.15,
              outer=rng.random() < 0.1)
    if form == "end" and any(w.index[0] == "rev" for w in walks[:1]):
        s = replace(s, form="ptr")
    extras = tuple(pick(rng) for pick in rng.sample(TRANSFORMERS, rng.randint(0, 3)))
    return replace(s, extras=extras)


def cases(quick: bool = False, seed: int = SEED, count: int = COUNT) -> list:
    rng = random.Random(seed)
    out = []
    for k in range(count if not quick else count // 50):
        name = f"rnd{seed}_{k:04d}"
        if rng.random() < 0.5:
            one = knobs(rng)
            out.append(cross.Builder(one).build(name).with_(family="fuzz"))
        else:
            one = shape(rng)
            case = concurrent.build(one, name=name)
            out.append(case.with_(family="fuzz"))
    return out
