#!/usr/bin/env python3
"""The tests/run/c corpus compiled, linked and run on m68k under qemu-user: the executed sweep that o0_sweep (compile only) is not.

m68k_exec_sweep.py [--cpu 68020|68000] [--levels O1,O2,Os] [--jobs N] [--tsv FILE] [NAME...]

Per program the target applies to and per level: llrm-cc -c, link with the stubs in tools/m68k_exec (crt + report, memcpy and friends, the
llrm_os layer over raw syscalls) and the compiler-rt archive built by llrm-cc, run under `qemu-m68k -cpu m<CPU>`. One verdict per cell:
  ok        exit 0 and stdout equals the reference (`NAME.out`, or `NAME.m68k.out`)
  noref     exit 0 but the program has no reference: it ran, nothing was compared. Never counted as ok.
  output    exit 0, stdout differs        run   signal, timeout or exit != 0        link   undefined or relocation error
  compile   llrm-cc failed        input  the program's `// data:` input is not available (--bench; UNMEASURED, not a failure of the compiler)
A program `// targets:` excludes from m68k is n/a: not a cell. Flags are the first `// flags:` configuration's (o0_sweep.flags_of: x87-only
options such as -mlong-double-80 are dropped) plus `// flags-m68k:`.
Exit 1 unless every cell is ok or noref. Environment: LLRM_BIN (llrm-cc), GCC_M68K (unpacked m68k binutils+gcc), LLRM_PIN_CLANG,
M68K_EXEC_CACHE (stubs and compiler-rt archive, default ~/.cache/llrm-m68k-exec; one directory per compiler binary).
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
import corpus_targets as ct  # noqa: E402
import llrmbin  # noqa: E402
import m68k_runtime_guard as guard  # noqa: E402
import o0_sweep  # noqa: E402
import benchlist  # noqa: E402

TIMEOUT = 180
RUN_TIMEOUT = 30
STUBS = HERE / "m68k_exec"
BUILD_FLAGS = "-include config/m68k/config.h"  # the archive's compile flags beyond the level, part of its cache key
RUNTIME = ROOT / "runtime/compiler-rt"


def verdict(returncode: int, stdout: bytes, want: bytes | None) -> tuple[str, str]:
    """A run's verdict. A program with no reference output ran and was not compared: `noref`, never `ok`."""
    if returncode != 0:
        return "run", f"exit {returncode}" + (" (signal)" if returncode < 0 else "")
    if want is None:
        return "noref", "no reference output"
    # a CRLF console write is a line end; nothing else is forgiven (a missing final newline or an extra blank is a difference)
    if stdout.replace(b"\r\n", b"\n") != want:
        return "output", "stdout differs from the reference"
    return "ok", ""


def inputs_of(source: Path) -> tuple[dict[str, bytes], bytes | None]:
    """What a program opens, from its `// data:` line: the files to put in its working directory (named as the program opens them, upper case) and
    the bytes for its standard input (`@dickens`, read by `input_read`). `@signal` is generated (tools/dosbatch/signal_gen), `@dickens` comes from
    the pinned corpus cache (dosbatch/corpus: SHA-256 checked, never another file in its place; raises corpus.Unavailable), a path is relative to the source."""
    found = re.search(r"^//\s*data:\s*(\S+)", source.read_text(), re.M)
    if not found:
        return {}, None
    what = found[1]
    sys.path.insert(0, str(HERE / "dosbatch"))
    import corpus  # noqa: E402
    if what == "@signal":
        return {"SIGNAL": corpus.path("signal").read_bytes()}, None
    if what == "@dickens":
        return {}, corpus.path("dickens").read_bytes()
    target = (source.parent / what).resolve()
    return {target.name.upper(): target.read_bytes()}, None


def tool_env(root: str) -> dict[str, str]:
    return {**os.environ, "LD_LIBRARY_PATH": f"{root}/usr/lib/x86_64-linux-gnu"}


def first_error(text: str) -> str:
    lines = [l for l in text.splitlines() if l.strip()]
    hits = [l for l in lines if re.search(r"error|llrm-|refus|undefined|truncated|does not fit|panick|Cannot", l, re.I)]
    return ((hits or lines or ["no output"])[0])[:200]


