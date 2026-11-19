#!/usr/bin/env python3
"""gcc 6502 column over tests/run/c programs: gcc_6502_progs.py CC1 LEVEL OUT.tsv [names]. Each program is compiled with its `// flags:` (flagset.py), refused if the sides differ. Same corpus as ~/scratch/z80-gcc-progs/run.py. Program output = the longs it report()s, logged to RAM and
dumped as stdout bytes at exit (one run, correctness); clocks = sim65 -c of a second run that skips the dump; bytes = CODE+RODATA+DATA of the program object and the helper-library
members the link pulls (map file), not the driver or cc65's startup."""
import sys, re, os, json, tempfile, shutil, subprocess
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / "tools" / "6502")); sys.path.insert(0, str(ROOT / "tools"))
import flagset, external_flags
GC = str(HERE)
SRC = ROOT / "tests" / "run" / "c"
ZP = 26
LOG = 0xF000
INSNS = os.environ.get("SIM65_INSNS", os.path.expanduser("~/scratch/cc65-v2.19-src/sim65-insns"))
PADS = [int(x) for x in os.environ.get("OURS_PADS", "0").split(",")]
EXTRA = os.environ.get("G6_EXTRA", "").split()
TYPES = [f"-D__{n}_TYPE__={t}" for n, t in (("INT8", "signed char"), ("UINT8", "unsigned char"), ("INT16", "short"), ("UINT16", "unsigned short"), ("INT32", "long"), ("UINT32", "unsigned long"), ("INT64", "long long"), ("UINT64", "unsigned long long"))]
def driver():
    s = ".export _main\n.import _gmain\n.import _putchar\n.export ___main\n.export _report\n" + "".join(f".exportzp rc{i}\n" for i in range(ZP))
    s += '.segment "ZEROPAGE"\n' + "".join(f"rc{i}: .res 1\n" for i in range(ZP)) + "lp: .res 2\nptr: .res 2\n"
    s += f'''.segment "CODE"
_main:
  lda #0
  sta rc0
  sta lp
  lda #$F0
  sta rc1
  lda #${LOG >> 8:02X}
  sta lp+1
  jsr _gmain
.ifdef DUMP
  lda #0
  sta ptr
  lda #${LOG >> 8:02X}
  sta ptr+1
d1:
  lda ptr
  cmp lp
  bne d2
  lda ptr+1
  cmp lp+1
  beq d3
d2:
  ldy #0
  lda (ptr),y
  ldx #0
  jsr _putchar
  inc ptr
  bne d1
  inc ptr+1
  jmp d1
d3:
  lda #10
  ldx #0
  jsr _putchar
.endif
  lda #0
  jmp $FFF9
___main: rts
_report:
  ldy #0
  lda rc2
  sta (lp),y
  iny
  lda rc3
  sta (lp),y
  iny
  lda rc4
  sta (lp),y
  iny
  lda rc5
  sta (lp),y
  lda lp
  clc
  adc #4
  sta lp
  bcc r1
  inc lp+1
r1:
  rts
'''
    return s
def sh(cmd, cwd, timeout=300, text=True):
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=text, timeout=timeout)
def expected(p):
    t = p.read_text().split()
    return [int(x) for x in t] if all(re.fullmatch(r"-?\d+", x) for x in t) else None
def rt_library(work):
    lib = Path(work); objs = []
    for p in sorted(Path(GC, "rt-6502").glob("*.s")):
        a = sh(["ca65", "-t", "sim6502", "-o", f"{p.stem}.o", str(p)], work); assert a.returncode == 0, a.stderr
        objs.append(f"{p.stem}.o")
    # the runtime is not a compared cell: without the flag its memset loop would compile to a call of itself
    for c in sorted((HERE / "rtc-6502").glob("*.c")):
        r = sh([CC1, "-quiet", "-Os", "-fno-tree-loop-distribute-patterns", "-w", "-I", str(c.parent), str(c), "-o", f"{c.stem}.s"], work)
        if r.returncode: print("rtc fail", c.name, r.stderr[-150:]); continue
        a = sh(["ca65", "-t", "sim6502", "-o", f"{c.stem}.o", f"{c.stem}.s"], work); assert a.returncode == 0, (c, a.stderr)
        objs.append(f"{c.stem}.o")
    sh(["rm", "-f", "rt.lib"], work); r = sh(["ar65", "a", "rt.lib"] + objs, work); assert r.returncode == 0, r.stderr
def bytes_of(mapfile):
    tot = 0; cur = None
    for l in open(mapfile).read().split("Segment list:")[0].splitlines():
        if l.endswith(":") and not l.startswith(" "):
            cur = l[:-1]
        elif cur and re.match(r"\s+(CODE|RODATA|DATA)\s", l) and (cur == "p.o" or cur.startswith("rt.lib(")):
            tot += int(re.search(r"Size=([0-9A-F]+)", l).group(1), 16)
    return tot
def split_output(stdout):
    """sim65 prints "<n> cycles" straight after the program's output; the driver ends its dump with a newline so a last byte that is an ASCII digit is not read as part of n."""
    m = re.search(rb"\n(\d+) cycles\n$", stdout)
    return (stdout[:m.start()], int(m.group(1))) if m else (None, None)
