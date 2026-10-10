"""How the compile time of llrm, gcc and clang grows with program size.

    uv run --project tools --with matplotlib python crates/target/llrm-x86-m32/vsgcc/scaling.py run [--axis NAME ...] [--programs]
    uv run --project tools --with matplotlib python crates/target/llrm-x86-m32/vsgcc/scaling.py report

`run` writes $VSGCC_WORK/scaling.json; `report` prints the tables and writes one PNG per axis there. Commands and flags are
levels_time.py's. Cost is `perf stat -e instructions:u,task-clock` (children included; task-clock in ns), median of RUNS,
less what the same compiler spends on an empty file. Axes are generated C programs whose size doubles from 2^MIN until a
compile takes LIMIT seconds; `--programs` adds QCport's modules, each as one point of (MIR instructions, cost).
`run` at -O2 also keeps llrm's per-pass own time (LLRM_DEBUG=time) at each size, and the MIR size (LLRM_DEBUG=mir).
"""
from concurrent.futures import ThreadPoolExecutor
import argparse, json, math, os, re, shutil, statistics, subprocess, sys, tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import levels_time

WORK = Path(os.environ.get("VSGCC_WORK", Path.home() / "scratch/vsgcc-work"))
COMPILERS = ("llrm", "gcc", "clang")
LEVELS = levels_time.LEVELS


# --- generators: each takes the size N and returns a C program that prints a checksum -----------------------------------


def lcg(k: int) -> int:
    return (k * 2654435761 + 12345) % 2**32


PRELUDE_C = "extern void report(long value);\n"


def small(k: int) -> int:
    """A small odd multiplier: a 32-bit one sends llrm's isel into a ~75 M-instruction synth_mult search per multiply (see `mulconst`)."""
    return lcg(k) % 61 * 2 + 3


def _main(axis: str, body: str) -> str:
    """`bench_AXIS`, the entry harness.py counts, running `body` (which returns a value), and a main that reports it."""
    return f"long bench_{axis}(int n) {{\n{body}}}\nint main(void) {{\n    long (*volatile entry)(int) = bench_{axis};\n    report(entry(0));\n    return 0;\n}}\n"


def functions(n: int) -> str:
    """N small functions, a loop and a call each, one file."""
    out = [PRELUDE_C, "static volatile unsigned sink;", "unsigned bump(unsigned x) { sink += x; return sink ^ x; }"]
    out += [f"unsigned f{k}(unsigned a) {{ unsigned s = {k + 1}u, i; for (i = 0; i < a; i++) s = s * {2 * k + 3}u + bump(i ^ s); return s; }}" for k in range(n)]
    out.append(_main("functions", "    unsigned t = 0;\n" + "".join(f"    t = t * 31u + f{k}(3);\n" for k in range(n)) + "    return (long)t;\n"))
    return "\n".join(out)


def straight(n: int) -> str:
    """One function of N straight-line statements on four live values."""
    v = "abcd"
    body = "".join(f"    {v[k % 4]} = {v[k % 4]} * {small(k)}u + ({v[(k + 1) % 4]} ^ ({v[(k + 2) % 4]} >> {k % 7 + 1})) + {lcg(k + 1)}u;\n" for k in range(n))
    return PRELUDE_C + "unsigned fn(unsigned a, unsigned b, unsigned c, unsigned d) {\n" + body + "    return a ^ b ^ c ^ d;\n}\n" + _main("straight", "    return (long)fn(1, 2, 3, 4);\n")


def mulconst(n: int) -> str:
    """N multiplies by large odd 32-bit constants on four values: what instruction selection spends on constant multiplies."""
    v = "abcd"
    body = "".join(f"    {v[k % 4]} = {v[k % 4]} * {lcg(k) | 1}u + {v[(k + 1) % 4]};\n" for k in range(n))
    return PRELUDE_C + "unsigned fn(unsigned a, unsigned b, unsigned c, unsigned d) {\n" + body + "    return a ^ b ^ c ^ d;\n}\n" + _main("mulconst", "    return (long)fn(1, 2, 3, 4);\n")


def branches(n: int) -> str:
    """One function of N branches and loops (every eighth is a loop) over six values live across all of them."""
    v = "abcdef"
    body = []
    for k in range(n):
        x, y, z = (v[(k + i) % 6] for i in range(3))
        if k % 8 == 7:
            body.append(f"    for (i = 0; i < ({x} & 3u); i++) {y} += i ^ {z};\n")
        else:
            body.append(f"    if (({x} ^ {lcg(k)}u) & {1 << (k % 5)}u) {{ {y} += {x} * {small(k + 1)}u; {z} ^= {y}; }} else {{ {x} += {z}; {y} ^= {x} >> 3; }}\n")
    return PRELUDE_C + "unsigned fn(unsigned a, unsigned b, unsigned c, unsigned d, unsigned e, unsigned f) {\n    unsigned i;\n" + "".join(body) + "    return a ^ b ^ c ^ d ^ e ^ f;\n}\n" + _main("branches", "    return (long)fn(1, 2, 3, 4, 5, 6);\n")


