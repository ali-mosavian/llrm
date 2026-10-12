"""NIBBLES and GORILLA played with typed keys under DOSBox-X on virtual time (qb32demos.py), as BCOM45 builds them (the oracle) and
with the llrm runtime at m16 and at m32: at each point where the screen holds nothing random the screenshots must agree, but
for the blink of the cursor.  What a game draws from RND (the skyline, the sparkles, where a number lies) is not compared.
Each target plays the introduction, the setup and a game (a snake into the wall, bananas thrown)."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

# BCOM45 (BC, LINK and its runtime) is the oracle these tests compare against: the gate drops the file where QB45 is not installed.
REQUIRES = ["qb45"]

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qb32demos  # noqa: E402
import qbbuild  # noqa: E402
import qbruntime  # noqa: E402

pytestmark = pytest.mark.slow_dos


@pytest.fixture(scope="module")
def differences(tmp_path_factory):
    found, reason = qbruntime.demo_sources()
    if not found or not dosbatch.QB45.is_dir():
        pytest.skip(reason or "QB45_DIR is unavailable")
    return qb32demos.run({path.stem.upper(): path for path in found.values()}, tmp_path_factory.mktemp("qbdemos"))


@pytest.mark.parametrize("name", ["NIBBLES", "GORILLA"])
def test_the_demo_shows_what_bcom45_shows_where_nothing_is_random(differences, name: str):
    assert len(differences[name]) == len(qbbuild.TARGETS)
    for target, wrong in differences[name].items():
        assert wrong == [], f"{target}: {wrong}"
