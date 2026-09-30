"""
`concurrent`: 1..12 arrays walked by one loop, the property a loop's
induction variables exist for.

A Shape says what each array's walk is (element, index form, where its base
lives, its start) and how the loop is written (form, trip, step, use, call).
`build` turns it into a case. Metamorphic variants are the same Shape with
`extras`, composable transformers applied in order, each judged against its
base's measurement, so no variant needs a count of its own.

Hand derivation, with no compiler: a walk's byte stride is element size x
index scale x step. Walks of one stride share one induction variable, as
many as the native address form can pair (PARTNERS register bases to an
index); a constant offset or start is a displacement; the exit test reuses
one. tests/test_loops.py checks this agrees with expect.py's derivation
from the spec.
"""

from __future__ import annotations

import random
from dataclasses import dataclass, replace

from spec import (
    I8, U8, I16, U16, I32, F64, F80, Struct, Array, Case, Input, Fill, Const, Var, Bin, Neg, Cast, Cmp, Load, AddrOf, PtrAdd,
    Deref, Len, Ptr, Assign, For, DoWhile, If, CallS, Return, Break, Continue, c, v, add, sub, mul,
)
from expect import PARTNERS

S6 = Struct("s6", (("x", I16), ("y", I16), ("z", I16)))
# 10 bytes as a struct: llrm-c converts long double to nothing (wccq E1090)
S10 = Struct("s10", (("x", I16), ("y", I32), ("z", I32)))
SIZES = {"equal": (I16,), "two": (I16, I32), "distinct": (U8, I16, I32, F64, S6, S10)}
EXTENT = 300  # elements an array holds past its largest start offset
TRIPS = (0, 1, 2, 3, 15, 16, 17, 255)


@dataclass(frozen=True)
class Walk:
    elem: object
    index: tuple = ("i",)  # ("i",) ("off", c) ("sym",) ("rev",) ("scale", k) ("stride",)
    where: str = "param"  # param, global, local
    ptr: str = "near"  # near, far
    start: int = 0  # a constant start offset, in elements
    same_as: int | None = None  # walks another walk's array


@dataclass(frozen=True)
class Shape:
    walks: tuple[Walk, ...]
    form: str = "index"  # index, ptr, end, mixed, do
    trip: str = "n"  # n, const, n-k, n+k, len
    step: int = 1  # 1, 2, -1
    use: str = "sum"  # sum, store, copy, mixed
    call: bool = False
    extras: tuple = ()  # metamorphic transformers, applied in order
    counter: object = I16  # the counter's type: the index width
    rows: tuple = ()  # trip counts, when not TRIPS
    margin: int = 8  # every index's start, so `off` can reach -7
    outer: bool = False  # inside a loop whose counter each walk's start takes (a + j*m)

    def strides(self) -> list[tuple]:
        """Per walk its byte stride per trip: (bytes,) or ("m", bytes)."""
        out = []
        for w in self.walks:
            size = w.elem.size
            kind = w.index[0]
            scale = {"rev": -1, "scale": w.index[1] if kind == "scale" else 1}.get(kind, 1)
            if kind == "stride":
                out.append(("m", size * self.step))
            else:
                out.append((size * scale * self.step,))
        return out


