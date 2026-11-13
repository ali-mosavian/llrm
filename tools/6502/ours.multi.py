#!/usr/bin/env python3
"""ours (llrm-cc 6502-sim65) over the same programs and instrument; gcc 6502 column over tests/run/c programs: run.py CC1 LEVEL OUT.tsv [names]. Same corpus as ~/scratch/z80-gcc-progs/run.py. Program output = the longs it report()s, logged to RAM and
dumped as stdout bytes at exit (one run, correctness); clocks = sim65 -c of a second run that skips the dump; bytes = CODE+RODATA+DATA of the program object and the helper-library
members the link pulls (map file), not the driver or cc65's startup."""
import sys, re, os, json, tempfile, shutil, subprocess
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from flagset import CORPUS_COMMON, program_flags
GC = os.environ.get("GCC_COLUMNS", os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "gcc-columns"))
OVERLAY = HERE / "overlay"  # what must differ from the repo corpus (the empty program the floor is read from)
CORPUS = Path(os.environ.get("OURS_CORPUS", HERE.parents[1] / "tests" / "run" / "c"))
def src_of(name):
    return OVERLAY / f"{name}.c" if (OVERLAY / f"{name}.c").exists() else CORPUS / f"{name}.c"
ZP = 26
LOG = 0xF000
EXTRA = os.environ.get("G6_EXTRA", "").split()
TYPES = [f"-D__{n}_TYPE__={t}" for n, t in (("INT8", "signed char"), ("UINT8", "unsigned char"), ("INT16", "short"), ("UINT16", "unsigned short"), ("INT32", "long"), ("UINT32", "unsigned long"), ("INT64", "long long"), ("UINT64", "unsigned long long"))]
INSNS = os.environ.get("SIM65_INSNS", os.path.expanduser("~/scratch/cc65-v2.19-src/sim65-insns"))
PADS = [int(x) for x in os.environ.get("OURS_PADS", os.environ.get("OURS_PAD", "0")).split(",")]
PAD = PADS[0]
# the runtime (mos_rt.s) of the tree under test: there is no default, a stale one hid 8 link failures and could skew an A/B
if not os.environ.get("OURS_RT"):
    sys.exit("ours: OURS_RT is not set: name the mos_rt.s of the tree under test (one per side of an A/B)")
if not os.path.isfile(os.environ["OURS_RT"]):
    sys.exit(f"ours: OURS_RT={os.environ['OURS_RT']} is not a file")
RT = open(os.environ["OURS_RT"]).read()
def reachable_runtime(text):
    """Port of tests/common/pipeline.rs reachable_runtime: routines are the top-level labels of mos_rt.s; one is reached when another's text names it."""
    blocks = []; bss = ""; in_bss = False; pending = ""; macros = ""; in_macro = False
    for line in RT.splitlines():
        if line.startswith(".macro"): in_macro = True
        if in_macro:
            macros += line + "\n"; in_macro = not line.startswith(".endmacro"); continue
        if line.startswith('.segment "BSS"'): in_bss = True
        if in_bss:
            bss += line + "\n"
            if line.startswith('.segment "CODE"'): in_bss = False
            continue
        label = line.split(":")[0] if ":" in line and re.fullmatch(r"[A-Za-z_]\w*", line.split(":")[0]) else None
        if label: blocks.append([label, pending]); pending = ""
        if blocks and (label or not line.startswith(".export")):
            blocks[-1][1] += line + "\n"
        if line.startswith(".export"): pending += line + "\n"
    by = {n: i for i, (n, _) in enumerate(blocks)}
    toks = lambda t: [by[x] for x in re.split(r"[^A-Za-z0-9_]", t) if x in by]
    want = [False] * len(blocks); q = list(toks(text))
    while q:
        i = q.pop()
        if not want[i]:
            want[i] = True; q += [j for j in toks(blocks[i][1]) if j != i]
    out = "".join(b for (n, b), w in zip(blocks, want) if w)
    if out and ("__sa" in out or "__ra" in out): out += bss
    return macros + out if out else out
def driver():
    s = ".export _main\n.import main\n.import _putchar\n.export report\nlp = $A0\nptr = $A2\n" + "".join(f"rc{i} = ${0x80 + i:X}\n" for i in range(6))
    s += """.segment "CODE"
_main:
  lda #0
  sta rc0
  sta lp
  lda #$F0
  sta rc1
  lda #$%02X
  sta lp+1
  jsr main
.ifdef DUMP
  lda #0
  sta ptr
  lda #$%02X
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
report:
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
""" % (LOG >> 8, LOG >> 8)
    return s
def sh(cmd, cwd, timeout=300, text=True):
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=text, timeout=timeout)
def expected(p):
    t = p.read_text().split()
    return [int(x) for x in t] if all(re.fullmatch(r"-?\d+", x) for x in t) else None
def bytes_of(mapfile):
    tot = 0; cur = None
    for l in open(mapfile).read().split("Segment list:")[0].splitlines():
        if l.endswith(":") and not l.startswith(" "):
            cur = l[:-1]
        elif cur and re.match(r"\s+(CODE|RODATA|DATA)\s", l) and cur == "f.o":
            tot += int(re.search(r"Size=([0-9A-F]+)", l).group(1), 16)
    return tot
