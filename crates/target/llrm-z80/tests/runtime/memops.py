#!/usr/bin/env python3
"""the memcpy, memmove and memset entries of runtime/z80_rt.s against C's meaning, on ucsim_z80: n of 0, 1, a few and 300 (the count
crosses 8 bits), overlap both ways and the same pointer. The result register (dst in de) is checked too.

    memops.py RUNTIME.s      prints ALL OK
"""
import os, re, subprocess, sys, tempfile
RT = os.path.abspath(sys.argv[1])
W = tempfile.mkdtemp(prefix="memops-")
def sh(cmd, inp=None): return subprocess.run(cmd, cwd=W, input=inp, capture_output=True, text=True, timeout=120)
A, B, RES, N = 0x9000, 0xA000, 0x8F00, 0x500
# (routine, dst, src-or-value, n)
OPS = [("copy", B, A, 0), ("copy", B, A, 1), ("copy", B + 0x10, A + 3, 5), ("copy", B + 0x100, A, 300),
       ("move", A + 5, A, 20), ("move", A, A + 5, 20), ("move", A + 0x40, A + 0x40, 10), ("move", A + 0x80, A + 0x100, 0),
       ("seta", B + 0x440, 0x77, 0), ("seta", B + 0x450, 0x78, 1), ("seta", B + 0x460, 0x79, 300)]
mem = bytearray(0x10000)
for i in range(N):
    mem[A + i] = (i * 7 + 3) & 0xFF
    mem[B + i] = (~i) & 0xFF
init = bytes(mem)
want_ret = []
for op, dst, src, n in OPS:
    if op == "seta":
        for i in range(n): mem[dst + i] = src
    else:
        mem[dst:dst + n] = bytes(mem[src:src + n])
    want_ret.append(dst)
def db(base):
    return "\n".join("\t.db " + ",".join(str(b) for b in init[base + i:base + i + 16]) for i in range(0, N, 16))
code = "\t.module t\n\t.globl _main, __memcpy, __memmove, __memseta\n\t.area _CODE\n_main:\n"
for k, (op, dst, src, n) in enumerate(OPS):
    name = {"copy": "__memcpy", "move": "__memmove", "seta": "__memseta"}[op]
    if op == "seta":  # the compiler's entry: value in a, n in de
        code += f"\tld hl,#{dst}\n\tld a,#{src}\n\tld de,#{n}\n\tcall {name}\n\tld ({RES + 2 * k}),de\n"
    else:
        code += f"\tld hl,#{dst}\n\tld de,#{src}\n\tld bc,#{n}\n\tcall {name}\n\tld ({RES + 2 * k}),de\n"
code += "\tret\n"
files = {
    "crt0.s": "\t.module crt0\n\t.globl _main\n\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tcall _main\n\thalt\n\t.area _CODE\n",
    "t.s": code,
    "data.s": f"\t.module data\n\t.area _DABS (ABS)\n\t.org {A:#x}\n{db(A)}\n\t.org {B:#x}\n{db(B)}\n",
    "rt.s": open(RT).read()}
for name, text in files.items():
    open(f"{W}/{name}", "w").write(text)
for s in ("crt0", "t", "data", "rt"):
    p = sh(["sdasz80", "-o", f"{s}.rel", f"{s}.s"]); assert p.returncode == 0, p.stdout + p.stderr
p = sh(["sdldz80", "-i", "-b", "_CODE=0x100", "x.ihx", "crt0.rel", "t.rel", "data.rel", "rt.rel"]); assert p.returncode == 0, p.stdout + p.stderr
out = sh(["ucsim_z80", "-t", "Z80", "-q", "-w", "x.ihx", "-c", "-"],
         f"run\ndump rom {A:#x} {A + N - 1:#x}\ndump rom {B:#x} {B + N - 1:#x}\ndump rom {RES:#x} {RES + 2 * len(OPS) - 1:#x}\nquit\n").stdout
got = {}
for m in re.finditer(r"^0x([0-9a-f]{4})((?:\s+[0-9a-f]{2}){1,16})", out, re.M):
    for i, b in enumerate(m.group(2).split()): got[int(m.group(1), 16) + i] = int(b, 16)
for base in (A, B):
    for i in range(N):
        assert got[base + i] == mem[base + i], f"byte {base + i:#x}: got {got[base + i]:#x}, want {mem[base + i]:#x}"
rets = [got[RES + 2 * k] | got[RES + 2 * k + 1] << 8 for k in range(len(OPS))]
assert rets == want_ret, f"results {rets} want {want_ret}"
print("ALL OK")
