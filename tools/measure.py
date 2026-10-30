#!/usr/bin/env python3
"""What llrm-cc costs to compile (clang's front end and llrm-llc, the driver's whole process tree) and how that cost grows, compared with the same measurement of the commit this one branches from.

    tools/measure.py check              measure this tree's compiler, compare with the merge-base's; exit 1 on a rise past tolerance
    tools/measure.py record SHA [BIN]   measure the compiler in BIN (default: this tree's) and store it as SHA's
    tools/measure.py show               print this tree's measurement
    tools/measure.py creep [REF]        compare REF (HEAD) with the commit 50 merges or a week back at the same tolerances

Three measurements, all user-space instructions (`perf stat`), so a loaded host does not move them: the compile of the 66 vsgcc
programs and QCport's modules at -O1/-O2/-O3/-Os (tools/compile-cost.py); the ratio of the compile at 2N to N on generated programs
(vsgcc/scaling_gate.py); and the same ratio for each step of the compile (`LLRM_DEBUG=time`'s [instr] rows).

A measurement is stored per tree under ~/.cache/llrm/measure (LLRM_MEASURE_DIR), never in the repository, so two branches never
touch a shared file. The base's is read from there; if it is missing the base is built in a tree and target directory of its own
and measured with this tree's tools and inputs (only the compiler differs), then stored. A rise past the tolerances in tiers.toml
[measure] fails; a drop does not need recording, the next branch's base has it. LLVM's compile-time tracker compares with the
parent the same way.
"""
from __future__ import annotations

import argparse
import concurrent.futures
import contextlib
import fcntl
import hashlib
import importlib.util
import json
import math
import os
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
CACHE = Path(os.environ.get("LLRM_MEASURE_DIR") or Path.home() / ".cache/llrm/measure")


# The profile the measured compiler is built with: `dist` (codegen-units 1, fat LTO: the shipped build), or `release` (16 units). An edit to one
# file re-partitions the release build's units and flips the inlining of unrelated functions: the same #1977 pair read +0.46..0.61% on its gate
# shapes under release, +0.01..0.04% under dist. A measurement is of one profile, so the profile is part of the method.
PROFILE = os.environ.get("LLRM_MEASURE_PROFILE") or "dist"


def repository_of(root: Path) -> Path:
    """The repository `root` is a working tree of: its common git directory, which every worktree of it shares (so does every commit made in one),
    else `root` itself where it is not a repository."""
    done = subprocess.run(["git", "rev-parse", "--git-common-dir"], cwd=root, capture_output=True, text=True) if root.is_dir() else None
    if done is None or done.returncode:
        return root
    return (root / done.stdout.strip()).resolve()


def build_tree(root: Path = ROOT, env: dict | None = None) -> Path:
    """Where the base is built: `LLRM_MEASURE_BUILD`, else a tree of this repository's own: one for all its worktrees (a tree for each was
    10-37 GB each, 489 GB of 48 on one host). Two clones share a tree no `git checkout` of the other's commit can enter ('unable to read
    tree'), so each repository has its own, keyed by its git directory."""
    env = os.environ if env is None else env
    return Path(env.get("LLRM_MEASURE_BUILD") or BUILDS / hashlib.sha256(str(repository_of(root)).encode()).hexdigest()[:10])


BUILDS = Path.home() / ".cache/llrm/measure-build"
# The builds kept, least recently used out first past this many bytes (LLRM_MEASURE_BUILD_CAP_GB, default 40 GB: a build tree is 10-37 GB; at 120 the cache held 92 GB and filled the disk).
DEFAULT_BUILD_CAP_GB = 40
BUILD_CAP = int(float(os.environ.get("LLRM_MEASURE_BUILD_CAP_GB") or DEFAULT_BUILD_CAP_GB) * 2**30)


def size_of(tree: Path) -> int:
    return int(subprocess.run(["du", "-sb", str(tree)], capture_output=True, text=True, check=True).stdout.split()[0])


def pruned(parent: Path, keep: Path, cap: int) -> list[Path]:
    """Trees under `parent` other than `keep`, least recently used first, removed until what is left (with `keep`) is within `cap` bytes.
    A tree's use is the time of its `.used` file, which `built` touches."""
    trees = [t for t in parent.iterdir() if t.is_dir() and t != keep] if parent.is_dir() else []
    used = lambda t: (t / ".used").stat().st_mtime if (t / ".used").exists() else t.stat().st_mtime
    sizes = {t: size_of(t) for t in trees}
    total = sum(sizes.values()) + (size_of(keep) if keep.exists() else 0)
    gone = []
    for tree in sorted(trees, key=used):
        if total <= cap:
            break
        shutil.rmtree(tree, ignore_errors=True)
        total -= sizes[tree]
        gone.append(tree)
    return gone


BUILD = build_tree()
VSGCC = next((ROOT / "crates/target").glob("*/vsgcc"))
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(VSGCC))

import benchlist  # noqa: E402
import llrmbin  # noqa: E402
import scaling_gate  # noqa: E402
import levels_time  # noqa: E402

sys.path.insert(0, str(HERE / "gate"))
import gate  # noqa: E402

