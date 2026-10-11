"""The jpeg benches' decoder in Python, step for step: prints the Adler-32 of the BMP, as the benches' .out holds it.

    python3 tools/jpeg_reference.py fixed|single FILE.JPG [OUT.BMP]

`fixed` is exact (32-bit integers, shifts floor). `single` runs in doubles, so it can differ from the float32 benches
where a value lands on a rounding tie.
"""

import sys
import math
import struct
from pathlib import Path

ZIGZAG = [0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28,
          35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63]
COS = [[(math.sqrt(0.5) if u == 0 else 1.0) / 2 * math.cos((2 * x + 1) * u * math.pi / 16) for u in range(8)] for x in range(8)]
COS12 = [[round(c * 4096) for c in row] for row in COS]


class Reader:
    def __init__(self, data: bytes) -> None:
        self.data, self.at, self.bits, self.count = data, 0, 0, 0

    def byte(self) -> int:
        self.at += 1
        return self.data[self.at - 1]

    def bit(self) -> int:
        if self.count == 0:
            self.bits = self.byte()
            if self.bits == 0xFF:
                if self.byte() != 0:
                    raise ValueError("marker in entropy data")
            self.count = 8
        self.count -= 1
        return (self.bits >> self.count) & 1

    def receive(self, n: int) -> int:
        v = 0
        for _ in range(n):
            v = v * 2 + self.bit()
        return v

    def restart(self) -> None:
        self.count = 0
        while True:
            if self.byte() == 0xFF and 0xD0 <= self.byte() <= 0xD7:
                return


def huffman(bits: list[int], values: list[int]):
    maxcode, valptr, mincode = [-1] * 18, [0] * 17, [0] * 17
    code = k = 0
    for length in range(1, 17):
        valptr[length], mincode[length] = k, code
        code += bits[length]
        k += bits[length]
        maxcode[length] = code - 1 if bits[length] else -1
        code *= 2
    return maxcode, valptr, mincode, values


def decode(reader: Reader, table) -> int:
    maxcode, valptr, mincode, values = table
    code, length = reader.bit(), 1
    while code > maxcode[length]:
        code, length = code * 2 + reader.bit(), length + 1
        if length > 16:
            raise ValueError("bad code")
    return values[valptr[length] + code - mincode[length]]


def extend(v: int, s: int) -> int:
    return v if v >= 1 << (s - 1) else v - (1 << s) + 1


def idct(coef: list[int], fixed: bool) -> list[int]:
    tmp, out = [0] * 64, [0] * 64
    for v in range(8):
        for x in range(8):
            if fixed:
                tmp[v * 8 + x] = (sum(COS12[x][u] * coef[v * 8 + u] for u in range(8)) + 512) >> 10
            else:
                tmp[v * 8 + x] = sum(COS[x][u] * coef[v * 8 + u] for u in range(8))
    for y in range(8):
        for x in range(8):
            if fixed:
                s = (sum(COS12[y][v] * tmp[v * 8 + x] for v in range(8)) + 8192) >> 14
            else:
                s = math.floor(sum(COS[y][v] * tmp[v * 8 + x] for v in range(8)) + 128.5)
            out[y * 8 + x] = min(255, max(0, s + 128 if fixed else s))
    return out


def clamp(v: int) -> int:
    return min(255, max(0, v))


def convert(y: int, cb: int, cr: int, fixed: bool) -> tuple[int, int, int]:
    cb, cr = cb - 128, cr - 128
    if fixed:
        return (clamp(y + ((91881 * cr + 32768) >> 16)), clamp(y + ((-22554 * cb - 46802 * cr + 32768) >> 16)),
                clamp(y + ((116130 * cb + 32768) >> 16)))
    return (clamp(math.floor(y + 1.402 * cr + 0.5)), clamp(math.floor(y - 0.344136 * cb - 0.714136 * cr + 0.5)),
            clamp(math.floor(y + 1.772 * cb + 0.5)))


