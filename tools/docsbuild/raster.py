"""Draws the SVG that figures.py writes to a PNG, with nothing but the standard library.

Only the subset figures.py emits is understood: rect (with rx), circle, path (M L H V C Q Z),
text, the arrowhead marker, and the classes of its stylesheet (read from the same table, light
colours). Shapes are filled by scanline with four sub-scanlines a pixel row and exact
horizontal coverage; coverage is quantised to LEVELS so the picture fits a 256-colour
palette. The result is a pure function of the SVG text.
"""
from __future__ import annotations

import math
import re
import xml.etree.ElementTree as ET

from . import figures, png, ttf

SCALE = 1.5  # output pixels per SVG unit
SUB = 4  # sub-scanlines per pixel row
LEVELS = 4  # coverage steps
FONT_SIZE = 13.0
BOLD = 0.35  # a bold glyph is drawn twice, this many units apart
SLANT = 0.2  # italic shear
HALO = 3  # pixels a label's halo reaches around its glyphs
NUM = re.compile(r"[MLCQHVZhvlm]|-?\d+(?:\.\d+)?")
NS = "{http://www.w3.org/2000/svg}"


class PaletteFull(Exception):
    """More than 256 colours: draw again as RGB."""


# ---------------------------------------------------------------- styles


def _hex(c: str) -> tuple[int, int, int]:
    c = c.lstrip("#")
    if len(c) == 3:
        c = "".join(ch * 2 for ch in c)
    return int(c[0:2], 16), int(c[2:4], 16), int(c[4:6], 16)


def _rules() -> tuple[list[tuple[frozenset[str], dict[str, str]]], dict[str, str]]:
    """The class rules and the light colour variables of figures.CSS."""
    css = re.sub(r"@media[^{]*\{.*?\}\}", "", figures.CSS, flags=re.S)
    rules, colours = [], {}
    for selector, body in re.findall(r"([^{}]+)\{([^{}]*)\}", css):
        decl = dict(d.split(":", 1) for d in body.split(";") if ":" in d)
        classes = frozenset(re.findall(r"\.([\w-]+)", selector)) - {"fig"}
        if classes:
            rules.append((classes, decl))
        else:
            colours = {k[2:]: v for k, v in decl.items() if k.startswith("--")}
    return rules, colours


RULES, COLOURS = _rules()


def style(classes: str) -> dict[str, str]:
    have = set(classes.split())
    out: dict[str, str] = {}
    for need, decl in RULES:
        if need <= have:
            out.update(decl)
    return out


def paint(value: str | None) -> tuple[int, int, int] | None:
    if not value or value == "none":
        return None
    m = re.fullmatch(r"var\(--(\w+)\)", value)
    if m:
        return _hex(COLOURS[m.group(1)])
    return _hex(value)


# ---------------------------------------------------------------- canvas


class Canvas:
    def __init__(self, width: int, height: int, indexed: bool, background=(255, 255, 255)) -> None:
        self.w, self.h, self.indexed = width, height, indexed
        self.palette: list[tuple[int, int, int]] = []
        self.lookup: dict[tuple[int, int, int], int] = {}
        self.memo: dict[tuple, object] = {}
        if indexed:
            self.rows = [bytearray([self.index(background)]) * width for _ in range(height)]
        else:
            self.rows = [bytearray(bytes(background) * width) for _ in range(height)]

    def index(self, color: tuple[int, int, int]) -> int:
        i = self.lookup.get(color)
        if i is None:
            if len(self.palette) >= 256:
                raise PaletteFull
            i = self.lookup[color] = len(self.palette)
            self.palette.append(color)
        return i

    def run(self, y: int, x0: int, x1: int, color: tuple[int, int, int], q: int) -> None:
        """Paint pixels x0..x1 of row y with `color` at q/LEVELS opacity."""
        x0, x1 = max(x0, 0), min(x1, self.w)
        if x1 <= x0 or not 0 <= y < self.h or q <= 0:
            return
        row = self.rows[y]
        if q >= LEVELS:
            if self.indexed:
                row[x0:x1] = bytes([self.index(color)]) * (x1 - x0)
            else:
                row[3 * x0 : 3 * x1] = bytes(color) * (x1 - x0)
            return
        if self.indexed:
            for x in range(x0, x1):
                key = (row[x], color, q)
                new = self.memo.get(key)
                if new is None:
                    old = self.palette[row[x]]
                    new = self.memo[key] = self.index(_mix(old, color, q))
                row[x] = new
        else:
            for x in range(x0, x1):
                o = 3 * x
                row[o : o + 3] = bytes(_mix((row[o], row[o + 1], row[o + 2]), color, q))


