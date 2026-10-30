#!/usr/bin/env python3
"""Two commits' llrm-cc (and the llrm-llc it runs), built the way CI builds them and compared on deterministic counts: the one way to quote a compile-cost delta.

    tools/abmeasure.py PARENT HEAD [--profile dist|release] [--levels O1 O2 Os] [--programs d_faces lru bintree] [--cycles N] [--cache]

Each commit gets a worktree and a target directory of its own under $LLRM_AB_DIR (default ~/.cache/llrm/ab), built with
`CARGO_INCREMENTAL=0 env -u RUSTFLAGS cargo build --profile dist --bin llrm-cc --bin llrm-llc`, the compiler the cost gate measures (tools/measure.py): `release`
re-partitions its 16 codegen units when one file changes and flips the inlining of unrelated functions (the same PR read +0.46..0.61% on the gate's
shapes in release, +0.01..0.04% in dist), and its `incremental = true` makes a 6% heavier llrm-c with the opposite sign (#2077: +0.24%
incremental, -0.58% CI mode). `--profile release` measures the old way. A CHECK or EXP switch in the environment that `--allow-env` does not name is refused (tools/envguard.py): both sides run in one environment.
Prints per program and level `perf stat` instructions:u of both, and with --cycles N the min and
median of cycles:u over N alternated runs (a loaded host's cycles are noise: instructions are the claim), with --cache callgrind's
Ir, D1 and last-level misses. d_faces needs QCPORT (QCport's src) and is left out without it.
"""

from __future__ import annotations

import os
import re
import sys
import shutil
import argparse
import statistics
import subprocess
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import corpora  # noqa: E402
import envguard  # noqa: E402
import llrmbin  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
CACHE = Path(os.environ.get("LLRM_AB_DIR") or Path.home() / ".cache/llrm/ab")
KEYWORDS = "far huge near pascal cdecl __far _far __near _near"


def build_env(target: Path) -> dict[str, str]:
    """The build environment of the gate (tools/measure.py MEASURE_BUILDS): no RUSTFLAGS, no incremental."""
    env = {k: v for k, v in os.environ.items() if k != "RUSTFLAGS"}
    return {**env, "CARGO_INCREMENTAL": "0", "CARGO_TARGET_DIR": str(target)}


def build(sha: str, root: Path = ROOT, cache: Path = CACHE, profile: str = "dist") -> Path:
    """The `profile` directory of `sha` built in CI mode (reused when the commit was built before)."""
    full = subprocess.run(["git", "rev-parse", sha], cwd=root, capture_output=True, text=True, check=True).stdout.strip()
    tree, target = cache / f"tree-{full[:12]}", cache / f"target-{profile}-{full[:12]}"
    if not tree.exists():
        cache.mkdir(parents=True, exist_ok=True)
        subprocess.run(["git", "worktree", "add", "-q", "--detach", str(tree), full], cwd=root, check=True)
    if not (target / profile / "llrm-cc").is_file():
        flags = ["--profile", "dist"] if profile == "dist" else ["--release"]
        done = subprocess.run(["cargo", "build", *flags, "-q", "--bin", "llrm-cc", "--bin", "llrm-llc"], cwd=tree, env=build_env(target))
        if done.returncode:
            raise SystemExit(f"abmeasure: {full[:12]} does not build")
    return target / profile


def programs(root: Path, names: list[str], qcport: str | None, work: Path) -> dict[str, tuple[Path, list[str]]]:
    """name -> (source, arguments before the level): d_faces wrapped as vsgcc does, bench programs as bench.py compiles them."""
    out = {}
    for name in names:
        if name == "d_faces":
            if not qcport:
                print("abmeasure: d_faces left out: QCPORT is not set", file=sys.stderr)
                continue
            src = Path(qcport)
            wrapped = work / "d_faces_flat.c"
            wrapped.write_text("".join(f"#define {k}\n" for k in KEYWORDS.split()) + f'#include "{src}/render/d_faces.c"\n')
            out[name] = (wrapped, [*llrmbin.c_args(32), "-mabi=sysv", *(f for d in ("", "render", "game", "host", "qgl") for f in ("-I", str(src / d)))])
        else:
            out[name] = (root / "bench" / name / f"{name}.c", [*llrmbin.c_args(32), "-mabi=sysv", "-I", str(root / "bench"), "-I", str(root / "bench" / name)])
    return out


# A compile that fails (QCPORT without the game headers: "Unable to open 'world.h'") stops in ~8 M instructions where d_faces takes ~2 G, and perf stat
# counts it all the same: fold-poc read 8.1 M as a result. A row is a compile that exited 0 and cost at least this much.
FLOOR = {"d_faces": 500_000_000}


def checked(done: subprocess.CompletedProcess, source: Path) -> None:
    """SystemExit with the compiler's first message when the compile failed."""
    if done.returncode:
        lines = (done.stdout + done.stderr).splitlines()
        key = [line for line in lines if "Error!" in line or "error:" in line or "No such file" in line]
        first = " | ".join(key[:2] or lines[:2]) or "no message"
        raise SystemExit(f"abmeasure: the compile of {source.name} failed (exit {done.returncode}): {first}")


