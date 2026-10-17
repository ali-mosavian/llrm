"""Where the built binaries are: LLRM_BIN, else $CARGO_TARGET_DIR/release, else <repo>/target/release.

The one resolver every Python tool and test uses; a tree built elsewhere (the gate's) has no ./target.
"""
import atexit
import hashlib
import os
import re
import shutil
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]


def bin_dir(env: dict | None = None, repo: Path = REPO) -> Path:
    env = os.environ if env is None else env
    if env.get("LLRM_BIN"):
        found = Path(env["LLRM_BIN"])
    elif env.get("CARGO_TARGET_DIR"):
        found = Path(env["CARGO_TARGET_DIR"]) / "release"
    else:
        found = target_dir(env, repo) / "release"
    stale = stale_binaries(found, repo)
    if stale:
        raise SystemExit(f"stale binaries in {found}: {', '.join(stale)} older than the source they are built from; run `cargo build --release --bins` (`cargo build -p X` builds only the library)")
    return found


BUILD_MARK = re.compile(rb"llrm-build:profile=([A-Za-z0-9_-]+);incremental=([01])")


def build_info(binary: Path) -> dict | None:
    """`{"profile": "dist", "incremental": False}` as the binary says it was built (llrm-mir's `buildinfo::MARK`, which
    `llrm-c --build-info` prints), None for one that does not say."""
    found = BUILD_MARK.search(Path(binary).read_bytes())
    return {"profile": found[1].decode(), "incremental": found[2] == b"1"} if found else None


def require_clean(binary: Path, env: dict | None = None, strict: bool = True, allow_dev: bool = False) -> dict | None:
    """Refuses a binary that is not the `dist` profile built without incremental compilation: a count from an incremental build
    moves by +-0.5% an edit (inlining of unrelated code), from a release build with 256 units by more than the changes measured.
    Build it with `CARGO_INCREMENTAL=0 cargo build --profile dist --bins` in a fresh target directory. `allow_dev` (the tools'
    `--allow-dev`) or LLRM_ALLOW_DEV=1 measures anyway, with the warning; `strict=False` is the gate's, which records what it measured
    with and warns. Returns what the binary says."""
    env = os.environ if env is None else env
    info = build_info(binary)
    if info and info["profile"] == "dist" and not info["incremental"]:
        return info
    got = (
        "does not say how it was built (an older build)"
        if info is None
        else f"is the {info['profile']} profile, " + ("incremental" if info["incremental"] else "not incremental")
    )
    why = f"{binary} {got}: build it with `CARGO_INCREMENTAL=0 cargo build --profile dist --bins` in a fresh target directory"
    if allow_dev or env.get("LLRM_ALLOW_DEV") == "1" or not strict:
        print(f"WARNING: {why}", file=sys.stderr)
        return info
    raise SystemExit(f"refusing to measure: {why} (--allow-dev measures anyway)")


def declared(repo: Path) -> set[str]:
    """The binaries `cargo build --bins` builds: the root package's [[bin]] names (others in target/ are leftovers)."""
    import tomllib

    manifest = repo / "Cargo.toml"
    return {one["name"] for one in tomllib.loads(manifest.read_text()).get("bin", [])} if manifest.is_file() else set()


def built_from(directory: Path, default: Path) -> Path:
    """The tree the binaries in `directory` were built from: the root (a Cargo.toml beside a crates/) above the first workspace source
    their dep-info lists, else `default`. A measurement's base is built in a tree of its own, not in the one being gated."""
    for info in sorted(directory.glob("*.d")) if directory.is_dir() else []:
        for path in info.read_text().partition(":")[2].replace("\\\n", " ").split():
            for parent in Path(path).parents:
                if (parent / "Cargo.toml").is_file() and (parent / "crates").is_dir():
                    return parent
    return default


def stale_binaries(directory: Path, repo: Path = REPO) -> list[str]:
    """The binaries in `directory` older than a file cargo's dep-info says they are built from, or than the toolchain
    patches (build.rs reruns on them; dep-info does not list them) of the tree they were built from, which is `repo` unless
    their dep-info names another. A directory without dep-info is not checked."""
    repo = built_from(directory, repo)
    patches = [one.stat().st_mtime for one in (repo / "toolchain/owshim").rglob("*") if one.is_file()]
    stale, built_here = [], declared(repo)
    for info in sorted(directory.glob("*.d")) if directory.is_dir() else []:
        binary = info.with_suffix("")
        if binary.name not in built_here or not binary.is_file():
            continue
        built = binary.stat().st_mtime
        sources = [Path(path) for path in info.read_text().partition(":")[2].replace("\\\n", " ").split()]
        # Sources only: a build script rewrites its outputs under the target directory whenever it reruns.
        written = directory.resolve().parent
        tracked = [source for source in sources if source.is_file() and source.resolve().is_relative_to(repo.resolve()) and not source.resolve().is_relative_to(written)]
        newest = max([source.stat().st_mtime for source in tracked] + patches, default=0)
        if newest > built:
            stale.append(binary.name)
    return stale


def target_dir(env: dict | None = None, repo: Path = REPO) -> Path:
    """Cargo's target directory, which the tools' scratch work goes under: CARGO_TARGET_DIR, else <repo>/target."""
    env = os.environ if env is None else env
    return Path(env["CARGO_TARGET_DIR"]) if env.get("CARGO_TARGET_DIR") else repo / "target"


if __name__ == "__main__":
    import sys

    kinds = {"bin": bin_dir, "target": target_dir}
    if len(sys.argv) != 2 or sys.argv[1] not in kinds:
        sys.exit("usage: llrmbin.py bin|target")
    print(kinds[sys.argv[1]]())


PINNED_BYTES = 120
_pins: list[str] = []


def pinned(binary: Path) -> Path:
    """`binary` by a path of PINNED_BYTES bytes, a symlink of this process's own. The compiler reads its argv[0] at start-up and the
    length of it moved an instruction count by 1.6e-4 to 5e-4 (`functions` at 2N: 3,192,407,376 from a short directory, 3,192,925,702
    from a long one), the size of a tolerance on a ratio: the tree a gate builds in and the one a base was measured in differ in it."""
    if not _pins:
        _pins.append(tempfile.mkdtemp(prefix="llrm-pin-", dir="/tmp"))
        atexit.register(shutil.rmtree, _pins[0], True)
    key = hashlib.sha256(str(binary).encode()).hexdigest()[:8]
    room = PINNED_BYTES - len(_pins[0]) - len("/llrm-c") - 1
    folder = Path(_pins[0]) / (key + "p" * (room - len(key)))
    folder.mkdir(exist_ok=True)
    link = folder / binary.name
    try:
        link.symlink_to(os.path.abspath(binary))
    except FileExistsError:  # another thread of this process pinned it first
        pass
    return link
