#!/usr/bin/env python3
"""T-states of __umodsi3 and __modsi3 on a 22-bit dividend and a 17-bit divisor (loop_interchange's
`(sum * 31 + t) % 100003`), and their results. usage: divlong_clocks.py z80_rt.s MAX_T"""
import re, subprocess, sys, tempfile

RT, MAX_T = sys.argv[1], int(sys.argv[2])
W = tempfile.mkdtemp(prefix="divlong-")


def sh(cmd, inp=None):
    return subprocess.run(cmd, cwd=W, input=inp, capture_output=True, text=True, timeout=120)


open(f"{W}/rt.s", "w").write(open(RT).read())
assert sh(["sdasz80", "-g", "-o", "rt.rel", "rt.s"]).returncode == 0


def run(routine, dividend, divisor):
    drv = ["\t.area _HEADER (ABS)", "\t.org 0", "\tld sp,#0"]
    if routine:
        drv += [f"\tld hl,#{divisor >> 16}", "\tpush hl", f"\tld de,#{dividend >> 16}", f"\tld hl,#{dividend & 0xFFFF}",
                f"\tld bc,#{divisor & 0xFFFF}", f"\tcall {routine}", "\tpop af",
                "\tld (0x8f00),de", "\tld (0x8f02),hl"]
    drv += ["\thalt", "\t.area _CODE"]
    open(f"{W}/d.s", "w").write("\n".join(drv) + "\n")
    assert sh(["sdasz80", "-g", "-o", "d.rel", "d.s"]).returncode == 0
    k = sh(["sdldz80", "-i", "-b", "_CODE=0x100", "x.ihx", "d.rel", "rt.rel"])
    assert k.returncode == 0, k.stdout + k.stderr
    out = sh(["ucsim_z80", "-t", "Z80", "-q", "-w", "x.ihx", "-c", "-"],
             "sp_limit=0\nrun\nstate\ndump rom 0x8f00 0x8f03\nquit\n").stdout
    assert "Halted" in out, out[-300:]
    clks = int(re.search(r"Total time since last reset=.*\((\d+) clks\)", out).group(1))
    m = re.search(r"0x8f00\s+([0-9a-f]{2})\s+([0-9a-f]{2})\s+([0-9a-f]{2})\s+([0-9a-f]{2})", out)
    return clks, sum(int(m.group(i + 1), 16) << 8 * i for i in range(4)) if routine else 0


ok = True
for routine, expect in (("__umodsi3", 3100000 % 100003), ("__udivsi3", 3100000 // 100003)):
    base, _ = run(None, 0, 0)
    clks, value = run(routine, 3100000, 100003)
    t = clks - base
    print(f"{routine} {t} T")
    ok &= value == expect and t <= MAX_T
print("ALL OK" if ok else "FAILED")
sys.exit(0 if ok else 1)
