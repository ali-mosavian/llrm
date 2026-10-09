#!/usr/bin/env python3
"""What llrm-c costs to compile and how that cost grows, compared with the same measurement of the commit this one branches from.

    tools/measure.py check              measure this tree's compiler, compare with the merge-base's; exit 1 on a rise past tolerance
    tools/measure.py record SHA [BIN]   measure the compiler in BIN (default: this tree's) and store it as SHA's
    tools/measure.py show               print this tree's measurement
    tools/measure.py creep [REF]        compare REF (HEAD) with the commit 50 merges or a week back at the same tolerances

Three measurements, all user-space instructions (`perf stat`), so a loaded host does not move them: the compile of the 66 vsgcc
programs and QCport's modules at -O1/-O2/-Os (tools/compile-cost.py); the ratio of the compile at 2N to N on generated programs
(vsgcc/scaling_gate.py); and the same ratio for each step of the compile (`LLRM_DEBUG=time`'s [instr] rows).

A measurement is stored per commit under ~/.cache/llrm/measure (LLRM_MEASURE_DIR), never in the repository, so two branches never
touch a shared file. The base's is read from there; if it is missing the base is built in a tree and target directory of its own
and measured with this tree's tools and inputs (only the compiler differs), then stored. A rise past the tolerances in tiers.toml
[measure] fails; a drop does not need recording, the next branch's base has it. LLVM's compile-time tracker compares with the
parent the same way.
"""
from __future__ import annotations

import argparse
import contextlib
import fcntl
import hashlib
import importlib.util
import json
import math
import os
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
CACHE = Path(os.environ.get("LLRM_MEASURE_DIR") or Path.home() / ".cache/llrm/measure")


# The profile the measured compiler is built with: `release` (the PR gate: two commits built the same way), or `dist` (the shipped build, which
# the creep run on main measures). A measurement is of one profile, so the profile is part of the method.
PROFILE = os.environ.get("LLRM_MEASURE_PROFILE") or "release"


def build_tree(root: Path = ROOT, env: dict | None = None) -> Path:
    """Where the base is built: `LLRM_MEASURE_BUILD`, else a tree of this repository's own. Two clones share a tree no `git checkout` of
    the other's commit can enter ('unable to read tree'), so each is keyed by the path of its working tree."""
    env = os.environ if env is None else env
    return Path(env.get("LLRM_MEASURE_BUILD") or Path.home() / ".cache/llrm/measure-build" / hashlib.sha256(str(root).encode()).hexdigest()[:10])


BUILD = build_tree()
VSGCC = next((ROOT / "crates/target").glob("*/vsgcc"))
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(VSGCC))

import llrmbin  # noqa: E402
import scaling_gate  # noqa: E402

sys.path.insert(0, str(HERE / "gate"))
import gate  # noqa: E402

DIST_BUILD = gate.DIST_BUILD

_spec = importlib.util.spec_from_file_location("compile_cost", HERE / "compile-cost.py")
compile_cost = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(compile_cost)

NoCounter = compile_cost.NoCounter


def tolerances(path: Path = HERE / "gate" / "tiers.toml") -> dict:
    return tomllib.loads(path.read_text())["measure"]


def method() -> str:
    """What the measurement is made of besides the compiler: the tools, the programs and QCport's modules. A stored measurement of
    another method is not comparable and is made again."""
    files = [HERE / "compile-cost.py", *(VSGCC / name for name in ("scaling_gate.py", "scaling.py", "levels_time.py", "programs.py", "wrap.py"))]
    files += sorted((ROOT / "bench").glob("*/*.c")) + sorted((VSGCC / "kernels").glob("*/*.c"))
    if qcport := os.environ.get("QCPORT"):
        files += sorted(Path(qcport).expanduser().glob("*/*.c"))
    digest = hashlib.sha256()
    for one in files:
        digest.update(str(one.name).encode() + b"\0" + one.read_bytes())
    digest.update(b"qcport" if qcport else b"-")
    digest.update(PROFILE.encode())
    return digest.hexdigest()[:12]


