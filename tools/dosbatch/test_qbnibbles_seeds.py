"""NIBBLES's death screen against BCOM45's at several fixed RND seeds and virtual-time start times, at m16 and m32 (differential), and a build
that is wrong on purpose, which must be caught: a comparison that cannot fail proves nothing."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

REQUIRES = ["qb45"]

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qb32  # noqa: E402
import qb32demos  # noqa: E402
import qbbuild  # noqa: E402
import qbruntime  # noqa: E402

pytestmark = pytest.mark.slow_dos

SEEDS = (7, 99, 31337)
STARTS = (946684800, 946697400)  # at the real timer these two start times gave 38 px and 34 px at m16 (run 7)


@pytest.fixture(scope="module")
def nibbles(tmp_path_factory):
    found, reason = qbruntime.demo_sources()
    sources = {path.stem.upper(): path for path in found.values()}
    if "NIBBLES" not in sources or not dosbatch.QB45.is_dir():
        pytest.skip(reason or "QB45_DIR is unavailable")
    work = tmp_path_factory.mktemp("nibseeds")
    return sources["NIBBLES"], work, qbbuild.runtimes(work)


def conf(start: int) -> str:
    return qb32.DEMO_CONF.replace("[cpu]", f"[cpu]\nvirtual time start={start}", 1)


@pytest.mark.parametrize("seed", SEEDS)
def test_the_death_screen_matches_bcom45_at_each_fixed_seed(nibbles, seed):
    source, work, objects = nibbles
    seeded = qb32demos.seeded(source, seed, work)
    shown = qb32demos.compare("NIBBLES", seeded, seeded, work / f"seed{seed}", objects)
    assert shown and all(wrong == [] for wrong in shown.values()), shown


@pytest.mark.parametrize("start", STARTS)
def test_the_death_screen_matches_bcom45_at_each_start_time(nibbles, start):
    source, work, objects = nibbles
    seeded = qb32demos.seeded(source, 1234, work)
    shown = qb32demos.compare("NIBBLES", seeded, seeded, work / f"start{start}", objects, conf(start))
    assert shown and all(wrong == [] for wrong in shown.values()), shown


def test_a_build_that_loses_a_life_is_caught(nibbles):
    """Negative control: the candidate's source never takes a life, so its death screen says `Lives: 5` where BCOM45's says 4."""
    source, work, objects = nibbles
    seeded = qb32demos.seeded(source, 1234, work)
    text = seeded.read_bytes()
    assert text.count(b"sammy(a).lives = sammy(a).lives - 1") == 1
    broken = work / "broken" / source.name
    broken.parent.mkdir(exist_ok=True)
    broken.write_bytes(text.replace(b"sammy(a).lives = sammy(a).lives - 1", b"sammy(a).lives = sammy(a).lives - 0"))
    shown = qb32demos.compare("NIBBLES", seeded, broken, work / "broken-run", objects)
    assert all(any(w.startswith("dies:") for w in wrong) for wrong in shown.values()), shown
