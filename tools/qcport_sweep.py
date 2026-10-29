#!/usr/bin/env python3
"""Compile every QCport module at -O1/-O2/-O3/-Os with QCport's flags on a feat/parity build, group the failures, file one issue per new kind, bisect it.

    tools/qcport_sweep.py --bin DIR [--issue] [--bisect GOOD]

A failure kind is the compiler's last line with numbers and block/label ids removed; `tools/qcport_sweep_known.toml` lists the kinds already filed (`match`,
`issue`). A kind in no entry is NEW: with --issue it is filed (title, repro, head sha, the modules and levels); with --bisect GOOD (a ref where it does not fail,
e.g. origin/main) the first-parent squashes GOOD..head are bisected with incremental release builds (one worktree and target, buildslot at nice 10) to the first bad one,
named in the issue. The report goes to stdout for the coordinator. Hourly: one dist target for the sweep, the bisect builds are the only other.
"""
from __future__ import annotations

import argparse
import contextlib
import os
import re
import subprocess
import sys
import tempfile
import tomllib
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import llrmbin  # noqa: E402
import stamp  # noqa: E402
sys.path.insert(0, str(HERE / "dosbatch"))
import cpulimit  # noqa: E402

LEVELS = ("-O1", "-O2", "-O3", "-Os")
MODULES = ("host", "render", "model", "game", "sound", "ui")
KNOWN = HERE / "qcport_sweep_known.toml"


@contextlib.contextmanager
def object_file():
    """An object path to compile to. the compiler writes the object only for an `.o` or `.obj` output and the assembly text for any other, `/dev/null` included, so a
    sweep that compiled to the null device never reached the encoder: ui/console.c died there for days while the sweep counted 0 failed."""
    with tempfile.TemporaryDirectory(prefix="sweep-") as directory:
        yield str(Path(directory) / "out.obj")


def kind(message: str) -> str:
    """The failure's kind: its last line without numbers, `name@3` function names and L1_125-style labels."""
    lines = message.strip().splitlines()
    for i, line in enumerate(lines):
        if "panicked at" in line and i + 1 < len(lines):  # the message under a panic, not rustc's trailing backtrace note
            where = re.sub(r"^.*panicked at ", "", line).rstrip(":")
            return re.sub(r"\d+", "N", f"panic {where}: {lines[i + 1].strip()}")[:160]
    last = next((line for line in reversed(lines) if "Error!" in line), None)  # a compiler's warnings follow its errors
    last = last or next((line for line in reversed(lines) if line.strip() and not line.startswith("note: run with")), "no message")
    last = re.sub(r"@?\w+@\d+:?", "F", last)
    last = re.sub(r"\bL\d+_\d+\b", "L", last)
    last = re.sub(r"\(\s*[\d, ]+\)", "(...)", last)
    return re.sub(r"\d+", "N", last)[:160]


def sweep(compiler: Path, qcport: Path, include: Path, levels=LEVELS, jobs: int = 8) -> list[tuple[str, str, str]]:
    """(module, level, kind) of every failed compile."""
    flags = [f for part in (*MODULES, "qgl") for f in ("-I", str(qcport / part))] + ["-I", str(include)]
    todo = [(s, level) for part in MODULES for s in sorted((qcport / part).glob("*.c")) for level in levels]

    def one(job):
        source, level = job
        try:
            with object_file() as out:
                done = cpulimit.run([str(compiler), level, "-c", *llrmbin.c_args(16), *flags, str(source), "-o", out], 300, capture_output=True, text=True, env={**os.environ, **llrmbin.cc_environment()})
        except cpulimit.MemoryLimit as error:
            return (source.stem, level, f"memory limit: {error}")
        except subprocess.TimeoutExpired:
            return (source.stem, level, "timeout: 300 s of CPU")
        return None if done.returncode == 0 else (source.stem, level, kind(done.stderr or done.stdout))

    with ThreadPoolExecutor(jobs) as pool:
        return [r for r in pool.map(one, todo) if r]