DIST_BUILD = gate.DIST_BUILD
# What a measurement needs is llrm-cc and the llrm-llc it runs (the compile cost is theirs); the other binaries would each be a fat-LTO link.
# The profile the bench's base is built in: its tools are all of the binaries (llrm-qb, llrm-nib ...), which the dist build above does not make.
BENCH_BASE_PROFILE = "release"
MEASURE_BUILDS = {"dist": "env -u RUSTFLAGS cargo build --profile dist -q --bin llrm-cc --bin llrm-llc", "release": gate.MEASURE_BUILD}

_spec = importlib.util.spec_from_file_location("compile_cost", HERE / "compile-cost.py")
compile_cost = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(compile_cost)

NoCounter = compile_cost.NoCounter


def head_branch() -> str:
    """The branch being measured: the PR's head in CI, else the checked-out one."""
    if named := os.environ.get("GITHUB_HEAD_REF"):
        return named
    done = subprocess.run(["git", "-C", str(ROOT), "branch", "--show-current"], capture_output=True, text=True)
    return done.stdout.strip()


def tolerances(path: Path = HERE / "gate" / "tiers.toml", branch: str | None = None) -> dict:
    """The `[measure]` table. An accepted row with a `branch` key is a decision for that PR alone: it is dropped for any other."""
    tol = tomllib.loads(path.read_text())["measure"]
    here = head_branch() if branch is None else branch
    tol["accepted"] = [one for one in tol.get("accepted", []) if one.get("branch", here) == here]
    return tol


def row_tolerance(tol: dict, label: str) -> dict:
    """`tol` as a recorded decision sets it for `label` (`[[measure.accepted]]` in tiers.toml): the limits the entry names replace the
    general ones for every row at or below its `row` ("nest O1" is also "nest O1 mir lsr"), as ceilings: a row above them fails as ever.
    An entry with a `suffix` ("mir gvn") is for every row ending in it. An entry for one PR (`branch`) does not replace the first
    match: its limits are laid over it, the higher of the two standing."""
    named = lambda entry: label == entry.get("row") or (
        "row" in entry and label.startswith(entry["row"] + " ")
    ) or ("suffix" in entry and (label == entry["suffix"] or label.endswith(" " + entry["suffix"])))
    own = lambda entry: {name: value for name, value in entry.items() if name not in ("row", "suffix", "why", "branch")}
    entries = tol.get("accepted", [])
    found = tol
    for entry in entries:
        if "branch" not in entry and named(entry):
            found = {**tol, **own(entry)}
            break
    for entry in entries:
        if "branch" in entry and named(entry):
            found = {**found, **{name: max(value, found.get(name, value)) for name, value in own(entry).items()}}
    return found


def method() -> str:
    """What the measurement is made of besides the compiler: the tools, the programs and QCport's modules. A stored measurement of
    another method is not comparable and is made again."""
    files = [HERE / "compile-cost.py", HERE / "llrmbin.py", *(VSGCC / name for name in ("scaling_gate.py", "scaling.py", "levels_time.py", "programs.py", "wrap.py"))]
    files += benchlist.sources(".c", ROOT / "bench", parity=False) + sorted((VSGCC / "kernels").glob("*/*.c"))
    if qcport := os.environ.get("QCPORT"):
        files += sorted(Path(qcport).expanduser().glob("*/*.c"))
    digest = hashlib.sha256()
    for one in files:
        digest.update(str(one.name).encode() + b"\0" + one.read_bytes())
    digest.update(b"qcport" if qcport else b"-")
    digest.update(PROFILE.encode())
    digest.update(levels_time.backend().encode())  # the counts of one backend are not another's
    return digest.hexdigest()[:12]


def measured_dir(env: dict | None = None) -> Path:
    """The directory of the compiler under measurement: LLRM_BIN, else the target's `PROFILE` directory. A binary of another profile is refused: a
    release llrm-cc measured against a dist base reads every shape wrong by the difference of their inlining (8% of the compile)."""
    env = os.environ if env is None else env
    found = Path(env["LLRM_BIN"]) if env.get("LLRM_BIN") else llrmbin.target_dir(env) / PROFILE
    if not found.name.startswith(PROFILE):
        raise SystemExit(f"measure: {found} is not a {PROFILE} build (LLRM_MEASURE_PROFILE={PROFILE}); `tools/measure.py dist` builds and prints it")
    return found


def ensure_dist(env: dict | None = None) -> Path:
    """target/dist with an llrm-cc at least as new as the sources, built if not: what a local gate run measures (CI downloads it)."""
    env = os.environ if env is None else env
    found = llrmbin.target_dir(env) / "dist"
    if not (found / "llrm-cc").is_file() or llrmbin.stale_binaries(found):
        require_space(found)
        subprocess.run(["bash", "-c", MEASURE_BUILDS["dist"]], cwd=ROOT, env={k: v for k, v in env.items() if k != "LLRM_BIN"}, check=True)
    return found


