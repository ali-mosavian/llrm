#!/usr/bin/env python3
"""The m68k matrix over the corpus, not the 22 hand-written kernels: code bytes of every program's whole object, three compilers.

m68k_corpus_matrix.py [--cpus 68000,68020,68040,68060] [--levels O1,O2,O3,Os] [--jobs N] --out DIR [NAME...]

Denominator: the tests/run/c programs (those a `// targets:` line leaves for the CPU, as tools/m68k_exec_sweep.py) plus the bench/*/NAME.c
programs; a file in both (same bytes) counts once and is reported. Columns, per program, CPU and level: `ours` (llrm-cc -c, the shipped
object; its -S text through gas is only the asm-error check), `gcc` and `clang` (the pin's clang into the pin's llc), both with the shared external flags
(tools/external_flags.py, hash cd1e12d510e4). Bytes are the CODE sections of the object (alignment fill included). No libc is linked, so
there are NO CLOCKS: the rows leave the field empty and tools/m68k_worse_cells.py judges them on bytes alone.

Every cell of the denominator gets a row. A failure is a row with no number and the reason in `note`; a program with no row is a hole and
the run exits 2. Reasons: ok, not-targeted (not a cell), refused (llrm-cc said so), timeout, compile-fail, asm-error (gas refused ours: exit 3), no-reference (ours built, neither
gcc nor clang did). OUT gets `m68k-corpus.tsv` (rows), `reasons.tsv` (every cell that is not ok, with the cause) and the header counts.
Environment: LLRM_CC (llrm-cc), GCC_M68K, LLRM_PIN_CLANG, LLVM_M68K (the pin's bin directory).
"""
import argparse
import hashlib
import os
import re
import subprocess
import sys
import tempfile
import threading
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import benchlist  # noqa: E402
import corpus_targets as ct  # noqa: E402
import external_flags  # noqa: E402
import m68k_exec_sweep as sweep  # noqa: E402
import m68k_program_size as program_size  # noqa: E402
import m68k_worse_cells as worse_cells  # noqa: E402
import o0_sweep  # noqa: E402

TIMEOUT = 180
CPUS = ("68000", "68010", "68020", "68030", "68040", "68060")  # test_the_matrix_cpus_are_the_supported_cpus
LEVELS = ("O1", "O2", "O3", "Os")
POSIX = ROOT / "runtime/shared/posix"
GCC_ONLY = ("-fzero-init-padding-bits",)
# gcc's m68k defaults to 68881 hardware floating point (`__HAVE_68881__`) at -mcpu=68020/040/060; ours and the pin's clang
# have no FPU, so the like-for-like gcc column is soft-float (nbody 68020 O2: 578 B hardware, 946 B soft; ours 1294 B)
GCC_FLOAT = ("-msoft-float",)
CONDITIONS = ("ra", "hi", "ls", "cc", "cs", "ne", "eq", "vc", "vs", "pl", "mi", "ge", "lt", "gt", "le", "hs")


def programs(run_dir: Path = ROOT / "tests/run/c", bench: Path = benchlist.BENCH) -> tuple[list[tuple[str, Path]], list[tuple[str, str]]]:
    """(id, path) of every program once, and the (kept id, dropped id) pairs whose files are the same bytes."""
    found = [(f"run/{p.stem}", p) for p in sorted(run_dir.glob("*.c"))]
    found += [(f"bench/{p.parent.name}/{p.stem}", p) for p in benchlist.sources(".c", bench)]
    seen: dict[str, str] = {}
    kept, same = [], []
    for ident, path in found:
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest in seen:
            same.append((seen[digest], ident))
            continue
        seen[digest] = ident
        kept.append((ident, path))
    return kept, same


