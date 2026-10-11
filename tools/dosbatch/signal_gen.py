"""The input of bench/stft_*: 64Ki 16-bit little-endian samples, two sines and a chirp with a little noise, from no input."""

from __future__ import annotations

import math
import struct

SAMPLES = 1 << 16


def signal() -> bytes:
    noise = 12345
    out = []
    for n in range(SAMPLES):
        noise = (noise * 1103515245 + 12345) & 0x7FFFFFFF
        chirp = 0.02 * n + (0.35 - 0.02) / (2 * SAMPLES) * n * n
        value = (8000 * math.sin(2 * math.pi * 0.0537 * n) + 5000 * math.sin(2 * math.pi * 0.2109 * n)
                 + 6000 * math.sin(2 * math.pi * chirp) + (noise >> 16) % 101 - 50)
        out.append(math.floor(value + 0.5))
    return struct.pack(f"<{SAMPLES}h", *out)


if __name__ == "__main__":
    import sys
    sys.stdout.buffer.write(signal())