def live(n: int) -> str:
    """One function with N volatile loads, all live across a call, then all used."""
    loads = "".join(f"    unsigned v{k} = p[{k}];\n" for k in range(n))
    uses = "".join(f"    s = s * 31u + (v{k} ^ v{(k + 7) % n});\n" for k in range(n))
    fill = f"    unsigned i;\n    for (i = 0; i < {n}; i++) data[i] = i * 2654435761u;\n"
    return (
        PRELUDE_C + "static volatile unsigned barrier;\nvoid bar(void) { barrier++; }\n"
        f"unsigned fn(const volatile unsigned *p) {{\n{loads}    unsigned s = 0;\n    bar();\n{uses}    return s;\n}}\n"
        f"static unsigned data[{n}];\n" + _main("live", fill + "    return (long)fn(data);\n")
    )


CALLEES = 8


def callers(n: int) -> str:
    """N callers of eight small callees: the inliner's work grows with N."""
    out = [PRELUDE_C] + [f"static unsigned c{k}(unsigned x) {{ return x * {2 * k + 3}u + (x >> {k % 3 + 1}); }}" for k in range(CALLEES)]
    out += [f"unsigned g{j}(unsigned x) {{ return c{j % CALLEES}(c{(j + 1) % CALLEES}(x + {j}u)) ^ c{(j + 3) % CALLEES}(x); }}" for j in range(n)]
    out.append(_main("callers", "    unsigned t = 0;\n" + "".join(f"    t = t * 31u + g{j}({j}u);\n" for j in range(n)) + "    return (long)t;\n"))
    return "\n".join(out)


def chain(n: int) -> str:
    """A call chain N deep, each link a static function with one caller."""
    out = [PRELUDE_C, "static volatile unsigned seed = 5u;", "static unsigned h0(unsigned x) { return x * 3u + 1u; }"]
    out += [f"static unsigned h{k}(unsigned x) {{ return h{k - 1}(x + {k}u) * 3u ^ x; }}" for k in range(1, n)]
    out.append(_main("chain", f"    return (long)h{n - 1}(seed);\n"))
    return "\n".join(out)


def nest(n: int) -> str:
    """One function of loops N deep, each bounded by a different low bits of the argument, a statement at every depth: the passes that
    walk a loop's blocks or its nest (hoist, lsr, jumps, peephole, the allocator) meet each block once per loop around it."""
    v = "abcd"
    out = [PRELUDE_C + "unsigned fn(unsigned a, unsigned b, unsigned c, unsigned d) {\n    unsigned " + ", ".join(f"i{k}" for k in range(n)) + ";\n"]
    for k in range(n):
        pad = "    " * (k + 1)
        x, y = v[k % 4], v[(k + 1) % 4]
        out.append(f"{pad}for (i{k} = 0; i{k} < (({x} >> {k % 5}) & 1u) + 1u; i{k}++) {{\n{pad}    {x} = {x} * {small(k)}u + ({y} ^ i{k});\n")
    out.append("    " * (n + 1) + "a ^= b + c;\n")
    out += ["    " * (k + 1) + "}\n" for k in reversed(range(n))]
    out.append("    return a ^ b ^ c ^ d;\n}\n" + _main("nest", "    return (long)fn(1u, 2u, 3u, 4u);\n"))
    return "".join(out)


def cells(n: int) -> str:
    """One function reading and writing N distinct memory cells (elements of one static array), each read after the stores to every
    cell before it: the loads that memory forwarding looks up meet N different cells, so a pass that scans every cell held per load
    (or per store) is quadratic here and not on `straight`, whose memory is none."""
    body = "".join(
        f"    cell[{k}] = cell[{(k * 7 + 3) % n}] + a;\n    a ^= cell[{(k * 5 + 1) % n}] >> {k % 7 + 1};\n" for k in range(n)
    )
    return PRELUDE_C + f"static unsigned cell[{n}];\nunsigned fn(unsigned a) {{\n" + body + "    return a;\n}\n" + _main("cells", "    return (long)fn(3u);\n")


