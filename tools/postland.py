#!/usr/bin/env python3
"""The post-landing loop: per new head of feat/parity, run the checks once, find what failed that the previous head did not, and name the PR that did it.

    tools/postland.py once [SHA]      check SHA (default origin/feat/parity) against the last checked head; report to $POSTLAND/<sha>.md
    tools/postland.py loop            `once` for each new head, polling

A failure is NEW when it is neither in the previous head's failure set nor on the XFAIL list. A new failure is bisected over the first-parent commits
between the two heads (one squash per landed PR); the culprit's `Owner:` line is the owner. Nothing is reverted here.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
from collections.abc import Callable
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import llrmbin  # noqa: E402
import pairruns  # noqa: E402
import parity_smoke  # noqa: E402
import xfail  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
HOME = Path(os.environ.get("POSTLAND", Path.home() / "scratch" / "postland"))
TOOLS = Path(os.environ.get("POSTLAND_TOOLS", Path.home() / "scratch" / "sb-head3" / "dist"))  # jwasm, jwlink, dosbox-x
BINS = ("llrm-cc", "llrm-llc", "llrm-qb", "llrm-nib", "nibfront")
BUILD = ["--profile", "dist", "-q", *(arg for one in BINS for arg in ("--bin", one))]
LOW = ["nice", "-n", "19"]


def new_failures(failing: set[str], previous: set[str], listed: dict[str, dict]) -> set[str]:
    """Failing now, failing neither on the previous head nor on the XFAIL list."""
    return failing - previous - listed.keys()


def culprit(commits: list[str], fails: Callable[[str], bool]) -> str | None:
    """The first of `commits` (oldest first, the last one known to fail) where `fails` holds, by bisection; None when the last does not fail."""
    if not commits or not fails(commits[-1]):
        return None
    low, high = 0, len(commits) - 1
    while low < high:
        middle = (low + high) // 2
        if fails(commits[middle]):
            high = middle
        else:
            low = middle + 1
    return commits[low]


def pr_of(subject: str) -> int | None:
    """The PR number a squash subject ends with: `fix(x): y (#123)`."""
    found = re.search(r"\(#(\d+)\)\s*$", subject)
    return int(found[1]) if found else None


def owner_of(body: str) -> str:
    """The session named by the PR body's `Owner:` line."""
    found = re.search(r"^Owner:\s*(\S+)", body, re.M)
    return found[1] if found else "unknown"


def repro(failure: str) -> str:
    """The command that runs one failure alone."""
    kind, _, name = failure.partition(" ")
    if kind == "smoke":
        return "python3 tools/parity_smoke.py (the step named in the failure)"
    if kind == "run":
        return f"python3 tools/dosbatch/run_tests.py {Path(name).stem}"
    crate, _, test = name.partition("::")
    return f"cargo test --release -p {crate} --lib {test}"


def report(sha: str, previous: str, rows: list[dict], covered: list[str]) -> str:
    """The markdown of one head: what ran (COVERED SUITES), then a table of failure, culprit PR, owner, repro. A short list of failures is not health unless the suites ran."""
    lines = [f"# {sha[:9]} (previous {previous[:9]})", "", "## COVERED SUITES", "", *(f"- {one}" for one in covered), ""]
    if not rows:
        return "\n".join([*lines, "No new failure."]) + "\n"
    lines += ["| failure | culprit | owner | repro |", "|---|---|---|---|"]
    for row in rows:
        pr = f"#{row['pr']}" if row["pr"] else (row["commit"] or "not found")[:9]
        lines.append(f"| {row['failure']} | {pr} | {row['owner']} | `{row['repro']}` |")
    return "\n".join(lines) + "\n"


def first_parent(old: str, new: str, root: Path = ROOT) -> list[tuple[str, str]]:
    """(sha, subject) of the first-parent commits in old..new, oldest first."""
    out = git("log", "--first-parent", "--reverse", "--format=%H %s", f"{old}..{new}", cwd=root)
    return [tuple(line.split(" ", 1)) for line in out.splitlines()]  # type: ignore[misc]


def git(*args: str, cwd: Path = ROOT) -> str:
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=True).stdout.strip()


