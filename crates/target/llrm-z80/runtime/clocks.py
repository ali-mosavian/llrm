#!/usr/bin/env python3
"""clocks.py [--check]: measure __mulsi3 in ucsim (the call instruction through the return) for a spread of multipliers, fit
T = BASE + PASS * max(1, bit_length(b)) + ADD * popcount(b), and with --check fail unless src/runtime_size.rs carries the same
constants. The routine shifts b right once per pass until b is spent (at least one pass: b = 0 takes one) and adds on each set bit (z80_rt.s), so the
multiplicand does not matter."""
import os, re, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
RT = os.path.join(HERE, "z80_rt.s")
RUST = os.path.join(HERE, "..", "src", "runtime_size.rs")
# The three points that fix BASE, PASS and ADD are the first three (bit lengths 1, 2, 2 with popcounts 1, 1, 2); the
# rest of POINTS spans bit lengths 3..32 and popcounts 1..32 and must agree with the fit. HELD_OUT were chosen after
# the fit and never used in it: zero, one, a large power of two, 2^31 - 1, three fixed-seed random 32-bit values.
POINTS = (1, 2, 3, 5, 7, 11, 100, 255, 256, 0xFFFF, 0x10001, 0xFFFFFFFF)
HELD_OUT = (0, 1, 1 << 31, 0x7FFFFFFF, 0x9E3779B9, 0x2545F491, 0xC2B2AE35)
# the multiplicand does not change the clocks: checked with these as well
MULTIPLICANDS = (0x12345678, 0, 1, 0xFFFFFFFF, 0xDEADBEEF)


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
    bits = lambda b: max(1, b.bit_length())
    pops = lambda b: bin(b).count("1")
    pass_ = (t2 - t1) // (bits(b2) - bits(b1))
    add = (t3 - t2) // (pops(b3) - pops(b2))
    base = t1 - pass_ * bits(b1) - add * pops(b1)
    bad = [(hex(b), t, base + pass_ * bits(b) + add * pops(b)) for b, t in rows if t != base + pass_ * bits(b) + add * pops(b)]
    return base, pass_, add, bad


SHIFTS = ("__ashlsi3", "__lshrsi3", "__ashrsi3")
COUNTS = (1, 2, 3, 5, 8, 13, 21, 31)
# counts chosen after the fit, not used in it
HELD_COUNTS = (0, 4, 7, 16, 24, 30)


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
        held = [(n, shift(work, routine, n), base + per * n) for n in HELD_COUNTS]
        wrong = [row for row in held if row[1] != row[2]]
        if wrong:
            sys.exit(f"clocks.py: {routine} held-out counts disagree (count, measured, model): {wrong}")
        out[routine] = (base, per)
    return out


# The long divide and remainder routines, call instruction through the return. The speed family does one step per
# quotient bit: T = BASE + PER * max(0, na - nb), na and nb the bit lengths of dividend and divisor, fitted on (1, 1) and
# (32, 1) with alternating-bit operands (half the quotient bits set). The size family (`__s_`) does 32 steps always:
# one constant, measured at (24, 8). Held-out points must agree within 13% (speed) and 8% (size): the step cost
# depends on the quotient bits, which a static price cannot see.
DIVIDES = ("__udivsi3", "__umodsi3", "__divsi3", "__modsi3", "__udivremsi4", "__divremsi4")
HELD_DIVIDES = ((16, 8), (24, 8), (32, 8), (20, 4), (32, 16), (8, 16), (16, 16), (32, 31))


def divide(work, routine, na, nb):
    a = (1 << (na - 1)) | (0xAAAAAAAA & ((1 << na) - 1))
    b = (1 << (nb - 1)) | (0x55555555 & ((1 << nb) - 1))
    remainder_at = "\tld bc, #0x8000\n\tpush bc\n" if "rem" in routine else ""
    setup = f"\tld hl, #{a & 0xFFFF}\n\tld de, #{a >> 16}\n{remainder_at}\tld bc, #{b >> 16}\n\tpush bc\n\tld bc, #{b & 0xFFFF}\n"
    return run_call(work, setup, routine)


def divide_fit(work):
    """(routine, BASE, PER): the speed family from two points, the size family a constant (PER 0)."""
    out = []
    for routine in DIVIDES + tuple(f"__s_{name[2:]}" for name in DIVIDES):
        if routine.startswith("__s_"):
            base, per = divide(work, routine, 24, 8), 0
            tolerance = 0.08
        else:
            low, high = divide(work, routine, 1, 1), divide(work, routine, 32, 1)
            base, per = low, round((high - low) / 31)
            tolerance = 0.13
        for na, nb in HELD_DIVIDES:
            measured, model = divide(work, routine, na, nb), base + per * max(0, na - nb)
            if abs(model - measured) > tolerance * measured:
                sys.exit(f"clocks.py: {routine} at {na} / {nb} bits: measured {measured}, model {model}")
        out.append((routine, base, per))
    return out


