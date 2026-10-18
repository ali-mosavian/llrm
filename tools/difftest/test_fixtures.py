"""Pinned dump pairs with known answers: a change to the normaliser or to a printer that moves a count fails here, not in a headline."""

import sys
import tomllib
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import difftest  # noqa: E402

FIXTURES = Path(__file__).parent / "fixtures"
EXPECTED = tomllib.loads((FIXTURES / "expected.toml").read_text())


def answer(name: str) -> dict:
    ours = difftest.normalise((FIXTURES / f"{name}.llrm.ll").read_text())
    theirs = difftest.normalise((FIXTURES / f"{name}.opt.ll").read_text())
    mine, llvm = difftest.functions(ours)["f"], difftest.functions(theirs)["f"]
    a, b = difftest.canonical(mine), difftest.canonical(llvm)
    return {"identical": a == b, "llrm_only": dict(difftest.histogram(a) - difftest.histogram(b)), "llvm_only": dict(difftest.histogram(b) - difftest.histogram(a))}


@pytest.mark.parametrize("name", sorted(EXPECTED))
def test_a_pinned_pair_counts_as_its_known_answer(name):
    """cc 1002 / cc1002 and 0x40445EB860000000 / 4.074000e+01 are spellings (a 64 vs 109 of 120 'identical' came from them); a missing load is a difference."""
    want = EXPECTED[name]
    got = answer(name)
    assert got["identical"] == want["identical"], got
    if not want["identical"]:
        assert got["llrm_only"] == want["llrm_only"] and got["llvm_only"] == want["llvm_only"], got