def require_space(where: Path, minimum_gb: float | None = None) -> None:
    """A build here needs room: /home filled up on tinytim (858 MB free) and every session's build failed. LLRM_MIN_FREE_GB (default 100; none
    on a hosted runner, a fresh VM with 78 GB free: #2109's bench shards failed on it)."""
    hosted = os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted"
    need = float(os.environ.get("LLRM_MIN_FREE_GB") or (0 if hosted else 100)) if minimum_gb is None else minimum_gb
    probe = where
    while not probe.exists() and probe != probe.parent:
        probe = probe.parent
    free = shutil.disk_usage(probe).free / 2**30
    if free < need:
        raise SystemExit(f"measure: {free:.0f} GB free under {probe}, {need:.0f} needed to build (LLRM_MIN_FREE_GB): delete your old build trees and targets")


def use_dist() -> None:
    """LLRM_BIN is the dist directory, whatever it was: the gate sets it to its own links of the release build for every step (target/gate-bins),
    and a binary of another profile is refused by `measured_dir`, not measured. A dist LLRM_BIN (a snapshot, a stored base) is kept."""
    if PROFILE == "dist" and not Path(os.environ.get("LLRM_BIN", "")).name.startswith("dist"):
        os.environ["LLRM_BIN"] = str(ensure_dist())


def measure_all(jobs: int) -> dict:
    """This process's compiler (LLRM_BIN, else the tree's target): compile cost, and the cost at N/2, N and 2N of each axis and step."""
    compiler = measured_dir() / "llrm-cc"
    passes = scaling_gate.measure_passes(jobs)
    return {
        "method": method(),
        "binary": {"path": str(compiler), "sha": hashlib.sha256(compiler.read_bytes()).hexdigest()[:12]},
        "compile": compile_cost.measure(compiler, jobs),
        "axes": {k: list(v) for k, v in scaling_gate.measure(jobs).items()},
        "passes": {k: [round(v, 3) for v in got] for k, got in passes.items()},
    }


# --- the store -------------------------------------------------------------------------------------------------------


def tree_of(sha: str, cwd: Path | None = None) -> str | None:
    """The tree `sha` holds, or None for a name git does not know. What the compiler is made of is the tree, not the commit: the squash
    that lands a gated branch is another commit of the same tree, and its measurement is the gated branch's."""
    done = subprocess.run(["git", "rev-parse", "--verify", "--quiet", f"{sha}^{{tree}}"], cwd=cwd or ROOT, capture_output=True, text=True)
    return done.stdout.strip() if done.returncode == 0 else None


def stored_path(sha: str, which: str, by: str | None = None) -> Path:
    return CACHE / f"{by or sha}-{which}.json"


def stored(sha: str, which: str) -> dict | None:
    """The measurement of `sha`'s tree, else one stored under the commit itself (made before trees were the key)."""
    for key in (tree_of(sha), sha):
        if key and (one := stored_path(sha, which, key)).is_file():
            return json.loads(one.read_text())
    return None


def save(sha: str, data: dict) -> Path:
    """Written whole or not at all: a session reading while another writes never sees half a file."""
    CACHE.mkdir(parents=True, exist_ok=True)
    one = stored_path(sha, data["method"], tree_of(sha))
    with tempfile.NamedTemporaryFile("w", dir=CACHE, suffix=".part", delete=False) as part:
        part.write(json.dumps(data, separators=(",", ":")) + "\n")
    os.replace(part.name, one)
    return one


def git(*args: str, cwd: Path = ROOT) -> str:
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


def base_of(head: str, ref: str = "origin/main") -> str:
    """The commit `head` branches from: the merge-base with `ref`, or its first parent where it is `ref` itself."""
    base = git("merge-base", head, ref)
    return git("rev-parse", f"{head}^1") if base == git("rev-parse", head) else base


def has_cc(sha: str) -> bool:
    """Whether `sha` has the clang front end (llrm-cc): main has not, so a measurement of llrm-cc has no base there."""
    try:
        git("cat-file", "-e", f"{sha}:crates/driver/llrm-cc/Cargo.toml")
    except subprocess.CalledProcessError:
        return False
    return True


def base_with_cc(head: str, base_sha: str) -> str | None:
    """`base_sha` if it can be built and measured as llrm-cc; else the nearest first-parent ancestor of `head` that can (gate run 3: the
    cost step built main, which has llrm-c only: "no bin target named llrm-cc"); None where there is none."""
    if has_cc(base_sha):
        return base_sha
    return next((sha for sha in git("rev-list", "--first-parent", "-n", "300", head).split()[1:] if has_cc(sha)), None)


def checked_out(sha: str, tree: Path, source: Path = ROOT) -> Path:
    """`sha` checked out in `tree`, a clone of `source` that measure.py owns (made once, never a worktree of anyone's repository): the
    commit is fetched from `source`, then from `source`'s own origin, so one that exists only on the remote is built too."""
    if not (tree / ".git").exists():
        tree.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(["git", "clone", "--quiet", "--no-checkout", str(source), str(tree)], check=True)
        origin = subprocess.run(["git", "remote", "get-url", "origin"], cwd=source, capture_output=True, text=True)
        if origin.returncode == 0:
            git("remote", "add", "upstream", origin.stdout.strip(), cwd=tree)
    if subprocess.run(["git", "cat-file", "-e", f"{sha}^{{commit}}"], cwd=tree, capture_output=True).returncode:
        for remote in (str(source), "origin", "upstream"):
            if subprocess.run(["git", "fetch", "--quiet", remote, sha], cwd=tree, capture_output=True).returncode == 0:
                break
        else:
            raise SystemExit(f"measure: {sha[:9]} is in neither {source} nor its origin")
    git("checkout", "--quiet", "--detach", "--force", sha, cwd=tree)
    return tree


