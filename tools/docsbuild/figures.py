"""Figure spec to a self-contained SVG string.

A spec is text: a first line `kind: graph|stack|lanes`, optional `title: ...`, then
one directive per line (see docs/working/figures.md). `build(spec)` returns the
`Figure` with its geometry for checks; `render(spec)` returns just the SVG. The
output is deterministic and carries no timestamp.
"""
from __future__ import annotations

import hashlib
import math
import re
from dataclasses import dataclass, field
from html import escape as _esc

CW = 7.2  # estimated width of one character at 13px system sans
LH = 17  # line height
PAD_X, PAD_Y = 14, 9
NODESEP, EDGESEP, RANKSEP = 34, 22, 64
GP, CAPH, MARGIN = 14, 20, 14
FILLS = "abcde"
SHAPES = ("box", "round", "note")
ARROWS = {"->": "", "..>": "dash", "==>": "bold"}


class FigureError(Exception):
    """A spec error, with the 1-based line of the spec it refers to."""

    def __init__(self, line: int, message: str):
        super().__init__(f"line {line}: {message}")
        self.line = line
        self.message = message


@dataclass
class Box:
    kind: str
    id: str
    x: float
    y: float
    w: float
    h: float


@dataclass
class Txt:
    x0: float
    y0: float
    x1: float
    y1: float
    owner: str | None
    text: str


@dataclass
class Figure:
    svg: str = ""
    viewbox: tuple[float, float, float, float] = (0, 0, 0, 0)
    boxes: list[Box] = field(default_factory=list)
    texts: list[Txt] = field(default_factory=list)
    labels: list[Box] = field(default_factory=list)


def fmt(v: float) -> str:
    s = f"{round(v, 1):.1f}"
    return s[:-2] if s.endswith(".0") else s


def tw(s: str) -> float:
    return max((len(x) for x in s.split("\n")), default=0) * CW


def nlines(s: str) -> int:
    return s.count("\n") + 1


# ---------------------------------------------------------------- tokens

_TOK = re.compile(
    r"""\s*(?:(?P<kv>[A-Za-z_]\w*)=(?P<val>"(?:[^"\\]|\\.)*"|\S+)"""
    r"""|(?P<str>"(?:[^"\\]|\\.)*")|(?P<colon>:(?=\s|$))|(?P<word>[^\s"]\S*))"""
)


@dataclass
class Tok:
    kind: str  # word | str | colon | kv
    val: str
    key: str = ""


def _unquote(s: str) -> str:
    if len(s) >= 2 and s[0] == '"' and s[-1] == '"':
        s = s[1:-1]
    return re.sub(r"\\(.)", lambda m: "\n" if m.group(1) == "n" else m.group(1), s)


def tokenize(no: int, line: str) -> list[Tok]:
    out, pos = [], 0
    while pos < len(line):
        if not line[pos:].strip():
            break
        m = _TOK.match(line, pos)
        if not m:
            raise FigureError(no, "unterminated string")
        pos = m.end()
        if m.group("kv"):
            out.append(Tok("kv", _unquote(m.group("val")), m.group("kv")))
        elif m.group("str") is not None:
            out.append(Tok("str", _unquote(m.group("str"))))
        elif m.group("colon"):
            out.append(Tok("colon", ":"))
        else:
            out.append(Tok("word", m.group("word")))
    return out


def _args(no: int, toks: list[Tok], allowed: tuple[str, ...]) -> tuple[list[Tok], dict[str, str]]:
    pos, kv = [], {}
    for t in toks:
        if t.kind == "kv":
            if t.key not in allowed:
                raise FigureError(no, f"unknown option '{t.key}'")
            kv[t.key] = t.val
        else:
            pos.append(t)
    return pos, kv


def _split(spec: str):
    lines = [
        (no, raw.strip())
        for no, raw in enumerate(spec.split("\n"), 1)
        if raw.strip() and not raw.strip().startswith("#")
    ]
    if not lines:
        raise FigureError(1, "empty figure")
    no, first = lines[0]
    m = re.fullmatch(r"kind:\s*(\w+)", first)
    if not m:
        raise FigureError(no, "first line must be 'kind: graph|stack|lanes'")
    kind, opts, body = m.group(1), {}, []
    if kind not in ("graph", "stack", "lanes"):
        raise FigureError(no, f"unknown kind '{kind}'")
    for no, s in lines[1:]:
        m = re.match(r"(title|dir|scale):\s*(.*)$", s)
        if m:
            if m.group(1) in opts:
                raise FigureError(no, f"duplicate '{m.group(1)}'")
            opts[m.group(1)] = (no, _unquote(m.group(2).strip()))
        else:
            body.append((no, s))
    return kind, opts, body


# ---------------------------------------------------------------- canvas


class Canvas:
    def __init__(self) -> None:
        self.els: list[str] = []
        self.fig = Figure()
        self.b = [math.inf, math.inf, -math.inf, -math.inf]

    def grow(self, x0: float, y0: float, x1: float | None = None, y1: float | None = None) -> None:
        x1 = x0 if x1 is None else x1
        y1 = y0 if y1 is None else y1
        self.b = [min(self.b[0], x0), min(self.b[1], y0), max(self.b[2], x1), max(self.b[3], y1)]

    def box(self, kind: str, id: str, x: float, y: float, w: float, h: float) -> Box:
        bx = Box(kind, id, x, y, w, h)
        self.fig.boxes.append(bx)
        self.grow(x, y, x + w, y + h)
        return bx

    def text(self, x: float, y: float, s: str, anchor="middle", cls="t", owner=None) -> Txt:
        """Draw `s` with its block's top at `y`; `x` is the anchor point."""
        w = tw(s)
        x0 = x - w / 2 if anchor == "middle" else (x if anchor == "start" else x - w)
        for i, line in enumerate(s.split("\n")):
            self.els.append(
                f'<text class="{cls}" x="{fmt(x)}" y="{fmt(y + LH * i + 13)}" '
                f'text-anchor="{anchor}">{_esc(line, quote=False)}</text>'
            )
        t = Txt(x0, y, x0 + w, y + LH * nlines(s), owner, s)
        self.fig.texts.append(t)
        self.grow(t.x0, t.y0, t.x1, t.y1)
        return t

    def add(self, s: str) -> None:
        self.els.append(s)


