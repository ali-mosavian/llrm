#!/usr/bin/env python3
"""The executed differential: generated programs built by llrm-cc and by the pin's clang, run, traces compared.

    tools/difftest/exec.py run [--count N] [--first S] [--shapes a,b] [--level O2] [--target x86_64-linux] [--bin DIR]
    tools/difftest/exec.py replay SEED SHAPE [same options]      one program: both sources, both traces

`gen.emit(seed, shape)` gives a program whose callees print their names. The driver calls `g` over 11 (or 55) argument
values; ours (`llrm-cc -LEVEL`) and the reference (`clang -O0`) must print the same trace. A difference prints the replay
command, then the program cut down by `reduce` to the lines that still differ. A reduced program only counts if clang -O0
and -O2 agree on it: a cut that introduced undefined behaviour differs for the wrong reason.
"""
from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import gen  # noqa: E402

CLANG = os.environ.get("LLRM_PIN_CLANG") or str(Path.home() / "scratch/llvm-build/bin/clang")
ONE = (-3, -1, 0, 1, 2, 3, 4, 5, 7, 9, 100)
TWO = (0, 1, 2, 5, 9)
BITS = {"x86_64-linux": "-m64", "x86_32-linux": "-m32"}
TIMEOUT = 20


def callees(text: str) -> list[str]:
    return sorted(set(re.findall(r"^int (c\d+)\(", text, re.M)))


def driver(text: str) -> str:
    """The driver C: each callee prints its name and returns a value of its arguments, `g` runs over the inputs."""
    names = callees(text)
    returns_int = "\nint g(" in "\n" + text
    defs = "".join(f'int {n}(int a, int b) {{ printf("{n}\\n"); return a * 3 + b; }}\n' for n in names)
    proto = f"{'int' if returns_int else 'void'} g(int, int);\n"
    call = (lambda a, b: f'printf("in %d %d\\n", {a}, {b}); printf("r %d\\n", g({a}, {b}));') if returns_int else (
        lambda a, b: f'printf("in %d %d\\n", {a}, {b}); g({a}, {b});'
    )
    calls = "\n".join(call(a, b) for a in ONE for b in TWO)
    return f"#include <stdio.h>\n{proto}{defs}int main(void)\n{{\n{calls}\nreturn 0;\n}}\n"