# The other runtime routines (see `runtime_size.rs`: LINEAR, MULTIPLIES, BYTE_DIVIDES, WORD_DIVIDES, TRAMPOLINES).
# Each is measured, the call instruction through the return, over a spread of operands and held-out ones.
COUNTS_SHORT = (1, 2, 4, 8)
HELD_SHORT = (0, 3, 15)
LENGTHS = (2, 3, 10, 40)
HELD_LENGTHS = (1, 7, 100, 300)
MULTIPLIERS_8 = (1, 2, 3, 0x55, 0xFF)
HELD_MULTIPLIERS_8 = (0x80, 0x7F, 0xA5)  # not 0: the routine takes no pass for a zero multiplier (69 and 99 T, the model counts one)
MULTIPLIERS_16 = (1, 2, 3, 0x55, 0xFFFF)
HELD_MULTIPLIERS_16 = (0x100, 0x5555, 0xA5A5, 0x8000)


def regs(**values):
    """`ld` lines for registers (pairs hl de bc by name) and a."""
    out = ""
    for name, value in values.items():
        out += f"\tld {name}, #{value}\n"
    return out


# routine: (setup(count), count register, fit counts) for the routines linear in a count; the fit is T = BASE + PER * n
# for n >= MINIMUM (below it the routine's early exit is cheaper than the line, and the held-out points at n = 0 are
# checked against an absolute slack instead).
LINEAR = {
    "__ashlqi3": (lambda n: regs(a="0x35", l=n), "l", COUNTS_SHORT, HELD_SHORT, 0),
    "__lshrqi3": (lambda n: regs(a="0x35", l=n), "l", COUNTS_SHORT, HELD_SHORT, 0),
    "__ashrqi3": (lambda n: regs(a="0x35", l=n), "l", COUNTS_SHORT, HELD_SHORT, 0),
    "__ashlhi3": (lambda n: regs(hl="0x1234", a=n), "a", COUNTS_SHORT, HELD_SHORT, 0),
    "__lshrhi3": (lambda n: regs(hl="0x1234", a=n), "a", COUNTS_SHORT, HELD_SHORT, 0),
    "__ashrhi3": (lambda n: regs(hl="0x1234", a=n), "a", COUNTS_SHORT, HELD_SHORT, 0),
    "__memcpy": (lambda n: regs(hl="0x8000", de="0x9000", bc=n), "bc", LENGTHS, HELD_LENGTHS, 1),
    "__memmove": (lambda n: regs(hl="0x8000", de="0x9000", bc=n), "bc", LENGTHS, HELD_LENGTHS, 1),
    "__memseta": (lambda n: regs(hl="0x8000", a=7, de=n), "de", LENGTHS, HELD_LENGTHS, 2),
}
MULTIPLIES = {
    "__mulqi3": (lambda b: regs(a="0x35", l=b), "l", MULTIPLIERS_8, HELD_MULTIPLIERS_8),
    "__mulhi3": (lambda b: regs(hl="0x1234", de=b), "de", MULTIPLIERS_16, HELD_MULTIPLIERS_16),
}


def linear_fit(work):
    out = []
    for routine, (setup, register, counts, held, minimum) in LINEAR.items():
        rows = [(n, run_call(work, setup(n), routine)) for n in counts]
        per = (rows[-1][1] - rows[0][1]) // (rows[-1][0] - rows[0][0])
        base = rows[0][1] - per * rows[0][0]
        for n in tuple(counts) + tuple(held):
            measured = rows[counts.index(n)][1] if n in counts else run_call(work, setup(n), routine)
            model = base + per * n
            slack = max(0.1 * measured, 40 if n < max(minimum, 1) or minimum else 0)
            if abs(model - measured) > slack:
                sys.exit(f"clocks.py: {routine} at {n}: measured {measured}, model {model}")
        out.append((routine, base, per, register))
    return out


