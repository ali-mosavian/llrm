"""PNG writer whose bytes depend only on the pixels: our own deflate, no timestamps or extra chunks.

zlib's output differs between zlib builds, so the compressed stream is made here: rows are
matched against the row above and against the previous byte, then coded with one dynamic
Huffman block. Standard readers (including zlib) decode it.
"""
from __future__ import annotations

import bisect
import heapq
import struct
import zlib

CHAIN = 12  # earlier places remembered per four-byte sequence
SIGNATURE = b"\x89PNG\r\n\x1a\n"
LEN_BASE = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258]
LEN_EXTRA = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0]
DIST_BASE = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073,
             4097, 6145, 8193, 12289, 16385, 24577]
DIST_EXTRA = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13]
CL_ORDER = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15]


LEN_CODE = [0] * 259
for _c in range(29):
    for _n in range(LEN_BASE[_c], 259):
        LEN_CODE[_n] = _c


def _len_code(n: int) -> int:
    return LEN_CODE[n]


def _dist_code(d: int) -> int:
    return bisect.bisect_right(DIST_BASE, d) - 1


def code_lengths(freq: list[int], limit: int) -> list[int]:
    """Huffman code lengths for `freq`, none longer than `limit`; ties broken by symbol, so deterministic."""
    freq = list(freq)
    while True:
        heap = [(f, s, None) for s, f in enumerate(freq) if f]
        if not heap:
            return [0] * len(freq)
        if len(heap) == 1:
            out = [0] * len(freq)
            out[heap[0][1]] = 1
            return out
        heapq.heapify(heap)
        nxt = len(freq)
        parent: dict[int, int] = {}
        while len(heap) > 1:
            a = heapq.heappop(heap)
            b = heapq.heappop(heap)
            parent[a[1]] = parent[b[1]] = nxt
            heapq.heappush(heap, (a[0] + b[0], nxt, None))
            nxt += 1
        out = [0] * len(freq)
        for s, f in enumerate(freq):
            if f:
                depth, n = 0, s
                while n in parent:
                    n = parent[n]
                    depth += 1
                out[s] = depth
        if max(out) <= limit:
            return out
        freq = [(f + 1) // 2 if f else 0 for f in freq]


def canonical(lengths: list[int]) -> list[int]:
    """Bit-reversed canonical codes (ready for an LSB-first bit writer)."""
    count = [0] * 16
    for n in lengths:
        count[n] += 1
    count[0] = 0
    code, nxt = 0, [0] * 17
    for bits in range(1, 16):
        code = (code + count[bits - 1]) << 1
        nxt[bits] = code
    out = []
    for n in lengths:
        if n:
            c = nxt[n]
            nxt[n] += 1
            out.append(int(format(c, f"0{n}b")[::-1], 2))
        else:
            out.append(0)
    return out


class Bits:
    def __init__(self) -> None:
        self.out = bytearray()
        self.acc = 0
        self.n = 0

    def put(self, value: int, bits: int) -> None:
        self.acc |= value << self.n
        self.n += bits
        while self.n >= 8:
            self.out.append(self.acc & 255)
            self.acc >>= 8
            self.n -= 8

    def finish(self) -> bytes:
        if self.n:
            self.out.append(self.acc & 255)
        return bytes(self.out)


def _match(a: bytes, i: int, b: bytes, j: int, cap: int) -> int:
    n = 0
    while n < cap:
        step = min(64, cap - n)
        if a[i + n : i + n + step] == b[j + n : j + n + step]:
            n += step
        else:
            while a[i + n] == b[j + n]:
                n += 1
            return n
    return n


def tokens(rows: list[bytes]) -> list[tuple[int, int]]:
    """(literal, 0) or (length, distance) per step.

    A row repeats the one above, or a byte repeats, or four bytes repeat something the last
    32 KB held (the most recent occurrence).
    """
    out: list[tuple[int, int]] = []
    stride = len(rows[0]) if rows else 0
    buf = b"".join(rows)
    seen: dict[bytes, list[int]] = {}
    base = 0
    for r, cur in enumerate(rows):
        prev = rows[r - 1] if r else None
        i, n = 0, len(cur)
        while i < n:
            lp = lr = lh = 0
            if prev is not None and cur[i] == prev[i]:
                lp = _match(cur, i, prev, i, min(258, n - i))
            if i and cur[i] == cur[i - 1]:
                lr = _match(cur, i, cur, i - 1, min(258, n - i))
            best, dist = (lp, stride) if lp >= lr else (lr, 1)
            if best < 4 and i + 4 <= n:
                key = cur[i : i + 4]
                chain = seen.setdefault(key, [])
                for at in reversed(chain):
                    if base + i - at > 32768:
                        break
                    lh = _match(buf, at, buf, base + i, min(258, n - i))
                    if lh > best:
                        best, dist = lh, base + i - at
                chain.append(base + i)
                del chain[:-CHAIN]
            if best >= 3:
                out.append((best, dist))
                for j in range(i + 1, min(i + best, n - 3) if best <= 24 else i + 1):
                    chain = seen.setdefault(cur[j : j + 4], [])
                    chain.append(base + j)
                    del chain[:-CHAIN]
                i += best
            else:
                out.append((cur[i], 0))
                i += 1
        base += n
    return out


def deflate(rows: list[bytes]) -> bytes:
    toks = tokens(rows)
    lit = [0] * 286
    dist = [0] * 30
    lit[256] = 1
    for a, d in toks:
        if d:
            lit[257 + _len_code(a)] += 1
            dist[_dist_code(d)] += 1
        else:
            lit[a] += 1
    ll = code_lengths(lit, 15)
    dl = code_lengths(dist, 15)
    if not any(dl):
        dl[0] = 1
    nlit = max(257, max(i for i, v in enumerate(ll) if v) + 1)
    ndist = max(1, max(i for i, v in enumerate(dl) if v) + 1)
    seq = ll[:nlit] + dl[:ndist]
    cl_syms: list[tuple[int, int, int]] = []  # symbol, extra bits value, extra bit count
    i = 0
    while i < len(seq):
        v, run = seq[i], 1
        while i + run < len(seq) and seq[i + run] == v:
            run += 1
        i += run
        if v == 0:
            while run >= 11:
                k = min(run, 138)
                cl_syms.append((18, k - 11, 7))
                run -= k
            if run >= 3:
                cl_syms.append((17, run - 3, 3))
                run = 0
        else:
            cl_syms.append((v, 0, 0))
            run -= 1
            while run >= 3:
                k = min(run, 6)
                cl_syms.append((16, k - 3, 2))
                run -= k
        cl_syms += [(v, 0, 0)] * run
    clf = [0] * 19
    for s, _e, _b in cl_syms:
        clf[s] += 1
    cll = code_lengths(clf, 7)
    clc = canonical(cll)
    nclen = max(4, max(i for i, s in enumerate(CL_ORDER) if cll[s]) + 1)
    lc, dc = canonical(ll), canonical(dl)
    w = Bits()
    w.put(1, 1)
    w.put(2, 2)
    w.put(nlit - 257, 5)
    w.put(ndist - 1, 5)
    w.put(nclen - 4, 4)
    for s in CL_ORDER[:nclen]:
        w.put(cll[s], 3)
    for s, extra, bits in cl_syms:
        w.put(clc[s], cll[s])
        if bits:
            w.put(extra, bits)
    for a, d in toks:
        if d:
            c = _len_code(a)
            w.put(lc[257 + c], ll[257 + c])
            if LEN_EXTRA[c]:
                w.put(a - LEN_BASE[c], LEN_EXTRA[c])
            c = _dist_code(d)
            w.put(dc[c], dl[c])
            if DIST_EXTRA[c]:
                w.put(d - DIST_BASE[c], DIST_EXTRA[c])
        else:
            w.put(lc[a], ll[a])
    w.put(lc[256], ll[256])
    return w.finish()


def chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def encode(width: int, height: int, rows: list[bytes], palette: list[tuple[int, int, int]] | None) -> bytes:
    """A PNG of `rows` (one byte per pixel into `palette`, or three bytes RGB when it is None)."""
    framed = [b"\0" + bytes(r) for r in rows]
    raw = deflate(framed)
    z = b"\x78\xda" + raw + struct.pack(">I", zlib.adler32(b"".join(framed)))
    head = struct.pack(">IIBBBBB", width, height, 8, 3 if palette else 2, 0, 0, 0)
    out = SIGNATURE + chunk(b"IHDR", head)
    if palette:
        out += chunk(b"PLTE", bytes(c for rgb in palette for c in rgb))
    return out + chunk(b"IDAT", z) + chunk(b"IEND", b"")
