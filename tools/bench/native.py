"""The x86_64-linux rows of the bench: each C program compiled by llrm-cc -m64 and, as reported columns, by gcc and clang, linked with one driver and
the POSIX OS layer, run natively. `code_bytes`: the program's object, executable sections only (tools/sizes.py reads the ELF); `instructions`: the
user-mode instructions of one run of `bench_main_`, the difference of 11 runs and 1 over 10, so the process start-up and the driver cancel."""

import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import sizes  # noqa: E402

DRIVER = Path(__file__).with_name("native_driver.c")
POSIX = ROOT / "runtime" / "shared" / "posix"
OURS = "c64"
REPORTED = {"gcc64": "gcc", "clang64": "clang"}  # columns beside ours: their compilers, never gated
FAR = re.compile(r"\bfar\b")  # a DOS far pointer: the program has no x86_64 reading


def candidates(directory: Path) -> Path | None:
    """The program's C source if it can run here: it has main, an expected output and no far pointers."""
    source = directory / f"{directory.name}.c"
    if not source.is_file() or not source.with_suffix(".out").is_file():
        return None
    text = source.read_text(errors="replace")
    return None if FAR.search(text) or not re.search(r"\bmain\s*\(", text) else source


def perf_missing() -> str | None:
    """Why user-mode instructions cannot be counted here, or None."""
    if shutil.which("perf") is None:
        return "no perf"
    done = subprocess.run(["perf", "stat", "-x,", "-e", "instructions:u", "true"], capture_output=True, text=True)
    return None if done.returncode == 0 and "instructions" in done.stderr and "not supported" not in done.stderr else "perf cannot count instructions:u here"


def instructions_of(executable: Path, argument: str, cwd: Path) -> int | None:
    done = subprocess.run(["perf", "stat", "-x,", "-e", "instructions:u", str(executable), argument], capture_output=True, text=True, cwd=cwd)
    for line in done.stderr.splitlines():
        fields = line.split(",")
        if len(fields) > 2 and fields[2].startswith("instructions") and fields[0].isdigit():
            return int(fields[0])
    return None


def compile_command(compiler: str, binary: Path, source: Path, opt: str, out: Path, bench: Path) -> tuple[list[str], dict[str, str]]:
    """The command and environment that compile `source` to the object `out`: the same flags for every compiler; `main` becomes `bench_main_`, which the driver runs."""
    flags = [f"-{opt}", "-c", "-Dmain=bench_main_", "-I", str(bench), "-I", str(POSIX), "-o", str(out), str(source)]
    if compiler == OURS:
        return [str(binary / "llrm-cc"), "-m64", *flags], {**os.environ, "LLRM_CC_TARGET": "x86_64-linux"}
    return [shutil.which(REPORTED[compiler]) or REPORTED[compiler], "-fno-pic", *flags], dict(os.environ)


def measure_one(compiler: str, binary: Path, directory: Path, source: Path, opt: str, work: Path, bench: Path) -> dict:
    """One program by one compiler: its row, or {"error": why} (never a number from a build that did not run right)."""
    work.mkdir(parents=True, exist_ok=True)
    obj, exe = work / f"{directory.name}.{compiler}.{opt}.o", work / f"{directory.name}.{compiler}.{opt}"
    command, env = compile_command(compiler, binary, source, opt, obj, bench)
    built = subprocess.run(command, capture_output=True, text=True, cwd=directory, env=env, timeout=1800)
    if built.returncode != 0 or not obj.exists():
        return {"error": "compile: " + (built.stderr.strip().splitlines() or ["failed"])[-1][:160]}
    linked = subprocess.run(["gcc", "-static", "-no-pie", "-o", str(exe), str(obj), str(DRIVER), str(POSIX / "os.c"), "-I", str(POSIX), "-lm"], capture_output=True, text=True)
    if linked.returncode != 0:
        return {"error": "link: " + (linked.stderr.strip().splitlines() or ["failed"])[-1][:160]}
    ran = subprocess.run([str(exe), "31"], capture_output=True, text=True, cwd=directory, timeout=600)
    want = source.with_suffix(".out").read_text().splitlines()[:1]
    if ran.returncode != 0 or ran.stdout.split()[:1] != want[:1]:
        return {"error": f"prints {ran.stdout.split()[:1]}, want {want[:1]}"}
    few, many = instructions_of(exe, "1", directory), instructions_of(exe, "11", directory)
    if few is None or many is None or many <= few:
        return {"error": f"instructions: {few} for one run, {many} for eleven"}
    data, bss = sizes.data_bytes(obj)
    return {"instructions": (many - few) // 10, "code_bytes": sizes.code_bytes(obj), "data_bytes": data, "bss_bytes": bss}


def measure(selected: list[tuple[str, Path]], binary: Path, opts: list[str], work: Path, bench: Path) -> dict[tuple[str, str, str], dict]:
    """Rows keyed (program, column, opt) of the (name, bench directory) pairs in `selected`: ours, and the reported columns."""
    why = perf_missing()
    out = {}
    for name, directory in selected:
        source = candidates(directory)
        if source is None:
            continue
        for opt in opts:
            for column in (OURS, *REPORTED):
                out[(name, column, opt)] = {"skip": why} if why else measure_one(column, binary, directory, source, opt, work, bench)
    return out