def _overlap(a, b, pad=0.0) -> bool:
    return a.x < b.x + b.w + pad and b.x < a.x + a.w + pad and a.y < b.y + b.h + pad and b.y < a.y + a.h + pad


def crosses_border(box, group, pad=4.0) -> bool:
    """True when `box` touches `group`'s outline: not wholly inside, not wholly outside."""
    if not _overlap(box, group, pad):
        return False
    inside = (box.x >= group.x + pad and box.y >= group.y + pad
              and box.x + box.w <= group.x + group.w - pad and box.y + box.h <= group.y + group.h - pad)
    return not inside


# ---------------------------------------------------------------- graph


@dataclass
class _Node:
    id: str
    label: str
    fill: str
    shape: str
    line: int


@dataclass
class _Edge:
    a: str
    b: str
    label: str
    style: str
    line: int


@dataclass
class _Group:
    name: str
    caption: str
    members: list[str]
    line: int


class _Item:
    def __init__(self, key, node, cs, ms):
        self.key, self.node, self.cs, self.ms = key, node, cs, ms
        self.gset: tuple[int, ...] = ()
        self.rr = 0.0
        self.layer = 0
        self.pos = 0.0
        self.w = self.h = 0.0


def _parse_graph(opts, body):
    nodes: dict[str, _Node] = {}
    edges: list[_Edge] = []
    groups: list[_Group] = []
    d = opts.get("dir", (0, "down"))
    if d[1] not in ("down", "right"):
        raise FigureError(d[0], "dir must be right or down")
    for no, s in body:
        t = tokenize(no, s)
        head = t[0].val if t[0].kind == "word" else ""
        if head == "node":
            pos, kv = _args(no, t[1:], ("fill", "shape"))
            if not pos or pos[0].kind != "word" or not re.fullmatch(r"[A-Za-z_][\w.]*", pos[0].val):
                raise FigureError(no, "node needs an id: node ID \"label\"")
            if len(pos) > 2 or (len(pos) == 2 and pos[1].kind != "str"):
                raise FigureError(no, "node takes an id and one quoted label")
            nid = pos[0].val
            if nid in nodes:
                raise FigureError(no, f"duplicate node '{nid}'")
            fill, shape = kv.get("fill", ""), kv.get("shape", "box")
            if fill and fill not in tuple(FILLS):
                raise FigureError(no, f"fill must be one of {', '.join(FILLS)}")
            if shape not in SHAPES:
                raise FigureError(no, f"shape must be one of {', '.join(SHAPES)}")
            nodes[nid] = _Node(nid, pos[1].val if len(pos) == 2 else nid, fill, shape, no)
        elif head == "edge":
            pos, kv = _args(no, t[1:], ())
            if len(pos) < 3 or pos[0].kind != "word" or pos[2].kind != "word" or pos[1].val not in ARROWS:
                raise FigureError(no, "edge syntax: edge A -> B \"label\" (arrows: -> ..> ==>)")
            if len(pos) > 4 or (len(pos) == 4 and pos[3].kind != "str"):
                raise FigureError(no, "edge takes two ids and one quoted label")
            edges.append(_Edge(pos[0].val, pos[2].val, pos[3].val if len(pos) == 4 else "", ARROWS[pos[1].val], no))
        elif head == "group":
            if len(t) < 5 or t[1].kind != "word" or t[2].kind != "str" or t[3].kind != "colon":
                raise FigureError(no, "group syntax: group NAME \"caption\": ID ID ...")
            if any(x.kind != "word" for x in t[4:]):
                raise FigureError(no, "group members are node ids")
            groups.append(_Group(t[1].val, t[2].val, [x.val for x in t[4:]], no))
        else:
            raise FigureError(no, f"unknown directive '{s.split()[0]}'")
    if not nodes:
        raise FigureError(1, "graph has no nodes")
    for e in edges:
        for x in (e.a, e.b):
            if x not in nodes:
                raise FigureError(e.line, f"unknown node '{x}'")
    for g in groups:
        for x in g.members:
            if x not in nodes:
                raise FigureError(g.line, f"unknown node '{x}'")
    return nodes, edges, groups, d[1]