def cache_dir(cc: str) -> Path:
    """One directory per compiler binary and runtime source: the archive's objects are that compiler's output for those sources and flags."""
    digest = hashlib.md5()
    for name in ("llrm-cc", "llrm-llc", "llrm-m68k-llc"):
        path = Path(cc).parent / name
        if path.exists():
            digest.update(path.read_bytes())
    for source in sorted([*RUNTIME.glob("lib/**/*.[ch]"), *RUNTIME.glob("lib/**/*.inc"), *RUNTIME.glob("include/*.h"), *RUNTIME.glob("config/m68k/*.h"), RUNTIME / "config/m68k/mulsi3.s"]):
        digest.update(source.read_bytes())
    digest.update(BUILD_FLAGS.encode())
    base = Path(os.environ.get("M68K_EXEC_CACHE") or Path.home() / ".cache/llrm-m68k-exec")
    return base / digest.hexdigest()[:12]


def build_support(cc: str, root: str, cpu: str, cache: Path, jobs: int, level: str = "O2", runnable: bool = True) -> list[str]:
    """The stubs and the compiler-rt archive for CPU, built at LEVEL once per compiler binary: the link arguments after the program's object.

    `runnable`: the archive will be executed, so a routine that reaches itself through a libcall (int32.c at -O1: `udivmod` and `__udivsi3`)
    is refused (m68k_runtime_guard); a size-only build passes False."""
    if runnable and (loops := guard.runtime_cycles(cc, cpu, level)):
        raise SystemExit(f"m68k_exec_sweep: the runtime built at -{level} calls itself ({'; '.join(' -> '.join(c) for c in loops)}): build it at -O2 or -Os")
    cache.mkdir(parents=True, exist_ok=True)
    env = tool_env(root)
    stubs = []
    for name in ("crt", "mem", "os"):
        obj = cache / f"{name}_{cpu}.o"
        if not obj.exists():
            built = subprocess.run(
                [f"{root}/usr/bin/m68k-linux-gnu-gcc", f"-m{cpu}", "-msoft-float", "-O1", "-ffreestanding", "-fno-builtin", "-fno-tree-loop-distribute-patterns",
                 "-w", "-c", "-o", str(obj), str(STUBS / f"{name}.c")],
                capture_output=True, text=True, env=env,
            )
            if built.returncode != 0:
                raise SystemExit(f"m68k_exec_sweep: stub {name} does not build: {built.stderr}")
        stubs.append(str(obj))
    archive = cache / f"compiler-rt_{cpu}_{level}.a"
    if not archive.exists():
        sources = sorted([*RUNTIME.glob("lib/*/*.c"), *RUNTIME.glob("lib/fp/*/*.c")])
        cenv = {**env, **llrmbin.cc_environment(), "LLRM_CC_TARGET": f"m68k-{cpu}"}

        def compile_one(src: Path) -> str | None:
            obj = cache / f"rt_{cpu}_{level}_{src.parent.name}_{src.stem}.o"
            run = subprocess.run(
                [cc, f"-{level}", "-w", "-c", "-o", str(obj), "-I", str(RUNTIME / "include"), "-I", str(RUNTIME / "config/m68k"), "-include", str(RUNTIME / "config/m68k/config.h"), str(src)],
                capture_output=True, text=True, env=cenv, timeout=TIMEOUT,
            )
            return str(obj) if run.returncode == 0 else None

        with ThreadPoolExecutor(jobs) as pool:
            objects = [o for o in pool.map(compile_one, sources) if o]
        # __mulsi3 is hand-written (config/m68k/mulsi3.s; SF_MULSI3_EXTERNAL keeps int32.c's out)
        mulsi3 = cache / f"rt_{cpu}_mulsi3.o"
        defsym = ["--defsym", "M68020=1"] if cpu in ("68020", "68030", "68040", "68060") else []
        subprocess.run([f"{root}/usr/bin/m68k-linux-gnu-as", f"-m{cpu}", *defsym, "-o", str(mulsi3), str(RUNTIME / "config/m68k/mulsi3.s")], check=True, env=env)
        objects.append(str(mulsi3))
        tmp = archive.with_suffix(".tmp")
        tmp.unlink(missing_ok=True)
        subprocess.run([f"{root}/usr/bin/m68k-linux-gnu-ar", "rcs", str(tmp), *objects], check=True, env=env)
        tmp.rename(archive)
    crt, mem, osl = stubs
    return [crt, "-Wl,--start-group", str(archive), mem, osl, "-Wl,--end-group"]


