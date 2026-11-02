"""The x86_64-linux rows of the bench: each C program compiled by llrm-cc -m64 and, as reported columns, by gcc and clang, linked with one driver and
the POSIX OS layer, run natively. `code_bytes`: the program's object, executable sections only (tools/sizes.py reads the ELF); `instructions`: the
user-mode instructions of one run of `bench_main_`, the difference of 101 runs and 1 over 100, so the process start-up and the driver cancel."""

import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import sizes  # noqa: E402

sys.path.insert(0, str(ROOT / "tools" / "dosbatch"))
from dosbatch import BIN  # noqa: E402

DRIVER = Path(__file__).with_name("native_driver.c")
POSIX = ROOT / "runtime" / "shared" / "posix"
OURS = "c64"
NOPIC = "c64nopic"  # ours with -fno-pic: the column that compares to gcc and clang, which are -fno-pic; never gated, no base leg
REPORTED = {NOPIC: "ours", "gcc64": "gcc", "clang64": "clang", "pin64": "pin"}  # pin: the pinned LLVM clang (LLRM_PIN_CLANG), the compiler llrm tracks  # columns beside ours, never gated
FAR = re.compile(r"\b(far|huge|__huge|__far)\b")  # a DOS far or huge pointer: the program has no x86_64 reading
DATA = re.compile(r"^//\s*data:", re.M)  # it reads a file by a DOS path the driver does not supply: not run natively


def candidates(directory: Path) -> Path | None:
    """The program's C source if it can run here: it has main, an expected output, no far pointers and reads no data file."""
    source = directory / f"{directory.name}.c"
    if not source.is_file() or not source.with_suffix(".out").is_file():
        return None
    text = source.read_text(errors="replace")
    return None if FAR.search(text) or DATA.search(text) or not re.search(r"\bmain\s*\(", text) else source


def perf_missing() -> str | None:
    """Why user-mode instructions cannot be counted here, or None."""
    if shutil.which("perf") is None:
        return "no perf"
    done = subprocess.run(["perf", "stat", "-x,", "-e", "instructions:u", "true"], capture_output=True, text=True)
    return None if done.returncode == 0 and "instructions" in done.stderr and "not supported" not in done.stderr else "perf cannot count instructions:u here"


