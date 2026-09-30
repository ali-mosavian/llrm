"""
`cross`: the dimensions of a loop, crossed.

One base loop, `for (i = start; i cond end; i += step) s += a[f(i)]`, and a
Knobs value per dimension. `cases()` takes every pair of dimensions and every
pair of their values, the rest at their defaults, so each pair of values is
seen together at least once; combinations a language or the loop cannot mean
are left out by `valid`, not special-cased in the runner.
"""

from __future__ import annotations

import itertools
from dataclasses import dataclass, replace, fields

from spec import (
    I8, U8, I16, U16, I32, U32, F64, Struct, Array, Case, Input, Fill, Const, Var, Bin, Neg, Cast, Cmp, Logic, Load,
    AddrOf, PtrAdd, Deref, Len, Ptr, Assign, For, While, DoWhile, If, CallS, Return, Break, Continue, c, v, add, sub, mul,
)

S3 = Struct("s3", (("x", I16), ("c", U8)))
S5 = Struct("s5", (("x", I16), ("y", I16), ("c", U8)))
S6 = Struct("s6", (("x", I16), ("y", I16), ("z", I16)))
S10 = Struct("s10", (("x", I16), ("y", I32), ("z", I32)))
S12 = Struct("s12", (("x", I32), ("y", I32), ("z", I16), ("w", I16)))
ELEMS = {"u8": U8, "i16": I16, "i32": I32, "f64": F64, "s3": S3, "s5": S5, "s6": S6, "s10": S10, "s12": S12}
COUNTERS = {"i8": I8, "u8": U8, "i16": I16, "u16": U16, "i32": I32}


@dataclass(frozen=True)
class Knobs:
    trip: str = "n"  # n, const, n-1, n+k, len, n*m, data, zero
    cond: str = "<"  # < <= != > >=
    counter: str = "i16"
    start: str = "0"  # 0, const, sym
    step: str = "1"  # 1 2 3 -1 -2 sym
    elem: str = "i16"
    field: str = "x"  # x, y: which field of a struct element
    storage: str = "param"  # param, global, local, far
    walk: str = "1d"  # 1d, rows (a[k][i]), cols (a[i][k])
    use: str = "addr"  # addr value store compare scale2 scale3 scale5 scale7 scalem1 scalesym shift square half mod accum
    shape: str = "plain"  # plain nest2 nest3 tri seq break continue exits return call condstore minmax do live changed
    pair: str = "none"  # none, j+=2, j=3i+1, offset


DIMENSIONS = {
    "trip": ["n", "const", "n-1", "n+k", "len", "n*m", "data", "zero"],
    "cond": ["<", "<=", "!=", ">", ">="],
    "counter": list(COUNTERS),
    "start": ["0", "const", "sym"],
    "step": ["1", "2", "3", "-1", "-2", "sym"],
    "elem": list(ELEMS),
    "field": ["x", "y"],
    "storage": ["param", "global", "local", "far"],
    "walk": ["1d", "rows", "cols"],
    "use": ["addr", "value", "store", "compare", "scale2", "scale3", "scale5", "scale7", "scalem1", "scalesym", "shift",
            "square", "half", "mod", "accum"],
    "shape": ["plain", "nest2", "nest3", "tri", "seq", "break", "continue", "exits", "return", "call", "condstore",
              "minmax", "do", "live", "changed"],
    "pair": ["none", "j+=2", "j=3i+1", "offset"],
}
NONAFFINE = {"square", "half", "mod", "accum"}
TRIPS = (0, 1, 2, 3, 15, 16, 17, 100)
ROWS = 4


def down(k: Knobs) -> bool:
    return k.cond in (">", ">=")


