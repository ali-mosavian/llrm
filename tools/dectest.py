#!/usr/bin/env python3
"""
Check declen against ndisasm over real BC output.

Not a self test: ndisasm is an independent decoder, so where the two agree on
where an instruction ends, that is two implementations agreeing rather than
one asserting. Boundaries are taken from ndisasm and each one is put to
declen, which isolates length accuracy from resynchronisation -- a single
wrong length would otherwise desynchronise the walk and every later
instruction would be counted wrong for one mistake.
"""

import sys
import struct
from pathlib import Path
import subprocess
import collections

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from qbopt.declen import length  # noqa: E402


def segments(exe, mapfile, cls="BC_CODE"):
    hdr = struct.unpack_from("<H", Path(exe).read_bytes(), 8)[0] * 16
    out = []
    for ln in Path(mapfile).read_text().splitlines():
        p = ln.split()
        if len(p) >= 5 and p[4] == cls:
            out.append((p[3], hdr + int(p[0][:-1], 16), int(p[2][:-1], 16)))
    return out


def ndis(blob):
    r = subprocess.run(["ndisasm", "-b16", "-"], input=blob, capture_output=True)
    for ln in r.stdout.decode("latin1").splitlines():
        if len(ln) > 10 and ln[8] == " ":
            off = int(ln[:8], 16)
            txt = ln[10:].strip()
            yield off, txt


exe, mp = sys.argv[1], sys.argv[2]
data = Path(exe).read_bytes()
agree = mismatch = bail = notcode = 0
badops = collections.Counter()
wrong = collections.Counter()
for name, base, size in segments(exe, mp):
    blob = data[base : base + size]
    marks = list(ndis(blob))
    for k, (off, txt) in enumerate(marks):
        want = (marks[k + 1][0] - off) if k + 1 < len(marks) else None
        if want is None:
            break
        got = length(blob, off)
        if got is None:
            bail += 1
            badops[blob[off]] += 1
        elif " db 0x" in txt:
            # ndisasm refused these bytes, so there is no instruction here to
            # agree about. They are the FP emulator's int 34h-3Dh patch points
            # and module header data -- both of which a rewriting pass has to
            # leave alone anyway, the emulator's because it overwrites them
            # itself at first execution and would then be patching code we had
            # moved.
            notcode += 1
        elif got == want:
            agree += 1
        else:
            mismatch += 1
            wrong[(blob[off], txt.split()[0])] += 1
    print(f"  {name:16} {size:6} bytes, {len(marks):5} instructions")

tot = agree + mismatch + bail
print(f"\n  of {tot} real instructions: agree {agree} ({100 * agree / tot:.2f}%), mismatch {mismatch}, bail {bail}")
print(f"  {notcode} further offsets ndisasm would not decode either (not code)")
if wrong:
    print("  worst length disagreements:")
    for (op, mn), n in wrong.most_common(8):
        print(f"     {op:02X} {mn:10} x{n}")
print()
if badops:
    print("  most common unknown opcodes:")
    for op, n in badops.most_common(8):
        print(f"     {op:02X} x{n}")
