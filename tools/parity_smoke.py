#!/usr/bin/env python3
"""The per-merge tests of the integration branch feat/parity: the touched crates' fastest unit tests, a few fast run tests and a compile smoke, 10 s of CPU in all.
First, untimed, `cargo check --workspace --all-targets` of the merged tree and every source-scan ratchet (whatever the merge touches).

    tools/parity_smoke.py [--bin DIR] [--base REF] [--rank CRATE...]

The last line is `PARITY SMOKE PASS ...` or `PARITY SMOKE FAIL ...`, with the CPU seconds spent. Build time is not in the 10 s: the unit tests are built first, untimed.
The selection (tools/parity_smoke.toml) is
  units   per crate that `git diff BASE` touches (crates/<group>/<crate>/...): its cheapest unit tests, the ranked list `--rank` writes, cut to the crate's share of
          what the other steps leave; with no ranking yet, all its `--lib` tests, killed at that share (a failure before the cut fails; the cut itself is a note)
  run     a few tests/run programs (tools/dosbatch/run_tests.py NAME...), compiled and run in one DOSBox launch
  compile bintree as BASIC, Nib and C at -m16/-m32 and QCport's sc.c, every oracle on; d_faces -m32 -O2 run in unicorn against gcc -O0's reports (~3 s)
The budget is CPU seconds (this process and its children): a loaded host stretches the wall clock of the same work 2-3x. A test fails when the declared selection
(the fixed steps plus the unit shares of every crate at once) exceeds it; an overrun at run time fails the merge. Slower checks belong to the CI gate of a main
merge candidate, not here. `--rank CRATE` measures that crate's unit tests once (not per merge) and rewrites its ranked list.
"""
from __future__ import annotations

import argparse
import hashlib
import os
import re
import signal
import subprocess
import sys
import tempfile
import time
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import llrmbin  # noqa: E402
from buildslot import slotted  # noqa: E402

SELECTION = HERE / "parity_smoke.toml"
BUDGET = 10.0
CARGO_OVERHEAD = 0.3  # cargo's own CPU to find the test binary up to date, per crate
MAX_CRATES = 2  # a merge touching more is tested in the 3 crates with most changed files: each crate costs its cargo start before any test
UNIT_FLOOR = 2.0  # the seconds the fixed steps always leave for unit tests
ORACLES = ("DATAFLOW", "FACTS", "CLOBBERED", "COALESCE", "REPLAY", "INTERN", "SKIPPED", "STALE")
PROGRAM = ROOT / "bench" / "bintree"
# d_faces.c as the scoreboard reads it (sha256 prefix), and the six reports `env.c` printed for gcc -O0: an llrm build must print the same.
DFACES_SHA = "1ab72e38cab4fbd2"
DFACES_REPORTS = [1502227429, 443461, -222045, 1788001, 271784, 714164]
KEYWORDS = "far huge near pascal cdecl __far _far __near _near"


_charged = [0.0]  # CPU of process groups killed at their cut: the dead are not reaped by us, so os.times() never counts them


def cpu() -> float:
    """CPU seconds of this process and its children, and of the groups cut short (each charged the whole time it was given)."""
    t = os.times()
    return t.user + t.system + t.children_user + t.children_system + _charged[0]


def run_cut(command: list[str], seconds: float, cwd: Path) -> tuple[int | None, str]:
    """(exit code, output) of `command`; (None, '') if it ran past `seconds` and its whole process group was killed. A cargo killed alone leaves its test binary
    running unseen: the unit step read '0.0 s' for 1,143 tests that went on in the background."""
    child = subprocess.Popen(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, start_new_session=True)
    try:
        out, _ = child.communicate(timeout=seconds)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:  # it ended between the timeout and the kill
            pass
        child.communicate()
        _charged[0] += seconds
        return None, ""
    return child.returncode, out


def workspace_check(root: Path = ROOT) -> str:
    """"" when the whole workspace, tests and benches included, compiles; else the first errors. Built before the clock starts, like the unit tests: two halves
    that each compile can fail together (a `&mut Context` parameter called with a `&Context` broke feat/parity's head for every build)."""
    done = subprocess.run(slotted(["cargo", "check", "--workspace", "--all-targets", "-q", "--message-format", "short"]), cwd=root, capture_output=True, text=True)
    if done.returncode == 0:
        return ""
    errors = [line for line in done.stderr.splitlines() if "error" in line]
    return " / ".join(errors[:3])[:300] or "cargo check failed"