def measure_all(jobs: int) -> dict:
    """This process's compiler (LLRM_BIN, else the tree's target): compile cost, and the cost at N/2, N and 2N of each axis and step."""
    compiler = llrmbin.bin_dir() / "llrm-c"
    passes = scaling_gate.measure_passes(jobs)
    return {
        "method": method(),
        "binary": {"path": str(compiler), "sha": hashlib.sha256(compiler.read_bytes()).hexdigest()[:12]},
        "compile": compile_cost.measure(compiler, jobs),
        "axes": {k: list(v) for k, v in scaling_gate.measure(jobs).items()},
        "passes": {k: [round(v, 3) for v in got] for k, got in passes.items()},
    }


# --- the store -------------------------------------------------------------------------------------------------------


def stored_path(sha: str, which: str) -> Path:
    return CACHE / f"{sha}-{which}.json"


def stored(sha: str, which: str) -> dict | None:
    one = stored_path(sha, which)
    return json.loads(one.read_text()) if one.is_file() else None


def save(sha: str, data: dict) -> Path:
    """Written whole or not at all: a session reading while another writes never sees half a file."""
    CACHE.mkdir(parents=True, exist_ok=True)
    one = stored_path(sha, data["method"])
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
        for remote in ("origin", "upstream"):
            if subprocess.run(["git", "fetch", "--quiet", remote, sha], cwd=tree, capture_output=True).returncode == 0:
                break
        else:
            raise SystemExit(f"measure: {sha[:9]} is in neither {source} nor its origin")
    git("checkout", "--quiet", "--detach", "--force", sha, cwd=tree)
    return tree


def built(sha: str) -> Path:
    """`sha` built in a tree and target directory of its own (reused, so each build is an increment); its release directory."""
    BUILD.mkdir(parents=True, exist_ok=True)
    tree = checked_out(sha, BUILD / "tree")
    env = {**os.environ, "CARGO_TARGET_DIR": str(BUILD / "target")}
    env.pop("LLRM_BIN", None)
    done = subprocess.run(["bash", "-c", DIST_BUILD if PROFILE == "dist" else gate.MEASURE_BUILD], cwd=tree, env=env, capture_output=True, text=True)
    if done.returncode:
        raise SystemExit(f"measure: the base {sha[:9]} does not build:\n{done.stderr[-2000:]}")
    return BUILD / "target" / PROFILE


@contextlib.contextmanager
def locked(name: str):
    """An exclusive lock named `name` between sessions (and threads: each opens the file itself)."""
    CACHE.mkdir(parents=True, exist_ok=True)
    with open(CACHE / f"{name}.lock", "w") as handle:
        fcntl.flock(handle, fcntl.LOCK_EX)
        yield


def base_measurement(base: str, jobs: int, build=built) -> dict:
    """`base`'s measurement by this tree's method: stored, else made with `base` built by `build`. Sessions that miss the same base at
    once wait on the base's lock and read what the first stored; the one tree and target the builds share are taken one at a time."""
    which = method()
    if (got := stored(base, which)) is not None:
        return got
    with locked(f"{base}-{which}"):
        if (got := stored(base, which)) is not None:
            return got
        print(f"measure: no measurement of {base[:9]} by this method; building and measuring it", flush=True)
        with locked("build"):
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
            lines.append(f"{group} {level}: geomean {g:.4f}, worst {ratios[worst]:.4f} ({worst.rsplit(' ', 1)[0]}), {len(ratios)} files")
            if g > tol["geomean"]:
                bad.append(f"{group} {level}: geomean {g:.4f} > {tol['geomean']}")
            if ratios[worst] > tol["worst"]:
                bad.append(f"{group} {level}: {worst} {ratios[worst]:.4f} > {tol['worst']}")
    return lines, bad


def second(half: float, small: float, big: float) -> float:
    """c(2N) - 3c(N) + 2c(N/2): of a cost a + bN + kN^2, 1.5kN^2. A fixed cost or a linear one cancels, so removing either leaves it alone
    (the excess c(2N) - 2c(N), -a + 2kN^2, rises when a fixed cost goes), and a quadratic term raises it."""
    return big - 3 * small + 2 * half


def superlinear(half: float, small: float, big: float) -> float:
    """`second` where it is above nothing: a sublinear cost (a ramp that flattens, a cap) has a negative one, which is no credit a later
    superlinear term may spend. A base that was concave and is now linear has not got worse; one that was linear and grows N^2 has."""
    return max(0.0, second(half, small, big))