def run(command: list[str], env: dict | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(command, capture_output=True, text=True, timeout=TIMEOUT, env={**os.environ, **(env or {})})


class Setup:
    """What the comparison needs: the binaries of ours and the target."""

    def __init__(self, bin_dir: str, level: str, target: str, ours: list[str] | None = None):
        self.level, self.target, self.bits = level, target, BITS[target]
        self.cc = ours or [str(Path(bin_dir) / "llrm-cc")]
        self.env = {
            "LLRM_CC_TARGET": target,
            "LLRM_PIN_CLANG": CLANG,
            "LLRM_LLC": str(Path(bin_dir) / "llrm-llc"),
            **({} if "LLRM_OW" in os.environ else {"LLRM_OW": str(Path.home() / "scratch/ow-src")}),
        }


def trace(setup: Setup, text: str, kind: str, work: Path, level: str | None = None) -> str:
    """The output of `text` built by `kind` (`ours` or `ref`); `FAIL ...` when a step fails."""
    (work / "g.c").write_text(text)
    (work / "drv.c").write_text(driver(text))
    if kind == "ours":
        done = run([*setup.cc, f"-{level or setup.level}", "-c", str(work / "g.c"), "-o", str(work / f"{kind}.o")], setup.env)
    else:
        done = run([CLANG, setup.bits, f"-{level or 'O0'}", "-c", "-fno-pic", "-w", str(work / "g.c"), "-o", str(work / f"{kind}.o")])
    if done.returncode:
        return f"FAIL compile {kind}: {done.stderr.strip()[-200:]}"
    link = run([CLANG, setup.bits, "-no-pie", "-w", str(work / "drv.c"), str(work / f"{kind}.o"), "-o", str(work / kind)])
    if link.returncode:
        return f"FAIL link {kind}: {link.stderr.strip()[-200:]}"
    try:
        out = run([str(work / kind)])
    except subprocess.TimeoutExpired:
        return f"FAIL timeout {kind}"
    return out.stdout if out.returncode == 0 else f"FAIL exit {out.returncode} {kind}"


def verdict(setup: Setup, text: str) -> tuple[str, str, str]:
    """(`match` | `differ` | `broken`, ours, reference): `broken` is a step that failed on either side."""
    work = Path(tempfile.mkdtemp(prefix="difftest-exec-"))
    try:
        ours, ref = trace(setup, text, "ours", work), trace(setup, text, "ref", work)
    finally:
        shutil.rmtree(work, ignore_errors=True)
    if ours.startswith("FAIL") or ref.startswith("FAIL"):
        return "broken", ours, ref
    return ("match" if ours == ref else "differ"), ours, ref


def defined(setup: Setup, text: str) -> bool:
    """Whether clang -O0 and -O2 print the same trace: the reduced program is not one that reads an undefined value."""
    work = Path(tempfile.mkdtemp(prefix="difftest-exec-"))
    try:
        low, high = trace(setup, text, "ref", work, "O0"), trace(setup, text, "ref", work, "O2")
    finally:
        shutil.rmtree(work, ignore_errors=True)
    return not low.startswith("FAIL") and low == high


def reduce(setup: Setup, text: str) -> str:
    """Delete lines (halves, then quarters, ... then single lines) while the program still differs and stays defined."""
    lines = text.splitlines(keepends=True)

    def keeps(candidate: list[str]) -> bool:
        body = "".join(candidate)
        return verdict(setup, body)[0] == "differ" and defined(setup, body)

    size = max(len(lines) // 2, 1)
    while size >= 1:
        at, shrunk = 0, False
        while at < len(lines):
            candidate = lines[:at] + lines[at + size:]
            if candidate and keeps(candidate):
                lines, shrunk = candidate, True
            else:
                at += size
        if not shrunk:
            size //= 2
    return "".join(lines)


def replay_command(args, seed: int, shape: str) -> str:
    return f"python3 tools/difftest/exec.py replay {seed} {shape} --level {args.level} --target {args.target} --bin {args.bin}"


def cases(args) -> list[tuple[int, str]]:
    shapes = args.shapes.split(",")
    return [(seed, shape) for seed in range(args.first, args.first + args.count) for shape in shapes]


def cmd_run(args, setup: Setup) -> int:
    start = time.time()
    todo = cases(args)
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        results = list(pool.map(lambda case: verdict(setup, gen.emit(*case)), todo))
    differ = [case for case, found in zip(todo, results) if found[0] == "differ"]
    broken = [case for case, found in zip(todo, results) if found[0] == "broken"]
    for seed, shape in differ:
        print(f"DIFFER seed={seed} shape={shape}\n  replay: {replay_command(args, seed, shape)}")
        if not args.no_reduce:
            print("  reduced:\n" + "".join("    " + line for line in reduce(setup, gen.emit(seed, shape)).splitlines(keepends=True)))
    for (seed, shape), found in ((case, found) for case, found in zip(todo, results) if found[0] == "broken"):
        print(f"BROKEN seed={seed} shape={shape}: {(found[1] if found[1].startswith('FAIL') else found[2]).splitlines()[0]}")
    print(f"{len(todo)} programs, {len(differ)} differ, {len(broken)} broken, {time.time() - start:.1f} s ({args.target} -{args.level})")
    return 1 if differ or broken else 0


def cmd_replay(args, setup: Setup) -> int:
    text = gen.emit(args.seed, args.shape)
    print(text)
    kind, ours, ref = verdict(setup, text)
    print(f"== {kind}")
    if kind != "match":
        print("-- ours\n" + ours + "-- reference\n" + ref)
    return 0 if kind == "match" else 1


def cmd_run_targets(args) -> int:
    """`run` on each target of `--targets`; the worst result."""
    worst = 0
    for target in args.targets.split(","):
        args.target = target
        worst = max(worst, cmd_run(args, Setup(args.bin, args.level, target, args.ours.split() if args.ours else None)))
    return worst


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("run", "replay"):
        one = sub.add_parser(name)
        one.add_argument("--level", default="O2")
        one.add_argument("--target", choices=sorted(BITS), default="x86_64-linux")
        one.add_argument("--bin", default=str(Path(__file__).resolve().parents[2] / "target/release"))
        one.add_argument("--ours", help="a command in place of llrm-cc (it is given `-LEVEL -c g.c -o g.o`)")
        if name == "run":
            one.add_argument("--count", type=int, default=25, help="seeds per shape")
            one.add_argument("--first", type=int, default=0)
            one.add_argument("--shapes", default=",".join(gen.SHAPES))
            one.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
            one.add_argument("--no-reduce", action="store_true")
            one.add_argument("--targets", default=None, help="comma list, one run each (the batch step: x86_64-linux,x86_32-linux)")
        else:
            one.add_argument("seed", type=int)
            one.add_argument("shape", choices=gen.SHAPES)
    args = ap.parse_args(argv)
    if args.cmd == "run" and args.targets:
        return cmd_run_targets(args)
    setup = Setup(args.bin, args.level, args.target, args.ours.split() if args.ours else None)
    return (cmd_run if args.cmd == "run" else cmd_replay)(args, setup)


if __name__ == "__main__":
    sys.exit(main())