SNAPSHOTS_KEPT = 6


def snapshot(release: Path, sha: str, kind: str = "") -> Path:
    """The files of `release` linked into a directory of `sha`'s own (a hard link keeps the inode: the next build of another commit in the
    shared tree writes new files and leaves these as they were). A base was measured from the release directory itself, and when a session
    checked out another base in the tree meanwhile its binaries read as older than the source: "stale binaries" in the measure step of
    train 5. No dep-info is linked, so nothing is compared with a tree that has moved on. The oldest snapshots beyond SNAPSHOTS_KEPT go."""
    into = BUILD / "snapshots" / f"{kind}{sha[:12]}"
    into.mkdir(parents=True, exist_ok=True)
    for one in sorted(release.iterdir()) if release.is_dir() else []:
        if one.is_file() and one.suffix != ".d":
            staged = into / f".{one.name}.new"
            staged.unlink(missing_ok=True)
            os.link(one, staged)
            os.replace(staged, into / one.name)
    os.utime(into)
    for old in sorted((p for p in into.parent.iterdir() if p != into), key=lambda p: p.stat().st_mtime)[: max(0, len(list(into.parent.iterdir())) - SNAPSHOTS_KEPT)]:
        shutil.rmtree(old, ignore_errors=True)
    return into


def built(sha: str, profile: str | None = None) -> Path:
    """`sha` built in a tree and target directory of its own (reused, so each build is an increment) while holding the build lock, and
    its binaries as a snapshot that no later build changes: the lock covers the build, not what is measured with it. `profile` is the
    measurement's (dist: llrm-cc and llrm-llc alone); the bench's base needs every tool of the release build."""
    profile = profile or PROFILE
    if prebuilt := profile == "dist" and os.environ.get("LLRM_MEASURE_BASE_BIN"):  # CI builds the base in a job of its own, beside the head's
        return Path(prebuilt)
    with locked("build"):
        BUILD.mkdir(parents=True, exist_ok=True)
        require_space(BUILD)
        (BUILD / ".used").touch()
        if BUILD.parent == BUILDS:
            for tree in pruned(BUILDS, BUILD, BUILD_CAP):
                print(f"measure: removed the build tree {tree.name} to keep the cache within {BUILD_CAP >> 30} GB", flush=True)
        tree = checked_out(sha, BUILD / "tree")
        env = {**os.environ, "CARGO_TARGET_DIR": str(BUILD / "target")}
        env.pop("LLRM_BIN", None)
        done = subprocess.run(["bash", "-c", MEASURE_BUILDS[profile]], cwd=tree, env=env, capture_output=True, text=True)
        if done.returncode:
            raise SystemExit(f"measure: the base {sha[:9]} does not build:\n{done.stderr[-2000:]}")
        return snapshot(BUILD / "target" / profile, sha, profile)


SYSTEM_BUILD = f"env -u RUSTFLAGS cargo build {'--profile dist' if PROFILE == 'dist' else '--release'} -q --bin llrm-cc --bin llrm-llc --features system-alloc"


def built_system(sha: str) -> Path | None:
    """`sha`'s llrm-cc with the system allocator in a target of its own (reused), a snapshot of its release directory (as `built`'s); None
    where `sha` has no `system-alloc` feature (a base from before it)."""
    with locked("build"):
        BUILD.mkdir(parents=True, exist_ok=True)
        tree = checked_out(sha, BUILD / "tree")
        env = {**os.environ, "CARGO_TARGET_DIR": str(BUILD / "target-system")}
        env.pop("LLRM_BIN", None)
        if "system-alloc" not in (tree / "Cargo.toml").read_text():
            return None
        done = subprocess.run(["bash", "-c", SYSTEM_BUILD], cwd=tree, env=env, capture_output=True, text=True)
        if done.returncode:
            raise SystemExit(f"measure: {sha[:9]} does not build with the system allocator:\n{done.stderr[-2000:]}")
        return snapshot(BUILD / "target-system" / PROFILE, sha, "system-")


def system_passes(sha: str, cells: set[str], build=built_system) -> dict[str, list[float]] | None:
    """The steps of `cells` ("axis level") of `sha` measured with the system allocator, stored by the tree like the baseline (a cell is
    measured, and the compiler built, on a miss only); None where `sha` cannot be built that way."""
    which = method() + "-system"
    one = stored_path(sha, which, tree_of(sha))
    have = json.loads(one.read_text()) if one.is_file() else {}
    if missing := sorted(cells - have.keys()):
        if (binary := build(sha)) is None:
            return None
        previous = os.environ.get("LLRM_BIN")
        os.environ["LLRM_BIN"] = str(binary)
        try:
            for cell in missing:
                axis, level = cell.split(" ")
                with tempfile.TemporaryDirectory(dir=os.environ.get("CARGO_TARGET_DIR")) as tmp:
                    have[cell] = {k: [round(v, 3) for v in got] for k, got in scaling_gate.pass_costs(axis, level, Path(tmp)).items()}
        finally:
            os.environ.pop("LLRM_BIN") if previous is None else os.environ.__setitem__("LLRM_BIN", previous)
        CACHE.mkdir(parents=True, exist_ok=True)
        one.write_text(json.dumps(have, separators=(",", ":")) + "\n")
    return {k: v for cell in cells for k, v in have[cell].items()}


