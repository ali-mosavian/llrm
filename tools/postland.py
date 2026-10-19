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
import pairruns  # noqa: E402
import parity_smoke  # noqa: E402
import xfail  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
HOME = Path(os.environ.get("POSTLAND", Path.home() / "scratch" / "postland"))
TOOLS = Path(os.environ.get("POSTLAND_TOOLS", Path.home() / "scratch" / "sb-head3" / "dist"))  # jwasm, jwlink, dosbox-x
BINS = ("llrm-c", "llrm-qb", "llrm-nib", "nibfront")
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
    if kind == "run":
        return f"python3 tools/dosbatch/run_tests.py {Path(name).stem}"
    crate, _, test = name.partition("::")
    return f"cargo test --release -p {crate} --lib {test}"


def report(sha: str, previous: str, rows: list[dict]) -> str:
    """The markdown of one head: a table of failure, culprit PR, owner, repro."""
    lines = [f"# {sha[:9]} (previous {previous[:9]})", ""]
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


def dist(at: Path, sha: str) -> Path:
    """The release binaries of the tree, built at low priority through buildslot into one persistent target."""
    out = HOME / f"dist-{sha[:9]}"
    if (out / "ok").exists():
        return out
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


def run_failures(at: Path, bins: Path, only: str | None = None) -> set[str]:
    """`run <program>` for each FAIL line of the tests/run groups (both widths, any config) or of one program: the XFAIL list's ids carry no config."""
    command = [sys.executable, "tools/dosbatch/run_groups.py"] if only is None else [sys.executable, "tools/dosbatch/run_tests.py", only]
    done = subprocess.run([*LOW, *command], cwd=at, env={**os.environ, "LLRM_BIN": str(bins)}, capture_output=True, text=True)
    return {f"run {name.split(' [')[0]}" for name in pairruns.failures(done.stdout + done.stderr)}


def by_binary(output: str) -> dict[str, list[str]]:
    """crate -> failing test names of a `cargo test --workspace` output: each `Running unittests <path> (target/.../llrm_x-hash)` line opens a crate's block."""
    found: dict[str, list[str]] = {}
    for block in output.split("     Running ")[1:]:
        binary = re.search(r"/(\w+)-[0-9a-f]+\)", block.splitlines()[0])
        failed = parity_smoke.failed_tests(block)
        if binary and failed:
            found[binary[1].replace("_", "-")] = failed
    return found


def merged(command: list[str], cwd: Path, env: dict[str, str]) -> tuple[str, int]:
    """(output, exit code) with stderr and stdout as one stream: cargo prints `Running` on stderr and the results on stdout, and joined as two strings the
    blocks lost their crate (0 named failures of 14)."""
    done = subprocess.run(command, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    return done.stdout, done.returncode


def unit_result(output: str, code: int) -> set[str]:
    """The failing `unit crate::test` ids of a workspace test run; a run that failed and named no test (a build error, an output this does not parse) is itself the failure,
    never a pass: `-q` printed no `Running` line, nothing parsed, and 59 landings read as 'No new failure'."""
    named = {f"unit {crate}::{test}" for crate, tests in by_binary(output).items() for test in tests}
    return named or ({"unit workspace::failed-without-a-test-name"} if code else set())


def unit_failures(at: Path, name: str | None = None) -> set[str]:
    """`unit <crate>::<test>` for each failing lib test of the workspace (or of one test), in one cargo through buildslot: a crate alone lacks the features the
    root package turns on (llrm-c's `toolchain`: 19 tests failed with "built without the toolchain feature" on a run per crate), a workspace run has them."""
    env = {**os.environ, "CARGO_TARGET_DIR": str(HOME / "target-test"), "CARGO_INCREMENTAL": "0"}
    env.pop("RUSTFLAGS", None)
    command = [*LOW, "buildslot", "cargo", "test", "--release", "--workspace", "--lib", "--no-fail-fast", *([name] if name else [])]
    output, code = merged(command, at, env)
    return unit_result(output, code)


def probe(failure: str, sha: str) -> bool:
    """Whether `failure` holds at `sha`."""
    at = tree(sha)
    kind, _, name = failure.partition(" ")
    if kind == "run":
        return failure in run_failures(at, dist(at, sha), Path(name).stem)
    return failure in unit_failures(at, name.partition("::")[2])


def check(sha: str) -> set[str]:
    """Every failure of the head: tests/run at m16+m32, then the workspace lib tests."""
    at = tree(sha)
    return run_failures(at, dist(at, sha)) | unit_failures(at)


def load_state() -> dict:
    path = HOME / "state.json"
    return json.loads(path.read_text()) if path.exists() else {"last": None, "heads": {}}


def save_state(state: dict) -> None:
    HOME.mkdir(parents=True, exist_ok=True)
    (HOME / "state.json").write_text(json.dumps(state, indent=1))


def pr_owner(number: int) -> str:
    done = subprocess.run(["gh", "pr", "view", str(number), "--json", "body", "-q", ".body"], capture_output=True, text=True)
    return owner_of(done.stdout)


def pr_of_commit(sha: str) -> int | None:
    """The PR a landed commit came from, by GitHub's own link, for a squash whose subject carries no number."""
    done = subprocess.run(["gh", "api", f"repos/{{owner}}/{{repo}}/commits/{sha}/pulls", "-q", ".[0].number"], capture_output=True, text=True)
    return int(done.stdout) if done.stdout.strip().isdigit() else None


def once(sha: str | None = None) -> str:
    """Check one head against the last checked one; write and return its report path."""
    git("fetch", "-q", "origin")
    sha = git("rev-parse", sha or "origin/feat/parity")
    state = load_state()
    previous = state["last"]
    failing = check(sha)
    listed = xfail.load()
    before = set(state["heads"].get(previous, [])) if previous else set()
    rows = []
    for failure in sorted(new_failures(failing, before, listed)):
        commits = first_parent(previous, sha) if previous else []
        found = culprit([one for one, _ in commits], lambda one: probe(failure, one)) if commits else None
        subject = dict(commits).get(found, "") if found else ""
        pr = pr_of(subject) or (pr_of_commit(found) if found else None)
        rows.append({"failure": failure, "commit": found, "pr": pr, "owner": pr_owner(pr) if pr else "unknown", "repro": repro(failure)})
    HOME.mkdir(parents=True, exist_ok=True)
    path = HOME / f"{sha[:9]}.md"
    path.write_text(report(sha, previous or sha, rows))
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
