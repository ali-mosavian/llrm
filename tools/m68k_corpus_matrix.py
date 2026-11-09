#!/usr/bin/env python3
"""The m68k matrix over the corpus, not the 22 hand-written kernels: code bytes of every program's whole object, three compilers.

m68k_corpus_matrix.py [--cpus 68000,68020,68040,68060] [--levels O1,O2,O3,Os] [--jobs N] --out DIR [NAME...]

Denominator: the tests/run/c programs (those a `// targets:` line leaves for the CPU, as tools/m68k_exec_sweep.py) plus the bench/*/NAME.c
programs; a file in both (same bytes) counts once and is reported. Columns, per program, CPU and level: `ours` (llrm-cc -S, relaxing
branches, gas: as examples/matrix.rs builds it), `gcc` and `clang` (the pin's clang into the pin's llc), both with the shared external flags
(tools/external_flags.py, hash cd1e12d510e4). Bytes are the CODE sections of the object (alignment fill included). No libc is linked, so
there are NO CLOCKS: the rows leave the field empty and tools/m68k_worse_cells.py judges them on bytes alone.

Every cell of the denominator gets a row. A failure is a row with no number and the reason in `note`; a program with no row is a hole and
the run exits 2. Reasons: ok, not-targeted (not a cell), refused (llrm-cc said so), timeout, compile-fail, no-reference (ours built, neither
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
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import benchlist  # noqa: E402
import corpus_targets as ct  # noqa: E402
import external_flags  # noqa: E402
import o0_sweep  # noqa: E402

TIMEOUT = 180
CPUS = ("68000", "68020", "68040", "68060")
LEVELS = ("O1", "O2", "O3", "Os")
POSIX = ROOT / "runtime/shared/posix"
GCC_ONLY = ("-fzero-init-padding-bits",)
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


def code_bytes(listing: str) -> int:
    """The summed size of the sections an `objdump -h` listing flags CODE (`.text`, `.text.startup`, `.text.unlikely`...)."""
    lines = listing.splitlines()
    total = 0
    for at, line in enumerate(lines):
        fields = line.split()
        if len(fields) >= 3 and fields[0].isdigit() and at + 1 < len(lines) and "CODE" in lines[at + 1].replace(",", " ").split():
            total += int(fields[2], 16)
    return total


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

    def build(self, compiler: str, cpu: str, level: str, path: Path, flags: list[str], work: Path) -> tuple[int | None, str, str]:
        """(code bytes, reason, cause) of one program in one column."""
        stem = f"{path.parent.name}_{path.stem}.{cpu}.{level}.{compiler}"
        obj = work / f"{stem}.o"
        if compiler == "ours":
            asm = work / f"{stem}.s"
            env = {**os.environ, "LLRM_CC_TARGET": f"m68k-{cpu}"}
            ok, timed, err = self.run([self.cc, f"-{level}", "-w", "-S", *flags, "-isystem", str(POSIX), str(path), "-o", str(asm)], env)
            if ok:
                asm.write_text(".text\n" + relaxing_branches(asm.read_text()))
                ok, timed, err = self.run([self.as_, "--fatal-warnings", f"-m{cpu}", "-o", str(obj), str(asm)])
        elif compiler == "gcc":
            ok, timed, err = self.run([self.gcc, f"-B{self.root}/usr/lib/gcc-cross/m68k-linux-gnu/15/", f"-B{self.root}/usr/m68k-linux-gnu/bin/", "-ffreestanding", "-nostdlib", "-w", *(["-isystem", self.headers] if self.headers else []), *self.gcc_flags, f"-mcpu={cpu}", f"-{level}", *flags, "-isystem", str(POSIX), "-c", str(path), "-o", str(obj)])
        else:
            ir = work / f"{stem}.ll"
            ok, timed, err = self.run([self.clang, "--target=m68k-linux-gnu", "-ffreestanding", "-fno-pic", "-w", *self.clang_flags, f"-mcpu={cpu}", f"-{level}", *flags, "-isystem", str(POSIX), "-S", "-emit-llvm", str(path), "-o", str(ir)])
            if ok:
                ok, timed, err = self.run([self.llc, "-mtriple=m68k-linux-gnu", "-relocation-model=static", "-O2", "-filetype=obj", f"-mcpu=M{cpu}", str(ir), "-o", str(obj)])
        if not ok:
            reason, cause = failure(compiler, timed, err)
            return None, reason, cause
        return self.size(obj), "ok", ""


def cell_rows(tools: Tools, ident: str, path: Path, cpu: str, level: str, work: Path) -> list[tuple[str, int | None, str, str]]:
    """(compiler, bytes, reason, cause) for the three columns."""
    flags = o0_sweep.flags_of(path.read_text(), f"m68k-{cpu}")
    return [(c, *tools.build(c, cpu, level, path, flags, work)) for c in ("ours", "gcc", "clang")]


def verdict(rows: list[tuple[str, int | None, str, str]]) -> str:
    """The cell's reason: ours' failure, or no-reference when ours built and neither gcc nor clang did, else ok."""
    ours = next(r for r in rows if r[0] == "ours")
    if ours[1] is None:
        return ours[2]
    return "ok" if any(r[1] is not None for r in rows if r[0] != "ours") else "no-reference"


def holes(denominator: list[str], cpus: list[str], levels: list[str], have: set[tuple[str, str, str]]) -> list[tuple[str, str, str]]:
    """The (cpu, level, program) cells of the denominator with no `ours` row: a program silently dropped from the matrix."""
    return [(cpu, level, p) for cpu in cpus for level in levels for p in denominator if (f"m68k-{cpu}", p, level) not in have]


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
    rows, reasons, counts = [], [], {}
    for (ident, _, cpu, level), cols in zip(jobs, results):
        why = verdict(cols)
        counts[why] = counts.get(why, 0) + 1
        if why != "ok":
            reasons.append(f"m68k-{cpu}\t{ident}\t{level}\t{why}\t" + "; ".join(c[3] for c in cols if c[3]))
        for compiler, size, reason, cause in cols:
            note = "" if size is not None else f"n/a: {reason}: {cause}"
            rows.append(f"m68k-{cpu}\t{ident}\t{level}\t{compiler}\t{'' if size is None else size}\t\t\t{note}")
    have = {tuple(r.split("\t")[:3]) for r in rows if r.split("\t")[3] == "ours"}
    skipped = {(f"m68k-{c}", i, lv) for c, i in na for lv in levels}
    missing = holes([i for i, _ in chosen], cpus, levels, have | skipped)
    n_bench = sum(1 for i, _ in corpus if i.startswith("bench/"))
    header = (f"# m68k corpus matrix (code bytes of the whole object, NO CLOCKS: no libc is linked); denominator {len(corpus)} programs = "
              f"{len(corpus) - n_bench} tests/run/c + {n_bench} bench, {len(same)} more in both (counted once, same.tsv); "
              f"cells {len(jobs)} ({len(na)} program-CPU pairs not targeted); " + ", ".join(f"{k} {v}" for k, v in sorted(counts.items()))
              + "; byte_rate: 68000 0.5, 68020 1.33, 68030/040/060 3.2 B per clk saved (unused: no clocks)"
              + "; external flags: tools/external_flags.py --target none, hash cd1e12d510e4\n")
    (out / "m68k-corpus.tsv").write_text(header + "target\tprog\tlevel\tcompiler\tcode_bytes\tclocks\tinstrs\tnote\n" + "\n".join(rows) + "\n")
    (out / "reasons.tsv").write_text("target\tprog\tlevel\treason\tcause\n" + "\n".join(reasons) + "\n" + "".join(f"m68k-{c}\t{i}\t-\tnot-targeted\t\n" for c, i in na))
    (out / "same.tsv").write_text("".join(f"{a}\t{b}\n" for a, b in same))
    print(header.strip())
    if missing:
        print(f"holes (denominator cells with no ours row): {missing[:8]}")
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
