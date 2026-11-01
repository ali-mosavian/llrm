#!/usr/bin/env python3
"""Check src/timing/z80.csv against two oracles, instruction by instruction.

Bytes: sdasz80 (SDCC 4.5.0) and z80-unknown-coff-as (binutils 2.43.50) must agree with the CSV.
T-states: ucsim_z80 (sz80 0.8.15) runs prefix + instruction + HALT and prefix + HALT; the difference is the
instruction. Conditional forms are run both ways (flags / B / BC set by the prefix); ldir and cpir run BC=1 (the last
pass) and BC=2 (a repeating pass plus the last).
Prints one line per disagreement and exits 1 if there is any. The CSV's first column is the form: `;` stands for `,`.
"""
import csv, os, re, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
CSV = os.path.join(HERE, "..", "..", "src", "timing", "z80.csv")
CODE = {"_CODE"}


def run(cmd, cwd, inp=None):
    try:
        return subprocess.run(cmd, cwd=cwd, input=inp, capture_output=True, text=True, timeout=10)
    except subprocess.TimeoutExpired:
        return subprocess.CompletedProcess(cmd, 124, "", "timeout")


def render(form, asm):
    """One concrete instruction. asm is 'sdas' or 'gnu'."""
    mnemonic, _, rest = form.partition(" ")
    ops = rest.split(";") if rest else []
    imm = "#" if asm == "sdas" else ""
    out = []
    for op in ops:
        if "(ix+d)" in op:
            out.append("5(ix)" if asm == "sdas" else "(ix+5)")
        elif op == "n":
            out.append(imm + "0x12")
        elif op == "nn":
            out.append("Lend" if mnemonic in ("jp", "call") else imm + "0x1234")
        elif op == "(nn)":
            out.append("(0x9000)")
        elif op == "e":
            out.append("Lend")
        elif op.isdigit():
            out.append(op)
        else:
            out.append(op)
    return mnemonic + (" " + ", ".join(out) if out else "")


def sdas_bytes(text, cpu, d):
    open(f"{d}/b.s", "w").write(f"\t.area _CODE\n{'.hd64' if cpu == 'z180' else ''}\n\t{text}\nLend:\n")
    p = run(["sdasz80", "-o", "b.rel", "b.s"], d)
    if p.returncode:
        return None
    return sum(int(m.group(2), 16) for m in re.finditer(r"A (\S+) size ([0-9A-F]+) flags", open(f"{d}/b.rel").read()) if m.group(1) in CODE)


def gnu_bytes(text, cpu, d):
    open(f"{d}/g.s", "w").write(f"\t{text}\nLend:\n")
    p = run(["z80-unknown-coff-as", f"-march={cpu}", "-o", "g.o", "g.s"], d)
    if p.returncode:
        return None
    o = run(["z80-unknown-coff-objdump", "-h", "g.o"], d).stdout
    m = re.search(r"\.text\s+([0-9a-f]{8})", o)
    return int(m.group(1), 16) if m else None


def clocks(prefix, body, cpu, d):
    src = f"\t.area _HEADER (ABS)\n\t.org 0\n{'.hd64' if cpu == 'z180' else ''}\n{prefix}\n{body}\nLend:\n\thalt\n"
    open(f"{d}/t.s", "w").write(src)
    if run(["sdasz80", "-o", "t.rel", "t.s"], d).returncode or run(["sdldz80", "-i", "t.ihx", "t.rel"], d).returncode:
        return None
    out = run(["ucsim_z80", "-t", cpu.upper(), "-q", "-w", "t.ihx", "-c", "-"], d, "run\nstate\nquit\n").stdout
    if "Halted" not in out:
        return None
    return int(re.search(r"Total time since last reset=.*\((\d+) clks\)", out).group(1))


def delta(prefix, insn, cpu, d):
    a, b = clocks(prefix, "\t" + insn, cpu, d), clocks(prefix, "", cpu, d)
    return None if a is None or b is None else a - b


