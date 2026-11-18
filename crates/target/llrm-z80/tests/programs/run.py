#!/usr/bin/env python3
"""tests/run/c programs compiled by llrm-z80 and run on ucsim_z80 against their .out files.

    run.py [--llc BIN] [--level 2|z|s] [--jobs N] [--src DIR] [--json OUT] [NAME ...]

Each `report(long)` call stores its four bytes in a log the harness reads back and compares with the .out lines.
Verdicts: PASS, WRONG (output differs), REFUSED (backend), SLOW (llc timed out), CLANG, ASM, LINK (undefined symbols named), HALT.
"""
import signal
import argparse, json, os, re, shutil, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
RUNTIME = HERE.parents[1] / "runtime" / "z80_rt.s"


def fnv1a64(data):
    """`llrm_z80::runtime_stamp`: FNV-1a, 64 bits, 16 hex digits."""
    hash = 0xCBF29CE484222325
    for byte in data:
        hash = ((hash ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return f"{hash:016x}"


def runtime_mismatch(llc, runtime):
    """Why `llc` and the runtime file a program is linked with are not the same: the compiler inlines and prices the runtime
    it was built with, and a sweep links the file it is given (by default the working tree's), so a runtime A/B that swaps
    the compiler and not the file measures two builds on one runtime (z80 2026-11-12: a divide fix showed 0 diff). None
    when they agree."""
    try:
        done = subprocess.run([llc, "--runtime-stamp"], capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired) as why:
        return f"cannot ask {llc} for its runtime stamp: {why}"
    stamp = done.stdout.strip()
    if done.returncode or not stamp:
        return f"{llc} does not report a runtime stamp (built before --runtime-stamp): rebuild it, or pass --allow-runtime-mismatch"
    have = fnv1a64(Path(runtime).read_bytes())
    if stamp != have:
        return (f"{llc} was built with a z80_rt.s of stamp {stamp}, but {runtime} has {have}: link the runtime the compiler "
                "was built with (--runtime FILE), or pass --allow-runtime-mismatch")
    return None
LIBC = HERE / "libc"  # the C library's entries over the runtime's: a library, so a program's own definition wins
DATALAYOUT = 'target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"'
sys.path.insert(0, str(ROOT / "tools"))
import external_flags  # noqa: E402  the one neutral flag set of every external bar (tools/external_flags.py)

CLANG_FLAGS = ["-target", "msp430", "-fsigned-char", "-fpack-struct=1", "-fno-vectorize", "-fno-slp-vectorize", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-O0-optnone", *external_flags.flags("none"), "-S", "-emit-llvm"]
LOG = 0xC000  # the log pointer, then the entries; the stack runs down from 0xFFFF
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
\t.area _BSS
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


def zero_areas(link_map, areas=("_BSS",)):
    """ucsim commands that zero the areas the program expects cleared at start (RAM starts as noise); a loader's job, not the program's clocks."""
    commands = ""
    for area in areas:
        start = re.search(rf"([0-9A-F]{{8}})\s+s_{area}\b", link_map)
        length = re.search(rf"([0-9A-F]{{8}})\s+l_{area}\b", link_map)
        if start and length and int(length.group(1), 16):
            commands += f"fill rom {int(start.group(1), 16):#x} {int(start.group(1), 16) + int(length.group(1), 16) - 1:#x} 0\n"
    return commands


def sh(cmd, cwd, inp=None, timeout=300):
    return subprocess.run(cmd, cwd=cwd, input=inp, capture_output=True, text=True, timeout=timeout)


# The status of a process stopped by `ulimit -t`: SIGXCPU.
CPU_LIMIT = -signal.SIGXCPU


def llc_limited(llc_command, cwd, cpu_seconds):
    """`llc_command` under 16 GB of address space and `cpu_seconds` of CPU time. The limit is CPU time, not the wall clock: the
    load guard stops a job's process group while the machine is busy, and an llc that takes 3 s alone read as `SLOW llc over 120 s`
    (fixed_point -O3, 2026-11-10). The wall timeout, 20 times the CPU one, is only for a process that hangs: it raises
    `subprocess.TimeoutExpired`. A CPU overrun is a return code of `CPU_LIMIT`."""
    return sh(["bash", "-c", f'ulimit -v 16777216; ulimit -S -t {int(cpu_seconds)}; exec "$@"', "llc", *llc_command], cwd, timeout=cpu_seconds * 20)


def libc_library(work):
    """The library directory of LIBC's objects and those of the `Z80_LIBC_EXTRA` directories (the soft-float helpers): sdld takes a member only for a symbol still undefined (an sdar archive, it takes whole)."""
    lib = Path(work) / "lib"
    lib.mkdir()
    stems = []
    extra = [Path(d) for d in os.environ.get("Z80_LIBC_EXTRA", "").split(":") if d]
    for s in sorted([*LIBC.glob("*.s"), *(p for d in extra for p in d.glob("*.s"))]):
        a = sh(["sdasz80", "-g", "-o", f"lib/{s.stem}.rel", str(s)], work)
        assert a.returncode == 0, a.stdout + a.stderr
        stems.append(s.stem)
    (lib / "libc.lib").write_text("\n".join(stems) + "\n")
    return str(lib)


def link_detail(undefined, extra):
    """The LINK line: the undefined names, and the unset `Z80_LIBC_EXTRA` (the soft-float library, runtime/compiler-rt/tests/z80_lib.sh) when it may be the cause."""
    names = " ".join(undefined)
    if undefined and not extra:
        return f"undefined {names[:90]} and Z80_LIBC_EXTRA unset"
    return names[:140]


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


def program_flags(src):
    """The `-f` flags of the program's first `// flags:` alternative, and whether it expects x86's types: any alternative
    of any `// flags:` line names an x86 mode (`-m32`, `-mabi=sysv`, `-march=i*`), a line `// x86-widths: WHY` says so (the
    other tools read only `// flags:`; a `-m64` there would move a program from the default m16 to m64), or it uses `long double`."""
    text = src.read_text()
    lines = [line.removeprefix("// flags:") for line in text.splitlines() if line.startswith("// flags:")]
    tokens = [t for line in lines for alternative in line.split("|") for t in alternative.split()]
    keep = [t for t in (lines[0].split("|")[0].split() if lines else []) if t.startswith("-f") and not t.startswith("-fsanitize")]
    x86 = any(t in ("-m32", "-mabi=sysv") or t.startswith(("-march=i", "-march=pentium")) for t in tokens) or "long double" in text or "\n// x86-widths:" in text
    return keep, x86


def one(job):
    src, args, work = job
    name = src.stem
    w = Path(tempfile.mkdtemp(prefix=f"z80prog-{name}-", dir=work))
    want = expected(src.with_suffix(".out"))
    if want is None:
        return name, "SKIP", "output is not integers"
    ll = w / "p.ll"
    c = sh([args.clang, f"-O{args.level}" if args.level != "z" else "-Oz", *CLANG_FLAGS, *program_flags(src)[0], str(src), "-o", str(ll)], ROOT)
    if c.returncode:
        return name, "CLANG", (c.stderr.strip().splitlines() or [""])[-1][:100]
    text = [l for l in ll.read_text().splitlines() if not l.startswith("target datalayout")]
    ll.write_text(DATALAYOUT + "\n" + "\n".join(text) + "\n")
    try:
        # a compile that runs away is stopped at 16 GB of address space and at the CPU time limit, not the machine's memory
        p = llc_limited([args.llc, "-S", f"-O{args.llc_level or args.level}", "-o", str(w / "p.s"), str(ll)], w, args.compile_timeout)
    except subprocess.TimeoutExpired:
        return name, "HUNG", f"llc made no progress for {args.compile_timeout * 20} s"
    if p.returncode == CPU_LIMIT:
        return name, "SLOW", f"llc over {args.compile_timeout} s of cpu"
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
    k = sh(["sdldz80", "-i", "-m", "-b", "_CODE=0x100", "x.ihx", "crt0.rel", "p.rel", "report.rel", "rt.rel", "-k", args.libc, "-l", "libc"], w)
    if k.returncode or "?ASlink-W-Undefined" in k.stdout + k.stderr:
        undefined = sorted(set(re.findall(r"Undefined Global (\S+)", k.stdout + k.stderr)))
        return name, "LINK", link_detail(undefined, os.environ.get("Z80_LIBC_EXTRA", "")) or (k.stdout + k.stderr)[-100:]
    try:
        r = sh(["ucsim_z80", "-t", "Z80", "-q", "-w", "x.ihx", "-c", "-"], w,
               f"sp_limit=0\n{zero_areas((w / 'x.map').read_text())}run\nstate\ndump rom {LOG:#x} {LOG + 0xFFF:#x}\nquit\n", timeout=args.timeout)
    except subprocess.TimeoutExpired:
        return name, "HALT", f"no halt in {args.timeout} s"
    if "Halted" not in r.stdout:
        return name, "HALT", " ".join(l for l in r.stdout.splitlines() if "overflow" in l or "Erroneous" in l or "Stop" in l)[:120]
    got = reported(r.stdout)
    if got == want:
        return name, "PASS", f"{len(got)} values"
    if program_flags(src)[1]:
        # int is 16 bits and long double 64 here; the .out is x86's, so a difference is not a miscompile
        return name, "X86", "expects x86's int and long double widths"
    first = next((i for i, (a, b) in enumerate(zip(got, want)) if a != b), min(len(got), len(want)))
    return name, "WRONG", f"value {first}: got {got[first] if first < len(got) else 'none'}, want {want[first] if first < len(want) else 'none'}"


def selected(src, names=()):
    """The programs of `src` that are for the z80: with an expected output, named if names are given, and not tagged for other targets by `// targets:` (stackover, an x86 recursion test, ran to the timeout)."""
    import corpus_targets
    found = sorted(p for p in Path(src).glob("*.c") if p.with_suffix(".out").exists() and corpus_targets.applies("z80-sdcc", p.read_text()))
    return [p for p in found if p.stem in names] if names else found


def main():
    global RUNTIME
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
    ap.add_argument("--runtime", default=str(RUNTIME), help="the z80_rt.s to link (the compiler's own build of it)")
    ap.add_argument("--allow-runtime-mismatch", action="store_true")
    args = ap.parse_args()
    RUNTIME = Path(args.runtime)
    if not args.allow_runtime_mismatch and (why := runtime_mismatch(args.llc, RUNTIME)):
        sys.exit(f"run.py: {why}")
    sources = selected(Path(args.src), args.names)
    work = tempfile.mkdtemp(prefix="z80prog-")
    try:
        args.libc = libc_library(work)
        with ThreadPoolExecutor(args.jobs) as pool:
            results = list(pool.map(one, [(s, args, work) for s in sources]))
    finally:
        # /tmp is a tmpfs with a fixed inode count: a tree per run filled it
        shutil.rmtree(work, ignore_errors=True)
    tally = {}
    for name, verdict, why in results:
        tally[verdict] = tally.get(verdict, 0) + 1
        if verdict != "PASS":
            print(f"{verdict:8} {name}: {why}")
    print(" ".join(f"{k} {v}" for k, v in sorted(tally.items())), f"of {len(results)}")
    if args.json:
        Path(args.json).write_text(json.dumps(results, indent=1))
    sys.exit(1 if any(v in ("WRONG", "HALT") for _, v, _ in results) else 0)


if __name__ == "__main__":
    main()