def _mix(old, new, q: int) -> tuple[int, int, int]:
    return tuple((o * (LEVELS - q) + n * q + LEVELS // 2) // LEVELS for o, n in zip(old, new))


# ---------------------------------------------------------------- scan conversion

Segment = tuple[int, int, int, int]  # y, x0, x1, q


def scan(contours: list[list[tuple[float, float]]], width: int, height: int, nonzero: bool = True) -> list[Segment]:
    """Coverage of the closed `contours` as (row, x0, x1, level) runs, clipped to the canvas."""
    edges = []
    for pts in contours:
        for (x0, y0), (x1, y1) in zip(pts, [*pts[1:], pts[0]]):
            if y0 == y1:
                continue
            if y0 < y1:
                edges.append((y0, y1, x0, (x1 - x0) / (y1 - y0), 1))
            else:
                edges.append((y1, y0, x1, (x0 - x1) / (y0 - y1), -1))
    if not edges:
        return []
    edges.sort()
    first = max(0, int(math.floor(edges[0][0])))
    last = min(height - 1, int(math.ceil(max(e[1] for e in edges))))
    out: list[Segment] = []
    active: list[tuple] = []
    nxt = 0
    wt = 1.0 / SUB
    for y in range(first, last + 1):
        part: dict[int, float] = {}
        diff: dict[int, float] = {}
        for k in range(SUB):
            sy = y + (k + 0.5) * wt
            while nxt < len(edges) and edges[nxt][0] <= sy:
                active.append(edges[nxt])
                nxt += 1
            if active and any(e[1] <= sy for e in active):
                active = [e for e in active if e[1] > sy]
            if not active:
                continue
            xs = sorted((e[2] + (sy - e[0]) * e[3], e[4]) for e in active)
            winding, start = 0, 0.0
            for x, d in xs:
                inside = winding != 0 if nonzero else winding & 1
                winding += d
                now = winding != 0 if nonzero else winding & 1
                if not inside and now:
                    start = x
                elif inside and not now:
                    _span(start, x, width, wt, part, diff)
        if not part and not diff:
            continue
        events = sorted(part.keys() | diff.keys())
        level = 0.0
        for i, pos in enumerate(events):
            level += diff.get(pos, 0.0)
            end = events[i + 1] if i + 1 < len(events) else pos + 1
            if pos in part:
                q = min(LEVELS, round((level + part[pos]) * LEVELS))
                if q > 0:
                    out.append((y, pos, pos + 1, q))
                pos += 1
            q = min(LEVELS, round(level * LEVELS))
            if q > 0 and end > pos:
                out.append((y, pos, end, q))
    return out


def _span(xa: float, xb: float, width: int, wt: float, part: dict, diff: dict) -> None:
    xa, xb = max(xa, 0.0), min(xb, float(width))
    if xb <= xa:
        return
    ia, ib = int(xa), int(xb)
    if ia == ib:
        part[ia] = part.get(ia, 0.0) + (xb - xa) * wt
        return
    part[ia] = part.get(ia, 0.0) + (ia + 1 - xa) * wt
    if ib > ia + 1:
        diff[ia + 1] = diff.get(ia + 1, 0.0) + wt
        diff[ib] = diff.get(ib, 0.0) - wt
    if xb > ib:
        part[ib] = part.get(ib, 0.0) + (xb - ib) * wt


# ---------------------------------------------------------------- geometry


def _area(poly) -> float:
    return sum(x0 * y1 - x1 * y0 for (x0, y0), (x1, y1) in zip(poly, [*poly[1:], poly[0]])) / 2


def _positive(poly):
    return poly if _area(poly) >= 0 else poly[::-1]


def stroke(points: list[tuple[float, float]], closed: bool, width: float) -> list[list[tuple[float, float]]]:
    """Polygons whose union is the stroke of a polyline (butt ends, mitre joins up to 4)."""
    h = width / 2
    pts = [p for i, p in enumerate(points) if i == 0 or p != points[i - 1]]
    if closed and len(pts) > 1 and pts[0] == pts[-1]:
        pts.pop()
    if len(pts) < 2:
        return []
    n = len(pts)
    segs = [(pts[i], pts[(i + 1) % n]) for i in range(n if closed else n - 1)]
    dirs = []
    for (x0, y0), (x1, y1) in segs:
        length = math.hypot(x1 - x0, y1 - y0)
        dirs.append(((x1 - x0) / length, (y1 - y0) / length))
    out = []
    for ((x0, y0), (x1, y1)), (dx, dy) in zip(segs, dirs):
        nx, ny = -dy * h, dx * h
        out.append(_positive([(x0 + nx, y0 + ny), (x1 + nx, y1 + ny), (x1 - nx, y1 - ny), (x0 - nx, y0 - ny)]))
    joins = range(len(segs)) if closed else range(len(segs) - 1)
    for i in joins:
        d1, d2 = dirs[i], dirs[(i + 1) % len(segs)]
        px, py = segs[i][1]
        cross = d1[0] * d2[1] - d1[1] * d2[0]
        dot = d1[0] * d2[0] + d1[1] * d2[1]
        if abs(cross) < 1e-9 and dot > 0:
            continue
        s = -1 if cross > 0 else 1
        n1 = (-d1[1] * s, d1[0] * s)
        n2 = (-d2[1] * s, d2[0] * s)
        a = (px + n1[0] * h, py + n1[1] * h)
        b = (px + n2[0] * h, py + n2[1] * h)
        nd = n1[0] * n2[0] + n1[1] * n2[1]
        if 1 + nd > 1e-9 and math.sqrt(2 / (1 + nd)) <= 4:
            k = h / (1 + nd)
            poly = [(px, py), a, (px + (n1[0] + n2[0]) * k, py + (n1[1] + n2[1]) * k), b]
        else:
            poly = [(px, py), a, b]
        if abs(_area(poly)) > 1e-9:
            out.append(_positive(poly))
    return out


def dashes(points, closed: bool, pattern: list[float]) -> list[list[tuple[float, float]]]:
    """The 'on' pieces of a polyline cut by a dash pattern."""
    pts = list(points) + ([points[0]] if closed else [])
    out, cur = [], [pts[0]]
    i, on, left = 0, True, pattern[0]
    for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
        length = math.hypot(x1 - x0, y1 - y0)
        pos = 0.0
        while length - pos > left:
            pos += left
            t = pos / length
            p = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)
            if on:
                cur.append(p)
                out.append(cur)
            i = (i + 1) % len(pattern)
            on, left = not on, pattern[i]
            cur = [p] if on else []
        left -= length - pos
        if on:
            cur.append((x1, y1))
    if on and len(cur) > 1:
        out.append(cur)
    return out


def flatten_path(d: str, scale: float, ox: float, oy: float):
    """Subpaths of `d` as (points, closed), in output pixels."""
    toks = NUM.findall(d)
    arity = {"M": 2, "L": 2, "C": 6, "Q": 4, "H": 1, "V": 1, "Z": 0, "h": 1, "v": 1, "l": 2, "m": 2}
    subs: list[tuple[list, bool]] = []
    cur: list = []
    pos = (0.0, 0.0)
    start = pos
    i = 0

    def P(x, y):
        return ((float(x) - ox) * scale, (float(y) - oy) * scale)

    while i < len(toks):
        c = toks[i]
        n = arity[c]
        v = toks[i + 1 : i + 1 + n]
        i += 1 + n
        if c == "M":
            if cur:
                subs.append((cur, False))
            pos = start = P(*v)
            cur = [pos]
        elif c == "L":
            pos = P(*v)
            cur.append(pos)
        elif c == "H":
            pos = ((float(v[0]) - ox) * scale, pos[1])
            cur.append(pos)
        elif c == "V":
            pos = (pos[0], (float(v[0]) - oy) * scale)
            cur.append(pos)
        elif c == "m":
            if cur:
                subs.append((cur, False))
            pos = start = (pos[0] + float(v[0]) * scale, pos[1] + float(v[1]) * scale)
            cur = [pos]
        elif c == "h":
            pos = (pos[0] + float(v[0]) * scale, pos[1])
            cur.append(pos)
        elif c == "v":
            pos = (pos[0], pos[1] + float(v[0]) * scale)
            cur.append(pos)
        elif c == "l":
            pos = (pos[0] + float(v[0]) * scale, pos[1] + float(v[1]) * scale)
            cur.append(pos)
        elif c in "CQ":
            ctl = [P(v[k], v[k + 1]) for k in range(0, n, 2)]
            steps = max(8, int(sum(math.hypot(b[0] - a[0], b[1] - a[1]) for a, b in zip([pos, *ctl], ctl)) / 6))
            for k in range(1, steps + 1):
                t = k / steps
                u = 1 - t
                if c == "C":
                    p0, p1, p2, p3 = pos, *ctl
                    cur.append((u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0],
                                u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1]))
                else:
                    p0, p1, p2 = pos, *ctl
                    cur.append((u * u * p0[0] + 2 * u * t * p1[0] + t * t * p2[0], u * u * p0[1] + 2 * u * t * p1[1] + t * t * p2[1]))
            pos = ctl[-1]
        else:  # Z
            subs.append((cur, True))
            cur, pos = [], start
    if cur:
        subs.append((cur, False))
    return subs


