#!/usr/bin/env python3
"""What the landing runner refuses a PR for, asked before the PR is marked ready.

    tools/landing/precheck.sh [PR] [--body-file FILE --title TEXT]

Run in the worktree of the PR's branch. Every check runs and every failure is named; exit 1 if any failed, and a PR that fails
stays draft. The checks are the runner's (tools/parity_squash.py `refusals`, `new_lib_failures`) on the branch itself:

1. the branch contains the current feat/parity tip, and merges with it cleanly (a conflict names its files);
2. the description has `Owner:` and `Rows:` lines at the start of a line, a perf PR's description states the ledger command and a
   `d_faces qbq-llrm` number, a `Deletes:` or `Reverts:` line naming every path whose
   squash deletes lines or whole test functions, no link to a deleted page, and a `Rows:` line where the title is a perf PR;
3. no added line, commit message, title or description names the assistant, and no added line names the assistant's config directory;
4. `RUSTFLAGS="-D warnings" cargo check -p <changed crates> --tests` is clean;
5. the changed .rs files are formatted (tools/fmt.sh);
6. a PR that adds a pass slot (registry entry or phase-table row) adds a test at pipeline level;
7. the root scanners (tests that read sources and docs; the landing layer runs them on every batch) have no failure the tip does not have, and
   `tools/docs/docsbuild/build.py --check` finds no generated page stale that the tip has not (seconds);
6. the lib tests of the changed crates (`cargo test -p <crate> --lib`, the layer's command) have no failure the feat/parity tip
   does not have, and where the change touches the backend's phase list or the parity registry, the slot-list tests of
   llrm-transforms (`parity`) run too: a new phase not listed in `parity_tests` is the trap that refused #3708.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import parity_smoke  # noqa: E402
import parity_squash  # noqa: E402
from buildslot import slotted  # noqa: E402

BRANCH = os.environ.get("LAND_BRANCH", "release/next")  # the integration branch (tools/landing/branch.sh)

BASE = f"origin/{BRANCH}"
OWNER = re.compile(r"^Owner:\s*\S+", re.M)
# The board's compile-cost command (docs/working/ledger-command.md): a perf PR's before/after pair is taken with it.
LEDGER_COMMAND = "-m32 -mabi=sysv -march=i486 -O2"
LEDGER_NUMBER = re.compile(r"d_faces qbq-llrm[^\n]*\d", re.I)
ROWS = re.compile(r"^Rows:\s*\S+", re.M)
# Built from pieces: this file must not itself name what it looks for (the runner refuses an added line that does).
CONFIG_DIR = re.compile(r"\." + "clau" + r"de\b")
# Files whose change can leave the slot lists out of step: the machine phases and the parity registry.
SLOT_FILES = re.compile(r"^crates/codegen/llrm-codegen/src/flow\.rs$|^crates/opt/llrm-transforms/src/parity[^/]*\.rs$")


def git(
    *args: str,
    cwd: Path,
) -> str:
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True).stdout


def branch_merges(
    root: Path,
    base: str = BASE,
    head: str = "HEAD",
) -> list[str]:
    """Why the branch is not mergeable with `base`: it does not contain the tip, or merging conflicts (the files named)."""
    if subprocess.run(["git", "merge-base", "--is-ancestor", base, head], cwd=root).returncode == 0:
        return []
    merged = subprocess.run(["git", "merge-tree", "--write-tree", "--name-only", base, head], cwd=root, capture_output=True, text=True)
    if merged.returncode:
        names = [line for line in merged.stdout.splitlines()[1:] if line and not line.startswith("CONFLICT")]
        return [f"the branch conflicts with {base} in {', '.join(names[:6]) or 'some files'}: merge fresh {base} and resolve"]
    return [f"the branch does not contain {base}: merge it in first (a stale head squashes other PRs' work away)"]


def description_failures(
    title: str,
    body: str,
) -> list[str]:
    """The description lines the runner needs."""
    why = []
    if not OWNER.search(body):
        why.append("no `Owner: <session>` line at the start of a line in the description")
    if not ROWS.search(body):
        why.append("no `Rows:` line at the start of a line in the description (`Rows: none` for a PR with no rows)")
    if parity_squash.PERF_TITLE.search(title) and not (LEDGER_COMMAND in body and LEDGER_NUMBER.search(body)):
        why.append(
            "a perf PR states the board's ledger command (`" + LEDGER_COMMAND + "`) and a `d_faces qbq-llrm: <M Ir>` number "
            "(docs/working/ledger-command.md); other programs may follow, labelled with their name"
        )
    return why


def config_dir_paths(
    root: Path,
    base: str = BASE,
    head: str = "HEAD",
) -> list[str]:
    """An added line that names the assistant's config directory."""
    path = ""
    for line in git("diff", "-U0", base, head, cwd=root).splitlines():
        if line.startswith("+++ "):
            path = line[6:]
        elif line.startswith("+") and CONFIG_DIR.search(line):
            return [f"{path}: an added line names the assistant's config directory: {line[1:80].strip()}"]
    return []


