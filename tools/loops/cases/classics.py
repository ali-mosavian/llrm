"""The named classics: easy anchors every change should keep passing."""

from __future__ import annotations

from spec import (
    I8, U8, I16, U16, I32, U32, Array, Case, Input, Fill, Const, Var, Bin, Cast, Cmp, Logic, Load, Assign, For,
    While, If, Return, c, v, add, sub, mul, ld,
)
from common import trips

N = 300  # room for every edge trip up to 256
SMALL = dict(lo=-1000, span=2001)


def _inputs(arrays, rows, fill=None, pokes=None) -> tuple[Input, ...]:
    out = []
    for at, row in enumerate(rows):
        fills = []
        for k, a in enumerate(arrays):
            f = (fill or {}).get(a.name, SMALL)
            fills.append((a.name, Fill(seed=7 + 97 * at + 13 * k, **f)))
        out.append(Input(tuple(row), tuple(fills), tuple(pokes(row) if pokes else ())))
    return tuple(out)


def count(var, end, body, start=c(0), step=c(1), cond="<"):
    return For(var, start, cond, end, step, tuple(body))


def plus(place, value):
    return Assign(place, add(place, value))


def i32(e):
    return Cast(e, I32)


def dot() -> Case:
    a, b = Array("a", I16, (N,)), Array("b", I16, (N,))
    body = (Assign(v("s"), c(0, I32)),
            count("i", v("n"), [plus(v("s"), mul(i32(ld("a", v("i"))), i32(ld("b", v("i")))))]),
            Return(v("s")))
    return Case("dot", "classic", (("n", I16),), (a, b), (("i", I16), ("s", I32)), body, I32,
                _inputs((a, b), [(t,) for t in trips(N)]))


def saxpy() -> Case:
    a, b = Array("a", I16, (N,)), Array("b", I16, (N,))
    body = (count("i", v("n"), [Assign(ld("a", v("i")), add(ld("a", v("i")), mul(v("k"), ld("b", v("i")))))]),
            Return(c(0, I32)))
    return Case("saxpy", "classic", (("n", I16), ("k", I16)), (a, b), (("i", I16),), body, I32,
                _inputs((a, b), [(t, 7) for t in trips(N)] + [(256, -9)]))


def copy(elem, name) -> Case:
    d, s = Array("d", elem, (N,)), Array("s", elem, (N,))
    body = (count("i", v("n"), [Assign(ld("d", v("i")), ld("s", v("i")))]), Return(c(0, I32)))
    fill = {"d": {}, "s": {}}
    return Case(name, "classic", (("n", I16),), (d, s), (("i", I16),), body, I32,
                _inputs((d, s), [(t,) for t in trips(N)], fill))


def setmem(elem, name) -> Case:
    d = Array("d", elem, (N,))
    body = (count("i", v("n"), [Assign(ld("d", v("i")), v("x"))]), Return(c(0, I32)))
    return Case(name, "classic", (("n", I16), ("x", elem)), (d,), (("i", I16),), body, I32,
                _inputs((d,), [(t, 77) for t in trips(N)], {"d": {}}))


def strlen(elem, name) -> Case:
    s = Array("s", elem, (N,))
    body = (Assign(v("i"), c(0)),
            While(Cmp("!=", ld("s", v("i")), c(0, elem)), (plus(v("i"), c(1)),)),
            Return(i32(v("i"))))
    return Case(name, "classic", (("t", I16),), (s,), (("i", I16),), body, I32,
                _inputs((s,), [(t,) for t in trips(N - 1)], {"s": dict(lo=1, span=200)},
                        pokes=lambda row: [("s", row[0], 0)]))


def reverse() -> Case:
    a = Array("a", I16, (N,))
    j = sub(sub(v("n"), c(1)), v("i"))
    body = (count("i", Bin("/", v("n"), c(2)), [
        Assign(v("t"), ld("a", v("i"))),
        Assign(ld("a", v("i")), ld("a", j)),
        Assign(ld("a", j), v("t")),
    ]), Return(c(0, I32)))
    return Case("reverse", "classic", (("n", I16),), (a,), (("i", I16), ("t", I16)), body, I32,
                _inputs((a,), [(t,) for t in trips(N)]))


def prefix() -> Case:
    a = Array("a", I32, (N,))
    body = (count("i", v("n"), [Assign(ld("a", v("i")), add(ld("a", v("i")), ld("a", sub(v("i"), c(1)))))],
                  start=c(1)),
            Return(c(0, I32)))
    return Case("prefix", "classic", (("n", I16),), (a,), (("i", I16),), body, I32,
                _inputs((a,), [(t,) for t in trips(N)]))