def rounded_rect(x, y, w, h, r, steps: int = 8):
    r = min(r, w / 2, h / 2)
    if r <= 0:
        return [(x, y), (x + w, y), (x + w, y + h), (x, y + h)]
    pts = []
    for cx, cy, a0 in ((x + w - r, y + r, -90), (x + w - r, y + h - r, 0), (x + r, y + h - r, 90), (x + r, y + r, 180)):
        for k in range(steps + 1):
            a = math.radians(a0 + 90 * k / steps)
            pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return pts


# ---------------------------------------------------------------- text

_glyphs: dict[tuple, list[Segment]] = {}


def glyph_segments(gid: int, bold: bool, italic: bool, fx: float, fy: float, halo: bool) -> list[Segment]:
    """Coverage of a glyph (or its halo) at sub-pixel offset (fx, fy), origin at pixel (0, 0)."""
    key = (gid, bold, italic, fx, fy, halo)
    if key in _glyphs:
        return _glyphs[key]
    f = ttf.font()
    k = FONT_SIZE * SCALE / f.units
    contours = []
    for off in ((0.0, BOLD * SCALE) if bold else (0.0,)):
        for contour in f.outline(gid):
            poly = []
            for x, y in ttf.flatten(contour, 6):
                poly.append((x * k + (y * k * SLANT if italic else 0) + off + fx, -y * k + fy))
            contours.append(poly)
    pad = HALO + 2
    xs = [p[0] for c in contours for p in c] or [0.0]
    ys = [p[1] for c in contours for p in c] or [0.0]
    ox, oy = math.floor(min(xs)) - pad, math.floor(min(ys)) - pad
    w, h = math.ceil(max(xs)) - ox + pad, math.ceil(max(ys)) - oy + pad
    shifted = [[(x - ox, y - oy) for x, y in c] for c in contours]
    segs = scan(shifted, w, h)
    if halo:
        grid = [[0] * w for _ in range(h)]
        for y, a, b, q in segs:
            grid[y][a:b] = [q] * (b - a)
        wide = [[max(r[max(0, x - HALO) : x + HALO + 1]) for x in range(w)] for r in grid]
        cols = [[wide[y][x] for y in range(h)] for x in range(w)]
        tall = [[max(c[max(0, y - HALO) : y + HALO + 1]) for y in range(h)] for c in cols]
        segs = []
        for y in range(h):
            x = 0
            while x < w:
                q = tall[x][y]
                e = x
                while e < w and tall[e][y] == q:
                    e += 1
                if q:
                    segs.append((y, x, e, q))
                x = e
    out = [(y + oy, a + ox, b + ox, q) for y, a, b, q in segs]
    _glyphs[key] = out
    return out