def confirmed(bad: list[str], base: dict, now: dict, base_sha: str, head: str, tol: dict, system=None) -> tuple[list[str], list[str]]:
    """(lines, bad) with each step rise kept only if it shows under the system allocator too: mimalloc's heap layout moves a step's count
    (#1755: `analysis through-memory` +27 Minstr under mimalloc, equal under glibc, though the step ran on the same input). The compile stays judged on the shipped binary and an axis less the steps dropped here
    (`without_layout`). A rise that cannot be checked stands."""
    steps = {b.split(": ")[0] for b in bad} & now["passes"].keys()
    if not steps:
        return [], bad
    cells = {" ".join(k.split(" ")[:2]) for k in steps}
    system = system or system_passes
    was, got = system(base_sha, cells), system(head, cells)
    if was is None or got is None:
        return [], bad
    judged = judged_axes(base, now)
    _, also = step_rises(was, got, tol, judged)
    kept = {b.split(": ")[0] for b in also}
    lines, out = [], []
    for b in bad:
        key = b.split(": ")[0]
        if key not in steps:
            out.append(b)
        elif key in kept:
            out.append(f"{b}; system allocator {'/'.join(map(str, got[key][:3]))} against {'/'.join(map(str, was[key][:3])) if key in was else 'none'} (mimalloc and system malloc both rose)")
        else:
            lines.append(f"{key}: not confirmed under the system allocator, a heap layout effect: {'/'.join(map(str, got.get(key, [])[:3]))} against {'/'.join(map(str, was.get(key, [])[:3]))}")
    dropped = sorted(steps - kept)
    if dropped:
        more, out = without_layout(out, dropped, base, now, tol)
        lines += more
    return lines, out


def without_layout(bad: list[str], dropped: list[str], base: dict, now: dict, tol: dict) -> tuple[list[str], list[str]]:
    """`bad` with the axis rises re-judged less the steps dropped as heap layout effects: the step is not a rise, so the part of the axis total that is
    its rise is not one either (#1991: `unrolled Os mir lsr` 117 -> 169 M under mimalloc, 149 M under both under the system allocator, was dropped as a
    step and still failed the axis it is part of). What is left of the axis is judged as before, so a real rise next to a dropped one stands."""
    cells = sorted({" ".join(key.split(" ")[:2]) for key in dropped} & base["axes"].keys() & now["axes"].keys())
    adjusted = dict(now["axes"])
    for key in dropped:
        cell = " ".join(key.split(" ")[:2])
        if cell in adjusted:
            was = base["passes"].get(key, [0.0, 0.0, 0.0])
            adjusted[cell] = [int(round(have - (got - before) * 1e6)) for have, got, before in zip(adjusted[cell], now["passes"][key], was)]
    _, again = axis_rises({c: base["axes"][c] for c in cells}, {c: adjusted[c] for c in cells}, tol)
    out = [b for b in bad if b.split(": ")[0] not in cells] + again
    notes = [
        f"{cell}: judged without {', '.join(k.split(' ', 2)[2] for k in dropped if k.startswith(cell + ' '))} (heap layout effects)"
        for cell in cells
        if any(b.startswith(cell + ": ") for b in bad)
    ]
    return notes, out


@contextlib.contextmanager
def locked(name: str, every: float = 60.0, clock=time.monotonic, sleep=time.sleep):
    """An exclusive lock named `name` between sessions (and threads: each opens the file itself). A wait says so every `every` seconds:
    the gate kills a step with no log growth and no CPU for 10 minutes, and #1755's measure waited that long for another session's build.
    `clock` and `sleep` are the test's, which waits on virtual time."""
    CACHE.mkdir(parents=True, exist_ok=True)
    with open(CACHE / f"{name}.lock", "w") as handle:
        began = last = clock()
        while True:
            try:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if clock() - last >= every:
                    last = clock()
                    print(f"measure: waiting {last - began:.0f}s for the lock {name[:24]} (another session holds it)", flush=True)
                sleep(min(0.5, every / 4))
        yield


def base_measurement(base: str, jobs: int, build=built) -> dict:
    """`base`'s measurement by this tree's method: stored, else made with `base` built by `build`. Sessions that miss the same base at
    once wait on the base's lock and read what the first stored; the one tree and target the builds share are taken one at a time
    (`built` holds the build lock for the build, and measures from a snapshot)."""
    which = method()
    if (got := stored(base, which)) is not None:
        return got
    with locked(f"{base}-{which}"):
        if (got := stored(base, which)) is not None:
            return got
        print(f"measure: no measurement of {base[:9]} by this method; building and measuring it", flush=True)
        previous = os.environ.get("LLRM_BIN")
        os.environ["LLRM_BIN"] = str(build(base))
        try:
            data = measure_all(jobs)
        finally:
            os.environ.pop("LLRM_BIN") if previous is None else os.environ.__setitem__("LLRM_BIN", previous)
        save(base, data)
        return data


