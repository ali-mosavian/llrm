#!/usr/bin/env python3
"""strlen of runtime/z80_rt.s against C: the empty string, one byte, a short string, 299 bytes (the count crosses 8 bits).

    strlen.py RUNTIME.s      prints ALL OK
"""
import os, re, subprocess, sys, tempfile
RT = os.path.abspath(sys.argv[1])
W = tempfile.mkdtemp(prefix="strlen-")
def sh(cmd, inp=None): return subprocess.run(cmd, cwd=W, input=inp, capture_output=True, text=True, timeout=120)
BASE = ["-mz80", "--no-std-crt0", "--code-loc", "0x100", "--data-loc", "0x8000"]
open(f"{W}/crt0.s", "w").write("\t.module crt0\n\t.globl _main\n\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tcall _main\n\thalt\n\t.area _CODE\n\t.area _DATA\n\t.area _INITIALIZED\n\t.area _GSINIT\n\t.area _GSFINAL\n\t.area _HOME\n")
open(f"{W}/d.c", "w").write("""extern unsigned _strlen(const char*) __sdcccall(1);
__at(0x8f00) volatile unsigned r[4];
char big[300];
void main(void) { unsigned i; for (i = 0; i < 299; i++) big[i] = 'x'; big[299] = 0;
  r[0] = _strlen(""); r[1] = _strlen("a"); r[2] = _strlen("hello, world"); r[3] = _strlen(big); }
""")
open(f"{W}/rt.s", "w").write(open(RT).read())
for cmd in (["sdasz80", "-o", "crt0.rel", "crt0.s"], ["sdasz80", "-l", "-o", "rt.rel", "rt.s"], ["sdcc", *BASE, "-c", "d.c", "-o", "d.rel"],
            ["sdcc", *BASE, "crt0.rel", "d.rel", "rt.rel", "-o", "d.ihx"]):
    p = sh(cmd); assert p.returncode == 0, p.stdout + p.stderr
out = sh(["ucsim_z80", "-t", "Z80", "-q", "-w", "d.ihx", "-c", "-"], "run\ndump rom 0x8f00 0x8f07\nquit\n").stdout
m = re.search(r"^0x8f00\s+(.*)$", out, re.M)
assert m, out[-400:]
b = [int(t, 16) for t in m.group(1).split()[:8]]
got = [b[i] | b[i + 1] << 8 for i in range(0, 8, 2)]
assert got == [0, 1, 12, 299], got
print("ALL OK")