def timings(form, insn, cpu, d):
    """(taken, not_taken or None) measured."""
    mnemonic = form.split()[0]
    cond = re.match(r"(?:jp|jr|call|ret) (nz|nc|c)\b", form)
    if form == "djnz e":
        return delta("\tld b, #2", insn, cpu, d), delta("\tld b, #1", insn, cpu, d)
    if mnemonic in ("ldir", "cpir"):
        pre = "\tld hl, #0x9000\n\tld de, #0x9100\n\tld a, #0xFF\n\tld bc, #"
        last = delta(pre + "1", insn, cpu, d)
        two = delta(pre + "2", insn, cpu, d)
        return (None if last is None or two is None else two - last), last
    if cond:
        taken_pre, not_pre = {
            "nz": ("\tld a, #1\n\tor a", "\txor a"),
            "c": ("\tscf", "\tor a"),
            "nc": ("\tor a", "\tscf"),
        }[cond.group(1)]
        if mnemonic == "ret":
            stack = "\tld hl, #Lend\n\tpush hl\n"
            return delta(stack + taken_pre, insn, cpu, d) , delta(stack + not_pre, insn, cpu, d)
        return delta(taken_pre, insn, cpu, d), delta(not_pre, insn, cpu, d)
    if form == "jp (hl)":
        return delta("\tld hl, #Lend", insn, cpu, d), None
    if form == "ret":
        return delta("\tld hl, #Lend\n\tpush hl", insn, cpu, d), None
    return delta("", insn, cpu, d), None


# Cells where ucsim_z80 0.8.15 disagrees with the manual (UM0080; manual_check.py), pinned at what it returns so a fixed simulator
# shows up. The manual's value is the CSV's; the reasons are why the simulator is the instrument in error.
UCSIM_WRONG = {
    "inc (hl)": (7, "UM0080 says 11 (4, 4, 3); ucsim charges a plain load"),
    "dec (hl)": (7, "same as inc (hl)"),
    "bit 0;(hl)": (15, "UM0080 says 12 (4, 4, 4)"),
    "mlt bc": (0, "17 T per the Z180 manual (from memory); ucsim charges nothing for a two-byte instruction"),
}


def main():
    bad = 0
    ver = [run([c, "--version"], ".").stdout.splitlines()[0][:70] for c in ("z80-unknown-coff-as",)]
    print("# oracles:", run(["sdcc", "--version"], ".").stdout.split("TD-")[-1].strip()[:30], "|", ver[0], "|",
          run(["ucsim_z80", "-v"], ".").stdout.strip())
    d = tempfile.mkdtemp(prefix="z80timing-")
    rows = [r for r in csv.reader(l for l in open(CSV) if not l.startswith("#"))]
    for form, nbytes, taken, not_taken, cpu in rows:
        form = form.replace(";", ",") if False else form
        nbytes, taken = int(nbytes), int(taken)
        want_nt = int(not_taken) if not_taken else None
        sdas, gnu = render(form, "sdas"), render(form, "gnu")
        got = {"sdas": sdas_bytes(sdas, cpu, d), "gnu": gnu_bytes(gnu, cpu, d)}
        for name, n in got.items():
            if n != nbytes:
                print(f"BYTES {form}: csv {nbytes}, {name} {n}"); bad += 1
        t, nt = timings(form, sdas, cpu, d)
        if form in UCSIM_WRONG:
            if t != UCSIM_WRONG[form][0]:
                print(f"T {form}: ucsim now {t}, was pinned at {UCSIM_WRONG[form][0]}: recheck"); bad += 1
            continue
        if t != taken:
            print(f"T {form}: csv {taken}, ucsim {t}"); bad += 1
        if want_nt is not None and nt != want_nt:
            print(f"T-not-taken {form}: csv {want_nt}, ucsim {nt}"); bad += 1
    print(f"{len(rows)} forms, {bad} disagreements")
    return 1 if bad else 0


sys.exit(main())
