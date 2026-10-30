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


if __name__ == "__main__":
    rest, named, allow = options(sys.argv[1:])
    for one in rest:
        check(one, named["sha"], allow)
