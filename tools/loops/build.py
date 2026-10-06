"""
Compiling a program: llrm's three frontends and the reference compilers, one
configuration each. Every compile keeps its MIR stages beside its object.
"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path
from dataclasses import dataclass

ROOT = Path(__file__).resolve().parents[2]
BIN = ROOT / "target" / "release"
HERE = Path(__file__).resolve().parent

OW = Path(os.environ.get("OW_BIN", Path.home() / "work/personal/open-watcom-v2/build/binbuild"))
IA16 = Path(os.environ.get("IA16_ROOT", Path.home() / "work/other/build-ia16"))
LLVM = Path(os.environ.get("LLVM20", "/usr/lib/llvm-20/bin"))

CPUS = ("386", "486", "P5", "Core")


def march(cpu: str) -> str:
    """The `-march` flag that names `cpu` to llrm: the `march` row of code16's timings.times, one column a CPU."""
    rows = {}
    for line in (ROOT / "crates/target/llrm-x86-code16/src/timings.times").read_text().splitlines():
        columns = line.split()
        if columns and columns[0] in ("cpus", "march"):
            rows[columns[0]] = columns[1:]
    return "-march=" + rows["march"][rows["cpus"].index(cpu)]
OPTS = ("-O2", "-O3", "-Os")
EXT = {"c": ".c", "bas": ".bas", "nib": ".nib"}


@dataclass(frozen=True)
class Config:
    cpu: str = "486"
    opt: str = "-O2"
    nib_checked: bool = False
    # Whether the compiler may inline the function under test into its driver.
    # Off, so a loop is measured where the case put it; on, only correctness
    # is judged (the compiled code is the driver's, not the case's).
    inline: bool = False

    @property
    def tag(self) -> str:
        return f"{self.cpu}{self.opt}{'-checked' if self.nib_checked else ''}{'-inline' if self.inline else ''}"


class CompileError(Exception):
    pass


COMPILE_SECONDS = 10


def _run(command: list[str], env: dict | None = None, cwd: Path | None = None, timeout: int = 600) -> str:
    try:
        done = subprocess.run(command, capture_output=True, text=True, env=env, cwd=cwd, timeout=timeout)
    except subprocess.TimeoutExpired:
        raise CompileError(f"{Path(command[0]).name} did not finish in {timeout}s")
    if done.returncode != 0:
        text = (done.stderr or done.stdout).strip()
        # the tool's own lines, not a build's warnings before them
        own = [line for line in text.splitlines() if line.startswith(Path(command[0]).name)]
        raise CompileError(f"{Path(command[0]).name}: {chr(10).join(own) if own else text[:2000]}")
    return done.stdout


def llrm(lang: str, source: Path, obj: Path, config: Config, stages: Path | None = None) -> None:
    """Compile with llrm's rich route; `stages` gets every MIR stage."""
    env = dict(os.environ)
    if stages:
        env["LLRM_MIR_STAGES"] = str(stages)
    # no unrolling or peeling: each case keeps one loop to measure, and compiles faster;
    # no inlining unless the config asks: the function under test stays a call, else the driver's constants fold it
    common = [config.opt, march(config.cpu), "-fno-unroll-loops", "-fno-peel-loops", "-o", str(obj)]
    if not config.inline:
        common.insert(-2, "-fno-inline-functions")
    if lang == "c":
        command = [str(BIN / "llrm-c"), str(source), *common]
    elif lang == "bas":
        command = [str(BIN / "llrm-qb"), str(source), "--dialect", "qb45", "--runtime", "qb45", *common]
    else:
        command = [str(BIN / "llrm-nib"), str(source), "--procedure-segments", *common]
        if not config.nib_checked:
            command.append("--unchecked-bounds")
    _run(command, env, timeout=COMPILE_SECONDS)


def binaries_stamp() -> float:
    """The newest frontend binary's mtime: a stale build is a stale answer."""
    return max((BIN / one).stat().st_mtime for one in ("llrm-c", "llrm-qb", "llrm-nib", "llrm-mir"))


FRONTENDS = ("llrm-c", "llrm-qb", "llrm-nib", "llrm-mir")


def stale_binaries(root: Path = ROOT, bin: Path = BIN) -> list[str]:
    """Why the frontends are not what the sources say, empty if they are: one
    missing, or older than the newest file under crates/. A run with them
    judges the tools and the file against another compiler's answers."""
    newest, where = 0.0, ""
    for base, _, names in os.walk(root / "crates"):
        for name in names:
            at = os.path.join(base, name)
            seen = os.stat(at).st_mtime
            if seen > newest:
                newest, where = seen, at
    problems = []
    for name in FRONTENDS:
        if not (bin / name).exists():
            problems.append(f"{name} is not built: run `cargo build --release --bins`")
        elif (bin / name).stat().st_mtime < newest:
            problems.append(f"{name} is older than {os.path.relpath(where, root)}: run `cargo build --release --bins`")
    return problems


# --- the references ------------------------------------------------------------

OW_CPU = {"386": "-3", "486": "-4", "P5": "-5", "Core": "-6"}
OW_OPT = {"-O2": ["-ox"], "-O3": ["-ox", "-ol+", "-oh"], "-Os": ["-os", "-ol"]}
# gcc-ia16 stops at the 286: no 32-bit registers or addressing.
IA16_ARCH = {"386": "i80286", "486": "i80286", "P5": "i80286", "Core": "i80286"}
IA16_OPT = {"-O2": "-O2", "-O3": "-O3", "-Os": "-Os"}


def watcom(source: Path, obj: Path, config: Config) -> None:
    """Open Watcom, medium model, cdecl as llrm-c calls, no stack checks."""
    _run([str(OW / "bwcc"), "-zq", "-mm", "-ecc", "-s", "-DOWREF", OW_CPU[config.cpu], *OW_OPT[config.opt],
          str(source), f"-fo={obj}"])


def ia16(source: Path, obj: Path, config: Config) -> None:
    """gcc-ia16's cc1 and as (its driver does not build here), medium model."""
    cc1 = IA16 / "build/gcc/cc1"
    asm = obj.with_suffix(".s")
    _run([str(cc1), "-quiet", str(source), "-o", str(asm), IA16_OPT[config.opt], f"-march={IA16_ARCH[config.cpu]}",
          "-mcmodel=medium", "-msegment-relocation-stuff", "-fno-inline", "-w"])
    _run([str(IA16 / "prefix/bin/ia16-elf-as"), str(asm), "-o", str(obj)])


def msp430(source: Path, obj: Path, config: Config) -> None:
    """LLVM 20 for msp430: a 16-bit LSR, a mechanism check only."""
    _run([str(LLVM / "clang"), "--target=msp430", "-c", IA16_OPT[config.opt], "-fno-inline", "-w", str(source),
          "-o", str(obj)])


REFERENCES = {"ow": watcom, "gcc": ia16, "llvm": msp430}


def available() -> dict[str, bool]:
    return {
        "ow": (OW / "bwcc").exists(),
        "gcc": (IA16 / "build/gcc/cc1").exists(),
        "llvm": (LLVM / "clang").exists(),
    }
