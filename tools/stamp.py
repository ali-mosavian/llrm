"""The commit a binary was built from: `llrm-llc --version` prints `llrm-llc SHA` or `llrm-llc SHA+dirty`.

A measurement names the sha it means to measure (`--expect-sha SHA`, or LLRM_EXPECT_SHA) and this refuses a binary
built from another tree (a shared worktree moved during the build gave a number for the wrong commit) and one built
from a tree with changes unless `--allow-dirty` (or LLRM_ALLOW_DIRTY=1) says so. The stamp is printed, never hashed.
"""
import os
import hashlib
import subprocess
import sys
from pathlib import Path


def flags(binary) -> tuple[str, set[str]]:
    """(sha, flags) of the stamp `llrm-llc SHA[+dirty][+incremental]`; SystemExit if it prints none."""
    binary = Path(binary)
    if binary.name == "llrm-cc":
        binary = binary.with_name("llrm-llc")
    try:
        done = subprocess.run([str(binary), "--version"], capture_output=True, text=True)
    except OSError as why:
        raise SystemExit(f"{binary}: no build stamp ({why})")
    words = done.stdout.split()
    if done.returncode != 0 or len(words) != 2:
        raise SystemExit(f"{binary}: no build stamp (`--version` printed {done.stdout.strip()!r}); rebuild it from the tree being measured")
    sha, *found = words[1].split("+")
    return sha, set(found)


def refuse_incremental(binary) -> None:
    """A count measured on an incremental build is not the cost gate's: the object writer runs ~13% more instructions in one (equal `object write`
    9.5 -> 10.9 Minstr on one commit, #5744 round 1), so budgets.toml rows are baselined on CARGO_INCREMENTAL=0 builds. LLRM_ALLOW_INCREMENTAL=1 measures anyway."""
    if os.environ.get("LLRM_ALLOW_INCREMENTAL") == "1":
        return
    sha, found = flags(binary)
    if "incremental" in found:
        raise SystemExit(f"{binary} was built from {sha} incrementally: its instruction counts are not the gate's; rebuild with CARGO_INCREMENTAL=0 (LLRM_ALLOW_INCREMENTAL=1 to measure anyway)")


def read(binary) -> tuple[str, bool]:
    """(sha, dirty) a binary prints for --version; SystemExit if it prints none (built before the stamp). `llrm-cc` forwards
    --version to the pinned clang: its stamp is the `llrm-llc` beside it."""
    sha, found = flags(binary)
    return sha, "dirty" in found


def verify_dist(binary) -> None:
    """A binary in SCRATCH/sbp-SHA/dist whose `ok` recorded md5s (pair_build.sh) must still have the recorded one: a file
    copied over it after the build is refused. A dist without recorded sums (built before) is not checked."""
    path = Path(binary).resolve()
    ok = path.parent.parent / "ok"
    if path.parent.name != "dist" or not ok.is_file():
        return
    for line in ok.read_text().splitlines():
        want, _, name = line.partition("  ")
        if name == path.name and len(want) == 32:
            got = hashlib.md5(path.read_bytes()).hexdigest()
            if got != want:
                raise SystemExit(f"{binary}: md5 {got} is not the {want} recorded when it was built ({ok}): refused")


def check(binary, expect_sha: str | None = None, allow_dirty: bool = False, label: str = "") -> str:
    """Prints the stamp; refuses a sha that does not start with `expect_sha` and a dirty tree unless allowed. Returns the sha."""
    verify_dist(binary)
    sha, dirty = read(binary)
    print(f"stamp {label or binary}: {sha}{'+dirty' if dirty else ''}", file=sys.stderr)
    if expect_sha and not sha.startswith(expect_sha) and not expect_sha.startswith(sha):
        raise SystemExit(f"{binary} was built from {sha}, not {expect_sha}: refused")
    if dirty and not allow_dirty:
        raise SystemExit(f"{binary} was built from {sha} with changes: refused (--allow-dirty to measure it anyway)")
    return sha


def stamped(binary) -> bool:
    """Whether `binary` answers --version with a stamp (a tree from before a backend had one does not)."""
    try:
        flags(binary)
    except SystemExit:
        return False
    return True


def same_build(path: Path, llc: Path) -> bool:
    """Whether `path` came from the build that made `llc`: the dist's build record (pair_build.sh's `ok` beside `dist/`) lists
    both with the md5s they still have. A dist without a record proves nothing (a file copied in later has a fresh mtime too)."""
    ok = path.resolve().parent.parent / "ok"
    if path.resolve().parent.name != "dist" or not ok.is_file():
        return False
    recorded = {name: want for want, _, name in (line.partition("  ") for line in ok.read_text().splitlines()) if len(want) == 32}
    return all(recorded.get(one.name) == hashlib.md5(one.read_bytes()).hexdigest() for one in (path, llc))