ORACLES = ("DATAFLOW", "FACTS", "CLOBBERED", "REPLAY", "INTERN", "STALE")
def qb_flat_programs(root: Path = ROOT) -> list[Path]:
    """tests/run/qb programs that name no target of their own: tests/run runs a BASIC program on its default target only, so nothing else compiles them for the flat one
    (ownf_local_pds71 and ownf_local_vbdos failed there with `@TOP: ptrtoint` while every run reported 0 fail)."""
    named = re.compile(r"^\s*'\s*targets:", re.M)
    return [source for source in sorted((root / "tests/run/qb").glob("*.bas")) if not named.search(source.read_text(encoding="latin1"))]


# The machines the front ends compile for. The 64-bit targets have no Nib runtime and OMF has no 64-bit records: their failures are not findings.
FRONT_END_MODES = (16, 32)


# What a tests/run program's header says makes it real-mode only: the far-string dialects, huge arrays and own frames have no flat form, so -m32 refuses or fails them
# by design (farchk, hugelp, intrinsics_vbdos, stackdeep... were seven "new kinds" of a sweep that compiled them at -m32).
M16_ONLY_DIALECTS = ("pds71", "vbdos")
M16_ONLY_FLAGS = ("--huge-arrays", "--own-frames")


def m16_only(source: Path) -> bool:
    """Whether the header of a tests/run program asks for a far-string dialect, `--huge-arrays` or `--own-frames` in any of its configurations."""
    sys.path.insert(0, str(ROOT / "tools/dosbatch"))
    import run_tests  # noqa: PLC0415

    settings = run_tests.header(source)
    # a `dialect:` line passes `--runtime <dialect>`, the Microsoft runtime, which has no flat form (intrinsics_qb45 and range_exp failed at -m32 with relocation overlaps)
    return "dialect" in settings or any(dialect in M16_ONLY_DIALECTS or any(flag in M16_ONLY_FLAGS for flag in flags) for _, flags, dialect in run_tests.configurations(settings))


def extra_jobs(root: Path = ROOT) -> list[tuple[str, Path, str, str]]:
    """(tool, source, machine, level): bench/bintree in Nib and BASIC and tests/run/qb at every front-end machine (`FRONT_END_MODES`), bintree at all levels, tests/run/qb at -O1 and -O2."""
    found = {tomllib.loads(path.read_text())["mode"] for path in (root / "crates/target").glob("*/src/machines/datalayout.toml")}
    machines = [f"-m{mode}" for mode in sorted(found & set(FRONT_END_MODES))]
    benches = [(tool, root / f"bench/bintree/bintree.{ext}", machine, level) for tool, ext in (("llrm-nib", "nib"), ("llrm-qb", "bas")) for machine in machines for level in LEVELS]
    # Borland's call-boundary fixtures (C): an `interrupt` handler with nine register parameters was refused for four days (#2936) because nothing compiled it outside a lib test.
    callconv = [("llrm-cc", source, "-m16", level) for source in sorted((root / "tests/fixtures/callconv/c").glob("*.c")) for level in ("-O1", "-O2")]
    return benches + [("llrm-qb", source, machine, level) for source in qb_flat_programs(root) for machine in machines if not (machine == "-m32" and m16_only(source)) for level in ("-O1", "-O2")] + callconv


# A refusal the compiler states as policy: a program that needs a second address space is not compiled for the flat target.
BY_DESIGN = re.compile(r"is not supported at one address space")


def program_flags(source: Path) -> list[str]:
    """What a tests/run program's header asks of the compiler apart from level and cpu (`--dialect pds71 --runtime pds71 --huge-arrays`), as tests/run builds it: swept
    without them, hugelp and hugerg failed with a backend selection error that tests/run never sees."""
    sys.path.insert(0, str(ROOT / "tools/dosbatch"))
    import run_tests  # noqa: PLC0415

    return run_tests.compiler_arguments(source) if source.suffix == ".bas" and "tests/run" in source.as_posix() else []


