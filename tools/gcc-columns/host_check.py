#!/usr/bin/env python3
"""Host check of the hand-derived expects in both gcc benches: kernels.c built with the host cc, 16-bit int/unsigned, run on the kernel arguments.

    host_check.py [KERNELS.c]    exit 1 if any expect differs from the host value
"""
import os, re, subprocess, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import gcc_z80_bench as Z
import gcc_6502_bench as B

SRC = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "..", "..", "crates/target/llrm-6502/tests/bench/kernels.c")
LZ = open(os.path.join(HERE, "lz.txt")).read().strip()
DATA = f"""#include <stdint.h>
#include <stdio.h>
unsigned char buf[16] = {{3,1,4,1,5,9,2,6,5,3,5,8,9,7,9,3}};
char str[16] = "hello world";
unsigned char tab[8] = {{2,5,8,11,14,17,20,23}};
unsigned char lz[] = {{{LZ}}};
unsigned char lzout[640];
"""
MASK = {Z.U8: 0xFF, Z.U16: 0xFFFF, "int": 0xFFFF}


def host(name, ret, args):
    t = open(SRC).read()
    t = re.sub(r"\bunsigned(?!\s+(char|long|short|int))", "uint16_t", t)
    t = re.sub(r"\bint\b", "int16_t", t)
    t = t.replace("uint16_t int", "uint16_t")
    call = f"{name}({', '.join(a for _, a in args)})"
    prog = f"#define K_{name}\n#include <stdint.h>\n{DATA}\n{t}\nint main(void){{ printf(\"%u\\n\", (unsigned)(uint16_t){call}); return 0; }}\n"
    with tempfile.TemporaryDirectory() as d:
        open(f"{d}/h.c", "w").write(prog)
        c = subprocess.run(["cc", "-w", "-fwrapv", "-O0", f"{d}/h.c", "-o", f"{d}/h"], capture_output=True, text=True)
        if c.returncode:
            return "compile: " + c.stderr.strip().splitlines()[0][:100]
        r = subprocess.run([f"{d}/h"], capture_output=True, text=True, timeout=20)
        return int(r.stdout.strip()) & MASK[ret]


def main():
    bad = 0
    e6 = {n: ex for n, _, _, ex in B.K}
    for name, ret, args, want in Z.K:
        got = host(name, ret, args)
        w = want & MASK[ret]
        flag = "" if got == w else "  <-- DIFFERS"
        if name in e6 and (e6[name] & MASK[ret]) != w:
            flag += f"  <-- 6502 bench expects {e6[name]}"
        if flag:
            bad += 1
        print(f"{name:10s} z80-expect={w:6d} host={got}{flag}")
    print(f"{len(Z.K)} kernels, {bad} differ")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
