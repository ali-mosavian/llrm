"""Every program tools/sizes.py builds compiles, bar tools/sizes-known.txt."""

from pathlib import Path

import pytest

from tools import sizes

ROOT = Path(__file__).resolve().parents[1]
BINS = ROOT / "target" / "release"
KNOWN = ROOT / "tools" / "sizes-known.txt"


def _known() -> set[str]:
    lines = (one.split("#")[0].split() for one in KNOWN.read_text().splitlines())
    return {line[0] for line in lines if line}


def test_every_program_compiles_but_the_known_failures():
    """Seven programs (flags, jumps, huge, entries, tally, roster, speaker)
    failed to compile for days, unseen: sizes.py lists them only if it is run."""
    missing = [one for one in ("llrm-qb", "llrm-c", "llrm-nib") if not (BINS / one).exists()]
    if missing:
        pytest.skip(f"NOT RUN: {', '.join(missing)} missing from {BINS}; cargo build --release --bins")
    built = sizes.table(BINS, "-O2", demos=False)
    failing = {name for name, measured in built.items() if measured is None}
    known = _known()
    assert not failing - known, f"fail to compile, not in {KNOWN.name}: {sorted(failing - known)}"
    assert not known - failing, f"compile now, remove from {KNOWN.name}: {sorted(known - failing)}"