def adler(data: bytes) -> int:
    a, b = 1, 0
    for byte in data:
        a = (a + byte) % 65521
        b = (b + a) % 65521
    value = b * 65536 + a
    return value - (1 << 32) if value >= 1 << 31 else value


def run(data: bytes, fixed: bool) -> bytes:
    quant, tables, comps, interval, at = {}, {}, [], 0, 2
    while True:
        marker, length = data[at + 1], struct.unpack(">H", data[at + 2:at + 4])[0]
        body = data[at + 4:at + 2 + length]
        at += 2 + length
        if marker == 0xDB:
            for p in range(0, len(body), 65):
                quant[body[p] & 15] = list(body[p + 1:p + 65])
        elif marker == 0xC0:
            height, width = struct.unpack(">HH", body[1:5])
            comps = [(body[6 + 3 * i], body[7 + 3 * i] >> 4, body[7 + 3 * i] & 15, body[8 + 3 * i]) for i in range(body[5])]
        elif marker == 0xC4:
            p = 0
            while p < len(body):
                counts = [0] + list(body[p + 1:p + 17])
                n = sum(counts)
                tables[body[p]] = huffman(counts, list(body[p + 17:p + 17 + n]))
                p += 17 + n
        elif marker == 0xDD:
            interval = struct.unpack(">H", body)[0]
        elif marker == 0xDA:
            sel = {body[1 + 2 * i]: body[2 + 2 * i] for i in range(body[0])}
            break
    reader = Reader(data[at:])
    hmax, vmax = comps[0][1], comps[0][2]
    mw, mh = 8 * hmax, 8 * vmax
    mcus_x, mcus_y = (width + mw - 1) // mw, (height + mh - 1) // mh
    stride = (width * 3 + 3) & ~3
    out = bytearray(b"BM" + struct.pack("<IHHI", 54 + stride * height, 0, 0, 54) +
                    struct.pack("<IiiHHIIiiII", 40, width, -height, 1, 24, 0, stride * height, 2835, 2835, 0, 0))
    pred, count = [0] * 3, 0
    for my in range(mcus_y):
        planes = [[[0] * (mcus_x * 8 * c[1]) for _ in range(8 * c[2])] for c in comps]
        for mx in range(mcus_x):
            if interval and count and count % interval == 0:
                reader.restart()
                pred = [0] * 3
            count += 1
            for ci, (cid, h, v, tq) in enumerate(comps):
                for by in range(v):
                    for bx in range(h):
                        coef, q = [0] * 64, quant[tq]
                        t = decode(reader, tables[sel[cid] >> 4])
                        pred[ci] += extend(reader.receive(t), t) if t else 0
                        coef[0] = pred[ci] * q[0]
                        k = 1
                        while k < 64:
                            rs = decode(reader, tables[0x10 | sel[cid] & 15])
                            r, s = rs >> 4, rs & 15
                            if s == 0:
                                if r != 15:
                                    break
                                k += 16
                                continue
                            k += r
                            coef[ZIGZAG[k]] = extend(reader.receive(s), s) * q[k]
                            k += 1
                        block = idct(coef, fixed)
                        for y in range(8):
                            for x in range(8):
                                planes[ci][by * 8 + y][(mx * h + bx) * 8 + x] = block[y * 8 + x]
        for y in range(mh):
            if my * mh + y >= height:
                break
            row = bytearray(stride)
            for x in range(width):
                r, g, b = convert(planes[0][y][x], planes[1][y * (comps[1][2]) // vmax][x * comps[1][1] // hmax],
                                  planes[2][y * (comps[2][2]) // vmax][x * comps[2][1] // hmax], fixed)
                row[3 * x:3 * x + 3] = bytes((b, g, r))
            out += row
    return bytes(out)


if __name__ == "__main__":
    bmp = run(Path(sys.argv[2]).read_bytes(), sys.argv[1] == "fixed")
    if len(sys.argv) > 3:
        Path(sys.argv[3]).write_bytes(bmp)
    print(adler(bmp))
