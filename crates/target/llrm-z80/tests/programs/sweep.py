#!/usr/bin/env python3
"""Bytes and T-states of the tests/run/c programs: ours against the best SDCC 4.5.0 variant.

    sweep.py LLC OUT.tsv [--level 2] [--src DIR] [--ours-only | --sdcc-only] [NAME ...]

OUT.tsv rows: `program, column, bytes, T-states` (column `ours` or an SDCC variant). Only a program whose output
equals its `.out` gets a row. `OUT.tsv.failed` lists programs without an `ours` row and why; `OUT.tsv.nobar` lists
programs no SDCC variant produced a row for and why. The last line is `N programs M failed; rows R of N` and the
exit status is 1 when a program of the corpus has no `ours` row (`sweep_guard`).

Bytes: the linked image minus the harness (crt0 and report), plus whole runtime members for ours. T-states: ucsim
clocks, minus the loader's copy of initialised data for SDCC (21 T a byte and 10).
"""
import argparse, hashlib, importlib.util, os, re, shutil, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


run = load("run", HERE / "run.py")
measure = load("measure", HERE.parent / "kernels" / "measure.py")
guard = load("sweep_guard", HERE / "sweep_guard.py")
same_width = load("same_width", HERE / "same_width.py")

SDCC = [
    ("sdcc-size", ["--fsigned-char", "--opt-code-size"]),
    ("sdcc-size-ma", ["--fsigned-char", "--opt-code-size", "--max-allocs-per-node", "200000"]),
    ("sdcc-speed", ["--fsigned-char", "--opt-code-speed"]),
    ("sdcc-speed-ma", ["--fsigned-char", "--opt-code-speed", "--max-allocs-per-node", "200000"]),
]
# SDCC's report passes the value in de, and its crt0 initialises the data the loader would copy
REPORT_SDCC = run.REPORT.replace("\tld c,l\n\tld b,h\n\tld hl,", "\tld c,e\n\tld b,d\n\tex de,hl\n\tld hl,")
CRT0_SDCC = (
    run.CRT0.replace("\tcall _main", "\tcall gsinit\n\tcall _main")
    .replace("\t.area _CODE\n", "\t.area _CODE\n\t.area _INITIALIZER\n\t.area _GSINIT\ngsinit::\n\tld bc,#l__INITIALIZER\n\tld a,b\n\tor a,c\n"
             "\tjr Z,gsdone\n\tld de,#s__INITIALIZED\n\tld hl,#s__INITIALIZER\n\tldir\ngsdone:\n\t.area _GSFINAL\n\tret\n", 1)
    .replace("\t.area _DATA\n", "\t.area _DATA\n\t.area _INITIALIZED\n")
)


def verdict(got, want):
    """Raises `Void` naming the first value that differs, or the lengths when one output is a prefix of the other."""
    if got == want:
        return
    first = next((i for i, (a, b) in enumerate(zip(got, want)) if a != b), min(len(got), len(want)))
    at = lambda values: values[first] if first < len(values) else "none"
    raise Void(f"WRONG value {first}: got {at(got)}, want {at(want)}")


PREDEFINED = re.compile(r"#define (__[A-Z0-9_]*_TYPE__|__SIZEOF_[A-Z0-9_]*__|__[A-Z0-9_]*_(?:MAX|WIDTH)__|__BYTE_ORDER__|__ORDER_[A-Z0-9_]*__|__CHAR_BIT__) (.+)")


def sdcc_preinclude(clang, path):
    """A header of the type, size, limit and byte-order macros the corpus's clang predefines (for a 16-bit int target) and SDCC's
    preprocessor lacks: `typedef __INT32_TYPE__ s32;` is a syntax error there, not an unsupported feature. One force-included
    file, not `-D` flags: sdcc aborts with a buffer overflow past about 90 of them. A macro SDCC defines itself keeps its
    value (`#ifndef`), so a program that checks `sizeof` against it reads SDCC's real sizes."""
    out = run.sh([clang, "-target", "msp430", "-dM", "-E", "-"], run.ROOT, inp="").stdout
    macros = [m for m in map(PREDEFINED.fullmatch, out.splitlines()) if m]
    path.write_text("".join(f"#ifndef {m.group(1)}\n#define {m.group(1)} {m.group(2)}\n#endif\n" for m in macros))
    return [f"-Wp-include,{path}", "-D__inline=", "-D__inline__="]


CONDITIONAL = re.compile(r"^\s*#\s*(?:if|ifdef|ifndef|elif)\b(.*)$", re.M)
DEFINED = re.compile(r"#define ([A-Za-z_]\w*) (.+)")