def joins(n: int) -> str:
    """Four functions, each with N static cells all known after its first N stores and N if/else joins that store to one and read
    another: every join meets N held cells, so a pass that keeps the cells of memory as one map per block, and meets the maps of the
    predecessors at a join (through-memory's dense solve), costs N^2 a function. `cells` is one block and has no join; GORILLA.BAS (1,300
    lines of graphics statements over global state) compiled 17 s at -O1, 62 % of it there."""
    out = [PRELUDE_C]
    for f in range(4):
        out.append(f"static unsigned c{f}[{n}];\nunsigned fn{f}(unsigned a) {{\n")
        out.append("".join(f"    c{f}[{k}] = {k + 1}u;\n" for k in range(n)))
        out.append(
            "".join(
                f"    if ((a >> {k % 7}) & 1u) {{ c{f}[{(k * 3) % n}] = {k + 5}u; }} else {{ a += c{f}[{(k * 5 + 1) % n}]; }}\n"
                for k in range(n)
            )
        )
        out.append(f"    return a + c{f}[{n - 1}];\n}}\n")
    calls = " + ".join(f"fn{f}(3u)" for f in range(4))
    out.append(_main("joins", f"    return (long)({calls});\n"))
    return "".join(out)


def fjoins(n: int) -> str:
    """`joins` in floats: four functions, each with N static float cells all known after N stores and N if/else joins that store one and
    add another, so the float facts meet N held cells at each join. A float load was read from the dense cell map float-facts solved
    beside the integers' (GORILLA.BAS: 6.5 % of its -O1 compile)."""
    out = [PRELUDE_C]
    for f in range(4):
        out.append(f"static float c{f}[{n}];\nfloat fn{f}(unsigned a, float x) {{\n")
        out.append("".join(f"    c{f}[{k}] = {k + 1}.5f;\n" for k in range(n)))
        out.append(
            "".join(
                f"    if ((a >> {k % 7}) & 1u) {{ c{f}[{(k * 3) % n}] = {k + 5}.25f; }} else {{ x += c{f}[{(k * 5 + 1) % n}]; }}\n"
                for k in range(n)
            )
        )
        out.append(f"    return x + c{f}[{n - 1}];\n}}\n")
    calls = " + ".join(f"fn{f}(3u, 1.0f)" for f in range(4))
    out.append(_main("fjoins", f"    return (long)({calls});\n"))
    return "".join(out)


AXES = {"functions": functions, "straight": straight, "mulconst": mulconst, "branches": branches, "live": live, "callers": callers, "chain": chain, "nest": nest, "cells": cells, "joins": joins, "fjoins": fjoins}


# --- measuring -------------------------------------------------------------------------------------------------------


ENVIRONMENT_BYTES = 4096


def measured_environment(env: dict | None = None) -> dict:
    """What the compiler under measurement is given: PATH, LLRM_BIN where set, `env`'s own, and padding to a constant size. The bytes
    of the caller's environment (a sourced script, a longer worktree path) move the stack and the allocator's first pages, and `lir
    peephole` read 157.0 or 168.8 Minstr of cells(224) by 4 bytes of environment alone: base and head must be given the same bytes."""
    kept = {"PATH": "/usr/bin:/bin", **({"LLRM_BIN": os.environ["LLRM_BIN"]} if "LLRM_BIN" in os.environ else {}), **(env or {})}
    used = sum(len(k) + len(v) + 2 for k, v in kept.items())
    return {**kept, "LLRM_PAD": "x" * max(0, ENVIRONMENT_BYTES - used - len("LLRM_PAD") - 2)}


def sample(cmd: list[str], env: dict | None = None, timeout: float = 120) -> tuple[int, int, str]:
    """(instructions:u, task-clock ns, stderr) of one run; raises on a failed compile.

    The child sees no LLRM_ variable of the caller's but LLRM_BIN (and `env`'s own): LLRM_CHECK_*, LLRM_VERIFY and the like add work to
    the step they check, and a count taken under them is not the compiler's (regparm16: 'lir peephole' read 4.5 Minstr over at every size).

    A source file is compiled from a directory of its own under /tmp, as `src.c`, and an llrm binary is run as `./llrm-c` through a
    link: the length of the paths the compiler is given (its own name as run, the source's) moves the allocator's pages, and `lir
    peephole` read 173.7 or 159.0 Minstr of cells(224) by that length alone (mimalloc: a block that lives all run in a size class's
    page, or none, decides whether the page is given back and made again at every free). Base and head, and one run and the next,
    must be given the same strings.
    """
    source = Path(cmd[-1]) if cmd and cmd[-1].endswith(".c") else None
    with tempfile.TemporaryDirectory(prefix="llrm-", dir="/tmp") as here, tempfile.NamedTemporaryFile("r") as out:
        if source is not None and source.is_file():
            shutil.copyfile(source, Path(here) / "src.c")
            cmd = [*cmd[:-1], "src.c"]
        if Path(cmd[0]).name.startswith("llrm-") and Path(cmd[0]).is_file():
            (Path(here) / Path(cmd[0]).name).symlink_to(Path(cmd[0]).resolve())
            cmd = [f"./{Path(cmd[0]).name}", *cmd[1:]]
        done = subprocess.run([*levels_time.UNRANDOMIZED, "perf", "stat", "-x,", "-e", "instructions:u,task-clock", "-o", out.name, *cmd], capture_output=True, text=True, timeout=timeout, cwd=here, env=measured_environment(env))
        if done.returncode:
            said = [l for l in (done.stderr or done.stdout).splitlines() if l and not l.startswith(("[time]", "[mir]"))]
            raise RuntimeError(f"{' '.join(cmd[-1:])}: " + " | ".join(said)[:300])
        found = {l.split(",")[2]: int(float(l.split(",")[0])) for l in out.read().splitlines() if l and not l.startswith("#")}
    return found["instructions:u"], found["task-clock"], done.stderr