def axis_rises(base: dict[str, list[int]], now: dict[str, list[int]], tol: dict) -> tuple[list[str], list[str]]:
    """Per axis and level: the second difference may not rise by more than `excess` of the base's cost at 2N, nor the cost at 2N by `worst`."""
    lines, bad = [], []
    for key in sorted(base.keys() & now.keys()):
        (half, small, big), (was_half, was_small, was_big) = now[key], base[key]
        more = superlinear(half, small, big) - superlinear(was_half, was_small, was_big)
        lines.append(f"{key}: second difference {second(half, small, big) / was_big:+.4f} of the base's 2N (base {second(was_half, was_small, was_big) / was_big:+.4f}), 2N x{big / was_big:.4f} ({half}/{small}/{big} against {was_half}/{was_small}/{was_big})")
        if more > tol["excess"] * was_big:
            bad.append(f"{key}: superlinear work grew by {more / was_big:.4f} of the base's cost at 2N (> {tol['excess']}): a step is superlinear where it was not")
        if big > was_big * tol["worst"]:
            bad.append(f"{key}: cost at 2N x{big / was_big:.4f} > {tol['worst']}")
    return lines, bad


def step_rises(base: dict[str, list[float]], now: dict[str, list[float]], tol: dict) -> tuple[list[str], list[str]]:
    """A step of `high` share or more whose second difference rose by `step_excess` of the compile's cost at 2N, or whose cost at 2N rose
    by `pass_slack`, fails. A step the base did not read is taken to have had none; one below `high`, or on the edge of the share floor,
    does not fail either way."""
    lines, bad = [], []
    for key, (half, small, big, whole) in sorted(now.items()):
        if big < tol["high"] * whole:
            continue
        was_half, was_small, was_big, _ = base.get(key, [half, small, big, whole])
        more = superlinear(half, small, big) - (superlinear(was_half, was_small, was_big) if key in base else 0.0)
        if more > tol["step_excess"] * whole or (key in base and big > was_big * tol["pass_slack"]):
            shown = f"{half}/{small}/{big} against {was_half}/{was_small}/{was_big}"
            lines.append(f"{key}: second difference {second(half, small, big):.1f} Minstr (base {second(was_half, was_small, was_big) if key in base else 0.0:.1f}), 2N x{big / was_big:.3f} ({shown})")
            bad.append(f"{key}: superlinear work {second(half, small, big):.1f} Minstr rose by {more / whole:.4f} of the compile (> {tol['step_excess']}), or 2N x{big / was_big:.3f} ({shown}){risen_elsewhere(key, base, now)}")
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


def rises(base: dict, now: dict, tol: dict | None = None) -> tuple[list[str], list[str]]:
    tol = tol or tolerances()
    lines, bad = [], []
    for part, check in (("compile", compile_rises), ("axes", axis_rises), ("passes", step_rises)):
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


def check(jobs: int, ref: str) -> int:
    try:
        now = measure_all(jobs)
        head = git("rev-parse", "HEAD")
        base_sha = base_of(head, ref)
        base = base_measurement(base_sha, jobs)
    except NoCounter as why:
        print(f"SKIPPED: instruction counter unavailable ({why})")
        return 77
    lines, bad = rises(base, now)
    print(f"measured {now['binary']['path']} ({now['binary']['sha']}) against {base_sha[:9]}")
    print("\n".join(lines))
    # A commit of the reference branch is a base for the next branches; a branch's own head is not.
    if not git("status", "--porcelain", "--untracked-files=no") and stored(head, now["method"]) is None and subprocess.run(["git", "merge-base", "--is-ancestor", head, ref], cwd=ROOT).returncode == 0:
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
    parser.add_argument("command", choices=["check", "record", "show", "creep", "compare"])
    parser.add_argument("rest", nargs="*", help="record: SHA [BIN]; compare: BASE_SHA SHA, two stored measurements, nothing measured")
    parser.add_argument("--base", default=os.environ.get("LLRM_MEASURE_REF", "origin/main"), help="check: the branch the merge-base is taken with")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "4")))
    args = parser.parse_args()
    if args.command == "check":
        return check(args.jobs, args.base)
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