TIERS = ROOT / "tools/gate/tiers.toml"
EXTRA_RATCHETS = (["cargo", "test", "-q", "--test", "block_list_ratchet"], ["cargo", "test", "-q", "-p", "llrm-mir", "--lib", "facts_rewrite"])


def ratchet_commands(tiers: Path = TIERS) -> list[list[str]]:
    """Every source-scan test of the gate (tools/gate/tiers.toml `[[scan]]`) and the two that list readers (block list, facts_rewrite), as cargo commands. A test
    that counts what a crate's files read fails on a diff in ANOTHER crate: a reader added in llrm-transforms never ran llrm-mir's ratchets (loopdeletion +3,
    simpleloopunswitch +5 slipped in), because the smoke ran only the touched crates' unit tests."""
    commands = [list(c) for c in EXTRA_RATCHETS]
    for scan in tomllib.loads(tiers.read_text()).get("scan", []):
        if "lib" in scan:
            commands.append(["cargo", "test", "-q", "-p", scan["package"], "--lib", "--", scan["lib"]])
        elif "test" in scan:
            commands.append(["cargo", "test", "-q", "-p", scan["package"], "--test", scan["test"]])
        elif scan.get("step") == "integration":
            commands.append(["cargo", "test", "-q", "--test", Path(scan["file"]).stem])
    return commands


def ratchets(root: Path = ROOT, tiers: Path = TIERS, run=None) -> str:
    """"" when every ratchet passes on the tree in `root`; else the first that fails. Run before the clock starts, whatever the merge touches."""
    run = run or (lambda command: subprocess.run(slotted(command), cwd=root, capture_output=True, text=True))
    for command in ratchet_commands(tiers):
        done = run(command)
        if done.returncode:
            lines = [line for line in (done.stdout + done.stderr).splitlines() if "panicked" in line or "FAILED" in line or "error" in line]
            return f"{' '.join(command[1:])}: {' / '.join(lines[:2])[:240] or 'failed'}"
    return ""


def selection() -> dict:
    return tomllib.loads(SELECTION.read_text()) if SELECTION.exists() else {}


# --- what a merge touches ------------------------------------------------------------------------------------------------------------------------------------


def crate_of(path: str, root: Path = ROOT) -> tuple[str, str] | None:
    """(package, directory) of the crate a file under crates/<group>/<crate>/ belongs to."""
    parts = path.split("/")
    if len(parts) < 4 or parts[0] != "crates":
        return None
    manifest = root / "/".join(parts[:3]) / "Cargo.toml"
    if not manifest.is_file():
        return None
    name = tomllib.loads(manifest.read_text()).get("package", {}).get("name")
    return (name, "/".join(parts[:3])) if name else None


def touched(base: str, root: Path = ROOT) -> list[str]:
    done = subprocess.run(["git", "diff", "--name-only", base], cwd=root, capture_output=True, text=True)
    counts: dict[str, int] = {}
    for f in done.stdout.split():
        if c := crate_of(f, root):
            counts[c[0]] = counts.get(c[0], 0) + 1
    return sorted(counts, key=lambda p: (-counts[p], p))


# --- the unit tests -------------------------------------------------------------------------------------------------------------------------------------------

REPORT = re.compile(r"^test (\S+) \.\.\. ok <([\d.]+)s>$", re.M)


def ranked(output: str) -> list[tuple[str, float]]:
    """(test, seconds) of the passing tests in a `-Z unstable-options --report-time` listing, cheapest first."""
    return sorted(((name, float(sec)) for name, sec in REPORT.findall(output)), key=lambda one: one[1])


def share(allowance: float, crates: int) -> float:
    return allowance / max(crates, 1)


def unit_tests(package: str, allowance: float, ranking: dict) -> list[str]:
    """The names to run for `package`: its cheapest ranked tests whose cumulative cost fits `allowance`; [] where it has no ranking (all of them, cut at the allowance)."""
    out, total = [], 0.0
    for name, seconds in ranking.get(package, []):
        if total + seconds > allowance:
            break
        out.append(name)
        total += seconds
    return out