def sections(listing: str) -> tuple[int, int]:
    """(code, data) bytes of an `objdump -h` listing: the sections flagged CODE (`.text`, `.text.startup`, `.text.unlikely`...), and the
    `.rodata*` and `.data*` ones (a jump table is data: gcc's 48 B against ours 12 B hid in `.rodata`). Debug, notes and `.comment` are in neither."""
    lines = listing.splitlines()
    code = data = 0
    for at, line in enumerate(lines):
        fields = line.split()
        if len(fields) < 3 or not fields[0].isdigit():
            continue
        size = int(fields[2], 16)
        if at + 1 < len(lines) and "CODE" in lines[at + 1].replace(",", " ").split():
            code += size
        elif fields[1].startswith((".rodata", ".data", ".sdata")):
            data += size
    return code, data


def code_bytes(listing: str) -> int:
    """The code plus the read-only and initialised data of an object: the size column."""
    return sum(sections(listing))


def code_without_comments(text: str) -> str:
    return re.sub(r"/\*.*?\*/", "", re.sub(r"//[^\n]*", "", text), flags=re.S)


def long_double_abi_mismatch(source: str) -> bool:
    """gcc's m68k `long double` is the 12-byte extended type (sizeof 12, `__addxf3`); the pin's clang and ours make it a 64-bit double
    (sizeof 8, `__adddf3`) and gcc has no option to match. A program that names the type is not like-for-like against gcc."""
    return bool(re.search(r"\blong\s+double\b", code_without_comments(source)))


def rate_note() -> str:
    """The header's byte-rate text: per CPU, read from the committed timings.times (a typed flat 2 B per clk voided every O2/O3 verdict of the 11-09 table)."""
    return f"byte_rate: {worse_cells.rate_text()} B per clk saved, read from crates/target/llrm-m68k/src/timings.times"


def gcc_program_flags(flags: list[str], source: str = "") -> list[str]:
    """The program's flags as gcc reads them (external_flags.GCC_PROGRAM_FLAGS: `-fno-inline-functions` becomes `-fno-inline`, else gcc
    inlines the static helpers: frameless.c 260 B against our 558 B). ValueError for a flag gcc has no meaning for, or any other
    difference from the program's own flags (external_flags.check_gcc_flags): the cell then has no gcc column."""
    # A source that tests `__clang__` has one branch for clang and another for the rest; all three columns build the clang one
    # (variadic.c: gcc built the macro walk, 178 B at 68000 -Os, against 276 B with the stdarg one ours and clang build).
    extra = (*external_flags.GCC_DIALECT, *(("-D__clang__",) if "__clang__" in source else ()))
    out = [*external_flags.gcc_program_flags(flags), *extra]
    external_flags.check_gcc_flags(flags, out, extra)
    return out


def relaxing_branches(text: str) -> str:
    """Plain `bcc label` lines as gas's relaxing `jcc label` (gas lengthens a plain forward `bcc`; examples/matrix.rs does the same)."""
    out = []
    for line in text.splitlines():
        words = line.split()
        if len(words) == 2 and words[0].startswith("b") and words[0][1:] in CONDITIONS and not words[1].startswith("#"):
            out.append(f"\tj{words[0][1:]}\t{words[1]}")
        else:
            out.append(line)
    return "".join(line + "\n" for line in out)


def failure(compiler: str, timed_out: bool, stderr: str) -> tuple[str, str]:
    """(reason, cause) of a failed compile: a timeout, llrm-cc's own refusal, or any other failure."""
    if timed_out:
        return "timeout", f"{compiler} timeout {TIMEOUT}s"
    lines = [l for l in stderr.splitlines() if l.strip()]
    hits = [l for l in lines if re.search(r"error|refus|panick|Cannot|unsupported", l, re.I)]
    cause = ((hits or lines or ["no output"])[0])[:160]
    return ("refused" if compiler == "ours" and re.search(r"refus", stderr, re.I) else "compile-fail"), f"{compiler}: {cause}"