def _graph(nodes, edges, groups, direction, cv: Canvas, pre: str):
    right = direction == "right"
    ids = list(nodes)
    items: dict[str, _Item] = {}
    for nid, n in nodes.items():
        w, h = max(tw(n.label) + 2 * PAD_X, 64), max(nlines(n.label) * LH + 2 * PAD_Y, 36)
        it = _Item(nid, n, h if right else w, w if right else h)
        it.w, it.h = w, h
        it.gset = tuple(i for i, g in enumerate(groups) if nid in g.members)
        items[nid] = it
    # cycle handling: DFS back edges are reversed out of the layout and drawn as arcs
    out: dict[str, list[tuple[int, str]]] = {i: [] for i in ids}
    indeg = dict.fromkeys(ids, 0)
    for ei, e in enumerate(edges):
        out[e.a].append((ei, e.b))
        indeg[e.b] += 1
    color: dict[str, int] = {}
    back: list[int] = []
    order: list[str] = []

    def dfs(u):
        color[u] = 1
        for ei, v in out[u]:
            if color.get(v, 0) == 0:
                dfs(v)
            elif color[v] == 1:
                back.append(ei)
        color[u] = 2
        order.append(u)

    for r in [i for i in ids if indeg[i] == 0] + ids:
        if r not in color:
            dfs(r)
    rank = dict.fromkeys(ids, 0)
    for u in reversed(order):
        for ei, v in out[u]:
            if ei not in back:
                rank[v] = max(rank[v], rank[u] + 1)
    nl = max(rank.values()) + 1
    layers: list[list[_Item]] = [[] for _ in range(nl)]
    for i in ids:
        items[i].layer = rank[i]
        layers[rank[i]].append(items[i])
    up: dict[str, list[_Item]] = {}
    down: dict[str, list[_Item]] = {}
    chains: dict[int, list[_Item]] = {}
    for ei, e in enumerate(edges):
        if ei in back:
            continue
        chain = [items[e.a]]
        for r in range(rank[e.a] + 1, rank[e.b]):
            d = _Item(f"d{ei}.{r}", None, 8, 0)
            d.layer = r
            d.gset = tuple(g for g in items[e.a].gset if g in items[e.b].gset)
            layers[r].append(d)
            chain.append(d)
        chain.append(items[e.b])
        chains[ei] = chain
        for p, q in zip(chain, chain[1:]):
            down.setdefault(p.key, []).append(q)
            up.setdefault(q.key, []).append(p)
    for ei in back:
        e = edges[ei]
        if e.a == e.b and e.label:
            items[e.a].rr = (nlines(e.label) * LH + 34) if right else tw(e.label) + 34
        elif e.a == e.b:
            items[e.a].rr = 28

    def regroup(layer):
        done, res = set(), []
        for it in layer:
            g = it.gset[0] if it.gset else None
            if g is None:
                res.append(it)
            elif g not in done:
                done.add(g)
                res += [x for x in layer if x.gset and x.gset[0] == g]
        return res

    def crossings():
        n = 0
        for k in range(nl - 1):
            ix = {it.key: i for i, it in enumerate(layers[k + 1])}
            es = [(i, ix[q.key]) for i, p in enumerate(layers[k]) for q in down.get(p.key, [])]
            for a in range(len(es)):
                for b in range(a + 1, len(es)):
                    if (es[a][0] - es[b][0]) * (es[a][1] - es[b][1]) < 0:
                        n += 1
        return n

    layers[:] = [regroup(layer) for layer in layers]
    best = (crossings(), [list(layer) for layer in layers])
    for sweep in range(16):
        fwd = sweep % 2 == 0
        for k in range(1, nl) if fwd else range(nl - 2, -1, -1):
            ref = layers[k - 1] if fwd else layers[k + 1]
            nb = up if fwd else down
            rix = {it.key: i for i, it in enumerate(ref)}
            cur = {it.key: i for i, it in enumerate(layers[k])}

            def bary(it):
                ns = [rix[n.key] for n in nb.get(it.key, []) if n.key in rix]
                return sum(ns) / len(ns) if ns else cur[it.key]

            layers[k] = regroup(sorted(layers[k], key=lambda it: (bary(it), cur[it.key])))
        c = crossings()
        if c < best[0]:
            best = (c, [list(layer) for layer in layers])
    layers[:] = best[1]

    def sep(a, b):
        s = (a.cs + b.cs) / 2 + a.rr + (NODESEP if a.node and b.node else EDGESEP)
        return s + (2 * GP + 8 if a.gset != b.gset else 0)

    def resolve(layer, want):
        n = len(layer)
        lo, hi = list(want), list(want)
        for i in range(1, n):
            lo[i] = max(lo[i], lo[i - 1] + sep(layer[i - 1], layer[i]))
        for i in range(n - 2, -1, -1):
            hi[i] = min(hi[i], hi[i + 1] - sep(layer[i], layer[i + 1]))
        for i, it in enumerate(layer):
            it.pos = (lo[i] + hi[i]) / 2

    for layer in layers:
        p = 0.0
        for i, it in enumerate(layer):
            if i:
                p += sep(layer[i - 1], it)
            it.pos = p
        mid = (layer[0].pos + layer[-1].pos) / 2
        for it in layer:
            it.pos -= mid
    for it_ in range(10):
        seq = range(1, nl) if it_ % 2 == 0 else range(nl - 2, -1, -1)
        for k in seq:
            want = []
            for it in layers[k]:
                ns = up.get(it.key, []) + down.get(it.key, [])
                want.append(sum(n.pos for n in ns) / len(ns) if ns else it.pos)
            resolve(layers[k], want)

    labelled = [(ei, edges[ei]) for ei in chains if edges[ei].label]
    gaps = [RANKSEP + 8 * len(back)] * max(nl - 1, 0)
    for ei, e in labelled:
        need = tw(e.label) + 8 + 28 if right else nlines(e.label) * LH + 4 + 24
        borders = len(set(items[e.a].gset) ^ set(items[e.b].gset))
        need += borders * (GP if right else GP + CAPH)
        gaps[rank[e.a]] = max(gaps[rank[e.a]], need)
    ms = [max(it.ms for it in layer) for layer in layers]
    start, end = [], []
    m = 0.0
    for k in range(nl):
        start.append(m)
        end.append(m + ms[k])
        m += ms[k] + (gaps[k] if k < nl - 1 else 0)

    def mc(it):
        return start[it.layer] + ms[it.layer] / 2

    def xy(c, m_):
        return (m_, c) if right else (c, m_)

    def rect(it):
        if it.node:
            x, y = xy(it.pos, mc(it))
            return Box("item", it.key, x - it.w / 2, y - it.h / 2, it.w, it.h)
        x, y = xy(it.pos, mc(it))
        if right:
            return Box("item", it.key, start[it.layer], it.pos - it.cs / 2, ms[it.layer], it.cs)
        return Box("item", it.key, it.pos - it.cs / 2, start[it.layer], it.cs, ms[it.layer])

    def grect(gi):
        rs = [rect(it) for layer in layers for it in layer if it.node and gi in it.gset]
        x0, y0 = min(r.x for r in rs) - GP, min(r.y for r in rs) - GP - CAPH
        x1, y1 = max(r.x + r.w for r in rs) + GP, max(r.y + r.h for r in rs) + GP
        x1 = max(x1, x0 + tw(groups[gi].caption) + 20)
        return Box("group", groups[gi].name, x0, y0, x1 - x0, y1 - y0)

    for _ in range(80):
        changed = False
        for gi in range(len(groups)):
            g = grect(gi)
            hit = None
            for layer in layers:
                for j, it in enumerate(layer):
                    if gi not in it.gset and _overlap(rect(it), g):
                        hit = (layer, j, it)
                        break
                if hit:
                    break
            if not hit:
                continue
            layer, j, it = hit
            glo, ghi = (g.y, g.y + g.h) if right else (g.x, g.x + g.w)
            mem = [i for i, x in enumerate(layer) if x.node and gi in x.gset]
            left = j < min(mem) if mem else it.pos < (glo + ghi) / 2
            if left:
                it.pos = min(it.pos, glo - it.cs / 2 - 4)
                for i in range(j - 1, -1, -1):
                    layer[i].pos = min(layer[i].pos, layer[i + 1].pos - sep(layer[i], layer[i + 1]))
            else:
                it.pos = max(it.pos, ghi + it.cs / 2 + 4)
                for i in range(j + 1, len(layer)):
                    layer[i].pos = max(layer[i].pos, layer[i - 1].pos + sep(layer[i - 1], layer[i]))
            changed = True
        if not changed:
            break

    # ports spread along a node side, ordered by where the edge goes
    port_out: dict[int, float] = {}
    port_in: dict[int, float] = {}
    for it in (i for layer in layers for i in layer if i.node):
        for table, lst in ((port_out, [(ei, c[1]) for ei, c in chains.items() if c[0] is it]),
                           (port_in, [(ei, c[-2]) for ei, c in chains.items() if c[-1] is it])):
            lst.sort(key=lambda t: (t[1].pos, t[0]))
            for i, (ei, _o) in enumerate(lst):
                table[ei] = it.pos - 0.4 * it.cs + 0.8 * it.cs * (i + 1) / (len(lst) + 1)

    def P(c, m_):
        x, y = xy(c, m_)
        cv.grow(x, y)
        return f"{fmt(x)} {fmt(y)}"

    # groups
    for gi, g in enumerate(groups):
        r = grect(gi)
        cv.box("group", g.name, r.x, r.y, r.w, r.h)
        cv.add(f'<rect class="grp" x="{fmt(r.x)}" y="{fmt(r.y)}" width="{fmt(r.w)}" height="{fmt(r.h)}" rx="10"/>')
        cv.text(r.x + 10, r.y + 4, g.caption, "start", "gc", owner=g.name)
    placed: list[Box] = []
    paths: list[str] = []
    for ei, chain in chains.items():
        e = edges[ei]
        a, b = chain[0], chain[-1]
        co = port_out[ei]
        d = [f"M{P(co, mc(a) + ms_half(a))}"]
        cp = co
        for p, q in zip(chain, chain[1:]):
            cq = port_in[ei] if q is b else q.pos
            mid = (end[p.layer] + start[q.layer]) / 2
            d.append(f"L{P(cp, end[p.layer])}")
            d.append(f"C{P(cp, mid)} {P(cq, mid)} {P(cq, start[q.layer])}")
            if p is a and e.label:
                _place_label(cv, e, ei, cp, cq, end[p.layer], start[q.layer], right, xy, placed, [grect(i) for i in range(len(groups))])
            cp = cq
        d.append(f"L{P(cp, mc(b) - ms_half(b))}")
        paths.append(
            f'<path class="{ecls(e.style)}" d="{" ".join(d)}" marker-end="url(#{pre}-ar)"/>'
        )
    # back edges and self loops
    allr = [rect(it) for layer in layers for it in layer] + [grect(i) for i in range(len(groups))]
    cmax = max((r.y + r.h) if right else (r.x + r.w) for r in allr)
    for k, ei in enumerate(back):
        e = edges[ei]
        a, b = items[e.a], items[e.b]
        if a is b:
            ra = rect(a)
            if right:
                cx_, by_ = ra.x + ra.w / 2, ra.y + ra.h
                d = (f"M{P_xy(cv, cx_ - 7, by_)} C{P_xy(cv, cx_ - 7, by_ + 24)} "
                     f"{P_xy(cv, cx_ + 7, by_ + 24)} {P_xy(cv, cx_ + 7, by_ + 1)}")
                lx, ly, anchor = cx_, by_ + 26, "middle"
            else:
                sx, sy = ra.x + ra.w, ra.y + ra.h / 2
                d = (f"M{P_xy(cv, sx, sy - 7)} C{P_xy(cv, sx + 24, sy - 7)} "
                     f"{P_xy(cv, sx + 24, sy + 7)} {P_xy(cv, sx + 1, sy + 7)}")
                lx, ly, anchor = sx + 28, sy - nlines(e.label) * LH / 2, "start"
            paths.append(f'<path class="{ecls(e.style)}" d="{d}" marker-end="url(#{pre}-ar)"/>')
            if e.label:
                t = cv.text(lx, ly, e.label, anchor, "t lbl")
                cv.fig.labels.append(Box("label", f"{e.a}->{e.b}", t.x0, t.y0, t.x1 - t.x0, t.y1 - t.y0))
            continue
        cb = cmax + 24 * (k + 1)
        ma = end[a.layer] + 12 + 6 * k
        mb = start[b.layer] - 12 - 6 * k
        pts = [
            (a.pos + 0.3 * a.cs, mc(a) + ms_half(a)),
            (a.pos + 0.3 * a.cs, ma),
            (cb, ma),
            (cb, mb),
            (b.pos + 0.3 * b.cs, mb),
            (b.pos + 0.3 * b.cs, mc(b) - ms_half(b)),
        ]
        paths.append(
            f'<path class="{ecls(e.style)}" d="{_rounded([xy(*p) for p in pts], 12, cv)}" marker-end="url(#{pre}-ar)"/>'
        )
        if e.label:
            mm = (ma + mb) / 2
            h = nlines(e.label) * LH
            if right:
                t = cv.text(mm, cb + 6, e.label, "middle", "t lbl")
            else:
                t = cv.text(cb + 8, mm - h / 2, e.label, "start", "t lbl")
            cv.fig.labels.append(Box("label", f"{e.a}->{e.b}", t.x0, t.y0, t.x1 - t.x0, t.y1 - t.y0))
    for p in paths:
        cv.add(p)
    # nodes last so edge ends sit under their outlines
    for layer in layers:
        for it in layer:
            if not it.node:
                continue
            n = it.node
            r = rect(it)
            cv.box("node", n.id, r.x, r.y, r.w, r.h)
            cls = f"nd f-{n.fill}" if n.fill else "nd"
            cv.add(_shape(n.shape, cls, r))
            cv.text(r.x + r.w / 2, r.y + (r.h - nlines(n.label) * LH) / 2, n.label, "middle", "t", owner=n.id)
    return (
        f"Graph, {len(nodes)} nodes, {len(edges)} edges: "
        + "; ".join(f"{nodes[e.a].label.replace(chr(10), ' ')} to {nodes[e.b].label.replace(chr(10), ' ')}" for e in edges)
        if edges
        else f"Graph, {len(nodes)} nodes: " + ", ".join(n.label.replace("\n", " ") for n in nodes.values())
    )