def tree(sha: str) -> Path:
    """The checkout pinned to `sha`, made once."""
    path = HOME / f"tree-{sha[:9]}"
    if not path.exists():
        git("worktree", "add", "-q", "--detach", str(path), sha)
    assert pairruns.tree_mismatch(path, sha) == "", f"{path} is not at {sha}"
    return path


def fresh(at: Path) -> None:
    """Make every tracked file newer than whatever a shared target holds. Cargo calls an artifact fresh when no source is newer than it, and a workspace crate's artifact
    is shared between checkouts: the tree of an older commit, built after a newer one, linked the newer tree's llrm-mir (10 compile errors that no commit had)."""
    files = git("ls-files", "-z", cwd=at).split("\0")
    for name in files:
        if name and (at / name).exists():
            os.utime(at / name)


def dist(at: Path, sha: str) -> Path:
    """The release binaries of the tree, built at low priority through buildslot into one persistent target."""
    out = HOME / f"dist-{sha[:9]}"
    if (out / "ok").exists():
        return out
    fresh(at)
    target = HOME / "target-dist"
    target.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "CARGO_TARGET_DIR": str(target), "CARGO_INCREMENTAL": "0"}
    env.pop("RUSTFLAGS", None)
    subprocess.run([*LOW, "buildslot", "cargo", "build", *BUILD], cwd=at, env=env, check=True)
    out.mkdir(parents=True, exist_ok=True)
    for name in BINS:
        subprocess.run(["cp", "-p", str(target / "dist" / name), str(out / name)], check=True)
    for name in ("jwasm", "jwlink", "dosbox-x"):
        link = out / name
        link.unlink(missing_ok=True)
        link.symlink_to(TOOLS / name)
    (out / "ok").touch()
    return out


def run_groups(at: Path, bins: Path, only: str | None = None) -> tuple[set[str], str, dict[str, str]]:
    """`run <program>` for each FAIL line of the tests/run groups (both widths, any config) or of one program: the XFAIL list's ids carry no config."""
    command = [sys.executable, "tools/dosbatch/run_groups.py"] if only is None else [sys.executable, "tools/dosbatch/run_tests.py", only]
    done = subprocess.run([*LOW, *command], cwd=at, env={**os.environ, "LLRM_BIN": str(bins)}, capture_output=True, text=True)
    output = done.stdout + done.stderr
    reasons = pairruns.failures(output)
    return {f"run {name.split(' [')[0]}" for name in reasons}, group_summary(output), reasons


def group_summary(output: str) -> str:
    """`57 groups, 858 programs, 0 failing` from the `[group] N programs: P pass, K known, F fail` lines; `NOT RUN` when there are none."""
    lines = re.findall(r"^\[\w+\] (\d+) programs: (\d+) pass, (\d+) known, (\d+) fail", output, re.M)
    if not lines:
        return "NOT RUN (no group line)"
    return f"{len(lines)} groups, {sum(int(one[0]) for one in lines)} programs, {sum(int(one[3]) for one in lines)} failing"


def unit_summary(output: str) -> str:
    """`llrm-c 142/6, llrm-mir 900/1 ...` (tests run / failing) per crate, from each binary's `test result:` line."""
    parts = []
    for block in output.split("     Running ")[1:]:
        binary = re.search(r"/(\w+)-[0-9a-f]+\)", block.splitlines()[0])
        result = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", block)
        if binary and result:
            parts.append(f"{binary[1].replace('_', '-')} {int(result[1]) + int(result[2])}/{result[2]}")
    return ", ".join(parts) or "NOT RUN (no test result)"


def by_binary(output: str) -> dict[str, list[str]]:
    """crate -> failing test names of a `cargo test --workspace` output: each `Running unittests <path> (target/.../llrm_x-hash)` line opens a crate's block."""
    found: dict[str, list[str]] = {}
    for block in output.split("     Running ")[1:]:
        binary = re.search(r"/(\w+)-[0-9a-f]+\)", block.splitlines()[0])
        failed = parity_smoke.failed_tests(block)
        if binary and failed:
            found[binary[1].replace("_", "-")] = failed
    return found


NAMELESS = "unit workspace::failed-without-a-test-name"
LAST_OUTPUT: dict[str, str] = {}