def instructions_of(executable: Path, argument: str, cwd: Path, repeats: int = 7) -> int | None:
    """User-mode instructions of `executable argument`: the median of `repeats` runs. An interrupt landing in the run moves the counter either way
    (one binary read 200126 five times of six, 200127 once and 200116 once): the least would take the low outlier."""
    found = []
    for _ in range(repeats):
        done = subprocess.run(["perf", "stat", "-x,", "-e", "instructions:u", str(executable), argument], capture_output=True, text=True, cwd=cwd)
        for line in done.stderr.splitlines():
            fields = line.split(",")
            if len(fields) > 2 and fields[2].startswith("instructions") and fields[0].isdigit():
                found.append(int(fields[0]))
    return sorted(found)[len(found) // 2] if found else None


def compile_command(compiler: str, binary: Path, source: Path, opt: str, out: Path, bench: Path) -> tuple[list[str], dict[str, str]]:
    """The command and environment that compile `source` to the object `out`: the same flags for every compiler; ours is flagless in the gated column (like for like with the base: main 65098fd950 ignores -fno-pic) and `-fno-pic` in its reported column, as gcc and clang are; `main` becomes `bench_main_`, which the driver runs."""
    flags = [f"-{opt}", "-c", "-Dmain=bench_main_", "-I", str(bench), "-I", str(POSIX), "-o", str(out), str(source)]
    if compiler in (OURS, NOPIC):
        return [str(binary / "llrm-cc"), "-m64", *(["-fno-pic"] if compiler == NOPIC else []), *flags], {**os.environ, "LLRM_CC_TARGET": "x86_64-linux"}
    program = os.environ.get("LLRM_PIN_CLANG", "") if compiler == "pin64" else shutil.which(REPORTED[compiler]) or REPORTED[compiler]
    return [program, "-fno-pic", *flags], dict(os.environ)


def compile_to(compiler: str, binary: Path, directory: Path, source: Path, opt: str, out: Path, bench: Path, extra: tuple[str, ...]) -> str | None:
    """Compile `source` to `out`; None, or the first line of why not."""
    command, env = compile_command(compiler, binary, source, opt, out, bench)
    built = subprocess.run([*command[:1], *extra, *command[1:]], capture_output=True, text=True, cwd=directory, env=env, timeout=1800)
    return None if built.returncode == 0 and out.exists() else (built.stderr.strip().splitlines() or ["failed"])[-1][:160]


def measure_one(compiler: str, binary: Path, directory: Path, source: Path, opt: str, work: Path, bench: Path, sized: Path | None = None, extra: tuple[str, ...] = ()) -> dict:
    """One program by one compiler: its row, or {"error": why} (never a number from a build that did not run right). `source` is run and counted, `sized` (default the
    same) is the one whose object is sized, `extra` flags for both: a kernel the compiler can inline into main folds to a constant (crc read 21 instructions on llrm
    and clang, 665 on gcc), so the harness's sources keep it a call (bench.kept_a_call) or else the flag is given."""
    work.mkdir(parents=True, exist_ok=True)
    obj, exe = work / f"{directory.name}.{compiler}.{opt}.o", work / f"{directory.name}.{compiler}.{opt}"
    if why := compile_to(compiler, binary, directory, source, opt, obj, bench, extra):
        return {"error": "compile: " + why}
    if sized is not None and sized != source:
        counted_obj, obj = obj, work / f"{directory.name}.{compiler}.{opt}.sized.o"
        if why := compile_to(compiler, binary, directory, sized, opt, obj, bench, extra):
            return {"error": "compile: " + why}
    else:
        counted_obj = obj
    linked = subprocess.run(["gcc", "-static", "-no-pie", "-o", str(exe), str(counted_obj), str(DRIVER), str(POSIX / "os.c"), "-I", str(POSIX), "-lm"], capture_output=True, text=True)
    if linked.returncode != 0:
        return {"error": "link: " + (linked.stderr.strip().splitlines() or ["failed"])[-1][:160]}
    ran = subprocess.run([str(exe), "31"], capture_output=True, text=True, cwd=directory, timeout=600)
    want = (directory / f"{directory.name}.out").read_text().splitlines()[:1]
    if ran.returncode != 0 or ran.stdout.split()[:1] != want[:1]:
        return {"error": f"prints {ran.stdout.split()[:1]}, want {want[:1]}"}
    few, many = instructions_of(exe, "1", directory), instructions_of(exe, "101", directory)
    if few is None or many is None or many <= few:
        return {"error": f"instructions: {few} for one run, {many} for 101"}
    data, bss = sizes.data_bytes(obj)
    size = sizes.code_size(obj)
    return {"instructions": round((many - few) / 100), "code_bytes": size.code, "align_bytes": size.align, "data_bytes": data, "bss_bytes": bss, **({"align_unread": size.unread} if size.unread else {})}


def measure(selected: list[tuple[str, Path]], binary: Path, opts: list[str], work: Path, bench: Path, keep=None) -> dict[tuple[str, str, str], dict]:
    """Rows keyed (program, column, opt) of the (name, bench directory) pairs in `selected`: ours, and the reported columns."""
    why = perf_missing()
    out = {}
    for name, directory in selected:
        source = candidates(directory)
        if source is None:
            continue
        pair = keep(name, directory, work) if keep else None  # (counted, sized) sources that keep the kernel a call, or None
        run, sized, extra = (*pair, ()) if pair else (source, source, ("-fno-inline-functions",))
        for opt in opts:
            if why:
                rows = {column: {"skip": why} for column in (OURS, *REPORTED)}
            else:
                rows = {column: measure_one(column, binary, directory, run, opt, work, bench, sized, extra) if column != "pin64" or os.environ.get("LLRM_PIN_CLANG") else {"skip": "no LLRM_PIN_CLANG"} for column in ("gcc64", "clang64", "pin64", OURS, NOPIC)}
                broken = rows["gcc64"].get("error", "")
                if broken.startswith(("compile:", "link:")) and rows["clang64"].get("error", "").startswith(("compile:", "link:")):
                    rows = {column: {"skip": f"not buildable natively (gcc and clang fail too): {broken}"} for column in rows}  # parity/scalar calls `keep`, which only the DOS harness defines
            out |= {(name, column, opt): row for column, row in rows.items()}
    return out