def histogram() -> Case:
    a, h = Array("a", I16, (N,)), Array("h", I16, (16,))
    slot = Bin("&", ld("a", v("i")), c(15))
    body = (count("i", v("n"), [Assign(ld("h", slot), add(ld("h", slot), c(1)))]), Return(c(0, I32)))
    return Case("histogram", "classic", (("n", I16),), (a, h), (("i", I16),), body, I32,
                _inputs((a, h), [(t,) for t in trips(N)], {"a": {}, "h": dict(lo=0, span=10)}))


M = 16


def matmul_ijk() -> Case:
    a, b, cc = Array("a", I16, (M, M)), Array("b", I16, (M, M)), Array("c", I32, (M, M))
    body = (count("i", v("n"), [count("j", v("n"), [
        Assign(v("s"), c(0, I32)),
        count("k", v("n"), [plus(v("s"), mul(i32(ld("a", v("i"), v("k"))), i32(ld("b", v("k"), v("j")))))]),
        Assign(ld("c", v("i"), v("j")), v("s")),
    ])]), Return(c(0, I32)))
    return Case("matmul_ijk", "classic", (("n", I16),), (a, b, cc), (("i", I16), ("j", I16), ("k", I16), ("s", I32)),
                body, I32, _inputs((a, b, cc), [(t,) for t in (0, 1, 2, 3, 15, 16)]))


def matmul_ikj() -> Case:
    a, b, cc = Array("a", I16, (M, M)), Array("b", I16, (M, M)), Array("c", I32, (M, M))
    body = (count("i", v("n"), [count("k", v("n"), [
        Assign(v("r"), i32(ld("a", v("i"), v("k")))),
        count("j", v("n"), [Assign(ld("c", v("i"), v("j")),
                                   add(ld("c", v("i"), v("j")), mul(v("r"), i32(ld("b", v("k"), v("j"))))))]),
    ])]), Return(c(0, I32)))
    return Case("matmul_ikj", "classic", (("n", I16),), (a, b, cc), (("i", I16), ("j", I16), ("k", I16), ("r", I32)),
                body, I32, _inputs((a, b, cc), [(t,) for t in (0, 1, 2, 3, 15, 16)]))


def transpose() -> Case:
    a, b = Array("a", I16, (M, M)), Array("b", I16, (M, M))
    body = (count("i", v("n"), [count("j", v("n"), [Assign(ld("b", v("j"), v("i")), ld("a", v("i"), v("j")))])]),
            Return(c(0, I32)))
    return Case("transpose", "classic", (("n", I16),), (a, b), (("i", I16), ("j", I16)), body, I32,
                _inputs((a, b), [(t,) for t in (0, 1, 2, 3, 15, 16)]))


def convolution() -> Case:
    a, o = Array("a", I16, (N + 2,)), Array("o", I32, (N,))
    tap = lambda k, w: mul(i32(ld("a", add(v("i"), c(k)))), i32(v(w)))  # noqa: E731
    body = (count("i", v("n"), [Assign(ld("o", v("i")), add(add(tap(0, "k0"), tap(1, "k1")), tap(2, "k2")))]),
            Return(c(0, I32)))
    return Case("convolution", "classic", (("n", I16), ("k0", I16), ("k1", I16), ("k2", I16)), (a, o), (("i", I16),),
                body, I32, _inputs((a, o), [(t, 3, -5, 2) for t in trips(N)]))


def stencil() -> Case:
    a, o = Array("a", I16, (M, M)), Array("o", I16, (M, M))
    at = lambda di, dj: ld("a", add(v("i"), c(di)), add(v("j"), c(dj)))  # noqa: E731
    value = sub(add(add(at(-1, 0), at(1, 0)), add(at(0, -1), at(0, 1))), mul(c(4), at(0, 0)))
    body = (count("i", sub(v("n"), c(1)), [count("j", sub(v("n"), c(1)), [Assign(ld("o", v("i"), v("j")), value)],
                                                  start=c(1))], start=c(1)),
            Return(c(0, I32)))
    return Case("stencil", "classic", (("n", I16),), (a, o), (("i", I16), ("j", I16)), body, I32,
                _inputs((a, o), [(t,) for t in (2, 3, 4, 15, 16)]))