def ecls(style: str) -> str:
    return f"e {style}" if style else "e"


def ms_half(it) -> float:
    return it.ms / 2


def P_xy(cv: Canvas, x, y) -> str:
    cv.grow(x, y)
    return f"{fmt(x)} {fmt(y)}"


def _rounded(pts, r, cv) -> str:
    d = [f"M{fmt(pts[0][0])} {fmt(pts[0][1])}"]
    for p in pts:
        cv.grow(*p)
    for i in range(1, len(pts) - 1):
        (x0, y0), (x1, y1), (x2, y2) = pts[i - 1], pts[i], pts[i + 1]
        l1, l2 = math.hypot(x1 - x0, y1 - y0), math.hypot(x2 - x1, y2 - y1)
        rr = min(r, l1 / 2, l2 / 2)
        if rr <= 0:
            continue
        ax, ay = x1 - (x1 - x0) / l1 * rr, y1 - (y1 - y0) / l1 * rr
        bx, by = x1 + (x2 - x1) / l2 * rr, y1 + (y2 - y1) / l2 * rr
        d.append(f"L{fmt(ax)} {fmt(ay)} Q{fmt(x1)} {fmt(y1)} {fmt(bx)} {fmt(by)}")
    d.append(f"L{fmt(pts[-1][0])} {fmt(pts[-1][1])}")
    return " ".join(d)


