"""Figure spec to mermaid text, and the one shared mermaid style of the docs.

`convert(spec)` returns a `Result`: the mermaid text of a ```mermaid fence, the figure's title
(the caption under it) and any extra caption text. The spec language is the one of the
retired PNG pipeline: `kind: graph|stack|lanes`, one directive per line.

This module owns the style: `INIT` and the per-diagram init strings, `CLASSES`, `TINT`.
Documents and the lint (`lint.py`) refer to these names; nothing else spells a colour.
"""
from __future__ import annotations

import re
from dataclasses import dataclass, field

FONT = "Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif"
_BASE = (
    f"'theme':'base','themeVariables':{{'fontFamily':'{FONT}','fontSize':'15px',"
    "'primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155',"
    "'lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent',"
    "'clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155',"
    "'textColor':'#64748b','titleColor':'#64748b'}"
)
INIT = (
    "%%{init: {" + _BASE + ",'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,"
    "'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%"
)
BLOCK_INIT = (
    f"%%{{init: {{'theme':'base','themeVariables':{{'fontFamily':'{FONT}','fontSize':'15px',"
    "'primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155',"
    "'lineColor':'#64748b','textColor':'#64748b'}}}%%"
)
GANTT_INIT = (
    f"%%{{init: {{'theme':'base','themeVariables':{{'fontFamily':'{FONT}','fontSize':'15px',"
    "'textColor':'#64748b','titleColor':'#64748b','gridColor':'#94a3b8','sectionBkgColor':'transparent',"
    "'sectionBkgColor2':'transparent','altSectionBkgColor':'transparent','taskTextColor':'#ffffff',"
    "'taskTextOutsideColor':'#64748b','taskTextLightColor':'#ffffff','taskTextDarkColor':'#ffffff',"
    "'doneTaskBkgColor':'#16a34a','doneTaskBorderColor':'#166534','activeTaskBkgColor':'#2563eb',"
    "'activeTaskBorderColor':'#1e40af','critBkgColor':'#dc2626','critBorderColor':'#991b1b',"
    "'taskBkgColor':'#475569','taskBorderColor':'#334155','todayLineColor':'transparent'},'gantt':{'barHeight':34,'barGap':12,'topPadding':48,"
    "'leftPadding':150,'fontSize':14,'sectionFontSize':14,'numberSectionStyles':2}}}%%"
)
SEQUENCE_INIT = (
    f"%%{{init: {{'theme':'base','themeVariables':{{'fontFamily':'{FONT}','fontSize':'15px',"
    "'actorBkg':'#2563eb','actorBorder':'#1e40af','actorTextColor':'#ffffff','actorLineColor':'#94a3b8',"
    "'signalColor':'#64748b','signalTextColor':'#64748b','noteBkgColor':'#ea580c','noteBorderColor':'#9a3412',"
    "'noteTextColor':'#ffffff','activationBkgColor':'#cbd5e1','activationBorderColor':'#64748b',"
    "'lineColor':'#64748b'},'sequence':{'mirrorActors':false,'messageAlign':'center','actorMargin':70,"
    "'boxMargin':10,'width':170,'height':46,'noteMargin':12}}}%%"
)
# diagram type -> its init line
INITS = {
    "flowchart": INIT,
    "block-beta": BLOCK_INIT,
    "gantt": GANTT_INIT,
    "sequenceDiagram": SEQUENCE_INIT,
}

def normalize_init(fence_text: str) -> str:
    """`fence_text` with its first `%%{init` line replaced by the INITS entry of its diagram type.

    Only flowchart, block-beta, gantt and sequenceDiagram figures change; others and figures without
    an init line are returned as they are.
    """
    lines = fence_text.split("\n")
    at = next((i for i, line in enumerate(lines) if line.startswith("%%{init")), None)
    if at is None:
        return fence_text
    kind = next((w for line in lines[at + 1 :] if line.strip() for w in line.split()[:1]), None)
    if kind not in INITS:
        return fence_text
    lines[at] = INITS[kind]
    return "\n".join(lines)