def multiply_fit(work):
    out = []
    bits = lambda b: max(1, b.bit_length())
    pops = lambda b: bin(b).count("1")
    for routine, (setup, register, points, held) in MULTIPLIES.items():
        rows = [(b, run_call(work, setup(b), routine)) for b in points]
        (b1, t1), (b2, t2), (b3, t3) = rows[0], rows[1], rows[2]
        pass_ = (t2 - t1) // (bits(b2) - bits(b1))
        add = (t3 - t2) // (pops(b3) - pops(b2))
        base = t1 - pass_ * bits(b1) - add * pops(b1)
        for b in tuple(points) + tuple(held):
            measured = run_call(work, setup(b), routine)
            model = base + pass_ * bits(b) + add * pops(b)
            if model != measured:
                sys.exit(f"clocks.py: {routine} at {hex(b)}: measured {measured}, model {model}")
        out.append((routine, base, pass_, add, register))
    return out


# 8-bit divides: one constant (8 steps whatever the operands, within 5%). 16-bit divides: a divisor under 128 takes 16
# steps, a larger one 9: two constants, measured at (1000, 7) and (65000, 200), held-out within 12% (a signed routine costs ~10% more for a negative dividend).
BYTE_DIVIDES = ("__udivqi3", "__umodqi3", "__divqi3", "__modqi3")
WORD_DIVIDES = ("__udivhi3", "__umodhi3", "__divhi3", "__modhi3", "__divremhi4")
HELD_BYTE = ((1, 1), (255, 1), (200, 3), (10, 100))
HELD_SMALL = ((1, 1), (40000, 3), (65000, 100))
HELD_LARGE = ((65000, 300), (65000, 20000), (100, 300))


def divide_small_fit(work):
    out = []
    for routine in BYTE_DIVIDES:
        at = lambda a, b: run_call(work, regs(a=a, l=b), routine)
        constant = at(100, 7)
        for a, b in HELD_BYTE:
            if abs(at(a, b) - constant) > 0.05 * constant:
                sys.exit(f"clocks.py: {routine} at {a}/{b}: measured {at(a, b)}, model {constant}")
        out.append((routine, constant, constant))
    for routine in WORD_DIVIDES:
        at = lambda a, b: run_call(work, regs(hl=a, de=b), routine)
        small, large = at(1000, 7), at(65000, 200)
        for held, model in ((HELD_SMALL, small), (HELD_LARGE, large)):
            for a, b in held:
                if abs(at(a, b) - model) > 0.12 * model:
                    sys.exit(f"clocks.py: {routine} at {a}/{b}: measured {at(a, b)}, model {model}")
        out.append((routine, small, large))
    return out


def strlen_fit(work):
    """`__strlen`: T = BASE + 21 * length (cpir takes 21 T a byte)."""
    def at(n):
        open(f"{work}/s.s", "w").write(f"\t.area _CODE\nstart::\n\tld sp, #0xF000\n\tld hl, #text\nbefore::\n\tcall __strlen\nafter::\n\thalt\ntext:\n\t.ascii \"{'x' * n}\"\n\t.db 0\n")
        for name, source in (("s", f"{work}/s.s"), ("rt", RT)):
            subprocess.run(["sdasz80", "-g", "-o", f"{work}/{name}.rel", source], check=True, capture_output=True)
        subprocess.run(["sdldz80", "-i", "-m", "-b", "_CODE=0x0", f"{work}/s.ihx", f"{work}/s.rel", f"{work}/rt.rel"], check=True, capture_output=True)
        text = open(f"{work}/s.map").read()
        sym = lambda name: int(re.search(r"([0-9A-F]{8})\s+%s\b" % name, text).group(1), 16)

        def clocks(stop):
            done = subprocess.run(["ucsim_z80", "-t", "Z80", "-q", "-w", f"{work}/s.ihx", "-c", "-"], input=f"sp_limit=0\nbreak 0x{stop:x}\nrun\nstate\nquit\n", capture_output=True, text=True).stdout
            return int(re.search(r"Total time since last reset=.*\((\d+) clks\)", done).group(1))

        return clocks(sym("after")) - clocks(sym("before"))

    low, high = at(1), at(40)
    per = (high - low) // 39
    base = low - per
    for n in (0, 3, 17, 100):
        if abs(at(n) - (base + per * n)) > 0.1 * at(n):
            sys.exit(f"clocks.py: __strlen at {n}: measured {at(n)}, model {base + per * n}")
    return ("__strlen", base, per, "")