def sweep_extra(bins: Path, jobs: int = 8, root: Path = ROOT, include: Path | None = None) -> list[tuple[str, str, str]]:
    """(program, 'machine level', kind) of every failed compile of `extra_jobs`, oracles on."""
    env = {**os.environ, **{f"LLRM_CHECK_{name}": "1" for name in ORACLES}}

    def one(job):
        tool, source, machine, level = job
        headers = ["-I", str(include)] if include and source.suffix == ".c" else []
        with object_file() as out:
            target = ["-c", *llrmbin.c_args(int(machine[2:]))] if tool == "llrm-cc" else [machine]
            done = subprocess.run([str(bins / tool), str(source), *program_flags(source), *headers, level, *target, "-o", out], capture_output=True, text=True, env={**env, **llrmbin.cc_environment()})
        if done.returncode == 0 or (machine == "-m32" and BY_DESIGN.search(done.stderr + done.stdout)):
            return None
        return (source.name, f"{machine} {level}", kind(done.stderr or done.stdout))

    with ThreadPoolExecutor(jobs) as pool:
        return [r for r in pool.map(one, extra_jobs(root)) if r]


def grouped(failures: list[tuple[str, str, str]]) -> dict[str, list[tuple[str, str]]]:
    out: dict[str, list[tuple[str, str]]] = {}
    for module, level, why in failures:
        out.setdefault(why, []).append((module, level))
    return out


def new_kinds(groups: dict, known: list[dict]) -> dict:
    """The kinds no `match` of the known file covers."""
    return {why: where for why, where in groups.items() if not any(entry["match"] in why for entry in known)}


def first_bad(commits: list[str], is_bad) -> str:
    """The first bad commit of `commits` (old to new, the last one bad): binary search, `is_bad(commit)` builds and tests it, and answers None for a commit that
    does not build (a head broke for hours between two merges that each built). Such a commit is passed over: the probe takes the nearest one that builds, above
    the middle or else below it. Assumes a single flip."""
    lo, hi = 0, len(commits) - 1  # hi is bad
    while lo < hi:
        mid = (lo + hi) // 2
        for probe in [*range(mid, hi), *range(mid - 1, lo - 1, -1)]:
            verdict = is_bad(commits[probe])
            if verdict is not None:
                break
        else:
            return commits[hi]  # nothing between lo and hi builds
        if verdict:
            hi = probe
        elif probe >= mid:
            lo = probe + 1
        else:
            return commits[hi]  # the nearest below mid is good and everything above it up to hi does not build
    return commits[hi]


