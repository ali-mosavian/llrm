"""The merge train: up to N smoke-green PRs onto origin/main, one full gate on that tip, and the tip printed.

    train.py PR... [--max 5]

It never pushes: on a pass it prints `TRAIN PASS TIP=<sha>` and the `git push` line, and whoever lands it runs that line. The pushed
commit is the commit the full gate passed on. A red batch is split in halves, the failed steps re-run on each, the culprits are named
with what they printed, and the rest is gated whole on its own tip before it is offered.
"""

from __future__ import annotations

import os
import re
import sys
import json
import fcntl
import shutil
import argparse
import subprocess
from pathlib import Path
from typing import Callable
from dataclasses import dataclass, field

import gate

ROOT = gate.ROOT
BASE = "origin/main"
MIN_FREE_GB = 50
LOGS = Path(os.environ.get("LLRM_TRAIN_LOGS") or Path.home() / "scratch/train-logs")
MIN_FREE_INODE_PERCENT = 10
# A PR that touches these is gated alone: its change is to what judges the others.
SOLO = ("tools/gate/", "tools/bench/", "tools/measure.py", "tools/compile-cost.py")


def git(*args: str, cwd: Path = ROOT, check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(cwd), *args], capture_output=True, text=True, check=check)


@dataclass
class Pr:
    number: int
    sha: str
    files: list[str] = field(default_factory=list)

    @property
    def solo(self) -> bool:
        return any(f.startswith(SOLO) for f in self.files)


def preflight(home: Path = Path.home(), tmp: Path = Path(os.environ.get("TMPDIR") or "/tmp"), environ: dict | None = None) -> list[str]:
    """What is wrong with the host, as an environment verdict (not a PR's failure): little disk, few inodes, no target dir."""
    environ = os.environ if environ is None else environ
    out = []
    free = shutil.disk_usage(home).free / 2**30
    if free < MIN_FREE_GB:
        out.append(f"{home}: {free:.0f} GB free, need {MIN_FREE_GB}")
    stat = os.statvfs(tmp)
    if stat.f_files and stat.f_favail * 100 / stat.f_files < MIN_FREE_INODE_PERCENT:
        out.append(f"{tmp}: under {MIN_FREE_INODE_PERCENT}% of inodes free")
    if not environ.get("CARGO_TARGET_DIR"):
        out.append("CARGO_TARGET_DIR is not set")
    return out


def describe(number: int) -> tuple[Pr, dict]:
    done = subprocess.run(["gh", "pr", "view", str(number), "--json", "headRefOid,baseRefName,title,body,state,files"], capture_output=True, text=True)
    if done.returncode:
        sys.exit(f"train: gh pr view {number}: {done.stderr.strip()}")
    info = json.loads(done.stdout)
    return Pr(number, info["headRefOid"], [f["path"] for f in info.get("files", [])]), info


def pr_problems(info: dict, sha: str, smoked: bool, root: Path = ROOT) -> list[str]:
    """Why a PR does not board: its base is not main (stacked), its text or commits or added lines name the assistant or its maker, or
    its head has no smoke PASS."""
    out = []
    if info.get("baseRefName") != "main":
        out.append(f"base is {info.get('baseRefName')!r}, not main")
    text = f"{info.get('title', '')}\n{info.get('body', '')}\n" + git("log", "--format=%B", f"{BASE}..{sha}", cwd=root).stdout
    added = [l for l in git("diff", "--unified=0", f"{BASE}...{sha}", cwd=root).stdout.splitlines() if l.startswith("+") and not l.startswith("+++")]
    if gate.MENTION.search(text) or any(gate.MENTION.search(l) for l in added):
        out.append("its text, commits or diff name the assistant or its maker")
    if not smoked:
        out.append(f"no smoke PASS at {sha[:9]} (gate.py run --tier smoke)")
    return out