def measure(cmd: list[str], runs: int = 3, env: dict | None = None) -> dict:
    """Median instructions and task-clock ns of `runs` runs."""
    got = [sample(cmd, env)[:2] for _ in range(runs)]
    return {"ins": int(statistics.median(g[0] for g in got)), "ns": int(statistics.median(g[1] for g in got))}


def slope(xs: list[float], ys: list[float]) -> float:
    """Least-squares exponent of y = c * x^k, from the points with y > 0."""
    pts = [(math.log(x), math.log(y)) for x, y in zip(xs, ys) if x > 0 and y > 0]
    if len(pts) < 2:
        return float("nan")
    mx, my = statistics.fmean(p[0] for p in pts), statistics.fmean(p[1] for p in pts)
    return sum((a - mx) * (b - my) for a, b in pts) / sum((a - mx) ** 2 for a, _ in pts)


def net(rows: list[dict], base: float, key: str = "ins") -> tuple[list[int], list[float]]:
    """(sizes, cost less `base`) of a series."""
    return [r["n"] for r in rows], [r[key] - base for r in rows]


TAIL = 4


def exponents(rows: list[dict], base: float, key: str = "ins") -> tuple[float, float]:
    """(slope over every size, slope over the largest TAIL sizes) of a series' cost above the empty file's."""
    xs, ys = net(rows, base, key)
    return slope(xs, ys), slope(xs[-TAIL:], ys[-TAIL:])


def parse_time(stderr: str) -> dict:
    """llrm's `[time] by own time` rows as {step: ms} and its `[instr]` rows as {step: own Minstr}, and `[mir] functions F instructions I`.
    ms is time under whatever else the host is doing; the instructions are the work, and what steps are ranked and fitted on."""
    steps = {m.group(2): float(m.group(1)) for m in re.finditer(r"^\[time\]\s+([\d.]+) ms own\s+[\d.]+ ms total\s+\d+x (.+)$", stderr, re.M)}
    instr = {m.group(2): float(m.group(1)) for m in re.finditer(r"^\[instr\]\s+([\d.]+) Minstr own\s+[\d.]+ Minstr total\s+\d+x (.+)$", stderr, re.M)}
    mir = re.search(r"^\[mir\] functions (\d+) instructions (\d+)", stderr, re.M)
    return {"steps": steps, "instr": instr, "functions": int(mir.group(1)) if mir else None, "mir": int(mir.group(2)) if mir else None}


def llrm_profile(source: Path, level: str = "O2", extra: list[str] = ()) -> dict:
    """One run of llrm with its time and MIR channels on: per-step own ms, MIR instructions and functions."""
    cmd = levels_time.command("llrm", level, source)
    cmd[-1:-1] = list(extra)
    return parse_time(sample(cmd, {"LLRM_DEBUG": "time,mir", "LLRM_TIME_TOP": "100000"})[2])


