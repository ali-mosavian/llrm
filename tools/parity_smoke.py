#!/usr/bin/env python3
"""The per-merge tests of the integration branch feat/parity: the touched crates' fastest unit tests, a few fast run tests and a compile smoke, 10 s of CPU in all.
First, untimed, `cargo check --workspace --all-targets` of the merged tree and every source-scan ratchet (whatever the merge touches).

    tools/parity_smoke.py [--bin DIR] [--base REF] [--rank CRATE...]

The last line is `PARITY SMOKE PASS ...` or `PARITY SMOKE FAIL ...`, with the CPU seconds spent. Build time is not in the 15 s: the unit tests are built first, untimed.
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
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import tomllib
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import llrmbin  # noqa: E402
from stages import stage  # noqa: E402
import xfail  # noqa: E402

sys.path.insert(0, str(HERE / "dosbatch"))
import cpulimit  # noqa: E402
from buildslot import slotted  # noqa: E402

SELECTION = HERE / "parity_smoke.toml"
BUDGET = 15.0  # CPU seconds (user+sys of this process and its children, never wall); the user's rule is a fast set within 10-15 s
INSTRUCTIONS = 60_000_000_000  # user-space instructions of the steps. Today's set (a tools-only merge, one unit crate) is 50.9 G, repeating to 0.1 %, in 12.6-13.4 s CPU at load 31-33: 4.0 G/s at best, so 15 s is 60 G
CARGO_OVERHEAD = 0.3  # cargo's own CPU to find the test binary up to date, per crate
MAX_CRATES = 2  # a merge touching more is tested in the 3 crates with most changed files: each crate costs its cargo start before any test
UNIT_FLOOR = 2.0  # the seconds the fixed steps always leave for unit tests
ORACLES = ("DATAFLOW", "FACTS", "CLOBBERED", "REPLAY", "INTERN", "STALE")
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


class Meter:
    """User-space instructions of every command the steps start while open: each is run under `perf stat -e instructions:u -- COMMAND` (load-independent, where CPU
    seconds stretch with contention) and the counts are summed. `perf stat -p PID` was tried first: it read 74 G, 799 G and 893 G for the same set, more than the
    cores can execute in the CPU seconds; wrapping the command repeats to 0.01 %. `.value` is None where perf is missing, and the budget then falls back to CPU."""

    def __init__(self) -> None:
        self.value: int | None = None
        self._files: list[Path] = []
        self._popen = None

    def __enter__(self) -> "Meter":
        if shutil.which("perf") is None:
            return self
        import tempfile

        meter, original = self, subprocess.Popen
        self._popen = original

        class Counting(original):  # type: ignore[valid-type, misc]
            def __init__(self, args, *rest, **kwargs):
                if isinstance(args, (list, tuple)) and not kwargs.get("shell") and args:
                    counts = Path(tempfile.mkstemp(prefix="instructions-")[1])
                    meter._files.append(counts)
                    args = ["perf", "stat", "-x,", "-e", "instructions:u", "-o", str(counts), "--", *args]
                super().__init__(args, *rest, **kwargs)

        subprocess.Popen = Counting
        return self

    def __exit__(self, *_) -> None:
        if self._popen is None:
            return
        subprocess.Popen = self._popen
        total, seen = 0, 0
        for counts in self._files:
            for line in counts.read_text().splitlines():
                fields = line.split(",")
                if len(fields) > 2 and fields[2].startswith("instructions") and fields[0].isdigit():
                    total, seen = total + int(fields[0]), seen + 1
            counts.unlink(missing_ok=True)
        self.value = total if seen else None


CUT_GRACE = 3.0  # seconds a cut group has to exit on SIGINT before SIGKILL


def run_cut(command: list[str], seconds: float, cwd: Path, env: dict[str, str] | None = None) -> tuple[int | None, str]:
    """(exit code, output) of `command`; (None, '') if it ran past `seconds` and its whole process group was stopped. A cargo killed alone leaves its test binary
    running unseen: the unit step read '0.0 s' for 1,143 tests that went on in the background. The group gets SIGINT first, so the `perf stat` wrapped around the command
    (see Meter) still writes the instructions it counted; SIGKILL took them to the grave and a cut step counted 0 G beside a finished one's 120 G."""
    child = subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, start_new_session=True)
    try:
        out, _ = child.communicate(timeout=seconds)
    except subprocess.TimeoutExpired:
        for sig in (signal.SIGINT, signal.SIGKILL):
            try:
                os.killpg(child.pid, sig)
            except ProcessLookupError:  # it ended between the timeout and the signal
                break
            try:
                child.communicate(timeout=CUT_GRACE if sig == signal.SIGINT else None)
                break
            except subprocess.TimeoutExpired:
                continue
        _charged[0] += seconds
        return None, ""
    return child.returncode, out