def batches(prs: list[Pr], most: int) -> list[list[Pr]]:
    """`prs` in order in batches of at most `most`; a solo PR is a batch of one, and the others carry on around it."""
    out, now = [], []
    for pr in prs:
        if pr.solo:
            out.append([pr])
            continue
        now.append(pr)
        if len(now) == most:
            out.append(now)
            now = []
    return out + ([now] if now else [])


def conflicted(tree: Path) -> list[str]:
    return git("diff", "--name-only", "--diff-filter=U", cwd=tree).stdout.split()


def baseline_only(tree: Path) -> bool:
    """Whether every file a merge conflicts in is a benchmark's expected.toml."""
    files = conflicted(tree)
    return bool(files) and all(re.fullmatch(r"bench/.+/expected\.toml", name) for name in files)


def build_tip(prs: list[Pr], tree: Path, base: str = BASE) -> tuple[str, list[tuple[Pr, str]]]:
    """`base` (the commit origin/main was at when the train started: a ref that moves under a running train made later tips of another base
    than the first gate's, and #1765 conflicted in one, was dropped, and was offered) with each PR merged in order in `tree` (a worktree, left at the result): (the tip, [(PR, why) bounced]). A merge that
    conflicts is undone and bounced; the rest go on."""
    git("checkout", "-q", "--detach", base, cwd=tree)
    bounced = []
    for pr in prs:
        done = git("merge", "--no-ff", "-q", "-m", f"train: #{pr.number}", pr.sha, cwd=tree, check=False)
        if done.returncode and baseline_only(tree):
            # Each PR blesses its own bench counts, so two that touch one benchmark conflict on its expected.toml (every PR of a train did, and
            # all but one bounced). The batch's file stands; a count the PR made better is blessed by the train, one it made worse fails.
            git("checkout", "--ours", "--", *conflicted(tree), cwd=tree)
            git("add", "-A", cwd=tree)
            done = git("commit", "-q", "--no-edit", cwd=tree, check=False)
        if done.returncode:
            git("merge", "--abort", cwd=tree, check=False)
            bounced.append((pr, "conflicts with the batch before it"))
    return git("rev-parse", "HEAD", cwd=tree).stdout.strip(), bounced


@dataclass
class Verdict:
    ok: bool
    failed: list[str] = field(default_factory=list)
    text: str = ""


def tree_env(env: dict, tree: Path) -> dict:
    """`env` with one target per tree: the tip's gate and the re-run on main shared one, and main's pytest read the tip's llrm-c, which
    names the tip's tree (the first run's failure was overwritten, and the second failed on that path)."""
    if target := env.get("CARGO_TARGET_DIR"):
        env = {**env, "CARGO_TARGET_DIR": f"{target}-{tree.name}"}
    return env


def full_gate(tip: str, tree: Path, steps: list[str] | None = None, strict: bool = False) -> Verdict:
    """The full tier at `tip` (checked out in `tree`), or only `steps` of it. The bench step runs in batch mode (a better count passes)
    unless `strict`."""
    git("checkout", "-q", "--detach", tip, cwd=tree)
    command = [sys.executable, "tools/gate/gate.py", "run", "--tier", "full", "--expect", tip, *(["--steps", *steps] if steps else [])]
    env = {k: v for k, v in os.environ.items() if k != "LLRM_BENCH_BATCH"} | ({} if strict else {"LLRM_BENCH_BATCH": "1"})
    env = tree_env(env, tree)
    done = subprocess.run(command, cwd=tree, capture_output=True, text=True, env=env)
    text = done.stdout + done.stderr
    LOGS.mkdir(parents=True, exist_ok=True)
    (LOGS / f"{tip[:12]}-{tree.name}-{'-'.join(steps or ['full'])}.log").write_text(text)
    last = [l for l in text.splitlines() if l.startswith("GATE ")][-1:] or [""]
    failed = re.findall(r"^\[time\] (\S+) (?:FAIL|INCOMPLETE)", text, re.M)
    return Verdict(done.returncode == 0 and " PASS" in last[0], failed, text)