def _place_label(cv, e, ei, ca, cb, m0, m1, right, xy, placed, borders=()):
    w, h = tw(e.label) + 8, nlines(e.label) * LH + 4
    ts = (0.5, 0.3, 0.7, 0.15, 0.85, 0.05, 0.95)
    opts = [(t, s) for t in ts for s in ((-1, 1) if right else (1, -1))]
    chosen = None
    for t, side in opts:
        m = m0 + (m1 - m0) * t
        c = ca + (cb - ca) * (3 * t * t - 2 * t**3)
        if right:
            box = Box("label", f"{e.a}->{e.b}", m - w / 2, (c - 3 - h) if side < 0 else c + 3, w, h)
        else:
            box = Box("label", f"{e.a}->{e.b}", c + 6 if side > 0 else c - 6 - w, m - h / 2, w, h)
        chosen = chosen or box
        if not any(_overlap(box, p) for p in placed) and not any(crosses_border(box, g) for g in borders):
            chosen = box
            break
    placed.append(chosen)
    cv.fig.labels.append(chosen)
    cv.text(chosen.x + chosen.w / 2, chosen.y + 2, e.label, "middle", "t lbl")


def _shape(kind, cls, r: Box) -> str:
    x, y, w, h = (fmt(v) for v in (r.x, r.y, r.w, r.h))
    if kind == "round":
        return f'<rect class="{cls}" x="{x}" y="{y}" width="{w}" height="{h}" rx="{fmt(r.h / 2)}"/>'
    if kind == "note":
        f = 10
        d = (f"M{x} {y}H{fmt(r.x + r.w - f)}L{fmt(r.x + r.w)} {fmt(r.y + f)}V{fmt(r.y + r.h)}H{x}Z"
             f"M{fmt(r.x + r.w - f)} {y}V{fmt(r.y + f)}H{fmt(r.x + r.w)}")
        return f'<path class="{cls}" d="{d}"/>'
    return f'<rect class="{cls}" x="{x}" y="{y}" width="{w}" height="{h}" rx="4"/>'


# ---------------------------------------------------------------- stack