# --- the steps ----------------------------------------------------------------------------------------------------------------------------------------------


def oracle_env() -> dict[str, str]:
    return {**os.environ, **{f"LLRM_CHECK_{name}": "1" for name in ORACLES}}


def compile_ok(command: list[str], env: dict[str, str]) -> str | None:
    """None where the compile succeeded; else its first line of complaint."""
    done = subprocess.run(command, capture_output=True, text=True, env=env)
    if done.returncode == 0:
        return None
    lines = [line for line in (done.stderr or done.stdout).splitlines() if line.strip()]
    return f"exit {done.returncode}: {lines[-1][:140] if lines else 'no message'}  [{' '.join(command[:3])}]"


def programs(bins: Path) -> tuple[str, str]:
    jobs = []
    for tool, ext in (("llrm-qb", "bas"), ("llrm-nib", "nib"), ("llrm-c", "c")):
        for machine in ("-m16", "-m32"):
            extra = ["-mabi=sysv", "-march=i486"] if tool == "llrm-c" and machine == "-m32" else []
            jobs.append([str(bins / tool), str(PROGRAM / f"bintree.{ext}"), "-O2", machine, *extra, "-o", os.devnull])
    for command in jobs:
        if why := compile_ok(command, oracle_env()):
            return "fail", why
    return "ok", f"{len(jobs)} compiles"


def sc(bins: Path) -> tuple[str, str]:
    qcport = Path(os.environ.get("QCPORT", Path.home() / "scratch/qcport/src")).expanduser()
    include = Path(os.environ.get("QCPORT_INC", Path.home() / "scratch/qctc/inc")).expanduser()
    source = qcport / "render" / "sc.c"
    if not source.is_file() or not include.is_dir():
        return "skipped", f"no QCport sources ({qcport})"
    flags = [f for part in ("", "render", "game", "host", "qgl", "model", "sound", "ui") for f in ("-I", str(qcport / part if part else qcport))] + ["-I", str(include)]
    why = compile_ok([str(bins / "llrm-c"), "-O1", *flags, str(source), "-o", os.devnull], oracle_env())
    return ("fail", why) if why else ("ok", "sc.c -m16 -O1")


def d_faces(bins: Path) -> tuple[str, str]:
    src = Path(os.environ.get("DFACES_SRC", Path.home() / "scratch/qbq-llrm/src")).expanduser()
    file = src / "render" / "d_faces.c"
    if not file.is_file():
        return "skipped", f"no d_faces.c ({src})"
    if hashlib.sha256(file.read_bytes()).hexdigest()[:16] != DFACES_SHA:
        return "skipped", f"{file} is not the pinned d_faces.c ({DFACES_SHA}): the reference reports are its"
    sys.path.insert(0, str(next((ROOT / "crates/target").glob("*/vsgcc/dfaces"))))  # a target's directory is found, not named (tests/target_facts.rs)
    os.environ["LLRM_BIN"], os.environ["DFACES_SRC"] = str(bins), str(src)
    with tempfile.TemporaryDirectory() as work:
        os.environ["VSGCC_WORK"] = work
        try:
            import run  # noqa: PLC0415  the scoreboard's runner: loads unicorn, so only here
            got = run.run("llrm")["reports"]
        except (AssertionError, ModuleNotFoundError) as why:
            return "fail", f"d_faces did not run: {str(why)[-200:]}"
    return ("ok", "reports equal gcc -O0's") if got == DFACES_REPORTS else ("fail", f"d_faces reports {got}, gcc -O0 printed {DFACES_REPORTS}")


