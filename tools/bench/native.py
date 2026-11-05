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
LONG32 = re.compile(r"\btypedef\s+(?:unsigned\s+)?long\s+\w*32\b")  # a 32-bit type spelled `long`: 64 bits here, so the program computes another result (sfm16_* printed 620745305, want 1984619853, in gcc, clang and ours alike)
DATA = re.compile(r"^//\s*data:", re.M)  # it reads a file by a DOS path the driver does not supply: not run natively


NO_NATIVE_LONG = "no native column (long is 64-bit)"


def spells_int32_as_long(directory: Path) -> bool:
    """The program has a source and an expected output but defines a 32-bit type as `long`: its native rows are a skip row with the reason, not a missing one."""
    source = directory / f"{directory.name}.c"
    return source.is_file() and source.with_suffix(".out").is_file() and bool(LONG32.search(source.read_text(errors="replace")))


def candidates(directory: Path) -> Path | None:
    """The program's C source if it can run here: it has main, an expected output, no far pointers and reads no data file."""
    source = directory / f"{directory.name}.c"
    if not source.is_file() or not source.with_suffix(".out").is_file():
        return None
    text = source.read_text(errors="replace")
    return None if FAR.search(text) or LONG32.search(text) or DATA.search(text) or not re.search(r"\bmain\s*\(", text) else source


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


WINDOW = 100  # extra runs between the two counts; widened (WIDE) while a window of them stays this cheap
WIDE, CHEAP = 1000, 10_000_000


def instructions_per_run(exe: Path, cwd: Path) -> int | str:
    """User-mode instructions of one run of the kernel: (count of 1 + N runs - count of 1 run) / N, rounded; or why not. A run costs a fixed
    start-up plus the kernel, and the first run is cold, so the difference per run is not an integer (crc read 482.4): the counter's few-instruction
    jitter over 100 runs (0.1) moved it across .5 and the row read 482 or 483. A window of 1000 runs cuts the jitter to 0.01 where 1000 runs stay cheap."""
    few, many = instructions_of(exe, "1", cwd), instructions_of(exe, str(1 + WINDOW), cwd)
    if few is None or many is None or many <= few:
        return f"instructions: {few} for one run, {many} for {1 + WINDOW}"
    window = WINDOW
    if many - few < CHEAP:
        window = WIDE
        many = instructions_of(exe, str(1 + window), cwd)
        if many is None or many <= few:
            return f"instructions: {few} for one run, {many} for {1 + window}"
    return round((many - few) / window)


def pic_mode(obj: Path) -> str:
    """"nopic", "pic" or "unknown" from the object's references to data (a data symbol or a .data/.bss/.rodata section). An absolute 32-bit one (R_X86_64_32, 32S)
    is no-pic and wins: position-independent code never writes one, while non-pic code still reaches some data pc-relative (gcc -fno-pic read `mixed` on soft-float
    code with a .rodata immediate beside a .bss load). Else a pc-relative or GOT one is pic; none says nothing. Calls and function addresses do not count. The flag a
    compiler was given is not the mode it wrote (main ignored -fno-pic)."""
    found = subprocess.run(["readelf", "-rsW", str(obj)], capture_output=True, text=True).stdout.splitlines()
    data = {fields[7] for line in found if len(fields := line.split()) >= 8 and fields[0].endswith(":") and fields[3] == "OBJECT"}
    modes = set()
    for line in found:
        fields = line.split()
        if len(fields) < 5 or not re.fullmatch(r"[0-9a-f]{8,16}", fields[0]) or not fields[2].startswith("R_X86_64_"):
            continue
        if not (fields[4] in data or fields[4].startswith((".data", ".bss", ".rodata"))):
            continue
        if fields[2] in ("R_X86_64_32", "R_X86_64_32S"):
            modes.add("nopic")
        elif fields[2] in ("R_X86_64_PC32", "R_X86_64_GOTPCREL", "R_X86_64_GOTPCRELX", "R_X86_64_REX_GOTPCRELX"):
            modes.add("pic")
    return "nopic" if "nopic" in modes else "pic" if modes else "unknown"


def mismatch(row: dict, column: str, obj: Path, ours: Path) -> dict:
    """`row`, annotated "pc-rel vs abs data refs" where the reported column's object reaches data the other way than ours (c64nopic): a pc-relative table beside an
    absolute one differs by a `lea` per access (hanoi2 -O2 read 10.6% behind clang with the pic one against the nopic one, 16% ahead like for like). A note, not an
    error: parity/*/clang64 Os is pc-relative where ours writes `mov $sym`, as -fno-pic clang does by flag and the pin does by shape."""
    if column not in REPORTED or column == NOPIC or "error" in row or "skip" in row or not obj.exists() or not ours.exists():
        return row
    theirs, mine = pic_mode(obj), pic_mode(ours)
    return row if "unknown" in (theirs, mine) or theirs == mine else {**row, "note": f"pc-rel vs abs data refs: {column} is {theirs}, ours is {mine}"}


def checked_object(work: Path, program: str, column: str, opt: str) -> Path:
    """The object whose position-independence is compared: the sized one where the harness wrapped the kernel (the program's own, `measure_one`), else the counted one. The counted wrapper's global pointer is a pc-relative data reference in every column, so gcc and clang read `pic` from it while ours, with the program's table beside it, read `mixed` (hanoi O2)."""
    sized = work / f"{program}.{column}.{opt}.sized.o"
    return sized if sized.exists() else work / f"{program}.{column}.{opt}.o"


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
    per_run = instructions_per_run(exe, directory)
    if isinstance(per_run, str):
        return {"error": per_run}
    data, bss = sizes.data_bytes(obj)
    size = sizes.code_size(obj)
    return {"instructions": per_run, "code_bytes": size.code, "align_bytes": size.align, "data_bytes": data, "bss_bytes": bss, **({"align_unread": size.unread} if size.unread else {})}


def measure(selected: list[tuple[str, Path]], binary: Path, opts: list[str], work: Path, bench: Path, keep=None) -> dict[tuple[str, str, str], dict]:
    """Rows keyed (program, column, opt) of the (name, bench directory) pairs in `selected`: ours, and the reported columns."""
    why = perf_missing()
    out = {}
    for name, directory in selected:
        source = candidates(directory)
        if source is None:
            if spells_int32_as_long(directory):
                out |= {(name, column, opt): {"skip": NO_NATIVE_LONG} for opt in opts for column in ("gcc64", "clang64", "pin64", OURS, NOPIC)}
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
            rows = {column: mismatch(row, column, checked_object(work, directory.name, column, opt), checked_object(work, directory.name, NOPIC, opt)) for column, row in rows.items()}
            out |= {(name, column, opt): row for column, row in rows.items()}
    return out