def cell(cc: str, root: str, qemu: str, cpu: str, level: str, path: Path, work: Path, support: list[str]) -> tuple[str, str]:
    target = f"m68k-{cpu}"
    name = f"{path.stem}.{level}"
    obj, exe = work / f"{name}.o", work / f"{name}.exe"
    cmd = [cc, f"-{level}", "-w", "-c", *o0_sweep.flags_of(path.read_text(), target), "-I", str(ROOT / "runtime/shared/posix"), "-o", str(obj), str(path)]
    env = {**os.environ, **llrmbin.cc_environment(), "LLRM_CC_TARGET": target}
    try:
        run = subprocess.run(cmd, capture_output=True, text=True, timeout=TIMEOUT, env=env)
    except subprocess.TimeoutExpired:
        return "compile", f"timeout {TIMEOUT}s"
    if run.returncode != 0 or not obj.exists() or obj.stat().st_size == 0:
        return "compile", first_error(run.stderr or run.stdout)
    link = subprocess.run(
        [f"{root}/usr/bin/m68k-linux-gnu-gcc", f"-m{cpu}", "-msoft-float", "-nostdlib", "-static", "-ffreestanding", "-o", str(exe), str(obj), *support],
        capture_output=True, text=True, env=tool_env(root),
    )
    obj.unlink(missing_ok=True)
    if link.returncode != 0:
        return "link", first_error(link.stderr)
    try:
        files, stdin = inputs_of(path)
    except Exception as error:  # a corpus that is missing or fails its hash: the cell has no input, so it is not run
        exe.unlink(missing_ok=True)
        return "input", f"{type(error).__name__}: {error}"[:200]
    cwd = work / f"{name}.{cpu}.dir"
    cwd.mkdir(exist_ok=True)
    for file, data in files.items():
        (cwd / file).write_bytes(data)
    try:
        ran = subprocess.run([qemu, "-cpu", f"m{cpu}", str(exe)], capture_output=True, timeout=RUN_TIMEOUT, cwd=cwd, input=stdin if stdin is not None else b"")
    except subprocess.TimeoutExpired:
        return "run", f"timeout {RUN_TIMEOUT}s"
    finally:
        exe.unlink(missing_ok=True)
    reference = ct.expected_output(path, target)
    return verdict(ran.returncode, ran.stdout, reference.read_bytes() if reference.exists() else None)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpu", default="68020")
    parser.add_argument("--levels", default="O1,O2,Os")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "4")))
    parser.add_argument("--bench", action="store_true", help="also the bench/*/NAME.c programs, with the input their `// data:` line names")
    parser.add_argument("--tsv")
    parser.add_argument("names", nargs="*")
    args = parser.parse_args()
    root = os.environ.get("GCC_M68K")
    qemu = "/usr/bin/qemu-m68k"
    if not root or not Path(qemu).exists():
        print("m68k_exec_sweep: GCC_M68K or qemu-m68k is missing", file=sys.stderr)
        return 2
    cc = o0_sweep.compiler()
    target = f"m68k-{args.cpu}"
    sources = sorted((ROOT / "tests/run/c").glob("*.c")) + (benchlist.sources(".c") if args.bench else [])
    chosen = [p for p in sources if not args.names or p.stem in args.names]
    programs = [p for p in chosen if ct.applies(target, p.read_text(), p)]
    support = build_support(cc, root, args.cpu, cache_dir(cc), args.jobs)
    levels = args.levels.split(",")
    jobs = [(p, level) for p in programs for level in levels]
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        with ThreadPoolExecutor(args.jobs) as pool:
            results = list(pool.map(lambda j: cell(cc, root, qemu, args.cpu, j[1], j[0], work, support), jobs))
    rows = [(p.stem, level, *r) for (p, level), r in zip(jobs, results)]
    counts: dict[str, int] = {}
    for row in rows:
        counts[row[2]] = counts.get(row[2], 0) + 1
    for name, level, status, cause in sorted(rows, key=lambda r: r[2] == "noref"):
        if status != "ok":
            print(f"{target} {name} {level}\t{status}\t{cause}")
    print(f"{target}: {len(programs)} programs x {len(levels)} levels = {len(rows)} cells ({len(chosen) - len(programs)} programs n/a): " + ", ".join(f"{k} {v}" for k, v in sorted(counts.items())))
    if args.tsv:
        Path(args.tsv).write_text("".join(f"{target}\t{n}\t{l}\t{s}\t{c}\n" for n, l, s, c in rows))
    return 0 if counts.get("ok", 0) + counts.get("noref", 0) == len(rows) else 1


if __name__ == "__main__":
    sys.exit(main())
