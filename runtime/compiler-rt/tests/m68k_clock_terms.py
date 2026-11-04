#!/usr/bin/env python3
"""68000 clocks the cycles tables leave out: `cycles --exe` prices a shift by a register count as 0 places and
mulu.w/divu.w at their maximum (70/140). This replays EXE under qemu with registers logged at those instructions
(library code only: not h_*, main, _start) and prints the exact correction per call:
    tests/m68k_clock_terms.py EXE [calls=3000]     env GCC_M68K (default ~/scratch/gcc-m68k/root)
mulu.w: 38 + 2 * ones(source word). divu.w: J. Cwik's exact 68000 algorithm. Shift by Dn: 2 * (count mod 64)."""
import os, re, subprocess, sys, tempfile

exe = sys.argv[1]
calls = int(sys.argv[2]) if len(sys.argv) > 2 else 3000
root = os.environ.get("GCC_M68K", os.path.expanduser("~/scratch/gcc-m68k/root"))
env = dict(os.environ, LD_LIBRARY_PATH=f"{root}/usr/lib/x86_64-linux-gnu")
dis = subprocess.run([f"{root}/usr/bin/m68k-linux-gnu-objdump", "-d", "--no-show-raw-insn", exe],
                     capture_output=True, text=True, env=env, check=True).stdout

sites, lib = {}, False
for line in dis.splitlines():
    m = re.match(r"^([0-9a-f]+) <(.+)>:$", line)
    if m:
        name = m.group(2)
        lib = not (name.startswith("h_") or name in ("main", "_start"))
        continue
    m = re.match(r"^\s*([0-9a-f]+):\s+(\S+)\s*(.*)$", line)
    if not (m and lib):
        continue
    pc, op, args = int(m.group(1), 16), m.group(2), m.group(3)
    r = re.match(r"^%d(\d),%d(\d)$", args)
    if op in ("muluw", "divuw") and r:
        sites[pc] = (op, int(r.group(1)), int(r.group(2)))
    elif re.match(r"^(ls|as|ro|rox)[lr][bwl]$", op) and r:
        sites[pc] = ("shift", int(r.group(1)), int(r.group(2)))


def divu(dividend, divisor):
    if (dividend >> 16) >= divisor:
        return 10
    cyc, hd = 38, divisor << 16
    for _ in range(15):
        t = dividend
        dividend = (dividend << 1) & 0xFFFFFFFF
        if t & 0x80000000:
            dividend = (dividend - hd) & 0xFFFFFFFF
        else:
            cyc += 2
            if dividend >= hd:
                dividend -= hd
                cyc -= 1
    return cyc * 2


if not sites:
    print("no data-dependent sites: correction 0")
    sys.exit(0)
log = tempfile.mktemp()
filt = ",".join(f"0x{pc:x}+2" for pc in sorted(sites))
cpu = "m68020" if "68020" in exe else "m68000"
subprocess.run(["qemu-m68k", "-cpu", cpu, "-one-insn-per-tb", "-d", "cpu", "-dfilter", filt, "-D", log, exe], env=env)
total, count, d = 0, {}, {}
for line in open(log):
    for k, v in re.findall(r"D(\d) = ([0-9a-f]{8})", line):
        d[int(k)] = int(v, 16)
    m = re.match(r"PC = ([0-9a-f]+)", line)
    if m and int(m.group(1), 16) in sites:
        op, s, t = sites[int(m.group(1), 16)]
        if op == "muluw":
            c = 38 + 2 * bin(d[s] & 0xFFFF).count("1")
            assert 38 <= c <= 70, c
            total += c - 70
        elif op == "divuw":
            c = divu(d[t], d[s] & 0xFFFF)
            assert c == 10 or 76 <= c <= 136, c
            total += c - 140
        else:
            total += 2 * (d[s] % 64)
        count[op] = count.get(op, 0) + 1
os.unlink(log)
print(f"{os.path.basename(exe)}: {count} correction {total:+d} clocks, {total / calls:+.1f} per call")