def valid(k: Knobs) -> bool:
    step = k.step
    if down(k) != step.startswith("-"):
        # a symbolic step counts up; > and >= count down
        if not (step == "sym" and not down(k)):
            return False
    if step == "sym" and down(k):
        return False
    if k.cond == "!=" and step not in ("1", "-1"):
        return False  # != with a larger step can pass its end
    if k.field == "y" and k.elem not in ("s5", "s6", "s10", "s12"):
        return False
    if k.cond == "!=" and (k.trip in ("n-1", "zero") or k.shape == "tri"):
        return False  # != needs its end at or past its start
    if not COUNTERS[k.counter].signed and k.trip == "n-1":
        return False  # n = 0 makes the end 65535: past the array
    if k.trip == "data" and k.walk != "1d":
        return False
    if (k.trip == "data" or k.shape == "changed") and k.shape == "continue":
        return False  # the counter steps at the body's end, which continue skips
    counter = COUNTERS[k.counter]
    if counter.bits == 8 and k.trip in ("n*m", "n+k"):
        return False
    if not counter.signed and down(k) and (k.cond == ">=" or k.step != "-1"):
        return False  # i >= 0 never fails unsigned; a step of -2 can pass 0
    if k.walk != "1d" and k.elem not in ("i16", "i32", "u8"):
        return False
    if k.trip == "data" and (down(k) or k.use in ("store",)):
        return False
    return True