def trampoline(work):
    """`__call_iy`: the clocks it adds beyond the call and the return of its target (a bare `ret`)."""
    open(f"{work}/y.s", "w").write("\t.area _CODE\nstart::\n\tld sp, #0xF000\n\tld iy, #target\nbefore::\n\tcall __call_iy\nafter::\n\thalt\ntarget:\n\tret\n")
    for name, source in (("y", f"{work}/y.s"), ("rt", RT)):
        subprocess.run(["sdasz80", "-g", "-o", f"{work}/{name}.rel", source], check=True, capture_output=True)
    subprocess.run(["sdldz80", "-i", "-m", "-b", "_CODE=0x0", f"{work}/y.ihx", f"{work}/y.rel", f"{work}/rt.rel"], check=True, capture_output=True)
    text = open(f"{work}/y.map").read()
    at = lambda sym: int(re.search(r"([0-9A-F]{8})\s+%s\b" % sym, text).group(1), 16)

    def clocks(stop):
        done = subprocess.run(["ucsim_z80", "-t", "Z80", "-q", "-w", f"{work}/y.ihx", "-c", "-"], input=f"sp_limit=0\nbreak 0x{stop:x}\nrun\nstate\nquit\n", capture_output=True, text=True).stdout
        return int(re.search(r"Total time since last reset=.*\((\d+) clks\)", done).group(1))

    return clocks(at("after")) - clocks(at("before")) - 17 - 10  # the call, the target's ret


def main():
    with tempfile.TemporaryDirectory() as work:
        rows = [(b, measure(work, 0x12345678, b)) for b in POINTS]
        shifts = shift_fit(work)
        divides = divide_fit(work)
        linears, multiplies, small_divides, trampoline_clocks = linear_fit(work) + [strlen_fit(work)], multiply_fit(work), divide_small_fit(work), trampoline(work)
        base, pass_, add, bad = fit(rows)
        if bad:
            sys.exit(f"clocks.py: the model does not fit: {bad}")
        model = lambda b: base + pass_ * max(1, b.bit_length()) + add * bin(b).count("1")
        held = [(hex(a), hex(b), measure(work, a, b), model(b)) for b in HELD_OUT for a in MULTIPLICANDS[:2]]
        held += [(hex(a), hex(b), measure(work, a, b), model(b)) for a in MULTIPLICANDS[2:] for b in (3, 0xFFFF, 0x9E3779B9)]
        wrong = [row for row in held if row[2] != row[3]]
        if wrong:
            sys.exit(f"clocks.py: held-out points disagree (a, b, measured, model): {wrong}")
    print(f"MULSI3_BASE={base} MULSI3_PASS={pass_} MULSI3_ADD={add}")
    for routine, (b, p) in shifts.items():
        print(f"{routine}: BASE={b} PER={p}")
    for routine, divide_base, divide_per in divides:
        print(f"{routine}: BASE={divide_base} PER={divide_per}")
    for row in linears + multiplies + small_divides:
        print(*row)
    print("__call_iy", trampoline_clocks)
    if "--check" in sys.argv:
        rust = open(RUST).read()
        wanted = [("MULSI3_BASE", base), ("MULSI3_PASS", pass_), ("MULSI3_ADD", add)]
        for routine, (b, p) in shifts.items():
            tag = routine.strip("_").upper()
            wanted += [(f"{tag}_BASE", b), (f"{tag}_PER", p)]
        for routine, base_, per_, register in linears:
            if f'("{routine}", {base_}, {per_}, "{register}"' not in rust:
                sys.exit(f"clocks.py: runtime_size.rs does not carry LINEAR ({routine}, {base_}, {per_}, {register})")
        for routine, base_, pass__, add_, register in multiplies:
            if f'("{routine}", {base_}, {pass__}, {add_}, "{register}"' not in rust:
                sys.exit(f"clocks.py: runtime_size.rs does not carry MULTIPLIES ({routine}, {base_}, {pass__}, {add_}, {register})")
        for routine, small, large in small_divides:
            if f'("{routine}", {small}, {large})' not in rust:
                sys.exit(f"clocks.py: runtime_size.rs does not carry ({routine}, {small}, {large})")
        if f'("__call_iy", {trampoline_clocks})' not in rust:
            sys.exit(f"clocks.py: runtime_size.rs does not carry (__call_iy, {trampoline_clocks})")
        for routine, divide_base, divide_per in divides:
            if f'("{routine}", {divide_base}, {divide_per})' not in rust:
                sys.exit(f"clocks.py: runtime_size.rs does not carry ({routine}, {divide_base}, {divide_per})")
        for name, value in wanted:
            if not re.search(r"const %s: u32 = %d;" % (name, value), rust):
                sys.exit(f"clocks.py: runtime_size.rs does not carry {name} = {value}")


if __name__ == "__main__":
    main()