SLOT_ADDED = re.compile(r'^\+\s*(?:entry|partial)\("[\w-]+@\d+"|^\+\s*\("[A-Z]\w+", "[\w]+@\d+"\)')
PIPELINE_TEST_FILES = re.compile(r"(?:^|/)(?:pipeline\w*_tests\.rs|tests/[^/]+\.rs)$")


def slot_without_pipeline_test(
    root: Path,
    base: str = BASE,
    head: str = "HEAD",
) -> list[str]:
    """A PR that adds a pass slot (a registry entry or a phase-table row) must add a test at pipeline level: a translated pass can pass every unit test and never run in a compile (#3732's JumpTableToSwitch)."""
    added_slot = any(SLOT_ADDED.match(line) for line in git("diff", "-U0", base, head, "--", "crates", cwd=root).splitlines())
    if not added_slot:
        return []
    path, tested = "", False
    for line in git("diff", "-U0", base, head, cwd=root).splitlines():
        if line.startswith("+++ "):
            path = line[6:]
        elif line.startswith("+") and PIPELINE_TEST_FILES.search(path) and "#[test]" in line:
            tested = True
    return [] if tested else ["the PR adds a pass slot and no test at pipeline level (pipeline::applied or an end-to-end compile in pipeline*_tests.rs or tests/): show the pass fires on a real input"]


def changed_packages(
    root: Path,
    files: list[str],
) -> list[str]:
    packages, _ = parity_smoke.workspace_graph(root)
    return sorted(parity_smoke.touched_packages(files, packages, root))


def warning_failures(
    root: Path,
    packages: list[str],
    run=subprocess.run,
    env: dict | None = None,
) -> list[str]:
    """`RUSTFLAGS="-D warnings" cargo check -p <crates> --tests`: its errors, one line each (a dead-code error refused a landing)."""
    if not packages:
        return []
    # A package whose tests exist only with a feature (llrm-c's `toolchain`) is checked with it: without, its lib tests are a compile_error.
    features = [f"{package}/{feature}" for package in packages for feature in parity_smoke.LIB_FEATURES.get(package, ())]
    command = slotted(["cargo", "check", "-q", "--tests", *[arg for package in packages for arg in ("-p", package)], *(["--features", ",".join(features)] if features else [])])
    done = run(command, cwd=root, capture_output=True, text=True, env={**(env or {}), "RUSTFLAGS": "-D warnings"})
    if not done.returncode:
        return []
    errors = [line.strip() for line in (done.stdout + done.stderr).splitlines() if line.startswith("error")]
    return [f"cargo check -D warnings: {line[:160]}" for line in errors[:8]] or ["cargo check -D warnings failed"]


def lib_failures(
    root: Path,
    packages: list[str],
    files: list[str],
    base: str = BASE,
    run=subprocess.run,
    on_base=None,
) -> list[str]:
    """The lib tests of `packages` that fail here and not on `base`, and the slot-list tests when the phase list or the registry changed."""
    failed: dict[str, set[str]] = {}
    targets = list(packages)
    extra: list[tuple[str, str]] = []
    if any(SLOT_FILES.match(path) for path in files):
        extra.append(("llrm-transforms", "parity"))
    for package in targets:
        failed.setdefault(package, set()).update(_failing(root, package, None, run))
    for package, filter_ in extra:
        failed.setdefault(package, set()).update(_failing(root, package, filter_, run))
    failed = {package: tests for package, tests in failed.items() if tests}
    if not failed:
        return []
    also = (on_base or _failing_on_base)(root, failed, base)
    return [f"lib test fails here and passes on {base}: {package}::{test}" for package, tests in sorted(failed.items()) for test in sorted(tests - also.get(package, set()))]


def _failing(
    root: Path,
    package: str,
    filter_: str | None,
    run,
) -> set[str]:
    base = parity_smoke.lib_command(package)
    command = [base[0], base[1], "-q", *base[2:]] + (["--", filter_] if filter_ else [])
    done = run(slotted(command), cwd=root, capture_output=True, text=True)
    output = done.stdout + done.stderr
    found = set(parity_smoke.failed_tests(output))
    if done.returncode and not found and "no library targets" not in output and "error: " in output:
        found = {"did-not-run"}
    return found


def _failing_on_base(
    root: Path,
    failed: dict[str, set[str]],
    base: str,
) -> dict[str, set[str]]:
    """Which of the failing tests also fail on `base`: run exactly those in a temporary worktree of it (the branch's own checkout is not touched)."""
    with tempfile.TemporaryDirectory(prefix="precheck-base-") as scratch:
        tree = Path(scratch) / "tree"
        subprocess.run(["git", "worktree", "add", "-q", "--detach", str(tree), base], cwd=root, check=True)
        try:
            return {package: parity_squash.failing_on_checkout(package, tests, tree) for package, tests in failed.items()}
        finally:
            subprocess.run(["git", "worktree", "remove", "--force", str(tree)], cwd=root, capture_output=True)