def predefined_values(clang):
    """Every object-like macro the corpus's clang predefines, as `{name: value}`."""
    out = run.sh([clang, "-target", "msp430", "-dM", "-E", "-"], run.ROOT, inp="").stdout
    return {m.group(1): m.group(2) for m in map(DEFINED.fullmatch, out.splitlines()) if m}


def branch_macros(source, defined):
    """The macros a program's `#if`/`#ifdef`/`#elif` lines name that the corpus's clang defines: SDCC must see them too, or it
    builds the other branch of the same file (variadic took `#else`, hand `va_arg` macros, on SDCC and `__clang__` on ours:
    331 B against 547 B was two different programs)."""
    named = set(re.findall(r"[A-Za-z_]\w*", " ".join(CONDITIONAL.findall(source))))
    return {name: defined[name] for name in sorted(named) if name in defined}


def branch_header(source, defined, path):
    """The force-included header that gives SDCC the branch macros of `source`; its flags, or none for a source with no such macro."""
    macros = branch_macros(source, defined)
    if not macros:
        return []
    path.write_text("".join(f"#ifndef {n}\n#define {n} {v}\n#endif\n" for n, v in macros.items()))
    return [f"-Wp-include,{path}"]


class Void(Exception):
    """A cell without a row; the message is the reason that is written next to it."""


def image_bytes(ihx):
    return sum(int(line[1:3], 16) for line in open(ihx) if line.startswith(":") and line[7:9] == "00")


def harness(report, crt0):
    d = Path(tempfile.mkdtemp(prefix="zph-"))
    (d / "crt0.s").write_text(crt0)
    (d / "report.s").write_text(report)
    for stem in ("crt0", "report"):
        run.sh(["sdasz80", "-g", "-o", f"{stem}.rel", f"{stem}.s"], d)
    run.sh(["sdldz80", "-i", "-b", "_CODE=0x100", "x.ihx", "crt0.rel", "report.rel"], d)
    n = image_bytes(d / "x.ihx")
    shutil.rmtree(d, True)
    return n


def link_and_run(w, objs, extra, ours_text, ctx):
    """(bytes, clocks, values) of the linked objects on ucsim; `Void` names why there is none."""
    ours = ours_text is not None
    area = ["-b", "_DATA=0x9000"] if not ours else []
    k = run.sh(["sdldz80", "-i", "-m", "-b", "_CODE=0x100", *area, "x.ihx", "crt0.rel", *objs, "report.rel", *extra], w)
    if k.returncode or "?ASlink-W-Undefined" in k.stdout + k.stderr:
        undefined = sorted(set(re.findall(r"Undefined Global (\S+)", k.stdout + k.stderr)))
        raise Void("LINK " + " ".join(undefined)[:100])
    areas = ("_BSS",) if ours else ("_BSS", "_DATA")
    try:
        r = run.sh(["ucsim_z80", "-t", "Z80", "-q", "-w", "x.ihx", "-c", "-"], w,
                   f"sp_limit=0\n{run.zero_areas((w / 'x.map').read_text(), areas)}run\nstate\ndump rom {run.LOG:#x} {run.LOG + 0xFFF:#x}\nquit\n",
                   timeout=ctx.timeout)
    except subprocess.TimeoutExpired:
        raise Void(f"TIMEOUT {ctx.timeout} s")
    if "Halted" not in r.stdout:
        raise Void("NOHALT")
    clocks = int(re.search(r"Total time since last reset=.*\((\d+) clks\)", r.stdout).group(1))
    size = image_bytes(w / "x.ihx")
    if ours:
        graph = ctx.graph
        wanted = measure.runtime_bytes(w, graph, ours_text, set(graph))
        size += wanted - measure.code_size(str(w / "rt.rel")) - ctx.harness_ours
    else:
        m = re.search(r"([0-9A-F]{8})\s+l__INITIALIZER\b", (w / "x.map").read_text())
        n = int(m.group(1), 16) if m else 0
        if n:
            clocks -= 21 * n + 10
        size -= ctx.harness_sdcc
    return size, clocks, run.reported(r.stdout)


def assemble(w, report, crt0):
    (w / "crt0.s").write_text(crt0)
    (w / "report.s").write_text(report)
    for stem in ("crt0", "report"):
        run.sh(["sdasz80", "-g", "-o", f"{stem}.rel", f"{stem}.s"], w)