def workspace_check(root: Path = ROOT, target: Path | None = None) -> str:
    """"" when the whole workspace, tests and benches included, compiles under `-D warnings` (the fast tier's `build` step); else the first errors. Built before the clock
    starts, like the unit tests: two halves that each compile can fail together (a `&mut Context` parameter called with a `&Context` broke feat/parity's head for every
    build). Under `-D warnings` because the land-time check once lacked it: #3390 landed an unused `mut` that failed the fast tier for everyone. Its own target directory:
    other RUSTFLAGS would rebuild the shared one."""
    target = target or Path(os.environ.get("LLRM_WARN_TARGET", Path.home() / ".cache/llrm/warn-target"))
    done = subprocess.run(slotted(["cargo", "check", "--workspace", "--all-targets", "-q", "--message-format", "short"]), cwd=root, capture_output=True, text=True, env={**os.environ, "RUSTFLAGS": "-D warnings", "CARGO_TARGET_DIR": str(target)})
    if done.returncode == 0:
        return ""
    errors = [line for line in done.stderr.splitlines() if "error" in line or "warning" in line]
    return " / ".join(errors[:3])[:300] or "cargo check failed"


TIERS = ROOT / "tools/gate/tiers.toml"
EXTRA_RATCHETS = (["cargo", "test", "-q", "--test", "block_list_ratchet"], ["cargo", "test", "-q", "-p", "llrm-mir", "--lib", "facts_rewrite"])


# tools/gate/prepush.py runs the gate's cheap static checks (fmt, docs, products, scans, facts, docs_links, the diff check) in about a minute and prints `PREPUSH FAIL: a, b`; its failing steps are
# ratchets of the smoke too (#2735 and #2732 landed with unformatted files that the smoke never looked at).
PREPUSH = ["python3", "tools/gate/prepush.py"]


def prepush_failures(root: Path = ROOT, run=None) -> dict[str, str]:
    run = run or (lambda command: subprocess.run(command, cwd=root, capture_output=True, text=True))
    done = run(PREPUSH)
    lines = [line for line in (done.stdout + done.stderr).splitlines() if line.startswith("PREPUSH FAIL")]
    if not lines and not done.returncode:
        return {}
    names = lines[0].split(":", 1)[1].split(",") if lines else ["did-not-run"]
    return {f"ratchet:prepush-{name.strip()}": " / ".join((done.stdout + done.stderr).strip().splitlines()[-2:])[:240] for name in names if name.strip()}


def warnings_check(root: Path = ROOT, target: Path | None = None) -> str:
    """"" when `cargo check --workspace --all-targets` under `-D warnings` is clean (deadstore.rs landed with unused code). Its own target directory: other RUSTFLAGS would rebuild the shared one."""
    target = target or Path(os.environ.get("LLRM_WARN_TARGET", Path.home() / ".cache/llrm/warn-target"))
    done = subprocess.run(slotted(["cargo", "check", "--workspace", "--all-targets", "-q", "--message-format", "short"]), cwd=root, capture_output=True, text=True, env={**os.environ, "RUSTFLAGS": "-D warnings", "CARGO_TARGET_DIR": str(target)})
    if done.returncode == 0:
        return ""
    return " / ".join([line for line in done.stderr.splitlines() if "error" in line or "warning" in line][:3])[:300] or "cargo check -D warnings failed"


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


def ratchet_name(command: list[str]) -> str:
    """The name a ratchet is listed by in a failure set: the test (`--test NAME`, or the lib filter after `--`), else the filter word."""
    if "--test" in command:
        return command[command.index("--test") + 1]
    return command[-1]


RATCHET_WORKERS = int(os.environ.get("RATCHET_WORKERS", "1"))  # 4 made no gain: each `cargo test -p X --test Y` unifies features differently and rebuilds, the build lock serializes the compiles, and 4 links at load 70 cost 600-750 s a PR


def ratchet_entries(
    command: list[str],
    output: str,
) -> list[str]:
    """The names a failed ratchet is listed by: each test it fails (`ratchet:<name>::<test>`), else the ratchet itself where the output names none. A ratchet is a test
    binary of many tests: listed whole, one red test hid every other test that went red in it."""
    return [f"ratchet:{ratchet_name(command)}::{test}" for test in failed_tests(output)] or [f"ratchet:{ratchet_name(command)}"]


def ratchet_failures(root: Path = ROOT, tiers: Path = TIERS, run=None) -> dict[str, str]:
    """name -> why, for EVERY ratchet that fails on the tree in `root` (the subset rule compares whole failure sets, not the first failure).
    The commands run RATCHET_WORKERS at a time (default 1); the result is in command order."""
    run = run or (lambda command: subprocess.run(slotted(command), cwd=root, capture_output=True, text=True))
    commands = ratchet_commands(tiers)
    with ThreadPoolExecutor(max_workers=RATCHET_WORKERS) as pool:  # read per call: the env and the tests set it
        done = list(pool.map(run, commands))
    failed = {}
    for command, result in zip(commands, done):
        if result.returncode:
            lines = [line for line in (result.stdout + result.stderr).splitlines() if "panicked" in line or "FAILED" in line or "error" in line]
            for name in ratchet_entries(command, result.stdout + result.stderr):
                failed[name] = " / ".join(lines[:2])[:240] or "failed"
    return failed