class Builder:
    """The case for one Knobs."""

    def __init__(self, k: Knobs):
        self.k = k
        self.counter = COUNTERS[k.counter]
        self.elem = ELEMS[k.elem]

    def one(self, value: int) -> Const:
        return Const(value, self.counter)

    def step_value(self):
        k = self.k
        return v("st") if k.step == "sym" else self.one(int(k.step))

    def index(self, i):
        """The element the trip reads, from the counter (an I16 index)."""
        k = self.k
        x = Cast(i, I16) if self.counter != I16 else i
        use = k.use
        scale = {"scale2": 2, "scale3": 3, "scale5": 5, "scale7": 7}.get(use)
        if scale:
            return mul(c(scale), x)
        if use == "scalem1":
            return sub(c(SPAN - 1), x)
        if use == "scalesym":
            return mul(x, v("m"))
        if use == "shift":
            return Bin("<<", x, c(1))
        if use == "square":
            return Bin("%", mul(x, x), c(SPAN))
        if use == "half":
            return Bin("/", x, c(2))
        if use == "mod":
            return Bin("%", x, v("q"))
        return x

    def build(self, name: str) -> Case:
        k = self.k
        elem = self.elem
        counter = self.counter
        rows = ROWS if k.walk != "1d" else 1
        dims = (SPAN,) if k.walk == "1d" else (SPAN, SPAN) if k.walk == "cols" else (ROWS, SPAN)
        if k.walk == "cols":
            dims = (SPAN, ROWS)
        ptr = "far" if k.storage == "far" else "near"
        where = "param" if k.storage == "far" else k.storage
        a = Array("a", elem, dims, where, ptr)
        out = Array("o", I32, (SPAN,), "global")
        arrays = [a, out] if k.use == "store" or k.shape == "condstore" else [a]
        params = [("n", counter), ("b", counter), ("st", counter), ("m", I16), ("q", I16)]
        locals_ = [("i", counter), ("s", I32), ("j", I32), ("t", I32), ("r", I16), ("x", counter), ("e", counter)]

        def at(i, row=None):
            idx = self.index(i)
            if k.walk == "1d":
                load = Load("a", (idx,))
            elif k.walk == "rows":
                load = Load("a", (row, idx))
            else:
                load = Load("a", (idx, row))
            if isinstance(elem, Struct):
                load = Load(load.array, load.index, k.field)
            return load

        def value(load):
            return Cast(load, I32)

        start = {"0": self.one(0), "const": self.one(3), "sym": v("b")}[k.start if k.trip != "zero" else "sym"]
        end = self.end()
        step = self.step_value()
        if down(k):
            start, end = self.down_bounds(start, end)
        i = v("i")
        row = v("x") if k.walk != "1d" else None
        body = self.body(at(i, row), value, i)
        loop = self.loop(start, end, step, body, rows)
        stmts = [Assign(v("s"), c(0, I32)), Assign(v("j"), c(0, I32)), Assign(v("t"), c(0, I32)),
                 Assign(v("i"), self.one(0)), Assign(v("x"), self.one(0)), *loop]
        result = add(add(v("s"), v("j")), v("t"))
        if k.shape == "live":
            result = add(result, Cast(v("i"), I32))
        stmts.append(Return(result))
        inputs = self.inputs(arrays)
        tags = frozenset(f"{f.name}:{getattr(k, f.name)}" for f in fields(k))
        return Case(name, "cross", tuple(params), tuple(arrays), tuple(locals_), tuple(stmts), I32, inputs, tags=tags,
                    note=repr(k))

    def end(self):
        k = self.k
        n = v("n")
        return {
            "n": n, "const": self.one(40), "n-1": sub(n, self.one(1)), "n+k": add(n, self.one(3)),
            "len": Cast(sub(Len("a", 0 if k.walk != "rows" else 1), c(SPAN - 100)), self.counter),
            "n*m": mul(n, Cast(v("m"), self.counter)), "data": n, "zero": n,
        }[k.trip]

    def down_bounds(self, start, end):
        """Counting down over the same range: from end while i > start, or
        from end - 1 while i >= start."""
        if self.k.cond == ">":
            return end, start
        return sub(end, self.one(1)), start

    def body(self, load, value, i) -> list:
        k = self.k
        use = k.use
        out = []
        if use == "value":
            out.append(Assign(v("s"), add(v("s"), add(value(load), Cast(i, I32)))))
        elif use == "store":
            out.append(Assign(Load("o", (Cast(i, I16) if self.counter != I16 else i,)), value(load)))
        elif use == "compare":
            out.append(If(Cmp(">", value(load), Cast(i, I32)), (Assign(v("s"), add(v("s"), c(1, I32))),)))
        elif use == "accum":
            out.append(Assign(v("t"), add(v("t"), Cast(i, I32))))
            out.append(Assign(v("s"), add(v("s"), value(load))))
        else:
            out.append(Assign(v("s"), add(v("s"), value(load))))
        pair = k.pair
        if pair == "j+=2":
            out.append(Assign(v("j"), add(v("j"), c(2, I32))))
        elif pair == "j=3i+1":
            out.append(Assign(v("j"), add(mul(c(3, I32), Cast(i, I32)), c(1, I32))))
        elif pair == "offset":
            out.append(Assign(v("j"), add(v("j"), Cast(add(Cast(i, I16), c(5)), I32))))
        shape = k.shape
        if shape == "break":
            out.insert(0, If(Cmp("==", value(load), c(7, I32)), (Break(),)))
        elif shape == "continue":
            out.insert(0, If(Cmp("==", Bin("&", value(load), c(3, I32)), c(1, I32)), (Continue(),)))
        elif shape == "exits":
            out.insert(0, If(Cmp("==", value(load), c(7, I32)), (Break(),)))
            out.append(If(Cmp(">", v("s"), c(900000, I32)), (Break(),)))
        elif shape == "return":
            out.append(If(Cmp("==", value(load), c(9, I32)), (Return(v("s")),)))
        elif shape == "call":
            out.insert(0, CallS("touch"))
        elif shape == "condstore":
            out.append(If(Cmp(">", value(load), c(0, I32)), (Assign(Load("o", (c(3),)), v("s")),)))
        elif shape == "minmax":
            out.append(If(Cmp(">", value(load), v("t")), (Assign(v("t"), value(load)),)))
        elif shape == "changed":
            out.append(If(Cmp("==", value(load), c(5, I32)), (Assign(v("i"), add(v("i"), self.step_value())),)))
        return out

    def loop(self, start, end, step, body, rows) -> list:
        k = self.k
        i = v("i")
        shape = k.shape
        counted = For("i", start, k.cond, end, step, tuple(body))
        if shape == "changed" or k.trip == "data":
            test = Cmp(k.cond, i, end)
            if k.trip == "data":
                test = Logic("and", test, Cmp("!=", self.data_load(i), Const(0, self.elem if not isinstance(self.elem, Struct) else I16)))
            counted = While(test, (*body, Assign(i, add(i, step))))
            core = [Assign(i, start), counted]
        elif shape == "do":
            core = [If(Cmp(k.cond, start, end), (Assign(i, start), DoWhile((*body, Assign(i, add(i, step))),
                                                                          Cmp(k.cond, i, end))))]
        else:
            core = [counted]
        if shape == "tri":
            core = [For("e", self.one(0), "<", self.one(3), self.one(1),
                        (For("i", v("e"), k.cond, end, step, tuple(body)),))] if not down(k) else core
        if shape in ("nest2", "nest3") or k.walk != "1d":
            core = [For("x", self.one(0), "<", self.one(rows if k.walk != "1d" else 3), self.one(1), tuple(core))]
            if shape == "nest3":
                core = [For("e", self.one(0), "<", self.one(2), self.one(1), tuple(core))]
        if shape == "seq":
            core = core + core
        return core

    def data_load(self, i):
        load = Load("a", (Cast(i, I16) if self.counter != I16 else i,))
        return Load("a", load.index, "x") if isinstance(self.elem, Struct) else load

    def inputs(self, arrays) -> tuple[Input, ...]:
        k = self.k
        rows = []
        for t in TRIPS:
            if k.trip == "zero":
                rows.append((1, 5, 1, 2, 3))  # start above end
                continue
            n = t
            if k.trip == "n*m":
                n = max(t // 2, 0)
            if k.start != "0" and not down(k):
                n = n + 3
            if k.step == "sym":
                rows.append((n, 3, 2, 2, 3))
            else:
                rows.append((n, 3, 1, 2, 3))
        if k.trip in ("const", "len", "zero"):
            rows = rows[:2]
        out = []
        for at, row in enumerate(rows):
            fills = tuple((a.name, Fill(seed=5 + 17 * at + 3 * j, lo=1 if k.trip == "data" else -100, span=200))
                          for j, a in enumerate(arrays))
            pokes = (("a", min(row[0], SPAN - 1) // 2, 0),) if k.trip == "data" and k.walk == "1d" and \
                not isinstance(self.elem, Struct) else ()
            out.append(Input(row, fills, pokes))
        return tuple(out)


SPAN = 760  # elements: room for 7 x 103 and the margins


def name_of(k: Knobs, base: Knobs) -> str:
    parts = [f"{f.name}{getattr(k, f.name)}" for f in fields(k) if getattr(k, f.name) != getattr(base, f.name)]
    text = "x_" + ("_".join(parts) or "base")
    for a, b in (("<=", "le"), (">=", "ge"), ("!=", "ne"), ("<", "lt"), (">", "gt"), ("+=", "pe"), ("=", "eq"),
                 ("*", "x"), ("+", "p"), ("-", "m")):
        text = text.replace(a, b)
    return text


def cases(quick: bool = False) -> list[Case]:
    base = Knobs()
    seen, out = set(), []
    for d1, d2 in itertools.combinations(DIMENSIONS, 2):
        for v1 in DIMENSIONS[d1]:
            for v2 in DIMENSIONS[d2]:
                k = replace(base, **{d1: v1, d2: v2})
                if not valid(k):
                    # the loop direction follows the condition: try its partner
                    k2 = replace(k, step="-1") if down(k) and not k.step.startswith("-") else k
                    k2 = replace(k2, cond=">" if k2.step.startswith("-") and not down(k2) else k2.cond)
                    if not valid(k2):
                        continue
                    k = k2
                if k in seen:
                    continue
                seen.add(k)
                out.append(Builder(k).build(name_of(k, base)))
    if quick:
        out = out[::25]
    return out