def check(axis: str, n: int, work: Path) -> str:
    """Runs the axis' program at size n three ways and returns the one value they agree on, else raises: gcc and clang on
    the host at -O0 and -O2, and llrm -O2 in harness.py's emulator (OMF linked there, `report` from stub.elf)."""
    import harness

    if not (harness.OUT / "stub.elf.src").exists():  # as run.sh builds it
        harness.OUT.mkdir(parents=True, exist_ok=True)
        subprocess.run(["gcc", "-m32", "-c", str(Path(harness.__file__).with_name("stub.s")), "-o", str(harness.OUT / "stub.o")], check=True)
        emulation = subprocess.run([sys.executable, str(levels_time.R / "tools/linkrecipe.py"), "x86-m32", "ld-emulation"], check=True, capture_output=True, text=True).stdout.strip()
        subprocess.run(["ld", "-m", emulation, "-static", "-e", "0", "-Ttext=0x8000", "-o", str(harness.OUT / "stub.elf"), str(harness.OUT / "stub.o")], check=True)
        harness.record_stub(harness.OUT)
    source = work / f"{axis}.c"
    source.write_text(AXES[axis](n))
    (work / "report.c").write_text("#include <stdio.h>\nvoid report(long v) { printf(\"%u\\n\", (unsigned)v); }\n")
    seen = {}
    for compiler, level in (("gcc", "O0"), ("gcc", "O2"), ("clang", "O2")):
        exe = work / f"{axis}.{compiler}{level}"
        subprocess.run([compiler, f"-{level}", "-w", "-o", str(exe), str(source), str(work / "report.c")], check=True, capture_output=True)
        seen[f"{compiler} {level}"] = int(subprocess.run([str(exe)], check=True, capture_output=True, text=True).stdout)
    (harness.OUT / "o").mkdir(parents=True, exist_ok=True)
    subprocess.run(levels_time.command("llrm", "O2", source)[:-3] + ["-o", str(harness.OUT / "o" / f"{axis}.llrm.obj"), str(source)], check=True, capture_output=True)
    seen["llrm O2"] = harness.run(axis, "llrm")["reports"][0] % 2**32
    if len(set(seen.values())) != 1:
        raise AssertionError(f"{axis} {n}: the compilers disagree: {seen}")
    return str(seen["gcc O0"])


# --- the generated axes ----------------------------------------------------------------------------------------------


def run_axis(axis: str, sizes: list[int], runs: int, limit_ns: float, work: Path, command=levels_time.command, compilers=COMPILERS, levels=LEVELS, profile: bool = True) -> dict:
    """{'base': {compiler level: cost of empty.c}, 'series': {compiler level: [{n, ins, ns}]}, 'passes': {n: llrm -O2 profile}}.
    A series stops at the first compile over `limit_ns` or that fails."""
    empty = work / f"{axis}_empty.c"
    empty.write_text("")
    keys = [f"{c} {l}" for c in compilers for l in levels]
    result = {"base": {k: measure(command(*k.split(), empty), runs) for k in keys}, "series": {}, "passes": {}}
    live_series = set(keys)
    for n in sizes:
        source = work / f"{axis}_{n}.c"
        source.write_text(AXES[axis](n))
        for key in keys:
            if key not in live_series:
                continue
            try:
                point = measure(command(*key.split(), source), runs)
            except (RuntimeError, subprocess.TimeoutExpired) as error:
                result.setdefault("failed", {})[key] = {"n": n, "why": str(error)[-300:]}
                live_series.discard(key)
                continue
            result["series"].setdefault(key, []).append({"n": n, **point})
            if point["ns"] > limit_ns:
                live_series.discard(key)
        if profile and "llrm O2" in result["series"] and result["series"]["llrm O2"][-1]["n"] == n:
            result["passes"][str(n)] = llrm_profile(source)
        print(f"{axis} {n}: " + " ".join(f"{k}={result['series'][k][-1]['ins'] / 1e6:.0f}M" for k in keys if result["series"].get(k) and result["series"][k][-1]["n"] == n), flush=True)
        source.unlink()
        if not live_series:
            break
    return result


# --- QCport and the real programs ------------------------------------------------------------------------------------

PARTS = ("host", "render", "model", "game", "sound", "ui", "qgl")
PRELUDE = """\
/* Borland names gcc and clang lack, forced in front of every QCport module (no system headers: -m32 has none here). */
int stricmp(const char *, const char *);
int _fstricmp(const char *, const char *);
int _fstrncmp(const char *, const char *, unsigned);
void *_fmemcpy(void *, const void *, unsigned);
void *_fmemmove(void *, const void *, unsigned);
void *_fmemset(void *, int, unsigned);
int _fmemcmp(const void *, const void *, unsigned);
#define FP_OFF(p) ((unsigned)(unsigned long)(p))
#define FP_SEG(p) 0u
#define __emit__(...)
"""
NEUTRALISED = [
    (r"^typedef char \w+_ok\s*\[[^\n]*\];", "16-bit struct-size asserts (`typedef char rec_*_ok[..]`; the sizes are 16-bit ones)"),
    (r"\b_?_asm\b\s*(\{[^{}]*\}|[^\n;{]*)", "inline assembly (`__asm { .. }`, `_asm ..`), which gcc and clang cannot read"),
]
SIZES = ["-Dfar=", "-Dhuge=", "-Dnear="]  # every compiler: -m32 has no segments
FOREIGN = ["-Dpascal=", "-Dcdecl=", "-Wno-implicit-function-declaration", "-w"]