def _stack(opts, body, cv: Canvas, pre: str) -> str:
    rows: list[dict] = []
    brackets: list[tuple[int, str, int, int]] = []
    scale = opts.get("scale", (0, "no"))[1] == "yes"
    for no, s in body:
        t = tokenize(no, s)
        head = t[0].val
        if head == "cell":
            pos, kv = _args(no, t[1:], ("off", "size", "note", "fill"))
            if len(pos) != 1 or pos[0].kind != "str":
                raise FigureError(no, 'cell syntax: cell "label" [off=..] [size=..] [note=..] [fill=a..e]')
            if kv.get("fill") and kv["fill"] not in tuple(FILLS):
                raise FigureError(no, f"fill must be one of {', '.join(FILLS)}")
            if scale and kv.get("size") is not None and not re.fullmatch(r"\d+(\.\d+)?", kv["size"]):
                raise FigureError(no, "size must be a number with scale: yes")
            rows.append(dict(kind="cell", label=pos[0].val, line=no, **kv))
        elif head == "gap":
            pos, _ = _args(no, t[1:], ())
            rows.append(dict(kind="gap", label=pos[0].val if pos and pos[0].kind == "str" else "...", line=no))
        elif head == "arrow":
            pos, _ = _args(no, t[1:], ())
            if len(pos) != 2 or pos[0].kind != "str" or pos[1].val not in ("up", "down"):
                raise FigureError(no, 'arrow syntax: arrow "label" up|down')
            rows.append(dict(kind="arrow", label=pos[0].val, dir=pos[1].val, line=no))
        elif head == "bracket":
            pos, kv = _args(no, t[1:], ("from", "to"))
            if len(pos) != 1 or pos[0].kind != "str" or set(kv) != {"from", "to"}:
                raise FigureError(no, 'bracket syntax: bracket "label" from=N to=M')
            try:
                a, b = int(kv["from"]), int(kv["to"])
            except ValueError:
                raise FigureError(no, "from and to are cell numbers") from None
            brackets.append((no, pos[0].val, a, b))
        else:
            raise FigureError(no, f"unknown directive '{head}'")
    cells = [r for r in rows if r["kind"] != "arrow"]
    if not cells:
        raise FigureError(1, "stack has no cells")
    for no, _l, a, b in brackets:
        if not 1 <= a <= b <= len(cells):
            raise FigureError(no, f"bracket range {a}..{b} outside 1..{len(cells)}")
    ow = max([tw(r.get("off", "")) for r in rows] + [0])
    x0 = ow + (14 if ow else 0)
    cwid = 180.0
    for r in rows:
        if r["kind"] == "cell":
            sz = tw(r.get("size", "")) + 10 if r.get("size") else 0
            cwid = max(cwid, tw(r["label"]) + 2 * PAD_X + 2 * sz)
        elif r["kind"] == "arrow":
            cwid = max(cwid, tw(r["label"]) + 44)
        else:
            cwid = max(cwid, tw(r["label"]) + 2 * PAD_X)
    y = 0.0
    for r in rows:
        if r["kind"] == "cell":
            h = max(34, nlines(r["label"]) * LH + 2 * PAD_Y)
            if scale and r.get("size"):
                h = max(h, min(float(r["size"]) * 8, 160))
        elif r["kind"] == "gap":
            h = 28
        else:
            h = 36
        r["y"], r["h"] = y, h
        y += h
    cellrows = [r for r in rows if r["kind"] != "arrow"]
    # bracket columns
    cols: list[list[tuple[int, int]]] = []
    bcol = []
    for no, label, a, b in sorted(brackets, key=lambda t: (t[2], t[3])):
        for ci, c in enumerate(cols):
            if all(b < s or a > e for s, e in c):
                c.append((a, b))
                break
        else:
            cols.append([(a, b)])
            ci = len(cols) - 1
        bcol.append((label, a, b, ci))
    colw = [0.0] * len(cols)
    for label, a, b, ci in bcol:
        colw[ci] = max(colw[ci], 16 + tw(label) + 10)
    bx0 = x0 + cwid + 10
    colx = [bx0 + sum(colw[:i]) for i in range(len(cols))]
    nx = bx0 + sum(colw) + 12
    for i, r in enumerate(rows):
        yy, h = r["y"], r["h"]
        if r["kind"] == "cell":
            cid = f"cell{cellrows.index(r) + 1}"
            cv.box("cell", cid, x0, yy, cwid, h)
            cls = f"nd f-{r['fill']}" if r.get("fill") else "nd"
            cv.add(f'<rect class="{cls}" x="{fmt(x0)}" y="{fmt(yy)}" width="{fmt(cwid)}" height="{fmt(h)}"/>')
            cv.text(x0 + cwid / 2, yy + (h - nlines(r["label"]) * LH) / 2, r["label"], "middle", "t", cid)
            if r.get("size"):
                cv.text(x0 + cwid - 8, yy + (h - LH) / 2, r["size"], "end", "t mu", cid)
        elif r["kind"] == "gap":
            cid = f"cell{cellrows.index(r) + 1}"
            cv.box("cell", cid, x0, yy, cwid, h)
            cv.add(f'<rect class="nd gap" x="{fmt(x0)}" y="{fmt(yy)}" width="{fmt(cwid)}" height="{fmt(h)}"/>')
            cv.text(x0 + cwid / 2, yy + (h - LH) / 2, r["label"], "middle", "t mu", cid)
        else:
            ax = x0 + 16
            y1, y2 = (yy + h - 5, yy + 5) if r["dir"] == "up" else (yy + 5, yy + h - 5)
            cv.grow(ax, y1, ax, y2)
            cv.add(f'<path class="e" d="M{fmt(ax)} {fmt(y1)}V{fmt(y2)}" marker-end="url(#{pre}-ar)"/>')
            cv.text(x0 + 30, yy + (h - LH) / 2, r["label"], "start", "t mu")
        if r.get("off"):
            cv.text(ow, yy + (h - LH) / 2, r["off"], "end", "t mu")
        if r.get("note"):
            cv.text(nx, yy + (h - nlines(r["note"]) * LH) / 2, r["note"], "start", "t")
    for label, a, b, ci in bcol:
        ya, yb = cellrows[a - 1]["y"] + 3, cellrows[b - 1]["y"] + cellrows[b - 1]["h"] - 3
        bx = colx[ci]
        cv.grow(bx, ya, bx + 8, yb)
        cv.add(f'<path class="br" d="M{fmt(bx)} {fmt(ya)}H{fmt(bx + 8)}V{fmt(yb)}H{fmt(bx)}"/>')
        cv.text(bx + 16, (ya + yb) / 2 - nlines(label) * LH / 2, label, "start", "t")
    return f"Stack of {len(cells)} cells, top to bottom: " + "; ".join(r["label"].replace("\n", " ") for r in cells)


# ---------------------------------------------------------------- lanes

KINDS = {"shared": "f-a", "mut": "f-b", "owner": "f-c", "info": ""}
MARKS = ("error", "ok", "info")