class Tools:
    def __init__(self) -> None:
        self.root = os.environ["GCC_M68K"]
        self.cc = os.environ["LLRM_CC"]
        self.clang = os.environ["LLRM_PIN_CLANG"]
        self.llc = str(Path(os.environ["LLVM_M68K"]) / "llc")
        self.env = {**os.environ, "LD_LIBRARY_PATH": f"{self.root}/usr/lib/x86_64-linux-gnu"}
        self.gcc = f"{self.root}/usr/bin/m68k-linux-gnu-gcc"
        # The unpacked cross gcc ships no compiler headers (stddef.h): M68K_GCC_HEADERS is a directory of target-neutral ones
        self.headers = os.environ.get("M68K_GCC_HEADERS")
        self.lock = threading.Lock()
        self.as_ = f"{self.root}/usr/bin/m68k-linux-gnu-as"
        self.objdump = f"{self.root}/usr/bin/m68k-linux-gnu-objdump"
        every = external_flags.flags("none", (), 15)
        self.gcc_flags = every
        self.clang_flags = [f for f in every if not f.startswith(GCC_ONLY)]  # clang has no such option

    def run(self, cmd: list[str], env: dict[str, str] | None = None) -> tuple[bool, bool, str]:
        """(ok, timed out, stderr)."""
        try:
            done = subprocess.run(cmd, capture_output=True, text=True, timeout=TIMEOUT, env=env or self.env)
        except subprocess.TimeoutExpired:
            return False, True, ""
        return done.returncode == 0, False, done.stderr or done.stdout

    def size(self, obj: Path) -> int:
        return code_bytes(subprocess.run([self.objdump, "-h", str(obj)], capture_output=True, text=True, env=self.env).stdout)

    def commands(self, compiler: str, cpu: str, level: str, path: str, flags: list[str], out: str) -> list[list[str]]:
        """The command lines of one column, in order: build() runs these and the table header prints them (`path`/`out` are placeholders there)."""
        if compiler == "ours":
            # the object is the shipped one (`-c`); the `-S` text through gas is a validity check only (asm-error), never measured: gas
            # assembles `jsr f` as absolute long, 2 B over the pc-relative call llrm-cc emits (select_of_constants 68020 -Os: 20948 B
            # through gas, 18532 B shipped)
            return [[self.cc, f"-{level}", "-w", "-c", *flags, "-isystem", str(POSIX), path, "-o", out + ".o"],
                    [self.cc, f"-{level}", "-w", "-S", *flags, "-isystem", str(POSIX), path, "-o", out + ".s"],
                    [self.as_, "--fatal-warnings", f"-m{cpu}", "-o", out + ".gas.o", out + ".s"]]
        if compiler == "gcc":
            return [[self.gcc, f"-B{self.root}/usr/lib/gcc-cross/m68k-linux-gnu/15/", f"-B{self.root}/usr/m68k-linux-gnu/bin/", "-ffreestanding", "-nostdlib", "-w", *GCC_FLOAT,
                     *(["-isystem", self.headers] if self.headers else []), *self.gcc_flags, f"-mcpu={cpu}", f"-{level}", *gcc_program_flags(flags, Path(path).read_text(errors="replace") if Path(path).is_file() else ""), "-isystem", str(POSIX), "-c", path, "-o", out + ".o"]]
        return [[self.clang, "--target=m68k-linux-gnu", "-ffreestanding", "-fno-pic", "-w", *self.clang_flags, f"-mcpu={cpu}", f"-{level}", *flags, "-isystem", str(POSIX), "-S", "-emit-llvm", path, "-o", out + ".ll"],
                [self.llc, "-mtriple=m68k-linux-gnu", "-relocation-model=static", "-O2", "-filetype=obj", f"-mcpu=M{cpu}", out + ".ll", "-o", out + ".o"]]

    def stamps(self) -> str:
        """Header text: md5 of every compiler binary a column runs, and the commit the dist was built from (`DIST_COMMIT`, required: a
        table whose ours column came from an unknown build compared a stale compiler once)."""
        commit = os.environ.get("DIST_COMMIT")
        if not commit:
            raise SystemExit("DIST_COMMIT: the commit the dist (LLRM_CC and its llc) was built from")

        def md5(path: str) -> str:
            return hashlib.md5(Path(path).read_bytes()).hexdigest()[:8] if Path(path).is_file() else "missing"

        dist = Path(self.cc).parent
        tools = {"ours llrm-cc": self.cc, "ours llrm-m68k-llc": str(dist / "llrm-m68k-llc"), "ours llrm-llc": str(dist / "llrm-llc"),
                 "gcc": self.gcc, "clang": self.clang, "pin llc": self.llc}
        return f"# stamps: dist commit {commit}; " + ", ".join(f"{name} md5 {md5(path)}" for name, path in tools.items()) + "\n"

    def column_lines(self) -> str:
        """Header text: every column's compile lines, so the next reader sees the setup (the flags hash is in the same header)."""
        lines = "".join(f"# {c}: " + " ; ".join(" ".join(cmd) for cmd in self.commands(c, "<cpu>", "<level>", "<program.c>", ["<program flags>"], "<out>")) + "\n" for c in ("ours", "gcc", "clang"))
        return (lines + "# gcc also gets -D__clang__ for a source that tests it (the branch ours and clang build)\n"
                "# inlining: ours and clang get the program's flags verbatim (`-fno-inline-functions` in clang's frontend, the same for both); gcc gets"
                " `-fno-inline`, its nearest (it still inlines the called-once and small functions under -fno-inline-functions); the three are not"
                " verified to leave the same functions out of line cell by cell\n")

    def library(self, cpu: str, level: str, obj: Path, work: Path) -> tuple[int | None, str]:
        """(bytes of the compiler-rt members the object pulls, unresolved helpers). Only the referenced members link, as in a real link.
        Every column links OUR archive at the same level (libgcc's m68k routines are not in the gcc root, as tools/m68k_program_size.py): the
        gcc and clang library numbers are an estimate. A `__helper` the archive lacks (gcc's `__addxf3` for a 96-bit long double) would read
        as 0 bytes, so it is named and the cell's whole-program size is unknown, not small. Other unresolved symbols (`report`, libc) are ignored."""
        archive = self.support(cpu, level)
        mapfile, exe = Path(str(obj) + ".map"), Path(str(obj) + ".exe")
        link = subprocess.run([f"{self.root}/usr/bin/m68k-linux-gnu-gcc", f"-m{cpu}", "-msoft-float", "-nostdlib", "-static", "-ffreestanding",
                               "-Wl,--unresolved-symbols=ignore-all", f"-Wl,-Map={mapfile}", "-o", str(exe), str(obj), archive],
                              capture_output=True, text=True, env=self.env)
        if link.returncode != 0 or not mapfile.exists():
            return None, "link failed"
        nm = f"{self.root}/usr/bin/m68k-linux-gnu-nm"
        wanted = {w for line in subprocess.run([nm, "-u", str(obj)], capture_output=True, text=True, env=self.env).stdout.splitlines() for w in line.split()[-1:] if w.startswith("__")}
        linked = {line.split()[-1] for line in subprocess.run([nm, "--defined-only", str(exe)], capture_output=True, text=True, env=self.env).stdout.splitlines() if line.split()}
        missing = sorted(wanted - linked)  # the linked exe drops a symbol it could not resolve, so ask the object what it wants
        return sum(program_size.members_of(program_size.text_bytes(mapfile.read_text())).values()), ",".join(missing)

    def support(self, cpu: str, level: str) -> str:
        """The compiler-rt archive of this llrm-cc at CPU and LEVEL (built once, cached by compiler binary)."""
        with self.lock:
            return sweep.build_support(self.cc, self.root, cpu, sweep.cache_dir(self.cc), 4, level, runnable=False)[2]

    def build(self, compiler: str, cpu: str, level: str, path: Path, flags: list[str], work: Path) -> tuple[int | None, str, str, int | None]:
        """(code bytes, reason, cause, library bytes) of one program in one column."""
        if compiler == "gcc":
            try:
                gcc_program_flags(flags, path.read_text(errors="replace"))
            except ValueError as why:
                return None, "no-gcc-column", str(why), None
        if compiler == "gcc" and long_double_abi_mismatch(path.read_text()):
            return None, "abi-mismatch", "gcc long double is 12 B (extended), clang and ours 8 B (double); no gcc option matches", None
        stem = f"{path.parent.name}_{path.stem}.{cpu}.{level}.{compiler}"
        out = str(work / stem)
        env = {**os.environ, "LLRM_CC_TARGET": f"m68k-{cpu}"} if compiler == "ours" else None
        timed, err = False, ""
        ok = True
        for at, cmd in enumerate(self.commands(compiler, cpu, level, str(path), flags, out)):
            ok, timed, err = self.run(cmd, env if compiler == "ours" and at < 2 else None)
            if ok and compiler == "ours" and at == 1:
                Path(out + ".s").write_text(".text\n" + relaxing_branches(Path(out + ".s").read_text()))
            if not ok:
                break
        obj = Path(out + ".o")
        if not ok:
            reason, cause = failure(compiler, timed, err)
            if compiler == "ours" and at == 2 and not timed:
                reason = "asm-error"  # llrm-cc produced assembly gas refuses: a compiler bug, not a missing cell
            return None, reason, cause, None
        library, unresolved = self.library(cpu, level, obj, work)
        if unresolved:
            return self.size(obj), "ok", f"unresolved helpers: {unresolved}", None
        return self.size(obj), "ok", "", library