def qcport_tree(qcport: Path, include: Path, work: Path) -> tuple[list[Path], dict]:
    """A copy of QCport's `src` with the Borland-only text stubbed, and of the Borland headers without their ^Z.
    Returns (the .c files, {what: [files stubbed]}); every compiler reads the same stubbed text."""
    stubbed: dict[str, list[str]] = {what: [] for _, what in NEUTRALISED}
    stubbed["^Z end-of-file byte in Borland headers (gcc: stray \\32)"] = []
    (work / "inc").mkdir(parents=True, exist_ok=True)
    for header in include.glob("**/*.[hH]"):
        text = header.read_bytes()
        target = work / "inc" / header.relative_to(include).as_posix().lower()
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(text.replace(b"\x1a", b""))
        if b"\x1a" in text:
            stubbed["^Z end-of-file byte in Borland headers (gcc: stray \\32)"].append(header.name)
    sources = []
    for part in PARTS:
        (work / "src" / part).mkdir(parents=True, exist_ok=True)
        for file in sorted((qcport / part).glob("*.[ch]")):
            text = original = file.read_text(encoding="latin-1")
            for pattern, what in NEUTRALISED:
                text, hits = re.subn(pattern, ";", text, flags=re.M)
                if hits:
                    stubbed[what].append(f"{part}/{file.name}")
            (work / "src" / part / file.name).write_text(text, encoding="latin-1")
            if file.suffix == ".c":
                sources.append(work / "src" / part / file.name)
    (work / "prelude.h").write_text(PRELUDE)
    return sources, stubbed


def qcport_flags(work: Path, compiler: str) -> list[str]:
    includes = [flag for part in PARTS for flag in ("-I", str(work / "src" / part))] + ["-I", str(work / "inc")]
    return includes if compiler == "llrm" else [*SIZES, *FOREIGN, "-include", str(work / "prelude.h"), *includes]


def run_programs(qcport: Path, include: Path, runs: int, work: Path, jobs: int = 6) -> dict:
    sources, stubbed = qcport_tree(qcport, include, work)
    def one(source: Path) -> tuple[str, dict]:
        row = {"failed": {}}
        try:
            profile = llrm_profile(source, "O2", qcport_flags(work, "llrm"))
            row["functions"], row["mir"], row["steps"], row["instr"] = profile["functions"], profile["mir"], profile["steps"], profile["instr"]
        except (RuntimeError, subprocess.TimeoutExpired) as error:
            row["failed"]["llrm O2 profile"] = str(error)[-200:]
        for compiler in COMPILERS:
            for level in LEVELS:
                cmd = levels_time.command(compiler, level, source)
                cmd[-1:-1] = qcport_flags(work, compiler)
                try:
                    row[f"{compiler} {level}"] = measure(cmd, runs)
                except (RuntimeError, subprocess.TimeoutExpired) as error:
                    row["failed"][f"{compiler} {level}"] = str(error)[-200:]
        print(f"{source.stem}: " + (f"mir {row.get('mir')} functions {row.get('functions')}, failed: {sorted(row['failed'])}" if row["failed"] else f"mir {row['mir']} functions {row['functions']}"), flush=True)
        return f"{source.parent.name}/{source.stem}", row

    with ThreadPoolExecutor(jobs) as pool:
        rows = dict(pool.map(one, sources))
    empty = work / "empty.c"
    empty.write_text("")
    base = {f"{c} {l}": measure(levels_time.command(c, l, empty), runs) for c in COMPILERS for l in LEVELS}
    return {"rows": rows, "stubbed": stubbed, "base": base}


# --- the report ------------------------------------------------------------------------------------------------------


def axis_tables(name: str, data: dict) -> list[str]:
    """Slopes of the three compilers over the sizes llrm reached (a compiler that reaches further is superlinear there too,
    which would skew a comparison over unequal ranges), and gcc's over its own range."""
    lines = [f"### {name}", "", "| level | llrm slope (all / top 4 sizes) | gcc, same N | clang, same N | gcc, its own range | llrm/gcc total, smallest N | llrm/gcc total, largest N | llrm/gcc net of empty file, largest N | sizes llrm / gcc |", "|---|---|---|---|---|---|---|---|---|"]
    for level in LEVELS:
        llrm, gcc = data["series"].get(f"llrm {level}", []), data["series"].get(f"gcc {level}", [])
        same = {r["n"] for r in llrm}
        cells = []
        for compiler in COMPILERS:
            rows = [r for r in data["series"].get(f"{compiler} {level}", []) if r["n"] in same]
            cells.append("{:.2f} / {:.2f}".format(*exponents(rows, data["base"][f"{compiler} {level}"]["ins"])) if len(rows) > 1 else "-")
        own = "{:.2f} / {:.2f}".format(*exponents(gcc, data["base"][f"gcc {level}"]["ins"])) if len(gcc) > 1 else "-"
        both = sorted({r["n"] for r in llrm} & {r["n"] for r in gcc})
        ratio = lambda n: next(r["ins"] for r in llrm if r["n"] == n) / next(r["ins"] for r in gcc if r["n"] == n)
        at = lambda n: f"{ratio(n):.2f}x @ 2^{int(math.log2(n))}"
        base = lambda c: data["base"][f"{c} {level}"]["ins"]
        netted = lambda n: (next(r["ins"] for r in llrm if r["n"] == n) - base("llrm")) / (next(r["ins"] for r in gcc if r["n"] == n) - base("gcc"))
        lines.append(f"| -{level} | {cells[0]} | {cells[1]} | {cells[2]} | {own} | " + (f"{at(both[0])} | {at(both[-1])} | {netted(both[-1]):.2f}x" if both else "- | - | -") + f" | {len(llrm)} / {len(gcc)} |")
    return lines + [""]


