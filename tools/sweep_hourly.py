#!/usr/bin/env python3
"""The hourly post-landing sweep of the feat/parity tip, and the one line it reports.

    python3 tools/sweep_hourly.py [--tip SHA] [--no-build] [--bisect-builds N] [--state FILE]

Builds the tip (a detached worktree, release profile, the target ~/scratch/sb-head3), checks the binaries' own stamp and
prints it with their md5s, then runs: tests/run ALL groups (`run_groups.py`), the qbrt differential on every program at m16 and
m32 (`qbrt_run.py --all`), the QCport sweep (65 modules at four levels, compile failures: `qcport_sweep.py`), and NIBBLES and
GORILLA compiled and played under DOSBox-X against BCOM45 (`test_qbdemos.py`). Every red has a name; the state file keeps the
names of the previous run, so the line says which are new. For each new red the merges between the previous tip and this one
(first parent, the PR in each subject) are named, and with `--bisect-builds N` (default 6) a red that a probe can ask is bisected
over them, one build per step, to the one merge that made it. A new WRONG OUTPUT (a program that runs and differs) is named first.

The line: `sweep SHA9 PASS|RED stamp=... md5 ... | counts | new: ... | first-bad: ...`. Exit 0 on PASS, 1 on RED.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import pidfile  # noqa: E402
import stamp  # noqa: E402

BRANCH = os.environ.get("LAND_BRANCH", "release/next")  # the integration branch (tools/landing/branch.sh)

SCRATCH = Path(os.environ.get("SCRATCH", Path.home() / "scratch"))
TARGET = SCRATCH / "sb-head3"
WORKTREE = SCRATCH / "wt-sweep-tip"
STATE = SCRATCH / "w8-logs" / "sweep-hourly-state.json"
LOGS = SCRATCH / "results" / "sweep-hourly"
DEMOS = SCRATCH / "gor"
BINARIES = ("llrm-cc", "llrm-llc", "llrm-qb", "llrm-nib", "nibfront")
WRONG = "wrong-output"
BUILD = "build-failure"
COMPILE = "compile"
OTHER = "other"


@dataclass
class Red:
    """One thing that is red: a stable name, what kind of red, and how to ask about it at another build."""

    name: str
    kind: str
    probe: list[str] = field(default_factory=list)


def parse_inventory(text: str) -> tuple[list[Red], str]:
    """The reds of `test_inventory.py --run`: a `package/target: N s, RED` line is a test target the landing gate never runs failing (`greedy_inline` sat red a day), and
    `inventory: N unrun targets run, M red` the count."""
    reds = [Red(f"unrun:{m[1]}", OTHER, ["test_inventory.py", m[1]]) for line in text.splitlines() if (m := re.match(r"(\S+): [\d.]+ s, RED$", line))]
    count = re.search(r"inventory: (\d+) unrun targets run, (\d+) red", text)
    return reds, f"{int(count[1]) - int(count[2])}/{count[1]}" if count else "0/0"


def parse_groups(text: str) -> tuple[list[Red], str]:
    """The reds of `run_groups.py` output, and `passed/total` programs."""
    reds: list[Red] = []
    passed = total = 0
    for line in text.splitlines():
        if (m := re.match(r"\[(\S+)\] (\d+) programs: (\d+) pass, (\d+) known, (\d+) fail(.*)", line)):
            total += int(m[2])
            passed += int(m[3]) + int(m[4])
            if "LAUNCH STOPPED" in m[6]:
                reds.append(Red(f"run:{m[1]}:launch-stopped", OTHER, ["dosbatch/run_groups.py", m[1]]))
        elif (m := re.match(r"\[(\S+)\] .*: no summary", line)):
            reds.append(Red(f"run:{m[1]}:no-summary", OTHER, ["dosbatch/run_groups.py", m[1]]))
        elif line.startswith("FAIL"):
            words = line.split()
            program = words[1].rstrip(":") if len(words) > 1 else line
            # `FAIL  c/x.c [-O2]: link: jwlink: Error! ...`: the program never ran, so its output is not wrong (run_groups prints `link:` or `compile:` for a build that failed).
            built = not re.search(r"\]: (link|compile): ", line)
            reds.append(Red(f"run:{program}", WRONG if built else BUILD, ["dosbatch/run_tests.py", Path(program).stem]))
    return reds, f"{passed}/{total}"


def parse_qbrt(text: str) -> tuple[list[Red], str]:
    """The reds of `qbrt_run.py --all` output, and `passed/total` runs."""
    reds: list[Red] = []
    result = "?"
    for line in text.splitlines():
        words = line.split()
        if line.startswith("NOTPASS") and len(words) >= 3:
            # `NOTPASS name target REFERENCE CANDIDATE why`: a candidate that was "not built" never ran, so its output is not wrong; it did not link.
            built = " ".join(words[4:6]) != "not built"
            reds.append(Red(f"qbrt:{words[1]}:{words[2]}", WRONG if built else BUILD, ["dosbatch/qbrt_run.py", "--only", words[1]]))
        elif line.startswith("COMPILE-FAIL") and len(words) >= 2:
            reds.append(Red(f"qbrt:{words[1]}:compile", COMPILE, ["dosbatch/qbrt_run.py", "--only", words[1]]))
        elif (m := re.match(r"RESULT (\d+) of (\d+) pass", line)):
            result = f"{m[1]}/{m[2]}"
    if result == "?":
        # The step died before it counted (a DOSBox job that never ended, a traceback): not a count of "?" but a red with the reason, as the 01:00 sweep of 2026-10-24 printed `qbrt ?`
        last = next((line.strip() for line in reversed(text.splitlines()) if line.strip()), "no output")
        reds.append(Red(f"qbrt:no-result: {last[:120]}", OTHER, ["dosbatch/qbrt_run.py", "--all"]))
    return reds, result


def parse_qcport(text: str) -> tuple[list[Red], str]:
    """The reds of `qcport_sweep.py` output (a NEW kind is red; a KNOWN one is the issue's), and `failed/compiles`."""
    reds: list[Red] = []
    count = "?"
    for line in text.splitlines():
        if (m := re.search(r"(\d+) modules x (\d+) levels \+ (\d+) front-end compiles.*?(\d+) failed compiles", line)):
            count = f"{m[4]} failed/{int(m[1]) * int(m[2]) + int(m[3])}"
        elif line.strip().startswith("NEW:"):
            text = re.sub(r"\s+", " ", line.strip()[4:]).strip()
            why, _, where = text.partition(" -- ")
            first = re.match(r"(\S+) (-m\d+) (-O\w+)", where)
            # A program of the front-end set can be asked again at another build; a QCport module (a file outside the tree) cannot.
            probe = ["qcport_probe.py", first[1], first[2], first[3], why[:100]] if first and "." in first[1] else []
            reds.append(Red("qcport:" + text[:100], COMPILE, probe))
    return reds, count


def parse_demos(text: str) -> tuple[list[Red], str]:
    """The reds of the demo tests (pytest -q output), and `passed/total`."""
    reds = [Red(f"demo:{m[1]}", WRONG) for m in re.finditer(r"FAILED \S+\[(\w+)\]", text)]
    passed = re.search(r"(\d+) passed", text)
    failed = re.search(r"(\d+) failed", text)
    total = int(passed[1] if passed else 0) + int(failed[1] if failed else 0)
    return reds, f"{passed[1] if passed else 0}/{total}"


def merges(
    old: str | None,
    new: str,
    cwd: Path = ROOT,
) -> list[tuple[str, str]]:
    """(sha, subject) of the first-parent commits after `old` up to `new`, oldest first; the newest alone without `old`."""
    spec = f"{old}..{new}" if old else f"{new}~1..{new}"
    out = subprocess.run(
        ["git", "log", "--first-parent", "--reverse", "--format=%H\t%s", spec], cwd=cwd, capture_output=True, text=True, check=True
    ).stdout
    return [(line.split("\t")[0], line.split("\t", 1)[1]) for line in out.splitlines() if line]


def pull_request(subject: str) -> str:
    """`#1234` of a squash subject `... (#1234)`, else the subject's first words."""
    found = re.findall(r"\(#(\d+)\)", subject)
    return f"#{found[-1]}" if found else subject[:40]


def newly_red(
    reds: list[Red],
    previous: dict[str, str],
) -> list[Red]:
    """The reds that were not red at the previous run."""
    return [one for one in reds if one.name not in previous]


def bisect(
    range_: list[tuple[str, str]],
    asks_red: "callable",
    budget: int,
) -> tuple[int, int] | None:
    """The index of the first merge for which `asks_red(sha)` holds, assuming none before it does and the last does: None
    when the budget of builds runs out first. Answers None (could not be asked) are skipped upward, as a build that fails."""
    lo, hi = -1, len(range_) - 1
    spent = 0
    while hi - lo > 1:
        mid = (lo + hi) // 2
        if spent >= budget:
            return None
        spent += 1
        answer = asks_red(range_[mid][0])
        while answer is None and mid + 1 < hi:
            mid += 1
            if spent >= budget:
                return None
            spent += 1
            answer = asks_red(range_[mid][0])
        if answer is None:
            return None
        if answer:
            hi = mid
        else:
            lo = mid
    return hi, spent


def first_bad(
    red: Red,
    range_: list[tuple[str, str]],
    budget: int,
    ask: "callable | None" = None,
) -> str:
    """Which merge made `red`: its PR, or the PRs of the range where the probe cannot be asked or the budget ends."""
    names = ", ".join(pull_request(subject) for _, subject in range_)
    if len(range_) <= 1:
        return names or "unknown"
    if not red.probe or not budget or ask is None:
        return f"one of {names}"
    found = bisect(range_, lambda sha: ask(red, sha), budget)
    if found is None:
        return f"one of {names}"
    at, _ = found
    return pull_request(range_[at][1])


def stamps(directory: Path) -> tuple[str, str]:
    """(the build's sha, `llrm-llc=md5 ...`) of the binaries in `directory`; refuses a binary with no stamp."""
    sha = stamp.check(directory / "llrm-llc", label="sweep")
    md5s = " ".join(f"{name}={hashlib.md5((directory / name).read_bytes()).hexdigest()[:8]}" for name in BINARIES if (directory / name).exists())
    return sha, md5s


def line(
    tip: str,
    sha: str,
    md5s: str,
    counts: dict[str, str],
    reds: list[Red],
    new: list[Red],
    bad: dict[str, str],
) -> str:
    """The one line of the report."""
    wrong = [one for one in new if one.kind == WRONG]
    broken = [one for one in new if one.kind == BUILD]
    rest = [one for one in new if one.kind not in (WRONG, BUILD)]
    state = "PASS" if not reds else f"RED {len(reds)}"
    shown = ", ".join(f"WRONG OUTPUT {one.name} ({bad.get(one.name, '?')})" for one in wrong)
    shown += (", " if shown and broken else "") + ", ".join(f"BUILD FAILURE {one.name} ({bad.get(one.name, '?')})" for one in broken)
    shown += (", " if shown and rest else "") + ", ".join(f"{one.name} ({bad.get(one.name, '?')})" for one in rest)
    return (
        f"sweep {tip[:9]} {state} stamp={sha[:9]} md5 {md5s} | "
        + " ".join(f"{key} {value}" for key, value in counts.items())
        + f" | new: {shown or 'none'}"
    )


def load_state(path: Path) -> dict:
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return {"tip": None, "reds": {}}


def save_state(
    path: Path,
    tip: str,
    reds: list[Red],
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({"tip": tip, "reds": {one.name: one.kind for one in reds}}, indent=1, sort_keys=True))


def run(
    argv: list[str],
    env: dict[str, str],
    log: Path,
    cwd: Path,
) -> str:
    """The output of a step, kept in `log`; the step's own failure is its reds, not an exception."""
    # A slot like a cargo build: the steps ran beside four builds at load 68 on 32 cores.
    slot = Path.home() / ".local/bin/buildslot"
    done = subprocess.run(["nice", "-n", "10", str(slot), *argv], cwd=cwd, env=env, capture_output=True, text=True)
    text = done.stdout + done.stderr
    log.parent.mkdir(parents=True, exist_ok=True)
    log.write_text(text)
    return text


def build(
    tip: str,
    target: Path = TARGET,
    worktree: Path = WORKTREE,
) -> Path:
    """The release binaries of `tip`, in `target/release`; the worktree is detached at the tip and this script's alone."""
    if not worktree.exists():
        subprocess.run(["git", "worktree", "add", "-q", "--detach", str(worktree), tip], cwd=ROOT, check=True)
    subprocess.run(["git", "checkout", "-q", "--detach", tip], cwd=worktree, check=True)
    env = {k: v for k, v in os.environ.items() if k != "RUSTFLAGS"} | {"CARGO_INCREMENTAL": "0", "CARGO_TARGET_DIR": str(target)}
    buildslot = Path.home() / ".local/bin/buildslot"
    subprocess.run(["nice", "-n", "10", str(buildslot), "cargo", "build", "--release", "-q", "--bins"], cwd=worktree, env=env, check=True)
    binaries = target / "release"
    for name in BINARIES:
        (binaries / name).touch()  # the runners refuse a binary older than its sources
    return binaries


def steps(
    binaries: Path,
    worktree: Path,
    logs: Path,
) -> tuple[list[Red], dict[str, str]]:
    """Every step's reds and counts."""
    env = os.environ | {"LLRM_BIN": str(binaries), "QB45_DEMOS_DIR": str(DEMOS)}
    tools = worktree / "tools"
    reds: list[Red] = []
    counts: dict[str, str] = {}
    work = logs / "qbrt-work"
    work.mkdir(parents=True, exist_ok=True)
    found, count = parse_groups(run([sys.executable, str(tools / "dosbatch/run_groups.py")], env, logs / "groups.log", worktree))
    reds += found
    counts["run"] = count
    found, count = parse_qbrt(run([sys.executable, str(tools / "dosbatch/qbrt_run.py"), "--all", str(work)], env, logs / "qbrt.log", worktree))
    reds += found
    counts["qbrt"] = count
    found, count = parse_qcport(run([sys.executable, str(tools / "qcport_sweep.py"), "--bin", str(binaries)], env, logs / "qcport.log", worktree))
    reds += found
    counts["qcport"] = count
    demos = [
        "uv", "run", "-q", "--project", str(tools), "python", "-m", "pytest", str(tools / "dosbatch/test_qbdemos.py"),
        "-q", "-p", "no:cacheprovider", "-m", "slow_dos",
    ]
    found, count = parse_demos(run(demos, env, logs / "demos.log", worktree))
    reds += found
    counts["demos"] = count
    # the test targets the landing gate never runs (tests/*.rs of every package, bins' unit tests): its lib-only layer let greedy_inline sit red for a day
    cargo = {k: v for k, v in env.items() if k != "RUSTFLAGS"} | {"CARGO_INCREMENTAL": "0", "CARGO_TARGET_DIR": str(TARGET)}
    build_json = logs / "tests-build.json"
    run(["nice", "-n", "10", str(Path.home() / ".local/bin/buildslot"), "cargo", "test", "--release", "--workspace", "--no-run", "--message-format=json"], cargo, build_json, worktree)
    found, count = parse_inventory(run([sys.executable, str(tools / "test_inventory.py"), str(build_json), "--run", "--skip", "turbo,cv4,run"], cargo, logs / "inventory.log", worktree))
    reds += found
    counts["unrun"] = count
    return reds, counts


RUNNING = Path(os.environ.get("SWEEP_PIDFILE") or LOGS / "running.pid")  # the loop waits on this pid, not on a command-line pattern


def main(argv: list[str]) -> int:
    """One sweep, with its pid recorded for the loop that waits for it."""
    with pidfile.recorded(RUNNING):
        return sweep(argv)


def sweep(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tip", help="the commit to sweep (default: origin/feat/parity after a fetch)")
    parser.add_argument("--no-build", action="store_true", help="use the binaries already in the target")
    parser.add_argument("--bisect-builds", type=int, default=6)
    parser.add_argument("--state", type=Path, default=STATE)
    args = parser.parse_args(argv)
    subprocess.run(["git", "fetch", "-q", "origin", BRANCH], cwd=ROOT, check=True)
    tip = subprocess.run(["git", "rev-parse", args.tip or f"origin/{BRANCH}"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    binaries = TARGET / "release" if args.no_build else build(tip)
    sha, md5s = stamps(binaries)
    # The run is of the binary's own commit, whatever the ref says now.
    logs = LOGS / sha[:9]
    reds, counts = steps(binaries, WORKTREE, logs)
    previous = load_state(args.state)
    new = newly_red(reds, previous["reds"])
    range_ = merges(previous["tip"], sha) if new else []
    bad = {one.name: first_bad(one, range_, args.bisect_builds, ask_at) for one in new}
    print(line(tip, sha, md5s, counts, reds, new, bad))
    save_state(args.state, sha, reds)
    return 1 if reds else 0


def ask_at(
    red: Red,
    sha: str,
) -> bool | None:
    """Whether `red` holds at the merge `sha`: its probe run with that commit's release binaries (built once, kept as
    ~/scratch/sbp-SHA9)."""
    binaries = SCRATCH / f"sbp-{sha[:9]}" / "release"
    if not (binaries / "llrm-llc").exists():
        worktree = SCRATCH / f"wt-sbp-{sha[:9]}"
        if not worktree.exists():
            subprocess.run(["git", "worktree", "add", "-q", "--detach", str(worktree), sha], cwd=ROOT, check=True)
        env = {k: v for k, v in os.environ.items() if k != "RUSTFLAGS"} | {"CARGO_INCREMENTAL": "0", "CARGO_TARGET_DIR": str(SCRATCH / f"sbp-{sha[:9]}")}
        built = subprocess.run(
            ["nice", "-n", "10", str(Path.home() / ".local/bin/buildslot"), "cargo", "build", "--release", "-q", "--bins"],
            cwd=worktree, env=env, capture_output=True,
        )
        if built.returncode != 0:
            return None
    env = os.environ | {"LLRM_BIN": str(binaries), "QB45_DEMOS_DIR": str(DEMOS)}
    script = ROOT / "tools" / red.probe[0]
    arguments = red.probe[1:]
    if script.name == "qbrt_run.py":  # its first argument is the work directory: without one every probe is a usage error, and a usage error read as red
        work = SCRATCH / f"qbrt-probe-{sha[:9]}"
        work.mkdir(parents=True, exist_ok=True)
        arguments = [str(work), *arguments]
    done = subprocess.run([sys.executable, str(script), *arguments], cwd=ROOT, env=env, capture_output=True, text=True)
    text = done.stdout + done.stderr
    if done.returncode in (2, 3):  # a usage error, or the probe has no such job at that commit: not asked
        return None
    return "FAIL" in text or "NOTPASS" in text or done.returncode != 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
