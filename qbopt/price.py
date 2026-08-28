#!/usr/bin/env python3
"""
Price what the pass does, in cycles rather than bytes.

Bytes are a proxy and not the thing: a shorter sequence with a dependency
chain through one register can be slower than a longer one, and the fixups
that put dx and bx back are three bytes each but sit at the end of a chain.
tools/cycles knows the difference, so ask it.

    uv run python -m qbopt.price PROG.EXE PROG.MAP
"""

import sys
import struct
from pathlib import Path

# cycles and timings are not vendored yet, so this module does not run. It is
# rewritten to take an .OBJ, and they are copied in, in the measurement phase.
from cycles import report  # ty: ignore[unresolved-import]
from timings import ARCHS  # ty: ignore[unresolved-import]

from qbopt.flags import ALL
from qbopt.lift import lift
from qbopt.lift import needed
from qbopt.lift import regions
from qbopt.lift import emit_region


def main() -> int:
    exe, mp = sys.argv[1], sys.argv[2]
    d = Path(exe).read_bytes()
    hdr = struct.unpack_from("<H", d, 8)[0] * 16
    n = 0
    for line in Path(mp).read_text().splitlines():
        f = line.split()
        if len(f) >= 5 and f[4] == "BC_CODE":
            n = max(n, int(f[1][:-1], 16) + 1)
    b = d[hdr : hdr + n]
    v, _ = lift(b, 0, n)
    need = needed(v)

    tb = ta = 0
    rows = []
    for reg in regions(v):
        at, end = v[reg[0]].at, v[reg[-1]].end
        # no CFG here, so the flags after a region are unknown and every one of
        # them has to be assumed live. The measurement phase gives this a module.
        emitted = emit_region(v, need, reg, ALL)
        if not emitted or len(emitted.code) > end - at:
            continue
        out = emitted.code
        core = len(out)
        while core > 0 and out[core - 1] == 0x90:
            core -= 1
        if core >= 2 and out[core - 2] == 0xEB:
            core -= 2
        _, bi, bc, _ = report("before", b[at:end].hex())
        _, ai, ac, _ = report("after", out[:core].hex())
        rows.append((at, end - at, bi, ai, bc, ac))

    if not rows:
        print("no regions taken")
        return 0
    ib = ia = 0
    for which, title in (
        (0, "standing alone -- the sequence waits on its own chain"),
        (1, "back to back -- the machine has other work to overlap"),
    ):
        tb = [0] * len(ARCHS)
        ta = [0] * len(ARCHS)
        ib = ia = 0
        for _at, _nb, bi, ai, bc, ac in rows:
            ib += bi
            ia += ai
            for k in range(len(ARCHS)):
                tb[k] += bc[which][k]
                ta[k] += ac[which][k]
        print()
        print(title)
        print(f"{'':16}" + "".join(f"{a:>8}" for a in ARCHS))
        print(f"{'BC emits':16}" + "".join(f"{x:>8g}" for x in tb))
        print(f"{'rewritten':16}" + "".join(f"{x:>8g}" for x in ta))
        print(f"{'speedup':16}" + "".join(f"{tb[k] / ta[k]:>7.2f}x" for k in range(len(ARCHS))))
    print()
    print(f"instructions {ib} -> {ia}, {ib / ia:.2f}x")
    print()
    print("BC's two 16-bit halves are independent chains, one through ax and one")
    print("through dx, and an out-of-order machine already runs them together.")
    print("Widening puts everything through one register, so it halves the")
    print("instruction count and leaves the dependency chain where it was. That")
    print("is most of the win on an in-order 486 or P5 and almost none of it on")
    print("anything later. DOSBox charges per instruction, so its 1.41x is the")
    print("in-order answer.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