def pass_table(name: str, data: dict, floor: float = 1.0, steepest: float = 1.15, shown: int = 10) -> list[str]:
    """Steps of llrm -O2 that grow faster than `steepest`, from the sizes where they take `floor` or more: own Minstr where every size
    has them, else own ms (a profile taken before the instruction rows)."""
    sizes = sorted(int(n) for n in data["passes"])
    if len(sizes) < 3:
        return []
    key, unit = ("instr", "Minstr") if all(data["passes"][str(n)].get("instr") for n in sizes) else ("steps", "ms")
    last = data["passes"][str(sizes[-1])][key]
    lines = [f"Passes of llrm -O2 above exponent {steepest} on {name} (top {TAIL} sizes, own {unit}; largest N = {sizes[-1]}):", "", f"| step | {unit} @ largest | slope |", "|---|---|---|"]
    found = []
    for step, cost in sorted(last.items(), key=lambda kv: -kv[1]):
        pts = [(n, data["passes"][str(n)][key].get(step, 0.0)) for n in sizes[-TAIL:]]
        if cost >= floor and (s := slope([p[0] for p in pts], [p[1] for p in pts])) > steepest:
            found.append(f"| {step} | {cost:.1f} | {s:.2f} |")
    return lines + (found[:shown] or ["| (none) | | |"]) + [""]


def chart(name: str, data: dict, out: Path, level: str = "O2") -> None:
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fig, ax = plt.subplots(figsize=(6, 4.2))
    for compiler, colour in zip(COMPILERS, ("#d55e00", "#0072b2", "#009e73")):
        rows = data["series"].get(f"{compiler} {level}", [])
        if f"{compiler} {level}" not in data["base"]:
            continue
        base = data["base"][f"{compiler} {level}"]["ins"]
        xs, ys = net(rows, base)
        if len(xs) > 1:
            ax.plot(xs, ys, "o-", color=colour, label=f"{compiler} (slope {exponents(rows, base)[0]:.2f})")
    ax.set_xscale("log", base=2)
    ax.set_yscale("log")
    ax.set_xlabel(f"{name}: N")
    ax.set_ylabel("instructions:u above an empty file")
    ax.set_title(f"compile cost, -{level}")
    ax.grid(True, which="both", alpha=0.3)
    ax.legend()
    fig.tight_layout()
    fig.savefig(out, dpi=130)
    plt.close(fig)