def ours_cell(path, w, ctx, want):
    ll = w / "p.ll"
    c = run.sh([ctx.clang, f"-O{ctx.level}", *run.CLANG_FLAGS, *run.program_flags(path)[0], str(path), "-o", str(ll)], run.ROOT)
    if c.returncode:
        error = next((l for l in c.stderr.splitlines() if "error" in l), "")
        raise Void("CLANG " + error.split("error:")[-1].strip()[:100])
    text = [l for l in ll.read_text().splitlines() if not l.startswith("target datalayout")]
    ll.write_text(run.DATALAYOUT + "\n" + "\n".join(text) + "\n")
    flags = run.program_flags(path)[0] if ctx.program_flags_to_llc else []
    try:
        p = run.llc_limited([ctx.llc, "-S", f"-O{ctx.level}", *flags, "-o", str(w / "p.s"), str(ll)], w, ctx.compile_timeout)
    except subprocess.TimeoutExpired:
        raise Void(f"HUNG llc made no progress for {ctx.compile_timeout * 20} s")
    if p.returncode == run.CPU_LIMIT:
        raise Void(f"SLOW llc over {ctx.compile_timeout} s of cpu")
    if p.returncode:
        raise Void("COMPILE " + (p.stderr.strip()[-100:] or "llc failed"))
    if path.read_text().startswith("#define double float\n") and (calls := same_width.f64_helpers((w / "p.s").read_text())):
        raise Void("WIDTH f64 helper called in a same-width source: " + " ".join(calls)[:80])
    (w / "rt.s").write_text(run.RUNTIME.read_text())
    assemble(w, run.REPORT, run.CRT0)
    for stem in ("rt", "p"):
        a = run.sh(["sdasz80", "-g", "-o", f"{stem}.rel", f"{stem}.s"], w)
        if a.returncode:
            raise Void("ASM " + (a.stdout + a.stderr).strip().splitlines()[-1][:80])
    size, clocks, got = link_and_run(w, ["p.rel", "rt.rel"], ["-k", ctx.libc, "-l", "libc"], (w / "p.s").read_text(), ctx)
    verdict(got, want)
    return size, clocks


def sdcc_cell(path, w, tag, flags, ctx, want):
    d = w / tag
    d.mkdir()
    assemble(d, REPORT_SDCC, CRT0_SDCC)
    try:
        branch = branch_header(path.read_text(errors="replace"), ctx.predefined, d / "branch.h")
        s = run.sh(["sdcc", "-mz80", "-c", *flags, "-D__attribute__(x)=", *ctx.defines, *branch, str(path), "-o", "p.rel"], d, timeout=ctx.sdcc_timeout)
    except subprocess.TimeoutExpired:
        raise Void("SDCC timeout")
    if s.returncode or not (d / "p.rel").exists():
        err = re.search(r"error \d+: .*", s.stdout + s.stderr)
        raise Void("SDCC " + (err.group(0)[:90] if err else "failed"))
    size, clocks, got = link_and_run(d, ["p.rel"], ["-k", "/usr/share/sdcc/lib/z80", "-l", "z80"], None, ctx)
    verdict(got, want)
    return size, clocks


def one(job):
    """`(name, rows, reasons)`: `rows` are `(column, bytes, clocks)`, `reasons` is `{column: why}` for every column without one."""
    path, work, ctx = job
    w = Path(tempfile.mkdtemp(prefix=f"zp-{path.stem}-", dir=work))
    want = run.expected(path.with_suffix(".out"))
    rows, reasons = [], {}
    not_a_bar = guard.incomparable(path.read_text(errors="replace"))
    columns = [] if ctx.ours_only or not_a_bar else SDCC
    reasons.update({tag: f"NOTCOMPARABLE {not_a_bar}" for tag, _ in SDCC if not_a_bar and not ctx.ours_only})
    cells = ([] if ctx.sdcc_only else [("ours", lambda: ours_cell(path, w, ctx, want))]) + \
            [(tag, lambda tag=tag, flags=flags: sdcc_cell(path, w, tag, flags, ctx, want)) for tag, flags in columns]
    for column, cell in cells:
        try:
            rows.append((column, *cell()))
        except Void as why:
            reasons[column] = str(why)
        except Exception as error:  # a cell that raises is a cell without a row, not a missing program
            reasons[column] = f"EXC {type(error).__name__}: {str(error)[:80]}"
    shutil.rmtree(w, True)
    return path.stem, rows, reasons


