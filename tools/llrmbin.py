"""Where the built binaries are: LLRM_BIN, else $CARGO_TARGET_DIR/release, else <repo>/target/release.

The one resolver every Python tool and test uses; a tree built elsewhere (the gate's) has no ./target.
"""
import os
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


def declared(repo: Path) -> set[str]:
    """The binaries `cargo build --bins` builds: the root package's [[bin]] names (others in target/ are leftovers)."""
    import tomllib

    manifest = repo / "Cargo.toml"
    return {one["name"] for one in tomllib.loads(manifest.read_text()).get("bin", [])} if manifest.is_file() else set()


def stale_binaries(directory: Path, repo: Path = REPO) -> list[str]:
    """The binaries in `directory` older than a file cargo's dep-info says they are built from, or than the toolchain
    patches (build.rs reruns on them; dep-info does not list them). A directory without dep-info is not checked."""
    patches = [one.stat().st_mtime for one in (repo / "toolchain/owshim").rglob("*") if one.is_file()]
    stale, built_here = [], declared(repo)
    for info in sorted(directory.glob("*.d")) if directory.is_dir() else []:
        binary = info.with_suffix("")
        if binary.name not in built_here or not binary.is_file():
            continue
        built = binary.stat().st_mtime
        sources = [Path(path) for path in info.read_text().partition(":")[2].replace("\\\n", " ").split()]
        newest = max([source.stat().st_mtime for source in sources if source.is_file()] + patches, default=0)
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