def program_tables(data: dict) -> list[str]:
    rows = {k: v for k, v in data["rows"].items() if v.get("mir")}
    lines = [f"### QCport: {len(data['rows'])} modules, cost against llrm's MIR instructions (instructions:u; slopes and \"net\" ratios are less the empty file's cost: gcc 18.7 M, clang 41 M, llrm 8.1 M)", ""]
    lines += ["| level | llrm slope (net) | gcc | clang | llrm/gcc total, geomean | llrm/gcc net of empty file, geomean | worst total llrm/gcc | modules |", "|---|---|---|---|---|---|---|---|"]
    for level in LEVELS:
        both = {k: v for k, v in rows.items() if all(f"{c} {level}" in v for c in COMPILERS)}
        xs = [v["mir"] for v in both.values()]
        cells = [f"{slope(xs, [v[f'{c} {level}']['ins'] - data['base'][f'{c} {level}']['ins'] for v in both.values()]):.2f}" for c in COMPILERS]
        ratios = {k: v[f"llrm {level}"]["ins"] / v[f"gcc {level}"]["ins"] for k, v in both.items()}
        worst = max(ratios, key=ratios.get)
        nets = [(v[f"llrm {level}"]["ins"] - data["base"][f"llrm {level}"]["ins"]) / (v[f"gcc {level}"]["ins"] - data["base"][f"gcc {level}"]["ins"]) for v in both.values()]
        lines.append(f"| -{level} | {cells[0]} | {cells[1]} | {cells[2]} | {levels_time.geomean(list(ratios.values())):.2f}x | {levels_time.geomean([n for n in nets if n > 0]):.2f}x | {ratios[worst]:.2f}x {worst} | {len(both)} |")
    failed = {k: v["failed"] for k, v in data["rows"].items() if v["failed"]}
    reason = lambda why: "peephole: value read but never defined" if "peephole" in why else "front end E1060 Invalid type (dos.h _FAR)" if "E1060" in why else why.strip()[-80:]
    lines += ["", "Modules a compiler failed on (levels, reason):", ""]
    lines += [f"- {k}: " + "; ".join(f"{c.split()[0]} {','.join(x.split()[1] for x in f if x.startswith(c.split()[0]) and 'profile' not in x)}: {reason(why)}" for c, why in [one for one in f.items() if 'profile' not in one[0]][:1]) for k, f in failed.items()]
    lines += ["", "Stubbed so gcc and clang can read QCport (llrm reads the same stubbed text):", ""]
    lines += [f"- {what}: {len(files)} files" for what, files in data["stubbed"].items()]
    lines += ["- Borland names declared for gcc/clang only (`-include prelude.h`): _fmemcpy, _fmemset, _fmemmove, _fmemcmp, _fstricmp, _fstrncmp, stricmp, FP_OFF, FP_SEG, __emit__; flags " + " ".join(SIZES + FOREIGN) + " (-Dfar etc. for gcc/clang only; llrm-c has no -D)"]
    biggest = sorted(rows, key=lambda k: -rows[k]["mir"])[:5]
    unit = "Minstr" if all(rows[k].get("instr") for k in biggest) else "ms"
    lines += ["", f"Largest modules, llrm -O2 steps by own {unit}:", ""]
    for k in biggest:
        steps = sorted(rows[k]["instr" if unit == "Minstr" else "steps"].items(), key=lambda kv: -kv[1])[:6]
        lines.append(f"- {k} ({rows[k]['mir']} MIR instructions, {rows[k]['functions']} functions): " + ", ".join(f"{s} {cost:.0f}" for s, cost in steps))
    return lines + [""]


def report(results: dict, out: Path) -> str:
    lines = []
    for name, data in results.get("axes", {}).items():
        lines += axis_tables(name, data) + pass_table(name, data)
        chart(name, data, out / f"scaling_{name}.png")
        if data.get("failed"):
            lines += ["Stopped early: " + "; ".join(f"{k} at N={v['n']}" for k, v in data["failed"].items()), ""]
    if "programs" in results:
        lines += program_tables(results["programs"])
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="mode", required=True)
    run = sub.add_parser("run")
    run.add_argument("--axis", nargs="*", default=list(AXES))
    run.add_argument("--min", type=int, default=4, help="smallest size, as a power of two")
    run.add_argument("--max", type=int, default=15)
    run.add_argument("--runs", type=int, default=3)
    run.add_argument("--limit", type=float, default=10.0, help="seconds: stop an axis for a compiler once a compile takes this long")
    run.add_argument("--jobs", type=int, default=6, help="axes (and QCport modules) measured at once; instructions:u does not care, task-clock does")
    run.add_argument("--compilers", nargs="*", default=list(COMPILERS), help="the compilers measured; llrm alone is enough to profile its steps")
    run.add_argument("--levels", nargs="*", default=list(LEVELS))
    run.add_argument("--programs", action="store_true", help="QCport as well")
    run.add_argument("--qcport", default=os.environ.get("QCPORT", str(Path.home() / "scratch/qcport/src")))
    run.add_argument("--inc", default=os.environ.get("QCPORT_INC", str(Path.home() / "scratch/qctc/inc")))
    sub.add_parser("report")
    checks = sub.add_parser("check", help="each axis at one size: gcc, clang and llrm must print the same value")
    checks.add_argument("--n", type=int, default=64)
    args = parser.parse_args()
    if args.mode == "check":
        with tempfile.TemporaryDirectory() as scratch:
            for axis in AXES:
                print(axis, args.n, check(axis, args.n, Path(scratch)))
        return
    store = WORK / "scaling.json"
    results = json.loads(store.read_text()) if store.exists() else {}
    if args.mode == "run":
        with tempfile.TemporaryDirectory() as scratch:
            sizes = [2**e for e in range(args.min, args.max + 1)]
            with ThreadPoolExecutor(args.jobs) as pool:
                futures = {axis: pool.submit(run_axis, axis, sizes, args.runs, args.limit * 1e9, Path(tempfile.mkdtemp(dir=scratch)), compilers=args.compilers, levels=args.levels) for axis in args.axis}
                for axis, future in futures.items():
                    results.setdefault("axes", {})[axis] = future.result()
                    store.write_text(json.dumps(results))
            if args.programs:
                results["programs"] = run_programs(Path(args.qcport), Path(args.inc), args.runs, Path(scratch), args.jobs)
                store.write_text(json.dumps(results))
    print(report(results, WORK))


if __name__ == "__main__":
    main()