def docs_failures(root: Path = ROOT, run=None) -> dict[str, str]:
    """`ratchet:docs` -> the generated pages that are out of date (or the figure problem) on the tree in `root`; none when `docsbuild --check` is clean. A header change
    that landed without `docsbuild --sync` (jumps.rs) refused every docs-touching PR after it (#2757, #2763, #2857, #2836) with nothing naming the page."""
    run = run or (lambda command: subprocess.run(command, cwd=root, capture_output=True, text=True))
    done = run([sys.executable, "tools/docs/docsbuild/build.py", "--check"])
    if not done.returncode:
        return {}
    lines = [line.strip() for line in (done.stdout + done.stderr).splitlines() if line.strip()]
    pages = [line for line in lines if line.startswith("docs/")]
    why = f"out of date: {', '.join(pages)}; run python3 tools/docs/docsbuild/build.py --sync" if pages else " / ".join(lines[:2])
    # one name per output line: docsbuild --check was red on the base with 23 lines, and a 24th hid behind the one `ratchet:docs`
    named = {f"ratchet:docs {line.replace(': ', ' - ').replace('; ', ' ')[:140]}": why[:300] for line in lines}
    return named or {"ratchet:docs": why[:300]}


def failing_names(line: str) -> list[str]:
    """The failed step names of a `PARITY SMOKE FAIL name: detail; name: detail | ...` line; a unit step names each failed test (`unit llrm-qb::test_x`)."""
    if not line.startswith("PARITY SMOKE FAIL "):
        return []
    head = line[len("PARITY SMOKE FAIL "):].split(" | ")[0]
    return [part.split(": ")[0].strip() for part in head.split("; ") if part.strip()]


def failed_tests(output: str) -> list[str]:
    """The tests a `cargo test` output lists in its last `failures:` block (the indented names after the stdout of each), once each; the earlier blocks hold the
    tests' own prints, whose indented words are not tests."""
    block = output.split("\nfailures:\n")[-1].split("\ntest result")[0] if "\nfailures:\n" in output else ""
    return list(dict.fromkeys(line.strip() for line in block.splitlines() if re.fullmatch(r"    [\w:]+", line)))


# `unit X::did-not-run` / `integration-did-not-run` -> why: the first error lines of the cargo output, else the exit status and its last 20 lines. A refusal that named only the
# token left #4269's author (refused twice) with nothing to read.
CAUSES: dict[str, str] = {}


def did_not_run_cause(
    output: str,
    returncode: int,
) -> str:
    lines = [line.rstrip() for line in output.splitlines() if line.strip()]
    first = next((at for at, line in enumerate(lines) if re.match(r"(error|thread .* panicked)", line.lstrip())), None)
    shown = lines[first : first + 8] if first is not None else [f"exit status {returncode}", *lines[-20:]]
    return " | ".join(shown)[:600]


def causes_of(names) -> str:
    """` [name: cause; ...]` for the did-not-run names among `names` that have a recorded cause, else an empty string."""
    text = "; ".join(f"{name}: {CAUSES[name]}" for name in sorted(names) if name in CAUSES)
    return f" [{text}]" if text else ""


def panic_reasons(output: str, more: int = 0) -> dict[str, str]:
    """Each test's first panic message from a `cargo test` output: `thread 'NAME' (id) panicked at FILE:L:C:` and the line after it, with a `left:`/`right:` pair appended. A refusal that
    names only the test sent its author to rerun the runner's command to see what failed (#4172's `debug_assert_eq!`, which fires in the debug profile the runner uses and not in release)."""
    found: dict[str, str] = {}
    lines = output.splitlines()
    for at, line in enumerate(lines):
        if not (m := re.match(r"thread '(?P<name>[^']+)'(?: \(\d+\))? panicked at .*:$", line)) or at + 1 >= len(lines):
            continue
        reason = lines[at + 1].strip()
        pair = [one.strip() for one in lines[at + 2 : at + 4] if re.match(r"\s*(left|right):", one)]
        short = m["name"].rsplit("::", 1)[-1]
        # `more` lines of the message that follow, up to a blank line or rustc's `note:`: docs_in_sync prints the stale pages' names there
        extra = []
        for one in lines[at + 2 : at + 2 + more]:
            if not one.strip() or one.startswith("note:"):
                break
            extra.append(one.strip())
        found.setdefault(short, reason + (f" ({', '.join(pair)})" if pair else "") + (f" [{'; '.join(extra)}]" if extra and not pair else ""))
        found.setdefault(m["name"], found[short])
    return found


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
    """None where the compile succeeded; else its first line of complaint. A `-o /dev/null` is an object in a scratch directory: llrm-c encodes only for an `.o` or
    `.obj` output, so the null device never reached the encoder."""
    try:
        with tempfile.TemporaryDirectory(prefix="smoke-") as work:
            command = [str(Path(work) / "out.obj") if after == "-o" and arg == os.devnull else arg for after, arg in zip(["", *command], command)]
            done = cpulimit.run(command, 300, capture_output=True, text=True, env=env)
    except cpulimit.MemoryLimit as error:
        return f"memory limit: {error}"
    except subprocess.TimeoutExpired:
        return f"timeout: 300 s of CPU  [{' '.join(command[:3])}]"
    if done.returncode == 0:
        return None
    lines = [line for line in (done.stderr or done.stdout).splitlines() if line.strip()]
    return f"exit {done.returncode}: {lines[-1][:140] if lines else 'no message'}  [{' '.join(command[:3])}]"