# class -> (fill, stroke, text); the meaning is in docs/working/figures.md
CLASSES = {
    "src": ("#2563eb", "#1e40af", "#ffffff"),
    "mid": ("#475569", "#334155", "#ffffff"),
    "opt": ("#ea580c", "#9a3412", "#ffffff"),
    "out": ("#16a34a", "#166534", "#ffffff"),
    "data": ("#7c3aed", "#5b21b6", "#ffffff"),
    "warn": ("#dc2626", "#991b1b", "#ffffff"),
    "note": ("transparent", "transparent", "#64748b"),
}
# block-beta only
BLOCK_CLASSES = {
    "off": ("transparent", "transparent", "#64748b"),
    "size": ("#1e293b", "#0f172a", "#e2e8f0"),
    "gap": ("#334155", "#64748b", "#e2e8f0"),
}
FILL_CLASS = {"a": "src", "b": "opt", "c": "out", "d": "data", "e": "warn", "": "mid"}
# group tint: the stroke colour of a class, with the fill at 7% alpha (hex 12)
TINT = {c: v[0] for c, v in CLASSES.items() if c != "note"}
LINK_STYLE = "linkStyle default stroke:#64748b,stroke-width:2.2px"
RESERVED = {
    "end", "graph", "subgraph", "direction", "class", "classdef", "style", "click", "default",
    "linkstyle", "call", "flowchart", "interpolate", "href", "participant", "note",
}
ARROWS = {"->": "-->", "..>": "-.->", "==>": "==>"}


class FigureError(Exception):
    """A spec error, with the 1-based line of the spec it refers to."""

    def __init__(self, line: int, message: str):
        super().__init__(f"line {line}: {message}")
        self.line = line
        self.message = message


@dataclass
class Result:
    text: str
    title: str = ""
    extra: str = ""  # caption text that the diagram cannot hold (axis labels)

    def caption(self) -> str:
        parts = [p for p in (f"*{self.title}*" if self.title else "", self.extra) if p]
        return " ".join(parts)


# ---------------------------------------------------------------- escaping

_ENT = {'"': "quot", "#": "35", ";": "59", "<": "lt", ">": "gt", "{": "123", "}": "125", "|": "124", "`": "96"}


def esc(s: str) -> str:
    """Text safe inside a quoted mermaid label; `\\n` stays a line break as <br/>."""
    out = "".join(f"#{_ENT[c]};" if c in _ENT else c for c in s.replace("\n", "\0"))
    return out.replace("\0", "<br/>")


def esc_plain(s: str) -> str:
    """Text for a gantt task: no entities are decoded there, so look-alikes replace the syntax characters."""
    return re.sub(r"\s+", " ", s).replace(":", "∶").replace(";", "；").replace("#", "＃")


def ident(name: str) -> str:
    out = re.sub(r"\W", "_", name)
    return out + "_" if out.lower() in RESERVED else out


def label_html(label: str, plain: bool = False) -> str:
    """A node label: bold first line, the rest small; `plain` for a note with no emphasis."""
    lines = [esc(x) for x in label.split("\n")]
    if plain:
        return "<br/>".join(lines)
    out = f"<b>{lines[0]}</b>"
    if len(lines) > 1:
        out += "<br/><small>" + "<br/>".join(lines[1:]) + "</small>"
    return out


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


def split(spec: str):
    """(kind, options, body lines) of a spec; options title, dir, scale."""
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


# ---------------------------------------------------------------- graph


@dataclass
class Node:
    id: str
    label: str
    fill: str
    shape: str
    line: int


@dataclass
class Edge:
    a: str
    b: str
    label: str
    arrow: str
    line: int


@dataclass
class Group:
    name: str
    caption: str
    members: list[str]
    line: int


