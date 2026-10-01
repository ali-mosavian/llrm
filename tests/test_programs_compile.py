"""Every program tools/sizes.py builds compiles, bar tools/sizes-known.txt.

Not run in CI. It needs the release binaries, current: set
LLRM_REQUIRE_BINARIES=1 to fail, not skip, without them.
"""

import os
import warnings
from pathlib import Path

import pytest

from tools import sizes

ROOT = Path(__file__).resolve().parents[1]
BINS = ROOT / "target" / "release"
KNOWN = ROOT / "tools" / "sizes-known.txt"
NAMES = ("llrm-qb", "llrm-c", "llrm-nib")
SOURCES = (".rs", ".toml", ".prs", ".lock", ".txt", ".isel", ".nib")


def _known() -> set[str]:
    lines = (one.split("#")[0].split() for one in KNOWN.read_text().splitlines())
    return {line[0] for line in lines if line}


def _only_tests(name: str) -> bool:
    """A file only `cargo test` builds does not make the binaries stale."""
    return name.endswith("_tests.rs") or name.startswith("test_") or name == "tests.rs"


def _newest_source() -> tuple[float, Path]:
    newest = (0.0, ROOT)
    for top in ("crates", "src"):
        for where, dirs, files in os.walk(ROOT / top):
            dirs[:] = [one for one in dirs if one not in ("target", "__pycache__")]
            for name in files:
                if name.endswith(SOURCES) and not _only_tests(name):
                    path = Path(where) / name
                    newest = max(newest, (path.stat().st_mtime, path))
    return newest


def _unusable() -> str | None:
    missing = [one for one in NAMES if not (BINS / one).exists()]
    if missing:
        return f"{', '.join(missing)} missing from {BINS}"
    built = min((BINS / one).stat().st_mtime for one in NAMES)
    changed, path = _newest_source()
    if changed > built:
        return f"{path.relative_to(ROOT)} is newer than the binaries"
    return None


def test_every_program_compiles_but_the_known_failures():
    """Seven programs (flags, jumps, huge, entries, tally, roster, speaker)
    failed to compile for days, unseen: sizes.py lists them only if it is run."""
    why = _unusable()
    if why:
        message = f"NOT RUN: {why}; cargo build --release --bins"
        if os.environ.get("LLRM_REQUIRE_BINARIES"):
            pytest.fail(message)
        warnings.warn(message, stacklevel=1)
        pytest.skip(message)
    built = sizes.table(BINS, "-O2", demos=sizes.DEMOS.is_dir())
    failing = {name for name, measured in built.items() if measured is None}
    known = _known()
    assert not failing - known, f"fail to compile, not in {KNOWN.name}: {sorted(failing - known)}"
    assert not known - failing, f"compile now, remove from {KNOWN.name}: {sorted(known - failing)}"