def qc_d_faces(bins: Path) -> tuple[str, str]:
    """QCport's own render/d_faces.c with the flags the QCport build uses (the compiler's default target, QCport's -I dirs), at all four levels: the flat d_faces of the `d_faces` step is another
    file at one level, and passed on 3858c5195 while this one failed the verifier at -O2/-O3 ('block 964 leaves for ... with no instruction choosing')."""
    qcport = Path(os.environ.get("QCPORT", Path.home() / "scratch/qcport/src")).expanduser()
    include = Path(os.environ.get("QCPORT_INC", Path.home() / "scratch/qctc/inc")).expanduser()
    source = qcport / "render" / "d_faces.c"
    if not source.is_file() or not include.is_dir():
        return "skipped", f"no QCport sources ({qcport})"
    flags = [f for part in ("host", "render", "model", "game", "sound", "ui", "qgl") for f in ("-I", str(qcport / part))] + ["-I", str(include)]
    for level in ("-O1", "-O2", "-O3", "-Os"):
        if why := compile_ok([str(bins / "llrm-c"), level, *flags, str(source), "-o", os.devnull], os.environ):
            return "fail", f"QCport d_faces.c {level}: {why}"
    return "ok", "QCport d_faces.c at 4 levels"


QC_MODULES = ("game/combat.c", "host/mathlib.c", "render/d_sprite.c", "sound/snd.c")


def qc_modules(bins: Path) -> tuple[str, str]:
    """Four more QCport modules at -O2 (compile only, the compiler's default target): 48 of 260 QCport compiles died on `BitCast of a float` (#2586) while the smoke
    passed, because it compiled sc.c at -O1 and one module (d_faces) at four levels, and none of them has that float shape."""
    qcport = Path(os.environ.get("QCPORT", Path.home() / "scratch/qcport/src")).expanduser()
    include = Path(os.environ.get("QCPORT_INC", Path.home() / "scratch/qctc/inc")).expanduser()
    if not (qcport / QC_MODULES[0]).is_file() or not include.is_dir():
        return "skipped", f"no QCport sources ({qcport})"
    flags = [f for part in ("host", "render", "model", "game", "sound", "ui", "qgl") for f in ("-I", str(qcport / part))] + ["-I", str(include)]
    for module in QC_MODULES:
        if why := compile_ok([str(bins / "llrm-c"), "-O2", *flags, str(qcport / module), "-o", os.devnull], os.environ):
            return "fail", f"QCport {module} -O2: {why}"
    return "ok", f"{len(QC_MODULES)} QCport modules at -O2"


def run_cases(bins: Path) -> tuple[str, str]:
    names = selection().get("run", {}).get("cases", [])
    if not names:
        return "skipped", "no tests/run cases chosen (parity_smoke.toml [run] cases)"
    env = {**os.environ, "LLRM_BIN": str(bins)}
    done = subprocess.run([sys.executable, str(ROOT / "tools/dosbatch/run_tests.py"), *names], cwd=ROOT, capture_output=True, text=True, env=env)
    if done.returncode:
        lines = [line for line in (done.stdout + done.stderr).splitlines() if line.strip()]
        return "fail", " / ".join(lines[-3:])[:300]
    return "ok", f"{len(names)} programs"


def unit_step(package: str, allowance: float):
    def step(bins: Path) -> tuple[str, str]:
        ranking = selection().get("units", {})
        names = unit_tests(package, allowance, {k: [(n, s) for n, s in v.get("tests", [])] for k, v in ranking.items()})
        command = ["cargo", "test", "-q", "--lib", "-p", package] + (["--", "--exact", *names] if names else [])
        code, output = run_cut(slotted(command), allowance + CARGO_OVERHEAD, ROOT)
        if code is None:
            return "ok", f"{package}: no ranking yet, cut at {allowance:.1f} s without a failure"
        if code:
            lines = [line for line in output.splitlines() if "FAILED" in line or "panicked" in line or "error" in line]
            return "fail", f"{package}: {' / '.join(lines[:3])[:300] or 'cargo test failed'}"
        return "ok", f"{package}: {len(names) or 'all'} tests"
    return step


FIXED = [("programs", programs, 0.3), ("sc", sc, 0.8), ("d_faces", d_faces, 1.8), ("qc_d_faces", qc_d_faces, 1.0), ("qc_modules", qc_modules, 0.8), ("run", run_cases, 3.6)]  # (name, step, declared seconds)


def fixed_cost() -> float:
    return sum(cost for _, _, cost in FIXED)