def parse_graph(opts, body):
    nodes: dict[str, Node] = {}
    edges: list[Edge] = []
    groups: list[Group] = []
    d = opts.get("dir", (0, "down"))
    if d[1] not in ("down", "right"):
        raise FigureError(d[0], "dir must be right or down")
    for no, s in body:
        t = tokenize(no, s)
        head = t[0].val if t[0].kind == "word" else ""
        if head == "node":
            pos, kv = _args(no, t[1:], ("fill", "shape"))
            if not pos or pos[0].kind != "word" or not re.fullmatch(r"[A-Za-z_][\w.]*", pos[0].val):
                raise FigureError(no, 'node needs an id: node ID "label"')
            if len(pos) > 2 or (len(pos) == 2 and pos[1].kind != "str"):
                raise FigureError(no, "node takes an id and one quoted label")
            nid = pos[0].val
            if nid in nodes:
                raise FigureError(no, f"duplicate node '{nid}'")
            fill, shape = kv.get("fill", ""), kv.get("shape", "box")
            if fill and fill not in "abcde":
                raise FigureError(no, "fill must be one of a, b, c, d, e")
            if shape not in ("box", "round", "note"):
                raise FigureError(no, "shape must be one of box, round, note")
            nodes[nid] = Node(nid, pos[1].val if len(pos) == 2 else nid, fill, shape, no)
        elif head == "edge":
            pos, _ = _args(no, t[1:], ())
            if len(pos) < 3 or pos[0].kind != "word" or pos[2].kind != "word" or pos[1].val not in ARROWS:
                raise FigureError(no, 'edge syntax: edge A -> B "label" (arrows: -> ..> ==>)')
            if len(pos) > 4 or (len(pos) == 4 and pos[3].kind != "str"):
                raise FigureError(no, "edge takes two ids and one quoted label")
            edges.append(Edge(pos[0].val, pos[2].val, pos[3].val if len(pos) == 4 else "", pos[1].val, no))
        elif head == "group":
            if len(t) < 5 or t[1].kind != "word" or t[2].kind != "str" or t[3].kind != "colon":
                raise FigureError(no, 'group syntax: group NAME "caption": ID ID ...')
            if any(x.kind != "word" for x in t[4:]):
                raise FigureError(no, "group members are node ids")
            groups.append(Group(t[1].val, t[2].val, [x.val for x in t[4:]], no))
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


def node_text(n: Node) -> str:
    """One node declaration: `id["label"]:::class`."""
    nid = ident(n.id)
    if n.shape == "note":
        return f'{nid}["{label_html(n.label, plain=True)}"]:::note'
    cls = FILL_CLASS[n.fill]
    body = f'"{label_html(n.label)}"'
    return f"{nid}([{body}]):::{cls}" if n.shape == "round" else f"{nid}[{body}]:::{cls}"


def edge_text(e: Edge) -> str:
    arrow = ARROWS[e.arrow]
    mid = f'{arrow}|"{esc(e.label)}"|' if e.label else arrow
    return f"{ident(e.a)} {mid} {ident(e.b)}"


FAN_IN = 3  # edges from one node into one group that are drawn as a single edge to the group


def edge_lines(edges: list[Edge], groups: list[Group], members: dict[str, set[str]]) -> list[str]:
    """Edge declarations; a fan of FAN_IN or more equal edges into one group ends at the group's border.

    Otherwise every edge crosses the group's caption on its way to a node inside.
    """
    def outer(a: str, b: str) -> str | None:
        out = [g for g in groups if b in members[g.name] and a not in members[g.name]]
        return max(out, key=lambda g: len(g.members)).name if out else None

    fans: dict[tuple, list[int]] = {}
    for i, e in enumerate(edges):
        g = outer(e.a, e.b)
        if g:
            fans.setdefault((e.a, g, e.arrow, e.label), []).append(i)
    skip: set[int] = set()
    first: dict[int, str] = {}
    for (a, g, arrow, label), idx in fans.items():
        if len(idx) >= FAN_IN:
            skip.update(idx[1:])
            first[idx[0]] = f"{ident(a)} {ARROWS[arrow]}{f'|\"{esc(label)}\"|' if label else ''} {gid(g)}"
    return [first.get(i) or edge_text(e) for i, e in enumerate(edges) if i not in skip]


