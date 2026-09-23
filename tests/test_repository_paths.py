"""Repository filenames stay portable across case-sensitive filesystems."""

from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
CARGO_PATHS = frozenset(
    {
        "Cargo.lock",
        "Cargo.toml",
    }
)


def test_tracked_paths_are_lowercase_and_casefold_unique() -> None:
    paths = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.split("\0")[:-1]

    assert CARGO_PATHS <= set(paths)
    assert all(path == path.lower() for path in set(paths) - CARGO_PATHS)
    assert len(paths) == len({path.casefold() for path in paths})