def record_nameless(sha: str, output: str) -> None:
    """Keep what a nameless run printed and the load it ran under, so the retry leaves a trace: HOME/nameless-<sha>.log."""
    HOME.mkdir(parents=True, exist_ok=True)
    (HOME / f"nameless-{sha[:9]}.log").write_text(f"load {os.getloadavg()}\n{output[-4000:]}\n")


def merged(command: list[str], cwd: Path, env: dict[str, str]) -> tuple[str, int]:
    """(output, exit code) with stderr and stdout as one stream: cargo prints `Running` on stderr and the results on stdout, and joined as two strings the
    blocks lost their crate (0 named failures of 14)."""
    done = subprocess.run(command, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    return done.stdout, done.returncode


def unit_result(output: str, code: int) -> set[str]:
    """The failing `unit crate::test` ids of a workspace test run; a run that failed and named no test (a build error, an output this does not parse) is itself the failure,
    never a pass: `-q` printed no `Running` line, nothing parsed, and 59 landings read as 'No new failure'."""
    named = {f"unit {crate}::{test}" for crate, tests in by_binary(output).items() for test in tests}
    return named or ({NAMELESS} if code else set())


def unit_failures(at: Path, name: str | None = None) -> tuple[set[str], str]:
    """`unit <crate>::<test>` for each failing lib test of the workspace (or of one test), in one cargo through buildslot: a crate alone lacks the features the
    root package turns on (llrm-c's `toolchain`: 19 tests failed with "built without the toolchain feature" on a run per crate), a workspace run has them."""
    env = {**os.environ, "CARGO_TARGET_DIR": str(HOME / "target-test"), "CARGO_INCREMENTAL": "0"}
    env.pop("RUSTFLAGS", None)
    command = [*LOW, "buildslot", "cargo", "test", "--release", "--workspace", "--lib", "--no-fail-fast", *([name] if name else [])]
    fresh(at)
    output, code = merged(command, at, env)
    LAST_OUTPUT["text"] = output
    return unit_result(output, code), unit_summary(output)


UNIT_SETS: dict[str, set[str]] = {}


def probe(failure: str, sha: str) -> bool:
    """Whether `failure` holds at `sha`. A unit failure is read from the whole workspace's failing set at that commit, run once and kept: the bisects of dozens of failures
    share their probes (56 failures x 6 probes of 4 minutes was 22 hours; one set per commit is a few dozen probes)."""
    kind, _, name = failure.partition(" ")
    if kind == "smoke":
        raise Inconclusive("smoke steps are not bisected")
    at = tree(sha)
    if kind == "run":
        return failure in run_groups(at, dist(at, sha), Path(name).stem)[0]
    if sha not in UNIT_SETS:
        UNIT_SETS[sha] = unit_failures(at)[0]
        if NAMELESS in UNIT_SETS[sha]:  # one retry: a build killed by load or a memory cap reads like a broken commit
            record_nameless(sha, LAST_OUTPUT.get("text", ""))
            UNIT_SETS[sha] = unit_failures(at)[0]
    if NAMELESS in UNIT_SETS[sha]:
        raise Inconclusive(f"{sha[:9]} does not build")
    return failure in UNIT_SETS[sha]


def smoke(at: Path, bins: Path, previous: str) -> tuple[set[str], str]:
    """parity_smoke's own checks on the pinned tree (workspace check, ratchets, docs, warnings, the instruction budget): (`smoke <name>` per failing step, its result line)."""
    env = {**os.environ, "CARGO_TARGET_DIR": str(HOME / "target-test"), "CARGO_INCREMENTAL": "0"}
    env.pop("RUSTFLAGS", None)
    output, _ = merged([*LOW, sys.executable, "tools/parity_smoke.py", "--bin", str(bins), "--base", previous], at, env)
    named = re.search(r"^FAILING: (.*)$", output, re.M)
    last = (output.strip().splitlines() or ["no output"])[-1][:300]
    if not named:
        return {"smoke nothing-printed"}, f"NOT RUN or unparsed: {last}"
    return {f"smoke {one}" for one in named[1].split(",") if one}, last


def partial(sha: str, previous: str, covered: list[str], raw: list[str]) -> str:
    """The report so far: what ran, and every failure line as the suite printed it (program [config]: want/got), before any bisect."""
    lines = [f"# {sha[:9]} (previous {previous[:9]}) PARTIAL", "", "## COVERED SUITES (so far)", "", *(f"- {one}" for one in covered), "", "## Raw failures, bisect pending", ""]
    return "\n".join([*lines, *(f"- {one}" for one in raw)]) + "\n"


def combine_off(at: Path, bins: Path, programs: set[str]) -> str:
    """Which of the failing tests/run programs still fail with the same binary under LLRM_NO_COMBINE=1: those that stop failing name combine as the cause."""
    wrapper = HOME / f"dist-{bins.name.removeprefix('dist-')}-nocombine"
    wrapper.mkdir(parents=True, exist_ok=True)
    for name in BINS:
        (wrapper / name).write_text(f'#!/bin/bash\nexec env LLRM_NO_COMBINE=1 {bins}/{name} "$@"\n')
        (wrapper / name).chmod(0o755)
    for name in ("jwasm", "jwlink", "dosbox-x"):
        link = wrapper / name
        link.unlink(missing_ok=True)
        link.symlink_to(TOOLS / name)
    cleared = sorted(one for one in programs if one not in run_groups(at, wrapper, Path(one.removeprefix("run ")).stem)[0])
    return f"LLRM_NO_COMBINE=1 clears: {', '.join(cleared) or 'none'} (of {len(programs)} failing)"


def digest(path: Path) -> str:
    import hashlib

    return hashlib.md5(path.read_bytes()).hexdigest()[:12]


def combine_premise(at: Path, bins: Path, built: Path) -> str:
    """The premise line: the tested backend is the dist build's (same md5), and a program's object differs between default and LLRM_NO_COMBINE=1 on it, so combine ran."""
    compiler, backend = llrmbin.compiler(bins), "llrm-llc" if (bins / "llrm-cc").is_file() else llrmbin.compiler(bins).name
    same = digest(bins / backend) == digest(built / backend)
    seen = "no program differed"
    for source in sorted((at / "tests/run/c").glob("*.c"))[:200]:
        objects = []
        for off in (False, True):
            out = HOME / ("premise-off.obj" if off else "premise-on.obj")
            env = {**os.environ, "LLRM_CC_TARGET": "x86_64-linux", **({"LLRM_NO_COMBINE": "1"} if off else {})}
            done = subprocess.run([str(compiler), "-O2", "-c", "-o", str(out), str(source)], env=env, capture_output=True)
            objects.append(digest(out) if done.returncode == 0 and out.exists() else None)
        if objects[0] and objects[1] and objects[0] != objects[1]:
            seen = f"{source.name} differs"
            break
    return f"premise: tested {backend} {digest(bins / backend)} {'equals' if same else 'DIFFERS FROM'} the dist build {digest(built / backend)}; default vs LLRM_NO_COMBINE=1: {seen}"


def check(sha: str, previous: str, emit=lambda covered, raw: None) -> tuple[set[str], list[str]]:
    """Every failure of the head and the COVERED SUITES lines: tests/run at both widths first (written out at once), the workspace lib tests, parity_smoke's checks and budget."""
    at = tree(sha)
    bins = dist(at, sha)
    runs, run_line, reasons = run_groups(at, bins)
    covered = [combine_premise(at, bins, HOME / "target-dist" / "dist"), f"tests/run, both widths (run_groups): {run_line}"]
    raw = [f"{name}: {why}" for name, why in reasons.items()]
    emit(covered, raw)
    if runs:
        covered.append(combine_off(at, bins, runs))
        emit(covered, raw)
    units, unit_line = unit_failures(at)
    covered.append(f"workspace lib tests (tests run/failing per crate): {unit_line}")
    raw += sorted(units)
    emit(covered, raw)
    smoked, smoke_line = smoke(at, bins, previous)
    covered += [f"parity_smoke (workspace check, ratchets, docs, warnings, instruction budget): {smoke_line}", "NOT RUN: fmt.sh, prepush.sh, the integration tests (tests/*.rs), the bench rows"]
    raw += sorted(smoked)
    emit(covered, raw)
    return runs | units | smoked, covered


def load_state() -> dict:
    path = HOME / "state.json"
    return json.loads(path.read_text()) if path.exists() else {"last": None, "heads": {}}


def save_state(state: dict) -> None:
    HOME.mkdir(parents=True, exist_ok=True)
    (HOME / "state.json").write_text(json.dumps(state, indent=1))


def pr_owner(number: int) -> str:
    done = subprocess.run(["gh", "pr", "view", str(number), "--json", "body", "-q", ".body"], capture_output=True, text=True)
    return owner_of(done.stdout)


def merging_pr(sha: str, pulls: list[dict]) -> int | None:
    """The PR whose merge commit is `sha`. GitHub lists every PR that contains the commit, so a PR that merely merged the head into its own branch (#2137) came first."""
    mine = [one["number"] for one in pulls if one.get("merge_commit_sha") == sha]
    return mine[0] if mine else None


def pr_of_commit(sha: str) -> int | None:
    """The PR a landed commit came from, for a squash whose subject carries no number."""
    done = subprocess.run(["gh", "api", f"repos/{{owner}}/{{repo}}/commits/{sha}/pulls"], capture_output=True, text=True)
    try:
        return merging_pr(sha, json.loads(done.stdout))
    except ValueError:
        return None


def verified_pr(pr: int | None, state_of=None) -> tuple[int | None, str]:
    """(pr, note): a culprit names a PR only if GitHub says it is MERGED; an open or closed one is rejected, as a bisect cannot blame a change that is not on the head."""
    if pr is None:
        return None, "no PR in the commit's subject or GitHub's link"
    state = (state_of or pr_state)(pr)
    return (pr, "") if state == "MERGED" else (None, f"#{pr} is {state}, not merged: rejected")


def pr_state(number: int) -> str:
    done = subprocess.run(["gh", "pr", "view", str(number), "--json", "state", "-q", ".state"], capture_output=True, text=True)
    return done.stdout.strip() or "UNKNOWN"


class Inconclusive(Exception):
    """A probe commit that does not build says nothing about a test: the bisect stops there instead of reading every test as passing."""


def once(sha: str | None = None) -> str:
    """Check one head against the last checked one; write and return its report path."""
    git("fetch", "-q", "origin")
    sha = git("rev-parse", sha or "origin/feat/parity")
    state = load_state()
    previous = state["last"]
    HOME.mkdir(parents=True, exist_ok=True)
    failing, covered = check(sha, previous or sha, lambda covered, raw: (HOME / f"{sha[:9]}.md").write_text(partial(sha, previous or sha, covered, raw)))
    listed = xfail.load()
    before = set(state["heads"].get(previous, [])) if previous else set()
    rows = []
    pending = sorted(new_failures(failing, before, listed))
    HOME.mkdir(parents=True, exist_ok=True)
    (HOME / f"{sha[:9]}.md").write_text(report(sha, previous or sha, [{"failure": one, "commit": None, "pr": None, "owner": "bisect pending", "repro": repro(one)} for one in pending], covered))
    for failure in pending:
        commits = first_parent(previous, sha) if previous else []
        try:
            found = culprit([one for one, _ in commits], lambda one: probe(failure, one)) if commits else None
            note = ""
        except Inconclusive as why:
            found, note = None, f"inconclusive: {why}"
        subject = dict(commits).get(found, "") if found else ""
        pr, why_not = verified_pr(pr_of(subject) or (pr_of_commit(found) if found else None)) if found else (None, note)
        rows.append({"failure": failure, "commit": found, "pr": pr, "owner": pr_owner(pr) if pr else (why_not or "unknown"), "repro": repro(failure)})
    HOME.mkdir(parents=True, exist_ok=True)
    path = HOME / f"{sha[:9]}.md"
    path.write_text(report(sha, previous or sha, rows, covered))
    state["heads"][sha] = sorted(failing)
    state["last"] = sha
    save_state(state)
    return str(path)


def main() -> int:
    if len(sys.argv) > 1 and sys.argv[1] == "loop":
        while True:
            git("fetch", "-q", "origin")
            if git("rev-parse", "origin/feat/parity") != load_state()["last"]:
                print(once(), flush=True)
            time.sleep(300)
    print(once(sys.argv[2] if len(sys.argv) > 2 else None))
    return 0


if __name__ == "__main__":
    sys.exit(main())