def gid(name: str) -> str:
    return "g_" + re.sub(r"\W", "_", name)


def tint_of(classes: list[str]) -> str:
    """The colour of a group: that of the commonest class among its nodes, grey when it is `mid`."""
    counts: dict[str, int] = {}
    for c in classes:
        if c != "note":
            counts[c] = counts.get(c, 0) + 1
    if not counts:
        return TINT["mid"]
    best = max(counts.items(), key=lambda kv: (kv[1], kv[0] != "mid"))[0]
    return TINT[best]


def graph(opts, body) -> Result:
    nodes, edges, groups, direction = parse_graph(opts, body)
    members = {g.name: set(g.members) for g in groups}
    # a node sits in the smallest group that holds it; a group nests in the smallest group that holds it all
    home: dict[str, str] = {}
    for nid in nodes:
        mine = [g for g in groups if nid in g.members]
        if mine:
            home[nid] = min(mine, key=lambda g: (len(g.members), g.line)).name
    parent: dict[str, str | None] = {}
    for g in groups:
        up = [h for h in groups if h is not g and members[g.name] < members[h.name]]
        parent[g.name] = min(up, key=lambda h: len(h.members)).name if up else None
    lines: list[str] = []
    used: set[str] = set()
    done: set[str] = set()
    styles: list[str] = []

    def all_nodes(g: Group) -> list[str]:
        return [n for n in nodes if n in members[g.name]]

    def emit(g: Group, depth: int) -> None:
        pad = "  " * depth
        done.add(g.name)
        lines.append(f'{pad}subgraph {gid(g.name)}["{esc(g.caption.upper())}"]')
        for h in groups:
            if parent[h.name] == g.name and h.name not in done:
                emit(h, depth + 1)
        for nid in nodes:
            if home.get(nid) == g.name:
                lines.append(f"{pad}  {node_text(nodes[nid])}")
                used.add(FILL_CLASS[nodes[nid].fill] if nodes[nid].shape != "note" else "note")
        lines.append(f"{pad}end")
        col = tint_of([FILL_CLASS[nodes[n].fill] if nodes[n].shape != "note" else "note" for n in all_nodes(g)])
        styles.append(
            f"style {gid(g.name)} fill:{col}12,stroke:{col},stroke-width:1.6px,stroke-dasharray:7 5,color:{col}"
        )

    by_name = {g.name: g for g in groups}
    for nid, n in nodes.items():
        if nid in home:
            top = home[nid]
            while parent[top]:
                top = parent[top]
            if top not in done:
                emit(by_name[top], 1)
        else:
            lines.append(f"  {node_text(n)}")
            used.add(FILL_CLASS[n.fill] if n.shape != "note" else "note")
    lines += [f"  {t}" for t in edge_lines(edges, groups, members)]
    head = [INIT, f"flowchart {'LR' if direction == 'right' else 'TB'}"]
    for c in CLASSES:
        if c in used:
            f, s, t = CLASSES[c]
            if c == "note":
                head.append(f"  classDef note fill:{f},stroke:{s},color:{t}")
            else:
                head.append(f"  classDef {c} fill:{f},stroke:{s},stroke-width:2px,color:{t},rx:12px,ry:12px")
    head.append(f"  {LINK_STYLE}")
    out = head + lines + [f"  {s}" for s in styles]
    return Result("\n".join(out), opts.get("title", (0, ""))[1])


# ---------------------------------------------------------------- stack