def draw_text(cv: Canvas, text: str, x: float, y: float, anchor: str, st: dict, ox: float, oy: float) -> None:
    f = ttf.font()
    bold = st.get("font-weight") == "600"
    italic = st.get("font-style") == "italic"
    width = f.width(text, FONT_SIZE)
    x -= width / 2 if anchor == "middle" else width if anchor == "end" else 0
    fill = paint(st.get("fill")) or (0, 0, 0)
    halo = paint(st.get("stroke")) if "paint-order" in st else None
    pen = x
    placed = []
    for ch in text:
        gid = f.glyph(ch)
        px, py = (pen - ox) * SCALE, (y - oy) * SCALE
        ix, iy = math.floor(px), math.floor(py)
        fx, fy = round((px - ix) * 2) / 2, round((py - iy) * 2) / 2
        placed.append((gid, ix, iy, fx, fy))
        pen += f.advance(gid) * FONT_SIZE / f.units
    for layer, color in (((True, halo),) if halo else ()) + ((False, fill),):
        for gid, ix, iy, fx, fy in placed:
            for sy, a, b, q in glyph_segments(gid, bold, italic, fx, fy, layer):
                cv.run(sy + iy, a + ix, b + ix, color, q)


# ---------------------------------------------------------------- the SVG


def _parse_dash(st: dict) -> list[float] | None:
    v = st.get("stroke-dasharray")
    return [float(t) * SCALE for t in v.split()] if v else None