def _lanes(opts, body, cv: Canvas, pre: str) -> str:
    lanes: dict[str, str] = {}
    ticks: dict[int, str] = {}
    spans, marks, msgs = [], [], []

    def lane(no, x):
        if x not in lanes:
            raise FigureError(no, f"unknown lane '{x}'")

    def num(no, x, what="column"):
        if not re.fullmatch(r"[1-9]\d*", x):
            raise FigureError(no, f"{what} must be a positive integer, got '{x}'")
        return int(x)

    for no, s in body:
        t = tokenize(no, s)
        head = t[0].val
        if head == "lane":
            pos, _ = _args(no, t[1:], ())
            if not pos or pos[0].kind != "word" or (len(pos) > 1 and pos[1].kind != "str") or len(pos) > 2:
                raise FigureError(no, 'lane syntax: lane ID "label"')
            if pos[0].val in lanes:
                raise FigureError(no, f"duplicate lane '{pos[0].val}'")
            lanes[pos[0].val] = pos[1].val if len(pos) == 2 else pos[0].val
        elif head == "tick":
            pos, _ = _args(no, t[1:], ())
            if len(pos) != 2 or pos[1].kind != "str":
                raise FigureError(no, 'tick syntax: tick N "label"')
            ticks[num(no, pos[0].val)] = pos[1].val
        elif head == "span":
            pos, kv = _args(no, t[1:], ("kind",))
            if len(pos) != 4 or pos[3].kind != "str":
                raise FigureError(no, 'span syntax: span LANE FROM TO "label" [kind=..]')
            lane(no, pos[0].val)
            a, b = num(no, pos[1].val), num(no, pos[2].val)
            if b < a:
                raise FigureError(no, "span ends before it starts")
            kind = kv.get("kind", "info")
            if kind not in KINDS:
                raise FigureError(no, f"span kind must be one of {', '.join(KINDS)}")
            spans.append((no, pos[0].val, a, b, pos[3].val, kind))
        elif head == "mark":
            pos, kv = _args(no, t[1:], ("kind",))
            if len(pos) != 3 or pos[2].kind != "str":
                raise FigureError(no, 'mark syntax: mark LANE N "label" [kind=error|ok]')
            lane(no, pos[0].val)
            kind = kv.get("kind", "info")
            if kind not in MARKS:
                raise FigureError(no, f"mark kind must be one of {', '.join(MARKS)}")
            marks.append((no, pos[0].val, num(no, pos[1].val), pos[2].val, kind))
        elif head == "msg":
            pos, kv = _args(no, t[1:], ("kind",))
            if len(pos) != 4 or pos[3].kind != "str":
                raise FigureError(no, 'msg syntax: msg FROMLANE TOLANE N "label" [kind=call|ret]')
            lane(no, pos[0].val)
            lane(no, pos[1].val)
            if pos[0].val == pos[1].val:
                raise FigureError(no, "msg needs two different lanes")
            if kv.get("kind", "call") not in ("call", "ret"):
                raise FigureError(no, "msg kind must be call or ret")
            msgs.append((no, pos[0].val, pos[1].val, num(no, pos[2].val), pos[3].val, kv.get("kind", "call")))
        else:
            raise FigureError(no, f"unknown directive '{head}'")
    if not lanes:
        raise FigureError(1, "no lanes")
    ncols = max([*ticks, *(s[3] for s in spans), *(m[2] for m in marks), *(m[3] for m in msgs), 1])
    colw = 70.0
    for _n, _l, a, b, label, _k in spans:
        colw = max(colw, (tw(label) + 18) / (b - a + 1) + 6)
    for m in marks:
        nxt = [o[2] for o in marks if o[1] == m[1] and o[2] > m[2]]
        colw = max(colw, (tw(m[3]) + 24) / (min(nxt) - m[2] if nxt else 1e9))
    for m in msgs:
        colw = max(colw, tw(m[4]) + 24)
    for n, lbl in ticks.items():
        if n + 1 in ticks:
            colw = max(colw, (tw(lbl) + tw(ticks[n + 1])) / 2 + 12)
    colw = math.ceil(colw)
    order = list(lanes)
    lw = max(tw(v) for v in lanes.values()) + 20
    x0 = lw
    gap = 26 if msgs else 10
    SH = 26
    layout = {}
    y = 0.0
    for lid in order:
        ls = sorted((s for s in spans if s[1] == lid), key=lambda s: (s[2], s[0]))
        rowsend: list[int] = []
        place = {}
        for s in ls:
            for ri, e in enumerate(rowsend):
                if s[2] > e:
                    rowsend[ri] = s[3]
                    place[s[0]] = ri
                    break
            else:
                rowsend.append(s[3])
                place[s[0]] = len(rowsend) - 1
        markrow = 26 if any(m[1] == lid for m in marks) else 0
        h = max(40, 8 + markrow + len(rowsend) * SH + 8)
        layout[lid] = (y, h, markrow, place)
        y += h + gap
    total = y - gap
    cx = lambda n: x0 + (n - 0.5) * colw  # noqa: E731
    for lid in order:
        ly, h, markrow, place = layout[lid]
        cv.box("lane", lid, x0, ly, ncols * colw, h)
        cv.add(f'<rect class="lane" x="{fmt(x0)}" y="{fmt(ly)}" width="{fmt(ncols * colw)}" height="{fmt(h)}" rx="4"/>')
        cv.text(x0 - 10, ly + h / 2 - LH / 2, lanes[lid], "end", "t")
    for n, lbl in sorted(ticks.items()):
        cv.add(f'<path class="grid" d="M{fmt(cx(n))} 0V{fmt(total + 10)}"/>')
        cv.grow(cx(n), 0, cx(n), total + 10)
    for no, lid, a, b, label, kind in spans:
        ly, h, markrow, place = layout[lid]
        sx, sy, sw, sh = x0 + (a - 1) * colw + 3, ly + 8 + markrow + place[no] * SH, (b - a + 1) * colw - 6, SH - 4
        sid = f"span{no}"
        cv.box("span", sid, sx, sy, sw, sh)
        cls = f"nd {KINDS[kind]}".strip()
        cv.add(f'<rect class="{cls}" x="{fmt(sx)}" y="{fmt(sy)}" width="{fmt(sw)}" height="{fmt(sh)}" rx="4"/>')
        cv.text(sx + sw / 2, sy + (sh - LH) / 2, label, "middle", "t", sid)
    for no, lid, n, label, kind in marks:
        ly, h, _mr, _p = layout[lid]
        mx, my = cx(n), ly + 8 + 11
        cv.box("mark", f"mark{no}", mx - 7, my - 7, 14, 14)
        cls = {"error": "mk f-e", "ok": "mk f-c", "info": "mk"}[kind]
        cv.add(f'<circle class="{cls}" cx="{fmt(mx)}" cy="{fmt(my)}" r="7"/>')
        glyph = {"error": f"M{fmt(mx - 3)} {fmt(my - 3)}l6 6m0 -6l-6 6", "ok": f"M{fmt(mx - 3)} {fmt(my)}l2.5 2.5l4 -5", "info": ""}[kind]
        if glyph:
            cv.add(f'<path class="gl" d="{glyph}"/>')
        cv.text(mx + 12, my - LH / 2, label, "start", "t")
    for no, a, b, n, label, kind in msgs:
        ia, ib = order.index(a), order.index(b)
        ay, ah = layout[a][0], layout[a][1]
        by, bh = layout[b][0], layout[b][1]
        x = cx(n)
        if ib > ia:
            ys, ye = ay + ah, by
            gy = ay + ah + gap / 2
        else:
            ys, ye = ay, by + bh
            gy = ay - gap / 2
        cv.grow(x, ys, x, ye)
        cv.add(f'<path class="{ecls("dash" if kind == "ret" else "")}" d="M{fmt(x)} {fmt(ys)}V{fmt(ye)}" marker-end="url(#{pre}-ar)"/>')
        t = cv.text(x + 8, gy - LH / 2, label, "start", "t lbl")
        cv.fig.labels.append(Box("label", f"msg{no}", t.x0, t.y0, t.x1 - t.x0, t.y1 - t.y0))
    if ticks:
        ay = total + 10
        cv.grow(x0, ay, x0 + ncols * colw, ay)
        cv.add(f'<path class="axis" d="M{fmt(x0)} {fmt(ay)}H{fmt(x0 + ncols * colw)}"/>')
        for n, lbl in sorted(ticks.items()):
            cv.add(f'<path class="axis" d="M{fmt(cx(n))} {fmt(ay)}v5"/>')
            cv.text(cx(n), ay + 7, lbl, "middle", "t mu")
    return f"Timeline with lanes {', '.join(lanes.values())}: " + "; ".join(s[4] for s in spans)