# --- the comparison: a rise past tolerance fails ----------------------------------------------------------------------


def geomean(ratios: list[float]) -> float:
    return math.exp(sum(math.log(r) for r in ratios) / len(ratios))


def compile_rises(base: dict[str, int], now: dict[str, int], tol: dict) -> tuple[list[str], list[str]]:
    """Per level, for QCport and for the programs: the geomean of now/base and the worst file."""
    lines, bad = [], []
    for group, member in {"qcport": lambda k: k.startswith("qcport/"), "programs": lambda k: not k.startswith("qcport/")}.items():
        for level in compile_cost.LEVELS:
            ratios = {k: now[k] / base[k] for k in base.keys() & now.keys() if member(k) and k.endswith(" " + level)}
            if not ratios:
                lines.append(f"{group} {level}: not in both measurements")
                continue
            g, worst = geomean(list(ratios.values())), max(ratios, key=ratios.get)
            here = row_tolerance(tol, f"{group} {level}")
            lines.append(f"{group} {level}: geomean {g:.4f}, worst {ratios[worst]:.4f} ({worst.rsplit(' ', 1)[0]}), {len(ratios)} files")
            if g > here["geomean"]:
                bad.append(f"{group} {level}: geomean {g:.4f} > {here['geomean']}")
            if ratios[worst] > here["worst"]:
                bad.append(f"{group} {level}: {worst} {ratios[worst]:.4f} > {here['worst']}")
    return lines, bad


def second(half: float, small: float, big: float) -> float:
    """c(2N) - 3c(N) + 2c(N/2): of a cost a + bN + kN^2, 1.5kN^2. A fixed cost or a linear one cancels, so removing either leaves it alone
    (the excess c(2N) - 2c(N), -a + 2kN^2, rises when a fixed cost goes), and a quadratic term raises it."""
    return big - 3 * small + 2 * half


def superlinear(half: float, small: float, big: float) -> float:
    """`second` where it is above nothing: a sublinear cost (a ramp that flattens, a cap) has a negative one, which is no credit a later
    superlinear term may spend. A base that was concave and is now linear has not got worse; one that was linear and grows N^2 has."""
    return max(0.0, second(half, small, big))


# A cost that at most doubles when N does grows no faster than linear there (a little over, for the compile's own noise).
LINEAR = 2.05


def grown(half: float, small: float, big: float) -> float:
    """`superlinear` of a measurement now: nil where its last doubling is linear. A step (work that stops above some N: `live O1 peephole`
    ran copyprop and regthrash at n=32 only, 16.2/23.4/44.9) is not a slope, and three points cannot tell a step from a curve by their second
    difference. The base's is `superlinear` as it was: a fixed cost or linear work removed moves the ratio, not the second difference."""
    return 0.0 if big <= LINEAR * small else superlinear(half, small, big)


def two_n(key: str) -> str:
    """The size the cost at 2N is at, as 2N=128: the rows are N/2, N and 2N and name no size."""
    n = scaling_gate.SIZES.get(key.split(" ")[0])
    return f"2N={2 * n}" if n else "2N"


def reading() -> str:
    """What a step's number is: its own Minstr (not the total with the analyses it made the manager compute), on this build profile. A direct
    reading of the inclusive total, or of a release build, differs by ~4% each (#2318: x1.583 own/dist against x1.41 total/release)."""
    return f"own, {PROFILE}"


def axis_rises(base: dict[str, list[int]], now: dict[str, list[int]], general: dict) -> tuple[list[str], list[str]]:
    """Per axis and level: the second difference may not rise by more than `excess` of the base's cost at 2N, nor the cost at 2N by `worst`.
    A rise of the second difference stands only if the cost at 2N did not fall (by more than `fall` of the base's): the claim is that no
    step became worse at scale, and a row where every point got cheaper has nothing worse in it. Removing a cost that weighs more at N
    than at 2N does raise the second difference while it lowers all three (#1955: 542.4/1079.1/2214.1 -> 538.9/1063.4/2202.5 Minstr)."""
    lines, bad = [], []
    for key in sorted(base.keys() & now.keys()):
        (half, small, big), (was_half, was_small, was_big) = now[key], base[key]
        tol = row_tolerance(general, key)
        more = grown(half, small, big) - superlinear(was_half, was_small, was_big)
        lines.append(f"{key}: second difference {second(half, small, big) / was_big:+.4f} of the base's 2N (base {second(was_half, was_small, was_big) / was_big:+.4f}), {two_n(key)} x{big / was_big:.4f} ({half}/{small}/{big} against {was_half}/{was_small}/{was_big})")
        if more > tol["excess"] * was_big and big >= was_big * (1 - tol["fall"]):
            bad.append(f"{key}: superlinear work grew by {more / was_big:.4f} of the base's cost at 2N (> {tol['excess']}): a step is superlinear where it was not")
        if big > was_big * tol["worst"]:
            bad.append(f"{key}: cost at {two_n(key)} x{big / was_big:.4f} > {tol['worst']}")
    return lines, bad