def programs(bins: Path) -> tuple[str, str]:
    jobs = []
    for tool, ext in (("llrm-qb", "bas"), ("llrm-nib", "nib"), (llrmbin.compiler(bins).name, "c")):
        for machine in ("-m16", "-m32"):
            extra = ["-mabi=sysv", "-march=i486"] if tool == llrmbin.compiler(bins).name and machine == "-m32" else []
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
    why = compile_ok([str(llrmbin.compiler(bins)), "-O1", *flags, str(source), "-o", os.devnull], oracle_env())
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
    broken = {level: why for level in ("-O1", "-O2", "-O3", "-Os") if (why := compile_ok([str(llrmbin.compiler(bins)), level, *flags, str(source), "-o", os.devnull], os.environ))}
    if broken:
        first = next(iter(broken))
        return "fail", f"QCport d_faces.c {first}: {broken[first]}", [f"qc_d_faces {level}" for level in broken]
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
    broken = {module: why for module in QC_MODULES if (why := compile_ok([str(llrmbin.compiler(bins)), "-O2", *flags, str(qcport / module), "-o", os.devnull], os.environ))}
    if broken:
        first = next(iter(broken))
        return "fail", f"QCport {first} -O2: {broken[first]}", [f"qc_modules {module}" for module in broken]
    return "ok", f"{len(QC_MODULES)} QCport modules at -O2"


RUN_FAIL = re.compile(r"^FAIL\s+(\S+ \[[^\]]*\])", re.M)


def failing_programs(output: str) -> list[str]:
    """The `run <program> [<flags>]` of each FAIL line of tools/dosbatch/run_tests.py, once each: a red `run` step is listed by the programs it fails."""
    return list(dict.fromkeys(f"run {one}" for one in RUN_FAIL.findall(output)))


# A PR touching one of these changes what a BASIC (or any) program prints without a unit test saying so: log_exp_digits, pow_stack_depth and pow_unrolled went red on
# parity for 12 results and the gate, which ran a dozen programs, did not see it. Such a PR runs every tests/run group (75 s of wall in all, the qb group 6 s).
WHOLE_GROUP_TRIGGERS = (
    "crates/frontends/qb/",
    "crates/frontends/qb/",
    "runtime/qb/",
    "runtime/qb/",
    "crates/target/llrm-x86/",
    "crates/codegen/llrm-codegen/",
)


def runs_whole_groups(files: list[str] | set[str]) -> bool:
    """Whether a PR with these changed files runs every tests/run group in the gate."""
    return any(path.startswith(WHOLE_GROUP_TRIGGERS) for path in files)


def group_failures(
    bins: Path | str,
    run=subprocess.run,
) -> set[str]:
    """The failing `run <program> [<flags>]` of every tests/run group (tools/dosbatch/run_groups.py) on `bins`, a launch that was stopped by its group, or `run-groups-did-not-finish`."""
    done = run([sys.executable, str(ROOT / "tools/dosbatch/run_groups.py")], cwd=ROOT, capture_output=True, text=True, env={**os.environ, "LLRM_BIN": str(bins)})
    output = done.stdout + done.stderr
    names = set(failing_programs(output))
    names |= {f"run {line.split(']')[0][1:]} launch stopped" for line in output.splitlines() if "LAUNCH STOPPED" in line}
    if not names and (done.returncode or not re.search(r"^\[\w+\] \d+ programs: ", output, re.M)):
        names.add("run-groups-did-not-finish")
    return names


def workspace_graph(
    root: Path = ROOT,
    metadata: dict | None = None,
) -> tuple[dict[str, Path], dict[str, set[str]]]:
    """(package -> its directory, package -> the workspace packages it depends on, dev-dependencies included) from `cargo metadata --no-deps`."""
    if metadata is None:
        metadata = json.loads(subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=root, capture_output=True, text=True, check=True).stdout)
    packages = {one["name"]: Path(one["manifest_path"]).parent for one in metadata["packages"]}
    return packages, {one["name"]: {dep["name"] for dep in one["dependencies"]} & packages.keys() for one in metadata["packages"]}


def touched_packages(files: list[str] | set[str], packages: dict[str, Path], root: Path = ROOT) -> set[str]:
    """The packages that own a changed file: the longest package directory above it."""
    owners = {name: directory.relative_to(root).as_posix() for name, directory in packages.items() if directory != root}
    found = set()
    for path in files:
        match = max((name for name, directory in owners.items() if path.startswith(directory + "/")), key=lambda name: len(owners[name]), default=None)
        if match:
            found.add(match)
    return found


def dependants(touched: set[str], graph: dict[str, set[str]]) -> set[str]:
    """`touched` and every package that depends on one of them, through any chain: the lib tests a change can turn red."""
    out = set(touched)
    while grown := {name for name, deps in graph.items() if deps & out} - out:
        out |= grown
    return out