def one(job):
    path, level, lib = job
    name = path.stem; w = Path(tempfile.mkdtemp(prefix=f"g6-{name}-", dir=WORK))
    want = expected(path.with_suffix(".out"))
    shutil.copy(lib / "rt.lib", w / "rt.lib"); (w / "d.s").write_text(driver())
    flags = {"O2": ["-O2"], "O1": ["-O1"], "Os": ["-Os"]}[level]
    try:
        if os.environ.get("G6_OLD"): gflags = [*flags, "-fsigned-char", "-fpack-struct=1", "-fno-tree-loop-distribute-patterns"]
        else:
            try: gflags = [*flags, *flagset.CORPUS_COMMON, *external_flags.gcc_program_flags(flagset.program_flags(path))]
            except ValueError as e: return name, "NA", str(e)
            why = flagset.mismatch([*flags, *flagset.CORPUS_COMMON, *flagset.program_flags(path)], gflags)
            if why: return name, "REFUSED", why
        c = sh([CC1, "-quiet", *gflags, "-w", "-std=gnu17", *EXTRA, *TYPES, "-I", str(ROOT / "tools" / "6502" / "inc"), str(path), "-o", "p.s"], w)
    except subprocess.TimeoutExpired:
        return name, "SLOW", "cc1 over 300 s"
    if c.returncode:
        why = [l for l in c.stderr.splitlines() if "error" in l or "internal compiler" in l]
        return name, "ICE" if "internal compiler" in c.stderr or "unable to find" in c.stderr else "CC1", (why or [c.stderr[-100:]])[0][:140]
    s = re.sub(r"\b_main\b", "_gmain", (w / "p.s").read_text()); (w / "p.s").write_text(s)
    out = []
    for dump in (True, False):
        k = sh(["cl65", "-t", "sim6502", *(["--asm-define", "DUMP"] if dump else []), "-m", "x.map", "-o", "x.bin", "d.s", "p.s", "rt.lib"], w)
        if k.returncode:
            und = sorted(set(re.findall(r"Unresolved external '(\w+)'", k.stderr)))
            return name, "LINK", (" ".join(und) or k.stderr[-100:])[:140]
        try:
            r = sh(["sim65", "-c", "-x", "4000000000", "x.bin"], w, timeout=120, text=False)
        except subprocess.TimeoutExpired:
            return name, "HALT", "no halt in 120 s"
        out.append(r)
    r = out[0]; body, _ = split_output(r.stdout)
    if r.returncode or body is None:
        return name, "HALT", f"exit {r.returncode}"
    got = [int.from_bytes(body[i:i + 4], "little", signed=True) for i in range(0, len(body), 4)]
    if got != want:
        return name, "WRONG", f"got {got[:3]} want {want[:3]}" + (f" first diff at {next(i for i in range(min(len(got), len(want))) if got[i] != want[i])}: {got[next(i for i in range(min(len(got), len(want))) if got[i] != want[i])]} want {want[next(i for i in range(min(len(got), len(want))) if got[i] != want[i])]}" if any(a != b for a, b in zip(got, want)) else f" length {len(got)} v {len(want)}")
    cycles = [int(re.search(rb"(\d+) cycles", out[1].stdout).group(1))]
    # one layout is not the clocks: sim65 charges page crossings. Mean over paddings.
    ptext = (w / "p.s").read_text()
    for pad in PADS[1:]:
        (w / "p.s").write_text(f'.segment "CODE"\n.res {pad}\n' + ptext)
        k = sh(["cl65", "-t", "sim6502", "-m", "x.map", "-o", "x.bin", "d.s", "p.s", "rt.lib"], w)
        if k.returncode:
            return name, "LINK", k.stderr[-100:]
        r = sh(["sim65", "-c", "-x", "4000000000", "x.bin"], w, timeout=120, text=False)
        cycles.append(int(re.search(rb"(\d+) cycles", r.stdout).group(1)))
    (w / "p.s").write_text(ptext)
    sh(["cl65", "-t", "sim6502", "-m", "x.map", "-o", "x.bin", "d.s", "p.s", "rt.lib"], w)
    nb = bytes_of(w / "x.map")
    ins = sh([INSNS, "-c", "-x", "4000000000", "x.bin"], w, timeout=300, text=False)
    m = re.search(rb"(\d+) instructions", ins.stdout + ins.stderr)
    shutil.rmtree(w, True)
    return name, "PASS", {"bytes": nb, "clks": round(sum(cycles) / len(cycles)), "insns": int(m.group(1)) if m else None, "layouts": cycles}
if __name__ == "__main__":
    cc1, level, out = sys.argv[1:4]; names = sys.argv[4:]; CC1 = cc1
    WORK = tempfile.mkdtemp(prefix="g6progs-"); lib = Path(WORK) / "lib"; lib.mkdir(); rt_library(lib)
    pass82 = [x.strip() for x in open(ROOT / "tools" / "6502" / "pass82.txt")]
    srcs = [SRC / f"{n}.c" for n in (names or pass82)]
    with ThreadPoolExecutor(4) as pool: res = list(pool.map(one, [(s, level, lib) for s in srcs]))
    json.dump(res, open(out + ".json", "w"), indent=1)
    open(out, "w").write("".join(f"{n}\tgcc\t{d['bytes']}\t{d['clks']}\n" for n, v, d in res if v == "PASS"))
    import collections; print(collections.Counter(v for _, v, _ in res))
