"""Just enough TrueType: character map, advance widths and glyph outlines, in pure Python."""
from __future__ import annotations

import struct
from functools import lru_cache
from pathlib import Path

FONT = Path(__file__).resolve().parent / "fonts" / "DejaVuSans.ttf"
U16 = (1 << 16) - 1  # largest 16-bit value: the mask of glyph arithmetic and the cmap's end marker
Contour = list[tuple[float, float, bool]]  # x, y (y up, font units), on the curve


class Font:
    def __init__(self, data: bytes) -> None:
        self.data = data
        count = struct.unpack(">H", data[4:6])[0]
        self.tables: dict[str, int] = {}
        for i in range(count):
            tag, _sum, off, _len = struct.unpack(">4sIII", data[12 + 16 * i : 28 + 16 * i])
            self.tables[tag.decode("latin1")] = off
        head = self.tables["head"]
        self.units = struct.unpack(">H", data[head + 18 : head + 20])[0]
        self.long_loca = struct.unpack(">h", data[head + 50 : head + 52])[0] == 1
        self.glyphs = struct.unpack(">H", data[self.tables["maxp"] + 4 :][:2])[0]
        hhea = self.tables["hhea"]
        self.ascent, self.descent = struct.unpack(">hh", data[hhea + 4 : hhea + 8])
        self.metrics = struct.unpack(">H", data[hhea + 34 : hhea + 36])[0]
        hmtx = self.tables["hmtx"]
        self.advances = [struct.unpack(">H", data[hmtx + 4 * i : hmtx + 4 * i + 2])[0] for i in range(self.metrics)]
        self._cmap: dict[int, int] | None = None

    # ---- character map
    @property
    def cmap(self) -> dict[int, int]:
        if self._cmap is None:
            self._cmap = self._read_cmap()
        return self._cmap

    def _read_cmap(self) -> dict[int, int]:
        d, base = self.data, self.tables["cmap"]
        n = struct.unpack(">H", d[base + 2 : base + 4])[0]
        found: dict[int, int] = {}
        for i in range(n):
            plat, enc, off = struct.unpack(">HHI", d[base + 4 + 8 * i : base + 12 + 8 * i])
            fmt = struct.unpack(">H", d[base + off : base + off + 2])[0]
            if (plat, enc, fmt) in ((3, 10, 12), (0, 4, 12), (3, 1, 4), (0, 3, 4)):
                found[fmt] = base + off
        out: dict[int, int] = {}
        if 4 in found:
            out.update(self._format4(found[4]))
        if 12 in found:
            out.update(self._format12(found[12]))
        return out

    def _format4(self, at: int) -> dict[int, int]:
        d = self.data
        segs = struct.unpack(">H", d[at + 6 : at + 8])[0] // 2
        ends = struct.unpack(f">{segs}H", d[at + 14 : at + 14 + 2 * segs])
        p = at + 16 + 2 * segs
        starts = struct.unpack(f">{segs}H", d[p : p + 2 * segs])
        deltas = struct.unpack(f">{segs}h", d[p + 2 * segs : p + 4 * segs])
        ro = p + 4 * segs
        offsets = struct.unpack(f">{segs}H", d[ro : ro + 2 * segs])
        out = {}
        for s in range(segs):
            for c in range(starts[s], ends[s] + 1):
                if c == U16:
                    continue
                if offsets[s] == 0:
                    g = (c + deltas[s]) & U16
                else:
                    q = ro + 2 * s + offsets[s] + 2 * (c - starts[s])
                    g = struct.unpack(">H", d[q : q + 2])[0]
                    g = (g + deltas[s]) & U16 if g else 0
                if g:
                    out[c] = g
        return out

    def _format12(self, at: int) -> dict[int, int]:
        d = self.data
        groups = struct.unpack(">I", d[at + 12 : at + 16])[0]
        out = {}
        for i in range(groups):
            a, b, g = struct.unpack(">III", d[at + 16 + 12 * i : at + 28 + 12 * i])
            for c in range(a, b + 1):
                out[c] = g + c - a
        return out

    # ---- metrics
    def glyph(self, char: str) -> int:
        """The glyph index of `char`, 0 (the box) when the font has none."""
        return self.cmap.get(ord(char), 0)

    def advance(self, gid: int) -> int:
        return self.advances[min(gid, self.metrics - 1)]

    def width(self, text: str, size: float) -> float:
        return sum(self.advance(self.glyph(c)) for c in text) * size / self.units

    # ---- outlines
    def _span(self, gid: int) -> tuple[int, int]:
        d, loca = self.data, self.tables["loca"]
        if self.long_loca:
            a, b = struct.unpack(">II", d[loca + 4 * gid : loca + 4 * gid + 8])
        else:
            a, b = (v * 2 for v in struct.unpack(">HH", d[loca + 2 * gid : loca + 2 * gid + 4]))
        return a, b

    def outline(self, gid: int) -> list[Contour]:
        """The contours of glyph `gid` in font units, components resolved."""
        a, b = self._span(gid)
        if a == b:
            return []
        d, at = self.data, self.tables["glyf"] + a
        n = struct.unpack(">h", d[at : at + 2])[0]
        return self._simple(at + 10, n) if n >= 0 else self._composite(at + 10)

    def _simple(self, at: int, n: int) -> list[Contour]:
        d = self.data
        ends = struct.unpack(f">{n}H", d[at : at + 2 * n])
        at += 2 * n
        at += 2 + struct.unpack(">H", d[at : at + 2])[0]
        total = ends[-1] + 1
        flags: list[int] = []
        while len(flags) < total:
            f = d[at]
            at += 1
            flags.append(f)
            if f & 8:
                flags += [f] * d[at]
                at += 1
        coords = []
        for short, same, in ((2, 16), (4, 32)):
            v, out = 0, []
            for f in flags:
                if f & short:
                    delta = d[at]
                    at += 1
                    v += delta if f & same else -delta
                elif not f & same:
                    v += struct.unpack(">h", d[at : at + 2])[0]
                    at += 2
                out.append(v)
            coords.append(out)
        xs, ys = coords
        contours, start = [], 0
        for e in ends:
            contours.append([(xs[i], ys[i], bool(flags[i] & 1)) for i in range(start, e + 1)])
            start = e + 1
        return contours

    def _composite(self, at: int) -> list[Contour]:
        d = self.data
        out: list[Contour] = []
        while True:
            flags, gid = struct.unpack(">HH", d[at : at + 4])
            at += 4
            if flags & 1:
                dx, dy = struct.unpack(">hh", d[at : at + 4])
                at += 4
            else:
                dx, dy = struct.unpack(">bb", d[at : at + 2])
                at += 2
            if not flags & 2:
                dx = dy = 0  # point matching is not used by the fonts we ship
            a = dd = 1.0
            b = c = 0.0
            if flags & 8:
                a = dd = struct.unpack(">h", d[at : at + 2])[0] / 16384
                at += 2
            elif flags & 0x40:
                a, dd = (v / 16384 for v in struct.unpack(">hh", d[at : at + 4]))
                at += 4
            elif flags & 0x80:
                a, b, c, dd = (v / 16384 for v in struct.unpack(">hhhh", d[at : at + 8]))
                at += 8
            for contour in self.outline(gid):
                out.append([(a * x + c * y + dx, b * x + dd * y + dy, on) for x, y, on in contour])
            if not flags & 0x20:
                return out