def floored(name: str, count: int) -> None:
    if count < FLOOR.get(name, 0):
        raise SystemExit(f"abmeasure: {name} counted {count:,} instructions, under its floor {FLOOR[name]:,}: it did not compile")


def counted(binary: Path, source: Path, arguments: list[str], level: str, events: str = "instructions:u") -> dict[str, int]:
    """perf stat's counts of one compile, by event name (cycles and instructions)."""
    with llrmbin.with_object(["perf", "stat", "-x,", "-e", events, str(binary), *arguments, f"-{level}", "-c", "-o", llrmbin.OBJECT, str(source)]) as command:
        done = subprocess.run(command, capture_output=True, text=True, env={**os.environ, **llrmbin.cc_environment()})
    checked(done, source)
    return {name: int(n) for n, name in re.findall(r"^(\d+),,(\w+)", done.stderr, re.M)}


def change(was: float, now: float) -> str:
    return f"{100 * (now / was - 1):+.2f}%"


def callgrind(binary: Path, source: Path, arguments: list[str], level: str) -> dict[str, int]:
    """Ir, D1 misses (read, write) and last-level misses of one compile under callgrind's cache simulation."""
    with llrmbin.with_object(
        ["valgrind", "--tool=callgrind", "--trace-children=yes", "--cache-sim=yes", "--callgrind-out-file=/dev/null", str(binary), *arguments, f"-{level}", "-c", "-o", llrmbin.OBJECT, str(source)]
    ) as command:
        done = subprocess.run(command, capture_output=True, text=True, env={**os.environ, **llrmbin.cc_environment()})
    checked(done, source)
    text = done.stderr
    # --trace-children: one report per process of the driver's tree (llrm-cc, clang, llrm-llc), summed
    num = lambda pattern: sum(int(one.replace(",", "")) for one in re.findall(pattern, text))  # noqa: E731
    return {
        "Ir": num(r"I\s+refs:\s+([\d,]+)"),
        "D1mr": num(r"D1\s+misses:\s+[\d,]+\s+\(\s*([\d,]+) rd"),
        "D1mw": num(r"D1\s+misses:\s+[\d,]+\s+\(\s*[\d,]+ rd \+\s*([\d,]+) wr"),
        "LL": num(r"LL misses:\s+([\d,]+)"),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("parent")
    parser.add_argument("head")
    parser.add_argument("--profile", choices=["dist", "release"], default="dist")
    parser.add_argument("--allow-env", nargs="*", default=[], metavar="NAME", help="the LLRM_CHECK_*/LLRM_EXP_* switch under test, set alike for both sides; any other is refused")
    parser.add_argument("--levels", nargs="+", default=["O1", "O2", "Os"])
    parser.add_argument("--programs", nargs="+", default=["d_faces", "lru", "bintree"])
    parser.add_argument("--cycles", type=int, default=0, metavar="N", help="also cycles:u, min and median of N alternated runs")
    parser.add_argument("--cache", action="store_true", help="also callgrind's cache simulation (slow)")
    parser.add_argument("--allow-other-corpus", action="store_true", help="measure a d_faces source tree whose hash is not the registered one (tools/corpora.py)")
    args = parser.parse_args()
    envguard.require_clean("abmeasure", allow=args.allow_env)
    dfaces = None
    if "d_faces" in args.programs:
        dfaces = str(corpora.resolve("dfaces", os.environ.get("DFACES_SRC") or os.environ.get("QCPORT"), args.allow_other_corpus))
        print(corpora.header("dfaces", dfaces))
    a, b = build(args.parent, profile=args.profile) / "llrm-cc", build(args.head, profile=args.profile) / "llrm-cc"
    print(llrmbin.assert_distinct(a, b, same_source=args.parent == args.head))
    work = CACHE / "work"
    work.mkdir(parents=True, exist_ok=True)
    print(f"parent {args.parent} -> {a}\nhead   {args.head} -> {b}\nbuild: CARGO_INCREMENTAL=0 env -u RUSTFLAGS cargo build --profile {args.profile} --bin llrm-cc --bin llrm-llc; switches: {' '.join(envguard.stray(allow=())) or 'none'}{' (allowed: ' + ' '.join(args.allow_env) + ')' if args.allow_env else ''}; load {open('/proc/loadavg').read().split()[0]}")
    for name, (source, flags) in programs(ROOT, args.programs, dfaces, work).items():
        for level in args.levels:
            ia, ib = counted(a, source, flags, level)["instructions"], counted(b, source, flags, level)["instructions"]
            floored(name, min(ia, ib))
            line = f"{name:8} -{level:2} instructions {ia / 1e6:9.1f}M -> {ib / 1e6:9.1f}M ({change(ia, ib)})"
            if args.cycles:
                ca, cb = [], []
                for k in range(args.cycles):
                    for binary, sink in ((a, ca), (b, cb)) if k % 2 == 0 else ((b, cb), (a, ca)):
                        sink.append(counted(binary, source, flags, level, "cycles:u")["cycles"])
                line += f"  cycles min {change(min(ca), min(cb))} median {change(statistics.median(ca), statistics.median(cb))}"
            if args.cache:
                x, y = callgrind(a, source, flags, level), callgrind(b, source, flags, level)
                line += "  " + " ".join(f"{k} {change(x[k], y[k])}" for k in x)
            print(line, flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