def _draw_shape(cv, subs, st):
    """Fill the closed subpaths, then stroke them, per the style `st` (pixels)."""
    fill = paint(st.get("fill", "#000"))
    if fill:
        for y, a, b, q in scan([p for p, _c in subs if len(p) > 2], cv.w, cv.h):
            cv.run(y, a, b, fill, q)
    color = paint(st.get("stroke"))
    width = float(st.get("stroke-width", "1").replace("px", "")) * SCALE
    if color and width > 0:
        pattern = _parse_dash(st)
        polys = []
        for pts, closed in subs:
            for piece, c in (((pts, closed),) if not pattern else [(d, False) for d in dashes(pts, closed, pattern)]):
                polys += stroke(piece, c, width)
        for y, a, b, q in scan(polys, cv.w, cv.h):
            cv.run(y, a, b, color, q)


def _arrow(cv, subs, st, sw: float) -> None:
    pts = subs[-1][0]
    (x1, y1) = pts[-1]
    prev = next((p for p in reversed(pts[:-1]) if math.hypot(x1 - p[0], y1 - p[1]) > 1e-6), None)
    if prev is None:
        return
    ang = math.atan2(y1 - prev[1], x1 - prev[0])
    s = 0.7 * sw  # the marker is 7 stroke widths wide for its 10 units
    ca, sa = math.cos(ang), math.sin(ang)
    tri = []
    for mx, my in ((0, 0), (10, 5), (0, 10)):
        ux, uy = (mx - 9) * s, (my - 5) * s
        tri.append((x1 + ux * ca - uy * sa, y1 + ux * sa + uy * ca))
    color = paint("var(--line)")
    for y, a, b, q in scan([_positive(tri)], cv.w, cv.h):
        cv.run(y, a, b, color, q)


def draw(svg: str, indexed: bool) -> Canvas:
    root = ET.fromstring(svg)
    vx, vy, vw, vh = (float(v) for v in root.get("viewBox").split())
    cv = Canvas(math.ceil(vw * SCALE), math.ceil(vh * SCALE), indexed)
    for el in root:
        tag = el.tag.replace(NS, "")
        st = style(el.get("class", ""))
        if tag == "rect":
            x, y, w, h = (float(el.get(k)) for k in ("x", "y", "width", "height"))
            r = float(el.get("rx", 0))
            pts = [((px - vx) * SCALE, (py - vy) * SCALE) for px, py in rounded_rect(x, y, w, h, r, max(4, int(r * SCALE / 3)))]
            _draw_shape(cv, [(pts, True)], st)
        elif tag == "circle":
            cx, cy, r = (float(el.get(k)) for k in ("cx", "cy", "r"))
            pts = [((cx + r * math.cos(a * math.tau / 32) - vx) * SCALE, (cy + r * math.sin(a * math.tau / 32) - vy) * SCALE) for a in range(32)]
            _draw_shape(cv, [(pts, True)], st)
        elif tag == "path":
            subs = flatten_path(el.get("d"), SCALE, vx, vy)
            _draw_shape(cv, subs, st)
            if ("marker-end" in st or el.get("marker-end")) and subs:
                _arrow(cv, subs, st, float(st.get("stroke-width", "1")) * SCALE)
        elif tag == "text":
            cls = el.get("class", "").split()
            anchor = el.get("text-anchor") or ("end" if "x" in cls else "middle" if "m" in cls else "start")
            draw_text(cv, el.text or "", float(el.get("x")), float(el.get("y")), anchor, st, vx, vy)
    return cv


def render_png(svg: str, key: str | None = None) -> bytes:
    """The PNG of `svg`: indexed colour when 256 colours are enough, RGB otherwise; `key` rides in a tEXt chunk."""
    text = ("llrm-fig", key) if key else None
    try:
        cv = draw(svg, True)
        return png.encode(cv.w, cv.h, cv.rows, cv.palette, text)
    except PaletteFull:
        cv = draw(svg, False)
        return png.encode(cv.w, cv.h, cv.rows, None, text)