@lru_cache(maxsize=1)
def font() -> Font:
    return Font(FONT.read_bytes())


def flatten(contour: Contour, steps: int = 6) -> list[tuple[float, float]]:
    """A closed polyline for `contour`: quadratic segments cut into `steps` straight ones."""
    pts = list(contour)
    if not pts:
        return []
    if not pts[0][2]:
        if pts[-1][2]:
            pts = [pts[-1], *pts[:-1]]
        else:
            pts = [((pts[0][0] + pts[-1][0]) / 2, (pts[0][1] + pts[-1][1]) / 2, True), *pts]
    out = [(pts[0][0], pts[0][1])]
    cur, ctrl = pts[0], None

    def quad(p0, c, p1):
        for k in range(1, steps + 1):
            t = k / steps
            u = 1 - t
            out.append((u * u * p0[0] + 2 * u * t * c[0] + t * t * p1[0], u * u * p0[1] + 2 * u * t * c[1] + t * t * p1[1]))

    for p in [*pts[1:], pts[0]]:
        if p[2]:
            if ctrl is None:
                out.append((p[0], p[1]))
            else:
                quad(cur, ctrl, p)
            cur, ctrl = p, None
        elif ctrl is None:
            ctrl = p
        else:
            mid = ((ctrl[0] + p[0]) / 2, (ctrl[1] + p[1]) / 2, True)
            quad(cur, ctrl, mid)
            cur, ctrl = mid, p
    return out