def search(prs: list[Pr], gated: Callable[[list[Pr], list[str]], Verdict], failed: list[str]) -> tuple[list[Pr], list[tuple[Pr, str]]]:
    """(the PRs that pass, the culprits with what they printed) of a batch whose full gate failed `failed`: halves, each run on the failed
    steps alone. A single PR that fails is the culprit; one that passes alone with its half is not."""
    if len(prs) == 1:
        verdict = gated(prs, failed)
        return ([prs[0]], []) if verdict.ok else ([], [(prs[0], gate.failure_text(verdict.text) or verdict.text[-1500:])])
    middle = len(prs) // 2
    green, culprits = [], []
    for half in (prs[:middle], prs[middle:]):
        verdict = gated(half, failed)
        if verdict.ok:
            green += half
        else:
            more_green, more_bad = search(half, gated, verdict.failed or failed)
            green, culprits = green + more_green, culprits + more_bad
    return green, culprits


def run(prs: list[Pr], tip_of: Callable[[list[Pr]], tuple[str, list]], gate_at: Callable[[str, list[str] | None], Verdict], on_main: Callable[[list[str]], Verdict], report: Callable[[str], None] = lambda text: print(text, flush=True), blessed: Callable[[str, list[Pr]], tuple[str, Verdict | None]] | None = None) -> int:
    """One batch end to end. `tip_of(prs)` builds the tip, `gate_at(tip, steps)` gates it, `on_main(steps)` gates origin/main alone."""
    tip, bounced = tip_of(prs)
    for pr, why in bounced:
        report(f"TRAIN BOUNCED #{pr.number}: {why}")
    merged = [p for p in prs if p not in [b for b, _ in bounced]]
    if not merged:
        report("TRAIN EMPTY: nothing merged")
        return 1
    verdict = gate_at(tip, None)
    if verdict.ok:
        return land_blessed(tip, merged, report, blessed)
    if on_main(verdict.failed).ok is False:
        report(f"TRAIN ENV: {', '.join(verdict.failed)} fail on origin/main alone; no PR is blamed")
        return 3

    lost: list[Pr] = []  # members a rebuilt tip left out: a conflict that shows only without the others, never tested, never offered

    def gated(some: list[Pr], steps: list[str]) -> Verdict:
        part, left_out = tip_of(some)
        lost.extend(pr for pr, _ in left_out if pr not in lost)
        return gate_at(part, steps)

    green, culprits = search(merged, gated, verdict.failed)
    for pr, why in culprits:
        report(f"TRAIN FAIL #{pr.number}:\n{why}")
    green = [pr for pr in green if pr not in lost]
    if green:
        tip, left_out = tip_of(green)
        lost.extend(pr for pr, _ in left_out if pr not in lost)
        green = [pr for pr in green if pr not in lost]
    for pr in lost:
        report(f"TRAIN DROPPED #{pr.number}: it conflicts when merged without the rest of the batch; not tested, not offered")
    if not green:
        return 1
    final = gate_at(tip, None)
    if not final.ok:
        report(f"TRAIN FAIL: the green rest ({' '.join('#%d' % p.number for p in green)}) fails the full gate together: {', '.join(final.failed)}")
        return 1
    return land_blessed(tip, green, report, blessed)