def stack(opts, body) -> Result:
    rows: list[dict] = []
    brackets: list[tuple[int, str, int, int]] = []
    for no, s in body:
        t = tokenize(no, s)
        head = t[0].val
        if head == "cell":
            pos, kv = _args(no, t[1:], ("off", "size", "note", "fill"))
            if len(pos) != 1 or pos[0].kind != "str":
                raise FigureError(no, 'cell syntax: cell "label" [off=..] [size=..] [note=..] [fill=a..e]')
            if kv.get("fill") and kv["fill"] not in "abcde":
                raise FigureError(no, "fill must be one of a, b, c, d, e")
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
    # a bracket sits in the first column where it overlaps no other; its label joins the middle cell's note
    cols: list[list[tuple[int, int]]] = []
    where: list[tuple[int, int, int]] = []
    for _no, _label, a, b in sorted(brackets, key=lambda t: (t[2], t[3])):
        for ci, c in enumerate(cols):
            if all(b < s or a > e for s, e in c):
                c.append((a, b))
                break
        else:
            cols.append([(a, b)])
            ci = len(cols) - 1
        where.append((a, b, ci))
    nb = len(cols)
    notes: dict[int, list[str]] = {}
    for _no, label, a, b in brackets:
        notes.setdefault((a + b) // 2, []).append(f"<b>{esc(label)}</b>")
    width = 1 + 3 + 1 + nb + 3
    lines = [BLOCK_INIT, "block-beta", f"  columns {width}"]
    classes: dict[str, list[str]] = {}

    def put(cls: str, bid: str) -> None:
        classes.setdefault(cls, []).append(bid)

    n = 0
    for r in rows:
        if r["kind"] == "arrow":
            sym = "▲" if r["dir"] == "up" else "▼"
            lines.append(f'  space:1 a{r["line"]}["{sym} {esc(r["label"])}"]:3 space:{1 + nb + 3}')
            put("off", f'a{r["line"]}')
            continue
        n += 1
        label = esc(r["label"]) if r["kind"] == "cell" else esc(r["label"])
        if r["kind"] == "gap":
            label = esc(r["label"])
        off = esc(r.get("off", "")) or " "
        size = esc(r.get("size", "")) or " "
        note_parts = ([esc(r["note"])] if r.get("note") else []) + notes.get(n, [])
        note = "<br/>".join(note_parts) or " "
        cls = "gap" if r["kind"] == "gap" else FILL_CLASS[r.get("fill", "")]
        if cls == "mid":
            cls = "cell"
        cells_txt = [f'o{n}["{off}"]', f'c{n}["{label}"]:3', f's{n}["{size}"]']
        for ci in range(nb):
            glyph = " "
            for a, b, c2 in where:
                if c2 == ci and a <= n <= b:
                    glyph = "┐" if a == b == n else "┐" if n == a else "┘" if n == b else "│"
            cells_txt.append(f'b{n}_{ci}["{glyph}"]')
            put("off", f"b{n}_{ci}")
        cells_txt.append(f'n{n}["{note}"]:3')
        lines.append("  " + " ".join(cells_txt))
        put("off", f"o{n}")
        put("off", f"n{n}")
        put(cls, f"c{n}")
        put("size" if r.get("size") else "off", f"s{n}")
    defs = {**CLASSES, **BLOCK_CLASSES, "cell": CLASSES["mid"]}
    for cls, ids in classes.items():
        f, s, t = defs[cls]
        extra = ",stroke-dasharray:4 3" if cls == "gap" else ""
        lines.append(f"  classDef {cls} fill:{f},stroke:{s},stroke-width:2px,color:{t}{extra}")
    for cls, ids in classes.items():
        lines.append(f"  class {','.join(ids)} {cls}")
    return Result("\n".join(lines), opts.get("title", (0, ""))[1])


# ---------------------------------------------------------------- lanes

SPAN_TAG = {"owner": "done, ", "shared": "active, ", "mut": "crit, ", "info": ""}
MARK_KINDS = ("error", "ok", "info")
MARK_TAG = {"error": "crit, ", "ok": "done, ", "info": ""}
MARK_GLYPH = {"error": "✗ ", "ok": "✓ ", "info": ""}


def lanes(opts, body) -> Result:
    names: dict[str, str] = {}
    ticks: dict[int, str] = {}
    spans, marks, msgs = [], [], []

    def lane(no, x):
        if x not in names:
            raise FigureError(no, f"unknown lane '{x}'")

    def num(no, x):
        if not re.fullmatch(r"[1-9]\d*", x):
            raise FigureError(no, f"column must be a positive integer, got '{x}'")
        return int(x)

    for no, s in body:
        t = tokenize(no, s)
        head = t[0].val
        if head == "lane":
            pos, _ = _args(no, t[1:], ())
            if not pos or pos[0].kind != "word" or (len(pos) > 1 and pos[1].kind != "str") or len(pos) > 2:
                raise FigureError(no, 'lane syntax: lane ID "label"')
            if pos[0].val in names:
                raise FigureError(no, f"duplicate lane '{pos[0].val}'")
            names[pos[0].val] = pos[1].val if len(pos) == 2 else pos[0].val
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
            if kind not in SPAN_TAG:
                raise FigureError(no, f"span kind must be one of {', '.join(SPAN_TAG)}")
            spans.append((no, pos[0].val, a, b, pos[3].val, kind))
        elif head == "mark":
            pos, kv = _args(no, t[1:], ("kind",))
            if len(pos) != 3 or pos[2].kind != "str":
                raise FigureError(no, 'mark syntax: mark LANE N "label" [kind=error|ok]')
            lane(no, pos[0].val)
            kind = kv.get("kind", "info")
            if kind not in MARK_KINDS:
                raise FigureError(no, f"mark kind must be one of {', '.join(MARK_KINDS)}")
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
    if not names:
        raise FigureError(1, "no lanes")
    title = opts.get("title", (0, ""))[1]
    return sequence(names, spans, marks, msgs, title) if msgs else gantt(names, ticks, spans, marks, title)


def sequence(names, spans, marks, msgs, title) -> Result:
    lines = [SEQUENCE_INIT, "sequenceDiagram"]
    for lid, label in names.items():
        lines.append(f'  participant {ident(lid)} as {esc(label)}')
    events = (
        [(m[3], 1, m[0], f"  {ident(m[1])}{'-->>' if m[5] == 'ret' else '->>'}{ident(m[2])}: {esc(m[4])}") for m in msgs]
        + [(m[2], 0, m[0], f"  Note over {ident(m[1])}: {MARK_GLYPH[m[4]]}{esc(m[3])}") for m in marks]
        + [(s[2], 0, s[0], f"  Note over {ident(s[1])}: {esc(s[4])}") for s in spans]
    )
    lines += [e[3] for e in sorted(events)]
    return Result("\n".join(lines), title)


def gantt(names, ticks, spans, marks, title) -> Result:
    lines = [GANTT_INIT, "gantt", "  dateFormat X", "  axisFormat %s", "  tickInterval 1second"]
    for lid, label in names.items():
        lines.append(f"  section {esc_plain(label)}")
        items = [(s[2], s[0], f"  {esc_plain(s[4])} :{SPAN_TAG[s[5]]}t{s[0]}, {s[2]}, {max(s[3], s[2] + 1)}")
                 for s in spans if s[1] == lid]
        items += [(m[2], m[0], f"  {MARK_GLYPH[m[4]]}{esc_plain(m[3])} :milestone, {MARK_TAG[m[4]]}t{m[0]}, {m[2]}, {m[2]}")
                  for m in marks if m[1] == lid]
        lines += [i[2] for i in sorted(items)]
    odd = [f"{n} {lbl}" for n, lbl in sorted(ticks.items()) if lbl != str(n)]
    extra = ("Axis: " + "; ".join(f"{n} {lbl}" for n, lbl in sorted(ticks.items()) if lbl != str(n)) + ".") if odd else ""
    return Result("\n".join(lines), title, extra)


# ---------------------------------------------------------------- entry


def convert(spec: str) -> Result:
    kind, opts, body = split(spec)
    if kind == "graph":
        return graph(opts, body)
    if kind == "stack":
        if "dir" in opts:
            raise FigureError(opts["dir"][0], "dir is for graphs")
        return stack(opts, body)
    return lanes(opts, body)