def horner() -> Case:
    cf = Array("cf", I16, (N,))
    body = (Assign(v("s"), c(0, I32)),
            count("i", v("n"), [Assign(v("s"), Bin("%", add(mul(v("s"), v("x")), i32(ld("cf", v("i")))), c(10007, I32)))]),
            Return(v("s")))
    return Case("horner", "classic", (("n", I16), ("x", I32)), (cf,), (("i", I16), ("s", I32)), body, I32,
                _inputs((cf,), [(t, 3) for t in trips(N)]))


def bubble() -> Case:
    a = Array("a", I16, (N,))
    j1 = add(v("j"), c(1))
    body = (count("j", sub(sub(v("n"), c(1)), v("p")), [
        If(Cmp(">", ld("a", v("j")), ld("a", j1)), (
            Assign(v("t"), ld("a", v("j"))), Assign(ld("a", v("j")), ld("a", j1)), Assign(ld("a", j1), v("t")))),
    ]), Return(c(0, I32)))
    return Case("bubble", "classic", (("n", I16), ("p", I16)), (a,), (("j", I16), ("t", I16)), body, I32,
                _inputs((a,), [(t, 0) for t in trips(N)] + [(256, 100)]))


def bsearch() -> Case:
    """Not affine: the negative control, which must stay correct."""
    a = Array("a", I16, (N,))
    mid = Bin("/", add(v("lo"), v("hi")), c(2))
    body = (Assign(v("lo"), c(0)), Assign(v("hi"), v("n")),
            While(Cmp("<", v("lo"), v("hi")), (
                Assign(v("m"), mid),
                If(Cmp("<", ld("a", v("m")), v("key")), (Assign(v("lo"), add(v("m"), c(1))),),
                   (Assign(v("hi"), v("m")),)))),
            Return(i32(v("lo"))))
    rows = [(t, key) for t in trips(N) for key in (-5, 3 * t // 2)]
    return Case("bsearch", "classic", (("n", I16), ("key", I16)), (a,), (("lo", I16), ("hi", I16), ("m", I16)), body,
                I32, _inputs((a,), rows, {"a": dict(lo=0, step=3)}))


def strcmp() -> Case:
    a, b = Array("a", I16, (N,)), Array("b", I16, (N,))
    body = (Assign(v("i"), c(0)),
            While(Logic("and", Cmp("==", ld("a", v("i")), ld("b", v("i"))), Cmp("!=", ld("a", v("i")), c(0))),
                  (plus(v("i"), c(1)),)),
            Return(i32(sub(ld("a", v("i")), ld("b", v("i"))))))
    same = dict(lo=1, span=200)
    rows = [(t, d) for t in trips(N - 1) for d in (0, 1)]
    out = []
    for at, (t, differ) in enumerate(rows):
        fills = (("a", Fill(seed=5 + at, **same)), ("b", Fill(seed=5 + at, **same)))
        pokes = [("a", t, 0), ("b", t, 0)] if not differ else [("a", t, 0), ("b", t, 9)]
        out.append(Input((t, differ), fills, tuple(pokes)))
    return Case("strcmp", "classic", (("t", I16), ("d", I16)), (a, b), (("i", I16),), body, I32, tuple(out))


def checksum() -> Case:
    a = Array("a", I16, (N,))
    body = (Assign(v("s"), c(0, I32)),
            count("i", v("n"), [Assign(v("s"), Bin("%", add(mul(v("s"), c(31, I32)), i32(ld("a", v("i")))),
                                                   c(65521, I32)))]),
            Return(v("s")))
    return Case("checksum", "classic", (("n", I16),), (a,), (("i", I16), ("s", I32)), body, I32,
                _inputs((a,), [(t,) for t in trips(N)]))


def fixscale() -> Case:
    a = Array("a", I32, (N,))
    body = (count("i", v("n"), [Assign(ld("a", v("i")), Bin("/", mul(ld("a", v("i")), v("k")), c(256, I32)))]),
            Return(c(0, I32)))
    return Case("fixscale", "classic", (("n", I16), ("k", I32)), (a,), (("i", I16),), body, I32,
                _inputs((a,), [(t, 181) for t in trips(N)], {"a": dict(lo=0, span=60000)}))


def cases() -> list[Case]:
    return [
        dot(), saxpy(), copy(U8, "memcpy"), copy(I16, "wordcopy"), setmem(U8, "memset"), setmem(I16, "wordset"),
        strlen(U8, "strlen"), strlen(I16, "wordlen"), reverse(), prefix(), histogram(), matmul_ijk(), matmul_ikj(),
        transpose(), convolution(), stencil(), horner(), bubble(), bsearch(), strcmp(), checksum(), fixscale(),
    ]