def require_clean(dist, sha: str) -> str:
    """`sha` when `dist`'s llrm-llc was built from it, clean, and so was every other stamped compiler in it (llrm-nib, llrm-qb,
    each llrm-*-llc); SystemExit naming the first that was not. A dist built from e83149f+dirty stood for b6541d3901 in a whole A/B.
    A backend that prints no stamp (a tree from before it had one) is the sha only as part of llrm-llc's build (`same_build`):
    named on stderr as unstamped, and refused when it came from anywhere else."""
    dist = Path(dist)
    llc = dist / "llrm-llc"
    if not llc.is_file():
        raise SystemExit(f"{dist}: no llrm-llc, no stamped compiler: refused")
    check(llc, sha, False, f"{dist}/llrm-llc")
    others = sorted(path for path in dist.glob("llrm-*") if path.name in ("llrm-nib", "llrm-qb") or (path.name.endswith("-llc") and path.name != "llrm-llc"))
    unstamped = []
    for path in others:
        if stamped(path):
            check(path, sha, False, f"{dist}/{path.name}")
        elif same_build(path, llc):
            unstamped.append(path.name)
        else:
            raise SystemExit(f"{path} prints no stamp and is not from {llc.name}'s build (no build record lists both; tools/pairs/pair_build.sh writes one): refused")
    if unstamped:
        print(f"unstamped (pre-stamp tree): {' '.join(unstamped)}", file=sys.stderr)
    return read(llc)[0]


def options(argv: list[str]) -> tuple[list[str], dict[str, str | None], bool]:
    """(argv without the options, {"sha", "base_sha"}, allow_dirty): `--expect-sha` names the build measured (the new side
    of a pair), `--expect-base-sha` the base side; the environment (LLRM_EXPECT_SHA, LLRM_EXPECT_BASE_SHA,
    LLRM_ALLOW_DIRTY=1) names them when the line does not."""
    rest = []
    named = {"sha": os.environ.get("LLRM_EXPECT_SHA") or None, "base_sha": os.environ.get("LLRM_EXPECT_BASE_SHA") or None}
    allow = os.environ.get("LLRM_ALLOW_DIRTY") == "1"
    keys = {"--expect-sha": "sha", "--expect-base-sha": "base_sha"}
    it = iter(argv)
    for one in it:
        flag, eq, value = one.partition("=")
        if flag in keys:
            named[keys[flag]] = value if eq else next(it)
        elif one == "--allow-dirty":
            allow = True
        else:
            rest.append(one)
    return rest, named, allow


def require_named(dist, side: str = "sha") -> str:
    """`require_clean` against the commit the environment names for `side`: LLRM_EXPECT_SHA (the new side, or the only one)
    or LLRM_EXPECT_BASE_SHA; SystemExit when it names none. A measurement that names no commit measures no commit."""
    variable = {"sha": "LLRM_EXPECT_SHA", "base_sha": "LLRM_EXPECT_BASE_SHA"}[side]
    sha = os.environ.get(variable)
    if not sha:
        raise SystemExit(f"{variable} names no commit for {dist}: refused")
    return require_clean(dist, sha)


if __name__ == "__main__":
    if sys.argv[1:2] == ["--require-clean"]:
        # stamp.py --require-clean sha|base_sha DIST: the shell tools' form of require_named.
        require_named(sys.argv[3], sys.argv[2])
        sys.exit(0)
    rest, named, allow = options(sys.argv[1:])
    for one in rest:
        check(one, named["sha"], allow)


COMPILERS = ("llrm-nib", "llrm-qb", "llrm-llc")


def dist_stamps(found) -> dict[str, tuple[str, str]]:
    """{name: (sha, md5)} of each compiler in the dist `found` that is there and prints a stamp."""
    out = {}
    for name in COMPILERS:
        path = Path(found) / name
        if path.is_file():
            out[name] = (flags(path)[0], hashlib.md5(path.read_bytes()).hexdigest())
    return out


def require_ancestor(found, commit, repo) -> dict[str, tuple[str, str]]:
    """`dist_stamps(found)` when every compiler in it was built from `commit` or an ancestor of it; SystemExit naming the first that was not.
    A table names the commit it measured: the backend's stamp alone let a frontend from before a fix pass for one built after it."""
    recorded = dist_stamps(found)
    for name, (sha, _) in recorded.items():
        if subprocess.run(["git", "-C", str(repo), "merge-base", "--is-ancestor", sha, commit], capture_output=True).returncode != 0:
            raise SystemExit(f"{found}/{name} is built from {sha}, not an ancestor of {commit}: its numbers are not that commit's")
    return recorded


def require_pair(base, new, head, tip, repo) -> None:
    """An A/B pair measures a PR only when `new` was built from `head`'s code and `base` from the merge-base of `head` and `tip`; SystemExit naming
    the side that was not. A pair built before the PR's last merge of tip measured a base several commits older than the one the PR lands on
    (d99, 11-16: +740 B on one cell that no tip build shows)."""

    def git(*args):
        return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True)

    new_sha, base_sha = flags(Path(new) / "llrm-llc")[0], flags(Path(base) / "llrm-llc")[0]
    if git("diff", "--quiet", new_sha, head, "--", "crates").returncode != 0:
        raise SystemExit(f"{new} is built from {new_sha}, whose crates/ differ from the PR head {head}: it measured another tree")
    want = git("merge-base", head, tip).stdout.strip()
    if not want or not (base_sha.startswith(want) or want.startswith(base_sha)):
        raise SystemExit(f"{base} is built from {base_sha}, not {want}, the merge-base of {head} and {tip}: its base is stale")