def scanner_failures(
    root: Path,
    files: list[str],
    base: str = BASE,
    run=subprocess.run,
    on_base=None,
) -> list[str]:
    """The root scanners (tests that read sources and docs) and a `tests/*.rs` the branch changes that fail here and pass on `base`: the landing layer's check
    (`parity_squash.new_scanner_failures`), so a docs-only PR that names a path of one machine or breaks a link is told before it is marked ready."""

    def base_failures(targets: list[str]) -> set[str]:
        with tempfile.TemporaryDirectory(prefix="precheck-base-") as scratch:
            tree = Path(scratch) / "tree"
            subprocess.run(["git", "worktree", "add", "-q", "--detach", str(tree), base], cwd=root, check=True)
            try:
                return parity_smoke.integration_failures(targets, tree, run)
            finally:
                subprocess.run(["git", "worktree", "remove", "--force", str(tree)], cwd=root, capture_output=True)

    here = parity_squash.new_scanner_failures(
        files,
        on_merged=lambda targets: parity_smoke.integration_failures(targets, root, run),
        on_head=on_base or base_failures,
    )
    return [f"scanner fails here and passes on {base}: {name}" for name in sorted(here)]


def docs_failures(root: Path, base: str = BASE, docs=None, docs_on_base=None) -> list[str]:
    """The generated pages `build.py --check` finds stale on the branch (which contains `base`, so it is the branch merged with it) and not on `base`: #5062, #5085 and #5088 changed
    sources and were marked ready without the page, and each failed the batch's docs check."""
    stale = (docs or parity_squash.docs_stale)(root)
    if not stale:
        return []

    def on_base() -> set[str]:
        with tempfile.TemporaryDirectory(prefix="precheck-base-") as scratch:
            work = Path(scratch) / "w"
            git("worktree", "add", "-q", "--detach", str(work), base, cwd=root)
            try:
                return parity_squash.docs_stale(work)
            finally:
                subprocess.run(["git", "worktree", "remove", "--force", str(work)], cwd=root, capture_output=True)

    new = sorted(stale - (docs_on_base or on_base)())
    return [f"generated docs out of date: {', '.join(new[:6])}; run python3 tools/docs/docsbuild/build.py --sync and commit the pages"] if new else []


def precheck(
    root: Path,
    title: str | None,
    body: str | None,
    base: str = BASE,
    run=subprocess.run,
    on_base=None,
) -> list[str]:
    """Every failure, named."""
    why = branch_merges(root, base)
    files = git("diff", "--name-only", f"{base}...HEAD", cwd=root).split()
    if body is None:
        why.append("no PR description to check (open the PR as a draft first, or pass --body-file)")
    else:
        why += description_failures(title or "", body)
        # The runner only warns about the description checks for now; a worker fixes them all, so they are failures here.
        why += parity_squash.body_checks("HEAD", body, base, root, title or "")
        why += parity_squash.refusals("HEAD", body, base, root, title or "")
    why += [f"{path}: tools/fmt.sh would change it; run tools/fmt.sh (the runner formats an unformatted PR with a commit on your branch, which races with your pushes)" for path in sorted(parity_squash.fmt_differs(root, files))]
    why += slot_without_pipeline_test(root, base)
    why += config_dir_paths(root, base)
    packages = changed_packages(root, files)
    why += warning_failures(root, packages, run, dict(os.environ))
    why += lib_failures(root, packages, files, base, run, on_base)
    why += scanner_failures(root, files, base, run)
    why += docs_failures(root, base)
    return list(dict.fromkeys(why))


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("pr", nargs="?", help="the PR number (default: the branch's PR, if it has one)")
    parser.add_argument("--body-file", type=Path)
    parser.add_argument("--title", default="")
    args = parser.parse_args(argv)
    root = Path(git("rev-parse", "--show-toplevel", cwd=Path.cwd()).strip())
    subprocess.run(["git", "fetch", "-q", "origin", BRANCH], cwd=root)
    title, body = args.title, args.body_file.read_text() if args.body_file else None
    if body is None:
        shown = subprocess.run(["gh", "pr", "view", *([args.pr] if args.pr else []), "--json", "title,body"], cwd=root, capture_output=True, text=True)
        if shown.returncode == 0:
            info = json.loads(shown.stdout)
            title, body = info["title"], info["body"] or ""
    failures = precheck(root, title, body)
    for line in failures:
        print(f"PRECHECK FAIL: {line}")
    print("precheck: PASS" if not failures else f"precheck: FAIL ({len(failures)}): keep the PR draft")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