def cell_rows(tools: Tools, ident: str, path: Path, cpu: str, level: str, work: Path) -> list[tuple[str, int | None, str, str, int | None]]:
    """(compiler, bytes, reason, cause, library bytes) for the three columns."""
    flags = o0_sweep.flags_of(path.read_text(), f"m68k-{cpu}")
    return [(c, *tools.build(c, cpu, level, path, flags, work)) for c in ("ours", "gcc", "clang")]


def verdict(rows: list[tuple]) -> str:
    """The cell's reason: ours' failure, or no-reference when ours built and neither gcc nor clang did, else ok."""
    ours = next(r for r in rows if r[0] == "ours")
    if ours[1] is None:
        return ours[2]
    return "ok" if any(r[1] is not None for r in rows if r[0] != "ours") else "no-reference"


def holes(denominator: list[str], cpus: list[str], levels: list[str], have: set[tuple[str, str, str]]) -> list[tuple[str, str, str]]:
    """The (cpu, level, program) cells of the denominator with no `ours` row: a program silently dropped from the matrix."""
    return [(cpu, level, p) for cpu in cpus for level in levels for p in denominator if (f"m68k-{cpu}", p, level) not in have]


def exit_status(missing: list, counts: dict[str, int]) -> int:
    """2 for a hole in the denominator, 3 for a cell whose assembly gas refused (the build is broken), else 0."""
    if missing:
        return 2
    return 3 if counts.get("asm-error") else 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpus", default=",".join(CPUS))
    parser.add_argument("--levels", default=",".join(LEVELS))
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--out", required=True)
    parser.add_argument("names", nargs="*")
    args = parser.parse_args()
    cpus, levels = args.cpus.split(","), args.levels.split(",")
    unknown = [x for x in cpus if x not in CPUS] + [x for x in levels if x not in LEVELS]
    if unknown:
        raise SystemExit(f"unknown cpu or level: {unknown}")
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    tools = Tools()
    corpus, same = programs()
    chosen = [(i, p) for i, p in corpus if not args.names or i in args.names or p.stem in args.names]
    for cpu in cpus:
        for level in levels:
            tools.support(cpu, level)  # before the pool: one archive build per CPU and level
    jobs, na = [], []
    for cpu in cpus:
        for ident, path in chosen:
            if not ct.applies(f"m68k-{cpu}", path.read_text()):
                na.append((cpu, ident))
                continue
            jobs += [(ident, path, cpu, level) for level in levels]
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        with ThreadPoolExecutor(args.jobs) as pool:
            results = list(pool.map(lambda j: cell_rows(tools, *j, work), jobs))
    rows, reasons, counts, unknown = [], [], {}, 0
    for (ident, _, cpu, level), cols in zip(jobs, results):
        why = verdict(cols)
        counts[why] = counts.get(why, 0) + 1
        if any(c[3].startswith("unresolved helpers") for c in cols):
            unknown += 1
            reasons.append(f"m68k-{cpu}\t{ident}\t{level}\twhole-program-unknown\t" + "; ".join(c[0] + " " + c[3] for c in cols if c[3].startswith("unresolved")))
        if why != "ok":
            reasons.append(f"m68k-{cpu}\t{ident}\t{level}\t{why}\t" + "; ".join(c[3] for c in cols if c[3]))
        for compiler, size, reason, cause, library in cols:
            note = cause if size is not None else f"n/a: {reason}: {cause}"
            rows.append(f"m68k-{cpu}\t{ident}\t{level}\t{compiler}\t{'' if size is None else size}\t\t\t{note}\t{'' if library is None else library}")
    have = {tuple(r.split("\t")[:3]) for r in rows if r.split("\t")[3] == "ours"}
    skipped = {(f"m68k-{c}", i, lv) for c, i in na for lv in levels}
    missing = holes([i for i, _ in chosen], cpus, levels, have | skipped)
    hdir = tools.headers
    headers_note = ""
    if hdir:
        stddef = Path(hdir) / "stddef.h"
        headers_note = f", gcc headers {hdir} (host gcc 15 include dir, stddef.h md5 {hashlib.md5(stddef.read_bytes()).hexdigest()[:12]})"
    n_bench = sum(1 for i, _ in corpus if i.startswith("bench/"))
    header = (f"# m68k corpus matrix (code_bytes = code + rodata + data of the object alone; lib_bytes = the compiler-rt members it links, OUR archive at the same level for all three columns, so the gcc and clang library numbers are an estimate (libgcc's m68k routines are not in the gcc root); whole program = code_bytes + lib_bytes; NO CLOCKS: no libc is linked, so O2/O3 are judged on bytes alone, an upper bound; gcc -msoft-float like ours{headers_note}; denominator {len(corpus)} programs = "
              f"{len(corpus) - n_bench} tests/run/c + {n_bench} bench, {len(same)} more in both (counted once, same.tsv); "
              f"cells {len(jobs)} ({len(na)} program-CPU pairs not targeted); " + ", ".join(f"{k} {v}" for k, v in sorted(counts.items())) + f"; whole-program unknown (a column calls a helper our archive lacks) {unknown}"
              + "; " + rate_note() + " (unused: no clocks)"
              + "; external flags: tools/external_flags.py --target none, hash cd1e12d510e4\n")
    (out / "m68k-corpus.tsv").write_text(header + tools.stamps() + tools.column_lines() + "target\tprog\tlevel\tcompiler\tcode_bytes\tclocks\tinstrs\tnote\tlib_bytes\n" + "\n".join(rows) + "\n")
    (out / "reasons.tsv").write_text("target\tprog\tlevel\treason\tcause\n" + "\n".join(reasons) + "\n" + "".join(f"m68k-{c}\t{i}\t-\tnot-targeted\t\n" for c, i in na))
    (out / "same.tsv").write_text("".join(f"{a}\t{b}\n" for a, b in same))
    print(header.strip())
    if missing:
        print(f"holes (denominator cells with no ours row): {missing[:8]}")
    if counts.get("asm-error"):
        print(f"asm-error: {counts['asm-error']} cells whose assembly gas refuses (see reasons.tsv)")
    return exit_status(missing, counts)


if __name__ == "__main__":
    sys.exit(main())