def step_rises(
    base: dict[str, list[float]], now: dict[str, list[float]], general: dict, judged: frozenset[str] = frozenset()
) -> tuple[list[str], list[str]]:
    """A step whose cost at 2N rose by `step_rise` and `step_floor` Minstr fails whatever its share. Of `high` share or more, one whose second difference rose by `step_excess` of the compile's cost at 2N, or whose cost at 2N rose
    by `pass_slack`, fails. A step the base did not read is taken to have had none; one below `high`, or on the edge of the share floor,
    does not fail either way. A new step on an axis both measurements carry whose second difference did not rise (`judged`) has no step baseline, since work
    moved into an analysis of its own was in other steps' spans before: the axis rule judges it. On an axis that rose it is new work."""
    lines, bad = [], []
    for key, (half, small, big, whole) in sorted(now.items()):
        tol = row_tolerance(general, key)
        rise = max(tol["step_rise"], tol["pass_slack"])  # a row accepted at a higher pass_slack is accepted at that for this too
        if key in base and big > base[key][2] * rise and big - base[key][2] >= tol["step_floor"]:
            # whatever its share of the compile: a step that was 907 Minstr at 2N and is 13,840 was 0.4% of a compile and is not now
            lines.append(f"{key}: {two_n(key)} ({reading()}) x{big / base[key][2]:.3f} (+{big - base[key][2]:.1f} Minstr, {half}/{small}/{big} against {'/'.join(map(str, base[key][:3]))})")
            bad.append(f"{key}: cost at {two_n(key)} ({reading()}) x{big / base[key][2]:.3f} (> {rise}, +{big - base[key][2]:.1f} Minstr), whatever its share of the compile{risen_elsewhere(key, base, now)}")
            continue
        if big < tol["high"] * whole:
            continue
        if key not in base and " ".join(key.split(" ")[:2]) in judged:
            continue
        was_half, was_small, was_big, _ = base.get(key, [half, small, big, whole])
        more = grown(half, small, big) - (superlinear(was_half, was_small, was_big) if key in base else 0.0)
        if more > tol["step_excess"] * whole or (key in base and big > was_big * tol["pass_slack"]):
            shown = f"{half}/{small}/{big} against {was_half}/{was_small}/{was_big}"
            lines.append(f"{key}: second difference {second(half, small, big):.1f} Minstr (base {second(was_half, was_small, was_big) if key in base else 0.0:.1f}), {two_n(key)} ({reading()}) x{big / was_big:.3f} ({shown})")
            bad.append(f"{key}: superlinear work {second(half, small, big):.1f} Minstr rose by {more / whole:.4f} of the compile (> {tol['step_excess']}), or {two_n(key)} ({reading()}) x{big / was_big:.3f} ({shown}){risen_elsewhere(key, base, now)}")
    return lines, bad


def risen_elsewhere(key: str, base: dict[str, list[float]], now: dict[str, list[float]]) -> str:
    """The other steps of `key`'s axis and level whose cost at 2N rose most: work moved into an engine that runs inside another step's span
    flags that step, though the total falls, and the callee is where the rise is."""
    prefix = key.rsplit(" ", 1)[0].split(" ")[:2]
    rows = []
    for other, (_, _, big, _) in now.items():
        if other != key and other.split(" ")[:2] == prefix and other in base and big > base[other][2]:
            rows.append((big - base[other][2], other))
    rows.sort(reverse=True)
    return "; steps of the same axis and level that rose: " + ", ".join(f"{name.split(' ', 2)[2]} +{rise:.1f} Minstr" for rise, name in rows[:3]) if rows else ""


def judged_axes(base: dict, now: dict) -> frozenset[str]:
    return frozenset(axis for axis in base["axes"].keys() & now["axes"].keys() if superlinear(*now["axes"][axis]) <= superlinear(*base["axes"][axis]))


def rises(base: dict, now: dict, tol: dict | None = None) -> tuple[list[str], list[str]]:
    tol = tol or tolerances()
    lines, bad = [], []
    # An axis judges a new row only when its own second difference did not rise: work moved between rows leaves it flat or lower.
    judged = judged_axes(base, now)
    for part, check in (("compile", compile_rises), ("axes", axis_rises), ("passes", lambda b, n, t: step_rises(b, n, t, judged))):
        got = check(base[part], now[part], tol)
        lines += got[0]
        bad += got[1]
    return lines, bad


# --- creep: the parent forgives what the tolerance allows, ten times ----------------------------------------------------


def anchor_of(head: str, merges: int = 50, days: float = 7, cwd: Path = ROOT) -> str:
    """The commit on the first-parent line of `head` that is `merges` back or a week older, whichever is nearer `head`."""
    line = [row.split() for row in git("rev-list", "--first-parent", "--timestamp", head, cwd=cwd).splitlines()]
    by_count = min(merges, len(line) - 1)
    stamp = int(line[0][0]) - days * 86400
    by_age = next((at for at, (when, _) in enumerate(line) if int(when) <= stamp), len(line) - 1)
    return line[min(by_count, by_age)][1]


