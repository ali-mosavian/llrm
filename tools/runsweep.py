#!/usr/bin/env python3
"""Executed sweep of tests/run/c: every program compiled for TARGET, run, its output compared with the expected one.

    runsweep.py --dist DIR --target x86_32-linux [--levels O1,O2] [--jobs N] [--only a,b]

DIR holds `llrm-cc` and the target's `llrm-llc`. A cell is one program at one level. Each is reported as one of:
ok, WRONG (ran, output differs), TIMEOUT, COMPILE-FAIL, UNLINKED (compiled, the host linker refused it), NA (`corpus_targets.applies`
says the program is not for TARGET), NO-OUT (no expected output file). Only ok and WRONG measure anything: the rest are listed by reason,
a floor on what is unmeasured, never a ceiling on what is right. Exit 1 on WRONG or TIMEOUT.

A runner per target says how objects become a process: `RUNNERS`. Native x86 (gcc -m32 / gcc) and arm64-linux (qemu-aarch64) are here; a target with no runner is refused,
not skipped (m68k qemu, wasm node and DOSBox attach as further entries). `llrm_os` programs link the host-compiled
runtime/shared/posix/os.c; `// stdin: FILE` programs get tests/run/c/FILE on stdin; each program runs in its own empty directory.
"""
from __future__ import annotations

import argparse
import collections
import os
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import corpus_targets  # noqa: E402

CORPUS = ROOT / "tests/run/c"
POSIX = ROOT / "runtime/shared/posix"
REPORT = "#include <stdio.h>\nvoid report(long v) { printf(\"%ld\\n\", v); }\n"
HEADER_LINES = 6


@dataclass(frozen=True)
class Runner:
    """Host tool chain that turns objects into a process: the compiler driver and its flags."""

    driver: list[str]
    host_flags: list[str]
    launcher: tuple[str, ...] = ()  # runs the linked program (an emulator for a foreign target)


RUNNERS = {
    "x86_32-linux": Runner(["gcc", "-m32", "-no-pie"], ["-I/usr/include/x86_64-linux-gnu"]),
    "x86_64-linux": Runner(["gcc", "-no-pie"], []),
    "arm64-linux": Runner(["aarch64-linux-gnu-gcc", "-static"], [], ("qemu-aarch64",)),  # LLRM_ARM64_SYSROOT as for llrm-cc
}
ABI_FLAGS = {"x86_32-linux": ["-mabi=sysv"]}


def header(source: str, key: str) -> str | None:
    for line in source.splitlines()[:HEADER_LINES]:
        if line.startswith(f"// {key}:"):
            return line.removeprefix(f"// {key}:").strip()
    return None


def flags_of(source: str, level: str, family: str) -> list[str]:
    """The `// flags:` of the first variant with its `-O` replaced by LEVEL, plus the family's `// flags-FAMILY:` line."""
    line = header(source, "flags")
    first = line.split("|")[0].split() if line else []
    kept = [token for token in first if not token.startswith("-O") and token not in ("-m32", "-mabi=sysv")]
    return [level, *kept, *corpus_targets.flags_for(family, source)]


def classify(produced: bytes, expected: bytes) -> str:
    """Line ends are compared as LF: the DOS-shaped programs write CRLF, their `.out` files hold LF."""
    return "ok" if produced.replace(b"\r\n", b"\n") == expected.replace(b"\r\n", b"\n") else "WRONG"


def cell(program: Path, level: str, target: str, dist: Path, shim: Path, posix: Path | None) -> tuple[str, str, str]:
    source = program.read_text()
    if not corpus_targets.applies(target, source):
        return program.name, level, "NA"
    expected = corpus_targets.expected_output(program, target)
    if not expected.exists():
        return program.name, level, "NO-OUT"
    runner = RUNNERS[target]
    family = target.split("-")[0]
    links = (header(source, "link") or "").split()
    work = Path(tempfile.mkdtemp(prefix="runsweep-"))
    env = dict(os.environ, LLRM_CC_TARGET=target, LLRM_LLC=str(next(dist.glob("llrm-llc"))))
    objects = []
    for at, one in enumerate([program, *(program.parent / name for name in links)]):
        out = work / f"{at}.o"
        extra = ABI_FLAGS.get(target, [])
        command = [str(dist / "llrm-cc"), *flags_of(source, level, family), *extra, "-w", "-c", "-o", str(out), str(one)]
        command += ["-I", str(ROOT / "bench"), "-I", str(POSIX)]
        if subprocess.run(command, env=env, capture_output=True, timeout=300).returncode:
            return program.name, level, "COMPILE-FAIL"
        objects.append(str(out))
    needs_os = "llrm_os.h" in source
    if needs_os and posix is not None:
        objects.append(str(posix))
    linked = subprocess.run([*runner.driver, *objects, str(shim), "-o", str(work / "x"), "-lm"], capture_output=True)
    if linked.returncode:
        return program.name, level, "UNLINKED"
    given = header(source, "stdin")
    run_dir = work / "run"
    run_dir.mkdir()
    stdin = (program.parent / given).read_bytes() if given else b""
    try:
        done = subprocess.run([*runner.launcher, str(work / "x")], cwd=run_dir, input=stdin, capture_output=True, timeout=10)
    except subprocess.TimeoutExpired:
        return program.name, level, "TIMEOUT"
    return program.name, level, classify(done.stdout, expected.read_bytes())


def host_objects(target: str) -> tuple[Path, Path | None]:
    """The report shim, and the POSIX layer's os.c, compiled for the host side of TARGET."""
    runner = RUNNERS[target]
    work = Path(tempfile.mkdtemp(prefix="runsweep-host-"))
    (work / "report.c").write_text(REPORT)
    shim = work / "report.o"
    subprocess.run([*runner.driver[:-1], "-c", str(work / "report.c"), "-o", str(shim)], check=True)
    posix = work / "os.o"
    built = subprocess.run([*runner.driver[:-1], *runner.host_flags, "-I", str(POSIX), "-c", str(POSIX / "os.c"), "-o", str(posix)], capture_output=True)
    return shim, posix if built.returncode == 0 else None


def sweep(dist: Path, target: str, levels: list[str], jobs: int, only: set[str]) -> list[tuple[str, str, str]]:
    if target not in RUNNERS:
        raise SystemExit(f"runsweep: no runner for {target}; have {sorted(RUNNERS)}")
    shim, posix = host_objects(target)
    programs = [p for p in sorted(CORPUS.glob("*.c")) if not only or p.stem in only]
    with ThreadPoolExecutor(jobs) as pool:
        return list(pool.map(lambda job: cell(*job, target, dist, shim, posix), [(p, level) for p in programs for level in levels]))


def report(results: list[tuple[str, str, str]]) -> int:
    counts = collections.Counter((level, status) for _, level, status in results)
    for key in sorted(counts):
        print(key, counts[key])
    for name, level, status in results:
        if status not in ("ok", "NA"):
            print(f"{status:12} {level} {name}")
    return 1 if any(status in ("WRONG", "TIMEOUT") for _, _, status in results) else 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--dist", required=True, type=Path)
    parser.add_argument("--target", required=True)
    parser.add_argument("--levels", default="O1,O2")
    parser.add_argument("--jobs", type=int, default=8)
    parser.add_argument("--only", default="")
    args = parser.parse_args(argv)
    only = {name for name in args.only.split(",") if name}
    return report(sweep(args.dist, args.target, [f"-{level.lstrip('-')}" for level in args.levels.split(",")], args.jobs, only))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
