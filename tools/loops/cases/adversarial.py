"""
`adversarial`: loops that must stay correct whatever a pass decides, and the
pressure ladder.

- ladder: k arrays and m accumulators, k 1..7 and m 0..6, past the registers;
  quality must degrade to the references' level, never miscompile.
- chase: an index loaded from memory (a linked walk): not affine.
- wrap: an unsigned counter through 65535 to 0, an 8-bit one through its
  range, a start above the end.
- alias: one array passed as two parameters, read at i and written at i+1.
- single: BASIC's default SINGLE counter indexing an array.
"""

from __future__ import annotations

from spec import (
    I8, U8, I16, U16, I32, F32, Array, Case, Input, Fill, Const, Var, Bin, Cast, Cmp, Load, Assign, For, While, If,
    Return, c, v, add, sub, mul,
)

N = 300


def _inputs(arrays, rows, fill=None):
    return tuple(
        Input(tuple(row), tuple((a.name, Fill(seed=9 + 41 * at + 5 * k, **(fill or dict(lo=-500, span=1001))))
                                for k, a in enumerate(arrays) if not a.alias))
        for at, row in enumerate(rows)
    )


def ladder() -> list[Case]:
    out = []
    for k in range(1, 8):
        for m in range(0, 7):
            arrays = tuple(Array(f"a{x}", I16, (N,)) for x in range(k))
            locals_ = [("i", I16), ("s", I32), *((f"t{y}", I32) for y in range(m))]
            body = [Assign(v("s"), add(v("s"), Cast(Load(f"a{x}", (v("i"),)), I32))) for x in range(k)]
            for y in range(m):
                # each accumulator a different function of the loads, so none folds into another
                source = Load(f"a{y % k}", (v("i"),))
                body.append(Assign(v(f"t{y}"), add(v(f"t{y}"), mul(Cast(source, I32), c(y + 2, I32)))))
            result = v("s")
            for y in range(m):
                result = add(result, v(f"t{y}"))
            stmts = (Assign(v("s"), c(0, I32)), *(Assign(v(f"t{y}"), c(0, I32)) for y in range(m)),
                     For("i", c(0), "<", v("n"), c(1), tuple(body)), Return(result))
            out.append(Case(f"ladder{k}x{m}", "adversarial", (("n", I16),), arrays, tuple(locals_), stmts, I32,
                            _inputs(arrays, [(t,) for t in (0, 1, 17, 256)]),
                            tags=frozenset({f"arrays:{k}", f"accumulators:{m}", "shape:ladder"})))
    return out


def chase() -> Case:
    nxt, a = Array("nxt", I16, (N,)), Array("a", I16, (N,))
    body = (Assign(v("s"), add(v("s"), Cast(Load("a", (v("j"),)), I32))),
            Assign(v("j"), Bin("%", Load("nxt", (v("j"),)), c(N))))
    stmts = (Assign(v("s"), c(0, I32)), Assign(v("j"), c(0)), For("i", c(0), "<", v("n"), c(1), body), Return(v("s")))
    return Case("chase", "adversarial", (("n", I16),), (nxt, a), (("i", I16), ("j", I16), ("s", I32)), stmts, I32,
                _inputs((nxt, a), [(t,) for t in (0, 1, 17, 256, 1000)], dict(lo=0, span=N)),
                tags=frozenset({"use:chase"}))


def wrap16() -> Case:
    """u16 i from b while i != e: through 65535 and 0 when b > e."""
    a = Array("a", U8, (256,))
    body = (Assign(v("s"), add(v("s"), Cast(Load("a", (Bin("&", v("i"), Const(255, U16)),)), I32))),)
    stmts = (Assign(v("s"), c(0, I32)), For("i", v("b"), "!=", v("e"), Const(1, U16), body), Return(v("s")))
    rows = [(0, 0), (5, 10), (65530, 6), (65535, 0), (40000, 39999), (1, 0)]
    return Case("wrap16", "adversarial", (("b", U16), ("e", U16)), (a,), (("i", U16), ("s", I32)), stmts, I32,
                _inputs((a,), rows, {}), tags=frozenset({"counter:u16", "trip:wrap"}))


def wrap8() -> Case:
    """An i8 counter across its range, indexing with an offset."""
    a = Array("a", I16, (256,))
    idx = add(Cast(v("i"), I16), c(128))
    body = (Assign(v("s"), add(v("s"), Cast(Load("a", (idx,)), I32))),)
    stmts = (Assign(v("s"), c(0, I32)), For("i", v("b"), "<", v("e"), Const(1, I8), body), Return(v("s")))
    rows = [(-128, 127), (-5, 5), (100, -100), (0, 0), (-128, -127)]
    return Case("wrap8", "adversarial", (("b", I8), ("e", I8)), (a,), (("i", I8), ("s", I32)), stmts, I32,
                _inputs((a,), rows), tags=frozenset({"counter:i8"}))


def alias() -> Case:
    """a and b are one array: b[i+1] = a[i] + 1 carries through every trip."""
    a = Array("a", I16, (N,))
    b = Array("b", I16, (N,), alias="a")
    body = (Assign(Load("b", (add(v("i"), c(1)),)), add(Load("a", (v("i"),)), c(1))),)
    stmts = (For("i", c(0), "<", v("n"), c(1), body), Return(c(0, I32)))
    return Case("alias", "adversarial", (("n", I16),), (a, b), (("i", I16),), stmts, I32,
                _inputs((a, b), [(t,) for t in (0, 1, 2, 17, 256)]), tags=frozenset({"arrays:alias"}))


def single() -> Case:
    """BASIC's default SINGLE counter indexing an INTEGER array."""
    a = Array("a", I16, (N,))
    body = (Assign(v("s"), add(v("s"), Cast(Load("a", (Cast(v("i"), I16),)), I32))),)
    stmts = (Assign(v("s"), c(0, I32)), For("i", Const(0, F32), "<", Cast(v("n"), F32), Const(1, F32), body),
             Return(v("s")))
    return Case("single", "adversarial", (("n", I16),), (a,), (("i", F32), ("s", I32)), stmts, I32,
                _inputs((a,), [(t,) for t in (0, 1, 17, 256)]), tags=frozenset({"counter:single"}))


def cases(quick: bool = False) -> list[Case]:
    rungs = ladder()
    if quick:
        rungs = rungs[::7]
    return [*rungs, chase(), wrap16(), wrap8(), alias(), single()]