def trail(anchor: str, head: str, which: str, cwd: Path = ROOT) -> list[str]:
    """The commits from `anchor` to `head`, oldest first, each measured one with its programs' -O2 geomean against the last measured."""
    out, last = [], stored(anchor, which)
    for sha in reversed(git("rev-list", "--first-parent", f"{anchor}..{head}", cwd=cwd).split()):
        here = stored(sha, which)
        subject = git("log", "-1", "--format=%s", sha, cwd=cwd)[:90]
        if here is None or last is None:
            out.append(f"{sha[:9]} {subject}: not measured")
        else:
            ratios = [here["compile"][k] / last["compile"][k] for k in here["compile"].keys() & last["compile"].keys() if k.endswith(" -O2")]
            out.append(f"{sha[:9]} {subject}: -O2 geomean x{geomean(ratios):.4f} of the last measured")
        if here is not None:
            last = here
    return out


def creep(jobs: int, ref: str) -> int:
    """`ref` against its anchor at the same tolerances: ten commits each inside them still add up."""
    try:
        head = git("rev-parse", ref)
        which = method()
        now = stored(head, which) or measure_all(jobs)
        anchor = anchor_of(head)
        base = base_measurement(anchor, jobs)
    except NoCounter as why:
        print(f"SKIPPED: instruction counter unavailable ({why})")
        return 77
    lines, bad = rises(base, now)
    print(f"{head[:9]} against its anchor {anchor[:9]}")
    print("\n".join(lines))
    if bad:
        print("CREEP:", *bad, sep="\n  ")
        print("what added it, oldest first:", *trail(anchor, head, which), sep="\n  ")
        return 1
    return 0


# --- commands --------------------------------------------------------------------------------------------------------


def check(jobs: int, ref: str, build=built, head_dir=lambda: None) -> int:
    head = git("rev-parse", "HEAD")
    base_sha = base_with_cc(head, base_of(head, ref))
    if base_sha is None:
        print(f"SKIPPED: no ancestor of {head[:9]} has llrm-cc to measure it against")
        return 77
    # Both compilers are dist builds (fat LTO: minutes of one thread each): the base is built beside the head, not after it.
    with concurrent.futures.ThreadPoolExecutor(1) as pool:
        future = pool.submit(build, base_sha) if stored(base_sha, method()) is None else None
        head_dir()
        try:
            now = measure_all(jobs)
            base = base_measurement(base_sha, jobs, build=(lambda sha: future.result()) if future else build)
        except NoCounter as why:
            print(f"SKIPPED: instruction counter unavailable ({why})")
            return 77
    return judged(now, base, base_sha, head)


def judged(now: dict, base: dict, base_sha: str, head: str) -> int:
    lines, bad = rises(base, now)
    if bad:
        more, bad = confirmed(bad, base, now, base_sha, head, tolerances())
        lines += more
    print(f"measured {now['binary']['path']} ({now['binary']['sha']}) against {base_sha[:9]}")
    print("\n".join(lines))
    # The gated head is stored by its tree: the commit that lands it (a squash of the same tree) is the next run's base, measured here.
    if not git("status", "--porcelain", "--untracked-files=no") and stored(head, now["method"]) is None:
        save(head, now)
    if bad:
        print("COMPILE COST RISE:", *bad, sep="\n  ")
        return 1
    print(f"no rise past tolerance of {base_sha[:9]} ({len(now['compile'])} files, {len(now['axes'])} axes, {len(now['passes'])} steps) of {now['binary']['path']} ({now['binary']['sha']})")
    return 0


def compare(base_sha: str, sha: str) -> int:
    """`check` over two stored measurements of the current method: the same lines and verdict, nothing measured. A reading that
    disagrees with a fresh `show` is then a disagreement of the measurements, not of the comparison."""
    which = method()
    base, now = (stored(git("rev-parse", one), which) for one in (base_sha, sha))
    if base is None or now is None:
        print(f"no stored measurement of {base_sha if base is None else sha} by this method ({which})")
        return 2
    lines, bad = rises(base, now)
    print("\n".join(lines))
    print(*(["COMPILE COST RISE:", *bad] if bad else ["no rise past tolerance"]), sep="\n  ")
    return 1 if bad else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("command", choices=["check", "record", "show", "creep", "compare", "dist"])
    parser.add_argument("rest", nargs="*", help="record: SHA [BIN]; compare: BASE_SHA SHA, two stored measurements, nothing measured")
    parser.add_argument("--base", default=os.environ.get("LLRM_MEASURE_REF", "origin/main"), help="check: the branch the merge-base is taken with")
    parser.add_argument("--jobs", type=int, default=gate.default_jobs(os.environ))
    args = parser.parse_args()
    if args.command == "dist":  # the directory the measured llrm-cc is in, built if need be
        print(ensure_dist())
        return 0
    if args.command == "check":
        return check(args.jobs, args.base, head_dir=use_dist)
    if args.command == "compare":
        return compare(*args.rest)
    if args.command == "creep":
        return creep(args.jobs, args.rest[0] if args.rest else "HEAD")
    if args.command == "record":
        sha = git("rev-parse", args.rest[0])
        if len(args.rest) > 1:
            os.environ["LLRM_BIN"] = args.rest[1]
        try:
            print(save(sha, measure_all(args.jobs)))
        except NoCounter as why:
            print(f"SKIPPED: instruction counter unavailable ({why})")
            return 77
        return 0
    print(json.dumps(measure_all(args.jobs)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
