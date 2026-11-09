#!/usr/bin/env python3
"""clocks.py [--check]: measure __mulsi3 in ucsim (the call instruction through the return) for a spread of multipliers, fit
T = BASE + PASS * bit_length(b) + ADD * popcount(b), and with --check fail unless src/runtime_size.rs carries the same
constants. The routine shifts b right once per pass until b is spent and adds on each set bit (z80_rt.s), so the
multiplicand does not matter."""
import os, re, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
RT = os.path.join(HERE, "z80_rt.s")
RUST = os.path.join(HERE, "..", "src", "runtime_size.rs")
POINTS = (1, 2, 3, 5, 7, 11, 100, 255, 256, 0xFFFF, 0x10001, 0xFFFFFFFF)


def measure(work, a, b):
    return run_call(
        work,
        f"\tld hl, #{a & 0xFFFF}\n\tld de, #{a >> 16}\n\tld bc, #{b >> 16}\n\tpush bc\n\tld bc, #{b & 0xFFFF}\n",
        "__mulsi3",
    )


def shift(work, routine, count):
    """A long in hl/de shifted by `count` (a byte in a)."""
    return run_call(work, f"\tld hl, #0x5678\n\tld de, #0x1234\n\tld a, #{count}\n", routine)


def run_call(work, setup, routine):
    open(f"{work}/t.s", "w").write(f"\t.area _CODE\nstart::\n\tld sp, #0xF000\n{setup}before::\n\tcall {routine}\nafter::\n\thalt\n")
    for name, source in (("t", f"{work}/t.s"), ("rt", RT)):
        subprocess.run(["sdasz80", "-g", "-o", f"{work}/{name}.rel", source], check=True, capture_output=True)
    subprocess.run(["sdldz80", "-i", "-m", "-b", "_CODE=0x0", f"{work}/t.ihx", f"{work}/t.rel", f"{work}/rt.rel"], check=True, capture_output=True)
    text = open(f"{work}/t.map").read()
    at = lambda sym: int(re.search(r"([0-9A-F]{8})\s+%s\b" % sym, text).group(1), 16)

    def clocks(stop):
        done = subprocess.run(["ucsim_z80", "-t", "Z80", "-q", "-w", f"{work}/t.ihx", "-c", "-"], input=f"sp_limit=0\nbreak 0x{stop:x}\nrun\nstate\nquit\n", capture_output=True, text=True).stdout
        return int(re.search(r"Total time since last reset=.*\((\d+) clks\)", done).group(1))

    return clocks(at("after")) - clocks(at("before"))


def fit(rows):
    """BASE, PASS, ADD from three points, then every point must agree."""
    (b1, t1), (b2, t2), (b3, t3) = rows[0], rows[1], rows[2]
    bits = lambda b: b.bit_length()
    pops = lambda b: bin(b).count("1")
    pass_ = (t2 - t1) // (bits(b2) - bits(b1))
    add = (t3 - t2) // (pops(b3) - pops(b2))
    base = t1 - pass_ * bits(b1) - add * pops(b1)
    bad = [(hex(b), t, base + pass_ * bits(b) + add * pops(b)) for b, t in rows if t != base + pass_ * bits(b) + add * pops(b)]
    return base, pass_, add, bad


SHIFTS = ("__ashlsi3", "__lshrsi3", "__ashrsi3")
COUNTS = (1, 2, 3, 5, 8, 13, 21, 31)


def shift_fit(work):
    """SHIFT_<routine>_BASE and _PER: T = BASE + PER * count, the call instruction through the return."""
    out = {}
    for routine in SHIFTS:
        rows = [(n, shift(work, routine, n)) for n in COUNTS]
        per = (rows[1][1] - rows[0][1]) // (rows[1][0] - rows[0][0])
        base = rows[0][1] - per * rows[0][0]
        bad = [(n, t) for n, t in rows if t != base + per * n]
        if bad:
            sys.exit(f"clocks.py: {routine} is not linear in the count: {bad}")
        out[routine] = (base, per)
    return out


def main():
    with tempfile.TemporaryDirectory() as work:
        rows = [(b, measure(work, 0x12345678, b)) for b in POINTS]
        shifts = shift_fit(work)
    base, pass_, add, bad = fit(rows)
    if bad:
        sys.exit(f"clocks.py: the model does not fit: {bad}")
    print(f"MULSI3_BASE={base} MULSI3_PASS={pass_} MULSI3_ADD={add}")
    for routine, (b, p) in shifts.items():
        print(f"{routine}: BASE={b} PER={p}")
    if "--check" in sys.argv:
        rust = open(RUST).read()
        wanted = [("MULSI3_BASE", base), ("MULSI3_PASS", pass_), ("MULSI3_ADD", add)]
        for routine, (b, p) in shifts.items():
            tag = routine.strip("_").upper()
            wanted += [(f"{tag}_BASE", b), (f"{tag}_PER", p)]
        for name, value in wanted:
            if not re.search(r"const %s: u32 = %d;" % (name, value), rust):
                sys.exit(f"clocks.py: runtime_size.rs does not carry {name} = {value}")


if __name__ == "__main__":
    main()