def base_members(work):
    w = Path(tempfile.mkdtemp(prefix="base-", dir=work)); (w / "d.s").write_text(driver()); (w / "f.s").write_text(".export main\nmain: rts\nrc0=$80\nrc1=$81\nrc2=$82\nrc3=$83\nrc4=$84\nrc5=$85\n")
    k = sh(["cl65", "-t", "sim6502", "--asm-define", "DUMP", "-m", "x.map", "-o", "x.bin", "d.s", "f.s"], w); assert k.returncode == 0, k.stderr
    return set(re.findall(r"sim6502\.lib\((\w+)\.o\)", (w / "x.map").read_text()))
def split_output(stdout):
    """sim65 prints "<n> cycles" straight after the program's output; the driver ends its dump with a newline so a last byte that is an ASCII digit is not read as part of n."""
    m = re.search(rb"\n(\d+) cycles\n$", stdout)
    return (stdout[:m.start()], int(m.group(1))) if m else (None, None)
def one(job):
    path, level, lib = job
    name = path.stem; w = Path(tempfile.mkdtemp(prefix=f"o6-{name}-", dir=WORK))
    want = expected(path.with_suffix(".out"))
    (w / "d.s").write_text(driver())
    flag = {"O0": "-O0", "O1": "-O1", "O2": "-O2", "O3": "-O3", "Os": "-Os"}[level]
    try:
        c = sh([OURS, "--target=6502-sim65", flag, "-S", "-w", *CORPUS_COMMON, *program_flags(path), "-I", str(HERE / "inc"), str(path), "-o", "o.s"], w, timeout=300)
    except subprocess.TimeoutExpired:
        return name, "SLOW", "llrm-cc over 300 s"
    if c.returncode:
        why = [l for l in c.stderr.splitlines() if "rror" in l or "panick" in l]
        return name, "REFUSED", (why or [c.stderr[-100:]])[0][:140]
    asm = (w / "o.s").read_text()
    (w / "f.s").write_text(".autoimport +\n" + (f'.segment "CODE"\n.res {PAD}\n' if PAD else "") + asm + reachable_runtime(asm))
    out = []
    for dump in (True, False):
        k = sh(["cl65", "-t", "sim6502", *(["--asm-define", "DUMP"] if dump else []), "-m", "x.map", "-o", "x.bin", "d.s", "f.s"], w)
        if k.returncode:
            und = sorted(set(re.findall(r"Unresolved external '(\w+)'", k.stderr)))
            return name, "LINK", (" ".join(und) or k.stderr[-100:])[:140]
        alien = set(re.findall(r"sim6502\.lib\((\w+)\.o\)", (w / "x.map").read_text())) - BASE
        if alien:
            return name, "ALIEN", "links the cc65 library: " + ",".join(sorted(alien))
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
    # sim65 adds a cycle per page-crossing branch or indexed access: one layout
    # is not the program's clocks. Mean over the other paddings of code.
    for pad in PADS[1:]:
        (w / "f.s").write_text(".autoimport +\n" + f'.segment "CODE"\n.res {pad}\n' + asm + reachable_runtime(asm))
        k = sh(["cl65", "-t", "sim6502", "-m", "x.map", "-o", "x.bin", "d.s", "f.s"], w)
        if k.returncode:
            return name, "LINK", k.stderr[-100:]
        r = sh(["sim65", "-c", "-x", "4000000000", "x.bin"], w, timeout=120, text=False)
        cycles.append(int(re.search(rb"(\d+) cycles", r.stdout).group(1)))
    # executed instructions and bytes of the first layout (the padding is not the program's bytes)
    (w / "f.s").write_text(".autoimport +\n" + (f'.segment "CODE"\n.res {PADS[0]}\n' if PADS[0] else "") + asm + reachable_runtime(asm))
    sh(["cl65", "-t", "sim6502", "-m", "x.map", "-o", "x.bin", "d.s", "f.s"], w)
    nb = bytes_of(w / "x.map")
    ins = sh([INSNS, "-c", "-x", "4000000000", "x.bin"], w, timeout=300, text=False)
    m = re.search(rb"(\d+) instructions", ins.stdout + ins.stderr)
    shutil.rmtree(w, True)
    return name, "PASS", {"bytes": nb, "clks": round(sum(cycles) / len(cycles)), "insns": int(m.group(1)) if m else None, "layouts": cycles}
if __name__ == "__main__":
    OURS, level, out = sys.argv[1:4]; names = sys.argv[4:]
    WORK = tempfile.mkdtemp(prefix="o6progs-"); lib = None
    import atexit, signal; signal.signal(signal.SIGTERM, lambda *_: sys.exit(143)); atexit.register(shutil.rmtree, WORK, True)  # one work directory per run, gone at exit (516 left in /tmp ran it out of inodes, 2026-11-10)
    BASE = base_members(WORK)
    pass82 = [x.strip() for x in open(HERE / "pass82.txt")]
    srcs = [src_of(n) for n in (names or pass82)]
    with ThreadPoolExecutor(4) as pool: res = list(pool.map(one, [(s, level, lib) for s in srcs]))
    json.dump(res, open(out + ".json", "w"), indent=1)
    open(out, "w").write("".join(f"{n}\tours\t{d['bytes']}\t{d['clks']}\n" for n, v, d in res if v == "PASS"))
    import collections; print(collections.Counter(v for _, v, _ in res))
