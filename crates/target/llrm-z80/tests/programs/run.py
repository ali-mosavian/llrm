#!/usr/bin/env python3
"""tests/run/c programs compiled by llrm-z80 and run on ucsim_z80 against their .out files.

    run.py [--llc BIN] [--level 2|z|s] [--jobs N] [--src DIR] [--json OUT] [NAME ...]

Each `report(long)` call stores its four bytes in a log the harness reads back and compares with the .out lines.
Verdicts: PASS, WRONG (output differs), REFUSED (backend), SLOW (llc timed out), CLANG, ASM, LINK (undefined symbols named), HALT.
"""
import argparse, json, os, re, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
RUNTIME = HERE.parents[1] / "runtime" / "z80_rt.s"
DATALAYOUT = 'target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"'
CLANG_FLAGS = ["-target", "msp430", "-fsigned-char", "-fpack-struct=1", "-fno-vectorize", "-fno-slp-vectorize", "-S", "-emit-llvm"]
LOG = 0xE000  # the log pointer, then the entries
CRT0 = f"""\t.module crt0
\t.globl _main
\t.area _HEADER (ABS)
\t.org 0
\tld sp,#0
\tld hl,#{LOG + 2:#x}
\tld ({LOG:#x}),hl
\tcall _main
\thalt
\t.area _CODE
\t.area _CONST
\t.area _DATA
"""
REPORT = f"""\t.module report
\t.globl _report
\t.area _CODE
_report::
\tld c,l
\tld b,h
\tld hl,({LOG:#x})
\tld (hl),c
\tinc hl
\tld (hl),b
\tinc hl
\tld (hl),e
\tinc hl
\tld (hl),d
\tinc hl
\tld ({LOG:#x}),hl
\tret
"""


def sh(cmd, cwd, inp=None, timeout=300):
    return subprocess.run(cmd, cwd=cwd, input=inp, capture_output=True, text=True, timeout=timeout)


def expected(path):
    lines = path.read_text().split()
    return [int(x) for x in lines] if all(re.fullmatch(r"-?\d+", x) for x in lines) else None


def reported(out):
    memory = {}
    for m in re.finditer(r"^0x([0-9a-f]{4})((?:\s+[0-9a-f]{2}){1,16})", out, re.M):
        for i, b in enumerate(m.group(2).split()):
            memory[int(m.group(1), 16) + i] = int(b, 16)
    end = memory[LOG] | memory[LOG + 1] << 8
    values = []
    for at in range(LOG + 2, end, 4):
        v = sum(memory[at + i] << 8 * i for i in range(4))
        values.append(v - (1 << 32) if v >= 1 << 31 else v)
    return values


def one(job):
    src, args, work = job
    name = src.stem
    w = Path(tempfile.mkdtemp(prefix=f"z80prog-{name}-", dir=work))
    want = expected(src.with_suffix(".out"))
    if want is None:
        return name, "SKIP", "output is not integers"
    ll = w / "p.ll"
    c = sh([args.clang, f"-O{args.level}" if args.level != "z" else "-Oz", *CLANG_FLAGS, str(src), "-o", str(ll)], ROOT)
    if c.returncode:
        return name, "CLANG", (c.stderr.strip().splitlines() or [""])[-1][:100]
    text = [l for l in ll.read_text().splitlines() if not l.startswith("target datalayout")]
    ll.write_text(DATALAYOUT + "\n" + "\n".join(text) + "\n")
    try:
        p = sh([args.llc, "-S", f"-O{args.llc_level or args.level}", "-o", str(w / "p.s"), str(ll)], w, timeout=args.compile_timeout)
    except subprocess.TimeoutExpired:
        return name, "SLOW", f"llc over {args.compile_timeout} s"
    if p.returncode:
        why = [l for l in p.stderr.splitlines() if "REFUSED" in l or "llrm-z80-llc" in l]
        return name, "REFUSED", (why or [p.stderr.strip()[-100:]])[0][:140]
    (w / "crt0.s").write_text(CRT0)
    (w / "report.s").write_text(REPORT)
    (w / "rt.s").write_text(RUNTIME.read_text())
    for stem in ("crt0", "report", "rt", "p"):
        a = sh(["sdasz80", "-g", "-o", f"{stem}.rel", f"{stem}.s"], w)
        if a.returncode:
            return name, "ASM", (a.stdout + a.stderr).strip().splitlines()[-1][:100]
    k = sh(["sdldz80", "-i", "-b", "_CODE=0x100", "x.ihx", "crt0.rel", "p.rel", "report.rel", "rt.rel"], w)
    if k.returncode or "?ASlink-W-Undefined" in k.stdout + k.stderr:
        undefined = sorted(set(re.findall(r"Undefined Global (\S+)", k.stdout + k.stderr)))
        return name, "LINK", " ".join(undefined)[:140] or (k.stdout + k.stderr)[-100:]
    try:
        r = sh(["ucsim_z80", "-t", "Z80", "-q", "-w", "x.ihx", "-c", "-"], w,
               f"run\nstate\ndump rom {LOG:#x} {LOG + 0xFFF:#x}\nquit\n", timeout=args.timeout)
    except subprocess.TimeoutExpired:
        return name, "HALT", f"no halt in {args.timeout} s"
    if "Halted" not in r.stdout:
        return name, "HALT", r.stdout[-100:]
    got = reported(r.stdout)
    if got == want:
        return name, "PASS", f"{len(got)} values"
    first = next((i for i, (a, b) in enumerate(zip(got, want)) if a != b), min(len(got), len(want)))
    return name, "WRONG", f"value {first}: got {got[first] if first < len(got) else 'none'}, want {want[first] if first < len(want) else 'none'}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("names", nargs="*")
    ap.add_argument("--llc", default=os.path.expanduser("~/scratch/z80-sweep/tgt/debug/llrm-z80-llc"))
    ap.add_argument("--clang", default=os.environ.get("LLRM_PIN_CLANG", os.path.expanduser("~/scratch/llvm-build/bin/clang")))
    ap.add_argument("--level", default="2")
    ap.add_argument("--llc-level", help="the backend's level when it differs from clang's")
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--timeout", type=int, default=60, help="seconds the program may run on ucsim")
    ap.add_argument("--compile-timeout", type=int, default=120)
    ap.add_argument("--json")
    ap.add_argument("--src", default=str(ROOT / "tests/run/c"))
    args = ap.parse_args()
    sources = sorted(p for p in Path(args.src).glob("*.c") if p.with_suffix(".out").exists())
    if args.names:
        sources = [p for p in sources if p.stem in args.names]
    work = tempfile.mkdtemp(prefix="z80prog-")
    with ThreadPoolExecutor(args.jobs) as pool:
        results = list(pool.map(one, [(s, args, work) for s in sources]))
    tally = {}
    for name, verdict, why in results:
        tally[verdict] = tally.get(verdict, 0) + 1
        if verdict != "PASS":
            print(f"{verdict:8} {name}: {why}")
    print(" ".join(f"{k} {v}" for k, v in sorted(tally.items())), f"of {len(results)}")
    if args.json:
        Path(args.json).write_text(json.dumps(results, indent=1))
    sys.exit(1 if any(v in ("WRONG", "HALT") for _, v, _ in results) else 0)


main()