def plan(packages: list[str], budget: float = BUDGET) -> list:
    """The steps for a merge touching `packages`: the fixed ones, then each crate's unit tests at an equal share of what is left."""
    packages = packages[:MAX_CRATES]
    left = max(budget - fixed_cost() - CARGO_OVERHEAD * len(packages), 0.0)
    return [*FIXED, *((f"unit {p}", unit_step(p, share(left, len(packages))), share(left, len(packages))) for p in packages)]


def declared(packages: list[str], budget: float = BUDGET) -> float:
    return sum(cost for _, _, cost in plan(packages, budget))


def run_steps(steps, bins: Path, clock=cpu, budget: float = BUDGET, known_red: frozenset[str] = frozenset()) -> tuple[bool, str]:
    """(passed, the last line). The clock is the test's, which waits on virtual time. A step named in `known_red` is not run and is said to be skipped."""
    started, wall, parts, failed, skipped = clock(), time.monotonic(), [], [], []
    for name, step, _ in steps:
        began = clock()
        verdict, detail = ("skipped", "known red, not run") if name in known_red else step(bins)
        parts.append(f"{name} {verdict} {clock() - began:.1f} s")
        if verdict == "fail":
            failed.append(f"{name}: {detail}")
        elif verdict == "skipped":
            skipped.append(f"{name}: {detail}")
    spent = clock() - started
    if spent > budget:
        failed.append(f"over the budget: {spent:.1f} s > {budget:.0f} s")
    line = f"{' | '.join(parts)} | {spent:.1f} s CPU of {budget:.0f} s ({time.monotonic() - wall:.1f} s wall)" + (f" | skipped {'; '.join(skipped)}" if skipped else "")
    if failed:
        return False, f"PARITY SMOKE FAIL {'; '.join(failed)} | {line}"
    return True, f"PARITY SMOKE PASS {line}"


def rank(package: str) -> None:
    """Measure `package`'s unit tests once and write the cheapest into parity_smoke.toml (not per merge)."""
    env = {**os.environ, "RUSTC_BOOTSTRAP": "1"}
    done = subprocess.run(slotted(["cargo", "test", "--lib", "-p", package, "--", "-Z", "unstable-options", "--report-time", "--test-threads", "1"]), cwd=ROOT, capture_output=True, text=True, env=env)
    tests = [(n, s) for n, s in ranked(done.stdout) if s <= 0.5][:40]
    data = selection()
    data.setdefault("units", {})[package] = {"tests": [[n, s] for n, s in tests]}
    SELECTION.write_text(dump(data))
    print(f"{package}: {len(tests)} tests ranked, cheapest {tests[0] if tests else None}")


def dump(data: dict) -> str:
    out = []
    if "run" in data:
        out.append("[run]\ncases = [" + ", ".join(f'"{c}"' for c in data["run"].get("cases", [])) + "]\n")
    for package, one in sorted(data.get("units", {}).items()):
        out.append(f'[units."{package}"]\ntests = [\n' + "".join(f'  ["{n}", {s}],\n' for n, s in one.get("tests", [])) + "]\n")
    return "\n".join(out)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--bin", type=Path, default=None, help="the llrm-c, llrm-qb and llrm-nib directory (default: LLRM_BIN, else the target dir's release)")
    parser.add_argument("--base", default="origin/feat/parity", help="what the merge result is compared with to find the touched crates")
    parser.add_argument("--skip", action="append", default=[], metavar="STEP", help="a step known red (tools/parity_squash.py --known-red): not run, named as skipped in the line")
    parser.add_argument("--rank", nargs="+", metavar="CRATE", help="measure these crates' unit tests once and write the ranking")
    args = parser.parse_args()
    if args.rank:
        for package in args.rank:
            rank(package)
        return 0
    if broken := workspace_check():
        print(f"PARITY SMOKE FAIL the workspace does not compile: {broken}")
        return 1
    if broken := ratchets():
        print(f"PARITY SMOKE FAIL a ratchet fails on the merged tree: {broken}")
        return 1
    packages = touched(args.base)
    for package in packages:  # built first, untimed
        subprocess.run(slotted(["cargo", "test", "-q", "--lib", "-p", package, "--no-run"]), cwd=ROOT, capture_output=True)
    ok, line = run_steps(plan(packages), args.bin or llrmbin.bin_dir(), known_red=frozenset(args.skip))
    print(line)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