# ---------------------------------------------------------------- output

CSS = """svg.fig{--ink:#1f2328;--mu:#57606a;--line:#57606a;--nd:#f6f8fa;--nds:#8c959f;--halo:#fff;--lane:#f6f8fa;--grid:#d0d7de;--a:#dbeafe;--as:#2563eb;--b:#ffedd5;--bs:#c2410c;--c:#dcfce7;--cs:#15803d;--d:#f3e8ff;--ds:#7e22ce;--e:#fee2e2;--es:#b91c1c;max-width:100%;height:auto;font:13px system-ui,-apple-system,"Segoe UI",Roboto,sans-serif}
@media (prefers-color-scheme:dark){svg.fig{--ink:#e6edf3;--mu:#9da7b3;--line:#9da7b3;--nd:#21262d;--nds:#6e7681;--halo:#14161a;--lane:#1b1f24;--grid:#30363d;--a:#172a4a;--as:#58a6ff;--b:#3b2412;--bs:#f0883e;--c:#12301d;--cs:#3fb950;--d:#2b1d45;--ds:#bc8cff;--e:#451a1a;--es:#ff7b72}}
svg.fig .t{fill:var(--ink)}svg.fig .mu{fill:var(--mu)}svg.fig .gc{fill:var(--mu);font-weight:600}svg.fig .cap{fill:var(--mu);font-style:italic}
svg.fig .lbl{stroke:var(--halo);stroke-width:4px;paint-order:stroke;stroke-linejoin:round}
svg.fig .nd{fill:var(--nd);stroke:var(--nds);stroke-width:1.5}svg.fig .gap{stroke-dasharray:3 3}
svg.fig .grp{fill:none;stroke:var(--nds);stroke-width:1.2;stroke-dasharray:6 4}
svg.fig .lane{fill:var(--lane);stroke:none}svg.fig .grid{stroke:var(--grid);stroke-dasharray:2 4}
svg.fig .e{fill:none;stroke:var(--line);stroke-width:1.5}svg.fig .e.dash{stroke-dasharray:5 4}svg.fig .e.bold{stroke-width:3}
svg.fig .ah{fill:var(--line)}svg.fig .axis{stroke:var(--mu);fill:none}svg.fig .br{fill:none;stroke:var(--mu);stroke-width:1.5}
svg.fig .mk{fill:var(--nd);stroke:var(--nds);stroke-width:1.5}svg.fig .gl{fill:none;stroke:var(--ink);stroke-width:1.6;stroke-linecap:round}
svg.fig .f-a{fill:var(--a);stroke:var(--as)}svg.fig .f-b{fill:var(--b);stroke:var(--bs)}svg.fig .f-c{fill:var(--c);stroke:var(--cs)}svg.fig .f-d{fill:var(--d);stroke:var(--ds)}svg.fig .f-e{fill:var(--e);stroke:var(--es)}"""


def build(spec: str) -> Figure:
    kind, opts, body = _split(spec)
    pre = "f" + hashlib.sha1(spec.encode()).hexdigest()[:8]
    cv = Canvas()
    if kind == "graph":
        nodes, edges, groups, direction = _parse_graph(opts, body)
        desc = _graph(nodes, edges, groups, direction, cv, pre)
        default = "Graph"
    elif kind == "stack":
        if "dir" in opts:
            raise FigureError(opts["dir"][0], "dir is for graphs")
        desc = _stack(opts, body, cv, pre)
        default = "Stack layout"
    else:
        desc = _lanes(opts, body, cv, pre)
        default = "Timeline"
    if "scale" in opts and kind != "stack":
        raise FigureError(opts["scale"][0], "scale is for stacks")
    title = opts.get("title", (0, ""))[1]
    x0, y0, x1, y1 = cv.b
    if title:
        t = cv.text((x0 + x1) / 2, y1 + 12, title, "middle", "cap")
        x0, y0, x1, y1 = cv.b
    vb = (x0 - MARGIN, y0 - MARGIN, x1 - x0 + 2 * MARGIN, y1 - y0 + 2 * MARGIN)
    head = (
        f'<svg xmlns="http://www.w3.org/2000/svg" class="fig" role="img" aria-labelledby="{pre}-t {pre}-d" '
        f'viewBox="{fmt(vb[0])} {fmt(vb[1])} {fmt(vb[2])} {fmt(vb[3])}" style="width:{fmt(vb[2])}px;min-width:{fmt(vb[2])}px;max-width:{fmt(vb[2])}px">\n'
        f'<title id="{pre}-t">{_esc(title or default, quote=False)}</title>\n'
        f'<desc id="{pre}-d">{_esc(desc, quote=False)}</desc>\n<style>{CSS}</style>\n'
        f'<defs><marker id="{pre}-ar" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto">'
        f'<path class="ah" d="M0 0L10 5L0 10z"/></marker></defs>\n'
    )
    cv.fig.svg = head + "\n".join(cv.els) + "\n</svg>"
    cv.fig.viewbox = vb
    return cv.fig


def render(spec: str) -> str:
    return build(spec).svg