def bless(tip: str, prs: list[Pr], tree: Path) -> tuple[str, Verdict | None]:
    """`tip` with the better bench counts of the batch blessed in one commit that names the PRs, and that commit gated on bench alone, strictly
    (a better count then fails): (the tip to offer, the verdict of that gate; None when nothing got better). Nothing worse is ever written."""
    git("checkout", "-q", "--detach", tip, cwd=tree)
    names = " ".join(f"#{p.number}" for p in prs)
    env = tree_env({k: v for k, v in os.environ.items() if k != "LLRM_BENCH_BATCH"}, tree)
    # the tools' environment, as the gate's bench step runs it: this process's python has no unicorn (train 4 passed its gate and died here)
    done = subprocess.run(["uv", "run", "-q", "--project", "tools", "python", "tools/bench/bench.py", "--batch", "--bless", "--reason", f"train: {names}"], cwd=tree, capture_output=True, text=True, env=env)
    if done.returncode:
        return tip, Verdict(False, ["bench"], done.stdout + done.stderr)
    if not git("status", "--porcelain", "--", "bench", cwd=tree).stdout.strip():
        return tip, None
    git("add", "bench", cwd=tree)
    git("commit", "-q", "-m", f"chore(bench): bless the counts that got better (train: {names})", cwd=tree)
    new = git("rev-parse", "HEAD", cwd=tree).stdout.strip()
    return new, full_gate(new, tree, ["bench"], strict=True)


def land_blessed(tip: str, prs: list[Pr], report: Callable[[str], None], blessed) -> int:
    """`land` after the bless step: a bless that fails its strict bench gate offers nothing."""
    if blessed is not None:
        tip, verdict = blessed(tip, prs)
        if verdict is not None and not verdict.ok:
            report("TRAIN FAIL: blessing the better bench counts did not pass the strict bench gate:\n" + (gate.failure_text(verdict.text) or verdict.text[-1500:]))
            return 1
    return land(tip, prs, report)


def land(tip: str, prs: list[Pr], report: Callable[[str], None]) -> int:
    report(f"TRAIN PASS TIP={tip} PRs={' '.join('#%d' % p.number for p in prs)}")
    report(f"git push origin {tip}:main")
    return 0


def places(name: str = "train", home: Path | None = None) -> tuple[Path, Path, Path]:
    """The lock and the two worktrees (the tip's and main's) of the train called `name`: trains of other names share none of them, so they
    run side by side (each gets its own target and logs from its tree's name, #1784)."""
    scratch = (home or Path.home()) / "scratch"
    return scratch / f"{name}.lock", scratch / f"{name}-tree", scratch / f"{name}-main"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("prs", nargs="+", type=int)
    ap.add_argument("--max", type=int, default=5)
    ap.add_argument("--name", default="train", help="trains of different names run at once, each in its own worktrees and target")
    args = ap.parse_args()
    if problems := preflight():
        print("TRAIN ENV: " + "; ".join(problems), flush=True)
        return 3
    lock_path, tree, main_tree = places(args.name)
    lock = open(lock_path, "w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        print("TRAIN BUSY: another train is running", flush=True)
        return 3
    git("fetch", "-q", "origin", "main")
    ready = []
    for number in args.prs:
        pr, info = describe(number)
        git("fetch", "-q", "origin", f"pull/{number}/head")
        if why := pr_problems(info, pr.sha, smoked=gate.smoked(pr.sha)):
            print(f"TRAIN REFUSED #{number}: {'; '.join(why)}", flush=True)
        else:
            ready.append(pr)
    code = 0
    base = git("rev-parse", BASE).stdout.strip()
    if tree.exists():
        git("worktree", "remove", "--force", str(tree), check=False)
    git("worktree", "add", "--detach", str(tree), base)
    try:
        if main_tree.exists():
            git("worktree", "remove", "--force", str(main_tree), check=False)
        git("worktree", "add", "--detach", str(main_tree), base)
        for batch in batches(ready, args.max):
            code |= run(batch, lambda some: build_tip(some, tree, base), lambda tip, steps: full_gate(tip, tree, steps), lambda steps: full_gate(base, main_tree, steps), blessed=lambda tip, some: bless(tip, some, tree))
    finally:
        for one in (tree, main_tree):
            git("worktree", "remove", "--force", str(one), check=False)
    return code


if __name__ == "__main__":
    sys.exit(main())