def bisect_one(good: str, repro: list[str], build) -> tuple[str, str]:
    """(sha, subject) of the first first-parent squash after `good` on which `repro` (a compile of the failing module) fails; `build(sha)` returns the llrm-cc path."""
    shas = subprocess.run(["git", "log", "--first-parent", "--reverse", "--format=%H", f"{good}..origin/feat/parity"], cwd=ROOT, capture_output=True, text=True).stdout.split()

    def bad(sha: str) -> bool | None:
        try:
            binary = build(sha)
        except subprocess.CalledProcessError:
            return None
        return subprocess.run([str(binary), *repro], capture_output=True).returncode != 0

    found = first_bad(shas, bad)
    subject = subprocess.run(["git", "log", "-1", "--format=%s", found], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    return found, subject


def worktree_build(sha: str) -> Path:
    """llrm-cc and llrm-llc at `sha`: one worktree, one target, incremental release builds under buildslot at nice 10."""
    tree = Path.home() / "scratch/qcport-sweep-tree"
    if not tree.exists():
        subprocess.run(["git", "worktree", "add", "-q", "--detach", str(tree), sha], cwd=ROOT, check=True)
    subprocess.run(["git", "checkout", "-q", "--detach", sha], cwd=tree, check=True)
    env = {**os.environ, "CARGO_TARGET_DIR": str(Path.home() / "scratch/qcport-sweep-target")}
    env.pop("RUSTFLAGS", None)
    subprocess.run(["nice", "-n", "10", str(Path.home() / ".local/bin/buildslot"), "cargo", "build", "--release", "-q", "--bin", "llrm-cc", "--bin", "llrm-llc"], cwd=tree, env=env, check=True)
    return Path(env["CARGO_TARGET_DIR"]) / "release/llrm-cc"


def issue_text(why: str, where: list[tuple[str, str]], head: str, first: tuple[str, str] | None) -> tuple[str, str]:
    module, level = where[0]
    title = f"qcport sweep: {why[:90]} ({module} {level}{', ...' if len(where) > 1 else ''})"
    mods = ", ".join(sorted({f'{m} {l}' for m, l in where})[:12])
    body = (
        f"TL;DR: compiling QCport's {module}.c at {level} fails on feat/parity {head[:9]}: `{why}`.\n\n"
        f"Repro: `llrm-cc {level} -c --target=x86_16-wat -march=i386 -I $QCPORT/{{host,render,model,game,sound,ui,qgl}} -I $QCPORT_INC $QCPORT/<dir>/{module}.c -o /tmp/out.obj` "
        f"(QCPORT=~/scratch/qcport/src, QCPORT_INC=~/scratch/qctc/inc).\n\nFails: {mods}{' ...' if len(where) > 12 else ''}.\n"
        + (f"\nFirst bad squash (first-parent bisection): {first[0][:9]} {first[1]}\n" if first else "")
    )
    return title, body


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--bin", type=Path, default=None)
    parser.add_argument("--issue", action="store_true", help="file one issue per new failure kind")
    parser.add_argument("--bisect", metavar="GOOD", help="bisect each new kind from this good ref to the head")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "8")))
    args = parser.parse_args()
    qcport = Path(os.environ.get("QCPORT", Path.home() / "scratch/qcport/src")).expanduser()
    include = Path(os.environ.get("QCPORT_INC", Path.home() / "scratch/qctc/inc")).expanduser()
    compiler = llrmbin.compiler(args.bin or llrmbin.bin_dir())
    # The binary names what was swept: origin/feat/parity moves during a run, and a report must not name a sha it did not test.
    head, dirty = stamp.read(compiler)
    failures = sweep(compiler, qcport, include, jobs=args.jobs) + sweep_extra(compiler.parent, jobs=args.jobs, include=include)
    groups = grouped(failures)
    known = tomllib.loads(KNOWN.read_text()).get("known", []) if KNOWN.exists() else []
    fresh = new_kinds(groups, known)
    modules = len(list((qcport / "render").glob("*.c"))) + sum(len(list((qcport / p).glob("*.c"))) for p in MODULES if p != "render")
    print(f"QCport sweep of {head[:9]}{"+dirty" if dirty else ""}: {modules} modules x {len(LEVELS)} levels + {len(extra_jobs())} front-end compiles ({len([s for s in qb_flat_programs() if m16_only(s)])} m16-only programs not compiled at -m32), {len(failures)} failed compiles, {len(groups)} kinds, {len(fresh)} new")
    for why, where in groups.items():
        entry = next((e for e in known if e["match"] in why), None)
        print(f"  {'KNOWN #' + str(entry['issue']) if entry else 'NEW'}: {why} -- {', '.join(sorted({f'{m} {l}' for m, l in where})[:8])}")
    for why, where in fresh.items():
        first = None
        if args.bisect:
            module, level = where[0]
            src = next((qcport / p / f"{module}.c" for p in MODULES if (qcport / p / f"{module}.c").exists()), None)
            flags = [level] + [f for part in (*MODULES, "qgl") for f in ("-I", str(qcport / part))] + ["-I", str(include), str(src), "-o", str(Path(tempfile.gettempdir()) / "sweep-bisect.obj")]
            first = bisect_one(args.bisect, flags, worktree_build)
            print(f"  first bad squash of '{why[:60]}': {first[0][:9]} {first[1]}")
        if args.issue:
            title, body = issue_text(why, where, head, first)
            done = subprocess.run(["gh", "issue", "create", "--title", title, "--body", body], capture_output=True, text=True)
            print(f"  filed: {done.stdout.strip() or done.stderr.strip()}")
    return 1 if fresh else 0


if __name__ == "__main__":
    sys.exit(main())