# Lib tests that exist only with a feature, which no layer enabled (llrm-c's `toolchain`: the call-boundary and compile listings; an interrupt handler refused for four days, #2936): the
# features each package's lib run turns on. 12 s on a warm target for llrm-c.
LIB_FEATURES: dict[str, tuple[str, ...]] = {"llrm-c": ("toolchain",)}


def lib_command(package: str, *extra: str) -> list[str]:
    """`cargo test --no-fail-fast -p PACKAGE --lib [--features ...]`: the one command the lib layers and the baseline run."""
    features = LIB_FEATURES.get(package)
    return ["cargo", "test", "--no-fail-fast", "-p", package, "--lib", *(["--features", ",".join(features)] if features else []), *extra]


# The last `dependants_lib_failures` run: crates, tests passed, a hash of the test names, seconds.
LAST_LIB: dict = {}


def dependants_lib_failures(
    packages: set[str],
    root: Path = ROOT,
    run=subprocess.run,
) -> set[str]:
    """The failing `unit <package>::<test>` of every lib test of `packages`, run one package at a time and not stopped at the first red, less the tests tools/xfail.toml lists."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import xfail  # noqa: PLC0415

    failed = set()
    # A listed test that passed: its fix removes its entry, and the list says otherwise (17 entries of llrm-nib, qb and transforms stood after their tests passed or went).
    stale = set()
    listed = xfail.load_for(root)
    started = time.monotonic()
    passed, names = 0, []
    for package in sorted(packages):
        done = run(slotted(lib_command(package)), cwd=root, capture_output=True, text=True)
        output = done.stdout + done.stderr
        found = {f"unit {package}::{test}" for test in failed_tests(output)}
        if done.returncode and not found and "no library targets" not in output and "error: " in output:
            found = {f"unit {package}::did-not-run"}
            CAUSES[f"unit {package}::did-not-run"] = did_not_run_cause(output, done.returncode)
        failed |= found
        stale |= {f"xpass unit {package}::{name}" for name in re.findall(r"^test (\S+) \.\.\. ok$", output, re.M) if f"unit {package}::{name}" in listed}
        passed += sum(int(count) for count in re.findall(r"test result: \w+\. (\d+) passed", output))
        names += [f"{package}::{name}" for name in re.findall(r"^test (\S+) \.\.\. (?:ok|FAILED)$", output, re.M)]
    # What ran, kept for the line the gate prints: a pass that says nothing cannot be told from a gate that never ran.
    LAST_LIB.clear()
    LAST_LIB.update(
        crates=len(packages),
        passed=passed,
        names=hashlib.sha1("\n".join(sorted(names)).encode()).hexdigest()[:8],
        seconds=time.monotonic() - started,
    )
    return (failed - listed.keys()) | stale


# Root integration targets (tests/*.rs) the landing gate does not run per PR, with the reason: the nightly run (tools/gate/nightly_integration.py) does.
NIGHTLY_ONLY = {
    "turbo": "a Turbo Debugger session over a program: 138 s and needs the debugger's DOS image",
    "cv4": "30 s of CodeView debug-info checks",
    "run": "every tests/run program: the tests/run groups run it on the PRs that matter",
}
# Targets that build the runtime's C sources with cc and import no crate: run on a PR that changes runtime/ or tests/qbhost/.
RUNTIME_C = {
    "posix_os": "the POSIX os layer",
    "qb_dev_interface": "the device interface headers",
    "qb_sdl": "the SDL device",
    "qb_dirty": "the dirty-line protocol under ThreadSanitizer (its child lifts testcap's soft data limit)",
    "qb_chk": "the range checks of EXP, ^ and LOG on a host",
    "qb_runtime_pin_clang": "the runtime's C sources through the pinned clang",
}
# Targets that read files, not crates: scans of sources and docs, run on every PR (all together about 2 s).
SCANNERS = {
    "block_list_ratchet": "counts a pattern in sources",
    "build_script_reruns": "reads build scripts",
    "core_depends_on_no_target": "reads crates' manifests",
    "core_fixtures_are_for_core_tests": "reads test fixtures",
    "descriptions_are_read": "reads the target descriptions",
    "docs_in_sync": "regenerates the doc pages and compares",
    "docs_links": "reads markdown",
    "dosbox_progress": "reads the harness",
    "no_kill_by_pattern": "scans scripts",
    "no_partial_ecx_counts": "scans the 32-bit runtime assembly",
    "selectiondag_registry": "reads the SD registry in the plan and every source's SD numbers",
    "no_local_paths": "scans docs",
    "target_facts": "reads the target descriptions' facts",
    "no_todo": "scans sources",
    "tracked_build_products": "lists tracked files",
}
TOOL_COMPILERS = ("llrm-cc", "llrm-llc", "llrm-qb", "llrm-nib", "nibfront")  # what tools/dosbatch and the run_tests scripts build programs with


def with_common(
    text: str,
    root: Path = ROOT,
) -> str:
    """`text` of a target and, if it says `mod common;`, the text of tests/common/mod.rs and of each `common::NAME` submodule it names: what a target reaches through the helper."""
    if not re.search(r"^\s*(?:pub\s+)?mod common;", text, re.M):
        return text
    common = root / "tests" / "common"
    files = [common / "mod.rs", *(common / f"{name}.rs" for name in set(re.findall(r"\bcommon::(\w+)", text)))]
    return "\n".join([text, *(one.read_text(errors="replace") for one in files if one.is_file())])


def integration_map(
    root: Path = ROOT,
    metadata: dict | None = None,
) -> dict[str, set[str]]:
    """target -> the workspace packages whose change can change it, derived and not listed: the crates its source imports (`llrm_x::`), the crates each compiler it
    runs (`CARGO_BIN_EXE_<bin>`) imports, all four compilers where it drives tools/dosbatch; each with its dependencies, to any depth. A target with no such crate and no
    entry in SCANNERS is unmapped (test_every_integration_target_is_mapped refuses it)."""
    if metadata is None:
        metadata = json.loads(subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=root, capture_output=True, text=True, check=True).stdout)
    packages, graph = workspace_graph(root, metadata)
    pattern = re.compile(r"\b(llrm_[a-z0-9_]+|qbfront)::")
    named = lambda text: {name.replace("_", "-") for name in pattern.findall(text)} & packages.keys()  # noqa: E731
    bins = {target["name"]: named(Path(target["src_path"]).read_text()) for one in metadata["packages"] if one["name"] == "llrm" for target in one["targets"] if target["kind"] == ["bin"]}
    out = {}
    for source in sorted((root / "tests").glob("*.rs")):
        text = with_common(source.read_text(errors="replace"), root)
        used = named(text)
        for name in set(re.findall(r"CARGO_BIN_EXE_([\w-]+)", text)):
            used |= {name} & bins.keys() and bins[name]
        if source.stem == "check":
            # tests/check.rs runs whichever tool a file's RUN line names, from beside llrm-qb: the compilers of its RUN lines, not only the one CARGO_BIN_EXE names
            # (copy_idioms.c, which drives llrm-c, went red on parity unseen: a change to llrm-c ran no check).
            for case in (root / "tests" / "check").rglob("*"):
                for tool in re.findall(r"RUN:\s*(?:not\s+)?([\w-]+)", case.read_text(errors="replace") if case.is_file() else ""):
                    used |= bins.get(tool, set())
        if "tools/dosbatch" in text or "run_tests" in text:
            used |= set().union(*(bins.get(name, set()) for name in TOOL_COMPILERS))
        reach = set(used)
        while grown := {dep for one in reach for dep in graph.get(one, set())} - reach:
            reach |= grown
        out[source.stem] = reach
    return out


def unmapped_targets(mapping: dict[str, set[str]]) -> list[str]:
    """The targets with no crate behind them that SCANNERS and NIGHTLY_ONLY do not name."""
    return sorted(name for name, reach in mapping.items() if not reach and name not in SCANNERS and name not in NIGHTLY_ONLY and name not in RUNTIME_C)


def integration_targets_for(
    files: list[str] | set[str],
    touched: set[str],
    mapping: dict[str, set[str]],
) -> list[str]:
    """The root integration targets a PR runs: those whose crates include a touched one, every scanner, and a target whose own tests/NAME.rs changed; never NIGHTLY_ONLY."""
    own = {Path(path).stem for path in files if path.startswith("tests/") and path.endswith(".rs") and "/" not in path[len("tests/"):]}
    chosen = {name for name, reach in mapping.items() if reach & touched} | set(SCANNERS) | own
    if any(path.startswith(("runtime/", "tests/qbhost/")) for path in files):
        chosen |= set(RUNTIME_C)
    return sorted(name for name in chosen if name in mapping and name not in NIGHTLY_ONLY)


def test_outcomes(output: str, status: str) -> list[str]:
    """`integration <target>::<test>` of each `test NAME ... STATUS` line, by the `Running tests/TARGET.rs` line above it."""
    target, found = "?", []
    for line in output.splitlines():
        if match := re.match(r"\s+Running (?:unittests )?(?:tests/)?(\w+?)(?:\.rs)?\s", line):
            target = match[1]
        if match := re.match(rf"test (?!result:)(\S+) \.\.\. {status}", line):
            found.append(f"integration {target}::{match[1]}")
    return sorted(dict.fromkeys(found))


def red_tests(output: str) -> list[str]:
    """`integration <target>::<test>` of each `test NAME ... FAILED` line, by the `Running tests/TARGET.rs` line above it. The `test result: FAILED. N passed; M failed`
    summary line of each binary is not a test: counting it read 94 red where 63 were."""
    target, found = "?", []
    for line in output.splitlines():
        if match := re.match(r"\s+Running (?:unittests )?(?:tests/)?(\w+?)(?:\.rs)?\s", line):
            target = match[1]
        if match := re.match(r"test (?!result:)(\S+) \.\.\. FAILED", line):
            found.append(f"integration {target}::{match[1]}")
    return sorted(dict.fromkeys(found))


def integration_failures(
    targets: list[str],
    root: Path = ROOT,
    run=subprocess.run,
) -> set[str]:
    """The failing `integration <target>::<test>` of the root crate's `targets`, less the tests tools/xfail.toml lists; `integration-did-not-run` if nothing ran."""
    if not targets:
        return set()
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import xfail  # noqa: PLC0415

    command = ["cargo", "test", "--release", "--no-fail-fast", "-p", "llrm", *[arg for name in targets for arg in ("--test", name)]]
    # one stream: cargo prints `Running tests/X.rs` on stderr and the test lines on stdout, and stdout + stderr read as two blocks lost which target a test belonged to
    done = run(slotted(command), cwd=root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    found = set(red_tests(done.stdout))
    if done.returncode and not found:
        found = {"integration-did-not-run"}
        CAUSES["integration-did-not-run"] = did_not_run_cause(done.stdout, done.returncode)
    reasons = panic_reasons(done.stdout, more=8)
    for one in found:
        short = one.split("::", 1)[-1]
        if short in reasons:
            CAUSES[one] = reasons[short][:400]
    listed = xfail.load_for(root).keys()
    # a listed test that passes is as much a gate failure as an unlisted one that fails: its fix removes its entry
    passing = {f"xpass {one}" for one in test_outcomes(done.stdout, "ok") if one in listed}
    return (found - listed) | passing


def qbrt_failures(
    bins: Path | str,
    run=subprocess.run,
) -> set[str]:
    """The failing `qbrt <program> <target>` of the whole tests/qbrt differential (tools/dosbatch/qbrt_run.py --all: 218 results, 67 s at load 7) on `bins`, a program that
    did not compile, or `qbrt-did-not-finish`."""
    work = Path(tempfile.mkdtemp(prefix="qbrt-gate-"))
    done = run([sys.executable, str(ROOT / "tools/dosbatch/qbrt_run.py"), str(work), "--all"], cwd=ROOT, capture_output=True, text=True, env={**os.environ, "LLRM_BIN": str(bins)})
    output = done.stdout + done.stderr
    names = {f"qbrt {line.split()[1]} {line.split()[2]}" for line in output.splitlines() if line.startswith("NOTPASS ")}
    names |= {f"qbrt {line.split()[1]} did-not-compile" for line in output.splitlines() if line.startswith("COMPILE-FAIL ")}
    if not names and (done.returncode or "RESULT " not in output):
        names.add("qbrt-did-not-finish")
    return names


def touches_the_compiler(files: list[str] | set[str]) -> bool:
    """Whether a PR changes a crate: any of them can change what a QCport module compiles to, and QCport's `item` stopped compiling at four levels on a mid-end PR
    (#3422) the gate passed, because it compiled four QCport modules at -O2."""
    return any(path.startswith("crates/") for path in files)


def qcport_failures(
    bins: Path | str,
    sweep=None,
) -> set[str]:
    """The failing `qc <module> <level>` of every QCport module (all 65) at all four levels on `bins`: 260 compiles, 3 s of wall at 8 at a time. Nothing when the QCport sources are
    not on this host (the `qc_modules` step says so)."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    sys.path.insert(0, str(Path(__file__).resolve().parent / "dosbatch"))
    import qcport_sweep  # noqa: PLC0415

    qcport = Path(os.environ.get("QCPORT", Path.home() / "scratch/qcport/src")).expanduser()
    include = Path(os.environ.get("QCPORT_INC", Path.home() / "scratch/qctc/inc")).expanduser()
    if not qcport.is_dir() or not include.is_dir():
        return set()
    return {f"qc {module} {level}" for module, level, _ in (sweep or qcport_sweep.sweep)(llrmbin.compiler(Path(bins)), qcport, include)}


def run_cases(bins: Path) -> tuple[str, str]:
    names = selection().get("run", {}).get("cases", [])
    if not names:
        return "skipped", "no tests/run cases chosen (parity_smoke.toml [run] cases)"
    env = {**os.environ, "LLRM_BIN": str(bins)}
    done = subprocess.run([sys.executable, str(ROOT / "tools/dosbatch/run_tests.py"), *names], cwd=ROOT, capture_output=True, text=True, env=env)
    if done.returncode:
        lines = [line for line in (done.stdout + done.stderr).splitlines() if line.strip()]
        return "fail", " / ".join(lines[-3:])[:300], failing_programs(done.stdout + done.stderr)
    return "ok", f"{len(names)} programs"


def unit_step(package: str, allowance: float):
    def step(bins: Path) -> tuple[str, str]:
        ranking = selection().get("units", {})
        names = unit_tests(package, allowance, {k: [(n, s) for n, s in v.get("tests", [])] for k, v in ranking.items()})
        command = ["cargo", "test", "-q", "--lib", "-p", package] + (["--", "--exact", *names] if names else [])
        # an unranked crate runs its whole lib, cut by wall time: one test thread keeps that wall time the CPU it is charged (32 threads ran 32x the budget in it)
        env = None if names else {**os.environ, "RUST_TEST_THREADS": "1"}
        code, output = run_cut(slotted(command), allowance + CARGO_OVERHEAD, ROOT, env=env)
        if code is None:
            return "ok", f"{package}: no ranking yet, cut at {allowance:.1f} s without a failure"
        listed = xfail.load_for(ROOT)
        failed = {f"unit {package}::{test}" for test in failed_tests(output)}
        ran = {f"unit {package}::{name}" for name in names} if names else {one for one in listed if one.startswith(f"unit {package}::")}
        unexpected, expected, passing = xfail.classify(failed, ran, listed)
        if passing:
            return "fail", f"{package}: listed in xfail.toml but passes (remove the entry): {', '.join(sorted(passing))}"[:300], sorted(f"xpass {one[5:]}" for one in passing)
        if code and (unexpected or not failed):
            lines = [line for line in output.splitlines() if "FAILED" in line or "panicked" in line or "error" in line]
            return "fail", f"{package}: {' / '.join(lines[:3])[:300] or 'cargo test failed'}", sorted(unexpected)
        note = f", {len(expected)} expected failures (xfail.toml)" if expected else ""
        return "ok", f"{package}: {len(names) or 'all'} tests{note}"
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


def run_steps(steps, bins: Path, clock=cpu, budget: float = BUDGET, known_red: frozenset[str] = frozenset(), meter=None, limit: int | None = None) -> tuple[bool, str]:
    """(passed, the last line). The clock is the test's, which waits on virtual time. A step named in `known_red` is not run and is said to be skipped.
    The budget is the steps' user-space instructions against `limit` (INSTRUCTIONS) where perf counted them and a limit is set; else the CPU seconds, as before:
    a loaded host stretches the CPU seconds of the same work (a tools-only PR failed 16.1 s > 15 s at load 32 on 32 cores), not its instructions."""
    limit = INSTRUCTIONS if limit is None else limit
    started, wall, parts, failed, skipped = clock(), time.monotonic(), [], [], []
    with (meter or Meter)() as counted:
        for name, step, _ in steps:
            began = clock()
            stage(f"step {name}")
            verdict, detail, *tests = ("skipped", "known red, not run") if name in known_red else step(bins)
            parts.append(f"{name} {verdict} {clock() - began:.1f} s")
            if verdict == "fail":
                # a step that names the tests it failed is listed by them: the subset rule compares test names, not crates
                failed.extend(f"{one}: {detail}" for one in (tests[0] if tests and tests[0] else [name]))
            elif verdict == "skipped":
                skipped.append(f"{name}: {detail}")
    spent, instructions = clock() - started, counted.value
    if instructions is not None and limit:
        measure = f"{instructions / 1e9:.2f} G instr of {limit / 1e9:.2f} G ({spent:.1f} s CPU for reference)"
        if instructions > limit:
            failed.append(f"over the budget: {instructions / 1e9:.2f} G instructions > {limit / 1e9:.2f} G")
    else:
        measure = f"{spent:.1f} s CPU of {budget:.0f} s" + ("" if instructions is None else f" ({instructions / 1e9:.2f} G instr, no limit set)") + (" (perf unavailable)" if instructions is None else "")
        if spent > budget:
            failed.append(f"over the budget: {spent:.1f} s > {budget:.0f} s")
    line = f"{' | '.join(parts)} | {measure} ({time.monotonic() - wall:.1f} s wall)" + (f" | skipped {'; '.join(skipped)}" if skipped else "")
    if failed:
        return False, f"PARITY SMOKE FAIL {'; '.join(failed)} | {line}"
    return True, f"PARITY SMOKE PASS {line}"


def run_twice_over_budget(run) -> tuple[bool, str]:
    """`run()` -> (passed, line); when the ONLY failure is the CPU budget it runs once more and takes that verdict: on a host at load 20-60 the CPU seconds of the same
    steps stretch (cache and hyper-thread contention), and a tools-only PR failed 'over the budget' with no unit test in the set."""
    ok, line = run()
    if not ok and failing_names(line) == ["over the budget"]:
        ok, line = run()
        line = line.replace("PARITY SMOKE", "PARITY SMOKE (second run, the first was over the budget)", 1)
    return ok, line


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
    failing: list[str] = []
    pre: list[str] = []
    if broken := workspace_check():
        failing.append("workspace")
        pre.append(f"workspace: {broken}")
    for name, why in {**ratchet_failures(), **prepush_failures(), **docs_failures()}.items():
        failing.append(name)
        pre.append(f"{name}: {why}")
    if broken := warnings_check():
        failing.append("warnings")
        pre.append(f"warnings: {broken}")
    packages = touched(args.base)
    for package in packages:  # built first, untimed
        subprocess.run(slotted(["cargo", "test", "-q", "--lib", "-p", package, "--no-run"]), cwd=ROOT, capture_output=True)
    ok, line = run_twice_over_budget(lambda: run_steps(plan(packages), args.bin or llrmbin.bin_dir(), known_red=frozenset(args.skip)))
    failing += failing_names(line.replace("PARITY SMOKE (second run, the first was over the budget)", "PARITY SMOKE"))
    print(f"FAILING: {','.join(failing)}")
    if pre:
        print(f"PARITY SMOKE FAIL {'; '.join(pre)} | {line}")
        return 1
    print(line)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