def hand_ivs(shape: Shape, lang: str) -> int:
    """The most induction variables the loop needs: per stride class, one per
    PARTNERS register bases. A global's base is a displacement, a local's is
    bp; a C near parameter holds its base in a register, and so does every
    far one (BASIC's descriptors and Nib's views are far)."""
    classes: dict[tuple, set] = {}
    for k, (w, stride) in enumerate(zip(shape.walks, shape.strides())):
        array = w.same_as if w.same_as is not None else k
        needs = w.where == "param"
        classes.setdefault(stride, set())
        if needs:
            classes[stride].add(array)
    return sum(max(1, -(-len(bases) // PARTNERS)) for bases in classes.values())


def _name(shape: Shape) -> str:
    kinds = "".join(sorted({f"{w.elem.size}" for w in shape.walks}))
    code = {"i": "i", "off": "o", "sym": "y", "rev": "r", "scale": "s", "stride": "t", "plusn": "n"}
    index = "".join(sorted({code[w.index[0]] + (str(w.index[1]) if len(w.index) > 1 else "") for w in shape.walks}))
    bases = "".join(sorted({w.where[0] + w.ptr[0] for w in shape.walks}))
    extras = "".join("_" + "".join(str(p) for p in one) for one in shape.extras)
    extras += (f"_c{shape.counter.name}" if shape.counter != I16 else "") + ("_whole" if shape.rows else "") + \
        ("_outer" if shape.outer else "")
    if any(w.start for w in shape.walks):
        extras += "_at" + "x".join(str(w.start) for w in shape.walks)
    if any(w.same_as is not None for w in shape.walks):
        extras += "_on" + "x".join("-" if w.same_as is None else str(w.same_as) for w in shape.walks)
    return (f"conc{len(shape.walks)}_s{kinds}_x{index}_b{bases}_{shape.form}_{shape.trip}_st{shape.step}_{shape.use}"
            f"{'_call' if shape.call else ''}{extras}").replace("-", "m").replace("+", "p")


def build(shape: Shape, name: str | None = None, base: str | None = None, relation: str | None = None) -> Case:
    walks = shape.walks
    step = shape.step
    params = [("n", shape.counter), ("o", I16), ("m", I16)]
    one_ = lambda k: Const(k, shape.counter)  # noqa: E731
    trip = {
        "n": v("n"), "const": one_(200), "n-k": sub(v("n"), one_(3)), "n+k": add(v("n"), one_(3)),
        "len": sub(Len("a0"), c(16)),
    }[shape.trip]
    extras = dict((one[0], one) for one in shape.extras)
    shift = extras["shift"][1] if "shift" in extras else 0

    def index_of(w: Walk, i) -> object:
        kind = w.index[0]
        start = w.start + shift + shape.margin
        if kind == "i":
            e = i
        elif kind == "off":
            e = add(i, c(w.index[1]))
        elif kind == "sym":
            e = add(i, v("o"))
        elif kind == "plusn":
            e = add(i, v("n"))
        elif kind == "rev":
            e = sub(sub(trip, one_(1)), i)
        elif kind == "scale":
            e = mul(c(w.index[1]), i)
        else:
            e = mul(i, v("m"))
        if shape.outer:
            e = add(e, mul(v("k"), v("m")))
        return add(e, Const(start, shape.counter)) if start else e

    # extents: the largest index any input reaches, plus room
    arrays = []
    reach = []
    for k, w in enumerate(walks):
        kind = w.index[0]
        top = {"scale": w.index[1] if kind == "scale" else 1, "stride": 3}.get(kind, 1) * (max(TRIPS) + 3)
        top += {"sym": 40, "plusn": max(TRIPS) + 8}.get(kind, 0) + w.start + shift + shape.margin + 8 * bool(shape.margin)
        top += 3 * 3 if shape.outer else 0
        if shape.rows:
            top = max(shape.rows) + w.start + shift + shape.margin
        reach.append(top)
    owners = {}
    for k, w in enumerate(walks):
        owner = k
        while walks[owner].same_as is not None:
            owner = walks[owner].same_as
        owners[k] = owner
    if "store" in extras and extras["store"][1] == "apart":
        arrays.append(Array("g", I16, (8,), "global"))
    # one extent for all: `len` takes a0's, and every walk must fit it
    extent = max([EXTENT, *reach]) if not shape.rows else max(reach)
    for k, w in enumerate(walks):
        if owners[k] == k:
            # past a segment only a huge pointer reaches it all
            ptr = "huge" if extent * w.elem.size > 0x10000 and w.ptr == "far" else w.ptr
            arrays.append(Array(f"a{k}", w.elem, (extent,), w.where, ptr))
    name_of = lambda k: f"a{owners[k]}"  # noqa: E731
    pointers = shape.form in ("ptr", "end") or shape.form == "mixed"
    walked = [k for k in range(len(walks)) if shape.form in ("ptr", "end") or (shape.form == "mixed" and k % 2 == 0)]

    def coef(w: Walk):
        kind = w.index[0]
        if kind == "rev":
            return c(-step)
        if kind == "scale":
            return c(w.index[1] * step)
        if kind == "stride":
            return mul(v("m"), c(step))
        return c(step)

    pressure = extras["pressure"][1] if "pressure" in extras else 0
    local_types = [("i", shape.counter), ("s", I32), ("u", I32), ("j", I32), ("q", I32), ("d", I32), ("k", I16),
                   *((f"r{x}", I32) for x in range(pressure))]
    for k in walked:
        # a walk on another's array points as that array does
        local_types.append((f"p{k}", Ptr(walks[k].elem, next(a.ptr for a in arrays if a.name == name_of(k)))))
    if shape.form == "end":
        local_types.append(("e", Ptr(walks[0].elem, next(a.ptr for a in arrays if a.name == name_of(0)))))

    def access(k: int):
        w = walks[k]
        if k in walked:
            return Deref(v(f"p{k}"), c(0))
        return Load(name_of(k), (index_of(w, v("i")),))

    def elem_value(k: int):
        load = access(k)
        elem = walks[k].elem
        if isinstance(elem, Struct):
            field = Load(load.array, load.index, "x") if isinstance(load, Load) else Deref(load.ptr, load.at, "x")
            return Cast(field, I32)
        return Cast(load, I32)

    def place(k: int):
        load = access(k)
        elem = walks[k].elem
        if isinstance(elem, Struct):
            return (Load(load.array, load.index, "x") if isinstance(load, Load) else Deref(load.ptr, load.at, "x")), I16
        return load, elem

    uses = []
    reads = list(range(len(walks)))
    if shape.use in ("store", "copy") and walks:
        target, kind = place(0)
        reads = reads[1:]
        if shape.use == "copy" and len(walks) > 1:
            value = Cast(elem_value(1), kind)
            reads = reads[1:]
        elif len(walks) > 1:
            value = Cast(Bin("%", sum_of([elem_value(k) for k in reads]) if reads else v("i"), c(97, I32)), kind)
            reads = []
        else:
            # the end-pointer form has no counter to store
            value = Cast(v("i"), kind) if shape.form != "end" else Cast(c(7), kind)
        uses.append(Assign(target, value))
    if shape.use == "mixed":
        writes = [k for k in range(len(walks)) if k % 2 == 0]
        for k in writes:
            target, kind = place(k)
            uses.append(Assign(target, add(target, Const(1, kind))))
        reads = [k for k in range(len(walks)) if k % 2 == 1]
    for k in reads:
        uses.append(Assign(v("s"), add(v("s"), elem_value(k))))
    uses = _transform(uses, shape, elem_value, len(walks))
    for x in range(pressure):
        # unrelated values live across every use
        uses.insert(x * len(uses) // max(pressure, 1), Assign(v(f"r{x}"), add(v(f"r{x}"), Bin("^", v("u"), c(x + 1, I32)))))
    if "store" in extras:
        target = Load("g", (Bin("&", v("i"), Const(7, shape.counter)),)) if extras["store"][1] == "apart" else None
        if target is None and walks and not isinstance(walks[0].elem, Struct) and not isinstance(walks[0].elem, type(F64)):
            target = Load(name_of(0), (Const(3, shape.counter),))
        if target is not None:
            kind = I16 if extras["store"][1] == "apart" else walks[0].elem
            uses.insert(len(uses) // 2, Assign(target, Cast(v("u"), kind)))
    steps = []
    for k in walked:
        steps.append(Assign(v(f"p{k}"), PtrAdd(v(f"p{k}"), coef(walks[k]))))
    body = ([CallS("touch")] if shape.call else []) + uses + steps
    starts = []
    first = one_(0) if step > 0 else sub(trip, one_(1))
    for k in walked:
        starts.append(Assign(v(f"p{k}"), AddrOf(name_of(k), index_of(walks[k], first))))
    head = [Assign(v("s"), c(0, I32)), Assign(v("u"), c(1, I32)), Assign(v("j"), c(0, I32)), Assign(v("q"), c(0, I32)),
            Assign(v("i"), one_(0)), *(Assign(v(f"r{x}"), c(x, I32)) for x in range(pressure))]
    if shape.form == "end":
        end = Assign(v("e"), AddrOf(name_of(0), index_of(walks[0], trip)))
        loop = [end, *starts, _while_ptr(body, step)]
    elif shape.form == "do":
        test = Cmp("<", v("i"), trip) if step > 0 else Cmp(">=", v("i"), one_(0))
        guard = Cmp(">", trip, one_(0))
        loop = [If(guard, (Assign(v("i"), first), *starts, DoWhile(tuple(body + [Assign(v("i"), add(v("i"), one_(step)))]), test)))]
    else:
        if step > 0:
            loop = [*starts, For("i", one_(0), "<", trip, one_(step), tuple(body))]
        else:
            loop = [*starts, For("i", sub(trip, one_(1)), ">=", one_(0), one_(step), tuple(body))]
    result = add(add(v("s"), v("u")), add(v("j"), v("q")))
    for x in range(pressure):
        result = add(result, v(f"r{x}"))
    if "after" in extras:
        # the final counter, read after the exit
        result = add(result, Cast(v("i"), I32))
    if shape.outer:
        loop = [For("k", c(0), "<", c(3), c(1), tuple(loop))]
    stmts = (*head, *loop, Return(result))
    locals_ = tuple(local_types)
    rows = []
    for t in shape.rows or TRIPS:
        if shape.trip == "n-k":
            t += 3
        if shape.trip == "len":
            continue
        rows.append((t, 5, 2))
    if shape.trip in ("const", "len"):
        rows = [(0, 5, 2), (1, 7, 3)]
    inputs = tuple(
        Input(row, tuple((a.name, Fill(seed=3 + 31 * at + 7 * k, **_fill(a.elem))) for k, a in enumerate(arrays)))
        for at, row in enumerate(rows)
    )
    tags = {
        f"arrays:{len(walks)}", f"sizes:{'/'.join(sorted({str(w.elem.size) for w in walks}))}",
        *(f"index:{w.index[0]}" for w in walks), *(f"base:{w.where}-{w.ptr}" for w in walks), f"form:{shape.form}",
        f"trip:{shape.trip}", f"step:{shape.step}", f"use:{shape.use}", f"call:{shape.call}",
        *(f"relation:{one[0]}" for one in shape.extras),
    }
    if all(one[0] in SAME_SIZE for one in shape.extras) and shape.extras:
        tags.add("same-size")
    return Case(name or _name(shape), "concurrent", tuple(params), tuple(arrays), locals_, stmts, I32, inputs,
                tags=frozenset(tags), base=base, relation=relation, note=repr(shape))


def _fill(elem) -> dict:
    return {} if elem in (U8, I8) else dict(lo=-1000, span=2001)


def sum_of(values):
    total = values[0]
    for one in values[1:]:
        total = add(total, one)
    return total


def _while_ptr(body, step):
    from spec import While

    return While(Cmp("<" if step > 0 else ">", v("p0"), v("e")), tuple(body))


# --- metamorphic transformers ---------------------------------------------------
#
# Each is ("kind", *parameters), applied to the loop's uses. SAME_SIZE ones
# must leave the inner loop's instruction count alone; the rest add their own
# instructions, so only the induction-variable count is compared.

SAME_SIZE = {"permute", "shift", "dead"}


def _transform(uses: list, shape: Shape, elem_value, arrays: int) -> list:
    out = list(uses)
    for one in shape.extras:
        kind = one[0]
        rng = random.Random(repr(one))
        if kind == "permute":
            reads = [u for u in out if isinstance(u, Assign) and u.place == v("s")]
            rest = [u for u in out if u not in reads]
            rng.shuffle(reads)
            out = rest + reads
        elif kind == "insert":
            # a chain of arithmetic on an unrelated value, between the uses
            at = rng.randrange(len(out) + 1)
            chain = [Assign(v("u"), Bin("%", add(mul(v("u"), c(3, I32)), c(7, I32)), c(10007, I32)))]
            out = out[:at] + chain + out[at:]
        elif kind == "counter":
            # an unrelated counter, affine (j += 3) or not (q += i)
            if one[1] == "affine":
                out.append(Assign(v("j"), add(v("j"), c(3, I32))))
            else:
                out.append(Assign(v("q"), add(v("q"), Cast(v("i"), I32))))
        elif kind == "dup" and arrays:
            k = one[1] % arrays
            out.append(Assign(v("s"), add(v("s"), elem_value(k))))
        elif kind == "dead" and arrays:
            # a load nothing reads
            k = one[1] % arrays
            out.insert(0, Assign(v("d"), elem_value(k)))
        elif kind == "cond":
            # a use behind a branch: taken every other trip, or rarely
            reads = [u for u in out if isinstance(u, Assign) and u.place == v("s")]
            if reads:
                chosen = reads[one[1] % len(reads)]
                test = Cmp("!=", Bin("&", v("i"), c(1)), c(0)) if one[2] == "half" else Cmp("==", v("i"), c(100))
                out[out.index(chosen)] = If(test, (chosen,))
        elif kind == "exit":
            # a way out between the uses
            at = rng.randrange(len(out) + 1)
            way = one[1]
            if way == "continue" and shape.form in ("do", "end"):
                way = "break"  # these forms step at the body's end, which continue would skip
            stop = {"break": Break(), "continue": Continue(), "return": Return(v("s"))}[way]
            test = Cmp("==", Bin("&", v("i"), c(31)), c(17)) if way != "continue" else \
                Cmp("==", Bin("&", v("i"), c(3)), c(1))
            out = out[:at] + [If(test, (stop,))] + out[at:]
        elif kind == "callmid":
            at = rng.randrange(len(out) + 1)
            out = out[:at] + [CallS(one[1])] + out[at:]
        elif kind == "invariant":
            out.append(Assign(v("s"), add(v("s"), Cast(v("o"), I32))))
        elif kind == "both":
            # the same uses in both arms of a branch
            reads = [u for u in out if isinstance(u, Assign) and u.place == v("s")]
            if reads:
                test = Cmp("!=", Bin("&", v("i"), c(1)), c(0))
                rest = [u for u in out if u not in reads]
                out = rest + [If(test, tuple(reads), tuple(reversed(reads)))]
    return out


TRANSFORMERS = [
    lambda rng: ("permute", rng.randrange(1000)),
    lambda rng: ("insert", rng.randrange(1000)),
    lambda rng: ("counter", rng.choice(["affine", "other"])),
    lambda rng: ("dup", rng.randrange(12)),
    lambda rng: ("dead", rng.randrange(12)),
    lambda rng: ("cond", rng.randrange(12), rng.choice(["half", "rare"])),
    lambda rng: ("both",),
    lambda rng: ("shift", rng.choice([1, 2, 5, 13])),
    lambda rng: ("exit", rng.choice(["break", "continue", "return"])),
    lambda rng: ("callmid", rng.choice(["touch", "tick"])),
    lambda rng: ("store", rng.choice(["apart", "maybe"])),
    lambda rng: ("invariant",),
    lambda rng: ("pressure", rng.choice([1, 2, 3, 5])),
    lambda rng: ("after",),
]


# --- the family -------------------------------------------------------------------


def _walks(n: int, sizes: str, index=("i",), where="param", ptr="near") -> tuple[Walk, ...]:
    elems = SIZES[sizes]
    return tuple(Walk(elems[k % len(elems)], index, where, ptr) for k in range(n))


BASES = {
    "param": ("param", "near"), "global": ("global", "near"), "local": ("local", "near"), "far": ("param", "far"),
}


def shapes() -> list[Shape]:
    out = []
    counts = range(1, 13)
    # sharing: n arrays of each size mix and base kind, indexed alike
    for n in counts:
        for sizes in SIZES:
            for base in (*BASES, "mixed"):
                if base == "mixed":
                    kinds = list(BASES.values())
                    walks = tuple(replace(w, where=kinds[k % 4][0], ptr=kinds[k % 4][1])
                                  for k, w in enumerate(_walks(n, sizes)))
                else:
                    walks = _walks(n, sizes, where=BASES[base][0], ptr=BASES[base][1])
                out.append(Shape(walks))
    # offsets and strides: each index form, n equal-size arrays
    forms = [("off", 1), ("off", -1), ("off", 7), ("off", -7), ("sym",), ("rev",), ("scale", 2), ("scale", 3), ("stride",)]
    for n in (1, 2, 3, 4, 6, 8):
        for form in forms:
            out.append(Shape(_walks(n, "equal", form)))
        # a[i], a[i+1] ... a[i+k] on one array: one base, one stride
        out.append(Shape(tuple(Walk(I16, ("off", k), same_as=0 if k else None) for k in range(n))))
        # forward and reversed of one stride: two classes
        out.append(Shape(tuple(Walk(I16, ("rev",) if k % 2 else ("i",)) for k in range(n))))
    # source forms and uses
    for n in (1, 2, 3, 4, 6):
        for form in ("index", "ptr", "end", "mixed", "do"):
            for use in ("sum", "store", "copy", "mixed"):
                out.append(Shape(_walks(n, "equal"), form=form, use=use))
    # trip, step and a call
    for n in (1, 2, 4):
        for trip in ("n", "const", "n-k", "n+k", "len"):
            for step in (1, 2, -1):
                for call in (False, True):
                    walks = _walks(n, "equal", where="global" if trip == "len" else "param")
                    out.append(Shape(walks, trip=trip, step=step, call=call))
    # start offsets: each array its own constant start, a symbolic one, both
    # signs, overlapping walks of one array, a start from an outer loop, and
    # pointers that begin mid-array
    for n in (2, 3, 4, 6):
        out.append(Shape(tuple(Walk(I16, start=3 * k) for k in range(n))))
        out.append(Shape(tuple(Walk(I16, start=(5 * k) % 7, where="global") for k in range(n))))
        out.append(Shape(tuple(Walk(I16, ("plusn",) if k % 2 else ("i",)) for k in range(n))))
        out.append(Shape(tuple(Walk(I16, ("off", 5 if k % 2 else -5)) for k in range(n))))
        out.append(Shape(tuple(Walk(I16, ("off", 2 * k), same_as=0 if k else None) for k in range(n))))
        out.append(Shape(_walks(n, "equal"), outer=True))
        out.append(Shape(tuple(Walk(I16, start=4 * k + 1) for k in range(n)), form="ptr"))
    return out + boundaries()


def boundaries() -> list[Shape]:
    """Walks across a whole segment: stride x n just below and at 32768 and
    65536, a counter that starts at 0 (-65536 in 16 bits), unsigned n, a
    negative step, and far or huge arrays, where the counter form must be
    declined or done right."""
    whole = (0, 1, 16383, 16384, 16385, 32767, 32768)
    far = lambda elem, where="global", index=("i",): Walk(elem, index, where, "far")  # noqa: E731
    return [
        Shape((far(I16),), counter=U16, rows=whole, margin=0),
        Shape((far(I16, "param"),), counter=U16, rows=whole, margin=0),
        Shape((far(U8),), counter=U16, rows=(0, 1, 32767, 32768, 65534, 65535), margin=0),
        Shape((far(I16, "param", ("rev",)),), counter=U16, rows=(0, 1, 32767, 32768), margin=0),
        Shape((far(I16, "param"),), step=-1, rows=(0, 1, 16384, 32767), margin=0),
        Shape((far(I16), far(I16, "param")), counter=U16, rows=(0, 32767, 32768), margin=0),
        Shape((far(I16, "param"),), form="do", counter=U16, rows=(0, 1, 32768), margin=0),
        Shape((Walk(I32, where="global", ptr="huge"),), rows=(0, 16383, 16384, 20000), margin=0),
        Shape((Walk(I32, where="param", ptr="huge"),), rows=(0, 16384, 20000), margin=0),
    ]


def released_bp() -> list[Case]:
    """3 and 4 near arrays in a call-free loop that touches no frame slot:
    with bp released as a seventh register (saved and restored around the
    loop), 32-bit addressing on every CPU here pairs one counter with all the
    bases: counter, bases, accumulator and a temporary fit 7. Then the same
    loops with a call, and with a local array (a frame slot), where bp must
    stay the frame."""
    out = []
    why = "bp released: counter + {n} bases + accumulator + temporary = {total} of 7 registers, 32-bit addressing"
    for n in (3, 4):
        shape = Shape(_walks(n, "equal"))
        case = build(shape, name=f"bp{n}_released")
        out.append(case.with_(bound=tuple((lang, 1, why.format(n=n, total=n + 3)) for lang in ("c",)),
                              tags=case.tags | {"bp:released"}))
        out.append(build(replace(shape, call=True), name=f"bp{n}_call").with_(tags=case.tags | {"bp:call"}))
        framed = tuple(replace(w, where="local") if k == 0 else w for k, w in enumerate(shape.walks))
        out.append(build(replace(shape, walks=framed), name=f"bp{n}_frame").with_(tags=case.tags | {"bp:frame"}))
    return out


def variants(bases: list[tuple[Shape, Case]], seed: int, per_base: int) -> list[Case]:
    """Each base with `per_base` compositions of 1..3 transformers, drawn from
    `seed` and recorded in the name, so any failure replays."""
    rng = random.Random(seed)
    out = []
    for shape, base in bases:
        if not shape.walks:
            continue
        chosen = set()
        for _ in range(per_base):
            extras = tuple(pick(rng) for pick in rng.sample(TRANSFORMERS, rng.randint(1, 3)))
            key = tuple(sorted(map(repr, extras)))
            if key in chosen:
                continue
            chosen.add(key)
            variant = replace(shape, extras=extras)
            out.append(build(variant, base=base.name, relation=" + ".join(one[0] for one in extras)))
        # form independence: the same loop, per-array pointers and an end compare
        # (the end pointer walks forward, from a start the margin keeps in bounds)
        if (shape.form == "index" and shape.step > 0 and shape.use == "sum" and shape.margin
                and shape.walks[0].index[0] != "rev"):
            for form in ("ptr", "end"):
                case = build(replace(shape, form=form), name=f"{base.name}_as_{form}", base=base.name,
                             relation=f"form:{form}")
                out.append(case.with_(tags=case.tags | {"same-size", f"relation:form-{form}"}))
    return out


def cases(quick: bool = False, seed: int = 98) -> list[Case]:
    """Every case; `quick` a sample of them, so each quick case is one the
    full run (and known.toml) also has."""
    every = _all(seed)
    if not quick:
        return every
    return [c for k, c in enumerate(every)
            if k % 60 == 0 or (c.base is None and "whole" in c.name) or c.name.startswith("bp")]


def _all(seed: int) -> list[Case]:
    built = []
    seen: dict[str, Shape] = {}
    for shape in shapes():
        case = build(shape)
        if case.name in seen:
            if seen[case.name] != shape:
                raise ValueError(f"two shapes named {case.name}")
            continue
        seen[case.name] = shape
        built.append((shape, case))
    # variants of the sharing bases, the loops the relations exist for
    bases = [(s, c) for s, c in built if s.form == "index" and s.use == "sum" and not s.call and s.trip == "n"]
    extra = variants(bases[::3], seed, 4)
    names = set(seen)
    for case in extra:
        if case.name in names:
            continue
        names.add(case.name)
        built.append((None, case))
    return [case for _, case in built] + released_bp()