def md5(path):
    return hashlib.md5(open(path, "rb").read()).hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("llc")
    ap.add_argument("out")
    ap.add_argument("names", nargs="*")
    ap.add_argument("--level", default="2")
    ap.add_argument("--src", default=str(run.ROOT / "tests/run/c"))
    ap.add_argument("--clang", default=os.environ.get("LLRM_PIN_CLANG", os.path.expanduser("~/scratch/llvm-build/bin/clang")))
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--timeout", type=int, default=60, help="seconds a program may run on ucsim")
    ap.add_argument("--compile-timeout", type=int, default=120)
    ap.add_argument("--sdcc-timeout", type=int, default=300, help="seconds SDCC may compile one cell (the -ma variants of div_constants_long need more)")
    ap.add_argument("--program-flags-to-llc", action="store_true")
    ap.add_argument("--ours-only", action="store_true")
    ap.add_argument("--sdcc-only", action="store_true")
    ap.add_argument("--runtime", default=str(run.RUNTIME), help="the z80_rt.s to link (the compiler's own build of it)")
    ap.add_argument("--allow-runtime-mismatch", action="store_true")
    ap.add_argument("--no-library-checks", action="store_true", help="skip the soft-float library's own run checks (a full sweep runs them first)")
    ap.add_argument("--no-softfloat", action="store_true", help="run without Z80_LIBC_EXTRA: the float programs will not link")
    ctx = ap.parse_args()
    run.RUNTIME = Path(ctx.runtime)
    if why := guard.softfloat_library_missing(os.environ, ctx.sdcc_only, ctx.no_softfloat):
        sys.exit(f"sweep.py: {why}")
    if not ctx.sdcc_only and not ctx.allow_runtime_mismatch and (why := run.runtime_mismatch(ctx.llc, run.RUNTIME)):
        sys.exit(f"sweep.py: {why}")
    if not (ctx.sdcc_only or ctx.no_softfloat or ctx.no_library_checks or ctx.names):
        if failed := guard.failed_library_checks(str(run.ROOT), ctx.llc):
            sys.exit("sweep.py: library check failed: " + "; ".join(f"{script}: {line}" for script, line in failed.items()))
    corpus = [p.stem for p in run.selected(Path(ctx.src))]
    expected = ctx.names or corpus
    # a requested name the corpus does not select (a typo, or tagged for other targets) is a dropped program, not a skipped one
    unknown = [n for n in expected if n not in corpus]
    sources = [Path(ctx.src) / f"{n}.c" for n in expected if n in corpus]
    llc_md5 = md5(ctx.llc)
    work = tempfile.mkdtemp(prefix="zprogs-")
    try:
        ctx.libc = run.libc_library(work)
        ctx.defines = sdcc_preinclude(ctx.clang, Path(work) / "predefined.h")
        ctx.predefined = predefined_values(ctx.clang)
        ctx.graph = measure.runtime_calls(str(run.RUNTIME))
        ctx.harness_ours = harness(run.REPORT, run.CRT0)
        ctx.harness_sdcc = harness(REPORT_SDCC, CRT0_SDCC)
        with ThreadPoolExecutor(ctx.jobs) as pool:
            done = list(pool.map(one, [(s, work, ctx) for s in sources]))
    finally:
        shutil.rmtree(work, True)
    if md5(ctx.llc) != llc_md5:
        sys.exit(f"sweep: {ctx.llc} changed during the run; results void")
    results = [(name, rows) for name, rows, _ in done]
    reasons = {name: why for name, _, why in done}
    required = (lambda tag: tag.startswith("sdcc")) if ctx.sdcc_only else (lambda tag: tag == "ours")
    excused = {n for n, why in reasons.items() if why and ctx.sdcc_only}  # a recorded SDCC error is a gap in the bar
    lost = guard.dropped(expected, results, required, excused)
    with open(ctx.out, "w") as f:
        for name, rows in results:
            for column, size, clocks in rows:
                f.write(f"{name}\t{column}\t{size}\t{clocks}\n")
    why = lambda n: "NOTSELECTED" if n in unknown else "; ".join(f"{c}: {r}" for c, r in sorted(reasons.get(n, {}).items())) or "NOROW"
    with open(ctx.out + ".failed", "w") as f:
        f.writelines(f"{n}\t{why(n)}\n" for n in sorted(lost))
    with open(ctx.out + ".nobar", "w") as f:
        for name, rows in results:
            if not any(c.startswith("sdcc") for c, _, _ in rows):
                f.write(f"{name}\t" + "; ".join(f"{c}: {r}" for c, r in sorted(reasons[name].items())) + "\n")
    line, ok = guard.summary(expected, results, {n: why(n) for n in lost}, required, excused)
    print(line)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
